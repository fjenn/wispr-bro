// SPDX-License-Identifier: GPL-3.0-or-later
//! How much speech the microphone hears right now (OT2-B), for the mic
//! meter and the Flow Bar's waveform.
//!
//! Owner test 1 made both read the level above the room's noise floor
//! ([`super::dynamics`]); the owner's rc3 report showed the cost: in a café
//! quiet speech and whispers are only 0-6 dB over the noise in broadband
//! RMS, inside the meter's knee, so neither the meter nor the bar moved at
//! all (Wispr Flow's do). A broadband level cannot tell a whisper at 0 dB
//! SNR from the café's own wobble, but a spectrum can: café noise is
//! low-heavy and steady per band, a voice adds energy in a few bands at a
//! time (voiced speech low, a whisper's turbulence high), and the
//! excursions of noise are independent per band while speech's are not.
//!
//! [`SpeechMeter`] therefore splits the audio into [`BANDS`] log-spaced
//! bands from 250 Hz to 5.5 kHz (biquad band-passes at any sample rate),
//! measures each band every [`FRAME_MS`] over a 32 ms window, and keeps a
//! noise floor per band (the [`FLOOR_PERCENTILE`] of the last
//! [`FLOOR_WINDOW_MS`]; slower than the old 1.5 s, so a long sentence
//! does not lift it). Per frame:
//!
//! - **presence**: the mean excess over the floor of the
//!   [`TOP_BANDS`] most excited bands, less [`PRESENCE_KNEE_DB`] (what
//!   steady noise scores), on a log-like curve ([`presence_curve`]) that
//!   gives the first few dB most of the range: 0 on noise, about half at a
//!   whisper's 4-6 dB, near 1 from ~15 dB;
//! - **loudness**: the frame's broadband dBFS, mapped from
//!   [`LOUD_MIN_DB`] to [`LOUD_MAX_DB`], which sizes what presence lets
//!   through: a whisper moves the meter a third of the way, ordinary speech
//!   most of it, and only loud speech pegs it;
//! - **level** = presence × (base + (1 − base) × loudness), with the
//!   meter's fast attack and smooth release;
//! - **bars**: per band, its own excess over its floor, so the waveform
//!   shows where in the spectrum the voice is, all scaled by the level.
//!
//! Steady café noise (pink, hum, distant babble) scores ~0 because each
//! band's floor is its own noise. A sound the floor has not learnt yet
//! (the first 400 ms) is held down by a warm-up margin.

use std::collections::VecDeque;

use crate::fork::pill::geometry::BARS;

/// Bands of the filterbank: one per waveform bar.
pub const BANDS: usize = BARS;
/// The filterbank's range (Hz).
pub const LOW_HZ: f32 = 250.0;
pub const HIGH_HZ: f32 = 5500.0;
/// A reading every this many ms, over a window of twice that.
pub const FRAME_MS: f32 = 16.0;
/// The noise floor looks back this far ...
pub const FLOOR_WINDOW_MS: f32 = 3000.0;
/// ... and takes this low percentile of each band.
pub const FLOOR_PERCENTILE: f32 = 0.12;
/// Until the window has this much history, readings are held down.
pub const WARMUP_MS: f32 = 400.0;
/// Bands averaged for presence (a voice excites a few at a time).
pub const TOP_BANDS: usize = 4;
/// What steady noise scores (the mean excess of its top bands over a low
/// percentile): presence counts from here.
pub const PRESENCE_KNEE_DB: f32 = 4.0;
/// The dB above the knee that bring presence to 1 − 1/e.
pub const PRESENCE_SCALE_DB: f32 = 5.0;
/// Loudness scale (frame dBFS): a whisper sits low, a raised voice high.
pub const LOUD_MIN_DB: f32 = -70.0;
pub const LOUD_MAX_DB: f32 = -4.0;
/// The shown level's share that does not depend on loudness.
pub const LEVEL_BASE: f32 = 0.3;
/// A band's own excess that fills its bar (above its knee).
pub const BAR_KNEE_DB: f32 = 2.0;
pub const BAR_SPAN_DB: f32 = 18.0;
/// Bars never fall below this share of the level while it shows, so a
/// whisper's quiet bands still move a little.
pub const BAR_BASE: f32 = 0.35;
/// Envelope time constants (as the meter's).
pub const ATTACK_MS: f32 = 40.0;
pub const RELEASE_MS: f32 = 120.0;
/// Below this (dBFS) is silence.
pub const SILENCE_DB: f32 = -90.0;

/// One reading.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reading {
    /// 0..1, the meter's level.
    pub level: f32,
    /// 0..1 per band, low to high: the waveform.
    pub bars: Vec<f32>,
    /// 0..1, how sure it is that a voice is there (before loudness).
    pub presence: f32,
}

#[derive(Clone, Debug)]
struct Biquad {
    b0: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    /// A band-pass with 0 dB peak gain at `hz`, quality `q` (RBJ).
    fn band(rate: f32, hz: f32, q: f32) -> Self {
        let w = 2.0 * std::f32::consts::PI * (hz / rate).min(0.49);
        let (s, c) = w.sin_cos();
        let alpha = s / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: alpha / a0,
            b2: -alpha / a0,
            a1: -2.0 * c / a0,
            a2: (1.0 - alpha) / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn run(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = if y.is_finite() { y } else { 0.0 };
        self.y1
    }
}

/// The meter's state for one stream of audio.
#[derive(Clone, Debug)]
pub struct SpeechMeter {
    filters: Vec<Biquad>,
    frame_len: usize,
    /// Running sums of the current frame: per band, and broadband.
    acc: Vec<f32>,
    acc_all: f32,
    acc_n: usize,
    /// The previous frame's sums (readings use two frames: 32 ms).
    prev: Vec<f32>,
    prev_all: f32,
    prev_n: usize,
    /// Per band, the last [`FLOOR_WINDOW_MS`] of band dB.
    history: Vec<VecDeque<f32>>,
    /// The broadband dB of the last [`FLOOR_WINDOW_MS`] (the log's floor).
    all_history: VecDeque<f32>,
    frames: usize,
    level: f32,
    bars: Vec<f32>,
    presence: f32,
    last_db: f32,
    floor_db: f32,
}

impl SpeechMeter {
    pub fn new(sample_rate: u32) -> Self {
        let rate = sample_rate.max(8_000) as f32;
        let ratio = (HIGH_HZ / LOW_HZ).powf(1.0 / (BANDS as f32 - 1.0));
        // Bands a third-octave-ish wide, overlapping at about -3 dB.
        let q = ratio.sqrt() / (ratio - 1.0);
        let filters = (0..BANDS)
            .map(|b| Biquad::band(rate, LOW_HZ * ratio.powi(b as i32), q))
            .collect();
        Self {
            filters,
            frame_len: (rate * FRAME_MS / 1000.0) as usize,
            acc: vec![0.0; BANDS],
            acc_all: 0.0,
            acc_n: 0,
            prev: vec![0.0; BANDS],
            prev_all: 0.0,
            prev_n: 0,
            history: vec![VecDeque::new(); BANDS],
            all_history: VecDeque::new(),
            frames: 0,
            level: 0.0,
            bars: vec![0.0; BANDS],
            presence: 0.0,
            last_db: SILENCE_DB,
            floor_db: SILENCE_DB,
        }
    }

    /// The last frame's broadband level (dBFS), for the log.
    pub fn last_db(&self) -> f32 {
        self.last_db
    }

    /// The broadband noise floor (dBFS), for the log; the meter itself
    /// uses one floor per band.
    pub fn floor_db(&self) -> f32 {
        self.floor_db
    }

    /// Time fed so far (ms).
    pub fn elapsed_ms(&self) -> f32 {
        self.frames as f32 * FRAME_MS
    }

    /// The latest reading.
    pub fn reading(&self) -> Reading {
        Reading {
            level: self.level,
            bars: self.bars.clone(),
            presence: self.presence,
        }
    }

    /// Feeds interleaved samples with `channels` channels (mixed to mono)
    /// and returns the latest reading.
    pub fn push(&mut self, samples: &[f32], channels: u16) -> Reading {
        let ch = channels.max(1) as usize;
        for frame in samples.chunks(ch) {
            let x = frame.iter().sum::<f32>() / frame.len() as f32;
            let x = if x.is_finite() { x } else { 0.0 };
            self.acc_all += x * x;
            for (f, a) in self.filters.iter_mut().zip(self.acc.iter_mut()) {
                let y = f.run(x);
                *a += y * y;
            }
            self.acc_n += 1;
            if self.acc_n >= self.frame_len {
                self.frame();
            }
        }
        self.reading()
    }

    fn frame(&mut self) {
        let n = (self.acc_n + self.prev_n).max(1) as f32;
        let to_db = |ms: f32| {
            if ms > 0.0 && ms.is_finite() {
                (10.0 * ms.log10()).max(SILENCE_DB)
            } else {
                SILENCE_DB
            }
        };
        let all_db = to_db((self.acc_all + self.prev_all) / n);
        let band_db: Vec<f32> = self
            .acc
            .iter()
            .zip(&self.prev)
            .map(|(a, p)| to_db((a + p) / n))
            .collect();
        self.prev.copy_from_slice(&self.acc);
        self.prev_all = self.acc_all;
        self.prev_n = self.acc_n;
        self.acc.iter_mut().for_each(|a| *a = 0.0);
        self.acc_all = 0.0;
        self.acc_n = 0;
        self.frames += 1;

        let keep = (FLOOR_WINDOW_MS / FRAME_MS) as usize;
        let warm = self.elapsed_ms() < WARMUP_MS;
        self.last_db = all_db;
        self.all_history.push_back(all_db);
        while self.all_history.len() > keep {
            self.all_history.pop_front();
        }
        self.floor_db = {
            let mut v: Vec<f32> = self.all_history.iter().copied().collect();
            let i = ((v.len() - 1) as f32 * FLOOR_PERCENTILE).round() as usize;
            *v.select_nth_unstable_by(i, f32::total_cmp).1
        };
        let mut excess = Vec::with_capacity(BANDS);
        for (h, d) in self.history.iter_mut().zip(&band_db) {
            h.push_back(*d);
            while h.len() > keep {
                h.pop_front();
            }
            let mut v: Vec<f32> = h.iter().copied().collect();
            let i = ((v.len() - 1) as f32 * FLOOR_PERCENTILE).round() as usize;
            let (_, floor, _) = v.select_nth_unstable_by(i, f32::total_cmp);
            let floor = *floor;
            excess.push(if *d <= SILENCE_DB {
                0.0
            } else {
                (d - floor).max(0.0)
            });
        }
        let mut sorted = excess.clone();
        sorted.sort_by(|a, b| b.total_cmp(a));
        let score = sorted.iter().take(TOP_BANDS).sum::<f32>() / TOP_BANDS as f32;
        let mut presence = presence_curve(score - PRESENCE_KNEE_DB);
        if warm || all_db <= SILENCE_DB {
            presence = 0.0;
        }
        let loud = ((all_db - LOUD_MIN_DB) / (LOUD_MAX_DB - LOUD_MIN_DB)).clamp(0.0, 1.0);
        let target = presence * (LEVEL_BASE + (1.0 - LEVEL_BASE) * loud);
        let k = |target: f32, now: f32| {
            let tau = if target > now { ATTACK_MS } else { RELEASE_MS };
            1.0 - (-FRAME_MS / tau).exp()
        };
        self.presence += (presence - self.presence) * k(presence, self.presence);
        self.level += (target - self.level) * k(target, self.level);
        if self.level < 1e-4 {
            self.level = 0.0;
        }
        for (bar, e) in self.bars.iter_mut().zip(&excess) {
            let x = ((e - BAR_KNEE_DB) / BAR_SPAN_DB).clamp(0.0, 1.0);
            let shape = 1.0 - (1.0 - x) * (1.0 - x);
            let t = target * (BAR_BASE + (1.0 - BAR_BASE) * shape);
            *bar += (t - *bar) * k(t, *bar);
            if *bar < 1e-4 {
                *bar = 0.0;
            }
        }
    }
}

/// Presence from the score above the knee (dB): 0 at or below it, then
/// `1 − e^(−x / scale)`, steep at first (a few dB already show) and
/// saturating gently (loud speech does not read as more present).
pub fn presence_curve(x_db: f32) -> f32 {
    if x_db.is_nan() || x_db <= 0.0 {
        return 0.0;
    }
    1.0 - (-x_db / PRESENCE_SCALE_DB).exp()
}

/// The take tap: every chunk the recorder delivers (a take, or the mic
/// test) through one [`SpeechMeter`], so the Flow Bar reads the same
/// speech meter as the mic test. Installed once from `fork::on_ready`
/// (`handy_recorder::set_chunk_tap`); runs on the recorder's delivery
/// thread, the same thread that then calls Handy's level callback.
struct Tap {
    meter: SpeechMeter,
    rate: u32,
    last: std::time::Instant,
}

static TAP: std::sync::Mutex<Option<Tap>> = std::sync::Mutex::new(None);

/// A gap longer than this between chunks starts a new stream (a new take
/// learns its own room).
pub const TAP_RESET_MS: u128 = 400;

/// Installs the tap (idempotent).
pub fn install() {
    handy_recorder::set_chunk_tap(Some(on_chunk));
}

fn on_chunk(samples: &[f32], rate: u32, channels: u16) {
    let mut tap = TAP.lock().unwrap_or_else(|e| e.into_inner());
    let now = std::time::Instant::now();
    let fresh = tap
        .as_ref()
        .is_some_and(|t| t.rate == rate && now.duration_since(t.last).as_millis() <= TAP_RESET_MS);
    if !fresh {
        *tap = Some(Tap {
            meter: SpeechMeter::new(rate),
            rate,
            last: now,
        });
    }
    if let Some(t) = tap.as_mut() {
        t.meter.push(samples, channels);
        t.last = now;
    }
}

/// The tap's latest reading while audio is arriving (`None` when no chunk
/// came within [`TAP_RESET_MS`], or the tap is not installed).
pub fn latest() -> Option<Reading> {
    let tap = TAP.lock().unwrap_or_else(|e| e.into_inner());
    tap.as_ref()
        .filter(|t| t.last.elapsed().as_millis() <= TAP_RESET_MS)
        .map(|t| t.meter.reading())
}

/// Presence above which the hands-free silence timer counts a voice
/// (rule 8): a whisper counts as speech too.
pub const VOICE_PRESENCE: f32 = 0.5;

/// Whether the tap hears a voice now.
pub fn voice_now() -> bool {
    latest().is_some_and(|r| r.presence >= VOICE_PRESENCE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::audio::quiet_set::{noise_take, synth_speech, take, RATE};

    /// Babble for the café: eight other synthetic talkers (a room of them).
    fn babble() -> Vec<Vec<f32>> {
        (0..8).map(|k| synth_speech(4.0, 50 + k, false)).collect()
    }

    /// Levels (one per 32 ms) and the loudest bar of each reading, in
    /// 10 ms chunks as the mic test gets them; `(noise, speech)` split
    /// at `lead..end` (samples), skipping the first 0.5 s.
    /// `(noise, speech)` readings.
    type Split = (Vec<f32>, Vec<f32>);

    fn run(audio: &[f32], lead: usize, end: usize) -> (Split, Split) {
        let mut m = SpeechMeter::new(RATE as u32);
        let (mut ln, mut ls, mut bn, mut bs) = (vec![], vec![], vec![], vec![]);
        for (i, c) in audio.chunks(160).enumerate() {
            let r = m.push(c, 1);
            let t = i * 160;
            if t < RATE / 2 || i % 3 != 0 || (t >= end && end > lead) {
                continue;
            }
            let bar = r.bars.iter().copied().fold(0.0, f32::max);
            if t >= lead && end > lead {
                ls.push(r.level);
                bs.push(bar);
            } else {
                ln.push(r.level);
                bn.push(bar);
            }
        }
        ((ln, ls), (bn, bs))
    }

    fn pct(v: &[f32], p: f32) -> f32 {
        let mut v = v.to_vec();
        v.sort_by(f32::total_cmp);
        v.get(((v.len() as f32 - 1.0) * p) as usize)
            .copied()
            .unwrap_or(0.0)
    }
    fn mean(v: &[f32]) -> f32 {
        v.iter().sum::<f32>() / v.len().max(1) as f32
    }

    /// A take: 1.5 s of café noise, `secs` of the talker at `level` and
    /// `snr`, 0.6 s of noise.
    fn talker(level: f32, snr: Option<f32>, whisper: bool, seed: u32) -> (Vec<f32>, usize, usize) {
        let s = synth_speech(3.0, seed, whisper);
        let audio = take(&s, level, snr, seed + 7, &babble(), 1.5, 0.6);
        let lead = (1.5 * RATE as f32) as usize;
        (audio, lead, lead + s.len())
    }

    #[test]
    fn steady_cafe_noise_lies_flat_at_any_level() {
        // Synthetic babble (eight formant talkers of the same rhythm) is
        // burstier than a real room's; the bench with real voices reads
        // p90 0.02-0.03 (`quiet_bench::meters`).
        for db in [-70.0, -55.0, -45.0, -35.0] {
            let audio = noise_take(4.0, Some(db), 3, &babble());
            let ((levels, _), (bars, _)) = run(&audio, 0, 0);
            assert!(pct(&levels, 0.9) < 0.1, "{db}: {}", pct(&levels, 0.9));
            assert!(pct(&bars, 0.9) < 0.1, "{db}: {}", pct(&bars, 0.9));
            assert!(pct(&levels, 1.0) < 0.2, "{db}: {}", pct(&levels, 1.0));
        }
        // Without babble (hum and pink noise): flat.
        for db in [-60.0, -40.0] {
            let ((levels, _), _) = run(&noise_take(4.0, Some(db), 5, &[]), 0, 0);
            assert!(pct(&levels, 1.0) < 0.05, "{db}: {}", pct(&levels, 1.0));
        }
        // A quiet room: nothing at all.
        let ((levels, _), _) = run(&noise_take(2.0, None, 4, &[]), 0, 0);
        assert!(levels.iter().all(|l| *l < 0.02), "{levels:?}");
    }

    #[test]
    fn a_whisper_in_a_cafe_moves_the_meter_and_the_bars() {
        // Whispered speech 30-35 dB under dictation level, 0-6 dB over a
        // café's noise (the owner's rc3 report: nothing moved).
        for (level, snr) in [(-56.0, 6.0), (-56.0, 3.0), (-61.0, 3.0), (-61.0, 0.0)] {
            let (audio, lead, end) = talker(level, Some(snr), true, 21);
            let ((ln, ls), (bn, bs)) = run(&audio, lead, end);
            let label = format!("whisper at {level} dBFS, SNR {snr}");
            assert!(pct(&ln, 0.9) < 0.1, "{label}: noise {}", pct(&ln, 0.9));
            assert!(mean(&ls) > 0.08, "{label}: mean {}", mean(&ls));
            assert!(pct(&ls, 0.9) > 0.2, "{label}: p90 {}", pct(&ls, 0.9));
            assert!(pct(&ls, 0.9) - pct(&ls, 0.1) > 0.1, "{label}: it moves");
            assert!(
                mean(&ls) > 3.0 * mean(&ln).max(0.01),
                "{label}: over the noise"
            );
            assert!(pct(&bs, 0.9) > 0.18, "{label}: bars {}", pct(&bs, 0.9));
            assert!(
                pct(&bn, 0.9) < 0.1,
                "{label}: bars on noise {}",
                pct(&bn, 0.9)
            );
        }
    }

    #[test]
    fn quiet_speech_shows_and_normal_speech_is_lively() {
        let (audio, lead, end) = talker(-46.0, Some(3.0), false, 31);
        let ((_, quiet), _) = run(&audio, lead, end);
        assert!(
            mean(&quiet) > 0.2,
            "quiet speech in a café: {}",
            mean(&quiet)
        );
        let (audio, lead, end) = talker(-26.0, Some(20.0), false, 31);
        let ((_, normal), (_, bars)) = run(&audio, lead, end);
        assert!(mean(&normal) > 0.45, "normal: {}", mean(&normal));
        assert!(
            pct(&normal, 0.9) > 0.65,
            "normal p90: {}",
            pct(&normal, 0.9)
        );
        assert!(pct(&normal, 0.9) - pct(&normal, 0.1) > 0.25, "it moves");
        assert!(pct(&bars, 0.9) > 0.6, "bars: {}", pct(&bars, 0.9));
        assert!(mean(&normal) > mean(&quiet) + 0.15, "louder reads higher");
    }

    #[test]
    fn loud_speech_is_high_but_not_pegged() {
        let (audio, lead, end) = talker(-12.0, Some(25.0), false, 41);
        let ((_, loud), _) = run(&audio, lead, end);
        assert!(pct(&loud, 0.9) > 0.8, "{}", pct(&loud, 0.9));
        let pegged = loud.iter().filter(|l| **l > 0.97).count();
        assert!(pegged * 5 < loud.len(), "pegged {pegged} of {}", loud.len());
    }

    #[test]
    fn any_rate_and_channel_count_reads_alike() {
        // The same tone burst at 16 kHz mono and 48 kHz stereo.
        let tone = |rate: usize, ch: usize| -> Vec<f32> {
            (0..rate * 2)
                .flat_map(|i| {
                    let t = i as f32 / rate as f32;
                    let on = t > 1.0;
                    let v = if on {
                        0.05 * (2.0 * std::f32::consts::PI * 700.0 * t).sin()
                    } else {
                        0.0005 * ((i * 7919 % 1000) as f32 / 500.0 - 1.0)
                    };
                    std::iter::repeat_n(v, ch)
                })
                .collect()
        };
        let mut a = SpeechMeter::new(16_000);
        let ra = a.push(&tone(16_000, 1), 1);
        let mut b = SpeechMeter::new(48_000);
        let rb = b.push(&tone(48_000, 2), 2);
        assert!(ra.level > 0.3, "{ra:?}");
        assert!((ra.level - rb.level).abs() < 0.1, "{ra:?} vs {rb:?}");
    }

    #[test]
    fn bad_input_is_silence() {
        let mut m = SpeechMeter::new(16_000);
        let r = m.push(&[f32::NAN; 1600], 1);
        assert_eq!(r.level, 0.0);
        let r = m.push(&[0.0; 16_000], 1);
        assert_eq!(r.level, 0.0);
        assert!(r.bars.iter().all(|b| *b == 0.0));
        assert_eq!(presence_curve(f32::NAN), 0.0);
        assert_eq!(presence_curve(-3.0), 0.0);
    }
}
