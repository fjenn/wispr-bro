// SPDX-License-Identifier: GPL-3.0-or-later
//! The onboarding steps and their persisted transitions (S-29a, RE-16). The
//! state itself lives in fork settings (`OnboardingState`); this is only the
//! logic over it. Navigation never looks at the speech-model download: no
//! step waits for it.
//!
//! Order (stage in the stepper): `welcome`, `move_to_applications` (only
//! when the app runs outside an Applications folder) and `languages`
//! (Welcome); `permissions` (Permissions); `mic_test` and `shortcut_test`
//! (Setup). S-29b appends its Practice steps to [`ORDER`].

use crate::fork::settings::{OnboardingState, OnboardingStatus};

pub const WELCOME: &str = "welcome";
pub const MOVE_TO_APPLICATIONS: &str = "move_to_applications";
pub const LANGUAGES: &str = "languages";
pub const PERMISSIONS: &str = "permissions";
pub const MIC_TEST: &str = "mic_test";
pub const SHORTCUT_TEST: &str = "shortcut_test";

pub const ORDER: &[&str] = &[
    WELCOME,
    MOVE_TO_APPLICATIONS,
    LANGUAGES,
    PERMISSIONS,
    MIC_TEST,
    SHORTCUT_TEST,
];

/// Steps that only some Macs see; they never count as completed.
const OPTIONAL: &[&str] = &[MOVE_TO_APPLICATIONS];

pub fn valid_step(step: &str) -> bool {
    ORDER.contains(&step)
}

/// Where a launch resumes: the saved step while onboarding is in progress,
/// else (not started, completed, or a damaged value) Welcome.
pub fn resume_step(state: &OnboardingState) -> &'static str {
    if state.status == OnboardingStatus::InProgress {
        if let Some(step) = state.current_step.as_deref() {
            if let Some(found) = ORDER.iter().copied().find(|s| *s == step) {
                return found;
            }
        }
    }
    WELCOME
}

fn completed_before(step: &str) -> Vec<String> {
    ORDER
        .iter()
        .take_while(|candidate| **candidate != step)
        .filter(|candidate| !OPTIONAL.contains(candidate))
        .map(|s| (*s).to_owned())
        .collect()
}

/// Moves to `step` (forward or Back). Starting over from a finished run
/// clears its checklist (S-30) and completion time.
pub fn go_to(state: &mut OnboardingState, step: &str) -> Result<(), String> {
    if !valid_step(step) {
        return Err(format!("unknown onboarding step {step:?}"));
    }
    if state.status == OnboardingStatus::Completed {
        state.checklist_done.clear();
    }
    state.status = OnboardingStatus::InProgress;
    state.current_step = Some(step.to_owned());
    state.completed_steps = completed_before(step);
    state.completed_at_ms = None;
    Ok(())
}

pub fn complete(state: &mut OnboardingState, now_ms: i64) {
    state.status = OnboardingStatus::Completed;
    state.current_step = None;
    state.completed_steps = ORDER
        .iter()
        .filter(|s| !OPTIONAL.contains(s))
        .map(|s| (*s).to_owned())
        .collect();
    state.completed_at_ms = Some(now_ms.max(0));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_step_resumes_after_a_relaunch_and_back_drops_later_completion() {
        let mut state = OnboardingState::default();
        assert_eq!(resume_step(&state), WELCOME);
        for step in ORDER {
            go_to(&mut state, step).unwrap();
            // A relaunch reads the same persisted state.
            let reloaded: OnboardingState =
                serde_json::from_value(serde_json::to_value(&state).unwrap()).unwrap();
            assert_eq!(resume_step(&reloaded), *step);
            assert_eq!(reloaded.status, OnboardingStatus::InProgress);
        }
        assert_eq!(
            state.completed_steps,
            [WELCOME, LANGUAGES, PERMISSIONS, MIC_TEST]
        );
        go_to(&mut state, PERMISSIONS).unwrap();
        assert_eq!(state.completed_steps, [WELCOME, LANGUAGES]);
        go_to(&mut state, WELCOME).unwrap();
        assert!(state.completed_steps.is_empty());
    }

    #[test]
    fn a_completed_run_starts_over_but_progress_keeps_the_checklist() {
        let mut state = OnboardingState {
            checklist_done: vec!["first_take".into()],
            ..OnboardingState::default()
        };
        go_to(&mut state, MIC_TEST).unwrap();
        assert_eq!(state.checklist_done, ["first_take"]);
        complete(&mut state, 123);
        assert_eq!(state.status, OnboardingStatus::Completed);
        assert_eq!(state.completed_at_ms, Some(123));
        assert_eq!(state.current_step, None);
        assert_eq!(resume_step(&state), WELCOME);
        go_to(&mut state, WELCOME).unwrap();
        assert!(state.checklist_done.is_empty());
        assert_eq!(state.completed_at_ms, None);
    }

    #[test]
    fn damaged_resume_state_and_unknown_steps_fail_safely() {
        let mut state = OnboardingState {
            status: OnboardingStatus::InProgress,
            current_step: Some("unknown".into()),
            ..OnboardingState::default()
        };
        assert_eq!(resume_step(&state), WELCOME);
        let before = state.clone();
        assert!(go_to(&mut state, "unknown").is_err());
        assert_eq!(state, before);
    }

    #[test]
    fn the_ui_and_the_backend_agree_on_the_steps() {
        // `src/fork/onboarding/steps.ts` lists the same ids in the same order.
        let ts = include_str!("../../../../src/fork/onboarding/steps.ts");
        let start = ts.find("ONBOARDING_STEPS = [").expect("steps list");
        let list = &ts[start..ts[start..].find(']').unwrap() + start];
        let ids: Vec<&str> = list.split('"').skip(1).step_by(2).collect();
        assert_eq!(ids, ORDER);
    }
}
