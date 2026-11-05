// SPDX-License-Identifier: GPL-3.0-or-later
//
// The RS-03 test pill: a `fork-pill` NSPanel built the way R5 §4.9 specifies
// (tauri-nspanel 2.1.1 + the Tolkovo setup: status level, non-activating,
// never key, `hides_on_deactivate(false)`, `accept_first_mouse(true)`, all
// Spaces + full-screen auxiliary), with a native tracking area for hover,
// placement at the bottom centre of the display under the cursor, a screen
// parameters observer, a 1 Hz cursor-display check and an optional re-show on
// Space change. `_setPreventsActivation:` is applied only when asked, so the
// spike can compare both.
//
// Panel set-up, `set_prevents_activation` and the Space observer are ported
// from Tolkovo `src-tauri/src/tolkovo/pill.rs` (commit ffc5b62):
// Copyright (c) 2025 CJ Pais, Copyright (c) 2026 kozliq (MIT).
// Modified by Wispr Bro contributors, 2026-10-30.

// panel_event! requires the explicit "-> ()" return type.
#![allow(clippy::unused_unit)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, WebviewUrl};

pub const LABEL: &str = "fork-pill";
/// Window size (logical points): a 200×40 capsule plus a 32 pt band above it
/// for the hover tooltip.
pub const WIN_W: f64 = 200.0;
pub const WIN_H: f64 = 72.0;
/// Gap above the Dock / visible-frame bottom.
pub const BOTTOM_GAP: f64 = 8.0;

#[derive(Clone, Debug, serde::Serialize)]
pub struct Ev {
    pub t_ms: f64,
    pub kind: String,
    pub detail: String,
}

static EPOCH: OnceLock<Instant> = OnceLock::new();
static LOG: Mutex<Vec<Ev>> = Mutex::new(Vec::new());
pub static RESHOW_ON_SPACE: AtomicBool = AtomicBool::new(false);

pub fn now_ms() -> f64 {
    EPOCH.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

pub fn log(kind: &str, detail: impl Into<String>) {
    let e = Ev {
        t_ms: now_ms(),
        kind: kind.into(),
        detail: detail.into(),
    };
    if let Ok(mut l) = LOG.lock() {
        l.push(e);
    }
}

/// Events logged after `t_ms` (inclusive).
pub fn events_since(t_ms: f64) -> Vec<Ev> {
    LOG.lock()
        .map(|l| l.iter().filter(|e| e.t_ms >= t_ms).cloned().collect())
        .unwrap_or_default()
}

pub fn clear_log() {
    if let Ok(mut l) = LOG.lock() {
        // Keep memory flat in the 8 h run.
        l.clear();
    }
}

#[tauri::command]
pub fn pill_event(kind: String, detail: String) {
    log(&kind, detail);
}

use tauri_nspanel::{tauri_panel, TrackingAreaOptions};
tauri_panel! {
        panel!(ForkPillPanel {
            config: {
                can_become_key_window: false,
                can_become_main_window: false,
                is_floating_panel: true
            }
            with: {
                tracking_area: {
                    options: TrackingAreaOptions::new()
                        .active_always()
                        .mouse_entered_and_exited(),
                    auto_resize: true
                }
            }
        })

        panel_event!(ForkPillEvents {
            window_did_become_key(notification: &NSNotification) -> ()
        })
}

pub struct Options {
    pub prevent_activation: bool,
    pub content_protected: bool,
}

/// Build the pill (shown). Must run on the main thread (Tauri `setup`).
pub fn create(app: &AppHandle, opts: &Options) -> Result<(), String> {
    use tauri_nspanel::{CollectionBehavior, PanelBuilder, PanelLevel, StyleMask};
    let panel = PanelBuilder::<_, ForkPillPanel>::new(app, LABEL)
        .url(WebviewUrl::App("index.html".into()))
        .title("Wispr Bro pill (RS-03)")
        .position(tauri::Position::Logical(tauri::LogicalPosition {
            x: 100.0,
            y: 100.0,
        }))
        .size(tauri::Size::Logical(tauri::LogicalSize {
            width: WIN_W,
            height: WIN_H,
        }))
        .level(PanelLevel::Status)
        .has_shadow(false)
        .transparent(true)
        .no_activate(true)
        .hides_on_deactivate(false)
        .corner_radius(0.0)
        .style_mask(StyleMask::empty().borderless().nonactivating_panel())
        .with_window(|w| {
            w.decorations(false)
                .transparent(true)
                .focusable(false)
                .accept_first_mouse(true)
                .skip_taskbar(true)
        })
        .collection_behavior(
            CollectionBehavior::new()
                .can_join_all_spaces()
                .full_screen_auxiliary(),
        )
        .build()
        .map_err(|e| e.to_string())?;

    let events = ForkPillEvents::new();
    let h = app.clone();
    events.on_mouse_entered(move |_| {
        log("native_enter", "");
        let _ = h.emit_to(LABEL, "pill-hover", true);
    });
    let h = app.clone();
    events.on_mouse_exited(move |_| {
        log("native_exit", "");
        let _ = h.emit_to(LABEL, "pill-hover", false);
    });
    events.window_did_become_key(|_| log("became_key", ""));
    panel.set_event_handler(Some(events.as_ref()));
    // The handler must outlive the panel (it is the window delegate).
    std::mem::forget(events);

    let window = app.get_webview_window(LABEL).ok_or("pill window missing")?;
    if opts.prevent_activation {
        set_prevents_activation(&window, true);
    }
    if opts.content_protected {
        let _ = window.set_content_protected(true);
    }
    place(app);
    panel.show();
    log("created", format!("prevent={}", opts.prevent_activation));
    watch_screens(app);
    watch_spaces(app);
    watch_activations();
    cursor_display_check(app);
    Ok(())
}

/// Tolkovo's private opt-out of app activation (DEC-28), behind a feature
/// check. Main thread only.
pub fn set_prevents_activation(window: &tauri::WebviewWindow, on: bool) {
    use objc2::runtime::{AnyObject, Bool, Sel};
    use objc2::{msg_send, sel};
    let Ok(ptr) = window.ns_window() else { return };
    let ns_window = ptr as *mut AnyObject;
    if ns_window.is_null() {
        return;
    }
    let selector: Sel = sel!(_setPreventsActivation:);
    unsafe {
        let responds: Bool = msg_send![&*ns_window, respondsToSelector: selector];
        if responds.as_bool() {
            let _: () = msg_send![&*ns_window, _setPreventsActivation: Bool::new(on)];
            log("prevent_activation", format!("{on}"));
        } else {
            log("prevent_activation", "selector missing");
        }
    }
}

/// Log every app activation (bundle id), to tell focus moved by a pill click
/// from other activity on the Mac.
fn watch_activations() {
    use block2::RcBlock;
    use objc2_app_kit::{
        NSRunningApplication, NSWorkspace, NSWorkspaceApplicationKey,
        NSWorkspaceDidActivateApplicationNotification,
    };
    use objc2_foundation::NSNotification;
    use std::ptr::NonNull;
    let block = RcBlock::new(move |n: NonNull<NSNotification>| {
        let n = unsafe { n.as_ref() };
        let id = n
            .userInfo()
            .and_then(|u| u.objectForKey(unsafe { NSWorkspaceApplicationKey }))
            .and_then(|o| o.downcast::<NSRunningApplication>().ok())
            .and_then(|a| a.bundleIdentifier())
            .map(|s| s.to_string())
            .unwrap_or_default();
        log("activated", id);
    });
    unsafe {
        let center = NSWorkspace::sharedWorkspace().notificationCenter();
        let obs = center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidActivateApplicationNotification),
            None,
            None,
            &block,
        );
        std::mem::forget(obs);
    }
}

fn ns_window(app: &AppHandle) -> Option<&'static objc2_app_kit::NSWindow> {
    let w = app.get_webview_window(LABEL)?;
    let ptr = w.ns_window().ok()?;
    if ptr.is_null() {
        return None;
    }
    // The pill lives for the whole run.
    Some(unsafe { &*(ptr as *const objc2_app_kit::NSWindow) })
}

/// Index of the screen under the cursor and its target origin (Cocoa coords).
fn target_origin() -> Option<(usize, objc2_foundation::NSPoint, f64)> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSEvent, NSScreen};
    let mtm = MainThreadMarker::new()?;
    let mouse = NSEvent::mouseLocation();
    let screens = NSScreen::screens(mtm);
    let mut chosen = None;
    for (i, s) in screens.iter().enumerate() {
        let f = s.frame();
        if mouse.x >= f.origin.x
            && mouse.x < f.origin.x + f.size.width
            && mouse.y >= f.origin.y
            && mouse.y < f.origin.y + f.size.height
        {
            chosen = Some((i, s));
            break;
        }
    }
    let (i, s) = match chosen {
        Some(c) => c,
        None => (0, screens.iter().next()?),
    };
    let f = s.frame();
    let vf = s.visibleFrame();
    let x = (f.origin.x + f.size.width / 2.0 - WIN_W / 2.0).round();
    let y = (vf.origin.y + BOTTOM_GAP).round();
    Some((
        i,
        objc2_foundation::NSPoint::new(x, y),
        s.backingScaleFactor(),
    ))
}

/// Place the pill at the bottom centre of the display under the cursor.
/// Main thread only.
pub fn place(app: &AppHandle) {
    let Some(w) = ns_window(app) else { return };
    let Some((i, origin, scale)) = target_origin() else {
        return;
    };
    let cur = w.frame().origin;
    if (cur.x - origin.x).abs() > 0.5 || (cur.y - origin.y).abs() > 0.5 {
        w.setFrameOrigin(origin);
        log(
            "placed",
            format!("screen={i} x={} y={} scale={scale}", origin.x, origin.y),
        );
    }
}

fn watch_screens(app: &AppHandle) {
    use block2::RcBlock;
    use objc2_app_kit::NSApplicationDidChangeScreenParametersNotification;
    use objc2_foundation::{NSNotification, NSNotificationCenter};
    use std::ptr::NonNull;
    let h = app.clone();
    let block = RcBlock::new(move |_: NonNull<NSNotification>| {
        log("screen_params_changed", "");
        place(&h);
    });
    unsafe {
        let center = NSNotificationCenter::defaultCenter();
        let obs = center.addObserverForName_object_queue_usingBlock(
            Some(NSApplicationDidChangeScreenParametersNotification),
            None,
            None,
            &block,
        );
        std::mem::forget(obs);
    }
}

fn watch_spaces(app: &AppHandle) {
    use block2::RcBlock;
    use objc2_app_kit::{NSWorkspace, NSWorkspaceActiveSpaceDidChangeNotification};
    use objc2_foundation::NSNotification;
    use std::ptr::NonNull;
    let h = app.clone();
    let block = RcBlock::new(move |_: NonNull<NSNotification>| {
        log("space_changed", "");
        if RESHOW_ON_SPACE.load(Ordering::SeqCst) {
            if let Some(w) = ns_window(&h) {
                w.orderFrontRegardless();
                log("reshown", "");
            }
        }
    });
    unsafe {
        let center = NSWorkspace::sharedWorkspace().notificationCenter();
        let obs = center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceActiveSpaceDidChangeNotification),
            None,
            None,
            &block,
        );
        std::mem::forget(obs);
    }
}

fn cursor_display_check(app: &AppHandle) {
    let h = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));
        let h2 = h.clone();
        let _ = h.run_on_main_thread(move || place(&h2));
    });
}

/// Run `f` on the main thread and wait for its result.
pub fn on_main<T: Send + 'static>(
    app: &AppHandle,
    f: impl FnOnce(&AppHandle) -> T + Send + 'static,
) -> Option<T> {
    let (tx, rx) = mpsc::channel();
    let h = app.clone();
    app.run_on_main_thread(move || {
        let _ = tx.send(f(&h));
    })
    .ok()?;
    rx.recv_timeout(Duration::from_secs(5)).ok()
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PillInfo {
    pub window_number: i64,
    pub visible: bool,
    pub on_active_space: bool,
    pub is_key: bool,
    pub app_active: bool,
    pub frame_cocoa: [f64; 4],
    pub backing_scale: f64,
    pub screen_scale: f64,
    pub frontmost_bundle: String,
}

pub fn info(app: &AppHandle) -> Option<PillInfo> {
    on_main(app, |h| {
        use objc2_app_kit::{NSApplication, NSWorkspace};
        let w = ns_window(h)?;
        let mtm = objc2::MainThreadMarker::new()?;
        let f = w.frame();
        let front = NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .and_then(|a| a.bundleIdentifier())
            .map(|s| s.to_string())
            .unwrap_or_default();
        Some(PillInfo {
            window_number: w.windowNumber() as i64,
            visible: w.isVisible(),
            on_active_space: w.isOnActiveSpace(),
            is_key: w.isKeyWindow(),
            app_active: NSApplication::sharedApplication(mtm).isActive(),
            frame_cocoa: [f.origin.x, f.origin.y, f.size.width, f.size.height],
            backing_scale: w.backingScaleFactor(),
            screen_scale: w.screen().map(|s| s.backingScaleFactor()).unwrap_or(0.0),
            frontmost_bundle: front,
        })
    })
    .flatten()
}

pub fn emit_state(app: &AppHandle, state: &str) {
    let _ = app.emit_to(LABEL, "pill-state", state);
}

pub fn emit_levels(app: &AppHandle, levels: &[f32]) {
    let _ = app.emit_to(LABEL, "pill-levels", levels);
}
