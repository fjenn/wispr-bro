// SPDX-License-Identifier: GPL-3.0-or-later
//! Launch reconcile between Handy's `history.db` and `fork.db` (R5 §4.3).
//!
//! The fork writes after Handy, so a crash (or a fork.db that was recreated
//! after corruption) can leave Handy rows without `entry_meta`, and a crash
//! during a delete can leave `entry_meta` whose Handy row is gone. At launch:
//!
//! 1. purge soft-deleted rows whose undo window passed (the app quit within
//!    the 10 s);
//! 2. backfill a row for every Handy row the fork does not know: marked
//!    `backfilled`, words from its text, speech time from its WAV header when
//!    the WAV is still there, local day from Handy's timestamp, app unknown
//!    (`other`); it is counted in the stats like any dictation, since the
//!    fork never counted it;
//! 3. drop `entry_meta` rows whose Handy row is gone (stats untouched).

use std::collections::HashSet;
use std::path::Path;

use super::entry_meta::{self, EntryWrite};
use super::handy::HandyHistory;
use super::{DbError, ForkDb};

/// What one reconcile did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub purged: usize,
    pub backfilled: usize,
    pub dropped: usize,
}

/// Speech time of a WAV from its header, in ms.
pub fn wav_ms(path: &Path) -> Option<u64> {
    let reader = hound::WavReader::open(path).ok()?;
    let rate = u64::from(reader.spec().sample_rate);
    let frames = u64::from(reader.duration());
    (rate > 0 && frames > 0).then(|| frames * 1000 / rate)
}

/// Runs the reconcile. `local_day` maps Unix ms to a local day (injected
/// for tests).
pub fn run(
    db: &ForkDb,
    handy: &dyn HandyHistory,
    now_ms: i64,
    local_day: impl Fn(i64) -> String,
) -> Result<Report, DbError> {
    let mut report = Report::default();

    let due = db.with_conn(|c| entry_meta::due_purges(c, now_ms, entry_meta::PURGE_DELAY))?;
    report.purged = entry_meta::purge(db, handy, &due)?.len();

    let rows = handy.entries().map_err(|e| {
        DbError::Io(std::io::Error::other(format!(
            "could not read Handy's history: {e}"
        )))
    })?;
    let handy_ids: HashSet<i64> = rows.iter().map(|e| e.id).collect();
    let known: HashSet<i64> = db.with_conn(|c| {
        let mut stmt = c.prepare("SELECT entry_id FROM entry_meta")?;
        let ids = stmt
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(ids)
    })?;

    for entry in rows.iter().filter(|e| !known.contains(&e.id)) {
        let mut w = EntryWrite::for_saved(entry, None, None, &local_day);
        w.backfilled = true;
        let wav = handy.audio_path(&entry.file_name);
        match wav_ms(&wav) {
            Some(ms) => w.audio_ms = ms,
            None => w.audio_missing = !wav.exists(),
        }
        db.with_conn(|c| entry_meta::write(c, &w, false))?;
        report.backfilled += 1;
    }

    for id in known.iter().filter(|id| !handy_ids.contains(id)) {
        db.with_conn(|c| super::delete_entry_meta(c, *id))?;
        report.dropped += 1;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::db::handy::fake::{entry, FakeHistory};

    fn day(_: i64) -> String {
        "2026-10-30".into()
    }

    fn count(db: &ForkDb, sql: &str) -> i64 {
        db.with_conn(|c| Ok(c.query_row(sql, [], |r| r.get(0))?))
            .unwrap()
    }

    #[test]
    fn kill_between_the_handy_write_and_the_fork_write_is_backfilled() {
        let dir = tempfile::tempdir().unwrap();
        let db = ForkDb::in_dir(dir.path());
        let handy = FakeHistory::new(dir.path());
        // Saved by Handy, then the app died before the fork's hook ran.
        handy.add(entry(1, 1_793_400_000, "hello there friend"), Some(2_000));
        // A failed take (empty text) and one whose WAV is gone.
        handy.add(entry(2, 1_793_400_100, ""), Some(1_000));
        handy.add(entry(3, 1_793_400_200, "no audio left"), None);

        let r = run(&db, &handy, 1_793_500_000_000, day).unwrap();
        assert_eq!(r.backfilled, 3);
        assert_eq!(
            count(&db, "SELECT COUNT(*) FROM entry_meta WHERE backfilled = 1"),
            3
        );
        let (ms, words, counted): (i64, i64, i64) = db
            .with_conn(|c| {
                Ok(c.query_row(
                    "SELECT audio_ms, word_count, counted FROM entry_meta WHERE entry_id = 1",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )?)
            })
            .unwrap();
        assert_eq!((ms, words, counted), (2_000, 3, 1));
        assert_eq!(
            count(
                &db,
                "SELECT audio_deleted_at IS NOT NULL FROM entry_meta WHERE entry_id = 3"
            ),
            1
        );
        // Stats: entries 1 and 3 count (2 is a failed take); 3 is untimed.
        let (words, dictations, timed): (i64, i64, i64) = db
            .with_conn(|c| {
                Ok(c.query_row(
                    "SELECT words, dictations, timed_words FROM daily_stats",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )?)
            })
            .unwrap();
        assert_eq!((words, dictations, timed), (6, 2, 3));

        // Running again changes nothing (idempotent, counted once).
        let again = run(&db, &handy, 1_793_500_000_000, day).unwrap();
        assert_eq!(again, Report::default());
        assert_eq!(count(&db, "SELECT SUM(words) FROM daily_stats"), 6);
    }

    #[test]
    fn orphans_are_dropped_and_stats_kept() {
        let dir = tempfile::tempdir().unwrap();
        let db = ForkDb::in_dir(dir.path());
        let handy = FakeHistory::new(dir.path());
        handy.add(entry(1, 1_793_400_000, "kept row"), Some(500));
        handy.add(
            entry(2, 1_793_400_001, "row deleted while the app was down"),
            Some(500),
        );
        run(&db, &handy, 1_793_500_000_000, day).unwrap();
        // Handy's row 2 disappears without the fork's hook running.
        handy.rows.lock().unwrap().remove(&2);
        let r = run(&db, &handy, 1_793_500_000_000, day).unwrap();
        assert_eq!(r.dropped, 1);
        assert_eq!(count(&db, "SELECT COUNT(*) FROM entry_meta"), 1);
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM history_fts WHERE history_fts MATCH 'deleted'"
            ),
            0
        );
        assert_eq!(count(&db, "SELECT SUM(words) FROM daily_stats"), 2 + 7);
    }

    #[test]
    fn soft_deletes_left_by_a_quit_are_purged_through_handy() {
        let dir = tempfile::tempdir().unwrap();
        let db = ForkDb::in_dir(dir.path());
        let handy = FakeHistory::new(dir.path());
        handy.add(entry(1, 1_793_400_000, "delete me"), Some(500));
        handy.add(entry(2, 1_793_400_001, "undo window still open"), Some(500));
        run(&db, &handy, 1_793_500_000_000, day).unwrap();
        let now = 1_793_500_000_000;
        db.with_conn(|c| entry_meta::soft_delete(c, &[1], now - 60_000))
            .unwrap();
        db.with_conn(|c| entry_meta::soft_delete(c, &[2], now - 1_000))
            .unwrap();
        let r = run(&db, &handy, now, day).unwrap();
        assert_eq!(r.purged, 1);
        assert_eq!(*handy.deleted.lock().unwrap(), vec![1]);
        assert_eq!(handy.ids(), vec![2]);
        assert!(!handy.audio_path("wispr-bro-1.wav").exists());
        assert_eq!(count(&db, "SELECT COUNT(*) FROM entry_meta"), 1);
    }

    #[test]
    fn wav_header_gives_the_speech_time() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        crate::audio_toolkit::save_wav_file(&path, &vec![0.0; 24_000]).unwrap();
        assert_eq!(wav_ms(&path), Some(1_500));
        assert_eq!(wav_ms(&dir.path().join("missing.wav")), None);
    }
}
