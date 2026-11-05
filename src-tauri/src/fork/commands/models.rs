// SPDX-License-Identifier: GPL-3.0-or-later
//! Speech model commands: the first-run setup (S-07a, `fork::models::setup`)
//! and the silent policy's Models panel (S-07b, `fork::models::upgrade`).

use tauri::{AppHandle, Manager, Runtime};

use crate::fork::models::setup::{self, ForkSpeechSetup};
use crate::fork::models::upgrade::{self, ForkModelPolicy};

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

/// The user's languages may have changed (onboarding Welcome, General):
/// re-pick the first model if setup is still running, and the best model
/// once it is done (S-07b).
#[tauri::command]
#[specta::specta]
pub fn fork_speech_setup_refresh() {
    setup::refresh();
}

/// Settings › Advanced › Models: what the policy chose and what it is doing
/// (also sent as `fork-model-policy`).
#[tauri::command]
#[specta::specta]
pub fn fork_model_policy_status() -> ForkModelPolicy {
    upgrade::status()
}

/// "Choose automatically" on the Models panel. Off pins the current model.
#[tauri::command]
#[specta::specta]
pub fn fork_model_policy_set_automatic(on: bool) -> ForkModelPolicy {
    setup::set_automatic(on);
    upgrade::status()
}

/// Handy model ids the Models panel hides: retired by the policy, or
/// restricted by the licence filter (DEC-27), unless downloaded.
#[tauri::command]
#[specta::specta]
pub fn fork_models_hidden<R: Runtime>(app: AppHandle<R>) -> Vec<String> {
    app.try_state::<std::sync::Arc<crate::managers::model::ModelManager>>()
        .map(|mm| upgrade::hidden_ids(mm.get_available_models()))
        .unwrap_or_default()
}
