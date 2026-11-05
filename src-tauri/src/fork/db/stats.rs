// SPDX-License-Identifier: GPL-3.0-or-later
//! The stats store: `daily_stats` and `daily_app_stats` (R5 §4.3, DEC-07,
//! DEC-11). These tables survive retention and deletes; only delete-all
//! (S-27) removes them.
//!
//! Rules (C-212):
//! - each non-empty dictation counts once, on its local day at record time;
//! - a retry changes the counts by the delta between what was counted and
//!   what the new text gives (a failed take that a retry fixes is counted
//!   then; a retry that comes back empty uncounts it);
//! - deleting a row, by the user or by retention, never subtracts;
//! - files, practice takes, commands, transforms and failed (empty) takes
//!   are not counted.
//!
//! Every column is clamped at zero, so no correction can drive a counter
//! negative (SpeakoFlow `apply_usage_delta`, MIT, pattern).

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};

use super::DbError;
use crate::fork::context::categories::UsageCategory;
use crate::fork::stats::{streak, wpm};

/// What one entry adds to the stats.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contribution {
    pub day: String,
    pub usage_category: UsageCategory,
    pub words: i64,
    pub speech_ms: i64,
    pub fixes_dictionary: i64,
    pub fixes_llm: i64,
}

impl Contribution {
    /// Words whose speech time is known (WPM numerator).
    fn timed_words(&self) -> i64 {
        if self.speech_ms > 0 {
            self.words
        } else {
            0
        }
    }
}

fn add(conn: &Connection, c: &Contribution, sign: i64) -> Result<(), DbError> {
    conn.prepare_cached(
        "INSERT INTO daily_stats
             (day, words, dictations, speech_ms, timed_words, fixes_dictionary, fixes_llm)
         VALUES (?1, MAX(0, ?2), MAX(0, ?3), MAX(0, ?4), MAX(0, ?5), MAX(0, ?6), MAX(0, ?7))
         ON CONFLICT (day) DO UPDATE SET
             words = MAX(0, words + ?2),
             dictations = MAX(0, dictations + ?3),
             speech_ms = MAX(0, speech_ms + ?4),
             timed_words = MAX(0, timed_words + ?5),
             fixes_dictionary = MAX(0, fixes_dictionary + ?6),
             fixes_llm = MAX(0, fixes_llm + ?7)",
    )?
    .execute(params![
        c.day,
        sign * c.words,
        sign,
        sign * c.speech_ms,
        sign * c.timed_words(),
        sign * c.fixes_dictionary,
        sign * c.fixes_llm,
    ])?;
    conn.prepare_cached(
        "INSERT INTO daily_app_stats (day, usage_category, words, dictations)
         VALUES (?1, ?2, MAX(0, ?3), MAX(0, ?4))
         ON CONFLICT (day, usage_category) DO UPDATE SET
             words = MAX(0, words + ?3),
             dictations = MAX(0, dictations + ?4)",
    )?
    .execute(params![
        c.day,
        c.usage_category.as_str(),
        sign * c.words,
        sign
    ])?;
    Ok(())
}

/// Replaces what an entry contributed (`before`, `None` if it was not
/// counted) with `after` (`None` if it no longer counts). Call inside the
/// transaction that writes the entry.
pub fn apply(
    conn: &Connection,
    before: Option<&Contribution>,
    after: Option<&Contribution>,
) -> Result<bool, DbError> {
    if before == after {
        return Ok(false);
    }
    if let Some(b) = before {
        add(conn, b, -1)?;
    }
    if let Some(a) = after {
        add(conn, a, 1)?;
    }
    Ok(true)
}

/// Lifetime and today's numbers for the Home rail (S-15a) and Insights
/// (S-25).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct StatsSummary {
    pub total_words: u64,
    pub total_dictations: u64,
    pub today_words: u64,
    pub today_dictations: u64,
    /// Speech time of the counted dictations, ms.
    pub speech_ms: u64,
    /// Words per minute on speech time; `None` with under 3 s of speech.
    pub words_per_minute: Option<f64>,
    pub current_streak_days: u32,
    pub longest_streak_days: u32,
    pub active_days: u32,
    pub fixes_dictionary: u64,
    pub fixes_llm: u64,
}

/// The summary as of local day `today`.
pub fn summary(conn: &Connection, today: NaiveDate) -> Result<StatsSummary, DbError> {
    let (words, dictations, speech_ms, timed, fd, fl): (i64, i64, i64, i64, i64, i64) = conn
        .query_row(
            "SELECT COALESCE(SUM(words), 0), COALESCE(SUM(dictations), 0),
                    COALESCE(SUM(speech_ms), 0), COALESCE(SUM(timed_words), 0),
                    COALESCE(SUM(fixes_dictionary), 0), COALESCE(SUM(fixes_llm), 0)
             FROM daily_stats",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )?;
    let today_key = today.format("%Y-%m-%d").to_string();
    let (today_words, today_dictations): (i64, i64) = conn
        .query_row(
            "SELECT words, dictations FROM daily_stats WHERE day = ?1",
            [&today_key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .unwrap_or((0, 0));
    let mut stmt = conn.prepare_cached("SELECT day FROM daily_stats WHERE dictations > 0")?;
    let days: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let active = streak::parse_days(days.iter().map(String::as_str));
    let streaks = streak::streaks(&active, today);
    let u = |v: i64| u64::try_from(v).unwrap_or(0);
    Ok(StatsSummary {
        total_words: u(words),
        total_dictations: u(dictations),
        today_words: u(today_words),
        today_dictations: u(today_dictations),
        speech_ms: u(speech_ms),
        words_per_minute: wpm::words_per_minute(u(timed), u(speech_ms)),
        current_streak_days: streaks.current,
        longest_streak_days: streaks.longest,
        active_days: u32::try_from(active.len()).unwrap_or(u32::MAX),
        fixes_dictionary: u(fd),
        fixes_llm: u(fl),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::db::ForkDb;

    fn c(day: &str, cat: UsageCategory, words: i64, ms: i64) -> Contribution {
        Contribution {
            day: day.into(),
            usage_category: cat,
            words,
            speech_ms: ms,
            fixes_dictionary: 0,
            fixes_llm: 0,
        }
    }

    fn row(conn: &Connection, day: &str) -> (i64, i64, i64, i64) {
        conn.query_row(
            "SELECT words, dictations, speech_ms, timed_words FROM daily_stats WHERE day = ?1",
            [day],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap()
    }

    #[test]
    fn add_replace_and_remove_a_contribution() {
        let dir = tempfile::tempdir().unwrap();
        let db = ForkDb::in_dir(dir.path());
        db.with_conn(|conn| {
            let a = c("2026-10-30", UsageCategory::Email, 10, 4000);
            assert!(apply(conn, None, Some(&a))?);
            assert_eq!(row(conn, "2026-10-30"), (10, 1, 4000, 10));
            // Same values again: nothing changes.
            assert!(!apply(conn, Some(&a), Some(&a))?);
            // A retry with more words.
            let b = c("2026-10-30", UsageCategory::Email, 14, 4000);
            apply(conn, Some(&a), Some(&b))?;
            assert_eq!(row(conn, "2026-10-30"), (14, 1, 4000, 14));
            let app: (i64, i64) = conn.query_row(
                "SELECT words, dictations FROM daily_app_stats WHERE day = '2026-10-30' AND usage_category = 'email'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            assert_eq!(app, (14, 1));
            // A retry that came back empty uncounts it.
            apply(conn, Some(&b), None)?;
            assert_eq!(row(conn, "2026-10-30"), (0, 0, 0, 0));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn untimed_words_do_not_feed_wpm_and_counters_never_go_negative() {
        let dir = tempfile::tempdir().unwrap();
        let db = ForkDb::in_dir(dir.path());
        db.with_conn(|conn| {
            apply(
                conn,
                None,
                Some(&c("2026-10-30", UsageCategory::Other, 100, 0)),
            )?;
            apply(
                conn,
                None,
                Some(&c("2026-10-30", UsageCategory::Other, 30, 12_000)),
            )?;
            assert_eq!(row(conn, "2026-10-30"), (130, 2, 12_000, 30));
            let s = summary(conn, NaiveDate::from_ymd_opt(2026, 10, 30).unwrap())?;
            assert_eq!(s.words_per_minute, Some(150.0));
            // Subtracting more than was there clamps at zero.
            apply(
                conn,
                Some(&c("2026-10-29", UsageCategory::Other, 5, 5)),
                None,
            )?;
            assert_eq!(row(conn, "2026-10-29"), (0, 0, 0, 0));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn summary_totals_today_and_streaks() {
        let dir = tempfile::tempdir().unwrap();
        let db = ForkDb::in_dir(dir.path());
        db.with_conn(|conn| {
            for (day, words) in [("2026-10-27", 5), ("2026-10-28", 7), ("2026-10-30", 11)] {
                apply(conn, None, Some(&c(day, UsageCategory::Work, words, 6000)))?;
            }
            let s = summary(conn, NaiveDate::from_ymd_opt(2026, 10, 30).unwrap())?;
            assert_eq!(s.total_words, 23);
            assert_eq!(s.total_dictations, 3);
            assert_eq!(s.today_words, 11);
            assert_eq!(s.today_dictations, 1);
            assert_eq!(s.speech_ms, 18_000);
            assert_eq!(s.current_streak_days, 1);
            assert_eq!(s.longest_streak_days, 2);
            assert_eq!(s.active_days, 3);
            let empty = summary(conn, NaiveDate::from_ymd_opt(2027, 1, 1).unwrap())?;
            assert_eq!(empty.today_words, 0);
            assert_eq!(empty.current_streak_days, 0);
            Ok(())
        })
        .unwrap();
    }
}
