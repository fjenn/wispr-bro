// SPDX-License-Identifier: GPL-3.0-or-later
//! The level meter behind "Test your microphone" (R5 S-08, A-40; S-29a's
//! mic test uses the same API).
//!
//! A monitor session opens its own handy-recorder on the microphone the
//! choice uses (or one the caller names) and sends `ForkMicLevel` about
//! [`EMIT_HZ`] times a second. It is not a take: it never goes through
//! Handy's recording manager, the transcription coordinator, history or the
//! Flow Bar, so it creates no history row and no stats, and it never pastes.
//! A session ends on [`stop`], after [`MAX_SESSION`], or when a take starts
//! (the device watch calls [`stop_for_take`]). The audio itself is dropped as
//! it arrives; only the level leaves the sink.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use handy_recorder::{AudioChunk, Recorder, RecorderConfig, Sink};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Runtime};
use tauri_specta::Event;

use super::dynamics::{rms_db, Dynamics};

/// The meter reads the RMS of this much recent audio.
const RMS_WINDOW_MS: usize = 30;

/// Level events per second.
pub const EMIT_HZ: u32 = 30;
/// A session stops by itself after this long.
pub const MAX_SESSION: Duration = Duration::from_secs(60);

/// A level reading of the monitored mic, or the end of a session.
#[derive(Clone, Debug, Serialize, Deserialize, Type, Event, PartialEq)]
pub struct ForkMicLevel {
    /// Session number (from `fork_mic_monitor_start`).
    pub session: u32,
    /// `false` once: the session ended.
    pub running: bool,
    /// 0..1: the level above the room's noise floor, on a dB scale, with a
    /// fast attack and a smooth release ([`super::dynamics`]). Steady room
    /// noise reads near 0; ordinary speech fills most of the meter.
    pub level: f32,
    /// Why the session could not start or ended early (`permission_denied`,
    /// `no_input_device`, `device_lost`, `failed`).
    pub error: Option<String>,
}

/// The sink: folds every chunk into the meter's [`Dynamics`] (timed by the
/// audio itself) and emits the highest level since the last event, at most
/// `EMIT_HZ` times a second.
struct LevelSink<F: FnMut(f32) + Send + 'static> {
    dynamics: Dynamics,
    /// `(frames, sum of squares)` of the last chunks, about [`RMS_WINDOW_MS`].
    recent: std::collections::VecDeque<(usize, f32)>,
    loudest: f32,
    last_emit: Option<Instant>,
    emit: F,
}

impl<F: FnMut(f32) + Send + 'static> LevelSink<F> {
    fn new(emit: F) -> Self {
        Self {
            dynamics: Dynamics::meter(),
            recent: std::collections::VecDeque::new(),
            loudest: 0.0,
            last_emit: None,
            emit,
        }
    }
}

impl<F: FnMut(f32) + Send + 'static> Sink for LevelSink<F> {
    fn process_chunk(&mut self, chunk: AudioChunk<'_>) {
        let real = chunk.valid_frames * chunk.channels as usize;
        let samples = &chunk.samples[..real];
        // RMS over the last ~30 ms: steadier on noise than one 10 ms chunk.
        self.recent
            .push_back((samples.len(), samples.iter().map(|s| s * s).sum()));
        let window = (chunk.sample_rate as usize * chunk.channels as usize) * RMS_WINDOW_MS / 1000;
        while self.recent.len() > 1
            && self.recent.iter().map(|(n, _)| n).sum::<usize>() - self.recent[0].0 >= window
        {
            self.recent.pop_front();
        }
        let (n, sum) = self
            .recent
            .iter()
            .fold((0usize, 0.0f32), |(n, s), (m, t)| (n + m, s + t));
        let db = if n == 0 || sum <= 0.0 {
            rms_db(&[])
        } else {
            (10.0 * (sum / n as f32).log10()).max(rms_db(&[]))
        };
        let dt_ms = chunk.valid_frames as f32 * 1000.0 / chunk.sample_rate.max(1) as f32;
        let level = self.dynamics.push(db, dt_ms);
        self.loudest = self.loudest.max(level);
        let due = self
            .last_emit
            .is_none_or(|t| t.elapsed() >= Duration::from_millis(1000 / EMIT_HZ as u64));
        if due {
            let shown = self.loudest;
            log::trace!(
                "fork audio: meter at {:.2} s: {db:.1} dB, floor {:.1} dB, level {shown:.2}",
                self.dynamics.elapsed_ms() / 1000.0,
                self.dynamics.floor()
            );
            self.loudest = 0.0;
            self.last_emit = Some(Instant::now());
            (self.emit)(shown);
        }
    }
}

struct Session {
    id: u32,
    stop: Arc<AtomicBool>,
}

static CURRENT: Mutex<Option<Session>> = Mutex::new(None);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn current() -> std::sync::MutexGuard<'static, Option<Session>> {
    CURRENT.lock().unwrap_or_else(|e| e.into_inner())
}

/// Whether a monitor session is open.
pub fn is_running() -> bool {
    current().is_some()
}

/// Starts a session on `device` (a recorder ID; `None` = the system
/// default) and returns its number. A running session is stopped first.
/// Opening happens on the session's thread; a failure arrives as the final
/// `ForkMicLevel` with `error`.
pub fn start<R: Runtime>(app: &AppHandle<R>, device: Option<String>) -> u32 {
    stop();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed) as u32;
    let stop_flag = Arc::new(AtomicBool::new(false));
    *current() = Some(Session {
        id,
        stop: Arc::clone(&stop_flag),
    });
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("wb-mic-monitor".into())
        .spawn(move || {
            let error = run(&app, id, device, &stop_flag);
            {
                let mut cur = current();
                if cur.as_ref().is_some_and(|s| s.id == id) {
                    *cur = None;
                }
            }
            send(
                &app,
                ForkMicLevel {
                    session: id,
                    running: false,
                    level: 0.0,
                    error,
                },
            );
        });
    if let Err(e) = spawned {
        log::warn!("fork audio: cannot start the mic test: {e}");
        *current() = None;
    }
    id
}

fn send<R: Runtime>(app: &AppHandle<R>, event: ForkMicLevel) {
    if let Err(e) = event.emit(app) {
        log::debug!("fork audio: cannot emit ForkMicLevel: {e}");
    }
}

fn error_name(e: &handy_recorder::Error) -> String {
    match e.kind() {
        handy_recorder::ErrorKind::PermissionDenied => "permission_denied",
        handy_recorder::ErrorKind::DeviceUnavailable => "no_input_device",
        handy_recorder::ErrorKind::DeviceLost => "device_lost",
        _ => "failed",
    }
    .into()
}

fn run<R: Runtime>(
    app: &AppHandle<R>,
    id: u32,
    device: Option<String>,
    stop: &AtomicBool,
) -> Option<String> {
    let failed = Arc::new(Mutex::new(None::<String>));
    let config = RecorderConfig {
        device,
        ..RecorderConfig::speech()
    };
    let recorder = {
        let failed = Arc::clone(&failed);
        Recorder::open_with_failure_handler(config, move |e| {
            *failed.lock().unwrap_or_else(|p| p.into_inner()) = Some(error_name(&e));
        })
    };
    let recorder = match recorder {
        Ok(r) => r,
        Err(e) => {
            log::info!("fork audio: mic test could not open the microphone: {e}");
            return Some(error_name(&e));
        }
    };
    log::info!("fork audio: mic test started (session {id})");
    let emitter = app.clone();
    let sink = LevelSink::new(move |level| {
        send(
            &emitter,
            ForkMicLevel {
                session: id,
                running: true,
                level,
                error: None,
            },
        )
    });
    if let Err(e) = recorder.start(sink) {
        let _ = recorder.close();
        return Some(error_name(&e.error));
    }
    let started = Instant::now();
    while !stop.load(Ordering::Acquire) && started.elapsed() < MAX_SESSION {
        if failed.lock().unwrap_or_else(|p| p.into_inner()).is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = recorder.stop();
    let _ = recorder.close();
    log::info!(
        "fork audio: mic test ended (session {id}, {:.1} s)",
        started.elapsed().as_secs_f32()
    );
    let error = failed.lock().unwrap_or_else(|p| p.into_inner()).take();
    error
}

/// Ends the running session, if any.
pub fn stop() {
    if let Some(s) = current().take() {
        s.stop.store(true, Ordering::Release);
    }
}

/// A take started: the meter gives the microphone back.
pub fn stop_for_take() {
    if is_running() {
        log::info!("fork audio: a take started; ending the mic test");
        stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(samples: &[f32]) -> AudioChunk<'_> {
        AudioChunk {
            samples,
            sample_rate: 16_000,
            channels: 1,
            valid_frames: samples.len(),
        }
    }

    fn sink(out: &Arc<Mutex<Vec<f32>>>) -> LevelSink<impl FnMut(f32) + Send + 'static> {
        let o = Arc::clone(out);
        LevelSink::new(move |l| o.lock().unwrap().push(l))
    }

    /// 10 ms of noise at about `db` dBFS (RMS).
    fn noise(db: f32, seed: &mut u32) -> Vec<f32> {
        let amp = 10f32.powf(db / 20.0) * 3f32.sqrt();
        (0..160)
            .map(|_| {
                *seed ^= *seed << 13;
                *seed ^= *seed >> 17;
                *seed ^= *seed << 5;
                amp * ((*seed as f32 / u32::MAX as f32) * 2.0 - 1.0)
            })
            .collect()
    }

    #[test]
    fn the_sink_emits_at_most_the_rate_and_keeps_the_loudest_level() {
        let out = Arc::new(Mutex::new(Vec::new()));
        let mut sink = sink(&out);
        let loud = [0.3f32; 160];
        let quiet = [0.0f32; 160];
        // 100 chunks of 10 ms in well under 33 ms: one emit (the first),
        // and the loud one among them is kept for the next.
        for i in 0..100 {
            sink.process_chunk(chunk(if i == 1 { &loud } else { &quiet }));
        }
        assert_eq!(out.lock().unwrap().len(), 1);
        assert_eq!(out.lock().unwrap()[0], 0.0);
        std::thread::sleep(Duration::from_millis(40));
        sink.process_chunk(chunk(&quiet));
        assert_eq!(out.lock().unwrap().len(), 2);
        assert!(out.lock().unwrap()[1] > 0.1, "{:?}", out.lock().unwrap());
        // A loud chunk between emits is not lost: it shows in the next one.
        sink.process_chunk(chunk(&loud));
        std::thread::sleep(Duration::from_millis(40));
        sink.process_chunk(chunk(&quiet));
        let got = out.lock().unwrap().clone();
        assert_eq!(got.len(), 3);
        assert!(got[2] > 0.1, "{got:?}");
    }

    #[test]
    fn steady_room_noise_reads_near_zero_and_speech_fills_the_meter() {
        let out = Arc::new(Mutex::new(Vec::new()));
        let mut sink = sink(&out);
        let mut seed = 9;
        let mut levels = Vec::new();
        // The levels as the audio produces them (emits are wall-clock).
        let run = |sink: &mut LevelSink<_>, db: f32, ms: u32, seed: &mut u32| {
            let mut v = Vec::new();
            for _ in 0..ms / 10 {
                sink.process_chunk(chunk(&noise(db, seed)));
                v.push(sink.loudest);
                sink.loudest = 0.0;
            }
            v
        };
        levels.extend(run(&mut sink, -30.0, 3000, &mut seed));
        let hum = &levels[150..];
        assert!(hum.iter().all(|l| *l < 0.2), "café hum: {hum:?}");
        let talk = run(&mut sink, -12.0, 400, &mut seed);
        assert!(talk.iter().copied().fold(0.0, f32::max) > 0.85, "{talk:?}");
    }

    #[test]
    fn the_sink_ignores_the_padding_of_the_last_chunk() {
        let out = Arc::new(Mutex::new(Vec::new()));
        let mut sink = sink(&out);
        // Silence in the real frames, loud padding after them: silence.
        let mut s = vec![1.0f32; 160];
        s[..10].fill(0.0);
        sink.process_chunk(AudioChunk {
            samples: &s,
            sample_rate: 16_000,
            channels: 1,
            valid_frames: 10,
        });
        assert_eq!(out.lock().unwrap()[0], 0.0);
    }

    /// `WB_METER_WAV=<16-bit mono WAV>`: plays the file through the meter's
    /// sink in 10 ms chunks and prints the level every 33 ms (what the meter
    /// shows), with the floor. Run by hand for the café check.
    #[test]
    #[ignore = "needs WB_METER_WAV"]
    fn meter_sequence_of_a_wav() {
        let Some(path) = std::env::var_os("WB_METER_WAV") else {
            return;
        };
        let source = crate::fork::audio::test_mic::load(std::path::Path::new(&path)).unwrap();
        let rate = source.sample_rate as usize;
        let step = rate / 100;
        let mut sink = LevelSink::new(|_| {});
        let mut shown = 0.0f32;
        for (i, c) in source.samples.chunks(step).enumerate() {
            sink.process_chunk(AudioChunk {
                samples: c,
                sample_rate: source.sample_rate,
                channels: 1,
                valid_frames: c.len(),
            });
            shown = shown.max(sink.loudest);
            sink.loudest = 0.0;
            if i % 3 == 2 {
                let t = (i + 1) as f32 / 100.0;
                let lit = if shown < 0.04 {
                    0
                } else {
                    (shown * 12.0).ceil() as u32
                };
                println!(
                    "{t:6.2}s level {shown:.2} lit {lit:2} floor {:.1} {}",
                    sink.dynamics.floor(),
                    "#".repeat(lit as usize)
                );
                shown = 0.0;
            }
        }
    }

    #[test]
    fn a_monitor_session_is_not_a_take() {
        // The monitor must never reach Handy's recording manager, the
        // coordinator, history or the bar: it has its own recorder.
        let src = include_str!("monitor.rs");
        let code = src.split("#[cfg(test)]").next().unwrap();
        for forbidden in [
            "AudioRecordingManager",
            "TranscriptionCoordinator",
            "HistoryManager",
            "fork::db",
            "pill::",
            "insert::",
        ] {
            assert!(
                !code.contains(forbidden),
                "monitor.rs must not use {forbidden}"
            );
        }
    }

    #[test]
    fn stop_without_a_session_is_a_no_op() {
        stop();
        stop_for_take();
        assert!(!is_running());
    }
}
