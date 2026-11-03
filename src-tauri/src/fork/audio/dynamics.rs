// SPDX-License-Identifier: GPL-3.0-or-later
//! Level dynamics for the microphone meter (S-29a's mic test, Settings'
//! "Test microphone") and the Flow Bar's waveform.
//!
//! A meter on a fixed dBFS scale reads steady room noise as a level: in a
//! café, -30 dBFS of hum sat at half the meter and speech moved it by one
//! bar (owner test 1). This maps a level in dB the way a person reads a
//! meter instead:
//!
//! - **Noise floor.** The [`FLOOR_PERCENTILE`] of the last
//!   [`FLOOR_WINDOW_MS`] of readings: it follows steady noise up within about
//!   a second and back down as soon as the room is quieter, and speech (with
//!   its gaps between words) barely moves it. Until the window has filled,
//!   the floor sits [`WARMUP_MARGIN_DB`] lower, so speech that starts at
//!   once still shows and a noisy room does not flash the meter full.
//! - **Gain.** The level is the height above the floor (minus a
//!   [`KNEE_DB`] dead band, so the hum itself reads 0) over a span of
//!   `span_db`, with an ease-out curve: speech 12-20 dB over a café's hum
//!   fills most of the meter, and peaks fill it.
//! - **Response.** An envelope with a fast attack ([`ATTACK_MS`]) and a
//!   smooth release ([`RELEASE_MS`]), computed from the real time between
//!   readings, so it behaves the same at any callback rate.

use std::collections::VecDeque;

/// How long the noise floor looks back.
pub const FLOOR_WINDOW_MS: f32 = 1500.0;
/// The low percentile of that window taken as the floor.
pub const FLOOR_PERCENTILE: f32 = 0.15;
/// Until the window has this much history, the floor is lowered by
/// [`WARMUP_MARGIN_DB`] (speech in the first moments is not taken as noise).
pub const WARMUP_MS: f32 = 400.0;
/// How much lower the floor sits while warming up.
pub const WARMUP_MARGIN_DB: f32 = 2.0;
/// Readings this close above the floor are noise and read 0 (the meter;
/// a steady room's 30 ms RMS wanders ±2-3 dB around its median).
pub const KNEE_DB: f32 = 4.5;
/// Envelope time constants.
pub const ATTACK_MS: f32 = 40.0;
pub const RELEASE_MS: f32 = 200.0;
/// Quieter than this is silence, whatever the floor (dBFS).
pub const SILENCE_DB: f32 = -80.0;
/// The meter's span above floor + knee: this many dB fill it.
pub const METER_SPAN_DB: f32 = 20.0;

/// One meter's state: the floor window and the envelope.
#[derive(Clone, Debug)]
pub struct Dynamics {
    span_db: f32,
    knee_db: f32,
    /// `(age in ms at push time, dB)`; ages are kept as running time.
    window: VecDeque<(f32, f32)>,
    now_ms: f32,
    first_ms: Option<f32>,
    envelope: f32,
    floor: f32,
}

impl Dynamics {
    /// A meter that reads 0 up to `knee_db` above the noise floor and fills
    /// over the next `span_db`.
    pub fn new(span_db: f32, knee_db: f32) -> Self {
        Self {
            span_db,
            knee_db,
            window: VecDeque::new(),
            now_ms: 0.0,
            first_ms: None,
            envelope: 0.0,
            floor: SILENCE_DB,
        }
    }

    /// The microphone meter's settings (RMS in dBFS).
    pub fn meter() -> Self {
        Self::new(METER_SPAN_DB, KNEE_DB)
    }

    /// Time fed so far (ms, the sum of `dt_ms`).
    pub fn elapsed_ms(&self) -> f32 {
        self.now_ms
    }

    /// The current noise floor estimate (dB).
    pub fn floor(&self) -> f32 {
        self.floor
    }

    /// Feeds one reading of `db`, `dt_ms` after the previous one, and
    /// returns the shown level, 0..1.
    pub fn push(&mut self, db: f32, dt_ms: f32) -> f32 {
        let dt = if dt_ms.is_finite() {
            dt_ms.clamp(0.0, 1000.0)
        } else {
            0.0
        };
        let db = if db.is_finite() {
            db.max(SILENCE_DB)
        } else {
            SILENCE_DB
        };
        self.now_ms += dt;
        let first = *self.first_ms.get_or_insert(self.now_ms);
        self.window.push_back((self.now_ms, db));
        while self
            .window
            .front()
            .is_some_and(|(t, _)| self.now_ms - t > FLOOR_WINDOW_MS)
        {
            self.window.pop_front();
        }
        let mut floor = percentile(self.window.iter().map(|(_, d)| *d), FLOOR_PERCENTILE);
        if self.now_ms - first < WARMUP_MS {
            floor -= WARMUP_MARGIN_DB;
        }
        self.floor = floor;

        let target = if db <= SILENCE_DB {
            0.0
        } else {
            shape((db - floor - self.knee_db) / self.span_db)
        };
        let tau = if target > self.envelope {
            ATTACK_MS
        } else {
            RELEASE_MS
        };
        let k = if dt <= 0.0 {
            1.0
        } else {
            1.0 - (-dt / tau).exp()
        };
        self.envelope += (target - self.envelope) * k;
        if self.envelope < 1e-4 {
            self.envelope = 0.0;
        }
        self.envelope
    }
}

/// Ease-out over 0..1: the upper part of the meter is reached by ordinary
/// speech, not only by shouting.
fn shape(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    1.0 - (1.0 - x) * (1.0 - x)
}

/// The `p` quantile (0..1) of `values` (nearest rank).
fn percentile(values: impl Iterator<Item = f32>, p: f32) -> f32 {
    let mut v: Vec<f32> = values.collect();
    if v.is_empty() {
        return SILENCE_DB;
    }
    let i = ((v.len() - 1) as f32 * p).round() as usize;
    let (_, nth, _) = v.select_nth_unstable_by(i, f32::total_cmp);
    *nth
}

/// RMS of `samples` in dBFS ([`SILENCE_DB`] for silence).
pub fn rms_db(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return SILENCE_DB;
    }
    let mean_square = samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32;
    if mean_square <= 0.0 || !mean_square.is_finite() {
        return SILENCE_DB;
    }
    (10.0 * mean_square.log10()).max(SILENCE_DB)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deterministic pseudo-random sequence in -1..1.
    fn jitter(seed: &mut u32) -> f32 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 17;
        *seed ^= *seed << 5;
        (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// Feeds `ms` of readings every 20 ms; returns the shown levels.
    fn feed(d: &mut Dynamics, ms: u32, mut db: impl FnMut(u32) -> f32) -> Vec<f32> {
        (0..ms / 20).map(|i| d.push(db(i * 20), 20.0)).collect()
    }

    fn max(v: &[f32]) -> f32 {
        v.iter().copied().fold(0.0, f32::max)
    }
    fn mean(v: &[f32]) -> f32 {
        v.iter().sum::<f32>() / v.len() as f32
    }

    /// Syllables: 120 ms loud, 60 ms dip, with some variation.
    fn speech(base_db: f32, seed: &mut u32) -> impl FnMut(u32) -> f32 + '_ {
        move |t| {
            let syllable = (t % 180) < 120;
            let level = if syllable { base_db } else { base_db - 14.0 };
            level + 3.0 * jitter(seed)
        }
    }

    #[test]
    fn cafe_noise_then_speech_then_silence() {
        let mut d = Dynamics::meter();
        let mut seed = 7;
        // 3 s of a café: steady -32 dBFS hum ± 1.5 dB.
        let noise = feed(&mut d, 3000, |_| -32.0 + 1.5 * jitter(&mut seed));
        let settled = &noise[noise.len() / 2..];
        assert!(
            mean(settled) < 0.05 && max(settled) < 0.15,
            "steady noise reads near zero (mean {}, max {})",
            mean(settled),
            max(settled)
        );
        assert!((d.floor() - -33.0).abs() < 2.0, "floor {}", d.floor());

        // 2 s of normal speech 16 dB over the hum (peaks), on top of it.
        let mut seed2 = 11;
        let talk = feed(&mut d, 2000, speech(-16.0, &mut seed2));
        assert!(
            max(&talk) > 0.85,
            "speech peaks fill the meter: {}",
            max(&talk)
        );
        assert!(mean(&talk) > 0.5, "speech reads high: {}", mean(&talk));
        // It moves: a range, not one block.
        let lo = talk[10..].iter().copied().fold(1.0, f32::min);
        assert!(max(&talk) - lo > 0.25, "dynamic range {lo}..{}", max(&talk));
        assert!(
            d.floor() < -28.0,
            "speech barely moves the floor: {}",
            d.floor()
        );

        // Back to the hum: falls back near zero within ~0.6 s.
        let mut seed3 = 13;
        let after = feed(&mut d, 1500, |_| -32.0 + 1.5 * jitter(&mut seed3));
        assert!(after[30..].iter().all(|l| *l < 0.15), "{:?}", &after[30..]);

        // Digital silence: zero.
        let silent = feed(&mut d, 500, |_| SILENCE_DB);
        assert_eq!(*silent.last().unwrap(), 0.0);
    }

    #[test]
    fn a_quiet_room_and_speech_at_once() {
        let mut d = Dynamics::meter();
        let mut seed = 3;
        // Speech from the first reading: the warm-up floor keeps it visible.
        let talk = feed(&mut d, 1000, speech(-22.0, &mut seed));
        assert!(max(&talk) > 0.7, "{}", max(&talk));
        let mut seed2 = 5;
        // A quiet room (-60 dBFS): reads 0.
        let quiet = feed(&mut d, 2000, |_| -60.0 + jitter(&mut seed2));
        assert!(quiet[40..].iter().all(|l| *l < 0.05));
    }

    #[test]
    fn attack_is_fast_and_release_smooth() {
        let mut d = Dynamics::meter();
        feed(&mut d, 2000, |_| -60.0);
        // A step to loud speech: most of the way within ~50 ms.
        let mut up = Vec::new();
        for _ in 0..10 {
            up.push(d.push(-15.0, 10.0));
        }
        assert!(up[4] > 0.6 * up[9], "attack {up:?}");
        // Release: still visible after 100 ms, mostly gone by 500 ms.
        let mut down = Vec::new();
        for _ in 0..50 {
            down.push(d.push(-60.0, 10.0));
        }
        assert!(down[9] > 0.4 * up[9], "release too fast {down:?}");
        assert!(down[49] < 0.15 * up[9], "release too slow {down:?}");
    }

    #[test]
    fn the_rate_of_readings_does_not_change_the_response() {
        let run = |step: f32| {
            let mut d = Dynamics::meter();
            let mut t = 0.0;
            let mut out = 0.0;
            while t < 2000.0 {
                out = d.push(if t < 1500.0 { -60.0 } else { -15.0 }, step);
                t += step;
            }
            out
        };
        let (a, b) = (run(10.0), run(32.0));
        assert!((a - b).abs() < 0.05, "{a} vs {b}");
    }

    #[test]
    fn bad_input_is_silence() {
        let mut d = Dynamics::meter();
        assert_eq!(d.push(f32::NAN, 20.0), 0.0);
        assert_eq!(d.push(f32::NEG_INFINITY, f32::NAN), 0.0);
        assert_eq!(rms_db(&[]), SILENCE_DB);
        assert_eq!(rms_db(&[0.0; 16]), SILENCE_DB);
        assert!((rms_db(&[1.0; 16])).abs() < 1e-4);
        assert!((rms_db(&[0.1; 16]) - -20.0).abs() < 1e-3);
    }
}
