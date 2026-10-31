// SPDX-License-Identifier: GPL-3.0-or-later
//! Delivery of a take's text (R5 S-04). S-11's post-LLM text stages
//! (`fork::text::finish`) run first; then S-05's target check ([`target`]:
//! still the element focused at key-up, and a place to type that is not a
//! password field, else the card); S-17 adds its stage in front of the
//! paste later.
//!
//! Handy reaches this through one hook: `actions.rs` calls [`deliver`]
//! where it called `utils::paste` (same signature). On macOS with the ⌘V
//! paste method, the fork owns the paste end to end (`fork::clipboard` +
//! [`deliver::paste`] / [`deliver::settle`]); every other paste method, and
//! every other platform, goes to Handy's `utils::paste` unchanged. Handy's
//! `clipboard.rs` and `paste_tx/` are not edited (R5 §4.2 row updated by
//! S-04).
//!
//! - [`deliver`]: the engine, pure over a `Platform` (tests: fakes).
//! - [`verify`]: the ≤ 300 ms field check behind `confirmed`/`unverified`.
//! - [`session`]: sleep, lock and user switching → card, never pasted blind.
//! - [`target`]: the key-up target and the "no text field" decision (S-05).
//!
//! Every take gets a `paste_outcome` ([`deliver::PasteOutcome`]), logged
//! here and sent as [`ForkInsertOutcome`](super::events::ForkInsertOutcome)
//! (S-09 stores it per take). A card goes to the Flow Bar
//! ([`crate::fork::pill::show_insert_card`]: the reason, the text, Copy, ✕); the text is in
//! History either way.

pub mod deliver;
pub mod session;
pub mod target;
pub mod verify;

#[cfg(target_os = "macos")]
mod mac;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

use tauri::{AppHandle, Listener, Manager, Runtime};

use super::events::{ForkCardReason, ForkInsertOutcome, ForkPasteOutcome};
use deliver::{CardReason, Delivery, Line, PasteOutcome};
use session::{Interruption, SessionLog, Signal};

/// The user's clipboard across back-to-back takes (hex `ClipboardRestore`)
/// and the paste still waiting for its read.
static LINE: Mutex<Line> = Mutex::new(Line::new());
/// One delivery at a time, in arrival order (the FIFO slot).
static SLOT: Mutex<()> = Mutex::new(());
static SESSION: Mutex<SessionLog> = Mutex::new(SessionLog::new());
static LAST_CARD: Mutex<Option<CardText>> = Mutex::new(None);
static TAKE_SEQ: AtomicU32 = AtomicU32::new(0);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A take that went to the card, for the pill's card (S-05).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardText {
    pub take: u32,
    pub reason: CardReason,
    pub text: String,
}

/// The most recent take that was not pasted, if any (S-05 shows it; Copy
/// replaces the clipboard and does not restore it).
pub fn last_card() -> Option<CardText> {
    lock(&LAST_CARD).clone()
}

/// What a delivery carries (S-06 adds the two recovery kinds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A take that was just transcribed (the `actions.rs` hook).
    Take,
    /// A cancelled take brought back by Undo: it goes into the element that
    /// had focus at the cancel (`target`, an AX capture) if that still has
    /// focus, else to the card (`focus_moved`).
    Undo { target: Option<u64> },
    /// The last transcript again (paste-last, ⌃⌘V): already final text, so
    /// no text stages and no auto-submit; it goes wherever focus is now.
    Again,
}

impl Kind {
    /// The take's own delivery, whose outcome History records through the
    /// take ledger (the recovery kinds record theirs by entry id).
    fn is_take(self) -> bool {
        self == Kind::Take
    }
}

/// The `actions.rs` hook: delivers one take's final text. Same contract as
/// Handy's `utils::paste`: `Err` when the text was not pasted (here: it went
/// to the card; it is in History either way).
pub fn deliver(text: String, app: AppHandle) -> Result<(), String> {
    deliver_as(text, app, Kind::Take).0
}

/// Delivers `text` as `kind`; returns the hook result and the paste outcome
/// (`None` for a delivery cancelled before the paste).
pub fn deliver_as(
    text: String,
    app: AppHandle,
    kind: Kind,
) -> (Result<(), String>, Option<PasteOutcome>) {
    // S-11: the text stages after the LLM (R5 §4.10 stages 9 and 12). The
    // last transcript already went through them.
    let text = match kind {
        Kind::Again => text,
        Kind::Take | Kind::Undo { .. } => crate::fork::text::finish(text),
    };
    #[cfg(target_os = "macos")]
    {
        let settings = crate::settings::get_settings(&app);
        if settings.paste_method == crate::settings::PasteMethod::CtrlV {
            return mac::deliver(text, app, &settings, kind);
        }
    }
    // S-05: Handy's other paste methods get the same target check.
    let take = next_take();
    if let Some(reason) = route(take, kind) {
        let delivery = Delivery::Card { reason };
        publish_outcome(&app, take, delivery, &text, kind);
        return (hook_result(delivery), delivery.paste_outcome());
    }
    // Handy's own paste: it reports only success or failure.
    let result = crate::utils::paste(text.clone(), app.clone());
    let outcome = match result {
        Ok(()) => PasteOutcome::Unverified,
        Err(_) => PasteOutcome::Card,
    };
    if kind.is_take() {
        crate::fork::db::entry_meta::on_paste_outcome(&app, &text, Some(outcome));
        crate::fork::recovery::on_take_delivered();
    }
    (result, Some(outcome))
}

/// The disk filled up during a recording (`fork::db::disk`): stop it like a
/// sleep would, and send its text to the card.
pub fn on_disk_full<R: Runtime>(app: &AppHandle<R>) {
    let now = Instant::now();
    on_signal(app, Signal::Begin(Interruption::DiskFull), now);
    on_signal(app, Signal::End(Interruption::DiskFull), now);
}

/// Registers the session observers and follows Handy's recording phases.
/// Called from `fork::on_ready` (main thread).
pub fn on_ready<R: Runtime>(app: &AppHandle<R>) {
    // Handy's coordinator emits `RecordingStateEvent` (`recording-state-event`)
    // with `{"phase": "started" | "locked" | "stopped" | "cancelled"}`.
    app.listen_any("recording-state-event", |event| {
        on_recording_phase(event.payload(), Instant::now());
        on_target_phase(event.payload());
    });
    #[cfg(all(target_os = "macos", not(test)))]
    {
        let app = app.clone();
        session::mac::observe(move |s| on_signal(&app, s, Instant::now()));
    }
}

fn on_recording_phase(payload: &str, at: Instant) {
    let phase = phase_of(payload);
    let mut s = lock(&SESSION);
    match phase.as_deref() {
        Some("started") => s.take_started(at),
        Some("stopped") => s.take_stopped(at),
        Some("cancelled") => s.take_cancelled(),
        _ => {}
    }
}

fn phase_of(payload: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .and_then(|v| v.get("phase")?.as_str().map(str::to_owned))
}

/// S-05: the take's target captures follow the same phases.
fn on_target_phase(payload: &str) {
    match phase_of(payload).as_deref() {
        Some("started") => target::on_started(),
        Some("stopped") => target::on_stopped(),
        Some("cancelled") => target::on_cancelled(),
        _ => {}
    }
}

/// S-05: where this take may go ([`target::plan`]). Reads the focused
/// element once (≤ [`crate::fork::ax::READ_BUDGET`], else paste anyway) and
/// logs the decision: roles and verdict only, never text.
fn route_take(take: u32) -> Option<CardReason> {
    let pending = target::take_pending();
    let captured = pending.map(|(t, _)| t.id);
    let focus = crate::fork::ax::focus_now(captured, crate::fork::ax::READ_BUDGET);
    let route = target::plan(pending.is_some(), focus.as_ref());
    if let Some((_, start)) = pending {
        target::done(start);
    }
    log::info!(
        "insert: take {take} target {}",
        describe_target(focus.as_ref(), pending.map(|(t, _)| t), route)
    );
    match route {
        target::Route::Card(reason) => Some(reason),
        target::Route::Paste { .. } => None,
    }
}

/// [`route_take`] for every kind. The recovery kinds never touch the take
/// waiting for delivery: Undo compares with its own capture (taken at the
/// cancel), paste-last with nothing (it goes wherever focus is now).
fn route(take: u32, kind: Kind) -> Option<CardReason> {
    let captured = match kind {
        Kind::Take => return route_take(take),
        Kind::Undo { target } => target,
        Kind::Again => None,
    };
    let focus = crate::fork::ax::focus_now(captured, crate::fork::ax::READ_BUDGET);
    let route = target::plan(captured.is_some(), focus.as_ref());
    let as_target = captured.map(|id| target::Target {
        id,
        auto_stop: false,
    });
    log::info!(
        "insert: take {take} ({kind:?}) target {}",
        describe_target(focus.as_ref(), as_target, route)
    );
    match route {
        target::Route::Card(reason) => Some(reason),
        target::Route::Paste { .. } => None,
    }
}

/// A take that will not be pasted for another reason (cancelled, session
/// interrupted) still releases its target.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn discard_target() {
    if let Some((_, start)) = target::take_pending() {
        target::done(start);
    }
}

fn describe_target(
    focus: Option<&crate::fork::ax::Focus>,
    t: Option<target::Target>,
    route: target::Route,
) -> String {
    let route = match route {
        target::Route::Paste { fail_open: false } => "paste".to_string(),
        target::Route::Paste { fail_open: true } => "paste (fail-open)".to_string(),
        target::Route::Card(r) => format!("card ({})", r.as_str()),
    };
    let Some(f) = focus else {
        return format!("{route}: AX did not answer");
    };
    serde_json::json!({
        "route": route,
        "app": f.app,
        "family": f.family,
        "role": f.facts.role,
        "subrole": f.facts.subrole,
        "window": f.facts.has_focused_window,
        "secure_input": f.facts.secure_input,
        "verdict": f.verdict,
        "same_as_key_up": f.same_as_captured,
        "auto_stop": t.map(|t| t.auto_stop),
        "ax_ms": f.ms,
    })
    .to_string()
}

#[cfg_attr(any(test, not(target_os = "macos")), allow(dead_code))]
fn on_signal<R: Runtime>(app: &AppHandle<R>, s: Signal, at: Instant) {
    log::info!("insert: session {s:?}");
    let recording = {
        let mut log = lock(&SESSION);
        log.signal(s, at);
        log.recording()
    };
    if let (Signal::Begin(i), true) = (s, recording) {
        stop_recording(app, i);
    }
}

/// Ends the recording in progress so its audio is kept and transcribed; its
/// text then goes to the card ([`SessionLog::verdict`]). Uses Handy's
/// external toggle (what SIGUSR2 and `--toggle-transcription` send), which
/// stops a recording of the `transcribe` binding and is ignored by any
/// other; S-01's `TranscriptionCoordinator::external_command(Stop)` replaces
/// it once it lands.
#[cfg_attr(any(test, not(target_os = "macos")), allow(dead_code))]
fn stop_recording<R: Runtime>(app: &AppHandle<R>, why: Interruption) {
    let recording = app
        .try_state::<std::sync::Arc<crate::managers::audio::AudioRecordingManager>>()
        .is_some_and(|rm| rm.is_recording());
    if !recording {
        return;
    }
    if let Some(c) = app.try_state::<crate::TranscriptionCoordinator>() {
        log::info!("insert: stopping the recording ({why:?})");
        c.send_external_input("transcribe", "fork-session");
    }
}

/// Why the take being delivered must not be pasted, if any.
fn interruption() -> Option<CardReason> {
    lock(&SESSION).verdict().map(Interruption::card_reason)
}

fn next_take() -> u32 {
    TAKE_SEQ.fetch_add(1, Ordering::Relaxed) + 1
}

/// Logs the outcome (no transcript text), remembers a card, and tells the
/// windows.
fn publish_outcome<R: Runtime>(
    app: &AppHandle<R>,
    take: u32,
    delivery: Delivery,
    text: &str,
    kind: Kind,
) {
    if kind.is_take() {
        crate::fork::db::entry_meta::on_paste_outcome(app, text, delivery.paste_outcome());
        if delivery != Delivery::Cancelled {
            // S-06: a take that reached its delivery is no longer undoable.
            crate::fork::recovery::on_take_delivered();
        }
    }
    let event = outcome_event(take, delivery, text);
    if let Delivery::Card { reason } = delivery {
        if text.trim().is_empty() {
            // Nothing to copy: an empty or formatting-only transcript.
            crate::fork::pill::show_message(crate::fork::pill::model::PillMessage::NoSpeech);
        } else {
            *lock(&LAST_CARD) = Some(CardText {
                take,
                reason,
                text: text.to_string(),
            });
            crate::fork::pill::show_insert_card(card_reason(reason), text.to_string());
        }
    }
    if let Err(e) = tauri_specta::Event::emit(&event, app) {
        log::warn!("insert: could not emit the outcome: {e}");
    }
}

fn outcome_event(take: u32, delivery: Delivery, text: &str) -> ForkInsertOutcome {
    ForkInsertOutcome {
        take,
        outcome: delivery.paste_outcome().map(|o| match o {
            PasteOutcome::Confirmed => ForkPasteOutcome::Confirmed,
            PasteOutcome::Unverified => ForkPasteOutcome::Unverified,
            PasteOutcome::Card => ForkPasteOutcome::Card,
            PasteOutcome::Refused => ForkPasteOutcome::Refused,
            PasteOutcome::Queued => ForkPasteOutcome::Queued,
        }),
        card_reason: match delivery {
            Delivery::Card { reason } => Some(card_reason(reason)),
            _ => None,
        },
        // Only a card carries the text: everything else is in the field. A
        // password field's text is never broadcast.
        text: matches!(delivery, Delivery::Card { reason } if reason != CardReason::SecureField)
            .then(|| text.to_string()),
    }
}

/// The wire name of a card reason.
pub fn card_reason(reason: CardReason) -> ForkCardReason {
    match reason {
        CardReason::Sleep => ForkCardReason::Sleep,
        CardReason::ScreenLocked => ForkCardReason::ScreenLocked,
        CardReason::SessionInactive => ForkCardReason::SessionInactive,
        CardReason::PasteFailed => ForkCardReason::PasteFailed,
        CardReason::DiskFull => ForkCardReason::DiskFull,
        CardReason::NoTextField => ForkCardReason::NoTextField,
        CardReason::FocusMoved => ForkCardReason::FocusMoved,
        CardReason::SecureField => ForkCardReason::SecureField,
    }
}

/// The hook's return value for a delivery.
fn hook_result(delivery: Delivery) -> Result<(), String> {
    match delivery {
        Delivery::Pasted { .. } | Delivery::Cancelled => Ok(()),
        Delivery::Card { reason } => Err(format!(
            "not pasted ({}); the text is on the card and in History",
            reason.as_str()
        )),
    }
}

#[cfg(test)]
mod tests;
