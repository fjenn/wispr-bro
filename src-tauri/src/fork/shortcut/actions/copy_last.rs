// SPDX-License-Identifier: GPL-3.0-or-later
//! `copy_last` (⌃⌘C, R2 A-27): puts the last transcript on the clipboard.
//! This is an explicit copy (R5 §4.7): it replaces the clipboard as plain,
//! unmarked text and is never restored, like the card's Copy. A take that
//! was refused for a password field is copied concealed from clipboard
//! managers, as its card's Copy does (S-05).

use tauri::AppHandle;

use crate::fork::recovery::last;

/// The registered handler (runs on the hotkey engine thread: only spawns).
pub fn run(app: &AppHandle) {
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("wb-copy-last".into())
        .spawn(move || copy(app));
    if let Err(e) = spawned {
        log::error!("recovery: cannot start copy-last: {e}");
    }
}

fn copy(app: AppHandle) {
    let Some(entry) = last::find(&app) else {
        log::info!("recovery: copy-last: no transcript yet");
        crate::fork::recovery::show_nothing_yet();
        return;
    };
    match write(&entry.text, entry.concealed) {
        Ok(()) => {
            log::info!(
                "recovery: copy-last of entry {} ({} chars{})",
                entry.entry_id,
                entry.text.chars().count(),
                if entry.concealed { ", concealed" } else { "" }
            );
            crate::fork::recovery::show_copied();
        }
        Err(e) => log::warn!("recovery: copy-last failed: {e}"),
    }
}

#[cfg(target_os = "macos")]
fn write(text: &str, concealed: bool) -> Result<(), String> {
    crate::fork::clipboard::nspb::write_text(text, concealed).map(|_| ())
}

#[cfg(not(target_os = "macos"))]
fn write(_text: &str, _concealed: bool) -> Result<(), String> {
    Err("copy-last needs macOS".into())
}
