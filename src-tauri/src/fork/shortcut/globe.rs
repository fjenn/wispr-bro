// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2024 OpenWhispr Team (OpenWhispr; MIT, notice below)
// Modified by Wispr Bro contributors, 2026-11-02.
//
// Ported from OpenWhispr `resources/macos-globe-listener.swift`
// (`GlobeSystemAction`; https://github.com/OpenWhispr/openwhispr, commit
// b9e9ebefac4b961456c269164a93ddfdd53438b9) to Rust. Changes: the marker lives
// in the app's data dir; the value is held while fn is the dictation trigger
// (not while a helper process runs), applied only after the user's consent in
// onboarding (DEC-44), and released when fn stops being the trigger.
//
// MIT License
//
// Copyright (c) 2024 OpenWhispr Team
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! DEC-44: the macOS Globe key's own action ("Press 🌐 key to": emoji
//! viewer, input-source switch) runs in WindowServer ahead of every event
//! tap, so the only way to stop it firing with each fn take is to set
//! `AppleFnUsageType` to Do Nothing. `TISUpdateFnUsageType` (Carbon, private)
//! is what System Settings calls: it persists the value and applies it live;
//! writing the preference directly is ignored until the next login.
//!
//! Crash-safe: the user's original value is written to a marker file
//! *before* anything changes. [`restore`] puts it back while the value is
//! still ours (Do Nothing); if the user picked another action since, theirs
//! wins and the marker is dropped. [`reconcile`] runs at launch, at quit and
//! after every binding change: with a marker and fn no longer the trigger,
//! it restores.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const DO_NOTHING: i32 = 0;
const STATE_VERSION: u32 = 1;
pub const MARKER_FILE: &str = "globe-preference.json";

/// What the restore marker records (OpenWhispr `GlobePreferenceState`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marker {
    pub version: u32,
    pub original_value: i32,
    /// The key was set before we changed it. When it was not (a
    /// macOS-computed default), restore clears it again so it keeps
    /// tracking the hardware and input sources.
    pub key_existed: bool,
}

/// The system calls, behind a trait so the decisions are unit-tested.
pub trait FnUsage {
    /// `TISGetFnUsageType`, `None` when the entry point is missing.
    fn get(&self) -> Option<i32>;
    fn update(&mut self, value: i32);
    /// The raw `AppleFnUsageType` key exists in the user's preferences.
    fn key_exists(&self) -> bool;
    fn clear_key(&mut self);
}

#[derive(Debug, PartialEq, Eq)]
pub enum Applied {
    /// Already Do Nothing: nothing changed, no marker.
    AlreadyDoNothing,
    Changed,
}

pub fn read_marker(path: &Path) -> Option<Marker> {
    let data = std::fs::read(path).ok()?;
    match serde_json::from_slice::<Marker>(&data) {
        Ok(m) if m.version == STATE_VERSION => Some(m),
        _ => {
            let _ = std::fs::remove_file(path);
            None
        }
    }
}

fn write_marker(path: &Path, marker: &Marker) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_vec(marker).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, json).map_err(|e| format!("cannot write the restore marker: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("cannot write the restore marker: {e}"))
}

/// Sets Do Nothing after recording how to get back. A marker left by an
/// earlier run is kept (its original value is the user's, not ours).
pub fn apply_with(sys: &mut dyn FnUsage, marker: &Path) -> Result<Applied, String> {
    let current = sys
        .get()
        .ok_or("this macOS does not let apps change the Globe key setting")?;
    if current == DO_NOTHING {
        return Ok(Applied::AlreadyDoNothing);
    }
    if read_marker(marker).is_none() {
        write_marker(
            marker,
            &Marker {
                version: STATE_VERSION,
                original_value: current,
                key_existed: sys.key_exists(),
            },
        )?;
    }
    sys.update(DO_NOTHING);
    if sys.get() != Some(DO_NOTHING) {
        return Err("macOS did not apply the Globe key setting".into());
    }
    Ok(Applied::Changed)
}

/// Puts the user's value back while it is still ours, then drops the
/// marker. Returns whether a value was restored.
pub fn restore_with(sys: &mut dyn FnUsage, marker: &Path) -> bool {
    let Some(state) = read_marker(marker) else {
        return false;
    };
    let mut restored = false;
    if sys.get() == Some(DO_NOTHING) {
        sys.update(state.original_value);
        if !state.key_existed {
            sys.clear_key();
        }
        restored = true;
    }
    let _ = std::fs::remove_file(marker);
    restored
}

/// The marker path in the app's (portable-aware) data dir.
pub fn marker_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Option<PathBuf> {
    use tauri::Manager;
    crate::portable::data_dir()
        .cloned()
        .or_else(|| app.path().app_data_dir().ok())
        .map(|dir| dir.join(MARKER_FILE))
}

/// Applies Do Nothing (the user consented in onboarding).
pub fn apply<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<Applied, String> {
    let marker = marker_path(app).ok_or("no app data folder for the restore marker")?;
    apply_with(&mut platform::System, &marker)
}

/// Restores the user's Globe setting if we changed it (S-27's "Restore
/// system keyboard settings" calls this too).
pub fn restore<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> bool {
    marker_path(app).is_some_and(|m| restore_with(&mut platform::System, &m))
}

/// Restores when fn is no longer the dictation trigger (launch, quit, any
/// binding change). Cheap when there is no marker.
pub fn reconcile<R: tauri::Runtime>(app: &tauri::AppHandle<R>, fn_is_trigger: bool) {
    if fn_is_trigger {
        return;
    }
    let Some(marker) = marker_path(app) else {
        return;
    };
    if marker.exists() && restore_with(&mut platform::System, &marker) {
        log::info!("fork hotkeys: fn is no longer the trigger; Globe key setting restored");
        super::refresh_globe();
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::c_void;

    type CFStringRef = *const c_void;
    type CFTypeRef = *const c_void;

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        static kCFPreferencesCurrentUser: CFStringRef;
        static kCFPreferencesAnyHost: CFStringRef;
        fn CFStringCreateWithCString(
            alloc: *const c_void,
            c_str: *const std::os::raw::c_char,
            encoding: u32,
        ) -> CFStringRef;
        fn CFPreferencesSynchronize(app: CFStringRef, user: CFStringRef, host: CFStringRef) -> u8;
        fn CFPreferencesCopyValue(
            key: CFStringRef,
            app: CFStringRef,
            user: CFStringRef,
            host: CFStringRef,
        ) -> CFTypeRef;
        fn CFPreferencesSetValue(
            key: CFStringRef,
            value: CFTypeRef,
            app: CFStringRef,
            user: CFStringRef,
            host: CFStringRef,
        );
        fn CFRelease(cf: CFTypeRef);
    }

    const UTF8: u32 = 0x0800_0100;

    type GetUsage = unsafe extern "C" fn() -> i32;
    type UpdateUsage = unsafe extern "C" fn(i32);

    fn entry_points() -> Option<(GetUsage, UpdateUsage)> {
        use std::sync::OnceLock;
        static ENTRY: OnceLock<Option<(usize, usize)>> = OnceLock::new();
        let (get, update) = (*ENTRY.get_or_init(|| {
            // SAFETY: dlopen/dlsym with valid NUL-terminated strings; the
            // handle stays open for the life of the process.
            unsafe {
                let carbon = libc::dlopen(
                    c"/System/Library/Frameworks/Carbon.framework/Carbon".as_ptr(),
                    libc::RTLD_LAZY,
                );
                if carbon.is_null() {
                    return None;
                }
                let get = libc::dlsym(carbon, c"TISGetFnUsageType".as_ptr());
                let update = libc::dlsym(carbon, c"TISUpdateFnUsageType".as_ptr());
                (!get.is_null() && !update.is_null()).then_some((get as usize, update as usize))
            }
        }))?;
        // SAFETY: the symbols have these C signatures (OpenWhispr's
        // `@convention(c) () -> Int32` and `(Int32) -> Void`).
        unsafe {
            Some((
                std::mem::transmute::<usize, GetUsage>(get),
                std::mem::transmute::<usize, UpdateUsage>(update),
            ))
        }
    }

    struct Names {
        domain: CFStringRef,
        key: CFStringRef,
    }

    impl Names {
        fn new() -> Option<Names> {
            // SAFETY: valid C strings; released in Drop.
            let (domain, key) = unsafe {
                (
                    CFStringCreateWithCString(
                        std::ptr::null(),
                        c"com.apple.HIToolbox".as_ptr(),
                        UTF8,
                    ),
                    CFStringCreateWithCString(std::ptr::null(), c"AppleFnUsageType".as_ptr(), UTF8),
                )
            };
            let names = Names { domain, key };
            (!domain.is_null() && !key.is_null()).then_some(names)
        }
    }

    impl Drop for Names {
        fn drop(&mut self) {
            // SAFETY: each non-null string was created above, released once.
            unsafe {
                if !self.domain.is_null() {
                    CFRelease(self.domain);
                }
                if !self.key.is_null() {
                    CFRelease(self.key);
                }
            }
        }
    }

    pub struct System;

    impl super::FnUsage for System {
        fn get(&self) -> Option<i32> {
            let (get, _) = entry_points()?;
            // SAFETY: a C function without arguments.
            Some(unsafe { get() })
        }

        fn update(&mut self, value: i32) {
            if let Some((_, update)) = entry_points() {
                // SAFETY: a C function taking one Int32.
                unsafe { update(value) }
            }
        }

        // CFPreferencesCopyAppValue caches foreign domains for the life of
        // the process; synchronize and read the current user's value.
        fn key_exists(&self) -> bool {
            let Some(n) = Names::new() else { return false };
            // SAFETY: valid CFStrings and the framework's constants; the
            // copied value follows the Create Rule.
            unsafe {
                CFPreferencesSynchronize(
                    n.domain,
                    kCFPreferencesCurrentUser,
                    kCFPreferencesAnyHost,
                );
                let v = CFPreferencesCopyValue(
                    n.key,
                    n.domain,
                    kCFPreferencesCurrentUser,
                    kCFPreferencesAnyHost,
                );
                if v.is_null() {
                    false
                } else {
                    CFRelease(v);
                    true
                }
            }
        }

        fn clear_key(&mut self) {
            let Some(n) = Names::new() else { return };
            // SAFETY: as above; a null value removes the key.
            unsafe {
                CFPreferencesSetValue(
                    n.key,
                    std::ptr::null(),
                    n.domain,
                    kCFPreferencesCurrentUser,
                    kCFPreferencesAnyHost,
                );
                CFPreferencesSynchronize(
                    n.domain,
                    kCFPreferencesCurrentUser,
                    kCFPreferencesAnyHost,
                );
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    pub struct System;

    impl super::FnUsage for System {
        fn get(&self) -> Option<i32> {
            None
        }
        fn update(&mut self, _value: i32) {}
        fn key_exists(&self) -> bool {
            false
        }
        fn clear_key(&mut self) {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        value: Option<i32>,
        key: bool,
        updates: Vec<i32>,
        /// macOS ignores the update (a broken private API).
        ignore: bool,
    }

    impl Fake {
        fn new(value: Option<i32>, key: bool) -> Fake {
            Fake {
                value,
                key,
                updates: Vec::new(),
                ignore: false,
            }
        }
    }

    impl FnUsage for Fake {
        fn get(&self) -> Option<i32> {
            self.value
        }
        fn update(&mut self, value: i32) {
            self.updates.push(value);
            if !self.ignore {
                self.value = Some(value);
                self.key = true;
            }
        }
        fn key_exists(&self) -> bool {
            self.key
        }
        fn clear_key(&mut self) {
            self.key = false;
        }
    }

    fn marker() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join(MARKER_FILE);
        (dir, path)
    }

    #[test]
    fn apply_records_the_original_first_and_restore_puts_it_back() {
        let (_d, m) = marker();
        let mut sys = Fake::new(Some(2), true);
        assert_eq!(apply_with(&mut sys, &m), Ok(Applied::Changed));
        assert_eq!(sys.value, Some(0));
        assert_eq!(
            read_marker(&m),
            Some(Marker {
                version: 1,
                original_value: 2,
                key_existed: true
            })
        );
        assert!(restore_with(&mut sys, &m));
        assert_eq!(sys.value, Some(2));
        assert!(sys.key);
        assert!(!m.exists());
    }

    #[test]
    fn an_unset_default_is_cleared_again_on_restore() {
        let (_d, m) = marker();
        // Unset key: macOS reports its computed default (Emoji).
        let mut sys = Fake::new(Some(2), false);
        apply_with(&mut sys, &m).unwrap();
        assert!(restore_with(&mut sys, &m));
        assert!(!sys.key, "the key tracks macOS defaults again");
    }

    #[test]
    fn do_nothing_already_needs_no_marker() {
        let (_d, m) = marker();
        let mut sys = Fake::new(Some(0), true);
        assert_eq!(apply_with(&mut sys, &m), Ok(Applied::AlreadyDoNothing));
        assert!(sys.updates.is_empty());
        assert!(!m.exists());
        assert!(!restore_with(&mut sys, &m));
    }

    #[test]
    fn the_users_later_choice_wins_over_the_marker() {
        let (_d, m) = marker();
        let mut sys = Fake::new(Some(1), true);
        apply_with(&mut sys, &m).unwrap();
        // The user picked Start Dictation in System Settings meanwhile.
        sys.value = Some(3);
        assert!(!restore_with(&mut sys, &m));
        assert_eq!(sys.value, Some(3));
        assert!(!m.exists());
    }

    #[test]
    fn a_crash_leftover_keeps_the_users_original_value() {
        let (_d, m) = marker();
        let mut sys = Fake::new(Some(2), true);
        apply_with(&mut sys, &m).unwrap();
        // Crash; the next run applies again over its own Do Nothing value
        // after the user switched back to Emoji by hand.
        sys.value = Some(1);
        apply_with(&mut sys, &m).unwrap();
        assert_eq!(read_marker(&m).unwrap().original_value, 2);
    }

    #[test]
    fn missing_entry_points_and_ignored_updates_fail_without_a_change() {
        let (_d, m) = marker();
        let mut sys = Fake::new(None, true);
        assert!(apply_with(&mut sys, &m).is_err());
        assert!(!m.exists());
        let mut sys = Fake::new(Some(2), true);
        sys.ignore = true;
        assert!(apply_with(&mut sys, &m).is_err());
        // The marker stays: a later restore is harmless (value not ours).
        assert!(!restore_with(&mut sys, &m));
        assert_eq!(sys.value, Some(2));
    }

    #[test]
    fn a_damaged_marker_is_dropped() {
        let (_d, m) = marker();
        std::fs::create_dir_all(m.parent().unwrap()).unwrap();
        std::fs::write(&m, b"{not json").unwrap();
        assert_eq!(read_marker(&m), None);
        assert!(!m.exists());
    }
}
