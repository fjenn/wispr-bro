// SPDX-License-Identifier: GPL-3.0-or-later
//! The health actor (S-02): one thread that owns the [`Machine`], runs
//! probes when a trigger arrives, rebuilds taps and tells the user.
//!
//! Where the user hears about a lost shortcut (R5 S-02, within 2 s):
//! - the Flow Bar: "Your shortcut stopped working" with Fix (opens the Fix
//!   sheet in the Hub), and "Shortcut works again" after recovery;
//! - the tray: the warning badge and a "Shortcut stopped working" item
//!   (the existing Secure Input path, `tray.rs` hook);
//! - the Hub: a banner with Fix; the Fix sheet itself;
//! - the notification inbox (and macOS while the app is not in front).
//!
//! Before onboarding is done nothing is shown: a missing permission is
//! onboarding's job then.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use super::{probe_tap, trusted, Effect, HealthStatus, Machine, Ms, Plan, TapId, Trigger};
use crate::fork::ForkRuntime;

/// Emitted when the overall status changes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkShortcutHealthChanged {
    pub status: HealthStatus,
}

/// Asks the Hub to open the Fix sheet (the bar's Fix, the tray item).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkShortcutFixRequested {
    /// What asked: `pill`, `tray`, `notification`.
    pub source: String,
}

enum Msg {
    Check(Trigger, Instant),
    Stopped(TapId),
    Rebuilt(TapId, bool),
}

static TX: OnceLock<Mutex<Sender<Msg>>> = OnceLock::new();
static STATUS: Mutex<Option<HealthStatus>> = Mutex::new(None);
/// The tray shows the warning (lost, after onboarding).
static TRAY_WARNING: AtomicBool = AtomicBool::new(false);

fn send(msg: Msg) {
    if let Some(tx) = TX.get() {
        let tx = tx.lock().unwrap_or_else(|e| e.into_inner());
        let _ = tx.send(msg);
    }
}

/// Runs a check soon (any thread, cheap). Before [`start`] it does nothing.
pub fn request_check(trigger: Trigger) {
    send(Msg::Check(trigger, Instant::now()));
}

/// A tap's event stream ended.
pub fn tap_stopped(tap: TapId) {
    send(Msg::Stopped(tap));
}

/// The result of a rebuild the actor asked for.
pub fn tap_rebuilt(tap: TapId, ok: bool) {
    send(Msg::Rebuilt(tap, ok));
}

/// The last status (healthy before the actor ran).
pub fn status() -> HealthStatus {
    STATUS
        .lock()
        .ok()
        .and_then(|s| s.clone())
        .unwrap_or_else(HealthStatus::healthy)
}

/// For the `tray.rs` hook: the shortcut is lost and onboarding is done.
pub fn tray_warning() -> bool {
    TRAY_WARNING.load(Ordering::SeqCst)
}

/// Hotkey effects that are triggers: a take the bar shows, a missed
/// release.
pub fn on_engine_effect(event: &crate::fork::shortcut::engine::Observed) {
    use crate::fork::shortcut::engine::Observed;
    use crate::fork::shortcut::processor::Effect as E;
    match event {
        Observed::Effect(E::TakeStarted) => request_check(Trigger::TakeShown),
        Observed::StuckReleased => request_check(Trigger::MissedRelease),
        _ => {}
    }
}

fn epoch() -> Instant {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

fn ms(t: Instant) -> Ms {
    t.saturating_duration_since(epoch()).as_millis() as Ms
}

/// Starts the actor (product builds, from `shortcut::start`). `fork_tap_ok`:
/// whether the fork's tap started.
pub fn start<R: ForkRuntime>(app: &AppHandle<R>, fork_tap_ok: bool) {
    let (tx, rx) = mpsc::channel::<Msg>();
    if TX.set(Mutex::new(tx)).is_err() {
        return;
    }
    let mut machine = Machine::new();
    let first = machine.started(TapId::Fork, fork_tap_ok);
    *STATUS.lock().unwrap_or_else(|e| e.into_inner()) = Some(machine.status());
    install_triggers(app);
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("wb-hotkey-health".into())
        .spawn(move || {
            let mut actor = Actor {
                app,
                machine,
                announced: false,
            };
            actor.run_effects(first, None);
            actor.check(Trigger::Startup, Instant::now());
            actor.run(rx);
        });
    if let Err(e) = spawned {
        log::error!("fork hotkeys health: cannot spawn the actor: {e}");
    }
}

struct Actor<R: ForkRuntime> {
    app: AppHandle<R>,
    machine: Machine,
    /// The user was told about the current loss (pill/notification).
    announced: bool,
}

impl<R: ForkRuntime> Actor<R> {
    fn run(&mut self, rx: mpsc::Receiver<Msg>) {
        loop {
            let now = ms(Instant::now());
            let msg = match self.machine.next_recheck() {
                Some(due) if due <= now => {
                    self.machine.take_due(now);
                    self.check(Trigger::Recheck, Instant::now());
                    continue;
                }
                Some(due) => match rx.recv_timeout(Duration::from_millis(due - now)) {
                    Ok(m) => m,
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => break,
                },
                None => match rx.recv() {
                    Ok(m) => m,
                    Err(_) => break,
                },
            };
            match msg {
                Msg::Check(trigger, at) => {
                    // Notifications come in pairs (one edit posts both):
                    // one probe answers everything already queued.
                    let mut trigger = trigger;
                    let mut at = at;
                    let mut later = Vec::new();
                    while let Ok(m) = rx.try_recv() {
                        match m {
                            Msg::Check(t, a) => {
                                if t.refreshes_taps() || !trigger.refreshes_taps() {
                                    trigger = t;
                                }
                                at = at.min(a);
                            }
                            other => later.push(other),
                        }
                    }
                    self.check(trigger, at);
                    for m in later {
                        self.handle(m);
                    }
                }
                other => self.handle(other),
            }
        }
        log::info!("fork hotkeys health: actor stopped");
    }

    fn handle(&mut self, msg: Msg) {
        let now = ms(Instant::now());
        match msg {
            Msg::Check(trigger, at) => self.check(trigger, at),
            Msg::Stopped(tap) => {
                log::warn!("fork hotkeys health: the {tap:?} tap stopped");
                let effects = self.machine.stopped(tap);
                self.run_effects(effects, Some(Trigger::TapStopped));
                self.check(Trigger::TapStopped, Instant::now());
            }
            Msg::Rebuilt(tap, ok) => {
                if ok {
                    log::info!("fork hotkeys health: the {tap:?} tap is running again");
                } else {
                    log::warn!("fork hotkeys health: rebuilding the {tap:?} tap failed");
                }
                let effects = self.machine.rebuilt(now, tap, ok);
                self.run_effects(effects, None);
            }
        }
    }

    fn check(&mut self, trigger: Trigger, at: Instant) {
        let started = Instant::now();
        let ok = match self.machine.plan(trigger) {
            Plan::TrustedOnly => trusted() || probe_tap().unwrap_or(true),
            Plan::Probe => probe_tap().unwrap_or(true),
        };
        log::debug!(
            "fork hotkeys health: {trigger:?} → probe {} ({} µs, {} ms after the trigger)",
            if ok { "ok" } else { "failed" },
            started.elapsed().as_micros(),
            at.elapsed().as_millis()
        );
        let effects = self.machine.checked(ms(Instant::now()), trigger, ok);
        self.run_effects(effects, Some(trigger));
    }

    fn run_effects(&mut self, effects: Vec<Effect>, trigger: Option<Trigger>) {
        for e in effects {
            match e {
                Effect::Rebuild { tap, refresh } => self.rebuild(tap, refresh),
                Effect::Changed(status) => self.changed(status, trigger),
            }
        }
    }

    fn rebuild(&self, tap: TapId, refresh: bool) {
        log::info!(
            "fork hotkeys health: rebuilding the {tap:?} tap{}",
            if refresh { " (refresh after wake)" } else { "" }
        );
        match tap {
            TapId::Fork => {
                if !crate::fork::shortcut::rebuild_tap(refresh) {
                    tap_rebuilt(TapId::Fork, false);
                }
            }
            TapId::Handy => {
                let app = self.app.clone();
                std::thread::spawn(move || {
                    let result = R::handy_keys_rebuild(&app);
                    if let Err(e) = &result {
                        log::warn!("fork hotkeys health: Handy's tap: {e}");
                    }
                    tap_rebuilt(TapId::Handy, result.is_ok());
                });
            }
        }
    }

    fn onboarded(&self) -> bool {
        R::handy_settings(&self.app).is_some_and(|s| s.0.onboarding_completed)
    }

    fn changed(&mut self, status: HealthStatus, trigger: Option<Trigger>) {
        log::info!(
            "fork hotkeys health: {:?} (reason {:?}, fork {:?}, handy {:?}, relaunch {}){}",
            status.state,
            status.reason,
            status.fork_tap,
            status.handy_tap,
            status.needs_relaunch,
            trigger.map(|t| format!(" after {t:?}")).unwrap_or_default()
        );
        *STATUS.lock().unwrap_or_else(|e| e.into_inner()) = Some(status.clone());
        crate::fork::shortcut::health_changed(&status);
        let onboarded = self.onboarded();
        let warn = status.lost() && onboarded;
        if TRAY_WARNING.swap(warn, Ordering::SeqCst) != warn {
            R::tray_refresh(&self.app);
        }
        if let Err(e) = (ForkShortcutHealthChanged {
            status: status.clone(),
        })
        .emit(&self.app)
        {
            log::warn!("fork hotkeys health: cannot emit the status: {e}");
        }
        if status.lost() && onboarded && !self.announced {
            self.announced = true;
            crate::fork::pill::shortcut_lost(status.reason);
            crate::fork::notify::shortcut_problem(
                &self.app,
                status.reason == Some(super::LostReason::Permission),
            );
        } else if status.state == super::TapHealth::Healthy && self.announced {
            self.announced = false;
            crate::fork::pill::shortcut_back();
        }
    }
}

/// Opens the Hub and asks it for the Fix sheet.
pub fn open_fix_sheet<R: tauri::Runtime>(app: &AppHandle<R>, source: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    #[cfg(target_os = "macos")]
    if let Err(e) = app.set_activation_policy(tauri::ActivationPolicy::Regular) {
        log::debug!("fork hotkeys health: activation policy: {e}");
    }
    let _ = ForkShortcutFixRequested {
        source: source.into(),
    }
    .emit(app);
}

/// The triggers: axwatch, workspace wake notifications, the main window.
fn install_triggers<R: ForkRuntime>(app: &AppHandle<R>) {
    static WATCH: OnceLock<crate::fork::vendor::axwatch::Watch> = OnceLock::new();
    use crate::fork::vendor::axwatch::{Wake, Watch};
    match Watch::start(Some(Duration::from_millis(super::HEARTBEAT_MS)), |wake| {
        request_check(match wake {
            Wake::AccessibilityChanged => Trigger::AccessibilityChanged,
            Wake::PrivacyChanged => Trigger::PrivacyChanged,
            Wake::Heartbeat => Trigger::Heartbeat,
        })
    }) {
        Ok(w) => {
            let _ = WATCH.set(w);
        }
        Err(e) => log::warn!("fork hotkeys health: axwatch unavailable: {e}"),
    }
    #[cfg(target_os = "macos")]
    {
        let result = app.run_on_main_thread(|| {
            workspace::observe(request_check);
        });
        if let Err(e) = result {
            log::warn!("fork hotkeys health: cannot observe wake notifications: {e}");
        }
    }
    if let Some(window) = app.get_webview_window("main") {
        window.on_window_event(|event| {
            if let tauri::WindowEvent::Focused(true) = event {
                request_check(Trigger::AppActivated);
            }
        });
    }
}

#[cfg(target_os = "macos")]
mod workspace {
    //! System wake, display wake and session-active notifications
    //! (VoiceInk's recovery triggers).
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::{define_class, msg_send, sel, AnyThread, DefinedClass};
    use objc2_app_kit::{
        NSWorkspace, NSWorkspaceDidWakeNotification, NSWorkspaceScreensDidWakeNotification,
        NSWorkspaceSessionDidBecomeActiveNotification,
    };
    use objc2_foundation::{NSNotification, NSObject};

    use super::Trigger;

    pub struct Ivars {
        on_trigger: fn(Trigger),
    }

    define_class!(
        // SAFETY: NSObject has no subclassing requirements; the ivar is a
        // plain function pointer.
        #[unsafe(super(NSObject))]
        #[name = "WisprBroHotkeyHealthObserver"]
        #[ivars = Ivars]
        pub struct Observer;

        impl Observer {
            #[unsafe(method(didWake:))]
            fn did_wake(&self, _n: &NSNotification) {
                (self.ivars().on_trigger)(Trigger::Wake);
            }
            #[unsafe(method(screensDidWake:))]
            fn screens_did_wake(&self, _n: &NSNotification) {
                (self.ivars().on_trigger)(Trigger::ScreensWake);
            }
            #[unsafe(method(sessionBecameActive:))]
            fn session_active(&self, _n: &NSNotification) {
                (self.ivars().on_trigger)(Trigger::SessionActive);
            }
        }
    );

    /// Subscribes once (main thread); the observer lives for the process.
    pub fn observe(on_trigger: fn(Trigger)) {
        let this = Observer::alloc().set_ivars(Ivars { on_trigger });
        // SAFETY: NSObject's init.
        let observer: Retained<Observer> = unsafe { msg_send![super(this), init] };
        let obj: &AnyObject = &observer;
        let center = NSWorkspace::sharedWorkspace().notificationCenter();
        // SAFETY: the selectors exist with the `(NSNotification*)`
        // signature; the observer is never deallocated (forgotten below).
        unsafe {
            for (sel, name) in [
                (sel!(didWake:), NSWorkspaceDidWakeNotification),
                (sel!(screensDidWake:), NSWorkspaceScreensDidWakeNotification),
                (
                    sel!(sessionBecameActive:),
                    NSWorkspaceSessionDidBecomeActiveNotification,
                ),
            ] {
                center.addObserver_selector_name_object(obj, sel, Some(name), None);
            }
        }
        std::mem::forget(observer);
    }
}
