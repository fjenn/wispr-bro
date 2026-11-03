// SPDX-License-Identifier: GPL-3.0-or-later
//! `dictionary_entries` (R5 §4.3, migration 0004): add, edit, delete and
//! list, with one entry per kind and duplicate key ([`super::norm_of`]).
//!
//! An entry that exists only as a suggestion (S-34) or a rejected
//! suggestion is turned into an active one when the user adds the same
//! word, rather than refused.

use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::{
    norm_of, CleanEntry, DictionaryCounts, DictionaryEntry, EntryKind, EntryProblem, EntrySource,
    EntryStatus, MAX_ENTRIES,
};
use crate::fork::db::DbError;

const COLUMNS: &str = "id, kind, term, from_text, to_text, source, status, created_at,
     COALESCE(updated_at, created_at), hit_count, last_hit_at";

/// Result of saving one entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SaveOutcome {
    /// Added or changed.
    Saved { entry: DictionaryEntry },
    /// The same entry is already there; nothing changed.
    Duplicate { existing: DictionaryEntry },
    /// An entry with the same word (or "from") but a different spelling or
    /// replacement is there; nothing changed. Saving over it is an edit of
    /// `existing`.
    Conflict { existing: DictionaryEntry },
    /// Not a valid entry; nothing changed.
    Invalid { problem: EntryProblem },
    /// The entry to edit no longer exists.
    Missing,
}

/// An entry row plus its raw status (`rejected` rows are never listed).
struct Stored {
    entry: DictionaryEntry,
    rejected: bool,
}

fn read_row(r: &Row<'_>) -> rusqlite::Result<Stored> {
    let kind: String = r.get(1)?;
    let kind = EntryKind::from_sql(&kind).unwrap_or(EntryKind::Word);
    let (phrase, replacement) = match kind {
        EntryKind::Word => (r.get::<_, Option<String>>(2)?.unwrap_or_default(), None),
        EntryKind::Rule => (
            r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            r.get::<_, Option<String>>(4)?,
        ),
    };
    let status: String = r.get(6)?;
    Ok(Stored {
        entry: DictionaryEntry {
            id: r.get(0)?,
            kind,
            phrase,
            replacement,
            source: EntrySource::from_sql(&r.get::<_, String>(5)?),
            status: if status == "suggested" {
                EntryStatus::Suggested
            } else {
                EntryStatus::Active
            },
            created_at: r.get(7)?,
            updated_at: r.get(8)?,
            hit_count: r.get(9)?,
            last_hit_at: r.get(10)?,
        },
        rejected: status == "rejected",
    })
}

/// Active and suggested entries, newest first.
pub fn list(conn: &Connection) -> Result<Vec<DictionaryEntry>, DbError> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM dictionary_entries
         WHERE status IN ('active', 'suggested')
         ORDER BY created_at DESC, id DESC"
    ))?;
    let rows = stmt
        .query_map([], read_row)?
        .map(|r| r.map(|s| s.entry))
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// One entry by id (any status but rejected).
pub fn get(conn: &Connection, id: i64) -> Result<Option<DictionaryEntry>, DbError> {
    let stored = conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM dictionary_entries WHERE id = ?1"),
            [id],
            read_row,
        )
        .optional()?;
    Ok(stored.filter(|s| !s.rejected).map(|s| s.entry))
}

fn find(conn: &Connection, kind: EntryKind, norm: &str) -> Result<Option<Stored>, DbError> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM dictionary_entries WHERE kind = ?1 AND norm = ?2"),
            params![kind.as_sql(), norm],
            read_row,
        )
        .optional()?)
}

/// Tab counts.
pub fn counts(conn: &Connection) -> Result<DictionaryCounts, DbError> {
    let (words, rules, suggested): (i64, i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(kind = 'word' AND status = 'active'), 0),
                COALESCE(SUM(kind = 'rule' AND status = 'active'), 0),
                COALESCE(SUM(status = 'suggested'), 0)
         FROM dictionary_entries",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let u = |n: i64| u32::try_from(n).unwrap_or(u32::MAX);
    Ok(DictionaryCounts {
        words: u(words),
        rules: u(rules),
        suggested: u(suggested),
    })
}

fn active_count(conn: &Connection) -> Result<usize, DbError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM dictionary_entries WHERE status = 'active'",
        [],
        |r| r.get(0),
    )?;
    Ok(usize::try_from(n).unwrap_or(usize::MAX))
}

/// The active words in the order they were added (Handy's list order).
pub fn active_words(conn: &Connection) -> Result<Vec<String>, DbError> {
    let mut stmt = conn.prepare_cached(
        "SELECT term FROM dictionary_entries
         WHERE kind = 'word' AND status = 'active' AND term IS NOT NULL
         ORDER BY created_at, id",
    )?;
    let words = stmt
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(words)
}

/// Whether `existing` already says what `entry` says (same spelling, same
/// replacement; the case of a "from" does not matter, matching ignores it).
fn same_as(existing: &DictionaryEntry, entry: &CleanEntry) -> bool {
    match entry.kind {
        EntryKind::Word => existing.phrase == entry.phrase,
        EntryKind::Rule => existing.replacement.as_deref() == entry.replacement.as_deref(),
    }
}

fn write_fields(
    conn: &Connection,
    id: i64,
    entry: &CleanEntry,
    source: Option<EntrySource>,
    now_ms: i64,
) -> Result<(), DbError> {
    let (term, from, to) = match entry.kind {
        EntryKind::Word => (Some(entry.phrase.as_str()), None, None),
        EntryKind::Rule => (
            None,
            Some(entry.phrase.as_str()),
            entry.replacement.as_deref(),
        ),
    };
    conn.execute(
        "UPDATE dictionary_entries
         SET kind = ?2, term = ?3, from_text = ?4, to_text = ?5, norm = ?6,
             status = 'active', updated_at = ?7, source = COALESCE(?8, source)
         WHERE id = ?1",
        params![
            id,
            entry.kind.as_sql(),
            term,
            from,
            to,
            entry.norm,
            now_ms,
            source.map(EntrySource::as_sql)
        ],
    )?;
    Ok(())
}

fn insert(
    conn: &Connection,
    entry: &CleanEntry,
    source: EntrySource,
    now_ms: i64,
) -> Result<i64, DbError> {
    conn.execute(
        "INSERT INTO dictionary_entries (kind, term, from_text, to_text, norm, source,
                                         status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'active', ?7, ?7)",
        params![
            entry.kind.as_sql(),
            (entry.kind == EntryKind::Word).then_some(entry.phrase.as_str()),
            (entry.kind == EntryKind::Rule).then_some(entry.phrase.as_str()),
            entry.replacement.as_deref(),
            entry.norm,
            source.as_sql(),
            now_ms
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

fn saved(conn: &Connection, id: i64) -> Result<SaveOutcome, DbError> {
    Ok(match get(conn, id)? {
        Some(entry) => SaveOutcome::Saved { entry },
        None => SaveOutcome::Missing,
    })
}

/// Adds an entry. A suggested or rejected entry with the same key becomes
/// this active one.
pub fn add(
    conn: &Connection,
    entry: &CleanEntry,
    source: EntrySource,
    now_ms: i64,
) -> Result<SaveOutcome, DbError> {
    match find(conn, entry.kind, &entry.norm)? {
        Some(s) if !s.rejected && s.entry.status == EntryStatus::Active => {
            Ok(if same_as(&s.entry, entry) {
                SaveOutcome::Duplicate { existing: s.entry }
            } else {
                SaveOutcome::Conflict { existing: s.entry }
            })
        }
        found => {
            if active_count(conn)? >= MAX_ENTRIES {
                return Ok(SaveOutcome::Invalid {
                    problem: EntryProblem::Full,
                });
            }
            let id = match found {
                Some(s) => {
                    write_fields(conn, s.entry.id, entry, Some(source), now_ms)?;
                    s.entry.id
                }
                None => insert(conn, entry, source, now_ms)?,
            };
            saved(conn, id)
        }
    }
}

/// Changes entry `id` to `entry` (its kind may change). Another active
/// entry with the same key is a conflict; a suggested or rejected one is
/// removed.
pub fn update(
    conn: &Connection,
    id: i64,
    entry: &CleanEntry,
    now_ms: i64,
) -> Result<SaveOutcome, DbError> {
    let Some(current) = get(conn, id)? else {
        return Ok(SaveOutcome::Missing);
    };
    match find(conn, entry.kind, &entry.norm)? {
        Some(other) if other.entry.id != id => {
            if !other.rejected && other.entry.status == EntryStatus::Active {
                return Ok(if same_as(&other.entry, entry) {
                    SaveOutcome::Duplicate {
                        existing: other.entry,
                    }
                } else {
                    SaveOutcome::Conflict {
                        existing: other.entry,
                    }
                });
            }
            conn.execute(
                "DELETE FROM dictionary_entries WHERE id = ?1",
                [other.entry.id],
            )?;
        }
        _ => {}
    }
    let unchanged = current.kind == entry.kind
        && current.phrase == entry.phrase
        && current.replacement.as_deref() == entry.replacement.as_deref()
        && current.status == EntryStatus::Active;
    if unchanged {
        return Ok(SaveOutcome::Saved { entry: current });
    }
    // A different replacement starts a new rule: its hits are not the old
    // one's.
    let rule_changed = current.kind != entry.kind
        || current.replacement.as_deref() != entry.replacement.as_deref();
    write_fields(conn, id, entry, None, now_ms)?;
    if rule_changed {
        conn.execute(
            "UPDATE dictionary_entries SET hit_count = 0, last_hit_at = NULL WHERE id = ?1",
            [id],
        )?;
    }
    saved(conn, id)
}

/// Deletes entries; returns what was deleted (for undo). A learned entry
/// (S-34) stays behind as a `rejected` row, so auto-learn never suggests
/// it again.
pub fn delete(conn: &mut Connection, ids: &[i64]) -> Result<Vec<DictionaryEntry>, DbError> {
    let tx = conn.transaction()?;
    let mut gone = Vec::with_capacity(ids.len());
    for &id in ids {
        if let Some(entry) = get(&tx, id)? {
            if entry.source == EntrySource::Learn {
                tx.execute(
                    "UPDATE dictionary_entries SET status = 'rejected' WHERE id = ?1",
                    [id],
                )?;
            } else {
                tx.execute("DELETE FROM dictionary_entries WHERE id = ?1", [id])?;
            }
            gone.push(entry);
        }
    }
    tx.commit()?;
    Ok(gone)
}

/// Puts deleted entries back as they were (undo), unless an entry with the
/// same key was added since. Returns how many came back.
pub fn restore(conn: &mut Connection, entries: &[DictionaryEntry]) -> Result<u32, DbError> {
    let tx = conn.transaction()?;
    let mut back = 0u32;
    for e in entries {
        let Ok(clean) = super::clean(&super::EntryInput {
            phrase: e.phrase.clone(),
            replacement: e.replacement.clone(),
        }) else {
            continue;
        };
        match find(&tx, clean.kind, &clean.norm)? {
            // The row a learned entry left behind when it was deleted.
            Some(s) if s.rejected && s.entry.id == e.id => {
                tx.execute("DELETE FROM dictionary_entries WHERE id = ?1", [e.id])?;
            }
            Some(_) => continue,
            None => {}
        }
        let status = match e.status {
            EntryStatus::Active => "active",
            EntryStatus::Suggested => "suggested",
        };
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM dictionary_entries WHERE id = ?1)",
            [e.id],
            |r| r.get(0),
        )?;
        tx.execute(
            "INSERT INTO dictionary_entries (id, kind, term, from_text, to_text, norm, source,
                                             status, created_at, updated_at, hit_count,
                                             last_hit_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                (!exists).then_some(e.id),
                clean.kind.as_sql(),
                (clean.kind == EntryKind::Word).then_some(clean.phrase.as_str()),
                (clean.kind == EntryKind::Rule).then_some(clean.phrase.as_str()),
                clean.replacement.as_deref(),
                clean.norm,
                e.source.as_sql(),
                status,
                e.created_at,
                e.updated_at,
                e.hit_count.max(0),
                e.last_hit_at
            ],
        )?;
        back += 1;
    }
    tx.commit()?;
    Ok(back)
}

/// Gives rows written without a key (by older code paths or tools) their
/// key; a row whose key is already taken is a duplicate and is removed.
pub fn ensure_norms(conn: &Connection) -> Result<(), DbError> {
    let rows: Vec<(i64, String, Option<String>, Option<String>)> = {
        let mut stmt = conn.prepare_cached(
            "SELECT id, kind, term, from_text FROM dictionary_entries
             WHERE norm IS NULL ORDER BY created_at, id",
        )?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<Result<_, _>>()?;
        rows
    };
    for (id, kind, term, from) in rows {
        let phrase = if kind == "word" { term } else { from }.unwrap_or_default();
        let norm = norm_of(&phrase);
        let taken = match EntryKind::from_sql(&kind) {
            Some(k) => norm.is_empty() || find(conn, k, &norm)?.is_some(),
            None => true,
        };
        if taken {
            conn.execute("DELETE FROM dictionary_entries WHERE id = ?1", [id])?;
        } else {
            conn.execute(
                "UPDATE dictionary_entries SET norm = ?2,
                        updated_at = COALESCE(updated_at, created_at)
                 WHERE id = ?1",
                params![id, norm],
            )?;
        }
    }
    Ok(())
}

/// The migration 0004 hook: fills `norm` and `updated_at` for older rows
/// (the key is a Rust case fold), removes a newer duplicate of an older
/// row, then makes the key unique.
pub fn migrate_v4(tx: &Transaction) -> rusqlite_migration::HookResult {
    let rows: Vec<(i64, String, Option<String>, Option<String>)> = {
        let mut stmt = tx.prepare(
            "SELECT id, kind, term, from_text FROM dictionary_entries ORDER BY created_at, id",
        )?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<Result<_, _>>()?;
        rows
    };
    let mut seen = std::collections::HashSet::new();
    for (id, kind, term, from) in rows {
        let phrase = if kind == "word" { term } else { from }.unwrap_or_default();
        let norm = norm_of(&phrase);
        if norm.is_empty() || !seen.insert((kind, norm.clone())) {
            tx.execute("DELETE FROM dictionary_entries WHERE id = ?1", [id])?;
            continue;
        }
        tx.execute(
            "UPDATE dictionary_entries SET norm = ?2, updated_at = COALESCE(updated_at, created_at)
             WHERE id = ?1",
            params![id, norm],
        )?;
    }
    tx.execute_batch(
        "CREATE UNIQUE INDEX dictionary_entries_kind_norm ON dictionary_entries (kind, norm);",
    )?;
    Ok(())
}
