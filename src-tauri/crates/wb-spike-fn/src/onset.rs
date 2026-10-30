// SPDX-License-Identifier: GPL-3.0-or-later
//! Speech onset in a captured take, to tell "clipped by us" from "the
//! speaker started late" (RS-01 round 2). Pure code, unit-tested.
//!
//! The rule: a take is **clipped by us** when speech is already present in
//! the very first captured frames (it began before the mic was live). A take
//! with leading silence is **not clipped**: the speaker started after the mic
//! was live, whatever the transcript says.
//!
//! Method (energy, 10 ms frames, no model):
//! 1. Skip leading digital zeros (the first callback of a cold open is often
//!    a frame of exact zeros).
//! 2. Noise floor = 10th percentile of the frame levels (dBFS), speech level
//!    = 95th percentile. Less than 10 dB apart → no clear speech.
//! 3. Threshold = floor + max(8 dB, 35 % of the range). Onset = the first
//!    frame that starts a 60 ms window with at least 50 ms above threshold,
//!    then walked back (≤ 100 ms) over frames still 4 dB above the floor, so
//!    a soft "t" counts as speech.
//! 4. Onset < [`AT_START_MS`] after the first non-zero sample → clipped by us.
//!
//! This needs some silence somewhere in the clip to find the floor; a take
//! saves 4 s, and "testing one two three" lasts about 1.3 s.

use serde::{Deserialize, Serialize};

pub const FRAME_MS: usize = 10;
/// Speech starting this close to the start of the capture counts as "already
/// on when the mic went live".
pub const AT_START_MS: f64 = 50.0;
const MIN_RANGE_DB: f64 = 10.0;
const WINDOW: usize = 6;
const WINDOW_HITS: usize = 5;
const WALK_BACK: usize = 10;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Onset {
    /// Leading digital silence skipped, ms.
    pub lead_zero_ms: f64,
    pub floor_db: f64,
    pub speech_db: f64,
    pub threshold_db: f64,
    /// Speech onset after the first non-zero sample, ms (None: no speech).
    pub onset_ms: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Clip {
    /// Speech already present in the first captured frames.
    ClippedByUs,
    /// Leading silence: the speaker started after the mic was live.
    NotClipped,
    /// No clear speech in the clip.
    NoSpeech,
    /// No audio (no WAV, or an empty one).
    NoAudio,
}

impl Clip {
    pub fn word(self) -> &'static str {
        match self {
            Clip::ClippedByUs => "CLIPPED BY US",
            Clip::NotClipped => "not clipped",
            Clip::NoSpeech => "no speech found",
            Clip::NoAudio => "no audio",
        }
    }
}

fn frame_db(frame: &[f32]) -> f64 {
    let e = frame.iter().map(|&x| (x as f64) * (x as f64)).sum::<f64>() / frame.len() as f64;
    10.0 * e.max(1e-12).log10()
}

fn pct(sorted: &[f64], p: f64) -> f64 {
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

/// Finds the speech onset in mono `samples` at `rate`.
pub fn analyse(samples: &[f32], rate: u32) -> Option<Onset> {
    let n = (rate as usize * FRAME_MS / 1000).max(1);
    let lead = samples.iter().take_while(|&&x| x == 0.0).count();
    let lead_frames = lead / n;
    let body = &samples[lead_frames * n..];
    let db: Vec<f64> = body.chunks_exact(n).map(frame_db).collect();
    if db.len() < WINDOW {
        return None;
    }
    let mut sorted = db.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let floor = pct(&sorted, 0.10);
    let speech = pct(&sorted, 0.95);
    let threshold = floor + (0.35 * (speech - floor)).max(8.0);
    let lead_zero_ms = (lead_frames * FRAME_MS) as f64;
    let mut out = Onset {
        lead_zero_ms,
        floor_db: floor,
        speech_db: speech,
        threshold_db: threshold,
        onset_ms: None,
    };
    if speech - floor < MIN_RANGE_DB {
        return Some(out);
    }
    let first = (0..=db.len() - WINDOW).find(|&i| {
        db[i] > threshold
            && db[i..i + WINDOW].iter().filter(|&&v| v > threshold).count() >= WINDOW_HITS
    });
    if let Some(mut i) = first {
        let stop = i.saturating_sub(WALK_BACK);
        while i > stop && db[i - 1] > floor + 4.0 {
            i -= 1;
        }
        out.onset_ms = Some((i * FRAME_MS) as f64);
    }
    Some(out)
}

pub fn classify(o: Option<&Onset>) -> Clip {
    match o {
        None => Clip::NoAudio,
        Some(Onset { onset_ms: None, .. }) => Clip::NoSpeech,
        Some(Onset {
            onset_ms: Some(ms), ..
        }) if *ms < AT_START_MS => Clip::ClippedByUs,
        Some(_) => Clip::NotClipped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 16_000;

    /// Room noise at about -45 dBFS, with "speech" (a 200 Hz tone at about
    /// -25 dBFS) from `from_ms` to `to_ms`, after `zeros_ms` of exact zeros.
    fn take(zeros_ms: usize, from_ms: usize, to_ms: usize, total_ms: usize) -> Vec<f32> {
        let mut seed: u32 = 12345;
        let per_ms = RATE as usize / 1000;
        (0..total_ms * per_ms)
            .map(|i| {
                if i < zeros_ms * per_ms {
                    return 0.0;
                }
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                let noise = ((seed >> 16) as f32 / 32768.0 - 1.0) * 0.01;
                let t = i as f32 / RATE as f32;
                let ms = i / per_ms;
                let voice = if (from_ms..to_ms).contains(&ms) {
                    0.08 * (2.0 * std::f32::consts::PI * 200.0 * t).sin()
                } else {
                    0.0
                };
                noise + voice
            })
            .collect()
    }

    #[test]
    fn leading_silence_means_not_clipped() {
        let o = analyse(&take(10, 600, 1900, 4000), RATE).unwrap();
        assert_eq!(o.lead_zero_ms, 10.0);
        let on = o.onset_ms.unwrap();
        assert!((570.0..=600.0).contains(&on), "{on}");
        assert_eq!(classify(Some(&o)), Clip::NotClipped);
    }

    #[test]
    fn speech_from_the_first_frame_is_clipped_by_us() {
        let o = analyse(&take(10, 0, 1300, 4000), RATE).unwrap();
        assert_eq!(o.onset_ms, Some(0.0));
        assert_eq!(classify(Some(&o)), Clip::ClippedByUs);
    }

    #[test]
    fn speech_just_inside_the_start_window_is_clipped() {
        // The take's own leading zeros do not count as silence.
        let o = analyse(&take(30, 30, 1300, 4000), RATE).unwrap();
        assert_eq!(o.lead_zero_ms, 30.0);
        assert!(o.onset_ms.unwrap() < AT_START_MS);
        assert_eq!(classify(Some(&o)), Clip::ClippedByUs);
    }

    #[test]
    fn a_short_click_is_not_speech_onset() {
        let mut x = take(0, 800, 2000, 4000);
        for v in &mut x[1600..1680] {
            *v += 0.3; // 5 ms click at 100 ms
        }
        let o = analyse(&x, RATE).unwrap();
        assert!(o.onset_ms.unwrap() >= 770.0, "{:?}", o.onset_ms);
    }

    #[test]
    fn noise_only_is_no_speech() {
        let o = analyse(&take(0, 0, 0, 4000), RATE).unwrap();
        assert_eq!(o.onset_ms, None);
        assert_eq!(classify(Some(&o)), Clip::NoSpeech);
        assert_eq!(classify(None), Clip::NoAudio);
        assert!(analyse(&[0.0; 10], RATE).is_none());
    }
}
