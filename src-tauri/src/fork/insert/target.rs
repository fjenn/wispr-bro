// SPDX-License-Identifier: GPL-3.0-or-later
//! Where a take's text goes (R5 §4.6 target rule, S-05).
//!
//! **The target is the element focused at key-up.** A take captures the
//! focused element when it starts (for the Chromium/Electron lease, and as
//! the target of an auto-stopped take, DEC-32) and again when the recording
//! stops. At delivery the focused element is read once more:
//!
//! 1. AX did not answer (no permission, a hung app, another platform):
//!    paste (fail-open; S-04's verification still runs).
//! 2. Another app, or another element of the same app, has focus than at
//!    key-up: the text goes to the card (`focus_moved`), never into the
//!    wrong place. Started in Slack and released in Notes is fine: the
//!    target is Notes.
//! 3. The editable verdict ([`crate::fork::ax::editable`]): a password
//!    field is refused (`secure_field`, the card hides the text), a control
//!    or no window gets the card (`no_text_field`), anything else pastes
//!    (text fields, terminals, and every "cannot tell" answer: fail-open).
//!
//! The capture-at-key-up pattern follows Talkify / BetterWispr's capture
//! target (behaviour; no code taken). One take is in flight at a time:
//! Handy's coordinator starts no recording while the previous one is still
//! processing, so "the latest stop" is the take being delivered; a
//! delivery that finds no capture (a file transcription, a retry) skips the
//! focus check.
//!
//! [`plan`] is pure (tests: a fake AX provider); the state below is fed by
//! Handy's recording phases (`fork::insert::on_recording_phase`) and the
//! hotkey engine (auto-stops).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use super::deliver::CardReason;
use crate::fork::ax::{editable, Focus};

/// What the delivery does with a take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// Paste. `fail_open`: AX could not say it is a text field.
    Paste { fail_open: bool },
    /// Do not paste; the text goes to the card.
    Card(CardReason),
}

/// The key-up target, as the plan sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    pub id: u64,
    /// The take stopped on its own (silence, 30 min): DEC-32 compares the
    /// field from the take's start.
    pub auto_stop: bool,
}

/// The decision table (R5 S-05 tests). `focus`: the focused element at
/// delivery, compared with the key-up capture when there was one.
pub fn plan(had_target: bool, focus: Option<&Focus>) -> Route {
    let Some(f) = focus else {
        return Route::Paste { fail_open: true };
    };
    // The lock screen (Mac wave 6): its password field reads as a plain
    // window, so the verdict would fail open and paste into it.
    if f.app == LOGIN_WINDOW {
        return Route::Card(CardReason::SecureField);
    }
    if had_target && f.same_as_captured == Some(false) {
        return Route::Card(CardReason::FocusMoved);
    }
    match editable::delivery(f.verdict) {
        editable::Delivery::Paste => Route::Paste { fail_open: false },
        editable::Delivery::PasteFailOpen => Route::Paste { fail_open: true },
        editable::Delivery::Card => Route::Card(CardReason::NoTextField),
        editable::Delivery::Refuse => Route::Card(CardReason::SecureField),
    }
}

/// The lock screen's process (`Focus::app`, the lower-case bundle name).
const LOGIN_WINDOW: &str = "loginwindow";

/// [`plan`] at delivery time: while the screen is locked nothing is pasted
/// (the text goes to the card, hidden like a password field's), whatever AX
/// reports or fails to report.
pub fn plan_now(had_target: bool, focus: Option<&Focus>) -> Route {
    #[cfg(target_os = "macos")]
    if crate::fork::ax::element::screen_locked() {
        return Route::Card(CardReason::SecureField);
    }
    plan(had_target, focus)
}

// ---------------------------------------------------------------------------
// The take's captures

#[derive(Debug, Default)]
struct Takes {
    /// Capture id of the running take's start.
    start: Option<u64>,
    /// The take waiting for delivery: its key-up target and start id.
    pending: Option<(Target, Option<u64>)>,
}

static TAKES: Mutex<Takes> = Mutex::new(Takes {
    start: None,
    pending: None,
});
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
/// The hotkey engine said the next stop is an auto-stop.
static AUTO_STOP: AtomicBool = AtomicBool::new(false);

fn takes() -> std::sync::MutexGuard<'static, Takes> {
    TAKES.lock().unwrap_or_else(|e| e.into_inner())
}

fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

/// Handy's `recording-state-event` `started`.
pub fn on_started() {
    let id = next_id();
    let previous = takes().start.replace(id);
    if let Some(old) = previous {
        crate::fork::ax::on_take_done(old);
    }
    AUTO_STOP.store(false, Ordering::Relaxed);
    crate::fork::ax::on_take_started(id);
}

/// Handy's `recording-state-event` `stopped`: key-up (or ✓, or the CLI).
pub fn on_stopped() {
    let auto_stop = AUTO_STOP.swap(false, Ordering::Relaxed);
    let mut t = takes();
    let start = t.start.take();
    let id = match (auto_stop, start) {
        // DEC-32: an auto-stopped take pastes only into the field captured
        // at its start.
        (true, Some(s)) => s,
        _ => {
            let id = next_id();
            crate::fork::ax::capture(id);
            id
        }
    };
    if let Some((_, Some(old))) = t.pending.replace((Target { id, auto_stop }, start)) {
        // A take that never reached delivery (empty, failed).
        crate::fork::ax::on_take_done(old);
    }
}

/// S-06: captures the element focused now (a cancelled take's field, for
/// Undo) and returns its capture id. Never waits.
pub fn capture_now() -> u64 {
    let id = next_id();
    crate::fork::ax::capture(id);
    id
}

/// Handy's `recording-state-event` `cancelled`.
pub fn on_cancelled() {
    // S-20: a cancelled take's context hints go at once.
    crate::fork::context::awareness::on_take_over();
    let mut t = takes();
    for id in [t.start.take(), t.pending.take().and_then(|(_, s)| s)]
        .into_iter()
        .flatten()
    {
        crate::fork::ax::on_take_done(id);
    }
}

/// The hotkey engine stopped the take on its own (silence or length).
pub fn note_auto_stop() {
    AUTO_STOP.store(true, Ordering::Relaxed);
}

/// The delivery takes its target (once).
pub fn take_pending() -> Option<(Target, Option<u64>)> {
    takes().pending.take()
}

/// The delivered take's lease reader goes.
pub fn done(start: Option<u64>) {
    if let Some(id) = start {
        crate::fork::ax::on_take_done(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::ax::editable::{Facts, Family, Reason, Verdict};

    /// A fake AX provider: the focused element as the AX thread would
    /// report it.
    struct FakeAx {
        app_at_keyup: i32,
        focused: Option<(i32, Facts, Family, bool)>,
    }

    impl FakeAx {
        fn focus(&self) -> Option<Focus> {
            let (pid, facts, family, same_element) = self.focused.clone()?;
            // As `ax::focus::compare`: another app is never the same; with
            // no element now, it cannot tell.
            let same_as_captured = if pid != self.app_at_keyup {
                Some(false)
            } else if !facts.has_focused_element {
                None
            } else {
                Some(same_element)
            };
            Some(Focus {
                pid: Some(pid),
                app: "fake".into(),
                family,
                verdict: editable::verdict(&facts, family),
                facts,
                same_as_captured,
                ms: 1.0,
            })
        }
    }

    fn el(role: &str) -> Facts {
        Facts {
            has_focused_window: true,
            has_focused_element: true,
            role: Some(role.into()),
            enabled: Some(true),
            value_settable: Some(matches!(role, "AXTextField" | "AXTextArea")),
            ..Facts::default()
        }
    }

    fn route(app_at_keyup: i32, focused: Option<(i32, Facts, Family, bool)>) -> Route {
        let ax = FakeAx {
            app_at_keyup,
            focused,
        };
        plan(true, ax.focus().as_ref())
    }

    const PASTE: Route = Route::Paste { fail_open: false };
    const FAIL_OPEN: Route = Route::Paste { fail_open: true };

    #[test]
    fn an_editable_field_pastes() {
        assert_eq!(
            route(1, Some((1, el("AXTextArea"), Family::Native, true))),
            PASTE
        );
        assert_eq!(
            route(1, Some((1, el("AXTextField"), Family::Webkit, true))),
            PASTE
        );
    }

    #[test]
    fn a_password_field_is_refused() {
        let mut f = el("AXTextField");
        f.subrole = Some("AXSecureTextField".into());
        assert_eq!(
            route(1, Some((1, f, Family::Chromium, true))),
            Route::Card(CardReason::SecureField)
        );
        // Secure Input held by the focused app, and AX cannot tell.
        let f = Facts {
            secure_input: true,
            has_focused_window: true,
            ..Facts::default()
        };
        assert_eq!(
            route(1, Some((1, f, Family::Electron, true))),
            Route::Card(CardReason::SecureField)
        );
    }

    #[test]
    fn no_window_and_the_desktop_get_the_card() {
        let none = Facts::default();
        assert_eq!(
            route(1, Some((1, none, Family::Native, false))),
            Route::Card(CardReason::NoTextField)
        );
        // Finder: the desktop and the file list (RS-04 / S-05 Finder probe:
        // `AXList` / `AXCollectionList`), or a container.
        let mut list = el("AXList");
        list.subrole = Some("AXCollectionList".into());
        assert_eq!(
            route(1, Some((1, list, Family::Finder, true))),
            Route::Card(CardReason::NoTextField)
        );
        assert_eq!(
            route(1, Some((1, el("AXScrollArea"), Family::Finder, true))),
            Route::Card(CardReason::NoTextField)
        );
        // Finder's rename and search fields paste.
        assert_eq!(
            route(1, Some((1, el("AXTextField"), Family::Finder, true))),
            PASTE
        );
        // A button anywhere.
        assert_eq!(
            route(1, Some((1, el("AXButton"), Family::Native, true))),
            Route::Card(CardReason::NoTextField)
        );
    }

    #[test]
    fn terminals_paste() {
        let mut f = el("AXTextArea");
        f.value_settable = Some(false);
        // Terminal with Secure Keyboard Entry on is still a terminal.
        f.secure_input = true;
        assert_eq!(route(1, Some((1, f, Family::Terminal, true))), PASTE);
    }

    #[test]
    fn electron_with_an_empty_tree_fails_open() {
        let f = Facts {
            has_focused_window: true,
            ..Facts::default()
        };
        assert_eq!(route(1, Some((1, f, Family::Electron, true))), FAIL_OPEN);
        assert_eq!(
            route(1, Some((1, el("AXWebArea"), Family::Electron, true))),
            FAIL_OPEN
        );
        assert_eq!(
            route(1, Some((1, el("AXGroup"), Family::Chromium, true))),
            FAIL_OPEN
        );
        // Gecko's "not a text field" is not trusted.
        assert_eq!(
            route(1, Some((1, el("AXButton"), Family::Gecko, true))),
            FAIL_OPEN
        );
    }

    #[test]
    fn focus_moved_to_another_apps_field_before_key_up_pastes_there() {
        // Started in Slack (pid 1), released in Notes (pid 2): the key-up
        // capture is Notes, and Notes still has focus.
        assert_eq!(
            route(2, Some((2, el("AXTextArea"), Family::Native, true))),
            PASTE
        );
    }

    #[test]
    fn focus_moved_after_key_up_goes_to_the_card() {
        // Another app.
        assert_eq!(
            route(2, Some((3, el("AXTextArea"), Family::Native, true))),
            Route::Card(CardReason::FocusMoved)
        );
        // Another field of the same app.
        assert_eq!(
            route(2, Some((2, el("AXTextField"), Family::Native, false))),
            Route::Card(CardReason::FocusMoved)
        );
        // Focus moved into a password field: the card, not the field.
        let mut f = el("AXTextField");
        f.subrole = Some("AXSecureTextField".into());
        assert_eq!(
            route(2, Some((3, f, Family::Native, true))),
            Route::Card(CardReason::FocusMoved)
        );
    }

    #[test]
    fn cannot_tell_means_paste() {
        // AX did not answer.
        assert_eq!(plan(true, None), FAIL_OPEN);
        // No element on one side (Electron): same app, so paste.
        let f = Focus {
            pid: Some(1),
            app: "cursor".into(),
            family: Family::Electron,
            facts: Facts {
                has_focused_window: true,
                ..Facts::default()
            },
            verdict: Verdict::Unknown(Reason::NoElementWithWindow),
            same_as_captured: None,
            ms: 0.5,
        };
        assert_eq!(plan(true, Some(&f)), FAIL_OPEN);
        // No capture (a file transcription): no focus check.
        let moved = Focus {
            same_as_captured: Some(false),
            verdict: Verdict::Editable(Reason::TextRole),
            ..f
        };
        assert_eq!(plan(false, Some(&moved)), PASTE);
    }

    /// Mac wave 6: the lock screen's field reads as a plain window
    /// (`container_role`, fail-open); its text must never be pasted there.
    #[test]
    fn the_lock_screen_gets_the_hidden_card() {
        let f = Focus {
            pid: Some(1),
            app: "loginwindow".into(),
            family: Family::Native,
            facts: Facts {
                has_focused_window: true,
                ..Facts::default()
            },
            verdict: Verdict::Unknown(Reason::NoElementWithWindow),
            same_as_captured: Some(true),
            ms: 0.5,
        };
        assert_eq!(plan(true, Some(&f)), Route::Card(CardReason::SecureField));
        assert_eq!(plan(false, Some(&f)), Route::Card(CardReason::SecureField));
    }

    #[test]
    fn captures_follow_the_recording_phases() {
        // Process-wide state: this is the only test that touches it.
        on_cancelled();
        let _ = take_pending();
        on_started();
        let start = takes().start;
        assert!(start.is_some());
        on_stopped();
        let (target, s) = take_pending().expect("a key-up target");
        assert_eq!(s, start);
        assert!(!target.auto_stop);
        assert_ne!(Some(target.id), start, "key-up is a capture of its own");
        assert_eq!(take_pending(), None, "taken once");

        // DEC-32: an auto-stop keeps the start capture as the target.
        on_started();
        let start = takes().start;
        note_auto_stop();
        on_stopped();
        let (target, _) = take_pending().unwrap();
        assert!(target.auto_stop);
        assert_eq!(Some(target.id), start);

        // A cancel drops the take.
        on_started();
        on_cancelled();
        on_stopped();
        let (target, s) = take_pending().unwrap();
        assert_eq!(s, None, "a stop after a cancel has no start");
        assert!(!target.auto_stop);
        done(s);
    }
}
