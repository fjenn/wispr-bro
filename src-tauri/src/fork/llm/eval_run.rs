// SPDX-License-Identifier: GPL-3.0-or-later
//! The cleanup eval through the shipped code path (S-12a, R5 §11.1): every
//! item of `eval/cleanup/items.jsonl` goes through
//! [`super::post_process::clean`] (gate, chunks, prompt builder, guard chain,
//! deadlines) on the real `llama-server` sidecar via the real supervisor and
//! transport. The scorer reads the JSONL this writes:
//!
//! ```text
//! WB_LLAMA_SERVER=<sidecar> WB_LLM_MODEL=<gguf> WB_EVAL_OUT=<run.jsonl> \
//!   [WB_EVAL_LEVELS=light,medium] [WB_EVAL_LANGUAGE=none|item] \
//!   cargo test --lib fork::llm::eval_run -- --ignored --nocapture
//! bun scripts/wb-eval-cleanup.ts --replay <run.jsonl> --level all
//! ```
//!
//! `WB_EVAL_LANGUAGE=item` passes each item's labelled language as the
//! take's language (a user who picked their language); the default `none`
//! leaves it unknown, as with an auto-detecting recognizer, so the guards
//! detect it.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use serde_json::{json, Value};

use super::post_process::{clean, LocalEngine, Request};
use super::supervisor::{Config, ProcessLauncher, Supervisor};
use crate::fork::settings::CleanupLevel;

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs WB_LLAMA_SERVER, WB_LLM_MODEL and WB_EVAL_OUT"]
async fn eval_run_through_the_shipped_path() {
    let (Some(bin), Some(model), Some(out)) = (
        env("WB_LLAMA_SERVER"),
        env("WB_LLM_MODEL"),
        env("WB_EVAL_OUT"),
    ) else {
        eprintln!("eval_run: set WB_LLAMA_SERVER, WB_LLM_MODEL and WB_EVAL_OUT");
        return;
    };
    let items_path = env("WB_EVAL_ITEMS").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../eval/cleanup/items.jsonl")
    });
    let levels: Vec<CleanupLevel> = env("WB_EVAL_LEVELS")
        .unwrap_or_else(|| "light".into())
        .split(',')
        .map(|l| match l.trim() {
            "medium" => CleanupLevel::Medium,
            "light" => CleanupLevel::Light,
            other => panic!("unknown level {other}"),
        })
        .collect();
    // none: unknown language; item: the labelled language; owner: detected
    // like the app for a user who speaks English, Dutch, French (and the
    // item's language), through `hint_language`.
    let language_mode = env("WB_EVAL_LANGUAGE").unwrap_or_else(|| "none".into());
    assert!(["none", "item", "owner"].contains(&language_mode.as_str()));
    let only: Option<Vec<String>> =
        env("WB_EVAL_IDS").map(|s| s.split(',').map(str::to_owned).collect());

    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::new(bin.into(), dir.path().join("llm"));
    cfg.model = Some(model.clone().into());
    let sup: Arc<Supervisor> = Supervisor::new(Arc::new(ProcessLauncher), cfg);
    let t0 = Instant::now();
    sup.ensure_ready().await.expect("sidecar starts");
    eprintln!("eval_run: server ready in {} ms", t0.elapsed().as_millis());
    // One warm-up request (Metal shaders, prompt cache), not scored.
    let _ = clean(
        &LocalEngine {
            supervisor: &sup,
            pill: false,
        },
        &Request {
            text: "um so this is a warm up",
            level: CleanupLevel::Light,
            language: None,
            hint: None,
            terms: &[],
            keep: &[],
        },
    )
    .await;

    let items: Vec<Value> = std::fs::read_to_string(&items_path)
        .expect("items")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("item json"))
        .filter(|v: &Value| {
            only.as_ref()
                .is_none_or(|ids| ids.iter().any(|i| v["id"].as_str() == Some(i.as_str())))
        })
        .collect();
    let mut file = std::fs::File::create(&out).expect("out file");
    writeln!(
        file,
        "{}",
        json!({
            "meta": {
                "runner": "src-tauri/src/fork/llm/eval_run.rs (S-12a shipped path)",
                "model": std::path::Path::new(&model).file_name().map(|f| f.to_string_lossy().to_string()),
                "items": items.len(),
                "levels": levels.iter().map(|l| l.as_str()).collect::<Vec<_>>(),
                "language_hint": language_mode != "none",
                "language_mode": language_mode,
                "system_prompt_light": super::post_process::system_prompt(&Request { text: "", level: CleanupLevel::Light, language: None, hint: None, terms: &[], keep: &[] }),
                "system_prompt_medium": super::post_process::system_prompt(&Request { text: "", level: CleanupLevel::Medium, language: None, hint: None, terms: &[], keep: &[] }),
            }
        })
    )
    .unwrap();
    let engine = LocalEngine {
        supervisor: &sup,
        pill: false,
    };
    let started = Instant::now();
    let mut n = 0;
    for level in &levels {
        for item in &items {
            let input = item["input"].as_str().unwrap_or_default();
            let lang = item["language"].as_str();
            let hinted: Option<String> = match language_mode.as_str() {
                "item" => lang.map(str::to_owned),
                "owner" => {
                    let mut spoken: Vec<String> = vec!["en".into(), "nl".into(), "fr".into()];
                    if let Some(l) = lang {
                        spoken.push(l.to_owned());
                    }
                    super::post_process::hint_language(input, &spoken)
                }
                _ => None,
            };
            let req = Request {
                text: input,
                level: *level,
                // "item": the recognizer knew the language; "owner": a guess.
                language: if language_mode == "item" {
                    hinted.as_deref()
                } else {
                    None
                },
                hint: if language_mode == "owner" {
                    hinted.as_deref()
                } else {
                    None
                },
                terms: &[],
                keep: &[],
            };
            let t = Instant::now();
            let cleaned = clean(&engine, &req).await;
            let wall = t.elapsed().as_secs_f64() * 1000.0;
            let expected = item["expected"][level.as_str()]
                .as_str()
                .or_else(|| item["expected"]["light"].as_str())
                .unwrap_or_default();
            let pre = cleaned.model_text.clone();
            let lang_id = |t: &str| crate::audio_toolkit::detect_output_language(t, &[]);
            let chunks: Vec<Value> = cleaned
                .chunks
                .iter()
                .map(|c| {
                    json!({
                        "input": c.input,
                        "raw": c.raw,
                        "text": c.verdict.as_ref().and_then(|v| v.text.clone()),
                        "accepted": c.verdict.as_ref().is_some_and(|v| v.accepted()),
                        "reasons": c.verdict.as_ref().map(|v| v.reasons.iter().map(|r| r.as_str()).collect::<Vec<_>>()).unwrap_or_default(),
                        "ms": c.ms,
                        "deadline_ms": c.deadline_ms,
                        "error": c.error,
                        "timed_out": c.timed_out,
                    })
                })
                .collect();
            writeln!(
                file,
                "{}",
                json!({
                    "id": item["id"],
                    "level": level.as_str(),
                    "language": lang,
                    "language_used": hinted,
                    "input": input,
                    "output": cleaned.text,
                    "pre": pre,
                    "outcome": cleaned.outcome.as_str(),
                    "why": cleaned.why.map(|w| w.as_str()),
                    "llm_ms": cleaned.llm_ms,
                    "lang_id": {
                        "expected": lang_id(expected),
                        "output": lang_id(&cleaned.text),
                        "pre": lang_id(&pre),
                    },
                    "wall_ms": wall,
                    "chunks": chunks,
                })
            )
            .unwrap();
            n += 1;
            if n % 25 == 0 {
                eprintln!("eval_run: {n}/{}", items.len() * levels.len());
            }
        }
    }
    eprintln!(
        "eval_run: {n} runs in {:.1} s -> {out}",
        started.elapsed().as_secs_f64()
    );
    sup.stop().await;
}
