// SPDX-License-Identifier: GPL-3.0-or-later
//! S-24 Scratchpad: durable notes, the floating window and its open shortcut.

pub mod store;
pub mod window;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Runtime};

pub const OPEN_ACTION: &str = "scratchpad.open";
pub const WINDOW_LABEL: &str = "fork-scratchpad";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkScratchpadChanged {
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkScratchpadOpen {
    pub note_id: i64,
}

pub fn changed<R: Runtime>(app: &AppHandle<R>) {
    use tauri_specta::Event;
    let event = ForkScratchpadChanged {
        revision: crate::fork::context::now_ms().max(0) as u64,
    };
    let _ = event.emit(app);
}

/// Registered before the fork tap compiles its profile. With no binding the
/// action remains unset; the page proposes Ctrl+Option+S only on confirmation.
pub fn on_ready<R: crate::fork::ForkRuntime + 'static>(app: &AppHandle<R>) {
    use std::sync::Arc;
    let app = app.clone();
    if let Err(error) = crate::fork::shortcut::registry().register(
        OPEN_ACTION,
        Arc::new(move || {
            if let Err(error) = window::open_shortcut(&app) {
                log::warn!("scratchpad: shortcut could not open a note: {error}");
            }
        }),
    ) {
        log::warn!("scratchpad: could not register its open action: {error}");
    }
}
