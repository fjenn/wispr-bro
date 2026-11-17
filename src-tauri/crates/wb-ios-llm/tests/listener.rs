// SPDX-License-Identifier: GPL-3.0-or-later
//! The in-process listener through the app's client stack (reqwest on a
//! Unix socket with a bearer key, streamed, cancelled by dropping the
//! request), as `fork/llm/{transport,health,chat}.rs` use the sidecar.
//!
//! ```text
//! WB_LLM_MODEL=~/wb-bench/llm/qwen35-2b-muesli-v12-Q4_K_M.gguf \
//!   cargo test --release --test listener -- --ignored --nocapture
//! ```

use std::time::{Duration, Instant};

use wb_ios_llm::serve::{Listen, Server, State};
use wb_ios_llm::Config;

const KEY: &str = "0123456789abcdef";

fn client(socket: &std::path::Path, key: &str) -> reqwest::Client {
    // `Transport::client`.
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        format!("Bearer {key}").parse().unwrap(),
    );
    reqwest::Client::builder()
        .no_proxy()
        .default_headers(headers)
        .unix_socket(socket.to_path_buf())
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
}

fn body(user: &str, max_tokens: u32, schema: bool) -> serde_json::Value {
    // `ChatRequest::body`.
    let mut body = serde_json::json!({
        "model": "local",
        "messages": [
            { "role": "system", "content": "Clean up the dictated transcript: remove filler words, fix punctuation and capitalization, keep the meaning. Reply only with JSON: {\"text\": \"<the cleaned text>\"}." },
            { "role": "user", "content": user },
        ],
        "temperature": 0,
        "seed": 42,
        "max_tokens": max_tokens,
        "stream": true,
        "chat_template_kwargs": { "enable_thinking": false },
    });
    if schema {
        body["response_format"] = serde_json::json!({
            "type": "json_schema",
            "json_schema": { "name": "cleanup", "strict": true, "schema": {
                "type": "object", "properties": { "text": { "type": "string" } },
                "required": ["text"], "additionalProperties": false } }
        });
    }
    body
}

/// `chat::complete` + `StreamState`: the streamed content and whether the
/// stream ended with `[DONE]`.
async fn complete(client: &reqwest::Client, body: &serde_json::Value) -> (u16, String, bool) {
    let mut resp = client
        .post("http://localhost/v1/chat/completions")
        .json(body)
        .send()
        .await
        .expect("send");
    let status = resp.status().as_u16();
    let mut buf = Vec::new();
    while let Some(chunk) = resp.chunk().await.expect("chunk") {
        buf.extend_from_slice(&chunk);
    }
    let text = String::from_utf8_lossy(&buf).into_owned();
    let mut content = String::new();
    let mut done = false;
    for line in text.lines() {
        let Some(data) = line.strip_prefix("data: ") else {
            continue;
        };
        if data == "[DONE]" {
            done = true;
        } else if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
            if let Some(t) = v["choices"][0]["delta"]["content"].as_str() {
                content.push_str(t);
            }
        }
    }
    if status != 200 {
        content = text;
    }
    (status, content, done)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs WB_LLM_MODEL"]
async fn the_apps_client_works_against_the_listener() {
    let Ok(model) = std::env::var("WB_LLM_MODEL") else {
        eprintln!("set WB_LLM_MODEL");
        return;
    };
    let dir = std::env::temp_dir().join(format!("wbl-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let socket = dir.join("llm.sock");
    let t0 = Instant::now();
    let mut server = Server::start(
        Listen::Unix(socket.clone()),
        Some(KEY.into()),
        Config::new(model),
    )
    .unwrap();
    let c = client(&socket, KEY);

    // `health::wait_ready`: 503 while loading, then 200.
    let mut saw_loading = false;
    loop {
        let r = c
            .get("http://localhost/health")
            .send()
            .await
            .expect("health");
        if r.status().is_success() {
            assert!(r.text().await.unwrap().contains("ok"));
            break;
        }
        assert_eq!(r.status().as_u16(), 503);
        saw_loading = true;
        assert!(t0.elapsed() < Duration::from_secs(60));
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    eprintln!(
        "ready in {:?} (saw 503 while loading: {saw_loading})",
        t0.elapsed()
    );
    assert_eq!(server.state(), State::Ready);
    assert!(server.model_bytes().is_some_and(|b| b > 100 << 20));

    // A cleanup, streamed, twice the same.
    let request = body("um so the meeting is uh on thursday", 64, true);
    let (status, text, done) = complete(&c, &request).await;
    assert_eq!((status, done), (200, true), "{text}");
    let parsed: serde_json::Value = serde_json::from_str(&text).expect("schema reply");
    assert!(parsed["text"]
        .as_str()
        .unwrap()
        .to_lowercase()
        .contains("thursday"));
    assert_eq!(complete(&c, &request).await.1, text);

    // The wrong key is refused; `/health` stays public.
    let wrong = client(&socket, "nope");
    assert_eq!(complete(&wrong, &request).await.0, 401);
    assert!(wrong
        .get("http://localhost/health")
        .send()
        .await
        .unwrap()
        .status()
        .is_success());
    // Unknown route, malformed body.
    assert_eq!(
        c.get("http://localhost/nope")
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        404
    );
    let bad = c
        .post("http://localhost/v1/chat/completions")
        .body("{\"messages\":")
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status().as_u16(), 400);

    // Esc: the request is dropped after the first bytes; the slot is free
    // for the next request at once (RS-06 measured this on the sidecar).
    let long = body("write a very long story about a dog", 1500, false);
    let mut resp = c
        .post("http://localhost/v1/chat/completions")
        .json(&long)
        .send()
        .await
        .unwrap();
    let _ = resp.chunk().await.unwrap();
    drop(resp);
    let t1 = Instant::now();
    let (status, again, done) = complete(&c, &request).await;
    let after_cancel = t1.elapsed();
    assert_eq!((status, done, again.as_str()), (200, true, text.as_str()));
    eprintln!("request after a dropped one answered in {after_cancel:?}");
    assert!(after_cancel < Duration::from_millis(1500));

    // A client timeout (the app's request deadline) is a timeout error.
    let quick = reqwest::Client::builder()
        .no_proxy()
        .unix_socket(socket.clone())
        .default_headers(
            [(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {KEY}").parse().unwrap(),
            )]
            .into_iter()
            .collect(),
        )
        .timeout(Duration::from_millis(60))
        .build()
        .unwrap();
    let timed_out = async {
        let mut r = quick
            .post("http://localhost/v1/chat/completions")
            .json(&long)
            .send()
            .await?;
        while r.chunk().await?.is_some() {}
        Ok::<(), reqwest::Error>(())
    }
    .await;
    assert!(timed_out.is_err_and(|e| e.is_timeout()));
    assert_eq!(complete(&c, &request).await.1, text);

    // Stop: unloads, the socket file is gone, a new server starts.
    let t2 = Instant::now();
    server.stop();
    eprintln!("stopped in {:?}", t2.elapsed());
    assert_eq!(server.state(), State::Stopped);
    assert!(!socket.exists());
    assert!(c.get("http://localhost/health").send().await.is_err());
    drop(server);
    let _ = std::fs::remove_dir_all(&dir);
}
