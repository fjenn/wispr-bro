// SPDX-License-Identifier: GPL-3.0-or-later
//! Microphone commands (S-08): the ordered list with its labels (Handy's
//! `get_available_microphones` stays as it is; this wraps it, R5 S-08), the
//! choice, and the mic test's level meter.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};

use super::ForkError;
use crate::fork::audio::choice::{self, MicEntry, RecorderDevice};
use crate::fork::audio::{self, monitor, transport};
use crate::fork::settings::{self, ForkSettingsHandle, HandySettings};
use crate::fork::ForkRuntime;
use crate::managers::audio::AudioRecordingManager;

/// Reply of [`fork_mic_list`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ForkMicList {
    /// Built-in (recommended), Auto-detect, then the other inputs.
    pub entries: Vec<ForkMicEntry>,
    /// The entry that is chosen: `"auto"` or a device name.
    pub selected: String,
    /// "Auto-detect" was chosen explicitly (it then follows a Bluetooth
    /// default too).
    pub follow_system: bool,
    /// The device takes use now, by name.
    pub in_use: Option<String>,
    /// The macOS default input, by name.
    pub system_default: Option<String>,
    /// The wireless default the rule is skipping (never chosen), by name.
    pub avoided: Option<String>,
}

/// One row (see `fork::audio::choice::MicEntry`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ForkMicEntry {
    pub value: String,
    pub name: String,
    pub kind: choice::MicEntryKind,
    pub recommended: bool,
    pub wireless: bool,
    pub system_default: bool,
    pub transport: transport::Transport,
}

impl From<MicEntry> for ForkMicEntry {
    fn from(e: MicEntry) -> Self {
        ForkMicEntry {
            value: e.value,
            name: e.name,
            kind: e.kind,
            recommended: e.recommended,
            wireless: e.wireless,
            system_default: e.system_default,
            transport: e.transport,
        }
    }
}

pub(crate) fn recorder_devices() -> Result<Vec<RecorderDevice>, ForkError> {
    handy_recorder::list_input_devices()
        .map_err(|e| ForkError::Internal(format!("cannot list the microphones: {e}")))
        .map(|list| {
            list.into_iter()
                .filter(|d| !d.is_monitor)
                .map(|d| RecorderDevice {
                    id: d.id,
                    name: d.name,
                })
                .collect()
        })
}

/// Builds the reply from the device lists and the two settings.
pub fn build_list(
    devices: &[RecorderDevice],
    snapshot: &transport::Snapshot,
    selected: Option<&str>,
    follow_system: bool,
) -> ForkMicList {
    let in_use = choice::in_use(snapshot, selected, follow_system).map(|d| d.name.clone());
    let avoided = (selected.is_none())
        .then(|| choice::avoid_wireless_default(snapshot, follow_system))
        .flatten()
        .and_then(|_| choice::system_default(snapshot))
        .map(|d| d.name.clone());
    ForkMicList {
        entries: choice::list(devices, snapshot)
            .into_iter()
            .map(Into::into)
            .collect(),
        selected: choice::selected_value(selected),
        follow_system,
        in_use,
        system_default: choice::system_default(snapshot).map(|d| d.name.clone()),
        avoided,
    }
}

fn follow_setting<R: ForkRuntime>(app: &AppHandle<R>) -> bool {
    app.try_state::<ForkSettingsHandle>()
        .and_then(|h| h.get(app).ok())
        .is_some_and(|s| s.mic_follow_system)
}

/// The microphone list, in the S-08 order, with what is chosen and in use.
#[tauri::command]
#[specta::specta]
pub async fn fork_mic_list<R: ForkRuntime>(app: AppHandle<R>) -> Result<ForkMicList, ForkError> {
    let HandySettings(handy) = R::handy_settings(&app).ok_or(ForkError::NotReady)?;
    let follow = follow_setting(&app);
    // Device enumeration can stall: off the main thread.
    tokio::task::spawn_blocking(move || {
        let devices = recorder_devices()?;
        let snapshot = transport::snapshot();
        Ok(build_list(
            &devices,
            &snapshot,
            handy.selected_microphone.as_deref(),
            follow,
        ))
    })
    .await
    .map_err(|e| ForkError::Internal(format!("audio task: {e}")))?
}

/// Chooses a microphone on a worker thread. Shared by the command and S-26a's
/// native menus; never call this on the main thread.
pub fn choose<R: ForkRuntime>(app: &AppHandle<R>, value: String) -> Result<ForkMicList, ForkError> {
    let value = value.trim().to_string();
    if value.is_empty() || value.len() > 512 {
        return Err(ForkError::InvalidSetting("microphone".into()));
    }
    let auto = value == choice::AUTO;
    let HandySettings(before) = R::handy_settings(app).ok_or(ForkError::NotReady)?;
    let mut after = before.clone();
    after.selected_microphone = (!auto).then(|| value.clone());
    if after.selected_microphone != before.selected_microphone {
        R::write_handy_settings(
            app,
            &HandySettings(before),
            HandySettings(after),
            &settings::ProductReport {
                enforced: vec![],
                first_run: vec![],
            },
        );
    }
    if let Some(handle) = app.try_state::<ForkSettingsHandle>() {
        handle
            .update(app, |s| {
                s.mic_follow_system = auto;
                Ok(())
            })
            .map_err(ForkError::Internal)?;
    }
    audio::set_follows_system(auto);
    log::info!(
        "fork audio: microphone choice changed ({})",
        if auto { "auto-detect" } else { "a device" }
    );
    if let Some(rm) = app
        .try_state::<Arc<AudioRecordingManager>>()
        .map(|m| Arc::clone(&m))
    {
        if !rm.is_recording() {
            if let Err(error) = rm.update_selected_device() {
                log::warn!("fork audio: re-preparing after the choice failed: {error}");
            }
        }
    }
    let handy = R::handy_settings(app).ok_or(ForkError::NotReady)?.0;
    let devices = recorder_devices()?;
    Ok(build_list(
        &devices,
        &transport::snapshot(),
        handy.selected_microphone.as_deref(),
        follow_setting(app),
    ))
}

/// Chooses a microphone: `"auto"` (Auto-detect, the system setting) or a
/// device name from [`fork_mic_list`]. Writes Handy's `selected_microphone`
/// and the fork's `mic_follow_system`, then re-prepares the microphone on
/// the new device (never during a take: Handy reopens at the next take).
#[tauri::command]
#[specta::specta]
pub async fn fork_mic_choose<R: ForkRuntime>(
    app: AppHandle<R>,
    value: String,
) -> Result<ForkMicList, ForkError> {
    let handle = app.clone();
    let list = tokio::task::spawn_blocking(move || choose(&handle, value))
        .await
        .map_err(|e| ForkError::Internal(format!("audio task: {e}")))?;
    // S-26a: the menus' Microphone ▸ shows the new choice.
    crate::fork::app_menu::refresh(&app, crate::fork::app_menu::Refresh::Devices);
    list
}

/// Starts the mic test's level meter on the microphone takes use now and
/// returns its session number. Levels arrive as `ForkMicLevel` events; the
/// session ends on [`fork_mic_monitor_stop`], after 60 s, or when a take
/// starts. Not a take: no history row, no paste.
#[tauri::command]
#[specta::specta]
pub async fn fork_mic_monitor_start<R: ForkRuntime>(app: AppHandle<R>) -> Result<u32, ForkError> {
    let HandySettings(handy) = R::handy_settings(&app).ok_or(ForkError::NotReady)?;
    let follow = follow_setting(&app);
    let selected = handy.selected_microphone.clone();
    let device = tokio::task::spawn_blocking(move || {
        let snapshot = transport::snapshot();
        choice::in_use(&snapshot, selected.as_deref(), follow)
            .map(|d| transport::recorder_id(&d.uid))
    })
    .await
    .map_err(|e| ForkError::Internal(format!("audio task: {e}")))?;
    Ok(monitor::start(&app, device))
}

/// Ends the mic test's level meter.
#[tauri::command]
#[specta::specta]
pub fn fork_mic_monitor_stop() {
    monitor::stop();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::audio::transport::{CoreDevice, Snapshot, Transport};

    fn snap(default: &str) -> Snapshot {
        Snapshot {
            devices: vec![
                CoreDevice {
                    uid: "built".into(),
                    name: "MacBook Pro Microphone".into(),
                    transport: Transport::BuiltIn,
                    has_input: true,
                },
                CoreDevice {
                    uid: "pods".into(),
                    name: "AirPods Pro".into(),
                    transport: Transport::Bluetooth,
                    has_input: true,
                },
            ],
            default_input: Some(default.into()),
        }
    }

    fn devices() -> Vec<RecorderDevice> {
        ["pods:AirPods Pro", "built:MacBook Pro Microphone"]
            .iter()
            .map(|s| {
                let (uid, name) = s.split_once(':').unwrap();
                RecorderDevice {
                    id: transport::recorder_id(uid),
                    name: name.into(),
                }
            })
            .collect()
    }

    #[test]
    fn never_chosen_with_airpods_as_default_says_the_built_in_mic_is_used() {
        let l = build_list(&devices(), &snap("pods"), None, false);
        assert_eq!(l.selected, "auto");
        assert_eq!(l.in_use.as_deref(), Some("MacBook Pro Microphone"));
        assert_eq!(l.avoided.as_deref(), Some("AirPods Pro"));
        assert_eq!(l.system_default.as_deref(), Some("AirPods Pro"));
        assert_eq!(l.entries[0].value, "MacBook Pro Microphone");
        assert!(l.entries[0].recommended);
        assert_eq!(l.entries[1].value, "auto");
        assert!(l.entries[2].wireless);
    }

    #[test]
    fn auto_detect_chosen_uses_the_airpods_and_avoids_nothing() {
        let l = build_list(&devices(), &snap("pods"), None, true);
        assert_eq!(l.in_use.as_deref(), Some("AirPods Pro"));
        assert_eq!(l.avoided, None);
        assert!(l.follow_system);
    }

    #[test]
    fn a_named_device_avoids_nothing() {
        let l = build_list(&devices(), &snap("pods"), Some("AirPods Pro"), false);
        assert_eq!(l.selected, "AirPods Pro");
        assert_eq!(l.in_use.as_deref(), Some("AirPods Pro"));
        assert_eq!(l.avoided, None);
    }
}
