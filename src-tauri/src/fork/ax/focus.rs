// SPDX-License-Identifier: GPL-3.0-or-later
//! What the product reads about the focused element (R5 §4.6 `focused_target`
//! and `is_editable`): only the facts the editable verdict needs, so a read
//! stays a few milliseconds even in Electron apps (RS-04: a full read there
//! reached p95 ≈ 310-330 ms). Runs on the AX thread only.
//!
//! The lookup is the RS-04 spike's (`probe.rs`, after SpeakoFlow's
//! `smart_spacing/macos.rs`, MIT): the frontmost app from NSWorkspace (RS-04:
//! system-wide `AXFocusedApplication` returned `CannotComplete` on every
//! call on macOS 26), then that app's `AXFocusedUIElement` and
//! `AXFocusedWindow`.
//!
//! Privacy: no text is read here. Secure fields are recognised by their
//! subrole and `AXProtectedContent` before anything else would be read.

use objc2_app_kit::NSWorkspace;
use objc2_application_services::AXUIElement;
use objc2_core_foundation::{CFBoolean, CFNumber, CFRetained};

use super::editable::{self, Facts};
use super::element as el;
use super::thread::{Captured, Ctx};
use super::Focus;

/// NSWorkspace's frontmost application (safe off the main thread; the main
/// thread's run loop keeps it fresh).
pub fn frontmost_pid() -> Option<i32> {
    NSWorkspace::sharedWorkspace()
        .frontmostApplication()
        .map(|a| a.processIdentifier())
        .filter(|p| *p > 0)
}

fn string(e: &AXUIElement, name: &str) -> Option<String> {
    el::attr_string(e, name)
}

fn boolean(e: &AXUIElement, name: &str) -> Option<bool> {
    let v = el::attr(e, name).ok()?;
    v.downcast_ref::<CFBoolean>()
        .map(|b| b.as_bool())
        .or_else(|| {
            v.downcast_ref::<CFNumber>()
                .and_then(|n| n.as_i64())
                .map(|n| n != 0)
        })
}

fn settable(e: &AXUIElement, name: &str) -> Option<bool> {
    el::is_settable(e, name).ok()
}

fn frame(e: &AXUIElement) -> Option<[f64; 4]> {
    el::attr_rect(e).map(|r| [r.origin.x, r.origin.y, r.size.width, r.size.height])
}

fn focused_element(app: &AXUIElement) -> Option<CFRetained<AXUIElement>> {
    el::attr_element(app, "AXFocusedUIElement")
}

/// Captures the focused element of the frontmost app as target `id`. Our
/// own window is captured by its pid only (no AX call into this process).
pub fn capture(ctx: &mut Ctx, id: u64) -> (Option<i32>, bool) {
    let pid = frontmost_pid();
    let element = pid.filter(|p| !super::is_own_pid(*p)).and_then(|p| {
        let app = ctx.app(p);
        focused_element(&app)
    });
    if let Some(e) = element.as_ref() {
        el::set_timeout(e, super::thread::CALL_TIMEOUT_S);
    }
    let role = element.as_ref().and_then(|e| string(e, "AXRole"));
    let frame = element.as_deref().and_then(frame);
    let has = element.is_some();
    ctx.keep_target(Captured {
        id,
        pid,
        element,
        role,
        frame,
    });
    (pid, has)
}

/// Whether `pid` builds its tree only on request (Chromium, Electron).
pub fn builds_tree_on_request(pid: i32) -> bool {
    el::family_of(&el::app_bundle_path(pid).unwrap_or_default()).builds_tree_on_request()
}

/// Reads the facts the verdict needs and compares the element with the
/// captured target `captured`, if given.
pub fn read(ctx: &mut Ctx, captured: Option<u64>) -> Focus {
    let start = std::time::Instant::now();
    let pid = frontmost_pid();
    if let Some(own) = pid.filter(|p| super::is_own_pid(*p)) {
        let mut focus = own_app(own, captured.and_then(|id| ctx.target(id)));
        focus.ms = start.elapsed().as_micros() as f64 / 1000.0;
        return focus;
    }
    let bundle = pid.and_then(el::app_bundle_path).unwrap_or_default();
    let family = el::family_of(&bundle);
    let app_name = el::bundle_name(&bundle);
    // Secure Input counts only when the focused app holds it (or the window
    // server does not say who does): Terminal's Secure Keyboard Entry or a
    // password field in another app must not refuse this paste.
    let secure_input =
        el::secure_input_on() && el::secure_input_pid().is_none_or(|holder| Some(holder) == pid);

    let mut facts = Facts {
        secure_input,
        ..Facts::default()
    };
    let mut same_as_captured = None;
    if let Some(pid) = pid {
        let app = ctx.app(pid);
        let tree_on_before = family.builds_tree_on_request()
            && (ctx.leases.held(pid) || el::attr_bool(&app, "AXManualAccessibility") == Some(true));
        let window = el::attr_element(&app, "AXFocusedWindow");
        let field = focused_element(&app);
        facts.has_focused_window = window.is_some();
        facts.has_focused_element = field.is_some();
        facts.tree_on = tree_on_before;
        if let Some(f) = field.as_ref() {
            el::set_timeout(f, super::thread::CALL_TIMEOUT_S);
            facts.role = string(f, "AXRole");
            if family.builds_tree_on_request() && !facts.tree_on {
                // A web area with children, or any specific role, means the
                // tree is built, whoever built it (the RS-04 spike's rule).
                facts.tree_on = match facts.role.as_deref() {
                    Some("AXWebArea") => !el::children(f).is_empty(),
                    Some(r) => r != "AXGroup" && r != "AXWindow",
                    None => false,
                };
            }
            facts.subrole = string(f, "AXSubrole");
            facts.protected_content = boolean(f, "AXProtectedContent");
            facts.enabled = boolean(f, "AXEnabled");
            facts.editable_attr = boolean(f, "AXEditable");
            facts.value_settable = settable(f, "AXValue");
            facts.selected_text_settable = settable(f, "AXSelectedText");
            facts.has_selected_range = el::attr(f, "AXSelectedTextRange").is_ok();
        }
        if let Some(id) = captured {
            same_as_captured = compare(ctx.target(id), Some(pid), field.as_deref(), &facts);
        }
    }
    let verdict = editable::verdict(&facts, family);
    Focus {
        pid,
        app: app_name,
        family,
        facts,
        verdict,
        same_as_captured,
        ms: start.elapsed().as_micros() as f64 / 1000.0,
    }
}

/// Up to `units` UTF-16 units of the focused field's text just before the
/// caret (or before the selection), for the smart leading space
/// (`fork::insert::spacing`). `Some("")` at the start of a field; `None`
/// when it cannot be told (our own window, no field, a secure field, no
/// selection range, or the app answers neither `AXStringForRange` nor a
/// bounded `AXValue`). The text is never logged or kept.
pub fn before_caret(ctx: &mut Ctx, units: isize) -> Option<String> {
    let pid = frontmost_pid().filter(|p| !super::is_own_pid(*p))?;
    let app = ctx.app(pid);
    let field = focused_element(&app)?;
    el::set_timeout(&field, super::thread::CALL_TIMEOUT_S);
    if string(&field, "AXSubrole").as_deref() == Some("AXSecureTextField")
        || boolean(&field, "AXProtectedContent") == Some(true)
    {
        return None;
    }
    let caret = el::attr_range(&field, "AXSelectedTextRange")?;
    if caret.location < 0 {
        return None;
    }
    if caret.location == 0 {
        return Some(String::new());
    }
    let length = caret.location.min(units);
    let range = objc2_core_foundation::CFRange {
        location: caret.location - length,
        length,
    };
    if let Some(text) = el::string_for_range(&field, range) {
        return Some(String::from_utf16_lossy(&text));
    }
    // No ranged read: a slice of a value of bounded size.
    let value: Vec<u16> = el::attr_string(&field, "AXValue")?.encode_utf16().collect();
    if value.len() > crate::fork::context::caret::MAX_VALUE_UNITS
        || caret.location as usize > value.len()
    {
        return None;
    }
    let hi = caret.location as usize;
    Some(String::from_utf16_lossy(&value[hi - length as usize..hi]))
}

/// Our own window in front: nothing is read (see [`super::is_own_pid`]).
/// The verdict is "cannot tell" (paste, fail-open, as for an app that does
/// not answer); the capture still decides `focus_moved` by pid: a take
/// started in another app and delivered with our window in front goes to
/// the card.
pub(crate) fn own_app(pid: i32, captured: Option<&Captured>) -> Focus {
    let facts = Facts::default();
    Focus {
        pid: Some(pid),
        app: "wispr bro".into(),
        family: super::Family::Native,
        same_as_captured: compare(captured, Some(pid), None, &facts),
        verdict: super::Verdict::Unknown(editable::Reason::OwnApp),
        facts,
        ms: 0.0,
    }
}

/// The same element as the capture: `CFEqual`, or (when an app hands out a
/// new object for the same field) the same role at the same place.
fn compare(
    captured: Option<&Captured>,
    pid: Option<i32>,
    now: Option<&AXUIElement>,
    facts: &Facts,
) -> Option<bool> {
    let c = captured?;
    if c.pid != pid {
        return Some(false);
    }
    let (Some(then), Some(now)) = (c.element.as_deref(), now) else {
        return None;
    };
    if then == now {
        return Some(true);
    }
    let same_place =
        c.role.is_some() && c.role == facts.role && c.frame.is_some() && c.frame == frame(now);
    Some(same_place)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::insert::target::{plan, Route};

    fn captured(pid: i32) -> Captured {
        Captured {
            id: 1,
            pid: Some(pid),
            element: None,
            role: None,
            frame: None,
        }
    }

    /// S-24.1: a take started and delivered with our own window in front
    /// (the Scratchpad note) pastes there, without one AX call into this
    /// process; one started elsewhere still goes to the card.
    #[test]
    fn our_own_window_pastes_fail_open_and_still_detects_focus_moved() {
        let own = std::process::id() as i32;
        assert!(super::super::is_own_pid(own));
        assert!(!super::super::is_own_pid(1));

        let focus = own_app(own, Some(&captured(own)));
        assert_eq!(
            focus.verdict,
            super::super::Verdict::Unknown(editable::Reason::OwnApp)
        );
        assert_eq!(focus.same_as_captured, None);
        assert_eq!(plan(true, Some(&focus)), Route::Paste { fail_open: true });

        let moved = own_app(own, Some(&captured(own + 1)));
        assert_eq!(moved.same_as_captured, Some(false));
        assert_eq!(
            plan(true, Some(&moved)),
            Route::Card(crate::fork::insert::deliver::CardReason::FocusMoved)
        );
    }
}
