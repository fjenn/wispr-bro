// SPDX-License-Identifier: GPL-3.0-only
// Copyright (c) the VoiceInk authors (VoiceInk, GPL-3.0)
// Modified by Wispr Bro contributors, 2026-10-30.
//
// Ported from VoiceInk
// `VoiceInk/Infrastructure/SystemIntegration/Accessibility/AccessibilityTextReader.swift`,
// class `AccessibilityWebAccess` (https://github.com/Beingpax/VoiceInk, commit
// 19546d2a7f85578f49efb538eb888b37ff2f8f8c), GPL-3.0. Kept: the per-PID
// reader refcount, remembering the value the app had before we switched it
// on and restoring exactly that, and the retry back-off after a failed
// enable (60 s for "unsupported"/"not implemented", 1 s otherwise).
// Changed: Swift → Rust; generic over the key and the clock so it is pure
// and unit-tested; the restore no longer happens at refcount 0 but `hold`
// after the last release (SpeakoFlow's 60 s lease, R5 §4.6), so a burst of
// reads does not toggle the app's tree on and off; `force_off` for RS-04's
// with/without comparison.

//! The `AXManualAccessibility` lease (R5 §4.6): on at the first read of a
//! Chromium/Electron app, off `hold` after the last read, refcounted per
//! PID, and restored to what the app had before (if something else, such as
//! a screen reader or another dictation app, had it on, we leave it on).

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

pub const HOLD: Duration = Duration::from_secs(60);

/// What the caller's `enable` closure reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnableResult {
    /// Switched on; `previous` is the value read before writing.
    Enabled { previous: bool },
    /// The write (or the read before it) failed.
    Failed { unsupported: bool },
}

#[derive(Clone, Debug)]
struct Entry {
    readers: u32,
    previous: bool,
    last_used: Instant,
}

#[derive(Debug)]
pub struct LeaseTable<K: Eq + Hash + Copy> {
    entries: HashMap<K, Entry>,
    retry_after: HashMap<K, Instant>,
    hold: Duration,
}

impl<K: Eq + Hash + Copy> LeaseTable<K> {
    pub fn new(hold: Duration) -> Self {
        LeaseTable {
            entries: HashMap::new(),
            retry_after: HashMap::new(),
            hold,
        }
    }

    /// One more reader for `key`. Calls `enable` only if no lease is held.
    /// Returns whether the tree is (now) on through us.
    pub fn acquire(&mut self, key: K, now: Instant, enable: impl FnOnce() -> EnableResult) -> bool {
        if let Some(e) = self.entries.get_mut(&key) {
            e.readers += 1;
            e.last_used = now;
            return true;
        }
        self.retry_after.retain(|_, until| *until > now);
        if self.retry_after.contains_key(&key) {
            return false;
        }
        match enable() {
            EnableResult::Enabled { previous } => {
                self.entries.insert(
                    key,
                    Entry {
                        readers: 1,
                        previous,
                        last_used: now,
                    },
                );
                true
            }
            EnableResult::Failed { unsupported } => {
                let delay = if unsupported {
                    Duration::from_secs(60)
                } else {
                    Duration::from_secs(1)
                };
                self.retry_after.insert(key, now + delay);
                false
            }
        }
    }

    /// One reader done. The lease stays until `expire` finds it idle for `hold`.
    pub fn release(&mut self, key: K, now: Instant) {
        if let Some(e) = self.entries.get_mut(&key) {
            e.readers = e.readers.saturating_sub(1);
            e.last_used = now;
        }
    }

    /// Restores every idle lease older than `hold`. `restore(key, previous)`
    /// is only called when the app had it off before (previous == false):
    /// writing `true` back would be a no-op at best.
    pub fn expire(&mut self, now: Instant, mut restore: impl FnMut(K)) -> usize {
        let hold = self.hold;
        let mut n = 0;
        self.entries.retain(|k, e| {
            let idle = e.readers == 0 && now.saturating_duration_since(e.last_used) >= hold;
            if idle {
                n += 1;
                if !e.previous {
                    restore(*k);
                }
            }
            !idle
        });
        n
    }

    /// Ends the lease for `key` now, whatever the refcount (RS-04's
    /// "without lease" probe; process exit). Returns true if a write back
    /// is needed (the app had it off before we switched it on).
    pub fn force_off(&mut self, key: K) -> bool {
        self.entries.remove(&key).is_some_and(|e| !e.previous)
    }

    /// Every key whose value must be written back (at exit).
    pub fn drain_restore(&mut self) -> Vec<K> {
        let keys: Vec<K> = self
            .entries
            .iter()
            .filter(|(_, e)| !e.previous)
            .map(|(k, _)| *k)
            .collect();
        self.entries.clear();
        keys
    }

    pub fn held(&self, key: K) -> bool {
        self.entries.contains_key(&key)
    }

    #[allow(dead_code)] // diagnostics; used by the tests
    pub fn readers(&self, key: K) -> u32 {
        self.entries.get(&key).map_or(0, |e| e.readers)
    }

    /// The value the app had before our first write, if we hold a lease.
    pub fn previous(&self, key: K) -> Option<bool> {
        self.entries.get(&key).map(|e| e.previous)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on(previous: bool) -> impl FnOnce() -> EnableResult {
        move || EnableResult::Enabled { previous }
    }

    #[test]
    fn enables_once_and_counts_readers() {
        let t0 = Instant::now();
        let mut t = LeaseTable::new(HOLD);
        let mut calls = 0;
        assert!(t.acquire(7, t0, || {
            calls += 1;
            EnableResult::Enabled { previous: false }
        }));
        assert!(t.acquire(7, t0, || panic!("must not enable twice")));
        assert_eq!(calls, 1);
        assert_eq!(t.readers(7), 2);
        t.release(7, t0);
        t.release(7, t0);
        assert_eq!(t.readers(7), 0);
        assert!(t.held(7), "still held until the hold expires");
    }

    #[test]
    fn restores_only_after_hold_and_only_if_it_was_off() {
        let t0 = Instant::now();
        let mut t = LeaseTable::new(HOLD);
        t.acquire(1, t0, on(false));
        t.acquire(2, t0, on(true)); // someone else had it on
        t.release(1, t0);
        t.release(2, t0);
        let mut restored = Vec::new();
        assert_eq!(
            t.expire(t0 + Duration::from_secs(59), |k| restored.push(k)),
            0
        );
        assert_eq!(t.expire(t0 + HOLD, |k| restored.push(k)), 2);
        assert_eq!(restored, vec![1], "the app that had it on is left on");
        assert!(!t.held(1) && !t.held(2));
    }

    #[test]
    fn a_reader_in_flight_keeps_the_lease() {
        let t0 = Instant::now();
        let mut t = LeaseTable::new(HOLD);
        t.acquire(1, t0, on(false));
        let mut restored = 0;
        t.expire(t0 + Duration::from_secs(600), |_| restored += 1);
        assert_eq!(restored, 0);
        assert!(t.held(1));
    }

    #[test]
    fn a_new_read_extends_the_hold() {
        let t0 = Instant::now();
        let mut t = LeaseTable::new(HOLD);
        t.acquire(1, t0, on(false));
        t.release(1, t0);
        let t1 = t0 + Duration::from_secs(50);
        t.acquire(1, t1, || panic!("held"));
        t.release(1, t1);
        let mut n = 0;
        t.expire(t0 + Duration::from_secs(70), |_| n += 1);
        assert_eq!(n, 0, "50 s + 60 s hold not reached at 70 s");
        t.expire(t1 + HOLD, |_| n += 1);
        assert_eq!(n, 1);
    }

    #[test]
    fn failed_enable_backs_off() {
        let t0 = Instant::now();
        let mut t = LeaseTable::new(HOLD);
        assert!(!t.acquire(3, t0, || EnableResult::Failed { unsupported: true }));
        assert!(!t.acquire(3, t0 + Duration::from_secs(59), || panic!("in back-off")));
        assert!(t.acquire(3, t0 + Duration::from_secs(60), on(false)));
        assert!(!t.acquire(4, t0, || EnableResult::Failed { unsupported: false }));
        assert!(t.acquire(4, t0 + Duration::from_secs(1), on(false)));
    }

    #[test]
    fn force_off_and_drain() {
        let t0 = Instant::now();
        let mut t = LeaseTable::new(HOLD);
        t.acquire(1, t0, on(false));
        t.acquire(2, t0, on(true));
        t.acquire(3, t0, on(false));
        assert!(t.force_off(1));
        assert!(!t.force_off(1), "already gone");
        assert!(!t.force_off(2), "was on before: no write back");
        assert_eq!(t.drain_restore(), vec![3]);
        assert!(!t.held(3));
    }
}
