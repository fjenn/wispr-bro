// SPDX-License-Identifier: GPL-3.0-or-later
//! Dictionary auto-learn (S-34, D-52, W-27; DEC-20): opt-in, off by
//! default ("Learn from my corrections", `ForkSettings::auto_learn_enabled`).
//!
//! After a take is pasted ([`on_pasted`], from `fork::insert`'s macOS
//! delivery), the field it went into is watched for up to 30 s on the AX
//! thread ([`watcher`]). When the user commits the field (Return, keypad
//! Enter, Tab, focus moving away, or the field emptying as a message is
//! sent), the dictation as pasted is compared with what the user left
//! ([`diff`]); spelling fixes of names and jargon become suggestions in
//! Dictionary › Suggested ([`review`]), which the user accepts or rejects.
//! Rejected words never return.
//!
//! Privacy (R5 S-34 rules and exit criterion): the field's text is held
//! in memory by the watch only and dropped when it ends; only the
//! corrected word pair is stored (a `dictionary_entries` suggestion and its
//! `corrections` row); the log gets the app family, the outcome, counts and
//! times, never text. The S-20 exclusions apply before anything is read
//! (Secure Input, secure, numeric and sensitive fields, password managers,
//! banking apps and sites, terminals, browser address bars, Wispr Bro), and
//! Secure Input coming on ends the watch. Chromium and Electron trees are
//! read under the S-05 lease, released when the watch ends.

pub mod diff;
pub mod review;
pub mod watcher;

#[cfg(test)]
mod tests;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Mutex;

use tauri::{AppHandle, Manager, Runtime};

/// The current watch; a newer paste (or turning the setting off) ends an
/// older one.
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// Commit keys for the current watch.
static KEYS: Mutex<Option<mpsc::Sender<()>>> = Mutex::new(None);

fn keys() -> std::sync::MutexGuard<'static, Option<mpsc::Sender<()>>> {
    KEYS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Whether "Learn from my corrections" is on.
pub fn enabled<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.try_state::<crate::fork::settings::ForkSettingsHandle>()
        .and_then(|h| h.get(app).ok())
        .is_some_and(|s| s.auto_learn_enabled)
}

/// Listens for commit keys (Return, keypad Enter, Tab) from the fork tap.
pub fn on_ready() {
    crate::fork::shortcut::registry().observe_commit_key(std::sync::Arc::new(|| {
        if let Some(tx) = keys().as_ref() {
            let _ = tx.send(());
        }
    }));
}

/// Ends the watch in progress, if any (setting turned off, data erased).
pub fn stop() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    *keys() = None;
}

/// A take's text was pasted into the focused field (not sent: a take with
/// an automatic Return is not watched). Starts the watch when auto-learn
/// is on. Never waits.
pub fn on_pasted<R: Runtime>(app: &AppHandle<R>, text: &str) {
    if !enabled(app) {
        return;
    }
    let Some(session) = watcher::Session::new(text) else {
        return;
    };
    #[cfg(target_os = "macos")]
    {
        if !crate::fork::ax::accessibility_trusted() {
            return;
        }
        let id = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
        let (tx, rx) = mpsc::channel();
        *keys() = Some(tx);
        let app = app.clone();
        let spawned = std::thread::Builder::new()
            .name("wb-learn".into())
            .spawn(move || run(&app, id, session, rx));
        if let Err(e) = spawned {
            log::warn!("learn: could not start the watch: {e}");
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = session;
}

#[cfg(target_os = "macos")]
fn run<R: Runtime>(
    app: &AppHandle<R>,
    id: u64,
    mut session: watcher::Session,
    keys: mpsc::Receiver<()>,
) {
    use std::time::{Duration, Instant};
    use watcher::{mac, Observation, Outcome, Reason};

    let budget = crate::fork::ax::READ_BUDGET;
    let start = Instant::now();
    match crate::fork::ax::thread::run(budget, move |ctx| mac::arm(ctx, id)) {
        Some(Ok(())) => log::info!("learn: watching the field"),
        Some(Err(skip)) => {
            log::info!("learn: not watched ({skip})");
            return;
        }
        None => {
            log::info!("learn: not watched (AX did not answer)");
            crate::fork::ax::thread::post(move |ctx| mac::disarm(ctx, id));
            return;
        }
    }
    let outcome = loop {
        if GENERATION.load(Ordering::SeqCst) != id || !enabled(app) {
            break Outcome::Nothing(Reason::Cancelled);
        }
        let mut key = match keys.recv_timeout(Duration::from_millis(watcher::POLL_MS)) {
            Ok(()) => true,
            Err(mpsc::RecvTimeoutError::Timeout) => false,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // A newer watch took the key channel: this one is over.
                break Outcome::Nothing(Reason::Cancelled);
            }
        };
        while keys.try_recv().is_ok() {
            key = true;
        }
        let obs = crate::fork::ax::thread::run(budget, move |ctx| mac::poll(ctx, id))
            .unwrap_or(Observation::Unreadable);
        let elapsed = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
        if let Some(o) = session.observe(elapsed, obs, key) {
            break o;
        }
    };
    drop(session);
    crate::fork::ax::thread::post(move |ctx| mac::disarm(ctx, id));
    let ms = start.elapsed().as_millis();
    match outcome {
        Outcome::Edited {
            commit,
            original,
            edited,
        } => {
            let found = diff::corrections(&original, &edited);
            drop((original, edited));
            let added = if found.is_empty() || !enabled(app) {
                0
            } else {
                save(app, &found)
            };
            log::info!(
                "learn: committed ({}) after {ms} ms: {} correction(s), {added} suggestion(s)",
                commit.as_str(),
                found.len()
            );
        }
        Outcome::Nothing(reason) => {
            log::info!("learn: nothing learned ({}) after {ms} ms", reason.as_str());
        }
    }
}

/// Stores the suggestions and tells the Dictionary page and the bar.
/// Returns how many were added.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn save<R: Runtime>(app: &AppHandle<R>, found: &[diff::Correction]) -> usize {
    let Some(db) = app.try_state::<crate::fork::db::ForkDb>() else {
        return 0;
    };
    let now = crate::fork::context::now_ms();
    match db.with_conn(|c| review::suggest_entries(c, found, now)) {
        Ok(added) => {
            if !added.is_empty() {
                super::notify_changed(app);
                // The bar's "New word" note offers Add for each (it waits in
                // Dictionary › Suggested either way).
                crate::fork::pill::words_learned(
                    added
                        .iter()
                        .map(|s| crate::fork::pill::model::LearnedWord {
                            id: s.id,
                            word: s.word.clone(),
                        })
                        .collect(),
                );
            }
            added.len()
        }
        Err(e) => {
            log::warn!("learn: could not save the suggestions: {e}");
            0
        }
    }
}
