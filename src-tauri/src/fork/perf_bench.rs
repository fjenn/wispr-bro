// SPDX-License-Identifier: GPL-3.0-or-later
//! The §5.1 latency bench through the shipped code (S-38), headless.
//!
//! Per take of `eval/perf/takes.jsonl` (WAVs made by
//! `scripts/wb-perf/takes.sh` with macOS TTS written to a file, never
//! played): the recognizer on the whole take as at key-up (transcribe-cpp,
//! the engine Handy's worker runs; warm model), the text pipeline
//! (`fork::text::process` around Handy's public `text.rs` stages), then,
//! with a sidecar, the cleanup at Light and Medium
//! (`fork::llm::post_process::clean`: gate, prompt, guards, deadlines, the
//! real `llama-server` through the real supervisor and transport) and the
//! Polish transform (`fork::transforms::run::generate`) on takes of
//! ≤ 150 words. Cold rows: the speech model's load and first run, repeated
//! `WB_PERF_COLD_RUNS` times, and the sidecar's start to ready.
//!
//! What is not here is GUI: the paste (S-04's soak measured text in the
//! field 13.8 ms p50 / 41.7 ms p95 after the paste started; the report adds
//! the p95 to every take), the AX read at key-up, fn-down → capture
//! (RS-01, S-03). `scripts/wb-perf/latency.ts` turns the JSONL into the
//! §5.1 rows with PASS/FAIL. The whole take is transcribed after key-up, as
//! a batch: the app's live streaming of long takes (DEC-51) only makes
//! that shorter, so the numbers are an upper bound.
//!
//! ```text
//! WB_ASR_MODEL=<gguf> WB_PERF_AUDIO=<dir> WB_PERF_OUT=<run.jsonl> \
//!   [WB_LLAMA_SERVER=<sidecar> WB_LLM_MODEL=<gguf>] [WB_PERF_COLD_RUNS=5] \
//!   cargo test --lib fork::perf_bench -- --ignored --nocapture
//! ```
//! (`scripts/wb-perf/latency.sh` does all of it.)

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use serde_json::{json, Value};

use crate::audio_toolkit::{
    normalize_transcription_output, remove_filler_words, OutputLanguageEvidence,
};
use crate::fork::context::take::ledger;
use crate::fork::llm::chat::CancellationToken;
use crate::fork::llm::post_process::{clean, LocalEngine, Request};
use crate::fork::llm::supervisor::{Config, ProcessLauncher, Supervisor};
use crate::fork::settings::CleanupLevel;
use crate::fork::transforms::TransformKind;

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

/// The 1-minute load average: a bench on a busy Mac measures the other
/// work, and the report refuses to judge it.
fn load_avg() -> f64 {
    let mut avg = [0f64; 3];
    // SAFETY: getloadavg writes at most 3 doubles into the array.
    let n = unsafe { libc::getloadavg(avg.as_mut_ptr(), 3) };
    if n >= 1 {
        avg[0]
    } else {
        -1.0
    }
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// Handy's `post_process_transcription_text` for English, from the same
/// public `text.rs` stages (as the S-11 pipeline tests build it).
/// No custom words: the dictionary is empty in the bench (its cost is
/// S-16's, bounded by `MAX_ENTRIES`).
fn handy_stage(text: String) -> String {
    let without = remove_filler_words(
        &text,
        &OutputLanguageEvidence::UserSelected("en".into()),
        &None,
        true,
    );
    normalize_transcription_output(&without)
}

fn load(asr: &str) -> transcribe_cpp::Model {
    transcribe_cpp::Model::load_with(
        asr,
        &transcribe_cpp::ModelOptions {
            backend: transcribe_cpp::Backend::Auto,
            device: None,
        },
    )
    .expect("speech model")
}

fn request(text: &str, level: CleanupLevel) -> Request<'_> {
    Request {
        text,
        level,
        language: Some("en"),
        hint: None,
        terms: &[],
        keep: &[],
        style: None,
        context: &[],
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs WB_ASR_MODEL, WB_PERF_AUDIO and WB_PERF_OUT"]
async fn latency_bench_through_the_shipped_path() {
    let (Some(asr), Some(audio), Some(out)) = (
        env("WB_ASR_MODEL"),
        env("WB_PERF_AUDIO"),
        env("WB_PERF_OUT"),
    ) else {
        eprintln!("perf_bench: set WB_ASR_MODEL, WB_PERF_AUDIO and WB_PERF_OUT");
        return;
    };
    let cold_runs: usize = env("WB_PERF_COLD_RUNS")
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let takes: Vec<Value> = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../eval/perf/takes.jsonl"),
    )
    .expect("takes.jsonl")
    .lines()
    .filter(|l| !l.trim().is_empty())
    .map(|l| serde_json::from_str(l).expect("take"))
    .collect();
    let pcm_of = |id: &str| {
        crate::audio_toolkit::read_wav_samples(PathBuf::from(&audio).join(format!("{id}.wav")))
            .expect("wav (scripts/wb-perf/takes.sh)")
    };

    transcribe_cpp::init_logging();
    transcribe_cpp::init_backends_default().expect("backends");

    // Cold: load + first run of a short take, after a drop of the model
    // (Handy's idle unload). The extra over a warm run is the §5.1 row.
    let short = pcm_of(takes[0]["id"].as_str().unwrap());
    let mut cold = Vec::new();
    for _ in 0..cold_runs {
        let t = Instant::now();
        let model = load(&asr);
        let load_ms = ms(t);
        let mut session = model.session().expect("session");
        let t = Instant::now();
        session
            .run(&short, &transcribe_cpp::RunOptions::default())
            .expect("cold run");
        let first_ms = ms(t);
        let t = Instant::now();
        session
            .run(&short, &transcribe_cpp::RunOptions::default())
            .expect("warm run");
        let warm_ms = ms(t);
        cold.push(json!({"load_ms": load_ms, "first_run_ms": first_ms, "warm_run_ms": warm_ms}));
        drop(session);
        drop(model);
    }

    let load_at_start = load_avg();
    let model = load(&asr);
    let asr_backend = model.backend();
    let mut session = model.session().expect("session");
    // Warm-up (Metal pipelines), not scored.
    let _ = session.run(&short, &transcribe_cpp::RunOptions::default());

    let dir = tempfile::tempdir().unwrap();
    let sup: Option<Arc<Supervisor>> = match (env("WB_LLAMA_SERVER"), env("WB_LLM_MODEL")) {
        (Some(bin), Some(llm)) => {
            let mut cfg = Config::new(bin.into(), dir.path().join("llm"));
            cfg.model = Some(llm.into());
            Some(Supervisor::new(Arc::new(ProcessLauncher), cfg))
        }
        _ => None,
    };
    let mut llm_start_ms = None;
    if let Some(sup) = &sup {
        let t = Instant::now();
        sup.ensure_ready().await.expect("sidecar starts");
        llm_start_ms = Some(ms(t));
    }
    let polish = {
        let db = crate::fork::db::ForkDb::in_dir(dir.path());
        db.with_conn(|c| crate::fork::transforms::record::definition(c, TransformKind::Polish))
            .expect("fork.db")
            .expect("Polish is seeded")
            .instruction
    };

    let mut file = std::fs::File::create(&out).expect("out");
    writeln!(
        file,
        "{}",
        json!({"meta": {
            "runner": "src-tauri/src/fork/perf_bench.rs (S-38, shipped path)",
            "asr": std::path::Path::new(&asr).file_name().map(|f| f.to_string_lossy().to_string()),
            "llm": env("WB_LLM_MODEL").as_deref().and_then(|m| std::path::Path::new(m).file_name().map(|f| f.to_string_lossy().to_string())),
            "debug_build": cfg!(debug_assertions),
            "asr_backend": asr_backend,
            "cores": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0),
            "cold": cold,
            "llm_start_ms": llm_start_ms,
        }})
    )
    .unwrap();

    if let Some(sup) = &sup {
        // Warm-up request (prompt cache), not scored.
        let engine = LocalEngine {
            supervisor: sup,
            pill: false,
        };
        let _ = clean(
            &engine,
            &request("um so this is a warm up", CleanupLevel::Light),
        )
        .await;
    }

    for (i, take) in takes.iter().enumerate() {
        let id = take["id"].as_str().unwrap();
        let pcm = pcm_of(id);
        let audio_s = pcm.len() as f64 / 16_000.0;
        let t = Instant::now();
        let raw = session
            .run(&pcm, &transcribe_cpp::RunOptions::default())
            .map(|r| r.text.trim().to_owned())
            .unwrap_or_default();
        let asr_ms = ms(t);
        let t = Instant::now();
        let kept = crate::fork::text::process(
            raw.clone(),
            &OutputLanguageEvidence::UserSelected("en".into()),
            handy_stage,
        );
        let text_ms = ms(t);
        let _ = ledger().take_processed(&kept);
        let words = crate::fork::llm::markers::words(&kept).len();

        let mut row = json!({
            "id": id, "audio_s": audio_s, "words": words,
            "asr_ms": asr_ms, "text_ms": text_ms,
        });
        if let Some(sup) = &sup {
            let engine = LocalEngine {
                supervisor: sup,
                pill: false,
            };
            for level in [CleanupLevel::Light, CleanupLevel::Medium] {
                let t = Instant::now();
                let cleaned = clean(&engine, &request(&kept, level)).await;
                row[level.as_str()] = json!({
                    "ms": ms(t),
                    "outcome": cleaned.outcome.as_str(),
                    "why": cleaned.why.map(|w| w.as_str()),
                });
            }
            if words <= 150 {
                let t = Instant::now();
                let r = crate::fork::transforms::run::generate(
                    &engine,
                    TransformKind::Polish,
                    &polish,
                    &kept,
                    &CancellationToken::new(),
                )
                .await;
                row["transform"] = json!({
                    "ms": ms(t),
                    "ok": r.is_ok(),
                    "error": r.err().map(|e| format!("{e:?}")),
                });
            }
        }
        writeln!(file, "{row}").unwrap();
        eprintln!(
            "perf_bench: {}/{} {id} asr {asr_ms:.0} ms",
            i + 1,
            takes.len()
        );
    }
    // The load during the run, as the report's last line.
    writeln!(
        file,
        "{}",
        json!({"load": {"start": load_at_start, "end": load_avg()}})
    )
    .unwrap();
    if let Some(sup) = sup {
        sup.stop().await;
    }
}
