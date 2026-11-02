// SPDX-License-Identifier: GPL-3.0-or-later
//! Update commands for the fork debug panel (F-02). S-28 builds the product
//! UI (Hub footer, tray) on the same `fork::update` calls.

use tauri::AppHandle;

use super::ForkError;
use crate::fork::update::{self, ForkUpdateCheck, ForkUpdateStatus, UpdateError};
use crate::fork::ForkRuntime;

fn to_fork(e: UpdateError) -> ForkError {
    ForkError::Internal(e.to_string())
}

/// Version, channel, endpoints and whether checks are on.
#[tauri::command]
#[specta::specta]
pub fn fork_update_status<R: ForkRuntime>(app: AppHandle<R>) -> ForkUpdateStatus {
    update::status(&app)
}

/// Checks the user's channel now.
#[tauri::command]
#[specta::specta]
pub async fn fork_update_check<R: ForkRuntime>(
    app: AppHandle<R>,
) -> Result<ForkUpdateCheck, ForkError> {
    update::check(&app).await.map_err(to_fork)
}

/// Downloads, verifies and installs the newest update, then relaunches.
/// Returns `null` when already up to date.
#[tauri::command]
#[specta::specta]
pub async fn fork_update_install<R: ForkRuntime>(
    app: AppHandle<R>,
) -> Result<Option<String>, ForkError> {
    let installed = update::install(&app).await.map_err(to_fork)?;
    if installed.is_some() {
        app.restart();
    }
    Ok(installed)
}
