// SPDX-License-Identifier: GPL-3.0-or-later
//! Hotkey self-healing (S-02, R5 §4.8; closes A-208, keeps A-207 and B-17).
//!
//! Two event taps carry the app's hotkeys: the fork's own
//! (`listener.rs`, the fn grammar) and the one inside Handy's handy-keys
//! `HotkeyManager` (`shortcut/handy_keys.rs`). Either can die without a
//! word: the Accessibility row is switched off or removed, the app is
//! re-signed (the grant no longer matches), or macOS invalidates the tap.
//! handy-keys re-enables a tap that macOS merely *disabled* (timeout, user
//! input) from its callback; nothing in the system reports the rest.
//!
//! This module watches both taps **without polling whether a tap is
//! enabled** (the `CGEventTap…IsEnabled` query;
//! handy-keys #1827: that RPC leaked kernel IPC vouchers until macOS
//! panicked). A check runs on a [`Trigger`]:
//!
//! - the two undocumented Accessibility notifications, through vendored
//!   axwatch 0.1.0 (`com.apple.accessibility.api`, distributed;
//!   `com.apple.tcc.access.changed`, Darwin), with a 10 s heartbeat that
//!   only reads `AXIsProcessTrusted` while everything is healthy;
//! - VoiceInk `ShortcutMonitor.swift`'s recovery triggers (GPL-3.0; the
//!   list of triggers, re-implemented): system wake, display wake, the
//!   session becoming active again, and a missed release (the S-01 stuck
//!   guard released the trigger);
//! - the app being activated and every take the bar shows (RS-02's
//!   fallback in case the notifications are withheld);
//! - a tap whose event stream ended.
//!
//! A check is a **probe tap**: creating a filtering `CGEventTap` (never
//! added to a run loop) and destroying it at once. Creation fails without
//! the permission even when `AXIsProcessTrusted` still says yes (it keeps
//! answering yes after the app's row is removed; axwatch docs, RT-19).
//!
//! [`Machine`] is the pure state machine (per tap: Healthy → Lost →
//! Recovering → Healthy); the actor at the bottom runs probes, rebuilds
//! taps and tells the pill, the tray, the Hub and the notification inbox.

use std::collections::{BTreeMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use specta::Type;

/// Milliseconds on the actor's monotonic clock.
pub type Ms = u64;

/// After an Accessibility/privacy notification while a tap is lost, check
/// again at these offsets: TCC can answer the old grant for a moment after
/// the switch moves (axwatch docs), and no second notification follows.
pub const RECHECK_AFTER_NOTIFICATION_MS: [Ms; 4] = [300, 1_000, 2_500, 4_500];
/// Rebuild attempts per tap before the UI offers "Relaunch to finish"
/// (RS-02 fallback).
pub const MAX_REBUILDS: u8 = 3;
/// Retry delay after a failed rebuild (× the attempt number).
pub const REBUILD_RETRY_MS: Ms = 1_000;
/// The heartbeat period (axwatch). While healthy it only reads
/// `AXIsProcessTrusted`; while a tap is lost it probes.
pub const HEARTBEAT_MS: Ms = 10_000;

/// One of the two taps.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Type,
)]
#[serde(rename_all = "snake_case")]
pub enum TapId {
    /// The fork's own tap (fn grammar, keyed fork bindings).
    Fork,
    /// Handy's handy-keys manager tap.
    Handy,
}

impl TapId {
    pub const ALL: [TapId; 2] = [TapId::Fork, TapId::Handy];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum TapHealth {
    Healthy,
    Lost,
    /// The permission is back; the tap is being rebuilt.
    Recovering,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum LostReason {
    /// The probe tap could not be created: Accessibility is off, the row
    /// was removed, or the app was re-signed.
    Permission,
    /// The permission is fine, but the tap stopped or could not be rebuilt.
    TapStopped,
}

/// Why a check runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    Startup,
    /// `com.apple.accessibility.api` (axwatch).
    AccessibilityChanged,
    /// `com.apple.tcc.access.changed` (axwatch).
    PrivacyChanged,
    Heartbeat,
    /// `NSWorkspaceDidWakeNotification`.
    Wake,
    /// `NSWorkspaceScreensDidWakeNotification`.
    ScreensWake,
    /// `NSWorkspaceSessionDidBecomeActiveNotification`.
    SessionActive,
    /// The main window became key.
    AppActivated,
    /// The bar showed a take.
    TakeShown,
    /// The stuck guard released a trigger the tap never saw go up.
    MissedRelease,
    /// A tap's event stream ended.
    TapStopped,
    /// A scheduled re-check.
    Recheck,
    /// The user pressed "Check again".
    User,
}

impl Trigger {
    fn is_notification(self) -> bool {
        matches!(
            self,
            Trigger::AccessibilityChanged | Trigger::PrivacyChanged
        )
    }

    /// System wake-ups after which a healthy tap is rebuilt anyway
    /// (VoiceInk's recovery triggers).
    pub fn refreshes_taps(self) -> bool {
        matches!(
            self,
            Trigger::Wake | Trigger::ScreensWake | Trigger::SessionActive
        )
    }
}

/// How a trigger is checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    /// Read `AXIsProcessTrusted` only; probe if it says no.
    TrustedOnly,
    Probe,
}

/// What the actor does next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Build a new tap. `refresh`: the old one was healthy (a wake); the
    /// engine may skip it during a take.
    Rebuild { tap: TapId, refresh: bool },
    /// The overall status changed.
    Changed(HealthStatus),
}

/// The whole picture (`fork_shortcut_health`, event
/// `ForkShortcutHealthChanged`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct HealthStatus {
    /// The worst of the two taps.
    pub state: TapHealth,
    pub reason: Option<LostReason>,
    pub fork_tap: TapHealth,
    pub handy_tap: TapHealth,
    /// A tap could not be rebuilt [`MAX_REBUILDS`] times although the
    /// permission is granted: offer "Relaunch to finish".
    pub needs_relaunch: bool,
}

impl HealthStatus {
    pub fn healthy() -> HealthStatus {
        HealthStatus {
            state: TapHealth::Healthy,
            reason: None,
            fork_tap: TapHealth::Healthy,
            handy_tap: TapHealth::Healthy,
            needs_relaunch: false,
        }
    }

    pub fn lost(&self) -> bool {
        self.state == TapHealth::Lost
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Tap {
    health: TapHealth,
    in_flight: bool,
    attempts: u8,
}

impl Tap {
    const HEALTHY: Tap = Tap {
        health: TapHealth::Healthy,
        in_flight: false,
        attempts: 0,
    };
}

/// The health state machine. Pure: time is passed in, probes and rebuilds
/// are done by the caller and fed back.
#[derive(Clone, Debug)]
pub struct Machine {
    taps: BTreeMap<TapId, Tap>,
    reason: Option<LostReason>,
    rechecks: VecDeque<Ms>,
    announced: HealthStatus,
}

impl Default for Machine {
    fn default() -> Self {
        Machine::new()
    }
}

impl Machine {
    /// Both taps assumed healthy until told otherwise.
    pub fn new() -> Machine {
        Machine {
            taps: TapId::ALL.iter().map(|t| (*t, Tap::HEALTHY)).collect(),
            reason: None,
            rechecks: VecDeque::new(),
            announced: HealthStatus::healthy(),
        }
    }

    fn tap(&self, id: TapId) -> Tap {
        self.taps.get(&id).copied().unwrap_or(Tap::HEALTHY)
    }

    fn tap_mut(&mut self, id: TapId) -> &mut Tap {
        self.taps.entry(id).or_insert(Tap::HEALTHY)
    }

    /// The current status.
    pub fn status(&self) -> HealthStatus {
        let fork = self.tap(TapId::Fork).health;
        let handy = self.tap(TapId::Handy).health;
        let state = if fork == TapHealth::Lost || handy == TapHealth::Lost {
            TapHealth::Lost
        } else if fork == TapHealth::Recovering || handy == TapHealth::Recovering {
            TapHealth::Recovering
        } else {
            TapHealth::Healthy
        };
        HealthStatus {
            state,
            reason: if state == TapHealth::Healthy {
                None
            } else {
                self.reason
            },
            fork_tap: fork,
            handy_tap: handy,
            needs_relaunch: state != TapHealth::Healthy
                && self.reason == Some(LostReason::TapStopped)
                && self.taps.values().any(|t| t.attempts >= MAX_REBUILDS),
        }
    }

    fn all_healthy(&self) -> bool {
        self.taps.values().all(|t| t.health == TapHealth::Healthy)
    }

    /// How to check `trigger`.
    pub fn plan(&self, trigger: Trigger) -> Plan {
        if trigger == Trigger::Heartbeat && self.all_healthy() {
            Plan::TrustedOnly
        } else {
            Plan::Probe
        }
    }

    /// When the next scheduled re-check is due.
    pub fn next_recheck(&self) -> Option<Ms> {
        self.rechecks.iter().copied().min()
    }

    /// Drops the re-checks due by `now`; true if one was due.
    pub fn take_due(&mut self, now: Ms) -> bool {
        let before = self.rechecks.len();
        self.rechecks.retain(|t| *t > now);
        self.rechecks.len() != before
    }

    fn schedule(&mut self, at: Ms) {
        if !self.rechecks.contains(&at) {
            self.rechecks.push_back(at);
        }
    }

    fn changed(&mut self, out: &mut Vec<Effect>) {
        let now = self.status();
        if now != self.announced {
            self.announced = now.clone();
            out.push(Effect::Changed(now));
        }
    }

    /// A tap was started (or failed to start) outside a rebuild.
    pub fn started(&mut self, tap: TapId, ok: bool) -> Vec<Effect> {
        let mut out = Vec::new();
        let t = self.tap_mut(tap);
        if ok {
            *t = Tap::HEALTHY;
        } else {
            t.health = TapHealth::Lost;
            self.reason.get_or_insert(LostReason::Permission);
        }
        if self.all_healthy() {
            self.reason = None;
        }
        self.changed(&mut out);
        out
    }

    /// The result of a check. `probe_ok`: the probe tap could be created
    /// (for [`Plan::TrustedOnly`], `AXIsProcessTrusted` said yes).
    pub fn checked(&mut self, now: Ms, trigger: Trigger, probe_ok: bool) -> Vec<Effect> {
        let mut out = Vec::new();
        if !probe_ok {
            for t in self.taps.values_mut() {
                t.health = TapHealth::Lost;
                t.in_flight = false;
                t.attempts = 0;
            }
            self.reason = Some(LostReason::Permission);
            if trigger.is_notification() {
                for d in RECHECK_AFTER_NOTIFICATION_MS {
                    self.schedule(now + d);
                }
            }
            self.changed(&mut out);
            return out;
        }
        if self.reason == Some(LostReason::Permission) {
            // The grant is back: rebuild every tap from scratch.
            self.reason = Some(LostReason::TapStopped);
            for t in self.taps.values_mut() {
                t.attempts = 0;
            }
        }
        for id in TapId::ALL {
            let t = self.tap_mut(id);
            if t.in_flight {
                continue;
            }
            match t.health {
                TapHealth::Lost => {
                    if t.attempts >= MAX_REBUILDS && trigger != Trigger::User {
                        continue;
                    }
                    if trigger == Trigger::User {
                        t.attempts = 0;
                    }
                    t.health = TapHealth::Recovering;
                    t.in_flight = true;
                    out.push(Effect::Rebuild {
                        tap: id,
                        refresh: false,
                    });
                }
                TapHealth::Recovering => {
                    t.in_flight = true;
                    out.push(Effect::Rebuild {
                        tap: id,
                        refresh: false,
                    });
                }
                TapHealth::Healthy if trigger.refreshes_taps() => {
                    t.in_flight = true;
                    out.push(Effect::Rebuild {
                        tap: id,
                        refresh: true,
                    });
                }
                TapHealth::Healthy => {}
            }
        }
        if self.all_healthy() {
            self.reason = None;
            self.rechecks.clear();
        }
        self.changed(&mut out);
        out
    }

    /// A tap's event stream ended. The caller then probes
    /// ([`Trigger::TapStopped`]).
    pub fn stopped(&mut self, tap: TapId) -> Vec<Effect> {
        let mut out = Vec::new();
        let t = self.tap_mut(tap);
        t.in_flight = false;
        if t.health == TapHealth::Healthy {
            t.health = TapHealth::Lost;
        }
        if self.reason.is_none() {
            self.reason = Some(LostReason::TapStopped);
        }
        self.changed(&mut out);
        out
    }

    /// The result of a [`Effect::Rebuild`].
    pub fn rebuilt(&mut self, now: Ms, tap: TapId, ok: bool) -> Vec<Effect> {
        let mut out = Vec::new();
        let permission_lost = self.reason == Some(LostReason::Permission);
        let t = self.tap_mut(tap);
        t.in_flight = false;
        if ok {
            // A rebuild that finished after a later probe failed does not
            // make the tap healthy: the grant is gone again.
            if !permission_lost {
                *t = Tap::HEALTHY;
            }
        } else {
            t.health = TapHealth::Lost;
            t.attempts = t.attempts.saturating_add(1);
            let attempts = t.attempts;
            if !permission_lost {
                self.reason = Some(LostReason::TapStopped);
            }
            if attempts < MAX_REBUILDS {
                self.schedule(now + REBUILD_RETRY_MS * Ms::from(attempts));
            }
        }
        if self.all_healthy() {
            self.reason = None;
            self.rechecks.clear();
        }
        self.changed(&mut out);
        out
    }
}

/// The taps a set of effects rebuilds (tests, logs).
pub fn rebuilds(effects: &[Effect]) -> HashSet<TapId> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Rebuild { tap, .. } => Some(*tap),
            Effect::Changed(_) => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The probe

/// Whether a filtering session event tap can be created right now. The tap
/// is never added to a run loop and is destroyed before returning, so it
/// sees no events; this is a capability check, not a poll of an existing
/// tap. `None` where there are no event taps (not macOS).
pub fn probe_tap() -> Option<bool> {
    #[cfg(target_os = "macos")]
    {
        Some(mac_probe::create_and_destroy())
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// `AXIsProcessTrusted` (no prompt).
pub fn trusted() -> bool {
    crate::fork::vendor::axwatch::is_trusted()
}

#[cfg(target_os = "macos")]
mod mac_probe {
    use std::ffi::c_void;

    type CGEventTapProxy = *mut c_void;
    type CGEventRef = *mut c_void;
    type CFMachPortRef = *mut c_void;
    type Callback =
        unsafe extern "C" fn(CGEventTapProxy, u32, CGEventRef, *mut c_void) -> CGEventRef;

    const SESSION_EVENT_TAP: u32 = 1;
    const HEAD_INSERT_EVENT_TAP: u32 = 0;
    /// `kCGEventTapOptionDefault`: a filtering tap, which needs the
    /// Accessibility grant (a listen-only tap would only need Input
    /// Monitoring).
    const OPTION_DEFAULT: u32 = 0;
    const FLAGS_CHANGED: u64 = 12;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventTapCreate(
            tap: u32,
            place: u32,
            options: u32,
            events_of_interest: u64,
            callback: Callback,
            user_info: *mut c_void,
        ) -> CFMachPortRef;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFMachPortInvalidate(port: CFMachPortRef);
        fn CFRelease(cf: *const c_void);
    }

    unsafe extern "C" fn pass_through(
        _proxy: CGEventTapProxy,
        _kind: u32,
        event: CGEventRef,
        _user_info: *mut c_void,
    ) -> CGEventRef {
        event
    }

    pub fn create_and_destroy() -> bool {
        // SAFETY: valid constants and a callback with the documented
        // signature; the port is never scheduled, then invalidated and
        // released exactly once.
        unsafe {
            let port = CGEventTapCreate(
                SESSION_EVENT_TAP,
                HEAD_INSERT_EVENT_TAP,
                OPTION_DEFAULT,
                1 << FLAGS_CHANGED,
                pass_through,
                std::ptr::null_mut(),
            );
            if port.is_null() {
                return false;
            }
            CFMachPortInvalidate(port);
            CFRelease(port);
            true
        }
    }
}

// ---------------------------------------------------------------------------
// The actor

mod actor;
pub use actor::{
    on_engine_effect, open_fix_sheet, request_check, start, status, tap_rebuilt, tap_stopped,
    tray_warning, ForkShortcutFixRequested, ForkShortcutHealthChanged,
};
