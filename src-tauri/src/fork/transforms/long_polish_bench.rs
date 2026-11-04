// SPDX-License-Identifier: GPL-3.0-or-later
//! Audit OT2: Scratchpad › Polish (`fork_scratchpad_polish` →
//! `run::preview` → [`super::run::generate_definition`]) on notes of
//! growing length, through the real sidecar and the seeded Polish
//! definition, the same call the app makes. Every transform has one fixed
//! generation budget (`run::LLM_BUDGET`, 2.8 s), while a note may hold
//! 200,000 characters; this measures where Polish stops working.
//!
//! ```text
//! WB_LLAMA_SERVER=<sidecar> WB_LLM_MODEL=<gguf> \
//!   cargo test --lib fork::transforms::long_polish_bench -- --ignored --nocapture
//! ```

use std::sync::Arc;
use std::time::Instant;

use crate::fork::llm::chat::CancellationToken;
use crate::fork::llm::post_process::LocalEngine;
use crate::fork::llm::supervisor::{Config, ProcessLauncher, Supervisor};
use crate::fork::transforms::TransformKind;

fn paragraphs() -> Vec<String> {
    let src = include_str!("../../../../eval/perf/takes.jsonl");
    src.lines()
        .filter(|l| l.contains("\"m0"))
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| v["text"].as_str().map(str::to_owned))
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs WB_LLAMA_SERVER and WB_LLM_MODEL"]
async fn scratchpad_polish_on_long_notes() {
    let (Ok(bin), Ok(llm)) = (
        std::env::var("WB_LLAMA_SERVER"),
        std::env::var("WB_LLM_MODEL"),
    ) else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::new(bin.into(), dir.path().join("llm"));
    cfg.model = Some(llm.into());
    let sup = Arc::new(Supervisor::new(Arc::new(ProcessLauncher), cfg));
    sup.ensure_ready().await.expect("sidecar starts");
    let db = crate::fork::db::ForkDb::in_dir(dir.path());
    let polish = db
        .with_conn(|c| super::record::definition(c, TransformKind::Polish))
        .expect("fork.db")
        .expect("Polish is seeded");
    let engine = LocalEngine {
        supervisor: &sup,
        pill: false,
    };
    let paras = paragraphs();
    assert!(
        paras.len() >= 4,
        "eval/perf/takes.jsonl has the m0x paragraphs"
    );
    // Warm-up, not scored.
    let _ = super::run::generate_definition(&engine, &polish, &paras[0], &CancellationToken::new())
        .await;
    for n in 1..=paras.len() {
        let note = paras[..n].join("\n\n");
        let words = note.split_whitespace().count();
        let t = Instant::now();
        let r = super::run::generate_definition(&engine, &polish, &note, &CancellationToken::new())
            .await;
        println!(
            "polish note of {words} words: {} in {} ms",
            match &r {
                Ok(_) => "ok".to_string(),
                Err(e) => format!("{e:?}"),
            },
            t.elapsed().as_millis()
        );
    }
}
