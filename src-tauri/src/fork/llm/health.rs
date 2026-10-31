// SPDX-License-Identifier: GPL-3.0-or-later
//! `/health` polling (R5 §4.5). `llama-server` answers 503 while it loads the
//! model and 200 `{"status":"ok"}` once it can serve.

use std::time::{Duration, Instant};

use super::transport::Transport;

/// One probe's result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    /// 200: ready for requests.
    Ok,
    /// 503 (loading the model) or any other HTTP answer: alive, not ready.
    Loading,
    /// No answer (not listening yet, or gone).
    Down,
}

/// Interval between probes while waiting.
pub const POLL: Duration = Duration::from_millis(25);

/// Probes `/health` once.
pub async fn probe(client: &reqwest::Client) -> Health {
    match client
        .get(Transport::url("/health"))
        .timeout(Duration::from_secs(2))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => Health::Ok,
        Ok(_) => Health::Loading,
        Err(_) => Health::Down,
    }
}

/// Why waiting for health ended without `Ok`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaitError {
    /// The process exited while loading (crash, bad model, bind failure).
    Exited,
    /// Still not ready after the timeout.
    Timeout,
}

/// Polls until `/health` is OK, the process dies (`alive` returns false) or
/// `timeout` passes. Returns the time it took.
pub async fn wait_ready(
    client: &reqwest::Client,
    timeout: Duration,
    mut alive: impl FnMut() -> bool,
) -> Result<Duration, WaitError> {
    let t0 = Instant::now();
    loop {
        if !alive() {
            return Err(WaitError::Exited);
        }
        if probe(client).await == Health::Ok {
            return Ok(t0.elapsed());
        }
        if t0.elapsed() >= timeout {
            return Err(WaitError::Timeout);
        }
        tokio::time::sleep(POLL).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::llm::transport::Endpoint;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// A server that answers 503 `loading_n` times, then 200.
    async fn fake_server(loading_n: u32) -> (u16, Arc<AtomicU32>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let hits = Arc::new(AtomicU32::new(0));
        let h = hits.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut s, _)) = listener.accept().await else {
                    return;
                };
                let n = h.fetch_add(1, Ordering::SeqCst);
                let mut buf = [0u8; 1024];
                let _ = s.read(&mut buf).await;
                let (code, body) = if n < loading_n {
                    (
                        "503 Service Unavailable",
                        r#"{"error":{"code":503,"message":"Loading model"}}"#,
                    )
                } else {
                    ("200 OK", r#"{"status":"ok"}"#)
                };
                let resp = format!(
                    "HTTP/1.1 {code}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = s.write_all(resp.as_bytes()).await;
            }
        });
        (port, hits)
    }

    fn client(port: u16) -> reqwest::Client {
        Transport::new(Endpoint::Tcp(port), "k".into())
            .client(Duration::from_secs(5))
            .unwrap()
    }

    #[tokio::test]
    async fn waits_through_loading_then_ok() {
        let (port, hits) = fake_server(3).await;
        let took = wait_ready(&client(port), Duration::from_secs(5), || true)
            .await
            .unwrap();
        assert_eq!(hits.load(Ordering::SeqCst), 4);
        assert!(took < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn health_timeout() {
        let (port, _) = fake_server(u32::MAX).await;
        let err = wait_ready(&client(port), Duration::from_millis(200), || true)
            .await
            .unwrap_err();
        assert_eq!(err, WaitError::Timeout);
    }

    #[tokio::test]
    async fn a_dead_process_ends_the_wait_at_once() {
        // Nothing listens on this port.
        let port = crate::fork::llm::transport::free_loopback_port().unwrap();
        let t0 = Instant::now();
        let mut calls = 0;
        let err = wait_ready(&client(port), Duration::from_secs(10), || {
            calls += 1;
            calls < 3
        })
        .await
        .unwrap_err();
        assert_eq!(err, WaitError::Exited);
        assert!(t0.elapsed() < Duration::from_secs(2));
        assert_eq!(probe(&client(port)).await, Health::Down);
    }
}
