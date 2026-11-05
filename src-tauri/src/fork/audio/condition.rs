// SPDX-License-Identifier: GPL-3.0-or-later
//! Input conditioning for quiet speech and whispers (OT2-B).
//!
//! What dropped quiet takes was the voice detector, not the recognizer
//! (`quiet_bench`, both default English models): Silero (and Earshot)
//! hear a voice 30 dB under dictation level in a café as noise, so the
//! recorder kept nothing or only fragments of a whisper and the take came
//! out empty ("No speech heard") or with words missing. Whisper large-v3
//! turbo and Parakeet unified decode the same quiet audio well once they
//! get it.
//!
//! [`detector`] therefore wraps the take's detector (one hook in Handy's
//! `create_detector`, so the recording's VAD and long-take chunking both
//! get it): each frame is judged on a copy brought towards speech level,
//! the recording keeps the original samples. The gain follows the recent
//! speech peak (up at once, down 6 dB/s, so a pause does not pump it) and
//! is limited by the room: never more than [`MAX_GAIN_DB`], and never so
//! much that the noise floor would rise above [`NOISE_CEILING_DB`], so a
//! quiet room's hiss or a café's noise is not raised into "speech".
//!
//! Mac wave 7: a whisper as loud as the café around it (0 dB SNR) gets no
//! gain (the noise cap), and Silero kept as little as 1.25 s of a 3.6 s
//! sentence. Each frame now also counts as speech when the spectral voice
//! presence of [`super::speech`] (the meter's) reaches [`VOICED_PRESENCE`]:
//! over the quiet set (turbo) whisper-30/35 at 0 dB WER 29.5 → 9.5 %, no
//! empty take (was 1/8 each), all tiers 5.8 → 3.7 %, normal and loud
//! unchanged, every noise-only take still empty.
//!
//! A take-level gain before the recognizer (`for_recognizer`, tests only)
//! was built and measured too: one gain per take towards [`TARGET_DB`]
//! with the same limits. It changed nothing for either model (turbo WER
//! 3.6 → 3.5 %, unified 5.0 → 5.1 % over the set, noise-only takes empty
//! either way), and applying it means dropping a take's live decode, so it
//! is not in the take path; the bench keeps it to re-measure when a default
//! model changes.

use anyhow::Result;

use crate::audio_toolkit::vad::{SpeechScorer, VadFrame, VoiceActivityDetector};

/// The level speech is brought to (active speech, dBFS).
pub const TARGET_DB: f32 = -24.0;
/// Most gain added.
pub const MAX_GAIN_DB: f32 = 30.0;
/// The noise floor is never lifted above this (dBFS).
pub const NOISE_CEILING_DB: f32 = -50.0;
/// Spectral voice presence that counts a frame as speech by itself.
pub const VOICED_PRESENCE: f32 = 0.6;
/// Peaks are kept under this after the take gain (linear).
#[cfg(test)]
pub const PEAK_LIMIT: f32 = 0.9;
/// A take needing less gain than this is left as it is.
#[cfg(test)]
pub const MIN_GAIN_DB: f32 = 6.0;

/// 20 ms frames' dB, `(level, floor)` of a clip: its active level (P.56-
/// like: the frames within 25 dB of the loudest) and its noise floor (the
/// 10th percentile frame).
#[cfg(test)]
pub fn levels_of(samples: &[f32]) -> (f32, f32) {
    let frames: Vec<f32> = samples
        .chunks(320)
        .filter(|c| c.len() == 320)
        .map(|c| {
            let ms = c.iter().map(|s| s * s).sum::<f32>() / c.len() as f32;
            if ms > 1e-12 {
                10.0 * ms.log10()
            } else {
                -120.0
            }
        })
        .collect();
    if frames.is_empty() {
        return (-120.0, -120.0);
    }
    let top = frames.iter().copied().fold(-120.0, f32::max);
    let active: Vec<f32> = frames.iter().copied().filter(|d| *d > top - 25.0).collect();
    let ms = active.iter().map(|d| 10f32.powf(d / 10.0)).sum::<f32>() / active.len() as f32;
    let mut sorted = frames.clone();
    sorted.sort_by(f32::total_cmp);
    let floor = sorted[(sorted.len() - 1) / 10];
    (10.0 * ms.log10(), floor)
}

/// The gain (dB, ≥ 0) that brings `level` to `target` within the limits.
pub fn gain_db(level: f32, floor: f32, target: f32) -> f32 {
    gain_db_under(level, floor, target, NOISE_CEILING_DB)
}

/// [`gain_db`] with another noise ceiling (the bench compares them).
fn gain_db_under(level: f32, floor: f32, target: f32, ceiling: f32) -> f32 {
    if level <= -119.0 {
        return 0.0;
    }
    (target - level)
        .min(ceiling - floor)
        .clamp(0.0, MAX_GAIN_DB)
}

/// A take before the recognizer: one gain towards [`TARGET_DB`], with
/// peaks kept under [`PEAK_LIMIT`]. Loud and ordinary takes are left
/// exactly as they are. Measured, not used (module docs).
#[cfg(test)]
pub fn for_recognizer(samples: Vec<f32>) -> Vec<f32> {
    for_recognizer_under(samples, NOISE_CEILING_DB)
}

/// [`for_recognizer`] with another noise ceiling (the bench).
#[cfg(test)]
pub fn for_recognizer_under(mut samples: Vec<f32>, ceiling: f32) -> Vec<f32> {
    let (level, floor) = levels_of(&samples);
    let mut g = gain_db_under(level, floor, TARGET_DB, ceiling);
    if g < MIN_GAIN_DB {
        return samples;
    }
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if peak > 0.0 {
        g = g.min(20.0 * (PEAK_LIMIT / peak).log10());
    }
    if g < MIN_GAIN_DB {
        return samples;
    }
    log::debug!(
        "fork audio: quiet take raised {g:.1} dB (level {level:.1}, floor {floor:.1} dBFS)"
    );
    let k = 10f32.powf(g / 20.0);
    samples.iter_mut().for_each(|s| *s *= k);
    samples
}

/// The gain for a take of this level, if [`for_recognizer`] would apply one.
#[cfg(test)]
pub fn take_gain_db(samples: &[f32]) -> f32 {
    let (level, floor) = levels_of(samples);
    let g = gain_db(level, floor, TARGET_DB);
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let g = if peak > 0.0 {
        g.min(20.0 * (PEAK_LIMIT / peak).log10())
    } else {
        g
    };
    if g < MIN_GAIN_DB {
        0.0
    } else {
        g
    }
}

/// The detector's level tracker: the recent speech peak (fast up, slow
/// down) and the noise floor (a slow minimum), per frame.
#[derive(Clone, Debug)]
struct Tracker {
    peak: f32,
    floor: f32,
    frames: usize,
}

impl Tracker {
    fn new() -> Self {
        Self {
            peak: -120.0,
            floor: -120.0,
            frames: 0,
        }
    }

    /// Feeds one frame's dB; returns the gain (dB) to score it with.
    fn push(&mut self, db: f32, frame_ms: f32) -> f32 {
        self.frames += 1;
        if self.frames == 1 {
            self.peak = db;
            self.floor = db;
        }
        // Peak: up at once, down 6 dB/s (a pause does not raise the gain).
        self.peak = db.max(self.peak - 6.0 * frame_ms / 1000.0);
        // Floor: down at once, up 3 dB/s.
        self.floor = db.min(self.floor + 3.0 * frame_ms / 1000.0);
        // The gain follows the louder of the recent peak and the frame, so
        // the onset of a loud word is never boosted.
        gain_db(self.peak.max(db) - 6.0, self.floor, TARGET_DB)
    }
}

/// A detector whose frames are judged at a conditioned level.
pub struct Conditioned {
    inner: Box<dyn SpeechScorer>,
    tracker: Tracker,
    scratch: Vec<f32>,
    meter: crate::fork::audio::speech::SpeechMeter,
}

/// Wraps `inner`, which keeps its own threshold and state; it sees each
/// frame at the conditioned level, the caller gets the original frame.
pub fn detector(inner: Box<dyn SpeechScorer>) -> Box<dyn SpeechScorer> {
    Box::new(Conditioned {
        inner,
        tracker: Tracker::new(),
        scratch: Vec::new(),
        meter: crate::fork::audio::speech::SpeechMeter::new(16_000),
    })
}

impl Conditioned {
    /// Whether the spectral voice presence of [`super::speech`] hears a
    /// voice in this frame (a whisper under café noise that Silero misses).
    fn voiced(&mut self, frame: &[f32]) -> bool {
        self.meter.push(frame, 1).presence >= VOICED_PRESENCE
    }

    fn scaled(&mut self, frame: &[f32]) -> f32 {
        let ms = frame.iter().map(|s| s * s).sum::<f32>() / frame.len().max(1) as f32;
        let db = if ms > 1e-12 {
            10.0 * ms.log10()
        } else {
            -120.0
        };
        let frame_ms = frame.len() as f32 / 16.0;
        let g = self.tracker.push(db, frame_ms);
        let k = 10f32.powf(g / 20.0);
        self.scratch.clear();
        self.scratch
            .extend(frame.iter().map(|s| (s * k).clamp(-1.0, 1.0)));
        g
    }
}

impl VoiceActivityDetector for Conditioned {
    fn push_frame<'a>(&'a mut self, frame: &'a [f32]) -> Result<VadFrame<'a>> {
        self.scaled(frame);
        let voiced = self.voiced(frame);
        let scratch = std::mem::take(&mut self.scratch);
        let speech = self.inner.push_frame(&scratch).map(|f| f.is_speech());
        self.scratch = scratch;
        Ok(if speech? || voiced {
            VadFrame::Speech(frame)
        } else {
            VadFrame::Noise
        })
    }

    fn frame_samples(&self) -> usize {
        self.inner.frame_samples()
    }

    fn set_hangover_frames(&mut self, frames: usize) {
        self.inner.set_hangover_frames(frames);
    }

    fn reset(&mut self) {
        self.inner.reset();
        self.tracker = Tracker::new();
        self.meter = crate::fork::audio::speech::SpeechMeter::new(16_000);
    }
}

impl SpeechScorer for Conditioned {
    fn score(&mut self, frames: &[f32]) -> Result<Vec<f32>> {
        let n = self.inner.frame_samples().max(1);
        let mut out = Vec::with_capacity(frames.len() / n);
        for frame in frames.chunks(n) {
            self.scaled(frame);
            let voiced = self.voiced(frame);
            let scratch = std::mem::take(&mut self.scratch);
            let p = self.inner.score(&scratch);
            self.scratch = scratch;
            out.extend(p?.into_iter().map(|p| if voiced { p.max(1.0) } else { p }));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::audio::quiet_set::{db, noise_take, synth_speech, take};

    #[test]
    fn ordinary_and_loud_takes_are_left_exactly_as_they_are() {
        for level in [-26.0, -20.0, -12.0] {
            let t = take(
                &synth_speech(2.0, 3, false),
                level,
                Some(20.0),
                5,
                &[],
                0.5,
                0.5,
            );
            assert_eq!(take_gain_db(&t), 0.0, "{level}");
            assert_eq!(for_recognizer(t.clone()), t, "{level}");
        }
    }

    #[test]
    fn a_quiet_take_reaches_dictation_level() {
        // A whisper 30 dB under dictation level in a quiet room.
        let t = take(&synth_speech(2.0, 3, true), -56.0, None, 5, &[], 0.5, 0.5);
        let g = take_gain_db(&t);
        assert!((g - 30.0).abs() < 1.5, "{g}");
        let out = for_recognizer(t.clone());
        let (level, _) = levels_of(&out);
        assert!((level - (TARGET_DB - 2.0)).abs() < 3.0, "{level}");
        // The same whisper 3 dB over café noise: the noise is not lifted
        // above the ceiling, so the gain stops there.
        let t = take(
            &synth_speech(2.0, 3, true),
            -56.0,
            Some(3.0),
            5,
            &[],
            0.5,
            0.5,
        );
        let out = for_recognizer(t);
        let (_, floor) = levels_of(&out);
        assert!(floor <= NOISE_CEILING_DB + 0.5, "{floor}");
        assert!(db(&out[..8000]) < NOISE_CEILING_DB + 3.0);
    }

    #[test]
    fn noise_alone_is_not_made_loud() {
        for noise in [None, Some(-70.0), Some(-55.0), Some(-40.0)] {
            let t = noise_take(3.0, noise, 7, &[]);
            let before = db(&t);
            let out = for_recognizer(t);
            let after = db(&out);
            assert!(
                after < before.max(NOISE_CEILING_DB) + 3.0,
                "{noise:?}: {before} -> {after}"
            );
        }
        assert!(for_recognizer(vec![]).is_empty());
        assert_eq!(for_recognizer(vec![0.0; 4000]), vec![0.0; 4000]);
    }

    #[test]
    fn peaks_stay_under_the_limit() {
        // A quiet take with one click: the click caps the gain.
        let mut t = take(&synth_speech(2.0, 3, false), -50.0, None, 5, &[], 0.5, 0.5);
        t[12_000] = 0.5;
        let out = for_recognizer(t);
        assert!(out.iter().all(|s| s.abs() <= PEAK_LIMIT + 1e-4));
    }

    /// Says "speech" for frames louder than -40 dBFS: a detector that,
    /// like a real one, does not hear a voice far below speech level.
    struct LevelDetector;

    impl VoiceActivityDetector for LevelDetector {
        fn push_frame<'a>(&'a mut self, frame: &'a [f32]) -> Result<VadFrame<'a>> {
            Ok(if db(frame) > -40.0 {
                VadFrame::Speech(frame)
            } else {
                VadFrame::Noise
            })
        }
        fn frame_samples(&self) -> usize {
            512
        }
    }

    impl SpeechScorer for LevelDetector {
        fn score(&mut self, frames: &[f32]) -> Result<Vec<f32>> {
            Ok(frames
                .chunks(512)
                .map(|f| if db(f) > -40.0 { 1.0 } else { 0.0 })
                .collect())
        }
    }

    fn speech_frames(d: &mut dyn VoiceActivityDetector, audio: &[f32]) -> usize {
        audio
            .chunks(512)
            .filter(|c| c.len() == 512)
            .filter(|c| d.push_frame(c).unwrap().is_speech())
            .count()
    }

    #[test]
    fn the_detector_hears_a_whisper_and_keeps_the_original_audio() {
        let quiet = take(&synth_speech(3.0, 9, true), -58.0, None, 5, &[], 0.5, 0.5);
        let mut plain = LevelDetector;
        assert_eq!(speech_frames(&mut plain, &quiet), 0);
        let mut wrapped = detector(Box::new(LevelDetector));
        let heard = speech_frames(wrapped.as_mut(), &quiet);
        assert!(heard > 10, "{heard}");
        // The frame handed back is the original, not the raised copy.
        let frame = &quiet[16_000..16_512];
        match wrapped.push_frame(frame).unwrap() {
            VadFrame::Speech(f) => assert_eq!(f, frame),
            VadFrame::Noise => {}
        }
        // Scores go through the same gain.
        wrapped.reset();
        let scores = wrapped.score(&quiet[..512 * 40]).unwrap();
        assert_eq!(scores.len(), 40);
    }

    /// Mac wave 7 (OT2-B.2): a whisper as loud as the café around it
    /// (0 dB SNR), which no gain may lift, is heard by its spectrum.
    #[test]
    fn a_whisper_at_the_cafes_level_is_heard_by_its_spectrum() {
        let babble: Vec<Vec<f32>> = (0..8).map(|k| synth_speech(4.0, 50 + k, false)).collect();
        let w = take(
            &synth_speech(3.0, 9, true),
            -48.0,
            Some(0.0),
            7,
            &babble,
            1.0,
            0.5,
        );
        let mut plain = LevelDetector;
        let mut wrapped = detector(Box::new(LevelDetector));
        let (a, b) = (
            speech_frames(&mut plain, &w),
            speech_frames(wrapped.as_mut(), &w),
        );
        assert!(b >= a + 8, "{a} vs {b}");
    }

    #[test]
    fn the_detector_does_not_raise_room_noise_into_speech() {
        for noise in [None, Some(-70.0), Some(-55.0)] {
            let audio = noise_take(4.0, noise, 3, &[]);
            let mut wrapped = detector(Box::new(LevelDetector));
            assert_eq!(speech_frames(wrapped.as_mut(), &audio), 0, "{noise:?}");
        }
        // Café noise with distant babble, no voice: the spectral presence
        // check must not call it speech either.
        let babble: Vec<Vec<f32>> = (0..8).map(|k| synth_speech(4.0, 50 + k, false)).collect();
        for (level, seed) in [(-38.0, 11), (-48.0, 12)] {
            let audio = noise_take(4.0, Some(level), seed, &babble);
            let mut plain = LevelDetector;
            let mut wrapped = detector(Box::new(LevelDetector));
            let (a, b) = (
                speech_frames(&mut plain, &audio),
                speech_frames(wrapped.as_mut(), &audio),
            );
            assert!(b <= a, "café {level}: {a} vs {b}");
        }
        // Ordinary speech is not boosted far past what the detector heard:
        // the gain adds nothing, the presence check only the meter's short
        // release at word ends (Mac wave 7: at most ~0.6 s over 3 s).
        let normal = take(
            &synth_speech(3.0, 9, false),
            -26.0,
            Some(20.0),
            5,
            &[],
            0.5,
            0.5,
        );
        let mut plain = LevelDetector;
        let mut wrapped = detector(Box::new(LevelDetector));
        let (a, b) = (
            speech_frames(&mut plain, &normal),
            speech_frames(wrapped.as_mut(), &normal),
        );
        assert!(b <= a + 20, "{a} vs {b}");
    }
}
