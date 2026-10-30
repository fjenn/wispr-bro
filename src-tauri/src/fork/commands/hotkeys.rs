// SPDX-License-Identifier: GPL-3.0-or-later
//! Hotkey commands (S-01): `fork_hotkey_status`, `fork_use_hotkey_profile`.

use tauri::AppHandle;

use super::ForkError;
use crate::fork::shortcut::{self, HotkeyStatus};
use crate::fork::{i18n, ForkRuntime};

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
