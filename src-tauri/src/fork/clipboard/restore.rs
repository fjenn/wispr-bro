// SPDX-License-Identifier: MIT AND GPL-3.0-or-later
// Copyright (c) 2026 Christopher Langton (hex: `ClipboardRestore`)
// Copyright (c) 2025 CJ Pais (Handy: `paste_tx::evaluate`)
// Modified by Wispr Bro contributors, 2026-10-30 (RS-05 spike; lifted
// into fork/clipboard by S-04 the same day, plus `is_current`).
//
// The restore guard is ported from hex `src/paste.rs` (`ClipboardRestore`
// and the restore closure of `paste_through_clipboard`), anomalyco/hex on
// GitHub, branch dev, commit 3607595d92345af4be9f0eaa86ae3750c8851e39, MIT.
// The wait rule is ported from Handy `src-tauri/src/paste_tx/mod.rs`
// (`TxState`, `evaluate`), MIT. Changes: the guard returns a decision
// instead of restoring inline (so it is testable without a pasteboard); the
// wait deadline depends on the target (R5 W-07: 1.5 s, 7 s for Chromium and
// Electron) and is counted from the ⌘V; Wispr Bro's new code is
// GPL-3.0-or-later. MIT texts: LICENSES/MIT-hex.txt, LICENSES/MIT-Handy.txt.
// See docs/fork/PORTS.md.

//! When to restore the user's clipboard after a paste, and whether to.

use serde::Serialize;
use std::time::{Duration, Instant};

use super::snapshot::Snapshot;

// ---------------------------------------------------------------------------
// The guard (hex `ClipboardRestore`)

/// Holds the user's clipboard across one or more back-to-back pastes.
#[derive(Debug, Default)]
pub struct RestoreGuard {
    generation: u64,
    original: Option<Snapshot>,
    last_change_count: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RestoreDecision {
    /// Put this back.
    Restore(Snapshot),
    /// A newer paste took over; it restores (hex: generation check).
    Superseded,
    /// Someone wrote the clipboard after our transcript (the user copied
    /// something): leave it alone and forget the snapshot.
    ClipboardChanged,
    /// Nothing held (already restored).
    NothingSaved,
}

impl RestoreDecision {
    pub fn label(&self) -> &'static str {
        match self {
            RestoreDecision::Restore(_) => "restore",
            RestoreDecision::Superseded => "superseded",
            RestoreDecision::ClipboardChanged => "clipboard_changed",
            RestoreDecision::NothingSaved => "nothing_saved",
        }
    }
}

impl RestoreGuard {
    /// An empty guard (usable in a `static`).
    pub const fn new() -> RestoreGuard {
        RestoreGuard {
            generation: 0,
            original: None,
            last_change_count: None,
        }
    }

    /// Records a paste. `previous` is the clipboard just before our write
    /// (`previous_change_count`), `inserted_change_count` the count after it.
    /// If the clipboard still holds our previous transcript (its count is the
    /// one we left), the earlier, real snapshot is carried forward instead
    /// of the transcript (R5 W-07 "back-to-back takes"). Returns this paste's
    /// generation.
    pub fn register(
        &mut self,
        previous: Snapshot,
        previous_change_count: i64,
        inserted_change_count: i64,
    ) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        if self.last_change_count != Some(previous_change_count) {
            self.original = Some(previous);
        }
        self.last_change_count = Some(inserted_change_count);
        self.generation
    }

    /// Whether this paste (`generation`) should restore now, given the
    /// pasteboard's current change count.
    pub fn decide(&mut self, generation: u64, current_change_count: i64) -> RestoreDecision {
        if self.generation != generation {
            return RestoreDecision::Superseded;
        }
        if self.last_change_count != Some(current_change_count) {
            self.original = None;
            self.last_change_count = None;
            return RestoreDecision::ClipboardChanged;
        }
        let Some(previous) = self.original.take() else {
            return RestoreDecision::NothingSaved;
        };
        self.last_change_count = None;
        RestoreDecision::Restore(previous)
    }

    /// Whether `generation` is still the latest paste (a newer one takes
    /// over the restore; the waiter of an older one can stop early).
    #[cfg_attr(not(test), allow(dead_code))] // used by fork::insert, not the spike
    pub fn is_current(&self, generation: u64) -> bool {
        self.generation == generation
    }

    /// The snapshot currently held (for reports).
    #[cfg_attr(not(test), allow(dead_code))] // S-04 API
    pub fn holding(&self) -> Option<&Snapshot> {
        self.original.as_ref()
    }
}

// ---------------------------------------------------------------------------
// The wait (Handy `paste_tx::evaluate`, deadlines per R5 W-07)

/// What kind of app receives the paste: decides how long to wait for it to
/// read the clipboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum TargetClass {
    Native,
    Chromium { major: Option<u32> },
    Electron { chromium_major: Option<u32> },
}

impl TargetClass {
    pub fn chromium_major(&self) -> Option<u32> {
        match *self {
            TargetClass::Native => None,
            TargetClass::Chromium { major } => major,
            TargetClass::Electron { chromium_major } => chromium_major,
        }
    }

    pub fn describe(&self) -> String {
        let v = |m: Option<u32>| m.map(|m| format!(" (Chromium {m})")).unwrap_or_default();
        match *self {
            TargetClass::Native => "native".into(),
            TargetClass::Chromium { major } => format!("Chromium{}", v(major)),
            TargetClass::Electron { chromium_major } => format!("Electron{}", v(chromium_major)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Timing {
    /// After the last read of our text, wait this long for more reads.
    pub quiet: Duration,
    /// No read seen: restore anyway this long after the ⌘V.
    pub native_deadline: Duration,
    pub chromium_deadline: Duration,
    /// The ⌘V could not be posted.
    pub failed_injection: Duration,
}

impl Timing {
    pub const R5: Timing = Timing {
        quiet: Duration::from_millis(200),
        native_deadline: Duration::from_millis(1500),
        chromium_deadline: Duration::from_millis(7000),
        failed_injection: Duration::from_millis(500),
    };

    pub fn deadline(&self, class: TargetClass) -> Duration {
        match class {
            TargetClass::Native => self.native_deadline,
            TargetClass::Chromium { .. } | TargetClass::Electron { .. } => self.chromium_deadline,
        }
    }
}

/// One paste's clipboard transaction, as the wait sees it.
#[derive(Clone, Debug)]
pub struct TxState {
    pub published_at: Instant,
    pub injected_at: Option<Instant>,
    pub injection_failed: bool,
    /// Every time the target (or anyone) read our text.
    pub receipts: Vec<Instant>,
    /// The pasteboard no longer holds our item (someone else wrote).
    pub ownership_lost: bool,
}

impl TxState {
    pub fn new(now: Instant) -> TxState {
        TxState {
            published_at: now,
            injected_at: None,
            injection_failed: false,
            receipts: Vec::new(),
            ownership_lost: false,
        }
    }

    pub fn last_receipt_after_injection(&self) -> Option<Instant> {
        let injected = self.injected_at?;
        self.receipts.iter().copied().rev().find(|t| *t >= injected)
    }

    /// Reads before the ⌘V (a clipboard watcher, or Chromium reading on the
    /// change notification) do not count as the paste.
    #[cfg_attr(not(test), allow(dead_code))] // S-04 API
    pub fn early_reads(&self) -> usize {
        match self.injected_at {
            Some(i) => self.receipts.iter().filter(|t| **t < i).count(),
            None => self.receipts.len(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Finish {
    /// The target read the text and then went quiet.
    ReadThenQuiet,
    /// No read in time: restoring blind.
    Deadline,
    OwnershipLost,
    InjectionFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wait {
    KeepWaiting,
    Finish(Finish),
}

pub fn evaluate(state: &TxState, timing: &Timing, class: TargetClass, now: Instant) -> Wait {
    if state.ownership_lost {
        return Wait::Finish(Finish::OwnershipLost);
    }
    if let Some(last) = state.last_receipt_after_injection() {
        if now.saturating_duration_since(last) >= timing.quiet {
            return Wait::Finish(Finish::ReadThenQuiet);
        }
    }
    let start = state.injected_at.unwrap_or(state.published_at);
    if state.injection_failed {
        if now.saturating_duration_since(start) >= timing.failed_injection {
            return Wait::Finish(Finish::InjectionFailed);
        }
        return Wait::KeepWaiting;
    }
    if now.saturating_duration_since(start) >= timing.deadline(class) {
        return Wait::Finish(Finish::Deadline);
    }
    Wait::KeepWaiting
}

#[cfg(test)]
mod tests {
    use super::super::snapshot::Item;
    use super::*;

    fn s(text: &str) -> Snapshot {
        Snapshot {
            items: vec![Item::new(&[("public.utf8-plain-text", text.as_bytes())])],
        }
    }

    #[test]
    fn single_paste_restores_when_unchanged() {
        let mut g = RestoreGuard::default();
        let gen = g.register(s("user"), 10, 11);
        assert_eq!(g.decide(gen, 11), RestoreDecision::Restore(s("user")));
        // Once only.
        assert_eq!(g.decide(gen, 11), RestoreDecision::ClipboardChanged);
    }

    #[test]
    fn user_copy_during_the_paste_wins() {
        let mut g = RestoreGuard::default();
        let gen = g.register(s("user"), 10, 11);
        assert_eq!(g.decide(gen, 12), RestoreDecision::ClipboardChanged);
        assert!(g.holding().is_none());
    }

    #[test]
    fn back_to_back_takes_carry_the_real_snapshot_forward() {
        let mut g = RestoreGuard::default();
        let first = g.register(s("user"), 10, 11);
        // Second take before the first restored: the clipboard holds the
        // first transcript (count 11), which is what it snapshots.
        let second = g.register(s("transcript 1"), 11, 12);
        assert_eq!(g.decide(first, 12), RestoreDecision::Superseded);
        assert_eq!(g.decide(second, 12), RestoreDecision::Restore(s("user")));
    }

    #[test]
    fn copy_between_takes_starts_a_new_snapshot() {
        let mut g = RestoreGuard::default();
        let first = g.register(s("user"), 10, 11);
        assert_eq!(g.decide(first, 11), RestoreDecision::Restore(s("user")));
        // Restore wrote count 12, then the user copied (13).
        let second = g.register(s("new copy"), 13, 14);
        assert_eq!(
            g.decide(second, 14),
            RestoreDecision::Restore(s("new copy"))
        );
    }

    #[test]
    fn user_copy_after_a_superseded_take() {
        let mut g = RestoreGuard::default();
        let _first = g.register(s("user"), 10, 11);
        let second = g.register(s("t1"), 11, 12);
        assert_eq!(g.decide(second, 13), RestoreDecision::ClipboardChanged);
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn deadlines_per_target() {
        let t = Timing::R5;
        assert_eq!(t.deadline(TargetClass::Native), ms(1500));
        assert_eq!(
            t.deadline(TargetClass::Chromium { major: Some(150) }),
            ms(7000)
        );
        assert_eq!(
            t.deadline(TargetClass::Electron {
                chromium_major: None
            }),
            ms(7000)
        );
    }

    #[test]
    fn read_then_quiet_finishes_early() {
        let t0 = Instant::now();
        let mut st = TxState::new(t0);
        st.injected_at = Some(t0 + ms(5));
        st.receipts.push(t0 + ms(40));
        let class = TargetClass::Native;
        assert_eq!(
            evaluate(&st, &Timing::R5, class, t0 + ms(100)),
            Wait::KeepWaiting
        );
        assert_eq!(
            evaluate(&st, &Timing::R5, class, t0 + ms(240)),
            Wait::Finish(Finish::ReadThenQuiet)
        );
        // A second read restarts the quiet period.
        st.receipts.push(t0 + ms(200));
        assert_eq!(
            evaluate(&st, &Timing::R5, class, t0 + ms(300)),
            Wait::KeepWaiting
        );
    }

    #[test]
    fn reads_before_the_paste_do_not_count() {
        let t0 = Instant::now();
        let mut st = TxState::new(t0);
        st.receipts.push(t0 + ms(1)); // a watcher read the change
        st.injected_at = Some(t0 + ms(10));
        assert_eq!(st.early_reads(), 1);
        let class = TargetClass::Chromium { major: Some(150) };
        assert_eq!(
            evaluate(&st, &Timing::R5, class, t0 + ms(1000)),
            Wait::KeepWaiting
        );
        assert_eq!(
            evaluate(&st, &Timing::R5, class, t0 + ms(7010)),
            Wait::Finish(Finish::Deadline)
        );
    }

    #[test]
    fn native_deadline_counts_from_the_paste() {
        let t0 = Instant::now();
        let mut st = TxState::new(t0);
        st.injected_at = Some(t0 + ms(300)); // modifier wait before ⌘V
        let class = TargetClass::Native;
        assert_eq!(
            evaluate(&st, &Timing::R5, class, t0 + ms(1600)),
            Wait::KeepWaiting
        );
        assert_eq!(
            evaluate(&st, &Timing::R5, class, t0 + ms(1800)),
            Wait::Finish(Finish::Deadline)
        );
    }

    #[test]
    fn failed_injection_and_lost_ownership() {
        let t0 = Instant::now();
        let mut st = TxState::new(t0);
        st.injected_at = Some(t0);
        st.injection_failed = true;
        let class = TargetClass::Native;
        assert_eq!(
            evaluate(&st, &Timing::R5, class, t0 + ms(400)),
            Wait::KeepWaiting
        );
        assert_eq!(
            evaluate(&st, &Timing::R5, class, t0 + ms(500)),
            Wait::Finish(Finish::InjectionFailed)
        );
        st.ownership_lost = true;
        assert_eq!(
            evaluate(&st, &Timing::R5, class, t0),
            Wait::Finish(Finish::OwnershipLost)
        );
    }
}
