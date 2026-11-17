// SPDX-License-Identifier: GPL-3.0-or-later
//
//! The take API the Swift session calls (`ios-native/WBCore/wb_core.h`).
//!
//! On the Mac a take is: the recorder's delivery thread runs each 16 kHz
//! frame through the capture sink (voice detector), and at key-up
//! `actions.rs` transcribes, cleans, saves History and delivers. On the
//! iPhone the Swift session owns the microphone and the keyboard extension
//! inserts the text, so this module stands where the recorder and the paste
//! stand, and calls the same functions in between:
//!
//! - `wb_take_start` starts a take thread that owns a [`CaptureSink`] (the
//!   Mac's, with the same detector and smoothing);
//! - `wb_take_feed` hands it the audio (never blocks the audio thread) and
//!   returns the Flow Bar's speech level ([`SpeechMeter`]);
//! - `wb_take_finish` runs what `TranscribeAction::stop` runs after the
//!   recording stopped, with [`crate::fork::insert::deliver_to_keyboard`] in
//!   place of the paste, and returns the final text.
//!
//! Swift and Tauri start in the same process in no fixed order. The only
//! shared state is the `AppHandle`, published by [`on_ready`] once Handy's
//! managers and the fork are up. A take that starts earlier just buffers its
//! audio; `wb_take_finish` waits for the handle (bounded).

use std::collections::HashMap;
use std::ffi::{c_char, CStr, CString};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use handy_recorder::{AudioChunk, Sink};
use tauri::{AppHandle, Emitter, Manager};
use tauri_specta::Event;

use crate::actions::{emit_transcription_complete, empty_outcome, TranscriptionOutcome};
use crate::audio_toolkit::constants::WHISPER_SAMPLE_RATE;
use crate::audio_toolkit::{CaptureSink, CaptureVad, VadPolicy};
use crate::fork::audio::speech::SpeechMeter;
use crate::managers::history::HistoryManager;
use crate::managers::transcription::{Transcription, TranscriptionManager};
use crate::transcription_coordinator::{RecordingPhase, RecordingStateEvent};
use crate::tray::{set_tray_state, TrayIconState};

/// How long a take waits for the app to finish starting.
const APP_WAIT: Duration = Duration::from_secs(30);
/// Frame size while the app (and so the voice detector) is not up yet.
const RAW_FRAME: usize = 512;

static APP: OnceLock<AppHandle> = OnceLock::new();
static APP_READY: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

/// The directories Swift passed to `wb_core_init`.
#[derive(Clone, Debug, Default)]
pub struct Dirs {
    pub app_support: Option<PathBuf>,
    pub app_group: Option<PathBuf>,
}

static DIRS: Mutex<Option<Dirs>> = Mutex::new(None);

pub fn dirs() -> Dirs {
    lock(&DIRS).clone().unwrap_or_default()
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// From `fork::on_ready`, after Handy's managers are in state and the speech
/// model setup has started.
pub fn on_ready(app: &AppHandle) {
    if APP.set(app.clone()).is_err() {
        return;
    }
    *lock(&APP_READY.0) = true;
    APP_READY.1.notify_all();
    log::info!("ios core: ready for takes");
}

fn app_now() -> Option<&'static AppHandle> {
    APP.get()
}

fn app_within(timeout: Duration) -> Option<&'static AppHandle> {
    if let Some(app) = APP.get() {
        return Some(app);
    }
    let ready = lock(&APP_READY.0);
    let _ready = APP_READY
        .1
        .wait_timeout_while(ready, timeout, |ready| !*ready)
        .unwrap_or_else(|e| e.into_inner());
    APP.get()
}

// ------------------------------------------------------------------ takes --

/// What a take's thread hands back.
struct Captured {
    /// The samples that passed the voice detector.
    samples: Vec<f32>,
    /// Only digital silence arrived (a muted microphone).
    input_silent: bool,
}

struct Take {
    audio: mpsc::Sender<Vec<f32>>,
    worker: std::thread::JoinHandle<Captured>,
    meter: SpeechMeter,
    cancelled: Arc<AtomicBool>,
}

static TAKES: Mutex<Option<HashMap<u64, Take>>> = Mutex::new(None);
/// Takes inside `wb_take_finish`, so a cancel can reach them.
static FINISHING: Mutex<Option<HashMap<u64, Arc<AtomicBool>>>> = Mutex::new(None);
/// The voice detector, kept between takes (as the Mac's recorder does).
static VAD: Mutex<Option<CaptureVad>> = Mutex::new(None);

/// The Mac's capture sink for one take: the same detector, policy and
/// smoothing `AudioRecordingManager` gives a recording.
fn new_sink(app: &AppHandle) -> Option<(CaptureSink, usize)> {
    let settings = crate::settings::get_settings(app);
    let vad = match lock(&VAD).take() {
        Some(vad) => vad,
        None => match crate::managers::audio::create_capture_vad(settings.vad_backend, app) {
            Ok(vad) => vad,
            Err(error) => {
                log::warn!("ios core: no voice detector, keeping all audio: {error}");
                return None;
            }
        },
    };
    let policy = if settings.vad_enabled {
        VadPolicy::Offline
    } else {
        VadPolicy::Disabled
    };
    // The app shows no spectrum (Swift draws the level `wb_take_feed`
    // returns) and takes are transcribed as a batch.
    let (ready, _) = mpsc::channel();
    let frame = vad.frame_samples();
    let sink = CaptureSink::new(vad, policy, Arc::new(|_| {}), Arc::new(|_| {}), ready);
    Some((sink, frame))
}

/// The take thread: frames the audio as the recorder does (whole detector
/// frames, the last one zero-padded) and runs it through the sink.
fn capture(audio: mpsc::Receiver<Vec<f32>>) -> Captured {
    // Audio queues in the channel while the app is still starting.
    let (mut sink, frame) = match app_within(APP_WAIT).and_then(new_sink) {
        Some((sink, frame)) => (Some(sink), frame),
        None => (None, RAW_FRAME),
    };
    let mut raw: Vec<f32> = Vec::new();
    let mut heard_sound = false;
    let mut pending: Vec<f32> = Vec::with_capacity(frame * 2);
    let mut push = |samples: &[f32], valid: usize, sink: &mut Option<CaptureSink>| match sink {
        Some(sink) => sink.process_chunk(AudioChunk {
            samples,
            sample_rate: WHISPER_SAMPLE_RATE,
            channels: 1,
            valid_frames: valid,
        }),
        None => {
            heard_sound |= samples[..valid].iter().any(|&s| s != 0.0);
            raw.extend_from_slice(&samples[..valid]);
        }
    };
    for chunk in audio {
        pending.extend_from_slice(&chunk);
        let mut at = 0;
        while pending.len() - at >= frame {
            push(&pending[at..at + frame], frame, &mut sink);
            at += frame;
        }
        pending.drain(..at);
    }
    if !pending.is_empty() {
        let valid = pending.len();
        pending.resize(frame, 0.0);
        push(&pending, valid, &mut sink);
    }
    match sink {
        Some(sink) => {
            let done = sink.finish();
            *lock(&VAD) = Some(done.vad);
            Captured {
                samples: done.samples,
                input_silent: done.input_silent,
            }
        }
        None => Captured {
            input_silent: !raw.is_empty() && !heard_sound,
            samples: raw,
        },
    }
}

fn take_start(id: u64) {
    let (audio, rx) = mpsc::channel();
    let worker = match std::thread::Builder::new()
        .name(format!("wb-take-{id}"))
        .spawn(move || capture(rx))
    {
        Ok(worker) => worker,
        Err(error) => {
            log::error!("ios core: could not start take {id}: {error}");
            return;
        }
    };
    let take = Take {
        audio,
        worker,
        meter: SpeechMeter::new(WHISPER_SAMPLE_RATE),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    if let Some(old) = lock(&TAKES)
        .get_or_insert_with(HashMap::new)
        .insert(id, take)
    {
        old.cancelled.store(true, Ordering::Release);
    }
    log::info!("ios core: take {id} started");
    // The Mac's record-start steps (`TranscribeAction::start`), without the
    // microphone: load the model, note the context, mark the take.
    if let Some(app) = app_now() {
        if let Some(tm) = app.try_state::<Arc<TranscriptionManager>>() {
            crate::fork::models::setup::initiate_model_load(&tm);
        }
        crate::fork::on_record_start(app);
        set_tray_state(app, TrayIconState::Recording);
        emit_phase(app, RecordingPhase::Started);
    }
}

fn take_feed(id: u64, samples: &[f32]) -> f32 {
    let mut takes = lock(&TAKES);
    let Some(take) = takes.as_mut().and_then(|takes| takes.get_mut(&id)) else {
        return 0.0;
    };
    let level = take.meter.push(samples, 1).level;
    let _ = take.audio.send(samples.to_vec());
    level.clamp(0.0, 1.0)
}

fn take_cancel(id: u64) {
    let take = lock(&TAKES).as_mut().and_then(|takes| takes.remove(&id));
    let finishing = lock(&FINISHING)
        .as_ref()
        .and_then(|takes| takes.get(&id).cloned());
    if take.is_none() && finishing.is_none() {
        return;
    }
    if let Some(flag) = &finishing {
        flag.store(true, Ordering::Release);
    }
    if let Some(take) = take {
        take.cancelled.store(true, Ordering::Release);
        // Dropping the sender ends the take thread; its audio is discarded.
        drop(take.audio);
        if let Some(app) = app_now() {
            set_tray_state(app, TrayIconState::Idle);
            emit_phase(app, RecordingPhase::Cancelled);
        }
    }
    log::info!("ios core: take {id} cancelled");
}

fn emit_phase(app: &AppHandle, phase: RecordingPhase) {
    if let Err(error) = (RecordingStateEvent { phase }).emit(app) {
        log::debug!("ios core: recording phase event: {error}");
    }
}

/// Why a take returned no text.
enum Failed {
    Cancelled,
    /// No speech model yet: the recording is saved and transcribed later.
    Queued,
    Error(String),
}

/// Ends the take when the pipeline returns, on every path (the Mac's
/// `FinishGuard` plus the tray state its paste thread resets).
struct FinishGuard<'a>(&'a AppHandle, Arc<TranscriptionManager>);

impl Drop for FinishGuard<'_> {
    fn drop(&mut self) {
        self.1.maybe_unload_immediately("transcription session");
        // A new take may have started while this one was transcribed.
        let recording = lock(&TAKES).as_ref().is_some_and(|t| !t.is_empty());
        if !recording {
            set_tray_state(self.0, TrayIconState::Idle);
        }
        crate::memory::trim_freed_memory();
    }
}

fn take_finish(id: u64) -> Result<String, Failed> {
    let stop_time = Instant::now();
    let Some(take) = lock(&TAKES).as_mut().and_then(|takes| takes.remove(&id)) else {
        return Err(Failed::Error("Unknown take".into()));
    };
    let cancelled = Arc::clone(&take.cancelled);
    lock(&FINISHING)
        .get_or_insert_with(HashMap::new)
        .insert(id, Arc::clone(&cancelled));
    let result = finish_pipeline(id, take, &cancelled, stop_time);
    if let Some(takes) = lock(&FINISHING).as_mut() {
        takes.remove(&id);
    }
    result
}

/// `TranscribeAction::stop`'s task, from "recording stopped" on.
fn finish_pipeline(
    id: u64,
    take: Take,
    cancelled: &AtomicBool,
    stop_time: Instant,
) -> Result<String, Failed> {
    let is_cancelled = || cancelled.load(Ordering::Acquire);
    drop(take.audio);
    let captured = take
        .worker
        .join()
        .map_err(|_| Failed::Error("The recording could not be read".into()))?;
    let Some(app) = app_within(APP_WAIT) else {
        return Err(Failed::Error("Wispr Bro is still starting".into()));
    };
    let (Some(tm), Some(hm)) = (
        app.try_state::<Arc<TranscriptionManager>>(),
        app.try_state::<Arc<HistoryManager>>(),
    ) else {
        return Err(Failed::Error("Wispr Bro is still starting".into()));
    };
    let (tm, hm) = (Arc::clone(&tm), Arc::clone(&hm));
    if is_cancelled() {
        return Err(Failed::Cancelled);
    }
    set_tray_state(app, TrayIconState::Transcribing);
    emit_phase(app, RecordingPhase::Stopped);
    let _guard = FinishGuard(app, Arc::clone(&tm));

    let Captured {
        samples,
        input_silent,
    } = captured;
    if samples.is_empty() {
        log::debug!("ios core: take {id} has no speech");
        emit_transcription_complete(app, empty_outcome(input_silent), 0, stop_time);
        return Ok(String::new());
    }

    // Save the WAV while transcribing.
    let sample_count = samples.len();
    let file_name = crate::fork::take_file_name(sample_count);
    let wav_path = hm.recordings_dir().join(&file_name);
    let wav_writer = {
        let wav_path = wav_path.clone();
        let samples = samples.clone();
        std::thread::Builder::new()
            .name("wb-take-wav".into())
            .spawn(move || crate::audio_toolkit::save_wav_file(&wav_path, &samples))
    };
    let transcribing = Instant::now();
    let was_ready = crate::fork::models::setup::is_ready();
    // A take can also be queued while a starter model is ready (its
    // language needs the full model, which is still on its way).
    let queued_before = crate::fork::models::setup::status().queued_takes;
    let transcription = crate::fork::models::setup::transcribe_take(app, &tm, &file_name, samples);
    let wav_saved = match wav_writer.map(|writer| writer.join()) {
        Ok(Ok(Ok(()))) => match crate::audio_toolkit::verify_wav_file(&wav_path, sample_count) {
            Ok(()) => true,
            Err(error) => {
                log::error!("WAV verification failed: {error}");
                false
            }
        },
        Ok(Ok(Err(error))) => {
            log::error!("Failed to save WAV file: {error}");
            false
        }
        _ => {
            log::error!("WAV save thread failed");
            false
        }
    };
    if is_cancelled() {
        return Err(Failed::Cancelled);
    }

    match transcription {
        Ok(Transcription { text, language }) => {
            log::debug!(
                "ios core: take {id} transcribed in {:?}",
                transcribing.elapsed()
            );
            // No LLM runtime on the phone yet: the cleanup step reports
            // "not ready" and returns the deterministic text with the style.
            let processed =
                tauri::async_runtime::block_on(crate::fork::llm::post_process(app, &text, false));
            if is_cancelled() {
                return Err(Failed::Cancelled);
            }
            if wav_saved || crate::fork::keep_take(&wav_path) {
                if let Err(error) = hm.save_entry(
                    file_name,
                    text,
                    false,
                    processed.post_processed_text.clone(),
                    processed.post_process_prompt.clone(),
                    language,
                ) {
                    log::error!("Failed to save history entry: {error}");
                }
            }
            if processed.final_text.is_empty() {
                emit_transcription_complete(
                    app,
                    empty_outcome(input_silent),
                    sample_count,
                    stop_time,
                );
                let queued = crate::fork::models::setup::status().queued_takes > queued_before;
                return if was_ready && !queued {
                    Ok(String::new())
                } else {
                    Err(Failed::Queued)
                };
            }
            let final_text = crate::fork::insert::deliver_to_keyboard(processed.final_text, app);
            emit_transcription_complete(app, TranscriptionOutcome::Typed, sample_count, stop_time);
            Ok(final_text)
        }
        Err(error) => {
            log::error!("Transcription failed: {error}");
            let _ = app.emit("transcription-error", error.to_string());
            // An empty row, so the take can be retried from History.
            if wav_saved {
                if let Err(save_error) =
                    hm.save_entry(file_name, String::new(), false, None, None, None)
                {
                    log::error!("Failed to save failed history entry: {save_error}");
                }
            }
            emit_transcription_complete(app, TranscriptionOutcome::Failed, sample_count, stop_time);
            Err(Failed::Error(error.to_string()))
        }
    }
}

// ----------------------------------------------------------------- status --

/// `wb_core_status_json` and `ios_core_status`.
pub fn status() -> serde_json::Value {
    use crate::fork::models::setup::{self, SetupPhase};
    let Some(app) = app_now() else {
        return serde_json::json!({
            "ready": false,
            "model": "",
            "downloading": false,
            "progress": 0.0,
            "message": "starting",
        });
    };
    let setup = setup::status();
    let ready = setup::is_ready();
    let downloading = matches!(setup.phase, SetupPhase::Downloading | SetupPhase::Verifying);
    let progress = if ready {
        1.0
    } else if setup.total_bytes > 0 {
        (setup.downloaded_bytes as f64 / setup.total_bytes as f64).clamp(0.0, 1.0)
    } else {
        setup.percent.map(|p| f64::from(p) / 100.0).unwrap_or(0.0)
    };
    let message = match (&setup.problem, setup.phase) {
        (Some(problem), _) => serde_json::to_value(problem)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| "problem".into()),
        (None, SetupPhase::Checking) => "checking".into(),
        (None, SetupPhase::Ready) => "ready".into(),
        (None, SetupPhase::Downloading) => "downloading".into(),
        (None, SetupPhase::Verifying) => "verifying".into(),
        (None, SetupPhase::Waiting) => "waiting".into(),
        (None, SetupPhase::Failed) => "failed".into(),
    };
    serde_json::json!({
        "ready": ready,
        "model": crate::settings::get_settings(app).selected_model,
        "downloading": downloading,
        "progress": progress,
        "message": message,
        "queuedTakes": setup.queued_takes,
    })
}

// ------------------------------------------------------------------ C ABI --

fn c_string(value: String) -> *mut c_char {
    CString::new(value.replace('\0', ""))
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

fn c_path(pointer: *const c_char) -> Option<PathBuf> {
    if pointer.is_null() {
        return None;
    }
    // SAFETY: the caller passes a NUL-terminated UTF-8 string (wb_core.h).
    let text = unsafe { CStr::from_ptr(pointer) }.to_string_lossy();
    (!text.is_empty()).then(|| PathBuf::from(text.as_ref()))
}

/// A panic must not unwind into Swift.
fn guarded<T>(fallback: T, body: impl FnOnce() -> T) -> T {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)).unwrap_or(fallback)
}

/// Called once, early at launch. Later calls are ignored.
#[no_mangle]
pub extern "C" fn wb_core_init(app_support_dir: *const c_char, app_group_dir: *const c_char) {
    guarded((), || {
        let mut dirs = lock(&DIRS);
        if dirs.is_some() {
            return;
        }
        *dirs = Some(Dirs {
            app_support: c_path(app_support_dir),
            app_group: c_path(app_group_dir),
        });
    });
}

/// `{"ready":bool,"model":"...","downloading":bool,"progress":0..1,"message":"..."}`
#[no_mangle]
pub extern "C" fn wb_core_status_json() -> *mut c_char {
    guarded(std::ptr::null_mut(), || c_string(status().to_string()))
}

#[no_mangle]
pub extern "C" fn wb_take_start(take_id: u64) {
    guarded((), || take_start(take_id));
}

/// Returns the display level 0..1.
///
/// # Safety
/// `samples_16k_mono` points at `count` floats (or `count` is 0).
#[no_mangle]
pub unsafe extern "C" fn wb_take_feed(
    take_id: u64,
    samples_16k_mono: *const f32,
    count: usize,
) -> f32 {
    if samples_16k_mono.is_null() || count == 0 {
        return 0.0;
    }
    // SAFETY: the caller's contract above.
    let samples = unsafe { std::slice::from_raw_parts(samples_16k_mono, count) };
    guarded(0.0, || take_feed(take_id, samples))
}

/// BLOCKING. `{"ok":bool,"text":"...","error":"..."}`
#[no_mangle]
pub extern "C" fn wb_take_finish(take_id: u64) -> *mut c_char {
    let reply = guarded(
        serde_json::json!({"ok": false, "text": "", "error": "Internal error"}),
        || match take_finish(take_id) {
            Ok(text) => serde_json::json!({"ok": true, "text": text, "error": ""}),
            Err(Failed::Cancelled) => {
                serde_json::json!({"ok": false, "text": "", "error": "cancelled", "cancelled": true})
            }
            Err(Failed::Queued) => serde_json::json!({
                "ok": false,
                "text": "",
                "error": "Still setting up. This dictation is saved and will appear in History.",
                "queued": true,
            }),
            Err(Failed::Error(error)) => {
                serde_json::json!({"ok": false, "text": "", "error": error})
            }
        },
    );
    c_string(reply.to_string())
}

#[no_mangle]
pub extern "C" fn wb_take_cancel(take_id: u64) {
    guarded((), || take_cancel(take_id));
}

/// # Safety
/// `s` came from this library (or is null) and is freed once.
#[no_mangle]
pub unsafe extern "C" fn wb_string_free(s: *mut c_char) {
    if !s.is_null() {
        // SAFETY: the caller's contract above.
        drop(unsafe { CString::from_raw(s) });
    }
}
