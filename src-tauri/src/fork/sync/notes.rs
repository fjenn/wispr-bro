// SPDX-License-Identifier: GPL-3.0-or-later
//! Scratchpad notes in a sync: what this device sends ([`collect`]) and how
//! the other device's notes merge here ([`merge`]).
//!
//! A note is found by its sync id (migration 0011). Its version is
//! `(changed_at, changed_by, base_at)`: when and on which device its title,
//! body or pin last changed, and the other device's version it was edited
//! from. One version *contains* another when it was made from it:
//!
//! - the same device's versions follow each other (a later one contains an
//!   earlier one: that device's database only ever moves forward);
//! - a version edited from the other device's version `base_at` contains
//!   that device's versions up to `base_at`.
//!
//! Merging a note (whole note, never a text merge):
//!
//! | here \ there        | rule                                                     |
//! | ------------------- | -------------------------------------------------------- |
//! | missing             | taken, unless a deletion here contains it                |
//! | contains theirs     | kept (theirs is stale or an echo)                        |
//! | contained in theirs | replaced by theirs                                       |
//! | neither (both edited since they last agreed) | the newer stays as the note; the other is saved as a new note "<title> (from <device>)" |
//!
//! A deletion removes the note there only when the deleted version contains
//! the one there; a note edited there in a way the deleting device never
//! saw stays (and comes back to it). The conflict copy's id comes from the
//! note's id and the losing version, so both devices make the same copy and
//! a repeated merge makes none. No text is ever lost.

use rusqlite::{params, Connection, OptionalExtension};

use super::{NoteItem, NoteTombstone, Payload};
use crate::fork::db::DbError;
use crate::fork::scratchpad::store::MAX_BODY_CHARS;

use super::merge::SyncCounts;

/// Largest total of note text one state carries; the most recently changed
/// notes go first (a session message holds at most 32 MB).
pub const MAX_STATE_BYTES: usize = 16 * 1024 * 1024;
/// Longest title taken from another device.
const MAX_TITLE_CHARS: usize = 200;

/// A note's version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub at: i64,
    pub by: String,
    pub base: i64,
}

impl Version {
    /// Whether this version was made from `older` (or is it).
    pub fn contains(&self, older: &Version) -> bool {
        if older.by == self.by {
            older.at <= self.at
        } else {
            older.at <= self.base
        }
    }
}

impl NoteItem {
    pub fn version(&self) -> Version {
        Version {
            at: self.changed_at,
            by: self.changed_by.clone(),
            base: self.base_at,
        }
    }

    fn content(&self) -> (&str, &str, bool) {
        (&self.title, &self.body, self.pinned)
    }
}

impl NoteTombstone {
    fn version(&self) -> Version {
        Version {
            at: self.changed_at,
            by: self.changed_by.clone(),
            base: self.base_at,
        }
    }
}

/// A sync id as this app makes them (a lowercase UUID).
pub fn valid_id(id: &str) -> bool {
    id.len() == 36
        && id.chars().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_digit() || ('a'..='f').contains(&c),
        })
}

/// A UUID-shaped id made from `seed` (the same seed, the same id).
fn derived_id(seed: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, seed.as_bytes());
    let mut b = [0u8; 16];
    b.copy_from_slice(&digest.as_ref()[..16]);
    b[6] = (b[6] & 0x0f) | 0x50;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

const ITEM_COLUMNS: &str =
    "sync_id, title, body, pinned, created_at, updated_at, changed_at, changed_by, base_at";

fn item_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<NoteItem> {
    Ok(NoteItem {
        id: r.get(0)?,
        title: r.get(1)?,
        body: r.get(2)?,
        pinned: r.get(3)?,
        created_at: r.get(4)?,
        updated_at: r.get(5)?,
        changed_at: r.get(6)?,
        changed_by: r.get(7)?,
        base_at: r.get(8)?,
    })
}

/// A note here by sync id: its row id, whether it is deleted, and the note.
fn local(conn: &Connection, id: &str) -> Result<Option<(i64, bool, NoteItem)>, DbError> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {ITEM_COLUMNS}, id, deleted_at IS NOT NULL FROM notes WHERE sync_id = ?1"
            ),
            [id],
            |r| Ok((r.get(9)?, r.get(10)?, item_from_row(r)?)),
        )
        .optional()?)
}

/// The live note with row id `rowid`, as a sync carries it.
pub fn by_rowid(conn: &Connection, rowid: i64) -> Result<Option<NoteItem>, DbError> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {ITEM_COLUMNS} FROM notes
                 WHERE id = ?1 AND deleted_at IS NULL AND sync_id IS NOT NULL"
            ),
            [rowid],
            item_from_row,
        )
        .optional()?)
}

fn tombstone(conn: &Connection, id: &str) -> Result<Option<NoteTombstone>, DbError> {
    Ok(conn
        .query_row(
            "SELECT sync_id, deleted_at, changed_at, changed_by, base_at
             FROM sync_note_tombstones WHERE sync_id = ?1",
            [id],
            |r| {
                Ok(NoteTombstone {
                    id: r.get(0)?,
                    deleted_at: r.get(1)?,
                    changed_at: r.get(2)?,
                    changed_by: r.get(3)?,
                    base_at: r.get(4)?,
                })
            },
        )
        .optional()?)
}

fn put_tombstone(conn: &Connection, t: &NoteTombstone) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO sync_note_tombstones (sync_id, deleted_at, changed_at, changed_by, base_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (sync_id) DO UPDATE SET
             deleted_at = excluded.deleted_at, changed_at = excluded.changed_at,
             changed_by = excluded.changed_by, base_at = excluded.base_at
         WHERE excluded.changed_at > sync_note_tombstones.changed_at
            OR (excluded.changed_at = sync_note_tombstones.changed_at
                AND excluded.deleted_at > sync_note_tombstones.deleted_at)",
        params![t.id, t.deleted_at, t.changed_at, t.changed_by, t.base_at],
    )?;
    Ok(())
}

fn reindex(conn: &Connection, rowid: i64, title: &str, body: &str) -> Result<(), DbError> {
    conn.execute("DELETE FROM notes_fts WHERE rowid = ?1", [rowid])?;
    conn.execute(
        "INSERT INTO notes_fts (rowid, title, body) VALUES (?1, ?2, ?3)",
        params![rowid, title, body],
    )?;
    Ok(())
}

/// Writes `item` (content and version) over the row `rowid`, live.
fn write_over(conn: &Connection, rowid: i64, item: &NoteItem) -> Result<(), DbError> {
    conn.execute(
        "UPDATE notes SET title = ?2, body = ?3, pinned = ?4, created_at = ?5, updated_at = ?6,
                          changed_at = ?7, changed_by = ?8, base_at = ?9, deleted_at = NULL
         WHERE id = ?1",
        params![
            rowid,
            item.title,
            item.body,
            item.pinned,
            item.created_at,
            item.updated_at,
            item.changed_at,
            item.changed_by,
            item.base_at
        ],
    )?;
    reindex(conn, rowid, &item.title, &item.body)
}

fn insert(conn: &Connection, item: &NoteItem) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO notes (sync_id, title, body, pinned, created_at, updated_at,
                            changed_at, changed_by, base_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            item.id,
            item.title,
            item.body,
            item.pinned,
            item.created_at,
            item.updated_at,
            item.changed_at,
            item.changed_by,
            item.base_at
        ],
    )?;
    let rowid = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO notes_fts (rowid, title, body) VALUES (?1, ?2, ?3)",
        params![rowid, item.title, item.body],
    )?;
    Ok(())
}

/// The name a device goes by here (this one's own included), for a
/// conflict copy's title.
pub fn device_name(conn: &Connection, device_id: &str) -> Result<String, DbError> {
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT name, platform FROM sync_devices WHERE device_id = ?1",
            [device_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(match row {
        Some((name, _)) if !name.trim().is_empty() => name,
        Some((_, platform)) if platform == "ios" => "iPhone".into(),
        _ => "Mac".into(),
    })
}

/// Saves `loser` (a version of the note `original`) as a note of its own:
/// "<title> (from <device>)". `false`: that copy exists already, or was
/// deleted.
pub fn conflict_copy(conn: &Connection, original: &str, loser: &NoteItem) -> Result<bool, DbError> {
    let id = derived_id(&format!(
        "wb-note-conflict\u{1f}{original}\u{1f}{}\u{1f}{}",
        loser.changed_by, loser.changed_at
    ));
    if local(conn, &id)?.is_some() || tombstone(conn, &id)?.is_some() {
        return Ok(false);
    }
    let title = if loser.title.trim().is_empty() {
        crate::fork::scratchpad::store::title_for(&loser.body)
    } else {
        loser.title.clone()
    };
    // Not translated: both devices make the same copy, whatever their
    // languages.
    let from = device_name(conn, &loser.changed_by)?;
    let title: String = format!("{title} (from {from})")
        .chars()
        .take(MAX_TITLE_CHARS)
        .collect();
    insert(
        conn,
        &NoteItem {
            id,
            title,
            pinned: false,
            ..loser.clone()
        },
    )?;
    Ok(true)
}

/// What this device sends: its live notes (the most recently changed
/// first, up to [`MAX_STATE_BYTES`] of text) and its deleted notes.
pub fn collect(conn: &Connection) -> Result<(Vec<NoteItem>, Vec<NoteTombstone>), DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {ITEM_COLUMNS} FROM notes
         WHERE deleted_at IS NULL AND sync_id IS NOT NULL
         ORDER BY changed_at DESC, sync_id"
    ))?;
    let mut notes = Vec::new();
    let mut bytes = 0usize;
    for item in stmt.query_map([], item_from_row)? {
        let item = item?;
        bytes += item.body.len() + item.title.len();
        if bytes > MAX_STATE_BYTES {
            break;
        }
        notes.push(item);
    }
    // A stable order, so an unchanged set of notes hashes the same.
    notes.sort_by(|a, b| a.id.cmp(&b.id));
    let mut stmt = conn.prepare(
        "SELECT sync_id, deleted_at, changed_at, changed_by, base_at
         FROM sync_note_tombstones ORDER BY sync_id",
    )?;
    let tombstones = stmt
        .query_map([], |r| {
            Ok(NoteTombstone {
                id: r.get(0)?,
                deleted_at: r.get(1)?,
                changed_at: r.get(2)?,
                changed_by: r.get(3)?,
                base_at: r.get(4)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok((notes, tombstones))
}

/// An incoming note made safe, or `None` when it is not taken.
fn clean(item: &NoteItem) -> Option<NoteItem> {
    if !valid_id(&item.id)
        || item.body.chars().count() > MAX_BODY_CHARS
        || item.changed_by.is_empty()
        || item.changed_by.len() > 64
        || item.changed_at < 0
    {
        return None;
    }
    let title: String = item
        .title
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_TITLE_CHARS)
        .collect();
    Some(NoteItem {
        title,
        created_at: item.created_at.max(0),
        updated_at: item.updated_at.max(0),
        base_at: item.base_at.max(0),
        ..item.clone()
    })
}

/// Whether `a` wins over `b` when neither contains the other: the later
/// change, then (the same time) the larger text, so both devices pick the
/// same one.
fn newer(a: &NoteItem, b: &NoteItem) -> bool {
    (a.changed_at, a.content(), &a.changed_by) > (b.changed_at, b.content(), &b.changed_by)
}

/// Merges the other device's notes and deletions into this database (a
/// transaction of the caller).
pub fn merge(conn: &Connection, payload: &Payload) -> Result<SyncCounts, DbError> {
    let mut counts = SyncCounts::default();
    for t in payload.note_tombstones.iter().filter(|t| valid_id(&t.id)) {
        match local(conn, &t.id)? {
            Some((rowid, false, here)) if t.version().contains(&here.version()) => {
                conn.execute(
                    "UPDATE notes SET deleted_at = ?2 WHERE id = ?1",
                    params![rowid, t.deleted_at],
                )?;
                conn.execute("DELETE FROM notes_fts WHERE rowid = ?1", [rowid])?;
                // The trigger stamped this device's version; keep theirs.
                conn.execute(
                    "DELETE FROM sync_note_tombstones WHERE sync_id = ?1",
                    [&t.id],
                )?;
                put_tombstone(conn, t)?;
                counts.removed += 1;
            }
            // Edited here in a way the deleting device never saw: it stays.
            Some((_, false, _)) => {}
            _ => put_tombstone(conn, t)?,
        }
    }
    for item in &payload.notes {
        let Some(theirs) = clean(item) else {
            counts.skipped += 1;
            continue;
        };
        match local(conn, &theirs.id)? {
            None => {
                if tombstone(conn, &theirs.id)?
                    .is_some_and(|t| t.version().contains(&theirs.version()))
                {
                    continue;
                }
                insert(conn, &theirs)?;
                counts.added += 1;
            }
            Some((rowid, true, _)) => {
                // Deleted here: back only with an edit the deletion never saw.
                let gone = tombstone(conn, &theirs.id)?;
                if gone.is_none_or(|t| t.version().contains(&theirs.version())) {
                    continue;
                }
                write_over(conn, rowid, &theirs)?;
                counts.added += 1;
            }
            Some((rowid, false, here)) => {
                if here == theirs {
                    continue;
                }
                let (mine, other) = (here.version(), theirs.version());
                if mine == other {
                    // One version, two texts (cannot happen between two
                    // copies of this app): the same pick on both sides.
                    if newer(&theirs, &here) {
                        write_over(conn, rowid, &theirs)?;
                        counts.changed += 1;
                    }
                } else if mine.contains(&other) {
                    // Stale, or this device's own version coming back.
                } else if other.contains(&mine) {
                    write_over(conn, rowid, &theirs)?;
                    if here.content() != theirs.content() {
                        counts.changed += 1;
                    }
                } else if here.content() == theirs.content() {
                    // Both made the same change: one version, the newer.
                    if newer(&theirs, &here) {
                        write_over(conn, rowid, &theirs)?;
                    }
                } else if newer(&theirs, &here) {
                    if conflict_copy(conn, &here.id, &here)? {
                        counts.added += 1;
                    }
                    write_over(conn, rowid, &theirs)?;
                    counts.changed += 1;
                } else if conflict_copy(conn, &here.id, &theirs)? {
                    counts.added += 1;
                }
            }
        }
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_ids_are_valid_and_stable() {
        let a = derived_id("x");
        assert!(valid_id(&a), "{a}");
        assert_eq!(a, derived_id("x"));
        assert_ne!(a, derived_id("y"));
        assert!(!valid_id("not-an-id"));
        assert!(!valid_id("ABCDEF00-0000-4000-8000-000000000000"));
    }

    #[test]
    fn containment_follows_devices_and_bases() {
        let v = |at, by: &str, base| Version {
            at,
            by: by.into(),
            base,
        };
        // The same device: later contains earlier.
        assert!(v(5, "a", 0).contains(&v(3, "a", 0)));
        assert!(!v(3, "a", 0).contains(&v(5, "a", 0)));
        // Edited on b from a's version 5.
        assert!(v(9, "b", 5).contains(&v(5, "a", 0)));
        assert!(!v(9, "b", 5).contains(&v(6, "a", 0)));
    }
}
