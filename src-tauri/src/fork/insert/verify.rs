// SPDX-License-Identifier: GPL-3.0-or-later
//! Paste verification (R5 §4.7 W-07, §5.4): read the focused field before
//! the paste, poll it for ≤ 300 ms afterwards, and call the paste
//! `confirmed` if it changed, `unverified` otherwise.
//!
//! The approach follows TypeWhisper's `insertTextAtAndVerifyChange` and
//! FluidVoice's `PasteVerifier` (behaviour only; no code taken): compare a
//! cheap fingerprint of the field instead of its text. RS-05 measured the
//! paste readable in the field 26-86 ms after ⌘V in native apps, so 300 ms
//! leaves room; a first paste into a freshly opened Chrome tab can take
//! ~1 s and is then reported `unverified`, which only matters for the log
//! (an unverified paste is never retried: it may have landed).
//!
//! The fingerprint never contains the text: the character count, the caret
//! and, only when the app reports neither, the length and a hash of the
//! value. Electron apps usually expose no focused element (RS-04): those
//! pastes are `unverified` (`no_field`).

use serde::Serialize;
use std::time::{Duration, Instant};

/// How long to watch the field after the paste (R5 §4.7).
pub const BUDGET: Duration = Duration::from_millis(300);
/// How often to look.
pub const POLL: Duration = Duration::from_millis(25);

/// What Accessibility says about the focused field, without its text.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct FieldState {
    pub role: Option<String>,
    /// `AXNumberOfCharacters`.
    pub chars: Option<i64>,
    /// `AXSelectedTextRange` location and length.
    pub caret: Option<(i64, i64)>,
    /// Only when neither of the above is reported: `AXValue` length in
    /// UTF-16 units and a hash of it.
    pub value_fingerprint: Option<(usize, u64)>,
}

impl FieldState {
    /// Whether the fingerprint says anything at all.
    pub fn readable(&self) -> bool {
        self.chars.is_some() || self.caret.is_some() || self.value_fingerprint.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Unverified {
    /// No focused element, or one that reports nothing we can compare.
    NoField,
    /// The field did not change within the budget.
    NoChange,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "result")]
pub enum Verification {
    Confirmed { after_ms: f64 },
    Unverified { reason: Unverified },
}

impl Verification {
    pub fn confirmed(&self) -> bool {
        matches!(self, Verification::Confirmed { .. })
    }
}

/// Polls `field` every [`POLL`] for up to `budget` after the paste was sent.
/// `before`: the field just before the paste.
pub fn verify(
    before: Option<&FieldState>,
    budget: Duration,
    now: impl Fn() -> Instant,
    wait: impl Fn(Duration),
    mut field: impl FnMut() -> Option<FieldState>,
) -> Verification {
    let Some(before) = before.filter(|b| b.readable()) else {
        return Verification::Unverified {
            reason: Unverified::NoField,
        };
    };
    let start = now();
    loop {
        if let Some(after) = field() {
            if after.readable() && &after != before {
                let d = now().saturating_duration_since(start);
                return Verification::Confirmed {
                    after_ms: (d.as_secs_f64() * 10_000.0).round() / 10.0,
                };
            }
        }
        if now().saturating_duration_since(start) >= budget {
            return Verification::Unverified {
                reason: Unverified::NoChange,
            };
        }
        wait(POLL);
    }
}

/// FNV-1a over UTF-16 units (stable, no dependency).
pub fn fingerprint(units: impl Iterator<Item = u16>) -> (usize, u64) {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut n = 0;
    for u in units {
        n += 1;
        for b in u.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    (n, h)
}

// ---------------------------------------------------------------------------
// macOS: the frontmost app's focused element

#[cfg(target_os = "macos")]
pub use mac::focused_field;

#[cfg(target_os = "macos")]
mod mac {
    use super::{fingerprint, FieldState};
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;
    use std::ffi::c_void;

    type CFTypeRef = *const c_void;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct CFRange {
        location: isize,
        length: isize,
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(v: CFTypeRef);
        fn CFGetTypeID(v: CFTypeRef) -> usize;
        fn CFStringGetTypeID() -> usize;
        fn CFNumberGetTypeID() -> usize;
        fn CFNumberGetValue(n: CFTypeRef, ty: isize, out: *mut c_void) -> u8;
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> u8;
        fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
        fn AXUIElementCopyAttributeValue(e: CFTypeRef, attr: CFTypeRef, out: *mut CFTypeRef)
            -> i32;
        fn AXUIElementSetMessagingTimeout(e: CFTypeRef, secs: f32) -> i32;
        fn AXValueGetValue(v: CFTypeRef, ty: u32, out: *mut c_void) -> u8;
    }

    /// kAXValueCFRangeType.
    const AX_VALUE_CF_RANGE: u32 = 4;
    /// kCFNumberSInt64Type.
    const CF_NUMBER_SINT64: isize = 4;
    /// Per-call messaging timeout: a hung app must not hold up the paste.
    const AX_TIMEOUT_S: f32 = 0.1;

    struct Cf(CFTypeRef);

    impl Drop for Cf {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: we own one retain (Create/Copy rule).
                unsafe { CFRelease(self.0) };
            }
        }
    }

    fn attr(e: CFTypeRef, name: &str) -> Option<Cf> {
        let n = NSString::from_str(name);
        let mut out: CFTypeRef = std::ptr::null();
        // SAFETY: valid element, toll-free-bridged attribute name, out-pointer.
        let err =
            unsafe { AXUIElementCopyAttributeValue(e, (&*n as *const NSString).cast(), &mut out) };
        (err == 0 && !out.is_null()).then_some(Cf(out))
    }

    fn as_string(v: &Cf) -> Option<String> {
        // SAFETY: type-checked before the toll-free bridge.
        unsafe {
            if CFGetTypeID(v.0) != CFStringGetTypeID() {
                return None;
            }
            Some((*(v.0 as *const NSString)).to_string())
        }
    }

    fn as_i64(v: &Cf) -> Option<i64> {
        let mut out: i64 = 0;
        // SAFETY: type-checked CFNumber; the output is an i64.
        unsafe {
            if CFGetTypeID(v.0) != CFNumberGetTypeID() {
                return None;
            }
            (CFNumberGetValue(v.0, CF_NUMBER_SINT64, (&mut out as *mut i64).cast()) != 0)
                .then_some(out)
        }
    }

    fn as_range(v: &Cf) -> Option<(i64, i64)> {
        let mut r = CFRange::default();
        // SAFETY: AXValueGetValue checks the AXValue's type and fails on a
        // mismatch; the output is a CFRange.
        let ok =
            unsafe { AXValueGetValue(v.0, AX_VALUE_CF_RANGE, (&mut r as *mut CFRange).cast()) };
        (ok != 0).then_some((r.location as i64, r.length as i64))
    }

    /// The pid of the frontmost app (NSWorkspace; RS-04: the system-wide AX
    /// element's focused application fails on macOS 26).
    pub fn frontmost_pid() -> Option<i32> {
        NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .map(|a| a.processIdentifier())
    }

    /// The focused element of the frontmost app, as a fingerprint. `None`
    /// without Accessibility permission or a focused element.
    pub fn focused_field() -> Option<FieldState> {
        // SAFETY: no arguments.
        if unsafe { AXIsProcessTrusted() } == 0 {
            return None;
        }
        let pid = frontmost_pid()?;
        // SAFETY: Create rule; released by `Cf`.
        let app = Cf(unsafe { AXUIElementCreateApplication(pid) });
        if app.0.is_null() {
            return None;
        }
        // SAFETY: valid element.
        unsafe { AXUIElementSetMessagingTimeout(app.0, AX_TIMEOUT_S) };
        let el = attr(app.0, "AXFocusedUIElement")?;
        let mut f = FieldState {
            role: attr(el.0, "AXRole").as_ref().and_then(as_string),
            chars: attr(el.0, "AXNumberOfCharacters").as_ref().and_then(as_i64),
            caret: attr(el.0, "AXSelectedTextRange")
                .as_ref()
                .and_then(as_range),
            value_fingerprint: None,
        };
        if f.chars.is_none() && f.caret.is_none() {
            f.value_fingerprint = attr(el.0, "AXValue")
                .as_ref()
                .and_then(as_string)
                .map(|s| fingerprint(s.encode_utf16()));
        }
        Some(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn state(chars: i64, caret: i64) -> FieldState {
        FieldState {
            role: Some("AXTextArea".into()),
            chars: Some(chars),
            caret: Some((caret, 0)),
            value_fingerprint: None,
        }
    }

    /// Runs `verify` on a fake clock; `after(ms)` is the field at that time.
    fn run(before: Option<FieldState>, after: impl Fn(u64) -> Option<FieldState>) -> Verification {
        let t0 = Instant::now();
        let elapsed = Cell::new(0u64);
        verify(
            before.as_ref(),
            BUDGET,
            || t0 + Duration::from_millis(elapsed.get()),
            |d| elapsed.set(elapsed.get() + d.as_millis() as u64),
            || after(elapsed.get()),
        )
    }

    #[test]
    fn change_within_the_budget_confirms() {
        let v = run(Some(state(10, 10)), |t| {
            Some(if t >= 50 {
                state(25, 25)
            } else {
                state(10, 10)
            })
        });
        assert_eq!(v, Verification::Confirmed { after_ms: 50.0 });
        assert!(v.confirmed());
    }

    #[test]
    fn no_change_is_unverified_after_300_ms() {
        let calls = Cell::new(0);
        let v = run(Some(state(10, 10)), |_| {
            calls.set(calls.get() + 1);
            Some(state(10, 10))
        });
        assert_eq!(
            v,
            Verification::Unverified {
                reason: Unverified::NoChange
            }
        );
        // 0, 25, …, 300 ms.
        assert_eq!(calls.get(), 13);
    }

    #[test]
    fn a_change_after_the_budget_is_too_late() {
        let v = run(Some(state(1, 1)), |t| {
            Some(if t > 300 { state(2, 2) } else { state(1, 1) })
        });
        assert!(!v.confirmed());
    }

    #[test]
    fn no_field_is_unverified_at_once() {
        assert_eq!(
            run(None, |_| Some(state(1, 1))),
            Verification::Unverified {
                reason: Unverified::NoField
            }
        );
        // A focused element that reports nothing comparable (Electron).
        let empty = FieldState {
            role: Some("AXGroup".into()),
            ..FieldState::default()
        };
        assert!(!empty.readable());
        assert_eq!(
            run(Some(empty), |_| Some(state(1, 1))),
            Verification::Unverified {
                reason: Unverified::NoField
            }
        );
    }

    #[test]
    fn focus_lost_after_the_paste_is_not_a_confirmation() {
        let v = run(Some(state(3, 3)), |_| None);
        assert_eq!(
            v,
            Verification::Unverified {
                reason: Unverified::NoChange
            }
        );
    }

    #[test]
    fn ime_composition_then_commit() {
        // An IME first shows marked text (caret moves, count unchanged
        // would still be a change) then commits: any change confirms.
        let v = run(Some(state(5, 5)), |t| {
            Some(if t >= 75 { state(5, 9) } else { state(5, 5) })
        });
        assert!(v.confirmed());
    }

    #[test]
    fn fingerprint_is_stable_and_length_aware() {
        let a = fingerprint("héllo".encode_utf16());
        assert_eq!(a, fingerprint("héllo".encode_utf16()));
        assert_eq!(a.0, 5);
        assert_ne!(a, fingerprint("hello".encode_utf16()));
        assert_eq!(fingerprint("😀".encode_utf16()).0, 2);
    }
}
