// SPDX-License-Identifier: GPL-3.0-or-later
//
//! iOS stand-in for Handy's `tray.rs` (`#[cfg_attr(target_os = "ios", path)]`
//! on `mod tray` in `lib.rs`). An iPhone has no menu bar icon; what the rest
//! of the app reads from the tray survives: the take state behind
//! [`dictation_in_progress`]. The names and signatures follow `tray.rs`.

use std::sync::Mutex;

use tauri::{AppHandle, Manager};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayIconState {
    Idle,
    Recording,
    Transcribing,
}

/// Tauri managed state: the take state every start, finish and cancel sets.
pub struct TrayState(Mutex<TrayIconState>);

impl TrayState {
    pub fn new() -> Self {
        Self(Mutex::new(TrayIconState::Idle))
    }
}

impl Default for TrayState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn set_tray_state(app: &AppHandle, state: TrayIconState) {
    if let Some(tray) = app.try_state::<TrayState>() {
        *tray.0.lock().unwrap_or_else(|p| p.into_inner()) = state;
    }
}

/// Whether a dictation is recording, transcribing or delivering.
pub fn dictation_in_progress(app: &AppHandle) -> bool {
    app.try_state::<TrayState>().is_some_and(|tray| {
        *tray.0.lock().unwrap_or_else(|p| p.into_inner()) != TrayIconState::Idle
    })
}

pub fn refresh_tray_icon(_app: &AppHandle) {}

pub fn update_tray_menu(_app: &AppHandle) {}

pub fn set_tray_visibility(_app: &AppHandle, _visible: bool) {}
