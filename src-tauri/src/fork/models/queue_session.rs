// SPDX-License-Identifier: GPL-3.0-or-later
//! Takes recorded before any speech model was ready, within this session
//! (S-07a, DEC-01, W-22; S-07c makes the queue durable across relaunches).
//!
//! A take that stops while setup is still running is never transcribed with
//! nothing and never refused: Handy saves its WAV and a History row with no
//! text as usual (the `actions.rs` hook returns an empty transcription), and
//! the take is recorded here and in `fork.db` › `queued_takes` as `waiting`.
//! When a model is ready, every waiting take is transcribed again from its
//! recording by Handy's own job queue (`TranscriptionJobManager::retry`,
//! which also updates the History row), and the result goes to the bar's
//! card with Copy. Nothing is pasted late and the clipboard is touched only
//! by Copy (R5 §4.7).

use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};

use crate::fork::db::DbError;

/// Where a queued take is (`queued_takes.status`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Waiting,
    Transcribing,
    Ready,
    Failed,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Waiting => "waiting",
            Status::Transcribing => "transcribing",
            Status::Ready => "ready",
            Status::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Take {
    /// Handy's recording file name (also `queued_takes.wav_path`).
    pub file_name: String,
    /// `queued_takes.id`, once written.
    pub row_id: Option<i64>,
    /// Handy's History row, once saved.
    pub entry_id: Option<i64>,
    pub status: Status,
}

/// This session's queued takes, in recording order.
#[derive(Debug, Default)]
pub struct Session {
    takes: Vec<Take>,
}

impl Session {
    pub const fn new() -> Session {
        Session { takes: Vec::new() }
    }

    /// A take stopped while no model was ready.
    pub fn queue(&mut self, file_name: &str) {
        if self.takes.iter().any(|t| t.file_name == file_name) {
            return;
        }
        self.takes.push(Take {
            file_name: file_name.to_string(),
            row_id: None,
            entry_id: None,
            status: Status::Waiting,
        });
    }

    pub fn set_row(&mut self, file_name: &str, row_id: i64) {
        if let Some(t) = self.takes.iter_mut().find(|t| t.file_name == file_name) {
            t.row_id = Some(row_id);
        }
    }

    /// Handy saved the History row for `file_name`. Returns the take if it
    /// is one of ours.
    pub fn bind_entry(&mut self, file_name: &str, entry_id: i64) -> Option<Take> {
        let t = self.takes.iter_mut().find(|t| t.file_name == file_name)?;
        t.entry_id = Some(entry_id);
        Some(t.clone())
    }

    /// Waiting takes with a History row, in order; marks them transcribing.
    pub fn start_ready(&mut self) -> Vec<Take> {
        self.takes
            .iter_mut()
            .filter(|t| t.status == Status::Waiting && t.entry_id.is_some())
            .map(|t| {
                t.status = Status::Transcribing;
                t.clone()
            })
            .collect()
    }

    /// A job for `entry_id` ended. Returns the take if it is ours and was
    /// being transcribed.
    pub fn finish(&mut self, entry_id: i64, ok: bool) -> Option<Take> {
        let t = self
            .takes
            .iter_mut()
            .find(|t| t.entry_id == Some(entry_id) && t.status == Status::Transcribing)?;
        t.status = if ok { Status::Ready } else { Status::Failed };
        Some(t.clone())
    }

    /// Takes still waiting for a model or being transcribed.
    pub fn pending(&self) -> u32 {
        self.takes
            .iter()
            .filter(|t| matches!(t.status, Status::Waiting | Status::Transcribing))
            .count() as u32
    }

    pub fn takes(&self) -> &[Take] {
        &self.takes
    }
}

pub static SESSION: Mutex<Session> = Mutex::new(Session::new());

pub fn session() -> std::sync::MutexGuard<'static, Session> {
    SESSION.lock().unwrap_or_else(|e| e.into_inner())
}

// ---------------------------------------------------------------------------
// `fork.db` › `queued_takes`

pub fn insert_row(conn: &Connection, file_name: &str, created_ms: i64) -> Result<i64, DbError> {
    conn.execute(
        "INSERT INTO queued_takes (wav_path, created_at, status) VALUES (?1, ?2, 'waiting')",
        params![file_name, created_ms],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn set_entry(conn: &Connection, row_id: i64, entry_id: i64) -> Result<(), DbError> {
    conn.execute(
        "UPDATE queued_takes SET entry_id = ?2 WHERE id = ?1",
        params![row_id, entry_id],
    )?;
    Ok(())
}

pub fn set_status(conn: &Connection, row_id: i64, status: Status) -> Result<(), DbError> {
    conn.execute(
        "UPDATE queued_takes SET status = ?2 WHERE id = ?1",
        params![row_id, status.as_str()],
    )?;
    Ok(())
}

/// Rows a previous session left `waiting` or `transcribing` (S-07c takes
/// them over; S-07a only reports them).
pub fn leftover_count(conn: &Connection) -> Result<u32, DbError> {
    let n: Option<u32> = conn
        .query_row(
            "SELECT COUNT(*) FROM queued_takes WHERE status IN ('waiting', 'transcribing')",
            [],
            |r| r.get(0),
        )
        .optional()?;
    Ok(n.unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_take_waits_binds_its_row_and_is_transcribed_once() {
        let mut s = Session::new();
        s.queue("wispr-bro-1.wav");
        s.queue("wispr-bro-1.wav"); // twice is once
        s.queue("wispr-bro-2.wav");
        assert_eq!(s.pending(), 2);
        // Nothing is started before Handy saved the row.
        assert!(s.start_ready().is_empty());
        assert_eq!(s.bind_entry("other.wav", 9), None);
        assert_eq!(
            s.bind_entry("wispr-bro-1.wav", 11).unwrap().entry_id,
            Some(11)
        );
        let started = s.start_ready();
        assert_eq!(started.len(), 1);
        assert_eq!(started[0].file_name, "wispr-bro-1.wav");
        // Started once only.
        assert!(s.start_ready().is_empty());
        s.bind_entry("wispr-bro-2.wav", 12);
        assert_eq!(s.start_ready()[0].entry_id, Some(12));
        assert_eq!(s.pending(), 2);
        // A job for an entry that is not ours, or ends twice, is ignored.
        assert_eq!(s.finish(99, true), None);
        assert_eq!(s.finish(11, true).unwrap().status, Status::Ready);
        assert_eq!(s.finish(11, true), None);
        assert_eq!(s.finish(12, false).unwrap().status, Status::Failed);
        assert_eq!(s.pending(), 0);
    }

    #[test]
    fn rows_follow_the_take() {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::fork::db::ForkDb::in_dir(dir.path());
        db.with_conn(|conn| {
            let id = insert_row(conn, "wispr-bro-5.wav", 1_000)?;
            assert_eq!(leftover_count(conn)?, 1);
            set_entry(conn, id, 42)?;
            set_status(conn, id, Status::Transcribing)?;
            assert_eq!(leftover_count(conn)?, 1);
            set_status(conn, id, Status::Ready)?;
            assert_eq!(leftover_count(conn)?, 0);
            let (wav, entry, status): (String, i64, String) = conn.query_row(
                "SELECT wav_path, entry_id, status FROM queued_takes WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            assert_eq!(
                (wav.as_str(), entry, status.as_str()),
                ("wispr-bro-5.wav", 42, "ready")
            );
            Ok(())
        })
        .unwrap();
    }
}
