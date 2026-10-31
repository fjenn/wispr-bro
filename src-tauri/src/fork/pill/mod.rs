// SPDX-License-Identifier: GPL-3.0-or-later
//! The Flow Bar (S-03, R5 §4.9, W-08).
//!
//! - [`model`]: what the bar shows, a pure state machine.
//! - [`geometry`]: every size, once (exported to the web view as JSON).
//! - [`placement`]: where it sits, pure.
//! - [`levels`]: mic levels → waveform frames.
//! - `native` (macOS): the resting bar, a native AppKit panel.
//! - `panel` (macOS): both windows on the main thread; the `fork-pill`
//!   web view exists only around a take (RS-03 memory fallback).
//!
//! The controller below runs on its own thread (`wb-pill`). Everything
//! reaches it as a message: hotkey effects (from `fork::shortcut`), Handy's
//! coordinator and completion events (listened to here), the `overlay.rs`
//! hooks, web view commands (`commands/pill.rs`), native clicks and fork
//! settings changes. After each message it renders the model into a
//! [`panel::Scene`] and hands it to the main thread when it changed.
//!
//! Hooks in Handy's files (R5 §4.2):
//! - `overlay.rs` `create_recording_overlay` (macOS):
//!   `if crate::fork::pill::replaces_handy_overlay() { return; }` — Handy's
//!   `recording_overlay` web view is never built in product builds.
//! - `overlay.rs` `show_overlay_state`: [`on_overlay_state`] (processing
//!   phases; before the `overlay_style` gate).
//! - `overlay.rs` `hide_recording_overlay`: [`on_overlay_hide`].
//! - `overlay.rs` `emit_levels`: the S-01 line `fork::audio::on_levels`
//!   now also feeds [`on_levels`] (no second hook).
//!
//! Recording phases come from Handy's typed `RecordingStateEvent` and the
//! engine's effects, so `actions.rs` needs no hook for S-03 (the planned
//! `on_record_start` hook is not needed; R5 S-03 as built).

pub mod geometry;
pub mod levels;
pub mod model;
#[cfg(target_os = "macos")]
mod native;
#[cfg(target_os = "macos")]
mod panel;
pub mod placement;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Runtime};

use self::geometry::Shape;
use self::model::{
    ActiveView, Input, Model, PillFix, PillMessage, PillPhase, PillTone, PillWarning, Prefs,
    SetupView,
};

/// How long the web view stays loaded after a take (a quick next take
/// reuses it), before it is destroyed and its WebContent process exits.
pub const WEB_LINGER_MS: u64 = 10_000;
/// A web view built at fn-down for a take that never started (a quick tap,
/// a chord) is destroyed after this.
pub const PREWARM_TTL_MS: u64 = 3_000;
/// "Hide for an hour".
pub const HIDE_FOR_MS: i64 = 60 * 60 * 1000;
/// How often the resting bar re-checks the cursor's display and the
/// settings it depends on. R5 §4.9 says 1 Hz, but with the main-thread hop
/// a 1 s period followed the cursor in up to 1,016 ms on the real slice
/// (S-03 run), over the "within 1 s" criterion; 2 Hz keeps it under.
pub const TICK_MS: u64 = 500;
/// A state not painted after this is sent to the page again (S-07a).
pub const RESEND_UNPAINTED_MS: u64 = 2_000;
/// A web view that has not painted the current state after this is
/// destroyed and built again (S-07a).
pub const REBUILD_UNPAINTED_MS: u64 = 8_000;

/// The web view is showing a take (levels are sent only then).
pub(crate) static WEB_SHOWN: AtomicBool = AtomicBool::new(false);
static RUNNING: AtomicBool = AtomicBool::new(false);
static SILENCE_WARNED: AtomicBool = AtomicBool::new(false);
static TX: OnceLock<Sender<Msg>> = OnceLock::new();
static THROTTLE: levels::Throttle = levels::Throttle::new();
/// The state the web view should show, for `fork_pill_snapshot`.
static SNAPSHOT: Mutex<Option<ForkPillState>> = Mutex::new(None);

/// The active bar, sent to `fork-pill` (and returned by
/// `fork_pill_snapshot` when the page loads).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkPillState {
    /// Increases with every change; the page echoes it in `fork_pill_ready`.
    pub seq: u32,
    pub phase: PillPhase,
    pub shape: Shape,
    /// The microphone name (first 1.5 s of a take).
    pub mic: Option<String>,
    pub message: Option<PillMessage>,
    pub tone: Option<PillTone>,
    pub fix: Option<PillFix>,
    pub warning: Option<PillWarning>,
    /// Setting up: download progress, 0-99 (S-07a).
    pub progress: Option<u8>,
    /// The card's text (S-07a); `None` on a card for a password field
    /// (S-05: the text never reaches the web view; Copy still copies it).
    pub text: Option<String>,
    /// Why the card's take was not pasted (S-05; `fork.json` >
    /// `rust.pill.card.<reason>`); `None` for a queued take (S-07a).
    pub card_reason: Option<crate::fork::events::ForkCardReason>,
    /// Takes that finished before the one on the card.
    pub more: u32,
    /// The card's text was just copied.
    pub copied: bool,
}

/// One waveform frame (≤ 30 per second, only to `fork-pill`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkPillLevels {
    pub bars: Vec<f32>,
}

/// What the web view's buttons ask for (`fork_pill_action`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PillUiAction {
    /// ✓: stop and insert.
    Stop,
    /// ✕ on a take: cancel.
    Cancel,
    /// ✕ on a message.
    Dismiss,
    /// The message's Fix button (Retry for a setup problem).
    Fix,
    /// The card's Copy button: the text replaces the clipboard (no restore).
    Copy,
}

#[derive(Debug)]
enum Msg {
    Input(Input),
    Prewarm,
    Ready(u32),
    Ui(PillUiAction),
    #[cfg(target_os = "macos")]
    Native(native::NativeEvent),
    Settings(Box<crate::fork::settings::ForkSettings>),
    Theme(Option<bool>),
}

fn send(msg: Msg) {
    if let Some(tx) = TX.get() {
        let _ = tx.send(msg);
    }
}

fn input(i: Input) {
    send(Msg::Input(i));
}

// ---------------------------------------------------------------------------
// Hooks and entry points
// ---------------------------------------------------------------------------

/// `overlay.rs` `create_recording_overlay` (macOS) hook: the fork's bar
/// replaces Handy's overlay in product builds, so Handy's hidden
/// `recording_overlay` web view (and its WebKit process) never exists.
/// Runs during Handy's setup, before [`start`].
pub fn replaces_handy_overlay() -> bool {
    cfg!(target_os = "macos") && crate::fork::is_product_build()
}

/// `overlay.rs` `show_overlay_state` hook (first line, before the
/// `overlay_style` gate): `recording`, `streaming`, `transcribing`,
/// `processing`.
pub fn on_overlay_state<R: Runtime>(_app: &AppHandle<R>, state: &str) {
    match state {
        "recording" | "streaming" => input(Input::TakeStarted { locked: false }),
        "transcribing" => input(Input::Transcribing),
        "processing" => input(Input::CleaningUp),
        _ => {}
    }
}

/// `overlay.rs` `hide_recording_overlay` hook.
pub fn on_overlay_hide<R: Runtime>(_app: &AppHandle<R>) {
    input(Input::Hide);
}

/// Mic levels (from `fork::audio::on_levels`, the `emit_levels` hook).
pub fn on_levels<R: Runtime>(app: &AppHandle<R>, levels: &[f32]) {
    if !WEB_SHOWN.load(Ordering::Acquire) {
        return;
    }
    if SILENCE_WARNED.load(Ordering::Relaxed) && crate::fork::audio::is_voice(levels) {
        SILENCE_WARNED.store(false, Ordering::Relaxed);
        input(Input::Voice);
    }
    if !THROTTLE.admit(monotonic_ms()) {
        return;
    }
    let frame = ForkPillLevels {
        bars: levels::bars(levels),
    };
    if log::log_enabled!(log::Level::Debug) {
        log_level_peak(&frame.bars);
    }
    let _ = tauri_specta::Event::emit_to(&frame, app, panel_label());
}

/// Debug log, once a second while a take shows: the loudest bar sent and
/// how many frames went out (the real-Mac waveform check).
fn log_level_peak(bars: &[f32]) {
    use std::sync::atomic::{AtomicU32, AtomicU64};
    static PEAK: AtomicU32 = AtomicU32::new(0);
    static FRAMES: AtomicU32 = AtomicU32::new(0);
    static LAST: AtomicU64 = AtomicU64::new(0);
    let peak = bars.iter().copied().fold(0.0f32, f32::max);
    PEAK.fetch_max(peak.to_bits(), Ordering::Relaxed);
    FRAMES.fetch_add(1, Ordering::Relaxed);
    let now = monotonic_ms();
    if now.saturating_sub(LAST.load(Ordering::Relaxed)) >= 1_000 {
        LAST.store(now, Ordering::Relaxed);
        let peak = f32::from_bits(PEAK.swap(0, Ordering::Relaxed));
        let frames = FRAMES.swap(0, Ordering::Relaxed);
        log::debug!("pill: level frames {frames}, loudest bar {peak:.3}");
    }
}

/// A hotkey decision (`fork::shortcut` engine host).
pub fn on_hotkey_effect(effect: &crate::fork::shortcut::processor::Effect) {
    use crate::fork::shortcut::processor::{Effect, LockVia};
    match effect {
        Effect::TakeStarted => input(Input::TakeStarted { locked: false }),
        Effect::Locked { via: LockVia::Pill } => input(Input::TakeStarted { locked: true }),
        Effect::Locked { .. } => input(Input::Locked),
        Effect::Stopped { .. } => input(Input::Stopped),
        Effect::Cancelled { .. } => input(Input::Cancelled),
        Effect::ChordIgnored {
            mic_was_open: true, ..
        } => input(Input::Cancelled),
        _ => {}
    }
}

/// A take is about to stop on its own (S-01 rule 8).
pub fn on_take_warning(kind: crate::fork::shortcut::processor::WarnKind) {
    use crate::fork::shortcut::processor::WarnKind;
    let w = match kind {
        WarnKind::Silence => {
            SILENCE_WARNED.store(true, Ordering::Relaxed);
            PillWarning::Silence
        }
        WarnKind::Length => PillWarning::Length,
    };
    input(Input::Warning(w));
}

/// The take's microphone disappeared (`fork::audio::watch`, S-08).
pub fn warn_mic_lost() {
    input(Input::Warning(PillWarning::MicLost));
}

/// fn went down: a take may start in `T_show`, so build the web view now.
pub fn prewarm() {
    send(Msg::Prewarm);
}

/// The fork settings changed (`fork_set_setting`).
pub fn settings_changed(settings: &crate::fork::settings::ForkSettings) {
    send(Msg::Settings(Box::new(settings.clone())));
}

/// The LLM cleanup of the take started (S-12a): "Cleaning up…". Handy's own
/// "processing" overlay state only comes with its post-process hotkey, which
/// the fork keeps off.
pub fn cleaning_up() {
    input(Input::CleaningUp);
}

/// A later slice reports a message (S-12a: "Inserted without cleanup").
pub fn show_message(message: PillMessage) {
    input(Input::Message(message));
}

/// S-07a: a take stopped before any speech model was ready.
pub fn on_take_queued() {
    input(Input::Queued);
}

/// S-07a: the speech setup changed.
pub fn on_speech_setup(status: &crate::fork::models::setup::ForkSpeechSetup) {
    input(Input::Setup(setup_view(status)));
}

/// S-07a: the queued takes are being transcribed.
pub fn on_queue_transcribing() {
    input(Input::QueueTranscribing);
}

/// S-07a: a queued take's text, for the card with Copy.
pub fn show_card(text: String) {
    input(Input::Card(text.into()));
}

/// S-06: a take was cancelled and can be brought back: "Cancelled · Undo"
/// (the cancel itself returns the bar to rest first, so a cancel the engine
/// did not see, such as `--cancel`, ends the take on screen too).
pub fn show_undo_offer() {
    input(Input::Cancelled);
    input(Input::Message(PillMessage::Cancelled));
}

/// S-06: Undo is transcribing the cancelled take.
pub fn undo_started() {
    input(Input::Undoing);
}

/// S-06: Undo pasted the take; the bar rests (after the usual grace).
pub fn undo_finished() {
    input(Input::Hide);
}

/// S-05: a take that was not pasted (`fork::insert`), on the same card with
/// its reason as the label.
pub fn show_insert_card(reason: crate::fork::events::ForkCardReason, text: String) {
    input(Input::Card(model::CardContent {
        text,
        reason: Some(reason),
    }));
}

/// The `rust.pill.card.*` key of a card reason (its label on the card and
/// its VoiceOver announcement).
pub fn card_key(reason: crate::fork::events::ForkCardReason) -> String {
    let name = serde_json::to_value(reason)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default();
    format!("rust.pill.card.{name}")
}

/// What the bar shows for a setup status.
pub fn setup_view(s: &crate::fork::models::setup::ForkSpeechSetup) -> SetupView {
    use crate::fork::models::download::SetupProblem;
    use crate::fork::models::setup::SetupPhase;
    let problem = match (s.phase, s.problem) {
        (_, Some(SetupProblem::NoSpace)) => Some(PillMessage::NoSpace),
        (SetupPhase::Failed | SetupPhase::Waiting, Some(SetupProblem::CheckConnection)) => {
            Some(PillMessage::CheckConnection)
        }
        (SetupPhase::Failed, _) => Some(PillMessage::DownloadFailed),
        _ => None,
    };
    SetupView {
        percent: s.percent,
        problem,
        ready: s.phase == SetupPhase::Ready,
    }
}

/// `fork_pill_snapshot`.
pub fn snapshot() -> Option<ForkPillState> {
    SNAPSHOT.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// `fork_pill_ready`: the page painted state `seq`.
pub fn ready(seq: u32) {
    send(Msg::Ready(seq));
}

/// `fork_pill_action`.
pub fn ui_action(action: PillUiAction) {
    send(Msg::Ui(action));
}

/// Whether the bar runs (product builds on macOS, after [`start`]).
pub fn is_running() -> bool {
    RUNNING.load(Ordering::Acquire)
}

fn panel_label() -> &'static str {
    "fork-pill"
}

fn monotonic_ms() -> u64 {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    EPOCH.get_or_init(Instant::now).elapsed().as_millis() as u64 + 1
}

fn unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// The state event for an active view.
pub fn state_event(seq: u32, a: &ActiveView) -> ForkPillState {
    ForkPillState {
        seq,
        phase: a.phase,
        shape: a.shape,
        mic: a.mic.clone(),
        message: a.message,
        tone: a.message.map(PillMessage::tone),
        fix: a.message.and_then(PillMessage::fix),
        warning: a.warning,
        progress: a.progress,
        text: a.text.clone(),
        card_reason: a.card_reason,
        more: a.more,
        copied: a.copied,
    }
}

/// The VoiceOver announcement for a change of the bar, if any (`rust.pill.*`
/// keys), and whether it is high priority.
pub fn announcement(
    before: Option<&ActiveView>,
    after: Option<&ActiveView>,
) -> Option<(String, bool)> {
    let phase = |v: Option<&ActiveView>| v.map(|a| a.phase);
    if phase(before) == phase(after) && before.map(|a| a.message) == after.map(|a| a.message) {
        let warned = after.and_then(|a| a.warning);
        if warned.is_some() && before.and_then(|a| a.warning) != warned {
            let key = match warned? {
                PillWarning::Silence => "rust.pill.announce.silenceWarning",
                PillWarning::Length => "rust.pill.announce.lengthWarning",
                PillWarning::MicLost => "rust.pill.announce.micLostWarning",
            };
            return Some((key.into(), true));
        }
        return None;
    }
    match (phase(before), after) {
        (None | Some(PillPhase::Message), Some(a))
            if matches!(a.phase, PillPhase::Held | PillPhase::Locked) =>
        {
            Some(("rust.pill.announce.listening".into(), false))
        }
        (_, Some(a)) if a.phase == PillPhase::Transcribing => {
            Some(("rust.pill.announce.transcribing".into(), false))
        }
        (_, Some(a)) if a.phase == PillPhase::CleaningUp => {
            Some(("rust.pill.announce.cleaningUp".into(), false))
        }
        (_, Some(a)) if a.phase == PillPhase::SettingUp => {
            Some(("rust.pill.announce.settingUp".into(), false))
        }
        (_, Some(a)) if a.phase == PillPhase::Card => Some((
            a.card_reason
                .map_or_else(|| "rust.pill.announce.card".into(), card_key),
            true,
        )),
        (_, Some(a)) if a.phase == PillPhase::Message => a.message.map(|m| {
            let key = match m {
                // "Cancelled" was just announced by the take's end.
                PillMessage::Cancelled => "rust.pill.announce.undo".to_string(),
                _ => format!("rust.pill.message.{}", m.key()),
            };
            (key, m.tone() == PillTone::Error)
        }),
        (Some(PillPhase::Held | PillPhase::Locked), None) => {
            Some(("rust.pill.announce.cancelled".into(), false))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Start (macOS product builds)
// ---------------------------------------------------------------------------

/// Starts the bar: the controller thread, the main-thread panels and the
/// event listeners. From `fork::on_ready` (main thread), product builds.
#[cfg(target_os = "macos")]
pub fn start(app: &AppHandle<tauri::Wry>) {
    use tauri::{Listener, Manager};
    if RUNNING.swap(true, Ordering::AcqRel) {
        return;
    }
    let (tx, rx) = mpsc::channel::<Msg>();
    if TX.set(tx.clone()).is_err() {
        return;
    }
    let native_tx = tx.clone();
    panel::init(app, move |e| {
        let _ = native_tx.send(Msg::Native(e));
    });

    app.listen_any("recording-state-event", |event| {
        let phase = serde_json::from_str::<serde_json::Value>(event.payload())
            .ok()
            .and_then(|v| v.get("phase")?.as_str().map(str::to_owned));
        match phase.as_deref() {
            Some("started") => input(Input::TakeStarted { locked: false }),
            Some("locked") => input(Input::Locked),
            Some("stopped") => input(Input::Stopped),
            Some("cancelled") => input(Input::Cancelled),
            _ => {}
        }
    });
    app.listen_any("transcription-complete-event", |event| {
        let outcome = serde_json::from_str::<serde_json::Value>(event.payload())
            .ok()
            .and_then(|v| v.get("outcome")?.as_str().map(str::to_owned));
        // S-07a: a queued take's empty outcome is not "No speech heard".
        if matches!(outcome.as_deref(), Some("empty" | "silent"))
            && crate::fork::models::setup::take_queued_outcome()
        {
            return;
        }
        if let Some(m) = outcome.as_deref().and_then(message_for_outcome) {
            input(Input::Message(m));
        }
    });
    app.listen_any("recording-error", |event| {
        let kind = serde_json::from_str::<serde_json::Value>(event.payload())
            .ok()
            .and_then(|v| v.get("error_type")?.as_str().map(str::to_owned));
        input(Input::Message(message_for_recording_error(
            kind.as_deref().unwrap_or("unknown"),
        )));
    });
    app.listen_any("fork-insert-outcome", |event| {
        let card = serde_json::from_str::<serde_json::Value>(event.payload())
            .ok()
            .and_then(|v| v.get("outcome")?.as_str().map(|s| s == "card"))
            .unwrap_or(false);
        if card {
            input(Input::Message(PillMessage::NotPasted));
        }
    });
    app.listen_any("theme-changed", |event| {
        let theme = serde_json::from_str::<crate::settings::Theme>(event.payload()).ok();
        if let Some(t) = theme {
            send(Msg::Theme(theme_dark(t)));
        }
    });

    // Esc dismisses a message, "Setting up" or the card (observed, never
    // blocked; an Esc that cancelled a take is the take's).
    crate::fork::shortcut::registry().observe_esc(std::sync::Arc::new(|ctx| {
        if !ctx.cancelled_take {
            input(Input::Dismiss);
        }
    }));

    let settings = app
        .try_state::<crate::fork::settings::ForkSettingsHandle>()
        .and_then(|h| h.get(app).ok())
        .unwrap_or_default();
    let handy = crate::settings::get_settings(app);
    let controller = Controller::new(app.clone(), settings, &handy);
    let spawned = std::thread::Builder::new()
        .name("wb-pill".into())
        .spawn(move || controller.run(rx));
    if let Err(e) = spawned {
        log::error!("pill: cannot spawn the controller thread: {e}");
        RUNNING.store(false, Ordering::Release);
        return;
    }
    log::info!("pill: Flow Bar started (native resting bar, web view per take)");
}

fn theme_dark(t: crate::settings::Theme) -> Option<bool> {
    match t {
        crate::settings::Theme::System => None,
        crate::settings::Theme::Light => Some(false),
        crate::settings::Theme::Dark => Some(true),
    }
}

/// Handy's `TranscriptionOutcome` → a message (`typed` has none).
pub fn message_for_outcome(outcome: &str) -> Option<PillMessage> {
    match outcome {
        "empty" => Some(PillMessage::NoSpeech),
        "silent" => Some(PillMessage::MicMuted),
        "failed" => Some(PillMessage::TranscriptionFailed),
        _ => None,
    }
}

/// Handy's `recording-error` `error_type` → a message.
pub fn message_for_recording_error(kind: &str) -> PillMessage {
    match kind {
        "microphone_permission_denied" => PillMessage::MicDenied,
        "no_input_device" => PillMessage::NoMicrophone,
        _ => PillMessage::MicFailed,
    }
}

/// Setup has ended (DEC-38): the fork's onboarding is completed, or, until
/// the fork's onboarding (S-29a) exists, Handy's own.
pub fn setup_done(
    fork: &crate::fork::settings::ForkSettings,
    handy_onboarding_completed: bool,
) -> bool {
    fork.onboarding.status == crate::fork::settings::OnboardingStatus::Completed
        || handy_onboarding_completed
}

/// What may show at rest, from the settings.
pub fn prefs_for(
    fork: &crate::fork::settings::ForkSettings,
    handy_onboarding_completed: bool,
    unix_now_ms: i64,
) -> Prefs {
    Prefs {
        setup_done: setup_done(fork, handy_onboarding_completed),
        always_visible: fork.bar_always_visible,
        hidden_for_now: fork.bar_hidden_until_ms.is_some_and(|t| t > unix_now_ms),
    }
}

// ---------------------------------------------------------------------------
// The controller (macOS)
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Web {
    Absent,
    /// Built; `shown` while visible at a shape.
    Built {
        shown: Option<Shape>,
    },
}

#[cfg(target_os = "macos")]
struct Controller {
    app: AppHandle<tauri::Wry>,
    model: Model,
    settings: crate::fork::settings::ForkSettings,
    handy_onboarding: bool,
    locale: &'static str,
    dark: Option<bool>,
    seq: u32,
    sent: Option<ActiveView>,
    painted: u32,
    web: Web,
    /// The web view is destroyed at this time (linger or prewarm).
    web_until: Option<u64>,
    /// When the current take's web view started loading (for the log).
    loading_since: Option<Instant>,
    scene: Option<panel::Scene>,
    next_tick: u64,
    /// The microphone name is being looked up for the current take.
    mic_lookup: bool,
    /// S-07a: since when the newest state waits for its paint, and when it
    /// was last resent.
    unpainted_since: Option<u64>,
    last_resend: u64,
}

#[cfg(target_os = "macos")]
impl Controller {
    fn new(
        app: AppHandle<tauri::Wry>,
        settings: crate::fork::settings::ForkSettings,
        handy: &crate::settings::AppSettings,
    ) -> Controller {
        let locale = crate::fork::i18n::current_locale(Some(&handy.app_language));
        let prefs = prefs_for(&settings, handy.onboarding_completed, unix_ms());
        Controller {
            app,
            model: Model::new(prefs),
            settings,
            handy_onboarding: handy.onboarding_completed,
            locale,
            dark: theme_dark(handy.theme),
            seq: 0,
            sent: None,
            painted: 0,
            web: Web::Absent,
            web_until: None,
            loading_since: None,
            scene: None,
            next_tick: 0,
            mic_lookup: false,
            unpainted_since: None,
            last_resend: 0,
        }
    }

    fn run(mut self, rx: Receiver<Msg>) {
        self.render(monotonic_ms());
        loop {
            let now = monotonic_ms();
            let deadline = [
                Some(self.next_tick),
                self.model.next_deadline(),
                self.web_until,
            ]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(now + TICK_MS);
            let wait = deadline.saturating_sub(now);
            match rx.recv_timeout(Duration::from_millis(wait.max(1))) {
                Ok(msg) => self.handle(msg),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            let now = monotonic_ms();
            if now >= self.next_tick {
                self.tick(now);
            }
            self.render(now);
        }
    }

    fn handle(&mut self, msg: Msg) {
        let now = monotonic_ms();
        match msg {
            Msg::Input(Input::Dismiss) => self.dismiss(now),
            Msg::Input(i) => {
                let starting = matches!(i, Input::TakeStarted { .. } | Input::Locked)
                    && !self.model.recording();
                self.model.step(now, i);
                if starting && self.model.recording() {
                    self.lookup_mic();
                }
            }
            Msg::Prewarm => {
                if self.web == Web::Absent {
                    self.web = Web::Built { shown: None };
                    self.loading_since = Some(Instant::now());
                }
                if !self.model.active() {
                    self.web_until = Some(now + PREWARM_TTL_MS);
                }
            }
            Msg::Ready(seq) => {
                if seq > self.painted {
                    self.painted = seq;
                }
                if seq == self.seq {
                    if let Some(t) = self.loading_since.take() {
                        log::info!(
                            "pill: web view painted state {seq} {:?} after load start",
                            t.elapsed()
                        );
                    }
                }
            }
            Msg::Ui(action) => self.ui_action(now, action),
            Msg::Native(e) => self.native_event(e),
            Msg::Settings(s) => {
                self.settings = *s;
                self.refresh_prefs();
            }
            Msg::Theme(dark) => self.dark = dark,
        }
    }

    fn ui_action(&mut self, now: u64, action: PillUiAction) {
        use crate::fork::shortcut::engine::PillAction;
        match action {
            PillUiAction::Stop => self.pill_action(PillAction::Stop),
            PillUiAction::Cancel => self.pill_action(PillAction::Cancel),
            PillUiAction::Dismiss => self.dismiss(now),
            PillUiAction::Fix => {
                let fix = self
                    .sent
                    .as_ref()
                    .and_then(|a| a.message)
                    .and_then(PillMessage::fix);
                if fix == Some(PillFix::Undo) {
                    // S-06: the window may have closed a moment ago.
                    if !crate::fork::recovery::undo("bar") {
                        self.model.step(now, Input::Dismiss);
                    }
                    return;
                }
                if fix == Some(PillFix::RetryDownload) {
                    // Back to the progress while the download starts again.
                    log::info!("pill: Retry (speech model download)");
                    crate::fork::models::setup::retry();
                    let current = self.model.setup();
                    self.model.step(
                        now,
                        Input::Setup(SetupView {
                            problem: None,
                            ..current
                        }),
                    );
                    return;
                }
                if let Some(fix) = fix {
                    open_fix(fix);
                }
                self.model.step(now, Input::Dismiss);
            }
            PillUiAction::Copy => {
                // The model holds the full text (a password field's is not
                // in the web view's state); it is copied concealed (S-05).
                let card = self.model.card().cloned();
                if let Some(card) = card {
                    let text = card.text.clone();
                    match crate::fork::clipboard::nspb::write_text(&text, card.hidden()) {
                        Ok(_) => {
                            log::info!("pill: card text copied ({} chars)", text.chars().count());
                            self.model.step(now, Input::Copied);
                        }
                        Err(e) => log::warn!("pill: could not copy the card text: {e}"),
                    }
                }
            }
        }
    }

    /// ✕ or Esc on a message, "Setting up" or the card. Dismissing
    /// "Cancelled · Undo" also drops the cancelled audio at once (S-06).
    fn dismiss(&mut self, now: u64) {
        let offer = self.sent.as_ref().and_then(|a| a.message) == Some(PillMessage::Cancelled);
        if offer {
            crate::fork::recovery::discard("dismissed");
        }
        self.model.step(now, Input::Dismiss);
    }

    fn pill_action(&self, action: crate::fork::shortcut::engine::PillAction) {
        use crate::fork::shortcut::engine::PillAction;
        use crate::transcription_coordinator::{ExternalCommand, TranscriptionCoordinator};
        use tauri::Manager;
        log::info!("pill: {action:?}");
        if crate::fork::shortcut::pill_action(action) {
            return;
        }
        // No fork hotkey engine (it failed to start): drive the coordinator.
        let Some(c) = self.app.try_state::<TranscriptionCoordinator>() else {
            return;
        };
        let origin = "fork:pill";
        match action {
            PillAction::Click if !c.is_busy() => {
                c.external_command(
                    ExternalCommand::Start {
                        binding_id: crate::fork::shortcut::engine::BINDING.into(),
                    },
                    origin,
                );
                c.external_command(ExternalCommand::Lock, origin);
            }
            PillAction::Click | PillAction::Stop => {
                c.external_command(ExternalCommand::Stop, origin)
            }
            PillAction::Cancel => c.external_command(ExternalCommand::Cancel, origin),
        }
    }

    fn native_event(&mut self, e: native::NativeEvent) {
        use crate::fork::shortcut::engine::PillAction;
        match e {
            native::NativeEvent::Click => {
                // Load the web view while the take starts.
                send(Msg::Prewarm);
                self.pill_action(PillAction::Click);
            }
            native::NativeEvent::HideForAnHour => {
                let until = unix_ms() + HIDE_FOR_MS;
                self.update_settings(move |s| s.bar_hidden_until_ms = Some(until));
            }
            native::NativeEvent::Dragged { key, x_frac } => {
                self.update_settings(move |s| match x_frac {
                    Some(f) => {
                        s.bar_x_by_display.insert(key, f);
                    }
                    None => {
                        s.bar_x_by_display.remove(&key);
                    }
                });
            }
        }
    }

    fn update_settings(&mut self, f: impl FnOnce(&mut crate::fork::settings::ForkSettings)) {
        use tauri::Manager;
        use tauri_specta::Event;
        let Some(handle) = self
            .app
            .try_state::<crate::fork::settings::ForkSettingsHandle>()
        else {
            return;
        };
        match handle.update(&self.app, |s| {
            f(s);
            // Keep the remembered positions within the settings limits.
            while s.bar_x_by_display.len() > crate::fork::settings::MAX_BAR_DISPLAYS {
                let first = s.bar_x_by_display.keys().next().cloned();
                if let Some(k) = first {
                    s.bar_x_by_display.remove(&k);
                }
            }
            Ok(())
        }) {
            Ok((next, ())) => {
                self.settings = next.clone();
                self.refresh_prefs();
                let event = crate::fork::events::ForkSettingsChanged {
                    key: "bar".into(),
                    settings: next,
                };
                let _ = event.emit(&self.app);
            }
            Err(e) => log::warn!("pill: cannot save the bar settings: {e}"),
        }
    }

    fn refresh_prefs(&mut self) {
        let prefs = prefs_for(&self.settings, self.handy_onboarding, unix_ms());
        if prefs != self.model.prefs() {
            self.model.step(monotonic_ms(), Input::Prefs(prefs));
        }
    }

    /// Once a second: Handy-side settings (onboarding, language), the
    /// "hide for an hour" timer, and the cursor's display.
    fn tick(&mut self, now: u64) {
        self.next_tick = now + TICK_MS;
        let handy = crate::settings::get_settings(&self.app);
        self.handy_onboarding = handy.onboarding_completed;
        self.locale = crate::fork::i18n::current_locale(Some(&handy.app_language));
        self.refresh_prefs();
        if self
            .scene
            .as_ref()
            .is_some_and(|s| s.native == panel::NativeScene::Rest)
        {
            let _ = self.app.run_on_main_thread(panel::follow_cursor);
        }
    }

    fn lookup_mic(&mut self) {
        if self.mic_lookup {
            return;
        }
        self.mic_lookup = true;
        let selected = crate::settings::get_settings(&self.app).selected_microphone;
        std::thread::Builder::new()
            .name("wb-pill-mic".into())
            .spawn(move || {
                // S-08: the mic the take really opened (the wireless rule
                // may have picked the built-in mic over the default).
                let name = crate::fork::audio::opened_name().or(selected).or_else(|| {
                    handy_recorder::list_input_devices()
                        .ok()?
                        .into_iter()
                        .find(|d| d.is_default)
                        .map(|d| d.name)
                });
                if let Some(name) = name {
                    input(Input::MicName(name));
                }
            })
            .map(|_| ())
            .unwrap_or_else(|e| log::warn!("pill: cannot look up the microphone: {e}"));
    }

    fn texts(&self) -> native::Texts {
        let t = |k: &str| crate::fork::i18n::t(self.locale, k, &[]);
        native::Texts {
            hint: t("rust.pill.hint"),
            name: t("rust.pill.name"),
            hide_for_an_hour: t("rust.pill.hideForAnHour"),
        }
    }

    fn render(&mut self, now: u64) {
        let view = self.model.view_at(now);
        if !self.model.recording() {
            self.mic_lookup = false;
        }
        // VoiceOver.
        if let Some((key, high)) = announcement(self.sent.as_ref(), view.active.as_ref()) {
            let text = crate::fork::i18n::t(self.locale, &key, &[]);
            let _ = self
                .app
                .run_on_main_thread(move || panel::announce(&text, high));
        }

        let (native, web) = match &view.active {
            Some(active) => {
                self.web_until = None;
                if self.sent.as_ref() != Some(active) {
                    self.seq += 1;
                    let state = state_event(self.seq, active);
                    *SNAPSHOT.lock().unwrap_or_else(|e| e.into_inner()) = Some(state.clone());
                    if self.web != Web::Absent {
                        let _ = tauri_specta::Event::emit_to(&state, &self.app, panel_label());
                    }
                    self.sent = Some(active.clone());
                }
                if self.web == Web::Absent {
                    self.web = Web::Built { shown: None };
                    self.loading_since = Some(Instant::now());
                }
                let shown = match self.web {
                    Web::Built { shown } => shown,
                    Web::Absent => None,
                };
                // S-07a: a state that waits for its paint gets resent, and a
                // web view that never paints is rebuilt (seen once on the real
                // Mac: a card in a fresh web view after a long idle stayed on
                // the bridge).
                if self.painted < self.seq {
                    let since = *self.unpainted_since.get_or_insert(now);
                    let waited = now.saturating_sub(since);
                    if waited >= REBUILD_UNPAINTED_MS {
                        log::warn!(
                            "pill: web view did not paint state {} in {waited} ms; rebuilding it",
                            self.seq
                        );
                        self.unpainted_since = None;
                        self.web = Web::Absent;
                        self.painted = 0;
                        self.loading_since = None;
                        let scene = panel::Scene {
                            native: panel::NativeScene::Bridge(active.shape),
                            web: panel::WebScene::None,
                            ..self.scene_base()
                        };
                        self.apply_scene(scene);
                        return;
                    }
                    if waited >= RESEND_UNPAINTED_MS
                        && now.saturating_sub(self.last_resend) >= RESEND_UNPAINTED_MS
                    {
                        self.last_resend = now;
                        log::debug!("pill: resending state {} (not painted yet)", self.seq);
                        if let Some(state) = snapshot() {
                            let _ = tauri_specta::Event::emit_to(&state, &self.app, panel_label());
                        }
                    }
                } else {
                    self.unpainted_since = None;
                }
                if self.painted >= self.seq {
                    self.web = Web::Built {
                        shown: Some(active.shape),
                    };
                    (
                        panel::NativeScene::Hidden,
                        panel::WebScene::Shown(active.shape),
                    )
                } else if let Some(old) = shown {
                    // Keep the last painted frame until the new one is ready.
                    (panel::NativeScene::Hidden, panel::WebScene::Shown(old))
                } else {
                    (
                        panel::NativeScene::Bridge(active.shape),
                        panel::WebScene::Loading,
                    )
                }
            }
            None => {
                if self.sent.take().is_some() {
                    *SNAPSHOT.lock().unwrap_or_else(|e| e.into_inner()) = None;
                    // Tell the page to clear, so a reuse never flashes the
                    // old state.
                    self.seq += 1;
                }
                let native = if view.resting {
                    panel::NativeScene::Rest
                } else {
                    panel::NativeScene::Hidden
                };
                let web = match self.web {
                    Web::Absent => panel::WebScene::None,
                    Web::Built { shown } => {
                        let until = *self.web_until.get_or_insert(
                            now + if shown.is_some() {
                                WEB_LINGER_MS
                            } else {
                                PREWARM_TTL_MS
                            },
                        );
                        if now >= until {
                            self.web = Web::Absent;
                            self.web_until = None;
                            self.painted = 0;
                            self.loading_since = None;
                            panel::WebScene::None
                        } else if shown.is_some() {
                            self.web = Web::Built { shown: None };
                            panel::WebScene::Lingering
                        } else if self
                            .scene
                            .as_ref()
                            .is_some_and(|s| s.web == panel::WebScene::Lingering)
                        {
                            panel::WebScene::Lingering
                        } else {
                            panel::WebScene::Loading
                        }
                    }
                };
                (native, web)
            }
        };
        let scene = panel::Scene {
            native,
            web,
            ..self.scene_base()
        };
        self.apply_scene(scene);
    }

    /// The scene's settings-driven parts (`native` and `web` are filled in
    /// by the caller).
    fn scene_base(&self) -> panel::Scene {
        panel::Scene {
            native: panel::NativeScene::Hidden,
            web: panel::WebScene::None,
            edge: match self.settings.bar_position {
                crate::fork::settings::BarPosition::Bottom => placement::Edge::Bottom,
                crate::fork::settings::BarPosition::Top => placement::Edge::Top,
            },
            x_by_display: self.settings.bar_x_by_display.clone(),
            protected: self.settings.bar_hide_in_recordings,
            dark: self.dark,
            texts: self.texts(),
        }
    }

    fn apply_scene(&mut self, scene: panel::Scene) {
        if self.scene.as_ref() != Some(&scene) {
            self.scene = Some(scene.clone());
            let _ = self.app.run_on_main_thread(move || panel::apply(scene));
        }
    }
}

/// Opens the System Settings pane a Fix button points at.
#[cfg(target_os = "macos")]
fn open_fix(fix: PillFix) {
    let url = match fix {
        PillFix::MicrophonePrivacy => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
        }
        PillFix::SoundInput => "x-apple.systempreferences:com.apple.Sound-Settings.extension?input",
        PillFix::RetryDownload => {
            crate::fork::models::setup::retry();
            return;
        }
        PillFix::Undo => {
            crate::fork::recovery::undo("bar");
            return;
        }
    };
    if let Err(e) = std::process::Command::new("/usr/bin/open").arg(url).spawn() {
        log::warn!("pill: cannot open System Settings: {e}");
    }
}

#[cfg(test)]
mod tests;
