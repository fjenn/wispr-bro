// SPDX-License-Identifier: GPL-3.0-or-later
//! The bar's two windows on the main thread (macOS, S-03).
//!
//! - The native resting panel (`native.rs`), built once at launch.
//! - The `fork-pill` web view panel, built with tauri-nspanel (the git v2.1
//!   pin already in the app; R5 §4.9 Tolkovo setup: status level,
//!   non-activating, never key or main, `hides_on_deactivate(false)`,
//!   `accept_first_mouse(true)`, all Spaces + full-screen auxiliary, no
//!   `_setPreventsActivation:`, DEC-28). It exists only around a take: it
//!   is built when fn goes down (or a take starts some other way), kept
//!   ordered in behind the native bar while it loads (so WebKit renders it), shown
//!   once the page has painted the current state, and destroyed
//!   [`super::WEB_LINGER_MS`] after the take ends (its WebContent process
//!   exits with it, which is what keeps idle memory flat; RS-03).
//!
//! Panel and placement code follows Tolkovo `src-tauri/src/tolkovo/pill.rs`
//! (commit ffc5b62; Copyright (c) 2025 CJ Pais, Copyright (c) 2026 kozliq;
//! MIT), as ported in `crates/wb-spike-pill/src/pill.rs` for RS-03.
//! Modified by Wispr Bro contributors, 2026-10-30: two windows, exact
//! sizes per state, drag positions, no private API.
//!
//! All functions here run on the main thread ([`with_ui`]).

// tauri_panel!'s generated code uses `-> ()` return types.
#![allow(clippy::unused_unit)]

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, sel, AnyThread, MainThreadMarker};
use objc2_app_kit::{
    NSApplicationDidChangeScreenParametersNotification, NSWindow, NSWindowOrderingMode,
    NSWorkspace, NSWorkspaceActiveSpaceDidChangeNotification,
};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObject, NSPoint, NSRect, NSSize};
use tauri::{AppHandle, Manager, WebviewUrl, Wry};
use tauri_nspanel::{CollectionBehavior, ManagerExt, PanelBuilder, PanelLevel, StyleMask};

use super::geometry::Shape;
use super::native::{self, Mode, NativeEvent, Resting, Texts};
use super::placement::{self, Anchor, Edge, Screen};

/// Label of the web view window (R5 §4.1).
pub const LABEL: &str = "fork-pill";
/// Its page (a Vite input, `src/fork/vite-inputs.ts`).
pub const URL: &str = "src/fork/pill/index.html";

/// The web view's panel class (its own module: `tauri_panel!` brings its
/// own imports).
mod web_panel {
    use tauri::Manager;
    use tauri_nspanel::tauri_panel;

    tauri_panel! {
        panel!(ForkPillWebPanel {
            config: {
                can_become_key_window: false,
                can_become_main_window: false,
                is_floating_panel: true
            }
        })
    }
}
use web_panel::ForkPillWebPanel;

/// What the main thread should show (computed by the controller).
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub native: NativeScene,
    pub web: WebScene,
    pub edge: Edge,
    /// Remembered drag positions, by display key.
    pub x_by_display: std::collections::BTreeMap<String, f64>,
    /// "Hide the bar in screen recordings".
    pub protected: bool,
    /// `Some(true)` dark, `Some(false)` light, `None` the system's.
    pub dark: Option<bool>,
    /// S-24: Dictate and Scratchpad share the resting bar.
    pub scratchpad_enabled: bool,
    pub texts: Texts,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NativeScene {
    Hidden,
    Rest,
    Bridge(Shape),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WebScene {
    /// No web view (destroyed or never built).
    None,
    /// Built (or being built), ordered in behind the native bar so WebKit
    /// paints it.
    Loading,
    /// Built, off screen, after a take (destroyed after the linger time).
    Lingering,
    /// Visible at this shape.
    Shown(Shape),
}

/// Main-thread state.
struct Ui {
    app: AppHandle<Wry>,
    resting: Resting,
    /// The anchor of the current (or last) placement and its display.
    anchor: Option<(Anchor, Screen)>,
    /// The web view exists.
    web_built: bool,
    web_shown: Option<Shape>,
    scene: Option<Scene>,
    /// Where the active bar sits during a take (fixed at its start).
    take_anchor: Option<(Anchor, Screen)>,
}

thread_local! {
    static UI: RefCell<Option<Ui>> = const { RefCell::new(None) };
}

/// Runs `f` with the UI state. Main thread only; `None` before [`init`].
fn with_ui<T>(f: impl FnOnce(&mut Ui) -> T) -> Option<T> {
    UI.with(|ui| ui.borrow_mut().as_mut().map(f))
}

/// Builds the resting panel and the observers. Main thread only.
pub fn init(app: &AppHandle<Wry>, on_event: impl Fn(NativeEvent) + 'static) {
    let Some(mtm) = MainThreadMarker::new() else {
        log::error!("pill: init called off the main thread");
        return;
    };
    let resting = Resting::new(mtm, on_event);
    UI.with(|ui| {
        *ui.borrow_mut() = Some(Ui {
            app: app.clone(),
            resting,
            anchor: None,
            web_built: false,
            web_shown: None,
            scene: None,
            take_anchor: None,
        })
    });
    observe(mtm);
}

/// Applies a scene. Main thread only.
pub fn apply(scene: Scene) {
    with_ui(|ui| ui.apply(scene));
}

/// Re-places the resting bar on the display under the cursor (1 Hz while
/// resting, and on display changes). Main thread only.
pub fn follow_cursor() {
    with_ui(|ui| {
        let active = ui.take_anchor.is_some();
        ui.place(!active);
    });
}

/// Where the bar is: the anchor of its last placement and its display, or,
/// before the first one, where it would rest on the display under the
/// cursor (S-30 spotlight). Main thread only.
pub fn bar_placement() -> Option<(Anchor, Screen)> {
    if let Some(Some(placed)) = with_ui(|ui| ui.take_anchor.clone().or_else(|| ui.anchor.clone())) {
        return Some(placed);
    }
    let mtm = MainThreadMarker::new()?;
    let screens = native::screens(mtm);
    let i = placement::screen_under(&screens, native::cursor())?;
    let screen = screens[i].clone();
    Some((
        placement::anchor(&screen, Edge::Bottom, None, false),
        screen,
    ))
}

impl Ui {
    fn apply(&mut self, scene: Scene) {
        let before = (
            self.resting.is_visible(),
            self.resting_mode(),
            self.web_shown,
        );
        let old = self.scene.replace(scene.clone());
        let prefs_changed = old.as_ref().is_none_or(|o| {
            o.edge != scene.edge
                || o.x_by_display != scene.x_by_display
                || o.protected != scene.protected
                || o.dark != scene.dark
                || o.scratchpad_enabled != scene.scratchpad_enabled
                || o.texts != scene.texts
        });
        if prefs_changed {
            self.resting.set_texts(&scene.texts);
            self.resting
                .set_scratchpad_enabled(scene.scratchpad_enabled);
            self.resting.set_protected(scene.protected);
            self.resting.set_appearance(scene.dark);
        }
        let active = !matches!(scene.native, NativeScene::Rest | NativeScene::Hidden)
            || matches!(scene.web, WebScene::Shown(_));
        // The active bar stays where the take started; at rest it follows
        // the cursor.
        if active && self.take_anchor.is_none() {
            self.place(true);
            self.take_anchor = self.anchor.clone();
        } else if !active {
            self.take_anchor = None;
            if prefs_changed {
                self.place(true);
            }
        }

        // Web view first, so the native bar hides only once the web bar is
        // on screen (no blank frame between the two).
        match scene.web {
            WebScene::None => self.destroy_web(),
            WebScene::Loading => {
                self.ensure_web(&scene);
                if let NativeScene::Bridge(shape) = scene.native {
                    self.frame_web(shape);
                }
                self.hide_web(false);
            }
            WebScene::Lingering => self.hide_web(true),
            WebScene::Shown(shape) => {
                self.ensure_web(&scene);
                self.show_web(shape);
            }
        }
        if self.web_built && prefs_changed {
            if let Some(w) = self.app.get_webview_window(LABEL) {
                let _ = w.set_content_protected(scene.protected);
            }
        }
        match scene.native {
            NativeScene::Hidden => self.resting.hide(),
            NativeScene::Rest => {
                self.resting.set_mode(Mode::Rest);
                self.resting.show();
            }
            NativeScene::Bridge(shape) => {
                self.resting.set_mode(Mode::Bridge(shape));
                self.resting.show();
            }
        }
        let after = (
            self.resting.is_visible(),
            self.resting_mode(),
            self.web_shown,
        );
        if after != before {
            // Wall-clock stamp, to line up with the hotkey log (R5 §5 pill
            // appear latency, `docs/fork/MAC-TEST-SCRIPT.md` S-03).
            let unix_us = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_micros())
                .unwrap_or(0);
            log::info!(
                "pill: on screen native={:?} web={:?} (unix_us {unix_us})",
                after.0.then_some(after.1),
                after.2
            );
            log::debug!("pill: {}", self.describe());
        }
    }

    fn resting_mode(&self) -> NativeScene {
        match self.scene.as_ref().map(|s| s.native) {
            Some(n) => n,
            None => NativeScene::Hidden,
        }
    }

    fn describe(&self) -> String {
        format!(
            "resting_window={} web_built={} anchor={:?}",
            self.resting.window_number(),
            self.web_built,
            self.anchor
                .as_ref()
                .map(|(a, s)| (a.centre_x, a.edge_y, s.key.clone()))
        )
    }

    /// Computes the anchor on the display under the cursor (or keeps the
    /// take's anchor) and applies it to both windows.
    fn place(&mut self, follow: bool) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let Some(scene) = self.scene.clone() else {
            return;
        };
        let (anchor, screen) = if follow || self.anchor.is_none() {
            let screens = native::screens(mtm);
            let Some(i) = placement::screen_under(&screens, native::cursor()) else {
                return;
            };
            let screen = screens[i].clone();
            let frac = scene.x_by_display.get(&screen.key).copied();
            (placement::anchor(&screen, scene.edge, frac, false), screen)
        } else {
            match self.anchor.clone() {
                Some(a) => a,
                None => return,
            }
        };
        if self.anchor.as_ref().map(|(a, s)| (a, &s.key)) != Some((&anchor, &screen.key)) {
            log::debug!(
                "pill: placed on {} at x={} edge_y={} (scale {})",
                screen.key,
                anchor.centre_x,
                anchor.edge_y,
                anchor.scale
            );
        }
        self.resting.set_anchor(anchor, &screen);
        self.anchor = Some((anchor, screen));
        if let Some(shape) = self.web_shown {
            self.frame_web(shape);
        }
    }

    fn web_window(&self) -> Option<&'static NSWindow> {
        let w = self.app.get_webview_window(LABEL)?;
        let ptr = w.ns_window().ok()?;
        if ptr.is_null() {
            return None;
        }
        // SAFETY: Tauri keeps the NSWindow alive while the webview window
        // exists; callers use the reference only within this main-thread
        // call, before any destroy.
        Some(unsafe { &*(ptr as *const NSWindow) })
    }

    fn ensure_web(&mut self, scene: &Scene) {
        if self.web_built && self.app.get_webview_window(LABEL).is_some() {
            return;
        }
        let started = std::time::Instant::now();
        let (w, h) = Shape::Held.size();
        let built = PanelBuilder::<_, ForkPillWebPanel>::new(&self.app, LABEL)
            .url(WebviewUrl::App(URL.into()))
            .title("Wispr Bro")
            .size(tauri::Size::Logical(tauri::LogicalSize {
                width: w,
                height: h,
            }))
            .level(PanelLevel::Status)
            .has_shadow(false)
            .transparent(true)
            // Never `.no_activate(true)`: tauri-nspanel implements it by
            // switching the whole app to the Prohibited activation policy
            // while it builds the window, which deactivates Wispr Bro when
            // one of its own windows is in front (S-24.1: a take started in
            // the Scratchpad note handed focus back to the previous app
            // within 0.2 s). The window is built hidden and unfocused
            // (`visible(false)`, `focused(false)`), so tao never activates
            // the app for it, and the non-activating style mask plus
            // `prevent_activation` cover clicks.
            .hides_on_deactivate(false)
            .corner_radius(0.0)
            .style_mask(StyleMask::empty().borderless().nonactivating_panel())
            .with_window(|w| {
                w.decorations(false)
                    .transparent(true)
                    .focusable(false)
                    .focused(false)
                    .accept_first_mouse(true)
                    .skip_taskbar(true)
                    .visible(false)
            })
            .collection_behavior(
                CollectionBehavior::new()
                    .can_join_all_spaces()
                    .full_screen_auxiliary()
                    .stationary()
                    .ignores_cycle(),
            )
            .build();
        match built {
            Ok(panel) => {
                self.web_built = true;
                self.web_shown = None;
                if let Some(w) = self.app.get_webview_window(LABEL) {
                    let _ = w.set_content_protected(scene.protected);
                }
                if let Some(win) = self.web_window() {
                    win.setIgnoresMouseEvents(true);
                    // SAFETY: plain setter.
                    unsafe {
                        let _: () = msg_send![win, setAnimationBehavior: 2isize /* None */];
                    }
                    prevent_activation(win);
                }
                self.frame_web(Shape::Held);
                // Ordered in (behind the native bar, see `hide_web`) so
                // WebKit loads and paints it.
                panel.show();
                log::info!("pill: web view built in {:?}", started.elapsed());
            }
            Err(e) => log::error!("pill: cannot build the web view: {e}"),
        }
    }

    fn frame_web(&self, shape: Shape) {
        let Some((anchor, _)) = self.take_anchor.clone().or_else(|| self.anchor.clone()) else {
            return;
        };
        let r = placement::frame(&anchor, shape.size());
        if let Some(win) = self.web_window() {
            let target = NSRect::new(NSPoint::new(r.x, r.y), NSSize::new(r.w, r.h));
            if win.frame() != target {
                win.setFrame_display(target, true);
            }
        }
    }

    fn show_web(&mut self, shape: Shape) {
        if self.web_shown == Some(shape) {
            return;
        }
        self.frame_web(shape);
        if let Some(win) = self.web_window() {
            win.setIgnoresMouseEvents(false);
            win.setAlphaValue(1.0);
            win.orderFrontRegardless();
        }
        if self.web_shown.is_none() {
            super::WEB_SHOWN.store(true, std::sync::atomic::Ordering::Release);
        }
        self.web_shown = Some(shape);
    }

    /// `order_out`: also take the window off screen (lingering after a
    /// take); otherwise keep it ordered in behind the native bar (loading).
    fn hide_web(&mut self, order_out: bool) {
        super::WEB_SHOWN.store(false, std::sync::atomic::Ordering::Release);
        self.web_shown = None;
        let native = self.resting.window_number();
        if let Some(win) = self.web_window() {
            win.setIgnoresMouseEvents(true);
            if order_out {
                if win.isVisible() {
                    win.orderOut(None);
                }
            } else {
                // Ordered in at full alpha (WebKit sees a visible page and
                // renders normally) but behind the native bar's bridge
                // capsule of the same size: a fresh page is empty and
                // transparent, a reused one is covered until it has painted
                // the new state.
                win.setAlphaValue(1.0);
                win.orderWindow_relativeTo(NSWindowOrderingMode::Below, native);
            }
        }
    }

    fn destroy_web(&mut self) {
        if !self.web_built {
            return;
        }
        super::WEB_SHOWN.store(false, std::sync::atomic::Ordering::Release);
        self.web_built = false;
        self.web_shown = None;
        if let Some(win) = self.web_window() {
            win.orderOut(None);
        }
        let window = match self.app.get_webview_panel(LABEL) {
            Ok(panel) => panel.to_window(),
            Err(_) => self.app.get_webview_window(LABEL),
        };
        if let Some(w) = window {
            if let Err(e) = w.destroy() {
                log::warn!("pill: cannot destroy the web view: {e}");
            } else {
                log::info!("pill: web view destroyed");
            }
        }
    }

    fn reassert(&self) {
        self.resting.reassert();
        if self.web_shown.is_some() {
            if let Some(win) = self.web_window() {
                win.orderFrontRegardless();
            }
        }
    }
}

/// DEC-28 fallback, as RS-03 specified it: `_setPreventsActivation:` set
/// once at creation, never toggled, behind a feature check. S-03 on macOS
/// 26.6.2: with the app's Dock icon on (a Regular app, unlike the RS-03
/// spike, which ran as an Accessory app), a click on the web view's ✓ made
/// the app frontmost (the paste then went to Wispr Bro instead of the
/// user's app); the native resting bar never did.
fn prevent_activation(win: &NSWindow) {
    // Diagnostic for re-running the DEC-28 check on a new macOS
    // (`docs/fork/MAC-TEST-SCRIPT.md` S-03): leave the flag off.
    if std::env::var_os("WISPRBRO_PILL_ALLOW_ACTIVATION").is_some() {
        log::warn!("pill: WISPRBRO_PILL_ALLOW_ACTIVATION set; clicks may activate the app");
        return;
    }
    let selector = sel!(_setPreventsActivation:);
    // SAFETY: a feature check, then the private setter with a BOOL.
    unsafe {
        let responds: bool = msg_send![win, respondsToSelector: selector];
        if responds {
            let _: () = msg_send![win, _setPreventsActivation: true];
            log::debug!("pill: web view panel set to prevent activation (DEC-28)");
        } else {
            log::warn!("pill: _setPreventsActivation: is missing; clicks may activate the app");
        }
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements; the observer has no
    // state and only calls back into the main-thread UI.
    #[unsafe(super(NSObject))]
    #[name = "WisprBroPillObserver"]
    struct Observer;

    impl Observer {
        #[unsafe(method(screensChanged:))]
        fn screens_changed(&self, _n: &NSNotification) {
            log::debug!("pill: screen parameters changed");
            with_ui(|ui| {
                // Displays changed under a running take: re-place it too.
                ui.take_anchor = None;
                let active = ui.web_shown.is_some();
                ui.place(true);
                if active {
                    ui.take_anchor = ui.anchor.clone();
                }
            });
        }

        #[unsafe(method(spaceChanged:))]
        fn space_changed(&self, _n: &NSNotification) {
            // A safety net: RS-03 saw the bar stay over full-screen apps
            // without it.
            with_ui(|ui| ui.reassert());
        }
    }
);

fn observe(_mtm: MainThreadMarker) {
    let this = Observer::alloc().set_ivars(());
    // SAFETY: NSObject's init.
    let observer: Retained<Observer> = unsafe { msg_send![super(this), init] };
    let obj: &AnyObject = &observer;
    // SAFETY: the selectors exist with the `(NSNotification*)` signature;
    // the observer is never deallocated (forgotten below).
    unsafe {
        NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
            obj,
            sel!(screensChanged:),
            Some(NSApplicationDidChangeScreenParametersNotification),
            None,
        );
        NSWorkspace::sharedWorkspace()
            .notificationCenter()
            .addObserver_selector_name_object(
                obj,
                sel!(spaceChanged:),
                Some(NSWorkspaceActiveSpaceDidChangeNotification),
                None,
            );
    }
    std::mem::forget(observer);
}
