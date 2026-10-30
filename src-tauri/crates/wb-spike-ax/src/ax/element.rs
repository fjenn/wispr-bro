// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Abhishek Barali (SpeakoFlow; MIT, notice below)
// Modified by Wispr Bro contributors, 2026-10-30.
//
// Ported from SpeakoFlow `src-tauri/src/smart_spacing/macos.rs`
// (https://github.com/AbhishekBarali/SpeakoFlow, commit
// 9818a17fcd124e5f1ea6d34780ddea9b154322d8), MIT. Kept: the AX helpers
// (`attr`, `attr_string`, `attr_element`, `attr_bool`, `attr_isize`,
// `attr_range`, `string_for_range` with its length check, `element_pid`,
// `set_timeout`, `set_manual_accessibility`), the app helpers
// (`app_bundle_path`, `bundle_name`, `is_terminal`,
// `builds_tree_on_request`) and Secure Input. Changed: the timeout is a
// parameter (RS-04 measures 0.1 s and 0.25 s); `set_manual_accessibility`
// returns the AXError. Added: `attr_raw` keeping the AXError, `attr_url`,
// `attr_rect`/`bounds_for_range` (CGRect), `is_settable`, `set_string`,
// `set_range`, `children`, `copy_names`, `parameterized`, `error_name`.
// SpeakoFlow's read logic (caret window, Chromium quirks) is in probe.rs.
//
// MIT License
//
// Copyright (c) 2026 Abhishek Barali
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

//! Thin, safe-ish wrappers over the AX C API (objc2-application-services).
//! Every call is a synchronous message answered by the target app's main
//! thread: call these only from the AX thread (`thread.rs`), never from a UI
//! thread. Attribute names are string literals (the crate has no constants).

use std::ffi::c_void;
use std::ptr::NonNull;

use objc2_application_services::{AXError, AXUIElement, AXValue, AXValueType};
use objc2_core_foundation::{
    CFArray, CFBoolean, CFNumber, CFRange, CFRetained, CFString, CFType, CGPoint, CGRect, CGSize,
    CFURL,
};

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    fn IsSecureEventInputEnabled() -> u8;
}

/// True while any process holds Secure Event Input (a focused password
/// field anywhere, Terminal's Secure Keyboard Entry).
pub fn secure_input_on() -> bool {
    // SAFETY: no arguments; reads global state.
    unsafe { IsSecureEventInputEnabled() != 0 }
}

/* ───────────────────────────── AX helpers ───────────────────────────── */

pub fn attr(element: &AXUIElement, name: &str) -> Result<CFRetained<CFType>, AXError> {
    let name = CFString::from_str(name);
    let mut value: *const CFType = std::ptr::null();
    // SAFETY: `value` is a valid out-pointer; on success it holds a +1 reference.
    let error = unsafe { element.copy_attribute_value(&name, NonNull::from(&mut value)) };
    if error != AXError::Success {
        return Err(error);
    }
    NonNull::new(value as *mut CFType)
        // SAFETY: the copy rule hands us ownership of one reference.
        .map(|ptr| unsafe { CFRetained::from_raw(ptr) })
        .ok_or(AXError::NoValue)
}

pub fn attr_string(element: &AXUIElement, name: &str) -> Option<String> {
    let value = attr(element, name).ok()?;
    value.downcast_ref::<CFString>().map(|s| s.to_string())
}

pub fn attr_element(element: &AXUIElement, name: &str) -> Option<CFRetained<AXUIElement>> {
    attr(element, name).ok()?.downcast::<AXUIElement>().ok()
}

pub fn attr_bool(element: &AXUIElement, name: &str) -> Option<bool> {
    let value = attr(element, name).ok()?;
    value.downcast_ref::<CFBoolean>().map(|b| b.as_bool())
}

#[allow(dead_code)] // kept from the port; S-05 lifts the full helper set
pub fn attr_isize(element: &AXUIElement, name: &str) -> Option<isize> {
    let value = attr(element, name).ok()?;
    value
        .downcast_ref::<CFNumber>()
        .and_then(|n| n.as_i64())
        .map(|n| n as isize)
}

/// A `CFRange` out of an `AXValue`.
pub fn value_range(value: &CFType) -> Option<CFRange> {
    let value = value.downcast_ref::<AXValue>()?;
    let mut range = CFRange {
        location: 0,
        length: 0,
    };
    // SAFETY: `range` matches the CFRange AXValue type.
    let ok = unsafe {
        value.value(
            AXValueType::CFRange,
            NonNull::from(&mut range).cast::<c_void>(),
        )
    };
    ok.then_some(range)
}

pub fn attr_range(element: &AXUIElement, name: &str) -> Option<CFRange> {
    value_range(&*attr(element, name).ok()?)
}

fn value_rect(value: &CFType) -> Option<CGRect> {
    let value = value.downcast_ref::<AXValue>()?;
    let mut rect = CGRect::default();
    // SAFETY: `rect` matches the CGRect AXValue type.
    let ok = unsafe {
        value.value(
            AXValueType::CGRect,
            NonNull::from(&mut rect).cast::<c_void>(),
        )
    };
    ok.then_some(rect)
}

/// The element's frame from `AXPosition` + `AXSize` (screen points).
pub fn attr_rect(element: &AXUIElement) -> Option<CGRect> {
    let pos = attr(element, "AXPosition").ok()?;
    let size = attr(element, "AXSize").ok()?;
    let (pos, size) = (
        pos.downcast_ref::<AXValue>()?,
        size.downcast_ref::<AXValue>()?,
    );
    let mut p = CGPoint::default();
    let mut s = CGSize::default();
    // SAFETY: the out values match the AXValue types asked for.
    let ok = unsafe {
        pos.value(AXValueType::CGPoint, NonNull::from(&mut p).cast::<c_void>())
            && size.value(AXValueType::CGSize, NonNull::from(&mut s).cast::<c_void>())
    };
    ok.then_some(CGRect { origin: p, size: s })
}

/// A parameterized attribute with a `CFRange` parameter.
pub fn parameterized_range(
    element: &AXUIElement,
    name: &str,
    range: CFRange,
) -> Result<CFRetained<CFType>, AXError> {
    // SAFETY: `range` outlives the call; AXValueCreate copies it.
    let parameter =
        unsafe { AXValue::new(AXValueType::CFRange, NonNull::from(&range).cast::<c_void>()) }
            .ok_or(AXError::Failure)?;
    parameterized(element, name, parameter.as_ref())
}

/// Any parameterized attribute.
pub fn parameterized(
    element: &AXUIElement,
    name: &str,
    parameter: &CFType,
) -> Result<CFRetained<CFType>, AXError> {
    let name = CFString::from_str(name);
    let mut value: *const CFType = std::ptr::null();
    // SAFETY: valid out-pointer; +1 reference on success.
    let error = unsafe {
        element.copy_parameterized_attribute_value(&name, parameter, NonNull::from(&mut value))
    };
    if error != AXError::Success {
        return Err(error);
    }
    NonNull::new(value as *mut CFType)
        // SAFETY: copy rule.
        .map(|ptr| unsafe { CFRetained::from_raw(ptr) })
        .ok_or(AXError::NoValue)
}

pub fn string_for_range(element: &AXUIElement, range: CFRange) -> Option<Vec<u16>> {
    let value = parameterized_range(element, "AXStringForRange", range).ok()?;
    let text = value.downcast_ref::<CFString>()?.to_string();
    let units: Vec<u16> = text.encode_utf16().collect();
    // An app that answers with a different length than asked is not trusted.
    (units.len() as isize == range.length).then_some(units)
}

pub fn bounds_for_range(element: &AXUIElement, range: CFRange) -> Result<CGRect, AXError> {
    let value = parameterized_range(element, "AXBoundsForRange", range)?;
    value_rect(&value).ok_or(AXError::IllegalArgument)
}

pub fn element_pid(element: &AXUIElement) -> Option<i32> {
    let mut pid: libc::pid_t = 0;
    // SAFETY: valid out-pointer.
    let error = unsafe { element.pid(NonNull::from(&mut pid)) };
    (error == AXError::Success && pid > 0).then_some(pid)
}

pub fn set_timeout(element: &AXUIElement, secs: f32) {
    // SAFETY: plain setter.
    unsafe {
        let _ = element.set_messaging_timeout(secs);
    }
}

/// Switches Chromium/Electron's on-request accessibility tree on or off.
/// Never `AXEnhancedUserInterface`: it flips an app into screen-reader mode
/// for the rest of its life (and breaks window animations).
pub fn set_manual_accessibility(app: &AXUIElement, on: bool) -> AXError {
    let name = CFString::from_str("AXManualAccessibility");
    let value: &CFType = CFBoolean::new(on).as_ref();
    // SAFETY: CFBoolean is a valid CFType value.
    unsafe { app.set_attribute_value(&name, value) }
}

pub fn is_settable(element: &AXUIElement, name: &str) -> Result<bool, AXError> {
    let name = CFString::from_str(name);
    let mut settable: u8 = 0;
    // SAFETY: valid out-pointer.
    let error = unsafe { element.is_attribute_settable(&name, NonNull::from(&mut settable)) };
    if error == AXError::Success {
        Ok(settable != 0)
    } else {
        Err(error)
    }
}

pub fn set_string(element: &AXUIElement, name: &str, text: &str) -> AXError {
    let name = CFString::from_str(name);
    let value = CFString::from_str(text);
    // SAFETY: CFString is a valid CFType value.
    unsafe { element.set_attribute_value(&name, value.as_ref()) }
}

pub fn set_range(element: &AXUIElement, name: &str, range: CFRange) -> AXError {
    let name = CFString::from_str(name);
    // SAFETY: AXValueCreate copies `range`.
    let Some(value) =
        (unsafe { AXValue::new(AXValueType::CFRange, NonNull::from(&range).cast::<c_void>()) })
    else {
        return AXError::Failure;
    };
    // SAFETY: AXValue is a valid CFType value.
    unsafe { element.set_attribute_value(&name, value.as_ref()) }
}

/// `AXURL` (a CFURL) or a string URL, as text.
pub fn attr_url(element: &AXUIElement, name: &str) -> Option<String> {
    let value = attr(element, name).ok()?;
    if let Some(url) = value.downcast_ref::<CFURL>() {
        return Some(url.string().to_string());
    }
    value.downcast_ref::<CFString>().map(|s| s.to_string())
}

/// `AXChildren` as elements.
pub fn children(element: &AXUIElement) -> Vec<CFRetained<AXUIElement>> {
    let Ok(value) = attr(element, "AXChildren") else {
        return Vec::new();
    };
    let Some(array) = value.downcast_ref::<CFArray>() else {
        return Vec::new();
    };
    // SAFETY: AXChildren is documented as an array of AXUIElements; each
    // entry is checked by downcast anyway.
    let array: &CFArray<CFType> = unsafe { array.cast_unchecked() };
    (0..array.len())
        .filter_map(|i| array.get(i))
        .filter_map(|v| v.downcast::<AXUIElement>().ok())
        .collect()
}

/// Attribute (or parameterized attribute) names an element supports.
pub fn copy_names(element: &AXUIElement, parameterized: bool) -> Vec<String> {
    let mut names: *const CFArray = std::ptr::null();
    // SAFETY: valid out-pointer; +1 reference on success.
    let error = unsafe {
        if parameterized {
            element.copy_parameterized_attribute_names(NonNull::from(&mut names))
        } else {
            element.copy_attribute_names(NonNull::from(&mut names))
        }
    };
    if error != AXError::Success {
        return Vec::new();
    }
    let Some(ptr) = NonNull::new(names as *mut CFArray) else {
        return Vec::new();
    };
    // SAFETY: copy rule.
    let array = unsafe { CFRetained::from_raw(ptr) };
    // SAFETY: an array of CFStrings; checked by downcast.
    let array: &CFArray<CFType> = unsafe { array.cast_unchecked() };
    (0..array.len())
        .filter_map(|i| array.get(i))
        .filter_map(|v| v.downcast_ref::<CFString>().map(|s| s.to_string()))
        .collect()
}

pub fn error_name(e: AXError) -> &'static str {
    match e {
        AXError::Success => "Success",
        AXError::Failure => "Failure",
        AXError::IllegalArgument => "IllegalArgument",
        AXError::InvalidUIElement => "InvalidUIElement",
        AXError::InvalidUIElementObserver => "InvalidUIElementObserver",
        AXError::CannotComplete => "CannotComplete",
        AXError::AttributeUnsupported => "AttributeUnsupported",
        AXError::ActionUnsupported => "ActionUnsupported",
        AXError::NotificationUnsupported => "NotificationUnsupported",
        AXError::NotImplemented => "NotImplemented",
        AXError::NotificationAlreadyRegistered => "NotificationAlreadyRegistered",
        AXError::NotificationNotRegistered => "NotificationNotRegistered",
        AXError::APIDisabled => "APIDisabled",
        AXError::NoValue => "NoValue",
        AXError::ParameterizedAttributeUnsupported => "ParameterizedAttributeUnsupported",
        AXError::NotEnoughPrecision => "NotEnoughPrecision",
        _ => "Other",
    }
}

/* ───────────────────────────── the app ───────────────────────────── */

pub fn app_bundle_path(pid: i32) -> Option<String> {
    let mut buffer = vec![0u8; 4096];
    // SAFETY: the buffer is 4096 bytes long, as passed.
    let len =
        unsafe { libc::proc_pidpath(pid, buffer.as_mut_ptr() as *mut c_void, buffer.len() as u32) };
    if len <= 0 {
        return None;
    }
    let path = String::from_utf8_lossy(&buffer[..len as usize]).into_owned();
    bundle_of_executable(&path)
}

/// ".../Foo.app/Contents/MacOS/Foo" -> ".../Foo.app" (the outermost bundle,
/// so a helper inside Foo.app still maps to Foo.app).
pub fn bundle_of_executable(path: &str) -> Option<String> {
    let end = path.find(".app/").map(|i| i + 4)?;
    Some(path[..end].to_string())
}

pub fn bundle_name(bundle: &str) -> String {
    bundle
        .rsplit('/')
        .next()
        .unwrap_or("")
        .trim_end_matches(".app")
        .to_ascii_lowercase()
}

pub fn is_terminal(name: &str) -> bool {
    matches!(
        name,
        "terminal"
            | "iterm"
            | "iterm2"
            | "warp"
            | "ghostty"
            | "kitty"
            | "alacritty"
            | "wezterm"
            | "hyper"
            | "tabby"
            | "cmux"
    )
}

pub fn is_chromium(name: &str) -> bool {
    matches!(
        name,
        "google chrome"
            | "google chrome beta"
            | "google chrome canary"
            | "chromium"
            | "arc"
            | "brave browser"
            | "microsoft edge"
            | "vivaldi"
            | "opera"
            | "dia"
            | "comet"
    )
}

pub fn is_electron(bundle: &str) -> bool {
    std::path::Path::new(bundle)
        .join("Contents/Frameworks/Electron Framework.framework")
        .exists()
}

/// Apps that only build their accessibility tree on request.
#[allow(dead_code)] // kept from the port (probe::family_of splits it by family)
pub fn builds_tree_on_request(bundle: &str, name: &str) -> bool {
    is_chromium(name) || is_electron(bundle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_paths() {
        assert_eq!(
            bundle_of_executable("/Applications/Foo.app/Contents/MacOS/Foo").as_deref(),
            Some("/Applications/Foo.app")
        );
        assert_eq!(
            bundle_of_executable(
                "/Applications/Google Chrome.app/Contents/Frameworks/x.framework/Helpers/H.app/Contents/MacOS/H"
            )
            .as_deref(),
            Some("/Applications/Google Chrome.app")
        );
        assert_eq!(bundle_of_executable("/usr/bin/true"), None);
        assert_eq!(
            bundle_name("/Applications/Google Chrome.app"),
            "google chrome"
        );
    }

    #[test]
    fn families() {
        assert!(is_terminal("terminal") && is_terminal("iterm2"));
        assert!(!is_terminal("textedit"));
        assert!(is_chromium("google chrome") && is_chromium("arc"));
        assert!(!is_chromium("safari"));
    }
}
