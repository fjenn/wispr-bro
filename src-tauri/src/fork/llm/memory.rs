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
//! - **Critical pressure (S-38).** While macOS reports critical memory
//!   pressure (`kern.memorystatus_vm_pressure_level` 4, the level at which
//!   it starts killing processes), no take uses the LLM, even a running one,
//!   and the idle owner stops an idle server at once instead of after
//!   120 s, so its memory goes back to the system. Warn pressure changes
//!   nothing: a running server has already paid for its memory, and the
//!   cleanup deadline fails a slow reply open.
//!
//! [`LoadPlanner`] is the pure state machine; `super::runtime` feeds it
//! Handy's `recording-state-event` and `model-state-changed` events.
//! [`allows`] and [`release_idle`] are the pure memory rule.
//!
//! Debug builds read `WB_DEBUG_MEMORY` (e.g.
//! `physical_gb=8,available_mb=2500,pressure=critical`) over the real
//! figures, so an 8 GB Mac's tier, headroom and pressure paths run on any
//! Mac (Mac test S-38.7). The speech-model policy reads the same physical
//! override ([`simulated_physical_bytes`]).

/// Headroom kept free beside the LLM (R5 §4.5).
pub const HEADROOM_BYTES: u64 = 1536 * 1024 * 1024;

/// macOS's memory pressure level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pressure {
    #[default]
    Normal,
    Warn,
    Critical,
}

impl Pressure {
    /// `kern.memorystatus_vm_pressure_level`: 1 normal, 2 warn, 4 critical
    /// (`DISPATCH_MEMORYPRESSURE_*`); anything else reads as normal.
    pub fn from_level(level: i32) -> Self {
        match level {
            2 => Pressure::Warn,
            4 => Pressure::Critical,
            _ => Pressure::Normal,
        }
    }
}

/// Memory figures for the policy and the debug panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemorySnapshot {
    /// `hw.memsize`.
    pub physical_bytes: u64,
    /// Free + inactive + purgeable pages: memory the kernel can hand out
    /// without swapping (the same pages `vm_stat` lists). Speculative pages
    /// are already part of `free_count`.
    pub available_bytes: u64,
    pub pressure: Pressure,
}

/// `true` when an LLM with `resident_bytes` may be started now.
pub fn may_start(available_bytes: u64, resident_bytes: u64) -> bool {
    available_bytes >= resident_bytes.saturating_add(HEADROOM_BYTES)
}

/// The memory rule for one take (R5 §5.2): may it use the LLM? A server
/// that is `running` already holds its memory and passes unless pressure is
/// critical; a stopped one must also fit beside the headroom. No reading
/// (off macOS, a failed kernel call) allows it: the deadline still fails a
/// stalled reply open.
pub fn allows(running: bool, snapshot: Option<MemorySnapshot>, resident_bytes: u64) -> bool {
    let Some(m) = snapshot else { return true };
    if m.pressure == Pressure::Critical {
        return false;
    }
    running || may_start(m.available_bytes, resident_bytes)
}

/// `true` when an idle server should be stopped now rather than after the
/// idle timeout: macOS reports critical pressure.
pub fn release_idle(snapshot: Option<MemorySnapshot>) -> bool {
    snapshot.is_some_and(|m| m.pressure == Pressure::Critical)
}

/// The debug override (`WB_DEBUG_MEMORY`), parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Override {
    pub physical_bytes: Option<u64>,
    pub available_bytes: Option<u64>,
    pub pressure: Option<Pressure>,
}

/// Debug builds: the variable that simulates another Mac's memory.
#[cfg(debug_assertions)]
pub const OVERRIDE_ENV: &str = "WB_DEBUG_MEMORY";

/// Parses `physical_gb=8,available_mb=2500,pressure=normal|warn|critical`
/// (any subset, any order). Unknown or malformed parts are ignored.
pub fn parse_override(spec: &str) -> Override {
    const MB: u64 = 1024 * 1024;
    let mut o = Override::default();
    for part in spec.split(',') {
        let Some((k, v)) = part.split_once('=') else {
            continue;
        };
        let (k, v) = (k.trim(), v.trim());
        let num = v.parse::<u64>().ok();
        match k {
            "physical_gb" => o.physical_bytes = num.map(|n| n.saturating_mul(1024 * MB)),
            "available_mb" => o.available_bytes = num.map(|n| n.saturating_mul(MB)),
            "pressure" => {
                o.pressure = match v {
                    "normal" => Some(Pressure::Normal),
                    "warn" => Some(Pressure::Warn),
                    "critical" => Some(Pressure::Critical),
                    _ => None,
                }
            }
            _ => {}
        }
    }
    o
}

impl Override {
    /// `real` with the overridden figures (available never above physical).
    pub fn apply(self, real: MemorySnapshot) -> MemorySnapshot {
        let physical_bytes = self.physical_bytes.unwrap_or(real.physical_bytes);
        MemorySnapshot {
            physical_bytes,
            available_bytes: self
                .available_bytes
                .unwrap_or(real.available_bytes)
                .min(physical_bytes),
            pressure: self.pressure.unwrap_or(real.pressure),
        }
    }
}

fn debug_override() -> Option<Override> {
    #[cfg(debug_assertions)]
    {
        std::env::var(OVERRIDE_ENV)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .map(|v| parse_override(&v))
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

/// The physical memory a debug build simulates (`WB_DEBUG_MEMORY`
/// `physical_gb`), for the speech-model policy; `None` in release builds
/// and without the variable.
pub fn simulated_physical_bytes() -> Option<u64> {
    debug_override().and_then(|o| o.physical_bytes)
}

/// [`read`] with the debug override applied.
pub fn snapshot() -> Option<MemorySnapshot> {
    let real = read()?;
    Some(match debug_override() {
        Some(o) => o.apply(real),
        None => real,
    })
}

/// Reads the machine's memory. `None` if the kernel call fails or off macOS.
#[cfg(target_os = "macos")]
fn read() -> Option<MemorySnapshot> {
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
    let mut level: i32 = 0;
    let mut level_len = size_of::<i32>();
    // SAFETY: the sysctl is an int; buffer and length match. A failure
    // leaves `level` 0 (normal).
    unsafe {
        libc::sysctlbyname(
            c"kern.memorystatus_vm_pressure_level".as_ptr(),
            (&mut level as *mut i32).cast(),
            &mut level_len,
            std::ptr::null_mut(),
            0,
        )
    };
    Some(MemorySnapshot {
        physical_bytes: physical,
        available_bytes: pages.saturating_mul(page).min(physical),
        pressure: Pressure::from_level(level),
    })
}

#[cfg(not(target_os = "macos"))]
fn read() -> Option<MemorySnapshot> {
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

    fn snap(physical_gb: u64, available_mb: u64, pressure: Pressure) -> Option<MemorySnapshot> {
        Some(MemorySnapshot {
            physical_bytes: physical_gb * GB,
            available_bytes: available_mb * 1024 * 1024,
            pressure,
        })
    }

    /// R5 §5.2 on an 8 GB Mac with the shipped model (RS-06 resident size):
    /// the LLM starts only with model + 1.5 GB free, a running one keeps
    /// working under warn pressure, and nothing uses it under critical.
    #[test]
    fn eight_gb_memory_rule() {
        let model = super::super::catalog::select(8 * GB, super::super::catalog::Cpu::AppleSilicon)
            .expect("8 GB tier has a model");
        let resident = model.resident_mib * 1024 * 1024;
        let need_mb = model.resident_mib + 1536;
        assert!(allows(false, snap(8, need_mb, Pressure::Normal), resident));
        assert!(!allows(
            false,
            snap(8, need_mb - 1, Pressure::Normal),
            resident
        ));
        assert!(!allows(
            false,
            snap(8, need_mb - 1, Pressure::Warn),
            resident
        ));
        // Running: its memory is already in use.
        assert!(allows(true, snap(8, 300, Pressure::Normal), resident));
        assert!(allows(true, snap(8, 300, Pressure::Warn), resident));
        // Critical: never, and an idle server is released at once.
        assert!(!allows(true, snap(8, 6000, Pressure::Critical), resident));
        assert!(!allows(false, snap(8, 6000, Pressure::Critical), resident));
        assert!(release_idle(snap(8, 300, Pressure::Critical)));
        assert!(!release_idle(snap(8, 300, Pressure::Warn)));
        assert!(!release_idle(None));
        // No reading: the take goes ahead (the deadline still applies).
        assert!(allows(false, None, resident));
    }

    #[test]
    fn pressure_levels_map() {
        assert_eq!(Pressure::from_level(1), Pressure::Normal);
        assert_eq!(Pressure::from_level(2), Pressure::Warn);
        assert_eq!(Pressure::from_level(4), Pressure::Critical);
        assert_eq!(Pressure::from_level(0), Pressure::Normal);
        assert_eq!(Pressure::from_level(7), Pressure::Normal);
    }

    #[test]
    fn the_debug_override_simulates_another_mac() {
        let o = parse_override("physical_gb=8, available_mb=2500 ,pressure=critical");
        assert_eq!(o.physical_bytes, Some(8 * GB));
        assert_eq!(o.available_bytes, Some(2500 * 1024 * 1024));
        assert_eq!(o.pressure, Some(Pressure::Critical));
        let real = MemorySnapshot {
            physical_bytes: 128 * GB,
            available_bytes: 100 * GB,
            pressure: Pressure::Normal,
        };
        assert_eq!(
            o.apply(real),
            MemorySnapshot {
                physical_bytes: 8 * GB,
                available_bytes: 2500 * 1024 * 1024,
                pressure: Pressure::Critical,
            }
        );
        // Only physical: available is capped by it, pressure stays real.
        let only = parse_override("physical_gb=8");
        assert_eq!(only.apply(real).available_bytes, 8 * GB);
        assert_eq!(only.apply(real).pressure, Pressure::Normal);
        // Junk is ignored.
        assert_eq!(
            parse_override("x,pressure=high,available_mb=lots"),
            Override::default()
        );
        assert_eq!(Override::default().apply(real), real);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn snapshot_reads_plausible_numbers() {
        let s = read().expect("snapshot");
        assert!(s.physical_bytes >= 4 * GB);
        assert!(s.available_bytes > 0 && s.available_bytes <= s.physical_bytes);
    }
}
