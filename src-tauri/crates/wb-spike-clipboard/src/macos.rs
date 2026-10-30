// SPDX-License-Identifier: GPL-3.0-or-later
//! macOS plumbing for the session (not part of the `fork/clipboard`
//! prototype): run-loop pumping, permissions, the frontmost app, reading a
//! field's text through Accessibility (to confirm a paste landed without
//! asking), input sources (the CJK IME step), a listen-only tap that records
//! the modifier flags our own ⌘V arrives with, and system sounds. Public
//! APIs only; no AppleScript.

use objc2::rc::Retained;
use objc2_app_kit::NSWorkspace;
use objc2_foundation::NSString;
use std::ffi::c_void;
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

type CFTypeRef = *const c_void;
type AXUIElementRef = *const c_void;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFRunLoopDefaultMode: CFTypeRef;
    static kCFRunLoopCommonModes: CFTypeRef;
    fn CFRunLoopRunInMode(mode: CFTypeRef, secs: f64, return_after_source: u8) -> i32;
    fn CFRunLoopGetCurrent() -> CFTypeRef;
    fn CFRunLoopAddSource(rl: CFTypeRef, source: CFTypeRef, mode: CFTypeRef);
    fn CFRunLoopRun();
    fn CFMachPortCreateRunLoopSource(alloc: CFTypeRef, port: CFTypeRef, order: isize) -> CFTypeRef;
    fn CFRelease(v: CFTypeRef);
    fn CFRetain(v: CFTypeRef) -> CFTypeRef;
    fn CFGetTypeID(v: CFTypeRef) -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFArrayGetTypeID() -> usize;
    fn CFArrayGetCount(a: CFTypeRef) -> isize;
    fn CFArrayGetValueAtIndex(a: CFTypeRef, i: isize) -> CFTypeRef;
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> u8;
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        e: AXUIElementRef,
        attr: CFTypeRef,
        out: *mut CFTypeRef,
    ) -> i32;
    fn AXUIElementSetMessagingTimeout(e: AXUIElementRef, secs: f32) -> i32;
    fn AXUIElementPerformAction(e: AXUIElementRef, action: CFTypeRef) -> i32;
    fn CGPreflightPostEventAccess() -> bool;
    fn CGRequestPostEventAccess() -> bool;
    fn CGPreflightListenEventAccess() -> bool;
    fn CGEventSourceButtonState(state: u32, button: u32) -> bool;
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        mask: u64,
        callback: extern "C" fn(*const c_void, u32, *mut c_void, *mut c_void) -> *mut c_void,
        user: *mut c_void,
    ) -> CFTypeRef;
    fn CGEventGetFlags(e: *mut c_void) -> u64;
    fn CGEventGetIntegerValueField(e: *mut c_void, field: u32) -> i64;
    fn CGEventTapEnable(tap: CFTypeRef, enable: bool);
}

// ---------------------------------------------------------------------------
// Run loop

/// Sleeps while servicing the main run loop (pasteboard promises, workspace
/// notifications). Returns early if the run was interrupted.
pub fn pump(d: Duration) {
    let end = Instant::now() + d;
    loop {
        let left = end.saturating_duration_since(Instant::now());
        if left.is_zero() || crate::owner::interrupted() {
            return;
        }
        let step = left.min(Duration::from_millis(10)).as_secs_f64();
        // SAFETY: called on the main thread with the default mode.
        unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, step, 0) };
    }
}

// ---------------------------------------------------------------------------
// Permissions

pub fn accessibility_trusted() -> bool {
    // SAFETY: no arguments.
    unsafe { AXIsProcessTrusted() != 0 }
}

pub fn can_post_events() -> bool {
    // SAFETY: no arguments.
    unsafe { CGPreflightPostEventAccess() }
}

pub fn request_post_events() -> bool {
    // SAFETY: no arguments; may show the system prompt.
    unsafe { CGRequestPostEventAccess() }
}

pub fn can_listen_events() -> bool {
    // SAFETY: no arguments.
    unsafe { CGPreflightListenEventAccess() }
}

pub fn open_accessibility_pane() {
    let _ = Command::new("/usr/bin/open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .status();
}

/// The left mouse button is down (HID state; needs no permission).
pub fn mouse_down() -> bool {
    // SAFETY: read-only query (1 = HIDSystemState, 0 = left button).
    unsafe { CGEventSourceButtonState(1, 0) }
}

// ---------------------------------------------------------------------------
// Apps

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Front {
    pub bundle_id: String,
    pub pid: i32,
    pub name: String,
}

pub fn frontmost() -> Option<Front> {
    let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
    Some(Front {
        bundle_id: app
            .bundleIdentifier()
            .map(|s| s.to_string())
            .unwrap_or_default(),
        pid: app.processIdentifier(),
        name: app
            .localizedName()
            .map(|s| s.to_string())
            .unwrap_or_default(),
    })
}

/// `open` with arguments (launches or activates through LaunchServices).
pub fn open(args: &[&str]) -> bool {
    Command::new("/usr/bin/open")
        .args(args)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Waits (pumping) until `bundle_id` is frontmost.
pub fn wait_front(bundle_id: &str, timeout: Duration) -> Option<Front> {
    let end = Instant::now() + timeout;
    while Instant::now() < end && !crate::owner::interrupted() {
        if let Some(f) = frontmost() {
            if f.bundle_id == bundle_id {
                return Some(f);
            }
        }
        pump(Duration::from_millis(50));
    }
    None
}

/// Selects `paths` in a Finder window (used to make real Finder copies).
pub fn reveal_in_finder(paths: &[std::path::PathBuf]) {
    use objc2_foundation::{NSArray, NSURL};
    let urls: Vec<Retained<NSURL>> = paths
        .iter()
        .map(|p| NSURL::fileURLWithPath(&NSString::from_str(&p.to_string_lossy())))
        .collect();
    let refs: Vec<&NSURL> = urls.iter().map(|u| &**u).collect();
    NSWorkspace::sharedWorkspace().activateFileViewerSelectingURLs(&NSArray::from_slice(&refs));
}

pub fn play(sound: &str) {
    let _ = Command::new("/usr/bin/afplay")
        .arg(format!("/System/Library/Sounds/{sound}.aiff"))
        .spawn();
}

pub fn plist_value(plist: &std::path::Path, key: &str) -> Option<String> {
    let out = Command::new("/usr/bin/plutil")
        .args(["-extract", key, "raw", "-o", "-"])
        .arg(plist)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

// ---------------------------------------------------------------------------
// Accessibility reads

struct Cf(CFTypeRef);

impl Drop for Cf {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: we own one retain.
            unsafe { CFRelease(self.0) };
        }
    }
}

fn cfstr(s: &str) -> Retained<NSString> {
    NSString::from_str(s)
}

fn as_cf(s: &NSString) -> CFTypeRef {
    (s as *const NSString).cast()
}

fn attr(e: AXUIElementRef, name: &str) -> Option<Cf> {
    let n = cfstr(name);
    let mut out: CFTypeRef = std::ptr::null();
    // SAFETY: valid element, attribute name and out-pointer.
    let err = unsafe { AXUIElementCopyAttributeValue(e, as_cf(&n), &mut out) };
    (err == 0 && !out.is_null()).then_some(Cf(out))
}

fn cf_to_string(v: &Cf) -> Option<String> {
    // SAFETY: type check before the toll-free bridge.
    unsafe {
        if CFGetTypeID(v.0) != CFStringGetTypeID() {
            return None;
        }
        let ns: &NSString = &*(v.0 as *const NSString);
        Some(ns.to_string())
    }
}

fn app_element(pid: i32) -> Cf {
    // SAFETY: Create Rule.
    let e = unsafe { AXUIElementCreateApplication(pid) };
    // SAFETY: valid element.
    unsafe { AXUIElementSetMessagingTimeout(e, 0.5) };
    Cf(e)
}

#[derive(Clone, Debug, Default)]
pub struct Focus {
    pub role: Option<String>,
    pub value: Option<String>,
    pub window_title: Option<String>,
}

/// What Accessibility says about `pid`'s focused field and window.
pub fn focus(pid: i32) -> Focus {
    if !accessibility_trusted() {
        return Focus::default();
    }
    let app = app_element(pid);
    let mut f = Focus::default();
    if let Some(el) = attr(app.0, "AXFocusedUIElement") {
        f.role = attr(el.0, "AXRole").as_ref().and_then(cf_to_string);
        f.value = attr(el.0, "AXValue").as_ref().and_then(cf_to_string);
    }
    if let Some(w) = attr(app.0, "AXFocusedWindow") {
        f.window_title = attr(w.0, "AXTitle").as_ref().and_then(cf_to_string);
    }
    f
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXValueGetValue(v: CFTypeRef, ty: u32, out: *mut c_void) -> u8;
    fn CGEventCreateMouseEvent(
        src: *const c_void,
        ty: u32,
        at: CGPoint,
        button: u32,
    ) -> *mut c_void;
    fn CGEventPost(tap: u32, event: *mut c_void);
}

/// Clicks once in `pid`'s focused window, `frac_y` of the way down its
/// height and in the middle horizontally (to give a web page focus before
/// ⌘A). Moves the pointer there.
pub fn click_in_window(pid: i32, frac_y: f64) -> bool {
    let app = app_element(pid);
    let Some(w) = attr(app.0, "AXFocusedWindow") else {
        return false;
    };
    let (Some(pos), Some(size)) = (attr(w.0, "AXPosition"), attr(w.0, "AXSize")) else {
        return false;
    };
    let mut p = CGPoint::default();
    let mut s = CGPoint::default(); // CGSize has the same layout
                                    // SAFETY: 1 = kAXValueCGPointType, 2 = kAXValueCGSizeType; outputs are
                                    // two f64s.
    let ok = unsafe {
        AXValueGetValue(pos.0, 1, (&mut p as *mut CGPoint).cast()) != 0
            && AXValueGetValue(size.0, 2, (&mut s as *mut CGPoint).cast()) != 0
    };
    if !ok || s.x < 50.0 || s.y < 50.0 {
        return false;
    }
    let at = CGPoint {
        x: p.x + s.x / 2.0,
        y: p.y + s.y * frac_y,
    };
    for ty in [1u32, 2] {
        // SAFETY: 1 = left mouse down, 2 = left mouse up; posted then released.
        unsafe {
            let e = CGEventCreateMouseEvent(std::ptr::null(), ty, at, 0);
            if e.is_null() {
                return false;
            }
            CGEventPost(0, e);
            CFRelease(e.cast_const());
        }
        pump(Duration::from_millis(60));
    }
    true
}

/// Raises the window of `pid` whose title contains `needle`.
pub fn raise_window(pid: i32, needle: &str) -> bool {
    let app = app_element(pid);
    let Some(ws) = attr(app.0, "AXWindows") else {
        return false;
    };
    // SAFETY: type-checked array of AX elements.
    unsafe {
        if CFGetTypeID(ws.0) != CFArrayGetTypeID() {
            return false;
        }
        for i in 0..CFArrayGetCount(ws.0) {
            let w = CFArrayGetValueAtIndex(ws.0, i);
            let title = attr(w, "AXTitle").as_ref().and_then(cf_to_string);
            if title.is_some_and(|t| t.contains(needle)) {
                let a = cfstr("AXRaise");
                return AXUIElementPerformAction(w, as_cf(&a)) == 0;
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Input sources (TIS)

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    fn TISCreateInputSourceList(props: CFTypeRef, include_all_installed: u8) -> CFTypeRef;
    fn TISGetInputSourceProperty(src: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
    fn TISSelectInputSource(src: CFTypeRef) -> i32;
    fn TISEnableInputSource(src: CFTypeRef) -> i32;
    fn TISDisableInputSource(src: CFTypeRef) -> i32;
    fn TISCopyCurrentKeyboardInputSource() -> CFTypeRef;
    static kTISPropertyInputSourceID: CFTypeRef;
}

pub struct InputSource {
    r: Cf,
    pub id: String,
}

fn tis_id(src: CFTypeRef) -> String {
    // SAFETY: Get Rule CFString.
    unsafe {
        let v = TISGetInputSourceProperty(src, kTISPropertyInputSourceID);
        if v.is_null() {
            return String::new();
        }
        CFRetain(v);
        cf_to_string(&Cf(v)).unwrap_or_default()
    }
}

/// Input sources (enabled ones, or every installed one).
pub fn input_sources(all_installed: bool) -> Vec<InputSource> {
    // SAFETY: Create Rule array; elements retained individually.
    unsafe {
        let list = TISCreateInputSourceList(std::ptr::null(), u8::from(all_installed));
        if list.is_null() {
            return Vec::new();
        }
        let list = Cf(list);
        (0..CFArrayGetCount(list.0))
            .map(|i| {
                let s = CFArrayGetValueAtIndex(list.0, i);
                CFRetain(s);
                InputSource {
                    id: tis_id(s),
                    r: Cf(s),
                }
            })
            .collect()
    }
}

pub fn current_input_source_id() -> String {
    // SAFETY: Create Rule, released by Cf.
    let c = Cf(unsafe { TISCopyCurrentKeyboardInputSource() });
    if c.0.is_null() {
        return String::new();
    }
    tis_id(c.0)
}

impl InputSource {
    pub fn select(&self) -> bool {
        // SAFETY: valid source; main thread.
        unsafe { TISSelectInputSource(self.r.0) == 0 }
    }
    pub fn enable(&self) -> bool {
        // SAFETY: valid source.
        unsafe { TISEnableInputSource(self.r.0) == 0 }
    }
    pub fn disable(&self) -> bool {
        // SAFETY: valid source.
        unsafe { TISDisableInputSource(self.r.0) == 0 }
    }
}

// ---------------------------------------------------------------------------
// Listen-only tap: the flags our own synthetic keys arrive with

static TAP_SEEN: Mutex<Vec<(Instant, u16, u64)>> = Mutex::new(Vec::new());
static TAP_STARTED: OnceLock<bool> = OnceLock::new();

extern "C" fn tap_callback(
    _proxy: *const c_void,
    ty: u32,
    event: *mut c_void,
    _user: *mut c_void,
) -> *mut c_void {
    const KEY_DOWN: u32 = 10;
    if ty == KEY_DOWN && !event.is_null() {
        // SAFETY: valid event for the callback's duration.
        let (marker, flags, code) = unsafe {
            (
                CGEventGetIntegerValueField(event, crate::clip::keys::EVENT_SOURCE_USER_DATA),
                CGEventGetFlags(event),
                CGEventGetIntegerValueField(event, 9), // kCGKeyboardEventKeycode
            )
        };
        if marker == crate::clip::keys::SYNTHETIC_EVENT_MARKER {
            if let Ok(mut v) = TAP_SEEN.lock() {
                v.push((Instant::now(), code as u16, flags));
            }
        }
    }
    event
}

/// Starts the tap once (needs Input Monitoring). False if it could not be
/// created.
pub fn start_key_tap() -> bool {
    *TAP_STARTED.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("wb-rs05-tap".into())
            .spawn(move || {
                // 1 = kCGSessionEventTap, 0 = head insert, 1 = listen only,
                // mask = keyDown.
                // SAFETY: valid callback; the tap lives for the process.
                let tap = unsafe {
                    CGEventTapCreate(1, 0, 1, 1 << 10, tap_callback, std::ptr::null_mut())
                };
                if tap.is_null() {
                    let _ = tx.send(false);
                    return;
                }
                // SAFETY: valid mach port; this thread's run loop.
                unsafe {
                    let src = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
                    CFRunLoopAddSource(CFRunLoopGetCurrent(), src, kCFRunLoopCommonModes);
                    CGEventTapEnable(tap, true);
                }
                let _ = tx.send(true);
                // SAFETY: runs this thread's run loop forever.
                unsafe { CFRunLoopRun() };
            })
            .is_ok()
            && rx.recv_timeout(Duration::from_secs(2)).unwrap_or(false)
    })
}

/// Our synthetic key-downs the tap saw since `since`: (key code, flags).
pub fn tapped_since(since: Instant) -> Vec<(u16, u64)> {
    TAP_SEEN
        .lock()
        .map(|v| {
            v.iter()
                .filter(|(t, _, _)| *t >= since)
                .map(|(_, c, f)| (*c, *f))
                .collect()
        })
        .unwrap_or_default()
}
