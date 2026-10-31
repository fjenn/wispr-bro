// SPDX-License-Identifier: GPL-3.0-or-later
//! AI engine commands (S-10): status, "Try it", retry a failed download;
//! S-12a: the last dictation's cleanup result; S-12b: Apple's model.

use tauri::{AppHandle, Runtime};

use super::ForkError;
use crate::fork::llm::runtime::{self, ForkLlmError, ForkLlmStatus, ForkLlmTryResult};

/// Longest sample "Try it" accepts (the Settings box is a sentence or two).
pub const TRY_MAX_CHARS: usize = 2_000;

/// The AI engine's state (model download, server).
#[tauri::command]
#[specta::specta]
pub fn fork_llm_status<R: Runtime>(app: AppHandle<R>) -> Result<ForkLlmStatus, ForkError> {
    runtime::runtime(&app)
        .map(|rt| rt.status())
        .ok_or(ForkError::NotReady)
}

/// Cleans `text` the way a dictation would be cleaned (the user's level, or
/// Light) and returns the result. Starts the server if it is stopped.
#[tauri::command]
#[specta::specta]
pub async fn fork_llm_try<R: Runtime>(
    app: AppHandle<R>,
    text: String,
) -> Result<ForkLlmTryResult, ForkLlmError> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > TRY_MAX_CHARS {
        return Err(ForkLlmError::Failed(
            "text must be 1-2000 characters".into(),
        ));
    }
    runtime::try_cleanup(&app, text).await
}

/// Retries a failed model download.
#[tauri::command]
#[specta::specta]
pub fn fork_llm_retry<R: Runtime>(app: AppHandle<R>) -> Result<ForkLlmStatus, ForkError> {
    runtime::retry(&app);
    fork_llm_status(app)
}

/// The last dictation's cleanup result since launch (also sent as the
/// `ForkCleanupResult` event after each take).
#[tauri::command]
#[specta::specta]
pub fn fork_llm_last_cleanup() -> Option<crate::fork::llm::post_process::ForkCleanupResult> {
    crate::fork::llm::post_process::last_result()
}

/// Apple's on-device model (S-12b): availability, its role (RS-08) and
/// request counts since launch.
#[tauri::command]
#[specta::specta]
pub fn fork_llm_apple_status() -> crate::fork::llm::apple::ForkAppleStatus {
    crate::fork::llm::apple::status()
}
