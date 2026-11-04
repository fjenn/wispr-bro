// SPDX-License-Identifier: GPL-3.0-or-later
//! OT2-B bench (ignored tests): quiet speech and whispers through the
//! shipped capture path, before and after the input conditioning.
//!
//! Each take of the quiet set ([`super::quiet_set`]) goes through Handy's
//! own `CaptureSink` with Handy's Silero VAD and smoothing (prefill, onset,
//! offline hangover; as `create_smoothed_detector` builds them), the short
//! take padding of `stop_recording`, then the take conditioning and the
//! recognizer (transcribe-cpp as the engine worker runs it, language en).
//! The WER per level and SNR is printed, plus what the noise-only takes
//! produced (they must stay empty). The same set also goes through the mic
//! meter and the bar's waveform (`meters_of_the_quiet_set`).
//!
//! ```text
//! scripts/wb-quiet/make-speech.sh <speech dir>
//! WB_QUIET_SPEECH=<speech dir> WB_ASR_MODEL=<gguf> \
//!   cargo test --lib fork::audio::quiet_bench::wer -- --ignored --nocapture
//! WB_QUIET_SPEECH=<speech dir> \
//!   cargo test --lib fork::audio::quiet_bench::meters -- --ignored --nocapture
//! ```
//! `WB_QUIET_MODES=before,after` picks the pipelines (default both).

use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};

use handy_recorder::{AudioChunk, Sink};

use super::quiet_set::{noise_take, take, whisperize};
use crate::audio_toolkit::audio::{
    AudioFrameCallback, AudioVisualiser, CaptureSink, CaptureVad, LevelCallback, VadPolicy,
};
use crate::audio_toolkit::vad::{
    frames_for_duration_ms, SmoothedVad, VoiceActivityDetector, VAD_OFFLINE_HANGOVER_MS,
    VAD_ONSET_MS, VAD_PREFILL_MS, VAD_STREAMING_HANGOVER_MS,
};
use crate::audio_toolkit::SileroVad;

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

/// One sentence of the set: its audio and words.
pub struct Sentence {
    pub audio: Vec<f32>,
    pub text: String,
}

pub struct Speech {
    pub talker: Vec<Sentence>,
    pub babble: Vec<Vec<f32>>,
}

pub fn load_speech(dir: &Path) -> Speech {
    let refs = std::fs::read_to_string(dir.join("refs.tsv")).expect("refs.tsv");
    let talker = refs
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(f, t)| Sentence {
            audio: crate::audio_toolkit::read_wav_samples(dir.join(f)).expect("wav"),
            text: t.to_owned(),
        })
        .collect();
    let mut babble = Vec::new();
    for i in 1..=12 {
        let p = dir.join(format!("b{i:02}.wav"));
        if p.exists() {
            babble.push(crate::audio_toolkit::read_wav_samples(p).expect("wav"));
        }
    }
    Speech { talker, babble }
}

/// A speech tier: name, active level (dBFS), whispered (LPC) or voiced,
/// and the SNRs it is heard at (`None`: a quiet room).
pub struct Tier {
    pub name: &'static str,
    pub level: f32,
    pub lpc: bool,
    pub snrs: &'static [Option<f32>],
}

pub const TIERS: &[Tier] = &[
    Tier {
        name: "normal",
        level: -26.0,
        lpc: false,
        snrs: &[None, Some(20.0), Some(10.0)],
    },
    Tier {
        name: "loud",
        level: -14.0,
        lpc: false,
        snrs: &[None, Some(20.0)],
    },
    Tier {
        name: "quiet-20",
        level: -46.0,
        lpc: false,
        snrs: &[None, Some(12.0), Some(6.0), Some(3.0), Some(0.0)],
    },
    Tier {
        name: "soft-30",
        level: -56.0,
        lpc: false,
        snrs: &[None, Some(12.0), Some(6.0), Some(3.0), Some(0.0)],
    },
    Tier {
        name: "soft-35",
        level: -61.0,
        lpc: false,
        snrs: &[None, Some(6.0), Some(3.0), Some(0.0)],
    },
    Tier {
        name: "whisper-30",
        level: -56.0,
        lpc: true,
        snrs: &[None, Some(12.0), Some(6.0), Some(3.0), Some(0.0)],
    },
    Tier {
        name: "whisper-35",
        level: -61.0,
        lpc: true,
        snrs: &[None, Some(6.0), Some(3.0), Some(0.0)],
    },
];

/// Noise-only takes: café noise at these dBFS (`None`: a quiet room).
pub const NOISE_ONLY: &[Option<f32>] = &[None, Some(-70.0), Some(-58.0), Some(-48.0), Some(-38.0)];

pub fn snr_name(s: Option<f32>) -> String {
    s.map_or("clean".into(), |s| format!("{s:.0}dB"))
}

/// The take for sentence `i` of `tier` at `snr`.
pub fn tier_take(
    speech: &Speech,
    i: usize,
    tier: &Tier,
    snr: Option<f32>,
    lead_s: f32,
) -> Vec<f32> {
    let s = &speech.talker[i];
    let src = if tier.lpc {
        whisperize(&s.audio, 11 + i as u32)
    } else {
        s.audio.clone()
    };
    take(
        &src,
        tier.level,
        snr,
        100 + i as u32,
        &speech.babble,
        lead_s,
        0.6,
    )
}

fn silero_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/models/silero-vad-v6.2-F32.gguf")
}

/// Handy's capture VAD (`create_capture_vad` for Silero), with the fork's
/// scorer wrapper when `after`.
fn capture_vad(after: bool) -> CaptureVad {
    let silero = SileroVad::new(silero_path(), 0.3).expect("silero");
    let detector: Box<dyn VoiceActivityDetector> = if after {
        super::condition::detector(Box::new(silero))
    } else {
        Box::new(silero)
    };
    let fs = detector.frame_samples();
    let smoothed = SmoothedVad::new(
        detector,
        frames_for_duration_ms(VAD_PREFILL_MS, fs),
        frames_for_duration_ms(VAD_OFFLINE_HANGOVER_MS, fs),
        frames_for_duration_ms(VAD_ONSET_MS, fs),
    );
    CaptureVad::new(
        Box::new(smoothed),
        frames_for_duration_ms(VAD_OFFLINE_HANGOVER_MS, fs),
        frames_for_duration_ms(VAD_STREAMING_HANGOVER_MS, fs),
    )
}

/// What Handy's recorder keeps of `audio` (VAD frames, 512-sample chunks
/// with the last zero-padded) and the short-take padding.
pub fn captured(audio: &[f32], vad: CaptureVad) -> (Vec<f32>, CaptureVad) {
    let fs = vad.frame_samples();
    let on_levels: LevelCallback = Arc::new(|_| {});
    let on_audio: AudioFrameCallback = Arc::new(|_| {});
    let (tx, _rx) = mpsc::channel();
    let mut sink = CaptureSink::new(vad, VadPolicy::Offline, on_levels, on_audio, tx);
    for c in audio.chunks(fs) {
        let mut buf = c.to_vec();
        buf.resize(fs, 0.0);
        sink.process_chunk(AudioChunk {
            samples: &buf,
            sample_rate: 16_000,
            channels: 1,
            valid_frames: c.len(),
        });
    }
    let out = sink.finish();
    let mut s = out.samples;
    if !s.is_empty() && s.len() < 16_000 {
        s.resize(16_000 * 5 / 4, 0.0);
    }
    (s, out.vad)
}

fn words(s: &str) -> Vec<String> {
    s.to_lowercase()
        .replace(['-', '—'], " ")
        .split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric() || *c == '\'')
                .collect::<String>()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// Word errors (substitutions + deletions + insertions) and reference words.
pub fn word_errors(reference: &str, hypothesis: &str) -> (usize, usize) {
    let r = words(reference);
    let h = words(hypothesis);
    let mut d: Vec<usize> = (0..=h.len()).collect();
    for (i, rw) in r.iter().enumerate() {
        let mut prev = d[0];
        d[0] = i + 1;
        for (j, hw) in h.iter().enumerate() {
            let cur = d[j + 1];
            d[j + 1] = if rw == hw {
                prev
            } else {
                1 + prev.min(d[j]).min(d[j + 1])
            };
            prev = cur;
        }
    }
    (d[h.len()], r.len())
}

/// A pipeline of the WER bench: the label, whether the detector is
/// wrapped, and the recognizer gain's noise ceiling (`None`: no gain).
/// `before`, `after`, `vad` (wrapper only), `gain` (gain only),
/// `after<N>` (both, ceiling -N dBFS).
fn wer_modes() -> Vec<(String, bool, Option<f32>)> {
    let m = env("WB_QUIET_MODES").unwrap_or_else(|| "before,after".into());
    m.split(',')
        .map(|m| m.trim().to_owned())
        .filter_map(|m| {
            let ceiling = super::condition::NOISE_CEILING_DB;
            Some(match m.as_str() {
                "before" => (m, false, None),
                "after" => (m, true, Some(ceiling)),
                "vad" => (m, true, None),
                "gain" => (m, false, Some(ceiling)),
                other => {
                    let n: f32 = other.strip_prefix("after")?.parse().ok()?;
                    (m, true, Some(-n))
                }
            })
        })
        .collect()
}

fn modes() -> Vec<bool> {
    let m = env("WB_QUIET_MODES").unwrap_or_else(|| "before,after".into());
    m.split(',')
        .filter_map(|m| match m.trim() {
            "before" => Some(false),
            "after" => Some(true),
            _ => None,
        })
        .collect()
}

/// The takes of the set by name: `(name, audio, reference words)`; the
/// noise-only takes have no words.
fn takes(speech: &Speech) -> Vec<(String, Vec<f32>, String)> {
    let mut out = Vec::new();
    for tier in TIERS {
        for &snr in tier.snrs {
            for (i, s) in speech.talker.iter().enumerate() {
                out.push((
                    format!("{}_{}_{i}", tier.name, snr_name(snr)),
                    tier_take(speech, i, tier, snr, 0.6),
                    s.text.clone(),
                ));
            }
        }
    }
    for &noise in NOISE_ONLY {
        for seed in 0..4u32 {
            let n = noise.map_or("room".into(), |n| format!("{:.0}dBFS", -n));
            out.push((
                format!("noise_{n}_{seed}"),
                noise_take(3.0, noise, 900 + seed, &speech.babble),
                String::new(),
            ));
        }
    }
    out
}

/// Phase 1 (CPU: Silero): what the recorder keeps of every take, as the
/// recognizer would get it, written to `WB_QUIET_KEPT/<mode>/<take>.wav`
/// (a process can register only one transcribe-cpp backend mask, so the
/// recognizer runs in phase 2, [`wer`]). An empty take writes no file.
#[test]
#[ignore = "needs WB_QUIET_SPEECH and WB_QUIET_KEPT"]
fn kept() {
    let (Some(dir), Some(out)) = (env("WB_QUIET_SPEECH"), env("WB_QUIET_KEPT")) else {
        eprintln!("quiet_bench: set WB_QUIET_SPEECH and WB_QUIET_KEPT");
        return;
    };
    let speech = load_speech(Path::new(&dir));
    let all = takes(&speech);
    for (label, wrap, ceiling) in wer_modes() {
        let dir = Path::new(&out).join(&label);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let condition = |pcm: Vec<f32>| match ceiling {
            Some(c) => super::condition::for_recognizer_under(pcm, c),
            None => pcm,
        };
        let mut vad = Some(capture_vad(wrap));
        let mut kept_n = 0;
        for (name, audio, words) in &all {
            // Noise-only takes also go in whole, as with the VAD setting
            // off: the gain must not make words out of noise.
            if words.is_empty() {
                let pcm = condition(audio.clone());
                crate::audio_toolkit::save_wav_file(dir.join(format!("novad_{name}.wav")), &pcm)
                    .unwrap();
            }
            let (kept, v) = captured(audio, vad.take().unwrap());
            vad = Some(v);
            if kept.is_empty() {
                continue;
            }
            kept_n += 1;
            let pcm = condition(kept);
            crate::audio_toolkit::save_wav_file(dir.join(format!("{name}.wav")), &pcm).unwrap();
        }
        println!("{label}: {kept_n} of {} takes kept audio", all.len());
    }
}

/// Phase 2 (the recognizer): WER per tier and SNR of what [`kept`] wrote,
/// the share of takes that came out empty, and the noise-only takes' text.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs WB_QUIET_SPEECH, WB_QUIET_KEPT and WB_ASR_MODEL"]
async fn wer() {
    let (Some(dir), Some(kept), Some(asr)) = (
        env("WB_QUIET_SPEECH"),
        env("WB_QUIET_KEPT"),
        env("WB_ASR_MODEL"),
    ) else {
        eprintln!("quiet_bench: set WB_QUIET_SPEECH, WB_QUIET_KEPT and WB_ASR_MODEL");
        return;
    };
    let speech = load_speech(Path::new(&dir));
    transcribe_cpp::init_backends_default().expect("backends");
    let model = transcribe_cpp::Model::load_with(
        &asr,
        &transcribe_cpp::ModelOptions {
            backend: transcribe_cpp::Backend::Auto,
            device: None,
        },
    )
    .expect("model");
    let mut session = model.session().expect("session");
    let opts = transcribe_cpp::RunOptions {
        language: Some("en".into()),
        ..Default::default()
    };
    let verbose = env("WB_QUIET_VERBOSE").is_some();
    for (label, _, _) in wer_modes() {
        let dir = Path::new(&kept).join(&label);
        let mut run = |name: &str| -> String {
            let p = dir.join(format!("{name}.wav"));
            if !p.exists() {
                return String::new();
            }
            let pcm = crate::audio_toolkit::read_wav_samples(&p).expect("wav");
            session
                .run(&pcm, &opts)
                .map(|t| t.text.trim().to_owned())
                .unwrap_or_default()
        };
        println!(
            "== {label} ({})",
            Path::new(&asr).file_name().unwrap().to_string_lossy()
        );
        println!("{:<11} {:>6} {:>6}  empty", "tier", "snr", "WER%");
        let (mut grand_e, mut grand_n) = (0, 0);
        for tier in TIERS {
            for &snr in tier.snrs {
                let (mut errs, mut total, mut empty) = (0, 0, 0);
                for (i, s) in speech.talker.iter().enumerate() {
                    let text = run(&format!("{}_{}_{i}", tier.name, snr_name(snr)));
                    let (e, n) = word_errors(&s.text, &text);
                    errs += e;
                    total += n;
                    empty += usize::from(text.is_empty());
                    if verbose {
                        println!("   {}#{i}: {text:?}", tier.name);
                    }
                }
                grand_e += errs;
                grand_n += total;
                println!(
                    "{:<11} {:>6} {:>6.1}  {empty}/{}",
                    tier.name,
                    snr_name(snr),
                    100.0 * errs as f32 / total.max(1) as f32,
                    speech.talker.len()
                );
            }
        }
        println!(
            "all         {:>6.1}",
            100.0 * grand_e as f32 / grand_n.max(1) as f32
        );
        for &noise in NOISE_ONLY {
            let n = noise.map_or("room".into(), |n| format!("{:.0}dBFS", -n));
            let outs: Vec<String> = (0..4)
                .map(|seed| run(&format!("noise_{n}_{seed}")))
                .collect();
            let whole: Vec<String> = (0..4)
                .map(|seed| run(&format!("novad_noise_{n}_{seed}")))
                .collect();
            println!("noise-only {n:>7}: {outs:?}  without VAD: {whole:?}");
        }
    }
}

/// The mic meter's and the bar's waveform levels over the set: per tier
/// and SNR, the mean and 90th percentile of what is shown during the
/// speech, and the same during the noise before it.
#[test]
#[ignore = "needs WB_QUIET_SPEECH"]
fn meters() {
    let Some(dir) = env("WB_QUIET_SPEECH") else {
        eprintln!("quiet_bench: set WB_QUIET_SPEECH");
        return;
    };
    let speech = load_speech(Path::new(&dir));
    for after in modes() {
        println!("== {}", if after { "after" } else { "before" });
        println!(
            "{:<11} {:>6} | meter noise p90 / speech p25 mean p90 | bars noise p90 / speech p25 mean p90",
            "tier", "snr"
        );
        for tier in TIERS {
            for &snr in tier.snrs {
                let mut m = Acc::default();
                let mut w = Acc::default();
                for i in 0..speech.talker.len().min(4) {
                    let audio = tier_take(&speech, i, tier, snr, 1.5);
                    let lead = (1.5 * 16_000.0) as usize;
                    let end = audio.len() - (0.6 * 16_000.0) as usize;
                    meter_run(&audio, after, lead, end, &mut m);
                    wave_run(&audio, after, lead, end, &mut w);
                }
                println!(
                    "{:<11} {:>6} | {:>5.2} / {:>5.2} {:>5.2} {:>5.2} | {:>5.2} / {:>5.2} {:>5.2} {:>5.2}",
                    tier.name,
                    snr_name(snr),
                    m.p90(false),
                    m.pct(true, 0.25),
                    m.mean(true),
                    m.p90(true),
                    w.p90(false),
                    w.pct(true, 0.25),
                    w.mean(true),
                    w.p90(true)
                );
            }
        }
        for &noise in NOISE_ONLY {
            let mut m = Acc::default();
            let mut w = Acc::default();
            for seed in 0..3u32 {
                let audio = noise_take(4.0, noise, 900 + seed, &speech.babble);
                meter_run(&audio, after, 0, 0, &mut m);
                wave_run(&audio, after, 0, 0, &mut w);
            }
            println!(
                "noise-only {:>6} | meter p90 {:.2} max {:.2} | bars p90 {:.2} max {:.2}",
                noise.map_or("room".into(), |n| format!("{n:.0}dBFS")),
                m.p90(false),
                m.max(false),
                w.p90(false),
                w.max(false)
            );
        }
    }
}

/// Shown levels, split into the noise lead and the speech.
#[derive(Default)]
pub struct Acc {
    noise: Vec<f32>,
    speech: Vec<f32>,
}

impl Acc {
    /// `t` (samples) against the speech `lead..end`; the tail after the
    /// speech (the release) counts as neither.
    fn push_at(&mut self, t: usize, lead: usize, end: usize, v: f32) {
        if t >= end && end > lead {
            return;
        }
        self.push(t >= lead && end > lead, v);
    }
    fn push(&mut self, speech: bool, v: f32) {
        if speech {
            self.speech.push(v)
        } else {
            self.noise.push(v)
        }
    }
    fn of(&self, speech: bool) -> Vec<f32> {
        let mut v = if speech {
            self.speech.clone()
        } else {
            self.noise.clone()
        };
        v.sort_by(f32::total_cmp);
        v
    }
    pub fn mean(&self, speech: bool) -> f32 {
        let v = self.of(speech);
        v.iter().sum::<f32>() / v.len().max(1) as f32
    }
    pub fn p90(&self, speech: bool) -> f32 {
        self.pct(speech, 0.9)
    }
    pub fn pct(&self, speech: bool, p: f32) -> f32 {
        let v = self.of(speech);
        v.get((v.len() as f32 * p) as usize).copied().unwrap_or(0.0)
    }
    pub fn max(&self, speech: bool) -> f32 {
        self.of(speech).last().copied().unwrap_or(0.0)
    }
}

/// The meter over `audio` (10 ms chunks, as the monitor gets them), one
/// reading per 33 ms (its emit rate, loudest since the last). Skips the
/// first 300 ms (warm-up). `lead..end` is the speech.
pub fn meter_run(audio: &[f32], after: bool, lead: usize, end: usize, acc: &mut Acc) {
    let mut levels = Vec::new();
    if after {
        let mut m = super::speech::SpeechMeter::new(16_000);
        for c in audio.chunks(160) {
            levels.push(m.push(c, 1).level);
        }
    } else {
        let mut d = super::dynamics::Dynamics::meter();
        let mut recent: std::collections::VecDeque<f32> = Default::default();
        for c in audio.chunks(160) {
            recent.push_back(c.iter().map(|s| s * s).sum::<f32>() / c.len() as f32);
            if recent.len() > 3 {
                recent.pop_front();
            }
            let ms = recent.iter().sum::<f32>() / recent.len() as f32;
            let db = if ms > 0.0 { 10.0 * ms.log10() } else { -120.0 };
            levels.push(d.push(db, 10.0));
        }
    }
    for (k, w) in levels.chunks(3).enumerate() {
        let t = k * 480;
        if t < 4_800 {
            continue;
        }
        let v = w.iter().copied().fold(0.0, f32::max);
        acc.push_at(t, lead, end, v);
    }
}

/// The bar's waveform over `audio` as a take feeds it: 512-sample chunks,
/// Handy's visualiser buckets and the chunk's uncapped level (before), or
/// the fork's take tap (after); records the loudest bar per frame.
pub fn wave_run(audio: &[f32], after: bool, lead: usize, end: usize, acc: &mut Acc) {
    let mut vis = AudioVisualiser::new(16_000, 512, 16, 400.0, 4000.0);
    let mut wave = crate::fork::pill::levels::Waveform::default();
    let mut tap = super::speech::SpeechMeter::new(16_000);
    for (k, c) in audio.chunks(512).enumerate() {
        let t = k * 512;
        let now = 1_000 + (t / 16) as u64;
        let reading = tap.push(c, 1);
        let Some(buckets) = vis.feed(c) else {
            continue;
        };
        let ms = c.iter().map(|s| s * s).sum::<f32>() / c.len() as f32;
        let loud = if ms > 1e-12 {
            10.0 * ms.log10()
        } else {
            -120.0
        };
        let bars = if after {
            wave.push_reading(&reading, now)
        } else {
            wave.push(&buckets, now, Some(loud))
        };
        if t < 4_800 {
            continue;
        }
        acc.push_at(t, lead, end, bars.iter().copied().fold(0.0, f32::max));
    }
}

/// The Mac wave's WAVs (MAC-TEST-SCRIPT OT2-B.1-OT2-B.3): a few takes of the
/// set as 16 kHz mono files under `WB_QUIET_OUT`, each with a `.json` of
/// its segments in `scripts/wb-meter/make-wav.py`'s format (noise, speech,
/// noise_after, silence) and a `.txt` of its words.
#[test]
#[ignore = "needs WB_QUIET_SPEECH and WB_QUIET_OUT"]
fn write_wavs() {
    let (Some(dir), Some(out)) = (env("WB_QUIET_SPEECH"), env("WB_QUIET_OUT")) else {
        eprintln!("quiet_bench: set WB_QUIET_SPEECH and WB_QUIET_OUT");
        return;
    };
    let speech = load_speech(Path::new(&dir));
    let out = Path::new(&out);
    std::fs::create_dir_all(out).unwrap();
    let pick = [
        ("normal", Some(20.0), 0usize),
        ("quiet-20", Some(3.0), 2),
        ("whisper-30", Some(3.0), 4),
        ("whisper-35", Some(0.0), 6),
    ];
    for (name, snr, i) in pick {
        let tier = TIERS.iter().find(|t| t.name == name).unwrap();
        let lead = 3.0f32;
        let mut audio = tier_take(&speech, i, tier, snr, lead);
        let tail = 2.0f32;
        let n = audio.len();
        // 2 s more noise, then 2 s of digital silence (as make-wav.py).
        let more = take(&[0.0; 16], tier.level, snr, 77, &speech.babble, tail, 0.0);
        audio.extend_from_slice(&more[..(tail * 16_000.0) as usize]);
        audio.extend(std::iter::repeat_n(0.0, 32_000));
        let t_speech = (n as f32 / 16_000.0) - lead - 0.6;
        let file = format!("{name}-{}", snr_name(snr));
        crate::audio_toolkit::save_wav_file(out.join(format!("{file}.wav")), &audio).unwrap();
        let s_end = lead + t_speech;
        let n_end = n as f32 / 16_000.0 + tail;
        std::fs::write(
            out.join(format!("{file}.wav.json")),
            format!(
                "{{\"noise\": [0, {lead}], \"speech\": [{lead}, {s_end:.2}], \"noise_after\": [{:.2}, {n_end:.2}], \"silence\": [{n_end:.2}, {:.2}]}}",
                s_end + 0.6,
                n_end + 2.0
            ),
        )
        .unwrap();
        std::fs::write(out.join(format!("{file}.txt")), &speech.talker[i].text).unwrap();
        println!("wrote {file}.wav");
    }
}
