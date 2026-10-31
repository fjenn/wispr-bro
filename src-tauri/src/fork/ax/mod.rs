// SPDX-License-Identifier: GPL-3.0-or-later
//! The Accessibility (AX) layer (R5 §4.6, W-05; S-05). Knows where a take's
//! text would land: the frontmost app, its focused element, whether that is
//! a place to type (and not a password field), and whether it is still the
//! element that had focus at key-up.
//!
//! - [`editable`]: the editable verdict and fail-open rule, pure (OpenWhispr
//!   port, MIT).
//! - [`lease`]: the `AXManualAccessibility` lease table, pure (VoiceInk
//!   port, GPL-3.0).
//! - `element` (macOS): AX call wrappers and app helpers (SpeakoFlow port,
//!   MIT).
//! - `markers` (macOS): selected text through text markers (VoiceInk port,
//!   GPL-3.0; the selection chain is S-21's, kept here with its layer).
//! - `thread` (macOS): the one AX thread; every call runs there.
//! - `focus` (macOS): the facts the verdict needs, and target capture.
//!
//! The RS-04 spike (`crates/wb-spike-ax`) compiles `editable`, `element`,
//! `lease` and `markers` through `#[path]`, so an RS-04 run scores the
//! shipped verdict.
//!
//! Rules (RS-04, 2026-10-30): the frontmost app comes from NSWorkspace,
//! never from system-wide `AXFocusedApplication`; only controls and "no
//! window" get the card, containers and web areas fail open; Secure Input
//! decides only when the element cannot; never `AXEnhancedUserInterface`;
//! no text is read from a secure field. Every caller tolerates "unknown"
//! (`None` here means paste anyway).

pub mod editable;
pub mod lease;

#[cfg(target_os = "macos")]
pub mod element;
#[cfg(target_os = "macos")]
mod focus;
#[cfg(target_os = "macos")]
pub mod markers;
#[cfg(target_os = "macos")]
pub mod thread;

use std::time::Duration;

pub use editable::{Facts, Family, Verdict};

/// How long a delivery waits for the AX read before it pastes anyway
/// (fail-open). RS-04: a verdict-sized read is a few ms; a hung app is cut
/// by the 0.2 s call timeout.
pub const READ_BUDGET: Duration = Duration::from_millis(300);

/// The focused element as the delivery sees it (no text).
#[derive(Clone, Debug, PartialEq)]
pub struct Focus {
    pub pid: Option<i32>,
    /// Lower-case bundle name (`google chrome`), for the log.
    pub app: String,
    pub family: Family,
    pub facts: Facts,
    pub verdict: Verdict,
    /// Compared with a captured target: `Some(true)` the same element,
    /// `Some(false)` another one (or another app), `None` cannot tell (no
    /// element on one side, or the capture is gone).
    pub same_as_captured: Option<bool>,
    /// How long the read took on the AX thread.
    pub ms: f64,
}

/// A take started: capture the focused element as target `id`, and switch
/// a Chromium/Electron app's tree on so it is built by key-up. Never waits.
pub fn on_take_started(id: u64) {
    #[cfg(target_os = "macos")]
    thread::post(move |ctx| {
        let (pid, _) = focus::capture(ctx, id);
        if let Some(pid) = pid.filter(|p| focus::builds_tree_on_request(*p)) {
            if ctx.lease_acquire(pid) {
                ctx.lease_by_take.insert(id, pid);
            }
        }
    });
    #[cfg(not(target_os = "macos"))]
    let _ = id;
}

/// Key-up (the recording stopped): capture the insertion target `id`.
/// Queued on the AX thread before any delivery read, so it always runs
/// first. Never waits.
pub fn capture(id: u64) {
    #[cfg(target_os = "macos")]
    thread::post(move |ctx| {
        let (pid, element) = focus::capture(ctx, id);
        log::debug!(
            "ax: target {id} captured (pid {}, element {element})",
            pid.unwrap_or(0)
        );
    });
    #[cfg(not(target_os = "macos"))]
    let _ = id;
}

/// The take that started with capture `start_id` is over (delivered or
/// cancelled): its lease reader goes (the lease itself stays 60 s).
pub fn on_take_done(start_id: u64) {
    #[cfg(target_os = "macos")]
    thread::post(move |ctx| {
        if let Some(pid) = ctx.lease_by_take.remove(&start_id) {
            ctx.lease_release(pid);
        }
    });
    #[cfg(not(target_os = "macos"))]
    let _ = start_id;
}

/// The focused element now, compared with capture `captured`. `None` when
/// AX did not answer within `budget` (or on another platform): paste anyway.
pub fn focus_now(captured: Option<u64>, budget: Duration) -> Option<Focus> {
    #[cfg(target_os = "macos")]
    {
        if !accessibility_trusted() {
            return None;
        }
        thread::run(budget, move |ctx| focus::read(ctx, captured))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (captured, budget);
        None
    }
}

/// App exit: every app whose tree we switched on is switched back.
pub fn on_exit() {
    #[cfg(target_os = "macos")]
    thread::shutdown();
}

#[cfg(target_os = "macos")]
pub(crate) fn accessibility_trusted() -> bool {
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> u8;
    }
    // SAFETY: no arguments; reads the TCC state.
    unsafe { AXIsProcessTrusted() != 0 }
}
