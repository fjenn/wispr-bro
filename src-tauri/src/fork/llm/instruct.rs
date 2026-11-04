// SPDX-License-Identifier: GPL-3.0-or-later
//! The instruction model (OT2-H): a small general instruct model for custom
//! transforms and Command Mode. Dictation cleanup
//! stays on the cleanup model ([`super::runtime`]); the cleanup fine-tune
//! answers any instruction with a cleaned copy of its input (OT2-D).
//!
//! - **On demand.** Nothing is downloaded until a custom transform is created
//!   or used, or Command Mode is turned on (Polish and Prompt Engineer stay
//!   on the cleanup model); a file already on disk is verified at launch.
//! - **Its own server.** A second [`Supervisor`] in `<app data>/llm-instruct`
//!   (socket, PID file, markers) runs the same bundled `llama-server`; it is
//!   stopped after the same idle timeout as the cleanup server, never
//!   prewarmed by a dictation, and never publishes Handy's `local`
//!   transport.
//! - **RAM.** Before it starts, the memory rule is checked for its size; when
//!   it does not fit, an idle cleanup server is stopped first (it starts
//!   again at the next take), so both are only resident together when there
//!   is room.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};
use tauri_specta::Event;

use super::catalog::{self, LlmModel};
use super::chat::CancellationToken;
use super::download::{self, Check, DownloadError, ProgressCounter};
use super::memory;
use super::supervisor::{Config, ProcessLauncher, Supervisor};

/// Sub-folder of the app data dir: socket, PID file, verification markers.
pub const INSTRUCT_DIR: &str = "llm-instruct";

/// Where the instruction model is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum InstructState {
    /// No local LLM on this Mac (Intel, under 8 GB, or a plain Handy build).
    Unsupported,
    /// The `llama-server` sidecar is not in this build.
    MissingRuntime,
    /// Not on disk; downloaded when a transform first needs it.
    NotDownloaded,
    Downloading,
    Verifying,
    Ready,
    /// The download failed (offline, a server error); Retry is offered and
    /// the next transform tries again.
    DownloadFailed,
}

/// `fork_instruct_status` reply and event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkInstructStatus {
    pub state: InstructState,
    pub model_name: Option<String>,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    /// Last download error (no user data).
    pub error: Option<String>,
}

/// Managed state.
pub struct InstructRuntime {
    model: Option<&'static LlmModel>,
    supervisor: Option<Arc<Supervisor>>,
    dir: PathBuf,
    state: Mutex<(InstructState, Option<String>)>,
    progress: Arc<ProgressCounter>,
    downloading: AtomicBool,
    cancel: Mutex<CancellationToken>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl InstructRuntime {
    pub fn new(
        model: Option<&'static LlmModel>,
        sidecar: Option<PathBuf>,
        dir: PathBuf,
        launcher: Arc<dyn super::supervisor::Launcher>,
    ) -> Self {
        let state = match (model, &sidecar) {
            (None, _) => InstructState::Unsupported,
            (Some(_), None) => InstructState::MissingRuntime,
            (Some(_), Some(_)) => InstructState::NotDownloaded,
        };
        let supervisor = match (model, sidecar) {
            (Some(_), Some(bin)) => Some(Supervisor::new(launcher, Config::new(bin, dir.clone()))),
            _ => None,
        };
        Self {
            model,
            supervisor,
            dir,
            state: Mutex::new((state, None)),
            progress: Arc::new(ProgressCounter::default()),
            downloading: AtomicBool::new(false),
            cancel: Mutex::new(CancellationToken::new()),
        }
    }

    pub fn status(&self) -> ForkInstructStatus {
        let (state, error) = lock(&self.state).clone();
        ForkInstructStatus {
            state,
            model_name: self.model.map(|m| m.display_name.to_owned()),
            downloaded_bytes: self.progress.done.load(Ordering::Relaxed),
            total_bytes: self
                .model
                .map(|m| m.size_bytes)
                .unwrap_or_default()
                .max(self.progress.total.load(Ordering::Relaxed)),
            error,
        }
    }

    fn set(&self, state: InstructState, error: Option<String>) {
        *lock(&self.state) = (state, error);
    }

    pub fn is_ready(&self) -> bool {
        lock(&self.state).0 == InstructState::Ready
    }

    pub fn supervisor(&self) -> Option<&Arc<Supervisor>> {
        self.supervisor.as_ref()
    }
}

pub fn runtime<R: Runtime>(app: &AppHandle<R>) -> Option<tauri::State<'_, InstructRuntime>> {
    app.try_state::<InstructRuntime>()
}

fn emit<R: Runtime>(app: &AppHandle<R>) {
    if let Some(rt) = runtime(app) {
        if let Err(e) = rt.status().emit(app) {
            log::debug!("instruct: status event: {e}");
        }
    }
}

/// Plugin `setup`: registers the state (same gate as the cleanup model).
pub fn register<R: Runtime>(app: &AppHandle<R>) {
    let enabled = crate::fork::is_product_build() || crate::portable::is_portable();
    let model = enabled
        .then(|| {
            memory::snapshot()
                .and_then(|m| catalog::select_instruct(m.physical_bytes, catalog::Cpu::current()))
        })
        .flatten();
    let dir = crate::portable::data_dir()
        .cloned()
        .or_else(|| app.path().app_data_dir().ok())
        .unwrap_or_default()
        .join(INSTRUCT_DIR);
    app.manage(InstructRuntime::new(
        model,
        super::process::sidecar_path(),
        dir,
        Arc::new(ProcessLauncher),
    ));
}

/// `true` when something needs the instruction model: Command Mode is on,
/// or Transforms is on with a custom transform. The built-ins run on the
/// cleanup model, so people who only use them never download it.
pub fn wanted<R: Runtime>(app: &AppHandle<R>) -> bool {
    let Some(settings) = app
        .try_state::<crate::fork::settings::ForkSettingsHandle>()
        .and_then(|h| h.get(app).ok())
    else {
        return false;
    };
    settings.command_mode_enabled || (settings.transforms_enabled && has_custom(app))
}

fn has_custom<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.try_state::<crate::fork::db::ForkDb>()
        .and_then(|db| {
            db.with_conn(|conn| crate::fork::transforms::store::list(conn))
                .ok()
        })
        .is_some_and(|entries| entries.iter().any(|e| e.builtin_key.is_none()))
}

/// `fork::on_ready`: stale-PID kill, idle owner, and the model: used at once
/// when it is on disk, downloaded when Transforms or Command Mode is on.
pub fn on_ready<R: Runtime>(app: &AppHandle<R>) {
    let Some(rt) = runtime(app) else { return };
    if let Some(pid) = super::process::kill_stale(&rt.dir) {
        log::warn!("instruct: killed a llama-server left by an earlier run (pid {pid})");
    }
    let Some(sup) = rt.supervisor.clone() else {
        return;
    };
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            if memory::release_idle(memory::snapshot()) && sup.stop_if_idle().await {
                log::info!("instruct: memory pressure is critical; stopped the idle server");
                continue;
            }
            sup.idle_tick().await;
        }
    });
    let (Some(model), app) = (rt.model, app.clone()) else {
        return;
    };
    let dir = rt.dir.join("verified");
    tauri::async_runtime::spawn(async move {
        let hub = download::hub_dir();
        let cached = tokio::task::spawn_blocking(move || {
            download::check_cached(&hub, &dir, model, &CancellationToken::new())
        })
        .await
        .ok()
        .and_then(Result::ok);
        if cached == Some(Check::Ok) || wanted(&app) {
            ensure_downloaded(&app);
        }
    });
}

/// Starts the check/download once (idempotent while one runs): Transforms
/// or Command Mode was turned on, a transform needs the model, or Retry.
pub fn ensure_downloaded<R: Runtime>(app: &AppHandle<R>) {
    let Some(rt) = runtime(app) else { return };
    let Some(model) = rt.model else { return };
    if rt.supervisor.is_none() || rt.is_ready() || rt.downloading.swap(true, Ordering::AcqRel) {
        return;
    }
    let cancel = {
        let mut c = lock(&rt.cancel);
        if c.is_cancelled() {
            *c = CancellationToken::new();
        }
        c.clone()
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(rt) = runtime(&app) else { return };
        rt.set(InstructState::Verifying, None);
        emit(&app);
        let ticker = {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    if let Some(rt) = runtime(&app) {
                        let st = rt.status();
                        if st.state == InstructState::Verifying && st.downloaded_bytes > 0 {
                            rt.set(InstructState::Downloading, None);
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
            rt.dir.join("verified"),
            rt.progress.clone(),
            cancel,
        )
        .await;
        ticker.abort();
        match result {
            Ok(path) => {
                log::info!("instruct: {} ready ({:?})", model.id, t0.elapsed());
                if let Some(sup) = &rt.supervisor {
                    sup.set_model(Some(path)).await;
                }
                rt.set(InstructState::Ready, None);
            }
            Err(DownloadError::Cancelled) => rt.set(InstructState::NotDownloaded, None),
            Err(e) => {
                log::warn!("instruct: download of {} failed: {e}", model.id);
                rt.set(InstructState::DownloadFailed, Some(e.to_string()));
            }
        }
        rt.downloading.store(false, Ordering::Release);
        emit(&app);
    });
}

/// The instruction model's server when it can be used now: verified and the
/// memory rule allows it (an idle cleanup server is stopped to make room).
/// When it is not on disk yet, the download starts and `None` comes back.
pub async fn ready_supervisor<R: Runtime>(app: &AppHandle<R>) -> Option<Arc<Supervisor>> {
    let rt = runtime(app)?;
    if !rt.is_ready() {
        ensure_downloaded(app);
        return None;
    }
    let (model, sup) = (rt.model?, rt.supervisor.clone()?);
    let resident = model.resident_mib * 1024 * 1024;
    let fits = |running| memory::allows(running, memory::snapshot(), resident);
    if fits(sup.is_running().await) {
        return Some(sup);
    }
    let cleanup = super::runtime::runtime(app).and_then(|c| c.supervisor().cloned());
    if let Some(cleanup) = cleanup {
        if cleanup.stop_if_idle().await {
            log::info!("instruct: stopped the idle cleanup server to make room");
            if fits(false) {
                return Some(sup);
            }
        }
    }
    log::info!("instruct: not enough free memory to start the instruction model");
    None
}

/// `RunEvent::Exit`: stop the server and any download.
pub fn on_exit<R: Runtime>(app: &AppHandle<R>) {
    if let Some(rt) = runtime(app) {
        if let Some(s) = rt.supervisor() {
            s.stop_blocking();
        }
        lock(&rt.cancel).cancel();
    }
}

/// Retry after a failed download.
pub fn retry<R: Runtime>(app: &AppHandle<R>) {
    if let Some(rt) = runtime(app) {
        if rt.status().state == InstructState::DownloadFailed {
            rt.set(InstructState::NotDownloaded, None);
        }
    }
    ensure_downloaded(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoLaunch;
    impl super::super::supervisor::Launcher for NoLaunch {
        fn launch(
            &self,
            _: &super::super::process::SpawnSpec,
            _: Box<dyn FnOnce() + Send>,
        ) -> std::io::Result<Box<dyn super::super::supervisor::Server>> {
            Err(std::io::Error::other("no launch in this test"))
        }
    }

    #[test]
    fn initial_states() {
        let dir = PathBuf::from("/tmp/wb-instruct-test");
        let none = InstructRuntime::new(None, Some("/x".into()), dir.clone(), Arc::new(NoLaunch));
        assert_eq!(none.status().state, InstructState::Unsupported);
        assert!(none.supervisor().is_none());
        let missing = InstructRuntime::new(
            Some(&catalog::INSTRUCT),
            None,
            dir.clone(),
            Arc::new(NoLaunch),
        );
        assert_eq!(missing.status().state, InstructState::MissingRuntime);
        let ok = InstructRuntime::new(
            Some(&catalog::INSTRUCT),
            Some("/x".into()),
            dir,
            Arc::new(NoLaunch),
        );
        let st = ok.status();
        assert_eq!(st.state, InstructState::NotDownloaded);
        assert_eq!(st.total_bytes, catalog::INSTRUCT.size_bytes);
        assert!(!ok.is_ready());
    }

    #[test]
    fn its_server_lives_apart_from_the_cleanup_server() {
        assert_ne!(INSTRUCT_DIR, super::super::runtime::LLM_DIR);
    }
}
