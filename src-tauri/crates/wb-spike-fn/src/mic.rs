// SPDX-License-Identifier: GPL-3.0-or-later
//! The microphone, opened cold at `T_show` and closed on release/cancel
//! (DEC-47), through Handy's own capture crate (handy-recorder 0.1, the one
//! `src-tauri/src/managers/audio.rs` uses) with Handy's speech format
//! (16 kHz mono). Nothing is recorded to disk except, in the clipping block,
//! the first 4 s of each take as a WAV. Runs on its own thread so a slow
//! device open never delays the key-event loop.
//!
//! RS-01 round 2 adds a second way to start a take, the **prepared** mic
//! (clipping variant b): the default input's CoreAudio unit is built and
//! initialised ahead of time (cpal `build_input_stream`, which on macOS ends
//! in `AudioUnitInitialize` and leaves the unit stopped), and each take only
//! calls `play` (`AudioOutputUnitStart`) and `pause` (`AudioOutputUnitStop`).
//! handy-recorder cannot do this: its `Recorder::open` builds *and* plays the
//! stream, and `start` only attaches a sink to the already-running stream.
//! Whether an initialised-but-stopped unit lights the orange indicator is
//! exactly what round 2 asks the owner.

use handy_recorder::{AudioChunk, Recorder, RecorderConfig, Sink};
use serde_json::json;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::log::Log;
use crate::verdict::MicSpan;

pub const SAVE_SECONDS: usize = 4;

pub enum MicCmd {
    Open {
        take: u64,
        requested: Instant,
        save: Option<PathBuf>,
    },
    Close {
        take: u64,
    },
    /// Build and initialise the prepared input unit now, without starting
    /// it. Later opens use it until `Unprepare`. Replies with the device name
    /// and the time the build took, or an error.
    Prepare {
        reply: Sender<Result<(String, f64), String>>,
    },
    /// Drop the prepared unit; opens are cold again.
    Unprepare,
}

#[derive(Debug)]
pub struct MicReport {
    pub take: u64,
    pub span: MicSpan,
}

#[derive(Default)]
struct Shared {
    first_audio: Option<Instant>,
    peak: f32,
}

struct ClipSink {
    keep: usize,
    samples: Vec<f32>,
    shared: Arc<Mutex<Shared>>,
}

impl Sink for ClipSink {
    fn process_chunk(&mut self, chunk: AudioChunk<'_>) {
        let real = &chunk.samples[..chunk.valid_frames * chunk.channels as usize];
        if let Ok(mut s) = self.shared.lock() {
            if s.first_audio.is_none() {
                s.first_audio = Some(Instant::now());
            }
            for &x in real {
                s.peak = s.peak.max(x.abs());
            }
        }
        let room = self.keep.saturating_sub(self.samples.len());
        self.samples
            .extend_from_slice(&real[..room.min(real.len())]);
    }
}

struct Open {
    take: u64,
    recorder: Recorder<ClipSink>,
    shared: Arc<Mutex<Shared>>,
    span: MicSpan,
    device: String,
    save: Option<PathBuf>,
}

fn open(
    log: &Log,
    take: u64,
    requested: Instant,
    save: Option<PathBuf>,
) -> Result<Open, Box<(MicSpan, String)>> {
    let mut span = MicSpan {
        take,
        requested_ms: log.ms(requested),
        ..Default::default()
    };
    let recorder = match Recorder::open_with_failure_handler(RecorderConfig::speech(), |e| {
        eprintln!("  (microphone stopped working: {e})");
    }) {
        Ok(r) => r,
        Err(e) => {
            span.error = Some(e.to_string());
            return Err(Box::new((span, e.to_string())));
        }
    };
    let opened = Instant::now();
    span.opened_ms = Some(log.ms(opened));
    let shared = Arc::new(Mutex::new(Shared::default()));
    let rate = recorder.info().format.sample_rate as usize;
    let keep = if save.is_some() {
        SAVE_SECONDS * rate
    } else {
        0
    };
    let device = recorder.info().device.name.clone();
    if let Err(e) = recorder.start(ClipSink {
        keep,
        samples: Vec::new(),
        shared: Arc::clone(&shared),
    }) {
        span.error = Some(e.to_string());
        return Err(Box::new((span, e.to_string())));
    }
    log.write(
        "mic_open",
        opened,
        json!({"take": take, "requested_ms": span.requested_ms, "open_took_ms": log.ms(opened) - span.requested_ms, "device": device}),
    );
    Ok(Open {
        take,
        recorder,
        shared,
        span,
        device,
        save,
    })
}

fn close(log: &Log, o: Open) -> MicReport {
    let Open {
        take,
        recorder,
        shared,
        mut span,
        device: _,
        save,
    } = o;
    let samples = match recorder.stop() {
        Ok(stopped) => stopped.sink.samples,
        Err(e) => {
            span.error = Some(e.to_string());
            Vec::new()
        }
    };
    let rate = recorder.info().format.sample_rate;
    if let Err(e) = recorder.close() {
        span.error.get_or_insert(e.to_string());
    }
    let closed = Instant::now();
    span.closed_ms = Some(log.ms(closed));
    let (first, peak) = shared
        .lock()
        .map(|s| (s.first_audio, s.peak))
        .unwrap_or((None, 0.0));
    span.first_audio_ms = first.map(|t| log.ms(t));
    span.peak = Some(peak);
    if let Some(path) = save {
        match write_wav(&path, &samples, rate) {
            Ok(()) => span.wav = Some(path.display().to_string()),
            Err(e) => span.error = Some(format!("wav: {e}")),
        }
    }
    log.write(
        "mic_close",
        closed,
        json!({"take": take, "first_audio_ms": span.first_audio_ms, "peak": peak, "wav": span.wav, "error": span.error}),
    );
    MicReport { take, span }
}

pub fn write_wav(path: &std::path::Path, samples: &[f32], rate: u32) -> Result<(), hound::Error> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec)?;
    for &s in samples {
        w.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)?;
    }
    w.finalize()
}

// ---------------------------------------------------------------------------
// The prepared mic (round 2, variant b)

#[derive(Default)]
struct PrepState {
    recording: bool,
    first_audio: Option<Instant>,
    peak: f32,
    /// Mono, at the device rate.
    samples: Vec<f32>,
    keep: usize,
}

struct Prepared {
    stream: cpal::Stream,
    rate: u32,
    device: String,
    state: Arc<Mutex<PrepState>>,
}

/// One take on the prepared unit.
struct Warm {
    take: u64,
    span: MicSpan,
    save: Option<PathBuf>,
}

fn prepare(log: &Log) -> Result<Prepared, String> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let started = Instant::now();
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or("no default input device")?;
    let name = device
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| "unknown".into());
    let supported = device.default_input_config().map_err(|e| e.to_string())?;
    if supported.sample_format() != cpal::SampleFormat::F32 {
        return Err(format!(
            "the prepared mic handles f32 input only (device gives {:?})",
            supported.sample_format()
        ));
    }
    let config = supported.config();
    let channels = config.channels.max(1) as usize;
    let rate = config.sample_rate;
    let state = Arc::new(Mutex::new(PrepState::default()));
    let cb_state = Arc::clone(&state);
    let stream = device
        .build_input_stream(
            config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // try_lock: the session thread holds the lock only for the
                // instant of start/stop; a spike may drop that one buffer.
                let Ok(mut s) = cb_state.try_lock() else {
                    return;
                };
                if !s.recording {
                    return;
                }
                if s.first_audio.is_none() {
                    s.first_audio = Some(Instant::now());
                }
                for frame in data.chunks(channels) {
                    let x = frame.iter().sum::<f32>() / frame.len() as f32;
                    s.peak = s.peak.max(x.abs());
                    if s.samples.len() < s.keep {
                        s.samples.push(x);
                    }
                }
            },
            |e| eprintln!("  (prepared microphone error: {e})"),
            None,
        )
        .map_err(|e| e.to_string())?;
    log.write(
        "mic_prepared",
        Instant::now(),
        json!({"device": name, "rate": rate, "channels": channels, "build_ms": started.elapsed().as_secs_f64() * 1000.0}),
    );
    Ok(Prepared {
        stream,
        rate,
        device: name,
        state,
    })
}

fn warm_start(
    log: &Log,
    p: &Prepared,
    take: u64,
    requested: Instant,
    save: Option<PathBuf>,
) -> Result<Warm, Box<(MicSpan, String)>> {
    use cpal::traits::StreamTrait;
    let mut span = MicSpan {
        take,
        requested_ms: log.ms(requested),
        ..Default::default()
    };
    if let Ok(mut s) = p.state.lock() {
        *s = PrepState {
            recording: true,
            keep: if save.is_some() {
                SAVE_SECONDS * p.rate as usize
            } else {
                0
            },
            ..Default::default()
        };
    }
    if let Err(e) = p.stream.play() {
        span.error = Some(e.to_string());
        return Err(Box::new((span, e.to_string())));
    }
    let opened = Instant::now();
    span.opened_ms = Some(log.ms(opened));
    log.write(
        "mic_open",
        opened,
        json!({"take": take, "requested_ms": span.requested_ms, "open_took_ms": log.ms(opened) - span.requested_ms, "device": p.device, "prepared": true}),
    );
    Ok(Warm { take, span, save })
}

fn warm_stop(log: &Log, p: &Prepared, w: Warm) -> MicReport {
    use cpal::traits::StreamTrait;
    let Warm {
        take,
        mut span,
        save,
    } = w;
    if let Err(e) = p.stream.pause() {
        span.error = Some(e.to_string());
    }
    let (first, peak, samples) = match p.state.lock() {
        Ok(mut s) => {
            s.recording = false;
            (s.first_audio, s.peak, std::mem::take(&mut s.samples))
        }
        Err(_) => (None, 0.0, Vec::new()),
    };
    let closed = Instant::now();
    span.closed_ms = Some(log.ms(closed));
    span.first_audio_ms = first.map(|t| log.ms(t));
    span.peak = Some(peak);
    if let Some(path) = save {
        match write_wav(&path, &to_16k(&samples, p.rate), 16_000) {
            Ok(()) => span.wav = Some(path.display().to_string()),
            Err(e) => span.error = Some(format!("wav: {e}")),
        }
    }
    log.write(
        "mic_close",
        closed,
        json!({"take": take, "first_audio_ms": span.first_audio_ms, "peak": peak, "wav": span.wav, "error": span.error, "prepared": true}),
    );
    MicReport { take, span }
}

/// Mono samples at `rate` → 16 kHz (block average for whole multiples of
/// 16 kHz, linear interpolation otherwise). Good enough for the onset check
/// and the speech model; this is a spike, not the product's resampler.
pub fn to_16k(samples: &[f32], rate: u32) -> Vec<f32> {
    if rate == 16_000 || samples.is_empty() {
        return samples.to_vec();
    }
    if rate.is_multiple_of(16_000) {
        let n = (rate / 16_000) as usize;
        return samples
            .chunks(n)
            .map(|c| c.iter().sum::<f32>() / c.len() as f32)
            .collect();
    }
    let ratio = rate as f64 / 16_000.0;
    let out_len = (samples.len() as f64 / ratio).floor() as usize;
    (0..out_len)
        .map(|i| {
            let pos = i as f64 * ratio;
            let j = pos.floor() as usize;
            let frac = (pos - j as f64) as f32;
            let a = samples[j];
            let b = *samples.get(j + 1).unwrap_or(&a);
            a + (b - a) * frac
        })
        .collect()
}

enum Current {
    Cold(Box<Open>),
    Warm(Warm),
}

/// The mic thread: executes the commands in order and reports each closed
/// take.
pub fn run(log: Arc<Log>, cmds: Receiver<MicCmd>, reports: impl Fn(MicReport)) {
    let mut current: Option<Current> = None;
    let mut prepared: Option<Prepared> = None;
    let finish = |c: Current, prepared: &Option<Prepared>| match (c, prepared) {
        (Current::Cold(o), _) => close(&log, *o),
        (Current::Warm(w), Some(p)) => warm_stop(&log, p, w),
        (Current::Warm(w), None) => MicReport {
            take: w.take,
            span: w.span,
        },
    };
    for cmd in cmds {
        match cmd {
            MicCmd::Prepare { reply } => {
                if let Some(c) = current.take() {
                    reports(finish(c, &prepared));
                }
                prepared = None;
                let started = Instant::now();
                let r = prepare(&log).map(|p| {
                    let out = (p.device.clone(), started.elapsed().as_secs_f64() * 1000.0);
                    prepared = Some(p);
                    out
                });
                if let Err(e) = &r {
                    log.write("mic_prepare_error", Instant::now(), json!({"error": e}));
                }
                let _ = reply.send(r);
            }
            MicCmd::Unprepare => {
                if let Some(c) = current.take() {
                    reports(finish(c, &prepared));
                }
                if prepared.take().is_some() {
                    log.write("mic_unprepared", Instant::now(), json!({}));
                }
            }
            MicCmd::Open {
                take,
                requested,
                save,
            } => {
                if let Some(c) = current.take() {
                    reports(finish(c, &prepared));
                }
                let opened = match &prepared {
                    Some(p) => warm_start(&log, p, take, requested, save).map(Current::Warm),
                    None => open(&log, take, requested, save).map(|o| Current::Cold(Box::new(o))),
                };
                match opened {
                    Ok(c) => current = Some(c),
                    Err(err) => {
                        let (span, e) = *err;
                        eprintln!("  (could not open the microphone: {e})");
                        log.write(
                            "mic_error",
                            Instant::now(),
                            json!({"take": take, "error": e}),
                        );
                        reports(MicReport { take, span });
                    }
                }
            }
            MicCmd::Close { take } => {
                let is_it = match &current {
                    Some(Current::Cold(o)) => o.take == take,
                    Some(Current::Warm(w)) => w.take == take,
                    None => false,
                };
                if is_it {
                    if let Some(c) = current.take() {
                        reports(finish(c, &prepared));
                    }
                }
            }
        }
    }
    if let Some(c) = current.take() {
        reports(finish(c, &prepared));
    }
}

/// Setup check: opens the mic for `dur`, returns (device, peak level,
/// fn-free open→first-audio ms). Triggers macOS's microphone prompt the
/// first time.
pub fn probe(dur: Duration) -> Result<(String, f32, Option<f64>), String> {
    let log = Log::null(Instant::now());
    let start = Instant::now();
    let o = open(&log, 0, start, None).map_err(|err| err.1)?;
    std::thread::sleep(dur);
    let device = o.device.clone();
    let report = close(&log, o);
    if let Some(e) = report.span.error {
        return Err(e);
    }
    Ok((
        device,
        report.span.peak.unwrap_or(0.0),
        report.span.first_audio_ms,
    ))
}

/// Starts the mic thread. Returns the command sender.
pub fn spawn(log: Arc<Log>, reports: impl Fn(MicReport) + Send + 'static) -> Sender<MicCmd> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("wb-spike-fn-mic".into())
        .spawn(move || run(log, rx, reports))
        .expect("spawn mic thread");
    tx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_sink_keeps_only_the_first_samples_and_marks_first_audio() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut sink = ClipSink {
            keep: 5,
            samples: Vec::new(),
            shared: Arc::clone(&shared),
        };
        let data = [0.1, -0.5, 0.2, 0.0];
        for _ in 0..3 {
            sink.process_chunk(AudioChunk {
                samples: &data,
                sample_rate: 16_000,
                channels: 1,
                valid_frames: 4,
            });
        }
        assert_eq!(sink.samples.len(), 5);
        let s = shared.lock().unwrap();
        assert!(s.first_audio.is_some());
        assert_eq!(s.peak, 0.5);
    }

    #[test]
    fn padding_frames_are_ignored() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let mut sink = ClipSink {
            keep: 100,
            samples: Vec::new(),
            shared: Arc::clone(&shared),
        };
        sink.process_chunk(AudioChunk {
            samples: &[0.1, 0.9, 0.9],
            sample_rate: 16_000,
            channels: 1,
            valid_frames: 1,
        });
        assert_eq!(sink.samples, vec![0.1]);
        assert_eq!(shared.lock().unwrap().peak, 0.1);
    }

    /// Real hardware: `cargo test --release -- --ignored prepared`. Prepares
    /// the default input, then starts and stops it three times, printing
    /// requested → started → first audio. Does not judge the indicator.
    #[test]
    #[ignore]
    fn prepared_mic_starts_and_stops_on_real_hardware() {
        let (tx, rx) = std::sync::mpsc::channel();
        let (rep_tx, rep_rx) = std::sync::mpsc::channel();
        let log = Arc::new(Log::null(Instant::now()));
        let l2 = Arc::clone(&log);
        std::thread::spawn(move || run(l2, rx, move |r| rep_tx.send(r).unwrap()));
        let (reply, got) = std::sync::mpsc::channel();
        tx.send(MicCmd::Prepare { reply }).unwrap();
        let (device, build_ms) = got.recv_timeout(Duration::from_secs(10)).unwrap().unwrap();
        println!("prepared {device} in {build_ms:.1} ms");
        for take in 1..=3 {
            std::thread::sleep(Duration::from_millis(700));
            tx.send(MicCmd::Open {
                take,
                requested: Instant::now(),
                save: None,
            })
            .unwrap();
            std::thread::sleep(Duration::from_millis(400));
            tx.send(MicCmd::Close { take }).unwrap();
            let r = rep_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            let s = r.span;
            println!(
                "take {take}: start {:.1} ms, first audio {:.1} ms after request, error {:?}",
                s.opened_ms.unwrap_or(f64::NAN) - s.requested_ms,
                s.first_audio_ms.unwrap_or(f64::NAN) - s.requested_ms,
                s.error
            );
            assert!(s.first_audio_ms.is_some(), "no audio on take {take}");
        }
        // Cold, for comparison.
        tx.send(MicCmd::Unprepare).unwrap();
        tx.send(MicCmd::Open {
            take: 9,
            requested: Instant::now(),
            save: None,
        })
        .unwrap();
        std::thread::sleep(Duration::from_millis(400));
        tx.send(MicCmd::Close { take: 9 }).unwrap();
        let s = rep_rx.recv_timeout(Duration::from_secs(5)).unwrap().span;
        println!(
            "cold: open {:.1} ms, first audio {:.1} ms after request",
            s.opened_ms.unwrap_or(f64::NAN) - s.requested_ms,
            s.first_audio_ms.unwrap_or(f64::NAN) - s.requested_ms
        );
    }

    #[test]
    fn resampling_to_16k() {
        let x: Vec<f32> = (0..48).map(|i| i as f32).collect();
        let y = to_16k(&x, 48_000);
        assert_eq!(y.len(), 16);
        assert_eq!(y[0], 1.0); // mean of 0, 1, 2
        assert_eq!(to_16k(&x, 16_000), x);
        let z = to_16k(&vec![0.5; 441], 44_100);
        assert_eq!(z.len(), 160);
        assert!(z.iter().all(|v| (*v - 0.5).abs() < 1e-6));
        assert!(to_16k(&[], 48_000).is_empty());
    }

    #[test]
    fn wav_round_trip() {
        let path = std::env::temp_dir().join(format!("wb-spike-fn-{}.wav", std::process::id()));
        write_wav(&path, &[0.0, 0.5, -0.5, 1.5], 16_000).unwrap();
        let r = hound::WavReader::open(&path).unwrap();
        assert_eq!(r.spec().sample_rate, 16_000);
        assert_eq!(r.len(), 4);
        std::fs::remove_file(&path).unwrap();
    }
}
