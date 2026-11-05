// SPDX-License-Identifier: GPL-3.0-or-later
//! Update commands: the fork debug panel's (F-02) and S-28's state, Check
//! for updates and Restart now for the Hub footer and About page.

use tauri::AppHandle;

use super::ForkError;
use crate::fork::update::scheduler::{self, ForkUpdatePhase, ForkUpdateState, RestartRefused};
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

/// S-28: the update state the footer, About page and tray show.
#[tauri::command]
#[specta::specta]
pub fn fork_update_state<R: ForkRuntime>(app: AppHandle<R>) -> ForkUpdateState {
    scheduler::state(&app)
}

/// S-28: "Check for updates" / "Retry". Returns at once; progress arrives
/// as `fork-update-state`. Refused while update checks are off.
#[tauri::command]
#[specta::specta]
pub fn fork_update_check_now<R: ForkRuntime>(
    app: AppHandle<R>,
) -> Result<ForkUpdateState, ForkError> {
    let state = scheduler::check_now(&app);
    if state.phase == ForkUpdatePhase::Off {
        return Err(to_fork(UpdateError::Disabled));
    }
    Ok(state)
}

/// S-28: "Restart now": installs the downloaded update and relaunches.
/// Refused mid-take (the relaunch veto) or when nothing is downloaded.
#[tauri::command]
#[specta::specta]
pub fn fork_update_restart_now<R: ForkRuntime>(app: AppHandle<R>) -> Result<(), ForkError> {
    scheduler::restart_now(&app).map_err(|refused| match refused {
        RestartRefused::Busy => ForkError::Internal("a dictation is in progress".into()),
        RestartRefused::NothingReady => ForkError::Internal("no update is downloaded".into()),
    })
}
