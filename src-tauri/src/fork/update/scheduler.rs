// SPDX-License-Identifier: GPL-3.0-or-later
//! Updates that never interrupt (S-28; R2 E-51, E-53, E-75, E-215, C-99).
//!
//! A background task checks the user's channel ([`super::find`]) shortly
//! after launch and every [`Timing::period`], downloads and verifies a new
//! release ([`super::download_verified`]) and keeps the verified bytes in
//! memory. It installs (`Update::install`, the plugin's `download()` then
//! `install()` split) only when [`idle_gate::may_install`] says so, or at
//! quit, and then relaunches quietly: hidden when the Hub was hidden
//! ([`super::relaunch`]). An install never starts mid-take, and an
//! installed update never relaunches mid-take. After three failed installs
//! background updates pause for a day ([`failures`]). The Hub footer, the
//! About page and the tray read [`ForkUpdateState`].
//!
//! [`Machine`] holds the decisions and is tested without a runtime; the
//! executor below only runs its steps.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_updater::Update;
use tauri_specta::Event;

use super::failures::{self, FailureLog};
use super::idle_gate::{self, Activity};
use crate::fork::settings::UpdateChannel;
use crate::fork::ForkRuntime;

/// Debug builds only: shorter timings for the scripted Mac test,
/// `first=3,period=60,retry=5,quiet=30,tick=1` (seconds).
pub const TIMING_ENV: &str = "WISPRBRO_UPDATE_TIMING";

/// How often the scheduler acts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timing {
    /// First check after launch.
    pub first_check: Duration,
    /// Between successful checks (E-53).
    pub period: Duration,
    /// Next check after a failed check.
    pub retry_check: Duration,
    /// Next attempt after a failed download or install.
    pub retry_install: Duration,
    /// Quiet window after the last take ([`idle_gate::QUIET_AFTER_TAKE`]).
    pub quiet: Duration,
    /// How often the gate is evaluated.
    pub tick: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            first_check: Duration::from_secs(60),
            period: Duration::from_secs(6 * 60 * 60),
            retry_check: Duration::from_secs(30 * 60),
            retry_install: Duration::from_secs(60 * 60),
            quiet: idle_gate::QUIET_AFTER_TAKE,
            tick: Duration::from_secs(10),
        }
    }
}

impl Timing {
    /// Parses [`TIMING_ENV`]; unknown or malformed parts keep the default.
    pub fn parse(spec: &str) -> Timing {
        let mut t = Timing::default();
        for part in spec.split(',') {
            let Some((key, value)) = part.split_once('=') else {
                continue;
            };
            let Ok(secs) = value.trim().parse::<u64>() else {
                continue;
            };
            let d = Duration::from_secs(secs.max(1));
            match key.trim() {
                "first" => t.first_check = d,
                "period" => t.period = d,
                "retry" => {
                    t.retry_check = d;
                    t.retry_install = d;
                }
                "quiet" => t.quiet = d,
                "tick" => t.tick = d,
                _ => {}
            }
        }
        t
    }

    /// [`Timing::parse`] of [`TIMING_ENV`] in debug builds, else the default.
    pub fn from_env() -> Timing {
        match std::env::var(TIMING_ENV) {
            Ok(spec) if cfg!(debug_assertions) => Timing::parse(&spec),
            _ => Timing::default(),
        }
    }
}

fn ms(d: Duration) -> i64 {
    i64::try_from(d.as_millis()).unwrap_or(i64::MAX)
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Where the update is (the footer's states).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum ForkUpdatePhase {
    /// Update checks are off (setting or kill switch).
    Off,
    /// No check yet.
    Idle,
    Checking,
    UpToDate,
    /// The last check failed (offline, GitHub down).
    CheckFailed,
    /// A newer release is downloading and being verified.
    Downloading,
    /// Downloaded and verified; installs when idle, at quit or on Restart now.
    Ready,
    Installing,
    /// The new version is on disk; relaunches once no take runs.
    Installed,
    /// The download, signature check or install failed.
    InstallFailed,
}

/// Everything the footer, About page and tray show. Also emitted as the
/// `fork-update-state` event whenever it changes.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Type, tauri_specta::Event)]
pub struct ForkUpdateState {
    pub current_version: String,
    pub channel: UpdateChannel,
    pub phase: ForkUpdatePhase,
    /// The release this state is about (downloading, ready, failed).
    pub version: Option<String>,
    /// Unix ms of the last finished check.
    pub last_checked_ms: Option<i64>,
    /// Background updates pause until this Unix ms after three failures.
    pub paused_until_ms: Option<i64>,
    /// Failed installs in a row.
    pub failures: u32,
    /// A take or transform runs now (What's New waits for it).
    pub busy: bool,
    /// Whether checks and installs run on their own in this build (not in
    /// development builds).
    pub automatic: bool,
    /// Replacing the app needs an administrator password (it sits in a
    /// folder the user cannot write), so it installs only on Restart now.
    pub needs_password: bool,
}

/// What the executor does next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Nothing,
    Check,
    Download,
    Install,
    Relaunch,
}

/// Why Restart now did nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestartRefused {
    /// A take runs; the relaunch waits (the veto).
    Busy,
    /// No update is downloaded.
    NothingReady,
}

/// The live inputs of one [`Machine::tick`].
#[derive(Clone, Copy, Debug)]
pub struct Inputs {
    pub checks_enabled: bool,
    /// [`idle_gate::background_allowed`].
    pub automatic: bool,
    pub channel: UpdateChannel,
    pub activity: Activity,
    /// The bundle can be replaced without an administrator password
    /// ([`install_needs_password`] is false). Otherwise the plugin would ask
    /// for one, so only Restart now installs.
    pub silent_install: bool,
}

/// The scheduler's decisions (pure).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Machine {
    pub phase: ForkUpdatePhase,
    pub version: Option<String>,
    /// Channel the downloaded release came from.
    pub ready_channel: Option<UpdateChannel>,
    pub last_checked_ms: Option<i64>,
    pub next_check_ms: i64,
    pub failures: FailureLog,
    pub timing: Timing,
}

impl Machine {
    pub fn new(now_ms: i64, failures: FailureLog, timing: Timing) -> Machine {
        Machine {
            phase: ForkUpdatePhase::Idle,
            version: None,
            ready_channel: None,
            last_checked_ms: None,
            next_check_ms: now_ms.saturating_add(ms(timing.first_check)),
            failures,
            timing,
        }
    }

    fn forget(&mut self) {
        self.version = None;
        self.ready_channel = None;
    }

    /// One evaluation of the live state.
    pub fn tick(&mut self, i: &Inputs) -> Step {
        let now = i.activity.now_ms;
        self.failures.expire(now);
        // The new version is already on disk: only the veto applies.
        if self.phase == ForkUpdatePhase::Installed {
            return if idle_gate::may_relaunch(i.activity.busy) {
                Step::Relaunch
            } else {
                Step::Nothing
            };
        }
        if !i.checks_enabled {
            // Off means off: a downloaded update is dropped, in-flight
            // results are ignored (their phase no longer matches).
            self.phase = ForkUpdatePhase::Off;
            self.forget();
            return Step::Nothing;
        }
        if self.phase == ForkUpdatePhase::Off {
            self.phase = ForkUpdatePhase::Idle;
            self.next_check_ms = now;
        }
        let paused = self.failures.is_paused(now);
        match self.phase {
            ForkUpdatePhase::Ready => {
                if self.ready_channel != Some(i.channel) {
                    // The channel changed: a beta never installs on stable
                    // (or the other way round); check the new channel.
                    self.phase = ForkUpdatePhase::Idle;
                    self.forget();
                    self.next_check_ms = now;
                    return Step::Nothing;
                }
                if !i.automatic || paused || !i.silent_install {
                    return Step::Nothing;
                }
                match idle_gate::may_install(&i.activity, self.timing.quiet) {
                    Ok(()) => {
                        self.phase = ForkUpdatePhase::Installing;
                        Step::Install
                    }
                    Err(_) => Step::Nothing,
                }
            }
            ForkUpdatePhase::Idle
            | ForkUpdatePhase::UpToDate
            | ForkUpdatePhase::CheckFailed
            | ForkUpdatePhase::InstallFailed => {
                if i.automatic && !paused && now >= self.next_check_ms {
                    self.phase = ForkUpdatePhase::Checking;
                    Step::Check
                } else {
                    Step::Nothing
                }
            }
            _ => Step::Nothing,
        }
    }

    /// "Check for updates" / "Retry": runs in any build and during a pause.
    pub fn request_check(&mut self) -> Step {
        match self.phase {
            ForkUpdatePhase::Idle
            | ForkUpdatePhase::UpToDate
            | ForkUpdatePhase::CheckFailed
            | ForkUpdatePhase::InstallFailed => {
                self.phase = ForkUpdatePhase::Checking;
                Step::Check
            }
            _ => Step::Nothing,
        }
    }

    /// A check finished: `Ok(Some(version))` is a newer release.
    pub fn on_checked(
        &mut self,
        now_ms: i64,
        channel: UpdateChannel,
        result: Result<Option<String>, String>,
    ) -> Step {
        if self.phase != ForkUpdatePhase::Checking {
            return Step::Nothing;
        }
        self.last_checked_ms = Some(now_ms);
        match result {
            Ok(Some(version)) => {
                self.phase = ForkUpdatePhase::Downloading;
                self.version = Some(version);
                self.ready_channel = Some(channel);
                Step::Download
            }
            Ok(None) => {
                self.phase = ForkUpdatePhase::UpToDate;
                self.forget();
                self.next_check_ms = now_ms.saturating_add(ms(self.timing.period));
                Step::Nothing
            }
            Err(_) => {
                self.phase = ForkUpdatePhase::CheckFailed;
                self.next_check_ms = now_ms.saturating_add(ms(self.timing.retry_check));
                Step::Nothing
            }
        }
    }

    fn failed(&mut self, now_ms: i64) -> bool {
        let version = self.version.clone().unwrap_or_default();
        let paused = self.failures.record(&version, now_ms);
        self.phase = ForkUpdatePhase::InstallFailed;
        self.ready_channel = None;
        self.next_check_ms = now_ms.saturating_add(ms(self.timing.retry_install));
        paused
    }

    /// The download and signature check finished. Returns `true` when the
    /// release is ready (the caller keeps the bytes and posts the notice).
    pub fn on_downloaded(&mut self, now_ms: i64, result: Result<(), String>) -> bool {
        if self.phase != ForkUpdatePhase::Downloading {
            return false;
        }
        match result {
            Ok(()) => {
                self.phase = ForkUpdatePhase::Ready;
                self.next_check_ms = now_ms.saturating_add(ms(self.timing.period));
                true
            }
            Err(_) => {
                self.failed(now_ms);
                false
            }
        }
    }

    /// The install finished.
    pub fn on_installed(&mut self, now_ms: i64, result: Result<(), String>) {
        if self.phase != ForkUpdatePhase::Installing {
            return;
        }
        match result {
            Ok(()) => {
                self.phase = ForkUpdatePhase::Installed;
                self.failures.clear();
            }
            Err(_) => {
                self.failed(now_ms);
            }
        }
    }

    /// "Restart now" (footer, About, tray, notification): installs a ready
    /// update and relaunches, but never mid-take.
    pub fn request_restart(&mut self, busy: bool) -> Result<Step, RestartRefused> {
        match self.phase {
            ForkUpdatePhase::Ready | ForkUpdatePhase::Installed
                if !idle_gate::may_relaunch(busy) =>
            {
                Err(RestartRefused::Busy)
            }
            ForkUpdatePhase::Ready => {
                self.phase = ForkUpdatePhase::Installing;
                Ok(Step::Install)
            }
            ForkUpdatePhase::Installed => Ok(Step::Relaunch),
            _ => Err(RestartRefused::NothingReady),
        }
    }

    /// At quit: whether the ready update installs now (no relaunch).
    pub fn take_quit_install(&mut self, checks_enabled: bool) -> bool {
        if self.phase == ForkUpdatePhase::Ready && checks_enabled {
            self.phase = ForkUpdatePhase::Installing;
            true
        } else {
            false
        }
    }
}

// ------------------------------------------------------------- executor ---

/// Installs verified bytes (the plugin's `Update::install`; tests replace it).
pub type InstallFn = fn(&Update, Vec<u8>) -> Result<(), String>;
/// Relaunches the app, hidden or not (tests replace it).
pub type RelaunchFn = Arc<dyn Fn(bool) + Send + Sync>;

fn plugin_install(update: &Update, bytes: Vec<u8>) -> Result<(), String> {
    update.install(bytes).map_err(|e| e.to_string())
}

/// A downloaded, verified release.
struct Held {
    update: Update,
    bytes: Vec<u8>,
}

/// Managed state of the scheduler.
pub struct UpdateScheduler {
    machine: Mutex<Machine>,
    held: Mutex<Option<Held>>,
    state_file: Option<PathBuf>,
    automatic: bool,
    install: InstallFn,
    relaunch: Mutex<Option<RelaunchFn>>,
    /// Test feed (the loopback stand-in); `None` uses the channel.
    endpoints: Option<Vec<reqwest::Url>>,
    last_emitted: Mutex<Option<ForkUpdateState>>,
    busy: std::sync::atomic::AtomicBool,
    password_logged: std::sync::atomic::AtomicBool,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl UpdateScheduler {
    pub fn new(
        machine: Machine,
        state_file: Option<PathBuf>,
        automatic: bool,
        install: InstallFn,
        endpoints: Option<Vec<reqwest::Url>>,
    ) -> UpdateScheduler {
        UpdateScheduler {
            machine: Mutex::new(machine),
            held: Mutex::new(None),
            state_file,
            automatic,
            install,
            relaunch: Mutex::new(None),
            endpoints,
            last_emitted: Mutex::new(None),
            busy: std::sync::atomic::AtomicBool::new(false),
            password_logged: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Sets how the app relaunches (the real restart, or a test probe).
    pub fn set_relaunch(&self, f: RelaunchFn) {
        *lock(&self.relaunch) = Some(f);
    }

    /// A clone of the decisions (tests, logs).
    pub fn machine(&self) -> Machine {
        lock(&self.machine).clone()
    }

    fn save_failures(&self, before: &FailureLog) {
        let after = lock(&self.machine).failures.clone();
        if &after == before {
            return;
        }
        if let Some(path) = &self.state_file {
            if let Err(e) = failures::save(path, &after) {
                log::warn!("update: could not save {}: {e}", failures::STATE_FILE);
            }
        }
    }
}

fn checks_enabled<R: ForkRuntime>(app: &AppHandle<R>) -> bool {
    super::checks_enabled(app)
}

fn any_window_focused<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.webview_windows()
        .values()
        .any(|w| w.is_focused().unwrap_or(false))
}

fn hub_hidden<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.get_webview_window("main")
        .is_none_or(|w| !w.is_visible().unwrap_or(false))
}

/// The `.app` bundle this process runs from (what the plugin replaces).
#[cfg(target_os = "macos")]
fn running_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    (macos.ends_with("MacOS") && contents.ends_with("Contents")).then(|| bundle.to_path_buf())
}

/// Whether `path` is writable by this user (`access(2)`).
#[cfg(target_os = "macos")]
fn writable(path: &std::path::Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let Ok(c) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: `c` is a valid NUL-terminated path for the call's duration.
    unsafe { libc::access(c.as_ptr(), libc::W_OK) == 0 }
}

/// `true` when installing would make the updater plugin ask for an
/// administrator password: it moves the running bundle aside, which needs
/// write access to the bundle's folder. A background or quit install never
/// raises that prompt (it would interrupt); Restart now may. Outside a
/// bundle (tests, `cargo run`) nothing is replaced, so `false`.
pub fn install_needs_password() -> bool {
    #[cfg(target_os = "macos")]
    {
        match running_bundle() {
            Some(bundle) => bundle.parent().is_none_or(|parent| !writable(parent)),
            None => false,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// The state the UI shows (also without a scheduler: checks off, or a
/// headless run).
pub fn state<R: ForkRuntime>(app: &AppHandle<R>) -> ForkUpdateState {
    let enabled = checks_enabled(app);
    let channel = super::current_channel(app);
    let current_version = app.package_info().version.to_string();
    let Some(s) = app.try_state::<UpdateScheduler>() else {
        return ForkUpdateState {
            current_version,
            channel,
            phase: if enabled {
                ForkUpdatePhase::Idle
            } else {
                ForkUpdatePhase::Off
            },
            version: None,
            last_checked_ms: None,
            paused_until_ms: None,
            failures: 0,
            busy: false,
            automatic: false,
            needs_password: false,
        };
    };
    let m = lock(&s.machine).clone();
    let phase = if !enabled && m.phase != ForkUpdatePhase::Installed {
        ForkUpdatePhase::Off
    } else {
        m.phase
    };
    ForkUpdateState {
        current_version,
        channel,
        phase,
        version: m.version.clone(),
        last_checked_ms: m.last_checked_ms,
        paused_until_ms: m
            .failures
            .paused_until_ms
            .filter(|_| m.failures.is_paused(now_ms())),
        failures: m.failures.count,
        busy: s.busy.load(std::sync::atomic::Ordering::Acquire),
        automatic: s.automatic,
        needs_password: install_needs_password(),
    }
}

/// Emits the state (and refreshes the tray) when it changed.
fn publish<R: ForkRuntime>(app: &AppHandle<R>) {
    let Some(s) = app.try_state::<UpdateScheduler>() else {
        return;
    };
    let next = state(app);
    {
        let mut last = lock(&s.last_emitted);
        if last.as_ref() == Some(&next) {
            return;
        }
        let tray_changed = last.as_ref().is_none_or(|l| {
            (l.phase == ForkUpdatePhase::Ready) != (next.phase == ForkUpdatePhase::Ready)
                || (l.phase == ForkUpdatePhase::Off) != (next.phase == ForkUpdatePhase::Off)
                || l.version != next.version
        });
        *last = Some(next.clone());
        if tray_changed {
            crate::fork::app_menu::request_refresh(crate::fork::app_menu::Refresh::Data);
        }
    }
    if let Err(e) = next.emit(app) {
        log::debug!("update: could not emit fork-update-state: {e}");
    }
}

/// Drops the held release once the machine left Ready/Installing (checks
/// switched off, channel changed).
fn drop_stale_held(s: &UpdateScheduler) {
    let phase = lock(&s.machine).phase;
    if !matches!(phase, ForkUpdatePhase::Ready | ForkUpdatePhase::Installing) {
        lock(&s.held).take();
    }
}

fn activity<R: ForkRuntime>(app: &AppHandle<R>, busy: bool) -> Activity {
    Activity {
        now_ms: now_ms(),
        busy,
        last_activity_ms: idle_gate::last_activity_ms(),
        window_focused: any_window_focused(app),
    }
}

/// One evaluation: samples the live state, asks the machine, runs the step.
pub async fn tick_once<R: ForkRuntime>(app: &AppHandle<R>) {
    let Some(s) = app.try_state::<UpdateScheduler>() else {
        return;
    };
    let busy = R::update_busy(app);
    if busy {
        idle_gate::note_activity();
    }
    s.busy.store(busy, std::sync::atomic::Ordering::Release);
    let inputs = Inputs {
        checks_enabled: checks_enabled(app),
        automatic: s.automatic,
        channel: super::current_channel(app),
        activity: activity(app, busy),
        silent_install: !install_needs_password(),
    };
    let before = lock(&s.machine).failures.clone();
    let step = lock(&s.machine).tick(&inputs);
    s.save_failures(&before);
    if !inputs.silent_install
        && lock(&s.machine).phase == ForkUpdatePhase::Ready
        && !s
            .password_logged
            .swap(true, std::sync::atomic::Ordering::AcqRel)
    {
        log::info!("update: installing needs an administrator password; waiting for Restart now");
    }
    drop_stale_held(&s);
    publish(app);
    run_step(app, step, !hub_hidden(app)).await;
}

/// Runs `step` (and what it leads to). `shown`: relaunch with the Hub shown.
async fn run_step<R: ForkRuntime>(app: &AppHandle<R>, step: Step, shown: bool) {
    match step {
        Step::Nothing | Step::Download => {}
        Step::Check => check_and_download(app).await,
        Step::Install => {
            if install_held(app) {
                relaunch_if_idle(app, shown);
            }
        }
        Step::Relaunch => relaunch_if_idle(app, shown),
    }
}

async fn check_and_download<R: ForkRuntime>(app: &AppHandle<R>) {
    let Some(s) = app.try_state::<UpdateScheduler>() else {
        return;
    };
    let channel = super::current_channel(app);
    publish(app);
    let found = match &s.endpoints {
        Some(endpoints) => super::find_on(app, endpoints.clone()).await,
        None => super::find(app, channel).await,
    };
    let (offer, result) = match found {
        Ok(Some((update, _))) => {
            let v = update.version.clone();
            (Some(update), Ok(Some(v)))
        }
        Ok(None) => (None, Ok(None)),
        Err(e) => (None, Err(e)),
    };
    if let Err(e) = &result {
        log::warn!("update: check failed: {e}");
    }
    let step = lock(&s.machine).on_checked(now_ms(), channel, result);
    publish(app);
    let (Step::Download, Some(update)) = (step, offer) else {
        if lock(&s.machine).phase == ForkUpdatePhase::UpToDate {
            log::info!("update: up to date on {channel:?}");
        }
        return;
    };
    log::info!("update: {} found, downloading", update.version);
    let downloaded = super::download_verified(&update).await;
    let before = lock(&s.machine).failures.clone();
    let ready = lock(&s.machine).on_downloaded(
        now_ms(),
        downloaded.as_ref().map(|_| ()).map_err(Clone::clone),
    );
    s.save_failures(&before);
    match downloaded {
        Ok(bytes) if ready => {
            log::info!(
                "update: {} ready ({} bytes, verified)",
                update.version,
                bytes.len()
            );
            let version = update.version.clone();
            *lock(&s.held) = Some(Held { update, bytes });
            crate::fork::notify::post(app, crate::fork::notify::Notice::update_ready(&version));
        }
        Ok(_) => log::info!("update: download no longer wanted"),
        Err(e) => {
            log::warn!(
                "update: {e} ({} failed in a row)",
                lock(&s.machine).failures.count
            );
            log_pause(&s);
        }
    }
    publish(app);
}

fn log_pause(s: &UpdateScheduler) {
    if lock(&s.machine).failures.is_paused(now_ms()) {
        log::warn!(
            "update: {} failed installs, pausing background updates for 24 h",
            failures::MAX_FAILURES
        );
    }
}

/// Installs the held release. Returns `true` on success.
fn install_held<R: ForkRuntime>(app: &AppHandle<R>) -> bool {
    let Some(s) = app.try_state::<UpdateScheduler>() else {
        return false;
    };
    publish(app);
    let held = lock(&s.held).take();
    let result = match held {
        Some(Held { update, bytes }) => {
            log::info!("update: installing {}", update.version);
            (s.install)(&update, bytes)
        }
        None => Err("no downloaded update".to_string()),
    };
    if let Err(e) = &result {
        log::warn!("update: install failed: {e}");
    }
    let before = lock(&s.machine).failures.clone();
    let ok = result.is_ok();
    lock(&s.machine).on_installed(now_ms(), result);
    s.save_failures(&before);
    if ok {
        log::info!(
            "update: installed {}",
            lock(&s.machine).version.clone().unwrap_or_default()
        );
    } else {
        log_pause(&s);
    }
    publish(app);
    ok
}

/// Relaunches unless a take runs (then the next tick tries again).
fn relaunch_if_idle<R: ForkRuntime>(app: &AppHandle<R>, shown: bool) {
    let Some(s) = app.try_state::<UpdateScheduler>() else {
        return;
    };
    if !idle_gate::may_relaunch(R::update_busy(app)) {
        log::info!("update: relaunch waits for the take to end");
        return;
    }
    let relaunch = lock(&s.relaunch).clone();
    match relaunch {
        Some(f) => {
            log::info!(
                "update: relaunching ({})",
                if shown { "shown" } else { "hidden" }
            );
            f(!shown);
        }
        None => log::warn!("update: no relaunch hook; the update applies at the next launch"),
    }
}

/// "Check for updates" / "Retry" (footer, About, tray). Returns at once;
/// progress arrives as `fork-update-state`.
pub fn check_now<R: ForkRuntime>(app: &AppHandle<R>) -> ForkUpdateState {
    if let Some(s) = app.try_state::<UpdateScheduler>() {
        if checks_enabled(app) {
            // A machine still marked Off picks the setting up first.
            let inputs = Inputs {
                checks_enabled: true,
                automatic: false,
                channel: super::current_channel(app),
                activity: activity(app, s.busy.load(std::sync::atomic::Ordering::Acquire)),
                silent_install: false,
            };
            let mut m = lock(&s.machine);
            if m.phase == ForkUpdatePhase::Off {
                m.tick(&inputs);
            }
            let step = m.request_check();
            drop(m);
            if step == Step::Check {
                let app = app.clone();
                tauri::async_runtime::spawn(async move { check_and_download(&app).await });
            }
        }
    }
    let now = state(app);
    publish(app);
    now
}

/// "Restart now": installs the ready update and relaunches (never mid-take).
pub fn restart_now<R: ForkRuntime>(app: &AppHandle<R>) -> Result<(), RestartRefused> {
    let Some(s) = app.try_state::<UpdateScheduler>() else {
        return Err(RestartRefused::NothingReady);
    };
    let busy = R::update_busy(app);
    let step = lock(&s.machine).request_restart(busy)?;
    let shown = !hub_hidden(app);
    // Off the caller's thread (a menu click or command runs on the main
    // thread; replacing the bundle takes a moment).
    let app = app.clone();
    std::thread::Builder::new()
        .name("wb-update-restart".into())
        .spawn(move || match step {
            Step::Install => {
                if install_held(&app) {
                    relaunch_if_idle(&app, shown);
                }
            }
            Step::Relaunch => relaunch_if_idle(&app, shown),
            _ => {}
        })
        .map_err(|e| {
            log::warn!("update: could not start the restart: {e}");
            RestartRefused::NothingReady
        })?;
    Ok(())
}

/// At quit (`RunEvent::ExitRequested` / `Exit`, never for our own restart):
/// a ready update installs now, without a relaunch.
pub fn on_exit<R: ForkRuntime>(app: &AppHandle<R>) {
    let Some(s) = app.try_state::<UpdateScheduler>() else {
        return;
    };
    // No password prompt at quit either; the next launch offers it again.
    if install_needs_password() {
        return;
    }
    if !lock(&s.machine).take_quit_install(checks_enabled(app)) {
        return;
    }
    log::info!("update: installing at quit");
    install_held(app);
}

/// Starts the scheduler (every non-headless run; automatic checks only in
/// release product builds, see [`idle_gate::background_allowed`]).
pub fn start<R: ForkRuntime>(app: &AppHandle<R>) {
    let version = app.package_info().version.to_string();
    let automatic = idle_gate::background_allowed(
        Some(&version),
        cfg!(debug_assertions),
        std::env::var_os(super::channel::ENDPOINT_OVERRIDE_ENV).is_some(),
        crate::fork::is_product_build(),
    );
    let state_file = crate::portable::data_dir()
        .cloned()
        .or_else(|| app.path().app_data_dir().ok())
        .map(|dir| failures::state_path(&dir));
    let mut log_ = state_file
        .as_deref()
        .map(failures::load)
        .unwrap_or_default();
    if log_.on_launch(&version) {
        if let Some(path) = &state_file {
            let _ = failures::save(path, &log_);
        }
    }
    let now = now_ms();
    // Launch counts as activity: nothing installs in the first quiet window.
    idle_gate::note_activity_at(now);
    let timing = Timing::from_env();
    let scheduler = UpdateScheduler::new(
        Machine::new(now, log_, timing),
        state_file,
        automatic,
        plugin_install,
        None,
    );
    let handle = app.clone();
    scheduler.set_relaunch(Arc::new(move |hidden| {
        super::relaunch::relaunch(&handle, hidden)
    }));
    app.manage(scheduler);
    log::info!(
        "update: scheduler started (automatic: {automatic}, first check in {} s)",
        timing.first_check.as_secs()
    );
    // A changed setting (checks, channel) is picked up at once.
    let handle = app.clone();
    crate::fork::events::ForkSettingsChanged::listen(app, move |_| {
        let app = handle.clone();
        tauri::async_runtime::spawn(async move { tick_once(&app).await });
    });
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tick_once(&app).await;
            tokio::time::sleep(timing.tick).await;
        }
    });
}

/// Test stand-in for [`ForkRuntime::update_busy`] on the mock runtime.
#[cfg(test)]
pub struct FakeBusy(pub std::sync::atomic::AtomicBool);
