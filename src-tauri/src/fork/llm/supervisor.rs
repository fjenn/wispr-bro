// SPDX-License-Identifier: GPL-3.0-or-later
//! The `llama-server` supervisor (R5 §4.5, W-01).
//!
//! Rust port of the lifecycle in TypeWhisper's `LlamaCppServer.swift`
//! (GPL-3.0-or-later; docs/fork/PORTS.md), with R5's additions:
//!
//! - **One start at a time.** A start holds the state lock, so a prewarm at
//!   key-down and the take's request share one server. It runs as its own
//!   task: a caller that gives up (a take's start budget, a transform's
//!   deadline) only stops waiting, the start finishes for the next request
//!   and no half-started process is left behind.
//! - **Never blocks a take on a start.** [`Supervisor::is_running`] does not
//!   wait for the lock: a start in progress counts as running (Mac wave 3: a
//!   take's memory check waited 9 s behind the launch warm-up's start).
//! - **Warm-up yields to takes.** [`Supervisor::warm_up`] (RS-06 first
//!   launch) stops its server afterwards only when nothing asked for it
//!   meanwhile; a take that started during the warm-up keeps it.
//! - **Failures.** A failed start is retried once at once ("restart once");
//!   a second failure on the GPU switches to the CPU (`-ngl 0`) for one more
//!   try; after that the supervisor backs off (30 s, doubling, at most
//!   10 min). A CPU fallback caused only by `/health` timeouts (a slow first
//!   exec: syspolicyd held a copied sidecar 58 s in Mac wave 3) tries the
//!   GPU again once at the next fresh start; one caused by the server
//!   exiting stays on the CPU until the app quits. Every launch starts on
//!   the GPU. A lost port race (`couldn't bind`) retries with a new port and
//!   does not count. A server that crashes after it was ready is restarted
//!   once in the background; a second crash within a minute waits for the
//!   next demand and the backoff.
//! - **One idle owner.** [`Supervisor::idle_tick`] stops the server after
//!   [`IDLE_TIMEOUT`] with no request in flight, nobody waiting for it and
//!   no take running ([`Supervisor::take_started`]; R5 §4.5; no
//!   `--sleep-idle-seconds`).
//! - **Wake.** [`Supervisor::on_wake`] probes `/health` and restarts a server
//!   that does not answer.
//! - **Abort.** A request's [`CancellationToken`] closes its connection
//!   (`chat::complete`), which frees the slot.
//!
//! The process side (spawn, watchdog, PID file) is behind [`Launcher`] so the
//! state machine is tested with a fake server that speaks HTTP.

use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::sync::watch;

use super::chat::{self, CancellationToken, ChatError, ChatReply, ChatRequest};
use super::health::{self, Health, WaitError};
use super::process::{self, Device, OutputTail, SpawnSpec};
use super::transport::{self, Endpoint, Transport};

/// R5 §4.5: the supervisor stops the sidecar after 120 s without a request.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(120);
/// Longest wait for `/health` after spawn (a cold model load on an 8 GB Mac
/// with a cold disk cache takes seconds; a hang must not block forever).
pub const HEALTH_TIMEOUT: Duration = Duration::from_secs(60);
/// A server not heard from for this long is probed before use.
pub const PROBE_AFTER: Duration = Duration::from_secs(30);
/// First backoff after repeated failures.
pub const BACKOFF_BASE: Duration = Duration::from_secs(30);
/// Longest backoff.
pub const BACKOFF_MAX: Duration = Duration::from_secs(600);
/// Two crashes this close together stop the automatic restart.
pub const CRASH_WINDOW: Duration = Duration::from_secs(60);
/// How a start that timed out waiting for `/health` is reported.
const TIMEOUT_PREFIX: &str = "no /health";
/// Port races retried per start.
const BIND_RETRIES: u32 = 3;
/// A take that never reported its end stops holding the server after this.
pub const TAKE_HOLD_MAX: Duration = Duration::from_secs(30 * 60);

// ---------------------------------------------------------------------------
// Process abstraction

/// A started server process.
pub trait Server: Send {
    fn pid(&self) -> u32;
    /// `true` once the process has exited (reaps it).
    fn has_exited(&mut self) -> bool;
    /// SIGTERM, then SIGKILL after the grace period; reaps.
    fn stop(&mut self);
    /// The last stderr lines.
    fn tail(&self) -> OutputTail;
}

/// Starts servers. `on_exit` must be called once when the process ends
/// (any reason); the supervisor ignores it for servers it stopped itself.
pub trait Launcher: Send + Sync + 'static {
    fn launch(
        &self,
        spec: &SpawnSpec,
        on_exit: Box<dyn FnOnce() + Send>,
    ) -> io::Result<Box<dyn Server>>;
}

// ---------------------------------------------------------------------------
// Configuration and status

/// What to run and where.
#[derive(Debug, Clone)]
pub struct Config {
    pub binary: PathBuf,
    /// The verified GGUF, once downloaded.
    pub model: Option<PathBuf>,
    /// `<app data>/llm`: socket, PID file.
    pub llm_dir: PathBuf,
    pub home: Option<PathBuf>,
    pub health_timeout: Duration,
    pub idle_timeout: Duration,
    /// Publish the running server's transport for Handy's chain
    /// (`fork::llm::set_local_transport`). Only the app's supervisor does.
    pub publish_transport: bool,
}

impl Config {
    pub fn new(binary: PathBuf, llm_dir: PathBuf) -> Self {
        Self {
            binary,
            model: None,
            llm_dir,
            home: std::env::var_os("HOME").map(PathBuf::from),
            health_timeout: HEALTH_TIMEOUT,
            idle_timeout: IDLE_TIMEOUT,
            publish_transport: false,
        }
    }
}

/// Server phase for the status event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ServerPhase {
    Stopped,
    Starting,
    Ready,
    /// Last start failed; waiting for the backoff to pass.
    Failed,
}

/// Snapshot published on every change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct ServerStatus {
    pub phase: ServerPhase,
    /// `gpu` or `cpu`.
    pub device: String,
    /// `unix` or `tcp` while running.
    pub transport: Option<String>,
    /// Spawn to `/health` OK of the current server, in ms.
    pub ready_ms: Option<u32>,
    /// Automatic restarts since launch.
    pub restarts: u32,
    /// Last failure, for the debug panel and logs (no user text).
    pub last_error: Option<String>,
}

impl Default for ServerStatus {
    fn default() -> Self {
        Self {
            phase: ServerPhase::Stopped,
            device: "gpu".into(),
            transport: None,
            ready_ms: None,
            restarts: 0,
            last_error: None,
        }
    }
}

/// Why the server is not available.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartError {
    /// No verified model yet.
    NoModel,
    /// Backing off after repeated failures.
    Backoff(Duration),
    /// The start failed (message has no user data).
    Failed(String),
}

impl std::fmt::Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StartError::NoModel => write!(f, "no model"),
            StartError::Backoff(d) => write!(f, "backing off for {} s", d.as_secs()),
            StartError::Failed(m) => write!(f, "{m}"),
        }
    }
}

/// Why a completion failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestError {
    Start(StartError),
    Chat(ChatError),
}

impl std::fmt::Display for RequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestError::Start(e) => write!(f, "not started: {e}"),
            RequestError::Chat(e) => write!(f, "{e}"),
        }
    }
}

/// A ready server's client.
#[derive(Clone)]
pub struct Handle {
    pub client: reqwest::Client,
    pub transport: Transport,
}

impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `Transport`'s Debug never prints the key.
        f.debug_struct("Handle")
            .field("transport", &self.transport)
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// Supervisor

struct Running {
    server: Box<dyn Server>,
    transport: Transport,
    client: reqwest::Client,
    generation: u64,
    last_contact: Instant,
}

struct Inner {
    running: Option<Running>,
    device: Device,
    consecutive_failures: u32,
    backoffs: u32,
    backoff_until: Option<Instant>,
    last_crash: Option<Instant>,
    /// The failures of the current start were all `/health` timeouts.
    only_timeouts: bool,
    /// The CPU fallback came from timeouts only: the next fresh start tries
    /// the GPU once more.
    retry_gpu: bool,
    /// That retry was used (once per launch).
    gpu_retried: bool,
}

/// See the module docs.
pub struct Supervisor {
    launcher: Arc<dyn Launcher>,
    config: Mutex<Config>,
    inner: tokio::sync::Mutex<Inner>,
    generation: AtomicU64,
    in_flight: AtomicUsize,
    /// Callers waiting in [`Supervisor::ensure_ready`].
    waiting: AtomicUsize,
    /// Bumped by every demand: a take starting, a prewarm, a request.
    demand: AtomicU64,
    /// When the running take started ([`Supervisor::take_started`]).
    take_since: Mutex<Option<Instant>>,
    last_used: Mutex<Instant>,
    status: watch::Sender<ServerStatus>,
    me: Weak<Supervisor>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

struct InFlight<'a>(&'a AtomicUsize);

impl Drop for InFlight<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

enum Attempt {
    Ok,
    Bind,
    Failed(String),
}

impl Supervisor {
    pub fn new(launcher: Arc<dyn Launcher>, config: Config) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            launcher,
            config: Mutex::new(config),
            inner: tokio::sync::Mutex::new(Inner {
                running: None,
                device: Device::Gpu,
                consecutive_failures: 0,
                backoffs: 0,
                backoff_until: None,
                last_crash: None,
                only_timeouts: true,
                retry_gpu: false,
                gpu_retried: false,
            }),
            generation: AtomicU64::new(0),
            in_flight: AtomicUsize::new(0),
            waiting: AtomicUsize::new(0),
            demand: AtomicU64::new(0),
            take_since: Mutex::new(None),
            last_used: Mutex::new(Instant::now()),
            status: watch::channel(ServerStatus::default()).0,
            me: me.clone(),
        })
    }

    /// Status updates.
    pub fn subscribe(&self) -> watch::Receiver<ServerStatus> {
        self.status.subscribe()
    }

    pub fn status(&self) -> ServerStatus {
        self.status.borrow().clone()
    }

    fn publish(&self, f: impl FnOnce(&mut ServerStatus)) {
        self.status.send_modify(f);
    }

    /// Updates the transport Handy's chain uses for `local`.
    fn publish_transport(&self, t: Option<&Transport>) {
        if lock(&self.config).publish_transport {
            super::set_local_transport(t.cloned());
        }
    }

    /// Sets the model to run (after download/verification). A running server
    /// with another model is stopped.
    pub async fn set_model(&self, model: Option<PathBuf>) {
        let changed = {
            let mut c = lock(&self.config);
            let changed = c.model != model;
            c.model = model;
            changed
        };
        if changed {
            self.stop().await;
        }
    }

    /// The sidecar binary.
    pub fn binary(&self) -> Option<PathBuf> {
        Some(lock(&self.config).binary.clone())
    }

    pub fn has_model(&self) -> bool {
        lock(&self.config).model.is_some()
    }

    fn touch(&self) {
        *lock(&self.last_used) = Instant::now();
    }

    /// `true` while a server process is up, or being started. Never waits
    /// for a start: while one holds the lock, the published phase answers.
    pub async fn is_running(&self) -> bool {
        match self.inner.try_lock() {
            Ok(mut inner) => match inner.running.as_mut() {
                Some(r) => !r.server.has_exited(),
                None => false,
            },
            Err(_) => matches!(
                self.status.borrow().phase,
                ServerPhase::Starting | ServerPhase::Ready
            ),
        }
    }

    /// A take started recording (Handy's `recording-state-event`): the idle
    /// owner leaves the server alone until [`Self::take_ended`], and a
    /// launch warm-up in flight keeps its server for it.
    pub fn take_started(&self) {
        self.demand.fetch_add(1, Ordering::AcqRel);
        *lock(&self.take_since) = Some(Instant::now());
        self.touch();
    }

    /// The take's recording ended or was cancelled. Its cleanup request,
    /// seconds later, is in flight while it runs; the idle timeout counts
    /// from now.
    pub fn take_ended(&self) {
        *lock(&self.take_since) = None;
        self.touch();
    }

    fn take_running(&self) -> bool {
        lock(&self.take_since).is_some_and(|t| t.elapsed() < TAKE_HOLD_MAX)
    }

    /// Something needs the server now: a request, a caller waiting for a
    /// start, or a take.
    fn wanted(&self) -> bool {
        self.in_flight.load(Ordering::Acquire) > 0
            || self.waiting.load(Ordering::Acquire) > 0
            || self.take_running()
    }

    /// PID of the running server (tests, RS-06 bench).
    pub async fn pid(&self) -> Option<u32> {
        self.inner
            .lock()
            .await
            .running
            .as_ref()
            .map(|r| r.server.pid())
    }

    /// Starts the server if needed and returns its client. The start runs
    /// as its own task: dropping this future (a deadline) stops the wait,
    /// not the start.
    pub async fn ensure_ready(&self) -> Result<Handle, StartError> {
        self.touch();
        self.demand.fetch_add(1, Ordering::AcqRel);
        self.waiting.fetch_add(1, Ordering::AcqRel);
        let _waiting = InFlight(&self.waiting);
        self.start_detached().await
    }

    /// [`Self::ensure_ready_now`] on its own task.
    async fn start_detached(&self) -> Result<Handle, StartError> {
        let (Some(me), Ok(rt)) = (self.me.upgrade(), tokio::runtime::Handle::try_current()) else {
            return self.ensure_ready_now().await;
        };
        match rt.spawn(async move { me.ensure_ready_now().await }).await {
            Ok(result) => result,
            Err(e) => Err(StartError::Failed(format!("start task: {e}"))),
        }
    }

    /// RS-06 first-launch warm-up: starts the server (paying the first
    /// Metal compile outside a take), then stops it again unless something
    /// asked for it meanwhile (a take started, a prewarm or a request came
    /// in, someone is waiting). `Ok(true)`: the server stays up for them;
    /// the idle owner stops it later. Does not count as a demand itself.
    pub async fn warm_up(&self) -> Result<bool, StartError> {
        let before = self.demand.load(Ordering::Acquire);
        self.start_detached().await?;
        let mut inner = self.inner.lock().await;
        if self.wanted() || self.demand.load(Ordering::Acquire) != before {
            log::info!("llm: warm-up done; a take wants the server, it stays up");
            return Ok(true);
        }
        self.stop_locked(&mut inner).await;
        Ok(false)
    }

    async fn ensure_ready_now(&self) -> Result<Handle, StartError> {
        let mut inner = self.inner.lock().await;
        if let Some(r) = inner.running.as_mut() {
            if !r.server.has_exited() {
                let fresh = r.last_contact.elapsed() < PROBE_AFTER;
                if fresh || health::probe(&r.client).await == Health::Ok {
                    r.last_contact = Instant::now();
                    return Ok(Handle {
                        client: r.client.clone(),
                        transport: r.transport.clone(),
                    });
                }
                log::warn!("llm: server does not answer /health; restarting");
            }
            let dead = inner.running.take();
            self.publish_transport(None);
            if let Some(mut dead) = dead {
                tokio::task::spawn_blocking(move || dead.server.stop())
                    .await
                    .ok();
            }
        }
        if let Some(until) = inner.backoff_until {
            let now = Instant::now();
            if until > now {
                return Err(StartError::Backoff(until - now));
            }
        }
        if lock(&self.config).model.is_none() {
            return Err(StartError::NoModel);
        }
        if inner.retry_gpu && inner.device == Device::Cpu {
            // The CPU fallback came from timeouts only (a slow first exec):
            // the GPU gets one more chance.
            inner.retry_gpu = false;
            inner.gpu_retried = true;
            inner.device = Device::Gpu;
            self.publish(|s| s.device = "gpu".into());
            log::info!("llm: the CPU fallback came from start timeouts; trying the GPU again");
        }
        inner.only_timeouts = true;

        let mut binds = 0;
        loop {
            match self.attempt(&mut inner).await {
                Attempt::Ok => {
                    inner.consecutive_failures = 0;
                    inner.backoffs = 0;
                    inner.backoff_until = None;
                    let r = inner.running.as_ref().expect("running after Ok");
                    return Ok(Handle {
                        client: r.client.clone(),
                        transport: r.transport.clone(),
                    });
                }
                Attempt::Bind if binds < BIND_RETRIES => {
                    binds += 1;
                    log::warn!("llm: port taken before the server bound it; new port");
                }
                Attempt::Bind => {
                    inner.only_timeouts = false;
                    if let Some(e) = self.after_failure(&mut inner, "could not bind".into()) {
                        return Err(e);
                    }
                }
                Attempt::Failed(msg) => {
                    if !msg.starts_with(TIMEOUT_PREFIX) {
                        inner.only_timeouts = false;
                    }
                    if let Some(e) = self.after_failure(&mut inner, msg) {
                        return Err(e);
                    }
                }
            }
        }
    }

    /// Bookkeeping after a failed start. `None` means "try again now".
    fn after_failure(&self, inner: &mut Inner, msg: String) -> Option<StartError> {
        inner.consecutive_failures += 1;
        log::warn!(
            "llm: start failed ({} in a row, {:?}): {msg}",
            inner.consecutive_failures,
            inner.device
        );
        match (inner.consecutive_failures, inner.device) {
            (1, _) => None, // restart once
            (_, Device::Gpu) => {
                log::warn!("llm: falling back to CPU (-ngl 0)");
                inner.device = Device::Cpu;
                inner.retry_gpu = inner.only_timeouts && !inner.gpu_retried;
                self.publish(|s| s.device = "cpu".into());
                None
            }
            _ => {
                let wait = BACKOFF_BASE
                    .saturating_mul(1u32 << inner.backoffs.min(5))
                    .min(BACKOFF_MAX);
                inner.backoffs += 1;
                inner.consecutive_failures = 0;
                inner.backoff_until = Some(Instant::now() + wait);
                self.publish(|s| {
                    s.phase = ServerPhase::Failed;
                    s.transport = None;
                    s.ready_ms = None;
                    s.last_error = Some(msg.clone());
                });
                Some(StartError::Failed(msg))
            }
        }
    }

    async fn attempt(&self, inner: &mut Inner) -> Attempt {
        let config = lock(&self.config).clone();
        let Some(model) = config.model.clone() else {
            return Attempt::Failed("no model".into());
        };
        if let Err(e) = std::fs::create_dir_all(&config.llm_dir) {
            return Attempt::Failed(format!("llm dir: {e}"));
        }
        let endpoint = match transport::choose(&config.llm_dir) {
            Ok(e) => e,
            Err(e) => return Attempt::Failed(format!("transport: {e}")),
        };
        if let Endpoint::Unix(path) = &endpoint {
            // A socket file left by a killed server would make the bind fail.
            let _ = std::fs::remove_file(path);
        }
        let key = match transport::random_api_key() {
            Ok(k) => k,
            Err(e) => return Attempt::Failed(format!("random key: {e}")),
        };
        let transport = Transport::new(endpoint, key);
        let client = match transport.client(Duration::from_secs(60)) {
            Ok(c) => c,
            Err(e) => return Attempt::Failed(format!("client: {e}")),
        };
        let spec = SpawnSpec {
            binary: config.binary.clone(),
            model,
            transport: transport.clone(),
            device: inner.device,
            state_dir: config.llm_dir.clone(),
            home: config.home.clone(),
        };
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        let on_exit = self.exit_callback(generation);
        self.publish(|s| {
            s.phase = ServerPhase::Starting;
            s.transport = Some(transport.endpoint.kind().into());
            s.ready_ms = None;
        });
        let t0 = Instant::now();
        let mut server = match self.launcher.launch(&spec, on_exit) {
            Ok(s) => s,
            Err(e) => return Attempt::Failed(format!("spawn: {e}")),
        };
        let waited =
            health::wait_ready(&client, config.health_timeout, || !server.has_exited()).await;
        match waited {
            Ok(_) => {
                let ready_ms = u32::try_from(t0.elapsed().as_millis()).unwrap_or(u32::MAX);
                log::info!(
                    "llm: llama-server ready in {ready_ms} ms (pid {}, {}, {:?})",
                    server.pid(),
                    transport.endpoint.kind(),
                    inner.device
                );
                self.publish_transport(Some(&transport));
                inner.running = Some(Running {
                    server,
                    transport,
                    client,
                    generation,
                    last_contact: Instant::now(),
                });
                self.publish(|s| {
                    s.phase = ServerPhase::Ready;
                    s.ready_ms = Some(ready_ms);
                    s.last_error = None;
                });
                Attempt::Ok
            }
            Err(why) => {
                let tail = server.tail();
                tokio::task::spawn_blocking(move || server.stop())
                    .await
                    .ok();
                for line in tail
                    .as_str()
                    .lines()
                    .rev()
                    .take(12)
                    .collect::<Vec<_>>()
                    .iter()
                    .rev()
                {
                    log::warn!("llm: llama-server: {line}");
                }
                self.publish(|s| {
                    s.phase = ServerPhase::Stopped;
                    s.transport = None;
                });
                if tail.is_bind_failure() {
                    return Attempt::Bind;
                }
                let detail = tail
                    .last_error_line()
                    .map(|l| format!(": {l}"))
                    .unwrap_or_default();
                Attempt::Failed(match why {
                    WaitError::Exited => format!("server exited while loading{detail}"),
                    WaitError::Timeout => format!(
                        "{TIMEOUT_PREFIX} after {} s{detail}",
                        config.health_timeout.as_secs()
                    ),
                })
            }
        }
    }

    fn exit_callback(&self, generation: u64) -> Box<dyn FnOnce() + Send> {
        let me = self.me.clone();
        let rt = tokio::runtime::Handle::try_current().ok();
        Box::new(move || {
            let (Some(me), Some(rt)) = (me.upgrade(), rt) else {
                return;
            };
            rt.spawn(async move { me.on_exit(generation).await });
        })
    }

    /// A server process ended. Ignored unless it is the current one (a
    /// server we stopped ourselves has already been taken out).
    async fn on_exit(&self, generation: u64) {
        let restart = {
            let mut inner = self.inner.lock().await;
            if inner.running.as_ref().map(|r| r.generation) != Some(generation) {
                return;
            }
            let Some(mut dead) = inner.running.take() else {
                return;
            };
            self.publish_transport(None);
            let tail = dead.server.tail();
            tokio::task::spawn_blocking(move || dead.server.stop())
                .await
                .ok();
            let detail = tail
                .last_error_line()
                .unwrap_or("no error line")
                .to_string();
            log::error!("llm: llama-server exited unexpectedly ({detail})");
            let now = Instant::now();
            let repeat = inner
                .last_crash
                .is_some_and(|t| now.duration_since(t) < CRASH_WINDOW);
            inner.last_crash = Some(now);
            if repeat {
                inner.backoff_until = Some(now + BACKOFF_BASE);
            }
            self.publish(|s| {
                s.phase = if repeat {
                    ServerPhase::Failed
                } else {
                    ServerPhase::Stopped
                };
                s.transport = None;
                s.ready_ms = None;
                s.last_error = Some(format!("exited unexpectedly: {detail}"));
            });
            !repeat
        };
        if restart {
            self.publish(|s| s.restarts += 1);
            if let Err(e) = self.ensure_ready().await {
                log::warn!("llm: restart after crash failed: {e}");
            }
        }
    }

    /// Stops the server (idle, quit, model change).
    pub async fn stop(&self) {
        let mut inner = self.inner.lock().await;
        self.stop_locked(&mut inner).await;
    }

    async fn stop_locked(&self, inner: &mut Inner) {
        if let Some(mut r) = inner.running.take() {
            self.publish_transport(None);
            let pid = r.server.pid();
            tokio::task::spawn_blocking(move || r.server.stop())
                .await
                .ok();
            log::info!("llm: llama-server stopped (pid {pid})");
        }
        self.publish(|s| {
            if s.phase != ServerPhase::Failed {
                s.phase = ServerPhase::Stopped;
            }
            s.transport = None;
            s.ready_ms = None;
        });
    }

    /// Synchronous stop for app exit (no runtime needed).
    pub fn stop_blocking(&self) {
        if let Ok(mut inner) = self.inner.try_lock() {
            if let Some(mut r) = inner.running.take() {
                self.publish_transport(None);
                r.server.stop();
            }
        }
    }

    /// Stops the server now if no request is in flight, nobody waits for it
    /// and no start runs (critical memory pressure, S-38; a recording take
    /// does not hold it: it skips cleanup for memory). `true` when it
    /// stopped one.
    pub async fn stop_if_idle(&self) -> bool {
        if self.in_flight.load(Ordering::Acquire) > 0 || self.waiting.load(Ordering::Acquire) > 0 {
            return false;
        }
        let Ok(mut inner) = self.inner.try_lock() else {
            return false;
        };
        if inner.running.is_none() {
            return false;
        }
        self.stop_locked(&mut inner).await;
        true
    }

    /// Called every few seconds: stops an idle server.
    pub async fn idle_tick(&self) -> bool {
        let idle_timeout = lock(&self.config).idle_timeout;
        if self.wanted() || lock(&self.last_used).elapsed() < idle_timeout {
            return false;
        }
        // Never wait behind a start.
        let Ok(mut inner) = self.inner.try_lock() else {
            return false;
        };
        if inner.running.is_none() {
            return false;
        }
        log::info!(
            "llm: idle for {} s, stopping the server",
            idle_timeout.as_secs()
        );
        self.stop_locked(&mut inner).await;
        true
    }

    /// After wake from sleep: a server that does not answer is restarted.
    pub async fn on_wake(&self) {
        let healthy = {
            let mut inner = self.inner.lock().await;
            let Some(r) = inner.running.as_mut() else {
                return;
            };
            !r.server.has_exited() && health::probe(&r.client).await == Health::Ok
        };
        if healthy {
            log::info!("llm: server healthy after wake");
            return;
        }
        log::warn!("llm: server unhealthy after wake; restarting");
        self.stop().await;
        if let Err(e) = self.ensure_ready().await {
            log::warn!("llm: restart after wake failed: {e}");
        }
    }

    /// One completion, starting the server if needed.
    pub async fn complete(
        &self,
        request: &ChatRequest,
        timeout: Duration,
        cancel: &CancellationToken,
    ) -> Result<ChatReply, RequestError> {
        self.in_flight.fetch_add(1, Ordering::AcqRel);
        let _guard = InFlight(&self.in_flight);
        let handle = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(RequestError::Chat(ChatError::Cancelled)),
            h = self.ensure_ready() => h.map_err(RequestError::Start)?,
        };
        let result = chat::complete(&handle.client, request, timeout, cancel).await;
        self.touch();
        if result.is_ok() {
            if let Ok(mut inner) = self.inner.try_lock() {
                if let Some(r) = inner.running.as_mut() {
                    r.last_contact = Instant::now();
                }
            }
        }
        result.map_err(RequestError::Chat)
    }
}

// ---------------------------------------------------------------------------
// The real launcher

/// Spawns the bundled `llama-server`, with the parent-death watchdog and
/// the PID file.
pub struct ProcessLauncher;

struct RealServer {
    child: std::process::Child,
    tail: Arc<Mutex<OutputTail>>,
    pid_dir: PathBuf,
}

impl Server for RealServer {
    fn pid(&self) -> u32 {
        self.child.id()
    }

    fn has_exited(&mut self) -> bool {
        !matches!(self.child.try_wait(), Ok(None))
    }

    fn stop(&mut self) {
        if let Err(e) = process::terminate(&mut self.child, process::TERM_GRACE) {
            log::warn!("llm: stopping llama-server: {e}");
        }
        process::remove_pid_file(&self.pid_dir);
    }

    fn tail(&self) -> OutputTail {
        lock(&self.tail).clone()
    }
}

impl Launcher for ProcessLauncher {
    fn launch(
        &self,
        spec: &SpawnSpec,
        on_exit: Box<dyn FnOnce() + Send>,
    ) -> io::Result<Box<dyn Server>> {
        use std::io::BufRead;
        let mut child = spec.command().spawn()?;
        let pid_dir = spec.state_dir.clone();
        if let Err(e) = process::write_pid_file(&pid_dir, child.id()) {
            log::warn!("llm: pid file: {e}");
        }
        #[cfg(target_os = "macos")]
        {
            // SAFETY: getpid has no preconditions.
            let me = unsafe { libc::getpid() };
            match process::spawn_watchdog(me, child.id() as libc::pid_t) {
                Ok(dog) => process::reap_in_background(dog),
                Err(e) => log::warn!("llm: watchdog: {e}"),
            }
        }
        let tail = Arc::new(Mutex::new(OutputTail::default()));
        let stderr = child.stderr.take();
        let t = tail.clone();
        std::thread::Builder::new()
            .name("wb-llm-stderr".into())
            .spawn(move || {
                if let Some(stderr) = stderr {
                    for line in std::io::BufReader::new(stderr).lines() {
                        let Ok(line) = line else { break };
                        lock(&t).push_line(&line);
                    }
                }
                on_exit();
            })?;
        Ok(Box::new(RealServer {
            child,
            tail,
            pid_dir,
        }))
    }
}

#[cfg(test)]
mod tests;
