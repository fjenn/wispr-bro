// SPDX-License-Identifier: GPL-3.0-or-later
//! Recovery (R5 S-06): Undo after a cancel, and the last transcript for
//! paste-last / copy-last.
//!
//! **Undo.** Handy's `cancel_current_operation` (`utils.rs`) calls
//! [`cancel_keeping_audio`] where it called `cancel_recording()`: the
//! recorder hands over what it recorded (`cancel_recording_keeping_samples`,
//! a small upstream-bound method in `managers/audio.rs`). A take cancelled
//! while transcribing reuses the WAV Handy wrote before transcribing
//! (R5 §5.4). The take is kept for [`UNDO_WINDOW_MS`] while the bar shows
//! "Cancelled · Undo", then dropped (a WAV no History row points at is
//! deleted). Undo (the bar's button, or the `undo_cancel` hotkey) saves the
//! take as a History row, has Handy's job queue transcribe it (the same
//! path as a retry, so the text pipeline and cleanup run), and delivers the
//! text into the element that had focus at the cancel if it still has
//! focus, otherwise onto the card (`fork::insert`, kind `Undo`).
//!
//! Undo is offered only for a cancel the user asked for (Esc, ✕, a third
//! quick press, `--cancel`) with at least half a second of speech
//! ([`undo::worth_keeping`]). A tap with no second press, a chord dropped
//! by DEC-54 and a reset stay silent ([`cancel_is_silent`]). A new take, ✕
//! or Esc on the offer drop it at once. Plain Handy builds (no fork engine)
//! discard the audio as Handy does.
//!
//! **Last transcript.** [`last`]: the newest dictation with text, as History
//! shows it. The four hotkey handlers live in `fork::shortcut::actions` and
//! are registered by [`on_ready`].

pub mod last;
pub mod undo;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Listener, Manager};

use crate::fork::insert::{self, Kind};
use crate::fork::pill::model::PillMessage;
use crate::fork::settings::hotkey_actions;
use crate::managers::audio::AudioRecordingManager;
use crate::managers::history::HistoryManager;
pub use undo::UNDO_WINDOW_MS;
use undo::{Audio, Kept, Ms, Plan, Window};

/// A silent-cancel note older than this is stale (the cancel it announced
/// never came).
const SILENT_NOTE_MS: Ms = 2_000;
/// A take being transcribed is undoable only this long after it stopped.
const PROCESSING_MAX: Duration = Duration::from_secs(600);
/// How long Undo waits for Handy's WAV of a take cancelled mid-transcription.
const WAV_WAIT: Duration = Duration::from_secs(5);

static WINDOW: Mutex<Window> = Mutex::new(Window::new());
static APP: OnceLock<AppHandle> = OnceLock::new();
/// `now_ms() + 1` of the last [`cancel_is_silent`] (0: none).
static SILENT_AT: AtomicU64 = AtomicU64::new(0);
/// The take between its stop (WAV named) and its end (delivered, empty,
/// failed): the one a cancel during transcription belongs to.
static IN_FLIGHT: Mutex<Option<InFlight>> = Mutex::new(None);
/// Undo transcriptions running in Handy's job queue: (entry, target).
static PENDING: Mutex<Vec<(i64, Option<u64>)>> = Mutex::new(Vec::new());

#[derive(Debug)]
struct InFlight {
    file_name: String,
    stopped: Instant,
    entry_id: Option<i64>,
    delivered: bool,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn now_ms() -> Ms {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    EPOCH.get_or_init(Instant::now).elapsed().as_millis() as Ms
}

// ---------------------------------------------------------------------------
// Hooks

/// The `utils.rs` `cancel_current_operation` hook, in place of
/// `audio_manager.cancel_recording()`. Never blocks: the samples move into
/// the window, the focused element is captured on the AX thread.
pub fn cancel_keeping_audio(_app: &AppHandle, rm: &AudioRecordingManager) {
    let samples = rm.cancel_recording_keeping_samples();
    if !crate::fork::is_product_build() {
        return;
    }
    let silent = take_silent_note();
    let kept = match samples {
        Some(s) if silent || !undo::worth_keeping(s.len()) => {
            log::debug!(
                "recovery: cancel without Undo ({} samples, silent {silent})",
                s.len()
            );
            None
        }
        Some(s) => {
            let samples = undo::pad(s);
            // Named now, so the take is bound to its app and day while the
            // ledger still has it (`fork::take_file_name`).
            let file_name = crate::fork::take_file_name(samples.len());
            // That call marks a take in flight; this one never was.
            *lock(&IN_FLIGHT) = None;
            Some(Kept {
                file_name,
                audio: Audio::Samples(samples),
                entry_id: None,
                target: None,
            })
        }
        None if silent => None,
        None => in_flight_take(),
    };
    let Some(mut kept) = kept else {
        return;
    };
    kept.target = Some(insert::target::capture_now());
    keep(kept);
}

/// The hotkey engine is about to cancel a take the user did not cancel (a
/// tap with no second press, a DEC-54 chord, a reset): no Undo for it.
pub fn cancel_is_silent() {
    SILENT_AT.store(now_ms() + 1, Ordering::Release);
}

fn take_silent_note() -> bool {
    take_silent_note_at(now_ms())
}

fn take_silent_note_at(now: Ms) -> bool {
    let at = SILENT_AT.swap(0, Ordering::AcqRel);
    at != 0 && (now + 1).saturating_sub(at) <= SILENT_NOTE_MS
}

/// `fork::take_file_name`: a stopped take's WAV was named.
pub fn note_take_file(file_name: &str) {
    *lock(&IN_FLIGHT) = Some(InFlight {
        file_name: file_name.to_string(),
        stopped: Instant::now(),
        entry_id: None,
        delivered: false,
    });
}

/// `fork::db::entry_meta::on_saved`: Handy saved a History row.
pub fn on_entry_saved(file_name: &str, entry_id: i64) {
    if let Some(t) = lock(&IN_FLIGHT)
        .as_mut()
        .filter(|t| t.file_name == file_name)
    {
        t.entry_id = Some(entry_id);
    }
    lock(&WINDOW).note_saved(file_name, entry_id);
}

/// `fork::insert`: the take reached its delivery (pasted or carded). From
/// then on a cancel has nothing left to undo.
pub fn on_take_delivered() {
    if let Some(t) = lock(&IN_FLIGHT).as_mut() {
        t.delivered = true;
    }
}

/// The take being transcribed, if a cancel can still bring it back.
fn in_flight_take() -> Option<Kept> {
    let t = lock(&IN_FLIGHT);
    let t = t.as_ref()?;
    if t.delivered || t.stopped.elapsed() > PROCESSING_MAX {
        return None;
    }
    Some(Kept {
        file_name: t.file_name.clone(),
        audio: Audio::File,
        entry_id: t.entry_id,
        target: None,
    })
}

// ---------------------------------------------------------------------------
// The window

fn keep(kept: Kept) {
    log::info!(
        "recovery: cancelled take kept for Undo ({}, {})",
        kept.file_name,
        match &kept.audio {
            Audio::Samples(s) => format!("{} ms of audio in memory", s.len() / 16),
            Audio::File => "its WAV".to_string(),
        }
    );
    let (seq, replaced) = lock(&WINDOW).keep(now_ms(), kept);
    if let Some(old) = replaced {
        clean_up(old, "replaced");
    }
    crate::fork::pill::show_undo_offer();
    let spawned = std::thread::Builder::new()
        .name("wb-undo-window".into())
        .spawn(move || loop {
            let Some(deadline) = lock(&WINDOW).deadline() else {
                return;
            };
            let now = now_ms();
            if now < deadline {
                std::thread::sleep(Duration::from_millis(deadline - now));
                continue;
            }
            if let Some(k) = lock(&WINDOW).expire_seq(now_ms(), seq) {
                clean_up(k, "expired");
            }
            return;
        });
    if let Err(e) = spawned {
        log::warn!("recovery: cannot start the undo timer: {e}");
    }
}

/// Drops a kept take: its samples go with it; a WAV no History row points
/// at is deleted.
fn clean_up(kept: Kept, why: &str) {
    log::info!("recovery: cancelled take dropped ({why})");
    let Some(file) = kept.leftover_file() else {
        return;
    };
    let Some(app) = APP.get() else { return };
    let Some(hm) = app.try_state::<Arc<HistoryManager>>() else {
        return;
    };
    let path = hm.recordings_dir().join(file);
    match std::fs::remove_file(&path) {
        Ok(()) => log::info!("recovery: deleted the cancelled take's WAV"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => log::warn!("recovery: could not delete the cancelled take's WAV: {e}"),
    }
}

/// ✕ or Esc on "Cancelled · Undo": the audio goes at once.
pub fn discard(why: &str) {
    let kept = lock(&WINDOW).discard();
    if let Some(k) = kept {
        clean_up(k, why);
    }
}

// ---------------------------------------------------------------------------
// Undo

/// Brings the cancelled take back. Returns `false` when there is nothing to
/// undo (the window closed, or it was already undone).
pub fn undo(origin: &str) -> bool {
    let Some(kept) = lock(&WINDOW).take(now_ms()) else {
        return false;
    };
    let Some(app) = APP.get().cloned() else {
        return false;
    };
    let plan = undo::plan(&kept);
    log::info!("recovery: Undo from the {origin}: {plan:?}");
    crate::fork::pill::undo_started();
    let spawned = std::thread::Builder::new()
        .name("wb-undo".into())
        .spawn(move || run_undo(&app, kept, plan));
    if let Err(e) = spawned {
        log::error!("recovery: cannot start Undo: {e}");
        crate::fork::pill::show_message(PillMessage::TranscriptionFailed);
    }
    true
}

fn run_undo(app: &AppHandle, kept: Kept, plan: Plan) {
    if let Err(e) = try_undo(app, &kept, plan) {
        log::warn!("recovery: Undo failed: {e}");
        crate::fork::pill::show_message(PillMessage::TranscriptionFailed);
    }
}

fn try_undo(app: &AppHandle, kept: &Kept, plan: Plan) -> Result<(), String> {
    let hm = app
        .try_state::<Arc<HistoryManager>>()
        .ok_or("no history manager")?;
    let path = hm.recordings_dir().join(&kept.file_name);
    match plan {
        Plan::Deliver { entry_id } => {
            deliver_entry(app, entry_id, kept.target);
            return Ok(());
        }
        Plan::WriteAndTranscribe => {
            if let Audio::Samples(samples) = &kept.audio {
                crate::audio_toolkit::save_wav_file(&path, samples)
                    .map_err(|e| format!("could not write the WAV: {e}"))?;
                crate::audio_toolkit::verify_wav_file(&path, samples.len())
                    .map_err(|e| format!("the WAV did not verify: {e}"))?;
            }
        }
        Plan::Transcribe => wait_for_wav(&path)?,
    }
    let entry = hm
        .save_entry(
            kept.file_name.clone(),
            String::new(),
            false,
            None,
            None,
            None,
        )
        .map_err(|e| format!("could not save the History row: {e}"))?;
    let jobs = app
        .try_state::<Arc<crate::managers::transcription_jobs::TranscriptionJobManager>>()
        .ok_or("no job queue")?;
    lock(&PENDING).push((entry.id, kept.target));
    log::info!(
        "recovery: transcribing the cancelled take (entry {})",
        entry.id
    );
    jobs.retry(entry.id);
    Ok(())
}

/// Handy writes a take's WAV beside its transcription; a cancel during
/// transcription may come before the write ended.
fn wait_for_wav(path: &std::path::Path) -> Result<(), String> {
    let start = Instant::now();
    let mut last_len = None;
    while start.elapsed() < WAV_WAIT {
        let len = std::fs::metadata(path).ok().map(|m| m.len());
        if len.is_some() && len == last_len && crate::audio_toolkit::read_wav_samples(path).is_ok()
        {
            return Ok(());
        }
        last_len = len;
        std::thread::sleep(Duration::from_millis(50));
    }
    Err("the take's WAV never appeared".into())
}

/// Handy's job queue finished (or failed) an entry.
fn on_job_event(payload: &str) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) else {
        return;
    };
    let Some(entry_id) = v.get("entry_id").and_then(|v| v.as_i64()) else {
        return;
    };
    let status = v.get("status");
    let state = status
        .and_then(|s| s.get("state"))
        .and_then(|s| s.as_str())
        .unwrap_or_default();
    if !matches!(state, "finished" | "failed") {
        return;
    }
    let target = {
        let mut pending = lock(&PENDING);
        let Some(at) = pending.iter().position(|(id, _)| *id == entry_id) else {
            return;
        };
        pending.remove(at).1
    };
    let Some(app) = APP.get().cloned() else {
        return;
    };
    if state == "failed" {
        let error = status
            .and_then(|s| s.get("error"))
            .and_then(|e| e.as_str())
            .unwrap_or_default();
        log::info!("recovery: the cancelled take could not be transcribed (entry {entry_id})");
        crate::fork::pill::show_message(if error.contains("no speech") {
            PillMessage::NoSpeech
        } else {
            PillMessage::TranscriptionFailed
        });
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("wb-undo-deliver".into())
        .spawn(move || deliver_entry(&app, entry_id, target));
    if let Err(e) = spawned {
        log::error!("recovery: cannot deliver the undone take: {e}");
    }
}

/// Delivers History row `entry_id` as an Undo and records its outcome.
fn deliver_entry(app: &AppHandle, entry_id: i64, target: Option<u64>) {
    let Some(hm) = app.try_state::<Arc<HistoryManager>>() else {
        return;
    };
    let entry = match tauri::async_runtime::block_on(hm.get_entry_by_id(entry_id)) {
        Ok(Some(e)) => e,
        Ok(None) => {
            log::warn!("recovery: entry {entry_id} is gone");
            return;
        }
        Err(e) => {
            log::warn!("recovery: cannot read entry {entry_id}: {e}");
            crate::fork::pill::show_message(PillMessage::TranscriptionFailed);
            return;
        }
    };
    let text = entry
        .post_processed_text
        .filter(|t| !t.trim().is_empty())
        .unwrap_or(entry.transcription_text);
    if text.trim().is_empty() {
        log::info!("recovery: the cancelled take had no speech (entry {entry_id})");
        crate::fork::pill::show_message(PillMessage::NoSpeech);
        return;
    }
    let (result, outcome) = insert::deliver_as(text, app.clone(), Kind::Undo { target });
    if let (Some(o), Some(db)) = (outcome, app.try_state::<crate::fork::db::ForkDb>()) {
        let _ = db.with_conn(|c| crate::fork::db::entry_meta::set_paste_outcome(c, entry_id, o));
    }
    log::info!(
        "recovery: Undo delivered entry {entry_id}: {}",
        outcome.map_or("cancelled", |o| o.as_str())
    );
    if result.is_ok() {
        crate::fork::pill::undo_finished();
    }
}

// ---------------------------------------------------------------------------
// Start, and the bar's short messages

/// From `fork::on_ready` (product builds, before the hotkey engine starts
/// so the bindings find their handlers): remembers the app, follows the
/// take's phases and Handy's job queue, registers the four actions.
pub fn on_ready(app: &AppHandle) {
    if APP.set(app.clone()).is_err() {
        return;
    }
    app.listen_any("recording-state-event", |event| {
        let started = serde_json::from_str::<serde_json::Value>(event.payload())
            .ok()
            .and_then(|v| v.get("phase")?.as_str().map(|p| p == "started"))
            .unwrap_or(false);
        if started {
            // A new take: the cancelled one is not coming back, and the take
            // in flight is this one from its stop on.
            *lock(&IN_FLIGHT) = None;
            discard("new take");
        }
    });
    app.listen_any("transcription-complete-event", |_| {
        *lock(&IN_FLIGHT) = None;
    });
    app.listen_any("transcription-job", |event| on_job_event(event.payload()));

    let registry = crate::fork::shortcut::registry();
    let handlers: [(&str, crate::fork::shortcut::actions::Handler); 4] = [
        (hotkey_actions::PASTE_LAST, {
            let app = app.clone();
            Arc::new(move || crate::fork::shortcut::actions::paste_last::run(&app))
        }),
        (hotkey_actions::COPY_LAST, {
            let app = app.clone();
            Arc::new(move || crate::fork::shortcut::actions::copy_last::run(&app))
        }),
        (
            hotkey_actions::PRESS_ENTER,
            Arc::new(crate::fork::shortcut::actions::press_enter::run),
        ),
        (
            hotkey_actions::UNDO_CANCEL,
            Arc::new(crate::fork::shortcut::actions::undo_cancel::run),
        ),
    ];
    for (id, handler) in handlers {
        if let Err(e) = registry.register(id, handler) {
            log::error!("recovery: cannot register {id}: {e}");
        }
    }
}

/// Paste-last / copy-last found nothing.
pub fn show_nothing_yet() {
    crate::fork::pill::show_message(PillMessage::NoTranscript);
}

/// Copy-last copied.
pub fn show_copied() {
    crate::fork::pill::show_message(PillMessage::CopiedLast);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_silent_note_covers_only_the_next_cancel() {
        SILENT_AT.store(0, Ordering::SeqCst);
        assert!(!take_silent_note_at(10_000));
        SILENT_AT.store(10_001, Ordering::SeqCst); // noted at 10 000
        assert!(take_silent_note_at(10_000 + SILENT_NOTE_MS));
        assert!(!take_silent_note_at(10_000 + SILENT_NOTE_MS), "consumed");
        // A stale note does not silence a later cancel.
        SILENT_AT.store(10_001, Ordering::SeqCst);
        assert!(!take_silent_note_at(10_001 + SILENT_NOTE_MS));
        // The live clock.
        cancel_is_silent();
        assert!(take_silent_note());
    }

    #[test]
    fn the_in_flight_take_is_undoable_until_it_is_delivered() {
        *lock(&IN_FLIGHT) = None;
        assert!(in_flight_take().is_none());
        note_take_file("wispr-bro-5.wav");
        let k = in_flight_take().expect("transcribing");
        assert_eq!(
            (k.file_name.as_str(), &k.audio, k.entry_id),
            ("wispr-bro-5.wav", &Audio::File, None)
        );
        // Handy saved its row before the cancel landed: Undo delivers it.
        on_entry_saved("other.wav", 1);
        assert_eq!(in_flight_take().unwrap().entry_id, None);
        on_entry_saved("wispr-bro-5.wav", 12);
        assert_eq!(
            undo::plan(&in_flight_take().unwrap()),
            Plan::Deliver { entry_id: 12 }
        );
        on_take_delivered();
        assert!(in_flight_take().is_none());
        *lock(&IN_FLIGHT) = None;
    }

    #[test]
    fn job_events_for_other_entries_are_ignored() {
        lock(&PENDING).clear();
        lock(&PENDING).push((41, Some(3)));
        on_job_event(r#"{"entry_id":40,"status":{"state":"finished"}}"#);
        on_job_event(r#"{"entry_id":41,"status":{"state":"running","percent":3}}"#);
        assert_eq!(lock(&PENDING).len(), 1);
        // Finished: taken out (no app in tests, so nothing is delivered).
        on_job_event(r#"{"entry_id":41,"status":{"state":"finished"}}"#);
        assert!(lock(&PENDING).is_empty());
    }
}
