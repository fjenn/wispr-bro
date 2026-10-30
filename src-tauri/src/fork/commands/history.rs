// SPDX-License-Identifier: GPL-3.0-or-later
//! History and stats commands (S-09): the read side of the history data
//! model for the Home rail (S-15a), History (S-15b) and Insights (S-25),
//! and soft delete with undo.

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};

use super::ForkError;
use crate::fork::db::stats::StatsSummary;
use crate::fork::db::{entry_meta, fts, DbError, ForkDb};
use crate::fork::ForkRuntime;

/// Most results one search returns.
pub const MAX_SEARCH_RESULTS: u32 = 500;

fn db<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<tauri::State<'_, ForkDb>, ForkError> {
    app.try_state::<ForkDb>().ok_or(ForkError::NotReady)
}

fn internal(e: DbError) -> ForkError {
    ForkError::Internal(e.to_string())
}

/// What the fork knows about one history entry, beyond Handy's row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct ForkEntryMeta {
    pub entry_id: i64,
    /// Unix ms at record start.
    pub created_at: i64,
    /// Verbatim ASR text (D-06), before any fix.
    pub asr_text: Option<String>,
    pub final_text: Option<String>,
    pub previous_text: Option<String>,
    pub audio_ms: i64,
    pub word_count: i64,
    pub app_bundle_id: Option<String>,
    pub app_name: Option<String>,
    pub style_group: Option<String>,
    pub usage_category: Option<String>,
    pub paste_outcome: Option<String>,
    pub source: String,
    /// The WAV is still on disk (not expired, not lost to a full disk).
    pub has_audio: bool,
    /// Rebuilt from Handy's row at launch.
    pub backfilled: bool,
    /// Soft-deleted: hidden, purged after 10 s unless undone.
    pub deleted: bool,
}

pub(crate) fn read_meta(
    conn: &rusqlite::Connection,
    id: i64,
) -> Result<Option<ForkEntryMeta>, DbError> {
    Ok(conn
        .query_row(
            "SELECT entry_id, created_at, asr_text, final_text, previous_text, audio_ms,
                    word_count, app_bundle_id, app_name, style_group, usage_category,
                    paste_outcome, source, audio_deleted_at IS NULL, backfilled,
                    deleted_at IS NOT NULL
             FROM entry_meta WHERE entry_id = ?1",
            [id],
            |r| {
                Ok(ForkEntryMeta {
                    entry_id: r.get(0)?,
                    created_at: r.get(1)?,
                    asr_text: r.get(2)?,
                    final_text: r.get(3)?,
                    previous_text: r.get(4)?,
                    audio_ms: r.get(5)?,
                    word_count: r.get(6)?,
                    app_bundle_id: r.get(7)?,
                    app_name: r.get(8)?,
                    style_group: r.get(9)?,
                    usage_category: r.get(10)?,
                    paste_outcome: r.get(11)?,
                    source: r.get(12)?,
                    has_audio: r.get(13)?,
                    backfilled: r.get::<_, i64>(14)? == 1,
                    deleted: r.get(15)?,
                })
            },
        )
        .optional()?)
}

/// Lifetime and today's stats (survive retention and deletes).
#[tauri::command]
#[specta::specta]
pub fn fork_stats_summary<R: ForkRuntime>(app: AppHandle<R>) -> Result<StatsSummary, ForkError> {
    let today = chrono::Local::now().date_naive();
    db(&app)?
        .with_conn(|c| crate::fork::db::stats::summary(c, today))
        .map_err(internal)
}

/// Entry ids whose text contains `query`, newest first, soft-deleted rows
/// left out. Fewer than 3 characters use a `LIKE` scan.
#[tauri::command]
#[specta::specta]
pub fn fork_history_search<R: ForkRuntime>(
    app: AppHandle<R>,
    query: String,
    limit: u32,
) -> Result<Vec<i64>, ForkError> {
    let limit = limit.clamp(1, MAX_SEARCH_RESULTS);
    db(&app)?
        .with_conn(|c| fts::search_history(c, &query, limit))
        .map_err(internal)
}

/// The fork's data for these entries (unknown ids are skipped).
#[tauri::command]
#[specta::specta]
pub fn fork_history_meta<R: ForkRuntime>(
    app: AppHandle<R>,
    ids: Vec<i64>,
) -> Result<Vec<ForkEntryMeta>, ForkError> {
    db(&app)?
        .with_conn(|c| {
            let mut out = Vec::with_capacity(ids.len());
            for id in ids {
                if let Some(m) = read_meta(c, id)? {
                    out.push(m);
                }
            }
            Ok(out)
        })
        .map_err(internal)
}

/// Soft-deletes entries: hidden now, deleted through Handy in 10 s unless
/// [`fork_history_undo_delete`] runs first. Returns how many were hidden.
#[tauri::command]
#[specta::specta]
pub fn fork_history_delete<R: ForkRuntime>(
    app: AppHandle<R>,
    ids: Vec<i64>,
) -> Result<u32, ForkError> {
    let now = crate::fork::context::now_ms();
    let n = db(&app)?
        .with_conn(|c| entry_meta::soft_delete(c, &ids, now))
        .map_err(internal)?;
    if n > 0 {
        crate::fork::db::jobs::schedule_purge(&app);
    }
    Ok(u32::try_from(n).unwrap_or(u32::MAX))
}

/// Restores soft-deleted entries still inside their undo window.
#[tauri::command]
#[specta::specta]
pub fn fork_history_undo_delete<R: ForkRuntime>(
    app: AppHandle<R>,
    ids: Vec<i64>,
) -> Result<u32, ForkError> {
    let n = db(&app)?
        .with_conn(|c| entry_meta::undo_delete(c, &ids))
        .map_err(internal)?;
    Ok(u32::try_from(n).unwrap_or(u32::MAX))
}
