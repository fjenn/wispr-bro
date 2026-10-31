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
//! S-12b / RS-08: `WB_EVAL_ENGINE=apple` runs the same path on Apple
//! Foundation Models instead (no sidecar or model needed), with
//! `WB_APPLE_GUARDRAILS=default|permissive` (default permissive) and
//! `WB_APPLE_GUIDED=1|0` (default: the shipped `apple::GUIDED`). `WB_EVAL_APPLE_PREWARM_MS=<ms>`
//! prewarms Apple's model with the item's prompt that long before each item
//! (the app's key-down prewarm); `WB_EVAL_NO_DEADLINE=1` (with
//! `WB_DEBUG_CLEANUP_DEADLINE_MS=<ms>` for each request) scores quality
//! without the latency budget.
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

use super::apple::AppleEngine;
use super::chat::ChatRequest;
use super::post_process::{clean, clean_within, Engine, EngineError, LocalEngine, Request};
use super::supervisor::{Config, ProcessLauncher, Supervisor};
use crate::fork::settings::CleanupLevel;

/// The engine under test.
enum EvalEngine<'a> {
    Local(LocalEngine<'a>),
    Apple(AppleEngine),
}

impl Engine for EvalEngine<'_> {
    async fn complete(
        &self,
        request: ChatRequest,
        timeout: std::time::Duration,
    ) -> Result<String, EngineError> {
        match self {
            EvalEngine::Local(e) => e.complete(request, timeout).await,
            EvalEngine::Apple(e) => e.complete(request, timeout).await,
        }
    }
}

fn apple_engine() -> AppleEngine {
    AppleEngine {
        pill: false,
        guardrails: match env("WB_APPLE_GUARDRAILS").as_deref() {
            Some("default") => wb_apple_fm::Guardrails::Default,
            None | Some("permissive") => wb_apple_fm::Guardrails::PermissiveContentTransformations,
            Some(other) => panic!("unknown WB_APPLE_GUARDRAILS {other}"),
        },
        guided: match env("WB_APPLE_GUIDED").as_deref() {
            Some("1") => true,
            Some("0") => false,
            _ => super::apple::GUIDED,
        },
    }
}

fn apple_label(e: &AppleEngine) -> String {
    format!(
        "apple-foundation-models ({}, {} guardrails)",
        if e.guided { "guided" } else { "plain" },
        if e.guardrails == wb_apple_fm::Guardrails::Default {
            "default"
        } else {
            "permissive"
        }
    )
}

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs WB_LLAMA_SERVER, WB_LLM_MODEL and WB_EVAL_OUT"]
async fn eval_run_through_the_shipped_path() {
    let apple = env("WB_EVAL_ENGINE").as_deref() == Some("apple");
    let (bin, model, Some(out)) = (
        env("WB_LLAMA_SERVER"),
        env("WB_LLM_MODEL"),
        env("WB_EVAL_OUT"),
    ) else {
        eprintln!("eval_run: set WB_EVAL_OUT (and WB_LLAMA_SERVER + WB_LLM_MODEL, or WB_EVAL_ENGINE=apple)");
        return;
    };
    if !apple && (bin.is_none() || model.is_none()) {
        eprintln!("eval_run: set WB_LLAMA_SERVER and WB_LLM_MODEL, or WB_EVAL_ENGINE=apple");
        return;
    }
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
    // S-19a: a style preset for every English item (the app applies styles
    // to English only): stage 10 runs on the output, so the scorer sees the
    // inserted text (a preset's prompt fragment, if one ever ships, too).
    let style: Option<crate::fork::settings::StylePreset> =
        env("WB_EVAL_STYLE").map(|s| match s.trim() {
            "formal" => crate::fork::settings::StylePreset::Formal,
            "relaxed" => crate::fork::settings::StylePreset::Relaxed,
            "very_relaxed" => crate::fork::settings::StylePreset::VeryRelaxed,
            "upbeat" => crate::fork::settings::StylePreset::Upbeat,
            other => panic!("unknown style {other}"),
        });
    let only: Option<Vec<String>> =
        env("WB_EVAL_IDS").map(|s| s.split(',').map(str::to_owned).collect());

    let dir = tempfile::tempdir().unwrap();
    let sup: Option<Arc<Supervisor>> = (!apple).then(|| {
        let mut cfg = Config::new(
            bin.clone().unwrap_or_default().into(),
            dir.path().join("llm"),
        );
        cfg.model = model.clone().map(Into::into);
        Supervisor::new(Arc::new(ProcessLauncher), cfg)
    });
    let t0 = Instant::now();
    if let Some(sup) = &sup {
        sup.ensure_ready().await.expect("sidecar starts");
        eprintln!("eval_run: server ready in {} ms", t0.elapsed().as_millis());
    }
    let engine = match &sup {
        Some(sup) => EvalEngine::Local(LocalEngine {
            supervisor: sup,
            pill: false,
        }),
        None => EvalEngine::Apple(apple_engine()),
    };
    // One warm-up request (Metal shaders, prompt cache), not scored.
    let _ = clean(
        &engine,
        &Request {
            text: "um so this is a warm up",
            level: CleanupLevel::Light,
            language: None,
            hint: None,
            terms: &[],
            keep: &[],
            style: None,
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
                "model": match &engine {
                    EvalEngine::Apple(e) => Some(apple_label(e)),
                    EvalEngine::Local(_) => model.as_deref().and_then(|m| std::path::Path::new(m).file_name()).map(|f| f.to_string_lossy().to_string()),
                },
                "engine": if apple { "apple" } else { "local" },
                "items": items.len(),
                "levels": levels.iter().map(|l| l.as_str()).collect::<Vec<_>>(),
                "language_hint": language_mode != "none",
                "language_mode": language_mode,
                "style": style.map(|s| format!("{s:?}")),
                "system_prompt_light": super::post_process::system_prompt(&Request { text: "", level: CleanupLevel::Light, language: None, hint: None, terms: &[], keep: &[], style }),
                "system_prompt_medium": super::post_process::system_prompt(&Request { text: "", level: CleanupLevel::Medium, language: None, hint: None, terms: &[], keep: &[], style }),
            }
        })
    )
    .unwrap();
    // RS-08: quality without the latency budget (debug builds also need
    // WB_DEBUG_CLEANUP_DEADLINE_MS for the per-request deadline), and the
    // app's key-down prewarm before each Apple item.
    let no_deadline = env("WB_EVAL_NO_DEADLINE").as_deref() == Some("1");
    let prewarm_ms: Option<u64> = env("WB_EVAL_APPLE_PREWARM_MS").and_then(|v| v.parse().ok());
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
                style: style.filter(|_| lang.is_none_or(|l| l == "en")),
            };
            if let (Some(ms), EvalEngine::Apple(_)) = (prewarm_ms, &engine) {
                // The app prewarms at key-down; the take is spoken meanwhile.
                super::apple::prewarm(&super::post_process::system_prompt(&req));
                tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
            }
            let t = Instant::now();
            let mut cleaned = if no_deadline {
                clean_within(&engine, &req, std::time::Duration::from_secs(120)).await
            } else {
                clean(&engine, &req).await
            };
            if let Some(preset) = req.style {
                // Stage 10, as `post_process::run` applies it.
                cleaned.text = crate::fork::text::style_rules::apply(preset, &cleaned.text, &[]);
            }
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
    if let Some(sup) = sup {
        sup.stop().await;
    }
}

/// S-19a: a style preset applied to a finished run without the model.
/// No preset changes the cleanup prompt (`prompt::style_fragment`), so a
/// preset's run is the run without a style plus stage 10 on what would be
/// inserted; this keeps the comparison free of model timing noise.
///
/// ```text
/// WB_EVAL_IN=<run.jsonl> WB_EVAL_STYLE=relaxed WB_EVAL_OUT=<styled.jsonl> \
///   cargo test --lib fork::llm::eval_run::eval_restyle -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs WB_EVAL_IN, WB_EVAL_STYLE and WB_EVAL_OUT"]
fn eval_restyle() {
    use crate::fork::settings::StylePreset;
    let (Some(input), Some(style), Some(out)) =
        (env("WB_EVAL_IN"), env("WB_EVAL_STYLE"), env("WB_EVAL_OUT"))
    else {
        eprintln!("eval_restyle: set WB_EVAL_IN, WB_EVAL_STYLE and WB_EVAL_OUT");
        return;
    };
    let preset = match style.trim() {
        "formal" => StylePreset::Formal,
        "relaxed" => StylePreset::Relaxed,
        "very_relaxed" => StylePreset::VeryRelaxed,
        "upbeat" => StylePreset::Upbeat,
        other => panic!("unknown style {other}"),
    };
    let mut file = std::fs::File::create(&out).expect("out file");
    let (mut styled, mut changed) = (0, 0);
    for line in std::fs::read_to_string(&input).expect("input").lines() {
        let mut row: Value = serde_json::from_str(line).expect("row json");
        if let Some(meta) = row.get_mut("meta") {
            meta["style"] = json!(format!("{preset:?} (restyled)"));
        } else if row["language"].as_str().is_none_or(|l| l == "en") {
            let before = row["output"].as_str().unwrap_or_default().to_owned();
            let after = crate::fork::text::style_rules::apply(preset, &before, &[]);
            styled += 1;
            if after != before {
                changed += 1;
            }
            row["lang_id"]["output"] =
                json!(crate::audio_toolkit::detect_output_language(&after, &[]));
            row["output"] = json!(after);
        }
        writeln!(file, "{row}").unwrap();
    }
    eprintln!("eval_restyle: {preset:?} on {styled} English rows, {changed} changed -> {out}");
}
