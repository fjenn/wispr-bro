// SPDX-License-Identifier: GPL-3.0-or-later
//! The one dedicated AX thread (R5 §4.6). Every AX call is a synchronous
//! message answered by the target app's main thread, so none may run on a UI
//! thread; jobs are closures run here, one at a time, and the caller waits
//! at most its budget. The thread owns the per-PID app elements and the
//! `AXManualAccessibility` lease table, and switches leases off when they
//! expire, on Ctrl+C and at exit. Pattern from SpeakoFlow's `smart_spacing`
//! worker (MIT), generalised to arbitrary jobs.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use objc2_application_services::{AXError, AXUIElement};
use objc2_core_foundation::CFRetained;

use super::element::{attr_bool, set_manual_accessibility, set_timeout};
use super::lease::{EnableResult, LeaseTable, HOLD};

type Job = Box<dyn FnOnce(&mut Ctx) + Send>;

/// NSWorkspace's frontmost app, kept fresh by the main thread (which runs
/// the run loop that delivers NSWorkspace's updates). 0 = unknown.
static FRONT_PID: AtomicI32 = AtomicI32::new(0);

pub fn set_front_pid(pid: Option<i32>) {
    FRONT_PID.store(pid.unwrap_or(0), Ordering::SeqCst);
}

pub fn front_pid() -> Option<i32> {
    Some(FRONT_PID.load(Ordering::SeqCst)).filter(|p| *p > 0)
}

static STOP: AtomicBool = AtomicBool::new(false);
type StopHook = Box<dyn FnOnce() + Send>;
static ON_STOP: OnceLock<Mutex<Option<StopHook>>> = OnceLock::new();

/// State that lives on the AX thread only (AX elements are not `Send`).
pub struct Ctx {
    pub system: CFRetained<AXUIElement>,
    apps: HashMap<i32, CFRetained<AXUIElement>>,
    pub leases: LeaseTable<i32>,
    /// The focused element when the session last "marked" (change detection).
    pub baseline: Option<CFRetained<AXUIElement>>,
    /// The AXError of the last failed `AXManualAccessibility` write.
    pub last_lease_error: Option<&'static str>,
}

impl Ctx {
    fn new() -> Ctx {
        // SAFETY: plain constructor.
        let system = unsafe { AXUIElement::new_system_wide() };
        Ctx {
            system,
            apps: HashMap::new(),
            leases: LeaseTable::new(HOLD),
            baseline: None,
            last_lease_error: None,
        }
    }

    pub fn app(&mut self, pid: i32) -> CFRetained<AXUIElement> {
        self.apps
            .entry(pid)
            // SAFETY: plain constructor.
            .or_insert_with(|| unsafe { AXUIElement::new_application(pid) })
            .clone()
    }

    /// One more reader of `pid`'s tree; switches it on if needed. Returns
    /// whether the lease is held and what the app had before.
    pub fn lease_acquire(&mut self, pid: i32, timeout: f32) -> (bool, Option<bool>) {
        let app = self.app(pid);
        set_timeout(&app, timeout);
        let mut error = None;
        let held = self.leases.acquire(pid, Instant::now(), || {
            let previous = attr_bool(&app, "AXManualAccessibility");
            match set_manual_accessibility(&app, true) {
                AXError::Success => EnableResult::Enabled {
                    previous: previous == Some(true),
                },
                e => {
                    error = Some(super::element::error_name(e));
                    EnableResult::Failed {
                        unsupported: matches!(
                            e,
                            AXError::AttributeUnsupported | AXError::NotImplemented
                        ),
                    }
                }
            }
        });
        if error.is_some() {
            self.last_lease_error = error;
        }
        (held, self.leases.previous(pid))
    }

    pub fn lease_release(&mut self, pid: i32) {
        self.leases.release(pid, Instant::now());
    }

    /// Ends our lease now (RS-04's "without lease" probe). Writes `false`
    /// only if the app had it off before we switched it on.
    pub fn lease_force_off(&mut self, pid: i32) -> bool {
        if self.leases.force_off(pid) {
            let app = self.app(pid);
            return set_manual_accessibility(&app, false) == AXError::Success;
        }
        false
    }

    fn expire(&mut self) {
        let mut off = Vec::new();
        self.leases.expire(Instant::now(), |pid| off.push(pid));
        for pid in off {
            let app = self.app(pid);
            let _ = set_manual_accessibility(&app, false);
        }
    }

    fn release_all(&mut self) {
        for pid in self.leases.drain_restore() {
            let app = self.app(pid);
            set_timeout(&app, 0.25);
            let _ = set_manual_accessibility(&app, false);
        }
    }
}

pub struct AxThread {
    tx: Sender<Job>,
}

/// Ctrl+C: sets a flag only (async-signal-safe); the AX thread sees it,
/// switches leases off, runs the registered hook and exits.
extern "C" fn on_sigint(_: libc::c_int) {
    STOP.store(true, Ordering::SeqCst);
}

/// Code run once on Ctrl+C after the leases are restored (the summary).
pub fn on_stop(f: impl FnOnce() + Send + 'static) {
    let slot = ON_STOP.get_or_init(|| Mutex::new(None));
    if let Ok(mut g) = slot.lock() {
        *g = Some(Box::new(f));
    }
}

impl AxThread {
    pub fn spawn() -> AxThread {
        let (tx, rx) = mpsc::channel::<Job>();
        // SAFETY: installs a handler that only stores to an atomic.
        unsafe {
            libc::signal(libc::SIGINT, on_sigint as *const () as libc::sighandler_t);
        }
        std::thread::Builder::new()
            .name("wb-ax".into())
            .spawn(move || worker(rx))
            .expect("spawn AX thread");
        AxThread { tx }
    }

    /// Runs `f` on the AX thread; None if it did not answer within `budget`
    /// (the job still finishes there; its answer is dropped).
    pub fn run<R: Send + 'static>(
        &self,
        budget: Duration,
        f: impl FnOnce(&mut Ctx) -> R + Send + 'static,
    ) -> Option<R> {
        let (reply, answer) = mpsc::sync_channel(1);
        let job: Job = Box::new(move |ctx| {
            let _ = reply.try_send(f(ctx));
        });
        self.tx.send(job).ok()?;
        answer.recv_timeout(budget).ok()
    }

    /// Restores every lease and waits for it (end of session).
    pub fn shutdown(&self) {
        let _ = self.run(Duration::from_secs(3), |ctx| ctx.release_all());
    }
}

fn worker(jobs: Receiver<Job>) {
    let mut ctx = Ctx::new();
    loop {
        match jobs.recv_timeout(Duration::from_millis(300)) {
            Ok(job) => {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job(&mut ctx)));
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        ctx.expire();
        if STOP.load(Ordering::SeqCst) {
            ctx.release_all();
            println!("\n\nStopped. Switched the apps' accessibility mode back as it was.");
            if let Some(slot) = ON_STOP.get() {
                if let Some(f) = slot.lock().ok().and_then(|mut g| g.take()) {
                    f();
                }
            }
            std::process::exit(130);
        }
    }
    ctx.release_all();
}
