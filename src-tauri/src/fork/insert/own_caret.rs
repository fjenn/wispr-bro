// SPDX-License-Identifier: GPL-3.0-or-later
//! The text before the caret in one of our own windows (the Scratchpad note,
//! a Hub field), for the smart leading space ([`super::spacing`]).
//!
//! Accessibility is never called on our own process (`fork::ax::is_own_pid`:
//! it can block or crash on our own web view), so the focused web view
//! answers instead: Rust emits `fork-before-caret-request` `{ id, units }`,
//! and the window that has focus and a text field focused replies with
//! `fork-before-caret-reply` `{ id, text }` (`src/fork/ui/beforeCaret.ts`).
//! No reply within the budget is "cannot tell", as for an app that does not
//! answer. The text is never logged or kept.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Listener};

const REQUEST: &str = "fork-before-caret-request";
const REPLY: &str = "fork-before-caret-reply";

static APP: OnceLock<AppHandle> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
static PENDING: Mutex<Option<HashMap<u64, SyncSender<Option<String>>>>> = Mutex::new(None);

#[derive(Serialize, Clone)]
struct Request {
    id: u64,
    units: usize,
}

#[derive(Deserialize)]
struct Reply {
    id: u64,
    text: Option<String>,
}

fn pending() -> std::sync::MutexGuard<'static, Option<HashMap<u64, SyncSender<Option<String>>>>> {
    PENDING.lock().unwrap_or_else(|p| p.into_inner())
}

/// From `fork::on_ready`: listens for the windows' replies.
pub fn on_ready(app: &AppHandle) {
    if APP.set(app.clone()).is_err() {
        return;
    }
    app.listen_any(REPLY, |event| {
        let Ok(reply) = serde_json::from_str::<Reply>(event.payload()) else {
            return;
        };
        if let Some(tx) = pending().as_mut().and_then(|m| m.remove(&reply.id)) {
            let _ = tx.try_send(reply.text);
        }
    });
}

/// Up to `units` UTF-16 units before the caret of the focused field in our
/// own focused window. `Some("")` at a field's start; `None` when no window
/// answered within `budget` (no field focused, or a window without the
/// listener).
pub fn before_caret(units: usize, budget: Duration) -> Option<String> {
    let app = APP.get()?;
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = mpsc::sync_channel(1);
    pending().get_or_insert_with(HashMap::new).insert(id, tx);
    let answer = match app.emit(REQUEST, Request { id, units }) {
        Ok(()) => rx.recv_timeout(budget).ok().flatten(),
        Err(_) => None,
    };
    if let Some(m) = pending().as_mut() {
        m.remove(&id);
    }
    // A reply longer than asked is cut to its tail.
    answer.map(|t| {
        let u: Vec<u16> = t.encode_utf16().collect();
        String::from_utf16_lossy(&u[u.len().saturating_sub(units)..])
    })
}
