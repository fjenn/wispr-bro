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
//! ([`super::queue_session`]).
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

use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Runtime};

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
    /// Takes recorded this session that wait for a model or are being
    /// transcribed.
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
    /// A Handy download ended (someone else's, or ours).
    ModelEvent,
}

static KICK: OnceLock<tokio::sync::mpsc::UnboundedSender<Kick>> = OnceLock::new();

fn kick(k: Kick) {
    if let Some(tx) = KICK.get() {
        let _ = tx.send(k);
    }
}

/// `fork_speech_setup_status`.
pub fn status() -> ForkSpeechSetup {
    let mut s = lock_status().clone();
    s.queued_takes = super::queue_session::session().pending();
    s
}

/// `fork_speech_setup_retry` and the bar's Retry.
pub fn retry() {
    kick(Kick::Retry);
}

/// `fork_speech_setup_refresh`: the languages may have changed.
pub fn refresh() {
    kick(Kick::Languages);
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
    if !tm.is_model_loaded() {
        tm.initiate_model_load();
    }
    tm.transcribe_recording(samples)
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
    use tauri::Manager;
    super::queue_session::session().queue(file_name);
    QUEUED_OUTCOMES.fetch_add(1, Ordering::AcqRel);
    if let Some(db) = app.try_state::<crate::fork::db::ForkDb>() {
        let now = chrono::Utc::now().timestamp_millis();
        match db.with_conn(|c| super::queue_session::insert_row(c, file_name, now)) {
            Ok(id) => super::queue_session::session().set_row(file_name, id),
            Err(e) => log::warn!("models: could not record a queued take: {e}"),
        }
    }
    log::info!("models: take queued until a speech model is ready ({file_name})");
    crate::fork::pill::on_take_queued();
    emit(app);
}

/// `fork::db::entry_meta::on_saved`: Handy saved a History row.
pub fn on_entry_saved<R: Runtime>(app: &AppHandle<R>, file_name: &str, entry_id: i64) {
    use tauri::Manager;
    let Some(take) = super::queue_session::session().bind_entry(file_name, entry_id) else {
        return;
    };
    if let (Some(row), Some(db)) = (take.row_id, app.try_state::<crate::fork::db::ForkDb>()) {
        if let Err(e) = db.with_conn(|c| super::queue_session::set_entry(c, row, entry_id)) {
            log::warn!("models: could not bind a queued take: {e}");
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
    if let Some(db) = app.try_state::<crate::fork::db::ForkDb>() {
        if let Ok(n) = db.with_conn(|c| super::queue_session::leftover_count(c)) {
            if n > 0 {
                log::info!(
                    "models: {n} queued take(s) from an earlier session are in History (S-07c)"
                );
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
        let before = lock_status().percent;
        let percent = percent_of(done, total);
        let s = set_status(|s| {
            s.downloaded_bytes = done;
            s.total_bytes = total;
            s.percent = Some(percent);
        });
        if before != s.percent {
            if let Some(app) = WRY_APP.get() {
                emit(app);
            }
        }
    });
    for name in [
        "model-download-complete",
        "model-download-failed",
        "model-download-cancelled",
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

fn follow(id: Option<&str>) {
    *FOLLOWED.lock().unwrap_or_else(|e| e.into_inner()) = id.map(str::to_owned);
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
        // Coalesce a burst of kicks: each round looks at the state afresh.
        while rx.try_recv().is_ok() {}
        match snapshot(&app, &mm) {
            SpeechReadiness::Ready { model_id } => {
                if READY.load(Ordering::Acquire) != YES {
                    log::info!("models: speech model ready ({model_id})");
                }
                become_ready(&app);
            }
            SpeechReadiness::UseOther { model_id } => {
                log::info!("models: selecting the speech model on disk ({model_id})");
                set_ready(false);
                if select_when_idle(&app, &model_id).await {
                    become_ready(&app);
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
                if download(&app, &mm, pinned, &mut rx).await {
                    let id = pinned.handy_id();
                    if select_when_idle(&app, &id).await {
                        become_ready(&app);
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
    for _ in 0..1200 {
        if !crate::tray::dictation_in_progress(app) {
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
                Err(e) if e.contains("already in progress") => {}
                Err(e) => {
                    log::error!("models: could not select {id}: {e}");
                    return false;
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

fn hub_dir() -> std::path::PathBuf {
    crate::fork::llm::download::hub_dir()
}

/// Downloads and verifies `pinned`. `true` when the file is on disk and
/// matches its pin.
async fn download(
    app: &AppHandle,
    mm: &std::sync::Arc<crate::managers::model::ModelManager>,
    pinned: &'static super::starter::Pinned,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<Kick>,
) -> bool {
    use super::download::{Event, Machine, Step};
    let id = pinned.handy_id();
    log::info!(
        "models: downloading the first speech model {} ({} MB) for {:?}",
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
                set_status(|s| {
                    s.phase = SetupPhase::Downloading;
                    s.problem = None;
                    s.needed_mb = None;
                    s.total_bytes = pinned.size;
                    s.percent.get_or_insert(0);
                });
                emit(app);
                log::info!("models: transfer {} of {id}", m.transfers());
                let result = {
                    let fut = mm.download_model(&id);
                    tokio::pin!(fut);
                    loop {
                        tokio::select! {
                            r = &mut fut => break r,
                            Some(k) = rx.recv() => {
                                if k == Kick::Languages {
                                    let next = target(app).map(|p| p.handy_id());
                                    if next.as_deref() != Some(id.as_str()) {
                                        log::info!("models: languages changed; switching downloads");
                                        let _ = mm.cancel_download(&id);
                                    }
                                }
                            }
                        }
                    }
                };
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
                set_status(|s| s.phase = SetupPhase::Verifying);
                emit(app);
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
                set_status(|s| {
                    s.phase = SetupPhase::Waiting;
                    s.problem = problem;
                });
                emit(app);
                log::info!("models: next attempt in {} s", delay.as_secs());
                let woke = wait_or_kick(app, delay, &id, rx).await;
                match woke {
                    Woken::Timer => m.on(Event::Waited),
                    Woken::Retry => m.on(Event::Retry),
                    Woken::NewTarget => return false,
                }
            }
            Step::Ready => return true,
            Step::Failed(problem) => {
                set_status(|s| {
                    s.phase = SetupPhase::Failed;
                    s.problem = Some(problem);
                    s.needed_mb =
                        (problem == SetupProblem::NoSpace).then(|| m.needed_bytes() / 1_000_000);
                });
                emit(app);
                log::warn!("models: download stopped: {problem:?} (Retry restarts it)");
                match wait_or_kick(app, std::time::Duration::MAX, &id, rx).await {
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

enum Woken {
    Timer,
    Retry,
    NewTarget,
}

async fn wait_or_kick(
    app: &AppHandle,
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
                Some(Kick::Languages) => {
                    if target(app).map(|p| p.handy_id()).as_deref() != Some(id) {
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
// Queued takes → Handy's job queue → the card

/// Transcribes every waiting take whose History row exists.
fn drain(app: &AppHandle) {
    use tauri::Manager;
    let started = super::queue_session::session().start_ready();
    if started.is_empty() {
        return;
    }
    let Some(jobs) = app
        .try_state::<std::sync::Arc<crate::managers::transcription_jobs::TranscriptionJobManager>>(
        )
    else {
        return;
    };
    let db = app.try_state::<crate::fork::db::ForkDb>();
    for t in &started {
        let Some(entry) = t.entry_id else { continue };
        if let (Some(row), Some(db)) = (t.row_id, db.as_ref()) {
            let _ = db.with_conn(|c| {
                super::queue_session::set_status(c, row, super::queue_session::Status::Transcribing)
            });
        }
        log::info!("models: transcribing queued take (entry {entry})");
        jobs.retry(entry);
    }
    crate::fork::pill::on_queue_transcribing();
    emit(app);
}

fn on_job_finished(app: &AppHandle, entry_id: i64, ok: bool, error: &str) {
    use tauri::Manager;
    let Some(take) = super::queue_session::session().finish(entry_id, ok) else {
        return;
    };
    let db = app.try_state::<crate::fork::db::ForkDb>();
    if let (Some(row), Some(db)) = (take.row_id, db.as_ref()) {
        let _ = db.with_conn(|c| super::queue_session::set_status(c, row, take.status));
    }
    emit(app);
    if !ok {
        log::info!("models: queued take {entry_id} could not be transcribed");
        crate::fork::pill::show_message(if error.contains("no speech") {
            crate::fork::pill::model::PillMessage::NoSpeech
        } else {
            crate::fork::pill::model::PillMessage::TranscriptionFailed
        });
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(hm) = app.try_state::<std::sync::Arc<crate::managers::history::HistoryManager>>()
        else {
            return;
        };
        let entry = match hm.get_entry_by_id(entry_id).await {
            Ok(Some(e)) => e,
            _ => return,
        };
        let text = entry
            .post_processed_text
            .filter(|t| !t.trim().is_empty())
            .unwrap_or(entry.transcription_text);
        if text.trim().is_empty() {
            crate::fork::pill::show_message(crate::fork::pill::model::PillMessage::NoSpeech);
            return;
        }
        if let Some(db) = app.try_state::<crate::fork::db::ForkDb>() {
            let _ = db.with_conn(|c| {
                crate::fork::db::entry_meta::set_paste_outcome(
                    c,
                    entry_id,
                    crate::fork::insert::deliver::PasteOutcome::Queued,
                )
            });
        }
        log::info!(
            "models: queued take {entry_id} ready ({} chars) on the card",
            text.chars().count()
        );
        crate::fork::pill::show_card(text);
    });
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
    }
}
