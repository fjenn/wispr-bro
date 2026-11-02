// SPDX-License-Identifier: GPL-3.0-or-later
//! The action registry and the Esc registry (R5 §S-01, §4.8).
//!
//! **Actions.** Fixed ids (`fork::settings::hotkey_actions`: `dictate`,
//! `hands_free`, `cancel`, and S-06's `paste_last`, `copy_last`,
//! `press_enter`, `undo_cancel`) plus runtime ids that later slices add
//! (user transforms `transform.<id>`, Scratchpad, Notetaker). The three
//! dictation ids are run by the grammar itself; every other id fires a
//! handler registered here. A keyed binding is only registered (and its
//! chord only blocked) while its action has a handler, so ⌃⌘V keeps its
//! normal meaning in other apps until paste-last is registered.
//!
//! The S-06 handlers live in `actions/` ([`paste_last`], [`copy_last`],
//! [`press_enter`], [`undo_cancel`]); `fork::recovery` registers them.
//!
//! **Esc registry.** Cards, toasts and the spotlight observe Esc from the
//! fork tap; they never block it. Observers are called on the engine
//! thread for every Esc key-down, in registration order, and must return
//! quickly (post to their own thread or window).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use crate::fork::settings::{hotkey_actions, MAX_ID_LEN};

pub mod copy_last;
pub mod paste_last;
pub mod press_enter;
pub mod undo_cancel;

/// A registered action handler.
pub type Handler = Arc<dyn Fn() + Send + Sync>;

/// An Esc observer. Gets whether a take was active (and so Esc cancelled
/// it); returns nothing and never blocks the key.
pub type EscObserver = Arc<dyn Fn(EscContext) + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EscContext {
    /// The Esc cancelled a recording or a transcription.
    pub cancelled_take: bool,
}

/// The ids the grammar runs (never in the handler table).
pub const GRAMMAR_ACTIONS: [&str; 4] = [
    hotkey_actions::DICTATE,
    hotkey_actions::HANDS_FREE,
    hotkey_actions::CANCEL,
    hotkey_actions::COMMAND_MODE,
];

/// Prefixes of runtime ids.
pub const RUNTIME_PREFIXES: [&str; 3] = ["transform.", "scratchpad.", "notetaker."];

/// Whether `id` is a known action id: fixed, or `<runtime prefix><name>`
/// within the settings id rules (`[a-z0-9_.-]`, ≤ 64).
pub fn is_action_id(id: &str) -> bool {
    use hotkey_actions::*;
    if [
        DICTATE,
        HANDS_FREE,
        CANCEL,
        COMMAND_MODE,
        PASTE_LAST,
        COPY_LAST,
        PRESS_ENTER,
        UNDO_CANCEL,
    ]
    .contains(&id)
    {
        return true;
    }
    id.len() <= MAX_ID_LEN
        && RUNTIME_PREFIXES
            .iter()
            .any(|p| id.strip_prefix(p).is_some_and(|rest| !rest.is_empty()))
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_.-".contains(c))
}

#[derive(Default)]
struct Inner {
    handlers: BTreeMap<String, Handler>,
    esc: Vec<(u64, EscObserver)>,
    next_esc: u64,
}

/// Handle returned by [`Registry::observe_esc`]; pass it back to stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EscToken(u64);

/// The registry. One per app (`fork::shortcut` manages it), cheap to clone.
#[derive(Clone, Default)]
pub struct Registry {
    inner: Arc<Mutex<Inner>>,
}

impl Registry {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Registers (or replaces) the handler of `id`. Grammar actions and
    /// unknown ids are refused.
    pub fn register(&self, id: &str, handler: Handler) -> Result<(), String> {
        if GRAMMAR_ACTIONS.contains(&id) {
            return Err(format!("{id} is run by the hotkey grammar"));
        }
        if !is_action_id(id) {
            return Err(format!("{id:?} is not an action id"));
        }
        self.lock().handlers.insert(id.to_string(), handler);
        Ok(())
    }

    /// Removes a handler (a transform was deleted). Returns whether it was
    /// there.
    pub fn unregister(&self, id: &str) -> bool {
        self.lock().handlers.remove(id).is_some()
    }

    pub fn handles(&self, id: &str) -> bool {
        self.lock().handlers.contains_key(id)
    }

    /// Runs the handler of `id`. Returns whether one ran.
    pub fn fire(&self, id: &str) -> bool {
        let handler = self.lock().handlers.get(id).cloned();
        match handler {
            Some(h) => {
                h();
                true
            }
            None => false,
        }
    }

    pub fn observe_esc(&self, observer: EscObserver) -> EscToken {
        let mut inner = self.lock();
        inner.next_esc += 1;
        let token = inner.next_esc;
        inner.esc.push((token, observer));
        EscToken(token)
    }

    pub fn unobserve_esc(&self, token: EscToken) {
        self.lock().esc.retain(|(t, _)| *t != token.0);
    }

    /// Tells every observer about an Esc key-down.
    pub fn notify_esc(&self, ctx: EscContext) {
        let observers: Vec<EscObserver> = self.lock().esc.iter().map(|(_, o)| o.clone()).collect();
        for o in observers {
            o(ctx);
        }
    }
}
