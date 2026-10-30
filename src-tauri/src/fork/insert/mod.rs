// SPDX-License-Identifier: GPL-3.0-or-later
//! Delivery of a take's text (R5 S-04; S-05, S-11 and S-17 add their stages
//! in front of the paste later).
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
//!
//! Every take gets a `paste_outcome` ([`deliver::PasteOutcome`]), logged
//! here and sent as [`ForkInsertOutcome`](super::events::ForkInsertOutcome)
//! (S-05 renders the card from it; S-09 stores it per take).

pub mod deliver;
pub mod session;
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

/// The `actions.rs` hook: delivers one take's final text. Same contract as
/// Handy's `utils::paste`: `Err` when the text was not pasted (here: it went
/// to the card; it is in History either way).
pub fn deliver(text: String, app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let settings = crate::settings::get_settings(&app);
        if settings.paste_method == crate::settings::PasteMethod::CtrlV {
            return mac::deliver(text, app, &settings);
        }
    }
    crate::utils::paste(text, app)
}

/// Registers the session observers and follows Handy's recording phases.
/// Called from `fork::on_ready` (main thread).
pub fn on_ready<R: Runtime>(app: &AppHandle<R>) {
    // Handy's coordinator emits `RecordingStateEvent` (`recording-state-event`)
    // with `{"phase": "started" | "locked" | "stopped" | "cancelled"}`.
    app.listen_any("recording-state-event", |event| {
        on_recording_phase(event.payload(), Instant::now());
    });
    #[cfg(all(target_os = "macos", not(test)))]
    {
        let app = app.clone();
        session::mac::observe(move |s| on_signal(&app, s, Instant::now()));
    }
}

fn on_recording_phase(payload: &str, at: Instant) {
    let phase = serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .and_then(|v| v.get("phase")?.as_str().map(str::to_owned));
    let mut s = lock(&SESSION);
    match phase.as_deref() {
        Some("started") => s.take_started(at),
        Some("stopped") => s.take_stopped(at),
        Some("cancelled") => s.take_cancelled(),
        _ => {}
    }
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
fn publish_outcome<R: Runtime>(app: &AppHandle<R>, take: u32, delivery: Delivery, text: &str) {
    let event = outcome_event(take, delivery, text);
    if let Delivery::Card { reason } = delivery {
        *lock(&LAST_CARD) = Some(CardText {
            take,
            reason,
            text: text.to_string(),
        });
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
            Delivery::Card { reason } => Some(match reason {
                CardReason::Sleep => ForkCardReason::Sleep,
                CardReason::ScreenLocked => ForkCardReason::ScreenLocked,
                CardReason::SessionInactive => ForkCardReason::SessionInactive,
                CardReason::PasteFailed => ForkCardReason::PasteFailed,
            }),
            _ => None,
        },
        // Only a card carries the text: everything else is in the field.
        text: matches!(delivery, Delivery::Card { .. }).then(|| text.to_string()),
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
