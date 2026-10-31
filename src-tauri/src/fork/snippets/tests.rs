// SPDX-License-Identifier: GPL-3.0-or-later
//! S-17 tests: validation (≤ 60 / ≤ 4,000 characters, line breaks kept,
//! control characters), the store (duplicates, conflicts, edits keep the
//! use count, undo, the 1,000 cap), a take's use recorded on the snippet
//! and its history row, migration 0006, CSV import (formula guard, multi-line
//! cells, dedupe, conflicts, the row cap) and export round trips.

use rusqlite::Connection;

use super::import::{self, SnippetImportRow};
use super::store::{self, SnippetSaveOutcome};
use super::*;
use crate::fork::db::ForkDb;
use crate::fork::text::snippets::SnippetHit;
use crate::fork::text::SnippetUse;

fn db() -> (tempfile::TempDir, ForkDb) {
    let dir = tempfile::tempdir().unwrap();
    let db = ForkDb::in_dir(dir.path());
    (dir, db)
}

fn input(trigger: &str, body: &str) -> SnippetInput {
    SnippetInput {
        trigger: trigger.into(),
        body: body.into(),
    }
}

fn add(c: &Connection, trigger: &str, body: &str) -> SnippetSaveOutcome {
    store::add(
        c,
        &clean(&input(trigger, body)).unwrap(),
        SnippetOrigin::User,
        1_000,
    )
    .unwrap()
}

fn saved_id(o: SnippetSaveOutcome) -> i64 {
    match o {
        SnippetSaveOutcome::Saved { snippet } => snippet.id,
        other => panic!("not saved: {other:?}"),
    }
}

// ---------------------------------------------------------------- validation

#[test]
fn triggers_are_tidied_and_limited_to_60_characters() {
    let s = clean(&input("  My   Address ", "x")).unwrap();
    assert_eq!(s.trigger, "My Address");
    assert_eq!(s.norm, "my address");
    assert!(clean(&input(&"é".repeat(60), "x")).is_ok());
    assert_eq!(
        clean(&input(&"é".repeat(61), "x")),
        Err(SnippetProblem::TriggerTooLong)
    );
    assert_eq!(clean(&input("   ", "x")), Err(SnippetProblem::EmptyTrigger));
    assert_eq!(
        clean(&input("-> !", "x")),
        Err(SnippetProblem::TriggerNotSpoken)
    );
    assert_eq!(
        clean(&input("my\naddress", "x")),
        Err(SnippetProblem::TriggerControlCharacter)
    );
    // Turkish İ folds like the matcher folds it.
    assert_eq!(clean(&input("İmza", "x")).unwrap().norm, "imza");
    // NFC: a decomposed trigger is stored composed.
    assert_eq!(clean(&input("cafe\u{301}", "x")).unwrap().trigger, "café");
}

#[test]
fn bodies_keep_their_lines_and_are_limited_to_4000_characters() {
    let s = clean(&input(
        "a",
        "\r\n\n  Alex Morgan\r\n12 Harbour Lane\r\n\r\n\tKiel  24103 \n\n",
    ))
    .unwrap();
    assert_eq!(s.body, "  Alex Morgan\n12 Harbour Lane\n\n\tKiel  24103");
    assert!(clean(&input("a", &"x".repeat(4_000))).is_ok());
    assert_eq!(
        clean(&input("a", &"x".repeat(4_001))),
        Err(SnippetProblem::BodyTooLong)
    );
    assert_eq!(clean(&input("a", " \n\t ")), Err(SnippetProblem::EmptyBody));
    assert_eq!(
        clean(&input("a", "bell\u{7}")),
        Err(SnippetProblem::BodyControlCharacter)
    );
}

// ---------------------------------------------------------------- store

#[test]
fn duplicates_and_conflicts_by_trigger_in_any_case() {
    let (_d, db) = db();
    db.with_conn(|c| {
        let id = saved_id(add(c, "my email", "finn@example.com"));
        assert!(matches!(
            add(c, "My Email", "finn@example.com"),
            SnippetSaveOutcome::Duplicate { existing } if existing.id == id
        ));
        assert!(matches!(
            add(c, "MY  EMAIL", "other@example.com"),
            SnippetSaveOutcome::Conflict { existing } if existing.id == id
        ));
        // Another punctuation is another trigger (inline it must match).
        saved_id(add(c, "my e-mail", "finn@example.com"));
        assert_eq!(store::counts(c)?.snippets, 2);
        Ok(())
    })
    .unwrap();
}

#[test]
fn edits_keep_the_use_count_and_refuse_a_taken_trigger() {
    let (_d, db) = db();
    db.with_conn(|c| {
        let a = saved_id(add(c, "my email", "a@example.com"));
        let b = saved_id(add(c, "my address", "Kiel"));
        c.execute("UPDATE snippets SET hit_count = 4 WHERE id = ?1", [a])?;
        let edited = store::update(
            c,
            a,
            &clean(&input("my mail", "b@example.com")).unwrap(),
            2_000,
        )?;
        let SnippetSaveOutcome::Saved { snippet } = edited else {
            panic!("{edited:?}")
        };
        assert_eq!(
            (
                snippet.trigger.as_str(),
                snippet.hit_count,
                snippet.updated_at
            ),
            ("my mail", 4, 2_000)
        );
        assert!(matches!(
            store::update(c, a, &clean(&input("My Address", "x")).unwrap(), 3_000)?,
            SnippetSaveOutcome::Conflict { existing } if existing.id == b
        ));
        assert_eq!(
            store::update(c, 999, &clean(&input("zz", "x")).unwrap(), 3_000)?,
            SnippetSaveOutcome::Missing
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn delete_and_undo_bring_back_the_same_snippet() {
    let (_d, db) = db();
    db.with_conn(|c| {
        let id = saved_id(add(c, "my address", "12 Harbour Lane\nKiel"));
        c.execute(
            "UPDATE snippets SET hit_count = 2, last_hit_at = 5 WHERE id = ?1",
            [id],
        )?;
        let gone = store::delete(c, &[id])?;
        assert_eq!(gone.len(), 1);
        assert!(store::list(c)?.is_empty());
        assert_eq!(store::restore(c, &gone)?, 1);
        let back = store::get(c, id)?.unwrap();
        assert_eq!(back, gone[0]);
        // A snippet with the same trigger added meanwhile wins.
        store::delete(c, &[id])?;
        saved_id(add(c, "My Address", "other"));
        assert_eq!(store::restore(c, &gone)?, 0);
        Ok(())
    })
    .unwrap();
}

#[test]
fn the_page_holds_at_most_1000_snippets() {
    let (_d, db) = db();
    db.with_conn(|c| {
        for i in 0..MAX_SNIPPETS {
            saved_id(add(c, &format!("snippet {i}"), "x"));
        }
        assert_eq!(
            add(c, "one more", "x"),
            SnippetSaveOutcome::Invalid {
                problem: SnippetProblem::Full
            }
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn for_matching_feeds_the_pipeline_matcher() {
    let (_d, db) = db();
    db.with_conn(|c| {
        saved_id(add(
            c,
            "my address",
            "Alex Morgan\n12 Harbour Lane\nKiel 24103",
        ));
        let set = crate::fork::text::snippets::SnippetSet::new(store::for_matching(c)?);
        let (out, hits) = crate::fork::text::snippets::expand(&set, "My address.");
        assert_eq!(out, "Alex Morgan\n12 Harbour Lane\nKiel 24103");
        assert_eq!(hits.len(), 1);
        Ok(())
    })
    .unwrap();
}

#[test]
fn a_takes_use_is_counted_and_recorded_on_its_history_row() {
    let (_d, db) = db();
    db.with_conn(|c| {
        let id = saved_id(add(c, "my address", "Kiel"));
        crate::fork::db::upsert_entry_meta(
            c,
            7,
            &[
                ("created_at", &1_i64),
                ("final_text", &"Send it to my address."),
                ("text_stages", &r#"[{"stage":"rules","changes":0}]"#),
            ],
        )?;
        let hit = SnippetHit {
            snippet_id: id,
            matched: "my address".into(),
            alone: false,
        };
        store::record_use(
            c,
            &SnippetUse {
                entry_id: Some(7),
                hits: vec![hit.clone(), hit],
                inserted: Some("Send it to Kiel.".into()),
            },
            9_000,
        )?;
        let s = store::get(c, id)?.unwrap();
        assert_eq!((s.hit_count, s.last_hit_at), (2, Some(9_000)));
        assert_eq!(
            store::counts(c)?,
            SnippetCounts {
                snippets: 1,
                uses: 2
            }
        );
        let (stages, inserted, final_text): (String, String, String) = c.query_row(
            "SELECT text_stages, inserted_text, final_text FROM entry_meta WHERE entry_id = 7",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        assert_eq!(
            stages,
            r#"[{"stage":"rules","changes":0},{"stage":"snippets","changes":2}]"#
        );
        assert_eq!(inserted, "Send it to Kiel.");
        // History keeps the spoken words.
        assert_eq!(final_text, "Send it to my address.");
        // A take with no row still counts on the snippet.
        store::record_use(
            c,
            &SnippetUse {
                entry_id: None,
                hits: vec![SnippetHit {
                    snippet_id: id,
                    matched: "my address".into(),
                    alone: true,
                }],
                inserted: None,
            },
            9_500,
        )?;
        assert_eq!(store::get(c, id)?.unwrap().hit_count, 3);
        Ok(())
    })
    .unwrap();
}

#[test]
fn migration_0006_adds_the_columns_to_a_v5_database() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fork.db");
    let old = ForkDb::with_migrations(&path, crate::fork::db::test_migrations(5));
    old.with_conn(|c| {
        c.execute_batch(
            r#"INSERT INTO snippets ("trigger", trigger_norm, body, created_at, updated_at)
               VALUES ('old', 'old', 'Old text', 1, 1);"#,
        )?;
        Ok(())
    })
    .unwrap();
    old.close();
    let db = ForkDb::in_dir(dir.path());
    db.with_conn(|c| {
        let s = store::list(c)?;
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].source, s[0].last_hit_at), (SnippetOrigin::User, None));
        let has_inserted: bool = c.query_row(
            "SELECT COUNT(*) = 1 FROM pragma_table_info('entry_meta') WHERE name = 'inserted_text'",
            [],
            |r| r.get(0),
        )?;
        assert!(has_inserted);
        Ok(())
    })
    .unwrap();
}

// ---------------------------------------------------------------- import

fn row(n: u32, trigger: &str, body: &str) -> SnippetImportRow {
    SnippetImportRow {
        row: n,
        trigger: trigger.into(),
        body: body.into(),
    }
}

#[test]
fn import_review_sorts_rows_and_commit_rechecks() {
    let (_d, db) = db();
    db.with_conn(|c| {
        let existing = saved_id(add(c, "my email", "a@example.com"));
        saved_id(add(c, "my address", "Kiel"));
        let rows = vec![
            row(2, "my sign-off", "Best wishes,\nFinn"),
            row(3, "My Address", "Kiel"),
            row(4, "my email", "b@example.com"),
            row(5, "my sign-off", "again"),
            row(6, "", "no trigger"),
            row(7, "'=calc", "'-1 guarded"),
        ];
        let review = import::review(c, &rows)?;
        assert_eq!(
            review
                .new_snippets
                .iter()
                .map(|r| r.row)
                .collect::<Vec<_>>(),
            [2, 7]
        );
        assert_eq!(review.new_snippets[1].trigger, "=calc");
        assert_eq!(review.new_snippets[1].body, "-1 guarded");
        assert_eq!(review.duplicates.len(), 2);
        assert!(review
            .duplicates
            .iter()
            .any(|d| d.in_file && d.incoming.row == 5));
        assert_eq!(review.conflicts.len(), 1);
        assert_eq!(review.conflicts[0].existing.id, existing);
        assert_eq!(review.invalid[0].problem, SnippetProblem::EmptyTrigger);
        assert_eq!(review.room, (MAX_SNIPPETS - 2) as u32);

        let done = import::commit(c, &rows, &[4], 5_000)?;
        assert_eq!(
            (
                done.added,
                done.replaced,
                done.skipped,
                done.kept,
                done.invalid
            ),
            (2, 1, 2, 0, 1)
        );
        let all = store::list(c)?;
        let sign = all.iter().find(|s| s.trigger == "my sign-off").unwrap();
        assert_eq!(
            (sign.body.as_str(), sign.source),
            ("Best wishes,\nFinn", SnippetOrigin::Import)
        );
        assert_eq!(store::get(c, existing)?.unwrap().body, "b@example.com");
        Ok(())
    })
    .unwrap();
}

#[test]
fn an_import_takes_at_most_1000_rows() {
    let (_d, db) = db();
    db.with_conn(|c| {
        let rows: Vec<_> = (0..1_005)
            .map(|i| row(i + 1, &format!("t {i}"), "x"))
            .collect();
        let review = import::review(c, &rows)?;
        assert_eq!((review.new_snippets.len(), review.ignored_rows), (1_000, 5));
        let done = import::commit(c, &rows, &[], 1)?;
        assert_eq!((done.added, done.over_limit), (1_000, 5));
        Ok(())
    })
    .unwrap();
}

// ---------------------------------------------------------------- export

#[test]
fn export_round_trips_line_breaks_quotes_and_the_formula_guard() {
    let (dir, db) = db();
    let list = db
        .with_conn(|c| {
            saved_id(add(c, "my sign-off", "Best wishes,\n\"Finn\""));
            saved_id(add(c, "minus", "-5 degrees"));
            saved_id(add(c, "my address", "12 Harbour Lane\n\nKiel"));
            store::list(c)
        })
        .unwrap();
    let path = dir.path().join("snippets");
    assert_eq!(export::write_file(&path, &list).unwrap(), 3);
    let bytes = std::fs::read(dir.path().join("snippets.csv")).unwrap();
    assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
    let text = String::from_utf8(bytes[3..].to_vec()).unwrap();
    assert!(text.starts_with("trigger,text\r\n"));
    assert!(text.contains("minus,'-5 degrees\r\n"));
    assert!(text.contains("\"Best wishes,\n\"\"Finn\"\"\""));

    // What the wizard sends back after papaparse reads the file.
    let mut reader = csv::Reader::from_reader(text.as_bytes());
    let rows: Vec<SnippetImportRow> = reader
        .records()
        .enumerate()
        .map(|(i, r)| {
            let r = r.unwrap();
            row(i as u32 + 2, &r[0], &r[1])
        })
        .collect();
    let (_d2, fresh) = super::tests::db();
    fresh
        .with_conn(|c| {
            let done = import::commit(c, &rows, &[], 1)?;
            assert_eq!(done.added, 3);
            let mut back: Vec<(String, String)> = store::list(c)?
                .into_iter()
                .map(|s| (s.trigger, s.body))
                .collect();
            back.sort();
            let mut want: Vec<(String, String)> = list
                .iter()
                .map(|s| (s.trigger.clone(), s.body.clone()))
                .collect();
            want.sort();
            assert_eq!(back, want);
            Ok(())
        })
        .unwrap();
}
