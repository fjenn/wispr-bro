// SPDX-License-Identifier: GPL-3.0-or-later
//! The one dedicated AX thread (R5 §4.6). Every AX call is a synchronous
//! message answered by the target app's main thread, so none may run on a UI
//! thread or hold up the paste: jobs are closures run here, one at a time,
//! in order, and a caller waits at most its budget (the job still finishes;
//! its answer is dropped). The thread owns the per-PID application
//! elements, the `AXManualAccessibility` lease table and the captured
//! insertion targets (AX elements are not `Send`), and switches leases off
//! when they expire and at exit.
//!
//! Lifted from the RS-04 spike's `thread.rs` (S-05, 2026-10-31), which
//! follows SpeakoFlow's `smart_spacing` worker (MIT) generalised to
//! arbitrary jobs. Dropped for the product: the Ctrl+C handler and the
//! spike's "without lease" probe.

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use objc2_application_services::{AXError, AXUIElement};
use objc2_core_foundation::CFRetained;

use super::element::{attr_bool, set_manual_accessibility, set_timeout};
use super::lease::{EnableResult, LeaseTable, HOLD};

type Job = Box<dyn FnOnce(&mut Ctx) + Send>;

/// Messaging timeout of every AX call the product makes (R5 §4.6). RS-04:
/// p95 single call ≤ 4.6 ms in 13 apps, 0 timeouts; 0.1 s never changed an
/// answer, so 0.2 s only bounds a hung app.
pub const CALL_TIMEOUT_S: f32 = 0.2;

/// How many captured targets the thread keeps (one per take start and stop;
/// a take is delivered long before eight more were captured).
const KEEP_TARGETS: usize = 8;

/// A focused element captured for a take (start or key-up).
pub struct Captured {
    pub id: u64,
    pub pid: Option<i32>,
    pub element: Option<CFRetained<AXUIElement>>,
    pub role: Option<String>,
    /// `AXPosition` + `AXSize` of the element, to recognise the same field
    /// when an app hands out a new element object for it.
    pub frame: Option<[f64; 4]>,
}

/// The field auto-learn watches after a paste (S-34,
/// `fork::dictionary::learn`).
pub struct Watched {
    pub id: u64,
    pub pid: i32,
    pub element: CFRetained<AXUIElement>,
    pub role: String,
    pub frame: Option<[f64; 4]>,
    /// The watch holds a lease on `pid`.
    pub lease: bool,
}

/// State that lives on the AX thread only.
pub struct Ctx {
    apps: HashMap<i32, CFRetained<AXUIElement>>,
    pub leases: LeaseTable<i32>,
    targets: VecDeque<Captured>,
    /// The app whose lease a take holds, by the take's start capture id.
    pub lease_by_take: HashMap<u64, i32>,
    /// S-34: the field watched for corrections, if any.
    pub watched: Option<Watched>,
}

impl Ctx {
    fn new() -> Ctx {
        Ctx {
            apps: HashMap::new(),
            leases: LeaseTable::new(HOLD),
            targets: VecDeque::new(),
            lease_by_take: HashMap::new(),
            watched: None,
        }
    }

    /// The application element of `pid` (cached), with the call timeout set.
    pub fn app(&mut self, pid: i32) -> CFRetained<AXUIElement> {
        let app = self
            .apps
            .entry(pid)
            // SAFETY: plain constructor.
            .or_insert_with(|| unsafe { AXUIElement::new_application(pid) })
            .clone();
        set_timeout(&app, CALL_TIMEOUT_S);
        app
    }

    /// One more reader of `pid`'s tree; switches `AXManualAccessibility` on
    /// if no lease is held. Returns whether the lease is held.
    pub fn lease_acquire(&mut self, pid: i32) -> bool {
        let app = self.app(pid);
        self.leases.acquire(pid, Instant::now(), || {
            let previous = attr_bool(&app, "AXManualAccessibility");
            match set_manual_accessibility(&app, true) {
                AXError::Success => EnableResult::Enabled {
                    previous: previous == Some(true),
                },
                e => EnableResult::Failed {
                    unsupported: matches!(
                        e,
                        AXError::AttributeUnsupported | AXError::NotImplemented
                    ),
                },
            }
        })
    }

    pub fn lease_release(&mut self, pid: i32) {
        self.leases.release(pid, Instant::now());
    }

    /// Keeps a captured target (the oldest beyond [`KEEP_TARGETS`] goes).
    pub fn keep_target(&mut self, c: Captured) {
        self.targets.retain(|t| t.id != c.id);
        self.targets.push_back(c);
        while self.targets.len() > KEEP_TARGETS {
            self.targets.pop_front();
        }
    }

    pub fn target(&self, id: u64) -> Option<&Captured> {
        self.targets.iter().find(|t| t.id == id)
    }

    fn expire(&mut self) {
        let mut off = Vec::new();
        self.leases.expire(Instant::now(), |pid| off.push(pid));
        for pid in off {
            let app = self.app(pid);
            let _ = set_manual_accessibility(&app, false);
        }
        // An app that quit leaves a dead element behind; drop the cache now
        // and then (it is rebuilt on the next read).
        if self.apps.len() > 32 {
            self.apps.clear();
        }
    }

    fn release_all(&mut self) {
        for pid in self.leases.drain_restore() {
            let app = self.app(pid);
            let _ = set_manual_accessibility(&app, false);
        }
        self.targets.clear();
        self.lease_by_take.clear();
        self.watched = None;
    }
}

static TX: OnceLock<Mutex<Sender<Job>>> = OnceLock::new();

fn sender() -> Option<Sender<Job>> {
    let tx = TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Job>();
        if let Err(e) = std::thread::Builder::new()
            .name("wb-ax".into())
            .spawn(move || worker(rx))
        {
            log::error!("ax: cannot start the AX thread: {e}");
        }
        Mutex::new(tx)
    });
    tx.lock().ok().map(|t| t.clone())
}

/// Runs `f` on the AX thread and waits at most `budget` for its answer.
pub fn run<R: Send + 'static>(
    budget: Duration,
    f: impl FnOnce(&mut Ctx) -> R + Send + 'static,
) -> Option<R> {
    let (reply, answer) = mpsc::sync_channel(1);
    let job: Job = Box::new(move |ctx| {
        let _ = reply.try_send(f(ctx));
    });
    sender()?.send(job).ok()?;
    answer.recv_timeout(budget).ok()
}

/// Queues `f` on the AX thread without waiting.
pub fn post(f: impl FnOnce(&mut Ctx) + Send + 'static) {
    if let Some(tx) = sender() {
        let _ = tx.send(Box::new(f));
    }
}

/// Restores every lease (app exit). Waits up to a second.
pub fn shutdown() {
    if TX.get().is_some() {
        let _ = run(Duration::from_secs(1), |ctx| ctx.release_all());
    }
}

fn worker(jobs: Receiver<Job>) {
    let mut ctx = Ctx::new();
    loop {
        match jobs.recv_timeout(Duration::from_secs(1)) {
            Ok(job) => {
                let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job(&mut ctx)));
                if run.is_err() {
                    log::error!("ax: a job panicked; the AX thread carries on");
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        ctx.expire();
    }
    ctx.release_all();
}
