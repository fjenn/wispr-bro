// SPDX-License-Identifier: GPL-3.0-or-later
//! S-04 tests: the delivery engine on a fake pasteboard and a fake clock
//! (R5 S-04 "Tests"), the outcome plumbing, and the `actions.rs` hook.

use std::cell::{Cell, RefCell};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::deliver::{
    self, CardReason, Delivery, Gate, Line, Method, PasteOutcome, Platform, PublishedText, Request,
};
use super::verify::{FieldState, Unverified, Verification};
use crate::fork::clipboard::markers::{self, manager_view};
use crate::fork::clipboard::restore::{TargetClass, Timing};
use crate::fork::clipboard::snapshot::{compare, Snapshot};
use crate::fork::clipboard::{Pasteboard, PasteboardItem};
use crate::fork::testutil::{FakePasteboard, PasteboardWriter};

// ---------------------------------------------------------------------------
// Fake platform

/// How the fake target app behaves when ⌘V arrives.
#[derive(Clone, Copy, Debug)]
struct Target {
    /// Reads the clipboard this long after ⌘V (`None`: never reads).
    reads_after_ms: Option<u64>,
    /// Its field is readable through Accessibility.
    ax: bool,
}

const NATIVE: Target = Target {
    reads_after_ms: Some(10),
    ax: true,
};
const ELECTRON_NO_AX: Target = Target {
    reads_after_ms: Some(15),
    ax: false,
};

struct FakePublished {
    change_count: i64,
    reads: Arc<Mutex<Vec<Instant>>>,
}

impl PublishedText for FakePublished {
    fn change_count(&self) -> i64 {
        self.change_count
    }
    fn reads(&self) -> Vec<Instant> {
        self.reads.lock().unwrap().clone()
    }
}

/// The reads of one published transcript (shared with its `FakePublished`).
type Reads = Arc<Mutex<Vec<Instant>>>;

enum Scheduled {
    UserCopy(Vec<PasteboardItem>),
    ReleaseModifiers,
}

struct Fake {
    pb: Arc<FakePasteboard>,
    t0: Instant,
    elapsed: Cell<u64>,
    target: Target,
    modifiers: Cell<u64>,
    /// What the target's field holds (what was pasted or typed into it).
    field_text: RefCell<String>,
    /// Pending reads: (at ms, reads list of the transcript then on the
    /// clipboard, its change count).
    pending_reads: RefCell<Vec<(u64, Reads, i64)>>,
    current: RefCell<Option<(i64, Reads, String)>>,
    schedule: RefCell<Vec<(u64, Scheduled)>>,
    /// Every paste or typing sent (the exactly-once count).
    insertions: Cell<usize>,
    returns: Cell<usize>,
    fail_publish: Cell<bool>,
    fail_post: Cell<bool>,
}

impl Fake {
    fn new(initial: Vec<PasteboardItem>, target: Target) -> Fake {
        Fake {
            pb: Arc::new(FakePasteboard::with_items(initial)),
            t0: Instant::now(),
            elapsed: Cell::new(0),
            target,
            modifiers: Cell::new(0),
            field_text: RefCell::new(String::new()),
            pending_reads: RefCell::new(Vec::new()),
            current: RefCell::new(None),
            schedule: RefCell::new(Vec::new()),
            insertions: Cell::new(0),
            returns: Cell::new(0),
            fail_publish: Cell::new(false),
            fail_post: Cell::new(false),
        }
    }

    fn at(&self, ms: u64, what: Scheduled) {
        self.schedule.borrow_mut().push((ms, what));
    }

    fn items(&self) -> Vec<PasteboardItem> {
        self.pb.read()
    }

    fn run_due(&self) {
        let now = self.elapsed.get();
        let due: Vec<Scheduled> = {
            let mut s = self.schedule.borrow_mut();
            let (due, later): (Vec<_>, Vec<_>) = s.drain(..).partition(|(t, _)| *t <= now);
            *s = later;
            due.into_iter().map(|(_, w)| w).collect()
        };
        for w in due {
            match w {
                Scheduled::UserCopy(items) => {
                    self.pb.user_copy(items);
                }
                Scheduled::ReleaseModifiers => self.modifiers.set(0),
            }
        }
        let reads: Vec<_> = {
            let mut p = self.pending_reads.borrow_mut();
            let (due, later): (Vec<_>, Vec<_>) = p.drain(..).partition(|(t, _, _)| *t <= now);
            *p = later;
            due
        };
        for (at, list, cc) in reads {
            // The target reads whatever is on the clipboard now; it is our
            // transcript (a receipt) only if nobody wrote since.
            if self.pb.change_count() == cc {
                list.lock()
                    .unwrap()
                    .push(self.t0 + Duration::from_millis(at));
                let text = self.current.borrow().as_ref().map(|c| c.2.clone());
                if let Some(t) = text {
                    self.field_text.borrow_mut().push_str(&t);
                }
            } else {
                // The old-clipboard paste RS-05 guards against: record it.
                let other = self
                    .pb
                    .read()
                    .first()
                    .and_then(|i| i.get(markers::TEXT).map(|b| b.to_vec()));
                if let Some(b) = other {
                    self.field_text
                        .borrow_mut()
                        .push_str(&String::from_utf8_lossy(&b));
                }
            }
        }
    }
}

impl Platform for Fake {
    type Published = FakePublished;

    fn now(&self) -> Instant {
        self.t0 + Duration::from_millis(self.elapsed.get())
    }
    fn wait(&self, d: Duration) {
        self.elapsed
            .set(self.elapsed.get() + (d.as_millis() as u64).max(1));
        self.run_due();
    }
    fn modifier_flags(&self) -> u64 {
        self.modifiers.get()
    }
    fn change_count(&self) -> i64 {
        self.pb.change_count()
    }
    fn read(&self) -> (Snapshot, Vec<String>) {
        (
            Snapshot {
                items: self.pb.read(),
            },
            Vec::new(),
        )
    }
    fn write(&self, snap: &Snapshot) -> Result<i64, String> {
        Ok(self.pb.write(snap.items.clone()))
    }
    fn write_plain_text(&self, text: &str) -> Result<i64, String> {
        Ok(self.pb.write(vec![PasteboardItem::text(text)]))
    }
    fn publish(&self, text: &str) -> Result<FakePublished, String> {
        if self.fail_publish.get() {
            // A failed write after the clear: the clipboard is empty.
            self.pb.write(Vec::new());
            return Err("could not write the clipboard".into());
        }
        let item = PasteboardItem::default()
            .with(markers::TRANSIENT, b"")
            .with(markers::AUTO_GENERATED, b"")
            .with(markers::CONCEALED, b"")
            .with(markers::TEXT, text.as_bytes());
        let cc = self.pb.write(vec![item]);
        let reads = Arc::new(Mutex::new(Vec::new()));
        *self.current.borrow_mut() = Some((cc, reads.clone(), text.to_string()));
        Ok(FakePublished {
            change_count: cc,
            reads,
        })
    }
    fn post_paste(&self) -> Result<(), String> {
        if self.fail_post.get() {
            return Err("could not post ⌘V".into());
        }
        self.insertions.set(self.insertions.get() + 1);
        if let (Some(after), Some((cc, list, _))) =
            (self.target.reads_after_ms, self.current.borrow().as_ref())
        {
            self.pending_reads
                .borrow_mut()
                .push((self.elapsed.get() + after, list.clone(), *cc));
        }
        Ok(())
    }
    fn type_text(&self, text: &str) -> Result<(), String> {
        self.insertions.set(self.insertions.get() + 1);
        self.field_text.borrow_mut().push_str(text);
        Ok(())
    }
    fn post_return(&self, _flags: u64) -> Result<(), String> {
        self.returns.set(self.returns.get() + 1);
        Ok(())
    }
    fn field(&self) -> Option<FieldState> {
        self.target.ax.then(|| {
            let t = self.field_text.borrow();
            FieldState {
                role: Some("AXTextArea".into()),
                chars: Some(t.chars().count() as i64),
                caret: Some((t.chars().count() as i64, 0)),
                value_fingerprint: None,
            }
        })
    }
}

fn never() -> bool {
    false
}
fn no_interruption() -> Option<CardReason> {
    None
}
const OPEN: Gate<'static> = Gate {
    cancelled: &never,
    interrupted: &no_interruption,
};

fn native(text: &str) -> Request {
    Request::new(text, TargetClass::Native)
}

fn user_items() -> Vec<PasteboardItem> {
    vec![
        PasteboardItem::default()
            .with("public.file-url", b"file:///Users/me/a.txt")
            .with(markers::TEXT, b"a.txt"),
        PasteboardItem::default()
            .with("public.file-url", b"file:///Users/me/b.png")
            .with("public.png", &[0x89, b'P', b'N', b'G', 0, 1, 2]),
    ]
}

fn one_take(f: &Fake, guard: &Mutex<Line>, req: &Request) -> (deliver::Report, deliver::Settled) {
    let (report, pending) = deliver::paste(f, guard, req, &OPEN);
    let settled = deliver::settle(f, guard, pending.expect("a clipboard paste"), &Timing::R5);
    (report, settled)
}

// ---------------------------------------------------------------------------
// Fake-pasteboard tests (R5 S-04)

#[test]
fn multi_item_round_trip_is_byte_identical() {
    let f = Fake::new(user_items(), NATIVE);
    let guard = Mutex::new(Line::new());
    let (report, settled) = one_take(&f, &guard, &native("hello world"));
    assert_eq!(
        report.delivery,
        Delivery::Pasted {
            outcome: PasteOutcome::Confirmed
        }
    );
    assert_eq!(settled.decision, "restore");
    let fidelity = compare(
        &Snapshot {
            items: user_items(),
        },
        &Snapshot { items: f.items() },
    );
    assert!(fidelity.byte_identical(), "{}", fidelity.describe());
    assert!(fidelity.order_kept);
    assert_eq!(*f.field_text.borrow(), "hello world");
    assert_eq!(f.insertions.get(), 1);
    // Read 10 ms after ⌘V, quiet 200 ms → restore ~210 ms after (RS-05).
    let after = settled.restore_after_ms.unwrap();
    assert!((205.0..=230.0).contains(&after), "{after}");
    assert_eq!(report.receipt_ms, Some(10.0));
}

#[test]
fn transcript_carries_the_markers_and_nothing_else() {
    let f = Fake::new(user_items(), NATIVE);
    let guard = Mutex::new(Line::new());
    one_take(&f, &guard, &native("secret words"));
    let log = f.pb.log();
    // App writes: the transcript, then the restore.
    assert_eq!(log.len(), 2);
    let transcript: Vec<Vec<String>> = Snapshot {
        items: log[0].1.clone(),
    }
    .types();
    assert!(manager_view(&transcript).convention_says_skip());
    // A manager following nspasteboard.org records only the restore (the
    // user's own content, byte-identical), never the dictated text.
    let visible = f.pb.writes_visible_to_clipboard_managers();
    assert_eq!(visible, vec![user_items()]);
}

#[test]
fn user_copy_during_the_paste_wins_no_restore() {
    let f = Fake::new(user_items(), NATIVE);
    let mine = vec![PasteboardItem::text("copied by the user meanwhile")];
    f.at(100, Scheduled::UserCopy(mine.clone()));
    let guard = Mutex::new(Line::new());
    let (_, settled) = one_take(&f, &guard, &native("dictated"));
    assert_eq!(settled.decision, "clipboard_changed");
    assert_eq!(f.items(), mine);
    assert_eq!(
        f.pb.log().last().unwrap().0,
        PasteboardWriter::User,
        "nothing written after the user's copy"
    );
}

#[test]
fn back_to_back_takes_carry_the_snapshot_forward() {
    // Take 2 is pasted before take 1's restore ran (its target never reads).
    let f = Fake::new(
        user_items(),
        Target {
            reads_after_ms: None,
            ax: true,
        },
    );
    let guard = Mutex::new(Line::new());
    let (r1, p1) = deliver::paste(&f, &guard, &native("first "), &OPEN);
    let (r2, p2) = deliver::paste(&f, &guard, &native("second"), &OPEN);
    assert!(r1.delivery.inserted() && r2.delivery.inserted());
    let s1 = deliver::settle(&f, &guard, p1.unwrap(), &Timing::R5);
    assert_eq!(s1.decision, "superseded");
    let s2 = deliver::settle(&f, &guard, p2.unwrap(), &Timing::R5);
    assert_eq!(s2.decision, "restore");
    // The user's clipboard, not the first transcript.
    assert_eq!(f.items(), user_items());
    // No read: restored at the native deadline, 1.5 s after the ⌘V.
    assert_eq!(
        s2.finish,
        Some(crate::fork::clipboard::restore::Finish::Deadline)
    );
}

#[test]
fn next_take_waits_for_the_previous_read() {
    // Take 1's target reads 800 ms after ⌘V. Take 2 must not replace the
    // transcript before that, or take 1's paste would insert take 2's text.
    let f = Fake::new(
        user_items(),
        Target {
            reads_after_ms: Some(800),
            ax: false,
        },
    );
    let guard = Mutex::new(Line::new());
    let (_, p1) = deliver::paste(&f, &guard, &native("one "), &OPEN);
    let (r2, p2) = deliver::paste(&f, &guard, &native("two"), &OPEN);
    assert!(r2.previous_read_wait_ms.unwrap() >= 800.0, "{r2:?}");
    deliver::settle(&f, &guard, p1.unwrap(), &Timing::R5);
    deliver::settle(&f, &guard, p2.unwrap(), &Timing::R5);
    f.wait(Duration::from_secs(5));
    assert_eq!(*f.field_text.borrow(), "one two");
    assert_eq!(f.items(), user_items());
}

#[test]
fn sequential_takes_each_restore_the_users_clipboard() {
    let f = Fake::new(user_items(), NATIVE);
    let guard = Mutex::new(Line::new());
    for i in 0..5 {
        let (r, s) = one_take(&f, &guard, &native(&format!("[{i}]")));
        assert!(r.delivery.inserted());
        assert_eq!(s.decision, "restore");
        assert_eq!(f.items(), user_items());
    }
    assert_eq!(*f.field_text.borrow(), "[0][1][2][3][4]");
}

#[test]
fn empty_clipboard_stays_empty() {
    let f = Fake::new(Vec::new(), NATIVE);
    let guard = Mutex::new(Line::new());
    one_take(&f, &guard, &native("x"));
    assert!(f.items().is_empty());
}

#[test]
fn skip_policy_drops_rtfd_and_keeps_the_rest() {
    let rich = vec![PasteboardItem::default()
        .with("com.apple.flat-rtfd", b"rtfd bytes")
        .with("public.rtf", b"{\\rtf1 hi}")
        .with(markers::TEXT, b"hi")];
    let f = Fake::new(rich.clone(), NATIVE);
    let guard = Mutex::new(Line::new());
    let (r, _) = one_take(&f, &guard, &native("x"));
    assert_eq!(r.skipped_types, 1);
    let fid = compare(&Snapshot { items: rich }, &Snapshot { items: f.items() });
    assert!(fid.byte_identical());
    assert_eq!(fid.skipped(), 1);
}

// ---------------------------------------------------------------------------
// Verification outcomes returned by paste()

#[test]
fn electron_without_ax_is_unverified_but_pasted() {
    let f = Fake::new(user_items(), ELECTRON_NO_AX);
    let guard = Mutex::new(Line::new());
    let req = Request::new(
        "x",
        TargetClass::Electron {
            chromium_major: Some(152),
        },
    );
    assert_eq!(req.method, Method::CommandV);
    let (r, s) = one_take(&f, &guard, &req);
    assert_eq!(
        r.delivery,
        Delivery::Pasted {
            outcome: PasteOutcome::Unverified
        }
    );
    assert_eq!(
        r.verification,
        Some(Verification::Unverified {
            reason: Unverified::NoField
        })
    );
    // No field to watch, so paste() returns at once (before the read); the
    // read receipt still makes the restore early, ~215 ms after the ⌘V.
    assert_eq!(r.receipt_ms, None);
    assert_eq!(s.reads_ms, vec![15.0]);
    assert!(s.restore_after_ms.unwrap() < 300.0);
}

#[test]
fn ime_or_slow_field_is_unverified_not_retried() {
    // The target reads the clipboard but its field shows nothing within
    // 300 ms (an IME holding the text, or a slow app).
    let f = Fake::new(
        user_items(),
        Target {
            reads_after_ms: Some(400),
            ax: true,
        },
    );
    let guard = Mutex::new(Line::new());
    let (r, s) = one_take(&f, &guard, &native("にほんご"));
    assert_eq!(
        r.verification,
        Some(Verification::Unverified {
            reason: Unverified::NoChange
        })
    );
    assert_eq!(f.insertions.get(), 1);
    // It did land later; the restore waited for the read.
    assert_eq!(*f.field_text.borrow(), "にほんご");
    assert_eq!(s.decision, "restore");
    assert_eq!(f.items(), user_items());
}

#[test]
fn chromium_155_156_types_and_never_touches_the_clipboard() {
    let f = Fake::new(user_items(), NATIVE);
    let guard = Mutex::new(Line::new());
    let req = Request::new("typed text", TargetClass::Chromium { major: Some(156) });
    assert_eq!(req.method, Method::Typing);
    let (r, pending) = deliver::paste(&f, &guard, &req, &OPEN);
    assert!(pending.is_none());
    assert_eq!(
        r.delivery,
        Delivery::Pasted {
            outcome: PasteOutcome::Confirmed
        }
    );
    assert!(f.pb.log().is_empty(), "no clipboard write at all");
    assert_eq!(*f.field_text.borrow(), "typed text");
}

#[test]
fn modifiers_are_released_before_the_paste() {
    let f = Fake::new(user_items(), NATIVE);
    f.modifiers.set((1 << 18) | (1 << 20)); // ⌃⌘ held
    f.at(250, Scheduled::ReleaseModifiers);
    let guard = Mutex::new(Line::new());
    let (r, _) = one_take(&f, &guard, &native("x"));
    assert_eq!(r.modifier_wait_ms, Some(250));
    assert!(!r.modifier_wait_timed_out);
    assert!(r.to_paste_ms.unwrap() >= 250.0);
    // Caps lock is not a hotkey modifier.
    let f = Fake::new(user_items(), NATIVE);
    f.modifiers.set(1 << 16);
    let (r, _) = one_take(&f, &guard, &native("x"));
    assert_eq!(r.modifier_wait_ms, Some(0));
    // Held past the wait (3 s): nothing is sent, the text goes to the card.
    let f = Fake::new(user_items(), NATIVE);
    f.modifiers.set(1 << 19); // ⌥ held by something else
    let (r, pending) = deliver::paste(&f, &guard, &native("x"), &OPEN);
    assert!(r.modifier_wait_timed_out);
    assert_eq!(
        r.delivery,
        Delivery::Card {
            reason: CardReason::PasteFailed
        }
    );
    assert!(pending.is_none() && f.pb.log().is_empty());
    assert_eq!(f.insertions.get(), 0);
}

#[test]
fn auto_submit_only_after_the_text_arrived() {
    let guard = Mutex::new(Line::new());
    let mut req = native("send me");
    req.submit = Some(0);
    let f = Fake::new(user_items(), NATIVE);
    let (r, _) = one_take(&f, &guard, &req);
    assert!(r.submitted);
    assert_eq!(f.returns.get(), 1);
    // A target that never reads and shows nothing: no Return.
    let f = Fake::new(
        user_items(),
        Target {
            reads_after_ms: None,
            ax: false,
        },
    );
    let (r, _) = one_take(&f, &guard, &req);
    assert!(!r.submitted);
    assert_eq!(f.returns.get(), 0);
}

/// S-19a "Press Enter after dictating" (RV-48): Return only after a
/// `confirmed` paste; an `unverified` one (the app read the text but its
/// field cannot be checked) gets none.
#[test]
fn strict_auto_send_needs_a_confirmed_paste() {
    let guard = Mutex::new(Line::new());
    let mut req = native("send me");
    req.submit = Some(0);
    req.submit_strict = true;
    let f = Fake::new(user_items(), ELECTRON_NO_AX);
    let (r, s) = one_take(&f, &guard, &req);
    assert_eq!(
        r.delivery,
        Delivery::Pasted {
            outcome: PasteOutcome::Unverified
        }
    );
    // The app did read the text, later.
    assert_eq!(s.reads_ms, vec![15.0]);
    assert!(!r.submitted);
    assert_eq!(f.returns.get(), 0);
    // Confirmed: Return.
    let f = Fake::new(user_items(), NATIVE);
    let (r, _) = one_take(&f, &guard, &req);
    assert!(r.submitted);
    assert_eq!(f.returns.get(), 1);
}

#[test]
fn copy_to_clipboard_setting_leaves_plain_text() {
    let f = Fake::new(user_items(), NATIVE);
    let guard = Mutex::new(Line::new());
    let mut req = native("keep me");
    req.keep_transcript = true;
    let (_, s) = one_take(&f, &guard, &req);
    assert!(s.kept_transcript);
    assert_eq!(f.items(), vec![PasteboardItem::text("keep me")]);
}

// ---------------------------------------------------------------------------
// Gates and failures: never paste blind, never twice

#[test]
fn interrupted_take_goes_to_the_card_and_touches_nothing() {
    for reason in [
        CardReason::Sleep,
        CardReason::ScreenLocked,
        CardReason::SessionInactive,
    ] {
        let f = Fake::new(user_items(), NATIVE);
        let guard = Mutex::new(Line::new());
        let interrupted = move || Some(reason);
        let gate = Gate {
            cancelled: &never,
            interrupted: &interrupted,
        };
        let (r, pending) = deliver::paste(&f, &guard, &native("x"), &gate);
        assert_eq!(r.delivery, Delivery::Card { reason });
        assert_eq!(r.delivery.paste_outcome(), Some(PasteOutcome::Card));
        assert!(pending.is_none());
        assert!(f.pb.log().is_empty());
        assert_eq!(f.insertions.get(), 0);
    }
}

#[test]
fn sleep_during_the_modifier_wait_stops_the_paste() {
    let f = Fake::new(user_items(), NATIVE);
    f.modifiers.set(1 << 23); // fn still down
    f.at(100, Scheduled::ReleaseModifiers);
    let asleep = Cell::new(false);
    let interrupted = || asleep.get().then_some(CardReason::Sleep);
    let gate = Gate {
        cancelled: &never,
        interrupted: &interrupted,
    };
    let guard = Mutex::new(Line::new());
    asleep.set(true); // the lid closes while fn is still held
    let (r, _) = deliver::paste(&f, &guard, &native("x"), &gate);
    assert!(!r.delivery.inserted());
    assert_eq!(f.insertions.get(), 0);
}

#[test]
fn cancelled_take_sends_nothing() {
    let f = Fake::new(user_items(), NATIVE);
    let guard = Mutex::new(Line::new());
    let yes = || true;
    let gate = Gate {
        cancelled: &yes,
        interrupted: &no_interruption,
    };
    let (r, pending) = deliver::paste(&f, &guard, &native("x"), &gate);
    assert_eq!(r.delivery, Delivery::Cancelled);
    assert_eq!(r.delivery.paste_outcome(), None);
    assert!(pending.is_none() && f.pb.log().is_empty());
}

#[test]
fn publish_failure_cards_and_puts_the_clipboard_back() {
    let f = Fake::new(user_items(), NATIVE);
    f.fail_publish.set(true);
    let guard = Mutex::new(Line::new());
    let (r, pending) = deliver::paste(&f, &guard, &native("x"), &OPEN);
    assert_eq!(
        r.delivery,
        Delivery::Card {
            reason: CardReason::PasteFailed
        }
    );
    assert!(pending.is_none());
    assert_eq!(f.items(), user_items());
    assert_eq!(f.insertions.get(), 0);
}

#[test]
fn post_failure_cards_once_and_restores_quickly() {
    let f = Fake::new(user_items(), NATIVE);
    f.fail_post.set(true);
    let guard = Mutex::new(Line::new());
    let (r, pending) = deliver::paste(&f, &guard, &native("x"), &OPEN);
    assert_eq!(
        r.delivery,
        Delivery::Card {
            reason: CardReason::PasteFailed
        }
    );
    let s = deliver::settle(&f, &guard, pending.unwrap(), &Timing::R5);
    assert_eq!(s.decision, "restore");
    assert!(s.restore_after_ms.unwrap() <= 510.0);
    assert_eq!(f.items(), user_items());
    // No second attempt on another path.
    assert_eq!(f.insertions.get(), 0);
    assert!(f.field_text.borrow().is_empty());
}

// ---------------------------------------------------------------------------
// Property test: one insertion path per take over cancel / retry races

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, pct: u64) -> bool {
        self.below(100) < pct
    }
}

#[test]
fn property_exactly_once_and_clipboard_safe() {
    let mut rng = Rng(0x5157_4f34_2026_1030);
    for case in 0..2000 {
        let target = Target {
            reads_after_ms: (!rng.chance(15)).then(|| rng.below(1200)),
            ax: rng.chance(70),
        };
        let f = Fake::new(user_items(), target);
        let guard = Mutex::new(Line::new());
        let takes = 1 + rng.below(4) as usize;
        let mut pendings = Vec::new();
        let mut expect_text = String::new();
        let mut user_copied: Option<Vec<PasteboardItem>> = None;
        for t in 0..takes {
            let text = format!("<{case}.{t}>");
            let cancel_at = rng.chance(15);
            let interrupt = rng.chance(10).then_some(CardReason::Sleep);
            f.fail_publish.set(rng.chance(5));
            f.fail_post.set(rng.chance(5));
            if rng.chance(10) {
                f.modifiers.set(1 << 20);
                let at = f.elapsed.get() + rng.below(400);
                f.at(at, Scheduled::ReleaseModifiers);
            }
            if rng.chance(10) {
                let items = vec![PasteboardItem::text(&format!("user {case}.{t}"))];
                let at = f.elapsed.get() + rng.below(600);
                f.at(at, Scheduled::UserCopy(items.clone()));
                user_copied = Some(items);
            }
            let class = match rng.below(3) {
                0 => TargetClass::Native,
                1 => TargetClass::Electron {
                    chromium_major: Some(150),
                },
                _ => TargetClass::Chromium {
                    major: Some(155 + rng.below(4) as u32),
                },
            };
            let cancelled = move || cancel_at;
            let interrupted = move || interrupt;
            let gate = Gate {
                cancelled: &cancelled,
                interrupted: &interrupted,
            };
            let before = f.insertions.get();
            let (r, pending) = deliver::paste(&f, &guard, &Request::new(&text, class), &gate);
            let sent = f.insertions.get() - before;
            // One insertion path per take: at most one paste or typing, and
            // exactly one iff the report says it was inserted.
            assert!(sent <= 1, "case {case}: {sent} insertions");
            assert_eq!(r.delivery.inserted(), sent == 1, "case {case}: {r:?}");
            match r.delivery {
                Delivery::Cancelled => assert!(cancel_at),
                Delivery::Card { reason } => {
                    assert!(interrupt == Some(reason) || reason == CardReason::PasteFailed)
                }
                Delivery::Pasted { .. } => {
                    assert!(!cancel_at && interrupt.is_none());
                    expect_text.push_str(&text);
                }
            }
            if let Some(p) = pending {
                // Sometimes settle at once, sometimes after the next take.
                if rng.chance(50) {
                    deliver::settle(&f, &guard, p, &Timing::R5);
                } else {
                    pendings.push(p);
                }
            }
        }
        for p in pendings {
            deliver::settle(&f, &guard, p, &Timing::R5);
        }
        // Let every scheduled read and copy happen.
        f.wait(Duration::from_millis(10_000));
        // No transcript is ever left behind, and the user's own content is
        // what remains: their original clipboard, or what they copied.
        let left = f.items();
        assert!(
            !left.iter().any(|i| i.has(markers::TRANSIENT)),
            "case {case}: transcript left on the clipboard"
        );
        if user_copied.is_none() {
            assert_eq!(left, user_items(), "case {case}");
        }
        // Every pasted take's text arrived at most once.
        let field = f.field_text.borrow().clone();
        for t in 0..takes {
            let token = format!("<{case}.{t}>");
            assert!(field.matches(&token).count() <= 1, "case {case}: {field}");
        }
        if user_copied.is_none() {
            assert!(
                !field.contains("a.txt"),
                "case {case}: the old clipboard was pasted: {field}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Outcome plumbing and the hook

#[test]
fn outcome_event_carries_text_only_for_a_card() {
    use crate::fork::events::{ForkCardReason, ForkPasteOutcome};
    let e = super::outcome_event(
        3,
        Delivery::Pasted {
            outcome: PasteOutcome::Confirmed,
        },
        "hi",
    );
    assert_eq!(e.outcome, Some(ForkPasteOutcome::Confirmed));
    assert_eq!(e.text, None);
    let e = super::outcome_event(
        4,
        Delivery::Card {
            reason: CardReason::ScreenLocked,
        },
        "hi",
    );
    assert_eq!(e.outcome, Some(ForkPasteOutcome::Card));
    assert_eq!(e.card_reason, Some(ForkCardReason::ScreenLocked));
    assert_eq!(e.text.as_deref(), Some("hi"));
    assert_eq!(
        super::outcome_event(5, Delivery::Cancelled, "x").outcome,
        None
    );
    // Wire names (`fork.json` > insert.cardReason.* keys use them).
    assert_eq!(
        serde_json::to_value(ForkCardReason::SessionInactive).unwrap(),
        "session_inactive"
    );
    assert_eq!(CardReason::SessionInactive.as_str(), "session_inactive");
}

#[test]
fn hook_result_matches_handys_paste_contract() {
    assert!(super::hook_result(Delivery::Pasted {
        outcome: PasteOutcome::Unverified
    })
    .is_ok());
    assert!(super::hook_result(Delivery::Cancelled).is_ok());
    let e = super::hook_result(Delivery::Card {
        reason: CardReason::Sleep,
    })
    .unwrap_err();
    assert!(e.contains("sleep") && e.contains("History"));
}

#[test]
fn recording_phases_feed_the_session_log() {
    let t0 = Instant::now();
    // Uses the process-wide log; other tests do not touch it.
    super::on_recording_phase(r#"{"phase":"started"}"#, t0);
    super::on_recording_phase(r#"{"phase":"locked"}"#, t0);
    super::on_recording_phase("not json", t0);
    assert!(super::lock(&super::SESSION).recording());
    super::on_recording_phase(r#"{"phase":"stopped"}"#, t0 + Duration::from_secs(1));
    assert!(!super::lock(&super::SESSION).recording());
    assert_eq!(super::interruption(), None);
}

#[test]
fn every_card_reason_has_an_english_string() {
    // The card's title and its VoiceOver announcement share
    // `rust.pill.card.<reason>` (S-05).
    let en: serde_json::Value =
        serde_json::from_str(include_str!("../../../../src/i18n/locales/en/fork.json")).unwrap();
    for r in CardReason::ALL {
        let key = crate::fork::pill::card_key(super::card_reason(r));
        assert_eq!(key, format!("rust.pill.card.{}", r.as_str()));
        let s = en["rust"]["pill"]["card"][r.as_str()]
            .as_str()
            .unwrap_or_default();
        assert!(!s.is_empty() && !s.contains("Handy"), "{key}");
    }
}

#[test]
fn a_password_field_is_refused_and_its_text_never_broadcast() {
    use crate::fork::events::{ForkCardReason, ForkPasteOutcome};
    let d = Delivery::Card {
        reason: CardReason::SecureField,
    };
    assert_eq!(d.paste_outcome(), Some(PasteOutcome::Refused));
    let e = super::outcome_event(9, d, "pw");
    assert_eq!(e.outcome, Some(ForkPasteOutcome::Refused));
    assert_eq!(e.card_reason, Some(ForkCardReason::SecureField));
    assert_eq!(e.text, None);
    for r in [CardReason::NoTextField, CardReason::FocusMoved] {
        let e = super::outcome_event(10, Delivery::Card { reason: r }, "hi");
        assert_eq!(e.outcome, Some(ForkPasteOutcome::Card));
        assert_eq!(e.text.as_deref(), Some("hi"));
    }
}

#[test]
fn actions_rs_hook_is_in_place() {
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/actions.rs"),
    )
    .unwrap();
    assert!(src.contains("use crate::fork::insert;"));
    assert!(src.contains("match insert::deliver(final_text, ah_clone.clone()) {"));
    assert!(!src.contains("utils::paste(final_text"));
    // Handy's clipboard code is not edited (R5 §4.2, S-04).
    for f in [
        "src/clipboard.rs",
        "src/paste_tx/macos.rs",
        "src/paste_tx/mod.rs",
    ] {
        let s = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(f))
            .unwrap();
        assert!(!s.contains("fork::"), "{f} calls into the fork");
    }
}

#[test]
fn a_disk_full_signal_sends_only_that_take_to_the_card() {
    use super::session::{Interruption, SessionLog, Signal};
    let t0 = Instant::now();
    let mut log = SessionLog::new();
    log.take_started(t0);
    // fork::insert::on_disk_full: begin and end at the same instant.
    log.signal(
        Signal::Begin(Interruption::DiskFull),
        t0 + Duration::from_secs(2),
    );
    log.signal(
        Signal::End(Interruption::DiskFull),
        t0 + Duration::from_secs(2),
    );
    log.take_stopped(t0 + Duration::from_secs(2));
    assert!(!log.recording());
    assert_eq!(log.verdict(), Some(Interruption::DiskFull));
    assert_eq!(Interruption::DiskFull.card_reason(), CardReason::DiskFull);
    assert_eq!(CardReason::DiskFull.as_str(), "disk_full");
    // The next take is not affected.
    log.take_started(t0 + Duration::from_secs(5));
    log.take_stopped(t0 + Duration::from_secs(6));
    assert_eq!(log.verdict(), None);
}
