// SPDX-License-Identifier: GPL-3.0-or-later
//! Retention (DEC-07, C-46): text forever by default, audio 14 days by
//! default; both set in fork settings (`text_retention_days`,
//! `audio_retention_days`). Runs at launch and every [`INTERVAL`], and when
//! either setting changes.
//!
//! - **Audio past its limit:** the WAV file is deleted and the row keeps its
//!   text (`audio_deleted_at`). Handy has no API for this; it is a plain file
//!   delete in Handy's recordings dir.
//! - **Text past its limit:** the whole row is deleted through Handy's
//!   `HistoryManager::delete_entry` (never SQL on `history.db`), whose hook
//!   drops `entry_meta`.
//! - **Never:** the stats tables, files the user transcribed (Handy keeps
//!   those until deleted, its own rule), rows the user saved (Handy's
//!   `saved` flag).
//!
//! Ages are measured from `entry_meta.created_at` (record start) against an
//! injected `now_ms`, so tests use a fake clock and the Mac test script uses
//! [`now_ms`]'s debug-build clock offset.

use std::path::Path;
use std::time::Duration;

use rusqlite::{params, Connection};

use super::handy::HandyHistory;
use super::{DbError, ForkDb};

/// How often the job runs while the app is open.
pub const INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/// Debug builds only: shift the retention clock by this many days (Mac test
/// script S-09.2 simulates the 14-day audio expiry with it).
pub const CLOCK_OFFSET_ENV: &str = "WB_DEBUG_RETENTION_CLOCK_OFFSET_DAYS";

/// The retention clock: wall time, plus the debug offset in debug builds.
pub fn now_ms() -> i64 {
    let now = chrono::Utc::now().timestamp_millis();
    #[cfg(debug_assertions)]
    if let Some(days) = std::env::var(CLOCK_OFFSET_ENV)
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
    {
        return now + days * DAY_MS;
    }
    now
}

/// The two limits, in days; `None` keeps forever, `Some(0)` keeps no audio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    pub text_days: Option<u32>,
    pub audio_days: Option<u32>,
}

impl Policy {
    pub fn from_settings(s: &crate::fork::settings::ForkSettings) -> Self {
        Self {
            text_days: s.text_retention_days,
            audio_days: s.audio_retention_days,
        }
    }
}

/// What one run would delete.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// (entry id, WAV file name)
    pub audio: Vec<(i64, String)>,
    /// Entry ids.
    pub rows: Vec<i64>,
}

fn cutoff(now_ms: i64, days: u32) -> i64 {
    now_ms - i64::from(days) * DAY_MS
}

/// Rows and WAVs past their limits as of `now_ms`. A row whose text expires
/// is listed under `rows` only (its WAV goes with it).
pub fn plan(conn: &Connection, now_ms: i64, policy: Policy) -> Result<Plan, DbError> {
    let mut out = Plan::default();
    if let Some(days) = policy.text_days {
        // `text_retention_days = 0` is refused by settings validation; be
        // safe anyway.
        let days = days.max(1);
        let mut stmt = conn.prepare_cached(
            "SELECT entry_id FROM entry_meta
             WHERE deleted_at IS NULL AND source <> 'file' AND created_at < ?1
             ORDER BY created_at",
        )?;
        out.rows = stmt
            .query_map([cutoff(now_ms, days)], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
    }
    if let Some(days) = policy.audio_days {
        let mut stmt = conn.prepare_cached(
            "SELECT entry_id, audio_file FROM entry_meta
             WHERE audio_deleted_at IS NULL AND audio_file IS NOT NULL
               AND source <> 'file' AND created_at < ?1
             ORDER BY created_at",
        )?;
        let audio: Vec<(i64, String)> = stmt
            .query_map([cutoff(now_ms, days)], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        out.audio = audio
            .into_iter()
            .filter(|(id, _)| !out.rows.contains(id))
            .collect();
    }
    Ok(out)
}

/// What one run did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub audio_deleted: usize,
    pub rows_deleted: usize,
    /// Saved by the user in Handy: kept.
    pub kept_saved: usize,
    pub failed: usize,
}

fn remove_wav(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Runs the plan as of `now_ms`.
pub fn run(
    db: &ForkDb,
    handy: &dyn HandyHistory,
    now_ms: i64,
    policy: Policy,
) -> Result<Report, DbError> {
    let plan = db.with_conn(|conn| plan(conn, now_ms, policy))?;
    let mut report = Report::default();
    let saved = |id: i64| -> Result<Option<bool>, String> { Ok(handy.entry(id)?.map(|e| e.saved)) };

    for (id, file) in &plan.audio {
        match saved(*id) {
            Ok(Some(true)) => {
                report.kept_saved += 1;
                continue;
            }
            Ok(_) => {}
            Err(e) => {
                log::warn!("retention: could not read entry {id}: {e}");
                report.failed += 1;
                continue;
            }
        }
        match remove_wav(&handy.audio_path(file)) {
            Ok(()) => {
                db.with_conn(|conn| {
                    conn.execute(
                        "UPDATE entry_meta SET audio_deleted_at = ?2 WHERE entry_id = ?1",
                        params![id, now_ms],
                    )?;
                    Ok(())
                })?;
                report.audio_deleted += 1;
            }
            Err(e) => {
                log::warn!("retention: could not delete the audio of entry {id}: {e}");
                report.failed += 1;
            }
        }
    }

    for id in &plan.rows {
        match saved(*id) {
            Ok(Some(true)) => {
                report.kept_saved += 1;
                continue;
            }
            Ok(_) => {}
            Err(e) => {
                log::warn!("retention: could not read entry {id}: {e}");
                report.failed += 1;
                continue;
            }
        }
        match handy.delete_entry(*id) {
            Ok(()) => {
                db.with_conn(|conn| super::delete_entry_meta(conn, *id))?;
                report.rows_deleted += 1;
            }
            Err(e) => {
                log::warn!("retention: could not delete entry {id}: {e}");
                report.failed += 1;
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::db::handy::fake::{entry, FakeHistory};
    use crate::fork::testutil::{Clock, FakeClock};

    const POLICY: Policy = Policy {
        text_days: None,
        audio_days: Some(14),
    };

    struct Setup {
        _dir: tempfile::TempDir,
        db: ForkDb,
        handy: FakeHistory,
        clock: FakeClock,
    }

    fn setup() -> Setup {
        let dir = tempfile::tempdir().unwrap();
        let db = ForkDb::in_dir(dir.path());
        let handy = FakeHistory::new(dir.path());
        Setup {
            _dir: dir,
            db,
            handy,
            clock: FakeClock::new(),
        }
    }

    /// A dictation made `days_ago` days before the fake clock's now, with a
    /// WAV, in Handy and in fork.db (through the real write path).
    fn take(s: &Setup, id: i64, days_ago: i64, text: &str) {
        let created = s.clock.unix_ms() - days_ago * DAY_MS;
        let e = entry(id, created / 1000, text);
        s.handy.add(e.clone(), Some(1000));
        let mut w = crate::fork::db::entry_meta::EntryWrite::for_saved(&e, None, None, |_| {
            "2026-01-01".to_owned()
        });
        w.created_at = created;
        w.audio_ms = 1000;
        s.db.with_conn(|c| crate::fork::db::entry_meta::write(c, &w, false))
            .unwrap();
    }

    fn total_words(db: &ForkDb) -> i64 {
        db.with_conn(|c| {
            Ok(
                c.query_row("SELECT COALESCE(SUM(words), 0) FROM daily_stats", [], |r| {
                    r.get(0)
                })?,
            )
        })
        .unwrap()
    }

    #[test]
    fn audio_expires_after_14_days_and_the_text_stays() {
        let s = setup();
        take(&s, 1, 20, "old take");
        take(&s, 2, 14, "exactly fourteen days");
        take(&s, 3, 1, "fresh");
        let r = run(&s.db, &s.handy, s.clock.unix_ms() + 1, POLICY).unwrap();
        // Strictly older than 14 days: ids 1 and 2 (2 is 14 days + 1 ms).
        assert_eq!(r.audio_deleted, 2);
        assert!(!s.handy.audio_path("wispr-bro-1.wav").exists());
        assert!(!s.handy.audio_path("wispr-bro-2.wav").exists());
        assert!(s.handy.audio_path("wispr-bro-3.wav").exists());
        // Rows and stats untouched.
        assert_eq!(s.handy.ids(), vec![1, 2, 3]);
        assert!(s.handy.deleted.lock().unwrap().is_empty());
        assert_eq!(total_words(&s.db), 2 + 3 + 1);
        // Second run: nothing left to do.
        let again = run(&s.db, &s.handy, s.clock.unix_ms() + 1, POLICY).unwrap();
        assert_eq!(again, Report::default());
    }

    #[test]
    fn the_clock_moving_forward_expires_more() {
        let s = setup();
        take(&s, 1, 0, "today");
        assert_eq!(
            run(&s.db, &s.handy, s.clock.unix_ms(), POLICY)
                .unwrap()
                .audio_deleted,
            0
        );
        s.clock.advance(Duration::from_secs(15 * 24 * 3600));
        assert_eq!(
            run(&s.db, &s.handy, s.clock.unix_ms(), POLICY)
                .unwrap()
                .audio_deleted,
            1
        );
    }

    #[test]
    fn text_limit_deletes_rows_only_through_handy_and_never_stats() {
        let s = setup();
        take(&s, 1, 40, "one two three");
        take(&s, 2, 10, "four five");
        let words_before = total_words(&s.db);
        let policy = Policy {
            text_days: Some(30),
            audio_days: Some(14),
        };
        let r = run(&s.db, &s.handy, s.clock.unix_ms(), policy).unwrap();
        assert_eq!(r.rows_deleted, 1);
        assert_eq!(*s.handy.deleted.lock().unwrap(), vec![1]);
        assert_eq!(s.handy.ids(), vec![2]);
        // Entry 1's WAV went with its row (Handy's delete), not as "audio".
        assert_eq!(r.audio_deleted, 0);
        let meta: Vec<i64> =
            s.db.with_conn(|c| {
                let mut st = c.prepare("SELECT entry_id FROM entry_meta")?;
                let v = st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
                Ok(v)
            })
            .unwrap();
        assert_eq!(meta, vec![2]);
        assert_eq!(total_words(&s.db), words_before, "stats never shrink");
    }

    #[test]
    fn saved_rows_files_and_failed_deletes_are_kept() {
        let s = setup();
        take(&s, 1, 40, "saved one");
        s.handy.rows.lock().unwrap().get_mut(&1).unwrap().saved = true;
        // A transcribed file.
        let mut f = entry(2, (s.clock.unix_ms() - 40 * DAY_MS) / 1000, "from a file");
        f.source_name = Some("talk.mp3".into());
        s.handy.add(f.clone(), Some(1000));
        let w = crate::fork::db::entry_meta::EntryWrite::for_saved(&f, None, None, |_| {
            "2026-01-01".into()
        });
        s.db.with_conn(|c| crate::fork::db::entry_meta::write(c, &w, false))
            .unwrap();

        let policy = Policy {
            text_days: Some(30),
            audio_days: Some(14),
        };
        let r = run(&s.db, &s.handy, s.clock.unix_ms(), policy).unwrap();
        assert_eq!(r.rows_deleted, 0);
        assert_eq!(r.audio_deleted, 0);
        assert_eq!(r.kept_saved, 1);
        assert_eq!(s.handy.ids(), vec![1, 2]);
        assert!(s.handy.audio_path("wispr-bro-2.wav").exists());

        take(&s, 3, 40, "cannot delete");
        *s.handy.fail_delete.lock().unwrap() = true;
        let r = run(&s.db, &s.handy, s.clock.unix_ms(), policy).unwrap();
        assert_eq!(r.failed, 1);
        assert!(s.handy.ids().contains(&3));
    }

    #[test]
    fn zero_audio_days_keeps_no_audio_and_forever_keeps_all() {
        let s = setup();
        take(&s, 1, 0, "now");
        let keep_none = Policy {
            text_days: None,
            audio_days: Some(0),
        };
        assert_eq!(
            run(&s.db, &s.handy, s.clock.unix_ms() + 1, keep_none)
                .unwrap()
                .audio_deleted,
            1
        );
        let s = setup();
        take(&s, 1, 3000, "ancient");
        let forever = Policy {
            text_days: None,
            audio_days: None,
        };
        assert_eq!(
            run(&s.db, &s.handy, s.clock.unix_ms(), forever).unwrap(),
            Report::default()
        );
    }

    #[test]
    fn soft_deleted_rows_still_lose_their_audio_but_are_not_text_expired_twice() {
        let s = setup();
        take(&s, 1, 40, "hidden");
        s.db.with_conn(|c| crate::fork::db::entry_meta::soft_delete(c, &[1], 0))
            .unwrap();
        let p =
            s.db.with_conn(|c| {
                plan(
                    c,
                    s.clock.unix_ms(),
                    Policy {
                        text_days: Some(30),
                        audio_days: Some(14),
                    },
                )
            })
            .unwrap();
        assert!(p.rows.is_empty(), "the purge owns soft-deleted rows");
        assert_eq!(p.audio, vec![(1, "wispr-bro-1.wav".to_owned())]);
    }
}
