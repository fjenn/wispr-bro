// SPDX-License-Identifier: GPL-3.0-or-later
//
//! iOS stand-in for `fork/app_menu.rs` (path-swapped in `fork/mod.rs`): an
//! iPhone app has no menu bar or tray menu. The event type stays so the
//! generated bindings are the same on every platform.

use serde::Serialize;
use specta::Type;
use tauri::AppHandle;

use super::ForkRuntime;

/// What a refresh re-reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refresh {
    Data,
    Devices,
    Force,
}

pub fn refresh<R: ForkRuntime>(_app: &AppHandle<R>, _kind: Refresh) {}

pub fn request_refresh(_kind: Refresh) {}

/// A Hub deep link requested by a native menu: `navigate(page, section)`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type, tauri_specta::Event)]
pub struct ForkMenuAction {
    pub page: String,
    pub section: Option<String>,
}

pub fn on_ready<R: ForkRuntime>(_app: &AppHandle<R>) {}
