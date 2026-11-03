// SPDX-License-Identifier: GPL-3.0-or-later
//! Hidden windows give their web content back (S-38, R5 §5.2).
//!
//! §5.2 caps the idle app (bar visible, models unloaded) at 250 MB RSS for
//! the app plus every WebKit process. S-03 measured 287-293 MB, of which the
//! Hub's hidden web view was ≈ 106 MB: Handy closes the main window by hiding
//! it (`lib.rs` `CloseRequested`), and a login-item start builds it hidden,
//! so its WebContent process lives for the whole session. The Scratchpad
//! window (S-24) is hidden the same way when closed.
//!
//! A *parkable* window that stays hidden for [`PARK_AFTER`] is destroyed,
//! which ends its WebContent process. It is rebuilt when it is needed again:
//!
//! - `main` by [`ensure_main`], first in Handy's `show_main_window` (the one
//!   `lib.rs` hook; tray Settings, Dock click, second launch, the command)
//!   and in the fork's own show paths ([`show_main_then`]). The rebuilt
//!   window has Handy's builder settings, the Hub's chrome
//!   ([`super::style_main`]) and the frame it had when it was parked. Events
//!   that must reach the new page (a notification's Open, View changes) wait
//!   until its listeners are up: the page calls `fork_hub_ready` then
//!   ([`hub_ready`]).
//! - `fork-scratchpad` by its own `open`, which already builds the window
//!   when there is none.
//!
//! The main window is parked only after its page reported ready once, so
//! Handy's `App.tsx` start-up work (Enigo, shortcuts, device lists) has run;
//! a page that never reports ready is never parked. A minimized window is
//! not hidden and is never parked. With every Tauri window gone, Tauri asks
//! to exit (`ExitRequested` without a code); [`keeps_running`] makes the
//! fork plugin refuse that while the Hub is parked. A real quit always has a
//! code.
//!
//! macOS only (the reference platform); elsewhere nothing is parked.
//! `WB_DEBUG_PARK_AFTER_S` (debug builds) sets the delay in seconds, `off`
//! disables parking (Mac test S-38: before/after RSS on one build).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};

use crate::fork::ForkRuntime;

/// Handy's main window (the Hub).
pub const MAIN: &str = "main";

/// Windows that are destroyed while hidden and rebuilt on demand.
pub const PARKABLE: [&str; 2] = [MAIN, crate::fork::scratchpad::WINDOW_LABEL];

/// How long a parkable window stays hidden before it is destroyed. Long
/// enough that closing and reopening the Hub right away costs nothing, short
/// against an idle session.
pub const PARK_AFTER: Duration = Duration::from_secs(60);

/// Debug builds: the delay in seconds, or `off`.
#[cfg(debug_assertions)]
pub const PARK_AFTER_ENV: &str = "WB_DEBUG_PARK_AFTER_S";

/// Events for the main window's page that wait for it to be ready.
type Pending = Box<dyn FnOnce() + Send>;

/// The main window's frame in logical points, kept while it is parked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub position: (f64, f64),
    pub size: (f64, f64),
}

#[derive(Default)]
struct State {
    /// Bumped whenever a window is hidden or shown; a scheduled park runs
    /// only if its generation is still current.
    generation: HashMap<&'static str, u64>,
    /// The main window was destroyed by [`park`] and not rebuilt yet.
    main_parked: bool,
    /// The current main page called `fork_hub_ready`.
    main_ready: bool,
    /// Where the main window was when it was parked.
    main_frame: Option<Frame>,
    pending: Vec<Pending>,
}

/// Fork state for parking (managed in the plugin's setup).
pub struct Parking {
    after: Option<Duration>,
    state: Mutex<State>,
}

impl Default for Parking {
    fn default() -> Self {
        Self::with_delay(default_delay())
    }
}

impl Parking {
    /// Parking after `after` (`None`: never).
    pub fn with_delay(after: Option<Duration>) -> Self {
        Self {
            after: after.filter(|_| cfg!(target_os = "macos") || cfg!(test)),
            state: Mutex::new(State::default()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Starts a new generation for `label` and returns it.
    fn bump(&self, label: &'static str) -> u64 {
        let mut state = self.lock();
        let generation = state.generation.entry(label).or_default();
        *generation += 1;
        *generation
    }

    fn current(&self, label: &'static str) -> u64 {
        self.lock().generation.get(label).copied().unwrap_or_default()
    }

    /// `true` while the main window is parked.
    pub fn main_parked(&self) -> bool {
        self.lock().main_parked
    }

    /// `true` once the current main page reported ready.
    pub fn main_ready(&self) -> bool {
        self.lock().main_ready
    }

    /// Marks the main window parked at `frame`, as [`park`] does before the
    /// destroy (tests: the mock runtime reports every window visible).
    #[cfg(test)]
    pub(crate) fn mark_main_parked(&self, frame: Option<Frame>) {
        let mut state = self.lock();
        state.main_parked = true;
        state.main_ready = false;
        state.main_frame = frame;
    }

    /// Whether a park of `label` scheduled in `generation` may destroy the
    /// window now; `shown` is the window's state (`None`: no such window).
    pub fn decide(
        &self,
        label: &'static str,
        generation: u64,
        shown: Option<WindowShown>,
    ) -> Result<(), Kept> {
        if self.current(label) != generation {
            return Err(Kept::Superseded);
        }
        let shown = shown.ok_or(Kept::Gone)?;
        if shown.visible {
            return Err(Kept::Visible);
        }
        if shown.minimized {
            return Err(Kept::Minimized);
        }
        if label == MAIN && !self.main_ready() {
            return Err(Kept::NotReady);
        }
        Ok(())
    }
}

/// What the window server says about a window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowShown {
    pub visible: bool,
    pub minimized: bool,
}

fn default_delay() -> Option<Duration> {
    #[cfg(debug_assertions)]
    if let Ok(value) = std::env::var(PARK_AFTER_ENV) {
        return parse_delay(&value);
    }
    Some(PARK_AFTER)
}

/// `WB_DEBUG_PARK_AFTER_S`: seconds, or `off`/`never`/`0` for no parking.
pub fn parse_delay(value: &str) -> Option<Duration> {
    match value.trim() {
        "off" | "never" | "0" => None,
        v => v
            .parse::<u64>()
            .ok()
            .map(Duration::from_secs)
            .or(Some(PARK_AFTER)),
    }
}

/// The label as one of [`PARKABLE`].
fn parkable(label: &str) -> Option<&'static str> {
    PARKABLE.iter().copied().find(|l| *l == label)
}

/// Registers [`Parking`] (plugin setup).
pub fn register<R: tauri::Runtime>(app: &AppHandle<R>) {
    app.manage(Parking::default());
}

/// Parks `window` whenever it is closed (Handy hides it) and stays hidden.
pub fn watch<R: ForkRuntime>(app: &AppHandle<R>, window: &WebviewWindow<R>) {
    let Some(label) = parkable(window.label()) else {
        return;
    };
    let app = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { .. } = event {
            schedule(&app, label);
        }
    });
}

/// A parkable window was shown, or is about to be: a park scheduled earlier
/// no longer applies.
pub fn shown<R: tauri::Runtime>(app: &AppHandle<R>, label: &'static str) {
    if let Some(parking) = app.try_state::<Parking>() {
        parking.bump(label);
    }
}

/// Schedules parking `label` after the delay, if it is still hidden then.
pub fn schedule<R: ForkRuntime>(app: &AppHandle<R>, label: &'static str) {
    let Some(parking) = app.try_state::<Parking>() else {
        return;
    };
    let Some(after) = parking.after else {
        return;
    };
    let generation = parking.bump(label);
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("wb-hub-park".into())
        .spawn(move || {
            std::thread::sleep(after);
            let handle = app.clone();
            if let Err(e) = app.run_on_main_thread(move || {
                if let Err(kept) = park(&handle, label, generation) {
                    log::debug!("hub park: {label} kept: {kept:?}");
                }
            }) {
                log::debug!("hub park: {label}: {e}");
            }
        });
    if let Err(e) = spawned {
        log::warn!("hub park: no timer thread: {e}");
    }
}

/// Why a scheduled park did not happen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kept {
    /// Shown or hidden again since it was scheduled.
    Superseded,
    /// No such window (already parked or never built).
    Gone,
    Visible,
    Minimized,
    /// The main page has not reported ready (Handy's start-up work may still
    /// be running); `hub_ready` schedules again.
    NotReady,
}

/// Destroys `label` if it is still hidden in `generation`. Main thread.
pub fn park<R: ForkRuntime>(
    app: &AppHandle<R>,
    label: &'static str,
    generation: u64,
) -> Result<(), Kept> {
    let parking = app.try_state::<Parking>().ok_or(Kept::Gone)?;
    let window = app.get_webview_window(label);
    let shown = window.as_ref().map(|w| WindowShown {
        visible: w.is_visible().unwrap_or(true),
        minimized: w.is_minimized().unwrap_or(true),
    });
    parking.decide(label, generation, shown)?;
    let window = window.ok_or(Kept::Gone)?;
    if label == MAIN {
        let frame = frame_of(&window);
        let mut state = parking.lock();
        state.main_parked = true;
        state.main_ready = false;
        state.main_frame = frame.or(state.main_frame);
        drop(state);
        crate::fork::tray::set_parked_appearance(Some(handy_appearance(app)));
    }
    if let Err(e) = window.destroy() {
        log::warn!("hub park: could not destroy {label}: {e}");
        if label == MAIN {
            parking.lock().main_parked = false;
            crate::fork::tray::set_parked_appearance(None);
        }
        return Err(Kept::Gone);
    }
    log::info!("hub park: {label} hidden, web view released");
    Ok(())
}

/// Handy's appearance setting (System where it cannot be read).
fn handy_appearance<R: ForkRuntime>(app: &AppHandle<R>) -> crate::settings::Theme {
    R::handy_settings(app)
        .map(|s| s.0.theme)
        .unwrap_or(crate::settings::Theme::System)
}

fn frame_of<R: tauri::Runtime>(window: &WebviewWindow<R>) -> Option<Frame> {
    let scale = window.scale_factor().ok()?;
    let position = window.outer_position().ok()?.to_logical::<f64>(scale);
    let size = window.inner_size().ok()?.to_logical::<f64>(scale);
    Some(Frame {
        position: (position.x, position.y),
        size: (size.width, size.height),
    })
}

/// The main window, rebuilt if it is parked. Also cancels a scheduled park.
/// First in Handy's `show_main_window` (`lib.rs` hook) and in every fork
/// path that shows the Hub.
pub fn ensure_main<R: ForkRuntime>(app: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    shown(app, MAIN);
    if let Some(window) = app.get_webview_window(MAIN) {
        return Some(window);
    }
    let parking = app.try_state::<Parking>()?;
    if !parking.main_parked() {
        // Not ours to rebuild (headless CLI modes have no main window).
        return None;
    }
    let frame = parking.lock().main_frame;
    match build_main(app, frame) {
        Ok(window) => {
            {
                let mut state = parking.lock();
                state.main_parked = false;
                state.main_ready = false;
            }
            crate::fork::tray::set_parked_appearance(None);
            watch(app, &window);
            log::info!("hub park: main window rebuilt");
            Some(window)
        }
        Err(e) => {
            log::error!("hub park: could not rebuild the main window: {e}");
            None
        }
    }
}

/// Handy's main-window builder (`lib.rs` `.setup()`), then the fork's chrome
/// and the parked frame. Hidden: the caller shows it.
fn build_main<R: ForkRuntime>(
    app: &AppHandle<R>,
    frame: Option<Frame>,
) -> tauri::Result<WebviewWindow<R>> {
    let locale = crate::fork::i18n::current_locale(R::handy_app_language(app).as_deref());
    let title = crate::fork::i18n::t(locale, "rust.window.mainTitle", &[]);
    let (width, height) = crate::utils::MAIN_WINDOW_SIZE;
    let mut builder = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("/".into()))
        .title(title)
        .inner_size(width, height)
        .min_inner_size(width, height)
        .resizable(true)
        .maximizable(true)
        .visible(false);
    if let Some(data_dir) = crate::portable::data_dir() {
        builder = builder.data_directory(data_dir.join("webview"));
    }
    let window = builder.build()?;
    R::hub_apply_theme(app);
    super::style_main(app, &window);
    if let Some(frame) = frame {
        let _ = window.set_size(LogicalSize::new(frame.size.0, frame.size.1));
        let _ = window.set_position(LogicalPosition::new(frame.position.0, frame.position.1));
    }
    Ok(window)
}

/// Shows the Hub (rebuilt if parked), then runs `deliver` once its page can
/// receive events: now if it is ready, else when it reports ready.
pub fn show_main_then<R: ForkRuntime>(app: &AppHandle<R>, deliver: impl FnOnce() + Send + 'static) {
    let Some(window) = ensure_main(app) else {
        log::warn!("hub park: no main window to show");
        return;
    };
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
    #[cfg(target_os = "macos")]
    if let Err(e) = app.set_activation_policy(tauri::ActivationPolicy::Regular) {
        log::debug!("hub park: activation policy: {e}");
    }
    let Some(parking) = app.try_state::<Parking>() else {
        deliver();
        return;
    };
    let mut state = parking.lock();
    if state.main_ready {
        drop(state);
        deliver();
    } else {
        state.pending.push(Box::new(deliver));
    }
}

/// The main page's listeners are up (`fork_hub_ready`): delivers what
/// waited for it and, if the window is hidden (a login-item start), lets it
/// be parked.
pub fn hub_ready<R: ForkRuntime>(app: &AppHandle<R>) {
    let Some(parking) = app.try_state::<Parking>() else {
        return;
    };
    let pending = {
        let mut state = parking.lock();
        state.main_ready = true;
        std::mem::take(&mut state.pending)
    };
    for deliver in pending {
        deliver();
    }
    let hidden = app
        .get_webview_window(MAIN)
        .is_some_and(|w| !w.is_visible().unwrap_or(true) && !w.is_minimized().unwrap_or(true));
    if hidden {
        schedule(app, MAIN);
    }
}

/// `ExitRequested` without a code while the Hub is parked: every Tauri
/// window is gone because we destroyed them, not because the user quit.
pub fn keeps_running<R: tauri::Runtime>(app: &AppHandle<R>, code: Option<i32>) -> bool {
    code.is_none()
        && app
            .try_state::<Parking>()
            .is_some_and(|parking| parking.main_parked())
}

#[cfg(test)]
#[path = "park_tests.rs"]
mod tests;
