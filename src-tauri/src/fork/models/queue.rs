// SPDX-License-Identifier: GPL-3.0-or-later
//! Durable takes recorded before a speech model was usable (S-07c, W-22).
//!
//! A row is written when recording stops, before Handy finishes saving its
//! WAV and History entry. Only a row bound to that saved entry may enter
//! Handy's ordered transcription-job queue. At launch, interrupted
//! `transcribing` rows become `waiting` again and surviving WAVs are matched
//! back to History. Failed jobs stay bound to their History row for Retry.

use std::collections::BTreeSet;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::fork::db::DbError;
use crate::managers::history::HistoryEntry;

/// Where a queued take is (`queued_takes.status`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum QueueStatus {
    Waiting,
    Transcribing,
    Ready,
    Failed,
}

impl QueueStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            QueueStatus::Waiting => "waiting",
            QueueStatus::Transcribing => "transcribing",
            QueueStatus::Ready => "ready",
            QueueStatus::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "waiting" => Some(QueueStatus::Waiting),
            "transcribing" => Some(QueueStatus::Transcribing),
            "ready" => Some(QueueStatus::Ready),
            "failed" => Some(QueueStatus::Failed),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Take {
    pub row_id: i64,
    pub file_name: String,
    pub created_at: i64,
    pub entry_id: Option<i64>,
    pub status: QueueStatus,
}

fn read_take(row: &rusqlite::Row<'_>) -> rusqlite::Result<Take> {
    let status: String = row.get(4)?;
    Ok(Take {
        row_id: row.get(0)?,
        file_name: row.get(1)?,
        created_at: row.get(2)?,
        entry_id: row.get(3)?,
        status: QueueStatus::parse(&status).unwrap_or(QueueStatus::Failed),
    })
}

const TAKE_COLUMNS: &str = "id, wav_path, created_at, entry_id, status";

/// Records a stopped take once, in recording order.
pub fn enqueue(conn: &Connection, file_name: &str, created_ms: i64) -> Result<i64, DbError> {
    conn.execute(
        "INSERT INTO queued_takes (wav_path, created_at, status)
         SELECT ?1, ?2, 'waiting'
         WHERE NOT EXISTS (SELECT 1 FROM queued_takes WHERE wav_path = ?1)",
        params![file_name, created_ms],
    )?;
    Ok(conn.query_row(
        "SELECT id FROM queued_takes WHERE wav_path = ?1",
        [file_name],
        |r| r.get(0),
    )?)
}

/// Binds the History row Handy saved after the WAV. `None` when the take
/// was never queued (every ordinary take; Mac wave 2 finding 6).
pub fn bind_entry(
    conn: &Connection,
    file_name: &str,
    entry_id: i64,
) -> Result<Option<i64>, DbError> {
    conn.execute(
        "UPDATE queued_takes SET entry_id = ?2 WHERE wav_path = ?1",
        params![file_name, entry_id],
    )?;
    Ok(conn
        .query_row(
            "SELECT id FROM queued_takes WHERE wav_path = ?1",
            [file_name],
            |r| r.get(0),
        )
        .optional()?)
}

pub fn pending(conn: &Connection) -> Result<u32, DbError> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM queued_takes WHERE status IN ('waiting', 'transcribing')",
        [],
        |r| r.get(0),
    )?)
}

/// Recoverable rows from an earlier process, oldest first.
pub fn interrupted(conn: &Connection) -> Result<Vec<Take>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {TAKE_COLUMNS} FROM queued_takes
         WHERE status IN ('waiting', 'transcribing') ORDER BY created_at, id"
    ))?;
    let rows = stmt.query_map([], read_take)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// A job interrupted by quit is waiting again. Existing History rows are
/// rebound by WAV name; rows without one are returned for WAV recovery.
pub fn recover(conn: &Connection, entries: &[HistoryEntry]) -> Result<Vec<Take>, DbError> {
    conn.execute(
        "UPDATE queued_takes SET status = 'waiting' WHERE status = 'transcribing'",
        [],
    )?;
    let mut rows = interrupted(conn)?;
    for take in &mut rows {
        if take.entry_id.is_none() {
            if let Some(entry) = entries.iter().find(|e| e.file_name == take.file_name) {
                take.entry_id = Some(entry.id);
                bind_entry(conn, &take.file_name, entry.id)?;
            }
        }
    }
    Ok(rows)
}

/// Starts every saved waiting take in recording order. Audio and its History
/// row necessarily existed before this point.
pub fn start_ready(conn: &mut Connection) -> Result<Vec<Take>, DbError> {
    let tx = conn.transaction()?;
    let started = {
        let mut stmt = tx.prepare(&format!(
            "SELECT {TAKE_COLUMNS} FROM queued_takes
             WHERE status = 'waiting' AND entry_id IS NOT NULL ORDER BY created_at, id"
        ))?;
        let rows = stmt.query_map([], read_take)?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    for take in &started {
        tx.execute(
            "UPDATE queued_takes SET status = 'transcribing' WHERE id = ?1 AND status = 'waiting'",
            [take.row_id],
        )?;
    }
    tx.commit()?;
    Ok(started
        .into_iter()
        .map(|mut t| {
            t.status = QueueStatus::Transcribing;
            t
        })
        .collect())
}

/// Marks an automatic or explicit queued-take retry finished.
pub fn finish(conn: &Connection, entry_id: i64, ok: bool) -> Result<Option<Take>, DbError> {
    let status = if ok {
        QueueStatus::Ready
    } else {
        QueueStatus::Failed
    };
    let changed = conn.execute(
        "UPDATE queued_takes SET status = ?2 WHERE entry_id = ?1 AND status = 'transcribing'",
        params![entry_id, status.as_str()],
    )?;
    if changed == 0 {
        return Ok(None);
    }
    Ok(conn
        .query_row(
            &format!("SELECT {TAKE_COLUMNS} FROM queued_takes WHERE entry_id = ?1"),
            [entry_id],
            read_take,
        )
        .optional()?)
}

/// A failed take explicitly retried from History joins the job listener, but
/// not the automatic batch's notification.
pub fn retry(conn: &Connection, entry_id: i64) -> Result<bool, DbError> {
    Ok(conn.execute(
        "UPDATE queued_takes SET status = 'transcribing' WHERE entry_id = ?1 AND status = 'failed'",
        [entry_id],
    )? > 0)
}

pub fn on_entry_deleted(conn: &Connection, entry_id: i64) -> Result<(), DbError> {
    conn.execute("DELETE FROM queued_takes WHERE entry_id = ?1", [entry_id])?;
    Ok(())
}

/// One automatic drain produces at most one batch notification. Explicit
/// retries of failed rows are deliberately not in this set.
#[derive(Debug, Default)]
struct Batch {
    active: BTreeSet<i64>,
    succeeded: u32,
}

static BATCH: Mutex<Batch> = Mutex::new(Batch {
    active: BTreeSet::new(),
    succeeded: 0,
});

#[cfg(test)]
pub static TEST_BATCH_LOCK: Mutex<()> = Mutex::new(());

pub fn begin_batch(started: &[Take]) {
    let mut batch = BATCH.lock().unwrap_or_else(|e| e.into_inner());
    batch
        .active
        .extend(started.iter().filter_map(|t| t.entry_id));
}

pub fn reset_batch() {
    *BATCH.lock().unwrap_or_else(|e| e.into_inner()) = Batch::default();
}

/// Returns the successful count exactly once, when the automatic batch ends.
pub fn finish_batch(entry_id: i64, ok: bool) -> Option<u32> {
    let mut batch = BATCH.lock().unwrap_or_else(|e| e.into_inner());
    if !batch.active.remove(&entry_id) {
        return None;
    }
    if ok {
        batch.succeeded += 1;
    }
    if !batch.active.is_empty() {
        return None;
    }
    let count = std::mem::take(&mut batch.succeeded);
    (count > 0).then_some(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::db::handy::fake::entry;
    use crate::fork::db::ForkDb;

    #[test]
    fn offline_takes_survive_relaunch_and_finish_as_one_ordered_batch() {
        let _batch = TEST_BATCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let db = ForkDb::in_dir(dir.path());
        reset_batch();
        db.with_conn(|c| {
            for n in 1..=3 {
                let name = format!("wispr-bro-{n}.wav");
                enqueue(c, &name, 1_000 + n)?;
                enqueue(c, &name, 9_999)?;
            }
            assert_eq!(pending(c)?, 3);
            assert!(start_ready(c)?.is_empty(), "audio is saved first");
            let entries: Vec<_> = (1..=3)
                .map(|n| entry(n, n, &format!("wispr-bro-{n}.wav")))
                .collect();
            // The usual save hook binds each row. A later launch can recover
            // the same bindings from Handy even if that hook was interrupted.
            for e in &entries[..2] {
                bind_entry(c, &e.file_name, e.id)?;
            }
            let recovered = recover(c, &entries)?;
            assert_eq!(recovered.len(), 3);
            assert!(recovered.iter().all(|t| t.entry_id.is_some()));

            let started = start_ready(c)?;
            let ids: Vec<_> = started.iter().map(|t| t.entry_id.unwrap()).collect();
            assert_eq!(ids, [1, 2, 3]);
            begin_batch(&started);
            assert_eq!(finish_batch(1, true), None);
            assert_eq!(finish_batch(2, true), None);
            assert_eq!(finish_batch(3, true), Some(3));
            assert_eq!(finish_batch(3, true), None, "one batch notice only");
            for id in ids {
                assert_eq!(finish(c, id, true)?.unwrap().status, QueueStatus::Ready);
            }
            assert_eq!(pending(c)?, 0);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn interrupted_jobs_retry_and_failed_jobs_stay_for_explicit_retry() {
        let _batch = TEST_BATCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let db = ForkDb::in_dir(dir.path());
        reset_batch();
        db.with_conn(|c| {
            enqueue(c, "wispr-bro-1.wav", 1)?;
            bind_entry(c, "wispr-bro-1.wav", 10)?;
            begin_batch(&start_ready(c)?);
            recover(c, &[entry(10, 1, "wispr-bro-1.wav")])?;
            assert_eq!(interrupted(c)?[0].status, QueueStatus::Waiting);

            begin_batch(&start_ready(c)?);
            assert_eq!(finish_batch(10, false), None);
            assert_eq!(finish(c, 10, false)?.unwrap().status, QueueStatus::Failed);
            assert!(start_ready(c)?.is_empty(), "failure is not a tight loop");
            assert!(retry(c, 10)?);
            assert_eq!(finish(c, 10, true)?.unwrap().status, QueueStatus::Ready);
            assert_eq!(
                finish_batch(10, true),
                None,
                "explicit Retry is not a batch"
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn binding_a_take_that_was_never_queued_is_not_an_error() {
        // Mac wave 2 finding 6: every ordinary take logged
        // "could not bind a queued take: … no rows".
        let dir = tempfile::tempdir().unwrap();
        let db = ForkDb::in_dir(dir.path());
        db.with_conn(|c| {
            assert_eq!(bind_entry(c, "wispr-bro-ordinary.wav", 7)?, None);
            let id = enqueue(c, "wispr-bro-queued.wav", 1)?;
            assert_eq!(bind_entry(c, "wispr-bro-queued.wav", 8)?, Some(id));
            Ok(())
        })
        .unwrap();
    }
}
