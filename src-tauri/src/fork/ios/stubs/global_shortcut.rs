// SPDX-License-Identifier: GPL-3.0-or-later
//
//! iOS stand-in for Handy's `shortcut/tauri_impl.rs` (path-swapped in
//! `shortcut/mod.rs`): tauri-plugin-global-shortcut is desktop-only.

use tauri::AppHandle;

use crate::settings::ShortcutBinding;

const UNAVAILABLE: &str = "Keyboard shortcuts are not available on iOS";

pub fn init_shortcuts(_app: &AppHandle) {}

pub fn validate_shortcut(_raw: &str) -> Result<(), String> {
    Err(UNAVAILABLE.into())
}

pub fn register_shortcut(_app: &AppHandle, _binding: ShortcutBinding) -> Result<(), String> {
    Err(UNAVAILABLE.into())
}

pub fn unregister_shortcut(_app: &AppHandle, _binding: ShortcutBinding) -> Result<(), String> {
    Ok(())
}
