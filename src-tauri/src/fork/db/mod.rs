// SPDX-License-Identifier: GPL-3.0-or-later
//! `fork.db`: the fork's own SQLite database (R5 §4.3, DEC-08).
//!
//! - One file, `fork.db`, in Tauri's app data dir, next to Handy's
//!   `history.db`, which this module never opens.
//! - Pragmas on every connection: `journal_mode=WAL`, `busy_timeout=5000`,
//!   `synchronous=NORMAL`, `foreign_keys=ON`.
//! - One connection per process, behind a mutex ([`ForkDb`]). The plugin
//!   `setup` only registers the handle ([`register`]); the file is opened
//!   lazily on first use, so `setup` does no I/O (R5 §4.1 init order).
//! - Migrations with `rusqlite_migration` on its own `user_version`. Before a
//!   migration runs on an existing database, the file is checkpointed and
//!   copied ([backup](BACKUP_PREFIX)). A schema newer than this build knows is
//!   refused with [`DbError::SchemaTooNew`] and the file is left untouched.
//! - A file that cannot be read as a database or fails `PRAGMA quick_check` is
//!   moved aside under a timestamped name and recreated
//!   ([`report_db_recovered`]). Opening never panics.
//! - Every fork write keyed by a Handy history row is an idempotent upsert
//!   keyed by `entry_id` ([`upsert_entry_meta`]).
//!
//! S-09 adds the history data model on top: [`entry_meta`] (the hooks in
//! Handy's `HistoryManager`), [`stats`] (the store that survives retention
//! and deletes), [`retention`], the launch [`reconcile`], [`disk`] (a full
//! disk keeps the text) and [`handy`] (the only way the fork touches Handy's
//! history). [`jobs`] schedules reconcile and retention. S-11 adds
//! [`rules`]: the active dictionary rules for the text pipeline and the
//! record of the ones that applied.

pub mod disk;
pub mod entry_meta;
pub mod fts;
pub mod handy;
pub mod jobs;
pub mod reconcile;
pub mod retention;
pub mod rules;
pub mod stats;

pub use entry_meta::{on_deleted, on_retried, on_saved};

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use rusqlite::config::DbConfig;
use rusqlite::{params_from_iter, Connection, ErrorCode, ToSql};
use rusqlite_migration::{MigrationDefinitionError, Migrations, M};
use tauri::{Manager, Runtime};

/// File name of the fork database inside the app data dir.
pub const DB_FILE_NAME: &str = "fork.db";

/// Name prefix of pre-migration backups: `fork.backup-v<N>-<ts>.db`.
pub const BACKUP_PREFIX: &str = "fork.backup-";

/// Name prefix of databases moved aside after corruption:
/// `fork.corrupt-<ts>.db`.
pub const CORRUPT_PREFIX: &str = "fork.corrupt-";

/// How many pre-migration backups are kept; older ones are removed.
pub const BACKUPS_KEPT: usize = 3;

/// `busy_timeout` in milliseconds.
pub const BUSY_TIMEOUT_MS: u64 = 5000;

/// SQLite's sidecar files in WAL mode.
const SIDECAR_SUFFIXES: [&str; 2] = ["-wal", "-shm"];

/// The fork's migrations, in order. Append only; never edit a shipped entry.
static MIGRATIONS: std::sync::LazyLock<Vec<M<'static>>> = std::sync::LazyLock::new(|| {
    vec![
        M::up(include_str!("migrations/0001_init.sql")),
        M::up(include_str!("migrations/0002_history.sql")),
        M::up(include_str!("migrations/0003_text.sql")),
        // S-16: the dictionary keys are Unicode case folds, computed in Rust.
        M::up_with_hook(
            include_str!("migrations/0004_dictionary.sql"),
            crate::fork::dictionary::store::migrate_v4,
        ),
        M::up(include_str!("migrations/0005_notifications.sql")),
    ]
});

/// The first `n` shipped migrations (tests build older databases with it).
#[cfg(test)]
pub(crate) fn test_migrations(n: usize) -> Vec<M<'static>> {
    MIGRATIONS[..n].to_vec()
}

/// Schema version this build creates and understands.
pub fn latest_version() -> usize {
    MIGRATIONS.len()
}

/// Errors from `fork.db`.
#[derive(Debug)]
pub enum DbError {
    /// The database was written by a newer build. The file is left as is.
    SchemaTooNew {
        /// `user_version` found in the file.
        found: usize,
        /// Latest version this build knows.
        known: usize,
    },
    /// `PRAGMA quick_check` did not return `ok`.
    Corrupt(String),
    /// `journal_mode=WAL` could not be enabled.
    NotWal(String),
    /// A column name passed to an upsert helper is not a column of the table.
    UnknownColumn(String),
    /// An upsert helper was called with no fields.
    NoFields,
    Sqlite(rusqlite::Error),
    Migration(rusqlite_migration::Error),
    Io(io::Error),
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DbError::SchemaTooNew { found, known } => write!(
                f,
                "fork.db has schema version {found}, but this build of Wispr Bro only knows \
                 version {known}: it was written by a newer version of the app. The file was \
                 left untouched. Update Wispr Bro to use it."
            ),
            DbError::Corrupt(detail) => write!(f, "fork.db failed quick_check: {detail}"),
            DbError::NotWal(mode) => {
                write!(
                    f,
                    "fork.db could not switch to WAL (journal_mode is {mode})"
                )
            }
            DbError::UnknownColumn(name) => write!(f, "unknown fork.db column `{name}`"),
            DbError::NoFields => write!(f, "upsert called with no fields"),
            DbError::Sqlite(e) => write!(f, "fork.db: {e}"),
            DbError::Migration(e) => write!(f, "fork.db migration failed: {e}"),
            DbError::Io(e) => write!(f, "fork.db file operation failed: {e}"),
        }
    }
}

impl std::error::Error for DbError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DbError::Sqlite(e) => Some(e),
            DbError::Migration(e) => Some(e),
            DbError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        DbError::Sqlite(e)
    }
}

impl From<io::Error> for DbError {
    fn from(e: io::Error) -> Self {
        DbError::Io(e)
    }
}

/// Handle to `fork.db`, registered as Tauri state by [`register`].
///
/// Holds at most one open connection. Every access goes through
/// [`ForkDb::with_conn`], which opens (and migrates, or recovers) the file on
/// first use.
pub struct ForkDb {
    path: PathBuf,
    migrations: Vec<M<'static>>,
    conn: Mutex<Option<Connection>>,
}

impl fmt::Debug for ForkDb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ForkDb").field("path", &self.path).finish()
    }
}

impl ForkDb {
    /// A handle for the database file at `path`. Does no I/O.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self::with_migrations(path, MIGRATIONS.to_vec())
    }

    /// A handle for `<dir>/fork.db`. Does no I/O.
    pub fn in_dir(dir: &Path) -> Self {
        Self::new(dir.join(DB_FILE_NAME))
    }

    /// Like [`ForkDb::new`] with an explicit migration list (tests simulate
    /// past and future schema versions with it).
    pub(crate) fn with_migrations(path: impl Into<PathBuf>, migrations: Vec<M<'static>>) -> Self {
        Self {
            path: path.into(),
            migrations,
            conn: Mutex::new(None),
        }
    }

    /// Path of `fork.db`.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Runs `f` on the process's single connection, opening the database
    /// first if needed. Calls are serialized by the mutex.
    pub fn with_conn<T>(
        &self,
        f: impl FnOnce(&mut Connection) -> Result<T, DbError>,
    ) -> Result<T, DbError> {
        let mut guard = self.lock();
        let conn = match &mut *guard {
            Some(conn) => conn,
            slot @ None => slot.insert(open_database(&self.path, &self.migrations)?),
        };
        f(conn)
    }

    /// Opens the database now (instead of on first use) and reports the
    /// outcome.
    pub fn open_now(&self) -> Result<(), DbError> {
        self.with_conn(|_| Ok(()))
    }

    /// Closes the connection. The next [`ForkDb::with_conn`] reopens it.
    pub fn close(&self) {
        let conn = self.lock().take();
        close_connection(conn);
    }

    /// Closes the connection and removes `fork.db`, `fork.db-wal`,
    /// `fork.db-shm`, plus every pre-migration backup and moved-aside copy in
    /// the same directory, which hold the same user data. Missing files are
    /// fine. Used by delete-all (S-27); the next access creates a fresh
    /// database.
    pub fn delete_all(&self) -> Result<(), DbError> {
        let mut guard = self.lock();
        close_connection(guard.take());
        remove_db_files(&self.path)?;
        if let Some(dir) = self.path.parent() {
            for aside in aside_files(dir, BACKUP_PREFIX)?
                .into_iter()
                .chain(aside_files(dir, CORRUPT_PREFIX)?)
            {
                remove_if_exists(&aside)?;
            }
        }
        drop(guard);
        log::info!("fork.db deleted");
        Ok(())
    }

    fn lock(&self) -> MutexGuard<'_, Option<Connection>> {
        // A panic inside `with_conn` drops any open transaction (rollback),
        // so the connection is still usable.
        self.conn
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Registers the [`ForkDb`] handle as Tauri state. Called from the fork
/// plugin `setup`; resolves the app data dir (the portable `Data/` dir in
/// portable mode, next to Handy's `history.db`) and does no other I/O.
pub fn register<R: Runtime, A: Manager<R>>(app: &A) {
    let dir = match crate::portable::data_dir() {
        Some(dir) => Ok(dir.clone()),
        None => app.path().app_data_dir(),
    };
    match dir {
        Ok(dir) => {
            app.manage(ForkDb::in_dir(&dir));
        }
        Err(e) => log::error!("fork.db: app data dir unavailable, fork.db disabled: {e}"),
    }
}

/// Called after a corrupted `fork.db` was moved aside and recreated.
///
/// Logs, and queues a notifications inbox entry (S-18) that `fork::notify`
/// posts once the new database is open. Called from inside the open path, so
/// it never touches the database itself.
pub fn report_db_recovered(db_path: &Path, moved_to: &[PathBuf], reason: &str) {
    log::warn!(
        "fork.db at {} could not be used ({reason}); moved it aside to {:?} and created a new one",
        db_path.display(),
        moved_to
    );
    crate::fork::notify::queue_data_recovered(db_path);
}

/// Opens one connection with the fork's pragmas, without migrating. Used by
/// [`ForkDb`] and by tests that need a second connection to the same file.
pub fn open_connection(path: &Path) -> Result<Connection, DbError> {
    let conn = Connection::open(path)?;
    configure(&conn)?;
    Ok(conn)
}

/// Applies the fork's pragmas to `conn`.
fn configure(conn: &Connection) -> Result<(), DbError> {
    conn.busy_timeout(Duration::from_millis(BUSY_TIMEOUT_MS))?;
    let mode: String = conn.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(DbError::NotWal(mode));
    }
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(())
}

/// `PRAGMA quick_check`, as an error unless it returns a single `ok`.
fn quick_check(conn: &Connection) -> Result<(), DbError> {
    let mut stmt = conn.prepare("PRAGMA quick_check")?;
    let rows: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    if rows.len() == 1 && rows[0] == "ok" {
        Ok(())
    } else {
        Err(DbError::Corrupt(rows.join("; ")))
    }
}

/// First 16 bytes of every SQLite database file.
const SQLITE_HEADER: &[u8; 16] = b"SQLite format 3\0";

/// Rejects a non-empty file that does not start with the SQLite header
/// without opening it, so SQLite never touches its `-wal`.
fn check_header(path: &Path) -> Result<(), DbError> {
    use std::io::Read;
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(DbError::Io(e)),
    };
    let mut header = Vec::with_capacity(SQLITE_HEADER.len());
    file.take(SQLITE_HEADER.len() as u64)
        .read_to_end(&mut header)?;
    if header.is_empty() || header == SQLITE_HEADER {
        Ok(())
    } else {
        Err(DbError::Corrupt("not an SQLite database file".to_owned()))
    }
}

fn open_checked(path: &Path) -> Result<Connection, DbError> {
    check_header(path)?;
    let conn = Connection::open(path)?;
    // While the file is unverified, closing must not checkpoint (and then
    // delete) the -wal: if the checks fail, the -wal moves aside with it.
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true)?;
    configure(&conn)?;
    quick_check(&conn)?;
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, false)?;
    Ok(conn)
}

/// `true` for errors that mean the file is not a usable SQLite database, so
/// moving it aside and starting fresh is the right recovery. Busy, locked,
/// permission and disk-full errors are not in this set: moving the file would
/// not help.
fn is_corruption(e: &DbError) -> bool {
    match e {
        DbError::Corrupt(_) => true,
        DbError::Sqlite(rusqlite::Error::SqliteFailure(err, _)) => matches!(
            err.code,
            ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt
        ),
        _ => false,
    }
}

/// Opens, recovers if corrupted, and migrates `fork.db`.
fn open_database(path: &Path, migrations: &[M<'static>]) -> Result<Connection, DbError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut conn = match open_checked(path) {
        Ok(conn) => conn,
        Err(e) if is_corruption(&e) => {
            let moved = move_aside(path, CORRUPT_PREFIX)?;
            report_db_recovered(path, &moved, &e.to_string());
            open_checked(path)?
        }
        Err(e) => return Err(e),
    };
    migrate(&mut conn, path, migrations)?;
    Ok(conn)
}

/// Current `user_version` of the connection's main database.
pub fn user_version(conn: &Connection) -> Result<usize, DbError> {
    let v: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    Ok(usize::try_from(v).unwrap_or(0))
}

fn migrate(conn: &mut Connection, path: &Path, migrations: &[M<'static>]) -> Result<(), DbError> {
    let known = migrations.len();
    let found = user_version(conn)?;
    if found > known {
        return Err(DbError::SchemaTooNew { found, known });
    }
    if found == known {
        return Ok(());
    }
    if found > 0 {
        backup_before_migrate(conn, path, found)?;
    }
    match Migrations::new(migrations.to_vec()).to_latest(conn) {
        Ok(()) => {
            log::info!("fork.db migrated from version {found} to {known}");
            Ok(())
        }
        Err(rusqlite_migration::Error::MigrationDefinition(
            MigrationDefinitionError::DatabaseTooFarAhead,
        )) => Err(DbError::SchemaTooNew { found, known }),
        Err(e) => Err(DbError::Migration(e)),
    }
}

/// Checkpoints the WAL into the main file and copies `fork.db` (and any
/// `-wal`/`-shm` left) to `fork.backup-v<version>-<ts>.db`. Keeps the newest
/// [`BACKUPS_KEPT`] backups.
fn backup_before_migrate(
    conn: &Connection,
    path: &Path,
    version: usize,
) -> Result<Vec<PathBuf>, DbError> {
    // (busy, wal frames, frames checkpointed); TRUNCATE also empties -wal.
    let (busy, _, _): (i64, i64, i64) =
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
    if busy != 0 {
        log::warn!("fork.db: WAL checkpoint before backup was blocked; copying -wal too");
    }
    let name = format!("{BACKUP_PREFIX}v{version}-{}.db", timestamp());
    let mut copied = Vec::new();
    for (from, to) in sidecar_pairs(path, &path.with_file_name(name)) {
        if from.exists() {
            fs::copy(&from, &to)?;
            copied.push(to);
        }
    }
    log::info!("fork.db backed up before migration to {:?}", copied);
    if let Some(dir) = path.parent() {
        prune_backups(dir)?;
    }
    Ok(copied)
}

fn prune_backups(dir: &Path) -> Result<(), DbError> {
    let mut mains: Vec<PathBuf> = aside_files(dir, BACKUP_PREFIX)?
        .into_iter()
        .filter(|p| p.extension().is_some_and(|ext| ext == "db"))
        .collect();
    // Names embed a sortable UTC timestamp after the version; sort by it.
    mains.sort_by_key(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.rsplit('-').next())
            .unwrap_or_default()
            .to_owned()
    });
    let excess = mains.len().saturating_sub(BACKUPS_KEPT);
    for old in mains.into_iter().take(excess) {
        for (file, _) in sidecar_pairs(&old, &old) {
            remove_if_exists(&file)?;
        }
    }
    Ok(())
}

/// Renames `fork.db`, `-wal` and `-shm` to `<prefix><ts>.db[-wal|-shm]`.
/// Returns the new paths of the files that existed.
fn move_aside(path: &Path, prefix: &str) -> Result<Vec<PathBuf>, DbError> {
    let target = path.with_file_name(format!("{prefix}{}.db", timestamp()));
    let mut moved = Vec::new();
    for (from, to) in sidecar_pairs(path, &target) {
        if from.exists() {
            fs::rename(&from, &to)?;
            moved.push(to);
        }
    }
    Ok(moved)
}

/// `(main, main-wal, main-shm)` of `from` zipped with the same for `to`.
fn sidecar_pairs(from: &Path, to: &Path) -> Vec<(PathBuf, PathBuf)> {
    let mut pairs = vec![(from.to_path_buf(), to.to_path_buf())];
    for suffix in SIDECAR_SUFFIXES {
        pairs.push((with_suffix(from, suffix), with_suffix(to, suffix)));
    }
    pairs
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

/// Removes `fork.db`, `fork.db-wal` and `fork.db-shm`.
fn remove_db_files(path: &Path) -> Result<(), DbError> {
    for (file, _) in sidecar_pairs(path, path) {
        remove_if_exists(&file)?;
    }
    Ok(())
}

fn remove_if_exists(path: &Path) -> Result<(), DbError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(DbError::Io(e)),
    }
}

/// Files in `dir` whose name starts with `prefix`.
fn aside_files(dir: &Path, prefix: &str) -> Result<Vec<PathBuf>, DbError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(DbError::Io(e)),
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with(prefix))
        {
            files.push(entry.path());
        }
    }
    Ok(files)
}

fn close_connection(conn: Option<Connection>) {
    if let Some(conn) = conn {
        if let Err((_, e)) = conn.close() {
            log::warn!("fork.db: close failed: {e}");
        }
    }
}

/// Sortable UTC timestamp with milliseconds, safe in file names.
fn timestamp() -> String {
    chrono::Utc::now().format("%Y%m%dT%H%M%S%3fZ").to_string()
}

/// Inserts or updates the `entry_meta` row for Handy history row `entry_id`.
///
/// Idempotent: the same call twice leaves the same row. On insert, columns
/// not in `fields` take their defaults (so a first write must include
/// `created_at`); on update, only the columns in `fields` change. Column names
/// are checked against the table, never interpolated unchecked.
pub fn upsert_entry_meta(
    conn: &Connection,
    entry_id: i64,
    fields: &[(&str, &dyn ToSql)],
) -> Result<(), DbError> {
    upsert_by_key(conn, "entry_meta", "entry_id", entry_id, fields)
}

/// `true` when an `entry_meta` row exists for `entry_id`.
pub fn entry_meta_exists(conn: &Connection, entry_id: i64) -> Result<bool, DbError> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM entry_meta WHERE entry_id = ?1)",
        [entry_id],
        |row| row.get(0),
    )?)
}

/// Removes the `entry_meta` row and its search document for `entry_id`, in
/// one transaction. Idempotent.
pub fn delete_entry_meta(conn: &mut Connection, entry_id: i64) -> Result<(), DbError> {
    let tx = conn.transaction()?;
    fts::delete_document(&tx, &fts::HISTORY, entry_id)?;
    tx.execute("DELETE FROM entry_meta WHERE entry_id = ?1", [entry_id])?;
    tx.commit()?;
    Ok(())
}

/// Upsert into `table` keyed by the unique column `key_column`. Both names
/// are fork constants; field names are validated against the table.
fn upsert_by_key(
    conn: &Connection,
    table: &'static str,
    key_column: &'static str,
    key: i64,
    fields: &[(&str, &dyn ToSql)],
) -> Result<(), DbError> {
    if fields.is_empty() {
        return Err(DbError::NoFields);
    }
    let columns = table_columns(conn, table)?;
    for (name, _) in fields {
        if *name == key_column || !columns.iter().any(|c| c == name) {
            return Err(DbError::UnknownColumn((*name).to_owned()));
        }
    }
    let names: Vec<String> = fields.iter().map(|(n, _)| format!("\"{n}\"")).collect();
    let params: Vec<&dyn ToSql> = std::iter::once(&key as &dyn ToSql)
        .chain(fields.iter().map(|(_, v)| *v))
        .collect();

    // UPDATE first, INSERT if no row: `INSERT … ON CONFLICT DO UPDATE` checks
    // NOT NULL columns before it sees the conflict, so it cannot do a partial
    // update. The single connection behind the mutex makes the pair safe.
    let assignments: Vec<String> = names
        .iter()
        .enumerate()
        .map(|(i, n)| format!("{n} = ?{}", i + 2))
        .collect();
    let update = format!(
        "UPDATE {table} SET {} WHERE {key_column} = ?1",
        assignments.join(", ")
    );
    let changed = conn
        .prepare_cached(&update)?
        .execute(params_from_iter(params.iter()))?;
    if changed == 0 {
        let placeholders: Vec<String> = (2..=fields.len() + 1).map(|i| format!("?{i}")).collect();
        let insert = format!(
            "INSERT INTO {table} ({key_column}, {}) VALUES (?1, {})",
            names.join(", "),
            placeholders.join(", ")
        );
        conn.prepare_cached(&insert)?
            .execute(params_from_iter(params.iter()))?;
    }
    Ok(())
}

fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>, DbError> {
    let mut stmt = conn.prepare_cached("SELECT name FROM pragma_table_info(?1)")?;
    let names = stmt
        .query_map([table], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;
    Ok(names)
}

#[cfg(test)]
mod tests;
