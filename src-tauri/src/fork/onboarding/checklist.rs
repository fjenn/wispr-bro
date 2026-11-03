// SPDX-License-Identifier: GPL-3.0-or-later
//! The setup checklist's persisted ticks (S-30, B-44). The items
//! themselves live in the Hub (`src/fork/hub/checklistRegistry.tsx`, which
//! later slices extend); this only records an item id in
//! `onboarding.checklist_done`, once, so a tick survives the thing that
//! caused it (a deleted word stays ticked) and a parked Hub (S-38).
//!
//! One item is ticked here, in the backend: [`TRY_IN_APP`], by the first
//! take pasted into another app ([`on_take_pasted`]), which can happen while
//! no Hub web view exists.

use tauri::{AppHandle, Manager, Runtime};
use tauri_specta::Event;

use crate::fork::events::ForkSettingsChanged;
use crate::fork::settings::{ForkSettingUpdate, ForkSettingsHandle, OnboardingState};

/// "Try it in another app": the first dictation pasted outside Wispr Bro.
pub const TRY_IN_APP: &str = "try_in_app";

/// Adds `id` to the ticks. `Ok(false)` when it was already there. Ids follow
/// the fork settings id rules (lowercase, `a-z0-9_.-`) and list limit.
pub fn mark(state: &mut OnboardingState, id: &str) -> Result<bool, String> {
    if state.checklist_done.iter().any(|done| done == id) {
        return Ok(false);
    }
    let mut next = state.clone();
    next.checklist_done.push(id.to_owned());
    crate::fork::settings::validate(&ForkSettingUpdate::Onboarding(next.clone()))?;
    *state = next;
    Ok(true)
}

/// Persists a tick and tells every window (`ForkSettingsChanged`, key
/// `onboarding`). Returns the onboarding state after the call.
pub fn mark_done<R: Runtime>(app: &AppHandle<R>, id: &str) -> Result<OnboardingState, String> {
    let handle = app
        .try_state::<ForkSettingsHandle>()
        .ok_or_else(|| "fork settings are not managed".to_string())?;
    let (next, added) = handle.update(app, |settings| mark(&mut settings.onboarding, id))?;
    if added {
        // An item id, never user text.
        log::info!("checklist: {id} done");
        let event = ForkSettingsChanged {
            key: "onboarding".into(),
            settings: next.clone(),
        };
        if let Err(e) = event.emit(app) {
            log::warn!("checklist: could not emit the change: {e}");
        }
    }
    Ok(next.onboarding)
}

/// Whether a bundle id is Wispr Bro itself (any identity: product, test
/// bundles `com.wisprbro.<slice>check`).
pub fn is_own_app(bundle_id: Option<&str>) -> bool {
    bundle_id.is_some_and(|id| id.starts_with("com.wisprbro."))
}

/// A take was pasted (S-04 delivery, `confirmed` or `unverified`). When the
/// app in front is not Wispr Bro, this is a real use: "Try it in another
/// app" ticks and the spotlight (if any) ends. Practice takes never get here
/// (they are delivered to the onboarding screen, S-29b).
pub fn on_take_pasted<R: Runtime>(app: &AppHandle<R>) {
    let front = crate::fork::context::frontmost::frontmost();
    let own = is_own_app(front.as_ref().and_then(|f| f.bundle_id.as_deref()));
    crate::fork::pill::spotlight::on_take_pasted(own);
    if own {
        return;
    }
    let done = app
        .try_state::<ForkSettingsHandle>()
        .and_then(|h| h.get(app).ok())
        .is_some_and(|s| {
            s.onboarding.checklist_done.iter().any(|d| d == TRY_IN_APP)
                || s.onboarding.status != crate::fork::settings::OnboardingStatus::Completed
        });
    if done {
        return;
    }
    if let Err(e) = mark_done(app, TRY_IN_APP) {
        log::warn!("checklist: could not tick {TRY_IN_APP}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_adds_once_and_keeps_order() {
        let mut state = OnboardingState::default();
        assert_eq!(mark(&mut state, "add_word"), Ok(true));
        assert_eq!(mark(&mut state, TRY_IN_APP), Ok(true));
        assert_eq!(mark(&mut state, "add_word"), Ok(false));
        assert_eq!(state.checklist_done, ["add_word", TRY_IN_APP]);
    }

    #[test]
    fn mark_refuses_ids_outside_the_settings_rules() {
        let mut state = OnboardingState::default();
        assert!(mark(&mut state, "Add Word").is_err());
        assert!(mark(&mut state, "").is_err());
        assert!(mark(&mut state, &"x".repeat(200)).is_err());
        assert!(state.checklist_done.is_empty());
    }

    #[test]
    fn own_app_is_any_wispr_bro_identity() {
        assert!(is_own_app(Some("com.wisprbro.desktop")));
        assert!(is_own_app(Some("com.wisprbro.s30check")));
        assert!(!is_own_app(Some("com.apple.mail")));
        assert!(!is_own_app(None));
    }
}
