// SPDX-License-Identifier: GPL-3.0-or-later
//! Mic levels for the waveform (R5 §4.9).
//!
//! Handy's recorder reports 16 visualiser buckets (low to high frequency)
//! from every audio callback, far faster than a screen refreshes. The pill
//! takes the first [`BARS`] buckets (where speech is, the same choice as
//! Handy's own overlay), clamps them, and sends at most one frame every
//! [`FRAME_MS`] to the `fork-pill` web view only (`emit_to`), and only
//! while that view is showing a take. Each frame drives WebKit work, so
//! fewer frames are less memory churn (Handy issue #1279, wry #1489).
//!
//! Handy maps each bucket on a fixed dB window, so steady room noise held
//! the bars up and speech barely moved them (owner test 1). [`Waveform`]
//! gives each bar the mic meter's dynamics ([`crate::fork::audio::dynamics`]):
//! its own noise floor (noise differs per frequency), a gain that lets
//! speech fill the bar, fast attack and smooth release.
//!
//! Handy's mapping also caps each bucket at its ceiling, so every syllable
//! of ordinary speech pinned the bars at the top and loud speech looked the
//! same (Design W4). The recorder (vendored, `take_peak_level_db`) reports
//! the uncapped RMS of the audio behind each frame. With it, each bar is
//! first stretched over the headroom Handy's scale really leaves above
//! that bar's noise floor (in a café only ~6 dB, so speech filled at most
//! ~3/4 of the old bar), then scaled by the loudness's own [`Dynamics`]
//! (a floor and a wider span): Handy's buckets say where and when there is
//! speech, the uncapped level says how loud. Ordinary speech reaches most
//! of the bar and only loud speech the top.
//!
//! Owner test 2 (rc3, a café): whispers and quiet speech left the bars flat:
//! their buckets sit under Handy's -68 floor and the loudness gate reads
//! them as hum. In product builds the bars now come from the take tap's
//! speech meter ([`Waveform::push_reading`], `fork::audio::speech`); the
//! bucket path stays for builds without the tap.

use std::sync::atomic::{AtomicU64, Ordering};

use super::geometry::BARS;
use crate::fork::audio::dynamics::Dynamics;

// Handy's visualiser window (`audio_toolkit/audio/visualizer.rs`), to undo
// its mapping: `v = (((db - MIN) / (MAX - MIN)) * GAIN) ^ CURVE`.
const HANDY_DB_MIN: f32 = -68.0;
const HANDY_DB_MAX: f32 = -30.0;
const HANDY_GAIN: f32 = 1.3;
const HANDY_CURVE: f32 = 0.7;
/// A bar fills over this many of Handy's dB above its floor + knee. Handy's
/// mapping saturates at `MIN + (MAX - MIN) / GAIN` (≈ -38.8 on its scale,
/// where room tone is ~-48 and dictation ~-32), so speech has only about
/// 5-10 dB above a room's floor to show in: the span is short.
const WAVE_SPAN_DB: f32 = 9.0;
/// Fluctuation of steady noise that still reads flat (Handy's dB).
const WAVE_KNEE_DB: f32 = 1.5;
/// Loudness (uncapped RMS, dBFS) fills its scale over this many dB above
/// its floor + [`LOUD_KNEE_DB`]: ordinary speech in a café is 12-18 dB over
/// the hum, a raised voice 25 or more.
const LOUD_SPAN_DB: f32 = 24.0;
const LOUD_KNEE_DB: f32 = 4.5;
/// The bars' height at the bottom of the loudness scale (they still need
/// Handy's buckets above the noise to show at all).
const LOUD_BASE: f32 = 0.55;
/// Below this broadband loudness (0..1, the meter's dynamics) the bars stay
/// down, ramping up to full over it. Mac wave 6 (OT1.2): a real café WAV's
/// buckets wander ±2-3 dB, more than [`WAVE_KNEE_DB`], and the reach
/// stretch multiplies that by up to 1/[`MIN_REACH`], so the hum alone held
/// the loudest bar at 0.55. The loudness reads hum as 0 (its knee is the
/// meter's), so it gates the bars: noise lies flat, speech opens them.
const LOUD_GATE: f32 = 0.12;
/// A floor this close to Handy's ceiling leaves no headroom worth
/// stretching (the room is louder than speech on that scale).
const MIN_REACH: f32 = 0.3;
/// A gap longer than this between level frames starts a new take.
pub const RESET_GAP_MS: u64 = 400;

/// Handy's bucket value back on its dB scale (0 → its floor).
pub fn handy_db(v: f32) -> f32 {
    if !v.is_finite() || v <= 0.0 {
        return HANDY_DB_MIN - 12.0;
    }
    let normalized = v.min(1.0).powf(1.0 / HANDY_CURVE) / HANDY_GAIN;
    HANDY_DB_MIN + normalized * (HANDY_DB_MAX - HANDY_DB_MIN)
}

/// The waveform's per-bar dynamics for one take.
#[derive(Clone, Debug)]
pub struct Waveform {
    bars: Vec<Dynamics>,
    last_ms: Option<u64>,
    /// Broadband loudness from the recorder's uncapped level.
    loud: Dynamics,
    loud_ms: Option<u64>,
    loudness: Option<f32>,
}

impl Default for Waveform {
    fn default() -> Self {
        Self {
            bars: vec![Dynamics::new(WAVE_SPAN_DB, WAVE_KNEE_DB); BARS],
            last_ms: None,
            loud: Dynamics::new(LOUD_SPAN_DB, LOUD_KNEE_DB),
            loud_ms: None,
            loudness: None,
        }
    }
}

impl Waveform {
    /// Feeds one frame of Handy's levels at `now_ms`, with the recorder's
    /// uncapped level when one arrived (`loud_db`, dBFS); returns the bars.
    pub fn push(&mut self, levels: &[f32], now_ms: u64, loud_db: Option<f32>) -> Vec<f32> {
        let dt = match self.last_ms {
            Some(last) if now_ms.saturating_sub(last) <= RESET_GAP_MS => {
                now_ms.saturating_sub(last) as f32
            }
            Some(_) => {
                *self = Self::default();
                0.0
            }
            None => 0.0,
        };
        self.last_ms = Some(now_ms);
        if let Some(db) = loud_db {
            let dt = self
                .loud_ms
                .map(|last| now_ms.saturating_sub(last) as f32)
                .unwrap_or(0.0);
            self.loud_ms = Some(now_ms);
            self.loudness = Some(self.loud.push(db, dt));
        }
        // Without the recorder's level (tests, another source) the bars
        // keep Handy's capped scale.
        let scale = self
            .loudness
            .map(|l| LOUD_BASE + (1.0 - LOUD_BASE) * l)
            .unwrap_or(1.0);
        let gate = self
            .loudness
            .map(|l| (l / LOUD_GATE).min(1.0))
            .unwrap_or(1.0);
        let stretch = self.loudness.is_some();
        let raw = bars(levels);
        self.bars
            .iter_mut()
            .zip(raw)
            .map(|(d, v)| {
                let level = d.push(handy_db(v), dt);
                if !stretch {
                    return level;
                }
                // The most this bar can show under Handy's ceiling.
                let x = (handy_db(1.0) - d.floor() - WAVE_KNEE_DB) / WAVE_SPAN_DB;
                let reach = 1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(2);
                (level / reach.max(MIN_REACH)).min(1.0) * scale * gate
            })
            .collect()
    }
}

impl Waveform {
    /// The bars from a reading of the take tap
    /// ([`crate::fork::audio::speech::SpeechMeter`]).
    pub fn push_reading(
        &mut self,
        reading: &crate::fork::audio::speech::Reading,
        now_ms: u64,
    ) -> Vec<f32> {
        self.last_ms = Some(now_ms);
        bars(&reading.bars)
    }
}

/// Minimum spacing between two frames (~30 fps).
pub const FRAME_MS: u64 = 33;

/// The waveform bars for one frame of levels.
pub fn bars(levels: &[f32]) -> Vec<f32> {
    (0..BARS)
        .map(|i| {
            let v = levels.get(i).copied().unwrap_or(0.0);
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            }
        })
        .collect()
}

/// A lock-free frame limiter shared by every audio callback.
#[derive(Debug, Default)]
pub struct Throttle {
    last_ms: AtomicU64,
}

impl Throttle {
    pub const fn new() -> Self {
        Self {
            last_ms: AtomicU64::new(0),
        }
    }

    /// `true` when a frame may go out at `now_ms` (and records it).
    pub fn admit(&self, now_ms: u64) -> bool {
        let last = self.last_ms.load(Ordering::Relaxed);
        if last != 0 && now_ms.saturating_sub(last) < FRAME_MS {
            return false;
        }
        self.last_ms
            .compare_exchange(last, now_ms.max(1), Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars_take_the_speech_buckets() {
        let levels: Vec<f32> = (0..16).map(|i| i as f32 / 10.0).collect();
        let b = bars(&levels);
        assert_eq!(b.len(), BARS);
        assert_eq!(b[0], 0.0);
        assert_eq!(b[5], 0.5);
        // Clamped to 0..1.
        assert_eq!(b[BARS - 1], 1.0);
        assert_eq!(b[8], 0.8);
        assert_eq!(bars(&[2.0, -1.0, f32::NAN])[..3], [1.0, 0.0, 0.0]);
        // Short input pads with silence.
        assert_eq!(bars(&[]), vec![0.0; BARS]);
    }

    #[test]
    fn handy_db_undoes_the_visualiser_mapping() {
        for db in [-60.0f32, -48.0, -42.0, -39.5] {
            let n = ((db - HANDY_DB_MIN) / (HANDY_DB_MAX - HANDY_DB_MIN)).clamp(0.0, 1.0);
            let v = (n * HANDY_GAIN).powf(HANDY_CURVE).clamp(0.0, 1.0);
            assert!((handy_db(v) - db).abs() < 0.01, "{db}");
        }
        assert!(handy_db(0.0) < HANDY_DB_MIN);
        assert!(handy_db(f32::NAN) < HANDY_DB_MIN);
    }

    /// Handy's value for `db` (its own mapping).
    fn handy_value(db: f32) -> f32 {
        let n = ((db - HANDY_DB_MIN) / (HANDY_DB_MAX - HANDY_DB_MIN)).clamp(0.0, 1.0);
        (n * HANDY_GAIN).powf(HANDY_CURVE).clamp(0.0, 1.0)
    }

    #[test]
    fn steady_noise_lies_flat_and_speech_moves_the_bars() {
        let mut w = Waveform::default();
        let mut t = 1_000u64;
        // A café: every bucket at -45 ± 0.7 on Handy's scale (its raw bars
        // stand ~70 % up: "half full and steady").
        let mut seed = 17u32;
        let mut hum = || -> Vec<f32> {
            (0..16)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    handy_value(-45.0 + 1.4 * (seed as f32 / u32::MAX as f32 - 0.5))
                })
                .collect()
        };
        assert!(bars(&hum())[0] > 0.6, "Handy's raw bar stands well up");
        let mut last = Vec::new();
        for _ in 0..90 {
            t += 32;
            last = w.push(&hum(), t, None);
        }
        assert!(last.iter().all(|b| *b < 0.05), "flat on hum: {last:?}");
        // Speech: buckets 12-16 dB over the hum, different per bar.
        let mut peak = vec![0.0f32; BARS];
        for i in 0..30u64 {
            t += 32;
            let frame: Vec<f32> = (0..16)
                .map(|b| {
                    let syllable = (i + b as u64) % 5 < 3;
                    handy_value(if syllable {
                        -36.0 + b as f32 % 4.0
                    } else {
                        -44.0
                    })
                })
                .collect();
            for (p, v) in peak.iter_mut().zip(w.push(&frame, t, None)) {
                *p = p.max(v);
            }
        }
        assert!(
            peak.iter().all(|p| *p > 0.6),
            "speech fills the bars: {peak:?}"
        );
        // A pause: back down within ~0.5 s.
        for _ in 0..16 {
            t += 32;
            last = w.push(&hum(), t, None);
        }
        assert!(last.iter().all(|b| *b < 0.15), "{last:?}");
    }

    #[test]
    fn a_new_take_starts_fresh() {
        let mut w = Waveform::default();
        let mut t = 0;
        // Take 1 in a loud room: the floor climbs to the top of the scale.
        let loud = vec![handy_value(-39.0); 16];
        for _ in 0..100 {
            t += 32;
            w.push(&loud, t, None);
        }
        // Take 2, seconds later, in a quiet room: its speech must show, so
        // take 1's floor is forgotten.
        t += 5_000;
        let quiet = vec![handy_value(-52.0); 16];
        for _ in 0..20 {
            t += 32;
            w.push(&quiet, t, None);
        }
        let speech = vec![handy_value(-42.0); 16];
        let mut peak = 0.0f32;
        for _ in 0..5 {
            t += 32;
            peak = peak.max(w.push(&speech, t, None)[0]);
        }
        assert!(peak > 0.5, "{peak}");
    }

    /// Design W4: Handy's buckets are capped, so ordinary and loud speech
    /// both pin them; the recorder's uncapped level tells them apart.
    #[test]
    fn loud_speech_reaches_higher_than_ordinary_speech() {
        let peak_for = |speech_db: f32| {
            let mut w = Waveform::default();
            let mut t = 0u64;
            // A second of café hum: buckets at -45, RMS at -42 dBFS.
            for _ in 0..32 {
                t += 32;
                w.push(&[handy_value(-45.0); 16], t, Some(-42.0));
            }
            // Syllables: every bucket past Handy's ceiling.
            let mut peak = 0.0f32;
            for i in 0..30u64 {
                t += 32;
                let syllable = i % 5 < 3;
                let (bucket, rms) = if syllable {
                    (-30.0, speech_db)
                } else {
                    (-44.0, -41.0)
                };
                let frame = w.push(&[handy_value(bucket); 16], t, Some(rms));
                peak = peak.max(frame.iter().copied().fold(0.0, f32::max));
            }
            peak
        };
        let ordinary = peak_for(-27.0);
        let loud = peak_for(-14.0);
        assert!(ordinary > 0.72, "ordinary speech fills most: {ordinary}");
        assert!(ordinary < 0.88, "but not the top: {ordinary}");
        assert!(loud > 0.9, "loud speech nears the top: {loud}");
        assert!(loud - ordinary > 0.08, "{ordinary} vs {loud}");
        // Hum alone stays flat with the loudness scale too.
        let mut w = Waveform::default();
        let mut last = vec![];
        for i in 0..60u64 {
            last = w.push(&[handy_value(-45.0); 16], 1_000 + i * 32, Some(-42.0));
        }
        assert!(last.iter().all(|b| *b < 0.05), "{last:?}");
    }

    /// Mac wave 6 (OT1.2): real café noise wanders ±3 dB per bucket and
    /// ±2 dB in RMS; with the loudness level it still lies flat, and the
    /// words that follow still open the bars.
    #[test]
    fn wandering_cafe_noise_lies_flat_with_the_loudness_level() {
        let mut w = Waveform::default();
        let mut t = 0u64;
        let mut seed = 41u32;
        let mut rnd = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as f32 / u32::MAX as f32 - 0.5
        };
        let mut worst = 0.0f32;
        for i in 0..150u64 {
            t += 32;
            let frame: Vec<f32> = (0..16).map(|_| handy_value(-42.0 + 6.0 * rnd())).collect();
            let rms = -38.0 + 4.0 * rnd();
            let bars = w.push(&frame, t, Some(rms));
            if i > 15 {
                worst = worst.max(bars.iter().copied().fold(0.0, f32::max));
            }
        }
        assert!(worst < 0.1, "flat on wandering hum: {worst}");
        let mut peak = 0.0f32;
        for i in 0..30u64 {
            t += 32;
            let syllable = i % 5 < 3;
            let (bucket, rms) = if syllable {
                (-31.0, -24.0)
            } else {
                (-41.0, -37.0)
            };
            let bars = w.push(&[handy_value(bucket); 16], t, Some(rms));
            peak = peak.max(bars.iter().copied().fold(0.0, f32::max));
        }
        assert!(peak > 0.6, "speech opens the bars: {peak}");
    }

    /// OT2-B: with the take tap's reading, the bars are the speech meter's
    /// own (a whisper's show; noise lies flat), clamped to 0..1.
    #[test]
    fn a_reading_gives_the_bars() {
        use crate::fork::audio::speech::Reading;
        let mut w = Waveform::default();
        let mut bars_in = vec![0.3; BARS];
        bars_in[0] = 1.5;
        bars_in[1] = f32::NAN;
        let r = Reading {
            level: 0.3,
            bars: bars_in,
            presence: 0.6,
        };
        let out = w.push_reading(&r, 1_000);
        assert_eq!(out.len(), BARS);
        assert_eq!((out[0], out[1], out[2]), (1.0, 0.0, 0.3));
        let flat = Reading {
            bars: vec![0.0; BARS],
            ..Reading::default()
        };
        assert!(w.push_reading(&flat, 1_033).iter().all(|b| *b == 0.0));
    }

    #[test]
    fn throttle_admits_about_30_fps() {
        let t = Throttle::new();
        let admitted = (0..1000u64).filter(|ms| t.admit(10_000 + ms)).count();
        // One per 33 ms over one second.
        assert!((30..=31).contains(&admitted), "{admitted}");
        // A clock at 0 still admits its first frame.
        let t = Throttle::new();
        assert!(t.admit(0));
        assert!(!t.admit(10));
        assert!(t.admit(40));
    }
}
