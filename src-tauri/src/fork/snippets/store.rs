// SPDX-License-Identifier: GPL-3.0-or-later
//! `snippets` (R5 §4.3, migrations 0001 and 0006): add, edit, delete and
//! list, one snippet per duplicate key ([`super::norm_of`]), and the record
//! of a take's expansions.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::{
    CleanSnippet, SnippetCounts, SnippetEntry, SnippetOrigin, SnippetProblem, MAX_SNIPPETS,
};
use crate::fork::db::DbError;
use crate::fork::text::counts::{Stage, StageCount};
use crate::fork::text::snippets::Snippet;
use crate::fork::text::SnippetUse;

const COLUMNS: &str =
    r#"id, "trigger", body, source, created_at, updated_at, hit_count, last_hit_at"#;

/// Result of saving one snippet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SnippetSaveOutcome {
    /// Added or changed.
    Saved { snippet: SnippetEntry },
    /// The same trigger with the same text is already there; nothing
    /// changed.
    Duplicate { existing: SnippetEntry },
    /// The same trigger with another text is there; nothing changed.
    /// Saving over it is an edit of `existing`.
    Conflict { existing: SnippetEntry },
    /// Not a valid snippet; nothing changed.
    Invalid { problem: SnippetProblem },
    /// The snippet to edit no longer exists.
    Missing,
}

fn read_row(r: &Row<'_>) -> rusqlite::Result<SnippetEntry> {
    Ok(SnippetEntry {
        id: r.get(0)?,
        trigger: r.get(1)?,
        body: r.get(2)?,
        source: SnippetOrigin::from_sql(&r.get::<_, String>(3)?),
        created_at: r.get(4)?,
        updated_at: r.get(5)?,
        hit_count: r.get(6)?,
        last_hit_at: r.get(7)?,
    })
}

/// Every snippet, newest first.
pub fn list(conn: &Connection) -> Result<Vec<SnippetEntry>, DbError> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM snippets ORDER BY created_at DESC, id DESC"
    ))?;
    let rows = stmt.query_map([], read_row)?.collect::<Result<_, _>>()?;
    Ok(rows)
}

/// What the text pipeline matches on, oldest first.
pub fn for_matching(conn: &Connection) -> Result<Vec<Snippet>, DbError> {
    let mut stmt =
        conn.prepare_cached(r#"SELECT id, "trigger", body FROM snippets ORDER BY id"#)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Snippet {
                id: r.get(0)?,
                trigger: r.get(1)?,
                body: r.get(2)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// One snippet by id.
pub fn get(conn: &Connection, id: i64) -> Result<Option<SnippetEntry>, DbError> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM snippets WHERE id = ?1"),
            [id],
            read_row,
        )
        .optional()?)
}

fn find(conn: &Connection, norm: &str) -> Result<Option<SnippetEntry>, DbError> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM snippets WHERE trigger_norm = ?1"),
            [norm],
            read_row,
        )
        .optional()?)
}

/// The page's counts.
pub fn counts(conn: &Connection) -> Result<SnippetCounts, DbError> {
    let (n, uses): (i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(hit_count), 0) FROM snippets",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let u = |n: i64| u32::try_from(n.max(0)).unwrap_or(u32::MAX);
    Ok(SnippetCounts {
        snippets: u(n),
        uses: u(uses),
    })
}

fn total(conn: &Connection) -> Result<usize, DbError> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM snippets", [], |r| r.get(0))?;
    Ok(usize::try_from(n).unwrap_or(usize::MAX))
}

fn saved(conn: &Connection, id: i64) -> Result<SnippetSaveOutcome, DbError> {
    Ok(match get(conn, id)? {
        Some(snippet) => SnippetSaveOutcome::Saved { snippet },
        None => SnippetSaveOutcome::Missing,
    })
}

/// Adds a snippet.
pub fn add(
    conn: &Connection,
    s: &CleanSnippet,
    source: SnippetOrigin,
    now_ms: i64,
) -> Result<SnippetSaveOutcome, DbError> {
    if let Some(existing) = find(conn, &s.norm)? {
        return Ok(if existing.body == s.body {
            SnippetSaveOutcome::Duplicate { existing }
        } else {
            SnippetSaveOutcome::Conflict { existing }
        });
    }
    if total(conn)? >= MAX_SNIPPETS {
        return Ok(SnippetSaveOutcome::Invalid {
            problem: SnippetProblem::Full,
        });
    }
    conn.execute(
        r#"INSERT INTO snippets ("trigger", trigger_norm, body, source, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?5)"#,
        params![s.trigger, s.norm, s.body, source.as_sql(), now_ms],
    )?;
    saved(conn, conn.last_insert_rowid())
}

/// Changes snippet `id` to `s`. Another snippet with the same trigger is a
/// conflict. Its use count stays: the trigger is the same habit.
pub fn update(
    conn: &Connection,
    id: i64,
    s: &CleanSnippet,
    now_ms: i64,
) -> Result<SnippetSaveOutcome, DbError> {
    let Some(current) = get(conn, id)? else {
        return Ok(SnippetSaveOutcome::Missing);
    };
    if let Some(other) = find(conn, &s.norm)?.filter(|o| o.id != id) {
        return Ok(if other.body == s.body {
            SnippetSaveOutcome::Duplicate { existing: other }
        } else {
            SnippetSaveOutcome::Conflict { existing: other }
        });
    }
    if current.trigger == s.trigger && current.body == s.body {
        return Ok(SnippetSaveOutcome::Saved { snippet: current });
    }
    conn.execute(
        r#"UPDATE snippets SET "trigger" = ?2, trigger_norm = ?3, body = ?4, updated_at = ?5
           WHERE id = ?1"#,
        params![id, s.trigger, s.norm, s.body, now_ms],
    )?;
    saved(conn, id)
}

/// Deletes snippets; returns what was deleted (for undo).
pub fn delete(conn: &mut Connection, ids: &[i64]) -> Result<Vec<SnippetEntry>, DbError> {
    let tx = conn.transaction()?;
    let mut gone = Vec::with_capacity(ids.len());
    for &id in ids {
        if let Some(s) = get(&tx, id)? {
            tx.execute("DELETE FROM snippets WHERE id = ?1", [id])?;
            gone.push(s);
        }
    }
    tx.commit()?;
    Ok(gone)
}

/// Puts deleted snippets back as they were (undo), unless one with the same
/// trigger was added since. Returns how many came back.
pub fn restore(conn: &mut Connection, entries: &[SnippetEntry]) -> Result<u32, DbError> {
    let tx = conn.transaction()?;
    let mut back = 0u32;
    for e in entries {
        let Ok(clean) = super::clean(&super::SnippetInput {
            trigger: e.trigger.clone(),
            body: e.body.clone(),
        }) else {
            continue;
        };
        if find(&tx, &clean.norm)?.is_some() {
            continue;
        }
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM snippets WHERE id = ?1)",
            [e.id],
            |r| r.get(0),
        )?;
        tx.execute(
            r#"INSERT INTO snippets (id, "trigger", trigger_norm, body, source, created_at,
                                     updated_at, hit_count, last_hit_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
            params![
                (!exists).then_some(e.id),
                clean.trigger,
                clean.norm,
                clean.body,
                e.source.as_sql(),
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

/// Records one delivery's stage 11: each expansion counts on its snippet;
/// the history row (if known) gets the stage in `text_stages` and the
/// inserted text.
pub fn record_use(conn: &mut Connection, used: &SnippetUse, now_ms: i64) -> Result<(), DbError> {
    let tx = conn.transaction()?;
    {
        let mut bump = tx.prepare_cached(
            "UPDATE snippets SET hit_count = hit_count + 1, last_hit_at = ?2 WHERE id = ?1",
        )?;
        for hit in &used.hits {
            bump.execute(params![hit.snippet_id, now_ms])?;
        }
    }
    if let Some(entry_id) = used.entry_id {
        if !used.hits.is_empty() {
            let stage = serde_json::to_string(&StageCount {
                stage: Stage::Snippets,
                changes: u32::try_from(used.hits.len()).unwrap_or(u32::MAX),
            })
            .unwrap_or_default();
            tx.execute(
                "UPDATE entry_meta
                 SET text_stages = json_insert(COALESCE(text_stages, '[]'), '$[#]', json(?2))
                 WHERE entry_id = ?1",
                params![entry_id, stage],
            )?;
        }
        if let Some(inserted) = &used.inserted {
            tx.execute(
                "UPDATE entry_meta SET inserted_text = ?2 WHERE entry_id = ?1",
                params![entry_id, inserted],
            )?;
        }
    }
    tx.commit()?;
    Ok(())
}
