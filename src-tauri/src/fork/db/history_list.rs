// SPDX-License-Identifier: GPL-3.0-or-later
//! The read side of the History list (S-15b): Handy's rows, through
//! [`HandyHistory`] only, merged with their `entry_meta`.
//!
//! - **Order and paging** follow Handy's own cursor (row id, newest first),
//!   so a 10,000-row history is read a page at a time. Soft-deleted rows are
//!   left out; a page is refilled from the next Handy page so it is never
//!   short while more rows exist.
//! - **Text:** the row shows what was inserted (`inserted_text`, after a
//!   snippet expanded, S-17; audit OT2), else `final_text` (the spoken
//!   words, which the stats use and `entry_meta` keeps for re-processing),
//!   or Handy's `transcription_text` (the text before the LLM cleanup)
//!   after "Undo AI edit" (`shown_variant = 'raw'`, DEC-19). Copy uses the
//!   shown text. Search covers the final and the inserted text.
//! - **Audio:** available while `audio_deleted_at` is NULL and the WAV is on
//!   disk; `expired` once the retention job removed it; `missing` when it
//!   was never written (a full disk sets `audio_deleted_at` to the take's
//!   start) or the file is gone.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::handy::HandyHistory;
use super::{fts, DbError, ForkDb};
use crate::managers::history::HistoryEntry;

/// Rows per page (Handy reads at most 100 at a time).
pub const PAGE_ROWS: usize = 100;

/// Which text a row shows and copies (`entry_meta.shown_variant`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ShownVariant {
    /// After the AI cleanup (the default).
    Final,
    /// Before it ("Undo AI edit").
    Raw,
}

impl ShownVariant {
    pub fn as_str(self) -> &'static str {
        match self {
            ShownVariant::Final => "final",
            ShownVariant::Raw => "raw",
        }
    }
}

/// Whether a row's recording can be played and saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum AudioState {
    Available,
    /// Removed by audio retention (14 days by default, DEC-07).
    Expired,
    /// Never saved (full disk) or the file is gone.
    Missing,
}

/// One History row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct HistoryRow {
    pub id: i64,
    /// Unix ms at record start (Handy's save time for rows the fork did not
    /// see start).
    pub created_at: i64,
    /// The text shown (empty: the transcription failed or is pending).
    pub text: String,
    /// What Copy puts on the clipboard.
    pub copy_text: String,
    pub variant: ShownVariant,
    /// The AI cleanup changed the text, so Undo / Redo AI edit applies.
    pub ai_edited: bool,
    /// `dictation`, `file`, `practice`, `command` or `transform`.
    pub source: String,
    /// The file a file transcription came from.
    pub source_name: Option<String>,
    pub app_name: Option<String>,
    /// Handy's WAV name (for its `get_audio_file_path`).
    pub file_name: String,
    pub audio: AudioState,
    pub flagged: bool,
    /// The cleanup level the text was made at, when the LLM step ran.
    pub cleanup_level: Option<String>,
    /// A take recorded without a usable speech model (S-07c).
    pub queue_status: Option<crate::fork::models::queue::QueueStatus>,
}

/// A page of rows and the cursor for the next one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct HistoryPage {
    pub rows: Vec<HistoryRow>,
    /// Pass back to read on; `None` at the end.
    pub next: Option<i64>,
}

/// What `entry_meta` adds to a Handy row.
#[derive(Debug, Default)]
struct Meta {
    created_at: Option<i64>,
    final_text: Option<String>,
    inserted_text: Option<String>,
    variant: Option<String>,
    source: Option<String>,
    app_name: Option<String>,
    audio_deleted_at: Option<i64>,
    flagged: bool,
    deleted: bool,
    cleanup_level: Option<String>,
    queue_status: Option<String>,
}

fn read_meta(conn: &Connection, id: i64) -> Result<Option<Meta>, DbError> {
    Ok(conn
        .prepare_cached(
            "SELECT created_at, final_text, inserted_text, shown_variant, source, app_name,
                    audio_deleted_at, flagged, deleted_at IS NOT NULL, cleanup_level,
                    (SELECT status FROM queued_takes WHERE entry_id = entry_meta.entry_id
                     ORDER BY id DESC LIMIT 1)
             FROM entry_meta WHERE entry_id = ?1",
        )?
        .query_row([id], |r| {
            Ok(Meta {
                created_at: r.get(0)?,
                final_text: r.get(1)?,
                inserted_text: r.get(2)?,
                variant: r.get(3)?,
                source: r.get(4)?,
                app_name: r.get(5)?,
                audio_deleted_at: r.get(6)?,
                flagged: r.get::<_, i64>(7)? == 1,
                deleted: r.get(8)?,
                cleanup_level: r.get(9)?,
                queue_status: r.get(10)?,
            })
        })
        .optional()?)
}

/// The row for `entry`, or `None` when it is soft-deleted. `audio_on_disk`
/// says whether its WAV exists.
fn row(entry: &HistoryEntry, meta: Option<Meta>, audio_on_disk: bool) -> Option<HistoryRow> {
    let meta = meta.unwrap_or_default();
    if meta.deleted {
        return None;
    }
    let raw = entry.transcription_text.clone();
    let final_text = meta
        .final_text
        .clone()
        .unwrap_or_else(|| super::entry_meta::final_text_of(entry).to_owned());
    let ai_edited = entry
        .post_processed_text
        .as_deref()
        .is_some_and(|p| p != entry.transcription_text)
        && !raw.trim().is_empty();
    let variant = if ai_edited && meta.variant.as_deref() == Some("raw") {
        ShownVariant::Raw
    } else {
        ShownVariant::Final
    };
    let (text, copy_text) = match variant {
        ShownVariant::Raw => (raw.clone(), raw),
        // Audit OT2: what was inserted (a snippet expanded) is what the row
        // shows and copies; `final_text` keeps the spoken words (stats,
        // retries, auto-learn).
        ShownVariant::Final => {
            let inserted = meta
                .inserted_text
                .clone()
                .filter(|t| !t.is_empty())
                .unwrap_or(final_text);
            (inserted.clone(), inserted)
        }
    };
    let created_at = meta
        .created_at
        .unwrap_or_else(|| entry.timestamp.saturating_mul(1000));
    let audio = match meta.audio_deleted_at {
        None if audio_on_disk => AudioState::Available,
        None => AudioState::Missing,
        Some(at) if at > created_at => AudioState::Expired,
        Some(_) => AudioState::Missing,
    };
    let source = meta.source.unwrap_or_else(|| {
        if entry.source_name.is_some() {
            "file".to_owned()
        } else {
            "dictation".to_owned()
        }
    });
    Some(HistoryRow {
        id: entry.id,
        created_at,
        text,
        copy_text,
        variant,
        ai_edited,
        source,
        source_name: entry.source_name.clone(),
        app_name: meta.app_name,
        file_name: entry.file_name.clone(),
        audio,
        flagged: meta.flagged,
        cleanup_level: meta.cleanup_level,
        queue_status: meta
            .queue_status
            .as_deref()
            .and_then(crate::fork::models::queue::QueueStatus::parse),
    })
}

fn rows_for(
    db: &ForkDb,
    handy: &dyn HandyHistory,
    entries: &[HistoryEntry],
) -> Result<Vec<HistoryRow>, DbError> {
    let metas = db.with_conn(|c| {
        entries
            .iter()
            .map(|e| read_meta(c, e.id))
            .collect::<Result<Vec<_>, _>>()
    })?;
    Ok(entries
        .iter()
        .zip(metas)
        .filter_map(|(e, m)| {
            let on_disk = handy.audio_path(&e.file_name).is_file();
            row(e, m, on_disk)
        })
        .collect())
}

/// Up to `limit` visible rows older than row `cursor`, newest first.
pub fn page(
    db: &ForkDb,
    handy: &dyn HandyHistory,
    cursor: Option<i64>,
    limit: usize,
) -> Result<HistoryPage, String> {
    let limit = limit.clamp(1, PAGE_ROWS);
    let mut rows = Vec::with_capacity(limit);
    let mut cursor = cursor;
    loop {
        let (entries, more) = handy.page(cursor, limit - rows.len())?;
        let last = entries.last().map(|e| e.id);
        rows.extend(rows_for(db, handy, &entries).map_err(|e| e.to_string())?);
        cursor = last;
        if !more || last.is_none() {
            return Ok(HistoryPage { rows, next: None });
        }
        if rows.len() >= limit {
            return Ok(HistoryPage { rows, next: cursor });
        }
    }
}

/// Rows whose text contains `query`, newest first: `limit` of them from
/// `offset` on (S-09's search index; `next` is the next offset).
pub fn find(
    db: &ForkDb,
    handy: &dyn HandyHistory,
    query: &str,
    offset: u32,
    limit: u32,
) -> Result<HistoryPage, String> {
    let limit = limit.clamp(1, PAGE_ROWS as u32);
    let want = offset.saturating_add(limit).saturating_add(1);
    let ids = db
        .with_conn(|c| fts::search_history(c, query, want))
        .map_err(|e| e.to_string())?;
    let more = ids.len() > (offset + limit) as usize;
    let mut entries = Vec::new();
    for &id in ids.iter().skip(offset as usize).take(limit as usize) {
        if let Some(e) = handy.entry(id)? {
            entries.push(e);
        }
    }
    let rows = rows_for(db, handy, &entries).map_err(|e| e.to_string())?;
    Ok(HistoryPage {
        rows,
        next: more.then_some(i64::from(offset + limit)),
    })
}

/// The rows for these ids (live updates of single rows; missing and
/// soft-deleted ids are left out).
pub fn rows(db: &ForkDb, handy: &dyn HandyHistory, ids: &[i64]) -> Result<Vec<HistoryRow>, String> {
    let mut entries = Vec::new();
    for &id in ids {
        if let Some(e) = handy.entry(id)? {
            entries.push(e);
        }
    }
    rows_for(db, handy, &entries).map_err(|e| e.to_string())
}

/// Shows `variant` for the row (Undo / Redo AI edit). Never re-pastes
/// (DEC-19) and never changes the stats. Returns whether a row changed.
pub fn set_variant(conn: &Connection, id: i64, variant: ShownVariant) -> Result<bool, DbError> {
    Ok(conn.execute(
        "UPDATE entry_meta SET shown_variant = ?2 WHERE entry_id = ?1",
        params![id, variant.as_str()],
    )? > 0)
}

/// Marks a row flagged and records the correction the user made from it
/// (`corrections` source `flag`, which Insights' "most corrected word"
/// counts, S-25).
pub fn record_flag(
    conn: &mut Connection,
    id: i64,
    from: &str,
    to: &str,
    now_ms: i64,
) -> Result<(), DbError> {
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE entry_meta SET flagged = 1 WHERE entry_id = ?1",
        [id],
    )?;
    tx.execute(
        "INSERT INTO corrections (entry_id, from_text, to_text, source, created_at)
         VALUES (?1, ?2, ?3, 'flag', ?4)",
        params![id, from, to, now_ms],
    )?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
#[path = "history_list_tests.rs"]
mod tests;
