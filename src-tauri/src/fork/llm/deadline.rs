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
    formula(text).min(REQUEST_CAP)
}

/// Deadline of a whole take of `text` with `words` words.
pub fn take(text: &str, words: u32) -> Duration {
    let blocks = u32::max(1, words.div_ceil(200));
    let cap = REQUEST_CAP.saturating_mul(blocks).min(TAKE_CAP);
    formula(text).min(cap)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_deadline_is_the_r5_formula() {
        assert_eq!(request("").as_millis(), 1200);
        assert_eq!(request(&"x".repeat(280)).as_millis(), 2950);
        assert_eq!(request(&"x".repeat(100_000)), REQUEST_CAP);
        // Same numbers as the eval scorer's deadlineMs().
        let scorer = include_str!("../../../../scripts/wb-eval-cleanup.ts");
        assert!(scorer.contains("Math.min(6000, 1200 + 25 * expectedTokens)"));
    }

    #[test]
    fn take_deadline_scales_its_cap_per_200_words() {
        // ≤ 200 words: the request rule.
        assert_eq!(take(&"x".repeat(280), 50), request(&"x".repeat(280)));
        assert_eq!(take(&"x".repeat(1200), 200).as_millis(), 6000);
        // 201-400 words: up to 12 s.
        assert_eq!(take(&"x".repeat(2000), 350).as_millis(), 12_000);
        // Never more than the take cap.
        assert_eq!(take(&"x".repeat(100_000), 5000), TAKE_CAP);
        assert_eq!(take("", 0).as_millis(), 1200);
    }
}
