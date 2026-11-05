// SPDX-License-Identifier: GPL-3.0-or-later
//
// Answers the macOS Microphone prompt for the RS-11 test app: finds an
// on-screen window whose AX tree has an "Allow" button and presses it.

use std::ffi::c_void;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use serde_json::{json, Value};

use crate::ffi::{cfstr, from_cfstr, CFRelease, CFTypeRef};
use crate::sleep_ms;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGWindowListCopyWindowInfo(opt: u32, rel: u32) -> CFTypeRef;
}
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFArrayGetCount(a: CFTypeRef) -> isize;
    fn CFArrayGetValueAtIndex(a: CFTypeRef, i: isize) -> CFTypeRef;
    fn CFDictionaryGetValue(d: CFTypeRef, k: CFTypeRef) -> CFTypeRef;
    fn CFNumberGetValue(n: CFTypeRef, t: i32, out: *mut c_void) -> bool;
}
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
    fn AXUIElementCopyAttributeValue(e: CFTypeRef, a: CFTypeRef, out: *mut CFTypeRef) -> i32;
    fn AXUIElementPerformAction(e: CFTypeRef, a: CFTypeRef) -> i32;
}

/// (pid, owner name) of every on-screen window's owner.
fn window_owners() -> Vec<(i32, String)> {
    let mut v = Vec::new();
    unsafe {
        let arr = CGWindowListCopyWindowInfo(1, 0);
        if arr.is_null() {
            return v;
        }
        let kp = cfstr("kCGWindowOwnerPID");
        let kn = cfstr("kCGWindowOwnerName");
        for i in 0..CFArrayGetCount(arr) {
            let d = CFArrayGetValueAtIndex(arr, i);
            let mut pid: i32 = 0;
            let n = CFDictionaryGetValue(d, kp);
            if !n.is_null() {
                CFNumberGetValue(n, 3, &mut pid as *mut i32 as *mut c_void);
            }
            let name = from_cfstr(CFDictionaryGetValue(d, kn)).unwrap_or_default();
            if !v.iter().any(|(p, _)| *p == pid) {
                v.push((pid, name));
            }
        }
        CFRelease(kp);
        CFRelease(kn);
        CFRelease(arr);
    }
    v
}

fn copy(e: CFTypeRef, attr: &str) -> CFTypeRef {
    let a = cfstr(attr);
    let mut v: CFTypeRef = std::ptr::null();
    let err = unsafe { AXUIElementCopyAttributeValue(e, a, &mut v) };
    unsafe { CFRelease(a) };
    if err != 0 {
        std::ptr::null()
    } else {
        v
    }
}

/// Depth-first search for a button titled `title`; presses it.
fn press_button(e: CFTypeRef, title: &str, depth: usize) -> bool {
    if depth > 12 {
        return false;
    }
    let role = copy(e, "AXRole");
    let is_button = from_cfstr(role).as_deref() == Some("AXButton");
    if !role.is_null() {
        unsafe { CFRelease(role) };
    }
    if is_button {
        let t = copy(e, "AXTitle");
        let matches = from_cfstr(t).as_deref() == Some(title);
        if !t.is_null() {
            unsafe { CFRelease(t) };
        }
        if matches {
            let a = cfstr("AXPress");
            let err = unsafe { AXUIElementPerformAction(e, a) };
            unsafe { CFRelease(a) };
            return err == 0;
        }
    }
    let kids = copy(e, "AXChildren");
    if kids.is_null() {
        return false;
    }
    let mut hit = false;
    unsafe {
        for i in 0..CFArrayGetCount(kids) {
            if press_button(CFArrayGetValueAtIndex(kids, i), title, depth + 1) {
                hit = true;
                break;
            }
        }
        CFRelease(kids);
    }
    hit
}

/// Wait up to `timeout_ms` for a window with an "Allow" button (the TCC
/// prompt) and press it. Returns what happened.
pub fn allow_mic_prompt(out: &Path, timeout_ms: u64) -> Value {
    let t0 = Instant::now();
    let before: Vec<i32> = window_owners().into_iter().map(|(p, _)| p).collect();
    let mut shot = false;
    while t0.elapsed().as_millis() < timeout_ms as u128 {
        if out.join("perm-recording").exists() {
            return json!({"pressed": false, "note": "recording started without a prompt"});
        }
        for (pid, name) in window_owners() {
            if before.contains(&pid) && name != "UserNotificationCenter" {
                continue;
            }
            if !shot {
                let _ = Command::new("screencapture")
                    .args(["-x", "-o"])
                    .arg(out.join("perm-prompt.png"))
                    .status();
                shot = true;
            }
            let app = unsafe { AXUIElementCreateApplication(pid) };
            let pressed = press_button(app, "Allow", 0);
            unsafe { CFRelease(app) };
            if pressed {
                return json!({"pressed": true, "owner": name,
                              "after_ms": t0.elapsed().as_millis() as u64});
            }
        }
        sleep_ms(250);
    }
    json!({"pressed": false, "note": "no Allow button found",
           "new_owners": window_owners().into_iter().filter(|(p, _)| !before.contains(p))
               .map(|(_, n)| n).collect::<Vec<_>>()})
}

fn attr_str(e: CFTypeRef, attr: &str) -> String {
    let v = copy(e, attr);
    let s = from_cfstr(v).unwrap_or_default();
    if !v.is_null() {
        unsafe { CFRelease(v) };
    }
    s
}

/// Depth-first: dump the tree (role, title, description) into `dump`, and
/// press the first switch / checkbox whose label mentions `label`.
fn press_switch(e: CFTypeRef, label: &str, depth: usize, dump: &mut Vec<String>) -> bool {
    if depth > 30 {
        return false;
    }
    let role = attr_str(e, "AXRole");
    let title = attr_str(e, "AXTitle");
    let desc = attr_str(e, "AXDescription");
    let ident = attr_str(e, "AXIdentifier");
    dump.push(format!(
        "{}{role} | {title} | {desc} | {ident}",
        " ".repeat(depth)
    ));
    if (role == "AXCheckBox" || role == "AXSwitch")
        && (ident == format!("{label}_Toggle") || title == label || desc == label)
    {
        let a = cfstr("AXPress");
        let err = unsafe { AXUIElementPerformAction(e, a) };
        unsafe { CFRelease(a) };
        dump.push(format!("PRESSED err={err}"));
        return err == 0;
    }
    let kids = copy(e, "AXChildren");
    if kids.is_null() {
        return false;
    }
    let mut hit = false;
    unsafe {
        for i in 0..CFArrayGetCount(kids) {
            if press_switch(CFArrayGetValueAtIndex(kids, i), label, depth + 1, dump) {
                hit = true;
                break;
            }
        }
        CFRelease(kids);
    }
    hit
}

/// Turn the app's Microphone switch off in System Settings › Privacy &
/// Security › Microphone, the way a user revokes it. Quits System Settings
/// afterwards if it was not running before.
pub fn revoke_in_settings(out: &Path, label: &str) -> Value {
    let was_running = window_owners().iter().any(|(_, n)| n == "System Settings");
    let t0 = Instant::now();
    let _ = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
        .status();
    let mut result = json!({"pressed": false});
    for _ in 0..40 {
        sleep_ms(250);
        let Some((pid, _)) = window_owners()
            .into_iter()
            .find(|(_, n)| n == "System Settings")
        else {
            continue;
        };
        let app = unsafe { AXUIElementCreateApplication(pid) };
        let mut dump = Vec::new();
        let pressed = press_switch(app, label, 0, &mut dump);
        unsafe { CFRelease(app) };
        let _ = std::fs::write(out.join("settings-ax.txt"), dump.join("\n"));
        if pressed {
            result = json!({"pressed": true, "after_ms": t0.elapsed().as_millis() as u64});
            break;
        }
    }
    sleep_ms(800);
    let _ = Command::new("screencapture")
        .args(["-x", "-o"])
        .arg(out.join("settings-after.png"))
        .status();
    if !was_running {
        let _ = Command::new("pkill")
            .args(["-x", "System Settings"])
            .status();
    }
    result
}
