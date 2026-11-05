// SPDX-License-Identifier: GPL-3.0-or-later
//! OT2-B's test set (tests only): quiet speech and whispers in a café.
//!
//! Everything is made at test time from a speech source, so no large
//! fixture is committed:
//!
//! - [`synth_speech`]: a deterministic formant "talker" (voiced syllables
//!   with fricatives and pauses), for the unit tests that always run;
//! - the `say` sentences of `scripts/wb-quiet/make-speech.sh`, for the
//!   ignored bench (`quiet_bench.rs`);
//! - [`whisperize`]: a whispered version of any speech: per 32 ms frame an
//!   LPC fit (order 18), re-excited with white noise of the residual's
//!   energy, so the formants stay and the voicing (F0) is gone, then tilted
//!   towards the highs like a real whisper;
//! - [`cafe`]: pink noise, a 120 Hz hum and, when given, distant babble
//!   (other voices, low-passed and summed);
//! - [`take`]: the speech at a level (active-speech dBFS, P.56-like) over
//!   the noise at an SNR, with noise before and after, as a take records.

/// A deterministic pseudo-random source (xorshift32).
pub struct Rng(pub u32);

impl Rng {
    pub fn uniform(&mut self) -> f32 {
        let s = &mut self.0;
        *s ^= *s << 13;
        *s ^= *s >> 17;
        *s ^= *s << 5;
        (*s as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

pub const RATE: usize = 16_000;

/// RMS in dBFS of `x` (-120 for silence).
pub fn db(x: &[f32]) -> f32 {
    if x.is_empty() {
        return -120.0;
    }
    let ms = x.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / x.len() as f64;
    if ms <= 1e-12 {
        -120.0
    } else {
        (10.0 * ms.log10()) as f32
    }
}

/// The active level of speech (dBFS): the RMS of the 20 ms frames within
/// 25 dB of the loudest, so pauses don't count (a simple ITU-T P.56).
pub fn active_db(x: &[f32]) -> f32 {
    let frames: Vec<f32> = x.chunks(RATE / 50).map(db).collect();
    let top = frames.iter().copied().fold(-120.0, f32::max);
    let active: Vec<f32> = frames.iter().copied().filter(|d| *d > top - 25.0).collect();
    if active.is_empty() {
        return -120.0;
    }
    let ms: f32 = active.iter().map(|d| 10f32.powf(d / 10.0)).sum::<f32>() / active.len() as f32;
    10.0 * ms.log10()
}

/// `x` scaled so its active level is `target_db`.
pub fn at_level(x: &[f32], target_db: f32) -> Vec<f32> {
    let g = 10f32.powf((target_db - active_db(x)) / 20.0);
    x.iter().map(|s| s * g).collect()
}

fn biquad_lowpass(x: &mut [f32], hz: f32) {
    let w = 2.0 * std::f32::consts::PI * hz / RATE as f32;
    let (s, c) = w.sin_cos();
    let alpha = s / (2.0 * std::f32::consts::FRAC_1_SQRT_2);
    let a0 = 1.0 + alpha;
    let b0 = (1.0 - c) / 2.0 / a0;
    let b1 = (1.0 - c) / a0;
    let a1 = -2.0 * c / a0;
    let a2 = (1.0 - alpha) / a0;
    let (mut x1, mut x2, mut y1, mut y2) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for v in x.iter_mut() {
        let y = b0 * *v + b1 * x1 + b0 * x2 - a1 * y1 - a2 * y2;
        x2 = x1;
        x1 = *v;
        y2 = y1;
        y1 = y;
        *v = y;
    }
}

/// `n` samples of café noise, normalised to an RMS of -30 dBFS ([`take`]
/// sets the real level). Babble voices, if
/// given, are looped from different offsets, low-passed at 2.5 kHz
/// (distant) and mixed 3 dB under the pink noise.
pub fn cafe(n: usize, seed: u32, babble: &[Vec<f32>]) -> Vec<f32> {
    let mut rng = Rng(seed.max(1));
    let mut st = [0.0f32; 3];
    let mut out: Vec<f32> = (0..n)
        .map(|i| {
            let x = rng.uniform();
            st[0] = 0.99765 * st[0] + x * 0.099_046;
            st[1] = 0.96300 * st[1] + x * 0.296_516_4;
            st[2] = 0.57000 * st[2] + x * 1.052_691_3;
            let pink = (st[0] + st[1] + st[2] + x * 0.1848) * 0.05;
            pink * 0.35
                + 0.004 * (2.0 * std::f32::consts::PI * 120.0 * i as f32 / RATE as f32).sin()
        })
        .collect();
    let voices: Vec<&Vec<f32>> = babble.iter().filter(|b| !b.is_empty()).collect();
    if !voices.is_empty() {
        let mut mix = vec![0.0f32; n];
        for (k, v) in voices.iter().enumerate() {
            let v = at_level(v, -30.0);
            let offset = (k * 7919 + seed as usize * 131) % v.len();
            for (i, m) in mix.iter_mut().enumerate() {
                *m += v[(offset + i) % v.len()];
            }
        }
        biquad_lowpass(&mut mix, 2500.0);
        let g = 10f32.powf((db(&out) - 3.0 - db(&mix)) / 20.0);
        for (o, m) in out.iter_mut().zip(mix) {
            *o += m * g;
        }
    }
    let g = 10f32.powf((-30.0 - db(&out)) / 20.0);
    out.iter_mut().for_each(|s| *s *= g);
    out
}

/// LPC coefficients `a[1..=p]` (x[n] ≈ -Σ a[k] x[n-k]) and the residual
/// energy, by Levinson-Durbin on the autocorrelation.
fn lpc(frame: &[f32], p: usize) -> (Vec<f64>, f64) {
    let r: Vec<f64> = (0..=p)
        .map(|k| {
            frame[k..]
                .iter()
                .zip(frame)
                .map(|(a, b)| *a as f64 * *b as f64)
                .sum::<f64>()
        })
        .collect();
    let mut a = vec![0.0f64; p + 1];
    a[0] = 1.0;
    let mut e = r[0] * (1.0 + 1e-9) + 1e-12;
    for i in 1..=p {
        let mut acc = r[i];
        for j in 1..i {
            acc += a[j] * r[i - j];
        }
        let k = -acc / e;
        let prev = a.clone();
        for j in 1..i {
            a[j] = prev[j] + k * prev[i - j];
        }
        a[i] = k;
        e *= 1.0 - k * k;
        if e <= 0.0 {
            e = 1e-12;
            break;
        }
    }
    (a, e)
}

/// A whispered version of `speech`: the same spectral envelope per 32 ms
/// frame (LPC order 18, 50 % overlap-add), excited by white noise instead
/// of the voice, then high-shelved (+6 dB/oct above ~1 kHz, cut below
/// 300 Hz) like a whisper. Keeps the speech's active level.
pub fn whisperize(speech: &[f32], seed: u32) -> Vec<f32> {
    const N: usize = 512;
    const HOP: usize = N / 2;
    const P: usize = 18;
    let mut rng = Rng(seed.max(1));
    let win: Vec<f32> = (0..N)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / N as f32).cos())
        .collect();
    // Pre-emphasis for the analysis, as speech LPC usually does.
    let pre: Vec<f32> = std::iter::once(speech.first().copied().unwrap_or(0.0))
        .chain(speech.windows(2).map(|w| w[1] - 0.9 * w[0]))
        .collect();
    let mut out = vec![0.0f32; speech.len() + N];
    let mut start = 0;
    while start + N <= pre.len() {
        let frame: Vec<f32> = (0..N).map(|i| pre[start + i] * win[i]).collect();
        let (a, e) = lpc(&frame, P);
        let gain = (e / N as f64).sqrt() as f32 * 1.7;
        let mut hist = [0.0f64; P + 1];
        let mut synth = vec![0.0f32; N];
        for s in synth.iter_mut() {
            let mut y = (rng.uniform() * gain) as f64;
            for k in 1..=P {
                y -= a[k] * hist[k - 1];
            }
            hist.rotate_right(1);
            hist[0] = y;
            *s = y as f32;
        }
        for i in 0..N {
            out[start + i] += synth[i] * win[i];
        }
        start += HOP;
    }
    out.truncate(speech.len());
    // Whisper tilt: a gentle high-pass (energy below ~300 Hz removed).
    let mut prev_x = 0.0f32;
    let mut prev_y = 0.0f32;
    let k = (-2.0 * std::f32::consts::PI * 300.0 / RATE as f32).exp();
    for v in out.iter_mut() {
        let y = k * (prev_y + *v - prev_x);
        prev_x = *v;
        prev_y = y;
        *v = y;
    }
    at_level(&out, active_db(speech))
}

/// A take: `lead_s` of noise, `speech` at `level_db` (active), `tail_s`
/// of noise. `snr_db` = speech active level − noise RMS; `None` means a
/// quiet room (mic self-noise at -80 dBFS only).
pub fn take(
    speech: &[f32],
    level_db: f32,
    snr_db: Option<f32>,
    noise_seed: u32,
    babble: &[Vec<f32>],
    lead_s: f32,
    tail_s: f32,
) -> Vec<f32> {
    let lead = (lead_s * RATE as f32) as usize;
    let tail = (tail_s * RATE as f32) as usize;
    let n = lead + speech.len() + tail;
    let speech = at_level(speech, level_db);
    let mut out = vec![0.0f32; n];
    out[lead..lead + speech.len()].copy_from_slice(&speech);
    add_noise(&mut out, snr_db.map(|s| level_db - s), noise_seed, babble);
    out
}

/// Only noise for `secs` (a take where nobody spoke): café noise at
/// `noise_db` dBFS RMS, or the mic's self-noise alone (`None`).
pub fn noise_take(secs: f32, noise_db: Option<f32>, seed: u32, babble: &[Vec<f32>]) -> Vec<f32> {
    let mut out = vec![0.0f32; (secs * RATE as f32) as usize];
    add_noise(&mut out, noise_db, seed, babble);
    out
}

fn add_noise(out: &mut [f32], noise_db: Option<f32>, seed: u32, babble: &[Vec<f32>]) {
    let mut rng = Rng(seed.wrapping_mul(2_654_435_761).max(1));
    // Mic self-noise, always: -80 dBFS white.
    let self_noise = 10f32.powf(-80.0 / 20.0) * 3f32.sqrt();
    for v in out.iter_mut() {
        *v += rng.uniform() * self_noise;
    }
    if let Some(level) = noise_db {
        let g = 10f32.powf((level + 30.0) / 20.0);
        let noise = cafe(out.len(), seed, babble);
        for (v, c) in out.iter_mut().zip(noise) {
            *v += c * g;
        }
    }
}

/// A formant "talker" for unit tests: `secs` of syllables (~200 ms, a
/// voiced vowel with an F0 around 120 Hz through three formants, often
/// with a fricative onset), words separated by short pauses and a longer
/// pause now and then. `whisper`: the vowels are excited by noise instead
/// of the voice (no F0), as in a whisper. Active level about -26 dBFS.
pub fn synth_speech(secs: f32, seed: u32, whisper: bool) -> Vec<f32> {
    let mut rng = Rng(seed.max(1));
    let n = (secs * RATE as f32) as usize;
    let mut out = vec![0.0f32; n];
    // Vowel formants (Hz).
    const VOWELS: [[f32; 3]; 5] = [
        [730.0, 1090.0, 2440.0],
        [270.0, 2290.0, 3010.0],
        [530.0, 1840.0, 2480.0],
        [570.0, 840.0, 2410.0],
        [300.0, 870.0, 2240.0],
    ];
    let mut t = 0usize;
    let mut phase = 0.0f32;
    let mut syllable = 0;
    while t < n {
        syllable += 1;
        // A pause between words, longer every few words.
        let pause = if syllable % 7 == 0 {
            350
        } else if syllable % 2 == 0 {
            90
        } else {
            30
        };
        t += pause * RATE / 1000;
        if t >= n {
            break;
        }
        // A fricative onset (noise, high band) half the time.
        if rng.uniform() > 0.0 {
            let len = (60 + (rng.uniform().abs() * 40.0) as usize) * RATE / 1000;
            let mut fric: Vec<f32> = (0..len).map(|_| rng.uniform() * 0.08).collect();
            // Crude high-pass: difference.
            for i in (1..fric.len()).rev() {
                fric[i] -= fric[i - 1];
            }
            for (i, f) in fric.iter().enumerate() {
                if t + i < n {
                    out[t + i] += *f;
                }
            }
            t += len;
        }
        let len = (140 + (rng.uniform().abs() * 120.0) as usize) * RATE / 1000;
        let v = VOWELS[(syllable * 3 + (rng.uniform().abs() * 5.0) as usize) % 5];
        let f0 = 120.0 + 20.0 * rng.uniform();
        // Source: a pulse train (voice) or noise (whisper).
        let src: Vec<f32> = (0..len)
            .map(|_| {
                if whisper {
                    rng.uniform() * 0.3
                } else {
                    phase += f0 / RATE as f32;
                    if phase >= 1.0 {
                        phase -= 1.0;
                        1.0
                    } else {
                        0.0
                    }
                }
            })
            .collect();
        let mut voiced = vec![0.0f32; len];
        for (k, f) in v.iter().enumerate() {
            let bw = 80.0 + 40.0 * k as f32;
            let r = (-std::f32::consts::PI * bw / RATE as f32).exp();
            let c = 2.0 * r * (2.0 * std::f32::consts::PI * f / RATE as f32).cos();
            let (mut y1, mut y2) = (0.0f32, 0.0f32);
            let g = (1.0 - r) * 0.6 / (k as f32 + 1.0);
            for (i, s) in src.iter().enumerate() {
                let y = s * g + c * y1 - r * r * y2;
                y2 = y1;
                y1 = y;
                voiced[i] += y;
            }
        }
        for (i, s) in voiced.iter().enumerate() {
            if t + i >= n {
                break;
            }
            // Syllable envelope: rise and fall.
            let x = i as f32 / len as f32;
            let env = (std::f32::consts::PI * x).sin().powf(0.6);
            out[t + i] += s * env;
        }
        t += len;
    }
    if whisper {
        // Same high tilt as `whisperize`.
        let mut prev = 0.0;
        for v in out.iter_mut() {
            let y = *v - 0.85 * prev;
            prev = *v;
            *v = y;
        }
    }
    at_level(&out, -26.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_and_snr_come_out_as_asked() {
        let s = synth_speech(3.0, 5, false);
        assert!((active_db(&s) - -26.0).abs() < 0.1);
        let t = take(&s, -50.0, Some(3.0), 9, &[], 0.5, 0.5);
        let lead = &t[..RATE / 2];
        assert!((db(lead) - -53.0).abs() < 1.0, "noise {}", db(lead));
        let n = noise_take(1.0, None, 3, &[]);
        assert!((db(&n) - -80.0).abs() < 1.0, "{}", db(&n));
    }

    #[test]
    fn a_whisper_has_no_voicing_and_keeps_the_level() {
        let s = synth_speech(2.0, 7, false);
        let w = whisperize(&s, 3);
        assert!((active_db(&w) - active_db(&s)).abs() < 0.5);
        // Voicing shows as a strong autocorrelation peak at the F0 lag
        // (100-140 Hz); the whisper has none.
        let peak = |x: &[f32]| {
            let seg = &x[RATE / 2..RATE / 2 + 2048];
            let r0: f32 = seg.iter().map(|v| v * v).sum::<f32>().max(1e-9);
            (114..160)
                .map(|lag| seg[lag..].iter().zip(seg).map(|(a, b)| a * b).sum::<f32>() / r0)
                .fold(f32::MIN, f32::max)
        };
        // The synthetic talker may pause at 0.5 s; search a voiced stretch.
        let voiced = (0..6)
            .map(|k| peak(&s[k * 1000..]))
            .fold(f32::MIN, f32::max);
        let whispered = (0..6)
            .map(|k| peak(&w[k * 1000..]))
            .fold(f32::MIN, f32::max);
        assert!(voiced > 0.3, "voiced {voiced}");
        assert!(
            whispered < voiced - 0.15,
            "whispered {whispered} vs {voiced}"
        );
    }
}
