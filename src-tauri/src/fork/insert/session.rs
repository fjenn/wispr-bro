// SPDX-License-Identifier: GPL-3.0-or-later
//! Sleep, screen lock and user switching during a take (R5 S-04, §5.4
//! negative cases): the recording stops, the audio is kept, and the result
//! goes to the card after wake. Nothing is pasted blind.
//!
//! [`SessionLog`] is the pure part: it records the takes' recording spans
//! (from Handy's `recording-state-event`) and the interruptions, and answers
//! "was this take interrupted?" at delivery. [`mac`] subscribes to the
//! system's notifications:
//!
//! | Notification                                   | Meaning              |
//! | ---------------------------------------------- | -------------------- |
//! | `NSWorkspaceWillSleepNotification` / `DidWake` | sleep, lid close     |
//! | `com.apple.screenIsLocked` / `…Unlocked`       | screen lock          |
//! | `NSWorkspaceSessionDidResignActive` / `…Become`| fast user switching  |
//!
//! Display sleep alone (`ScreensDidSleep`) is not an interruption: a long
//! hands-free take may outlast the display timeout.

use std::collections::VecDeque;
use std::time::Instant;

use super::deliver::CardReason;

/// A system event that interrupts a take.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Interruption {
    Sleep,
    ScreenLocked,
    SessionInactive,
    /// The disk filled up during the recording (S-09, `fork::db::disk`).
    /// Signalled as a begin and end at the same instant.
    DiskFull,
}

impl Interruption {
    pub fn card_reason(self) -> CardReason {
        match self {
            Interruption::Sleep => CardReason::Sleep,
            Interruption::ScreenLocked => CardReason::ScreenLocked,
            Interruption::SessionInactive => CardReason::SessionInactive,
            Interruption::DiskFull => CardReason::DiskFull,
        }
    }
}

/// What the system told us.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    Begin(Interruption),
    End(Interruption),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Span {
    start: Instant,
    stop: Option<Instant>,
}

const KEEP: usize = 32;

/// Takes and interruptions, newest last.
#[derive(Debug, Default)]
pub struct SessionLog {
    spans: VecDeque<Span>,
    events: VecDeque<(Instant, Interruption)>,
    /// Interruptions in progress (begun, not yet ended).
    active: Vec<Interruption>,
}

fn push_bounded<T>(q: &mut VecDeque<T>, v: T) {
    if q.len() == KEEP {
        q.pop_front();
    }
    q.push_back(v);
}

impl SessionLog {
    /// An empty log (usable in a `static`).
    pub const fn new() -> SessionLog {
        SessionLog {
            spans: VecDeque::new(),
            events: VecDeque::new(),
            active: Vec::new(),
        }
    }

    /// A recording started.
    pub fn take_started(&mut self, at: Instant) {
        push_bounded(
            &mut self.spans,
            Span {
                start: at,
                stop: None,
            },
        );
    }

    /// The recording stopped (transcription follows).
    pub fn take_stopped(&mut self, at: Instant) {
        if let Some(s) = self.spans.back_mut().filter(|s| s.stop.is_none()) {
            s.stop = Some(at);
        }
    }

    /// The take was cancelled: it will never be delivered.
    pub fn take_cancelled(&mut self) {
        if self.spans.back().is_some_and(|s| s.stop.is_none()) {
            self.spans.pop_back();
        }
    }

    pub fn signal(&mut self, s: Signal, at: Instant) {
        match s {
            Signal::Begin(i) => {
                push_bounded(&mut self.events, (at, i));
                if !self.active.contains(&i) {
                    self.active.push(i);
                }
            }
            Signal::End(i) => self.active.retain(|a| *a != i),
        }
    }

    /// Whether a recording is open (started, not stopped).
    pub fn recording(&self) -> bool {
        self.spans.back().is_some_and(|s| s.stop.is_none())
    }

    /// Asked at delivery: the reason to send this take to the card instead
    /// of pasting, if any. The take being delivered is the latest one whose
    /// recording stopped (Handy processes one take at a time; the next one
    /// may already be recording). Without a recorded span, only an
    /// interruption still in progress counts.
    pub fn verdict(&self) -> Option<Interruption> {
        if let Some(i) = self.active.first() {
            return Some(*i);
        }
        let span = self.spans.iter().rev().find(|s| s.stop.is_some())?;
        self.events
            .iter()
            .rev()
            .find(|(t, _)| *t >= span.start)
            .map(|(_, i)| *i)
    }
}

// ---------------------------------------------------------------------------
// macOS notifications

#[cfg(target_os = "macos")]
pub mod mac {
    use super::{Interruption, Signal};
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::{define_class, msg_send, sel, AnyThread, DefinedClass};
    use objc2_app_kit::{
        NSWorkspace, NSWorkspaceDidWakeNotification, NSWorkspaceSessionDidBecomeActiveNotification,
        NSWorkspaceSessionDidResignActiveNotification, NSWorkspaceWillSleepNotification,
    };
    use objc2_foundation::{NSDistributedNotificationCenter, NSNotification, NSObject, NSString};

    pub const SCREEN_LOCKED: &str = "com.apple.screenIsLocked";
    pub const SCREEN_UNLOCKED: &str = "com.apple.screenIsUnlocked";

    type Callback = Box<dyn Fn(Signal) + Send + Sync>;

    pub struct ObserverIvars {
        on_signal: Callback,
    }

    define_class!(
        // SAFETY: NSObject has no subclassing requirements; the ivar is a
        // Send + Sync Rust closure.
        #[unsafe(super(NSObject))]
        #[name = "WispBroSessionObserver"]
        #[ivars = ObserverIvars]
        pub struct SessionObserver;

        impl SessionObserver {
            #[unsafe(method(willSleep:))]
            fn will_sleep(&self, _n: &NSNotification) {
                (self.ivars().on_signal)(Signal::Begin(Interruption::Sleep));
            }
            #[unsafe(method(didWake:))]
            fn did_wake(&self, _n: &NSNotification) {
                (self.ivars().on_signal)(Signal::End(Interruption::Sleep));
            }
            #[unsafe(method(sessionResigned:))]
            fn session_resigned(&self, _n: &NSNotification) {
                (self.ivars().on_signal)(Signal::Begin(Interruption::SessionInactive));
            }
            #[unsafe(method(sessionBecameActive:))]
            fn session_active(&self, _n: &NSNotification) {
                (self.ivars().on_signal)(Signal::End(Interruption::SessionInactive));
            }
            #[unsafe(method(screenLocked:))]
            fn screen_locked(&self, _n: &NSNotification) {
                (self.ivars().on_signal)(Signal::Begin(Interruption::ScreenLocked));
            }
            #[unsafe(method(screenUnlocked:))]
            fn screen_unlocked(&self, _n: &NSNotification) {
                (self.ivars().on_signal)(Signal::End(Interruption::ScreenLocked));
            }
        }
    );

    /// Subscribes `on_signal` to the six notifications. Call once, on the
    /// main thread; the observer lives for the rest of the process.
    pub fn observe(on_signal: impl Fn(Signal) + Send + Sync + 'static) {
        let this = SessionObserver::alloc().set_ivars(ObserverIvars {
            on_signal: Box::new(on_signal),
        });
        // SAFETY: NSObject's init.
        let observer: Retained<SessionObserver> = unsafe { msg_send![super(this), init] };
        let obj: &AnyObject = &observer;
        let workspace = NSWorkspace::sharedWorkspace().notificationCenter();
        // SAFETY: the selectors exist on the observer with the
        // `(NSNotification*)` signature; the observer is never deallocated
        // (forgotten below), so the centers never message a dead object.
        unsafe {
            for (sel, name) in [
                (sel!(willSleep:), NSWorkspaceWillSleepNotification),
                (sel!(didWake:), NSWorkspaceDidWakeNotification),
                (
                    sel!(sessionResigned:),
                    NSWorkspaceSessionDidResignActiveNotification,
                ),
                (
                    sel!(sessionBecameActive:),
                    NSWorkspaceSessionDidBecomeActiveNotification,
                ),
            ] {
                workspace.addObserver_selector_name_object(obj, sel, Some(name), None);
            }
            let distributed = NSDistributedNotificationCenter::defaultCenter();
            for (sel, name) in [
                (sel!(screenLocked:), SCREEN_LOCKED),
                (sel!(screenUnlocked:), SCREEN_UNLOCKED),
            ] {
                let name = NSString::from_str(name);
                distributed.addObserver_selector_name_object(obj, sel, Some(&name), None);
            }
        }
        std::mem::forget(observer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn at(t0: Instant, ms: u64) -> Instant {
        t0 + Duration::from_millis(ms)
    }

    #[test]
    fn a_quiet_take_pastes() {
        let t0 = Instant::now();
        let mut s = SessionLog::default();
        s.take_started(at(t0, 0));
        s.take_stopped(at(t0, 1000));
        assert_eq!(s.verdict(), None);
    }

    #[test]
    fn sleep_during_recording_goes_to_the_card_after_wake() {
        let t0 = Instant::now();
        let mut s = SessionLog::default();
        s.take_started(at(t0, 0));
        s.signal(Signal::Begin(Interruption::Sleep), at(t0, 500));
        assert!(s.recording());
        // Still asleep: card.
        assert_eq!(s.verdict(), Some(Interruption::Sleep));
        s.signal(Signal::End(Interruption::Sleep), at(t0, 60_000));
        s.take_stopped(at(t0, 60_100));
        // Awake again, but the take was interrupted.
        assert_eq!(s.verdict(), Some(Interruption::Sleep));
    }

    #[test]
    fn sleep_during_processing_goes_to_the_card() {
        let t0 = Instant::now();
        let mut s = SessionLog::default();
        s.take_started(at(t0, 0));
        s.take_stopped(at(t0, 1000));
        s.signal(Signal::Begin(Interruption::SessionInactive), at(t0, 1200));
        s.signal(Signal::End(Interruption::SessionInactive), at(t0, 9000));
        assert_eq!(s.verdict(), Some(Interruption::SessionInactive));
    }

    #[test]
    fn an_old_interruption_does_not_card_a_later_take() {
        let t0 = Instant::now();
        let mut s = SessionLog::default();
        s.signal(Signal::Begin(Interruption::ScreenLocked), at(t0, 0));
        s.signal(Signal::End(Interruption::ScreenLocked), at(t0, 100));
        s.take_started(at(t0, 200));
        s.take_stopped(at(t0, 900));
        assert_eq!(s.verdict(), None);
    }

    #[test]
    fn locked_now_cards_even_without_a_span() {
        let t0 = Instant::now();
        let mut s = SessionLog::default();
        assert_eq!(s.verdict(), None);
        s.signal(Signal::Begin(Interruption::ScreenLocked), at(t0, 0));
        assert_eq!(s.verdict(), Some(Interruption::ScreenLocked));
        // A second Begin of the same kind is not counted twice.
        s.signal(Signal::Begin(Interruption::ScreenLocked), at(t0, 1));
        s.signal(Signal::End(Interruption::ScreenLocked), at(t0, 2));
        assert_eq!(s.verdict(), None);
    }

    #[test]
    fn the_next_take_recording_does_not_hide_the_delivered_one() {
        // Take 1 stopped and was interrupted while processing; take 2 starts
        // before take 1 is delivered (Handy frees the pipeline before the
        // paste thread finishes).
        let t0 = Instant::now();
        let mut s = SessionLog::default();
        s.take_started(at(t0, 0));
        s.take_stopped(at(t0, 1000));
        s.signal(Signal::Begin(Interruption::Sleep), at(t0, 1100));
        s.signal(Signal::End(Interruption::Sleep), at(t0, 5000));
        s.take_started(at(t0, 5100));
        assert_eq!(s.verdict(), Some(Interruption::Sleep));
        // Take 2 then stops cleanly: its own delivery pastes.
        s.take_stopped(at(t0, 6000));
        assert_eq!(s.verdict(), None);
    }

    #[test]
    fn cancelled_take_is_forgotten() {
        let t0 = Instant::now();
        let mut s = SessionLog::default();
        s.take_started(at(t0, 0));
        s.take_stopped(at(t0, 100));
        s.take_started(at(t0, 200));
        s.take_cancelled();
        assert!(!s.recording());
        // The stopped take is still the one being delivered.
        s.signal(Signal::Begin(Interruption::Sleep), at(t0, 150));
        assert_eq!(s.verdict(), Some(Interruption::Sleep));
    }

    #[test]
    fn bounded_history() {
        let t0 = Instant::now();
        let mut s = SessionLog::default();
        for i in 0..100 {
            s.take_started(at(t0, i * 10));
            s.take_stopped(at(t0, i * 10 + 5));
            s.signal(Signal::Begin(Interruption::Sleep), at(t0, i * 10 + 1));
            s.signal(Signal::End(Interruption::Sleep), at(t0, i * 10 + 2));
        }
        assert_eq!(s.spans.len(), KEEP);
        assert_eq!(s.events.len(), KEEP);
        assert_eq!(s.verdict(), Some(Interruption::Sleep));
        assert_eq!(
            Interruption::ScreenLocked.card_reason(),
            CardReason::ScreenLocked
        );
    }
}
