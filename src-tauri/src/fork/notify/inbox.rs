// SPDX-License-Identifier: GPL-3.0-or-later
//! The inbox: `fork.db` › `notifications` (R5 §4.3, migration 0003).
//!
//! Plain functions on a connection, so the tests run on a temp database.
//! The inbox keeps the newest [`MAX_ENTRIES`] rows; older ones are pruned on
//! insert. Entries with a `dedupe_key` exist at most once: a second insert
//! with the same key is a no-op (`ON CONFLICT DO NOTHING`). Pruning drops
//! old keys too; the producers that use keys (milestones, tips, crash files,
//! features) only ever offer values that are new by their own rules (a higher
//! milestone, a newer crash file, the current version), so a pruned key does
//! not come back.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::catalog::Notice;
use super::policy::{buttons, Args, NotificationAction, NotificationButton, NotificationKind};
use crate::fork::db::DbError;

/// Entries kept; the oldest beyond this are deleted on insert.
pub const MAX_ENTRIES: i64 = 200;

/// One inbox entry as the panel shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct NotificationItem {
    pub id: i64,
    pub kind: NotificationKind,
    pub title_key: String,
    pub body_key: Option<String>,
    pub args: Args,
    pub action: Option<NotificationAction>,
    pub buttons: Vec<NotificationButton>,
    /// Unix ms.
    pub created_at: i64,
    pub read: bool,
}

/// Which entries the panel lists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum NotificationFilter {
    #[default]
    All,
    Unread,
}

/// A page of the inbox plus the unread count (for the bell badge).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct NotificationsPage {
    pub items: Vec<NotificationItem>,
    pub unread: u32,
    pub total: u32,
}

/// Inserts `notice` at `now_ms`. Returns the new row's id, or `None` when an
/// entry with the same dedupe key exists.
pub fn insert(conn: &Connection, notice: &Notice, now_ms: i64) -> Result<Option<i64>, DbError> {
    // A string map and a plain enum always serialize.
    let args = (!notice.args.is_empty())
        .then(|| serde_json::to_string(&notice.args).ok())
        .flatten();
    let action = notice
        .action
        .as_ref()
        .and_then(|a| serde_json::to_string(a).ok());
    let changed = conn.execute(
        "INSERT INTO notifications (kind, title_key, body_key, args_json, action, created_at, dedupe_key)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT (dedupe_key) WHERE dedupe_key IS NOT NULL DO NOTHING",
        params![
            notice.kind.as_str(),
            notice.title_key,
            notice.body_key,
            args,
            action,
            now_ms,
            notice.dedupe_key
        ],
    )?;
    if changed == 0 {
        return Ok(None);
    }
    let id = conn.last_insert_rowid();
    conn.execute(
        "DELETE FROM notifications WHERE id NOT IN
           (SELECT id FROM notifications ORDER BY created_at DESC, id DESC LIMIT ?1)",
        [MAX_ENTRIES],
    )?;
    Ok(Some(id))
}

/// Whether an entry with `dedupe_key` exists.
pub fn has_key(conn: &Connection, dedupe_key: &str) -> Result<bool, DbError> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM notifications WHERE dedupe_key = ?1)",
        [dedupe_key],
        |r| r.get(0),
    )?)
}

/// `created_at` of the newest entry of `kind`.
pub fn last_created(conn: &Connection, kind: NotificationKind) -> Result<Option<i64>, DbError> {
    Ok(conn.query_row(
        "SELECT MAX(created_at) FROM notifications WHERE kind = ?1",
        [kind.as_str()],
        |r| r.get(0),
    )?)
}

fn row_to_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<Option<NotificationItem>> {
    let kind: String = row.get("kind")?;
    let Some(kind) = NotificationKind::parse(&kind) else {
        // A kind from a newer build: skip it rather than fail the list.
        return Ok(None);
    };
    let args: Option<String> = row.get("args_json")?;
    let args: Args = args
        .and_then(|a| serde_json::from_str(&a).ok())
        .unwrap_or_default();
    let action: Option<String> = row.get("action")?;
    let action = action.and_then(|a| serde_json::from_str(&a).ok());
    let read_at: Option<i64> = row.get("read_at")?;
    Ok(Some(NotificationItem {
        id: row.get("id")?,
        kind,
        title_key: row.get("title_key")?,
        body_key: row.get("body_key")?,
        buttons: buttons(kind, &args),
        args,
        action,
        created_at: row.get("created_at")?,
        read: read_at.is_some(),
    }))
}

/// One entry.
pub fn get(conn: &Connection, id: i64) -> Result<Option<NotificationItem>, DbError> {
    Ok(conn
        .query_row(
            "SELECT * FROM notifications WHERE id = ?1",
            [id],
            row_to_item,
        )
        .optional()?
        .flatten())
}

/// Newest first, at most `limit` entries.
pub fn list(
    conn: &Connection,
    filter: NotificationFilter,
    limit: u32,
) -> Result<NotificationsPage, DbError> {
    let sql = match filter {
        NotificationFilter::All => {
            "SELECT * FROM notifications ORDER BY created_at DESC, id DESC LIMIT ?1"
        }
        NotificationFilter::Unread => {
            "SELECT * FROM notifications WHERE read_at IS NULL
             ORDER BY created_at DESC, id DESC LIMIT ?1"
        }
    };
    let mut stmt = conn.prepare(sql)?;
    let items = stmt
        .query_map([limit], row_to_item)?
        .filter_map(|r| r.transpose())
        .collect::<Result<Vec<_>, _>>()?;
    let (unread, total): (u32, u32) = conn.query_row(
        "SELECT COALESCE(SUM(read_at IS NULL), 0), COUNT(*) FROM notifications",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok(NotificationsPage {
        items,
        unread,
        total,
    })
}

/// Unread entries.
pub fn unread_count(conn: &Connection) -> Result<u32, DbError> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM notifications WHERE read_at IS NULL",
        [],
        |r| r.get(0),
    )?)
}

/// Marks `ids` read at `now_ms` (already read ones keep their time).
pub fn mark_read(conn: &Connection, ids: &[i64], now_ms: i64) -> Result<usize, DbError> {
    let mut n = 0;
    let mut stmt =
        conn.prepare("UPDATE notifications SET read_at = ?2 WHERE id = ?1 AND read_at IS NULL")?;
    for id in ids {
        n += stmt.execute(params![id, now_ms])?;
    }
    Ok(n)
}

/// Marks every entry read.
pub fn mark_all_read(conn: &Connection, now_ms: i64) -> Result<usize, DbError> {
    Ok(conn.execute(
        "UPDATE notifications SET read_at = ?1 WHERE read_at IS NULL",
        [now_ms],
    )?)
}

/// Deletes every entry (S-27's "delete all" removes the whole file instead).
pub fn clear(conn: &Connection) -> Result<usize, DbError> {
    Ok(conn.execute("DELETE FROM notifications", [])?)
}
