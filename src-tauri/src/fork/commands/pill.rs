// SPDX-License-Identifier: GPL-3.0-or-later
//! Flow Bar commands (S-03), called by the `fork-pill` web view.
//!
//! - `fork_pill_snapshot`: the state to show, when the page loads (later
//!   changes arrive as `ForkPillState` events).
//! - `fork_pill_ready`: the page has painted state `seq`; the bar's window
//!   shows (or resizes) only then, so no blank or stale frame is visible.
//! - `fork_pill_action`: ✓, ✕, Dismiss and Fix.

use tauri::AppHandle;

use crate::fork::pill::{self, ForkPillState, PillUiAction};
use crate::fork::ForkRuntime;

/// The state the bar should show now (`None` at rest).
#[tauri::command]
#[specta::specta]
pub fn fork_pill_snapshot<R: ForkRuntime>(_app: AppHandle<R>) -> Option<ForkPillState> {
    pill::snapshot()
}

/// The page painted `seq` (after two animation frames).
#[tauri::command]
#[specta::specta]
pub fn fork_pill_ready<R: ForkRuntime>(_app: AppHandle<R>, seq: u32) {
    pill::ready(seq);
}

/// A button on the bar.
#[tauri::command]
#[specta::specta]
pub fn fork_pill_action<R: ForkRuntime>(_app: AppHandle<R>, action: PillUiAction) {
    pill::ui_action(action);
}
