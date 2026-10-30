// SPDX-License-Identifier: GPL-3.0-or-later
//
// Raw CoreGraphics / CoreFoundation / Accessibility / CoreAudio calls for the
// RS-11 checks: synthetic HID events, one AX read, and a CoreAudio aggregate
// device that the checks create and destroy to play an unplugged mic.

#![allow(non_upper_case_globals)]

use std::ffi::{c_char, c_void, CStr, CString};

pub type CFTypeRef = *const c_void;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}

const kCGHIDEventTap: u32 = 0;
pub const kCGEventOtherMouseDown: u32 = 25;
pub const kCGEventOtherMouseUp: u32 = 26;
const kCGMouseEventButtonNumber: u32 = 3;
pub const FLAG_OPT: u64 = 0x0008_0000;
pub const FLAG_CTRL: u64 = 0x0004_0000;
pub const FLAG_CMD: u64 = 0x0010_0000;
const kCFStringEncodingUTF8: u32 = 0x0800_0100;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventCreateMouseEvent(src: *const c_void, t: u32, p: CGPoint, b: u32) -> *mut c_void;
    fn CGEventCreateKeyboardEvent(src: *const c_void, key: u16, down: bool) -> *mut c_void;
    fn CGEventSetFlags(e: *mut c_void, flags: u64);
    fn CGEventSetIntegerValueField(e: *mut c_void, field: u32, v: i64);
    fn CGEventPost(tap: u32, e: *mut c_void);
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    pub fn CFRelease(o: CFTypeRef);
    fn CFStringCreateWithCString(a: *const c_void, s: *const c_char, enc: u32) -> CFTypeRef;
    fn CFStringGetCString(s: CFTypeRef, buf: *mut c_char, len: isize, enc: u32) -> bool;
    fn CFStringGetLength(s: CFTypeRef) -> isize;
    fn CFGetTypeID(o: CFTypeRef) -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFDictionaryCreate(
        a: *const c_void,
        keys: *const CFTypeRef,
        values: *const CFTypeRef,
        n: isize,
        kcb: *const c_void,
        vcb: *const c_void,
    ) -> CFTypeRef;
    fn CFArrayCreate(
        a: *const c_void,
        values: *const CFTypeRef,
        n: isize,
        cb: *const c_void,
    ) -> CFTypeRef;
    static kCFTypeDictionaryKeyCallBacks: c_void;
    static kCFTypeDictionaryValueCallBacks: c_void;
    static kCFTypeArrayCallBacks: c_void;
    static kCFBooleanTrue: CFTypeRef;
    static kCFBooleanFalse: CFTypeRef;
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
    fn AXUIElementCopyAttributeValue(e: CFTypeRef, a: CFTypeRef, out: *mut CFTypeRef) -> i32;
}

#[repr(C)]
struct PropAddr {
    selector: u32,
    scope: u32,
    element: u32,
}

#[link(name = "CoreAudio", kind = "framework")]
extern "C" {
    fn AudioObjectGetPropertyData(
        id: u32,
        addr: *const PropAddr,
        qsize: u32,
        q: *const c_void,
        size: *mut u32,
        out: *mut c_void,
    ) -> i32;
    fn AudioObjectSetPropertyData(
        id: u32,
        addr: *const PropAddr,
        qsize: u32,
        q: *const c_void,
        size: u32,
        data: *const c_void,
    ) -> i32;
    fn AudioHardwareCreateAggregateDevice(desc: CFTypeRef, out: *mut u32) -> i32;
    fn AudioHardwareDestroyAggregateDevice(id: u32) -> i32;
}

const fn fourcc(s: &[u8; 4]) -> u32 {
    ((s[0] as u32) << 24) | ((s[1] as u32) << 16) | ((s[2] as u32) << 8) | s[3] as u32
}
const SYSTEM_OBJECT: u32 = 1;
const GLOB: u32 = fourcc(b"glob");
const DEFAULT_INPUT: u32 = fourcc(b"dIn ");
const DEVICE_UID: u32 = fourcc(b"uid ");

pub fn cfstr(s: &str) -> CFTypeRef {
    let c = CString::new(s).unwrap_or_default();
    unsafe { CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), kCFStringEncodingUTF8) }
}

pub fn from_cfstr(s: CFTypeRef) -> Option<String> {
    if s.is_null() || unsafe { CFGetTypeID(s) != CFStringGetTypeID() } {
        return None;
    }
    let len = unsafe { CFStringGetLength(s) } * 4 + 1;
    let mut buf = vec![0 as c_char; len.max(1) as usize];
    unsafe { CFStringGetCString(s, buf.as_mut_ptr(), len, kCFStringEncodingUTF8) }.then(|| {
        unsafe { CStr::from_ptr(buf.as_ptr()) }
            .to_string_lossy()
            .into_owned()
    })
}

fn post(e: *mut c_void) {
    if !e.is_null() {
        unsafe {
            CGEventPost(kCGHIDEventTap, e);
            CFRelease(e);
        }
    }
}

/// One other-mouse-button event (button 2 = middle, 3/4 = side) at `p`.
pub fn other_mouse(kind: u32, button: u32, p: CGPoint, flags: u64) {
    let e = unsafe { CGEventCreateMouseEvent(std::ptr::null(), kind, p, button) };
    unsafe {
        CGEventSetIntegerValueField(e, kCGMouseEventButtonNumber, button as i64);
        CGEventSetFlags(e, flags);
    }
    post(e);
}

fn key_event(code: u16, down: bool, flags: u64) {
    let e = unsafe { CGEventCreateKeyboardEvent(std::ptr::null(), code, down) };
    unsafe { CGEventSetFlags(e, flags) };
    post(e);
}

/// Press `code` with the given modifiers held, the way a keyboard does it:
/// modifier key down (FlagsChanged), key down/up with the flags, modifier up.
pub fn chord(code: u16, mods: &[(u16, u64)]) {
    let ms = |n| std::thread::sleep(std::time::Duration::from_millis(n));
    let mut flags = 0;
    for &(mk, mf) in mods {
        flags |= mf;
        key_event(mk, true, flags);
        ms(15);
    }
    key_event(code, true, flags);
    ms(15);
    key_event(code, false, flags);
    ms(15);
    for &(mk, mf) in mods.iter().rev() {
        flags &= !mf;
        key_event(mk, false, flags);
        ms(15);
    }
}

/// Click an other-mouse button at `p` with the given modifiers held.
pub fn chord_mouse(button: u32, p: CGPoint, mods: &[(u16, u64)]) {
    let ms = |n| std::thread::sleep(std::time::Duration::from_millis(n));
    let mut flags = 0;
    for &(mk, mf) in mods {
        flags |= mf;
        key_event(mk, true, flags);
        ms(15);
    }
    other_mouse(kCGEventOtherMouseDown, button, p, flags);
    ms(40);
    other_mouse(kCGEventOtherMouseUp, button, p, flags);
    ms(15);
    for &(mk, mf) in mods.iter().rev() {
        flags &= !mf;
        key_event(mk, false, flags);
        ms(15);
    }
}

pub const OPT: (u16, u64) = (58, FLAG_OPT);
pub const CTRL: (u16, u64) = (59, FLAG_CTRL);
pub const CMD: (u16, u64) = (55, FLAG_CMD);

fn ax_copy(e: CFTypeRef, attr: &str) -> CFTypeRef {
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

fn ax_string(pid: i32, path: &[&str]) -> Option<String> {
    unsafe {
        let mut cur = AXUIElementCreateApplication(pid);
        for attr in path {
            let next = ax_copy(cur, attr);
            CFRelease(cur);
            if next.is_null() {
                return None;
            }
            cur = next;
        }
        let s = from_cfstr(cur);
        CFRelease(cur);
        s
    }
}

pub fn ax_focused_value(pid: i32) -> Option<String> {
    ax_string(pid, &["AXFocusedUIElement", "AXValue"])
}

pub fn ax_focused_window_title(pid: i32) -> Option<String> {
    ax_string(pid, &["AXFocusedWindow", "AXTitle"])
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXValueGetValue(v: CFTypeRef, t: u32, out: *mut c_void) -> bool;
}

/// Whether the app is frontmost (AXFrontmost).
pub fn ax_frontmost(pid: i32) -> bool {
    unsafe {
        let app = AXUIElementCreateApplication(pid);
        let v = ax_copy(app, "AXFrontmost");
        CFRelease(app);
        if v.is_null() {
            return false;
        }
        let yes = v == kCFBooleanTrue;
        CFRelease(v);
        yes
    }
}

/// Centre of the app's focused window (global, top-left origin).
pub fn ax_focused_window_center(pid: i32) -> Option<CGPoint> {
    unsafe {
        let app = AXUIElementCreateApplication(pid);
        let w = ax_copy(app, "AXFocusedWindow");
        CFRelease(app);
        if w.is_null() {
            return None;
        }
        let pos = ax_copy(w, "AXPosition");
        let size = ax_copy(w, "AXSize");
        CFRelease(w);
        if pos.is_null() || size.is_null() {
            return None;
        }
        let mut p = CGPoint::default();
        let mut s = [0f64; 2];
        let ok = AXValueGetValue(pos, 1, &mut p as *mut CGPoint as *mut c_void)
            && AXValueGetValue(size, 2, s.as_mut_ptr() as *mut c_void);
        CFRelease(pos);
        CFRelease(size);
        ok.then(|| CGPoint {
            x: p.x + s[0] / 2.0,
            y: p.y + s[1] / 2.0 + 20.0,
        })
    }
}

// ---- CoreAudio ----

fn get_u32(obj: u32, sel: u32) -> Option<u32> {
    let addr = PropAddr {
        selector: sel,
        scope: GLOB,
        element: 0,
    };
    let mut v: u32 = 0;
    let mut size = 4u32;
    let err = unsafe {
        AudioObjectGetPropertyData(
            obj,
            &addr,
            0,
            std::ptr::null(),
            &mut size,
            &mut v as *mut u32 as *mut c_void,
        )
    };
    (err == 0).then_some(v)
}

pub fn default_input() -> Option<u32> {
    get_u32(SYSTEM_OBJECT, DEFAULT_INPUT)
}

pub fn set_default_input(id: u32) -> bool {
    let addr = PropAddr {
        selector: DEFAULT_INPUT,
        scope: GLOB,
        element: 0,
    };
    unsafe {
        AudioObjectSetPropertyData(
            SYSTEM_OBJECT,
            &addr,
            0,
            std::ptr::null(),
            4,
            &id as *const u32 as *const c_void,
        ) == 0
    }
}

pub fn device_uid(id: u32) -> Option<String> {
    let addr = PropAddr {
        selector: DEVICE_UID,
        scope: GLOB,
        element: 0,
    };
    let mut v: CFTypeRef = std::ptr::null();
    let mut size = std::mem::size_of::<CFTypeRef>() as u32;
    let err = unsafe {
        AudioObjectGetPropertyData(
            id,
            &addr,
            0,
            std::ptr::null(),
            &mut size,
            &mut v as *mut CFTypeRef as *mut c_void,
        )
    };
    if err != 0 || v.is_null() {
        return None;
    }
    let s = from_cfstr(v);
    unsafe { CFRelease(v) };
    s
}

/// An aggregate input device wrapping one real device (`sub_uid`). `private`
/// devices are visible to this process only.
pub fn create_aggregate(name: &str, uid: &str, sub_uid: &str, private: bool) -> Result<u32, i32> {
    unsafe {
        let sub_key = cfstr("uid");
        let sub_val = cfstr(sub_uid);
        let sub = CFDictionaryCreate(
            std::ptr::null(),
            [sub_key].as_ptr(),
            [sub_val].as_ptr(),
            1,
            &kCFTypeDictionaryKeyCallBacks as *const c_void,
            &kCFTypeDictionaryValueCallBacks as *const c_void,
        );
        let subs = CFArrayCreate(
            std::ptr::null(),
            [sub].as_ptr(),
            1,
            &kCFTypeArrayCallBacks as *const c_void,
        );
        let keys = [
            cfstr("uid"),
            cfstr("name"),
            cfstr("subdevices"),
            cfstr("master"),
            cfstr("private"),
        ];
        let vals = [
            cfstr(uid),
            cfstr(name),
            subs,
            cfstr(sub_uid),
            if private {
                kCFBooleanTrue
            } else {
                kCFBooleanFalse
            },
        ];
        let desc = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            vals.as_ptr(),
            keys.len() as isize,
            &kCFTypeDictionaryKeyCallBacks as *const c_void,
            &kCFTypeDictionaryValueCallBacks as *const c_void,
        );
        let mut id = 0u32;
        let err = AudioHardwareCreateAggregateDevice(desc, &mut id);
        CFRelease(desc);
        for k in keys {
            CFRelease(k);
        }
        for (i, v) in vals.iter().enumerate() {
            if i != 4 {
                CFRelease(*v);
            }
        }
        CFRelease(sub);
        CFRelease(sub_key);
        CFRelease(sub_val);
        if err == 0 {
            Ok(id)
        } else {
            Err(err)
        }
    }
}

pub fn destroy_aggregate(id: u32) -> i32 {
    unsafe { AudioHardwareDestroyAggregateDevice(id) }
}
