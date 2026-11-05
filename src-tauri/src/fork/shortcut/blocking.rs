// SPDX-License-Identifier: GPL-3.0-or-later
//! The dynamic blocking set of R5 §4.8, as handy-keys hotkeys. handy-keys
//! blocks an event when a hotkey in the set matches its modifiers exactly
//! and its key (`platform/state.rs`); for a modifier-only hotkey it blocks
//! the modifier's key-down `FlagsChanged` (never a release).
//!
//! - `{FN}` (the bare fn press): only when the Globe key ("Press 🌐 key
//!   to") is set to Start Dictation. RS-01 showed that blocking the fn press
//!   never stopped the Emoji or Input Source action (DEC-44 sets the setting
//!   to Do Nothing instead), and with Do Nothing nothing fires unblocked, so
//!   the fork leaves fn unblocked for every other value (DEC-44 update,
//!   round 2). A modifier-pair trigger (Ctrl+Opt) is never blocked: it has no
//!   system action and other apps' Ctrl+Opt shortcuts must keep working.
//! - `{modifiers, key}` of a modifier+key trigger (⌥Space): always.
//! - `{trigger, Space}`: while the trigger is down, so Space is swallowed
//!   only as the lock chord (rule 6).
//! - `{Esc}`, `{FN, Esc}` (or `{trigger, Esc}`): while a take is recording
//!   or processing; idle Esc is never swallowed (rule 7).
//! - Keyed action bindings with a handler: always.

use std::collections::HashSet;

use handy_keys::{Hotkey, Key, Modifiers};

use super::bindings::Profile;
use super::decision::Trigger;
use super::processor::Blocking;

/// What drives the set besides the grammar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Extra {
    /// The coordinator is recording or transcribing (Esc cancels).
    pub busy: bool,
}

fn hk(modifiers: Modifiers, key: Option<Key>) -> Hotkey {
    Hotkey { modifiers, key }
}

/// The set for this moment.
pub fn hotkeys(profile: &Profile, grammar: Blocking, extra: Extra) -> HashSet<Hotkey> {
    let mut set = HashSet::new();
    if let Some(trigger) = profile.trigger {
        let m = trigger.modifiers();
        if grammar.fn_press && trigger == Trigger::Fn && (profile.hold || profile.double_tap) {
            set.insert(hk(Modifiers::FN, None));
        }
        // A modifier+key trigger (⌥Space) is always swallowed, repeats
        // included, so it never types into the focused app.
        if let Some(k) = trigger.key() {
            if profile.hold || profile.double_tap {
                set.insert(hk(m, Some(k)));
            }
        }
        if grammar.fn_space && profile.trigger_space {
            set.insert(hk(m, Some(Key::Space)));
        }
        if profile.esc_cancels && (grammar.esc || extra.busy) {
            set.insert(hk(m, Some(Key::Escape)));
        }
    }
    if profile.esc_cancels && (grammar.esc || extra.busy) {
        set.insert(hk(Modifiers::empty(), Some(Key::Escape)));
        set.insert(hk(Modifiers::FN, Some(Key::Escape)));
    }
    for k in &profile.keyed {
        set.insert(k.hotkey);
    }
    set
}

/// What handy-keys' `should_block` decides for an event against `set`
/// (for logs and tests: the real decision happens in the tap callback).
pub fn would_block(
    set: &HashSet<Hotkey>,
    modifiers: Modifiers,
    key: Option<Key>,
    down: bool,
) -> bool {
    if key.is_none() && !down {
        return false; // modifier releases are never blocked
    }
    set.iter()
        .any(|h| h.modifiers.matches(modifiers) && h.key == key)
}

/// `AppleFnUsageType` in `com.apple.HIToolbox`: 0 Do Nothing, 1 Change
/// Input Source, 2 Show Emoji & Symbols, 3 Start Dictation; unset means the
/// macOS default (Emoji).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlobeUsage {
    DoNothing,
    InputSource,
    Emoji,
    Dictation,
    /// Unset or unknown.
    Default,
}

impl GlobeUsage {
    pub fn from_value(v: Option<i64>) -> GlobeUsage {
        match v {
            Some(0) => GlobeUsage::DoNothing,
            Some(1) => GlobeUsage::InputSource,
            Some(2) => GlobeUsage::Emoji,
            Some(3) => GlobeUsage::Dictation,
            _ => GlobeUsage::Default,
        }
    }

    /// Whether `{FN}` goes in the blocking set (DEC-44 update).
    pub fn blocks_fn(self) -> bool {
        self == GlobeUsage::Dictation
    }
}

/// Reads the Globe setting now (fresh from disk: `CFPreferencesAppSynchronize`
/// first, since RS-01 once read a stale value). macOS only; elsewhere
/// `Default`.
pub fn read_globe_usage() -> GlobeUsage {
    GlobeUsage::from_value(platform::fn_usage_type())
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::c_void;

    type CFTypeRef = *const c_void;
    type CFStringRef = *const c_void;

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFStringCreateWithCString(
            alloc: *const c_void,
            c_str: *const std::os::raw::c_char,
            encoding: u32,
        ) -> CFStringRef;
        fn CFPreferencesAppSynchronize(app: CFStringRef) -> u8;
        fn CFPreferencesCopyAppValue(key: CFStringRef, app: CFStringRef) -> CFTypeRef;
        fn CFNumberGetValue(number: CFTypeRef, the_type: isize, value: *mut c_void) -> u8;
        fn CFGetTypeID(cf: CFTypeRef) -> usize;
        fn CFNumberGetTypeID() -> usize;
        fn CFRelease(cf: CFTypeRef);
    }

    const UTF8: u32 = 0x0800_0100;
    const CF_NUMBER_SINT64: isize = 4;

    fn cfstr(s: &std::ffi::CStr) -> CFStringRef {
        // SAFETY: a valid NUL-terminated C string; the result is released by
        // the caller.
        unsafe { CFStringCreateWithCString(std::ptr::null(), s.as_ptr(), UTF8) }
    }

    pub fn fn_usage_type() -> Option<i64> {
        let app = cfstr(c"com.apple.HIToolbox");
        let key = cfstr(c"AppleFnUsageType");
        if app.is_null() || key.is_null() {
            return None;
        }
        // SAFETY: both strings are valid CFStrings; the copied value follows
        // the Create Rule and is released below; every pointer is checked.
        let out = unsafe {
            CFPreferencesAppSynchronize(app);
            let value = CFPreferencesCopyAppValue(key, app);
            let mut n: i64 = 0;
            let ok = !value.is_null()
                && CFGetTypeID(value) == CFNumberGetTypeID()
                && CFNumberGetValue(value, CF_NUMBER_SINT64, &mut n as *mut i64 as *mut c_void)
                    != 0;
            if !value.is_null() {
                CFRelease(value);
            }
            ok.then_some(n)
        };
        // SAFETY: created above, released once.
        unsafe {
            CFRelease(key);
            CFRelease(app);
        }
        out
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    pub fn fn_usage_type() -> Option<i64> {
        None
    }
}
