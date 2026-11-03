// SPDX-License-Identifier: GPL-3.0-or-later
//! "Setting up": the first speech model and the mic gate (S-07a, DEC-01).
//!
//! At launch ([`start`], product builds) the fork checks whether Handy has a
//! speech model to transcribe with ([`super::readiness`]). If not, it
//! downloads the starter for the user's languages ([`super::starter`])
//! through Handy's downloader, wrapped in the fork's state machine
//! ([`super::download`]): disk check, retry with backoff and `Retry-After`,
//! captive-portal detection, sha256 against the catalog pin. When the file
//! is verified and no take runs, it is selected (Handy's
//! `switch_active_model`) and the takes queued meanwhile are transcribed
//! ([`super::queue`]).
//!
//! Until then a press still records. Handy's files reach this module through
//! three `actions.rs` hooks (R5 §4.2):
//! - the mic gate: `if !tm.is_model_loaded() && !…::can_record_without_model()`;
//! - [`initiate_model_load`] in place of `tm.initiate_model_load()` (no load
//!   attempt, and so no "failed to load" toast, while there is no model);
//! - [`transcribe_take`] in place of `tm.transcribe_recording(samples)`: a
//!   take that stops before a model is ready is queued and comes back as an
//!   empty transcription, so Handy keeps its WAV and History row and pastes
//!   nothing.
//!
//! Progress goes out as `fork-speech-setup` ([`ForkSpeechSetup`]) and to the
//! bar. No model name is in it (R5 S-07a exit criterion); the log has it.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::download::SetupProblem;

/// Where setup is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SetupPhase {
    /// Not decided yet (before launch finished, or not a product build).
    Checking,
    /// A speech model is selected and on disk.
    Ready,
    Downloading,
    /// Checking the downloaded file.
    Verifying,
    /// Between attempts; `problem` says why when the user can act.
    Waiting,
    /// Stopped: "Couldn't download · Retry".
    Failed,
}

/// The `fork-speech-setup` event and `fork_speech_setup_status` reply.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkSpeechSetup {
    pub phase: SetupPhase,
    /// 0-99 while a download runs (100 only once ready).
    pub percent: Option<u8>,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub problem: Option<SetupProblem>,
    /// For `NoSpace`: how much free space the download needs, in MB.
    pub needed_mb: Option<u64>,
    /// Durable takes that wait for a model or are being transcribed.
    pub queued_takes: u32,
}

impl ForkSpeechSetup {
    const fn checking() -> ForkSpeechSetup {
        ForkSpeechSetup {
            phase: SetupPhase::Checking,
            percent: None,
            downloaded_bytes: 0,
            total_bytes: 0,
            problem: None,
            needed_mb: None,
            queued_takes: 0,
        }
    }
}

/// `READY`: not decided (Handy's behaviour), ready, or not ready.
const UNKNOWN: u8 = 0;
const YES: u8 = 1;
const NO: u8 = 2;
static READY: AtomicU8 = AtomicU8::new(UNKNOWN);
/// Empty outcomes of queued takes the bar must not report as "No speech".
static QUEUED_OUTCOMES: AtomicU32 = AtomicU32::new(0);
static STATUS: Mutex<ForkSpeechSetup> = Mutex::new(ForkSpeechSetup::checking());

fn lock_status() -> std::sync::MutexGuard<'static, ForkSpeechSetup> {
    STATUS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Why the setup loop wakes up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kick {
    Start,
    /// The user pressed Retry.
    Retry,
    /// The Mac woke up.
    Wake,
    /// The user's languages changed (Welcome).
    Languages,
    /// A Handy download ended (someone else's, or ours), or the selection
    /// changed.
    ModelEvent,
    /// S-07b: five more warm takes were timed (re-check the speed).
    Speed,
    /// S-07b: "Choose automatically" changed on the Models panel.
    Policy,
}

static KICK: OnceLock<tokio::sync::mpsc::UnboundedSender<Kick>> = OnceLock::new();

pub(super) fn kick(k: Kick) {
    if let Some(tx) = KICK.get() {
        let _ = tx.send(k);
    }
}

/// `fork_speech_setup_status`.
pub fn status() -> ForkSpeechSetup {
    let mut s = lock_status().clone();
    s.queued_takes = WRY_APP
        .get()
        .and_then(|app| app.try_state::<crate::fork::db::ForkDb>())
        .and_then(|db| db.with_conn(|c| super::queue::pending(c)).ok())
        .unwrap_or(0);
    s
}

/// `fork_speech_setup_retry` and the bar's Retry.
pub fn retry() {
    kick(Kick::Retry);
}

/// `fork_speech_setup_refresh`: the languages may have changed.
pub fn refresh() {
    super::upgrade::clear_pause();
    kick(Kick::Languages);
}

/// `fork_model_policy_set_automatic`: "Choose automatically" on the Models
/// panel.
pub fn set_automatic(on: bool) {
    if let Some(app) = WRY_APP.get() {
        super::upgrade::set_automatic(app, on);
    }
    super::upgrade::clear_pause();
    kick(Kick::Policy);
}

/// Whether a speech model is ready (the first model is done). Handy's
/// `model-download-complete` for a later, silent download then says nothing
/// to the user (`fork::notify`).
pub fn is_ready() -> bool {
    READY.load(Ordering::Acquire) == YES
}

// ---------------------------------------------------------------------------
// The `actions.rs` hooks

/// Mic gate: `true` while no speech model is ready, so the take records
/// anyway (it is queued). `false` before the fork decided, so Handy's own
/// check applies (plain Handy builds, headless modes, early start).
pub fn can_record_without_model() -> bool {
    READY.load(Ordering::Acquire) == NO
}

/// `tm.initiate_model_load()` unless there is no model to load yet.
pub fn initiate_model_load(tm: &crate::managers::transcription::TranscriptionManager) {
    if READY.load(Ordering::Acquire) != NO {
        tm.initiate_model_load();
    }
}

/// `tm.transcribe_recording(samples)`, or, while no model is ready, queues
/// the take (`file_name` is its WAV) and returns an empty transcription:
/// Handy then saves the WAV and an empty History row and pastes nothing.
pub fn transcribe_take(
    app: &AppHandle,
    tm: &crate::managers::transcription::TranscriptionManager,
    file_name: &str,
    samples: Vec<f32>,
) -> anyhow::Result<crate::managers::transcription::Transcription> {
    if READY.load(Ordering::Acquire) == NO {
        // A live decode may have started at key-down (it has no model);
        // tear it down so the next take can start one.
        tm.cancel_stream();
        queue_take(app, file_name);
        return Ok(crate::managers::transcription::Transcription {
            text: String::new(),
            language: None,
        });
    }
    // The model may have been selected while this take recorded (it was
    // queued at key-down): load it now; the batch run waits for the load.
    let warm = tm.is_model_loaded();
    if !warm {
        tm.initiate_model_load();
    }
    let audio_ms = samples.len() as u64 * 1000 / 16_000;
    let started = std::time::Instant::now();
    let result = tm.transcribe_recording(samples);
    super::upgrade::on_take_ended();
    if result.is_ok() {
        on_take_timed(app, warm, audio_ms, started.elapsed().as_millis() as u64);
    }
    result
}

/// S-07b: a transcribed take proves the selection loads, and a warm one of
/// dictation length feeds the measured speed (lengths and times only).
fn on_take_timed(app: &AppHandle, warm: bool, audio_ms: u64, keyup_ms: u64) {
    if READY.load(Ordering::Acquire) != YES {
        return;
    }
    let model = crate::settings::get_settings(app).selected_model;
    if model.is_empty() {
        return;
    }
    super::upgrade::record_loaded(app, &model);
    if super::speed::counts(audio_ms, warm)
        && super::upgrade::record_speed(app, &model, audio_ms, keyup_ms)
    {
        kick(Kick::Speed);
    }
}

/// The bar asks whether a take's empty outcome is a queued take (so it says
/// "Setting up", not "No speech heard"). Consumes one.
pub fn take_queued_outcome() -> bool {
    let mut n = QUEUED_OUTCOMES.load(Ordering::Acquire);
    while n > 0 {
        match QUEUED_OUTCOMES.compare_exchange(n, n - 1, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return true,
            Err(now) => n = now,
        }
    }
    false
}

fn queue_take<R: Runtime>(app: &AppHandle<R>, file_name: &str) {
    QUEUED_OUTCOMES.fetch_add(1, Ordering::AcqRel);
    if let Some(db) = app.try_state::<crate::fork::db::ForkDb>() {
        let now = chrono::Utc::now().timestamp_millis();
        if let Err(e) = db.with_conn(|c| super::queue::enqueue(c, file_name, now)) {
            log::warn!("models: could not record a queued take: {e}");
        }
    }
    log::info!("models: take queued until a speech model is ready ({file_name})");
    crate::fork::onboarding::practice::on_queued(app, file_name);
    crate::fork::pill::on_take_queued();
    emit(app);
}

/// `fork::db::entry_meta::on_saved`: Handy saved a History row after its WAV.
pub fn on_entry_saved<R: Runtime>(app: &AppHandle<R>, file_name: &str, entry_id: i64) {
    let Some(db) = app.try_state::<crate::fork::db::ForkDb>() else {
        return;
    };
    match db.with_conn(|c| super::queue::bind_entry(c, file_name, entry_id)) {
        Ok(Some(_)) => {}
        Ok(None) => return,
        Err(e) => {
            log::warn!("models: could not bind a queued take: {e}");
            return;
        }
    }
    if READY.load(Ordering::Acquire) == YES {
        if let Some(handle) = WRY_APP.get() {
            drain(handle);
        }
    }
}

// ---------------------------------------------------------------------------
// Status

fn set_status(f: impl FnOnce(&mut ForkSpeechSetup)) -> ForkSpeechSetup {
    let mut s = lock_status();
    f(&mut s);
    s.clone()
}

fn emit<R: Runtime>(app: &AppHandle<R>) {
    use tauri_specta::Event;
    let s = status();
    crate::fork::pill::on_speech_setup(&s);
    if let Err(e) = s.emit(app) {
        log::debug!("models: could not emit fork-speech-setup: {e}");
    }
}

fn set_ready(ready: bool) {
    READY.store(if ready { YES } else { NO }, Ordering::Release);
}

// ---------------------------------------------------------------------------
// The setup loop (Wry: Handy's managers)

static WRY_APP: OnceLock<AppHandle> = OnceLock::new();

/// From `fork::on_ready` (product builds): decides at once (so the first
/// press already knows), then runs the setup loop in the background.
pub fn start(app: &AppHandle) {
    use tauri::{Listener, Manager};
    if WRY_APP.set(app.clone()).is_err() {
        return;
    }
    EXITING.store(false, Ordering::Release);
    let Some(mm) = app.try_state::<std::sync::Arc<crate::managers::model::ModelManager>>() else {
        log::warn!("models: no model manager; speech setup off");
        return;
    };
    let mm = mm.inner().clone();
    let initial = snapshot(app, &mm);
    let ready = matches!(initial, super::readiness::SpeechReadiness::Ready { .. });
    set_ready(ready);
    set_status(|s| {
        s.phase = if ready {
            SetupPhase::Ready
        } else {
            SetupPhase::Downloading
        };
    });
    log::info!("models: speech setup at launch: {initial:?}");
    super::queue::reset_batch();
    RECOVERING.store(false, Ordering::Release);
    if let Some(db) = app.try_state::<crate::fork::db::ForkDb>() {
        if let Ok(n) = db.with_conn(|c| super::queue::pending(c)) {
            if n > 0 {
                log::info!("models: recovering {n} queued take(s) from an earlier session");
                RECOVERING.store(true, Ordering::Release);
                let app = app.clone();
                let spawned = std::thread::Builder::new()
                    .name("wb-queued-takes".into())
                    .spawn(move || recover_interrupted(&app));
                if let Err(e) = spawned {
                    RECOVERING.store(false, Ordering::Release);
                    log::warn!("models: could not start queued-take recovery: {e}");
                }
            }
        }
    }

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let _ = KICK.set(tx);

    // Progress of the download we follow.
    app.listen_any("model-download-progress", |event| {
        let Ok(p) = serde_json::from_str::<serde_json::Value>(event.payload()) else {
            return;
        };
        let id = p
            .get("model_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if FOLLOWED
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_deref()
            != Some(id)
        {
            return;
        }
        let done = p.get("downloaded").and_then(|v| v.as_u64()).unwrap_or(0);
        let total = p.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
        on_download_progress(done, total);
    });
    for name in [
        "model-download-complete",
        "model-download-failed",
        "model-download-cancelled",
        // S-07b: a selection on the Models page (pins it) or a deleted
        // model.
        "model-state-changed",
        "models-updated",
    ] {
        app.listen_any(name, |_| kick(Kick::ModelEvent));
    }
    // A queued take transcribed again by Handy's job queue.
    app.listen_any("transcription-job", |event| {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(event.payload()) else {
            return;
        };
        let Some(id) = v.get("entry_id").and_then(|v| v.as_i64()) else {
            return;
        };
        let status = v.get("status");
        let state = status
            .and_then(|s| s.get("state"))
            .and_then(|s| s.as_str())
            .unwrap_or_default();
        let ok = match state {
            "finished" => true,
            "failed" => false,
            _ => return,
        };
        let error = status
            .and_then(|s| s.get("error"))
            .and_then(|e| e.as_str())
            .unwrap_or_default()
            .to_string();
        if let Some(app) = WRY_APP.get() {
            on_job_finished(app, id, ok, &error);
        }
    });
    #[cfg(target_os = "macos")]
    crate::fork::insert::session::mac::observe(|s| {
        use crate::fork::insert::session::{Interruption, Signal};
        if s == Signal::End(Interruption::Sleep) {
            kick(Kick::Wake);
        }
    });

    let app2 = app.clone();
    tauri::async_runtime::spawn(async move { run(app2, mm, rx).await });
    kick(Kick::Start);
    emit(app);
}

/// The Handy model id whose progress events feed the status.
static FOLLOWED: Mutex<Option<String>> = Mutex::new(None);
/// The followed download is a silent upgrade (S-07b): its progress goes to
/// the Models panel, not to the bar.
static BACKGROUND: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The alternate transfer's token; Handy's own token is reached through its
/// ModelManager. Both are cancelled at quit so hf-hub can flush its offset.
static DOWNLOAD_CANCEL: Mutex<Option<hf_hub::api::tokio::CancellationToken>> = Mutex::new(None);
static TRANSFER_ACTIVE: Mutex<bool> = Mutex::new(false);
static TRANSFER_DONE: Condvar = Condvar::new();
static EXITING: AtomicBool = AtomicBool::new(false);

struct ActiveTransfer;

impl ActiveTransfer {
    fn new() -> Self {
        *TRANSFER_ACTIVE.lock().unwrap_or_else(|e| e.into_inner()) = true;
        ActiveTransfer
    }
}

impl Drop for ActiveTransfer {
    fn drop(&mut self) {
        *TRANSFER_ACTIVE.lock().unwrap_or_else(|e| e.into_inner()) = false;
        TRANSFER_DONE.notify_all();
    }
}

/// Gives an in-flight hf-hub download a clean cancellation unwind before the
/// process exits. Without it the preallocated `.sync.part` has no committed
/// offset and the next launch starts at byte zero (S-07a's open item).
pub fn on_exit<R: Runtime>(app: &AppHandle<R>) {
    if EXITING.swap(true, Ordering::AcqRel) {
        return;
    }
    if let Some(token) = DOWNLOAD_CANCEL
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
    {
        token.cancel();
    }
    let id = FOLLOWED.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let (Some(mm), Some(id)) = (
        app.try_state::<std::sync::Arc<crate::managers::model::ModelManager>>(),
        id,
    ) {
        if let Err(e) = mm.cancel_download(&id) {
            log::debug!("models: could not cancel {id} at quit: {e}");
        }
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(6);
    let mut active = TRANSFER_ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
    while *active {
        let now = std::time::Instant::now();
        if now >= deadline {
            log::warn!("models: download cancellation did not finish before quit");
            break;
        }
        let (next, _) = TRANSFER_DONE
            .wait_timeout(active, deadline - now)
            .unwrap_or_else(|e| e.into_inner());
        active = next;
    }
}

fn follow(id: Option<&str>) {
    *FOLLOWED.lock().unwrap_or_else(|e| e.into_inner()) = id.map(str::to_owned);
}

/// Progress of the followed download: the bar's status for the first model,
/// the Models panel for a silent upgrade (S-07b).
fn on_download_progress(done: u64, total: u64) {
    let percent = percent_of(done, total);
    let Some(app) = WRY_APP.get() else { return };
    if BACKGROUND.load(Ordering::Acquire) {
        super::upgrade::on_progress(app, percent);
        return;
    }
    let before = lock_status().percent;
    let s = set_status(|s| {
        s.downloaded_bytes = done;
        s.total_bytes = total;
        s.percent = Some(percent);
    });
    if before != s.percent {
        emit(app);
    }
}

/// Never 100 before the file is verified.
pub fn percent_of(done: u64, total: u64) -> u8 {
    if total == 0 {
        return 0;
    }
    ((done.min(total) as u128 * 100 / total as u128) as u8).min(99)
}

fn snapshot(
    app: &AppHandle,
    mm: &crate::managers::model::ModelManager,
) -> super::readiness::SpeechReadiness {
    let selected = crate::settings::get_settings(app).selected_model;
    let retired: Vec<String> = mm.get_retired_models().into_iter().map(|m| m.id).collect();
    let models = mm.get_available_models();
    let facts: Vec<super::readiness::ModelFacts<'_>> = models
        .iter()
        .map(|m| super::readiness::ModelFacts {
            id: &m.id,
            is_downloaded: m.is_downloaded,
            is_downloading: m.is_downloading,
            is_retired: retired.contains(&m.id),
        })
        .collect();
    super::readiness::resolve(&selected, &facts)
}

fn user_languages(app: &AppHandle) -> Vec<String> {
    let spoken = crate::settings::get_settings(app).spoken_languages;
    let os: Vec<String> = sys_locale::get_locales().collect();
    super::starter::user_languages(&spoken, &os)
}

fn target(app: &AppHandle) -> Option<&'static super::starter::Pinned> {
    let starters = super::starter::STARTERS.as_ref()?;
    Some(super::starter::pick(starters, &user_languages(app)))
}

async fn run(
    app: AppHandle,
    mm: std::sync::Arc<crate::managers::model::ModelManager>,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<Kick>,
) {
    use super::readiness::SpeechReadiness;
    while rx.recv().await.is_some() {
        if EXITING.load(Ordering::Acquire) {
            return;
        }
        // Coalesce a burst of kicks: each round looks at the state afresh.
        while rx.try_recv().is_ok() {}
        match snapshot(&app, &mm) {
            SpeechReadiness::Ready { model_id } => {
                if READY.load(Ordering::Acquire) != YES {
                    log::info!("models: speech model ready ({model_id})");
                }
                become_ready(&app);
                // S-07b: the best model for this Mac, silently.
                super::upgrade::step(&app, &mm, &model_id, &mut rx).await;
            }
            SpeechReadiness::UseOther { model_id } => {
                log::info!("models: selecting the speech model on disk ({model_id})");
                set_ready(false);
                if select_when_idle(&app, &model_id).await {
                    become_ready(&app);
                    kick(Kick::Start);
                }
            }
            SpeechReadiness::Downloading { model_id } => {
                // Someone else's download (Handy's model page): follow it;
                // its end kicks us again.
                set_ready(false);
                follow(Some(&model_id));
                set_status(|s| {
                    s.phase = SetupPhase::Downloading;
                    s.problem = None;
                });
                emit(&app);
            }
            SpeechReadiness::Missing => {
                set_ready(false);
                let Some(pinned) = target(&app) else {
                    log::error!("models: no starter table; nothing to download");
                    continue;
                };
                if download(&app, &mm, pinned, &mut rx, Mode::Setup).await {
                    let id = pinned.handy_id();
                    super::upgrade::record(&app, &id, super::cleanup::Origin::Starter);
                    if select_when_idle(&app, &id).await {
                        super::upgrade::record_loaded(&app, &id);
                        become_ready(&app);
                        // S-07b: right after the starter, the best model.
                        kick(Kick::Start);
                    }
                }
                // Not ready: wait for the next kick (Retry, wake, a new
                // language, a Handy download).
            }
        }
    }
}

fn become_ready(app: &AppHandle) {
    follow(None);
    set_ready(true);
    set_status(|s| {
        s.phase = SetupPhase::Ready;
        s.problem = None;
        s.needed_mb = None;
        s.percent = None;
    });
    emit(app);
    drain(app);
}

/// Selects `id` once no take records or transcribes ("swap never during a
/// take"). `false` if Handy refused it.
async fn select_when_idle(app: &AppHandle, id: &str) -> bool {
    select_when(
        app,
        id,
        |app| !crate::tray::dictation_in_progress(app),
        1200,
    )
    .await
}

/// S-07b: like [`select_when_idle`], but also not within
/// [`super::upgrade::QUIET_MS`] of a take; waits as long as it takes.
pub(super) async fn select_when_quiet(app: &AppHandle, id: &str) -> bool {
    select_when(app, id, super::upgrade::quiet_now, u32::MAX).await
}

async fn select_when(app: &AppHandle, id: &str, ready: fn(&AppHandle) -> bool, tries: u32) -> bool {
    for _ in 0..tries {
        if ready(app) {
            let previous = crate::settings::get_settings(app).selected_model;
            super::upgrade::note_fork_selection(app, id);
            let (app2, id2) = (app.clone(), id.to_string());
            let r = tauri::async_runtime::spawn_blocking(move || {
                crate::commands::models::switch_active_model(&app2, &id2)
            })
            .await
            .unwrap_or_else(|e| Err(e.to_string()));
            match r {
                Ok(()) => {
                    log::info!("models: selected {id}");
                    return true;
                }
                Err(e) => {
                    // Handy kept (or put back) the old selection.
                    super::upgrade::note_fork_selection(app, &previous);
                    if !e.contains("already in progress") {
                        log::error!("models: could not select {id}: {e}");
                        return false;
                    }
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    log::warn!("models: {id} not selected: a take kept running");
    false
}

/// Free bytes on the volume of `path` or its nearest existing parent.
fn free_space(path: &std::path::Path) -> Option<u64> {
    path.ancestors()
        .find(|p| p.exists())
        .and_then(crate::fork::db::disk::free_bytes)
}

/// Whether a download shows on the bar (the first model) or only on the
/// Models panel (a silent upgrade, S-07b).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    Setup,
    Background,
}

/// How a background download ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Background {
    /// On disk and verified.
    Done,
    /// It gave up (network, disk, checksum).
    Failed,
    /// The pick changed or a kick stopped it.
    Stopped,
}

/// Set by the background download machine when it gives up.
static BACKGROUND_FAILED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// S-07b: downloads and verifies a policy pick without telling the bar.
pub(super) async fn download_background(
    app: &AppHandle,
    mm: &std::sync::Arc<crate::managers::model::ModelManager>,
    pinned: &'static super::starter::Pinned,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<Kick>,
) -> Background {
    BACKGROUND.store(true, Ordering::Release);
    BACKGROUND_FAILED.store(false, Ordering::Release);
    let ok = download(app, mm, pinned, rx, Mode::Background).await;
    BACKGROUND.store(false, Ordering::Release);
    follow(None);
    match (ok, BACKGROUND_FAILED.swap(false, Ordering::AcqRel)) {
        (true, _) => Background::Done,
        (false, true) => Background::Failed,
        (false, false) => Background::Stopped,
    }
}

/// The model the download should still be fetching (a language change may
/// have moved the pick).
async fn wanted(
    app: &AppHandle,
    mm: &crate::managers::model::ModelManager,
    mode: Mode,
) -> Option<String> {
    match mode {
        Mode::Setup => target(app).map(|p| p.handy_id()),
        Mode::Background => super::upgrade::target_handy_id(app, mm).await,
    }
}

pub(super) fn hub_dir() -> std::path::PathBuf {
    crate::fork::llm::download::hub_dir()
}

/// Downloads and verifies `pinned`. `true` when the file is on disk and
/// matches its pin.
async fn download(
    app: &AppHandle,
    mm: &std::sync::Arc<crate::managers::model::ModelManager>,
    pinned: &'static super::starter::Pinned,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<Kick>,
    mode: Mode,
) -> bool {
    use super::download::{Event, Machine, Step};
    let id = pinned.handy_id();
    let fg = mode == Mode::Setup;
    // The bar's status only for the first model; a silent upgrade reports
    // to the Models panel (`upgrade::on_progress`) and the log.
    let show = |f: &dyn Fn(&mut ForkSpeechSetup)| {
        if fg {
            set_status(f);
            emit(app);
        }
    };
    log::info!(
        "models: downloading {} {} ({} MB) for {:?}",
        if fg {
            "the first speech model"
        } else {
            "a better speech model"
        },
        pinned.catalog_id,
        pinned.size / 1_000_000,
        user_languages(app)
    );
    follow(Some(&id));
    let mut m = Machine::new(pinned.size);
    let mut step = m.step().clone();
    loop {
        step = match step {
            Step::CheckDisk => {
                let free = free_space(&hub_dir());
                if free.is_some_and(|f| f < m.needed_bytes()) {
                    log::warn!(
                        "models: not enough disk space ({} MB free, {} MB needed)",
                        free.unwrap_or(0) / 1_000_000,
                        m.needed_bytes() / 1_000_000
                    );
                }
                m.on(Event::DiskFree(free))
            }
            Step::Transfer => {
                show(&|s| {
                    s.phase = SetupPhase::Downloading;
                    s.problem = None;
                    s.needed_mb = None;
                    s.total_bytes = pinned.size;
                    s.percent.get_or_insert(0);
                });
                log::info!("models: transfer {} of {id}", m.transfers());
                // A catalog default goes through Handy's downloader; another
                // quant of a catalog model (the English starter, S-07b) is not
                // in Handy's registry until its file is in the cache, so the
                // fork fetches it with the same hf-hub cache and pin.
                let alternate = mm.get_model_info(&id).is_none();
                let cancel = hf_hub::api::tokio::CancellationToken::new();
                *DOWNLOAD_CANCEL.lock().unwrap_or_else(|e| e.into_inner()) = Some(cancel.clone());
                let active = ActiveTransfer::new();
                let result = {
                    let fut = async {
                        if alternate {
                            transfer_alternate(mm, pinned, cancel.clone()).await
                        } else {
                            mm.download_model(&id).await
                        }
                    };
                    tokio::pin!(fut);
                    loop {
                        tokio::select! {
                            r = &mut fut => break r,
                            Some(k) = rx.recv() => {
                                if k == Kick::Languages {
                                    let next = wanted(app, mm, mode).await;
                                    if next.as_deref() != Some(id.as_str()) {
                                        log::info!("models: languages changed; switching downloads");
                                        cancel.cancel();
                                        let _ = mm.cancel_download(&id);
                                    }
                                }
                            }
                        }
                    }
                };
                drop(active);
                *DOWNLOAD_CANCEL.lock().unwrap_or_else(|e| e.into_inner()) = None;
                let on_disk = mm.get_model_info(&id).is_some_and(|i| i.is_downloaded);
                match result {
                    Ok(()) if on_disk => m.on(Event::TransferDone),
                    Ok(()) => m.on(Event::TransferCancelled),
                    Err(e) => {
                        log::warn!("models: transfer failed: {e:#}");
                        let (url, allowed) = probe_target(pinned);
                        let verdict = super::probe::probe(&url, allowed).await;
                        log::info!("models: probe after the failure: {verdict:?}");
                        m.on(Event::TransferFailed(verdict))
                    }
                }
            }
            Step::Verify => {
                show(&|s| s.phase = SetupPhase::Verifying);
                let ok = verify(mm, &id, pinned).await;
                log::info!(
                    "models: checksum of {id}: {}",
                    if ok { "ok" } else { "MISMATCH" }
                );
                m.on(Event::Verified(ok))
            }
            Step::Delete => {
                if let Err(e) = mm.delete_model(&id) {
                    log::warn!("models: could not delete the bad file: {e}");
                }
                m.on(Event::Deleted)
            }
            Step::Wait { delay, problem } => {
                show(&|s| {
                    s.phase = SetupPhase::Waiting;
                    s.problem = problem;
                });
                log::info!("models: next attempt in {} s", delay.as_secs());
                let woke = wait_or_kick(app, mm, mode, delay, &id, rx).await;
                match woke {
                    Woken::Timer => m.on(Event::Waited),
                    Woken::Retry => m.on(Event::Retry),
                    Woken::NewTarget => return false,
                }
            }
            Step::Ready => return true,
            Step::Failed(problem) => {
                if !fg {
                    // A silent upgrade never waits for a Retry the user
                    // cannot see: the next launch or change tries again.
                    log::warn!("models: background download stopped: {problem:?}");
                    BACKGROUND_FAILED.store(true, Ordering::Release);
                    return false;
                }
                let needed = m.needed_bytes() / 1_000_000;
                show(&|s| {
                    s.phase = SetupPhase::Failed;
                    s.problem = Some(problem);
                    s.needed_mb = (problem == SetupProblem::NoSpace).then_some(needed);
                });
                log::warn!("models: download stopped: {problem:?} (Retry restarts it)");
                match wait_or_kick(app, mm, mode, std::time::Duration::MAX, &id, rx).await {
                    Woken::Retry | Woken::Timer => m.on(Event::Retry),
                    Woken::NewTarget => return false,
                }
            }
            Step::Stopped => {
                log::info!("models: download of {id} stopped");
                // A new target: the loop's next round picks it up.
                kick(Kick::Start);
                return false;
            }
        };
    }
}

/// The URL the probe asks and the hosts it may reach. Handy's downloader
/// honours `HF_ENDPOINT` (hf-hub `from_env`); in debug builds, and only for
/// a loopback endpoint (the S-07a test mirror, `scripts/wb-s07a/`), the
/// probe follows it so a simulated portal or 429 is seen by both. Release
/// builds always probe `huggingface.co`.
fn probe_target(pinned: &super::starter::Pinned) -> (String, fn(&str) -> bool) {
    #[cfg(debug_assertions)]
    if let Ok(endpoint) = std::env::var("HF_ENDPOINT") {
        if crate::fork::net::is_loopback(&endpoint) {
            fn loopback(url: &str) -> bool {
                crate::fork::net::is_loopback(url)
            }
            let url = format!(
                "{}/{}/resolve/{}/{}",
                endpoint.trim_end_matches('/'),
                pinned.repo,
                pinned.revision,
                pinned.file
            );
            return (url, loopback);
        }
    }
    (pinned.resolve_url(), crate::fork::net::is_allowed_host)
}

/// The Hugging Face endpoint the fork's own transfer uses: `huggingface.co`,
/// or in debug builds a loopback `HF_ENDPOINT` (the S-07a test stand-in),
/// the same rule as [`probe_target`].
fn transfer_endpoint() -> String {
    #[cfg(debug_assertions)]
    if let Ok(endpoint) = std::env::var("HF_ENDPOINT") {
        if crate::fork::net::is_loopback(&endpoint) {
            return endpoint.trim_end_matches('/').to_string();
        }
    }
    crate::fork::llm::download::HF_ENDPOINT.to_string()
}

#[derive(Clone)]
struct AltProgress {
    done: std::sync::Arc<std::sync::atomic::AtomicU64>,
    total: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl hf_hub::api::tokio::Progress for AltProgress {
    async fn init(&mut self, size: usize, _filename: &str) {
        self.total.store(size as u64, Ordering::Relaxed);
        self.done.store(0, Ordering::Relaxed);
        on_download_progress(0, size as u64);
    }
    async fn update(&mut self, size: usize) {
        let done = self.done.fetch_add(size as u64, Ordering::Relaxed) + size as u64;
        on_download_progress(done, self.total.load(Ordering::Relaxed));
    }
    async fn finish(&mut self) {}
}

/// S-07b: one transfer of a non-default quant of a catalog model into the
/// Hugging Face cache Handy scans (`HF_HOME/hub`), at the pinned revision,
/// no token; then Handy's rescan registers it under `<repo>/<file>`. The
/// checksum is checked by the caller like any other download. hf-hub keeps
/// a `.sync.part` on failure, so the next transfer resumes.
async fn transfer_alternate(
    mm: &std::sync::Arc<crate::managers::model::ModelManager>,
    pinned: &super::starter::Pinned,
    cancel: hf_hub::api::tokio::CancellationToken,
) -> anyhow::Result<()> {
    use hf_hub::api::tokio::ApiBuilder;
    use hf_hub::{Cache, Repo, RepoType};
    let endpoint = transfer_endpoint();
    let url = format!(
        "{endpoint}/{}/resolve/{}/{}",
        pinned.repo, pinned.revision, pinned.file
    );
    if !crate::fork::net::is_allowed_host(&url) && !crate::fork::net::is_loopback(&url) {
        anyhow::bail!("download host not allowed");
    }
    let api = ApiBuilder::from_cache(Cache::new(hub_dir()))
        .with_endpoint(endpoint)
        .with_token(None)
        .with_progress(false)
        .build()?;
    let repo = api.repo(Repo::with_revision(
        pinned.repo.clone(),
        RepoType::Model,
        pinned.revision.clone(),
    ));
    let progress = AltProgress {
        done: Default::default(),
        total: Default::default(),
    };
    match repo
        .download_with_progress_cancellable(&pinned.file, progress, cancel.clone())
        .await
    {
        Ok(_) => {}
        // A cancel is not a failure: the machine sees "not on disk".
        Err(_) if cancel.is_cancelled() => return Ok(()),
        Err(e) => return Err(e.into()),
    }
    let mm = mm.clone();
    tauri::async_runtime::spawn_blocking(move || mm.rescan_local_models())
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))??;
    // What Handy's downloader announces after its own downloads: the model
    // list refreshes and the first model's "Dictation is ready" is posted.
    if let Some(app) = WRY_APP.get() {
        use tauri::Emitter;
        let _ = app.emit("model-download-complete", pinned.handy_id());
    }
    Ok(())
}

enum Woken {
    Timer,
    Retry,
    NewTarget,
}

async fn wait_or_kick(
    app: &AppHandle,
    mm: &crate::managers::model::ModelManager,
    mode: Mode,
    delay: std::time::Duration,
    id: &str,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<Kick>,
) -> Woken {
    let sleep = tokio::time::sleep(delay.min(std::time::Duration::from_secs(365 * 86_400)));
    tokio::pin!(sleep);
    loop {
        tokio::select! {
            _ = &mut sleep => return Woken::Timer,
            k = rx.recv() => match k {
                Some(Kick::Retry) | Some(Kick::Wake) | Some(Kick::Start) => return Woken::Retry,
                Some(Kick::Languages) | Some(Kick::Speed) | Some(Kick::Policy) => {
                    if wanted(app, mm, mode).await.as_deref() != Some(id) {
                        kick(Kick::Start);
                        return Woken::NewTarget;
                    }
                }
                Some(Kick::ModelEvent) => {}
                None => return Woken::Timer,
            }
        }
    }
}

/// sha256 and size of the downloaded file against the pin.
async fn verify(
    mm: &crate::managers::model::ModelManager,
    id: &str,
    pinned: &super::starter::Pinned,
) -> bool {
    let Ok(path) = mm.get_model_path(id) else {
        return false;
    };
    let (size, sha) = (pinned.size, pinned.sha256.clone());
    tauri::async_runtime::spawn_blocking(move || {
        let len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        len == size
            && crate::fork::llm::download::sha256_file(
                &path,
                &hf_hub::api::tokio::CancellationToken::new(),
            )
            .is_ok_and(|h| h == sha)
    })
    .await
    .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Durable queued takes → Handy's ordered job queue → one batch notification

static RECOVERING: AtomicBool = AtomicBool::new(false);

/// Rebinds saved rows and reconstructs a History row when quit happened after
/// the WAV write but before Handy saved it. Recovery is serialized before the
/// batch starts, so its notification cannot split into one notice per take.
fn recover_interrupted(app: &AppHandle) {
    let result = (|| -> Result<(), String> {
        let db = app
            .try_state::<crate::fork::db::ForkDb>()
            .ok_or("fork.db is not ready")?;
        let hm = app
            .try_state::<std::sync::Arc<crate::managers::history::HistoryManager>>()
            .ok_or("History is not ready")?;
        let entries = tauri::async_runtime::block_on(hm.get_history_entries(None, None))
            .map_err(|e| e.to_string())?
            .entries;
        let rows = db
            .with_conn(|c| super::queue::recover(c, &entries))
            .map_err(|e| e.to_string())?;
        for take in rows.into_iter().filter(|t| t.entry_id.is_none()) {
            if !hm.get_audio_file_path(&take.file_name).is_file() {
                log::warn!("models: queued WAV is missing ({})", take.file_name);
                continue;
            }
            let entry = hm
                .save_entry(
                    take.file_name.clone(),
                    String::new(),
                    false,
                    None,
                    None,
                    None,
                )
                .map_err(|e| e.to_string())?;
            let day = crate::fork::stats::local_day(take.created_at);
            db.with_conn(|c| {
                c.execute(
                    "UPDATE entry_meta SET created_at = ?2, day = ?3 WHERE entry_id = ?1",
                    rusqlite::params![entry.id, take.created_at, day],
                )?;
                Ok(())
            })
            .map_err(|e| e.to_string())?;
            log::info!(
                "models: recovered queued take into History (entry {})",
                entry.id
            );
        }
        Ok(())
    })();
    if let Err(e) = result {
        log::warn!("models: queued-take recovery failed: {e}");
    }
    RECOVERING.store(false, Ordering::Release);
    if READY.load(Ordering::Acquire) == YES {
        drain(app);
    }
}

/// Transcribes every waiting take whose saved audio and History row exist.
fn drain(app: &AppHandle) {
    if RECOVERING.load(Ordering::Acquire) {
        return;
    }
    let Some(jobs) = app
        .try_state::<std::sync::Arc<crate::managers::transcription_jobs::TranscriptionJobManager>>(
        )
    else {
        return;
    };
    let Some(db) = app.try_state::<crate::fork::db::ForkDb>() else {
        return;
    };
    let started = match db.with_conn(super::queue::start_ready) {
        Ok(started) => started,
        Err(e) => {
            log::warn!("models: could not start queued takes: {e}");
            return;
        }
    };
    if started.is_empty() {
        return;
    }
    super::queue::begin_batch(&started);
    for take in &started {
        let Some(entry) = take.entry_id else { continue };
        log::info!("models: transcribing queued take (entry {entry})");
        jobs.retry(entry);
    }
    crate::fork::pill::on_queue_transcribing();
    emit(app);
}

fn on_job_finished(app: &AppHandle, entry_id: i64, ok: bool, error: &str) {
    let Some(db) = app.try_state::<crate::fork::db::ForkDb>() else {
        return;
    };
    let finished = db.with_conn(|c| {
        let take = super::queue::finish(c, entry_id, ok)?;
        if ok && take.is_some() {
            crate::fork::db::entry_meta::set_paste_outcome(
                c,
                entry_id,
                crate::fork::insert::deliver::PasteOutcome::Queued,
            )?;
        }
        Ok(take)
    });
    match finished {
        Ok(Some(take)) => {
            if ok {
                // S-29b: the practice screen may still wait for this take.
                crate::fork::onboarding::practice::on_queued_take_done(
                    app,
                    &take.file_name,
                    entry_id,
                );
            }
        }
        Ok(None) => {
            // A deleted take's queue row is already gone; it must not hold
            // the rest of its batch open forever.
            notify_batch(app, super::queue::finish_batch(entry_id, false));
            return;
        }
        Err(e) => {
            log::warn!("models: could not finish queued take {entry_id}: {e}");
            return;
        }
    }
    emit(app);
    if ok {
        log::info!("models: queued take {entry_id} ready in History");
    } else {
        log::info!("models: queued take {entry_id} stays in History for Retry: {error}");
    }
    notify_batch(app, super::queue::finish_batch(entry_id, ok));
}

fn notify_batch(app: &AppHandle, count: Option<u32>) {
    let Some(count) = count else { return };
    log::info!("models: {count} queued dictation(s) ready");
    crate::fork::notify::post(
        app,
        crate::fork::notify::catalog::Notice::queued_takes_ready(count),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_never_reaches_100_before_ready() {
        assert_eq!(percent_of(0, 0), 0);
        assert_eq!(percent_of(50, 100), 50);
        assert_eq!(percent_of(100, 100), 99);
        assert_eq!(percent_of(200, 100), 99);
        assert_eq!(percent_of(135_373_279, 135_373_280), 99);
    }

    #[test]
    fn queued_outcomes_are_consumed_once() {
        QUEUED_OUTCOMES.store(0, Ordering::SeqCst);
        assert!(!take_queued_outcome());
        QUEUED_OUTCOMES.fetch_add(2, Ordering::SeqCst);
        assert!(take_queued_outcome());
        assert!(take_queued_outcome());
        assert!(!take_queued_outcome());
    }

    #[test]
    fn the_gate_follows_handy_until_the_fork_decides() {
        // Unknown (plain Handy, early start): Handy's own check applies.
        READY.store(UNKNOWN, Ordering::SeqCst);
        assert!(!can_record_without_model());
        set_ready(false);
        assert!(can_record_without_model());
        set_ready(true);
        assert!(!can_record_without_model());
        READY.store(UNKNOWN, Ordering::SeqCst);
    }

    #[test]
    fn the_status_names_no_model() {
        let json = serde_json::to_value(status()).unwrap();
        let keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
        assert!(!keys
            .iter()
            .any(|k| k.contains("model") || k.contains("name")));
    }

    #[test]
    fn hooks_are_in_place() {
        let actions = include_str!("../../actions.rs");
        assert!(actions.contains(
            "if !tm.is_model_loaded() && !crate::fork::models::setup::can_record_without_model() {"
        ));
        assert!(actions.contains("crate::fork::models::setup::initiate_model_load(&tm);"));
        assert!(actions.contains(
            "crate::fork::models::setup::transcribe_take(&ah, &tm, &file_name, samples);"
        ));
        assert!(!actions.contains("tm.transcribe_recording(samples)"));
        let plugin = include_str!("../../fork/mod.rs");
        assert_eq!(plugin.matches("models::setup::on_exit(app);").count(), 2);
    }
}
