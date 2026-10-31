// SPDX-License-Identifier: GPL-3.0-or-later
//! Watches the microphones while the app runs (R5 S-08, A-211).
//!
//! One thread, `wb-mic-watch`, reads the HAL device list ([`super::transport`])
//! once a second at rest and five times a second during a take. It never
//! opens a device. It does three things:
//!
//! - **Device list changed** → `ForkMicDevicesChanged`, so an open
//!   microphone list refreshes (a USB mic plugged in, AirPods connecting).
//! - **The take's mic vanished** (unplugged, Bluetooth dropped) → the Flow
//!   Bar says so for the rest of the take. Handy keeps the audio heard
//!   before the loss (RS-11: 10/10), and the take ends as usual, so what was
//!   said still lands.
//! - **At rest, the prepared mic is no longer the one the choice wants** (the
//!   macOS default changed under "Auto-detect", a wireless default went away
//!   so the rule no longer applies, or the prepared device is gone) → Handy's
//!   `update_selected_device` re-prepares on the right one, so the next take
//!   does not start on a stale device. Never during a take, never within
//!   [`PRESS_QUIET_MS`] of a trigger press, at most once per
//!   [`REPREPARE_MIN_GAP_MS`].

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use super::choice;
use super::transport::{self, Snapshot};
use crate::fork::ForkRuntime;
use crate::managers::audio::AudioRecordingManager;

/// Poll period at rest.
pub const IDLE_POLL: Duration = Duration::from_millis(1000);
/// Poll period during a take (a lost mic shows within this).
pub const TAKE_POLL: Duration = Duration::from_millis(200);
/// No re-prepare this soon after a trigger press (a take may be starting).
pub const PRESS_QUIET_MS: u64 = 2_000;
/// At most one re-prepare per this long (a device that flaps).
pub const REPREPARE_MIN_GAP_MS: u64 = 3_000;

/// The microphones changed (plugged, unplugged, default changed). Payload:
/// how many inputs there are now.
#[derive(Clone, Debug, Serialize, Deserialize, Type, Event)]
pub struct ForkMicDevicesChanged {
    pub inputs: u32,
}

/// What a device list means for the list UI: inputs and the default.
pub fn fingerprint(s: &Snapshot) -> (Vec<(String, String)>, Option<String>) {
    (
        s.inputs()
            .map(|d| (d.uid.clone(), d.name.clone()))
            .collect(),
        s.default_input.clone(),
    )
}

/// Whether the device the take opened (`opened`, a UID) is gone.
pub fn lost(s: &Snapshot, opened: Option<&str>) -> bool {
    // An empty snapshot is a failed read, not "every device unplugged".
    !s.devices.is_empty() && opened.is_some_and(|uid| s.find(uid).is_none())
}

/// At rest: the UID the prepared mic should be on, when it differs from
/// `opened`. `selected` is Handy's `selected_microphone`, `clamshell`
/// whether Handy's clamshell mic applies (then Handy decides; only a vanished
/// device triggers a re-prepare).
pub fn wanted_change(
    s: &Snapshot,
    opened: Option<&str>,
    selected: Option<&str>,
    follow_system: bool,
    clamshell: bool,
) -> Option<String> {
    if s.devices.is_empty() {
        return None;
    }
    if lost(s, opened) {
        // Gone: re-prepare on whatever Handy resolves now.
        return Some(
            choice::in_use(s, selected, follow_system)
                .map(|d| d.uid.clone())
                .unwrap_or_default(),
        );
    }
    if selected.is_some() || clamshell {
        return None;
    }
    let want = choice::device_for(s, None, follow_system)?;
    (Some(want.uid.as_str()) != opened).then(|| want.uid.clone())
}

/// Starts the watch thread (product builds, from `fork::on_ready`).
pub fn start<R: ForkRuntime>(app: &AppHandle<R>) {
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("wb-mic-watch".into())
        .spawn(move || run(app));
    if let Err(e) = spawned {
        log::warn!("fork audio: cannot start the microphone watch: {e}");
    }
}

fn run<R: ForkRuntime>(app: AppHandle<R>) {
    let mut last = fingerprint(&transport::snapshot());
    let mut warned_this_take = false;
    let mut last_reprepare: Option<Instant> = None;
    loop {
        let rm = app
            .try_state::<Arc<AudioRecordingManager>>()
            .map(|m| Arc::clone(&m));
        let recording = rm.as_ref().is_some_and(|m| m.is_recording());
        std::thread::sleep(if recording { TAKE_POLL } else { IDLE_POLL });

        let snap = transport::snapshot();
        let now = fingerprint(&snap);
        if now != last {
            log::info!(
                "fork audio: microphones changed ({} input(s), default {})",
                now.0.len(),
                snap.default_input
                    .as_deref()
                    .and_then(|u| snap.find(u))
                    .map_or("none".into(), |d| format!("{:?}", d.transport))
            );
            let event = ForkMicDevicesChanged {
                inputs: now.0.len() as u32,
            };
            if let Err(e) = event.emit(&app) {
                log::debug!("fork audio: cannot emit ForkMicDevicesChanged: {e}");
            }
            last = now;
        }

        let opened = super::opened_uid();
        let recording = rm.as_ref().is_some_and(|m| m.is_recording());
        if recording {
            super::monitor::stop_for_take();
            if !warned_this_take && lost(&snap, opened.as_deref()) {
                warned_this_take = true;
                log::warn!("fork audio: the take's microphone disappeared; keeping what it heard");
                crate::fork::pill::warn_mic_lost();
            }
            continue;
        }
        warned_this_take = false;

        let Some(rm) = rm else { continue };
        if !super::is_prepared() || super::pressed_within(PRESS_QUIET_MS) {
            continue;
        }
        if last_reprepare.is_some_and(|t| t.elapsed() < Duration::from_millis(REPREPARE_MIN_GAP_MS))
        {
            continue;
        }
        let Some(crate::fork::settings::HandySettings(handy)) = R::handy_settings(&app) else {
            continue;
        };
        let want = wanted_change(
            &snap,
            opened.as_deref(),
            handy.selected_microphone.as_deref(),
            super::follows_system(),
            handy.clamshell_microphone.is_some(),
        );
        if want.is_none() {
            continue;
        }
        last_reprepare = Some(Instant::now());
        if rm.is_recording() {
            continue;
        }
        log::info!("fork audio: the prepared microphone is stale; preparing the current one");
        if let Err(e) = rm.update_selected_device() {
            log::warn!("fork audio: re-preparing the microphone failed: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::audio::transport::{CoreDevice, Transport};

    fn dev(uid: &str, t: Transport) -> CoreDevice {
        CoreDevice {
            uid: uid.into(),
            name: uid.into(),
            transport: t,
            has_input: true,
        }
    }

    fn snap(default: &str, devices: &[(&str, Transport)]) -> Snapshot {
        Snapshot {
            devices: devices.iter().map(|(u, t)| dev(u, *t)).collect(),
            default_input: Some(default.into()),
        }
    }

    #[test]
    fn a_vanished_take_mic_is_lost_but_a_failed_read_is_not() {
        let s = snap("built", &[("built", Transport::BuiltIn)]);
        assert!(lost(&s, Some("usb")));
        assert!(!lost(&s, Some("built")));
        assert!(!lost(&s, None));
        assert!(!lost(&Snapshot::default(), Some("usb")));
    }

    #[test]
    fn fingerprint_ignores_outputs_but_sees_the_default() {
        let mut a = snap("built", &[("built", Transport::BuiltIn)]);
        let mut out = dev("speakers", Transport::BuiltIn);
        out.has_input = false;
        let b = Snapshot {
            devices: [a.devices.clone(), vec![out]].concat(),
            ..a.clone()
        };
        assert_eq!(fingerprint(&a), fingerprint(&b));
        a.default_input = Some("usb".into());
        assert_ne!(fingerprint(&a), fingerprint(&b));
    }

    #[test]
    fn auto_detect_follows_a_new_default_at_rest() {
        let s = snap(
            "usb",
            &[("built", Transport::BuiltIn), ("usb", Transport::Usb)],
        );
        assert_eq!(
            wanted_change(&s, Some("built"), None, true, false),
            Some("usb".into())
        );
        assert_eq!(wanted_change(&s, Some("usb"), None, true, false), None);
    }

    #[test]
    fn airpods_becoming_the_default_do_not_move_a_never_chosen_mic() {
        let s = snap(
            "airpods",
            &[
                ("built", Transport::BuiltIn),
                ("airpods", Transport::Bluetooth),
            ],
        );
        assert_eq!(wanted_change(&s, Some("built"), None, false, false), None);
        // Under Auto-detect they do.
        assert_eq!(
            wanted_change(&s, Some("built"), None, true, false),
            Some("airpods".into())
        );
    }

    #[test]
    fn a_named_or_clamshell_choice_only_reprepares_when_its_device_is_gone() {
        let s = snap(
            "usb",
            &[("built", Transport::BuiltIn), ("usb", Transport::Usb)],
        );
        assert_eq!(
            wanted_change(&s, Some("built"), Some("built"), false, false),
            None
        );
        assert_eq!(wanted_change(&s, Some("built"), None, true, true), None);
        let s = snap("built", &[("built", Transport::BuiltIn)]);
        assert_eq!(
            wanted_change(&s, Some("usb"), Some("usb"), false, false),
            Some("built".into())
        );
    }

    #[test]
    fn nothing_changes_on_a_failed_read() {
        assert_eq!(
            wanted_change(&Snapshot::default(), Some("built"), None, true, false),
            None
        );
    }
}
