// SPDX-License-Identifier: GPL-3.0-or-later
//! A lesson's practice step and the real shortcuts (S-31).
//!
//! The Transforms lesson asks the user to select text in a drawn composer
//! and press their Polish shortcut. With Transforms off the shortcut is not
//! registered, so the key reaches the web view and the page sees it. With
//! Transforms on, the fork tap swallows it (RS-11) and the page would never
//! hear it; a real run would also have to read and paste into our own web
//! view (S-20 showed reading our own WebKit window is fragile).
//!
//! So while a practice step is open (`fork_lesson_practice`, two minutes
//! unless renewed, off when the step goes) and Wispr Bro is the frontmost
//! app, a transform shortcut is handed to the page as [`ForkLessonShortcut`]
//! instead of starting a run. The page does the practice rewrite itself
//! (through the transform preview, never a paste). Pressed in any other app,
//! the shortcut runs as usual.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Runtime};

/// The step renews the mode every 60 s; a crashed page lets it lapse.
pub const TTL: Duration = Duration::from_secs(120);

/// A shortcut pressed while a practice step was open with Wispr Bro in
/// front. Only the action id: the page reads its own composer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkLessonShortcut {
    /// The fork action id, e.g. `transform.polish`, `transform.view_changes`.
    pub action: String,
}

static UNTIL: Mutex<Option<Instant>> = Mutex::new(None);

fn until() -> std::sync::MutexGuard<'static, Option<Instant>> {
    UNTIL.lock().unwrap_or_else(|e| e.into_inner())
}

/// Turns the mode on (or renews it) or off.
pub fn set(on: bool, now: Instant) {
    *until() = on.then_some(now + TTL);
}

/// Whether a shortcut pressed now goes to the practice step.
pub fn routes_to_lesson(self_in_front: bool, now: Instant) -> bool {
    self_in_front && until().is_some_and(|until| now < until)
}

/// The shortcut handlers call this first: `true` means the page got the
/// shortcut and the handler must not run.
pub fn intercept<R: Runtime>(app: &AppHandle<R>, action: &str) -> bool {
    if !routes_to_lesson(
        crate::fork::onboarding::practice::self_in_front(),
        Instant::now(),
    ) {
        return false;
    }
    log::info!("tutorials: {action} went to the practice step");
    let event = ForkLessonShortcut {
        action: action.to_owned(),
    };
    if let Err(e) = tauri_specta::Event::emit_to(&event, app, "main") {
        log::warn!("tutorials: could not hand {action} to the practice step: {e}");
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    // One test: the mode is process-wide state.
    #[test]
    fn routes_only_while_on_with_us_in_front_and_lapses() {
        let now = Instant::now();
        set(false, now);
        assert!(!routes_to_lesson(true, now), "off by default");

        set(true, now);
        assert!(routes_to_lesson(true, now));
        assert!(
            !routes_to_lesson(false, now),
            "a shortcut pressed in another app runs as usual"
        );
        assert!(routes_to_lesson(true, now + TTL - Duration::from_secs(1)));
        assert!(
            !routes_to_lesson(true, now + TTL),
            "a page that stopped renewing lets it lapse"
        );

        set(true, now + Duration::from_secs(100));
        assert!(
            routes_to_lesson(true, now + TTL + Duration::from_secs(10)),
            "renewing extends it"
        );

        set(false, now);
        assert!(!routes_to_lesson(true, now), "off ends it at once");
    }
}
