// SPDX-License-Identifier: GPL-3.0-or-later
//! Apple's own Dictation on "Press fn twice" (DEC-44 follow-up, OT2). With
//! Keyboard › Dictation on and its Shortcut set to "Press 🌐 twice" /
//! "Press fn twice", our hands-free double tap also starts macOS Dictation,
//! which types the take a second time.
//!
//! Where macOS keeps it (read-only probes, macOS 26.6):
//! - Dictation on: `"Dictation Enabled"` in `com.apple.assistant.support`.
//! - The shortcut: symbolic hotkey 164 in `com.apple.symbolichotkeys`
//!   (`AppleSymbolicHotKeys`). "Press fn twice" is
//!   `{enabled = 1; value = {type = modifier; parameters = (8388608, …)}}`
//!   (8388608 = `NSEventModifierFlagFunction`); the other double presses
//!   (Control, Command) carry their own modifier flag, Off is `enabled = 0`.
//! - Live: WindowServer's symbolic hotkey table
//!   (`CGSIsSymbolicHotKeyEnabled(164)`, SkyLight), loaded from the
//!   preference at login.
//!
//! The owner's call: while fn is the trigger the app switches the shortcut
//! off itself (no consent step), says so once, and has a Settings switch to
//! keep it. It is switched off live with `CGSSetSymbolicHotKeyEnabled(164,
//! false)` (the call AltTab uses for ⌘-Tab): the user's preference is never
//! written, so System Settings still shows their choice and the next login
//! starts from it; the app re-applies at every launch. Writing the
//! preference instead is not picked up until the next login.
//!
//! Lifecycle as in `globe.rs`: a marker is written *before* the change;
//! [`reconcile`] (launch, binding change, the Settings switch) applies while
//! fn is the trigger and restores otherwise; a restore only re-enables when
//! the preference still says "Press fn twice" (a later Off in System
//! Settings wins). An unset entry 164 (macOS default, never touched) is not
//! acted on: the default differs by keyboard.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;

/// Apple's Dictation shortcut in `AppleSymbolicHotKeys`.
pub const HOTKEY_ID: i32 = 164;
pub const MARKER_FILE: &str = "apple-dictation-shortcut.json";
const STATE_VERSION: u32 = 1;

/// `NSEventModifierFlagFunction`, the first `parameters` value of a
/// modifier-type symbolic hotkey for fn/🌐.
const FN_FLAG: i64 = 0x80_0000;

/// Symbolic hotkey 164 as read from `AppleSymbolicHotKeys`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shortcut {
    pub enabled: Option<bool>,
    /// `value.type`: `modifier` for the "Press … twice" choices.
    pub kind: Option<String>,
    /// `value.parameters[0]`: the modifier flag of a double press.
    pub first_parameter: Option<i64>,
}

/// Whether pressing fn twice starts Apple's Dictation (pure).
pub fn fires_on_fn_double(dictation_enabled: Option<bool>, shortcut: Option<&Shortcut>) -> bool {
    if dictation_enabled != Some(true) {
        return false;
    }
    let Some(s) = shortcut else {
        return false;
    };
    s.enabled == Some(true)
        && s.kind.as_deref() == Some("modifier")
        && s.first_parameter.is_some_and(|p| p & FN_FLAG != 0)
}

/// Reads both preferences now (fresh, through cfprefsd). Off macOS: false.
pub fn fires_on_fn_double_now() -> bool {
    let (enabled, shortcut) = platform::read();
    fires_on_fn_double(enabled, shortcut.as_ref())
}

/// What Settings › Shortcuts and onboarding show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum AppleDictation {
    /// fn is not the trigger, or Apple's shortcut is not "Press fn twice".
    None,
    /// We switched it off while fn is the trigger.
    TurnedOff,
    /// It still starts on fn twice (the user kept it, or macOS refused).
    Active,
}

/// WindowServer's live symbolic hotkey table, behind a trait for tests.
pub trait LiveHotkey {
    /// `None` when the private entry points are missing.
    fn is_enabled(&self) -> Option<bool>;
    fn set_enabled(&mut self, on: bool);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Marker {
    version: u32,
}

fn has_marker(path: &Path) -> bool {
    match std::fs::read(path)
        .ok()
        .and_then(|d| serde_json::from_slice::<Marker>(&d).ok())
    {
        Some(m) if m.version == STATE_VERSION => true,
        Some(_) => {
            let _ = std::fs::remove_file(path);
            false
        }
        None => {
            if path.exists() {
                let _ = std::fs::remove_file(path);
            }
            false
        }
    }
}

fn write_marker(path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_vec(&Marker {
        version: STATE_VERSION,
    })
    .map_err(|e| e.to_string())?;
    std::fs::write(&tmp, json).map_err(|e| format!("cannot write the restore marker: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("cannot write the restore marker: {e}"))
}

#[derive(Debug, PartialEq, Eq)]
pub enum Applied {
    /// The preference is not "Press fn twice" (or Dictation is off).
    NotNeeded,
    /// Already off live (by us earlier this login, or by someone else).
    AlreadyOff,
    TurnedOff,
}

/// Switches the shortcut off live after recording that we did.
/// `fn_double`: the preference says "Press fn twice" with Dictation on.
pub fn apply_with(
    sys: &mut dyn LiveHotkey,
    marker: &Path,
    fn_double: bool,
) -> Result<Applied, String> {
    if !fn_double {
        return Ok(Applied::NotNeeded);
    }
    match sys.is_enabled() {
        None => Err("this macOS does not let apps switch the Dictation shortcut off".into()),
        Some(false) => Ok(Applied::AlreadyOff),
        Some(true) => {
            write_marker(marker)?;
            sys.set_enabled(false);
            if sys.is_enabled() != Some(false) {
                return Err("macOS did not switch the Dictation shortcut off".into());
            }
            Ok(Applied::TurnedOff)
        }
    }
}

/// Switches it back on if we switched it off and the preference still
/// asks for it, then drops the marker. Returns whether it was re-enabled.
pub fn restore_with(sys: &mut dyn LiveHotkey, marker: &Path, fn_double: bool) -> bool {
    if !has_marker(marker) {
        return false;
    }
    let mut restored = false;
    if fn_double && sys.is_enabled() == Some(false) {
        sys.set_enabled(true);
        restored = true;
    }
    let _ = std::fs::remove_file(marker);
    restored
}

/// The state shown to the user (pure).
pub fn state_with(
    sys: &dyn LiveHotkey,
    marker: &Path,
    fn_is_trigger: bool,
    fn_double: bool,
) -> AppleDictation {
    if !fn_is_trigger || !fn_double {
        return AppleDictation::None;
    }
    match sys.is_enabled() {
        Some(false) if has_marker(marker) => AppleDictation::TurnedOff,
        Some(false) => AppleDictation::None,
        _ => AppleDictation::Active,
    }
}

/// Only the product identity changes the live table, or a test identity
/// whose Mac check asks for it: a test run must never switch the
/// developer's own shortcut off.
pub fn may_change(identifier: &str) -> bool {
    identifier == crate::fork::PRODUCT_IDENTIFIER
        || (crate::fork::is_test_identity(identifier)
            && std::env::var_os("WISPRBRO_TEST_APPLE_DICTATION").is_some_and(|v| v == "1"))
}

/// The marker path in the app's (portable-aware) data dir.
pub fn marker_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Option<PathBuf> {
    use tauri::Manager;
    crate::portable::data_dir()
        .cloned()
        .or_else(|| app.path().app_data_dir().ok())
        .map(|dir| dir.join(MARKER_FILE))
}

/// Applies (fn is the trigger and the user did not keep Apple's shortcut)
/// or restores. Launch, binding changes and the Settings switch call it.
pub fn reconcile<R: tauri::Runtime>(app: &tauri::AppHandle<R>, want_off: bool) {
    if !may_change(&app.config().identifier) {
        return;
    }
    let Some(marker) = marker_path(app) else {
        return;
    };
    let fn_double = fires_on_fn_double_now();
    let mut sys = platform::System;
    if want_off {
        match apply_with(&mut sys, &marker, fn_double) {
            Ok(Applied::TurnedOff) => {
                log::info!("fork hotkeys: Apple Dictation's fn-twice shortcut switched off")
            }
            Ok(_) => {}
            Err(e) => log::warn!("fork hotkeys: Apple Dictation shortcut: {e}"),
        }
    } else if restore_with(&mut sys, &marker, fn_double) {
        log::info!("fork hotkeys: Apple Dictation's fn-twice shortcut switched back on");
    }
}

/// The state now, for the hotkey status.
pub fn state<R: tauri::Runtime>(app: &tauri::AppHandle<R>, fn_is_trigger: bool) -> AppleDictation {
    if !fn_is_trigger {
        return AppleDictation::None;
    }
    let Some(marker) = marker_path(app) else {
        return AppleDictation::None;
    };
    state_with(&platform::System, &marker, true, fires_on_fn_double_now())
}

#[cfg(target_os = "macos")]
mod platform {
    use super::Shortcut;
    use std::ffi::{c_void, CStr};

    type CFTypeRef = *const c_void;

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFStringCreateWithCString(
            alloc: *const c_void,
            c_str: *const std::os::raw::c_char,
            encoding: u32,
        ) -> CFTypeRef;
        fn CFPreferencesAppSynchronize(app: CFTypeRef) -> u8;
        fn CFPreferencesCopyAppValue(key: CFTypeRef, app: CFTypeRef) -> CFTypeRef;
        fn CFDictionaryGetValue(dict: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
        fn CFArrayGetCount(array: CFTypeRef) -> isize;
        fn CFArrayGetValueAtIndex(array: CFTypeRef, idx: isize) -> CFTypeRef;
        fn CFNumberGetValue(number: CFTypeRef, the_type: isize, value: *mut c_void) -> u8;
        fn CFBooleanGetValue(boolean: CFTypeRef) -> u8;
        fn CFStringGetCString(
            s: CFTypeRef,
            buf: *mut std::os::raw::c_char,
            size: isize,
            encoding: u32,
        ) -> u8;
        fn CFGetTypeID(cf: CFTypeRef) -> usize;
        fn CFNumberGetTypeID() -> usize;
        fn CFBooleanGetTypeID() -> usize;
        fn CFStringGetTypeID() -> usize;
        fn CFDictionaryGetTypeID() -> usize;
        fn CFArrayGetTypeID() -> usize;
        fn CFRelease(cf: CFTypeRef);
    }

    const UTF8: u32 = 0x0800_0100;
    const CF_NUMBER_SINT64: isize = 4;

    /// An owned CF object (Create Rule), released on drop.
    struct Owned(CFTypeRef);

    impl Owned {
        fn string(s: &CStr) -> Option<Owned> {
            // SAFETY: a valid NUL-terminated C string.
            let r = unsafe { CFStringCreateWithCString(std::ptr::null(), s.as_ptr(), UTF8) };
            (!r.is_null()).then_some(Owned(r))
        }
    }

    impl Drop for Owned {
        fn drop(&mut self) {
            // SAFETY: non-null, owned by us, released once.
            unsafe { CFRelease(self.0) }
        }
    }

    fn is(v: CFTypeRef, type_id: usize) -> bool {
        // SAFETY: `v` is a non-null CF object.
        !v.is_null() && unsafe { CFGetTypeID(v) } == type_id
    }

    /// A Boolean or a number (macOS writes both for flags).
    fn as_bool(v: CFTypeRef) -> Option<bool> {
        // SAFETY: the type is checked before each typed call.
        unsafe {
            if is(v, CFBooleanGetTypeID()) {
                Some(CFBooleanGetValue(v) != 0)
            } else {
                as_i64(v).map(|n| n != 0)
            }
        }
    }

    fn as_i64(v: CFTypeRef) -> Option<i64> {
        // SAFETY: the type is checked; `n` outlives the call.
        unsafe {
            if !is(v, CFNumberGetTypeID()) {
                return None;
            }
            let mut n: i64 = 0;
            (CFNumberGetValue(v, CF_NUMBER_SINT64, &mut n as *mut i64 as *mut c_void) != 0)
                .then_some(n)
        }
    }

    fn as_string(v: CFTypeRef) -> Option<String> {
        if !is(v, unsafe { CFStringGetTypeID() }) {
            return None;
        }
        let mut buf = [0 as std::os::raw::c_char; 64];
        // SAFETY: a CFString and a buffer of the given size.
        let ok = unsafe { CFStringGetCString(v, buf.as_mut_ptr(), buf.len() as isize, UTF8) };
        if ok == 0 {
            return None;
        }
        // SAFETY: CFStringGetCString wrote a NUL-terminated string.
        Some(
            unsafe { CStr::from_ptr(buf.as_ptr()) }
                .to_string_lossy()
                .into_owned(),
        )
    }

    /// A dictionary member (borrowed, Get Rule).
    fn get(dict: CFTypeRef, key: &CStr) -> CFTypeRef {
        // SAFETY: the type is checked; the key lives across the lookup.
        unsafe {
            if !is(dict, CFDictionaryGetTypeID()) {
                return std::ptr::null();
            }
            match Owned::string(key) {
                Some(k) => CFDictionaryGetValue(dict, k.0),
                None => std::ptr::null(),
            }
        }
    }

    /// A fresh copy of one app preference (Create Rule).
    fn copy_pref(domain: &CStr, key: &CStr) -> Option<Owned> {
        let d = Owned::string(domain)?;
        let k = Owned::string(key)?;
        // SAFETY: valid CFStrings; the copy is owned by the result.
        unsafe {
            CFPreferencesAppSynchronize(d.0);
            let v = CFPreferencesCopyAppValue(k.0, d.0);
            (!v.is_null()).then_some(Owned(v))
        }
    }

    type IsEnabled = unsafe extern "C" fn(i32) -> bool;
    type SetEnabled = unsafe extern "C" fn(i32, bool) -> i32;

    fn entry_points() -> Option<(IsEnabled, SetEnabled)> {
        use std::sync::OnceLock;
        static ENTRY: OnceLock<Option<(usize, usize)>> = OnceLock::new();
        let (is, set) = (*ENTRY.get_or_init(|| {
            // SAFETY: dlopen/dlsym with valid NUL-terminated strings; the
            // handle stays open for the life of the process.
            unsafe {
                for path in [
                    c"/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight",
                    c"/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics",
                ] {
                    let lib = libc::dlopen(path.as_ptr(), libc::RTLD_LAZY);
                    if lib.is_null() {
                        continue;
                    }
                    let is = libc::dlsym(lib, c"CGSIsSymbolicHotKeyEnabled".as_ptr());
                    let set = libc::dlsym(lib, c"CGSSetSymbolicHotKeyEnabled".as_ptr());
                    if !is.is_null() && !set.is_null() {
                        return Some((is as usize, set as usize));
                    }
                }
                None
            }
        }))?;
        // SAFETY: `bool CGSIsSymbolicHotKeyEnabled(CGSSymbolicHotKey)` and
        // `CGError CGSSetSymbolicHotKeyEnabled(CGSSymbolicHotKey, bool)`.
        unsafe {
            Some((
                std::mem::transmute::<usize, IsEnabled>(is),
                std::mem::transmute::<usize, SetEnabled>(set),
            ))
        }
    }

    pub struct System;

    impl super::LiveHotkey for System {
        fn is_enabled(&self) -> Option<bool> {
            let (is, _) = entry_points()?;
            // SAFETY: a C function taking one int.
            Some(unsafe { is(super::HOTKEY_ID) })
        }

        fn set_enabled(&mut self, on: bool) {
            if let Some((_, set)) = entry_points() {
                // SAFETY: a C function taking an int and a bool.
                let err = unsafe { set(super::HOTKEY_ID, on) };
                if err != 0 {
                    log::warn!("fork hotkeys: CGSSetSymbolicHotKeyEnabled returned {err}");
                }
            }
        }
    }

    pub fn read() -> (Option<bool>, Option<Shortcut>) {
        let enabled = copy_pref(c"com.apple.assistant.support", c"Dictation Enabled")
            .and_then(|v| as_bool(v.0));
        let shortcut =
            copy_pref(c"com.apple.symbolichotkeys", c"AppleSymbolicHotKeys").and_then(|all| {
                let entry = get(all.0, c"164");
                if entry.is_null() {
                    return None;
                }
                let value = get(entry, c"value");
                let params = get(value, c"parameters");
                // SAFETY: the type is checked before the array calls.
                let first_parameter = unsafe {
                    (is(params, CFArrayGetTypeID()) && CFArrayGetCount(params) > 0)
                        .then(|| as_i64(CFArrayGetValueAtIndex(params, 0)))
                        .flatten()
                };
                Some(Shortcut {
                    enabled: as_bool(get(entry, c"enabled")),
                    kind: as_string(get(value, c"type")),
                    first_parameter,
                })
            });
        (enabled, shortcut)
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    pub fn read() -> (Option<bool>, Option<super::Shortcut>) {
        (None, None)
    }

    pub struct System;

    impl super::LiveHotkey for System {
        fn is_enabled(&self) -> Option<bool> {
            None
        }
        fn set_enabled(&mut self, _on: bool) {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        live: Option<bool>,
        ignore: bool,
        sets: Vec<bool>,
    }

    impl LiveHotkey for Fake {
        fn is_enabled(&self) -> Option<bool> {
            self.live
        }
        fn set_enabled(&mut self, on: bool) {
            self.sets.push(on);
            if !self.ignore {
                self.live = Some(on);
            }
        }
    }

    fn fake(live: Option<bool>) -> Fake {
        Fake {
            live,
            ignore: false,
            sets: Vec::new(),
        }
    }

    fn marker() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join(MARKER_FILE);
        (dir, path)
    }

    #[test]
    fn apply_marks_first_then_switches_off_and_restore_switches_back() {
        let (_d, m) = marker();
        let mut sys = fake(Some(true));
        assert_eq!(apply_with(&mut sys, &m, true), Ok(Applied::TurnedOff));
        assert_eq!(sys.live, Some(false));
        assert!(m.exists());
        assert_eq!(state_with(&sys, &m, true, true), AppleDictation::TurnedOff);
        assert!(restore_with(&mut sys, &m, true));
        assert_eq!(sys.live, Some(true));
        assert!(!m.exists());
    }

    #[test]
    fn nothing_changes_when_the_shortcut_is_not_fn_twice() {
        let (_d, m) = marker();
        let mut sys = fake(Some(true));
        assert_eq!(apply_with(&mut sys, &m, false), Ok(Applied::NotNeeded));
        assert!(sys.sets.is_empty());
        assert!(!m.exists());
        assert_eq!(state_with(&sys, &m, true, false), AppleDictation::None);
    }

    #[test]
    fn already_off_needs_no_marker_and_is_not_ours_to_restore() {
        let (_d, m) = marker();
        let mut sys = fake(Some(false));
        assert_eq!(apply_with(&mut sys, &m, true), Ok(Applied::AlreadyOff));
        assert!(!m.exists());
        assert!(!restore_with(&mut sys, &m, true));
        assert_eq!(sys.live, Some(false));
    }

    #[test]
    fn a_later_off_in_system_settings_wins_over_the_restore() {
        let (_d, m) = marker();
        let mut sys = fake(Some(true));
        apply_with(&mut sys, &m, true).unwrap();
        // The user picked Off in System Settings meanwhile.
        assert!(!restore_with(&mut sys, &m, false));
        assert_eq!(sys.live, Some(false));
        assert!(!m.exists());
    }

    #[test]
    fn missing_entry_points_or_a_refusal_leave_it_active() {
        let (_d, m) = marker();
        let mut sys = fake(None);
        assert!(apply_with(&mut sys, &m, true).is_err());
        assert!(!m.exists());
        assert_eq!(state_with(&sys, &m, true, true), AppleDictation::Active);
        let mut sys = fake(Some(true));
        sys.ignore = true;
        assert!(apply_with(&mut sys, &m, true).is_err());
        assert_eq!(state_with(&sys, &m, true, true), AppleDictation::Active);
    }

    #[test]
    fn re_enabled_by_the_user_shows_active_and_a_new_login_reapplies() {
        let (_d, m) = marker();
        let mut sys = fake(Some(true));
        apply_with(&mut sys, &m, true).unwrap();
        // A new login loads the preference again: live is on, marker kept.
        sys.live = Some(true);
        assert_eq!(state_with(&sys, &m, true, true), AppleDictation::Active);
        assert_eq!(apply_with(&mut sys, &m, true), Ok(Applied::TurnedOff));
    }

    #[test]
    fn not_the_trigger_shows_nothing() {
        let (_d, m) = marker();
        let sys = fake(Some(true));
        assert_eq!(state_with(&sys, &m, false, true), AppleDictation::None);
    }

    #[test]
    fn a_damaged_marker_is_dropped() {
        let (_d, m) = marker();
        std::fs::create_dir_all(m.parent().unwrap()).unwrap();
        std::fs::write(&m, b"{not json").unwrap();
        let mut sys = fake(Some(false));
        assert!(!restore_with(&mut sys, &m, true));
        assert!(!m.exists());
    }

    #[test]
    fn only_the_product_identity_changes_the_live_table() {
        assert!(may_change("com.wisprbro.desktop"));
        assert!(!may_change("com.wisprbro.other"));
        // Test identities only with WISPRBRO_TEST_APPLE_DICTATION=1 (unset here).
        if std::env::var_os("WISPRBRO_TEST_APPLE_DICTATION").is_none() {
            assert!(!may_change("com.wisprbro.ot2check"));
        }
    }

    fn modifier(flag: i64, enabled: bool) -> Shortcut {
        Shortcut {
            enabled: Some(enabled),
            kind: Some("modifier".into()),
            first_parameter: Some(flag),
        }
    }

    #[test]
    fn press_fn_twice_with_dictation_on_is_flagged() {
        // The owner's Mac: (8388608, 4286578687).
        assert!(fires_on_fn_double(
            Some(true),
            Some(&modifier(8_388_608, true))
        ));
    }

    #[test]
    fn dictation_off_or_unknown_is_not_flagged() {
        let fn_twice = modifier(8_388_608, true);
        assert!(!fires_on_fn_double(Some(false), Some(&fn_twice)));
        assert!(!fires_on_fn_double(None, Some(&fn_twice)));
    }

    #[test]
    fn shortcut_off_or_unset_is_not_flagged() {
        assert!(!fires_on_fn_double(
            Some(true),
            Some(&modifier(8_388_608, false))
        ));
        assert!(!fires_on_fn_double(
            Some(true),
            Some(&Shortcut {
                enabled: Some(false),
                ..Shortcut::default()
            })
        ));
        assert!(!fires_on_fn_double(Some(true), None));
        assert!(!fires_on_fn_double(Some(true), Some(&Shortcut::default())));
    }

    #[test]
    fn other_double_presses_are_not_flagged() {
        for flag in [
            0x4_0000,  // Control
            0x10_0010, // Right Command
            0x10_0008, // Left Command
            0x10_0000, // Either Command
        ] {
            assert!(!fires_on_fn_double(Some(true), Some(&modifier(flag, true))));
        }
    }

    #[test]
    fn a_custom_key_shortcut_is_not_flagged() {
        let custom = Shortcut {
            enabled: Some(true),
            kind: Some("standard".into()),
            first_parameter: Some(8_388_608),
        };
        assert!(!fires_on_fn_double(Some(true), Some(&custom)));
    }
}
