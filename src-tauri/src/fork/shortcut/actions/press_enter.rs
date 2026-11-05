// SPDX-License-Identifier: GPL-3.0-or-later
//! `press_enter` ("Press Enter", R2 A-28): presses Return in the frontmost
//! app, so a dictated message can be sent without reaching for the
//! keyboard. Unset by default; it can be bound to a key chord or to a
//! mouse button (middle, back, forward: RS-11 showed the fork tap sees
//! them and the binding fires).
//!
//! - handy-keys never blocks mouse events, so the button also does its own
//!   click in the app under the pointer (a middle click in a browser still
//!   opens a link). The settings say so next to the binding and suggest a
//!   side button.
//! - Return itself cannot be the binding ([`is_allowed`]): the tap would
//!   swallow every Return, and the Return we post would fire it again.
//! - The binding's own modifiers are waited for (up to
//!   [`MODIFIER_WAIT`]), so ⌥F13 sends Return, not ⌥Return; Return is
//!   posted with no modifier flags.

use std::time::{Duration, Instant};

use handy_keys::{Hotkey, Key};

/// How long the handler waits for the binding's modifiers to come up.
pub const MODIFIER_WAIT: Duration = Duration::from_millis(1_500);

/// Whether `hotkey` may be the Press Enter binding (not Return or the
/// keypad's Enter, with or without modifiers).
pub fn is_allowed(hotkey: &Hotkey) -> bool {
    !matches!(hotkey.key, Some(Key::Return | Key::KeypadEnter))
}

/// The registered handler (runs on the hotkey engine thread: only spawns).
pub fn run() {
    let spawned = std::thread::Builder::new()
        .name("wb-press-enter".into())
        .spawn(press);
    if let Err(e) = spawned {
        log::error!("recovery: cannot start press-enter: {e}");
    }
}

#[cfg(target_os = "macos")]
fn press() {
    use crate::fork::clipboard::keys;
    let start = Instant::now();
    while keys::flags_now() & keys::HOTKEY_MODIFIERS != 0 {
        if start.elapsed() >= MODIFIER_WAIT {
            log::info!(
                "recovery: press-enter skipped: {} still held",
                keys::modifier_names(keys::flags_now())
            );
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    match keys::post_return(0) {
        Ok(()) => log::info!(
            "recovery: press-enter posted Return (waited {} ms)",
            start.elapsed().as_millis()
        ),
        Err(e) => log::warn!("recovery: press-enter failed: {e}"),
    }
}

#[cfg(not(target_os = "macos"))]
fn press() {
    let _ = Instant::now();
    log::warn!("recovery: press-enter needs macOS");
}
