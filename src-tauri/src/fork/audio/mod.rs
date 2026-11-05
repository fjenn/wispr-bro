// SPDX-License-Identifier: GPL-3.0-or-later
//! The microphone: the prepared mic (DEC-53, S-01), the mic-level tap, and
//! S-08's microphone and sound polish:
//!
//! - [`transport`]: what kind of connection each device uses (HAL);
//! - [`choice`]: the default-mic rule (a wireless default the user never
//!   chose is skipped for the built-in mic) and the list order;
//! - [`watch`]: device changes, a mic lost mid-take, re-preparing a stale mic;
//! - [`monitor`]: the level meter for the mic test (not a take);
//! - [`mute`]: "Mute all audio while dictating" on CoreAudio (AGPL port);
//! - [`permission`]: DEC-55, the take running at quit is saved and offered.
//!
//! The wireless rule runs inside the existing [`recorder_config`] hook: when
//! Handy asks for the system default (`device: None`), the hook may name the
//! built-in mic instead, and it records which device the take opens
//! ([`opened_uid`], [`opened_name`]) for the Flow Bar's label and the watch.
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

pub mod choice;
pub mod condition;
pub mod dynamics;
pub mod monitor;
pub mod mute;
pub mod permission;
#[cfg(test)]
mod quiet_bench;
#[cfg(test)]
pub mod quiet_set;
pub mod speech;
pub mod test_mic;
pub mod transport;
pub mod watch;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use handy_recorder::RecorderConfig;
use tauri::{AppHandle, Manager, Runtime};

use crate::managers::audio::{AudioRecordingManager, MicrophoneMode};

static PREPARED: AtomicBool = AtomicBool::new(false);
static PREPARING: AtomicBool = AtomicBool::new(false);

/// S-08's rules apply (product builds, from [`on_ready`]); off in tests and
/// plain Handy builds, so Handy's own config tests see Handy's config.
static RULES_ON: AtomicBool = AtomicBool::new(false);
/// The fork setting `mic_follow_system` ("Auto-detect" was chosen).
static FOLLOW_SYSTEM: AtomicBool = AtomicBool::new(false);
/// The device the last open used: (UID, name).
static OPENED: Mutex<Option<(String, String)>> = Mutex::new(None);
/// Monotonic ms of the last trigger press.
static LAST_PRESS_MS: AtomicU64 = AtomicU64::new(0);

/// Hook in Handy's `microphone_config`.
pub fn recorder_config(config: RecorderConfig) -> RecorderConfig {
    let config = with_prepared(config, PREPARED.load(Ordering::Acquire));
    if !RULES_ON.load(Ordering::Acquire) {
        return config;
    }
    let snapshot = transport::snapshot();
    let config = with_wireless_rule(config, &snapshot, follows_system());
    let opened = choice::device_for(&snapshot, config.device.as_deref(), follows_system())
        .map(|d| (d.uid.clone(), d.name.clone()));
    if let Some((_, name)) = &opened {
        log::debug!("fork audio: the microphone opens on {name:?}");
    }
    *OPENED.lock().unwrap_or_else(|e| e.into_inner()) = opened;
    config
}

/// The wireless rule on a config: `device: None` (system default) becomes
/// the built-in mic when the default is wireless and the user never chose.
fn with_wireless_rule(
    mut config: RecorderConfig,
    snapshot: &transport::Snapshot,
    follow_system: bool,
) -> RecorderConfig {
    if config.device.is_none() {
        if let Some(built_in) = choice::avoid_wireless_default(snapshot, follow_system) {
            log::info!(
                "fork audio: the system microphone is wireless and none was chosen; using the built-in mic"
            );
            config.device = Some(transport::recorder_id(&built_in.uid));
        }
    }
    config
}

/// Whether "Auto-detect (system setting)" was chosen.
pub fn follows_system() -> bool {
    FOLLOW_SYSTEM.load(Ordering::Acquire)
}

/// Sets the "Auto-detect" flag (from the fork setting).
pub fn set_follows_system(follow: bool) {
    FOLLOW_SYSTEM.store(follow, Ordering::Release);
}

/// UID of the device the microphone last opened on.
pub fn opened_uid() -> Option<String> {
    OPENED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|(uid, _)| uid.clone())
}

/// Name of the device the microphone last opened on (the Flow Bar label).
pub fn opened_name() -> Option<String> {
    OPENED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|(_, name)| name.clone())
        .filter(|n| !n.is_empty())
}

fn monotonic_ms() -> u64 {
    static EPOCH: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    EPOCH
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_millis() as u64
        + 1
}

/// Whether a trigger was pressed within the last `ms`.
pub fn pressed_within(ms: u64) -> bool {
    let last = LAST_PRESS_MS.load(Ordering::Acquire);
    last != 0 && monotonic_ms().saturating_sub(last) < ms
}

/// From `fork::on_ready` (product builds), before [`enable_prepared`]: the
/// rules, the device watch, and the takes saved at the last quit.
pub fn on_ready<R: crate::fork::ForkRuntime>(app: &AppHandle<R>) {
    let follow = app
        .try_state::<crate::fork::settings::ForkSettingsHandle>()
        .and_then(|h| h.get(app).ok())
        .is_some_and(|s| s.mic_follow_system);
    set_follows_system(follow);
    RULES_ON.store(true, Ordering::Release);
    // OT2-B: the Flow Bar reads the speech meter of every delivered chunk.
    speech::install();
    watch::start(app);
    permission::on_ready(app);
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
    LAST_PRESS_MS.store(monotonic_ms(), Ordering::Release);
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
    if is_voice(levels) || speech::voice_now() {
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

    fn snapshot(default: &str) -> transport::Snapshot {
        use transport::{CoreDevice, Transport};
        transport::Snapshot {
            devices: vec![
                CoreDevice {
                    uid: "BuiltInMicrophoneDevice".into(),
                    name: "MacBook Pro Microphone".into(),
                    transport: Transport::BuiltIn,
                    has_input: true,
                },
                CoreDevice {
                    uid: "AA:input".into(),
                    name: "AirPods".into(),
                    transport: Transport::Bluetooth,
                    has_input: true,
                },
            ],
            default_input: Some(default.into()),
        }
    }

    #[test]
    fn the_wireless_rule_names_the_built_in_mic_only_for_the_default() {
        let s = snapshot("AA:input");
        let c = with_wireless_rule(RecorderConfig::speech(), &s, false);
        assert_eq!(
            c.device.as_deref(),
            Some("coreaudio:BuiltInMicrophoneDevice")
        );
        // Everything else is Handy's.
        assert_eq!(
            RecorderConfig { device: None, ..c },
            RecorderConfig::speech()
        );
        // Auto-detect chosen: the headset.
        assert_eq!(
            with_wireless_rule(RecorderConfig::speech(), &s, true).device,
            None
        );
        // A named device is never replaced.
        let named = RecorderConfig {
            device: Some("coreaudio:AA:input".into()),
            ..RecorderConfig::speech()
        };
        assert_eq!(
            with_wireless_rule(named.clone(), &s, false).device,
            named.device
        );
        // A wired default is kept.
        let s = snapshot("BuiltInMicrophoneDevice");
        assert_eq!(
            with_wireless_rule(RecorderConfig::speech(), &s, false).device,
            None
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
