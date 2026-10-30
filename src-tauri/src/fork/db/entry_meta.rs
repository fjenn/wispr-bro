// SPDX-License-Identifier: GPL-3.0-or-later
//! One `entry_meta` row per Handy history row (R5 §4.3, S-09), written by
//! the hooks in Handy's `HistoryManager`:
//!
//! | Handy                        | Hook            | What the fork does                         |
//! | ---------------------------- | --------------- | ------------------------------------------ |
//! | `insert_entry` (every save)  | [`on_saved`]    | row + search document + stats              |
//! | `update_transcription`       | [`on_retried`]  | new text, stats by the delta, re-index     |
//! | `delete_entry`               | [`on_deleted`]  | drop row + search document, never stats    |
//!
//! plus [`on_paste_outcome`] from `fork::insert` (the take's `paste_outcome`).
//!
//! Every write is an idempotent upsert keyed by `entry_id`, in one
//! transaction with its stats change and its search document, so the three
//! never disagree. A hook never fails Handy's save: errors are logged.
//!
//! Deleting from the fork UI is a soft delete ([`soft_delete`]): the row is
//! hidden at once and purged through Handy's API after [`PURGE_DELAY`]
//! unless [`undo_delete`] runs first; a purge cut short by quitting is
//! finished by the launch reconcile.

use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension};
use tauri::{AppHandle, Manager, Runtime};

use super::stats::{self, Contribution};
use super::{fts, upsert_entry_meta, DbError, ForkDb};
use crate::fork::context::categories::{AppClass, UsageCategory};
use crate::fork::context::frontmost::FrontApp;
use crate::fork::context::take::{ledger, BoundTake, TakeContext, TakeSource};
use crate::fork::insert::deliver::PasteOutcome;
use crate::fork::stats::wordcount;
use crate::managers::history::HistoryEntry;

/// How long a soft-deleted row can be restored.
pub const PURGE_DELAY: Duration = Duration::from_secs(10);

/// Everything one write knows about an entry.
#[derive(Clone, Debug, PartialEq)]
pub struct EntryWrite {
    pub entry_id: i64,
    /// Unix ms at record start (or Handy's save time for rows the fork did
    /// not see start).
    pub created_at: i64,
    pub day: String,
    pub source: TakeSource,
    /// Verbatim ASR text (D-06); Handy's text when the verbatim was missed.
    pub asr_text: String,
    /// The text that was inserted (Handy's post-processed text, else its
    /// transcription).
    pub final_text: String,
    pub audio_ms: u64,
    pub audio_file: Option<String>,
    /// The WAV was never written (disk full) or is gone.
    pub audio_missing: bool,
    pub language: Option<String>,
    pub app: Option<FrontApp>,
    pub class: AppClass,
    /// Rebuilt by the launch reconcile from Handy's row.
    pub backfilled: bool,
}

/// The text Handy inserted for `entry`.
pub fn final_text_of(entry: &HistoryEntry) -> &str {
    entry
        .post_processed_text
        .as_deref()
        .unwrap_or(&entry.transcription_text)
}

impl EntryWrite {
    /// The write for a row Handy just saved, from the take the ledger bound
    /// to its file (if any) and the verbatim text (if known).
    pub fn for_saved(
        entry: &HistoryEntry,
        bound: Option<BoundTake>,
        verbatim: Option<String>,
        local_day: impl Fn(i64) -> String,
    ) -> EntryWrite {
        let saved_ms = entry.timestamp.saturating_mul(1000);
        let (ctx, audio_ms, audio_missing) = match bound {
            Some(b) => (b.context, b.audio_ms, b.audio_missing),
            None => (
                TakeContext {
                    started_at_ms: saved_ms,
                    day: local_day(saved_ms),
                    app: None,
                    class: AppClass::OTHER,
                    source: TakeSource::Dictation,
                },
                0,
                false,
            ),
        };
        let source = if entry.source_name.is_some() {
            TakeSource::File
        } else {
            ctx.source
        };
        EntryWrite {
            entry_id: entry.id,
            created_at: ctx.started_at_ms,
            day: ctx.day,
            source,
            asr_text: verbatim.unwrap_or_else(|| entry.transcription_text.clone()),
            final_text: final_text_of(entry).to_owned(),
            audio_ms,
            audio_file: Some(entry.file_name.clone()),
            audio_missing,
            language: entry.language.clone(),
            app: ctx.app,
            class: ctx.class,
            backfilled: false,
        }
    }
}

/// What is stored for an existing row and decides its stats.
struct Stored {
    counted: bool,
    day: Option<String>,
    usage_category: Option<String>,
    source: String,
    word_count: i64,
    audio_ms: i64,
    fix_count_dictionary: i64,
    fix_count_llm: i64,
    final_text: Option<String>,
}

fn stored(conn: &Connection, entry_id: i64) -> Result<Option<Stored>, DbError> {
    Ok(conn
        .query_row(
            "SELECT counted, day, usage_category, source, word_count, audio_ms,
                    fix_count_dictionary, fix_count_llm, final_text
             FROM entry_meta WHERE entry_id = ?1",
            [entry_id],
            |r| {
                Ok(Stored {
                    counted: r.get::<_, i64>(0)? == 1,
                    day: r.get(1)?,
                    usage_category: r.get(2)?,
                    source: r.get(3)?,
                    word_count: r.get(4)?,
                    audio_ms: r.get(5)?,
                    fix_count_dictionary: r.get(6)?,
                    fix_count_llm: r.get(7)?,
                    final_text: r.get(8)?,
                })
            },
        )
        .optional()?)
}

fn parse_category(s: Option<&str>) -> UsageCategory {
    match s {
        Some("documents") => UsageCategory::Documents,
        Some("ai_prompts") => UsageCategory::AiPrompts,
        Some("email") => UsageCategory::Email,
        Some("work") => UsageCategory::Work,
        Some("personal") => UsageCategory::Personal,
        _ => UsageCategory::Other,
    }
}

/// What a row with these values contributes to the stats, if anything.
fn contribution(
    source: TakeSource,
    day: Option<&str>,
    category: UsageCategory,
    final_text: &str,
    words: i64,
    audio_ms: i64,
    fixes: (i64, i64),
) -> Option<Contribution> {
    let day = day?;
    (source.counts_in_stats() && !final_text.trim().is_empty()).then(|| Contribution {
        day: day.to_owned(),
        usage_category: category,
        words,
        speech_ms: audio_ms,
        fixes_dictionary: fixes.0,
        fixes_llm: fixes.1,
    })
}

/// Writes a new row, or, if the row exists, updates its texts only (the
/// first write's time, day, app and speech time stay). `retry` marks a new
/// transcription of the same audio: the old final text becomes
/// `previous_text`. Returns whether the stats changed.
pub fn write(conn: &mut Connection, w: &EntryWrite, retry: bool) -> Result<bool, DbError> {
    let tx = conn.transaction()?;
    let existing = stored(&tx, w.entry_id)?;
    let words = i64::from(wordcount::word_count(&w.final_text));
    let chars = i64::from(wordcount::char_count(&w.final_text));

    let (before, after) = match &existing {
        None => {
            let audio_deleted_at = w.audio_missing.then_some(w.created_at);
            let audio_ms = i64::try_from(w.audio_ms).unwrap_or(i64::MAX);
            let after = contribution(
                w.source,
                Some(&w.day),
                w.class.usage_category,
                &w.final_text,
                words,
                audio_ms,
                (0, 0),
            );
            let counted = i64::from(after.is_some());
            let bundle = w.app.as_ref().and_then(|a| a.bundle_id.clone());
            let name = w.app.as_ref().and_then(|a| a.name.clone());
            upsert_entry_meta(
                &tx,
                w.entry_id,
                &[
                    ("created_at", &w.created_at),
                    ("day", &w.day),
                    ("source", &w.source.as_str()),
                    ("asr_text", &w.asr_text),
                    ("final_text", &w.final_text),
                    ("audio_ms", &audio_ms),
                    ("audio_file", &w.audio_file),
                    ("audio_deleted_at", &audio_deleted_at),
                    ("word_count", &words),
                    ("char_count", &chars),
                    ("language", &w.language),
                    ("app_bundle_id", &bundle),
                    ("app_name", &name),
                    ("style_group", &w.class.style_group.as_str()),
                    ("usage_category", &w.class.usage_category.as_str()),
                    ("backfilled", &i64::from(w.backfilled)),
                    ("counted", &counted),
                ],
            )?;
            (None, after)
        }
        Some(s) => {
            let source = TakeSource::parse(&s.source).unwrap_or(TakeSource::Dictation);
            let category = parse_category(s.usage_category.as_deref());
            let fixes = (s.fix_count_dictionary, s.fix_count_llm);
            let before = s.counted.then(|| Contribution {
                day: s.day.clone().unwrap_or_default(),
                usage_category: category,
                words: s.word_count,
                speech_ms: s.audio_ms,
                fixes_dictionary: fixes.0,
                fixes_llm: fixes.1,
            });
            let after = contribution(
                source,
                s.day.as_deref(),
                category,
                &w.final_text,
                words,
                s.audio_ms,
                fixes,
            );
            let previous = match (retry, &s.final_text) {
                (true, Some(old)) if *old != w.final_text => Some(old.clone()),
                _ => None,
            };
            let counted = i64::from(after.is_some());
            let mut fields: Vec<(&str, &dyn rusqlite::ToSql)> = vec![
                ("asr_text", &w.asr_text),
                ("final_text", &w.final_text),
                ("word_count", &words),
                ("char_count", &chars),
                ("language", &w.language),
                ("counted", &counted),
            ];
            if let Some(p) = &previous {
                fields.push(("previous_text", p));
            }
            upsert_entry_meta(&tx, w.entry_id, &fields)?;
            (before, after)
        }
    };
    if w.final_text.trim().is_empty() {
        fts::delete_document(&tx, &fts::HISTORY, w.entry_id)?;
    } else {
        fts::index_history_entry(&tx, w.entry_id, &w.final_text)?;
    }
    let changed = stats::apply(&tx, before.as_ref(), after.as_ref())?;
    tx.commit()?;
    Ok(changed)
}

/// Stores a take's `paste_outcome`.
pub fn set_paste_outcome(
    conn: &Connection,
    entry_id: i64,
    outcome: PasteOutcome,
) -> Result<(), DbError> {
    conn.execute(
        "UPDATE entry_meta SET paste_outcome = ?2 WHERE entry_id = ?1",
        params![entry_id, outcome.as_str()],
    )?;
    Ok(())
}

/// Hides rows at once (search and History skip them); they are purged
/// through Handy after [`PURGE_DELAY`]. Returns how many were hidden.
pub fn soft_delete(conn: &Connection, ids: &[i64], now_ms: i64) -> Result<usize, DbError> {
    let mut n = 0;
    for id in ids {
        n += conn.execute(
            "UPDATE entry_meta SET deleted_at = ?2 WHERE entry_id = ?1 AND deleted_at IS NULL",
            params![id, now_ms],
        )?;
    }
    Ok(n)
}

/// Brings soft-deleted rows back (before their purge). Returns how many.
pub fn undo_delete(conn: &Connection, ids: &[i64]) -> Result<usize, DbError> {
    let mut n = 0;
    for id in ids {
        n += conn.execute(
            "UPDATE entry_meta SET deleted_at = NULL WHERE entry_id = ?1",
            [id],
        )?;
    }
    Ok(n)
}

/// Soft-deleted rows whose undo window has passed by `now_ms`.
pub fn due_purges(conn: &Connection, now_ms: i64, delay: Duration) -> Result<Vec<i64>, DbError> {
    let cutoff = now_ms - i64::try_from(delay.as_millis()).unwrap_or(i64::MAX);
    let mut stmt = conn.prepare_cached(
        "SELECT entry_id FROM entry_meta WHERE deleted_at IS NOT NULL AND deleted_at <= ?1
         ORDER BY entry_id",
    )?;
    let ids = stmt
        .query_map([cutoff], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

/// Deletes each row through Handy's API, then its `entry_meta` (Handy's
/// delete hook does that too; doing it here as well keeps the purge whole
/// when the hook could not reach the database). Stats are untouched.
/// Returns the ids that were purged.
pub fn purge(
    db: &ForkDb,
    handy: &dyn super::handy::HandyHistory,
    ids: &[i64],
) -> Result<Vec<i64>, DbError> {
    let mut done = Vec::new();
    for &id in ids {
        match handy.delete_entry(id) {
            Ok(()) => {
                db.with_conn(|conn| super::delete_entry_meta(conn, id))?;
                done.push(id);
            }
            Err(e) => log::warn!("history: could not delete entry {id} through Handy: {e}"),
        }
    }
    Ok(done)
}

// ---------------------------------------------------------------------------
// Hooks (called from Handy's files; never fail Handy's flow)

fn with_db<R: Runtime, T>(
    app: &AppHandle<R>,
    what: &str,
    f: impl FnOnce(&mut Connection) -> Result<T, DbError>,
) -> Option<T> {
    let db = app.try_state::<ForkDb>()?;
    match db.with_conn(f) {
        Ok(v) => Some(v),
        Err(e) => {
            log::error!("history: {what} failed: {e}");
            None
        }
    }
}

fn stats_changed<R: Runtime>(app: &AppHandle<R>, day: &str) {
    let event = crate::fork::events::ForkStatsUpdated {
        day: day.to_owned(),
    };
    if let Err(e) = tauri_specta::Event::emit(&event, app) {
        log::debug!("history: could not emit fork-stats-updated: {e}");
    }
}

/// `HistoryManager::insert_entry` saved `entry` (a dictation, a failed take
/// or a queued file).
pub fn on_saved<R: Runtime>(app: &AppHandle<R>, entry: &HistoryEntry) {
    let (bound, verbatim) = {
        let mut l = ledger();
        (
            l.take_bound(&entry.file_name),
            l.take_verbatim(&entry.transcription_text),
        )
    };
    let w = EntryWrite::for_saved(entry, bound, verbatim, crate::fork::stats::local_day);
    if let Some(changed) = with_db(app, "save", |conn| write(conn, &w, false)) {
        log::info!(
            "history: entry {} saved ({}, {} in {}, {} words, {} ms)",
            w.entry_id,
            w.source.as_str(),
            w.class.usage_category.as_str(),
            w.app
                .as_ref()
                .and_then(|a| a.bundle_id.as_deref())
                .unwrap_or("?"),
            wordcount::word_count(&w.final_text),
            w.audio_ms
        );
        if changed {
            stats_changed(app, &w.day);
        }
    }
    if !w.final_text.trim().is_empty() && w.source == TakeSource::Dictation {
        ledger().await_outcome(w.entry_id, &w.final_text);
    }
}

/// `HistoryManager::update_transcription` stored a new transcription of
/// `entry` (a retry, or a queued file finishing).
pub fn on_retried<R: Runtime>(app: &AppHandle<R>, entry: &HistoryEntry) {
    let verbatim = ledger().take_verbatim(&entry.transcription_text);
    // For a row the fork never saw, this is its first write.
    let w = EntryWrite::for_saved(entry, None, verbatim, crate::fork::stats::local_day);
    let day = with_db(app, "retry", |conn| {
        let changed = write(conn, &w, true)?;
        let day: Option<String> = conn
            .query_row(
                "SELECT day FROM entry_meta WHERE entry_id = ?1",
                [w.entry_id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        Ok(changed.then_some(day).flatten())
    });
    log::info!("history: entry {} re-transcribed", w.entry_id);
    if let Some(Some(day)) = day {
        stats_changed(app, &day);
    }
}

/// `HistoryManager::delete_entry` removed row `id`. The stats keep it.
pub fn on_deleted<R: Runtime>(app: &AppHandle<R>, id: i64) {
    if with_db(app, "delete", |conn| super::delete_entry_meta(conn, id)).is_some() {
        log::info!("history: entry {id} deleted");
    }
}

/// `fork::insert` finished delivering `text` (S-04's `paste_outcome`; `None`
/// for a take cancelled before the paste).
pub fn on_paste_outcome<R: Runtime>(app: &AppHandle<R>, text: &str, outcome: Option<PasteOutcome>) {
    let Some(entry_id) = ledger().take_awaiting(text) else {
        log::debug!("history: paste outcome for a take with no history row");
        return;
    };
    let Some(outcome) = outcome else {
        return;
    };
    if with_db(app, "paste outcome", |conn| {
        set_paste_outcome(conn, entry_id, outcome)
    })
    .is_some()
    {
        log::info!(
            "history: entry {entry_id} paste_outcome {}",
            outcome.as_str()
        );
    }
}

#[cfg(test)]
#[path = "entry_meta_tests.rs"]
mod tests;
