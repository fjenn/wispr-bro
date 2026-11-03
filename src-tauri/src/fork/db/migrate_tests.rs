// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for the schema-aware migration runner ([`super::migrate`]): the
//! statement guard, databases whose schema is ahead of (or out of step with)
//! their `user_version` because of the renumberings in the git history, a
//! half-applied migration, one backup per version step and concurrent first
//! opens.

use std::sync::{Arc, Barrier};
use std::thread;

use super::migrate::{classify, expected_schemas, split_statements, Schema, Target};
use super::*;

// The migration files, by their current number.
const INIT: &str = include_str!("migrations/0001_init.sql");
const HISTORY: &str = include_str!("migrations/0002_history.sql");
const TEXT: &str = include_str!("migrations/0003_text.sql");
const DICTIONARY: &str = include_str!("migrations/0004_dictionary.sql");
const NOTIFICATIONS: &str = include_str!("migrations/0005_notifications.sql");
const SNIPPETS: &str = include_str!("migrations/0006_snippets.sql");
const MODELS: &str = include_str!("migrations/0007_models.sql");
const APP_RULES: &str = include_str!("migrations/0008_app_rules.sql");

fn dictionary() -> M {
    M::up_with_hook(DICTIONARY, crate::fork::dictionary::store::migrate_v4)
}

/// Every migration chain a build on some branch shipped, from
/// `git log --all --reflog` and the unreachable rebase commits of
/// `src-tauri/src/fork/db/mod.rs` (the SQL of each file never changed; only
/// its number did). Each entry: the commit, then the chain.
fn historical_chains() -> Vec<(&'static str, Vec<M>)> {
    let m = M::up;
    vec![
        ("84a92433 F-06", vec![m(INIT)]),
        ("00a52369 S-09", vec![m(INIT), m(HISTORY)]),
        ("5e5474d0 S-11", vec![m(INIT), m(HISTORY), m(TEXT)]),
        // S-18 before its rebase onto S-11: notifications as v3.
        ("623f40da S-18", vec![m(INIT), m(HISTORY), m(NOTIFICATIONS)]),
        (
            "1fb2f486 S-16",
            vec![m(INIT), m(HISTORY), m(TEXT), dictionary()],
        ),
        // S-18 rebased onto S-11 only: notifications as v4.
        (
            "1c71628e S-18",
            vec![m(INIT), m(HISTORY), m(TEXT), m(NOTIFICATIONS)],
        ),
        // S-17 before S-18 merged: snippets as v5.
        (
            "41a8c4e0 S-17",
            vec![m(INIT), m(HISTORY), m(TEXT), dictionary(), m(SNIPPETS)],
        ),
        (
            "018673c6 S-16+S-18",
            vec![m(INIT), m(HISTORY), m(TEXT), dictionary(), m(NOTIFICATIONS)],
        ),
        (
            "07778491 S-17",
            vec![
                m(INIT),
                m(HISTORY),
                m(TEXT),
                dictionary(),
                m(NOTIFICATIONS),
                m(SNIPPETS),
            ],
        ),
        (
            "03a26856 S-07b",
            vec![
                m(INIT),
                m(HISTORY),
                m(TEXT),
                dictionary(),
                m(NOTIFICATIONS),
                m(SNIPPETS),
                m(MODELS),
            ],
        ),
        // S-19a before S-07b merged: app rules as v7.
        (
            "05e7bb95 S-19a",
            vec![
                m(INIT),
                m(HISTORY),
                m(TEXT),
                dictionary(),
                m(NOTIFICATIONS),
                m(SNIPPETS),
                m(APP_RULES),
            ],
        ),
        (
            "025749cd S-19a",
            vec![
                m(INIT),
                m(HISTORY),
                m(TEXT),
                dictionary(),
                m(NOTIFICATIONS),
                m(SNIPPETS),
                m(MODELS),
                m(APP_RULES),
            ],
        ),
        ("0061289c S-21", MIGRATIONS.to_vec()),
    ]
}

/// What the old runner (`rusqlite_migration`) did with `chain` on an
/// existing file: everything after its `user_version` in one transaction,
/// rolled back on the first error.
fn old_runner(path: &Path, chain: &[M]) {
    let mut conn = Connection::open(path).unwrap();
    let found = user_version(&conn).unwrap();
    if found >= chain.len() {
        return;
    }
    let tx = conn.transaction().unwrap();
    for m in &chain[found..] {
        if tx.execute_batch(m.sql).is_err() {
            return; // dropped: rolled back
        }
        if let Some(hook) = m.hook {
            if hook(&tx).is_err() {
                return;
            }
        }
    }
    tx.pragma_update(None, "user_version", chain.len() as i64)
        .unwrap();
    tx.commit().unwrap();
}

/// A test migration that applies to an empty database (so its expected
/// schema can be built) but fails on a file with an `entry_meta` row.
fn failing_on_data(before: &'static str) -> M {
    let sql: &'static str = Box::leak(
        format!(
            "{before}\nINSERT INTO entry_meta (entry_id, created_at) \
             SELECT entry_id + 1000, NULL FROM entry_meta;"
        )
        .into_boxed_str(),
    );
    M::up(sql)
}

fn backups(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = aside_files(dir, BACKUP_PREFIX)
        .unwrap()
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "db"))
        .collect();
    files.sort();
    files
}

/// Writes one row the way every chain's schema allows (v1 columns only).
fn seed(path: &Path) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(
        "INSERT INTO entry_meta (entry_id, created_at, final_text) VALUES (42, 1, 'kept');
         INSERT INTO notes (title, body, created_at, updated_at) VALUES ('n', 'note kept', 1, 1);",
    )
    .unwrap();
}

/// Uses every table the current build writes, the way the product does.
fn exercise_every_table(conn: &Connection) -> Result<(), DbError> {
    use crate::fork::dictionary::{self, store, EntryInput, EntrySource};
    // Dictionary add (the owner's failing case), through the store.
    store::ensure_norms(conn)?;
    let entry = dictionary::clean(&EntryInput {
        phrase: "Siobhán".into(),
        replacement: None,
    })
    .unwrap();
    let outcome = store::add(conn, &entry, EntrySource::User, 10)?;
    assert!(
        matches!(outcome, store::SaveOutcome::Saved { .. }),
        "{outcome:?}"
    );
    assert_eq!(store::list(conn)?.len(), 1);
    // Snippets with v6's columns.
    let snippet = crate::fork::snippets::clean(&crate::fork::snippets::SnippetInput {
        trigger: "my address".into(),
        body: "Kiel".into(),
    })
    .unwrap();
    crate::fork::snippets::store::add(
        conn,
        &snippet,
        crate::fork::snippets::SnippetOrigin::User,
        10,
    )?;
    conn.execute("UPDATE snippets SET last_hit_at = 11", [])?;
    // History: v2, v3 and v6 columns, the paste outcome.
    upsert_entry_meta(
        conn,
        43,
        &[
            ("created_at", &2_i64),
            ("day", &"2026-11-03"),
            ("text_stages", &"{}"),
            ("inserted_text", &"x"),
            ("paste_outcome", &"confirmed"),
        ],
    )?;
    // Notifications (v5), models (v7), app rules (v8), transforms (v9).
    conn.execute(
        "INSERT INTO notifications (kind, title_key, created_at, dedupe_key) VALUES ('k', 't', 1, 'd')",
        [],
    )?;
    conn.execute(
        "INSERT INTO speech_models (handy_id, origin, added_at) VALUES ('m', 'starter', 1)",
        [],
    )?;
    conn.execute(
        "INSERT INTO model_policy_state (key, value) VALUES ('k', 'v')",
        [],
    )?;
    crate::fork::context::app_rules::set_auto_send(conn, "com.example.a", None, true, 1)?;
    let builtins: i64 = conn.query_row(
        "SELECT COUNT(*) FROM transforms WHERE builtin_key IN ('polish', 'prompt_engineer')",
        [],
        |r| r.get(0),
    )?;
    assert_eq!(builtins, 2);
    conn.execute(
        "INSERT INTO queued_takes (wav_path, created_at) VALUES ('a.wav', 1)",
        [],
    )?;
    Ok(())
}

/// Opens `path` with this build and checks the result: latest version, the
/// expected schema, the seeded rows kept, every table usable.
fn assert_repaired(path: &Path, label: &str) {
    let db = ForkDb::new(path);
    db.with_conn(|conn| {
        assert_eq!(user_version(conn)?, latest_version(), "{label}");
        let expected = expected_schemas(&MIGRATIONS).unwrap();
        let missing = expected[latest_version()].missing_in(&Schema::read(conn)?);
        assert!(missing.is_empty(), "{label}: still missing {missing:?}");
        let kept: String = conn.query_row(
            "SELECT final_text FROM entry_meta WHERE entry_id = 42",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(kept, "kept", "{label}");
        let note: String = conn.query_row("SELECT body FROM notes", [], |r| r.get(0))?;
        assert_eq!(note, "note kept", "{label}");
        exercise_every_table(conn)
    })
    .unwrap_or_else(|e| panic!("{label}: {e}"));
    db.close();
    // A second open has nothing left to do.
    let again = ForkDb::new(path);
    again.open_now().unwrap();
}

#[test]
fn statements_split_whole_and_classify() {
    let all: Vec<&str> = MIGRATIONS
        .iter()
        .flat_map(|m| split_statements(m.sql))
        .collect();
    let trigger = all
        .iter()
        .find(|s| s.contains("CREATE TRIGGER"))
        .expect("0001's trigger");
    assert!(trigger.trim_end().ends_with("END;"), "{trigger}");
    for stmt in &all {
        let target = classify(stmt);
        let head = stmt
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty() && !l.starts_with("--"))
            .unwrap();
        if head.starts_with("ALTER") || head.starts_with("CREATE") || head.starts_with("DROP") {
            assert_ne!(target, Target::Other, "{stmt}");
        }
    }
    assert_eq!(
        classify("-- c\nALTER TABLE snippets ADD COLUMN last_hit_at INTEGER;"),
        Target::AddColumn {
            table: "snippets".into(),
            column: "last_hit_at".into()
        }
    );
    assert_eq!(
        classify("ALTER TABLE main.\"Entry_Meta\" ADD inserted_text TEXT"),
        Target::AddColumn {
            table: "entry_meta".into(),
            column: "inserted_text".into()
        }
    );
    assert_eq!(
        classify("CREATE UNIQUE INDEX IF NOT EXISTS x_y ON t (a)"),
        Target::Create {
            kind: "index",
            name: "x_y".into()
        }
    );
    assert_eq!(
        classify("CREATE VIRTUAL TABLE notes_fts USING fts5 (title)"),
        Target::Create {
            kind: "table",
            name: "notes_fts".into()
        }
    );
    assert_eq!(
        classify("DROP TABLE IF EXISTS app_rules"),
        Target::Drop {
            kind: "table",
            name: "app_rules".into()
        }
    );
    assert_eq!(classify("INSERT INTO t VALUES (1)"), Target::Other);
    // A semicolon inside a string does not split.
    assert_eq!(
        split_statements("INSERT INTO t VALUES ('a;b'); -- tail\n").len(),
        1
    );
}

#[test]
fn a_fresh_database_has_exactly_the_expected_schema() {
    let dir = tempfile::tempdir().unwrap();
    let db = ForkDb::in_dir(dir.path());
    let expected = expected_schemas(&MIGRATIONS).unwrap();
    db.with_conn(|conn| {
        assert_eq!(Schema::read(conn)?, expected[latest_version()]);
        Ok(())
    })
    .unwrap();
}

/// The owner's alpha database: `user_version` 5 with every v6 column (the
/// rc2 build failed on it with `duplicate column name: last_hit_at`).
#[test]
fn a_v5_database_with_the_v6_columns_already_present_is_repaired() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    let old = ForkDb::with_migrations(&path, test_migrations(5));
    old.open_now().unwrap();
    old.close();
    seed(&path);
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(SNIPPETS).unwrap();
        assert_eq!(user_version(&conn).unwrap(), 5);
    }
    // The old runner fails on it, as the owner's build did.
    {
        let mut conn = Connection::open(&path).unwrap();
        let tx = conn.transaction().unwrap();
        let err = tx.execute_batch(SNIPPETS).unwrap_err().to_string();
        assert!(err.contains("duplicate column name: last_hit_at"), "{err}");
    }
    assert_repaired(&path, "v5 + v6 columns");
    assert_eq!(backups(dir.path()).len(), 1, "one backup of the v5 file");
}

/// The exact shape of the owner's file: built by an S-18 branch build (v3 =
/// notifications), then an S-17 branch build (v4 = dictionary, v5 =
/// snippets). It has v6's columns and lacks v3's `entry_meta.text_stages`.
#[test]
fn the_owners_mixed_dev_history_is_repaired() {
    let chains = historical_chains();
    let chain = |id: &str| {
        chains
            .iter()
            .find(|(c, _)| c.starts_with(id))
            .unwrap()
            .1
            .clone()
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    old_runner(&path, &chain("623f40da"));
    seed(&path);
    old_runner(&path, &chain("41a8c4e0"));
    {
        let conn = Connection::open(&path).unwrap();
        assert_eq!(user_version(&conn).unwrap(), 5);
        let cols: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('entry_meta')")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(cols.contains(&"inserted_text".to_owned()));
        assert!(!cols.contains(&"text_stages".to_owned()));
    }
    assert_repaired(&path, "owner");
}

#[test]
fn every_historical_numbering_is_repaired_forward() {
    let chains = historical_chains();
    for (label, chain) in &chains {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        old_runner(&path, chain);
        seed(&path);
        assert_repaired(&path, label);
    }
}

/// A file built by one dev build and then opened by another (the alpha
/// testers' case), for every ordered pair of chains.
#[test]
fn every_pair_of_historical_builds_is_repaired_forward() {
    let chains = historical_chains();
    for (a, first) in &chains {
        for (b, second) in &chains {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join(DB_FILE_NAME);
            old_runner(&path, first);
            seed(&path);
            old_runner(&path, second);
            assert_repaired(&path, &format!("{a} then {b}"));
        }
    }
}

/// A migration that stopped halfway (written by a runner without one
/// transaction per migration): only its first statement is in the file.
#[test]
fn a_half_applied_migration_is_completed() {
    for (version, first_statement) in [
        (6, "ALTER TABLE snippets ADD COLUMN last_hit_at INTEGER;"),
        (4, "ALTER TABLE dictionary_entries ADD COLUMN norm TEXT;"),
        (2, "ALTER TABLE entry_meta ADD COLUMN day TEXT;"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        let old = ForkDb::with_migrations(&path, test_migrations(version - 1));
        old.open_now().unwrap();
        old.close();
        seed(&path);
        Connection::open(&path)
            .unwrap()
            .execute_batch(first_statement)
            .unwrap();
        assert_repaired(&path, &format!("half of v{version}"));
    }
}

/// A migration that fails is rolled back whole: the schema and
/// `user_version` are as before, and the next open can retry.
#[test]
fn a_failing_migration_leaves_nothing_half_done() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    ForkDb::new(&path).open_now().unwrap();
    seed(&path);
    let mut broken = MIGRATIONS.to_vec();
    broken.push(failing_on_data(
        "ALTER TABLE entry_meta ADD COLUMN test_a TEXT;",
    ));
    let err = ForkDb::with_migrations(&path, broken)
        .open_now()
        .unwrap_err();
    assert!(
        matches!(&err, DbError::Migration(f) if f.version == latest_version() + 1),
        "{err:?}"
    );
    let conn = Connection::open(&path).unwrap();
    assert_eq!(user_version(&conn).unwrap(), latest_version());
    let has_a: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('entry_meta') WHERE name = 'test_a'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        !has_a,
        "the failed migration's first statement was rolled back"
    );
}

/// Failed attempts (every fork.db caller retries the open) make one backup
/// per version step, not one per attempt.
#[test]
fn repeated_failed_attempts_make_one_backup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    ForkDb::new(&path).open_now().unwrap();
    seed(&path);
    let mut broken = MIGRATIONS.to_vec();
    broken.push(failing_on_data(""));
    let db = ForkDb::with_migrations(&path, broken);
    for _ in 0..5 {
        assert!(db.open_now().is_err());
    }
    let made = backups(dir.path());
    assert_eq!(made.len(), 1, "{made:?}");
    // A write since then is worth a new backup.
    thread::sleep(Duration::from_millis(20));
    ForkDb::new(&path)
        .with_conn(|c| upsert_entry_meta(c, 1, &[("created_at", &1_i64)]))
        .unwrap();
    {
        // Fold the write into the main file, as a later launch would.
        let conn = Connection::open(&path).unwrap();
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
            .unwrap();
    }
    assert!(db.open_now().is_err());
    assert_eq!(backups(dir.path()).len(), 2);
}

/// Several handles (threads here; processes in the field) opening an old
/// file at once: one migrates and backs up, the others wait and find it
/// done.
#[test]
fn concurrent_first_opens_wait_on_one_migration() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    let old = ForkDb::with_migrations(&path, test_migrations(5));
    old.open_now().unwrap();
    old.close();
    seed(&path);
    let n = 6;
    let barrier = Arc::new(Barrier::new(n));
    let handles: Vec<_> = (0..n)
        .map(|_| {
            let path = path.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let db = ForkDb::new(&path);
                barrier.wait();
                db.with_conn(|c| user_version(c))
            })
        })
        .collect();
    for h in handles {
        assert_eq!(h.join().unwrap().unwrap(), latest_version());
    }
    let made = backups(dir.path());
    assert_eq!(made.len(), 1, "{made:?}");
    assert_repaired(&path, "concurrent");
}

/// A schema that is ahead keeps the extra objects' data: the v8 app rules a
/// v7-numbered build wrote are not dropped by 0008's rebuild.
#[test]
fn a_rebuild_migration_is_not_rerun_over_its_own_result() {
    let chains = historical_chains();
    let s19a = chains
        .iter()
        .find(|(c, _)| c.starts_with("05e7bb95"))
        .unwrap()
        .1
        .clone();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    old_runner(&path, &s19a);
    seed(&path);
    Connection::open(&path)
        .unwrap()
        .execute(
            "INSERT INTO app_rules (\"match\", kind, style_group, auto_send) VALUES ('com.kept', 'app', 'work', 0)",
            [],
        )
        .unwrap();
    assert_repaired(&path, "S-19a v7");
    let conn = Connection::open(&path).unwrap();
    let kept: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM app_rules WHERE \"match\" = 'com.kept'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(kept, 1);
}

/// `WB_OWNER_DB_COPY=<copy of a real fork.db>`: migrates a scratch copy of
/// it (never the file itself) and uses every table. Run by hand.
#[test]
#[ignore = "needs WB_OWNER_DB_COPY"]
fn migrates_a_copy_of_a_real_database() {
    let Some(source) = std::env::var_os("WB_OWNER_DB_COPY") else {
        return;
    };
    struct Stderr;
    impl log::Log for Stderr {
        fn enabled(&self, m: &log::Metadata) -> bool {
            m.level() <= log::Level::Info
        }
        fn log(&self, r: &log::Record) {
            if self.enabled(r.metadata()) {
                eprintln!("[{}] {}", r.level(), r.args());
            }
        }
        fn flush(&self) {}
    }
    let _ = log::set_logger(&Stderr).map(|()| log::set_max_level(log::LevelFilter::Info));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    fs::copy(&source, &path).unwrap();
    let db = ForkDb::new(&path);
    db.with_conn(|conn| {
        assert_eq!(user_version(conn)?, latest_version());
        let expected = expected_schemas(&MIGRATIONS).unwrap();
        let missing = expected[latest_version()].missing_in(&Schema::read(conn)?);
        assert!(missing.is_empty(), "still missing {missing:?}");
        exercise_every_table(conn)
    })
    .unwrap();
    eprintln!("migrated copy at {}", path.display());
}
