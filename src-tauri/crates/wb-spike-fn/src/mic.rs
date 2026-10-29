// SPDX-License-Identifier: GPL-3.0-or-later
//! The microphone, opened cold at `T_show` and closed on release/cancel
//! (DEC-47), through Handy's own capture crate (handy-recorder 0.1, the one
//! `src-tauri/src/managers/audio.rs` uses) with Handy's speech format
//! (16 kHz mono). Nothing is recorded to disk except, in the clipping block,
//! the first 2 s of each take as a WAV. Runs on its own thread so a slow
//! device open never delays the key-event loop.

use handy_recorder::{AudioChunk, Recorder, RecorderConfig, Sink};
use serde_json::json;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::log::Log;
use crate::verdict::MicSpan;

pub const SAVE_SECONDS: usize = 2;

pub enum MicCmd {
    Open {
        take: u64,
        requested: Instant,
        save: Option<PathBuf>,
    },
    Close {
        take: u64,
    },
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

/// The mic thread: executes Open/Close in order and reports each closed take.
pub fn run(log: Arc<Log>, cmds: Receiver<MicCmd>, reports: impl Fn(MicReport)) {
    let mut current: Option<Open> = None;
    for cmd in cmds {
        match cmd {
            MicCmd::Open {
                take,
                requested,
                save,
            } => {
                if let Some(o) = current.take() {
                    reports(close(&log, o));
                }
                match open(&log, take, requested, save) {
                    Ok(o) => current = Some(o),
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
                if current.as_ref().is_some_and(|o| o.take == take) {
                    if let Some(o) = current.take() {
                        reports(close(&log, o));
                    }
                }
            }
        }
    }
    if let Some(o) = current.take() {
        reports(close(&log, o));
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
