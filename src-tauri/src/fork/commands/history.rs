// SPDX-License-Identifier: GPL-3.0-or-later
//! History and stats commands (S-09): the read side of the history data
//! model for the Home rail (S-15a), History (S-15b) and Insights (S-25),
//! and soft delete with undo.
//!
//! S-15b adds the History list and its row actions: pages and search
//! ([`fork_history_page`], [`fork_history_find`], [`fork_history_rows`]),
//! Undo / Redo AI edit ([`fork_history_set_variant`]), Retry with a
//! cleanup level ([`fork_history_retry`]), Flag → dictionary
//! ([`fork_history_flag`]) and Save audio as… ([`fork_history_save_audio`]).
//! Playing audio reuses Handy's `get_audio_file_path`.

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};

use super::ForkError;
use crate::fork::db::history_list::{self, HistoryPage, HistoryRow, ShownVariant};
use crate::fork::db::stats::StatsSummary;
use crate::fork::db::{entry_meta, fts, DbError, ForkDb};
use crate::fork::dictionary::store::{self as dictionary_store, SaveOutcome};
use crate::fork::dictionary::{self, EntryInput, EntrySource};
use crate::fork::settings::CleanupLevel;
use crate::fork::ForkRuntime;
use crate::managers::transcription_jobs::TranscriptionJobManager;

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
    /// S-20: text near the caret helped this take (History says "Context
    /// used"; the text itself is never stored).
    pub context_used: bool,
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
                    deleted_at IS NOT NULL, context_used
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
                    context_used: r.get::<_, i64>(16)? == 1,
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
    // As Handy's own delete does: a retry still queued for it stops.
    if let Some(jobs) = app.try_state::<std::sync::Arc<TranscriptionJobManager>>() {
        for &id in &ids {
            jobs.cancel(id);
        }
    }
    if n > 0 {
        crate::fork::db::jobs::schedule_purge(&app);
        crate::fork::app_menu::refresh(&app, crate::fork::app_menu::Refresh::Data);
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
    if n > 0 {
        crate::fork::app_menu::refresh(&app, crate::fork::app_menu::Refresh::Data);
    }
    Ok(u32::try_from(n).unwrap_or(u32::MAX))
}

fn handy<R: ForkRuntime>(
    app: &AppHandle<R>,
) -> Result<std::sync::Arc<dyn crate::fork::db::handy::HandyHistory>, ForkError> {
    R::handy_history(app).ok_or(ForkError::NotReady)
}

/// History rows older than row `cursor` (the newest without one), at most
/// `limit` (≤ 100). Soft-deleted rows are left out.
#[tauri::command]
#[specta::specta]
pub fn fork_history_page<R: ForkRuntime>(
    app: AppHandle<R>,
    cursor: Option<i64>,
    limit: u32,
) -> Result<HistoryPage, ForkError> {
    let handy = handy(&app)?;
    history_list::page(&*db(&app)?, handy.as_ref(), cursor, limit as usize)
        .map_err(ForkError::Internal)
}

/// History rows whose text contains `query`, newest first, `limit` from
/// `offset` on; `next` is the offset of the next page.
#[tauri::command]
#[specta::specta]
pub fn fork_history_find<R: ForkRuntime>(
    app: AppHandle<R>,
    query: String,
    offset: u32,
    limit: u32,
) -> Result<HistoryPage, ForkError> {
    let handy = handy(&app)?;
    history_list::find(&*db(&app)?, handy.as_ref(), &query, offset, limit)
        .map_err(ForkError::Internal)
}

/// These rows as the list shows them (after Handy added or re-transcribed
/// them). Missing and soft-deleted ids are left out.
#[tauri::command]
#[specta::specta]
pub fn fork_history_rows<R: ForkRuntime>(
    app: AppHandle<R>,
    ids: Vec<i64>,
) -> Result<Vec<HistoryRow>, ForkError> {
    let handy = handy(&app)?;
    history_list::rows(&*db(&app)?, handy.as_ref(), &ids).map_err(ForkError::Internal)
}

fn one_row<R: ForkRuntime>(app: &AppHandle<R>, id: i64) -> Result<Option<HistoryRow>, ForkError> {
    let handy = handy(app)?;
    Ok(history_list::rows(&*db(app)?, handy.as_ref(), &[id])
        .map_err(ForkError::Internal)?
        .into_iter()
        .next())
}

/// Undo AI edit (`raw`) or Redo AI edit (`final`): which text the row shows
/// and copies. Never re-pastes (DEC-19); the stats are unchanged. Returns
/// the row.
#[tauri::command]
#[specta::specta]
pub fn fork_history_set_variant<R: ForkRuntime>(
    app: AppHandle<R>,
    id: i64,
    variant: ShownVariant,
) -> Result<Option<HistoryRow>, ForkError> {
    db(&app)?
        .with_conn(|c| history_list::set_variant(c, id, variant))
        .map_err(internal)?;
    log::info!("history: entry {id} shows the {} text", variant.as_str());
    crate::fork::app_menu::refresh(&app, crate::fork::app_menu::Refresh::Data);
    one_row(&app, id)
}

/// Transcribes the row's audio again with the current model (Handy's
/// background job, C-43) and cleans it at `level`, or at the cleanup
/// setting when `None`. The text it replaces becomes `previous_text`.
#[tauri::command]
#[specta::specta]
pub fn fork_history_retry<R: ForkRuntime>(
    app: AppHandle<R>,
    id: i64,
    level: Option<CleanupLevel>,
) -> Result<(), ForkError> {
    let jobs = app
        .try_state::<std::sync::Arc<TranscriptionJobManager>>()
        .ok_or(ForkError::NotReady)?;
    db(&app)?
        .with_conn(|c| crate::fork::models::queue::retry(c, id))
        .map_err(internal)?;
    crate::fork::llm::post_process::set_retry_level(id, level);
    jobs.retry(id);
    log::info!(
        "history: entry {id} retry queued (cleanup {})",
        level.map(CleanupLevel::as_str).unwrap_or("setting")
    );
    Ok(())
}

/// What the "Wrong word?" dialog saves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum FlagKind {
    /// Replace `from` with `to` every time (a dictionary replacement).
    Rule,
    /// Teach the spelling `to` (a dictionary word).
    Word,
}

/// The dictionary's answer to a flag, and the row after it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct FlagResult {
    pub outcome: SaveOutcome,
    /// The flagged row (when the entry was saved or already there).
    pub row: Option<HistoryRow>,
}

/// The dictionary input a flag asks for.
pub fn flag_input(from: &str, to: &str, kind: FlagKind) -> EntryInput {
    match kind {
        FlagKind::Rule => EntryInput {
            phrase: from.to_owned(),
            replacement: Some(to.to_owned()),
        },
        FlagKind::Word => EntryInput {
            phrase: to.to_owned(),
            replacement: None,
        },
    }
}

/// Saves a flag in `conn`: the dictionary entry (source `flag`, or an edit
/// of `replace`), and for a saved or already present entry the row's
/// `flagged` mark and its `corrections` row.
pub fn save_flag(
    conn: &mut rusqlite::Connection,
    id: i64,
    from: &str,
    to: &str,
    kind: FlagKind,
    replace: Option<i64>,
    now: i64,
) -> Result<SaveOutcome, DbError> {
    // A blank correction would turn a replacement into a word.
    if to.trim().is_empty() || (kind == FlagKind::Rule && from.trim().is_empty()) {
        return Ok(SaveOutcome::Invalid {
            problem: dictionary::EntryProblem::Empty,
        });
    }
    let entry = match dictionary::clean(&flag_input(from, to, kind)) {
        Ok(e) => e,
        Err(problem) => return Ok(SaveOutcome::Invalid { problem }),
    };
    dictionary_store::ensure_norms(conn)?;
    let outcome = match replace {
        Some(existing) => dictionary_store::update(conn, existing, &entry, now)?,
        None => dictionary_store::add(conn, &entry, EntrySource::Flag, now)?,
    };
    if matches!(
        outcome,
        SaveOutcome::Saved { .. } | SaveOutcome::Duplicate { .. }
    ) {
        history_list::record_flag(conn, id, from.trim(), to.trim(), now)?;
    }
    Ok(outcome)
}

/// Flag (DEC-18): the user picked `from` in the row's text and typed `to`.
/// Adds a replacement or a word through the S-16 store; `replace` edits
/// that existing entry instead (the dialog's "Replace it" after a
/// conflict).
#[tauri::command]
#[specta::specta]
pub fn fork_history_flag<R: ForkRuntime>(
    app: AppHandle<R>,
    id: i64,
    from: String,
    to: String,
    kind: FlagKind,
    replace: Option<i64>,
) -> Result<FlagResult, ForkError> {
    let now = crate::fork::context::now_ms();
    let outcome = db(&app)?
        .with_conn(|c| save_flag(c, id, &from, &to, kind, replace, now))
        .map_err(internal)?;
    let row = match &outcome {
        SaveOutcome::Saved { .. } => {
            super::dictionary::changed(&app);
            log::info!("history: entry {id} flagged, dictionary entry saved");
            one_row(&app, id)?
        }
        SaveOutcome::Duplicate { .. } => one_row(&app, id)?,
        _ => None,
    };
    Ok(FlagResult { outcome, row })
}

/// Why Save audio as… did not write the file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", content = "message")]
pub enum SaveAudioError {
    /// The row or its recording is gone.
    NoAudio,
    /// Writing the copy failed (message for the log).
    WriteFailed(String),
}

/// Copies the row's recording to `path` (the save panel's choice; written
/// beside it and renamed, so a failure never leaves half a file).
#[tauri::command]
#[specta::specta]
pub fn fork_history_save_audio<R: ForkRuntime>(
    app: AppHandle<R>,
    id: i64,
    path: String,
) -> Result<Option<SaveAudioError>, ForkError> {
    let handy = handy(&app)?;
    let removed = db(&app)?
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT audio_deleted_at IS NOT NULL FROM entry_meta WHERE entry_id = ?1",
                [id],
                |r| r.get::<_, bool>(0),
            )
            .optional()?
            .unwrap_or(false))
        })
        .map_err(internal)?;
    let entry = handy.entry(id).map_err(ForkError::Internal)?;
    let Some(entry) = entry.filter(|_| !removed) else {
        return Ok(Some(SaveAudioError::NoAudio));
    };
    let source = handy.audio_path(&entry.file_name);
    if !source.is_file() {
        return Ok(Some(SaveAudioError::NoAudio));
    }
    match copy_atomically(&source, std::path::Path::new(&path)) {
        Ok(()) => {
            log::info!("history: entry {id} audio saved");
            Ok(None)
        }
        Err(e) => {
            log::warn!("history: entry {id} audio not saved: {e:?}");
            Ok(Some(e))
        }
    }
}

/// Lets the webview play Handy's recordings dir through the asset protocol
/// when it lies outside the static `$APPDATA/**` scope (portable mode).
/// Nothing else is added.
pub fn allow_recordings_in_asset_scope<R: tauri::Runtime>(app: &AppHandle<R>) {
    let Some(hm) = app.try_state::<std::sync::Arc<crate::managers::history::HistoryManager>>()
    else {
        return;
    };
    let dir = hm.recordings_dir().to_path_buf();
    let inside = app
        .path()
        .app_data_dir()
        .is_ok_and(|data| dir.starts_with(data));
    if inside {
        return;
    }
    match app.asset_protocol_scope().allow_directory(&dir, true) {
        Ok(()) => log::info!("history: recordings dir added to the asset scope (portable)"),
        Err(e) => log::warn!("history: could not add the recordings dir to the asset scope: {e}"),
    }
}

/// Copies `from` to `to` through a temporary file next to `to`.
pub fn copy_atomically(from: &std::path::Path, to: &std::path::Path) -> Result<(), SaveAudioError> {
    let fail = |e: std::io::Error| SaveAudioError::WriteFailed(e.to_string());
    let name = to
        .file_name()
        .ok_or_else(|| SaveAudioError::WriteFailed("no file name".into()))?;
    let mut partial = name.to_os_string();
    partial.push(".partial");
    let tmp = to.with_file_name(partial);
    std::fs::copy(from, &tmp).map_err(fail)?;
    if let Err(e) = std::fs::rename(&tmp, to) {
        let _ = std::fs::remove_file(&tmp);
        return Err(fail(e));
    }
    Ok(())
}
