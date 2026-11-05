// SPDX-License-Identifier: GPL-3.0-or-later
//! Hotkey commands (S-01): `fork_hotkey_status`, `fork_use_hotkey_profile`;
//! S-02: the shortcut health and the Fix sheet's actions.

use tauri::{AppHandle, Runtime};

use super::ForkError;
use crate::fork::shortcut::health::{self, HealthStatus, Trigger};
use crate::fork::shortcut::{self, HotkeyStatus};
use crate::fork::{i18n, ForkRuntime};

/// System Settings › Privacy & Security › Accessibility.
pub const ACCESSIBILITY_PANE: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

/// The hotkey engine's state, the bindings with layout-aware labels and the
/// bindings that could not be registered. Synchronous, so Tauri runs it on
/// the main thread, where the keyboard layout can be read.
#[tauri::command]
#[specta::specta]
pub fn fork_hotkey_status<R: ForkRuntime>(app: AppHandle<R>) -> Result<HotkeyStatus, ForkError> {
    let locale = i18n::current_locale(R::handy_app_language(&app).as_deref());
    Ok(shortcut::status(&app, locale))
}

/// Switches the dictation bindings to the default fn profile (`fallback`
/// false) or to the Ctrl+Opt fallback for keyboards without fn (true).
#[tauri::command]
#[specta::specta]
pub fn fork_use_hotkey_profile<R: ForkRuntime>(
    app: AppHandle<R>,
    fallback: bool,
) -> Result<HotkeyStatus, ForkError> {
    shortcut::use_profile(&app, fallback).map_err(ForkError::Internal)?;
    fork_hotkey_status(app)
}

/// S-02: whether the shortcut works (both taps), and why not.
#[tauri::command]
#[specta::specta]
pub fn fork_shortcut_health() -> HealthStatus {
    health::status()
}

/// S-02: "Check again". The answer arrives as `ForkShortcutHealthChanged`
/// when it differs; the status returned is the one before the check.
#[tauri::command]
#[specta::specta]
pub fn fork_shortcut_recheck() -> HealthStatus {
    health::request_check(Trigger::User);
    health::status()
}

/// S-02: opens System Settings at Accessibility.
#[tauri::command]
#[specta::specta]
pub fn fork_shortcut_open_accessibility() -> Result<(), ForkError> {
    open_url(ACCESSIBILITY_PANE)
}

fn open_url(url: &str) -> Result<(), ForkError> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("/usr/bin/open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|e| ForkError::Internal(format!("cannot open System Settings: {e}")))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = url;
        Err(ForkError::Internal("macOS only".into()))
    }
}

/// The `tccutil` arguments that reset this app's Accessibility record.
pub fn reset_args(bundle_id: &str) -> [String; 3] {
    ["reset".into(), "Accessibility".into(), bundle_id.into()]
}

/// S-02: "Reset and re-grant" (Yap's recovery for a stale row after a
/// re-signed update): clears this app's Accessibility record with
/// `tccutil`, asks macOS to list the app again (the system prompt), and
/// opens the pane. Only this app's own record is touched.
#[tauri::command]
#[specta::specta]
pub fn fork_shortcut_reset_permission<R: Runtime>(app: AppHandle<R>) -> Result<(), ForkError> {
    let bundle_id = app.config().identifier.clone();
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("/usr/bin/tccutil")
            .args(reset_args(&bundle_id))
            .status()
            .map_err(|e| ForkError::Internal(format!("tccutil: {e}")))?;
        if !status.success() {
            return Err(ForkError::Internal(format!("tccutil exited with {status}")));
        }
        log::info!("fork hotkeys health: Accessibility record reset");
        crate::fork::vendor::axwatch::request();
        health::request_check(Trigger::User);
        open_url(ACCESSIBILITY_PANE)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = bundle_id;
        Err(ForkError::Internal("macOS only".into()))
    }
}

/// S-02 / RS-02 fallback: "Relaunch to finish" when a re-granted tap can
/// only be rebuilt by a new process.
#[tauri::command]
#[specta::specta]
pub fn fork_shortcut_relaunch<R: Runtime>(app: AppHandle<R>) {
    log::info!("fork hotkeys health: relaunch requested from the Fix sheet");
    app.restart();
}
