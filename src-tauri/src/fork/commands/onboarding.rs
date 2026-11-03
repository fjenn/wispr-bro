// SPDX-License-Identifier: GPL-3.0-or-later
//! S-29a persisted navigation and explicit, user-initiated Mac setup actions.

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use super::ForkError;
use crate::fork::events::ForkSettingsChanged;
use crate::fork::onboarding::{native, state};
use crate::fork::settings::{ForkSettingsHandle, OnboardingState};
use crate::fork::ForkRuntime;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ForkOnboardingLocation {
    AlreadyInApplications,
    MoveNeeded,
    NotBundled,
}

fn settings_changed<R: ForkRuntime>(app: &AppHandle<R>) -> Result<OnboardingState, ForkError> {
    let event = ForkSettingsChanged {
        key: "onboarding".into(),
        settings: app
            .try_state::<ForkSettingsHandle>()
            .ok_or_else(|| ForkError::Internal("fork settings are not managed".into()))?
            .get(app)
            .map_err(ForkError::Internal)?,
    };
    let state = event.settings.onboarding.clone();
    event
        .emit(app)
        .map_err(|e| ForkError::Internal(e.to_string()))?;
    Ok(state)
}

#[tauri::command]
#[specta::specta]
pub fn fork_onboarding_go_to<R: ForkRuntime>(
    app: AppHandle<R>,
    step: String,
) -> Result<OnboardingState, ForkError> {
    let handle = app
        .try_state::<ForkSettingsHandle>()
        .ok_or_else(|| ForkError::Internal("fork settings are not managed".into()))?;
    handle
        .update(&app, |settings| {
            state::go_to(&mut settings.onboarding, &step)
        })
        .map_err(ForkError::InvalidSetting)?;
    // A step id, never user text (the Mac wave times the flow from these).
    log::info!("onboarding: step {step}");
    settings_changed(&app)
}

#[tauri::command]
#[specta::specta]
pub fn fork_onboarding_complete<R: ForkRuntime>(
    app: AppHandle<R>,
) -> Result<OnboardingState, ForkError> {
    let now = crate::fork::context::now_ms();
    let handle = app
        .try_state::<ForkSettingsHandle>()
        .ok_or_else(|| ForkError::Internal("fork settings are not managed".into()))?;
    handle
        .update(&app, |settings| {
            state::complete(&mut settings.onboarding, now);
            Ok(())
        })
        .map_err(ForkError::Internal)?;
    log::info!("onboarding: completed");
    settings_changed(&app)
}

#[tauri::command]
#[specta::specta]
pub fn fork_onboarding_app_location() -> Result<ForkOnboardingLocation, ForkError> {
    native::app_location().map_err(ForkError::Internal)
}

/// Moves only when the user pressed the button. Never overwrites another
/// app. On success the app reopens from the new place and this one quits.
#[tauri::command]
#[specta::specta]
pub fn fork_onboarding_move_to_applications<R: ForkRuntime>(
    app: AppHandle<R>,
) -> Result<(), ForkError> {
    native::move_to_applications(&app)
        .map(|_| ())
        .map_err(ForkError::Internal)
}

/// The shortcut test (S-29a). `on`: trigger presses light the on-screen key
/// (`fork-hotkey-event` `key_test`) and start no take, for two minutes unless
/// renewed; the tap is started first if it could not start at launch (the
/// user granted Accessibility a moment ago). `off` ends the test at once.
#[tauri::command]
#[specta::specta]
pub fn fork_onboarding_key_test<R: ForkRuntime>(
    app: AppHandle<R>,
    on: bool,
) -> Result<crate::fork::shortcut::HotkeyStatus, ForkError> {
    if on {
        crate::fork::shortcut::retry_tap(&app);
    }
    crate::fork::shortcut::set_key_test(on);
    super::hotkeys::fork_hotkey_status(app)
}

/// DEC-44: called only after the screen explained the change and the user
/// pressed the button. The user's own value is kept for the restore
/// (`shortcut::globe`), which runs when fn stops being the trigger.
#[tauri::command]
#[specta::specta]
pub fn fork_onboarding_globe_do_nothing<R: ForkRuntime>(
    app: AppHandle<R>,
) -> Result<crate::fork::shortcut::HotkeyStatus, ForkError> {
    let applied = crate::fork::shortcut::globe::apply(&app).map_err(ForkError::Internal)?;
    log::info!("onboarding: Globe key set to Do Nothing ({applied:?})");
    crate::fork::shortcut::refresh_globe();
    super::hotkeys::fork_hotkey_status(app)
}

/// System Settings panes the diagnoses open (local URL scheme; no host).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ForkSettingsPane {
    /// Privacy & Security › Accessibility (the tap is not live).
    Accessibility,
    /// Keyboard (the "Press 🌐 key to" setting).
    Keyboard,
}

pub fn pane_url(pane: ForkSettingsPane) -> &'static str {
    match pane {
        ForkSettingsPane::Accessibility => crate::fork::notify::ACCESSIBILITY_PANE,
        ForkSettingsPane::Keyboard => {
            "x-apple.systempreferences:com.apple.Keyboard-Settings.extension"
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn fork_onboarding_open_pane<R: ForkRuntime>(
    app: AppHandle<R>,
    pane: ForkSettingsPane,
) -> Result<(), ForkError> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(pane_url(pane), None::<&str>)
        .map_err(|e| ForkError::Internal(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panes_open_only_local_settings_urls() {
        for pane in [ForkSettingsPane::Accessibility, ForkSettingsPane::Keyboard] {
            assert!(pane_url(pane).starts_with("x-apple.systempreferences:"));
        }
    }
}
