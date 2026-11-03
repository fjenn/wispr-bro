// SPDX-License-Identifier: GPL-3.0-or-later
//! When an update may install and when the app may relaunch (S-28, E-215).
//!
//! The cases follow MacParakeet's `SparkleUpdateGuard`
//! (`Sources/MacParakeet/App/SparkleUpdateGuard.swift` and its tests,
//! commit b6ca965c), Copyright (C) 2026 Daniel Moon, GPL-3.0. Modified by
//! Wispr Bro contributors, 2026-11-02: ported as behaviour and tests, no
//! code copied (PORTS.md, NOTICE §3). A check
//! never runs in a development build, an install never starts while a
//! recording runs, and an update that is already on disk never relaunches
//! the app mid-take. Wispr's policy adds the quiet window: nothing installs
//! until 20 minutes after the last take.
//!
//! Everything here is pure; [`super::scheduler`] feeds it the live state.

use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

/// Time after the last take before a background install (R5 S-28).
pub const QUIET_AFTER_TAKE: Duration = Duration::from_secs(20 * 60);

/// What the app is doing right now, as the gate sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Activity {
    pub now_ms: i64,
    /// A take is recording, transcribing, cleaning up or being delivered, or
    /// a transform runs.
    pub busy: bool,
    /// When the last take started or was last seen running; the app's
    /// launch time before the first take.
    pub last_activity_ms: i64,
    /// One of our windows (Hub, Scratchpad) has keyboard focus: the user is
    /// in the app, so a background relaunch would interrupt them.
    pub window_focused: bool,
}

/// Why a background install waits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wait {
    /// A take or transform is in progress.
    Busy,
    /// The last take was less than the quiet window ago; ms left.
    RecentTake { remaining_ms: i64 },
    /// The user is working in one of our windows.
    InUse,
}

/// Whether a background install may start now.
pub fn may_install(activity: &Activity, quiet: Duration) -> Result<(), Wait> {
    if activity.busy {
        return Err(Wait::Busy);
    }
    let quiet_ms = i64::try_from(quiet.as_millis()).unwrap_or(i64::MAX);
    let since = activity.now_ms.saturating_sub(activity.last_activity_ms);
    if since < quiet_ms {
        return Err(Wait::RecentTake {
            remaining_ms: quiet_ms - since,
        });
    }
    if activity.window_focused {
        return Err(Wait::InUse);
    }
    Ok(())
}

/// The relaunch veto: an installed update relaunches only while no take
/// runs (MacParakeet's `updaterShouldRelaunchApplication`). The quiet window
/// does not apply here: the files on disk are already the new version.
pub fn may_relaunch(busy: bool) -> bool {
    !busy
}

/// Version strings of local builds that must never update themselves to the
/// published release (MacParakeet's `isDevBuildVersion`: missing, empty,
/// `0.0.0`, `dev`).
pub fn is_dev_version(version: Option<&str>) -> bool {
    let Some(raw) = version else { return true };
    let v = raw.trim().to_ascii_lowercase();
    v.is_empty() || v == "0.0.0" || v == "dev"
}

/// Whether the background scheduler runs in this build. Debug builds skip it
/// (a work-in-progress build must not replace itself with the release)
/// unless the loopback test feed is set (`scripts/wb-f02/update-e2e.sh`);
/// builds without our identity (plain Handy) never run it.
pub fn background_allowed(
    version: Option<&str>,
    debug_build: bool,
    test_feed: bool,
    product_build: bool,
) -> bool {
    product_build && !is_dev_version(version) && (!debug_build || test_feed)
}

/// Unix ms of the last take (or launch); see [`note_activity`].
static LAST_ACTIVITY_MS: AtomicI64 = AtomicI64::new(0);

/// Records that a take started, ended or is running now.
pub fn note_activity_at(now_ms: i64) {
    LAST_ACTIVITY_MS.fetch_max(now_ms, Ordering::AcqRel);
}

/// [`note_activity_at`] with the current time.
pub fn note_activity() {
    note_activity_at(chrono::Utc::now().timestamp_millis());
}

/// The last recorded activity (0 before [`note_activity`] ran).
pub fn last_activity_ms() -> i64 {
    LAST_ACTIVITY_MS.load(Ordering::Acquire)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: i64 = 60_000;

    fn at(now_ms: i64, last: i64) -> Activity {
        Activity {
            now_ms,
            busy: false,
            last_activity_ms: last,
            window_focused: false,
        }
    }

    // MacParakeet `isDevBuildVersion` cases ("pdx" is their own sentinel).
    #[test]
    fn dev_versions_are_missing_empty_zero_or_dev() {
        assert!(is_dev_version(None));
        assert!(is_dev_version(Some("")));
        assert!(is_dev_version(Some("   ")));
        assert!(is_dev_version(Some("0.0.0")));
        for v in ["dev", "DEV", "Dev"] {
            assert!(is_dev_version(Some(v)), "{v}");
        }
        for v in ["0.5.7", "1.0.0", "1.0.0-alpha.1", "10.20.30"] {
            assert!(!is_dev_version(Some(v)), "{v}");
        }
    }

    // "skips dev builds", checked before anything else (MacParakeet:
    // `testBlockReasonBlocksDevBuildBeforeMeetingState`).
    #[test]
    fn background_checks_skip_dev_and_debug_builds() {
        assert!(background_allowed(Some("1.0.0"), false, false, true));
        assert!(!background_allowed(Some("0.0.0"), false, false, true));
        assert!(!background_allowed(None, false, true, true));
        assert!(!background_allowed(Some("1.0.0"), true, false, true));
        assert!(background_allowed(Some("1.0.0"), true, true, true));
        assert!(!background_allowed(Some("1.0.0"), false, false, false));
    }

    // "blocks during recording" (MacParakeet:
    // `testBlockReasonBlocksReleaseBuildDuringMeeting`), whatever else holds.
    #[test]
    fn install_waits_while_a_take_runs() {
        let mut a = at(100 * MIN, 0);
        a.busy = true;
        assert_eq!(may_install(&a, QUIET_AFTER_TAKE), Err(Wait::Busy));
        a.window_focused = true;
        assert_eq!(may_install(&a, QUIET_AFTER_TAKE), Err(Wait::Busy));
    }

    #[test]
    fn install_waits_twenty_minutes_after_the_last_take() {
        assert_eq!(
            may_install(&at(19 * MIN, 0), QUIET_AFTER_TAKE),
            Err(Wait::RecentTake { remaining_ms: MIN })
        );
        assert_eq!(may_install(&at(20 * MIN, 0), QUIET_AFTER_TAKE), Ok(()));
        // A clock that went backwards (or a take noted in the future) waits.
        assert!(may_install(&at(0, 5 * MIN), QUIET_AFTER_TAKE).is_err());
    }

    // MacParakeet: `testBlockReasonAllowsReleaseBuildWhenIdle`.
    #[test]
    fn install_runs_when_idle_and_not_in_use() {
        assert_eq!(may_install(&at(60 * MIN, 0), QUIET_AFTER_TAKE), Ok(()));
        let mut a = at(60 * MIN, 0);
        a.window_focused = true;
        assert_eq!(may_install(&a, QUIET_AFTER_TAKE), Err(Wait::InUse));
    }

    // "vetoes relaunch mid-take" (MacParakeet:
    // `updaterShouldRelaunchApplication`).
    #[test]
    fn relaunch_is_vetoed_mid_take_only() {
        assert!(!may_relaunch(true));
        assert!(may_relaunch(false));
    }

    #[test]
    fn activity_only_moves_forward() {
        note_activity_at(5_000);
        note_activity_at(4_000);
        assert!(last_activity_ms() >= 5_000);
    }
}
