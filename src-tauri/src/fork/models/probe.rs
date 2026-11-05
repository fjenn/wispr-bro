// SPDX-License-Identifier: GPL-3.0-or-later
//! One look at the pinned model URL after a failed transfer (S-07a).
//!
//! Handy's downloader (hf-hub) reports an error but not the status or
//! headers behind it, so after a failed transfer the fork asks the same
//! immutable resolve URL for its first byte (`Range: bytes=0-0`) and
//! classifies the answer ([`classify`]): offline, a captive portal (a web
//! page where the file should be, or a redirect off the allowed hosts),
//! rate limited (429/503 with `Retry-After`), another server error, or
//! reachable. Redirects are followed only to hosts on the R5 §1 list; the
//! request carries no token, cookie or identifying header.

use std::time::Duration;

use super::download::{parse_retry_after, Probe};

/// The whole probe, connect included.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// What came back, reduced to what [`classify`] needs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Seen {
    pub status: u16,
    pub content_type: Option<String>,
    pub retry_after: Option<String>,
}

/// Why no response came back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoResponse {
    /// DNS, connect, TLS or timeout.
    Network,
    /// The URL (or a redirect) left the allowed hosts.
    Disallowed,
}

/// The verdict for one probe at `now_unix` (for `Retry-After` dates).
pub fn classify(seen: Result<Seen, NoResponse>, now_unix: i64) -> Probe {
    let seen = match seen {
        Ok(s) => s,
        Err(NoResponse::Network) => return Probe::Offline,
        Err(NoResponse::Disallowed) => return Probe::CaptivePortal,
    };
    let html = seen
        .content_type
        .as_deref()
        .is_some_and(|c| c.to_ascii_lowercase().contains("text/html"));
    match seen.status {
        429 | 503 => Probe::RateLimited {
            retry_after: seen
                .retry_after
                .as_deref()
                .and_then(|v| parse_retry_after(v, now_unix)),
        },
        // A redirect we refused to follow (off the allowed hosts), or a
        // proxy asking to log in.
        300..=399 | 407 | 511 => Probe::CaptivePortal,
        // The file is binary; a page in its place is a login page.
        200..=299 if html => Probe::CaptivePortal,
        200..=299 => Probe::Reachable,
        // Portals that answer every URL with their own 401/403 page.
        401 | 403 if html => Probe::CaptivePortal,
        _ => Probe::ServerError,
    }
}

/// Probes `url` (which must pass `allowed`). `allowed` also gates every
/// redirect; production passes [`crate::fork::net::is_allowed_host`].
pub async fn probe(url: &str, allowed: fn(&str) -> bool) -> Probe {
    let now = chrono::Utc::now().timestamp();
    classify(fetch(url, allowed).await, now)
}

async fn fetch(url: &str, allowed: fn(&str) -> bool) -> Result<Seen, NoResponse> {
    if !allowed(url) {
        return Err(NoResponse::Disallowed);
    }
    let policy = reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() >= 5 || !allowed(attempt.url().as_str()) {
            attempt.stop()
        } else {
            attempt.follow()
        }
    });
    let client = reqwest::Client::builder()
        .redirect(policy)
        .timeout(PROBE_TIMEOUT)
        .connect_timeout(PROBE_TIMEOUT)
        .user_agent(concat!("wispr-bro/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| NoResponse::Network)?;
    let response = client
        .get(url)
        .header(reqwest::header::RANGE, "bytes=0-0")
        .send()
        .await
        .map_err(|e| {
            log::debug!("models: probe error: {}", e.without_url());
            NoResponse::Network
        })?;
    let header = |name: reqwest::header::HeaderName| {
        response
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    };
    Ok(Seen {
        status: response.status().as_u16(),
        content_type: header(reqwest::header::CONTENT_TYPE),
        retry_after: header(reqwest::header::RETRY_AFTER),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn seen(status: u16, ct: Option<&str>, ra: Option<&str>) -> Result<Seen, NoResponse> {
        Ok(Seen {
            status,
            content_type: ct.map(str::to_owned),
            retry_after: ra.map(str::to_owned),
        })
    }

    #[test]
    fn classifies_every_answer() {
        assert_eq!(classify(Err(NoResponse::Network), 0), Probe::Offline);
        assert_eq!(
            classify(Err(NoResponse::Disallowed), 0),
            Probe::CaptivePortal
        );
        assert_eq!(
            classify(seen(206, Some("application/octet-stream"), None), 0),
            Probe::Reachable
        );
        assert_eq!(classify(seen(200, None, None), 0), Probe::Reachable);
        assert_eq!(
            classify(seen(200, Some("text/html; charset=utf-8"), None), 0),
            Probe::CaptivePortal
        );
        assert_eq!(classify(seen(302, None, None), 0), Probe::CaptivePortal);
        assert_eq!(classify(seen(511, None, None), 0), Probe::CaptivePortal);
        assert_eq!(classify(seen(407, None, None), 0), Probe::CaptivePortal);
        assert_eq!(
            classify(seen(403, Some("text/html"), None), 0),
            Probe::CaptivePortal
        );
        assert_eq!(
            classify(seen(403, Some("application/json"), None), 0),
            Probe::ServerError
        );
        assert_eq!(
            classify(seen(429, None, Some("30")), 0),
            Probe::RateLimited {
                retry_after: Some(Duration::from_secs(30))
            }
        );
        assert_eq!(
            classify(seen(503, Some("text/html"), None), 0),
            Probe::RateLimited { retry_after: None }
        );
        assert_eq!(classify(seen(500, None, None), 0), Probe::ServerError);
        assert_eq!(classify(seen(404, None, None), 0), Probe::ServerError);
    }

    /// A one-shot HTTP server on loopback answering every connection with
    /// `response`; returns its base URL.
    fn serve(responses: Vec<String>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for response in responses {
                let Ok((mut s, _)) = listener.accept() else {
                    return;
                };
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let _ = s.write_all(response.as_bytes());
            }
        });
        format!("http://{addr}")
    }

    fn loopback_only(url: &str) -> bool {
        crate::fork::net::host_of(url).as_deref() == Some("127.0.0.1")
    }

    fn reply(status: &str, headers: &[&str], body: &str) -> String {
        let mut r = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\n", body.len());
        for h in headers {
            r.push_str(h);
            r.push_str("\r\n");
        }
        r.push_str("Connection: close\r\n\r\n");
        r.push_str(body);
        r
    }

    #[tokio::test]
    async fn real_http_answers() {
        let base = serve(vec![reply(
            "206 Partial Content",
            &[
                "Content-Type: application/octet-stream",
                "Content-Range: bytes 0-0/9",
            ],
            "G",
        )]);
        assert_eq!(
            probe(&format!("{base}/m.gguf"), loopback_only).await,
            Probe::Reachable
        );

        let base = serve(vec![reply(
            "429 Too Many Requests",
            &["Retry-After: 17"],
            "",
        )]);
        assert_eq!(
            probe(&format!("{base}/m.gguf"), loopback_only).await,
            Probe::RateLimited {
                retry_after: Some(Duration::from_secs(17))
            }
        );

        let base = serve(vec![reply(
            "200 OK",
            &["Content-Type: text/html"],
            "<html><body>Log in to CoffeeShop Wi-Fi</body></html>",
        )]);
        assert_eq!(
            probe(&format!("{base}/m.gguf"), loopback_only).await,
            Probe::CaptivePortal
        );

        // A redirect to a host that is not allowed is not followed.
        let base = serve(vec![reply(
            "302 Found",
            &["Location: http://portal.example.com/login"],
            "",
        )]);
        assert_eq!(
            probe(&format!("{base}/m.gguf"), loopback_only).await,
            Probe::CaptivePortal
        );

        // A redirect inside the allowed hosts is followed.
        let target = serve(vec![reply(
            "206 Partial Content",
            &["Content-Type: application/octet-stream"],
            "G",
        )]);
        let base = serve(vec![reply(
            "302 Found",
            &[&format!("Location: {target}/blob")],
            "",
        )]);
        assert_eq!(
            probe(&format!("{base}/m.gguf"), loopback_only).await,
            Probe::Reachable
        );

        let base = serve(vec![reply("500 Internal Server Error", &[], "")]);
        assert_eq!(
            probe(&format!("{base}/m.gguf"), loopback_only).await,
            Probe::ServerError
        );
    }

    #[tokio::test]
    async fn nothing_listening_is_offline_and_bad_hosts_are_refused() {
        // Bind and drop: the port is closed.
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        assert_eq!(
            probe(&format!("http://127.0.0.1:{port}/m.gguf"), loopback_only).await,
            Probe::Offline
        );
        // Never contacted: the URL itself is not allowed.
        assert_eq!(
            probe(
                "https://example.com/m.gguf",
                crate::fork::net::is_allowed_host
            )
            .await,
            Probe::CaptivePortal
        );
    }
}
