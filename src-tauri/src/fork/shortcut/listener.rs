// SPDX-License-Identifier: GPL-3.0-or-later
//! The fork's own tap (R5 §4.8): handy-keys 0.3.4
//! `KeyboardListener::new_with_blocking` with the dynamic blocking set, on
//! its own thread, next to Handy's `HotkeyManager` (which no longer
//! registers anything the fork owns). Events are timestamped on receipt and
//! forwarded to the engine thread.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use handy_keys::{BlockingHotkeys, Hotkey, KeyEvent, KeyboardListener};

/// What the tap thread sends.
pub enum TapMsg {
    Event {
        at: Instant,
        ev: KeyEvent,
    },
    /// The listener's channel closed (tap removed, permission revoked).
    Ended,
}

/// A running tap. Dropping it stops the listener.
pub struct Tap {
    pub blocking: BlockingHotkeys,
}

/// Starts the tap with an initial blocking set. Fails without
/// Accessibility / Input Monitoring permission.
pub fn start<F>(initial: HashSet<Hotkey>, forward: F) -> Result<Tap, String>
where
    F: Fn(TapMsg) + Send + 'static,
{
    let blocking: BlockingHotkeys = Arc::new(Mutex::new(initial));
    let listener = KeyboardListener::new_with_blocking(Arc::clone(&blocking))
        .map_err(|e| format!("cannot start the fork keyboard tap: {e}"))?;
    std::thread::Builder::new()
        .name("wb-hotkey-tap".into())
        .spawn(move || {
            while let Ok(ev) = listener.recv() {
                forward(TapMsg::Event {
                    at: Instant::now(),
                    ev,
                });
            }
            forward(TapMsg::Ended);
        })
        .map_err(|e| format!("cannot spawn the tap thread: {e}"))?;
    Ok(Tap { blocking })
}

impl Tap {
    /// Replaces the blocking set; the next event is judged against it.
    pub fn set_blocking(&self, set: HashSet<Hotkey>) {
        match self.blocking.lock() {
            Ok(mut s) => *s = set,
            Err(e) => *e.into_inner() = set,
        }
    }
}
