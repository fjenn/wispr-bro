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
//! History either way. OT2-A: when there was no field to put it in (no text
//! field at the release, or focus moved on), the text goes on the clipboard
//! instead and the bar says "Copied to clipboard" ([`end_card`]).

pub mod deliver;
pub mod session;
pub mod spacing;
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
    // S-19a: Return after this take, if the cleanup step armed it. Only a
    // fresh take: Undo and paste-last never send (S-06).
    let auto_send = match kind {
        Kind::Take => crate::fork::context::app_rules::take_armed(&text),
        _ => None,
    };
    let text = delivery_text(text, kind);
    // S-29b: the practice screen's take fills its composer, never a paste.
    if kind.is_take() && crate::fork::onboarding::practice::take_delivering(Instant::now()) {
        return deliver_practice(&app, text);
    }
    #[cfg(target_os = "macos")]
    {
        let settings = crate::settings::get_settings(&app);
        if settings.paste_method == crate::settings::PasteMethod::CtrlV {
            return mac::deliver(text, app, &settings, kind, auto_send);
        }
    }
    // Handy's other paste methods report no verified outcome: no Return.
    let _ = auto_send;
    // S-05: Handy's other paste methods get the same target check.
    let take = next_take();
    if let target::Route::Card(reason) = route(take, kind) {
        let delivery = Delivery::Card { reason };
        let end = publish_outcome(&app, take, delivery, &text, kind);
        return (hook_result(delivery, end), delivery.paste_outcome());
    }
    // Handy's own paste: it reports only success or failure.
    let result = crate::utils::paste(with_leading_space(take, &text), app.clone());
    let outcome = match result {
        Ok(()) => PasteOutcome::Unverified,
        Err(_) => PasteOutcome::Card,
    };
    if kind.is_take() {
        crate::fork::db::entry_meta::on_paste_outcome(&app, &text, Some(outcome));
        crate::fork::recovery::on_take_delivered();
        if pasted(Some(outcome)) {
            crate::fork::onboarding::checklist::on_take_pasted(&app);
        }
    }
    (result, Some(outcome))
}

/// The text a delivery inserts, decided before where it goes: a paste, the
/// clipboard ("Copied to clipboard") and the card all carry this same text.
/// S-11: the text stages after the LLM (R5 §4.10 stages 9 and 12, then the
/// snippets); the last transcript already went through them. The cleanup
/// (S-12a) ran before `deliver` and does not depend on the target either.
pub(crate) fn delivery_text(text: String, kind: Kind) -> String {
    match kind {
        Kind::Again => text,
        Kind::Take | Kind::Undo { .. } => crate::fork::text::finish(text),
    }
}

/// The smart leading space ([`spacing`]): one space in front of `text`
/// when the text before the caret ends with a word. Reads the field only
/// when the text could take a space; an unreadable field changes nothing.
/// Logs the decision, never the text.
fn with_leading_space(take: u32, text: &str) -> String {
    if text.chars().next().is_none_or(char::is_whitespace) {
        return text.to_owned();
    }
    let before = crate::fork::ax::before_caret(spacing::TAIL_UNITS);
    let out = spacing::lead(text.to_owned(), before.as_deref());
    log::debug!(
        "insert: take {take} leading space {}",
        match before {
            None => "unknown (field not read)",
            Some(_) if out.len() != text.len() => "added",
            Some(_) => "not needed",
        }
    );
    out
}

/// The text reached a field (S-30 counts it as a real use).
fn pasted(outcome: Option<PasteOutcome>) -> bool {
    matches!(
        outcome,
        Some(PasteOutcome::Confirmed | PasteOutcome::Unverified)
    )
}

/// A practice take (S-29b): the text goes to the onboarding composer. It is
/// in History (source `practice`) and counts as arrived.
fn deliver_practice(app: &AppHandle, text: String) -> (Result<(), String>, Option<PasteOutcome>) {
    let take = next_take();
    discard_target();
    crate::fork::db::entry_meta::on_paste_outcome(app, &text, Some(PasteOutcome::Confirmed));
    crate::fork::recovery::on_take_delivered();
    log::info!("insert: take {take} went to the practice screen");
    crate::fork::onboarding::practice::emit(
        app,
        crate::fork::onboarding::practice::ForkPracticeTake {
            kind: crate::fork::onboarding::practice::ForkPracticeTakeKind::Text,
            text: Some(text),
            late: false,
        },
    );
    (Ok(()), Some(PasteOutcome::Confirmed))
}

/// S-21: replaces a selection (or a whole field) with one clipboard paste.
/// Shares the dictation line and slot, so a transform cannot interleave its
/// transcript or restore with an ordinary take's.
pub fn deliver_transform<R: Runtime>(
    app: &AppHandle<R>,
    text: String,
    target: u64,
    select_all: bool,
    cancelled: &(dyn Fn() -> bool + Send + Sync),
) -> Result<deliver::Report, String> {
    #[cfg(target_os = "macos")]
    return mac::deliver_transform(app.clone(), text, target, select_all, cancelled);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, text, target, select_all, cancelled);
        Err("Transforms replacement is available on macOS".into())
    }
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
fn route_take(take: u32) -> target::Route {
    let pending = target::take_pending();
    let captured = pending.map(|(t, _)| t.id);
    let focus = crate::fork::ax::focus_now(captured, crate::fork::ax::READ_BUDGET);
    let route = target::plan_now(pending.is_some(), focus.as_ref());
    if let Some((_, start)) = pending {
        target::done(start);
    }
    log::info!(
        "insert: take {take} target {}",
        describe_target(focus.as_ref(), pending.map(|(t, _)| t), route)
    );
    route
}

/// [`route_take`] for every kind. The recovery kinds never touch the take
/// waiting for delivery: Undo compares with its own capture (taken at the
/// cancel), paste-last with nothing (it goes wherever focus is now).
fn route(take: u32, kind: Kind) -> target::Route {
    let captured = match kind {
        Kind::Take => return route_take(take),
        Kind::Undo { target } => target,
        Kind::Again => None,
    };
    let focus = crate::fork::ax::focus_now(captured, crate::fork::ax::READ_BUDGET);
    let route = target::plan_now(captured.is_some(), focus.as_ref());
    let as_target = captured.map(|id| target::Target {
        id,
        auto_stop: false,
    });
    log::info!(
        "insert: take {take} ({kind:?}) target {}",
        describe_target(focus.as_ref(), as_target, route)
    );
    route
}

/// A take that will not be pasted for another reason (cancelled, session
/// interrupted) still releases its target.
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

/// How a take that was not pasted reaches the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CardEnd {
    /// An empty or formatting-only transcript: "No speech heard".
    Empty,
    /// OT2-A: on the clipboard (plain, unmarked, never restored) and the bar
    /// says "Copied to clipboard".
    Copied,
    /// The bar's card (reason, text, Copy, ✕).
    Card,
}

/// Decides [`CardEnd`] and, for a reason that copies, writes the clipboard
/// with `write` (falling back to the card when the write fails).
pub(crate) fn end_card(
    reason: CardReason,
    text: &str,
    write: impl FnOnce(&str) -> Result<i64, String>,
) -> CardEnd {
    if text.trim().is_empty() {
        return CardEnd::Empty;
    }
    if !reason.copies_to_clipboard() {
        return CardEnd::Card;
    }
    match write(text) {
        Ok(_) => CardEnd::Copied,
        Err(e) => {
            log::warn!("insert: could not copy the text, showing the card: {e}");
            CardEnd::Card
        }
    }
}

/// The clipboard write of [`CardEnd::Copied`]: plain text, as the card's
/// Copy writes it (§4.7 explicit copy: replaced, not restored).
fn copy_plain(text: &str) -> Result<i64, String> {
    #[cfg(target_os = "macos")]
    return crate::fork::clipboard::nspb::write_text(text, false);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = text;
        Err("copying needs macOS".into())
    }
}

/// Logs the outcome (no transcript text), remembers a card, and tells the
/// windows. Returns how a card ended ([`CardEnd`]), `None` for a paste or
/// a cancel.
fn publish_outcome<R: Runtime>(
    app: &AppHandle<R>,
    take: u32,
    delivery: Delivery,
    text: &str,
    kind: Kind,
) -> Option<CardEnd> {
    if kind.is_take() {
        crate::fork::db::entry_meta::on_paste_outcome(app, text, delivery.paste_outcome());
        if delivery != Delivery::Cancelled {
            // S-06: a take that reached its delivery is no longer undoable.
            crate::fork::recovery::on_take_delivered();
        }
        if pasted(delivery.paste_outcome()) {
            // S-30: a real use ends the spotlight and ticks the checklist.
            crate::fork::onboarding::checklist::on_take_pasted(app);
            // S-37: VoiceOver hears that the text landed.
            crate::fork::pill::accessibility::on_inserted();
        }
    }
    let event = outcome_event(take, delivery, text);
    let mut end = None;
    if let Delivery::Card { reason } = delivery {
        let e = end_card(reason, text, copy_plain);
        end = Some(e);
        match e {
            // Nothing to copy: an empty or formatting-only transcript.
            CardEnd::Empty => {
                crate::fork::pill::show_message(crate::fork::pill::model::PillMessage::NoSpeech)
            }
            CardEnd::Copied => {
                log::info!(
                    "insert: take {take} copied to the clipboard ({})",
                    reason.as_str()
                );
                crate::fork::pill::show_message(
                    crate::fork::pill::model::PillMessage::CopiedToClipboard,
                );
            }
            CardEnd::Card => {
                *lock(&LAST_CARD) = Some(CardText {
                    take,
                    reason,
                    text: text.to_string(),
                });
                crate::fork::pill::show_insert_card(card_reason(reason), text.to_string());
            }
        }
    }
    if let Err(e) = tauri_specta::Event::emit(&event, app) {
        log::warn!("insert: could not emit the outcome: {e}");
    }
    end
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

/// The hook's return value for a delivery. Text copied to the clipboard
/// (OT2-A) is delivered: Handy shows no paste error for it.
fn hook_result(delivery: Delivery, end: Option<CardEnd>) -> Result<(), String> {
    match delivery {
        Delivery::Pasted { .. } | Delivery::Cancelled => Ok(()),
        Delivery::Card { .. } if end == Some(CardEnd::Copied) => Ok(()),
        Delivery::Card { reason } => Err(format!(
            "not pasted ({}); the text is on the card and in History",
            reason.as_str()
        )),
    }
}

#[cfg(test)]
mod tests;
