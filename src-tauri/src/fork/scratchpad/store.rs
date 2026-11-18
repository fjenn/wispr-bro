// SPDX-License-Identifier: GPL-3.0-or-later
//! Scratchpad notes and their transactional trigram search index.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::fork::db::DbError;

pub const MAX_BODY_CHARS: usize = 200_000;
pub const RECENTS_LIMIT: usize = 100;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Note {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub pinned: bool,
    pub created_at: i64,
    pub updated_at: i64,
    /// When its title, body or pin last changed, here or on another device
    /// (sync). An editor passes it back to [`save_from`].
    pub version: i64,
}

fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Note> {
    Ok(Note {
        id: row.get(0)?,
        title: row.get(1)?,
        body: row.get(2)?,
        pinned: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        version: row.get(6)?,
    })
}

const COLUMNS: &str = "notes.id, notes.title, notes.body, notes.pinned, notes.created_at, \
     notes.updated_at, notes.changed_at";

pub fn title_for(body: &str) -> String {
    body.lines()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .unwrap_or_default()
        .chars()
        .take(80)
        .collect()
}

pub fn valid_body(body: &str) -> bool {
    body.chars().count() <= MAX_BODY_CHARS
}

fn get_in(conn: &Connection, id: i64) -> Result<Option<Note>, DbError> {
    conn.query_row(
        &format!("SELECT {COLUMNS} FROM notes WHERE id = ?1 AND deleted_at IS NULL"),
        [id],
        from_row,
    )
    .optional()
    .map_err(Into::into)
}

pub fn get(conn: &Connection, id: i64) -> Result<Option<Note>, DbError> {
    get_in(conn, id)
}

pub fn latest(conn: &Connection) -> Result<Option<Note>, DbError> {
    conn.query_row(
        &format!(
            "SELECT {COLUMNS} FROM notes WHERE deleted_at IS NULL
             ORDER BY updated_at DESC, id DESC LIMIT 1"
        ),
        [],
        from_row,
    )
    .optional()
    .map_err(Into::into)
}

pub fn create(conn: &mut Connection, now: i64) -> Result<Note, DbError> {
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO notes (created_at, updated_at) VALUES (?1, ?1)",
        [now],
    )?;
    let id = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO notes_fts (rowid, title, body) VALUES (?1, '', '')",
        [id],
    )?;
    tx.commit()?;
    get(conn, id)?.ok_or_else(|| DbError::Corrupt("new note is missing".into()))
}

/// Saves the complete body and its FTS row in one transaction. An unchanged
/// body is a no-op, so a debounce retry does not move a note within Recents.
pub fn save(conn: &mut Connection, id: i64, body: &str, now: i64) -> Result<Option<Note>, DbError> {
    save_from(conn, id, body, now, None)
}

/// [`save`] from an editor that opened the note at `base` (its
/// [`Note::version`]). When a sync changed the note since, the typed text
/// still wins (the user is writing it now), and the synced text is kept as
/// a note of its own, "<title> (from <device>)"; when a sync deleted it,
/// the typed text brings it back. Nothing typed is lost.
pub fn save_from(
    conn: &mut Connection,
    id: i64,
    body: &str,
    now: i64,
    base: Option<i64>,
) -> Result<Option<Note>, DbError> {
    let deleted: Option<(bool, String)> = conn
        .query_row(
            "SELECT deleted_at IS NOT NULL, body FROM notes WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let restore = match deleted {
        None => return Ok(None),
        Some((true, stored)) if base.is_some() && stored != body => true,
        Some((true, _)) => return Ok(None),
        Some((false, _)) => false,
    };
    let title = title_for(body);
    let tx = conn.transaction()?;
    if restore {
        tx.execute("UPDATE notes SET deleted_at = NULL WHERE id = ?1", [id])?;
    } else if let Some(base) = base {
        let me = crate::fork::sync::store::self_id(&tx)?;
        if let Some(synced) = crate::fork::sync::notes::by_rowid(&tx, id)? {
            if synced.changed_at != base && synced.changed_by != me && synced.body != body {
                crate::fork::sync::notes::conflict_copy(&tx, &synced.id, &synced)?;
            }
        }
    }
    let changed = tx.execute(
        "UPDATE notes SET title = ?2, body = ?3, updated_at = ?4
         WHERE id = ?1 AND deleted_at IS NULL AND (title != ?2 OR body != ?3)",
        params![id, title, body, now],
    )?;
    if changed > 0 || restore {
        tx.execute("DELETE FROM notes_fts WHERE rowid = ?1", [id])?;
        tx.execute(
            "INSERT INTO notes_fts (rowid, title, body) VALUES (?1, ?2, ?3)",
            params![id, title, body],
        )?;
    }
    tx.commit()?;
    get(conn, id)
}

pub fn set_pinned(conn: &Connection, id: i64, pinned: bool) -> Result<Option<Note>, DbError> {
    conn.execute(
        "UPDATE notes SET pinned = ?2 WHERE id = ?1 AND deleted_at IS NULL",
        params![id, pinned],
    )?;
    get(conn, id)
}

pub fn delete(conn: &mut Connection, id: i64, now: i64) -> Result<bool, DbError> {
    let tx = conn.transaction()?;
    let changed = tx.execute(
        "UPDATE notes SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
        params![id, now],
    )?;
    if changed > 0 {
        tx.execute("DELETE FROM notes_fts WHERE rowid = ?1", [id])?;
    }
    tx.commit()?;
    Ok(changed > 0)
}

fn fts_query(query: &str) -> String {
    format!("\"{}\"", query.replace('"', "\"\""))
}

fn like_query(query: &str) -> String {
    let escaped = query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

pub fn list(conn: &Connection, query: Option<&str>) -> Result<Vec<Note>, DbError> {
    let query = query.map(str::trim).filter(|query| !query.is_empty());
    let sql = match query {
        Some(query) if query.chars().count() >= 3 => format!(
            "SELECT {COLUMNS} FROM notes JOIN notes_fts ON notes_fts.rowid = notes.id
             WHERE notes.deleted_at IS NULL AND notes_fts MATCH ?1
             ORDER BY notes.pinned DESC, notes.updated_at DESC, notes.id DESC LIMIT {RECENTS_LIMIT}"
        ),
        Some(_) => format!(
            "SELECT {COLUMNS} FROM notes
             WHERE deleted_at IS NULL
               AND (title LIKE ?1 ESCAPE '\\' OR body LIKE ?1 ESCAPE '\\')
             ORDER BY pinned DESC, updated_at DESC, id DESC LIMIT {RECENTS_LIMIT}"
        ),
        None => format!(
            "SELECT {COLUMNS} FROM notes WHERE deleted_at IS NULL
             ORDER BY pinned DESC, updated_at DESC, id DESC LIMIT {RECENTS_LIMIT}"
        ),
    };
    let mut statement = conn.prepare(&sql)?;
    let notes = match query {
        Some(query) if query.chars().count() >= 3 => statement
            .query_map([fts_query(query)], from_row)?
            .collect::<Result<Vec<_>, _>>()?,
        Some(query) => statement
            .query_map([like_query(query)], from_row)?
            .collect::<Result<Vec<_>, _>>()?,
        None => statement
            .query_map([], from_row)?
            .collect::<Result<Vec<_>, _>>()?,
    };
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A migrated database (the notes' sync columns come from 0011).
    fn connection() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::fork::db::ForkDb::in_dir(dir.path());
        db.open_now().unwrap();
        db.close();
        let conn = crate::fork::db::open_connection(db.path()).unwrap();
        (dir, conn)
    }

    #[test]
    fn autosave_and_index_updates_are_atomic_and_reopen_after_a_crash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fork.db");
        let db = crate::fork::db::ForkDb::new(&path);
        let note = db.with_conn(|conn| create(conn, 1)).unwrap();
        db.with_conn(|conn| save(conn, note.id, "first durable words", 2))
            .unwrap();
        // Dropping the handle without a clean checkpoint models process death.
        drop(db);
        let reopened = crate::fork::db::ForkDb::new(&path);
        assert_eq!(
            reopened
                .with_conn(|conn| get(conn, note.id))
                .unwrap()
                .unwrap()
                .body,
            "first durable words"
        );
        assert_eq!(
            reopened
                .with_conn(|conn| list(conn, Some("durable")))
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn search_pin_delete_and_noop_save_follow_the_recents_contract() {
        let (_dir, mut conn) = connection();
        let first = create(&mut conn, 1).unwrap();
        let second = create(&mut conn, 2).unwrap();
        save(&mut conn, first.id, "alpha café", 3).unwrap();
        save(&mut conn, second.id, "beta launch", 4).unwrap();
        assert_eq!(list(&conn, Some("café")).unwrap()[0].id, first.id);
        assert_eq!(list(&conn, Some("bet")).unwrap()[0].id, second.id);
        assert!(list(&conn, Some("missing")).unwrap().is_empty());
        set_pinned(&conn, first.id, true).unwrap();
        assert_eq!(list(&conn, None).unwrap()[0].id, first.id);
        let unchanged = save(&mut conn, first.id, "alpha café", 99)
            .unwrap()
            .unwrap();
        assert_eq!(unchanged.updated_at, 3);
        assert!(delete(&mut conn, second.id, 5).unwrap());
        assert!(!delete(&mut conn, second.id, 6).unwrap());
        assert!(list(&conn, Some("beta")).unwrap().is_empty());
    }
}
