// SPDX-License-Identifier: GPL-3.0-or-later
//! Both ggml copies in one binary, as in the iPhone app: transcribe-cpp
//! (speech, its own ggml) and this crate (llama.cpp's ggml, hidden). Speech
//! and cleanup run at the same time on two threads, then the model is
//! unloaded and loaded again while the speech model stays, with the app's
//! memory footprint after each step.
//!
//! ```text
//! cargo run --release --example coexist -- <speech.gguf> <16k-mono-s16.wav> <cleanup.gguf> [--cpu]
//! ```
//!
//! Also the link test for iOS: `cargo build --example coexist --target
//! aarch64-apple-ios` fails on any duplicate symbol.

use std::time::Instant;

use transcribe_cpp::{Model, RunOptions};
use wb_ios_llm::{footprint_bytes, Config, Device, Engine, Stop, CHAT_COMPLETIONS};

fn mb() -> f64 {
    footprint_bytes().unwrap_or(0) as f64 / 1_048_576.0
}

/// 16-bit PCM WAV → f32 samples (the `data` chunk; no resampling).
fn read_wav(path: &str) -> Vec<f32> {
    let bytes = std::fs::read(path).expect("wav");
    let at = bytes
        .windows(4)
        .position(|w| w == b"data")
        .expect("data chunk");
    bytes[at + 8..]
        .chunks_exact(2)
        .map(|b| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32768.0)
        .collect()
}

fn cleanup(engine: &Engine, text: &str) -> (u16, String, f64) {
    // The app's request shape (`fork/llm/chat.rs`), prompt shortened.
    let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
    let body = format!(
        r#"{{"model":"local","messages":[{{"role":"system","content":"Clean up the dictated transcript: remove filler words, fix punctuation and capitalization, keep the meaning.\n\nReply only with JSON: {{\"text\": \"<the cleaned text>\"}}."}},{{"role":"user","content":"<transcript>\n{escaped}\n</transcript>\n\nOutput only the cleaned transcript."}}],"temperature":0,"seed":42,"max_tokens":256,"stream":false,"chat_template_kwargs":{{"enable_thinking":false}},"response_format":{{"type":"json_schema","json_schema":{{"name":"cleanup","strict":true,"schema":{{"type":"object","properties":{{"text":{{"type":"string"}}}},"required":["text"],"additionalProperties":false}}}}}}}}"#
    );
    let t0 = Instant::now();
    let mut out = Vec::new();
    let status = engine
        .request(CHAT_COMPLETIONS, &body, &Stop::default(), |b| {
            out.extend_from_slice(b)
        })
        .expect("request");
    (
        status,
        String::from_utf8_lossy(&out).into_owned(),
        t0.elapsed().as_secs_f64() * 1000.0,
    )
}

/// The `"content"` string of a non-streamed reply, without a JSON parser.
fn content(reply: &str) -> String {
    let Some(at) = reply.find("\"content\":\"") else {
        return reply.chars().take(300).collect();
    };
    let rest = &reply[at + 11..];
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(n) = chars.next() {
                    out.push(n)
                }
            }
            '"' => break,
            c => out.push(c),
        }
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (speech, wav, llm) = (&args[1], &args[2], &args[3]);
    let mut config = Config::new(llm);
    if args.iter().any(|a| a == "--cpu") {
        config.device = Device::Cpu;
    }
    println!("llama.cpp {}", wb_ios_llm::build_info());
    println!("footprint at start            {:8.1} MB", mb());

    let model = Model::load(speech).expect("speech model loads");
    let mut session = model.session().expect("speech session");
    let pcm = read_wav(wav);
    let t0 = Instant::now();
    let heard = session
        .run(&pcm, &RunOptions::default())
        .expect("transcribe")
        .text;
    println!(
        "speech alone: {:.0} ms: {heard:?}",
        t0.elapsed().as_secs_f64() * 1000.0
    );
    println!("footprint with speech model   {:8.1} MB", mb());

    let t0 = Instant::now();
    let engine = Engine::load(&config).expect("cleanup model loads");
    println!(
        "cleanup model loaded in {:.0} ms ({:.0} MB of tensors, {} ctx)",
        t0.elapsed().as_secs_f64() * 1000.0,
        engine.model_bytes() as f64 / 1_048_576.0,
        engine.context_tokens()
    );
    let (status, reply, ms) = cleanup(&engine, &heard);
    println!(
        "cleanup alone: {status} in {ms:.0} ms: {:?}",
        content(&reply)
    );
    println!("footprint with both models    {:8.1} MB", mb());

    // Both at once: the speech thread transcribes in a loop while cleanups
    // run, as when a new take starts during the last one's cleanup.
    let same = std::thread::scope(|s| {
        let speech_thread = s.spawn(|| {
            let mut texts = Vec::new();
            for _ in 0..6 {
                texts.push(
                    session
                        .run(&pcm, &RunOptions::default())
                        .expect("transcribe")
                        .text,
                );
            }
            texts
        });
        let mut replies = Vec::new();
        for _ in 0..6 {
            let (status, r, _) = cleanup(&engine, &heard);
            assert_eq!(status, 200);
            replies.push(content(&r));
        }
        let texts = speech_thread.join().expect("speech thread");
        texts.iter().all(|t| *t == heard) && replies.iter().all(|r| *r == content(&reply))
    });
    println!("6 transcriptions and 6 cleanups at the same time, results unchanged: {same}");
    assert!(same);

    // One engine per process: a second load is refused while one is loaded.
    assert_eq!(Engine::load(&config).err(), Some(wb_ios_llm::Error::Busy));
    drop(engine);
    println!("footprint after unload        {:8.1} MB", mb());
    for round in 1..=3 {
        let t0 = Instant::now();
        let engine = Engine::load(&config).expect("reload");
        let load_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let (status, r, ms) = cleanup(&engine, &heard);
        assert_eq!((status, content(&r)), (200, content(&reply)));
        let loaded = mb();
        drop(engine);
        println!(
            "round {round}: load {load_ms:.0} ms, cleanup {ms:.0} ms, footprint loaded {loaded:.1} MB, unloaded {:.1} MB",
            mb()
        );
    }
    std::thread::sleep(std::time::Duration::from_secs(3));
    println!("footprint 3 s after the last unload {:6.1} MB", mb());
    let again = session
        .run(&pcm, &RunOptions::default())
        .expect("transcribe")
        .text;
    assert_eq!(again, heard);
    println!("speech model still answers after the unloads: {again:?}");
    println!("OK");
}
