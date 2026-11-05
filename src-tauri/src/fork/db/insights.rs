// SPDX-License-Identifier: GPL-3.0-or-later
//! The read side of Insights (S-25): rows from the stats store and the
//! history for `fork::stats::{insights, voice}`. Read-only; nothing here
//! writes.

use chrono::NaiveDate;
use rusqlite::Connection;

use super::DbError;
use crate::fork::context::categories::UsageCategory;
use crate::fork::stats::insights::{CategoryTotals, DayTotals, Totals};
use crate::fork::stats::voice::{LoggedCorrection, VoiceEntry, MAX_ENTRIES};

fn u(v: i64) -> u64 {
    u64::try_from(v).unwrap_or(0)
}

/// Lifetime sums of `daily_stats`.
pub fn totals(conn: &Connection) -> Result<Totals, DbError> {
    Ok(conn.query_row(
        "SELECT COALESCE(SUM(words), 0), COALESCE(SUM(dictations), 0),
                COALESCE(SUM(speech_ms), 0), COALESCE(SUM(timed_words), 0),
                COALESCE(SUM(fixes_dictionary), 0), COALESCE(SUM(fixes_llm), 0)
         FROM daily_stats",
        [],
        |r| {
            Ok(Totals {
                words: u(r.get(0)?),
                dictations: u(r.get(1)?),
                speech_ms: u(r.get(2)?),
                timed_words: u(r.get(3)?),
                fixes_dictionary: u(r.get(4)?),
                fixes_llm: u(r.get(5)?),
            })
        },
    )?)
}

/// Every `daily_stats` day (malformed days skipped).
pub fn days(conn: &Connection) -> Result<Vec<DayTotals>, DbError> {
    let mut stmt =
        conn.prepare_cached("SELECT day, words, dictations FROM daily_stats ORDER BY day")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (day, words, dictations) = row?;
        if let Ok(day) = NaiveDate::parse_from_str(&day, "%Y-%m-%d") {
            out.push(DayTotals {
                day,
                words: u(words),
                dictations: u(dictations),
            });
        }
    }
    Ok(out)
}

/// Lifetime words per usage category (`daily_app_stats`).
pub fn categories(conn: &Connection) -> Result<Vec<CategoryTotals>, DbError> {
    let mut stmt = conn.prepare_cached(
        "SELECT usage_category, SUM(words), SUM(dictations)
         FROM daily_app_stats GROUP BY usage_category",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (cat, words, dictations) = row?;
        if let Some(category) = UsageCategory::parse(&cat) {
            out.push(CategoryTotals {
                category,
                words: u(words),
                dictations: u(dictations),
            });
        }
    }
    Ok(out)
}

/// The newest counted dictations still in History, newest first, at most
/// [`MAX_ENTRIES`].
pub fn voice_entries(conn: &Connection) -> Result<Vec<VoiceEntry>, DbError> {
    let mut stmt = conn.prepare_cached(
        "SELECT created_at, final_text, asr_text, COALESCE(app_name, app_bundle_id),
                word_count, language
         FROM entry_meta
         WHERE counted = 1 AND deleted_at IS NULL AND source = 'dictation'
               AND final_text IS NOT NULL AND final_text <> ''
         ORDER BY created_at DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map([MAX_ENTRIES as i64], |r| {
        Ok(VoiceEntry {
            created_at: r.get(0)?,
            final_text: r.get(1)?,
            asr_text: r.get(2)?,
            app_name: r.get(3)?,
            word_count: u(r.get(4)?),
            language: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// `corrections` rows a later fix wrote without changing the stored text
/// (flags, auto-learn). Rule rows are left out: the text diff already has
/// them.
pub fn logged_corrections(conn: &Connection) -> Result<Vec<LoggedCorrection>, DbError> {
    let mut stmt = conn.prepare_cached(
        "SELECT from_text, to_text FROM corrections
         WHERE source IN ('flag', 'learn')
         ORDER BY id DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([MAX_ENTRIES as i64], |r| {
        Ok(LoggedCorrection {
            from_text: r.get(0)?,
            to_text: r.get(1)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::db::stats::{apply, Contribution};
    use crate::fork::db::ForkDb;
    use rusqlite::params;

    fn contribution(day: &str, cat: UsageCategory, words: i64, ms: i64) -> Contribution {
        Contribution {
            day: day.into(),
            usage_category: cat,
            words,
            speech_ms: ms,
            fixes_dictionary: 1,
            fixes_llm: 2,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn meta(
        conn: &Connection,
        id: i64,
        created_at: i64,
        text: &str,
        asr: &str,
        app: Option<&str>,
        counted: bool,
        source: &str,
        deleted: bool,
    ) {
        conn.execute(
            "INSERT INTO entry_meta (entry_id, created_at, final_text, asr_text, app_name,
                 word_count, counted, source, deleted_at, language)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'en')",
            params![
                id,
                created_at,
                text,
                asr,
                app,
                crate::fork::stats::wordcount::word_count(text),
                counted as i64,
                source,
                deleted.then_some(1_i64),
            ],
        )
        .unwrap();
    }

    /// The stats store as the S-09 writer leaves it, read back for the Usage
    /// tab and checked against hand-computed numbers.
    #[test]
    fn usage_reads_the_stats_store() {
        let dir = tempfile::tempdir().unwrap();
        let db = ForkDb::in_dir(dir.path());
        db.with_conn(|conn| {
            for (day, cat, words, ms) in [
                ("2026-10-29", UsageCategory::Email, 300, 120_000),
                ("2026-10-30", UsageCategory::AiPrompts, 450, 180_000),
                ("2026-10-30", UsageCategory::Email, 50, 0),
                ("2026-10-31", UsageCategory::Documents, 800, 300_000),
            ] {
                apply(conn, None, Some(&contribution(day, cat, words, ms)))?;
            }
            let t = totals(conn)?;
            assert_eq!(
                (t.words, t.dictations, t.speech_ms, t.timed_words),
                (1_600, 4, 600_000, 1_550)
            );
            assert_eq!((t.fixes_dictionary, t.fixes_llm), (4, 8));
            let d = days(conn)?;
            assert_eq!(d.len(), 3);
            assert_eq!((d[1].words, d[1].dictations), (500, 2));
            let mut c = categories(conn)?;
            c.sort_by_key(|x| x.category.as_str());
            assert_eq!(c.len(), 3);
            let email = c
                .iter()
                .find(|x| x.category == UsageCategory::Email)
                .unwrap();
            assert_eq!((email.words, email.dictations), (350, 2));

            let today = NaiveDate::from_ymd_opt(2026, 10, 31).unwrap();
            let u = crate::fork::stats::insights::usage(&t, &d, &c, None, today);
            // 1,550 timed words / 10 min = 155 wpm; 155 / 45 = 3.4×.
            assert_eq!(u.speed.words_per_minute, Some(155.0));
            assert_eq!(u.speed.times_typing, Some(3.4));
            // 800 / 1,600 documents = 50 %.
            assert_eq!(u.categories[0].share, 0.5);
            let last: Vec<u8> = u.heatmap[362..].iter().map(|x| x.level).collect();
            assert_eq!(last, vec![2, 3, 4]);
            assert_eq!(u.current_streak_days, 3);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn voice_reads_counted_live_dictations_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let db = ForkDb::in_dir(dir.path());
        db.with_conn(|conn| {
            meta(
                conn,
                1,
                100,
                "Old one.",
                "old one",
                Some("Mail"),
                true,
                "dictation",
                false,
            );
            meta(
                conn,
                2,
                300,
                "Newest.",
                "newest",
                None,
                true,
                "dictation",
                false,
            );
            meta(
                conn,
                3,
                200,
                "Not counted.",
                "x",
                None,
                false,
                "dictation",
                false,
            );
            meta(conn, 4, 250, "A file.", "x", None, true, "file", false);
            meta(conn, 5, 260, "Deleted.", "x", None, true, "dictation", true);
            let rows = voice_entries(conn)?;
            assert_eq!(
                rows.iter()
                    .map(|r| r.final_text.as_str())
                    .collect::<Vec<_>>(),
                vec!["Newest.", "Old one."]
            );
            assert_eq!(rows[1].app_name.as_deref(), Some("Mail"));
            assert_eq!(rows[1].asr_text.as_deref(), Some("old one"));

            for (from, to, source) in [
                ("gonna", "going to", "rule"),
                ("teh", "the", "flag"),
                ("wanna", "want to", "learn"),
            ] {
                conn.execute(
                    "INSERT INTO corrections (entry_id, from_text, to_text, source, created_at)
                     VALUES (1, ?1, ?2, ?3, 1)",
                    params![from, to, source],
                )?;
            }
            let logged = logged_corrections(conn)?;
            assert_eq!(logged.len(), 2);
            assert!(logged.iter().all(|c| c.from_text != "gonna"));
            Ok(())
        })
        .unwrap();
    }
}
