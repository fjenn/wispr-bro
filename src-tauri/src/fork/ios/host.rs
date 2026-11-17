// SPDX-License-Identifier: GPL-3.0-or-later
//
//! Calls into the Swift session code in the same process
//! (`ios-native/WBSession/wb_ios.h`). Until those sources are in the app
//! target the weak C stand-ins in `swift_stubs.c` answer ("no session").

use std::ffi::{c_char, CStr};

extern "C" {
    fn wb_ios_session_state_json() -> *mut c_char;
    fn wb_ios_string_free(s: *mut c_char);
    fn wb_ios_start_session();
    fn wb_ios_end_session();
    fn wb_ios_keyboard_enabled() -> i32;
    fn wb_ios_full_access_known() -> i32;
    fn wb_ios_mic_permission() -> i32;
    fn wb_ios_request_mic_permission();
    fn wb_ios_open_settings();
}

/// The session state as the Swift side publishes it (a JSON object).
pub fn session_state() -> serde_json::Value {
    // SAFETY: the function returns a malloc'd NUL-terminated string (or
    // null) that we free with its own free function.
    let json = unsafe {
        let pointer = wb_ios_session_state_json();
        if pointer.is_null() {
            return serde_json::Value::Null;
        }
        let json = CStr::from_ptr(pointer).to_string_lossy().into_owned();
        wb_ios_string_free(pointer);
        json
    };
    serde_json::from_str(&json).unwrap_or(serde_json::Value::Null)
}

pub fn start_session() {
    // SAFETY: no arguments; safe from any thread (wb_ios.h).
    unsafe { wb_ios_start_session() }
}

pub fn end_session() {
    // SAFETY: as above.
    unsafe { wb_ios_end_session() }
}

pub fn keyboard_enabled() -> bool {
    // SAFETY: as above.
    unsafe { wb_ios_keyboard_enabled() == 1 }
}

/// `Some(on)` once the keyboard has reported Full Access, else `None`.
pub fn full_access() -> Option<bool> {
    // SAFETY: as above.
    match unsafe { wb_ios_full_access_known() } {
        1 => Some(true),
        0 => Some(false),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MicPermission {
    Undetermined,
    Denied,
    Granted,
}

pub fn mic_permission() -> MicPermission {
    // Swift's numbering (wb_ios.h): 0 undetermined, 1 granted, 2 denied.
    // SAFETY: as above.
    match unsafe { wb_ios_mic_permission() } {
        1 => MicPermission::Granted,
        2 => MicPermission::Denied,
        _ => MicPermission::Undetermined,
    }
}

pub fn request_mic_permission() {
    // SAFETY: as above.
    unsafe { wb_ios_request_mic_permission() }
}

pub fn open_settings() {
    // SAFETY: as above.
    unsafe { wb_ios_open_settings() }
}

extern "C" {
    fn wb_ios_app_state() -> i32;
}

/// Where the app is: `"active"`, `"inactive"` or `"background"` (`scene.m`).
pub fn app_state() -> &'static str {
    // SAFETY: reads one atomic int.
    match unsafe { wb_ios_app_state() } {
        0 => "active",
        2 => "background",
        _ => "inactive",
    }
}
