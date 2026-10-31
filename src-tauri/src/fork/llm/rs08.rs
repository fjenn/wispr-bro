// SPDX-License-Identifier: GPL-3.0-or-later
//! RS-08 bench (test-only): Apple Foundation Models through the shipped
//! cleanup path ([`super::post_process::clean`] with [`AppleEngine`]).
//!
//! ```text
//! WB_RS08_OUT=<result.json> [WB_RS08_IDLE_S=25] [WB_RS08_COLD=8] [WB_RS08_PREWARMED=16] \
//!   cargo test --lib fork::llm::rs08 -- --ignored --nocapture
//! ```
//!
//! - **cold**: no prewarm, after `WB_RS08_IDLE_S` seconds without a request
//!   (the system unloads the model a few seconds after one);
//! - **prewarmed**: the same idle gap, then [`super::apple::prewarm`] with the
//!   take's system prompt, 1.5 s of "recording", then the request (the app's
//!   key-down prewarm);
//! - **warm**: back to back;
//! - **burst**: `WB_RS08_BURST` (60) requests back to back, counting
//!   `rateLimited` and other errors.
//!
//! The 50-word samples ([`SAMPLES_50`]) are our own text, written for RS-08.

use std::time::{Duration, Instant};

use serde_json::json;

use super::apple::{self, AppleEngine};
use super::post_process::{clean, system_prompt, Cleaned, Request};
use crate::fork::settings::CleanupLevel;

use super::apple::SAMPLES_50;

fn env_u64(k: &str, d: u64) -> u64 {
    std::env::var(k)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(d)
}

fn pct(v: &[f64], p: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let rank = (p / 100.0 * (s.len() - 1) as f64).round() as usize;
    s[rank.min(s.len() - 1)]
}

fn summary(v: &[f64]) -> serde_json::Value {
    json!({
        "n": v.len(),
        "p50_ms": pct(v, 50.0).round(),
        "p95_ms": pct(v, 95.0).round(),
        "max_ms": v.iter().cloned().fold(f64::NAN, f64::max).round(),
    })
}

fn req(text: &str) -> Request<'_> {
    Request {
        text,
        level: CleanupLevel::Light,
        language: Some("en"),
        hint: None,
        terms: &[],
        keep: &[],
    }
}

fn row(phase: &str, i: usize, c: &Cleaned, wall: f64) -> serde_json::Value {
    json!({
        "phase": phase,
        "i": i,
        "outcome": c.outcome.as_str(),
        "llm_ms": c.llm_ms,
        "request_ms": c.chunks.iter().map(|t| t.ms).sum::<u32>(),
        "wall_ms": wall,
        "errors": c.chunks.iter().filter_map(|t| t.error.clone()).collect::<Vec<_>>(),
        "reasons": c.guard_reasons(),
        "output": c.text,
    })
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs macOS 26 with Apple Intelligence on, and WB_RS08_OUT"]
async fn rs08_apple_latency_and_burst() {
    let Ok(out) = std::env::var("WB_RS08_OUT") else {
        eprintln!("rs08: set WB_RS08_OUT");
        return;
    };
    assert_eq!(apple::availability(), apple::AppleAvailability::Available);
    let idle = Duration::from_secs(env_u64("WB_RS08_IDLE_S", 25));
    let n_cold = env_u64("WB_RS08_COLD", 8) as usize;
    let n_pre = env_u64("WB_RS08_PREWARMED", 16) as usize;
    let n_burst = env_u64("WB_RS08_BURST", 60) as usize;
    let engine = AppleEngine {
        pill: false,
        ..AppleEngine::for_dictation()
    };
    let mut rows = Vec::new();
    let (mut cold, mut pre, mut warm) = (Vec::new(), Vec::new(), Vec::new());
    let words: Vec<usize> = SAMPLES_50
        .iter()
        .map(|s| s.split_whitespace().count())
        .collect();
    eprintln!(
        "rs08: sample words {words:?}, context {}",
        wb_apple_fm::context_size()
    );

    for i in 0..n_cold {
        tokio::time::sleep(idle).await;
        let t = SAMPLES_50[i % SAMPLES_50.len()];
        let t0 = Instant::now();
        let c = clean(&engine, &req(t)).await;
        let wall = t0.elapsed().as_secs_f64() * 1000.0;
        eprintln!("rs08: cold {i}: {} ms {}", c.llm_ms, c.outcome.as_str());
        cold.push(f64::from(c.llm_ms));
        rows.push(row("cold", i, &c, wall));
    }
    for i in 0..n_pre {
        tokio::time::sleep(idle).await;
        let t = SAMPLES_50[(i + 3) % SAMPLES_50.len()];
        apple::prewarm(&system_prompt(&req(t)));
        tokio::time::sleep(Duration::from_millis(1500)).await;
        let t0 = Instant::now();
        let c = clean(&engine, &req(t)).await;
        let wall = t0.elapsed().as_secs_f64() * 1000.0;
        eprintln!(
            "rs08: prewarmed {i}: {} ms {}",
            c.llm_ms,
            c.outcome.as_str()
        );
        pre.push(f64::from(c.llm_ms));
        rows.push(row("prewarmed", i, &c, wall));
    }
    let mut burst_errors: std::collections::BTreeMap<String, u32> = Default::default();
    for i in 0..n_burst {
        let t = SAMPLES_50[i % SAMPLES_50.len()];
        let t0 = Instant::now();
        let c = clean(&engine, &req(t)).await;
        let wall = t0.elapsed().as_secs_f64() * 1000.0;
        for e in c.chunks.iter().filter_map(|t| t.error.clone()) {
            *burst_errors.entry(e).or_default() += 1;
        }
        if i > 0 {
            warm.push(f64::from(c.llm_ms));
        }
        rows.push(row("burst", i, &c, wall));
    }
    let applied = |phase: &str| {
        rows.iter()
            .filter(|r| r["phase"] == phase && r["outcome"] == "applied")
            .count()
    };
    let result = json!({
        "meta": {
            "runner": "src-tauri/src/fork/llm/rs08.rs",
            "engine": format!(
                "apple ({}, {:?} guardrails)",
                if engine.guided { "guided" } else { "plain" },
                engine.guardrails
            ),
            "context_tokens": wb_apple_fm::context_size(),
            "idle_s": idle.as_secs(),
            "sample_words": words,
        },
        "cold": summary(&cold),
        "prewarmed": summary(&pre),
        "warm": summary(&warm),
        "applied": {
            "cold": applied("cold"),
            "prewarmed": applied("prewarmed"),
            "burst": applied("burst"),
        },
        "burst_errors": burst_errors,
        "rows": rows,
    });
    eprintln!(
        "rs08: cold {} prewarmed {} warm {} burst errors {:?}",
        result["cold"], result["prewarmed"], result["warm"], result["burst_errors"]
    );
    std::fs::write(&out, serde_json::to_string_pretty(&result).unwrap()).unwrap();
}
