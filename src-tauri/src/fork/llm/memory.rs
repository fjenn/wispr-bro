// SPDX-License-Identifier: GPL-3.0-or-later
//! Memory policy and ordered loads (R5 §4.5, D-213, §5.2).
//!
//! - **Headroom rule.** Start the LLM only if available memory is at least
//!   the model's resident size plus 1.5 GB; otherwise that take runs without
//!   cleanup ("Inserted without cleanup").
//! - **Ordered loads.** The speech model and the LLM never load at the same
//!   time. At key-down: ASR loaded → wake the LLM now; ASR cold → wait until
//!   ASR reports loaded or until key-up, whichever comes first.
//!
//! [`LoadPlanner`] is the pure state machine; `super::runtime` feeds it
//! Handy's `recording-state-event` and `model-state-changed` events.

/// Headroom kept free beside the LLM (R5 §4.5).
pub const HEADROOM_BYTES: u64 = 1536 * 1024 * 1024;

/// Memory figures for the policy and the debug panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemorySnapshot {
    /// `hw.memsize`.
    pub physical_bytes: u64,
    /// Free + inactive + purgeable pages: memory the kernel can hand out
    /// without swapping (the same pages `vm_stat` lists). Speculative pages
    /// are already part of `free_count`.
    pub available_bytes: u64,
}

/// `true` when an LLM with `resident_bytes` may be started now.
pub fn may_start(available_bytes: u64, resident_bytes: u64) -> bool {
    available_bytes >= resident_bytes.saturating_add(HEADROOM_BYTES)
}

/// Reads the machine's memory. `None` if the kernel call fails or off macOS.
#[cfg(target_os = "macos")]
pub fn snapshot() -> Option<MemorySnapshot> {
    use std::mem::{size_of, MaybeUninit};

    let mut physical: u64 = 0;
    let mut len = size_of::<u64>();
    // SAFETY: `hw.memsize` is a u64; buffer and length match.
    let rc = unsafe {
        libc::sysctlbyname(
            c"hw.memsize".as_ptr(),
            (&mut physical as *mut u64).cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 {
        return None;
    }

    let mut stats = MaybeUninit::<libc::vm_statistics64>::zeroed();
    let mut count = libc::HOST_VM_INFO64_COUNT;
    // SAFETY: the buffer is a `vm_statistics64` and `count` is its size in
    // `integer_t` units, as `host_statistics64` requires.
    #[allow(deprecated)] // libc marks mach_host_self deprecated in favour of the mach2 crate
    let kr = unsafe {
        libc::host_statistics64(
            libc::mach_host_self(),
            libc::HOST_VM_INFO64,
            stats.as_mut_ptr().cast(),
            &mut count,
        )
    };
    if kr != libc::KERN_SUCCESS {
        return None;
    }
    // SAFETY: filled by the successful call above.
    let stats = unsafe { stats.assume_init() };
    // SAFETY: plain libc call.
    let page = u64::try_from(unsafe { libc::sysconf(libc::_SC_PAGESIZE) }).ok()?;
    let pages = u64::from(stats.free_count)
        + u64::from(stats.inactive_count)
        + u64::from(stats.purgeable_count);
    Some(MemorySnapshot {
        physical_bytes: physical,
        available_bytes: pages.saturating_mul(page).min(physical),
    })
}

#[cfg(not(target_os = "macos"))]
pub fn snapshot() -> Option<MemorySnapshot> {
    None
}

/// State of Handy's speech model as the planner sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsrState {
    Loaded,
    Loading,
    Unloaded,
}

/// What the runtime must do after an input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanAction {
    /// Start (or wake) the LLM now.
    StartLlm,
    /// Nothing to do yet.
    Wait,
}

/// Ordered-load state machine for one take at a time.
#[derive(Debug, Default, Clone)]
pub struct LoadPlanner {
    /// A take wants the LLM, but the speech model is still cold.
    pending: bool,
    asr: Option<AsrState>,
}

impl LoadPlanner {
    /// Handy's speech model changed state (`model-state-changed`).
    pub fn on_asr(&mut self, state: AsrState) -> PlanAction {
        self.asr = Some(state);
        if state == AsrState::Loaded && self.pending {
            self.pending = false;
            return PlanAction::StartLlm;
        }
        PlanAction::Wait
    }

    /// Recording started (fn down). `asr_loaded` is read from Handy's
    /// transcription manager at that moment.
    pub fn on_key_down(&mut self, asr_loaded: bool) -> PlanAction {
        if asr_loaded {
            self.asr = Some(AsrState::Loaded);
        } else if self.asr == Some(AsrState::Loaded) {
            // The event stream said loaded but the manager says not: it was
            // unloaded in between (Handy's idle unload emits an event too;
            // trust the direct read).
            self.asr = Some(AsrState::Unloaded);
        }
        if self.asr == Some(AsrState::Loaded) {
            self.pending = false;
            PlanAction::StartLlm
        } else {
            self.pending = true;
            PlanAction::Wait
        }
    }

    /// Recording stopped (fn up). A pending start runs now even if ASR is
    /// still loading: the take is about to need the LLM.
    pub fn on_key_up(&mut self) -> PlanAction {
        if std::mem::take(&mut self.pending) {
            PlanAction::StartLlm
        } else {
            PlanAction::Wait
        }
    }

    /// The take was cancelled: forget a pending start.
    pub fn on_cancel(&mut self) {
        self.pending = false;
    }

    /// `true` while a start waits for the speech model.
    pub fn is_pending(&self) -> bool {
        self.pending
    }
}

/// Maps Handy's `ModelStateEvent::event_type` to a planner state.
pub fn asr_state_from_event(event_type: &str) -> Option<AsrState> {
    match event_type {
        "loading_started" => Some(AsrState::Loading),
        "loading_completed" => Some(AsrState::Loaded),
        "loading_failed" | "unloaded" => Some(AsrState::Unloaded),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1024 * 1024 * 1024;

    #[test]
    fn headroom_rule_needs_model_plus_one_and_a_half_gb() {
        let model = 2 * GB;
        assert!(may_start(model + HEADROOM_BYTES, model));
        assert!(!may_start(model + HEADROOM_BYTES - 1, model));
        assert!(!may_start(0, model));
        assert!(may_start(u64::MAX, u64::MAX));
    }

    #[test]
    fn warm_asr_wakes_the_llm_at_key_down() {
        let mut p = LoadPlanner::default();
        assert_eq!(p.on_key_down(true), PlanAction::StartLlm);
        assert!(!p.is_pending());
        assert_eq!(p.on_key_up(), PlanAction::Wait);
    }

    #[test]
    fn cold_asr_never_overlaps_an_llm_load() {
        let mut p = LoadPlanner::default();
        assert_eq!(p.on_key_down(false), PlanAction::Wait);
        assert_eq!(p.on_asr(AsrState::Loading), PlanAction::Wait);
        assert!(p.is_pending());
        // ASR finishes while the user still speaks: LLM starts then.
        assert_eq!(p.on_asr(AsrState::Loaded), PlanAction::StartLlm);
        // …and key-up does not start it twice.
        assert_eq!(p.on_key_up(), PlanAction::Wait);
    }

    #[test]
    fn key_up_starts_a_pending_llm_when_asr_is_slower() {
        let mut p = LoadPlanner::default();
        assert_eq!(p.on_key_down(false), PlanAction::Wait);
        assert_eq!(p.on_asr(AsrState::Loading), PlanAction::Wait);
        assert_eq!(p.on_key_up(), PlanAction::StartLlm);
        // A late "loaded" does not start it again.
        assert_eq!(p.on_asr(AsrState::Loaded), PlanAction::Wait);
    }

    #[test]
    fn cancel_drops_a_pending_start() {
        let mut p = LoadPlanner::default();
        p.on_key_down(false);
        p.on_cancel();
        assert_eq!(p.on_asr(AsrState::Loaded), PlanAction::Wait);
        assert_eq!(p.on_key_up(), PlanAction::Wait);
    }

    #[test]
    fn an_unload_between_takes_is_seen_at_key_down() {
        let mut p = LoadPlanner::default();
        p.on_asr(AsrState::Loaded);
        // Handy unloaded the model (idle) and the direct read says so.
        assert_eq!(p.on_key_down(false), PlanAction::Wait);
        assert_eq!(p.on_asr(AsrState::Loaded), PlanAction::StartLlm);
        // Failed load: key-up still starts the LLM (the take fails open
        // later if ASR has nothing to transcribe).
        p.on_key_down(false);
        assert_eq!(p.on_asr(AsrState::Unloaded), PlanAction::Wait);
        assert_eq!(p.on_key_up(), PlanAction::StartLlm);
    }

    #[test]
    fn handy_event_types_map() {
        assert_eq!(
            asr_state_from_event("loading_started"),
            Some(AsrState::Loading)
        );
        assert_eq!(
            asr_state_from_event("loading_completed"),
            Some(AsrState::Loaded)
        );
        assert_eq!(
            asr_state_from_event("loading_failed"),
            Some(AsrState::Unloaded)
        );
        assert_eq!(asr_state_from_event("unloaded"), Some(AsrState::Unloaded));
        assert_eq!(asr_state_from_event("other"), None);
        // The strings are Handy's (managers/transcription.rs).
        let src = include_str!("../../managers/transcription.rs");
        for t in [
            "loading_started",
            "loading_completed",
            "loading_failed",
            "unloaded",
        ] {
            assert!(src.contains(&format!("event_type: \"{t}\"")), "{t}");
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn snapshot_reads_plausible_numbers() {
        let s = snapshot().expect("snapshot");
        assert!(s.physical_bytes >= 4 * GB);
        assert!(s.available_bytes > 0 && s.available_bytes <= s.physical_bytes);
    }
}
