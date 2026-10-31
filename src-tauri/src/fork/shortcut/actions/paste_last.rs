// SPDX-License-Identifier: GPL-3.0-or-later
//! `paste_last` (⌃⌘V, R2 A-26): pastes the last transcript again into the
//! frontmost app, through S-04's delivery (the clipboard is restored, the
//! hotkey's modifiers are waited for, the field is checked by S-05's target
//! rule). Nothing to paste: the bar says so.
//!
//! The text is the one History shows ([`crate::fork::recovery::last`]).
//! It goes in as is: no text stages again, no auto-submit, and History
//! keeps the take's own paste outcome.

use tauri::AppHandle;

use crate::fork::insert::{self, Kind};
use crate::fork::recovery::last;

/// The registered handler (runs on the hotkey engine thread: only spawns).
pub fn run(app: &AppHandle) {
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("wb-paste-last".into())
        .spawn(move || paste(app));
    if let Err(e) = spawned {
        log::error!("recovery: cannot start paste-last: {e}");
    }
}

fn paste(app: AppHandle) {
    let Some(entry) = last::find(&app) else {
        log::info!("recovery: paste-last: no transcript yet");
        crate::fork::recovery::show_nothing_yet();
        return;
    };
    log::info!(
        "recovery: paste-last of entry {} ({} chars)",
        entry.entry_id,
        entry.text.chars().count()
    );
    let (result, outcome) = insert::deliver_as(entry.text, app, Kind::Again);
    log::info!(
        "recovery: paste-last {}",
        match (&result, outcome) {
            (_, Some(o)) => o.as_str(),
            (Ok(()), None) => "cancelled",
            (Err(_), None) => "failed",
        }
    );
}
