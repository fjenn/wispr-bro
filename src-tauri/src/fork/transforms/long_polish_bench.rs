// SPDX-License-Identifier: GPL-3.0-or-later
//! Audit OT2: Scratchpad › Polish (`fork_scratchpad_polish` →
//! `run::preview` → [`super::run::generate_definition`]) on notes of
//! growing length, through the real sidecar and the seeded Polish
//! definition, the same call the app makes. The audit found one fixed 2.8 s
//! budget for every transform; since OT2-D the budget scales with the text
//! (`run::budget`, 2.8-24 s), and this measures where Polish stops working.
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
    let _ = super::run::generate_definition(
        Some(&engine),
        &polish,
        &paras[0],
        &CancellationToken::new(),
    )
    .await;
    for n in 1..=paras.len() {
        let note = paras[..n].join("\n\n");
        let words = note.split_whitespace().count();
        let t = Instant::now();
        let r = super::run::generate_definition(
            Some(&engine),
            &polish,
            &note,
            &CancellationToken::new(),
        )
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

/// Realistic scratchpad notes (our own text): dictated paragraphs with the
/// slips a quick note has (rc9: Polish refused every note on Apple's model
/// with `NewTokens` and `ContentWords`).
pub(super) const NOTES: [&str; 6] = [
    "so for the meeting tomorrow i think we should talk about the budget first and then the hiring plan. sarah said the numbers are not final yet but she will send them tonight.\n\nalso we need to decide if we move the launch to march or keep it in february, i lean towards march because the design isnt ready.",
    "Ideas for the weekend: go to the market on saturday morning, call mom about her birthday, and maybe finally fix the bike. The back tire has been flat for two weeks now and its getting annoying.",
    "morgen moet ik de huur betalen en de verzekering bellen want die hebben het verkeerde adres. daarna boodschappen doen voor het etentje van vrijdag, we zijn met zes mensen.\n\nniet vergeten om peter te vragen of hij de stoelen kan meenemen.",
    "Feedback on the onboarding flow. The first screen is to busy, there are three buttons that all look the same and users dont know which one to press. The permission step works well but the text is to long. I would cut it in half and add a picture.",
    "notes from call with the landlord. he says the heating will be fixed next week, probably tuesday. the rent goes up by 3 percent from january. i asked about the parking spot and he will check with the owner of the building.",
    "Quick thought about the app: when you dictate a long note the text appears all at once, which feels slow. It would be nicer if it showed up while you speak, even if it gets corrected later. Not sure how hard this is with the current model though.",
];

/// `WB_POLISH_ENGINE=apple` (macOS 26, Apple Intelligence on) or `local`
/// (with `WB_LLAMA_SERVER` and `WB_LLM_MODEL`): each note's reply and the
/// guards' verdict.
///
/// ```text
/// WB_POLISH_ENGINE=apple cargo test --lib \
///   fork::transforms::long_polish_bench::polish_realistic_notes -- --ignored --nocapture
/// ```
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs WB_POLISH_ENGINE (apple, or local with WB_LLAMA_SERVER and WB_LLM_MODEL)"]
async fn polish_realistic_notes() {
    use crate::fork::llm::post_process::Engine;
    let Ok(which) = std::env::var("WB_POLISH_ENGINE") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let db = crate::fork::db::ForkDb::in_dir(dir.path());
    let polish = db
        .with_conn(|c| super::record::definition(c, TransformKind::Polish))
        .expect("fork.db")
        .expect("Polish is seeded");
    let sup = if which == "local" {
        let bin = std::env::var("WB_LLAMA_SERVER").expect("WB_LLAMA_SERVER");
        let llm = std::env::var("WB_LLM_MODEL").expect("WB_LLM_MODEL");
        let mut cfg = Config::new(bin.into(), dir.path().join("llm"));
        cfg.model = Some(llm.into());
        let sup = Arc::new(Supervisor::new(Arc::new(ProcessLauncher), cfg));
        sup.ensure_ready().await.expect("sidecar starts");
        Some(sup)
    } else {
        None
    };
    let mut accepted = 0;
    for note in NOTES {
        let (system, request) = super::run::request_for(&polish, note);
        let budget = super::run::budget(note).max(std::time::Duration::from_secs(30));
        let raw = match &sup {
            Some(sup) => {
                LocalEngine {
                    supervisor: sup,
                    pill: false,
                }
                .complete(request, budget)
                .await
            }
            None => {
                crate::fork::llm::apple::AppleEngine {
                    pill: false,
                    ..crate::fork::llm::apple::AppleEngine::for_dictation()
                }
                .complete(request, budget)
                .await
            }
        };
        let Ok(raw) = raw else {
            println!("--- engine error: {raw:?}");
            continue;
        };
        let json = super::run::as_reply_json(&raw);
        let verdict = super::run::guarded_reply(&json, polish.guard, note, &system);
        let all = crate::fork::llm::guards::check(
            &json,
            &crate::fork::llm::guards::Context {
                input: note,
                level: crate::fork::settings::CleanupLevel::Medium,
                language: None,
                system_prompt: &system,
                terms: &[],
            },
        );
        accepted += usize::from(verdict.is_ok());
        println!(
            "--- {which}: {:?} (all reasons {:?})\nIN:  {note}\nOUT: {}\n",
            verdict.as_ref().map(|_| "accepted"),
            all.reasons,
            all.text.unwrap_or_default()
        );
    }
    println!("{which}: {accepted}/{} notes polished", NOTES.len());
}
