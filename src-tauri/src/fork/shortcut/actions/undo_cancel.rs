// SPDX-License-Identifier: GPL-3.0-or-later
//! `undo_cancel`: brings back the take just cancelled, while the bar shows
//! "Cancelled · Undo" ([`crate::fork::recovery::UNDO_WINDOW_MS`]). The
//! bar's Undo button does the same; this binding is for the keyboard and is
//! unset by default. Outside the window it does nothing.

/// The registered handler (runs on the hotkey engine thread: never waits).
pub fn run() {
    if !crate::fork::recovery::undo("hotkey") {
        log::debug!("recovery: undo hotkey with nothing to undo");
    }
}
