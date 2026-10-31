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

/// Level events per second.
pub const EMIT_HZ: u32 = 20;
/// A session stops by itself after this long.
pub const MAX_SESSION: Duration = Duration::from_secs(60);
/// The meter's floor: quieter than this shows as 0.
pub const FLOOR_DB: f32 = -60.0;

/// A level reading of the monitored mic, or the end of a session.
#[derive(Clone, Debug, Serialize, Deserialize, Type, Event, PartialEq)]
pub struct ForkMicLevel {
    /// Session number (from `fork_mic_monitor_start`).
    pub session: u32,
    /// `false` once: the session ended.
    pub running: bool,
    /// 0..1 on a dB scale from [`FLOOR_DB`] to 0 dBFS, lightly smoothed.
    pub level: f32,
    /// Why the session could not start or ended early (`permission_denied`,
    /// `no_input_device`, `device_lost`, `failed`).
    pub error: Option<String>,
}

/// RMS of `samples` on the meter's scale: `FLOOR_DB`..0 dBFS → 0..1.
pub fn level_of(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let mean_square = samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32;
    let rms = mean_square.sqrt();
    if rms <= 0.0 {
        return 0.0;
    }
    let db = 20.0 * rms.log10();
    ((db - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0)
}

/// Fast attack, slower release, so speech reads as bars that move, not
/// flicker.
#[derive(Clone, Debug, Default)]
pub struct Meter {
    level: f32,
}

impl Meter {
    pub fn push(&mut self, instant: f32) -> f32 {
        let k = if instant > self.level { 0.6 } else { 0.25 };
        self.level += (instant - self.level) * k;
        self.level
    }
}

/// The sink: folds chunks into the meter and emits at most `EMIT_HZ`.
struct LevelSink<F: FnMut(f32) + Send + 'static> {
    meter: Meter,
    loudest: f32,
    last_emit: Option<Instant>,
    emit: F,
}

impl<F: FnMut(f32) + Send + 'static> Sink for LevelSink<F> {
    fn process_chunk(&mut self, chunk: AudioChunk<'_>) {
        let real = chunk.valid_frames * chunk.channels as usize;
        let level = level_of(&chunk.samples[..real]);
        self.loudest = self.loudest.max(level);
        let due = self
            .last_emit
            .is_none_or(|t| t.elapsed() >= Duration::from_millis(1000 / EMIT_HZ as u64));
        if due {
            let shown = self.meter.push(self.loudest);
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
    let sink = LevelSink {
        meter: Meter::default(),
        loudest: 0.0,
        last_emit: None,
        emit: move |level| {
            send(
                &emitter,
                ForkMicLevel {
                    session: id,
                    running: true,
                    level,
                    error: None,
                },
            )
        },
    };
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

    #[test]
    fn levels_map_dbfs_onto_zero_to_one() {
        assert_eq!(level_of(&[]), 0.0);
        assert_eq!(level_of(&[0.0; 160]), 0.0);
        assert!((level_of(&[1.0; 160]) - 1.0).abs() < 1e-6);
        // -20 dBFS → 2/3 of the way up a 60 dB scale.
        let l = level_of(&[0.1; 160]);
        assert!((l - 2.0 / 3.0).abs() < 1e-3, "{l}");
        // Below the floor is silence on the meter.
        assert_eq!(level_of(&[0.0001; 160]), 0.0);
        // A sine at amplitude 0.5 has RMS 0.354 (-9 dBFS).
        let sine: Vec<f32> = (0..1600)
            .map(|i| 0.5 * (i as f32 * std::f32::consts::TAU / 32.0).sin())
            .collect();
        let l = level_of(&sine);
        assert!((l - (60.0 - 9.03) / 60.0).abs() < 0.01, "{l}");
    }

    #[test]
    fn the_meter_rises_fast_and_falls_slowly() {
        let mut m = Meter::default();
        let up = m.push(1.0);
        assert!(up > 0.5);
        let down = m.push(0.0);
        assert!(down > up * 0.7, "{up} → {down}");
    }

    fn chunk(samples: &[f32]) -> AudioChunk<'_> {
        AudioChunk {
            samples,
            sample_rate: 16_000,
            channels: 1,
            valid_frames: samples.len(),
        }
    }

    #[test]
    fn the_sink_emits_at_most_the_rate_and_keeps_the_loudest_chunk() {
        let out = Arc::new(Mutex::new(Vec::new()));
        let o = Arc::clone(&out);
        let mut sink = LevelSink {
            meter: Meter::default(),
            loudest: 0.0,
            last_emit: None,
            emit: move |l| o.lock().unwrap().push(l),
        };
        let loud = [0.5f32; 160];
        let quiet = [0.0f32; 160];
        // 100 chunks of 10 ms in well under 50 ms: one emit (the first).
        for i in 0..100 {
            sink.process_chunk(chunk(if i == 0 { &loud } else { &quiet }));
        }
        assert_eq!(out.lock().unwrap().len(), 1);
        assert!(out.lock().unwrap()[0] > 0.4);
        // A loud chunk between emits is not lost: it shows in the next one.
        sink.process_chunk(chunk(&loud));
        std::thread::sleep(Duration::from_millis(60));
        sink.process_chunk(chunk(&quiet));
        let got = out.lock().unwrap().clone();
        assert_eq!(got.len(), 2);
        assert!(got[1] > 0.4, "{got:?}");
    }

    #[test]
    fn the_sink_ignores_the_padding_of_the_last_chunk() {
        let out = Arc::new(Mutex::new(Vec::new()));
        let o = Arc::clone(&out);
        let mut sink = LevelSink {
            meter: Meter::default(),
            loudest: 0.0,
            last_emit: None,
            emit: move |l| o.lock().unwrap().push(l),
        };
        let mut s = vec![0.0f32; 160];
        s[..10].fill(0.5);
        sink.process_chunk(AudioChunk {
            samples: &s,
            sample_rate: 16_000,
            channels: 1,
            valid_frames: 10,
        });
        assert!(out.lock().unwrap()[0] > 0.4);
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
