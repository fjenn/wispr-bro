// SPDX-License-Identifier: GPL-3.0-or-later
//! Hub window commands (S-38).
//!
//! - `fork_hub_ready`: the main page's event listeners are up. Events that
//!   waited for a rebuilt Hub are delivered now, and a Hub that started
//!   hidden may be parked (`fork::hub::park`).

use tauri::AppHandle;

use crate::fork::hub::park;
use crate::fork::ForkRuntime;

/// The main window's page is listening.
#[tauri::command]
#[specta::specta]
pub fn fork_hub_ready<R: ForkRuntime>(app: AppHandle<R>) {
    park::hub_ready(&app);
}
