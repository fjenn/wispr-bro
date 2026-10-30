// SPDX-License-Identifier: GPL-3.0-or-later
//! Words per minute on speech time (DEC-11).
//!
//! The rate is `timed words / speech minutes`, where speech time is the
//! VAD-trimmed audio Handy kept (`entry_meta.audio_ms`), and timed words are
//! only the words of dictations whose speech time is known. A backfilled
//! entry whose audio is gone adds words but no time, so it must not raise
//! the rate.

/// Below this much speech the rate is noise; no rate is shown.
pub const MIN_SPEECH_MS: u64 = 3_000;

/// Words per minute, or `None` with too little speech time.
pub fn words_per_minute(timed_words: u64, speech_ms: u64) -> Option<f64> {
    if speech_ms < MIN_SPEECH_MS {
        return None;
    }
    Some(timed_words as f64 * 60_000.0 / speech_ms as f64)
}

/// Speech time of `samples` mono samples at Handy's 16 kHz capture rate.
pub fn samples_to_ms(samples: usize) -> u64 {
    (samples as u64).saturating_mul(1_000) / 16_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_on_speech_time() {
        assert_eq!(words_per_minute(150, 60_000), Some(150.0));
        assert_eq!(words_per_minute(50, 20_000), Some(150.0));
        assert_eq!(words_per_minute(0, 60_000), Some(0.0));
    }

    #[test]
    fn too_little_speech_has_no_rate() {
        assert_eq!(words_per_minute(10, 0), None);
        assert_eq!(words_per_minute(10, MIN_SPEECH_MS - 1), None);
        assert!(words_per_minute(10, MIN_SPEECH_MS).is_some());
    }

    #[test]
    fn samples_at_16_khz() {
        assert_eq!(samples_to_ms(16_000), 1_000);
        assert_eq!(samples_to_ms(8_000), 500);
        assert_eq!(samples_to_ms(0), 0);
    }
}
