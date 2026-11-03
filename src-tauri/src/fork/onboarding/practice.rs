// SPDX-License-Identifier: GPL-3.0-or-later
//! The onboarding practice take (S-29b). While the practice screen is open
//! (`fork_onboarding_practice`, two minutes unless renewed, off when the
//! screen goes), a take that starts with Wispr Bro in front is a practice
//! take:
//!
//! - its source is `practice` (DEC-11: never counted in the stats, never
//!   "the last dictation" for paste-last), set at record start;
//! - its text goes to the simulated composer through [`ForkPracticeTake`]
//!   instead of a paste (`fork::insert::deliver_as`), so it never depends on
//!   ⌘V reaching our own web view and never lands twice;
//! - when no speech model is ready yet, the take is queued as usual (S-07c)
//!   and the screen hears `queued`; the composer fills when that queued take
//!   is transcribed, if the screen is still open (`fork::models::setup`).
//!   It is in History either way.
//!
//! A take that starts in another app is an ordinary dictation. Every take's
//! start overwrites the mark, so a stale one never reaches a later take.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Runtime};

/// The screen renews the mode every 60 s; a crashed page lets it lapse.
pub const TTL: Duration = Duration::from_secs(120);
/// Queued practice takes remembered for their late text.
const KEEP_QUEUED: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ForkPracticeTakeKind {
    /// The take's final text, for the composer.
    Text,
    /// No speech model is ready yet: the take waits in the queue.
    Queued,
}

/// A practice take reached the practice screen. Only the onboarding window
/// listens; the text is the user's own take, shown where they dictated it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkPracticeTake {
    pub kind: ForkPracticeTakeKind,
    pub text: Option<String>,
    /// The text arrived after the take had waited for the speech model.
    pub late: bool,
}

#[derive(Debug, Default)]
struct State {
    until: Option<Instant>,
    /// The take recording now (or being transcribed) is a practice take.
    take: bool,
    /// WAV names of practice takes waiting for the speech model.
    queued: Vec<String>,
}

static STATE: Mutex<State> = Mutex::new(State {
    until: None,
    take: false,
    queued: Vec::new(),
});

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Turns the mode on (or renews it) or off. Off forgets the queued takes:
/// their text then reaches History only, like any queued take.
pub fn set(on: bool, now: Instant) {
    let mut s = state();
    if on {
        s.until = Some(now + TTL);
    } else {
        *s = State::default();
    }
}

fn active(s: &State, now: Instant) -> bool {
    s.until.is_some_and(|until| now < until)
}

/// Record start: marks the take when the mode is on and Wispr Bro is in
/// front (`self_in_front`). Returns whether it is a practice take.
pub fn on_record_start(self_in_front: bool, now: Instant) -> bool {
    let mut s = state();
    s.take = self_in_front && active(&s, now);
    s.take
}

/// An ordinary (or command) take started: no mark.
pub fn clear_take() {
    state().take = false;
}

/// Delivery of a take: consumes the mark. `true` = send the text to the
/// practice screen instead of pasting.
pub fn take_delivering(now: Instant) -> bool {
    let mut s = state();
    std::mem::take(&mut s.take) && active(&s, now)
}

/// The take was queued for the speech model (S-07c). Remembers a practice
/// take's WAV for its late text; returns whether it was one.
pub fn take_queued(file_name: &str, now: Instant) -> bool {
    let mut s = state();
    if !(std::mem::take(&mut s.take) && active(&s, now)) {
        return false;
    }
    s.queued.retain(|f| f != file_name);
    s.queued.push(file_name.to_owned());
    let excess = s.queued.len().saturating_sub(KEEP_QUEUED);
    s.queued.drain(..excess);
    true
}

/// A queued take was transcribed: whether it is a practice take the open
/// screen still waits for (consumed).
pub fn queued_ready(file_name: &str, now: Instant) -> bool {
    let mut s = state();
    let Some(at) = s.queued.iter().position(|f| f == file_name) else {
        return false;
    };
    s.queued.remove(at);
    active(&s, now)
}

/// Wispr Bro is the frontmost app.
pub fn self_in_front() -> bool {
    #[cfg(target_os = "macos")]
    {
        crate::fork::ax::focus::frontmost_pid() == Some(std::process::id() as i32)
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

pub fn emit<R: Runtime>(app: &AppHandle<R>, event: ForkPracticeTake) {
    // Kind only: the text is never logged.
    log::info!(
        "onboarding: practice take {:?}{}",
        event.kind,
        if event.late { " (late)" } else { "" }
    );
    if let Err(e) = tauri_specta::Event::emit(&event, app) {
        log::warn!("onboarding: could not emit the practice take: {e}");
    }
}

/// `fork::models::queue_take`: tell the screen its take waits.
pub fn on_queued<R: Runtime>(app: &AppHandle<R>, file_name: &str) {
    if take_queued(file_name, Instant::now()) {
        emit(
            app,
            ForkPracticeTake {
                kind: ForkPracticeTakeKind::Queued,
                text: None,
                late: false,
            },
        );
    }
}

/// `fork::models::setup`: a queued take finished; fill the composer if it
/// was the open screen's practice take.
pub fn on_queued_take_done(app: &AppHandle, file_name: &str, entry_id: i64) {
    use tauri::Manager;
    if !queued_ready(file_name, Instant::now()) {
        return;
    }
    let Some(hm) = app.try_state::<std::sync::Arc<crate::managers::history::HistoryManager>>()
    else {
        return;
    };
    let entry = match tauri::async_runtime::block_on(hm.get_entry_by_id(entry_id)) {
        Ok(Some(entry)) => entry,
        Ok(None) => return,
        Err(e) => {
            log::warn!("onboarding: could not read the practice take: {e}");
            return;
        }
    };
    let text = entry
        .post_processed_text
        .filter(|t| !t.trim().is_empty())
        .unwrap_or(entry.transcription_text);
    emit(
        app,
        ForkPracticeTake {
            kind: ForkPracticeTakeKind::Text,
            text: Some(text),
            late: true,
        },
    );
}

#[cfg(test)]
pub(crate) fn reset_for_tests() {
    *state() = State::default();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The state is global: these run as one test so they cannot interleave.
    #[test]
    fn practice_takes_follow_the_screen_and_the_front_app() {
        let t0 = Instant::now();
        reset_for_tests();

        // Off: never a practice take.
        assert!(!on_record_start(true, t0));
        assert!(!take_delivering(t0));

        // On, Wispr Bro in front: marked, consumed once at delivery.
        set(true, t0);
        assert!(on_record_start(true, t0));
        assert!(take_delivering(t0));
        assert!(!take_delivering(t0), "consumed");

        // On, but the take started in another app: an ordinary dictation.
        assert!(!on_record_start(false, t0));
        assert!(!take_delivering(t0));

        // A command take clears a mark left by a take that never delivered.
        assert!(on_record_start(true, t0));
        clear_take();
        assert!(!take_delivering(t0));

        // Queued: remembered by its WAV, filled once, only while open.
        assert!(on_record_start(true, t0));
        assert!(take_queued("a.wav", t0));
        assert!(!take_delivering(t0), "a queued take is not delivered");
        assert!(
            !take_queued("b.wav", t0),
            "an unmarked take is not practice"
        );
        assert!(queued_ready("a.wav", t0));
        assert!(!queued_ready("a.wav", t0), "consumed");

        // Closing the screen forgets its queued take.
        assert!(on_record_start(true, t0));
        assert!(take_queued("c.wav", t0));
        set(false, t0);
        assert!(!queued_ready("c.wav", t0));

        // The mode lapses without renewal.
        set(true, t0);
        assert!(!on_record_start(true, t0 + TTL));
        set(true, t0 + TTL);
        assert!(on_record_start(true, t0 + TTL + Duration::from_secs(60)));
        assert!(!take_delivering(t0 + TTL * 3), "lapsed before delivery");

        // Bounded memory of queued takes.
        for i in 0..10 {
            assert!(on_record_start(true, t0 + TTL));
            assert!(take_queued(&format!("{i}.wav"), t0 + TTL));
        }
        assert_eq!(state().queued.len(), KEEP_QUEUED);
        assert!(!queued_ready("0.wav", t0 + TTL));
        assert!(queued_ready("9.wav", t0 + TTL));
        reset_for_tests();
    }
}
