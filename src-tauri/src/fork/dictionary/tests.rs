// SPDX-License-Identifier: GPL-3.0-or-later
//! S-16 tests: validation (≤ 60 characters, apostrophes and non-ASCII),
//! the store (duplicates, conflicts, suggestions, undo), words vs
//! replacements routing, the one-time migration of Handy's custom words and
//! the mirror, the migration 0004 hook, CSV import (formula guard, 3 MB
//! cap, encodings, dedupe, conflicts) and export round trips.

use std::path::Path;

use rusqlite::Connection;
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::{App, Manager};

use super::import::{self, ImportFile, ImportRow};
use super::store::{self, SaveOutcome};
use super::*;
use crate::fork::db::ForkDb;
use crate::fork::settings::FakeHandyStore;
use crate::settings::get_default_settings;

fn db() -> (tempfile::TempDir, ForkDb) {
    let dir = tempfile::tempdir().unwrap();
    let db = ForkDb::in_dir(dir.path());
    (dir, db)
}

fn word(s: &str) -> EntryInput {
    EntryInput {
        phrase: s.into(),
        replacement: None,
    }
}

fn rule(from: &str, to: &str) -> EntryInput {
    EntryInput {
        phrase: from.into(),
        replacement: Some(to.into()),
    }
}

fn add(c: &Connection, input: EntryInput) -> SaveOutcome {
    store::add(c, &clean(&input).unwrap(), EntrySource::User, 1_000).unwrap()
}

fn saved_id(outcome: SaveOutcome) -> i64 {
    match outcome {
        SaveOutcome::Saved { entry } => entry.id,
        other => panic!("not saved: {other:?}"),
    }
}

// ---------------------------------------------------------------- validation

#[test]
fn phrases_are_tidied_and_limited_to_60_characters() {
    let e = clean(&word("  Siobhán \u{00A0}  Ní   Bhriain ")).unwrap();
    assert_eq!(e.phrase, "Siobhán Ní Bhriain");
    assert_eq!(e.kind, EntryKind::Word);
    assert_eq!(e.norm, "siobhán ní bhriain");

    assert!(
        clean(&word(&"é".repeat(60))).is_ok(),
        "60 characters, not bytes"
    );
    assert_eq!(clean(&word(&"a".repeat(61))), Err(EntryProblem::TooLong));
    assert_eq!(clean(&word("   ")), Err(EntryProblem::Empty));
    assert_eq!(
        clean(&word("two\nlines")),
        Err(EntryProblem::ControlCharacter)
    );
    assert_eq!(
        clean(&rule("gonna", &"x".repeat(MAX_REPLACEMENT_CHARS + 1))),
        Err(EntryProblem::ReplacementTooLong)
    );
    assert_eq!(
        clean(&rule("gonna", "gonna")),
        Err(EntryProblem::SameAsReplacement)
    );
}

#[test]
fn a_blank_replacement_makes_a_word_and_case_only_rules_are_valid() {
    assert_eq!(
        clean(&rule("Kubernetes", "  ")).unwrap().kind,
        EntryKind::Word
    );
    let jira = clean(&rule("jira", "Jira")).unwrap();
    assert_eq!(jira.kind, EntryKind::Rule);
    assert_eq!(jira.replacement.as_deref(), Some("Jira"));
}

#[test]
fn apostrophes_are_kept_and_typographic_ones_share_a_key() {
    let straight = clean(&word("O'Brien")).unwrap();
    let curly = clean(&word("O\u{2019}Brien")).unwrap();
    assert_eq!(straight.phrase, "O'Brien");
    assert_eq!(curly.phrase, "O\u{2019}Brien", "spelling kept as typed");
    assert_eq!(straight.norm, curly.norm);
}

// --------------------------------------------------------------------- store

#[test]
fn duplicates_and_conflicts_change_nothing() {
    let (_d, db) = db();
    db.with_conn(|c| {
        let id = saved_id(add(c, word("iPhone")));
        assert!(matches!(add(c, word("iPhone")), SaveOutcome::Duplicate { existing } if existing.id == id));
        assert!(matches!(add(c, word("IPHONE")), SaveOutcome::Conflict { existing } if existing.id == id));

        let r = saved_id(add(c, rule("gonna", "going to")));
        assert!(matches!(add(c, rule("Gonna", "going to")), SaveOutcome::Duplicate { existing } if existing.id == r));
        assert!(matches!(add(c, rule("gonna", "gon' do")), SaveOutcome::Conflict { existing } if existing.id == r));

        // A word and a replacement may share their text.
        saved_id(add(c, word("gonna")));
        assert_eq!(store::counts(c)?, DictionaryCounts { words: 2, rules: 1, suggested: 0 });
        Ok(())
    })
    .unwrap();
}

#[test]
fn adding_a_suggested_or_rejected_entry_makes_it_active() {
    let (_d, db) = db();
    db.with_conn(|c| {
        c.execute_batch(
            "INSERT INTO dictionary_entries (kind, term, norm, source, status, created_at)
                 VALUES ('word', 'Kubernetees', 'kubernetees', 'learn', 'suggested', 1);
             INSERT INTO dictionary_entries (kind, term, norm, source, status, created_at)
                 VALUES ('word', 'Zyx', 'zyx', 'learn', 'rejected', 2);",
        )?;
        assert_eq!(store::counts(c)?.suggested, 1);
        assert_eq!(
            store::list(c)?.len(),
            1,
            "rejected entries are never listed"
        );
        let SaveOutcome::Saved { entry } = add(c, word("Zyx")) else {
            panic!("rejected word not re-added");
        };
        assert_eq!(
            (entry.status, entry.source),
            (EntryStatus::Active, EntrySource::User)
        );
        let SaveOutcome::Saved { entry } = add(c, word("Kubernetees")) else {
            panic!("suggestion not accepted");
        };
        assert_eq!(entry.status, EntryStatus::Active);
        assert_eq!(
            store::counts(c)?,
            DictionaryCounts {
                words: 2,
                rules: 0,
                suggested: 0
            }
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn edits_can_change_the_kind_and_conflict_with_other_entries() {
    let (_d, db) = db();
    db.with_conn(|c| {
        let a = saved_id(add(c, word("Siobhan")));
        let b = saved_id(add(c, rule("gonna", "going to")));
        c.execute(
            "UPDATE dictionary_entries SET hit_count = 4 WHERE id = ?1",
            [b],
        )?;

        // Fix the spelling in place.
        let SaveOutcome::Saved { entry } =
            store::update(c, a, &clean(&word("Siobhán")).unwrap(), 2_000)?
        else {
            panic!()
        };
        assert_eq!(
            (entry.phrase.as_str(), entry.updated_at),
            ("Siobhán", 2_000)
        );

        // A new replacement resets the hit count; same text keeps it.
        let SaveOutcome::Saved { entry } =
            store::update(c, b, &clean(&rule("gonna", "going to")).unwrap(), 3_000)?
        else {
            panic!()
        };
        assert_eq!(entry.hit_count, 4);
        let SaveOutcome::Saved { entry } =
            store::update(c, b, &clean(&rule("gonna", "gonna be")).unwrap(), 3_000)?
        else {
            panic!()
        };
        assert_eq!(entry.hit_count, 0);

        // Word → replacement.
        let SaveOutcome::Saved { entry } =
            store::update(c, a, &clean(&rule("shivawn", "Siobhán")).unwrap(), 4_000)?
        else {
            panic!()
        };
        assert_eq!(entry.kind, EntryKind::Rule);

        // Onto another entry's key: refused.
        assert!(matches!(
            store::update(c, a, &clean(&rule("GONNA", "x")).unwrap(), 5_000)?,
            SaveOutcome::Conflict { existing } if existing.id == b
        ));
        assert_eq!(
            store::update(c, 999, &clean(&word("x")).unwrap(), 1)?,
            SaveOutcome::Missing
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn delete_then_restore_puts_entries_back_as_they_were() {
    let (_d, db) = db();
    db.with_conn(|c| {
        let a = saved_id(add(c, word("Anna-Lena")));
        let b = saved_id(add(c, rule("wispr bro", "Wispr Bro")));
        c.execute(
            "UPDATE dictionary_entries SET hit_count = 9 WHERE id = ?1",
            [b],
        )?;
        let before = store::list(c)?;
        let gone = store::delete(c, &[a, b, 12345])?;
        assert_eq!(gone.len(), 2);
        assert!(store::list(c)?.is_empty());
        assert_eq!(store::restore(c, &gone)?, 2);
        assert_eq!(store::list(c)?, before);
        // Restoring again does nothing (the keys are taken).
        assert_eq!(store::restore(c, &gone)?, 0);
        Ok(())
    })
    .unwrap();
}

#[test]
fn the_dictionary_is_full_at_max_entries() {
    let (_d, db) = db();
    db.with_conn(|c| {
        let tx = c.transaction()?;
        for i in 0..MAX_ENTRIES {
            tx.execute(
                "INSERT INTO dictionary_entries (kind, term, norm, created_at) VALUES ('word', ?1, ?1, 1)",
                [format!("w{i}")],
            )?;
        }
        tx.commit()?;
        assert_eq!(
            add(c, word("one more")),
            SaveOutcome::Invalid { problem: EntryProblem::Full }
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn words_feed_the_recognizer_and_replacements_feed_the_rule_stage() {
    let (_d, db) = db();
    db.with_conn(|c| {
        add(c, word("Siobhán"));
        add(c, rule("gonna", "going to"));
        add(c, word("Kubernetes"));
        c.execute_batch(
            "INSERT INTO dictionary_entries (kind, from_text, to_text, norm, status, created_at)
             VALUES ('rule', 'a', 'b', 'a', 'suggested', 3);",
        )?;
        assert_eq!(store::active_words(c)?, ["Siobhán", "Kubernetes"]);
        let rules = crate::fork::db::rules::active_rules(c)?;
        assert_eq!(
            rules
                .iter()
                .map(|r| (r.from.as_str(), r.to.as_str()))
                .collect::<Vec<_>>(),
            [("gonna", "going to")]
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn a_dictionary_entry_changes_the_pipeline_output() {
    let (_d, db) = db();
    let rules = db
        .with_conn(|c| {
            add(c, rule("gonna", "going to"));
            crate::fork::db::rules::active_rules(c)
        })
        .unwrap();
    let set = crate::fork::text::rules::RuleSet::new(rules);
    let (out, applied) =
        crate::fork::text::rules::apply(&set, "I'm gonna call Siobhán", |r| r.to.clone());
    assert_eq!(out, "I'm going to call Siobhán");
    assert_eq!(applied.len(), 1);
}

#[test]
fn rows_written_without_a_key_get_one_and_duplicates_go() {
    let (_d, db) = db();
    db.with_conn(|c| {
        c.execute_batch(
            "INSERT INTO dictionary_entries (kind, from_text, to_text, created_at) VALUES ('rule', 'Wispr  bro', 'Wispr Bro', 1);
             INSERT INTO dictionary_entries (kind, from_text, to_text, created_at) VALUES ('rule', 'wispr bro', 'X', 2);",
        )?;
        store::ensure_norms(c)?;
        let rules = crate::fork::db::rules::active_rules(c)?;
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].to, "Wispr Bro", "the older rule wins (S-11)");
        Ok(())
    })
    .unwrap();
}

// ------------------------------------------------------- migration 0004 hook

#[test]
fn migration_v4_keys_older_rows_and_makes_the_key_unique() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fork.db");
    {
        // A v3 database (S-11) with rows the old code wrote.
        let v3 =
            crate::fork::db::ForkDb::with_migrations(&path, crate::fork::db::test_migrations(3));
        v3.with_conn(|c| {
            c.execute_batch(
                "INSERT INTO dictionary_entries (kind, from_text, to_text, created_at) VALUES ('rule', 'ÉCOLE', 'école', 1);
                 INSERT INTO dictionary_entries (kind, from_text, to_text, created_at) VALUES ('rule', 'école', 'Ecole', 2);
                 INSERT INTO dictionary_entries (kind, term, created_at) VALUES ('word', 'Siobhán', 3);",
            )?;
            Ok(())
        })
        .unwrap();
        v3.close();
    }
    let db = ForkDb::new(&path);
    db.with_conn(|c| {
        let rows: Vec<(String, Option<String>, i64)> = c
            .prepare("SELECT kind, norm, updated_at FROM dictionary_entries ORDER BY id")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<_, _>>()?;
        assert_eq!(
            rows,
            [
                ("rule".to_string(), Some("école".to_string()), 1),
                ("word".to_string(), Some("siobhán".to_string()), 3)
            ]
        );
        let dup = c.execute(
            "INSERT INTO dictionary_entries (kind, term, norm, created_at) VALUES ('word', 'SIOBHÁN', 'siobhán', 4)",
            [],
        );
        assert!(dup.is_err(), "the key is unique per kind");
        Ok(())
    })
    .unwrap();
}

// ----------------------------------------------------- Handy's custom words

fn mock_app(dir: &Path, handy_words: &[&str]) -> App<MockRuntime> {
    let mut handy = get_default_settings();
    handy.custom_words = handy_words.iter().map(|s| s.to_string()).collect();
    mock_builder()
        // Managed before the fork plugin's setup, which then keeps them.
        .manage(ForkDb::in_dir(dir))
        .manage(FakeHandyStore::with(handy))
        .manage(crate::fork::settings::ForkSettingsHandle::new(
            dir.join("fork_settings.json"),
        ))
        .plugin(crate::fork::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .build(mock_context(noop_assets()))
        .expect("mock app")
}

fn handy_words(app: &App<MockRuntime>) -> Vec<String> {
    app.state::<FakeHandyStore>().get().unwrap().custom_words
}

fn dictionary_words(app: &App<MockRuntime>) -> Vec<(String, EntrySource)> {
    app.state::<ForkDb>()
        .with_conn(|c| store::list(c))
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == EntryKind::Word)
        .map(|e| (e.phrase, e.source))
        .collect()
}

#[test]
fn handy_custom_words_are_migrated_once_and_idempotently() {
    let dir = tempfile::tempdir().unwrap();
    let app = mock_app(dir.path(), &["Handy", "O'Brien", "Siobhán", "handy"]);
    let report = migrate::sync(app.handle()).unwrap();
    assert_eq!(
        report.taken_in, 3,
        "\"handy\" repeats \"Handy\" with another case"
    );
    assert!(report.wrote_handy, "Handy's list loses the case duplicate");
    let mut words = dictionary_words(&app);
    words.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        words,
        [
            ("Handy".to_string(), EntrySource::Migrated),
            ("O'Brien".to_string(), EntrySource::Migrated),
            ("Siobhán".to_string(), EntrySource::Migrated)
        ]
    );
    assert_eq!(handy_words(&app), ["Handy", "O'Brien", "Siobhán"]);

    // Again (every launch): nothing changes.
    let again = migrate::sync(app.handle()).unwrap();
    assert_eq!(again, migrate::SyncReport::default());
    assert_eq!(dictionary_words(&app).len(), 3);
}

#[test]
fn a_word_deleted_in_the_dictionary_is_not_taken_back_from_handy() {
    let dir = tempfile::tempdir().unwrap();
    let app = mock_app(dir.path(), &["Handy", "Kubernetes"]);
    migrate::sync(app.handle());
    let id = app
        .state::<ForkDb>()
        .with_conn(|c| store::list(c))
        .unwrap()
        .into_iter()
        .find(|e| e.phrase == "Handy")
        .unwrap()
        .id;
    crate::fork::commands::dictionary::fork_dictionary_delete(app.handle().clone(), vec![id])
        .unwrap();
    assert_eq!(handy_words(&app), ["Kubernetes"]);
    migrate::sync(app.handle());
    assert_eq!(dictionary_words(&app).len(), 1);
}

#[test]
fn words_added_or_removed_in_handys_own_settings_follow() {
    let dir = tempfile::tempdir().unwrap();
    let app = mock_app(dir.path(), &["Kubernetes"]);
    migrate::sync(app.handle());
    // The user edits the list in Settings › Advanced.
    let store_ = app.state::<FakeHandyStore>();
    let mut s = store_.get().unwrap();
    s.custom_words = vec!["Postgres".into()];
    store_.set(s);
    let report = migrate::sync(app.handle()).unwrap();
    assert_eq!((report.taken_in, report.removed), (1, 1));
    assert_eq!(
        dictionary_words(&app),
        [("Postgres".to_string(), EntrySource::Migrated)]
    );
}

#[test]
fn page_commands_mirror_words_but_never_replacements_into_handy() {
    use crate::fork::commands::dictionary as cmd;
    let dir = tempfile::tempdir().unwrap();
    let app = mock_app(dir.path(), &[]);
    let h = app.handle().clone();
    let out = cmd::fork_dictionary_add(h.clone(), word("Siobhán")).unwrap();
    assert!(matches!(out, SaveOutcome::Saved { .. }));
    cmd::fork_dictionary_add(h.clone(), rule("gonna", "going to")).unwrap();
    assert_eq!(handy_words(&app), ["Siobhán"]);
    let list = cmd::fork_dictionary_list(h.clone()).unwrap();
    assert_eq!(
        list.counts,
        DictionaryCounts {
            words: 1,
            rules: 1,
            suggested: 0
        }
    );
    let invalid = cmd::fork_dictionary_add(h.clone(), word(&"x".repeat(61))).unwrap();
    assert_eq!(
        invalid,
        SaveOutcome::Invalid {
            problem: EntryProblem::TooLong
        }
    );

    let siobhan = list
        .entries
        .iter()
        .find(|e| e.kind == EntryKind::Word)
        .unwrap();
    cmd::fork_dictionary_update(h.clone(), siobhan.id, word("Siobhán Ní Bhriain")).unwrap();
    assert_eq!(handy_words(&app), ["Siobhán Ní Bhriain"]);
    let gone = cmd::fork_dictionary_delete(h.clone(), vec![siobhan.id]).unwrap();
    assert!(handy_words(&app).is_empty());
    assert_eq!(cmd::fork_dictionary_restore(h.clone(), gone).unwrap(), 1);
    assert_eq!(handy_words(&app), ["Siobhán Ní Bhriain"]);
}

// -------------------------------------------------------------------- import

fn row(n: u32, phrase: &str, replacement: Option<&str>) -> ImportRow {
    ImportRow {
        row: n,
        phrase: phrase.into(),
        replacement: replacement.map(Into::into),
    }
}

#[test]
fn files_must_be_csv_and_at_most_3_mb() {
    let dir = tempfile::tempdir().unwrap();
    let txt = dir.path().join("words.txt");
    std::fs::write(&txt, "a").unwrap();
    assert_eq!(import::read_file(&txt), ImportFile::NotCsv);
    let big = dir.path().join("big.csv");
    std::fs::write(&big, vec![b'a'; (import::MAX_FILE_BYTES + 1) as usize]).unwrap();
    assert_eq!(
        import::read_file(&big),
        ImportFile::TooLarge {
            bytes: import::MAX_FILE_BYTES + 1
        }
    );
    let ok = dir.path().join("Words.CSV");
    std::fs::write(&ok, vec![b'a'; import::MAX_FILE_BYTES as usize]).unwrap();
    assert!(matches!(import::read_file(&ok), ImportFile::Read { name, .. } if name == "Words.CSV"));
    assert_eq!(
        import::read_file(&dir.path().join("missing.csv")),
        ImportFile::Unreadable
    );
}

#[test]
fn files_decode_from_utf8_utf16_and_windows_1252() {
    assert_eq!(import::decode(b"\xEF\xBB\xBFSiobh\xC3\xA1n"), "Siobhán");
    assert_eq!(import::decode("Siobhán".as_bytes()), "Siobhán");
    let utf16: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain("Siobhán".encode_utf16().flat_map(|u| u.to_le_bytes()))
        .collect();
    assert_eq!(import::decode(&utf16), "Siobhán");
    assert_eq!(import::decode(b"Siobh\xE1n"), "Siobhán", "Windows-1252");
}

#[test]
fn the_formula_guard_is_undone_on_import() {
    assert_eq!(import::unguard("'=SUM(A1)"), "=SUM(A1)");
    assert_eq!(import::unguard("'+1"), "+1");
    assert_eq!(import::unguard("'-ish"), "-ish");
    assert_eq!(import::unguard("'@here"), "@here");
    assert_eq!(
        import::unguard("'tis"),
        "'tis",
        "an ordinary apostrophe stays"
    );
    assert_eq!(export::guard("=SUM(A1)"), "'=SUM(A1)");
    assert_eq!(export::guard("@here"), "'@here");
    assert_eq!(export::guard("Siobhán"), "Siobhán");
}

#[test]
fn review_sorts_rows_and_commit_honours_the_choices() {
    let (_d, db) = db();
    db.with_conn(|c| {
        add(c, word("Kubernetes"));
        add(c, rule("gonna", "going to"));
        add(c, rule("wanna", "want to"));
        let rows = vec![
            row(2, "Siobhán", None),
            row(3, "kubernetes", Some("")),    // conflict: spelling
            row(4, "Gonna", Some("going to")), // duplicate of the dictionary
            row(5, "wanna", Some("wants to")), // conflict: replacement
            row(6, "siobhán", None),           // repeats row 2
            row(7, &"x".repeat(61), None),     // too long
            row(8, "'=HYPERLINK(1)", None),    // guarded formula → literal
            row(9, "same", Some("same")),      // invalid
            row(10, "O'Brien", Some("")),
        ];
        let review = import::review(c, &rows)?;
        let rows_of = |v: &[import::ReviewRow]| v.iter().map(|r| r.row).collect::<Vec<_>>();
        assert_eq!(rows_of(&review.new_entries), [2, 8, 10]);
        assert_eq!(review.new_entries[1].phrase, "=HYPERLINK(1)");
        assert_eq!(
            review
                .duplicates
                .iter()
                .map(|d| (d.incoming.row, d.in_file))
                .collect::<Vec<_>>(),
            [(4, false), (6, true)]
        );
        assert_eq!(
            review
                .conflicts
                .iter()
                .map(|x| x.incoming.row)
                .collect::<Vec<_>>(),
            [3, 5]
        );
        assert_eq!(
            review
                .invalid
                .iter()
                .map(|i| (i.row, i.problem))
                .collect::<Vec<_>>(),
            [
                (7, EntryProblem::TooLong),
                (9, EntryProblem::SameAsReplacement)
            ]
        );
        assert_eq!(review.room as usize, MAX_ENTRIES - 3);

        let outcome = import::commit(c, &rows, &[5], 9_000)?;
        assert_eq!(
            outcome,
            import::ImportOutcome {
                added: 3,
                replaced: 1,
                skipped: 2,
                kept: 1,
                invalid: 2,
                over_limit: 0
            }
        );
        let rules = crate::fork::db::rules::active_rules(c)?;
        assert!(rules
            .iter()
            .any(|r| r.from == "wanna" && r.to == "wants to"));
        let words = store::active_words(c)?;
        assert_eq!(words, ["Kubernetes", "Siobhán", "=HYPERLINK(1)", "O'Brien"]);
        let sources: Vec<EntrySource> = store::list(c)?
            .into_iter()
            .filter(|e| e.phrase == "Siobhán")
            .map(|e| e.source)
            .collect();
        assert_eq!(sources, [EntrySource::Import]);
        // Importing the same file again adds nothing.
        let again = import::commit(c, &rows, &[], 9_500)?;
        assert_eq!(again.added, 0);
        Ok(())
    })
    .unwrap();
}

#[test]
fn import_stops_at_the_limit() {
    let (_d, db) = db();
    db.with_conn(|c| {
        let tx = c.transaction()?;
        for i in 0..MAX_ENTRIES - 1 {
            tx.execute(
                "INSERT INTO dictionary_entries (kind, term, norm, created_at) VALUES ('word', ?1, ?1, 1)",
                [format!("w{i}")],
            )?;
        }
        tx.commit()?;
        let rows = vec![row(1, "a1", None), row(2, "a2", None)];
        assert_eq!(import::review(c, &rows)?.room, 1);
        let out = import::commit(c, &rows, &[], 1)?;
        assert_eq!((out.added, out.over_limit), (1, 1));
        Ok(())
    })
    .unwrap();
}

// -------------------------------------------------------------------- export

#[test]
fn export_round_trips_through_import() {
    let (dir, db) = db();
    let entries = db
        .with_conn(|c| {
            add(c, word("Siobhán"));
            add(c, rule("gonna", "going to"));
            add(c, word("=cmd|' /C calc'!A0"));
            add(c, rule("say, \"hi\"", "Hi"));
            store::list(c)
        })
        .unwrap();
    let path = dir.path().join("my words");
    let n = export::write_file(&path, &entries).unwrap();
    assert_eq!(n, 4);
    let written = std::fs::read(dir.path().join("my words.csv")).unwrap();
    assert!(
        written.starts_with(&[0xEF, 0xBB, 0xBF]),
        "UTF-8 BOM for spreadsheets"
    );
    let text = import::decode(&written);
    assert!(text.starts_with("phrase,replacement\r\n"));
    assert!(text.contains("'=cmd"), "formula guarded: {text}");

    let mut reader = csv::Reader::from_reader(text.as_bytes());
    let rows: Vec<ImportRow> = reader
        .records()
        .enumerate()
        .map(|(i, r)| {
            let r = r.unwrap();
            row(i as u32 + 2, &r[0], Some(&r[1]))
        })
        .collect();
    let review = db.with_conn(|c| import::review(c, &rows)).unwrap();
    assert!(
        review.new_entries.is_empty() && review.conflicts.is_empty() && review.invalid.is_empty()
    );
    assert_eq!(review.duplicates.len(), 4);
}
