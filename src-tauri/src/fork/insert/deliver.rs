// SPDX-License-Identifier: GPL-3.0-or-later
//! One insertion, start to finish (R5 §4.7 W-07, S-04). Lifted from the
//! RS-05 spike's `take.rs` and split in two so the app can return as soon as
//! the text is in the field:
//!
//! [`paste`]:
//! 1. stop if the take was cancelled or the session was interrupted (sleep,
//!    lock, user switch: the text goes to the card, never pasted blind);
//! 2. wait for the hotkey's modifiers to be physically released;
//! 3. read the focused field (for verification);
//! 4. if the previous take's target has not read its text yet, wait for
//!    that read (or that take's deadline): otherwise a late reader would get
//!    this take's text and insert it twice;
//! 5. snapshot every item and type (skip policy applied), write the
//!    transcript (markers + lazy text) and register with the guard, all
//!    under the [`Line`] lock so a restore of an earlier take cannot land in
//!    between;
//! 6. post ⌘V, or type for Chromium 155/156 (no clipboard at all);
//! 7. verify: poll the field ≤ 300 ms for a change → `confirmed`, else
//!    `unverified` (R5 §4.7; a read receipt is logged, not counted);
//! 8. auto-submit (Handy's setting) only once the text demonstrably arrived.
//!
//! [`settle`] (on its own thread in the app): wait for the target to read the
//! text, then a quiet period, or for the deadline (1.5 s native, 7 s
//! Chromium/Electron), and restore only if the guard says so (same
//! generation, `changeCount` unchanged).
//!
//! Exactly once: neither function ever retries. A ⌘V that could not be
//! posted is reported as a card (`paste_failed`), never followed by a second
//! attempt on another path, because the first one may have half-worked.
//!
//! Pure over [`Platform`]: the app implements it on AppKit
//! (`fork::insert::mac`), the tests on `testutil::FakePasteboard`, and the
//! S-04 soak tool (`crates/wb-spike-clipboard`, which compiles this file
//! through `#[path]`) on the real pasteboard with a pumped run loop.

use serde::Serialize;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use super::super::clipboard::restore::{
    evaluate, Finish, RestoreDecision, RestoreGuard, TargetClass, Timing, TxState, Wait,
};
use super::super::clipboard::snapshot::{self, Snapshot};
use super::verify::{self, FieldState, Verification};

// ---------------------------------------------------------------------------
// Outcomes

/// What one take's delivery ended in (`entry_meta.paste_outcome`, R5 §4.3;
/// `refused` and `queued` are written by S-05 and S-07a).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PasteOutcome {
    /// The field's content changed after the paste.
    Confirmed,
    /// Pasted, but the field could not be read or did not change in time
    /// (Electron apps with no AX tree, some IMEs).
    Unverified,
    /// Not pasted: the text waits on the card (and is in History).
    Card,
    /// Not pasted: a secure field (S-05).
    Refused,
    /// Not pasted yet: waiting for the model (S-07a).
    Queued,
}

impl PasteOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            PasteOutcome::Confirmed => "confirmed",
            PasteOutcome::Unverified => "unverified",
            PasteOutcome::Card => "card",
            PasteOutcome::Refused => "refused",
            PasteOutcome::Queued => "queued",
        }
    }
}

/// Why a take went to the card instead of the field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CardReason {
    /// The Mac went to sleep (or the lid closed) during the take.
    Sleep,
    /// The screen was locked during the take.
    ScreenLocked,
    /// The user switched accounts (fast user switching) during the take.
    SessionInactive,
    /// The text could not be put on the clipboard or the paste key could
    /// not be sent.
    PasteFailed,
}

impl CardReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            CardReason::Sleep => "sleep",
            CardReason::ScreenLocked => "screen_locked",
            CardReason::SessionInactive => "session_inactive",
            CardReason::PasteFailed => "paste_failed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Delivery {
    /// The paste was sent (exactly once).
    Pasted { outcome: PasteOutcome },
    /// Nothing was sent; the text goes to the card.
    Card { reason: CardReason },
    /// The user cancelled before the paste; nothing was sent.
    Cancelled,
}

impl Delivery {
    /// The take's `paste_outcome`; `None` for a cancelled take (it has none).
    pub fn paste_outcome(&self) -> Option<PasteOutcome> {
        match self {
            Delivery::Pasted { outcome } => Some(*outcome),
            Delivery::Card { .. } => Some(PasteOutcome::Card),
            Delivery::Cancelled => None,
        }
    }

    /// Whether a paste (⌘V or typing) was sent.
    pub fn inserted(&self) -> bool {
        matches!(self, Delivery::Pasted { .. })
    }
}

// ---------------------------------------------------------------------------
// Platform

/// Our transcript on the clipboard (the lazy item and its read receipts).
pub trait PublishedText {
    fn change_count(&self) -> i64;
    /// Every time anyone read our text.
    fn reads(&self) -> Vec<Instant>;
}

/// Everything [`paste`] and [`settle`] touch outside themselves.
pub trait Platform {
    type Published: PublishedText;

    fn now(&self) -> Instant;
    /// Sleeps (the app) or pumps the run loop (the tools).
    fn wait(&self, d: Duration);
    /// Modifier flags held right now (CGEventSource HID | session).
    fn modifier_flags(&self) -> u64;
    fn change_count(&self) -> i64;
    /// Every item and type, plus the types whose data could not be read.
    fn read(&self) -> (Snapshot, Vec<String>);
    /// Replaces the clipboard; returns the new change count.
    fn write(&self, snap: &Snapshot) -> Result<i64, String>;
    /// Replaces the clipboard with unmarked plain text (Handy's "copy to
    /// clipboard" setting).
    fn write_plain_text(&self, text: &str) -> Result<i64, String>;
    /// Writes the transcript as one item: the three markers plus the text as
    /// a lazy promise.
    fn publish(&self, text: &str) -> Result<Self::Published, String>;
    /// ⌘V on the active layout.
    fn post_paste(&self) -> Result<(), String>;
    /// Types the text (Chromium 155/156; never touches the clipboard).
    fn type_text(&self, text: &str) -> Result<(), String>;
    /// Return with these modifier flags (auto-submit).
    fn post_return(&self, flags: u64) -> Result<(), String>;
    /// The focused field, as far as Accessibility can tell.
    fn field(&self) -> Option<FieldState>;
}

// ---------------------------------------------------------------------------
// Request and report

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    CommandV,
    /// hex's Chromium 155/156 path.
    Typing,
}

impl Method {
    pub fn for_target(class: TargetClass) -> Method {
        if super::super::clipboard::target::has_clipboard_race(class) {
            Method::Typing
        } else {
            Method::CommandV
        }
    }
}

#[derive(Clone, Debug)]
pub struct Request {
    pub text: String,
    pub class: TargetClass,
    pub method: Method,
    /// Press Return (with these modifier flags) once the text arrived.
    pub submit: Option<u64>,
    /// Leave the transcript on the clipboard as plain text instead of
    /// restoring (Handy's `ClipboardHandling::CopyToClipboard`).
    pub keep_transcript: bool,
    pub max_modifier_wait: Duration,
    pub verify_budget: Duration,
    /// Restore deadlines (R5: 1.5 s native, 7 s Chromium/Electron).
    pub timing: Timing,
}

/// Modifiers that must be up before we post ⌘V (shift, control, option,
/// command, fn; the same set as `clipboard::keys::HOTKEY_MODIFIERS`, here so
/// the engine stays platform-free).
pub const HOTKEY_MODIFIERS: u64 = (1 << 17) | (1 << 18) | (1 << 19) | (1 << 20) | (1 << 23);

impl Request {
    pub fn new(text: &str, class: TargetClass) -> Request {
        Request {
            text: text.to_string(),
            class,
            method: Method::for_target(class),
            submit: None,
            keep_transcript: false,
            max_modifier_wait: Duration::from_millis(3000),
            verify_budget: verify::BUDGET,
            timing: Timing::R5,
        }
    }
}

/// Checks the caller supplies: both are polled more than once.
pub struct Gate<'a> {
    /// The take was cancelled (Esc, a new take).
    pub cancelled: &'a dyn Fn() -> bool,
    /// The session was interrupted during this take (`session.rs`).
    pub interrupted: &'a dyn Fn() -> Option<CardReason>,
}

impl Gate<'_> {
    fn stop(&self) -> Option<Delivery> {
        if (self.cancelled)() {
            return Some(Delivery::Cancelled);
        }
        (self.interrupted)().map(|reason| Delivery::Card { reason })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub delivery: Delivery,
    pub method: Method,
    pub class: TargetClass,
    /// How long we waited for the hotkey's modifiers to come up.
    pub modifier_wait_ms: Option<u64>,
    pub modifier_wait_timed_out: bool,
    /// Waited for the previous take's target to read its text.
    pub previous_read_wait_ms: Option<f64>,
    pub snapshot_ms: Option<f64>,
    pub snapshot_bytes: usize,
    pub skipped_types: usize,
    pub unreadable_types: usize,
    /// Start of [`paste`] → the paste was sent.
    pub to_paste_ms: Option<f64>,
    /// First read of our text after the ⌘V, while verifying.
    pub receipt_ms: Option<f64>,
    pub verification: Option<Verification>,
    pub submitted: bool,
    pub error: Option<String>,
}

/// A paste waiting for its restore ([`settle`]).
pub struct Pending<P> {
    generation: u64,
    published: P,
    state: TxState,
    class: TargetClass,
    keep_transcript: Option<String>,
}

impl<P> Pending<P> {
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Settled {
    pub finish: Option<Finish>,
    /// `restore`, `superseded`, `clipboard_changed`, `nothing_saved`.
    pub decision: &'static str,
    /// ⌘V → the restore was written.
    pub restore_after_ms: Option<f64>,
    /// Reads of our text, ms after the ⌘V (negative: before it).
    pub reads_ms: Vec<f64>,
    /// The transcript was left on the clipboard as plain text instead.
    pub kept_transcript: bool,
    pub error: Option<String>,
}

fn ms(d: Duration) -> f64 {
    (d.as_secs_f64() * 1000.0 * 10.0).round() / 10.0
}

/// The clipboard state shared by every delivery: the restore guard (the
/// user's snapshot across back-to-back takes) and the paste still waiting
/// for its target to read.
#[derive(Debug)]
pub struct Line {
    guard: RestoreGuard,
    unread: Option<Unread>,
}

#[derive(Clone, Copy, Debug)]
struct Unread {
    generation: u64,
    /// The take's restore deadline: past it, nobody waits any more.
    until: Instant,
}

impl Line {
    pub const fn new() -> Line {
        Line {
            guard: RestoreGuard::new(),
            unread: None,
        }
    }

    fn read_seen(&mut self, generation: u64) {
        if self.unread.is_some_and(|u| u.generation == generation) {
            self.unread = None;
        }
    }
}

impl Default for Line {
    fn default() -> Line {
        Line::new()
    }
}

fn lock(line: &Mutex<Line>) -> MutexGuard<'_, Line> {
    line.lock().unwrap_or_else(|e| e.into_inner())
}

/// Longest wait for the previous take's read before pasting anyway.
const PREVIOUS_READ_MAX: Duration = Duration::from_millis(7000);

// ---------------------------------------------------------------------------
// paste

/// Sends one take's text to the focused field. Returns the report and, for
/// a clipboard paste, the restore still to do.
pub fn paste<P: Platform>(
    p: &P,
    line: &Mutex<Line>,
    req: &Request,
    gate: &Gate<'_>,
) -> (Report, Option<Pending<P::Published>>) {
    let start = p.now();
    let mut r = Report {
        delivery: Delivery::Cancelled,
        method: req.method,
        class: req.class,
        modifier_wait_ms: None,
        modifier_wait_timed_out: false,
        previous_read_wait_ms: None,
        snapshot_ms: None,
        snapshot_bytes: 0,
        skipped_types: 0,
        unreadable_types: 0,
        to_paste_ms: None,
        receipt_ms: None,
        verification: None,
        submitted: false,
        error: None,
    };

    // 1. Cancelled or interrupted before anything happened.
    if let Some(d) = gate.stop() {
        r.delivery = d;
        return (r, None);
    }

    // 2. Modifiers up (hex `wait_for_modifiers_released`).
    let w = wait_for_modifiers(p, req.max_modifier_wait);
    r.modifier_wait_ms = Some(w.0);
    r.modifier_wait_timed_out = w.1;
    if let Some(d) = gate.stop() {
        r.delivery = d;
        return (r, None);
    }

    // 3. The field before the paste.
    let before = p.field();

    if req.method == Method::Typing {
        let injected = p.now();
        r.to_paste_ms = Some(ms(injected.duration_since(start)));
        if let Err(e) = p.type_text(&req.text) {
            r.error = Some(e);
            r.delivery = Delivery::Card {
                reason: CardReason::PasteFailed,
            };
            return (r, None);
        }
        let v = verify::verify(
            before.as_ref(),
            req.verify_budget,
            || p.now(),
            |d| p.wait(d),
            || p.field(),
        );
        finish_paste(p, req, &mut r, v, false);
        return (r, None);
    }

    // 4. The previous paste's target has read (or given up).
    let waited_from = p.now();
    loop {
        let unread = lock(line).unread;
        match unread {
            Some(u) if p.now() < u.until.min(waited_from + PREVIOUS_READ_MAX) => {
                p.wait(Duration::from_millis(5));
            }
            _ => break,
        }
    }
    let waited = p.now().saturating_duration_since(waited_from);
    if waited >= Duration::from_millis(1) {
        r.previous_read_wait_ms = Some(ms(waited));
        if let Some(d) = gate.stop() {
            r.delivery = d;
            return (r, None);
        }
    }

    // 5. Snapshot, transcript, register: one critical section, so an earlier
    // take's restore cannot slip between our read and our write.
    let mut l = lock(line);
    let t = p.now();
    let mut snap = None;
    for _ in 0..3 {
        let cc0 = p.change_count();
        let (full, unreadable) = p.read();
        if p.change_count() == cc0 {
            snap = Some((cc0, full, unreadable));
            break;
        }
    }
    let Some((cc0, full, unreadable)) = snap else {
        drop(l);
        r.error = Some("the clipboard kept changing while it was being saved".into());
        r.delivery = Delivery::Card {
            reason: CardReason::PasteFailed,
        };
        return (r, None);
    };
    let (keep, skipped) = snapshot::apply_policy(&full);
    r.snapshot_ms = Some(ms(p.now().duration_since(t)));
    r.snapshot_bytes = full.bytes();
    r.skipped_types = skipped.len();
    r.unreadable_types = unreadable.len();

    if let Some(d) = gate.stop() {
        r.delivery = d;
        return (r, None);
    }
    let published = match p.publish(&req.text) {
        Ok(pb) => pb,
        Err(e) => {
            // The write may have cleared the clipboard first. Then put back
            // what the guard holds: `keep`, or, if `keep` is an earlier
            // transcript still waiting for its restore, the user's own
            // snapshot carried forward. Untouched: leave the earlier take's
            // restore to its own waiter.
            let now_cc = p.change_count();
            if now_cc != cc0 {
                let generation = l.guard.register(keep, cc0, now_cc);
                if let RestoreDecision::Restore(snap) = l.guard.decide(generation, now_cc) {
                    let _ = p.write(&snap);
                }
            }
            drop(l);
            r.error = Some(e);
            r.delivery = Delivery::Card {
                reason: CardReason::PasteFailed,
            };
            return (r, None);
        }
    };
    let generation = l.guard.register(keep, cc0, published.change_count());
    // Whatever the previous take was waiting for is superseded now.
    l.unread = None;
    drop(l);

    // 6. ⌘V.
    let mut state = TxState::new(p.now());
    let injected = p.now();
    state.injected_at = Some(injected);
    r.to_paste_ms = Some(ms(injected.duration_since(start)));
    let posted = p.post_paste();
    let pending_for = |state: TxState, published| Pending {
        generation,
        published,
        state,
        class: req.class,
        keep_transcript: req.keep_transcript.then(|| req.text.clone()),
    };
    if let Err(e) = posted {
        state.injection_failed = true;
        r.error = Some(e);
        r.delivery = Delivery::Card {
            reason: CardReason::PasteFailed,
        };
        return (r, Some(pending_for(state, published)));
    }
    lock(line).unread = Some(Unread {
        generation,
        until: injected + req.timing.deadline(req.class),
    });

    // 7. Verify.
    let v = verify::verify(
        before.as_ref(),
        req.verify_budget,
        || p.now(),
        |d| p.wait(d),
        || p.field(),
    );
    let receipt = published.reads().into_iter().find(|t| *t >= injected);
    if receipt.is_some() {
        lock(line).read_seen(generation);
    }
    r.receipt_ms = receipt.map(|t| ms(t.duration_since(injected)));
    finish_paste(p, req, &mut r, v, receipt.is_some());
    (r, Some(pending_for(state, published)))
}

/// Records the verification and sends the auto-submit Return if the text
/// demonstrably arrived (Handy's rule: never submit an unconfirmed paste).
fn finish_paste<P: Platform>(p: &P, req: &Request, r: &mut Report, v: Verification, receipt: bool) {
    let outcome = if v.confirmed() {
        PasteOutcome::Confirmed
    } else {
        PasteOutcome::Unverified
    };
    r.verification = Some(v);
    r.delivery = Delivery::Pasted { outcome };
    if let Some(flags) = req.submit {
        if outcome == PasteOutcome::Confirmed || receipt {
            match p.post_return(flags) {
                Ok(()) => r.submitted = true,
                Err(e) => r.error = Some(format!("auto-submit failed: {e}")),
            }
        }
    }
}

/// (waited ms, timed out).
fn wait_for_modifiers<P: Platform>(p: &P, max: Duration) -> (u64, bool) {
    let start = p.now();
    while p.modifier_flags() & HOTKEY_MODIFIERS != 0 {
        let waited = p.now().saturating_duration_since(start);
        if waited >= max {
            return (max.as_millis() as u64, true);
        }
        p.wait(Duration::from_millis(10));
    }
    (
        p.now().saturating_duration_since(start).as_millis() as u64,
        false,
    )
}

// ---------------------------------------------------------------------------
// settle

/// Waits for the target to read the text, then restores the user's
/// clipboard if it is still ours. Blocks; the app runs it on its own thread.
pub fn settle<P: Platform>(
    p: &P,
    line: &Mutex<Line>,
    mut pending: Pending<P::Published>,
    timing: &Timing,
) -> Settled {
    let injected = pending
        .state
        .injected_at
        .unwrap_or(pending.state.published_at);
    let mut out = Settled {
        finish: None,
        decision: "superseded",
        restore_after_ms: None,
        reads_ms: Vec::new(),
        kept_transcript: false,
        error: None,
    };
    let finish = loop {
        p.wait(Duration::from_millis(5));
        if !lock(line).guard.is_current(pending.generation) {
            // A newer paste carries the snapshot forward and restores it.
            out.reads_ms = reads_ms(&pending.published, injected);
            return out;
        }
        pending.state.receipts = pending.published.reads();
        if pending.state.last_receipt_after_injection().is_some() {
            lock(line).read_seen(pending.generation);
        }
        // Not the provider's "finished" callback: AppKit also sends it once
        // every promised type has been provided (RS-05), which is no loss.
        pending.state.ownership_lost = p.change_count() != pending.published.change_count();
        if let Wait::Finish(f) = evaluate(&pending.state, timing, pending.class, p.now()) {
            break f;
        }
    };
    out.finish = Some(finish);
    out.reads_ms = reads_ms(&pending.published, injected);

    // Decide and write under the lock (see `paste` step 4).
    // `unread` stays set if the target never read: until its deadline the
    // next take still waits, in case the read comes late (after a user copy
    // or a slow app) and would otherwise pick up the next transcript.
    let mut l = lock(line);
    let decision = l.guard.decide(pending.generation, p.change_count());
    out.decision = decision.label();
    if let RestoreDecision::Restore(snap) = decision {
        out.restore_after_ms = Some(ms(p.now().duration_since(injected)));
        let written = match &pending.keep_transcript {
            Some(text) => {
                out.kept_transcript = true;
                p.write_plain_text(text)
            }
            None => p.write(&snap),
        };
        if let Err(e) = written {
            out.error = Some(e);
        }
    }
    drop(l);
    out
}

fn reads_ms<P: PublishedText>(published: &P, injected: Instant) -> Vec<f64> {
    published
        .reads()
        .iter()
        .map(|t| {
            if *t >= injected {
                ms(t.duration_since(injected))
            } else {
                -ms(injected.duration_since(*t))
            }
        })
        .collect()
}
