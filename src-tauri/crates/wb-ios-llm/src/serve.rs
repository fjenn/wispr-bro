// SPDX-License-Identifier: GPL-3.0-or-later
//! The engine behind the sidecar's own front door: a listener inside the app
//! that answers `GET /health` and `POST /v1/chat/completions` on a Unix
//! socket (or a loopback port) exactly as `llama-server` does, so the app's
//! supervisor, transport, health check and chat client
//! (`fork/llm/{supervisor,transport,health,chat}.rs`) work unchanged: on
//! iPhone "start the server" is a thread instead of a process.
//!
//! Like the sidecar: `/health` is 503 while the model loads and 200 after;
//! every other route needs the bearer key; a client that hangs up (Esc, a
//! deadline in the app) stops the generation at the next token.
//!
//! Only what the app's client uses is implemented: one request per
//! connection (`Connection: close`), `Content-Length` request bodies, a
//! `Content-Length` or chunked response.

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;

use crate::{Config, Engine, Error, Stop};

/// Where to listen (`fork/llm/transport.rs` `Endpoint`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Listen {
    /// A Unix socket file. The path must fit `sockaddr_un` (104 bytes).
    Unix(PathBuf),
    /// `127.0.0.1:<port>`.
    Tcp(u16),
}

/// What the server is doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// Listening; the model is loading (`/health` answers 503).
    Loading,
    /// Serving.
    Ready,
    /// The model did not load; the message is llama.cpp's.
    Failed(String),
    /// Stopped (by [`Server::stop`] or drop).
    Stopped,
}

const MAX_HEAD: usize = 64 * 1024;
const MAX_BODY: usize = 16 * 1024 * 1024;

struct Shared {
    engine: RwLock<Option<Engine>>,
    state: Mutex<State>,
    stopping: AtomicBool,
    api_key: Option<String>,
    /// Cancel flags of the requests in flight.
    in_flight: Mutex<Vec<Arc<AtomicBool>>>,
}

impl Shared {
    fn state(&self) -> State {
        lock(&self.state).clone()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// A listening engine. Dropping it stops the listener, cancels requests in
/// flight and unloads the model.
pub struct Server {
    shared: Arc<Shared>,
    listen: Listen,
    accept: Option<JoinHandle<()>>,
    loader: Option<JoinHandle<()>>,
}

impl Server {
    /// Binds the listener (so `/health` answers at once), then loads the
    /// model on its own thread. `api_key`: the bearer key every route but
    /// `/health` requires (the app's `Transport::api_key`).
    pub fn start(listen: Listen, api_key: Option<String>, config: Config) -> io::Result<Self> {
        let listener = match &listen {
            Listen::Unix(path) => {
                // A socket file left by a crashed run.
                let _ = std::fs::remove_file(path);
                Listener::Unix(UnixListener::bind(path)?)
            }
            Listen::Tcp(port) => Listener::Tcp(TcpListener::bind(SocketAddr::from((
                Ipv4Addr::LOCALHOST,
                *port,
            )))?),
        };
        let shared = Arc::new(Shared {
            engine: RwLock::new(None),
            state: Mutex::new(State::Loading),
            stopping: AtomicBool::new(false),
            api_key,
            in_flight: Mutex::new(Vec::new()),
        });
        let s = shared.clone();
        let loader = std::thread::Builder::new()
            .name("wb-llm-load".into())
            .spawn(move || {
                let loaded = Engine::load(&config);
                // Stopped while loading: drop the engine again.
                if s.stopping.load(Ordering::SeqCst) {
                    return;
                }
                match loaded {
                    Ok(engine) => {
                        *s.engine.write().unwrap_or_else(|p| p.into_inner()) = Some(engine);
                        *lock(&s.state) = State::Ready;
                    }
                    Err(e) => *lock(&s.state) = State::Failed(e.to_string()),
                }
            })?;
        let s = shared.clone();
        let accept = std::thread::Builder::new()
            .name("wb-llm-accept".into())
            .spawn(move || accept_loop(listener, s))?;
        Ok(Self {
            shared,
            listen,
            accept: Some(accept),
            loader: Some(loader),
        })
    }

    pub fn state(&self) -> State {
        self.shared.state()
    }

    /// A handle that reports the state from another thread.
    pub fn watch(&self) -> impl Fn() -> State + Send + 'static {
        let shared = self.shared.clone();
        move || shared.state()
    }

    /// Tensor bytes of the loaded model, once ready.
    pub fn model_bytes(&self) -> Option<u64> {
        self.shared
            .engine
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .map(Engine::model_bytes)
    }

    /// Stops listening, cancels the requests in flight, unloads the model.
    /// Blocks until the model is freed (a request in flight ends at its next
    /// token; a load in progress finishes first).
    pub fn stop(&mut self) {
        if self.shared.stopping.swap(true, Ordering::SeqCst) {
            return;
        }
        for flag in lock(&self.shared.in_flight).iter() {
            flag.store(true, Ordering::SeqCst);
        }
        // Wake the accept loop.
        match &self.listen {
            Listen::Unix(path) => drop(UnixStream::connect(path)),
            Listen::Tcp(port) => drop(TcpStream::connect(SocketAddr::from((
                Ipv4Addr::LOCALHOST,
                *port,
            )))),
        }
        if let Some(t) = self.accept.take() {
            let _ = t.join();
        }
        if let Some(t) = self.loader.take() {
            let _ = t.join();
        }
        // Waits for the readers (requests) to finish, then frees the model.
        drop(
            self.shared
                .engine
                .write()
                .unwrap_or_else(|p| p.into_inner())
                .take(),
        );
        if let Listen::Unix(path) = &self.listen {
            let _ = std::fs::remove_file(path);
        }
        *lock(&self.shared.state) = State::Stopped;
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
    }
}

enum Listener {
    Unix(UnixListener),
    Tcp(TcpListener),
}

trait Conn: Read + Write + Send {
    fn fd(&self) -> RawFd;
    fn close(&self);
}

impl Conn for UnixStream {
    fn fd(&self) -> RawFd {
        self.as_raw_fd()
    }
    fn close(&self) {
        let _ = self.shutdown(Shutdown::Both);
    }
}

impl Conn for TcpStream {
    fn fd(&self) -> RawFd {
        self.as_raw_fd()
    }
    fn close(&self) {
        let _ = self.shutdown(Shutdown::Both);
    }
}

fn accept_loop(listener: Listener, shared: Arc<Shared>) {
    loop {
        let conn: io::Result<Box<dyn Conn>> = match &listener {
            Listener::Unix(l) => l.accept().map(|(s, _)| Box::new(s) as Box<dyn Conn>),
            Listener::Tcp(l) => l.accept().map(|(s, _)| {
                let _ = s.set_nodelay(true);
                Box::new(s) as Box<dyn Conn>
            }),
        };
        if shared.stopping.load(Ordering::SeqCst) {
            return;
        }
        let Ok(conn) = conn else {
            std::thread::sleep(std::time::Duration::from_millis(20));
            continue;
        };
        no_sigpipe(conn.fd());
        let s = shared.clone();
        let _ = std::thread::Builder::new()
            .name("wb-llm-conn".into())
            .spawn(move || {
                let mut conn = conn;
                let _ = handle(&mut *conn, &s);
                conn.close();
            });
    }
}

struct Request {
    method: String,
    path: String,
    authorization: Option<String>,
    body: String,
}

fn read_request(conn: &mut dyn Conn) -> io::Result<Request> {
    let bad = |m: &str| io::Error::new(io::ErrorKind::InvalidData, m.to_owned());
    let mut buf: Vec<u8> = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break at;
        }
        if buf.len() > MAX_HEAD {
            return Err(bad("request head too large"));
        }
        let n = conn.read(&mut chunk)?;
        if n == 0 {
            return Err(bad("connection closed before a request"));
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or_default().split_whitespace();
    let method = first.next().unwrap_or_default().to_owned();
    let target = first.next().unwrap_or_default();
    let path = target.split('?').next().unwrap_or_default().to_owned();
    let (mut length, mut authorization) = (0usize, None);
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("content-length") {
            length = value
                .trim()
                .parse()
                .map_err(|_| bad("bad content-length"))?;
        } else if name.eq_ignore_ascii_case("authorization") {
            authorization = Some(value.trim().to_owned());
        }
    }
    if length > MAX_BODY {
        return Err(bad("request body too large"));
    }
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < length {
        let n = conn.read(&mut chunk)?;
        if n == 0 {
            return Err(bad("connection closed in the body"));
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(length);
    Ok(Request {
        method,
        path,
        authorization,
        body: String::from_utf8(body).map_err(|_| bad("body is not UTF-8"))?,
    })
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

fn write_plain(conn: &mut dyn Conn, status: u16, body: &[u8]) -> io::Result<()> {
    write!(
        conn,
        "HTTP/1.1 {status} {}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        reason(status),
        body.len()
    )?;
    conn.write_all(body)?;
    conn.flush()
}

/// llama-server's error body (`format_error_response`).
fn error_body(message: &str, kind: &str, code: u16) -> Vec<u8> {
    format!(r#"{{"error":{{"code":{code},"message":"{message}","type":"{kind}"}}}}"#).into_bytes()
}

fn handle(conn: &mut dyn Conn, shared: &Shared) -> io::Result<()> {
    let request = read_request(conn)?;
    let health = matches!(request.path.as_str(), "/health" | "/v1/health");
    // llama-server's middleware order: the key first (public routes aside),
    // then the server state.
    if !health {
        if let Some(key) = &shared.api_key {
            if request.authorization.as_deref() != Some(&format!("Bearer {key}")) {
                return write_plain(
                    conn,
                    401,
                    &error_body("Invalid API Key", "authentication_error", 401),
                );
            }
        }
    }
    match shared.state() {
        State::Ready => {}
        State::Loading => {
            return write_plain(
                conn,
                503,
                &error_body("Loading model", "unavailable_error", 503),
            )
        }
        State::Failed(_) | State::Stopped => {
            return write_plain(
                conn,
                500,
                &error_body("The model is not loaded", "server_error", 500),
            )
        }
    }
    if !matches!(request.method.as_str(), "GET" | "POST") {
        return write_plain(
            conn,
            404,
            &error_body("File Not Found", "not_found_error", 404),
        );
    }

    let fd = conn.fd();
    let write_failed = Arc::new(AtomicBool::new(false));
    let failed = write_failed.clone();
    let stop =
        Stop::default().with_probe(move || failed.load(Ordering::Relaxed) || peer_closed(fd));
    let flag = stop.flag();
    lock(&shared.in_flight).push(flag.clone());
    let guard = shared.engine.read().unwrap_or_else(|p| p.into_inner());
    let Some(engine) = guard.as_ref() else {
        return write_plain(
            conn,
            500,
            &error_body("The model is not loaded", "server_error", 500),
        );
    };

    // Head and body arrive through two callbacks that both write to `conn`.
    let conn = std::cell::RefCell::new(conn);
    let head: std::cell::Cell<Option<(u16, bool)>> = std::cell::Cell::new(None);
    let wrote_plain = std::cell::Cell::new(false);
    let fail = |r: io::Result<()>| {
        if r.is_err() {
            write_failed.store(true, Ordering::Relaxed);
        }
    };
    let result = engine.request_with(
        &request.path,
        &request.body,
        &stop,
        |status, is_stream| {
            head.set(Some((status, is_stream)));
            if is_stream {
                let mut c = conn.borrow_mut();
                fail(write!(
                    c,
                    "HTTP/1.1 {status} {}\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
                    reason(status)
                ));
            }
        },
        |bytes| {
            let mut c = conn.borrow_mut();
            match head.get() {
                Some((_, true)) => {
                    fail((|| {
                        write!(c, "{:x}\r\n", bytes.len())?;
                        c.write_all(bytes)?;
                        c.write_all(b"\r\n")?;
                        c.flush()
                    })());
                }
                Some((status, false)) => {
                    wrote_plain.set(true);
                    fail(write_plain(&mut **c, status, bytes));
                }
                None => {}
            }
        },
    );
    drop(guard);
    lock(&shared.in_flight).retain(|f| !Arc::ptr_eq(f, &flag));
    let conn = conn.into_inner();
    match (result, head.get()) {
        (Err(Error::UnknownPath(_)), _) => write_plain(
            conn,
            404,
            &error_body("File Not Found", "not_found_error", 404),
        ),
        // Stopped before a response: the client is gone.
        (Err(_), _) | (Ok(_), None) => Ok(()),
        (Ok(_), Some((_, true))) => {
            // A stream that ended on its own gets its last chunk; one that
            // was stopped is simply closed, as when the sidecar is cut off.
            if !stop.should_stop() {
                conn.write_all(b"0\r\n\r\n")?;
                conn.flush()?;
            }
            Ok(())
        }
        (Ok(status), Some((_, false))) => {
            if !wrote_plain.get() {
                write_plain(conn, status, b"")?;
            }
            Ok(())
        }
    }
}

/// `true` once the peer has closed its end (nothing to read and EOF).
fn peer_closed(fd: RawFd) -> bool {
    const MSG_PEEK: i32 = 0x2;
    const MSG_DONTWAIT: i32 = 0x80;
    let mut byte = 0u8;
    // SAFETY: a one-byte buffer; the fd is the connection's, open for the
    // duration of the request.
    let n = unsafe {
        recv(
            fd,
            (&mut byte as *mut u8).cast(),
            1,
            MSG_PEEK | MSG_DONTWAIT,
        )
    };
    // 0: orderly shutdown. -1 with EAGAIN: still open, nothing sent.
    n == 0
        || (n < 0
            && !matches!(
                io::Error::last_os_error().kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ))
}

/// A write to a closed connection must be an error, never SIGPIPE (the app
/// does not ignore that signal the way a Rust `main` does).
fn no_sigpipe(fd: RawFd) {
    #[cfg(target_vendor = "apple")]
    {
        const SOL_SOCKET: i32 = 0xffff;
        const SO_NOSIGPIPE: i32 = 0x1022;
        let on: i32 = 1;
        // SAFETY: a valid socket and a 4-byte option value.
        unsafe {
            setsockopt(
                fd,
                SOL_SOCKET,
                SO_NOSIGPIPE,
                (&on as *const i32).cast(),
                std::mem::size_of::<i32>() as u32,
            );
        }
    }
    #[cfg(not(target_vendor = "apple"))]
    let _ = fd;
}

extern "C" {
    fn recv(fd: i32, buf: *mut std::os::raw::c_void, len: usize, flags: i32) -> isize;
    #[cfg(target_vendor = "apple")]
    fn setsockopt(
        fd: i32,
        level: i32,
        name: i32,
        value: *const std::os::raw::c_void,
        len: u32,
    ) -> i32;
}
