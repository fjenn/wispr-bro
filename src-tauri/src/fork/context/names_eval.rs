// SPDX-License-Identifier: GPL-3.0-or-later
//! The S-20 name eval through the shipped code path (R5 S-20 exit
//! criterion: on the 30-item name set, the correct-spelling rate improves
//! by ≥ 20 percentage points with context on vs off).
//!
//! Every item of `eval/context/names.jsonl` has a short text that would be
//! near the caret (`context`), a sentence spoken with the name in it
//! (`said`, a WAV per item made by `scripts/wb-s20/names-audio.sh` with
//! macOS TTS written to a file, never played) and the name as spelled
//! (`name`). Per item:
//!
//! - **off**: the recognizer's text (no vocabulary from context), then the
//!   cleanup at Light without a CONTEXT line;
//! - **on**: the hints of `context` (`caret::hints_of`, as the AX read
//!   makes them), the recognizer's vocabulary when the model takes one,
//!   the spelling stage (`fork::text::context_stage`), then the cleanup
//!   with the relevant hints (`hints::relevant`) and the guards' keep list.
//!
//! A take is right when the final text has the name exactly. The time the
//! on path adds without the LLM (hints + spelling stage) is reported too.
//!
//! ```text
//! WB_ASR_MODEL=<gguf> WB_NAMES_AUDIO=<dir> WB_EVAL_OUT=<run.jsonl> \
//!   [WB_LLAMA_SERVER=<sidecar> WB_LLM_MODEL=<gguf>] \
//!   cargo test --lib fork::context::names_eval -- --ignored --nocapture
//! ```

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use serde_json::{json, Value};

use super::caret::{hints_of, CaretWindow};
use super::hints;
use crate::fork::llm::post_process::{clean, keep_with_context, LocalEngine, Request};
use crate::fork::llm::supervisor::{Config, ProcessLauncher, Supervisor};
use crate::fork::settings::CleanupLevel;
use crate::fork::text::counts::TextRecord;

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

fn percentile(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let i = ((p / 100.0) * (v.len() as f64 - 1.0)).round() as usize;
    v[i.min(v.len() - 1)]
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs WB_ASR_MODEL, WB_NAMES_AUDIO and WB_EVAL_OUT"]
async fn names_eval_through_the_shipped_path() {
    let (Some(asr), Some(audio), Some(out)) = (
        env("WB_ASR_MODEL"),
        env("WB_NAMES_AUDIO"),
        env("WB_EVAL_OUT"),
    ) else {
        eprintln!("names_eval: set WB_ASR_MODEL, WB_NAMES_AUDIO and WB_EVAL_OUT");
        return;
    };
    let items: Vec<Value> = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../eval/context/names.jsonl"),
    )
    .expect("names.jsonl")
    .lines()
    .filter(|l| !l.trim().is_empty())
    .map(|l| serde_json::from_str(l).expect("item"))
    .collect();
    assert_eq!(items.len(), 30, "R5 S-20: 30 items");

    transcribe_cpp::init_logging();
    transcribe_cpp::init_backends_default().expect("backends");
    let model = transcribe_cpp::Model::load_with(
        &asr,
        &transcribe_cpp::ModelOptions {
            backend: transcribe_cpp::Backend::Auto,
            device: None,
        },
    )
    .expect("model");
    let supports_vocabulary = model.supports(transcribe_cpp::Feature::Vocabulary);
    let mut session = model.session().expect("session");

    let dir = tempfile::tempdir().unwrap();
    let sup: Option<Arc<Supervisor>> = match (env("WB_LLAMA_SERVER"), env("WB_LLM_MODEL")) {
        (Some(bin), Some(llm)) => {
            let mut cfg = Config::new(bin.into(), dir.path().join("llm"));
            cfg.model = Some(llm.into());
            let sup = Supervisor::new(Arc::new(ProcessLauncher), cfg);
            sup.ensure_ready().await.expect("sidecar starts");
            Some(sup)
        }
        _ => None,
    };

    let mut file = std::fs::File::create(&out).expect("out");
    writeln!(
        file,
        "{}",
        json!({"meta": {
            "runner": "src-tauri/src/fork/context/names_eval.rs (S-20 shipped path)",
            "asr": std::path::Path::new(&asr).file_name().map(|f| f.to_string_lossy().to_string()),
            "vocabulary": supports_vocabulary,
            "cleanup": sup.is_some(),
        }})
    )
    .unwrap();

    let (mut right_off, mut right_on, mut right_asr) = (0, 0, 0);
    let mut added_ms = Vec::new();
    for item in &items {
        let id = item["id"].as_str().unwrap();
        let name = item["name"].as_str().unwrap();
        let context = item["context"].as_str().unwrap();
        let wav = PathBuf::from(&audio).join(format!("{id}.wav"));
        let pcm = crate::audio_toolkit::read_wav_samples(&wav).expect("wav");

        // Off: the recognizer alone.
        let off_asr = session
            .run(&pcm, &transcribe_cpp::RunOptions::default())
            .map(|t| t.text.trim().to_owned())
            .unwrap_or_default();

        // On: the read's hints (the AX layer would hand the same text to
        // `hints_of`), then every use.
        let t = Instant::now();
        let hint_list = hints_of(&CaretWindow::new(context, "", ""), &[], &[]);
        let mut extra_ms = t.elapsed().as_secs_f64() * 1000.0;
        let on_asr = if supports_vocabulary && !hint_list.is_empty() {
            let opts = transcribe_cpp::RunOptions {
                vocabulary: hints::vocabulary_terms(&[], &hint_list),
                ..Default::default()
            };
            session
                .run(&pcm, &opts)
                .map(|t| t.text.trim().to_owned())
                .unwrap_or_default()
        } else {
            off_asr.clone()
        };
        let t = Instant::now();
        let mut record = TextRecord::default();
        let corrected = crate::fork::text::context_stage(on_asr.clone(), &hint_list, &mut record);
        let relevant = hints::relevant(&corrected, &hint_list);
        extra_ms += t.elapsed().as_secs_f64() * 1000.0;
        added_ms.push(extra_ms);

        let (off_final, on_final) = match &sup {
            Some(sup) => {
                let engine = LocalEngine {
                    supervisor: sup,
                    pill: false,
                };
                let off = clean(
                    &engine,
                    &Request {
                        text: &off_asr,
                        level: CleanupLevel::Light,
                        language: Some("en"),
                        hint: None,
                        terms: &[],
                        keep: &[],
                        style: None,
                        context: &[],
                    },
                )
                .await;
                let keep = keep_with_context(&[], &relevant, &corrected);
                let on = clean(
                    &engine,
                    &Request {
                        text: &corrected,
                        level: CleanupLevel::Light,
                        language: Some("en"),
                        hint: None,
                        terms: &[],
                        keep: &keep,
                        style: None,
                        context: &relevant,
                    },
                )
                .await;
                (off.text, on.text)
            }
            None => (off_asr.clone(), corrected.clone()),
        };
        let ok_off = off_final.contains(name);
        let ok_on = on_final.contains(name);
        right_off += usize::from(ok_off);
        right_on += usize::from(ok_on);
        right_asr += usize::from(off_asr.contains(name));
        eprintln!(
            "{id} {name}: off {} on {} | asr {off_asr:?} -> on {on_final:?}",
            if ok_off { "ok" } else { "--" },
            if ok_on { "ok" } else { "--" },
        );
        writeln!(
            file,
            "{}",
            json!({
                "id": id, "name": name, "hints": hint_list, "relevant": relevant,
                "asr_off": off_asr, "asr_on": on_asr, "corrected": corrected,
                "final_off": off_final, "final_on": on_final,
                "right_off": ok_off, "right_on": ok_on, "added_ms": extra_ms,
            })
        )
        .unwrap();
    }
    let n = items.len() as f64;
    let p95 = percentile(&mut added_ms, 95.0);
    let summary = json!({"summary": {
        "items": items.len(),
        "right_asr": right_asr,
        "right_off": right_off,
        "right_on": right_on,
        "rate_off": right_off as f64 / n,
        "rate_on": right_on as f64 / n,
        "gain_points": (right_on as f64 - right_off as f64) / n * 100.0,
        "added_ms_p95_without_llm": p95,
    }});
    writeln!(file, "{summary}").unwrap();
    eprintln!("names_eval: {summary}");
}
