// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests only: a small local stand-in for publik's API on 127.0.0.1, so the
//! client and the gate run their real curl requests against it.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use super::client::{uuid_v4, Env};

#[derive(Default)]
struct State {
    issued: Vec<String>,
    installs: u32,
    install_bodies: Vec<Value>,
    wallet_reads: u32,
    /// Wallet reads after this many are signed in.
    claim_after: u32,
    telemetry: Vec<(Value, Option<String>)>,
}

pub struct FakePublik {
    port: u16,
    state: Arc<Mutex<State>>,
}

impl FakePublik {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        let state = Arc::new(Mutex::new(State {
            claim_after: u32::MAX,
            ..State::default()
        }));
        let shared = state.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let state = shared.clone();
                std::thread::spawn(move || {
                    let _ = serve(stream, port, &state);
                });
            }
        });
        FakePublik { port, state }
    }

    pub fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn claim_after_wallet_reads(&self, n: u32) {
        self.state.lock().unwrap().claim_after = n;
    }

    pub fn installs(&self) -> u32 {
        self.state.lock().unwrap().installs
    }

    pub fn install_bodies(&self) -> Vec<Value> {
        self.state.lock().unwrap().install_bodies.clone()
    }

    /// Usage events received: body and bearer key.
    pub fn telemetry(&self) -> Vec<(Value, Option<String>)> {
        self.state.lock().unwrap().telemetry.clone()
    }
}

/// An `Env` pointed at `publik`, with its own empty publik folder and a
/// fast poll (3 looks, 10 ms apart).
pub fn env(publik: &FakePublik) -> Env {
    let dir = std::env::temp_dir().join(format!("wb-publik-test-{}", uuid_v4()));
    Env {
        origin: publik.origin(),
        dir: Some(dir),
        token: Some("pat_test".into()),
        api_key: None,
        poll_every: Duration::from_millis(10),
        polls: 3,
    }
}

fn serve(stream: TcpStream, port: u16, state: &Mutex<State>) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut first = String::new();
    reader.read_line(&mut first)?;
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let mut length = 0usize;
    let mut bearer = None;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            let value = value.trim();
            match name.to_ascii_lowercase().as_str() {
                "content-length" => length = value.parse().unwrap_or(0),
                "authorization" => {
                    bearer = value.strip_prefix("Bearer ").map(str::to_string);
                }
                _ => {}
            }
        }
    }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let origin = format!("http://127.0.0.1:{port}");

    let (code, reply) = {
        let mut s = state.lock().unwrap();
        match (method.as_str(), path.as_str()) {
            ("POST", "/api/v1/installs") => {
                let token = body.get("app_token").and_then(Value::as_str).unwrap_or("");
                if !token.starts_with("pat_") {
                    (401, json!({"error": {"message": "Unknown app token."}}))
                } else {
                    s.installs += 1;
                    s.install_bodies.push(body.clone());
                    let key = format!("pk_test_{}", s.installs);
                    s.issued.push(key.clone());
                    (
                        201,
                        json!({
                            "key": key,
                            "base_url": format!("{origin}/api/v1"),
                            "claim_url": format!("{origin}/claim/abc"),
                        }),
                    )
                }
            }
            ("GET", "/api/v1/wallet") => match bearer {
                Some(key) if s.issued.contains(&key) => {
                    s.wallet_reads += 1;
                    let claimed = s.wallet_reads > s.claim_after;
                    (
                        200,
                        json!({
                            "claim_state": if claimed { "claimed" } else { "unclaimed" },
                            "claim_url": format!("{origin}/claim/abc"),
                            "plan": {"kind": "none"},
                            "balance_micros": 0,
                        }),
                    )
                }
                _ => (401, json!({"error": {"message": "Unknown key."}})),
            },
            ("POST", "/api/v1/telemetry") => {
                s.telemetry.push((body, bearer));
                (202, json!({"ok": true}))
            }
            _ => (404, json!({"error": {"message": "Not found."}})),
        }
    };
    let text = reply.to_string();
    let mut out = stream;
    write!(
        out,
        "HTTP/1.1 {code} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}",
        text.len()
    )?;
    out.flush()
}
