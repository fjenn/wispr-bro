// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for the history hooks, the stats invariants and search (S-09).

use std::path::Path;
use std::time::Instant;

use super::*;
use crate::fork::context::categories::{self, StyleGroup};
use crate::fork::context::take::Ledger;
use crate::fork::db::handy::fake::{entry, FakeHistory};

fn db() -> (tempfile::TempDir, ForkDb) {
    let dir = tempfile::tempdir().unwrap();
    let db = ForkDb::in_dir(dir.path());
    (dir, db)
}

fn day(_: i64) -> String {
    "2026-10-30".into()
}

fn ctx(bundle: &str, name: &str) -> TakeContext {
    let mut c = TakeContext::new(
        1_793_400_000_000,
        "2026-10-30".into(),
        Some(FrontApp {
            bundle_id: Some(bundle.into()),
            name: Some(name.into()),
        }),
    );
    c.day = "2026-10-30".into();
    c
}

/// A dictation through the same path the hooks use: record start, WAV
/// name, verbatim text, Handy's save.
fn dictate(
    conn: &mut Connection,
    l: &mut Ledger,
    id: i64,
    app: (&str, &str),
    verbatim: &str,
    kept: &str,
) -> EntryWrite {
    l.record_started(ctx(app.0, app.1));
    let file = format!("wispr-bro-{id}.wav");
    l.bind(&file, 4_000, || unreachable!());
    l.note_verbatim(verbatim.into(), kept);
    let mut e = entry(id, 1_793_400_000, kept);
    e.file_name = file.clone();
    let w = EntryWrite::for_saved(
        &e,
        l.take_bound(&file),
        l.take_verbatim(&e.transcription_text),
        day,
    );
    write(conn, &w, false).unwrap();
    w
}

fn totals(conn: &Connection) -> (i64, i64, i64) {
    conn.query_row(
        "SELECT COALESCE(SUM(words), 0), COALESCE(SUM(dictations), 0), COALESCE(SUM(speech_ms), 0)
         FROM daily_stats",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .unwrap()
}

fn app_words(conn: &Connection, cat: &str) -> i64 {
    conn.query_row(
        "SELECT COALESCE(SUM(words), 0) FROM daily_app_stats WHERE usage_category = ?1",
        [cat],
        |r| r.get(0),
    )
    .unwrap()
}

#[test]
fn a_dictation_row_knows_its_app_verbatim_text_and_counts() {
    let (_d, db) = db();
    let mut l = Ledger::new();
    db.with_conn(|conn| {
        let w = dictate(
            conn,
            &mut l,
            1,
            ("com.apple.mail", "Mail"),
            "um so hello there",
            "So hello there.",
        );
        assert_eq!(w.asr_text, "um so hello there");
        assert_eq!(w.class.style_group, StyleGroup::Email);
        let m = crate::fork::commands::history::read_meta(conn, 1)?.unwrap();
        assert_eq!(m.app_bundle_id.as_deref(), Some("com.apple.mail"));
        assert_eq!(m.app_name.as_deref(), Some("Mail"));
        assert_eq!(m.style_group.as_deref(), Some("email"));
        assert_eq!(m.usage_category.as_deref(), Some("email"));
        assert_eq!(m.asr_text.as_deref(), Some("um so hello there"));
        assert_eq!(m.final_text.as_deref(), Some("So hello there."));
        assert_eq!(m.word_count, 3);
        assert_eq!(m.audio_ms, 4_000);
        assert_eq!(m.created_at, 1_793_400_000_000);
        assert_eq!(m.source, "dictation");
        assert!(m.has_audio);
        assert!(!m.backfilled);
        assert_eq!(totals(conn), (3, 1, 4_000));
        assert_eq!(app_words(conn, "email"), 3);
        let day: String = conn.query_row("SELECT day FROM entry_meta", [], |r| r.get(0))?;
        assert_eq!(day, "2026-10-30");
        Ok(())
    })
    .unwrap();
}

#[test]
fn saving_twice_counts_once() {
    let (_d, db) = db();
    let mut l = Ledger::new();
    db.with_conn(|conn| {
        let w = dictate(
            conn,
            &mut l,
            1,
            ("com.apple.Notes", "Notes"),
            "a b c",
            "a b c",
        );
        // The hook runs again for the same row (idempotent upsert).
        assert!(!write(conn, &w, false)?);
        assert_eq!(totals(conn), (3, 1, 4_000));
        Ok(())
    })
    .unwrap();
}

#[test]
fn retry_updates_counts_by_the_delta_and_keeps_the_first_context() {
    let (_d, db) = db();
    let mut l = Ledger::new();
    db.with_conn(|conn| {
        dictate(
            conn,
            &mut l,
            1,
            ("com.tinyspeck.slackmacgap", "Slack"),
            "one two",
            "one two",
        );
        dictate(
            conn,
            &mut l,
            2,
            ("com.apple.mail", "Mail"),
            "three",
            "three",
        );
        assert_eq!(totals(conn), (3, 2, 8_000));

        // Retry of 1 gives four words; the Handy row has no app or start.
        let mut e = entry(1, 1_793_499_999, "one two three four");
        e.file_name = "wispr-bro-1.wav".into();
        let w = EntryWrite::for_saved(&e, None, None, |_| "2026-11-02".into());
        assert!(write(conn, &w, true)?);
        assert_eq!(totals(conn), (5, 2, 8_000), "+2 words, still 2 dictations");
        assert_eq!(app_words(conn, "work"), 4);
        let m = crate::fork::commands::history::read_meta(conn, 1)?.unwrap();
        assert_eq!(
            m.app_bundle_id.as_deref(),
            Some("com.tinyspeck.slackmacgap")
        );
        assert_eq!(m.previous_text.as_deref(), Some("one two"));
        assert_eq!(m.created_at, 1_793_400_000_000);
        let d: String =
            conn.query_row("SELECT day FROM entry_meta WHERE entry_id = 1", [], |r| {
                r.get(0)
            })?;
        assert_eq!(
            d, "2026-10-30",
            "a retry corrects the day it was dictated on"
        );

        // A retry that comes back empty uncounts the take.
        let mut empty = e.clone();
        empty.transcription_text = String::new();
        let w = EntryWrite::for_saved(&empty, None, None, day);
        write(conn, &w, true)?;
        assert_eq!(totals(conn), (1, 1, 4_000));
        Ok(())
    })
    .unwrap();
}

#[test]
fn a_failed_take_fixed_by_retry_is_counted_then() {
    let (_d, db) = db();
    let mut l = Ledger::new();
    db.with_conn(|conn| {
        dictate(conn, &mut l, 1, ("com.apple.Notes", "Notes"), "", "");
        assert_eq!(totals(conn), (0, 0, 0), "failed takes are not counted");
        let e = entry(1, 1_793_400_000, "now it worked");
        write(conn, &EntryWrite::for_saved(&e, None, None, day), true)?;
        assert_eq!(totals(conn), (3, 1, 4_000));
        Ok(())
    })
    .unwrap();
}

#[test]
fn deletes_never_subtract() {
    let (_d, db) = db();
    let mut l = Ledger::new();
    db.with_conn(|conn| {
        dictate(
            conn,
            &mut l,
            1,
            ("com.apple.Notes", "Notes"),
            "keep me counted",
            "keep me counted",
        );
        super::super::delete_entry_meta(conn, 1)?;
        assert_eq!(totals(conn), (3, 1, 4_000));
        assert_eq!(app_words(conn, "documents"), 3);
        assert!(fts::search_history(conn, "counted", 10)?.is_empty());
        Ok(())
    })
    .unwrap();
}

#[test]
fn files_practice_and_other_sources_are_not_counted() {
    let (_d, db) = db();
    db.with_conn(|conn| {
        let mut f = entry(1, 1_793_400_000, "");
        f.source_name = Some("talk.mp3".into());
        write(conn, &EntryWrite::for_saved(&f, None, None, day), false)?;
        // The file finishes transcribing (Handy's update_transcription).
        f.transcription_text = "a long talk from a file".into();
        write(conn, &EntryWrite::for_saved(&f, None, None, day), true)?;
        assert_eq!(totals(conn), (0, 0, 0));
        // It is still searchable.
        assert_eq!(fts::search_history(conn, "long talk", 10)?, vec![1]);

        let mut ctx = ctx("com.apple.Notes", "Notes");
        ctx.source = TakeSource::Practice;
        let e = entry(2, 1_793_400_000, "practice words here");
        let w = EntryWrite::for_saved(
            &e,
            Some(BoundTake {
                file_name: e.file_name.clone(),
                context: ctx,
                audio_ms: 1_000,
                audio_missing: false,
            }),
            None,
            day,
        );
        write(conn, &w, false)?;
        assert_eq!(totals(conn), (0, 0, 0));
        Ok(())
    })
    .unwrap();
}

#[test]
fn post_processed_text_is_the_final_text() {
    let (_d, db) = db();
    db.with_conn(|conn| {
        let mut e = entry(1, 1_793_400_000, "raw words here now");
        e.post_processed_text = Some("Cleaned words.".into());
        let w = EntryWrite::for_saved(&e, None, None, day);
        assert_eq!(w.final_text, "Cleaned words.");
        assert_eq!(w.asr_text, "raw words here now");
        write(conn, &w, false)?;
        assert_eq!(totals(conn).0, 2);
        Ok(())
    })
    .unwrap();
}

#[test]
fn a_take_without_audio_is_kept_and_marked() {
    let (_d, db) = db();
    let mut l = Ledger::new();
    db.with_conn(|conn| {
        l.record_started(ctx("com.apple.Notes", "Notes"));
        l.bind("wispr-bro-1.wav", 2_000, || unreachable!());
        l.mark_audio_missing("wispr-bro-1.wav");
        let mut e = entry(1, 1_793_400_000, "disk was full");
        e.file_name = "wispr-bro-1.wav".into();
        write(
            conn,
            &EntryWrite::for_saved(&e, l.take_bound(&e.file_name), None, day),
            false,
        )?;
        let m = crate::fork::commands::history::read_meta(conn, 1)?.unwrap();
        assert!(!m.has_audio);
        assert_eq!(m.final_text.as_deref(), Some("disk was full"));
        // The audio retention job has nothing to delete for it.
        let p = super::super::retention::plan(
            conn,
            i64::MAX / 2,
            super::super::retention::Policy {
                text_days: None,
                audio_days: Some(14),
            },
        )?;
        assert!(p.audio.is_empty());
        Ok(())
    })
    .unwrap();
}

#[test]
fn paste_outcome_is_stored_per_take() {
    let (_d, db) = db();
    let mut l = Ledger::new();
    db.with_conn(|conn| {
        dictate(
            conn,
            &mut l,
            1,
            ("com.apple.TextEdit", "TextEdit"),
            "first",
            "First.",
        );
        dictate(
            conn,
            &mut l,
            2,
            ("com.apple.mail", "Mail"),
            "second",
            "Second.",
        );
        l.await_outcome(1, "First.");
        l.await_outcome(2, "Second.");
        // Delivered with Handy's trailing space.
        let id = l.take_awaiting("Second. ").unwrap();
        set_paste_outcome(conn, id, PasteOutcome::Card)?;
        let id = l.take_awaiting("First.").unwrap();
        set_paste_outcome(conn, id, PasteOutcome::Confirmed)?;
        let get = |id: i64| -> Option<String> {
            conn.query_row(
                "SELECT paste_outcome FROM entry_meta WHERE entry_id = ?1",
                [id],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(get(1).as_deref(), Some("confirmed"));
        assert_eq!(get(2).as_deref(), Some("card"));
        // Every outcome S-04 can report fits the column.
        for o in [
            PasteOutcome::Confirmed,
            PasteOutcome::Unverified,
            PasteOutcome::Card,
            PasteOutcome::Refused,
            PasteOutcome::Queued,
        ] {
            set_paste_outcome(conn, 1, o)?;
            assert_eq!(get(1).as_deref(), Some(o.as_str()));
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn soft_delete_hides_at_once_undo_restores_and_purge_goes_through_handy() {
    let dir = tempfile::tempdir().unwrap();
    let db = ForkDb::in_dir(dir.path());
    let handy = FakeHistory::new(dir.path());
    let mut l = Ledger::new();
    for id in 1..=2 {
        handy.add(entry(id, 1_793_400_000, "secret plans"), Some(500));
        db.with_conn(|c| {
            dictate(
                c,
                &mut l,
                id,
                ("com.apple.Notes", "Notes"),
                "secret plans",
                "secret plans",
            );
            Ok(())
        })
        .unwrap();
    }
    let now = 1_793_500_000_000;
    db.with_conn(|c| {
        assert_eq!(soft_delete(c, &[1, 2], now)?, 2);
        assert_eq!(soft_delete(c, &[1], now)?, 0, "already hidden");
        assert!(fts::search_history(c, "secret", 10)?.is_empty());
        assert!(fts::search_history(c, "se", 10)?.is_empty());
        assert_eq!(undo_delete(c, &[2])?, 1);
        assert_eq!(fts::search_history(c, "secret", 10)?, vec![2]);
        // Inside the window nothing is due; after it, only 1.
        assert!(due_purges(c, now + 5_000, PURGE_DELAY)?.is_empty());
        assert_eq!(due_purges(c, now + 10_000, PURGE_DELAY)?, vec![1]);
        Ok(())
    })
    .unwrap();
    let words_before = db.with_conn(|c| Ok(totals(c))).unwrap();
    let ids = db
        .with_conn(|c| due_purges(c, now + 10_000, PURGE_DELAY))
        .unwrap();
    assert_eq!(purge(&db, &handy, &ids).unwrap(), vec![1]);
    assert_eq!(*handy.deleted.lock().unwrap(), vec![1]);
    assert_eq!(handy.ids(), vec![2]);
    db.with_conn(|c| {
        assert!(read_meta_exists(c, 2));
        assert!(!read_meta_exists(c, 1));
        assert_eq!(totals(c), words_before, "a user delete never subtracts");
        Ok(())
    })
    .unwrap();
}

fn read_meta_exists(c: &Connection, id: i64) -> bool {
    super::super::entry_meta_exists(c, id).unwrap()
}

#[test]
fn fifty_takes_in_five_apps_give_correct_per_app_rows() {
    // The unit-level twin of Mac test S-09.1.
    let (_d, db) = db();
    let mut l = Ledger::new();
    let apps = [
        ("com.apple.mail", "Mail", "email"),
        ("com.tinyspeck.slackmacgap", "Slack", "work"),
        ("com.apple.MobileSMS", "Messages", "personal"),
        ("com.apple.Notes", "Notes", "documents"),
        ("com.openai.chat", "ChatGPT", "ai_prompts"),
    ];
    db.with_conn(|conn| {
        for i in 0..50 {
            let (b, n, _) = apps[i % 5];
            dictate(conn, &mut l, i as i64 + 1, (b, n), "two words", "two words");
        }
        for (b, _, cat) in apps {
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM entry_meta WHERE app_bundle_id = ?1 AND usage_category = ?2",
                [b, cat],
                |r| r.get(0),
            )?;
            assert_eq!(n, 10, "{b}");
            assert_eq!(app_words(conn, cat), 20, "{cat}");
        }
        assert_eq!(totals(conn), (100, 50, 200_000));
        Ok(())
    })
    .unwrap();
}

#[test]
fn ten_thousand_rows_search_within_100_ms() {
    let (_d, db) = db();
    let words = [
        "meeting",
        "budget",
        "launch",
        "review",
        "coffee",
        "design",
        "travel",
        "invoice",
        "garden",
        "piano",
        "rocket",
        "window",
        "おはよう",
        "東京",
        "Straße",
    ];
    db.with_conn(|conn| {
        let tx = conn.transaction()?;
        for i in 0..10_000i64 {
            let text = format!(
                "Note {i}: {} {} {} and the {} for next week",
                words[(i % 15) as usize],
                words[((i * 7) % 15) as usize],
                words[((i * 11) % 15) as usize],
                words[((i * 13) % 15) as usize]
            );
            upsert_entry_meta(
                &tx,
                i + 1,
                &[("created_at", &(i * 1000)), ("final_text", &text)],
            )?;
            fts::index_history_entry(&tx, i + 1, &text)?;
        }
        tx.commit()?;
        Ok(())
    })
    .unwrap();
    db.with_conn(|conn| {
        // Warm the statement cache once, like the second keystroke would.
        fts::search_history(conn, "warm", 50)?;
        for (q, expect_hits) in [
            ("budget", true),
            ("next week", true),
            ("東京", true),
            ("ok", false),
            ("zz", false),
            ("Note 9999", true),
        ] {
            // Best of 5: the test suite runs in parallel on a shared Mac, so
            // a single sample measures the machine's load, not the query.
            let mut took = std::time::Duration::MAX;
            let mut hits = Vec::new();
            for _ in 0..5 {
                let t = Instant::now();
                hits = fts::search_history(conn, q, 50)?;
                took = took.min(t.elapsed());
            }
            eprintln!("search {q:?}: {} hits in {took:?}", hits.len());
            assert_eq!(!hits.is_empty(), expect_hits, "{q}");
            assert!(
                took.as_millis() <= 100,
                "search {q:?} took {took:?} on 10,000 rows (budget 100 ms)"
            );
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn categories_drive_the_row_values() {
    // Same classification the ledger uses at record start.
    let c = categories::classify(Some("com.openai.chat"), Some("ChatGPT"));
    assert!(c.is_ai_chat);
    assert_eq!(c.usage_category.as_str(), "ai_prompts");
}

/// Fork code never touches Handy's database directly (R5 §4.3): rows are
/// read and deleted only through `HistoryManager` (`fork::db::handy`).
#[test]
fn fork_code_never_opens_history_db() {
    fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs" || x == "sql") {
                out.push(p);
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/fork");
    let mut files = Vec::new();
    walk(&root, &mut files);
    assert!(files.len() > 20);
    for f in files {
        // Test files simulate a history.db on purpose (F-06's E-71 test).
        let name = f.file_name().unwrap().to_str().unwrap();
        if name == "tests.rs" || name.ends_with("_tests.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&f).unwrap();
        for needle in [
            "transcription_history",
            "\"history.db\"",
            "join(\"history.db",
        ] {
            assert!(
                !text.contains(needle),
                "{} mentions {needle}: use fork::db::handy instead",
                f.display()
            );
        }
    }
}

/// The Handy hooks are in place (R5 §4.2; budgets in ci/hooks.txt).
#[test]
fn handy_hooks_are_in_place() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |rel: &str| std::fs::read_to_string(root.join(rel)).unwrap();
    let history = read("src/managers/history.rs");
    assert!(history.contains("crate::fork::db::on_saved(&self.app_handle, &entry);"));
    assert!(history.contains("crate::fork::db::on_retried(&self.app_handle, &entry);"));
    assert!(history.contains("crate::fork::db::on_deleted(&self.app_handle, id);"));
    let actions = read("src/actions.rs");
    assert!(actions.contains("crate::fork::on_record_start(app);"));
    assert!(actions.contains("crate::fork::take_file_name(sample_count)"));
    assert!(actions.contains("if wav_saved || crate::fork::keep_take(&wav_path_for_verify) {"));
    let transcription = read("src/managers/transcription.rs");
    assert!(transcription.contains("crate::fork::note_verbatim(verbatim, finished)"));
}
