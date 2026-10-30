// SPDX-License-Identifier: GPL-3.0-or-later
//
// Thin CoreGraphics / CoreFoundation / Accessibility FFI for the RS-03 driver:
// synthetic mouse and key events (posted at the HID tap, like hardware),
// window list queries, display bounds and one AX read of TextEdit's text.

#![allow(non_upper_case_globals, clippy::upper_case_acronyms)]

use std::ffi::{c_char, c_void, CStr, CString};

pub type CFTypeRef = *const c_void;
type CGEventRef = *mut c_void;
type CFStringRef = *const c_void;
type CFArrayRef = *const c_void;
type CFDictionaryRef = *const c_void;
type AXUIElementRef = *const c_void;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct CGSize {
    pub width: f64,
    pub height: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

impl CGRect {
    pub fn contains_rect(&self, o: &CGRect) -> bool {
        o.origin.x >= self.origin.x - 0.5
            && o.origin.y >= self.origin.y - 0.5
            && o.origin.x + o.size.width <= self.origin.x + self.size.width + 0.5
            && o.origin.y + o.size.height <= self.origin.y + self.size.height + 0.5
    }
    pub fn contains_point(&self, p: CGPoint) -> bool {
        p.x >= self.origin.x
            && p.y >= self.origin.y
            && p.x < self.origin.x + self.size.width
            && p.y < self.origin.y + self.size.height
    }
    pub fn mid_x(&self) -> f64 {
        self.origin.x + self.size.width / 2.0
    }
    pub fn max_y(&self) -> f64 {
        self.origin.y + self.size.height
    }
}

const kCGHIDEventTap: u32 = 0;
const kCGEventLeftMouseDown: u32 = 1;
const kCGEventLeftMouseUp: u32 = 2;
const kCGEventMouseMoved: u32 = 5;
const kCGMouseEventClickState: u32 = 1;
pub const kCGEventFlagMaskControl: u64 = 0x0004_0000;
const kCFStringEncodingUTF8: u32 = 0x0800_0100;
const kCGWindowListOptionOnScreenOnly: u32 = 1;
const kCGNullWindowID: u32 = 0;
const kCFNumberDoubleType: i32 = 13;
const kCFNumberSInt64Type: i32 = 4;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventCreateMouseEvent(src: *const c_void, t: u32, p: CGPoint, b: u32) -> CGEventRef;
    fn CGEventCreateKeyboardEvent(src: *const c_void, key: u16, down: bool) -> CGEventRef;
    fn CGEventSetFlags(e: CGEventRef, flags: u64);
    fn CGEventSetIntegerValueField(e: CGEventRef, field: u32, v: i64);
    fn CGEventPost(tap: u32, e: CGEventRef);
    fn CGEventCreate(src: *const c_void) -> CGEventRef;
    fn CGEventGetLocation(e: CGEventRef) -> CGPoint;
    fn CGWarpMouseCursorPosition(p: CGPoint) -> i32;
    fn CGWindowListCopyWindowInfo(opt: u32, rel: u32) -> CFArrayRef;
    fn CGGetActiveDisplayList(max: u32, out: *mut u32, count: *mut u32) -> i32;
    fn CGDisplayBounds(d: u32) -> CGRect;
    fn CGMainDisplayID() -> u32;
    fn CGRectMakeWithDictionaryRepresentation(d: CFDictionaryRef, r: *mut CGRect) -> bool;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(o: CFTypeRef);
    fn CFArrayGetCount(a: CFArrayRef) -> isize;
    fn CFArrayGetValueAtIndex(a: CFArrayRef, i: isize) -> CFTypeRef;
    fn CFDictionaryGetValue(d: CFDictionaryRef, k: CFTypeRef) -> CFTypeRef;
    fn CFStringCreateWithCString(a: *const c_void, s: *const c_char, enc: u32) -> CFStringRef;
    fn CFStringGetCString(s: CFStringRef, buf: *mut c_char, len: isize, enc: u32) -> bool;
    fn CFStringGetLength(s: CFStringRef) -> isize;
    fn CFNumberGetValue(n: CFTypeRef, t: i32, out: *mut c_void) -> bool;
    fn CFGetTypeID(o: CFTypeRef) -> usize;
    fn CFStringGetTypeID() -> usize;
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(e: AXUIElementRef, a: CFStringRef, out: *mut CFTypeRef)
        -> i32;
    fn AXUIElementSetAttributeValue(e: AXUIElementRef, a: CFStringRef, v: CFTypeRef) -> i32;
}

extern "C" {
    static kCFBooleanTrue: CFTypeRef;
    static kCFBooleanFalse: CFTypeRef;
}

struct CfStr(CFStringRef);
impl CfStr {
    fn new(s: &str) -> Self {
        let c = CString::new(s).unwrap_or_default();
        Self(unsafe {
            CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), kCFStringEncodingUTF8)
        })
    }
}
impl Drop for CfStr {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) }
        }
    }
}

fn cf_string_to_rust(s: CFStringRef) -> Option<String> {
    if s.is_null() || unsafe { CFGetTypeID(s) != CFStringGetTypeID() } {
        return None;
    }
    let len = unsafe { CFStringGetLength(s) } * 4 + 1;
    let mut buf = vec![0 as c_char; len.max(1) as usize];
    if unsafe { CFStringGetCString(s, buf.as_mut_ptr(), len, kCFStringEncodingUTF8) } {
        Some(
            unsafe { CStr::from_ptr(buf.as_ptr()) }
                .to_string_lossy()
                .into_owned(),
        )
    } else {
        None
    }
}

fn post(e: CGEventRef) {
    if !e.is_null() {
        unsafe {
            CGEventPost(kCGHIDEventTap, e);
            CFRelease(e);
        }
    }
}

/// Current cursor position (global, top-left origin).
pub fn cursor() -> CGPoint {
    unsafe {
        let e = CGEventCreate(std::ptr::null());
        let p = CGEventGetLocation(e);
        CFRelease(e);
        p
    }
}

pub fn move_mouse(p: CGPoint) {
    post(unsafe { CGEventCreateMouseEvent(std::ptr::null(), kCGEventMouseMoved, p, 0) });
}

pub fn warp(p: CGPoint) {
    unsafe {
        CGWarpMouseCursorPosition(p);
    }
    move_mouse(p);
}

pub fn click(p: CGPoint) {
    move_mouse(p);
    std::thread::sleep(std::time::Duration::from_millis(30));
    for t in [kCGEventLeftMouseDown, kCGEventLeftMouseUp] {
        let e = unsafe { CGEventCreateMouseEvent(std::ptr::null(), t, p, 0) };
        unsafe { CGEventSetIntegerValueField(e, kCGMouseEventClickState, 1) };
        post(e);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

pub fn key(code: u16, flags: u64) {
    for down in [true, false] {
        let e = unsafe { CGEventCreateKeyboardEvent(std::ptr::null(), code, down) };
        unsafe { CGEventSetFlags(e, flags) };
        post(e);
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
}

/// Every active display: (id, bounds in global top-left coordinates).
pub fn displays() -> Vec<(u32, CGRect)> {
    let mut ids = [0u32; 16];
    let mut n = 0u32;
    unsafe { CGGetActiveDisplayList(16, ids.as_mut_ptr(), &mut n) };
    ids[..n as usize]
        .iter()
        .map(|&d| (d, unsafe { CGDisplayBounds(d) }))
        .collect()
}

pub fn main_display() -> u32 {
    unsafe { CGMainDisplayID() }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WinInfo {
    pub number: i64,
    pub pid: i64,
    pub owner: String,
    pub name: String,
    pub layer: i64,
    pub bounds: CGRect,
}

/// On-screen windows (CGWindowList, front to back).
pub fn onscreen_windows() -> Vec<WinInfo> {
    let mut out = Vec::new();
    unsafe {
        let arr = CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly, kCGNullWindowID);
        if arr.is_null() {
            return out;
        }
        let k_num = CfStr::new("kCGWindowNumber");
        let k_pid = CfStr::new("kCGWindowOwnerPID");
        let k_owner = CfStr::new("kCGWindowOwnerName");
        let k_name = CfStr::new("kCGWindowName");
        let k_layer = CfStr::new("kCGWindowLayer");
        let k_bounds = CfStr::new("kCGWindowBounds");
        for i in 0..CFArrayGetCount(arr) {
            let d = CFArrayGetValueAtIndex(arr, i);
            let num = |k: &CfStr| -> i64 {
                let v = CFDictionaryGetValue(d, k.0);
                let mut x: i64 = 0;
                if !v.is_null() {
                    CFNumberGetValue(v, kCFNumberSInt64Type, &mut x as *mut i64 as *mut c_void);
                }
                x
            };
            let mut bounds = CGRect::default();
            let b = CFDictionaryGetValue(d, k_bounds.0);
            if !b.is_null() {
                CGRectMakeWithDictionaryRepresentation(b, &mut bounds);
            }
            out.push(WinInfo {
                number: num(&k_num),
                pid: num(&k_pid),
                owner: cf_string_to_rust(CFDictionaryGetValue(d, k_owner.0)).unwrap_or_default(),
                name: cf_string_to_rust(CFDictionaryGetValue(d, k_name.0)).unwrap_or_default(),
                layer: num(&k_layer),
                bounds,
            });
        }
        CFRelease(arr);
    }
    let _ = kCFNumberDoubleType;
    out
}

fn ax_copy(e: AXUIElementRef, attr: &str) -> CFTypeRef {
    let a = CfStr::new(attr);
    let mut v: CFTypeRef = std::ptr::null();
    let err = unsafe { AXUIElementCopyAttributeValue(e, a.0, &mut v) };
    if err != 0 {
        return std::ptr::null();
    }
    v
}

/// The AXValue of the app's focused element (TextEdit's document text).
pub fn ax_focused_value(pid: i32) -> Option<String> {
    unsafe {
        let app = AXUIElementCreateApplication(pid);
        let el = ax_copy(app, "AXFocusedUIElement");
        CFRelease(app);
        if el.is_null() {
            return None;
        }
        let v = ax_copy(el, "AXValue");
        CFRelease(el);
        if v.is_null() {
            return None;
        }
        let s = cf_string_to_rust(v);
        CFRelease(v);
        s
    }
}

/// The title of the app's focused window.
pub fn ax_focused_window_title(pid: i32) -> Option<String> {
    unsafe {
        let app = AXUIElementCreateApplication(pid);
        let w = ax_copy(app, "AXFocusedWindow");
        CFRelease(app);
        if w.is_null() {
            return None;
        }
        let v = ax_copy(w, "AXTitle");
        CFRelease(w);
        if v.is_null() {
            return None;
        }
        let s = cf_string_to_rust(v);
        CFRelease(v);
        s
    }
}

/// Set AXFullScreen on the app's focused window.
pub fn ax_set_fullscreen(pid: i32, on: bool) -> bool {
    unsafe {
        let app = AXUIElementCreateApplication(pid);
        let w = ax_copy(app, "AXFocusedWindow");
        CFRelease(app);
        if w.is_null() {
            return false;
        }
        let a = CfStr::new("AXFullScreen");
        let err =
            AXUIElementSetAttributeValue(w, a.0, if on { kCFBooleanTrue } else { kCFBooleanFalse });
        CFRelease(w);
        err == 0
    }
}
