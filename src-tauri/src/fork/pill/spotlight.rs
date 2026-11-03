// SPDX-License-Identifier: GPL-3.0-or-later
//! The bar spotlight (S-30, A-16, W-08): after "Try it now" in the
//! first-use chooser, the screen dims, the Flow Bar stays lit in a cut-out,
//! and a hint above it says where to go next ("Open your email, click where
//! you want to write, hold fn and speak").
//!
//! - One click-through panel, `fork-spotlight` (a web view, page
//!   `src/fork/spotlight/index.html`), on the display the bar is on, one
//!   level below the bar (`NSMainMenuWindowLevel`, the bar is at status
//!   level), on every Space and over full-screen apps. It ignores the mouse,
//!   never activates the app and never becomes key: the user keeps working
//!   through it.
//! - The resting bar is shown for the whole spotlight, even with "Always
//!   show the bar" off or "Hide for an hour" running ([`is_active`] is read
//!   by the bar's preferences).
//! - The cut-out follows the bar (it moves with the cursor's display, 2 Hz).
//! - It ends on the first take pasted into another app
//!   ([`on_take_pasted`]), on Esc (observed through the S-01 registry, never
//!   blocked; an Esc that cancelled a take belongs to the take), after
//!   [`MAX_MS`], or from the Hub (`fork_spotlight_stop`). The window is
//!   destroyed then, so its WebContent process exits.
//!
//! The rules ([`Machine`]) and the layout ([`layout`]) are pure and tested
//! here; the window is macOS-only.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use specta::Type;

use super::placement::{Anchor, Edge, Screen};

/// Window label (R5 §4.1) and page (a Vite input, `src/fork/vite-inputs.ts`).
pub const LABEL: &str = "fork-spotlight";
pub const URL: &str = "src/fork/spotlight/index.html";

/// The chooser's options (`src/fork/onboarding/firstUse.ts` lists the same
/// ids in the same order; a test keeps them in step). Each has a scene in
/// the chooser and a hint here (`rust.spotlight.hint.<id>` in `fork.json`).
pub const CHOICES: &[&str] = &[
    "ai_chat", "note", "message", "email", "comment", "document", "coding",
];

/// The spotlight gives up after ten minutes (a user who walked away does
/// not come back to a dimmed screen for the rest of the day).
pub const MAX_MS: u64 = 10 * 60 * 1000;
/// The cut-out around the bar, in points: room for the active bar
/// ("Transcribing…" is 208 pt) with a margin.
pub const HOLE_W: f64 = 248.0;
pub const HOLE_H: f64 = 64.0;

pub fn valid_choice(choice: &str) -> bool {
    CHOICES.contains(&choice)
}

/// The `fork.json` key of a choice's hint.
pub fn hint_key(choice: &str) -> String {
    format!("rust.spotlight.hint.{choice}")
}

/// A rectangle in the page's coordinates: CSS pixels (= points) from the
/// top-left corner of the display.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct SpotlightRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SpotlightEdge {
    Bottom,
    Top,
}

/// What the `fork-spotlight` page shows (event, and `fork_spotlight_state`
/// when the page loads). Also sent to the Hub, which shows "the hint is on
/// screen" with a Stop button.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkSpotlightState {
    /// Increases with every change; a lower one never replaces a higher one.
    pub seq: u32,
    pub active: bool,
    /// The chooser option (one of [`CHOICES`]).
    pub choice: Option<String>,
    /// The cut-out around the bar, `None` until the bar is placed.
    pub hole: Option<SpotlightRect>,
    /// Which display edge the bar rests on (the hint goes on the other side
    /// of the cut-out).
    pub edge: SpotlightEdge,
}

/// Why a spotlight ended (logged; the Hub shows nothing for any of them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
    /// The first take pasted into another app.
    Dictated,
    Esc,
    Timeout,
    /// From the Hub ("Stop the hint") or a new chooser run.
    Stopped,
}

/// The cut-out for the bar at `anchor` on `screen`, in page coordinates
/// (the panel covers `screen.frame`). Kept inside the display.
pub fn layout(screen: &Screen, anchor: &Anchor) -> (SpotlightRect, SpotlightEdge) {
    let f = screen.frame;
    let (_, rest_h) = super::geometry::Shape::Rest.size();
    // The bar's vertical centre in AppKit coordinates (y up).
    let centre_y = match anchor.edge {
        Edge::Bottom => anchor.edge_y + rest_h / 2.0,
        Edge::Top => anchor.edge_y - rest_h / 2.0,
    };
    let w = HOLE_W.min(f.w);
    let h = HOLE_H.min(f.h);
    let x = (anchor.centre_x - w / 2.0).clamp(f.x, f.max_x() - w) - f.x;
    // Flip to top-left page coordinates.
    let top = f.max_y() - (centre_y + h / 2.0);
    let y = top.clamp(0.0, f.h - h);
    let edge = match anchor.edge {
        Edge::Bottom => SpotlightEdge::Bottom,
        Edge::Top => SpotlightEdge::Top,
    };
    (
        SpotlightRect {
            x: x.round(),
            y: y.round(),
            w,
            h,
        },
        edge,
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Active {
    choice: String,
    started_ms: u64,
}

/// The dismissal rules, pure over time and events.
#[derive(Debug, Default)]
pub struct Machine {
    active: Option<Active>,
}

impl Machine {
    pub const fn new() -> Machine {
        Machine { active: None }
    }

    pub fn choice(&self) -> Option<&str> {
        self.active.as_ref().map(|a| a.choice.as_str())
    }

    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    /// Starts (or restarts with another choice) the spotlight.
    pub fn start(&mut self, choice: &str, now_ms: u64) -> Result<(), String> {
        if !valid_choice(choice) {
            return Err(format!("unknown first-use choice {choice:?}"));
        }
        self.active = Some(Active {
            choice: choice.to_owned(),
            started_ms: now_ms,
        });
        Ok(())
    }

    fn end(&mut self, reason: EndReason) -> Option<EndReason> {
        self.active.take().map(|_| reason)
    }

    /// Esc. One that cancelled a take is the take's: the user is mid-try.
    pub fn on_esc(&mut self, cancelled_take: bool) -> Option<EndReason> {
        if cancelled_take {
            return None;
        }
        self.end(EndReason::Esc)
    }

    /// A take was pasted. Into Wispr Bro itself (a Scratchpad note, a
    /// field in the Hub) it is not the real use the hint asks for.
    pub fn on_take_pasted(&mut self, own_app: bool) -> Option<EndReason> {
        if own_app {
            return None;
        }
        self.end(EndReason::Dictated)
    }

    pub fn on_tick(&mut self, now_ms: u64) -> Option<EndReason> {
        let expired = self
            .active
            .as_ref()
            .is_some_and(|a| now_ms.saturating_sub(a.started_ms) >= MAX_MS);
        if expired {
            self.end(EndReason::Timeout)
        } else {
            None
        }
    }

    pub fn stop(&mut self) -> Option<EndReason> {
        self.end(EndReason::Stopped)
    }
}

static MACHINE: Mutex<Machine> = Mutex::new(Machine::new());
static SEQ: AtomicU32 = AtomicU32::new(0);
static LAST: Mutex<Option<ForkSpotlightState>> = Mutex::new(None);

fn machine() -> std::sync::MutexGuard<'static, Machine> {
    MACHINE.lock().unwrap_or_else(|e| e.into_inner())
}

/// The spotlight is on screen (the bar's preferences show the resting bar).
pub fn is_active() -> bool {
    machine().is_active()
}

/// What the page should show now.
pub fn state() -> ForkSpotlightState {
    if let Some(last) = LAST.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return last;
    }
    ForkSpotlightState {
        seq: SEQ.load(Ordering::Acquire),
        active: false,
        choice: None,
        hole: None,
        edge: SpotlightEdge::Bottom,
    }
}

fn publish(active: bool, choice: Option<String>, hole: Option<(SpotlightRect, SpotlightEdge)>) {
    let seq = SEQ.fetch_add(1, Ordering::AcqRel) + 1;
    let state = ForkSpotlightState {
        seq,
        active,
        choice,
        hole: hole.map(|(r, _)| r),
        edge: hole.map_or(SpotlightEdge::Bottom, |(_, e)| e),
    };
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) = Some(state.clone());
    #[cfg(target_os = "macos")]
    mac::emit(&state);
}

fn monotonic_ms() -> u64 {
    super::monotonic_ms()
}

/// `fork_spotlight_start`: `Ok(true)` when the spotlight is on screen,
/// `Ok(false)` where there is no bar to point at (not macOS, no product
/// runtime, the bar failed to start).
pub fn start(choice: &str) -> Result<bool, String> {
    if !valid_choice(choice) {
        return Err(format!("unknown first-use choice {choice:?}"));
    }
    #[cfg(target_os = "macos")]
    {
        if !super::is_running() || !mac::ready() {
            return Ok(false);
        }
        machine().start(choice, monotonic_ms())?;
        log::info!("spotlight: on ({choice})");
        publish(true, Some(choice.to_owned()), None);
        super::refresh();
        mac::show(choice.to_owned());
        Ok(true)
    }
    #[cfg(not(target_os = "macos"))]
    Ok(false)
}

fn ended(reason: Option<EndReason>) {
    let Some(reason) = reason else {
        return;
    };
    log::info!("spotlight: off ({reason:?})");
    publish(false, None, None);
    super::refresh();
    #[cfg(target_os = "macos")]
    mac::hide();
}

/// `fork_spotlight_stop`.
pub fn stop() {
    let reason = machine().stop();
    ended(reason);
}

/// A take was pasted (`fork::onboarding::checklist::on_take_pasted`).
pub fn on_take_pasted(own_app: bool) {
    let reason = machine().on_take_pasted(own_app);
    ended(reason);
}

fn on_esc(cancelled_take: bool) {
    let reason = machine().on_esc(cancelled_take);
    ended(reason);
}

fn on_tick() {
    let reason = machine().on_tick(monotonic_ms());
    ended(reason);
}

/// The new cut-out (the bar moved), from the main thread.
#[cfg(target_os = "macos")]
fn moved(hole: (SpotlightRect, SpotlightEdge)) {
    let choice = machine().choice().map(str::to_owned);
    if choice.is_some() {
        publish(true, choice, Some(hole));
    }
}

/// From `pill::start` (macOS, main thread): keeps the app handle and
/// observes Esc.
#[cfg(target_os = "macos")]
pub fn init(app: &tauri::AppHandle<tauri::Wry>) {
    mac::init(app);
    crate::fork::shortcut::registry().observe_esc(std::sync::Arc::new(|ctx| {
        on_esc(ctx.cancelled_take);
    }));
    // Mac test S-30 (`scripts/wb-s30/run.sh`): start the spotlight a few
    // seconds after launch without clicking through the Hub. Test
    // identities only (`com.wisprbro.<slice>check`).
    if let Some(choice) = std::env::var_os("WISPRBRO_S30_SPOTLIGHT") {
        let id = app.config().identifier.clone();
        if id.starts_with("com.wisprbro.") && id.ends_with("check") {
            let choice = choice.to_string_lossy().into_owned();
            let _ = std::thread::Builder::new()
                .name("wb-spotlight-test".into())
                .spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(4));
                    match start(&choice) {
                        Ok(on) => log::info!("spotlight: test start ({choice}) -> {on}"),
                        Err(e) => log::warn!("spotlight: test start failed: {e}"),
                    }
                });
        }
    }
}

#[cfg(target_os = "macos")]
mod mac {
    //! The `fork-spotlight` panel on the main thread.

    // tauri_panel!'s generated code uses `-> ()` return types.
    #![allow(clippy::unused_unit)]

    use std::cell::RefCell;
    use std::sync::OnceLock;
    use std::time::Duration;

    use objc2::{msg_send, MainThreadMarker};
    use objc2_app_kit::NSWindow;
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    use tauri::{AppHandle, Manager, WebviewUrl, Wry};
    use tauri_nspanel::{CollectionBehavior, ManagerExt, PanelBuilder, PanelLevel, StyleMask};

    use super::{layout, ForkSpotlightState, SpotlightEdge, SpotlightRect, LABEL, URL};

    mod panel_class {
        use tauri::Manager;
        use tauri_nspanel::tauri_panel;

        tauri_panel! {
            panel!(ForkSpotlightPanel {
                config: {
                    can_become_key_window: false,
                    can_become_main_window: false,
                    is_floating_panel: true
                }
            })
        }
    }
    use panel_class::ForkSpotlightPanel;

    static APP: OnceLock<AppHandle<Wry>> = OnceLock::new();
    /// How often the cut-out re-checks where the bar is (the bar follows the
    /// cursor's display at 2 Hz) and the time limit.
    const FOLLOW_MS: u64 = 500;

    thread_local! {
        /// Main thread: the frame and cut-out last applied.
        static PLACED: RefCell<Option<(NSRect, SpotlightRect, SpotlightEdge)>> =
            const { RefCell::new(None) };
    }

    pub fn init(app: &AppHandle<Wry>) {
        let _ = APP.set(app.clone());
    }

    pub fn ready() -> bool {
        APP.get().is_some()
    }

    pub fn emit(state: &ForkSpotlightState) {
        if let Some(app) = APP.get() {
            if let Err(e) = tauri_specta::Event::emit(state, app) {
                log::warn!("spotlight: could not emit the state: {e}");
            }
        }
    }

    pub fn show(choice: String) {
        let Some(app) = APP.get().cloned() else {
            return;
        };
        let main = app.clone();
        let _ = main.run_on_main_thread(move || {
            build(&app);
            follow(&app);
            announce(&app, &choice);
        });
        start_follower();
    }

    pub fn hide() {
        let Some(app) = APP.get().cloned() else {
            return;
        };
        let main = app.clone();
        let _ = main.run_on_main_thread(move || destroy(&app));
    }

    fn start_follower() {
        use std::sync::atomic::{AtomicBool, Ordering};
        static RUNNING: AtomicBool = AtomicBool::new(false);
        if RUNNING.swap(true, Ordering::AcqRel) {
            return;
        }
        let spawned = std::thread::Builder::new()
            .name("wb-spotlight".into())
            .spawn(|| {
                while super::is_active() {
                    std::thread::sleep(Duration::from_millis(FOLLOW_MS));
                    super::on_tick();
                    if let Some(app) = APP.get().cloned() {
                        let main = app.clone();
                        let _ = main.run_on_main_thread(move || {
                            if super::is_active() {
                                follow(&app);
                            }
                        });
                    }
                }
                RUNNING.store(false, Ordering::Release);
            });
        if let Err(e) = spawned {
            log::warn!("spotlight: cannot start the follower: {e}");
            RUNNING.store(false, Ordering::Release);
        }
    }

    fn window(app: &AppHandle<Wry>) -> Option<&'static NSWindow> {
        let w = app.get_webview_window(LABEL)?;
        let ptr = w.ns_window().ok()?;
        if ptr.is_null() {
            return None;
        }
        // SAFETY: Tauri keeps the NSWindow alive while the webview window
        // exists; it is used only within this main-thread call.
        Some(unsafe { &*(ptr as *const NSWindow) })
    }

    fn build(app: &AppHandle<Wry>) {
        if app.get_webview_window(LABEL).is_some() {
            return;
        }
        PLACED.with(|p| *p.borrow_mut() = None);
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        // S-28.8: wry activates the app at the end of every web view build,
        // which unhides a hidden app (and its Hub); see `activation.rs`.
        let built = super::super::activation::without_app_activation(mtm, || {
            PanelBuilder::<_, ForkSpotlightPanel>::new(app, LABEL)
                .url(WebviewUrl::App(URL.into()))
                .title("Wispr Bro")
                .size(tauri::Size::Logical(tauri::LogicalSize {
                    width: 800.0,
                    height: 600.0,
                }))
                // One below the bar (status level, 25), above every app window,
                // full-screen ones included.
                .level(PanelLevel::MainMenu)
                .has_shadow(false)
                .transparent(true)
                // No `.no_activate(true)`: tauri-nspanel flips the whole app to
                // the Prohibited activation policy while it builds, which
                // deactivates the Hub the chooser's "Try it now" was clicked in
                // (see `panel.rs`, S-24.1). Built hidden and unfocused with the
                // non-activating mask, the panel never activates the app.
                .hides_on_deactivate(false)
                .corner_radius(0.0)
                .style_mask(StyleMask::empty().borderless().nonactivating_panel())
                .with_window(|w| {
                    w.decorations(false)
                        .transparent(true)
                        .focusable(false)
                        .focused(false)
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
                .build()
        });
        match built {
            Ok(panel) => {
                if let Some(win) = window(app) {
                    // Click-through: every click reaches the app below.
                    win.setIgnoresMouseEvents(true);
                    // SAFETY: plain setter (no window animation).
                    unsafe {
                        let _: () = msg_send![win, setAnimationBehavior: 2isize];
                    }
                    super::super::native::stay_when_app_hidden(win);
                }
                panel.show();
                log::info!("spotlight: panel built");
            }
            Err(e) => log::error!("spotlight: cannot build the panel: {e}"),
        }
    }

    /// Covers the bar's display and sends the cut-out when it changed.
    fn follow(app: &AppHandle<Wry>) {
        let Some((anchor, screen)) = crate::fork::pill::bar_placement() else {
            return;
        };
        let hole = layout(&screen, &anchor);
        let f = screen.frame;
        let frame = NSRect::new(NSPoint::new(f.x, f.y), NSSize::new(f.w, f.h));
        let changed = PLACED.with(|p| {
            let next = Some((frame, hole.0, hole.1));
            let changed = *p.borrow() != next;
            *p.borrow_mut() = next;
            changed
        });
        if !changed {
            if let Some(win) = window(app) {
                // Spaces and full-screen switches: stay in front.
                win.orderFrontRegardless();
            }
            return;
        }
        if let Some(win) = window(app) {
            win.setFrame_display(frame, true);
            win.orderFrontRegardless();
        }
        super::moved(hole);
    }

    fn destroy(app: &AppHandle<Wry>) {
        PLACED.with(|p| *p.borrow_mut() = None);
        if let Some(win) = window(app) {
            win.orderOut(None);
        }
        let window = match app.get_webview_panel(LABEL) {
            Ok(panel) => panel.to_window(),
            Err(_) => app.get_webview_window(LABEL),
        };
        if let Some(w) = window {
            match w.destroy() {
                Ok(()) => log::info!("spotlight: panel destroyed"),
                Err(e) => log::warn!("spotlight: cannot destroy the panel: {e}"),
            }
        }
    }

    /// VoiceOver users hear the hint (the dim is not something they see).
    fn announce(app: &AppHandle<Wry>, choice: &str) {
        let locale = crate::fork::i18n::current_locale(Some(
            &crate::settings::get_settings(app).app_language,
        ));
        let text = format!(
            "{} {}",
            crate::fork::i18n::t(locale, &super::hint_key(choice), &[]),
            crate::fork::i18n::t(locale, "rust.spotlight.escape", &[])
        );
        crate::fork::pill::accessibility::announce_main(&text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::pill::placement::{self, Rect};

    fn screen() -> Screen {
        Screen {
            key: "Built-in 1512x982".into(),
            frame: Rect::new(0.0, 0.0, 1512.0, 982.0),
            visible: Rect::new(0.0, 70.0, 1512.0, 875.0),
            scale: 2.0,
        }
    }

    #[test]
    fn hole_sits_on_the_bar_at_the_bottom() {
        let s = screen();
        let a = placement::anchor(&s, Edge::Bottom, None, false);
        let (hole, edge) = layout(&s, &a);
        assert_eq!(edge, SpotlightEdge::Bottom);
        // Centred on the bar.
        assert_eq!(hole.x + hole.w / 2.0, 756.0);
        // The bar (12 pt high, its bottom 8 pt above the visible frame at
        // y = 70 in AppKit coordinates) is inside the cut-out.
        let bar_top_page = 982.0 - (70.0 + 8.0 + 12.0);
        let bar_bottom_page = 982.0 - (70.0 + 8.0);
        assert!(hole.y <= bar_top_page && hole.y + hole.h >= bar_bottom_page);
    }

    #[test]
    fn hole_sits_on_the_bar_at_the_top_and_stays_on_screen() {
        let s = screen();
        let a = placement::anchor(&s, Edge::Top, Some(0.0), false);
        let (hole, edge) = layout(&s, &a);
        assert_eq!(edge, SpotlightEdge::Top);
        assert!(hole.x >= 0.0 && hole.x + hole.w <= 1512.0);
        assert!(hole.y >= 0.0);
        let bar_top_page = 982.0 - (945.0 - 8.0);
        assert!(hole.y <= bar_top_page && hole.y + hole.h >= bar_top_page + 12.0);
    }

    #[test]
    fn hole_is_relative_to_a_secondary_display() {
        let s = Screen {
            key: "Studio 2560x1440".into(),
            frame: Rect::new(-2560.0, 200.0, 2560.0, 1440.0),
            visible: Rect::new(-2560.0, 200.0, 2560.0, 1415.0),
            scale: 1.0,
        };
        let a = placement::anchor(&s, Edge::Bottom, None, false);
        let (hole, _) = layout(&s, &a);
        assert_eq!(hole.x + hole.w / 2.0, 1280.0);
        assert!(hole.y > 1300.0 && hole.y + hole.h <= 1440.0);
    }

    #[test]
    fn esc_ends_it_unless_it_cancelled_a_take() {
        let mut m = Machine::new();
        m.start("email", 0).unwrap();
        assert_eq!(m.on_esc(true), None);
        assert!(m.is_active());
        assert_eq!(m.on_esc(false), Some(EndReason::Esc));
        assert!(!m.is_active());
        // Nothing to end twice.
        assert_eq!(m.on_esc(false), None);
    }

    #[test]
    fn only_a_take_in_another_app_ends_it() {
        let mut m = Machine::new();
        m.start("note", 0).unwrap();
        assert_eq!(m.on_take_pasted(true), None);
        assert_eq!(m.choice(), Some("note"));
        assert_eq!(m.on_take_pasted(false), Some(EndReason::Dictated));
        assert_eq!(m.on_take_pasted(false), None);
    }

    #[test]
    fn it_times_out_and_can_be_stopped_or_restarted() {
        let mut m = Machine::new();
        m.start("coding", 1_000).unwrap();
        assert_eq!(m.on_tick(1_000 + MAX_MS - 1), None);
        assert_eq!(m.on_tick(1_000 + MAX_MS), Some(EndReason::Timeout));
        m.start("coding", 0).unwrap();
        m.start("email", 5).unwrap();
        assert_eq!(m.choice(), Some("email"));
        assert_eq!(m.stop(), Some(EndReason::Stopped));
        assert_eq!(m.stop(), None);
    }

    #[test]
    fn unknown_choices_are_refused() {
        let mut m = Machine::new();
        assert!(m.start("tweet", 0).is_err());
        assert!(!m.is_active());
        assert!(start("../x").is_err());
    }

    #[test]
    fn every_choice_has_a_hint_and_matches_the_chooser() {
        let en: serde_json::Value =
            serde_json::from_str(include_str!("../../../../src/i18n/locales/en/fork.json"))
                .unwrap();
        for choice in CHOICES {
            let hint = en["rust"]["spotlight"]["hint"][choice].as_str();
            assert!(
                hint.is_some_and(|h| !h.trim().is_empty()),
                "no hint for {choice}"
            );
            assert_eq!(hint_key(choice), format!("rust.spotlight.hint.{choice}"));
        }
        assert!(en["rust"]["spotlight"]["escape"].as_str().is_some());
        // The chooser lists the same ids in the same order.
        let ts = include_str!("../../../../src/fork/onboarding/firstUse.ts");
        let start = ts
            .find("FIRST_USE_CHOICES")
            .expect("FIRST_USE_CHOICES in firstUse.ts");
        let list = &ts[start..ts[start..].find(']').map(|i| start + i).unwrap()];
        let ids: Vec<&str> = list.split('"').skip(1).step_by(2).collect();
        assert_eq!(ids, CHOICES);
    }
}
