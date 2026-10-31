// SPDX-License-Identifier: GPL-3.0-or-later
//! The prepared microphone (DEC-53, S-01) and the mic-level tap.
//!
//! While idle, the CoreAudio input unit is built and initialised but not
//! started (handy-recorder's `RecorderConfig::pause_when_idle`, vendored in
//! `crates/wb-handy-recorder`), so a take only starts it: fn-down → first
//! audio ≈ `T_show` + 30 ms instead of `T_show` + ~110 ms, and the orange
//! microphone indicator is never lit while idle. How it is wired into Handy:
//!
//! - `managers/audio.rs` `microphone_config` passes its config through
//!   [`recorder_config`] (one-line hook), which sets `pause_when_idle` once
//!   preparing is enabled.
//! - [`enable_prepared`] (from `fork::on_ready`, only when microphone access
//!   is already granted, so launch never triggers the macOS prompt) switches
//!   Handy's recorder to its always-open mode at runtime (not persisted):
//!   the stream is opened now (prepared, stopped) and never lazily closed.
//! - [`ensure_prepared`] runs at every trigger press: if the unit failed
//!   (device unplugged, default input changed), Handy's reopen path builds a
//!   new prepared unit while `T_show` runs.
//! - Fallback: if preparing fails, the recorder goes back to on-demand cold
//!   opens (≈ `T_show` + 110 ms, Handy's behaviour).
//!
//! Handy's 30 s warm stream after a take therefore no longer exists in
//! product builds, and rule 1's warm-stream pre-roll has nothing to read
//! from (R5 S-01 note).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use handy_recorder::RecorderConfig;
use tauri::{AppHandle, Manager, Runtime};

use crate::managers::audio::{AudioRecordingManager, MicrophoneMode};

static PREPARED: AtomicBool = AtomicBool::new(false);
static PREPARING: AtomicBool = AtomicBool::new(false);

/// Hook in Handy's `microphone_config`.
pub fn recorder_config(config: RecorderConfig) -> RecorderConfig {
    with_prepared(config, PREPARED.load(Ordering::Acquire))
}

fn with_prepared(mut config: RecorderConfig, prepared: bool) -> RecorderConfig {
    config.pause_when_idle = prepared;
    config
}

/// Whether takes start a prepared unit.
pub fn is_prepared() -> bool {
    PREPARED.load(Ordering::Acquire)
}

fn manager<R: Runtime>(app: &AppHandle<R>) -> Option<Arc<AudioRecordingManager>> {
    app.try_state::<Arc<AudioRecordingManager>>()
        .map(|m| Arc::clone(&m))
}

/// Turns the prepared mic on (product builds, microphone access granted).
/// Runs the open on its own thread; never blocks the caller.
pub fn enable_prepared<R: Runtime>(app: &AppHandle<R>) {
    if handy_recorder::permission_status() != handy_recorder::Permission::Granted {
        log::info!("fork audio: microphone access not granted yet; takes open the mic cold");
        return;
    }
    let Some(rm) = manager(app) else {
        log::warn!("fork audio: Handy's audio manager is not managed; mic not prepared");
        return;
    };
    std::thread::Builder::new()
        .name("wb-mic-prepare".into())
        .spawn(move || {
            let started = std::time::Instant::now();
            PREPARED.store(true, Ordering::Release);
            match rm.update_mode(MicrophoneMode::AlwaysOn) {
                Ok(()) => log::info!(
                    "fork audio: microphone prepared in {:?} (input unit built, not started)",
                    started.elapsed()
                ),
                Err(e) => {
                    PREPARED.store(false, Ordering::Release);
                    if let Err(e2) = rm.update_mode(MicrophoneMode::OnDemand) {
                        log::warn!("fork audio: back to on-demand failed: {e2}");
                    }
                    log::warn!(
                        "fork audio: cannot prepare the microphone ({e}); takes open it cold"
                    );
                }
            }
        })
        .map(|_| ())
        .unwrap_or_else(|e| log::warn!("fork audio: cannot spawn the prepare thread: {e}"));
}

/// At a trigger press: rebuild a failed prepared unit before `T_show`
/// (Handy's `start_microphone_stream` is a cheap no-op when the unit is
/// healthy, and reopens it when it failed).
pub fn ensure_prepared<R: Runtime>(app: &AppHandle<R>) {
    if !is_prepared() || PREPARING.swap(true, Ordering::AcqRel) {
        return;
    }
    let Some(rm) = manager(app) else {
        PREPARING.store(false, Ordering::Release);
        return;
    };
    let spawned = std::thread::Builder::new()
        .name("wb-mic-ensure".into())
        .spawn(move || {
            if !rm.is_recording() {
                if let Err(e) = rm.start_microphone_stream() {
                    log::warn!("fork audio: re-preparing the microphone failed: {e}");
                }
            }
            PREPARING.store(false, Ordering::Release);
        });
    if spawned.is_err() {
        PREPARING.store(false, Ordering::Release);
    }
}

/// Spectrum level above which a frame counts as speech for the hands-free
/// silence timer (rule 8). Handy's visualiser maps room tone near 0 and
/// dictation to ~0.3-1 after its noise-floor subtraction.
pub const VOICE_LEVEL: f32 = 0.2;

/// Hook in Handy's `overlay::emit_levels`, before its overlay gate: the
/// hands-free silence timer (S-01) and the Flow Bar's waveform (S-03).
pub fn on_levels<R: Runtime>(app: &AppHandle<R>, levels: &[f32]) {
    if is_voice(levels) {
        crate::fork::shortcut::on_voice();
    }
    crate::fork::pill::on_levels(app, levels);
}

pub fn is_voice(levels: &[f32]) -> bool {
    levels.iter().any(|l| *l >= VOICE_LEVEL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorder_config_follows_the_prepared_flag() {
        // The global flag stays off in tests (Handy's own config tests run
        // in parallel and expect Handy's config).
        assert!(!recorder_config(RecorderConfig::speech()).pause_when_idle);
        assert!(!with_prepared(RecorderConfig::speech(), false).pause_when_idle);
        let c = with_prepared(RecorderConfig::speech(), true);
        assert!(c.pause_when_idle);
        // Everything else is Handy's.
        assert_eq!(
            RecorderConfig {
                pause_when_idle: false,
                ..c
            },
            RecorderConfig::speech()
        );
    }

    #[test]
    fn voice_needs_one_band_over_the_threshold() {
        assert!(!is_voice(&[]));
        assert!(!is_voice(&[0.0, 0.05, 0.19]));
        assert!(is_voice(&[0.0, 0.2]));
        assert!(is_voice(&[0.9; 16]));
    }
}
