// SPDX-License-Identifier: GPL-3.0-or-later
//! Cleanup deadlines (R5 §4.5, §5.1; S-12a).
//!
//! - **Per request:** `1.2 s + 25 ms × expected output tokens`, at most 6 s;
//!   expected tokens ≈ input characters / 4 (the RS-07 / F-10 formula).
//! - **Per take:** the same formula over the whole text. Its 6 s cap applies
//!   per started 200 words (§5.1: "51-200 words: deadline ≤ 6 s, then
//!   fail-open"), at most [`TAKE_CAP`], so a long take cleaned paragraph by
//!   paragraph (DEC-51) is not cut off after its first paragraphs. Paragraphs
//!   not cleaned by then are inserted as the deterministic text.
//! - **Server start:** a cold sidecar gets [`START_BUDGET`] to become ready
//!   (§5.1 "first take with ASR and LLM both cold": +2.5 s p95, "else that
//!   take fails open"), outside the request deadline. Prewarm at key-down
//!   (S-10) normally hides the start inside the recording.
//! - **Intel Macs:** the model runs on the CPU, so every request, take,
//!   transform and instruction deadline is [`CPU_FACTOR`] times longer there
//!   ([`for_this_mac`]); without it a cleanup would fail open on most takes.

use std::time::Duration;

/// Hard cap of one request.
pub const REQUEST_CAP: Duration = Duration::from_millis(6000);
/// Hard cap of one take.
pub const TAKE_CAP: Duration = Duration::from_secs(24);
/// How long a take waits for the server to start.
pub const START_BUDGET: Duration = Duration::from_millis(2500);

const BASE_MS: u64 = 1200;
const PER_TOKEN_MS: u64 = 25;

fn tokens(text: &str) -> u64 {
    u64::try_from(text.chars().count().div_ceil(4)).unwrap_or(u64::MAX)
}

/// How much longer a cleanup may take on an Intel Mac, where the model runs
/// on the CPU (estimate; Intel test checklist in docs/fork/INTEL.md).
pub const CPU_FACTOR: f64 = 2.5;

/// `d` for this Mac: [`CPU_FACTOR`] times longer on Intel, as is otherwise.
pub fn for_this_mac(d: Duration) -> Duration {
    if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        d.mul_f64(CPU_FACTOR)
    } else {
        d
    }
}

fn formula(text: &str) -> Duration {
    Duration::from_millis(BASE_MS.saturating_add(tokens(text).saturating_mul(PER_TOKEN_MS)))
}

/// Debug builds only (Mac test A6.1): every request gets this many
/// milliseconds, to force the "Inserted without cleanup" path.
#[cfg(debug_assertions)]
pub const FORCE_ENV: &str = "WB_DEBUG_CLEANUP_DEADLINE_MS";

/// Deadline of one request for `text`.
pub fn request(text: &str) -> Duration {
    #[cfg(debug_assertions)]
    if let Some(ms) = std::env::var(FORCE_ENV)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
    {
        return Duration::from_millis(ms);
    }
    for_this_mac(formula(text).min(REQUEST_CAP))
}

/// Floor of a transform's deadline: what the S-21 shortcut always had.
pub const TRANSFORM_FLOOR: Duration = Duration::from_millis(2800);

/// Deadline of one transform or Command Mode request on `text` (OT2-D): the
/// same per-token formula as cleanup, never below the S-21 2.8 s and at most
/// [`TAKE_CAP`]. A user-started rewrite of a long note or a multi-paragraph
/// selection needs the time a dictation of that length gets; Esc cancels.
pub fn transform(text: &str) -> Duration {
    for_this_mac(formula(text).max(TRANSFORM_FLOOR).min(TAKE_CAP))
}

/// How long the instruction model's server may take to start (XF2). It is
/// started on demand, not prewarmed by a take: its first start after launch
/// loads a 2B model and can compile Metal kernels (several seconds; PERF.md
/// "LLM cold start"), so 2.5 s cut the owner's first custom transforms off.
pub const INSTRUCT_START: Duration = Duration::from_secs(30);
/// Floor and cap of one instruction-model request (custom transforms and
/// Command Mode). Esc cancels at any time.
pub const INSTRUCT_FLOOR: Duration = Duration::from_secs(15);
pub const INSTRUCT_CAP: Duration = Duration::from_secs(60);
const INSTRUCT_BASE_MS: u64 = 5000;
/// Qwen3.5-2B: prompt processing plus a reply about as long as the text.
const INSTRUCT_PER_TOKEN_MS: u64 = 45;

/// Deadline of one instruction-model request on `text` (XF2): the 2B
/// instruction model is slower than Muesli and its replies can grow the
/// text, so it gets more room than a cleanup; a run that times out says
/// "Couldn't apply", so the deadline marks a real failure.
pub fn instruct(text: &str) -> Duration {
    for_this_mac(
        Duration::from_millis(
            INSTRUCT_BASE_MS.saturating_add(tokens(text).saturating_mul(INSTRUCT_PER_TOKEN_MS)),
        )
        .clamp(INSTRUCT_FLOOR, INSTRUCT_CAP),
    )
}

/// Deadline of a whole take of `text` with `words` words.
pub fn take(text: &str, words: u32) -> Duration {
    let blocks = u32::max(1, words.div_ceil(200));
    let cap = REQUEST_CAP.saturating_mul(blocks).min(TAKE_CAP);
    for_this_mac(formula(text).min(cap))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deadline in ms as this Mac gets it.
    fn ms(n: u64) -> Duration {
        for_this_mac(Duration::from_millis(n))
    }

    #[test]
    fn intel_gets_longer_deadlines_and_apple_silicon_the_same() {
        let d = Duration::from_millis(2000);
        if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
            assert_eq!(for_this_mac(d).as_millis(), 5000);
        } else {
            assert_eq!(for_this_mac(d), d);
        }
    }

    #[test]
    fn request_deadline_is_the_r5_formula() {
        assert_eq!(request(""), ms(1200));
        assert_eq!(request(&"x".repeat(280)), ms(2950));
        assert_eq!(request(&"x".repeat(100_000)), for_this_mac(REQUEST_CAP));
        // Same numbers as the eval scorer's deadlineMs().
        let scorer = include_str!("../../../../scripts/wb-eval-cleanup.ts");
        assert!(scorer.contains("Math.min(6000, 1200 + 25 * expectedTokens)"));
    }

    #[test]
    fn transform_deadline_scales_with_the_text() {
        assert_eq!(transform("short"), for_this_mac(TRANSFORM_FLOOR));
        // A 336-word note (~1,900 characters) gets the cleanup formula.
        assert_eq!(transform(&"x".repeat(1900)), ms(1200 + 475 * 25));
        // 675 words (~3,800 characters): 24.95 s by the formula, capped.
        assert_eq!(transform(&"x".repeat(3800)), for_this_mac(TAKE_CAP));
    }

    #[test]
    fn instruct_deadline_allows_the_slower_model_and_scales() {
        assert_eq!(instruct("short"), for_this_mac(INSTRUCT_FLOOR));
        // The owner's rc6 runs took up to 6.5 s on a sentence: well inside.
        assert!(instruct(&"x".repeat(200)) >= Duration::from_secs(10));
        assert_eq!(instruct(&"x".repeat(4000)), ms(5000 + 1000 * 45));
        assert_eq!(instruct(&"x".repeat(100_000)), for_this_mac(INSTRUCT_CAP));
        assert!(INSTRUCT_START > START_BUDGET);
    }

    #[test]
    fn take_deadline_scales_its_cap_per_200_words() {
        // ≤ 200 words: the request rule.
        assert_eq!(take(&"x".repeat(280), 50), request(&"x".repeat(280)));
        assert_eq!(take(&"x".repeat(1200), 200), ms(6000));
        // 201-400 words: up to 12 s.
        assert_eq!(take(&"x".repeat(2000), 350), ms(12_000));
        // Never more than the take cap.
        assert_eq!(take(&"x".repeat(100_000), 5000), for_this_mac(TAKE_CAP));
        assert_eq!(take("", 0), ms(1200));
    }
}
