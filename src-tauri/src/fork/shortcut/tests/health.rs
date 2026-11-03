// SPDX-License-Identifier: GPL-3.0-or-later
//! S-02: the health state machine (Healthy → Lost → Recovering → Healthy)
//! for each tap, driven by fake probe results and notifications.

use crate::fork::shortcut::health::*;

fn changed(effects: &[Effect]) -> Option<HealthStatus> {
    effects.iter().rev().find_map(|e| match e {
        Effect::Changed(s) => Some(s.clone()),
        Effect::Rebuild { .. } => None,
    })
}

fn rebuild_list(effects: &[Effect]) -> Vec<(TapId, bool)> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Rebuild { tap, refresh } => Some((*tap, *refresh)),
            Effect::Changed(_) => None,
        })
        .collect()
}

#[test]
fn a_healthy_check_changes_nothing() {
    let mut m = Machine::new();
    assert!(m.checked(0, Trigger::Startup, true).is_empty());
    assert_eq!(m.status(), HealthStatus::healthy());
}

#[test]
fn revoke_then_regrant_recovers_both_taps() {
    let mut m = Machine::new();
    // The notification arrives, the probe fails: both taps lost.
    let e = m.checked(100, Trigger::PrivacyChanged, false);
    let s = changed(&e).expect("announced");
    assert_eq!(s.state, TapHealth::Lost);
    assert_eq!(s.reason, Some(LostReason::Permission));
    assert_eq!(s.fork_tap, TapHealth::Lost);
    assert_eq!(s.handy_tap, TapHealth::Lost);
    assert!(!s.needs_relaunch);
    // Re-checks are scheduled after a notification (TCC lags).
    assert_eq!(
        m.next_recheck(),
        Some(100 + RECHECK_AFTER_NOTIFICATION_MS[0])
    );

    // Re-granted: the next probe succeeds, both taps are rebuilt.
    let e = m.checked(2_000, Trigger::AccessibilityChanged, true);
    assert_eq!(
        rebuild_list(&e),
        vec![(TapId::Fork, false), (TapId::Handy, false)]
    );
    let s = changed(&e).unwrap();
    assert_eq!(s.state, TapHealth::Recovering);
    assert_eq!(s.fork_tap, TapHealth::Recovering);

    let e = m.rebuilt(2_050, TapId::Fork, true);
    assert_eq!(changed(&e).unwrap().fork_tap, TapHealth::Healthy);
    assert_eq!(m.status().state, TapHealth::Recovering);
    let e = m.rebuilt(2_060, TapId::Handy, true);
    assert_eq!(changed(&e), Some(HealthStatus::healthy()));
    assert_eq!(m.next_recheck(), None, "healthy: no more re-checks");
}

#[test]
fn a_repeated_failed_probe_is_announced_once() {
    let mut m = Machine::new();
    assert!(changed(&m.checked(0, Trigger::PrivacyChanged, false)).is_some());
    assert!(changed(&m.checked(10, Trigger::AccessibilityChanged, false)).is_none());
    assert!(changed(&m.checked(5_000, Trigger::Heartbeat, false)).is_none());
}

#[test]
fn a_rebuild_already_running_is_not_asked_for_twice() {
    let mut m = Machine::new();
    m.checked(0, Trigger::PrivacyChanged, false);
    let e = m.checked(500, Trigger::Recheck, true);
    assert_eq!(rebuild_list(&e).len(), 2);
    let e = m.checked(600, Trigger::AppActivated, true);
    assert!(rebuild_list(&e).is_empty());
}

#[test]
fn a_stopped_tap_is_lost_then_rebuilt_when_the_probe_passes() {
    let mut m = Machine::new();
    let e = m.stopped(TapId::Fork);
    let s = changed(&e).unwrap();
    assert_eq!(s.state, TapHealth::Lost);
    assert_eq!(s.reason, Some(LostReason::TapStopped));
    assert_eq!(s.handy_tap, TapHealth::Healthy);
    let e = m.checked(10, Trigger::TapStopped, true);
    assert_eq!(rebuild_list(&e), vec![(TapId::Fork, false)]);
    m.rebuilt(20, TapId::Fork, true);
    assert_eq!(m.status(), HealthStatus::healthy());
}

#[test]
fn rebuilds_that_keep_failing_ask_for_a_relaunch() {
    let mut m = Machine::new();
    m.stopped(TapId::Fork);
    let mut now = 0;
    for attempt in 1..=MAX_REBUILDS {
        let e = m.checked(now, Trigger::Recheck, true);
        assert_eq!(
            rebuild_list(&e),
            vec![(TapId::Fork, false)],
            "attempt {attempt}"
        );
        m.rebuilt(now + 5, TapId::Fork, false);
        now += 5_000;
    }
    let s = m.status();
    assert_eq!(s.state, TapHealth::Lost);
    assert!(s.needs_relaunch, "{s:?}");
    // No automatic attempt any more; "Check again" tries once more.
    assert!(rebuild_list(&m.checked(now, Trigger::Heartbeat, true)).is_empty());
    let e = m.checked(now + 10, Trigger::User, true);
    assert_eq!(rebuild_list(&e), vec![(TapId::Fork, false)]);
    m.rebuilt(now + 20, TapId::Fork, true);
    assert_eq!(m.status(), HealthStatus::healthy());
}

#[test]
fn a_failed_rebuild_schedules_a_retry() {
    let mut m = Machine::new();
    m.stopped(TapId::Handy);
    m.checked(0, Trigger::TapStopped, true);
    m.rebuilt(100, TapId::Handy, false);
    assert_eq!(m.next_recheck(), Some(100 + REBUILD_RETRY_MS));
    assert!(m.take_due(100 + REBUILD_RETRY_MS));
    assert_eq!(m.next_recheck(), None);
}

#[test]
fn wake_refreshes_healthy_taps_without_announcing() {
    let mut m = Machine::new();
    for trigger in [Trigger::Wake, Trigger::ScreensWake, Trigger::SessionActive] {
        let e = m.checked(0, trigger, true);
        assert_eq!(
            rebuild_list(&e),
            vec![(TapId::Fork, true), (TapId::Handy, true)],
            "{trigger:?}"
        );
        assert!(changed(&e).is_none(), "a refresh is silent");
        m.rebuilt(5, TapId::Fork, true);
        m.rebuilt(5, TapId::Handy, true);
        assert_eq!(m.status(), HealthStatus::healthy());
    }
    // Other triggers do not rebuild a healthy tap.
    for trigger in [
        Trigger::AppActivated,
        Trigger::TakeShown,
        Trigger::MissedRelease,
    ] {
        assert!(rebuild_list(&m.checked(0, trigger, true)).is_empty());
    }
}

#[test]
fn a_rebuild_that_finishes_after_the_grant_went_again_stays_lost() {
    let mut m = Machine::new();
    m.checked(0, Trigger::PrivacyChanged, false);
    m.checked(100, Trigger::PrivacyChanged, true);
    // Revoked again before the rebuild reported.
    m.checked(150, Trigger::PrivacyChanged, false);
    m.rebuilt(200, TapId::Fork, true);
    assert_eq!(m.status().fork_tap, TapHealth::Lost);
    assert_eq!(m.status().reason, Some(LostReason::Permission));
}

#[test]
fn the_heartbeat_only_reads_trust_while_healthy() {
    let mut m = Machine::new();
    assert_eq!(m.plan(Trigger::Heartbeat), Plan::TrustedOnly);
    assert_eq!(m.plan(Trigger::PrivacyChanged), Plan::Probe);
    m.checked(0, Trigger::PrivacyChanged, false);
    assert_eq!(m.plan(Trigger::Heartbeat), Plan::Probe);
}

#[test]
fn a_tap_that_could_not_start_is_lost_until_the_grant_arrives() {
    let mut m = Machine::new();
    let e = m.started(TapId::Fork, false);
    let s = changed(&e).unwrap();
    assert_eq!(s.fork_tap, TapHealth::Lost);
    assert_eq!(s.reason, Some(LostReason::Permission));
    // Startup probe still fails (onboarding not done yet).
    m.checked(0, Trigger::Startup, false);
    // Granted during onboarding: the notification builds the first tap.
    let e = m.checked(9_000, Trigger::AccessibilityChanged, true);
    assert!(rebuilds(&e).contains(&TapId::Fork));
    m.rebuilt(9_010, TapId::Fork, true);
    m.rebuilt(9_010, TapId::Handy, true);
    assert_eq!(m.status(), HealthStatus::healthy());
}

#[test]
fn the_probe_is_a_create_and_destroy_never_a_poll_of_an_existing_tap() {
    // Exit criterion (R5 S-02): no polling of the tap-enabled query in the
    // fork's code.
    let needle = ["CGEventTap", "IsEnabled"].concat();
    let rust_needle = ["tap_is", "_enabled"].concat();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/fork");
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                assert!(
                    !text.contains(&needle) && !text.contains(&rust_needle),
                    "{} polls the tap state",
                    path.display()
                );
            }
        }
    }
}

#[cfg(target_os = "macos")]
#[test]
fn the_probe_tap_answers_without_side_effects() {
    // Whatever this test process may do, the probe returns and leaves
    // nothing behind; twice in a row gives the same answer.
    let a = probe_tap().unwrap();
    let b = probe_tap().unwrap();
    assert_eq!(a, b);
}
