// SPDX-License-Identifier: GPL-3.0-or-later
//! The fork's own tap (R5 §4.8): handy-keys 0.3.4
//! `KeyboardListener::new_with_blocking` with the dynamic blocking set, on
//! its own thread, next to Handy's `HotkeyManager` (which no longer
//! registers anything the fork owns). Events are timestamped on receipt and
//! forwarded to the engine thread.
//!
//! S-02: a tap can be replaced (the health check rebuilds it after the
//! permission comes back, and after wake). Dropping a [`Tap`] stops its
//! listener within [`STOP_POLL`]; its thread then forwards nothing more,
//! not even [`TapMsg::Ended`], so a replaced tap never reports itself lost.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use handy_keys::{BlockingHotkeys, Error, Hotkey, KeyEvent, KeyboardListener};

/// How often the tap thread looks at its stop flag while no key event
/// arrives (a channel wait inside this process; nothing asks macOS).
pub const STOP_POLL: Duration = Duration::from_millis(500);

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
    stop: Arc<AtomicBool>,
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
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = Arc::clone(&stop);
    std::thread::Builder::new()
        .name("wb-hotkey-tap".into())
        .spawn(move || {
            loop {
                match listener.recv_timeout(STOP_POLL) {
                    Ok(ev) => {
                        if stopping.load(Ordering::SeqCst) {
                            return;
                        }
                        forward(TapMsg::Event {
                            at: Instant::now(),
                            ev,
                        });
                    }
                    Err(Error::Timeout) => {
                        if stopping.load(Ordering::SeqCst) {
                            return;
                        }
                    }
                    Err(_) => break,
                }
            }
            if !stopping.load(Ordering::SeqCst) {
                forward(TapMsg::Ended);
            }
            // `listener` drops here and stops the tap's run loop.
        })
        .map_err(|e| format!("cannot spawn the tap thread: {e}"))?;
    Ok(Tap { blocking, stop })
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

impl Drop for Tap {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Block nothing while the old listener winds down.
        self.set_blocking(HashSet::new());
    }
}
