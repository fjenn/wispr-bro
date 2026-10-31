// SPDX-License-Identifier: GPL-3.0-or-later
//! Notifications inbox commands (S-18): list, mark read, activate, the
//! version's feature announcement, the macOS permission state, and a
//! debug-only poster for the Mac test script.

use tauri::{AppHandle, Manager};

use super::ForkError;
use crate::fork::db::{DbError, ForkDb};
use crate::fork::notify::{
    self, inbox, FeatureAnnouncement, Notice, NotificationAction, NotificationButton,
    NotificationFilter, NotificationKind, NotificationPermission, NotificationsPage, Notifier,
    Origin,
};
use crate::fork::{ForkRuntime, ForkState};

/// Most entries one list call returns.
pub const MAX_LIST: u32 = 200;

fn ready<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<(), ForkError> {
    let ready = app.try_state::<ForkState>().is_some_and(|s| s.is_ready());
    if ready && app.try_state::<Notifier>().is_some() {
        Ok(())
    } else {
        Err(ForkError::NotReady)
    }
}

fn internal(e: DbError) -> ForkError {
    ForkError::Internal(e.to_string())
}

fn with_db<R: tauri::Runtime, T>(
    app: &AppHandle<R>,
    f: impl FnOnce(&mut rusqlite::Connection) -> Result<T, DbError>,
) -> Result<T, ForkError> {
    ready(app)?;
    app.try_state::<ForkDb>()
        .ok_or(ForkError::NotReady)?
        .with_conn(f)
        .map_err(internal)
}

fn emit_unread<R: tauri::Runtime>(app: &AppHandle<R>, unread: u32) {
    let event = notify::ForkNotificationsChanged {
        unread,
        posted: None,
    };
    if let Err(e) = tauri_specta::Event::emit(&event, app) {
        log::debug!("notify: could not emit fork-notifications-changed: {e}");
    }
}

/// Inbox entries, newest first (`limit` ≤ 200), with the unread count.
#[tauri::command]
#[specta::specta]
pub fn fork_notifications_list<R: ForkRuntime>(
    app: AppHandle<R>,
    filter: NotificationFilter,
    limit: Option<u32>,
) -> Result<NotificationsPage, ForkError> {
    let limit = limit.unwrap_or(MAX_LIST).clamp(1, MAX_LIST);
    with_db(&app, |c| inbox::list(c, filter, limit))
}

/// Marks entries read. Returns the unread count.
#[tauri::command]
#[specta::specta]
pub fn fork_notifications_mark_read<R: ForkRuntime>(
    app: AppHandle<R>,
    ids: Vec<i64>,
) -> Result<u32, ForkError> {
    let now = crate::fork::context::now_ms();
    let unread = with_db(&app, |c| {
        inbox::mark_read(c, &ids, now)?;
        inbox::unread_count(c)
    })?;
    emit_unread(&app, unread);
    Ok(unread)
}

/// Marks every entry read. Returns the unread count (0).
#[tauri::command]
#[specta::specta]
pub fn fork_notifications_mark_all_read<R: ForkRuntime>(
    app: AppHandle<R>,
) -> Result<u32, ForkError> {
    let now = crate::fork::context::now_ms();
    let unread = with_db(&app, |c| {
        inbox::mark_all_read(c, now)?;
        inbox::unread_count(c)
    })?;
    emit_unread(&app, unread);
    Ok(unread)
}

/// Activates an entry from the Hub: a click (`button` null) or one of its
/// buttons. Marks it read and runs the Rust side of the action (copy,
/// System Settings, Finder); returns the action so the Hub navigates.
#[tauri::command]
#[specta::specta]
pub fn fork_notifications_activate<R: ForkRuntime>(
    app: AppHandle<R>,
    id: i64,
    button: Option<NotificationButton>,
) -> Result<Option<NotificationAction>, ForkError> {
    ready(&app)?;
    notify::activate(&app, id, button, Origin::Hub).map_err(internal)
}

/// The feature this version announces (the Hub's one-time popup), or null.
#[tauri::command]
#[specta::specta]
pub fn fork_notifications_feature<R: ForkRuntime>(
    app: AppHandle<R>,
) -> Result<Option<FeatureAnnouncement>, ForkError> {
    ready(&app)?;
    Ok(notify::feature_announcement(
        &app,
        env!("CARGO_PKG_VERSION"),
    ))
}

/// macOS notification authorization. Never prompts.
#[tauri::command]
#[specta::specta]
pub fn fork_notifications_permission<R: ForkRuntime>(
    app: AppHandle<R>,
) -> Result<NotificationPermission, ForkError> {
    ready(&app)?;
    let notifier = app.try_state::<Notifier>().ok_or(ForkError::NotReady)?;
    Ok(notifier.sink().permission())
}

/// A sample notice of `kind`, for the debug panel and the Mac test script.
pub fn sample_notice(kind: NotificationKind, seq: i64) -> Notice {
    use crate::fork::notify::catalog;
    match kind {
        NotificationKind::ModelReady => Notice::model_ready(),
        NotificationKind::UpdateReady => Notice::update_ready(&format!("1.0.0-test.{seq}")),
        NotificationKind::QueuedTakesReady => Notice::queued_takes_ready(3),
        NotificationKind::ShortcutProblem => Notice::shortcut_problem(true),
        NotificationKind::CrashReport => Notice::crash_report(&format!("crash-test-{seq}.txt")),
        NotificationKind::DataRecovered => Notice::data_recovered(seq),
        NotificationKind::Milestone => Notice::words_milestone(catalog::WORD_MILESTONES[0]),
        NotificationKind::Tip => Notice::tip(&catalog::TIPS[0]),
        NotificationKind::Announcement => Notice::announcement(&catalog::FEATURES[0]),
    }
}

/// Posts [`sample_notice`] through the real path. Samples repeat: an entry
/// with the same dedupe key (a milestone already earned, …) is removed first.
pub fn post_sample<R: ForkRuntime>(
    app: &AppHandle<R>,
    kind: NotificationKind,
    seq: i64,
) -> Option<notify::NotificationItem> {
    let notice = sample_notice(kind, seq);
    if let Some(key) = notice.dedupe_key.clone() {
        if let Some(db) = app.try_state::<ForkDb>() {
            let cleared = db.with_conn(|c| {
                c.execute("DELETE FROM notifications WHERE dedupe_key = ?1", [key])?;
                Ok(())
            });
            if let Err(e) = cleared {
                log::debug!("notify: sample key not cleared: {e}");
            }
        }
    }
    notify::post(app, notice)
}

/// Debug builds and `--debug` only: posts a sample of `kind` after
/// `delay_ms` (≤ 60 s, so the tester can bring another app to the front),
/// through the real path (toggles, inbox, macOS policy). Samples repeat:
/// their dedupe keys are cleared first.
#[tauri::command]
#[specta::specta]
pub fn fork_notifications_debug_post<R: ForkRuntime>(
    app: AppHandle<R>,
    kind: NotificationKind,
    delay_ms: Option<u32>,
) -> Result<(), ForkError> {
    ready(&app)?;
    let cli_debug = app.try_state::<crate::CliArgs>().is_some_and(|a| a.debug);
    if !(cfg!(debug_assertions) || cli_debug) {
        return Err(ForkError::Internal("debug builds only".into()));
    }
    let seq = crate::fork::context::now_ms();
    let delay = std::time::Duration::from_millis(u64::from(delay_ms.unwrap_or(0).min(60_000)));
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        post_sample(&app, kind, seq);
    });
    Ok(())
}
