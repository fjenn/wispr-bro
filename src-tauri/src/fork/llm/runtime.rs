// SPDX-License-Identifier: GPL-3.0-or-later
//! The app-level LLM runtime (S-10): ties the catalog, the download, the
//! memory policy and the supervisor to Handy's events.
//!
//! - **After the speech model, the cleanup model.** At `on_ready`, and again
//!   on each `model-download-complete`, the right model for this Mac
//!   ([`catalog::select`]) is checked and, if missing, downloaded silently
//!   once Handy has a speech model on disk.
//! - **Prewarm with ordered loads** (§4.5, D-211, D-213): Handy's
//!   `recording-state-event` (`started` / `stopped` / `cancelled`) and
//!   `model-state-changed` drive the [`LoadPlanner`]; when it says start, the
//!   memory rule is checked and the supervisor is woken in the background.
//! - **Idle and wake.** A 5 s ticker lets the supervisor stop an idle server;
//!   `NSWorkspaceDidWakeNotification` health-checks it and resumes a
//!   download that was cut by the sleep.
//!
//! Status changes go out as the `fork-llm-status` event; `fork_llm_status`
//! returns the same struct.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Listener, Manager, Runtime};
use tauri_specta::Event;

use super::catalog::{self, LlmModel};
use super::chat::CancellationToken;
use super::download::{self, DownloadError, ProgressCounter};
use super::memory::{self, AsrState, LoadPlanner, PlanAction};
use super::supervisor::{
    Config, ProcessLauncher, ServerPhase, ServerStatus, StartError, Supervisor,
};

/// Sub-folder of the app data dir: socket, PID file, verification markers.
pub const LLM_DIR: &str = "llm";

/// Overall state shown in Settings › Advanced › AI engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum LlmState {
    /// No local LLM on this Mac (Intel, DEC-29; or a plain Handy build).
    Unsupported,
    /// The `llama-server` sidecar is not in this build.
    MissingRuntime,
    /// Waiting for the speech model before downloading.
    WaitingForSpeechModel,
    Downloading,
    Verifying,
    /// Model verified; the server starts on demand.
    Ready,
    /// Download failed; Retry is offered.
    DownloadFailed,
}

/// Status event and `fork_llm_status` reply.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkLlmStatus {
    pub state: LlmState,
    /// Display name of the model for this Mac.
    pub model_name: Option<String>,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub server: ServerStatus,
    /// Last download error (no user data).
    pub error: Option<String>,
}

/// `fork_llm_try` reply.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct ForkLlmTryResult {
    /// What a dictation of this text would insert: the cleaned text, or
    /// the text as given when cleanup did not apply.
    pub cleaned: String,
    /// Request time, after the server was ready.
    pub cleanup_ms: u32,
    /// Time spent starting the server first (0 when it was running).
    pub start_ms: u32,
    /// The level used (the user's, or Light when it is None).
    pub level: crate::fork::settings::CleanupLevel,
    pub outcome: super::post_process::CleanupOutcome,
    pub why: Option<super::post_process::CleanupWhy>,
    /// Guard checks that rejected the reply.
    pub guard_reasons: Vec<String>,
    /// The engine that cleaned it (S-12b: Apple's model when it stands in
    /// for the local one; `none` when no request was made).
    pub engine: super::post_process::CleanupEngine,
}

/// `fork_llm_try` error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", content = "message")]
pub enum ForkLlmError {
    /// No model yet, unsupported Mac or missing runtime.
    NotReady,
    /// Not enough free memory (R5 §4.5 headroom rule).
    LowMemory,
    /// The server could not start or the request failed (log text).
    Failed(String),
    /// The model replied with something that is not the expected JSON.
    BadReply,
}

struct ModelState {
    state: LlmState,
    error: Option<String>,
    path: Option<PathBuf>,
}

/// Managed state (`app.manage`).
pub struct LlmRuntime {
    model: Option<&'static LlmModel>,
    supervisor: Option<Arc<Supervisor>>,
    llm_dir: PathBuf,
    state: Mutex<ModelState>,
    progress: Arc<ProgressCounter>,
    planner: Mutex<LoadPlanner>,
    downloading: AtomicBool,
    download_cancel: Mutex<CancellationToken>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl LlmRuntime {
    /// Builds the runtime; nothing starts until [`on_ready`].
    pub fn new(
        model: Option<&'static LlmModel>,
        sidecar: Option<PathBuf>,
        llm_dir: PathBuf,
        launcher: Arc<dyn super::supervisor::Launcher>,
        publish_transport: bool,
    ) -> Self {
        let initial = match (model, &sidecar) {
            (None, _) => LlmState::Unsupported,
            (Some(_), None) => LlmState::MissingRuntime,
            (Some(_), Some(_)) => LlmState::WaitingForSpeechModel,
        };
        let supervisor = match (model, sidecar) {
            (Some(_), Some(bin)) => {
                let mut config = Config::new(bin, llm_dir.clone());
                config.publish_transport = publish_transport;
                Some(Supervisor::new(launcher, config))
            }
            _ => None,
        };
        Self {
            model,
            supervisor,
            llm_dir,
            state: Mutex::new(ModelState {
                state: initial,
                error: None,
                path: None,
            }),
            progress: Arc::new(ProgressCounter::default()),
            planner: Mutex::new(LoadPlanner::default()),
            downloading: AtomicBool::new(false),
            download_cancel: Mutex::new(CancellationToken::new()),
        }
    }

    pub fn supervisor(&self) -> Option<&Arc<Supervisor>> {
        self.supervisor.as_ref()
    }

    /// Current status.
    pub fn status(&self) -> ForkLlmStatus {
        let s = lock(&self.state);
        ForkLlmStatus {
            state: s.state,
            model_name: self.model.map(|m| m.display_name.to_string()),
            downloaded_bytes: self.progress.done.load(Ordering::Relaxed),
            total_bytes: self
                .model
                .map(|m| m.size_bytes)
                .unwrap_or_default()
                .max(self.progress.total.load(Ordering::Relaxed)),
            server: self
                .supervisor
                .as_ref()
                .map(|s| s.status())
                .unwrap_or_default(),
            error: s.error.clone(),
        }
    }

    fn set_state(&self, state: LlmState, error: Option<String>) {
        let mut s = lock(&self.state);
        s.state = state;
        s.error = error;
    }

    /// `true` when the model is verified and the server can be started.
    pub fn is_ready(&self) -> bool {
        lock(&self.state).state == LlmState::Ready
    }

    /// R5 §4.5 memory rule for starting the server now. A running server
    /// always passes.
    pub async fn memory_allows_start(&self) -> bool {
        let (Some(model), Some(sup)) = (self.model, &self.supervisor) else {
            return false;
        };
        if sup.is_running().await {
            return true;
        }
        match memory::snapshot() {
            Some(m) => memory::may_start(m.available_bytes, model.resident_mib * 1024 * 1024),
            None => true,
        }
    }
}

/// The runtime for this app (state + supervisor), or `None` in headless
/// mode.
pub fn runtime<R: Runtime>(app: &AppHandle<R>) -> Option<tauri::State<'_, LlmRuntime>> {
    app.try_state::<LlmRuntime>()
}

fn emit<R: Runtime>(app: &AppHandle<R>) {
    if let Some(rt) = runtime(app) {
        if let Err(e) = rt.status().emit(app) {
            log::debug!("llm: status event: {e}");
        }
    }
}

/// App data dir (portable-aware).
fn data_dir<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    crate::portable::data_dir()
        .cloned()
        .or_else(|| app.path().app_data_dir().ok())
}

/// Plugin `setup`: registers the state. Off (Unsupported) in plain Handy
/// builds, where `HF_HOME` is not under our app data and nothing may be
/// downloaded on Handy's behalf.
pub fn register<R: Runtime>(app: &AppHandle<R>) {
    let enabled = crate::fork::is_product_build() || crate::portable::is_portable();
    let model = if enabled {
        memory::snapshot().and_then(|m| catalog::select(m.physical_bytes, catalog::Cpu::current()))
    } else {
        None
    };
    let llm_dir = data_dir(app).unwrap_or_default().join(LLM_DIR);
    app.manage(LlmRuntime::new(
        model,
        super::process::sidecar_path(),
        llm_dir,
        Arc::new(ProcessLauncher),
        true,
    ));
}

/// `fork::on_ready`: stale-PID kill, event wiring, background download.
pub fn on_ready<R: Runtime>(app: &AppHandle<R>) {
    let Some(rt) = runtime(app) else { return };
    if let Some(pid) = super::process::kill_stale(&rt.llm_dir) {
        log::warn!("llm: killed a llama-server left by an earlier run (pid {pid})");
    }
    let state = rt.status().state;
    log::info!(
        "llm: model {}, state {state:?}",
        rt.model.map(|m| m.id).unwrap_or("none")
    );
    let Some(sup) = rt.supervisor.clone() else {
        return;
    };

    // Status → event.
    {
        let app = app.clone();
        let mut rx = sup.subscribe();
        tauri::async_runtime::spawn(async move {
            while rx.changed().await.is_ok() {
                emit(&app);
            }
        });
    }
    // Idle owner.
    {
        let sup = sup.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                sup.idle_tick().await;
            }
        });
    }
    // Ordered prewarm.
    {
        let app2 = app.clone();
        app.listen_any("recording-state-event", move |event| {
            let phase = serde_json::from_str::<serde_json::Value>(event.payload())
                .ok()
                .and_then(|v| v.get("phase")?.as_str().map(str::to_owned));
            on_recording_phase(&app2, phase.as_deref());
        });
        let app3 = app.clone();
        app.listen_any("model-state-changed", move |event| {
            let kind = serde_json::from_str::<serde_json::Value>(event.payload())
                .ok()
                .and_then(|v| v.get("event_type")?.as_str().map(str::to_owned));
            if let Some(state) = kind.as_deref().and_then(memory::asr_state_from_event) {
                on_asr_state(&app3, state);
            }
        });
    }
    // A finished speech-model download may unblock ours.
    {
        let app4 = app.clone();
        app.listen_any("model-download-complete", move |_| start_download(&app4));
    }
    // Wake from sleep.
    #[cfg(all(target_os = "macos", not(test)))]
    {
        let app5 = app.clone();
        crate::fork::insert::session::mac::observe(move |s| {
            if s == crate::fork::insert::session::Signal::End(
                crate::fork::insert::session::Interruption::Sleep,
            ) {
                let app = app5.clone();
                tauri::async_runtime::spawn(async move { on_wake(&app).await });
            }
        });
    }
    start_download(app);
    llm_selfcheck(app);
}

/// Env var that runs the AI-engine self-check (S-10 Mac test script).
pub const LLM_SELFCHECK_ENV: &str = "WISPRBRO_SELFCHECK_LLM";

/// With `WISPRBRO_SELFCHECK_LLM=1`: waits (up to 30 min) for the model to be
/// ready, then runs "Try it" twice on a fixed sample (the second run is warm)
/// and logs `fork llm selfcheck: PASS|FAIL …`. The sample is ours, never
/// user text.
fn llm_selfcheck<R: Runtime>(app: &AppHandle<R>) {
    if !crate::utils::env_flag_enabled(LLM_SELFCHECK_ENV) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        const SAMPLE: &str = "um so I was thinking we could uh meet on friday no wait thursday";
        let t0 = Instant::now();
        loop {
            let Some(rt) = runtime(&app) else { return };
            let st = rt.status();
            match st.state {
                LlmState::Ready => break,
                LlmState::Unsupported | LlmState::MissingRuntime | LlmState::DownloadFailed => {
                    log::error!(
                        "fork llm selfcheck: FAIL state {:?} ({:?})",
                        st.state,
                        st.error
                    );
                    return;
                }
                _ => {}
            }
            if t0.elapsed() > Duration::from_secs(1800) {
                log::error!("fork llm selfcheck: FAIL model not ready after 30 min");
                return;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        log::info!("fork llm selfcheck: model ready after {:?}", t0.elapsed());
        for run in ["cold", "warm"] {
            match try_cleanup(&app, SAMPLE).await {
                Ok(r) => log::info!(
                    "fork llm selfcheck: PASS {run}: {:?} (cleanup {} ms, start {} ms)",
                    r.cleaned,
                    r.cleanup_ms,
                    r.start_ms
                ),
                Err(e) => {
                    log::error!("fork llm selfcheck: FAIL {run}: {e:?}");
                    return;
                }
            }
        }
    });
}

/// `RunEvent::Exit`: stop the server synchronously.
pub fn on_exit<R: Runtime>(app: &AppHandle<R>) {
    if let Some(rt) = runtime(app) {
        if let Some(s) = rt.supervisor() {
            s.stop_blocking();
        }
        lock(&rt.download_cancel).cancel();
    }
}

fn asr_loaded<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.try_state::<Arc<crate::managers::transcription::TranscriptionManager>>()
        .is_some_and(|t| t.is_model_loaded())
}

fn has_speech_model<R: Runtime>(app: &AppHandle<R>) -> bool {
    use crate::fork::commands::HandyModels;
    app.try_state::<Arc<crate::fork::HandyModelManager>>()
        .is_some_and(|m| !m.model_status().downloaded_model_ids.is_empty())
}

fn on_recording_phase<R: Runtime>(app: &AppHandle<R>, phase: Option<&str>) {
    let Some(rt) = runtime(app) else { return };
    if phase == Some("started") {
        // S-12b: Apple's model too, when it would clean this take.
        super::post_process::prewarm_apple(app);
    }
    let action = {
        let mut p = lock(&rt.planner);
        match phase {
            Some("started") => p.on_key_down(asr_loaded(app)),
            Some("stopped") => p.on_key_up(),
            Some("cancelled") => {
                p.on_cancel();
                PlanAction::Wait
            }
            _ => PlanAction::Wait,
        }
    };
    if action == PlanAction::StartLlm {
        prewarm(app);
    }
}

fn on_asr_state<R: Runtime>(app: &AppHandle<R>, state: AsrState) {
    let Some(rt) = runtime(app) else { return };
    if lock(&rt.planner).on_asr(state) == PlanAction::StartLlm {
        prewarm(app);
    }
}

/// Wakes the server in the background if the model is ready and memory
/// allows it (D-211).
pub fn prewarm<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(rt) = runtime(&app) else { return };
        if !rt.is_ready() {
            return;
        }
        let Some(sup) = rt.supervisor.clone() else {
            return;
        };
        if !rt.memory_allows_start().await {
            log::info!("llm: not enough free memory to start the LLM; this take skips cleanup");
            return;
        }
        let t0 = Instant::now();
        match sup.ensure_ready().await {
            Ok(_) => log::debug!("llm: prewarm ready after {:?}", t0.elapsed()),
            Err(e) => log::warn!("llm: prewarm failed: {e}"),
        }
    });
}

#[cfg_attr(any(test, not(target_os = "macos")), allow(dead_code))]
async fn on_wake<R: Runtime>(app: &AppHandle<R>) {
    let Some(rt) = runtime(app) else { return };
    if let Some(sup) = rt.supervisor.clone() {
        sup.on_wake().await;
    }
    // A download cut by the sleep is retried by its own loop; a failed one is
    // restarted here.
    if lock(&rt.state).state == LlmState::DownloadFailed {
        start_download(app);
    }
}

/// Debug builds only (S-12b Mac test): the cleanup model is never checked
/// or downloaded, so the app runs as if it were still downloading and Apple's
/// model stands in (RS-08 fallback role).
#[cfg(debug_assertions)]
pub const HOLD_MODEL_ENV: &str = "WB_DEBUG_LLM_HOLD_MODEL";

/// Starts the model check/download once (idempotent while one runs).
pub fn start_download<R: Runtime>(app: &AppHandle<R>) {
    let Some(rt) = runtime(app) else { return };
    #[cfg(debug_assertions)]
    if crate::utils::env_flag_enabled(HOLD_MODEL_ENV) {
        log::info!("llm: {HOLD_MODEL_ENV} is set; the cleanup model stays not ready");
        return;
    }
    let Some(model) = rt.model else { return };
    if rt.supervisor.is_none() || rt.is_ready() {
        return;
    }
    if !has_speech_model(app) {
        rt.set_state(LlmState::WaitingForSpeechModel, None);
        emit(app);
        return;
    }
    if rt.downloading.swap(true, Ordering::AcqRel) {
        return;
    }
    let cancel = {
        let mut c = lock(&rt.download_cancel);
        if c.is_cancelled() {
            *c = CancellationToken::new();
        }
        c.clone()
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(rt) = runtime(&app) else { return };
        rt.set_state(LlmState::Verifying, None);
        emit(&app);
        // Progress events while the download runs.
        let ticker = {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    if let Some(rt) = runtime(&app) {
                        let st = rt.status();
                        if st.state == LlmState::Verifying && st.downloaded_bytes > 0 {
                            rt.set_state(LlmState::Downloading, None);
                        }
                    }
                    emit(&app);
                }
            })
        };
        let t0 = Instant::now();
        let result = download::ensure(
            model,
            download::hub_dir(),
            rt.llm_dir.join("verified"),
            rt.progress.clone(),
            cancel,
        )
        .await;
        ticker.abort();
        match result {
            Ok(path) => {
                log::info!("llm: {} ready ({:?})", model.id, t0.elapsed());
                lock(&rt.state).path = Some(path.clone());
                if let Some(sup) = &rt.supervisor {
                    sup.set_model(Some(path)).await;
                }
                rt.set_state(LlmState::Ready, None);
                emit(&app);
                first_launch_warmup(&app).await;
            }
            Err(DownloadError::Cancelled) => {
                rt.set_state(LlmState::WaitingForSpeechModel, None);
            }
            Err(e) => {
                log::warn!("llm: download of {} failed: {e}", model.id);
                rt.set_state(LlmState::DownloadFailed, Some(e.to_string()));
            }
        }
        rt.downloading.store(false, Ordering::Release);
        emit(&app);
    });
}

/// Name of the marker that records a warmed-up sidecar binary.
fn warm_marker_name(binary: &std::path::Path) -> Option<String> {
    let meta = std::fs::metadata(binary).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(format!("warm-{}-{mtime}", meta.len()))
}

/// RS-06: the first launch of a newly installed (or updated) sidecar takes
/// ~3.5 s more on an M5 Max, because Metal compiles the embedded shader
/// library and caches it per binary. Pay that once in the background, right
/// after the model is ready, instead of in the user's first take. Skipped
/// when memory is short; the server is stopped again at once.
async fn first_launch_warmup<R: Runtime>(app: &AppHandle<R>) {
    let Some(rt) = runtime(app) else { return };
    let Some(sup) = rt.supervisor.clone() else {
        return;
    };
    let Some(marker) = sup.binary().and_then(|b| warm_marker_name(&b)) else {
        return;
    };
    let path = rt.llm_dir.join(marker);
    if path.exists() || sup.is_running().await || !rt.memory_allows_start().await {
        return;
    }
    let t0 = Instant::now();
    match sup.ensure_ready().await {
        Ok(_) => {
            log::info!("llm: first-launch warm-up took {:?}", t0.elapsed());
            if let Ok(entries) = std::fs::read_dir(&rt.llm_dir) {
                for e in entries.flatten() {
                    if e.file_name().to_string_lossy().starts_with("warm-") {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
            }
            let _ = std::fs::write(&path, b"");
            sup.stop().await;
        }
        Err(e) => log::warn!("llm: first-launch warm-up failed: {e}"),
    }
}

/// "Try it": cleans `text` the way a dictation would be cleaned (the user's
/// level, Light when it is None; the gate, guards and deadlines of
/// [`super::post_process::clean`]), with the server started first.
pub async fn try_cleanup<R: Runtime>(
    app: &AppHandle<R>,
    text: &str,
) -> Result<ForkLlmTryResult, ForkLlmError> {
    use super::apple;
    use super::post_process::{clean, Cleaned, CleanupEngine, LocalEngine, Request};
    use crate::fork::settings::CleanupLevel;
    let rt = runtime(app).ok_or(ForkLlmError::NotReady)?;
    let level = match app
        .try_state::<crate::fork::settings::ForkSettingsHandle>()
        .and_then(|h| h.get(app).ok())
        .map(|s| s.cleanup_level)
    {
        Some(CleanupLevel::Medium) => CleanupLevel::Medium,
        _ => CleanupLevel::Light,
    };
    let req = Request {
        text,
        level,
        language: None,
        hint: None,
        terms: &[],
        keep: &[],
        style: None,
        context: &[],
    };
    let local_ready = rt.is_ready() && rt.supervisor.is_some();
    let memory_ok = local_ready && rt.memory_allows_start().await;
    let (skip, engine) = super::post_process::choose(
        apple::ROLE,
        false,
        local_ready,
        memory_ok,
        apple::usable_for(None),
    );
    let (cleaned, start_ms): (Cleaned, u32) = match (skip, engine) {
        (_, CleanupEngine::Apple) => {
            let engine = apple::AppleEngine {
                pill: false,
                ..apple::AppleEngine::for_dictation()
            };
            (clean(&engine, &req).await, 0)
        }
        (_, CleanupEngine::Local) => {
            let sup = rt.supervisor.clone().ok_or(ForkLlmError::NotReady)?;
            let t0 = Instant::now();
            let was_ready = sup.status().phase == ServerPhase::Ready && sup.is_running().await;
            sup.ensure_ready().await.map_err(|e| match e {
                StartError::NoModel => ForkLlmError::NotReady,
                e => ForkLlmError::Failed(e.to_string()),
            })?;
            let start_ms = if was_ready {
                0
            } else {
                u32::try_from(t0.elapsed().as_millis()).unwrap_or(u32::MAX)
            };
            let engine = LocalEngine {
                supervisor: &sup,
                pill: false,
            };
            (clean(&engine, &req).await, start_ms)
        }
        (Some(super::post_process::CleanupWhy::LowMemory), _) => {
            return Err(ForkLlmError::LowMemory)
        }
        _ => return Err(ForkLlmError::NotReady),
    };
    if let Some(e) = cleaned.chunks.iter().find_map(|c| c.error.clone()) {
        return Err(ForkLlmError::Failed(e));
    }
    Ok(ForkLlmTryResult {
        guard_reasons: cleaned.guard_reasons(),
        engine: if cleaned.chunks.is_empty() {
            CleanupEngine::None
        } else {
            engine
        },
        cleaned: cleaned.text,
        cleanup_ms: cleaned.llm_ms,
        start_ms,
        level,
        outcome: cleaned.outcome,
        why: cleaned.why,
    })
}

/// Retry after a failed download.
pub fn retry<R: Runtime>(app: &AppHandle<R>) {
    if let Some(rt) = runtime(app) {
        if rt.status().state == LlmState::DownloadFailed {
            rt.set_state(LlmState::WaitingForSpeechModel, None);
        }
    }
    start_download(app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::llm::catalog::MUESLI_2B_V12;
    use crate::fork::llm::supervisor::Launcher;

    struct NoLaunch;
    impl Launcher for NoLaunch {
        fn launch(
            &self,
            _: &crate::fork::llm::process::SpawnSpec,
            _: Box<dyn FnOnce() + Send>,
        ) -> std::io::Result<Box<dyn crate::fork::llm::supervisor::Server>> {
            Err(std::io::Error::other("no launch in this test"))
        }
    }

    #[test]
    fn initial_states() {
        let dir = std::path::PathBuf::from("/tmp/wb-llm-test");
        let unsupported = LlmRuntime::new(
            None,
            Some("/x".into()),
            dir.clone(),
            Arc::new(NoLaunch),
            false,
        );
        assert_eq!(unsupported.status().state, LlmState::Unsupported);
        assert!(unsupported.supervisor().is_none());
        let missing = LlmRuntime::new(
            Some(&MUESLI_2B_V12),
            None,
            dir.clone(),
            Arc::new(NoLaunch),
            false,
        );
        assert_eq!(missing.status().state, LlmState::MissingRuntime);
        let ok = LlmRuntime::new(
            Some(&MUESLI_2B_V12),
            Some("/x".into()),
            dir,
            Arc::new(NoLaunch),
            false,
        );
        let st = ok.status();
        assert_eq!(st.state, LlmState::WaitingForSpeechModel);
        assert_eq!(st.model_name.as_deref(), Some("Muesli 2B v12"));
        assert_eq!(st.total_bytes, MUESLI_2B_V12.size_bytes);
        assert_eq!(st.server.phase, ServerPhase::Stopped);
    }

    #[test]
    fn handy_event_names_are_the_ones_we_listen_to() {
        let coordinator = include_str!("../../transcription_coordinator.rs");
        let actions = include_str!("../../actions.rs");
        let model = include_str!("../../managers/model.rs");
        let transcription = include_str!("../../managers/transcription.rs");
        let _ = actions;
        assert!(coordinator.contains("pub struct RecordingStateEvent"));
        assert!(coordinator.contains("Started,") && coordinator.contains("Stopped,"));
        assert!(transcription.contains("\"model-state-changed\""));
        assert!(model.contains("\"model-download-complete\""));
    }
}
