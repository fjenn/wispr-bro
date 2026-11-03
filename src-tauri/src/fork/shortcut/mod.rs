// SPDX-License-Identifier: GPL-3.0-or-later
//! Fork hotkeys (S-01, R5 §4.8): the fn grammar on the fork's own tap.
//!
//! - `listener.rs`: handy-keys `KeyboardListener::new_with_blocking` on its
//!   own thread.
//! - `decision.rs`: raw events → grammar inputs for the active trigger;
//!   stuck-trigger guard (rule 9).
//! - `processor.rs`: the grammar (rules 1-8), pure.
//! - `blocking.rs`: the dynamic blocking set; the Globe setting.
//! - `globe.rs`: DEC-44, the consented Globe → Do Nothing change and its
//!   crash-safe restore (OpenWhispr port).
//! - `bindings.rs`: fork settings → profile; conflicts; the Ctrl+Opt
//!   fallback.
//! - `actions.rs`: action registry and Esc registry.
//! - `labels.rs`: layout-aware chord labels.
//! - `engine.rs`: the loop that ties them together, behind a `Host`.
//! - `health.rs` (S-02): watches both taps, rebuilds them, tells the user.
//! - `secure.rs` (S-02): the fork's bindings under Secure Input (Carbon
//!   shadows through Handy's `secure_input.rs`).
//!
//! Handy's own hotkey manager keeps running for everything else: the
//! `owns()` hook in `shortcut/mod.rs` stops it registering `transcribe`,
//! `transcribe_with_post_process` and `cancel`, which the fork drives
//! through `TranscriptionCoordinator::external_command`. The CLI flags
//! (`--toggle-transcription`, `--cancel`) still go straight to Handy's
//! coordinator, unchanged.

pub mod actions;
pub mod bindings;
pub mod blocking;
pub mod decision;
pub mod engine;
pub mod globe;
pub mod health;
pub mod labels;
pub mod listener;
pub mod processor;
pub mod secure;

/// The `secure_input.rs` hook (S-02): see [`secure::secure_input_bindings`].
pub use secure::secure_input_bindings;

#[cfg(test)]
mod tests;

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use handy_keys::{Hotkey, KeyEvent};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};
use tauri_specta::Event;

use self::actions::Registry;
use self::bindings::{Issue, Profile};
use self::blocking::GlobeUsage;
use self::engine::{Engine, Host, Observed};
use self::listener::{Tap, TapMsg};
use self::processor::{Effect, Ms, Timings, WarnKind};
use crate::fork::settings::{ForkSettingsHandle, HotkeyActivation};
use crate::transcription_coordinator::{ExternalCommand, TranscriptionCoordinator};

/// Handy binding ids the fork owns (never registered by Handy's manager).
pub const OWNED_HANDY_BINDINGS: [&str; 3] =
    ["transcribe", "transcribe_with_post_process", "cancel"];

/// Whether the fork owns this Handy binding. True in product builds (the
/// identity overlay), where the fork's engine runs; plain Handy builds keep
/// Handy's hotkeys. Called first in Handy's `register_shortcut` and
/// `unregister_shortcut`.
pub fn owns(binding_id: &str) -> bool {
    owns_in(crate::fork::is_product_build(), binding_id)
}

pub fn owns_in(product_build: bool, binding_id: &str) -> bool {
    product_build && OWNED_HANDY_BINDINGS.contains(&binding_id)
}

/// A hotkey decision, for listeners (the pill from S-03, the debug panel).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkHotkeyEvent {
    /// `take_started`, `locked`, `stopped`, `cancelled`, `chord`, `warning`,
    /// `action`, `stuck_released`; `key_test` (detail `down`/`up`) while
    /// onboarding's shortcut test runs ([`set_key_test`]).
    pub kind: String,
    /// The detail: how it locked/stopped, why it cancelled, the warning
    /// kind, the action id.
    pub detail: Option<String>,
    /// For `chord`: the take ran ≥ `T` and its transcript should be kept
    /// for History (DEC-54).
    pub keep: bool,
}

impl ForkHotkeyEvent {
    fn from_observed(o: &Observed) -> Option<ForkHotkeyEvent> {
        let ev = |kind: &str, detail: Option<String>| ForkHotkeyEvent {
            kind: kind.into(),
            detail,
            keep: false,
        };
        let snake = |v: serde_json::Value| -> Option<String> {
            v.as_object()?
                .iter()
                .find(|(k, _)| k.as_str() != "effect")
                .and_then(|(_, v)| v.as_str().map(str::to_string))
        };
        Some(match o {
            Observed::Effect(e) => {
                let detail = serde_json::to_value(e).ok().and_then(snake);
                match e {
                    Effect::TakeStarted => ev("take_started", None),
                    Effect::Locked { .. } => ev("locked", detail),
                    Effect::Stopped { .. } => ev("stopped", detail),
                    Effect::Cancelled { .. } => ev("cancelled", detail),
                    Effect::Warning { .. } => ev("warning", detail),
                    Effect::ChordIgnored {
                        mic_was_open: true,
                        keep,
                        ..
                    } => ForkHotkeyEvent {
                        kind: "chord".into(),
                        detail: None,
                        keep: *keep,
                    },
                    _ => return None,
                }
            }
            Observed::Action(id) => ev("action", Some(id.clone())),
            Observed::EscCancelledBusy => ev("cancelled", Some("escape".into())),
            Observed::StuckReleased => ev("stuck_released", None),
        })
    }
}

/// One binding with its label, for the hotkeys UI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub struct BindingLabel {
    pub action: String,
    pub chord: String,
    pub activation: HotkeyActivation,
    pub label: labels::Label,
}

/// What `fork_hotkey_status` returns.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub struct HotkeyStatus {
    /// The fork tap is running.
    pub running: bool,
    /// Why it is not (no Accessibility permission, …).
    pub error: Option<String>,
    /// `fn`, `ctrl+option`, or `None` when no trigger is bound.
    pub trigger: Option<String>,
    /// The Globe setting as last read: `do_nothing`, `input_source`,
    /// `emoji`, `dictation`, `default`.
    pub globe: String,
    /// `{FN}` is in the blocking set.
    pub fn_blocked: bool,
    pub issues: Vec<Issue>,
    pub bindings: Vec<BindingLabel>,
}

enum Msg {
    Tap(TapMsg),
    Voice(Instant),
    Profile(Box<Profile>),
    Pill(engine::PillAction),
    Globe,
    /// S-02: build a new tap (`refresh`: the old one is healthy; skipped
    /// during a take).
    Rebuild {
        refresh: bool,
    },
    /// S-02 / S-29a: a tap built outside the engine thread (`retry_tap`).
    InstallTap(Tap),
    /// S-02: a Secure Input shadow fired for this fork action.
    Shadow(String),
}

struct Live {
    tx: Sender<Msg>,
    status: Mutex<HotkeyStatus>,
}

static RUNTIME: OnceLock<Live> = OnceLock::new();
static REGISTRY: OnceLock<Registry> = OnceLock::new();

/// The app's action registry (later slices register handlers here).
pub fn registry() -> Registry {
    REGISTRY.get_or_init(Registry::default).clone()
}

fn globe_name(g: GlobeUsage) -> &'static str {
    match g {
        GlobeUsage::DoNothing => "do_nothing",
        GlobeUsage::InputSource => "input_source",
        GlobeUsage::Emoji => "emoji",
        GlobeUsage::Dictation => "dictation",
        GlobeUsage::Default => "default",
    }
}

fn trigger_name(p: &Profile) -> Option<String> {
    p.trigger.map(|t| match t {
        decision::Trigger::Fn => "fn".to_string(),
        decision::Trigger::Mods(m) => bindings::normalize(&m.to_string())
            .map(|(c, _)| c)
            .unwrap_or_else(|| m.to_string()),
    })
}

/// The current profile from fork settings.
fn load_profile<R: Runtime>(
    app: &AppHandle<R>,
) -> Option<(
    Profile,
    Vec<crate::fork::settings::HotkeyBinding>,
    Vec<String>,
)> {
    let handle = app.try_state::<ForkSettingsHandle>()?;
    let settings = match handle.get(app) {
        Ok(s) => s,
        Err(e) => {
            log::error!("fork hotkeys: cannot read fork settings: {e}");
            return None;
        }
    };
    let reg = registry();
    let command_mode = settings.command_mode_enabled;
    let profile = bindings::compile(&settings.hotkeys, |id| {
        reg.handles(id)
            || (command_mode && id == crate::fork::settings::hotkey_actions::COMMAND_MODE)
    });
    let mut flat = Vec::new();
    let mut actions = Vec::new();
    for (action, list) in &settings.hotkeys {
        for b in list {
            flat.push(b.clone());
            actions.push(action.clone());
        }
    }
    Some((profile, flat, actions))
}

fn status_for(
    profile: &Profile,
    globe: GlobeUsage,
    flat: &[crate::fork::settings::HotkeyBinding],
    actions: &[String],
    locale: &str,
    layout: &dyn labels::Layout,
) -> HotkeyStatus {
    HotkeyStatus {
        running: false,
        error: None,
        trigger: trigger_name(profile),
        globe: globe_name(globe).into(),
        fn_blocked: profile.trigger == Some(decision::Trigger::Fn) && globe.blocks_fn(),
        issues: profile.issues.clone(),
        bindings: flat
            .iter()
            .zip(actions)
            .filter_map(|(b, action)| {
                Some(BindingLabel {
                    action: action.clone(),
                    chord: b.chord.clone(),
                    activation: b.activation,
                    label: labels::label(&b.chord, layout, locale)?,
                })
            })
            .collect(),
    }
}

/// Starts the engine (from `fork::on_ready`, product builds only).
pub fn start<R: crate::fork::ForkRuntime>(app: &AppHandle<R>) {
    if RUNTIME.get().is_some() {
        return;
    }
    let Some((profile, _, _)) = load_profile(app) else {
        log::warn!("fork hotkeys: fork settings unavailable; hotkeys not started");
        return;
    };
    let globe = blocking::read_globe_usage();
    let (tx, rx) = mpsc::channel::<Msg>();
    let rt = Live {
        tx: tx.clone(),
        status: Mutex::new(HotkeyStatus {
            running: false,
            error: None,
            trigger: trigger_name(&profile),
            globe: globe_name(globe).into(),
            fn_blocked: false,
            issues: profile.issues.clone(),
            bindings: Vec::new(),
        }),
    };
    if RUNTIME.set(rt).is_err() {
        return;
    }
    for issue in &profile.issues {
        log::warn!("fork hotkeys: binding skipped: {issue:?}");
    }
    {
        // S-02: Handy's Carbon fallback re-reads the fork's bindings
        // (only while Secure Input is sustained; otherwise nothing is
        // shadowed and there is nothing to redo).
        let app = app.clone();
        secure::set_reconciler(move || {
            if R::secure_input_info(&app).is_some_and(|i| i.sustained) {
                R::secure_input_reconcile(&app);
            }
        });
    }
    let engine = Engine::new(profile, registry(), Timings::DEFAULT, globe.blocks_fn());
    secure::set_spec(secure::ShadowSpec::from_profile(engine.profile()));
    let initial = blocking::hotkeys(
        engine.profile(),
        engine.grammar().blocking(),
        Default::default(),
    );
    remember_blocking(&initial);
    // S-02: without the tap the engine still runs (the bar's clicks work);
    // the health check builds the tap once the permission is there (S-29a's
    // onboarding grant included) and tells the user meanwhile (it also
    // posts the S-18 notification).
    let tap = match listener::start(initial.clone(), tap_forwarder(tx.clone())) {
        Ok(t) => {
            set_status(|s| {
                s.running = true;
                s.error = None;
                s.fn_blocked = engine.grammar().blocks_fn() && s.trigger.as_deref() == Some("fn");
            });
            // S-38: launch → hotkey ready (§5.3).
            crate::fork::startup::mark(crate::fork::startup::Milestone::Hotkeys);
            log::info!(
                "fork hotkeys: tap running (trigger {:?}, Globe {}, fn blocked {})",
                trigger_name(engine.profile()),
                globe_name(globe),
                engine.grammar().blocks_fn()
            );
            Some(t)
        }
        Err(e) => {
            log::error!("fork hotkeys: {e}");
            set_status(|s| s.error = Some(e.clone()));
            None
        }
    };
    let tap_ok = tap.is_some();
    let engine_app = app.clone();
    match std::thread::Builder::new()
        .name("wb-hotkey-engine".into())
        .spawn(move || run(engine_app, engine, tap, initial, tx, rx))
    {
        Ok(_) => health::start(app, tap_ok),
        Err(e) => log::error!("fork hotkeys: cannot spawn the engine thread: {e}"),
    }
}

/// The blocking set the engine last applied (a tap built outside the
/// engine thread starts with it).
static LAST_BLOCKING: Mutex<Option<HashSet<Hotkey>>> = Mutex::new(None);

fn remember_blocking(set: &HashSet<Hotkey>) {
    *LAST_BLOCKING.lock().unwrap_or_else(|e| e.into_inner()) = Some(set.clone());
}

fn tap_forwarder(tx: Sender<Msg>) -> impl Fn(TapMsg) + Send + 'static {
    move |m| {
        let _ = tx.send(Msg::Tap(m));
    }
}

/// S-02: asks the engine for a new tap. `false` when the engine is not
/// running.
pub fn rebuild_tap(refresh: bool) -> bool {
    let Some(rt) = RUNTIME.get() else {
        return false;
    };
    rt.tx.send(Msg::Rebuild { refresh }).is_ok()
}

/// S-02: a Secure Input shadow fired (`secure::on_shadow_event`).
pub fn shadow_fired(action: &str) {
    if let Some(rt) = RUNTIME.get() {
        let _ = rt.tx.send(Msg::Shadow(action.to_string()));
    }
}

/// S-02: the health check's verdict, for `fork_hotkey_status`.
pub fn health_changed(status: &health::HealthStatus) {
    set_status(|s| match status.fork_tap {
        health::TapHealth::Healthy => {
            s.running = true;
            s.error = None;
        }
        health::TapHealth::Lost | health::TapHealth::Recovering => {
            s.running = false;
            s.error = Some(match status.reason {
                Some(health::LostReason::Permission) => "no Accessibility permission".into(),
                _ => "the keyboard tap stopped".into(),
            });
        }
    });
}

/// Starts the tap again if it could not start at launch (S-29a: after the
/// user granted Accessibility in onboarding, no relaunch needed). Returns
/// whether the tap is running now.
pub fn retry_tap<R: Runtime>(_app: &AppHandle<R>) -> bool {
    let Some(rt) = RUNTIME.get() else {
        return false;
    };
    if rt.status.lock().unwrap_or_else(|e| e.into_inner()).running {
        return true;
    }
    let set = LAST_BLOCKING
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_default();
    match listener::start(set, tap_forwarder(rt.tx.clone())) {
        Ok(tap) => {
            log::info!("fork hotkeys: tap started on retry");
            let _ = rt.tx.send(Msg::InstallTap(tap));
            set_status(|s| {
                s.running = true;
                s.error = None;
            });
            // S-02: the health check confirms the grant and Handy's tap.
            health::request_check(health::Trigger::User);
            true
        }
        Err(e) => {
            log::warn!("fork hotkeys: tap retry failed: {e}");
            set_status(|s| s.error = Some(e));
            false
        }
    }
}

/// Onboarding's shortcut test (S-29a): while on, trigger presses light the
/// on-screen key (`key_test` events) and start no take. Expires on its own
/// so a closed window never leaves dictation off; the test renews it.
static KEY_TEST_UNTIL_MS: AtomicU64 = AtomicU64::new(0);
pub const KEY_TEST_TTL_MS: u64 = 120_000;

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn set_key_test(on: bool) {
    let until = if on {
        now_unix_ms() + KEY_TEST_TTL_MS
    } else {
        0
    };
    KEY_TEST_UNTIL_MS.store(until, Ordering::Relaxed);
}

fn key_test_on() -> bool {
    key_test_active(KEY_TEST_UNTIL_MS.load(Ordering::Relaxed), now_unix_ms())
}

fn key_test_active(until_ms: u64, now_ms: u64) -> bool {
    now_ms < until_ms
}

/// Every modifier group of the trigger is held (either side counts).
pub(crate) fn trigger_held(trigger: decision::Trigger, held: handy_keys::Modifiers) -> bool {
    use handy_keys::Modifiers as M;
    let want = trigger.modifiers();
    [M::SHIFT, M::CTRL, M::OPT, M::CMD, M::FN]
        .into_iter()
        .filter(|group| want.intersects(*group))
        .all(|group| held.intersects(group))
}

static KEY_TEST_DOWN: AtomicBool = AtomicBool::new(false);

fn emit_key_test<R: Runtime>(app: &AppHandle<R>, down: bool) {
    if KEY_TEST_DOWN.swap(down, Ordering::Relaxed) == down {
        return;
    }
    log::info!(
        "fork hotkeys: key test {}",
        if down { "down" } else { "up" }
    );
    let ev = ForkHotkeyEvent {
        kind: "key_test".into(),
        detail: Some(if down { "down" } else { "up" }.into()),
        keep: false,
    };
    if let Err(e) = ev.emit(app) {
        log::warn!("fork hotkeys: cannot emit the key test: {e}");
    }
}

fn set_status(f: impl FnOnce(&mut HotkeyStatus)) {
    if let Some(rt) = RUNTIME.get() {
        let mut s = rt.status.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut s);
    }
}

/// Re-reads the bindings (a hotkey setting changed).
pub fn reload<R: Runtime>(app: &AppHandle<R>) {
    let Some(rt) = RUNTIME.get() else { return };
    if let Some((profile, _, _)) = load_profile(app) {
        set_status(|s| {
            s.trigger = trigger_name(&profile);
            s.issues = profile.issues.clone();
        });
        // DEC-44: fn stopped being the trigger → the Globe setting we
        // changed goes back to the user's.
        globe::reconcile(app, fn_is_trigger(&profile));
        secure::set_spec(secure::ShadowSpec::from_profile(&profile));
        let _ = rt.tx.send(Msg::Profile(Box::new(profile)));
    }
}

fn fn_is_trigger(profile: &Profile) -> bool {
    profile.trigger == Some(decision::Trigger::Fn)
}

/// DEC-44 at launch and quit: restore the Globe setting if fn is no longer
/// the trigger (a crash, or bindings edited while the app was not running).
pub fn reconcile_globe<R: Runtime>(app: &AppHandle<R>) {
    if let Some((profile, _, _)) = load_profile(app) {
        globe::reconcile(app, fn_is_trigger(&profile));
    }
}

/// Re-reads the Globe preference (after the DEC-44 change or its restore).
pub fn refresh_globe() {
    if let Some(rt) = RUNTIME.get() {
        let _ = rt.tx.send(Msg::Globe);
    }
}

static LAST_VOICE_MS: AtomicU64 = AtomicU64::new(0);

/// Speech-level sound during a take (`fork::audio::on_levels`). Throttled
/// to one message per 250 ms.
pub fn on_voice() {
    let Some(rt) = RUNTIME.get() else { return };
    let now = epoch_ms(Instant::now());
    let last = LAST_VOICE_MS.load(Ordering::Relaxed);
    if now.saturating_sub(last) < 250 {
        return;
    }
    LAST_VOICE_MS.store(now, Ordering::Relaxed);
    let _ = rt.tx.send(Msg::Voice(Instant::now()));
}

/// A Flow Bar click (S-03). Returns `false` when the engine is not
/// running (plain Handy builds, or the tap failed to start).
pub fn pill_action(action: engine::PillAction) -> bool {
    let Some(rt) = RUNTIME.get() else {
        return false;
    };
    rt.tx.send(Msg::Pill(action)).is_ok()
}

fn epoch() -> Instant {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

fn epoch_ms(t: Instant) -> Ms {
    t.saturating_duration_since(epoch()).as_millis() as Ms
}

/// Status for the hotkeys UI, with labels for the active layout. Call on
/// the main thread for layout-aware letters (TIS); elsewhere the ANSI
/// names are used.
pub fn status<R: Runtime>(app: &AppHandle<R>, locale: &str) -> HotkeyStatus {
    let globe = blocking::read_globe_usage();
    let (running, error) = RUNTIME
        .get()
        .map(|rt| {
            let s = rt.status.lock().unwrap_or_else(|e| e.into_inner());
            (s.running, s.error.clone())
        })
        .unwrap_or((false, None));
    let Some((profile, flat, actions)) = load_profile(app) else {
        return HotkeyStatus {
            running,
            error,
            trigger: None,
            globe: globe_name(globe).into(),
            fn_blocked: false,
            issues: Vec::new(),
            bindings: Vec::new(),
        };
    };
    #[cfg(target_os = "macos")]
    let current = labels::CurrentLayout::load();
    #[cfg(target_os = "macos")]
    let layout: &dyn labels::Layout = match &current {
        Some(l) => l,
        None => &labels::Ansi,
    };
    #[cfg(not(target_os = "macos"))]
    let layout: &dyn labels::Layout = &labels::Ansi;
    let mut s = status_for(&profile, globe, &flat, &actions, locale, layout);
    s.running = running;
    s.error = error;
    s
}

/// The Host the live engine runs on.
struct AppHost<R: crate::fork::ForkRuntime> {
    app: AppHandle<R>,
    /// `None` while the health check has no working tap to give us.
    tap: Option<Tap>,
    /// The blocking set last applied (a rebuilt tap starts with it).
    blocking: HashSet<Hotkey>,
    trigger_mask: u64,
}

impl<R: crate::fork::ForkRuntime> Host for AppHost<R> {
    fn command(&mut self, command: ExternalCommand) {
        let unix_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros())
            .unwrap_or(0);
        log::debug!("fork hotkeys: command {command:?} (unix_us {unix_us})");
        match &command {
            ExternalCommand::Start { binding_id } => {
                crate::fork::transforms::command::route_start(binding_id)
            }
            ExternalCommand::Cancel => crate::fork::transforms::command::cancel(),
            _ => {}
        }
        match self.app.try_state::<TranscriptionCoordinator>() {
            Some(c) => c.external_command(command, engine::ORIGIN),
            None => log::warn!("fork hotkeys: coordinator not ready; dropped {command:?}"),
        }
    }

    fn busy(&self) -> bool {
        self.app
            .try_state::<TranscriptionCoordinator>()
            .is_some_and(|c| c.is_busy())
    }

    fn set_blocking(&mut self, set: HashSet<Hotkey>) {
        log::trace!("fork hotkeys: blocking {set:?}");
        if let Some(tap) = &self.tap {
            tap.set_blocking(set.clone());
        }
        remember_blocking(&set);
        self.blocking = set;
    }

    fn prepare_mic(&mut self) {
        crate::fork::audio::ensure_prepared(&self.app);
        // The take may start in `T_show`: load the bar's web view now.
        crate::fork::pill::prewarm();
    }

    fn trigger_physically_down(&self) -> Option<bool> {
        physical_flags().map(|f| f & self.trigger_mask == self.trigger_mask)
    }

    fn warn(&mut self, kind: WarnKind) {
        log::info!("fork hotkeys: take warning {kind:?}");
        crate::fork::pill::on_take_warning(kind);
    }

    fn observe(&mut self, event: &Observed) {
        log::debug!("fork hotkeys: {event:?}");
        health::on_engine_effect(event);
        if let Observed::Effect(effect) = event {
            crate::fork::pill::on_hotkey_effect(effect);
            secure_take_effect(&self.app, effect);
            // S-05 / DEC-32: an auto-stopped take keeps its start target.
            if let Effect::Stopped {
                via: processor::StopVia::Silence | processor::StopVia::MaxLength,
            } = effect
            {
                crate::fork::insert::target::note_auto_stop();
            }
            // S-06: only a cancel the user asked for (Esc, ✕, a third
            // quick press) offers Undo. A tap with no second press, a
            // dropped chord (DEC-54) and a reset are silent. Observed
            // before the engine sends the cancel to the coordinator.
            if matches!(
                effect,
                Effect::Cancelled {
                    why: processor::CancelWhy::NoSecondPress | processor::CancelWhy::Reset,
                } | Effect::ChordIgnored {
                    mic_was_open: true,
                    ..
                }
            ) {
                crate::fork::recovery::cancel_is_silent();
            }
        }
        if let Some(ev) = ForkHotkeyEvent::from_observed(event) {
            if let Err(e) = ev.emit(&self.app) {
                log::warn!("fork hotkeys: cannot emit ForkHotkeyEvent: {e}");
            }
        }
    }
}

/// `CGEventSourceFlagsState(kCGEventSourceStateHIDSystemState)`.
fn physical_flags() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGEventSourceFlagsState(state_id: i32) -> u64;
        }
        const HID_SYSTEM_STATE: i32 = 1;
        // SAFETY: a pure query with a valid state id.
        Some(unsafe { CGEventSourceFlagsState(HID_SYSTEM_STATE) })
    }
    #[cfg(not(target_os = "macos"))]
    None
}

/// S-02: a take's start or end, for the Esc shadow and the bar's Secure
/// Input notice.
fn secure_take_effect<R: crate::fork::ForkRuntime>(app: &AppHandle<R>, effect: &Effect) {
    match effect {
        Effect::TakeStarted | Effect::Locked { .. } => {
            let was_active = secure::is_active();
            secure::set_active(true);
            if !was_active {
                if let Some(info) = R::secure_input_info(app) {
                    if info.enabled {
                        let holder = secure::holder_name(info.pid, info.process.as_deref());
                        log::info!(
                            "fork hotkeys: take under Secure Input (holder known: {})",
                            holder.is_some()
                        );
                        crate::fork::pill::secure_input_notice(holder);
                    }
                }
            }
        }
        Effect::Cancelled { .. }
        | Effect::ChordIgnored {
            mic_was_open: true, ..
        } => secure::set_active(false),
        _ => {}
    }
}

/// S-02: the take is over (Handy hid its overlay after processing): Esc is
/// no longer shadowed. Called from `fork::pill::on_overlay_hide`.
pub fn take_finished() {
    secure::set_active(false);
}

fn run<R: crate::fork::ForkRuntime>(
    app: AppHandle<R>,
    mut engine: Engine,
    tap: Option<Tap>,
    blocking: HashSet<Hotkey>,
    tx: Sender<Msg>,
    rx: mpsc::Receiver<Msg>,
) {
    let mask = |p: &Profile| {
        p.trigger
            .map(|t| engine::trigger_flag_mask(t.modifiers()))
            .unwrap_or(0)
    };
    let mut host = AppHost {
        app,
        tap,
        blocking,
        trigger_mask: mask(engine.profile()),
    };
    let mut was_idle = true;
    loop {
        let now = epoch_ms(Instant::now());
        let msg = match engine.next_wake(now) {
            Some(due) if due <= now => {
                engine.tick(now, &mut host);
                continue;
            }
            Some(due) => match rx.recv_timeout(Duration::from_millis(due - now)) {
                Ok(m) => m,
                Err(RecvTimeoutError::Timeout) => {
                    engine.tick(epoch_ms(Instant::now()), &mut host);
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => break,
            },
            None => match rx.recv() {
                Ok(m) => m,
                Err(_) => break,
            },
        };
        match msg {
            Msg::Tap(TapMsg::Event { ev, .. }) if key_test_on() && engine.grammar().is_idle() => {
                let down = engine
                    .profile()
                    .trigger
                    .is_some_and(|t| trigger_held(t, ev.modifiers));
                emit_key_test(&host.app, down);
            }
            Msg::Tap(TapMsg::Event { at, ev }) => {
                log_event(&ev, engine.grammar().fn_is_down());
                if ev.key.is_none() {
                    // Trigger edges with a wall-clock stamp, to line up with
                    // the recorder's first-audio stamp (R5 §5.1 latency).
                    let unix_us = std::time::SystemTime::now()
                        .checked_sub(at.elapsed())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_micros())
                        .unwrap_or(0);
                    log::debug!(
                        "fork hotkeys: modifiers {} down={} (unix_us {unix_us})",
                        ev.modifiers,
                        ev.is_key_down
                    );
                }
                engine.event(epoch_ms(at), &ev, &mut host);
            }
            Msg::Tap(TapMsg::Ended) => {
                log::error!("fork hotkeys: the keyboard tap stopped");
                engine.reset(epoch_ms(Instant::now()), &mut host);
                host.tap = None;
                set_status(|s| {
                    s.running = false;
                    s.error = Some("the keyboard tap stopped".into());
                });
                // S-02: the health check probes and rebuilds it.
                health::tap_stopped(health::TapId::Fork);
            }
            Msg::Rebuild { refresh } => {
                if refresh && host.tap.is_some() && !engine.grammar().is_idle() {
                    // A take is running on this tap: it works.
                    health::tap_rebuilt(health::TapId::Fork, true);
                    continue;
                }
                if !refresh && !engine.grammar().is_idle() {
                    engine.reset(epoch_ms(Instant::now()), &mut host);
                }
                match listener::start(host.blocking.clone(), tap_forwarder(tx.clone())) {
                    Ok(t) => {
                        // The old tap (if any) stops when dropped here.
                        host.tap = Some(t);
                        set_status(|s| {
                            s.running = true;
                            s.error = None;
                        });
                        log::info!("fork hotkeys: tap rebuilt");
                        health::tap_rebuilt(health::TapId::Fork, true);
                    }
                    Err(e) => {
                        log::warn!("fork hotkeys: tap rebuild failed: {e}");
                        health::tap_rebuilt(health::TapId::Fork, false);
                    }
                }
            }
            Msg::InstallTap(t) => {
                t.set_blocking(host.blocking.clone());
                host.tap = Some(t);
            }
            Msg::Shadow(action) => engine.shadow(epoch_ms(Instant::now()), &action, &mut host),
            Msg::Voice(at) => engine.voice(epoch_ms(at), &mut host),
            Msg::Pill(action) => engine.pill(epoch_ms(Instant::now()), action, &mut host),
            Msg::Profile(p) => {
                host.trigger_mask = mask(&p);
                engine.set_profile(epoch_ms(Instant::now()), *p, &mut host);
                log::info!(
                    "fork hotkeys: bindings reloaded (trigger {:?})",
                    trigger_name(engine.profile())
                );
            }
            Msg::Globe => {
                let globe = blocking::read_globe_usage();
                engine.set_fn_block(globe.blocks_fn(), &mut host);
                set_status(|s| {
                    s.globe = globe_name(globe).into();
                    s.fn_blocked = globe.blocks_fn() && s.trigger.as_deref() == Some("fn");
                });
            }
        }
        // Re-read the Globe setting once per take, when the grammar goes idle
        // again (the user may have changed it in System Settings).
        let idle = engine.grammar().is_idle();
        if idle && !was_idle {
            let g = blocking::read_globe_usage();
            if g.blocks_fn() != engine.grammar().blocks_fn() {
                log::info!("fork hotkeys: Globe setting is now {}", globe_name(g));
                engine.set_fn_block(g.blocks_fn(), &mut host);
                set_status(|s| s.globe = globe_name(g).into());
            }
        }
        was_idle = idle;
    }
    log::info!("fork hotkeys: engine stopped");
}

/// Trace-level event log. Typing with the trigger up is never logged by
/// key name.
fn log_event(ev: &KeyEvent, trigger_down: bool) {
    if log::log_enabled!(log::Level::Trace) {
        log::trace!(
            "fork hotkeys: event {} key={:?} down={} changed={:?}",
            ev.modifiers,
            ev.key.map(|k| decision::logged_key_name(k, trigger_down)),
            ev.is_key_down,
            ev.changed_modifier.map(|m| m.to_string())
        );
    }
}

/// Activates the default (fn) or the fallback (Ctrl+Opt) profile by
/// rewriting the dictation bindings in fork settings.
pub fn use_profile<R: Runtime>(app: &AppHandle<R>, fallback: bool) -> Result<(), String> {
    let handle = app
        .try_state::<ForkSettingsHandle>()
        .ok_or("fork settings are not managed")?;
    let wanted = if fallback {
        bindings::fallback_hotkeys()
    } else {
        crate::fork::settings::default_hotkeys()
    };
    handle.update(app, |s| {
        use crate::fork::settings::hotkey_actions::*;
        for id in [DICTATE, HANDS_FREE] {
            if let Some(b) = wanted.get(id) {
                s.hotkeys.insert(id.to_string(), b.clone());
            }
        }
        Ok(())
    })?;
    reload(app);
    Ok(())
}

#[cfg(test)]
pub(crate) fn status_for_test(
    profile: &Profile,
    globe: GlobeUsage,
    flat: &[crate::fork::settings::HotkeyBinding],
    actions: &[String],
    layout: &dyn labels::Layout,
) -> HotkeyStatus {
    status_for(profile, globe, flat, actions, "en", layout)
}
