// SPDX-License-Identifier: GPL-3.0-or-later
//! The history jobs' thread: the launch [`reconcile`](super::reconcile),
//! then [`retention`](super::retention) at launch, every
//! [`retention::INTERVAL`] and whenever a retention setting changes
//! ([`wake`]); plus the delayed purge of soft-deleted rows
//! ([`schedule_purge`]).
//!
//! Everything runs on the fork's own threads, never on Handy's async
//! runtime, because Handy's history API is driven with `block_on`.

use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Manager};

use super::handy::HandyHistory;
use super::{entry_meta, reconcile, retention, ForkDb};
use crate::fork::settings::ForkSettingsHandle;
use crate::fork::ForkRuntime;

static WAKE: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

/// Runs retention again soon (a retention setting changed).
pub fn wake() {
    let (flag, cv) = &WAKE;
    *flag.lock().unwrap_or_else(|e| e.into_inner()) = true;
    cv.notify_all();
}

/// Waits until `timeout` passes or [`wake`] is called.
fn sleep_or_wake(timeout: Duration) {
    let (flag, cv) = &WAKE;
    let guard = flag.lock().unwrap_or_else(|e| e.into_inner());
    let (mut guard, _) = cv
        .wait_timeout_while(guard, timeout, |woken| !*woken)
        .unwrap_or_else(|e| e.into_inner());
    *guard = false;
}

fn policy<R: ForkRuntime>(app: &AppHandle<R>) -> Option<retention::Policy> {
    let handle = app.try_state::<ForkSettingsHandle>()?;
    match handle.get(app) {
        Ok(s) => Some(retention::Policy::from_settings(&s)),
        Err(e) => {
            log::warn!("retention: fork settings unavailable ({e}); skipping this run");
            None
        }
    }
}

fn run_retention<R: ForkRuntime>(app: &AppHandle<R>, db: &ForkDb, handy: &dyn HandyHistory) {
    let Some(policy) = policy(app) else {
        return;
    };
    match retention::run(db, handy, retention::now_ms(), policy) {
        Ok(r) => log::info!(
            "retention: {} audio file(s) and {} row(s) deleted, {} saved kept, {} failed \
             (text {:?} days, audio {:?} days)",
            r.audio_deleted,
            r.rows_deleted,
            r.kept_saved,
            r.failed,
            policy.text_days,
            policy.audio_days
        ),
        Err(e) => log::error!("retention: run failed: {e}"),
    }
}

/// Starts the jobs thread (from `fork::on_ready`, after Handy's managers
/// exist).
pub fn start<R: ForkRuntime>(app: &AppHandle<R>) {
    let Some(handy) = R::handy_history(app) else {
        log::warn!("history: Handy's history manager is not available; jobs not started");
        return;
    };
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("fork-history-jobs".into())
        .spawn(move || {
            let Some(db) = app.try_state::<ForkDb>() else {
                return;
            };
            match reconcile::run(
                &db,
                handy.as_ref(),
                crate::fork::context::now_ms(),
                crate::fork::stats::local_day,
            ) {
                Ok(r) => log::info!(
                    "history: reconcile backfilled {}, dropped {}, purged {}",
                    r.backfilled,
                    r.dropped,
                    r.purged
                ),
                Err(e) => log::error!("history: reconcile failed: {e}"),
            }
            loop {
                run_retention(&app, &db, handy.as_ref());
                sleep_or_wake(retention::INTERVAL);
            }
        });
    if let Err(e) = spawned {
        log::error!("history: could not start the jobs thread: {e}");
    }
}

/// Purges soft-deleted rows once their undo window has passed.
pub fn schedule_purge<R: ForkRuntime>(app: &AppHandle<R>) {
    let Some(handy) = R::handy_history(app) else {
        return;
    };
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("fork-history-purge".into())
        .spawn(move || {
            std::thread::sleep(entry_meta::PURGE_DELAY + Duration::from_millis(100));
            purge_due(&app, handy);
        });
    if let Err(e) = spawned {
        log::error!("history: could not schedule the purge: {e}");
    }
}

fn purge_due<R: ForkRuntime>(app: &AppHandle<R>, handy: Arc<dyn HandyHistory>) {
    let Some(db) = app.try_state::<ForkDb>() else {
        return;
    };
    let now = crate::fork::context::now_ms();
    let result = db
        .with_conn(|c| entry_meta::due_purges(c, now, entry_meta::PURGE_DELAY))
        .and_then(|ids| entry_meta::purge(&db, handy.as_ref(), &ids));
    match result {
        Ok(ids) if !ids.is_empty() => log::info!("history: purged {} deleted row(s)", ids.len()),
        Ok(_) => {}
        Err(e) => log::error!("history: purge failed: {e}"),
    }
}
