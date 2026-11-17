// SPDX-License-Identifier: GPL-3.0-or-later
//! With a real model (ignored by default):
//!
//! ```text
//! WB_LLM_MODEL=~/wb-bench/llm/qwen35-2b-muesli-v12-Q4_K_M.gguf \
//!   cargo test --release --test real_model -- --ignored --nocapture
//! ```

use std::time::{Duration, Instant};

use wb_ios_llm::{Config, Engine, Error, Stop, CHAT_COMPLETIONS};

fn body(user: &str, max_tokens: u32, schema: bool) -> String {
    let format = if schema {
        r#","response_format":{"type":"json_schema","json_schema":{"name":"cleanup","strict":true,"schema":{"type":"object","properties":{"text":{"type":"string"}},"required":["text"],"additionalProperties":false}}}"#
    } else {
        ""
    };
    format!(
        r#"{{"model":"local","messages":[{{"role":"system","content":"Clean up the dictated transcript: remove filler words, fix punctuation and capitalization, keep the meaning. Reply only with JSON: {{\"text\": \"<the cleaned text>\"}}."}},{{"role":"user","content":"{user}"}}],"temperature":0,"seed":42,"max_tokens":{max_tokens},"stream":true,"chat_template_kwargs":{{"enable_thinking":false}}{format}}}"#
    )
}

fn chat(engine: &Engine, body: &str, stop: &Stop) -> (Result<u16, Error>, String) {
    let mut out = Vec::new();
    let status = engine.request(CHAT_COMPLETIONS, body, stop, |b| out.extend_from_slice(b));
    (status, String::from_utf8_lossy(&out).into_owned())
}

/// The text of a streamed reply and its last `timings.cache_n`.
fn read(sse: &str) -> (String, Option<u64>) {
    let mut text = String::new();
    let mut cached = None;
    for line in sse.lines() {
        let Some(data) = line.strip_prefix("data: ") else {
            continue;
        };
        let mut rest = data;
        while let Some(at) = rest.find("\"content\":\"") {
            rest = &rest[at + 11..];
            let mut chars = rest.chars();
            while let Some(c) = chars.next() {
                match c {
                    '\\' => {
                        if let Some(n) = chars.next() {
                            text.push(n)
                        }
                    }
                    '"' => break,
                    c => text.push(c),
                }
            }
        }
        if let Some(at) = data.find("\"cache_n\":") {
            cached = data[at + 10..]
                .split(|c: char| !c.is_ascii_digit())
                .next()
                .and_then(|n| n.parse().ok());
        }
    }
    (text, cached)
}

#[test]
#[ignore = "needs WB_LLM_MODEL"]
fn streams_caches_cancels_and_meets_deadlines() {
    let Ok(model) = std::env::var("WB_LLM_MODEL") else {
        eprintln!("set WB_LLM_MODEL");
        return;
    };
    let engine = Engine::load(&Config::new(model)).expect("loads");
    assert!(engine.model_bytes() > 100 << 20);
    assert_eq!(engine.context_tokens(), 4096);

    // Health and properties, as the supervisor's health check reads them.
    let mut health = Vec::new();
    let status = engine.request("/health", "", &Stop::default(), |b| {
        health.extend_from_slice(b)
    });
    assert_eq!(status, Ok(200));
    assert!(String::from_utf8_lossy(&health).contains("ok"));
    assert!(matches!(
        engine.request("/nope", "", &Stop::default(), |_| {}),
        Err(Error::UnknownPath(_))
    ));

    // A streamed, schema-constrained reply that ends properly.
    let first = body("um so the meeting is uh on thursday", 64, true);
    let (status, sse) = chat(&engine, &first, &Stop::default());
    assert_eq!(status, Ok(200));
    assert!(sse.trim_end().ends_with("data: [DONE]"), "{sse}");
    let (text, _) = read(&sse);
    assert!(
        text.starts_with("{\"text\":") || text.starts_with("{ \"text\""),
        "{text}"
    );
    assert!(text.to_lowercase().contains("thursday"), "{text}");

    // Same system prompt again: its tokens come from the prompt cache, and
    // temperature 0 with the fixed seed repeats the reply.
    let (_, again) = chat(&engine, &first, &Stop::default());
    assert_eq!(read(&again).0, text);
    let (_, other) = chat(
        &engine,
        &body("so basically we ship on friday", 64, true),
        &Stop::default(),
    );
    let cached = read(&other).1.expect("timings");
    assert!(
        cached >= 20,
        "system prompt not served from the cache: {cached}"
    );

    // A bad request is the server's own 400, not a crash.
    let (status, error) = chat(&engine, "{\"messages\":", &Stop::default());
    assert_eq!(status, Ok(400), "{error}");
    // A reply cut off at the token limit says so (the app refuses those).
    let (_, cut) = chat(
        &engine,
        &body("write a very long story about a dog", 4, false),
        &Stop::default(),
    );
    assert!(cut.contains("\"finish_reason\":\"length\""), "{cut}");

    // Cancel in the middle of a long, unconstrained reply: the call returns
    // at the next token, without `[DONE]`, and the slot is free again.
    let long = body("write a very long story about a dog", 1500, false);
    let stop = Stop::default();
    let canceller = stop.clone();
    let t0 = Instant::now();
    let mut pieces = 0;
    let mut out = Vec::new();
    let status = engine.request(CHAT_COMPLETIONS, &long, &stop, |b| {
        out.extend_from_slice(b);
        pieces += 1;
        if pieces == 5 {
            canceller.cancel();
        }
    });
    let cancelled_in = t0.elapsed();
    assert_eq!(status, Ok(200));
    assert!(!String::from_utf8_lossy(&out).contains("[DONE]"));
    assert!(pieces <= 7, "stopped late: {pieces} pieces");
    eprintln!("cancelled after {pieces} pieces in {cancelled_in:?}");

    // A deadline ends a reply the same way (here it passes while the
    // prompt is still being evaluated: the call returns at the first token).
    let t0 = Instant::now();
    let stop = Stop::with_deadline(t0 + Duration::from_millis(30));
    let (status, sse) = chat(&engine, &long, &stop);
    let took = t0.elapsed();
    assert_eq!(status, Ok(200));
    assert!(stop.deadline_passed() && !sse.contains("[DONE]"));
    assert!(
        took < Duration::from_millis(1400),
        "deadline overrun: {took:?}"
    );
    eprintln!("30 ms deadline returned after {took:?}");

    // An already cancelled request does no work.
    let stop = Stop::default();
    stop.cancel();
    let t0 = Instant::now();
    let (status, sse) = chat(&engine, &long, &stop);
    assert!(matches!(status, Err(Error::Stopped)) || !sse.contains("[DONE]"));
    assert!(t0.elapsed() < Duration::from_millis(1500));

    // And the engine still answers, with the same text as before.
    let t0 = Instant::now();
    let (status, sse) = chat(&engine, &first, &Stop::default());
    assert_eq!(status, Ok(200));
    assert_eq!(read(&sse).0, text);
    eprintln!("request after the cancels: {:?}", t0.elapsed());

    // Unload, then load again in the same process.
    drop(engine);
    let engine =
        Engine::load(&Config::new(std::env::var("WB_LLM_MODEL").unwrap())).expect("reloads");
    let (_, sse) = chat(&engine, &first, &Stop::default());
    assert_eq!(read(&sse).0, text);
}
