// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for `fork.db` (F-06).

use std::sync::Arc;
use std::thread;

use super::*;

/// Every M1 table from R5 §4.3.
const M1_TABLES: &[&str] = &[
    "entry_meta",
    "daily_stats",
    "daily_app_stats",
    "corrections",
    "history_fts",
    "dictionary_entries",
    "snippets",
    "notes",
    "notes_fts",
    "notifications",
    "app_rules",
    "transforms",
    "transform_runs",
    "queued_takes",
];

const M2_TABLES: &[&str] = &["meetings", "meeting_segments", "meeting_notes", "folders"];

fn table_exists(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1)",
        [name],
        |row| row.get(0),
    )
    .unwrap()
}

fn files_with_prefix(dir: &Path, prefix: &str) -> Vec<PathBuf> {
    let mut files = aside_files(dir, prefix).unwrap();
    files.sort();
    files
}

/// The shipped migrations plus test-only future ones.
fn future_migrations() -> Vec<M<'static>> {
    let mut all = MIGRATIONS.to_vec();
    all.push(M::up(
        "ALTER TABLE entry_meta ADD COLUMN test_future_a TEXT;",
    ));
    all.push(M::up(
        "CREATE TABLE test_future_b (id INTEGER PRIMARY KEY);",
    ));
    all
}

fn raw_user_version(path: &Path) -> i64 {
    let conn = Connection::open(path).unwrap();
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap()
}

#[test]
fn fresh_create_has_every_m1_table_and_the_pragmas() {
    let dir = tempfile::tempdir().unwrap();
    let db = ForkDb::in_dir(dir.path());
    assert!(!db.path().exists(), "handle creation does no I/O");

    db.with_conn(|conn| {
        assert_eq!(user_version(conn)?, latest_version());
        assert_eq!(latest_version(), 3);
        for table in M1_TABLES {
            assert!(table_exists(conn, table), "missing table {table}");
        }
        for table in M2_TABLES {
            assert!(!table_exists(conn, table), "M2 table {table} created in M1");
        }
        let mode: String = conn.pragma_query_value(None, "journal_mode", |r| r.get(0))?;
        assert_eq!(mode, "wal");
        let sync: i64 = conn.pragma_query_value(None, "synchronous", |r| r.get(0))?;
        assert_eq!(sync, 1, "synchronous=NORMAL");
        let fk: i64 = conn.pragma_query_value(None, "foreign_keys", |r| r.get(0))?;
        assert_eq!(fk, 1);
        let busy: i64 = conn.pragma_query_value(None, "busy_timeout", |r| r.get(0))?;
        assert_eq!(busy, 5000);
        for index in ["entry_meta_created_at", "entry_meta_app_bundle_id"] {
            let found: bool = conn.query_row(
                "SELECT EXISTS (SELECT 1 FROM sqlite_schema WHERE type = 'index' AND name = ?1)",
                [index],
                |r| r.get(0),
            )?;
            assert!(found, "missing index {index}");
        }
        Ok(())
    })
    .unwrap();
    assert!(dir.path().join(DB_FILE_NAME).exists());
    assert!(
        files_with_prefix(dir.path(), BACKUP_PREFIX).is_empty(),
        "no backup for a fresh database"
    );
}

#[test]
fn bundled_sqlite_has_fts5_and_trigram() {
    let conn = Connection::open_in_memory().unwrap();
    let version: String = conn
        .query_row("SELECT sqlite_version()", [], |r| r.get(0))
        .unwrap();
    let parts: Vec<u32> = version.split('.').map(|p| p.parse().unwrap()).collect();
    // `contentless_delete` needs 3.43; trigram needs 3.34.
    assert!(
        (parts[0], parts[1]) >= (3, 43),
        "bundled SQLite {version} too old"
    );
    let fts5: bool = conn
        .query_row("SELECT sqlite_compileoption_used('ENABLE_FTS5')", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(fts5, "SQLite built without FTS5");
    conn.execute_batch(
        "CREATE VIRTUAL TABLE t USING fts5(x, content='', contentless_delete=1, tokenize='trigram');
         INSERT INTO t (rowid, x) VALUES (1, 'abcdef');",
    )
    .unwrap();
    let hit: i64 = conn
        .query_row("SELECT rowid FROM t WHERE t MATCH '\"cde\"'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(hit, 1);
}

#[test]
fn empty_v0_file_is_migrated_without_a_backup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    fs::write(&path, b"").unwrap();
    let db = ForkDb::new(&path);
    db.open_now().unwrap();
    assert_eq!(raw_user_version(&path), latest_version() as i64);
    assert!(files_with_prefix(dir.path(), BACKUP_PREFIX).is_empty());
    assert!(files_with_prefix(dir.path(), CORRUPT_PREFIX).is_empty());
}

#[test]
fn upgrades_from_every_prior_version_keeping_data() {
    let all = future_migrations();
    for start in 0..all.len() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);

        if start > 0 {
            let old = ForkDb::with_migrations(&path, all[..start].to_vec());
            old.with_conn(|conn| {
                upsert_entry_meta(conn, 42, &[("created_at", &1_i64), ("final_text", &"kept")])
            })
            .unwrap();
            old.close();
            assert_eq!(raw_user_version(&path), start as i64);
        }

        let new = ForkDb::with_migrations(&path, all.clone());
        new.with_conn(|conn| {
            assert_eq!(user_version(conn)?, all.len());
            assert!(table_exists(conn, "test_future_b"));
            if start > 0 {
                let text: String = conn.query_row(
                    "SELECT final_text FROM entry_meta WHERE entry_id = 42",
                    [],
                    |r| r.get(0),
                )?;
                assert_eq!(text, "kept");
            }
            Ok(())
        })
        .unwrap();
        new.close();

        let backups: Vec<PathBuf> = files_with_prefix(dir.path(), BACKUP_PREFIX)
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e == "db"))
            .collect();
        if start == 0 {
            assert!(backups.is_empty(), "no backup for v0");
        } else {
            assert_eq!(backups.len(), 1, "one backup before migrating v{start}");
            let name = backups[0].file_name().unwrap().to_str().unwrap();
            assert!(
                name.starts_with(&format!("{BACKUP_PREFIX}v{start}-")),
                "{name}"
            );
            // The backup is a complete, consistent copy at the old version.
            assert_eq!(raw_user_version(&backups[0]), start as i64);
            let conn = Connection::open(&backups[0]).unwrap();
            let n: i64 = conn
                .query_row("SELECT COUNT(*) FROM entry_meta", [], |r| r.get(0))
                .unwrap();
            assert_eq!(n, 1);
        }
    }
}

#[test]
fn backups_are_pruned_to_the_newest_few() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    let all = future_migrations();
    for (i, name) in [
        "20200101T000000000Z",
        "20210101T000000000Z",
        "20220101T000000000Z",
    ]
    .iter()
    .enumerate()
    {
        fs::write(
            dir.path().join(format!("{BACKUP_PREFIX}v{i}-{name}.db")),
            b"x",
        )
        .unwrap();
    }
    ForkDb::with_migrations(&path, all[..1].to_vec())
        .open_now()
        .unwrap();
    ForkDb::with_migrations(&path, all.clone())
        .open_now()
        .unwrap();
    let mains: Vec<String> = files_with_prefix(dir.path(), BACKUP_PREFIX)
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "db"))
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(mains.len(), BACKUPS_KEPT, "{mains:?}");
    assert!(!mains.iter().any(|n| n.contains("2020")), "{mains:?}");
}

#[test]
fn newer_schema_is_refused_and_left_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    ForkDb::new(&path).open_now().unwrap();
    {
        let conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
    }
    let before = fs::read(&path).unwrap();

    let db = ForkDb::new(&path);
    let err = db.open_now().unwrap_err();
    assert!(
        matches!(err, DbError::SchemaTooNew { found: 99, known } if known == latest_version()),
        "{err:?}"
    );
    let message = err.to_string();
    assert!(message.contains("schema version 99"), "{message}");
    assert!(message.contains("newer version"), "{message}");
    assert!(message.contains("left untouched"), "{message}");

    // Still refused on retry, not moved aside, not backed up, not changed.
    assert!(db.with_conn(|_| Ok(())).is_err());
    assert_eq!(raw_user_version(&path), 99);
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(files_with_prefix(dir.path(), CORRUPT_PREFIX).is_empty());
    assert!(files_with_prefix(dir.path(), BACKUP_PREFIX).is_empty());
}

#[test]
fn wal_allows_a_writer_and_two_readers_without_busy_errors() {
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(ForkDb::in_dir(dir.path()));
    db.open_now().unwrap();
    const WRITES: i64 = 300;

    let writer = {
        let db = Arc::clone(&db);
        thread::spawn(move || {
            for i in 1..=WRITES {
                db.with_conn(|conn| {
                    let tx = conn.transaction()?;
                    upsert_entry_meta(
                        &tx,
                        i,
                        &[("created_at", &i), ("final_text", &format!("entry {i}"))],
                    )?;
                    fts::index_history_entry(&tx, i, &format!("entry {i}"))?;
                    tx.commit()?;
                    Ok(())
                })
                .unwrap_or_else(|e| panic!("write {i} failed: {e}"));
            }
        })
    };

    // Readers use their own connections (same pragmas), so they really read
    // concurrently with the writer instead of queueing on the mutex.
    let readers: Vec<_> = (0..2)
        .map(|r| {
            let path = db.path().to_path_buf();
            thread::spawn(move || {
                let conn = open_connection(&path).unwrap();
                let mut last = 0_i64;
                for n in 0..300 {
                    let count: i64 = conn
                        .query_row("SELECT COUNT(*) FROM entry_meta", [], |row| row.get(0))
                        .unwrap_or_else(|e| panic!("reader {r} query {n} failed: {e}"));
                    assert!(count >= last, "reader saw rows disappear");
                    last = count;
                    fts::search_history(&conn, "entry", 5)
                        .unwrap_or_else(|e| panic!("reader {r} search {n} failed: {e}"));
                }
            })
        })
        .collect();

    // Shared-handle readers queue on the mutex alongside the writer.
    let shared = {
        let db = Arc::clone(&db);
        thread::spawn(move || {
            for _ in 0..300 {
                db.with_conn(|conn| entry_meta_exists(conn, 1)).unwrap();
            }
        })
    };

    writer.join().unwrap();
    for reader in readers {
        reader.join().unwrap();
    }
    shared.join().unwrap();

    let total: i64 = db
        .with_conn(|conn| Ok(conn.query_row("SELECT COUNT(*) FROM entry_meta", [], |r| r.get(0))?))
        .unwrap();
    assert_eq!(total, WRITES);
}

#[test]
fn corrupted_file_is_moved_aside_and_recreated() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    let garbage: Vec<u8> = b"this is not an sqlite database file "
        .iter()
        .copied()
        .cycle()
        .take(8192)
        .collect();
    fs::write(&path, &garbage).unwrap();
    fs::write(with_suffix(&path, "-wal"), b"garbage wal").unwrap();

    let db = ForkDb::new(&path);
    db.with_conn(|conn| {
        assert_eq!(user_version(conn)?, latest_version());
        assert!(table_exists(conn, "entry_meta"));
        Ok(())
    })
    .unwrap();

    let aside = files_with_prefix(dir.path(), CORRUPT_PREFIX);
    let main = aside
        .iter()
        .find(|p| p.extension().is_some_and(|e| e == "db"))
        .expect("corrupt copy");
    assert_eq!(
        fs::read(main).unwrap(),
        garbage,
        "the bad file is kept, not lost"
    );
    assert!(
        aside
            .iter()
            .any(|p| p.to_string_lossy().ends_with(".db-wal")),
        "the -wal moved with it: {aside:?}"
    );
}

#[test]
fn failing_quick_check_counts_as_corruption() {
    // A truncated database: valid header, missing pages.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    let db = ForkDb::new(&path);
    db.with_conn(|conn| {
        let tx = conn.transaction()?;
        for i in 0..500_i64 {
            upsert_entry_meta(
                &tx,
                i,
                &[("created_at", &i), ("final_text", &"x".repeat(200))],
            )?;
        }
        tx.commit()?;
        Ok(())
    })
    .unwrap();
    db.close();
    let bytes = fs::read(&path).unwrap();
    assert!(bytes.len() > 16 * 4096);
    fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();

    let db = ForkDb::new(&path);
    db.with_conn(|conn| {
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM entry_meta", [], |r| r.get(0))?;
        assert_eq!(n, 0, "recreated empty");
        Ok(())
    })
    .unwrap();
    assert!(!files_with_prefix(dir.path(), CORRUPT_PREFIX).is_empty());
}

#[test]
fn delete_all_removes_db_wal_shm_and_copies() {
    let dir = tempfile::tempdir().unwrap();
    let db = ForkDb::in_dir(dir.path());
    db.with_conn(|conn| upsert_entry_meta(conn, 1, &[("created_at", &1_i64)]))
        .unwrap();
    let path = db.path().to_path_buf();
    let wal = with_suffix(&path, "-wal");
    let shm = with_suffix(&path, "-shm");
    assert!(path.exists() && wal.exists() && shm.exists());
    fs::write(dir.path().join(format!("{BACKUP_PREFIX}v1-x.db")), b"x").unwrap();
    fs::write(dir.path().join(format!("{CORRUPT_PREFIX}x.db-wal")), b"x").unwrap();
    let unrelated = dir.path().join("history.db");
    fs::write(&unrelated, b"handy").unwrap();

    db.delete_all().unwrap();
    assert!(!path.exists() && !wal.exists() && !shm.exists());
    assert!(files_with_prefix(dir.path(), BACKUP_PREFIX).is_empty());
    assert!(files_with_prefix(dir.path(), CORRUPT_PREFIX).is_empty());
    assert!(unrelated.exists(), "delete_all only touches fork.db files");

    // Deleting again is fine; the next use creates a fresh database.
    db.delete_all().unwrap();
    let exists = db.with_conn(|conn| entry_meta_exists(conn, 1)).unwrap();
    assert!(!exists);
}

#[test]
fn opening_fork_db_never_changes_handy_history_db() {
    let dir = tempfile::tempdir().unwrap();
    let history = dir.path().join("history.db");
    {
        let conn = Connection::open(&history).unwrap();
        conn.execute_batch(
            "CREATE TABLE transcription_history (id INTEGER PRIMARY KEY, text TEXT);
             INSERT INTO transcription_history (text) VALUES ('hello');
             PRAGMA user_version = 4;",
        )
        .unwrap();
    }
    let before = fs::read(&history).unwrap();

    let db = ForkDb::in_dir(dir.path());
    db.with_conn(|conn| upsert_entry_meta(conn, 1, &[("created_at", &1_i64)]))
        .unwrap();
    db.close();
    ForkDb::with_migrations(db.path(), future_migrations())
        .open_now()
        .unwrap();
    db.delete_all().unwrap();

    assert_eq!(raw_user_version(&history), 4);
    assert_eq!(fs::read(&history).unwrap(), before);
    assert!(!with_suffix(&history, "-wal").exists());
}

#[test]
fn entry_meta_upsert_is_idempotent_and_validated() {
    let dir = tempfile::tempdir().unwrap();
    let db = ForkDb::in_dir(dir.path());
    db.with_conn(|conn| {
        let fields: &[(&str, &dyn ToSql)] = &[
            ("created_at", &10_i64),
            ("final_text", &"Hi"),
            ("source", &"dictation"),
        ];
        upsert_entry_meta(conn, 5, fields)?;
        upsert_entry_meta(conn, 5, fields)?;
        upsert_entry_meta(conn, 5, &[("paste_outcome", &"confirmed")])?;
        let (n, text, outcome): (i64, String, String) = conn.query_row(
            "SELECT COUNT(*), final_text, paste_outcome FROM entry_meta",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        assert_eq!((n, text.as_str(), outcome.as_str()), (1, "Hi", "confirmed"));
        assert!(entry_meta_exists(conn, 5)?);

        assert!(matches!(
            upsert_entry_meta(conn, 5, &[("nope; DROP TABLE x", &1_i64)]),
            Err(DbError::UnknownColumn(_))
        ));
        assert!(matches!(
            upsert_entry_meta(conn, 5, &[("entry_id", &6_i64)]),
            Err(DbError::UnknownColumn(_))
        ));
        assert!(matches!(
            upsert_entry_meta(conn, 5, &[]),
            Err(DbError::NoFields)
        ));
        // CHECK constraints reject unknown enum values.
        assert!(upsert_entry_meta(conn, 5, &[("paste_outcome", &"maybe")]).is_err());
        assert!(upsert_entry_meta(conn, 5, &[("flagged", &2_i64)]).is_err());

        fts::index_history_entry(conn, 5, "Hi there")?;
        delete_entry_meta(conn, 5)?;
        delete_entry_meta(conn, 5)?;
        assert!(!entry_meta_exists(conn, 5)?);
        assert!(fts::search_history(conn, "there", 10)?.is_empty());
        Ok(())
    })
    .unwrap();
}

#[test]
fn transform_runs_keep_only_the_last_20() {
    let dir = tempfile::tempdir().unwrap();
    let db = ForkDb::in_dir(dir.path());
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO transforms (id, name, updated_at) VALUES (1, 'Polish', 0)",
            [],
        )?;
        for i in 0..25_i64 {
            conn.execute(
                "INSERT INTO transform_runs (transform_id, source_text, created_at) \
                 VALUES (1, 'x', ?1)",
                [i],
            )?;
        }
        let (n, oldest): (i64, i64) = conn.query_row(
            "SELECT COUNT(*), MIN(created_at) FROM transform_runs",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        assert_eq!((n, oldest), (20, 5));
        // Foreign keys are on: deleting the transform cascades.
        conn.execute("DELETE FROM transforms WHERE id = 1", [])?;
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM transform_runs", [], |r| r.get(0))?;
        assert_eq!(n, 0);
        Ok(())
    })
    .unwrap();
}

#[test]
fn migrations_are_valid() {
    Migrations::new(MIGRATIONS.to_vec()).validate().unwrap();
}

#[test]
fn plugin_setup_registers_the_handle_without_opening_the_file() {
    use tauri::test::{mock_builder, mock_context, noop_assets};
    let app = mock_builder()
        .plugin(crate::fork::init())
        .build(mock_context(noop_assets()))
        .expect("failed to build mock app");
    let db = app
        .try_state::<ForkDb>()
        .expect("ForkDb registered in setup");
    assert_eq!(db.path().file_name().unwrap(), DB_FILE_NAME);
    assert!(db.lock().is_none(), "no connection opened during setup");
}
