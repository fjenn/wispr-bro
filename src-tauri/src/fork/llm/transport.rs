// SPDX-License-Identifier: GPL-3.0-or-later
//! How the app reaches `llama-server` (DEC-49).
//!
//! - **Unix socket** `<app data>/llm/llm.sock` (RS-06/RS-07: b11624 serves
//!   `/health` and chat completions on `--host <path>.sock`, and opens no inet
//!   socket). The socket file is created by the server with mode 755, so only
//!   the user can connect (`connect(2)` needs write permission).
//! - **Loopback TCP** on a random free port when the socket path does not fit
//!   `sockaddr_un` (104 bytes on macOS, e.g. a long portable-mode path).
//!
//! Both carry a random API key (`LLAMA_API_KEY` in the server's env, `Bearer`
//! on every request) and are loopback only, so AGPL §13 does not apply and
//! nothing is reachable from the network.
//!
//! Every request uses the base URL [`BASE_URL`] (`http://localhost`): with a
//! Unix socket reqwest ignores the host; with TCP the client resolves
//! `localhost` to `127.0.0.1:<port>`. So one URL works for both, including
//! Handy's own `llm_client` chain through the `client_builder` hook.

use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};

/// Base URL of every request to the sidecar. Loopback by name, so the
/// F-08 refusal in `client_builder` passes it.
pub const BASE_URL: &str = "http://localhost";

/// File name of the socket inside the LLM dir.
pub const SOCKET_FILE: &str = "llm.sock";

/// `sizeof(sockaddr_un.sun_path)` on macOS, including the NUL.
const SUN_PATH_MAX: usize = 104;

/// Where the server listens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Endpoint {
    Unix(PathBuf),
    Tcp(u16),
}

impl Endpoint {
    /// `--host` (and `--port`) arguments for `llama-server`.
    pub fn server_args(&self) -> Vec<String> {
        match self {
            Endpoint::Unix(path) => vec!["--host".into(), path.to_string_lossy().into_owned()],
            Endpoint::Tcp(port) => vec![
                "--host".into(),
                Ipv4Addr::LOCALHOST.to_string(),
                "--port".into(),
                port.to_string(),
            ],
        }
    }

    /// Short label for logs and the status event (no path).
    pub fn kind(&self) -> &'static str {
        match self {
            Endpoint::Unix(_) => "unix",
            Endpoint::Tcp(_) => "tcp",
        }
    }
}

/// `true` when `path` fits in `sockaddr_un` (and ends in `.sock`, which is
/// how `llama-server` tells a socket path from an address).
pub fn socket_path_fits(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().len() < SUN_PATH_MAX
        && path.extension().is_some_and(|e| e == "sock")
}

/// DEC-49: the socket in `llm_dir` if it fits, else loopback TCP on a free
/// port.
pub fn choose(llm_dir: &Path) -> std::io::Result<Endpoint> {
    let socket = llm_dir.join(SOCKET_FILE);
    if socket_path_fits(&socket) {
        Ok(Endpoint::Unix(socket))
    } else {
        Ok(Endpoint::Tcp(free_loopback_port()?))
    }
}

/// A port that was free a moment ago. The server may still lose a bind race;
/// the supervisor then picks a new port and retries.
pub fn free_loopback_port() -> std::io::Result<u16> {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
    Ok(listener.local_addr()?.port())
}

/// 32 random bytes as hex, from the OS RNG (`getentropy`).
pub fn random_api_key() -> std::io::Result<String> {
    let mut buf = [0u8; 32];
    // SAFETY: getentropy writes at most 256 bytes into the buffer we own.
    let rc = unsafe { libc::getentropy(buf.as_mut_ptr().cast(), buf.len()) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}

/// Endpoint + key of one running server.
#[derive(Clone, PartialEq, Eq)]
pub struct Transport {
    pub endpoint: Endpoint,
    api_key: String,
}

impl std::fmt::Debug for Transport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the key.
        f.debug_struct("Transport")
            .field("endpoint", &self.endpoint.kind())
            .finish_non_exhaustive()
    }
}

impl Transport {
    pub fn new(endpoint: Endpoint, api_key: String) -> Self {
        Self { endpoint, api_key }
    }

    /// The key for the server's `LLAMA_API_KEY`.
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// `Authorization: Bearer <key>`.
    pub fn auth_header(&self) -> HeaderValue {
        let mut value = HeaderValue::from_str(&format!("Bearer {}", self.api_key))
            .unwrap_or_else(|_| HeaderValue::from_static(""));
        value.set_sensitive(true);
        value
    }

    /// Points `builder` at this server: Unix socket or `localhost` resolved
    /// to the loopback port, the bearer key, no proxy. Used by the fork's own
    /// client and by Handy's chain (`fork::llm::client_builder`).
    pub fn configure(&self, builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, self.auth_header());
        let builder = builder.no_proxy().default_headers(headers);
        match &self.endpoint {
            Endpoint::Unix(path) => builder.unix_socket(path.clone()),
            Endpoint::Tcp(port) => {
                builder.resolve("localhost", SocketAddr::from((Ipv4Addr::LOCALHOST, *port)))
            }
        }
    }

    /// A ready client for this server. `timeout` bounds every request.
    pub fn client(&self, timeout: Duration) -> reqwest::Result<reqwest::Client> {
        self.configure(reqwest::Client::builder())
            .connect_timeout(Duration::from_secs(2))
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
    }

    /// `http://localhost<path>`.
    pub fn url(path: &str) -> String {
        format!("{BASE_URL}{path}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Reads one HTTP request head and answers 200 `{"status":"ok"}`.
    async fn answer_once<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>(
        mut stream: S,
        seen: Arc<Mutex<String>>,
    ) {
        let mut buf = vec![0u8; 4096];
        let mut head = Vec::new();
        while !head.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = stream.read(&mut buf).await.unwrap();
            if n == 0 {
                break;
            }
            head.extend_from_slice(&buf[..n]);
        }
        *seen.lock().unwrap() = String::from_utf8_lossy(&head).into_owned();
        let body = r#"{"status":"ok"}"#;
        let resp = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(resp.as_bytes()).await.unwrap();
    }

    fn header<'a>(head: &'a str, name: &str) -> Option<&'a str> {
        head.lines().find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.eq_ignore_ascii_case(name).then(|| v.trim())
        })
    }

    #[tokio::test]
    async fn unix_socket_transport_reaches_the_server_with_the_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.sock");
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        let seen = Arc::new(Mutex::new(String::new()));
        let server = tokio::spawn({
            let seen = seen.clone();
            async move {
                let (s, _) = listener.accept().await.unwrap();
                answer_once(s, seen).await;
            }
        });
        let t = Transport::new(Endpoint::Unix(path), "k3y".into());
        let resp = t
            .client(Duration::from_secs(5))
            .unwrap()
            .get(Transport::url("/health"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        server.await.unwrap();
        let head = seen.lock().unwrap().clone();
        assert!(head.starts_with("GET /health HTTP/1.1"), "{head}");
        assert_eq!(header(&head, "authorization"), Some("Bearer k3y"));
        assert_eq!(header(&head, "user-agent"), None);
    }

    #[tokio::test]
    async fn loopback_transport_resolves_localhost_to_the_port() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(String::new()));
        let server = tokio::spawn({
            let seen = seen.clone();
            async move {
                let (s, _) = listener.accept().await.unwrap();
                answer_once(s, seen).await;
            }
        });
        let t = Transport::new(Endpoint::Tcp(port), "k".into());
        let resp = t
            .client(Duration::from_secs(5))
            .unwrap()
            .get(Transport::url("/health"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        server.await.unwrap();
        let head = seen.lock().unwrap().clone();
        assert_eq!(header(&head, "authorization"), Some("Bearer k"));
    }

    #[test]
    fn long_socket_paths_fall_back_to_loopback_tcp() {
        let short = Path::new("/Users/a/Library/Application Support/com.wisprbro.desktop/llm");
        assert!(matches!(choose(short).unwrap(), Endpoint::Unix(_)));
        let long = PathBuf::from(format!("/Volumes/{}/Data/llm", "x".repeat(100)));
        assert!(matches!(choose(&long).unwrap(), Endpoint::Tcp(p) if p > 0));
        assert!(!socket_path_fits(Path::new("/tmp/x.sok")));
        assert!(socket_path_fits(Path::new("/tmp/x.sock")));
        // 103 bytes + NUL fits, 104 does not.
        let p = |n: usize| PathBuf::from(format!("/{}.sock", "a".repeat(n - 6)));
        assert!(socket_path_fits(&p(103)));
        assert!(!socket_path_fits(&p(104)));
    }

    #[test]
    fn server_args_are_loopback_only() {
        assert_eq!(
            Endpoint::Tcp(5).server_args(),
            ["--host", "127.0.0.1", "--port", "5"]
        );
        assert_eq!(
            Endpoint::Unix("/x/llm.sock".into()).server_args(),
            ["--host", "/x/llm.sock"]
        );
        assert!(crate::fork::net::is_loopback(&Transport::url("/v1")));
    }

    #[test]
    fn api_keys_are_random_and_never_printed() {
        let a = random_api_key().unwrap();
        let b = random_api_key().unwrap();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
        let t = Transport::new(Endpoint::Tcp(1), a.clone());
        assert!(!format!("{t:?}").contains(&a));
        assert!(t.auth_header().is_sensitive());
    }
}
