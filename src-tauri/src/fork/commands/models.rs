// SPDX-License-Identifier: GPL-3.0-or-later
//! Speech model setup commands (S-07a, `fork::models::setup`).

use crate::fork::models::setup::{self, ForkSpeechSetup};

/// Where the first-run speech setup is (also sent as `fork-speech-setup`).
#[tauri::command]
#[specta::specta]
pub fn fork_speech_setup_status() -> ForkSpeechSetup {
    setup::status()
}

/// Starts a stopped download again ("Couldn't download · Retry").
#[tauri::command]
#[specta::specta]
pub fn fork_speech_setup_retry() {
    setup::retry();
}

/// The user's languages may have changed (onboarding Welcome): re-pick the
/// first model if setup is still running.
#[tauri::command]
#[specta::specta]
pub fn fork_speech_setup_refresh() {
    setup::refresh();
}
