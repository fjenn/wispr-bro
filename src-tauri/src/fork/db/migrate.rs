// SPDX-License-Identifier: GPL-3.0-or-later
//! The schema-aware migration runner of `fork.db`.
//!
//! During development, migrations were renumbered when slices were rebased,
//! so a dev build could apply columns under a different number than the
//! released chain (an alpha tester's database was at `user_version` 5 with
//! version 6's columns, and without version 3's). A plain numbered runner
//! then fails on every open (`duplicate column name`). This runner:
//!
//! - knows the full expected schema after every version, built by applying
//!   the migrations to an in-memory database ([`expected_schemas`]):
//!   `pragma_table_info` of every table, plus indexes, triggers and views;
//! - before migrating, compares the file's schema with the one expected at
//!   its `user_version`. If something expected is missing, every migration is
//!   replayed from the start; if the schema is ahead, the extra objects are
//!   kept. Either way an INFO line names the differences (object and column
//!   names only, never data);
//! - skips a migration whose changes are already all there, and inside a
//!   migration skips each statement whose target already exists
//!   (`ALTER TABLE … ADD COLUMN` of a present column, `CREATE` of a present
//!   table/index/trigger/view, `DROP` of an absent one);
//! - runs every migration in its own `BEGIN IMMEDIATE` transaction, together
//!   with its `user_version` bump, so a migration is applied whole or not at
//!   all, and a second process waits for the first instead of racing.
//!
//! Rules for new migrations: one change per statement, data statements
//! idempotent (`INSERT … WHERE NOT EXISTS`), hooks idempotent.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::CString;
use std::fmt;

use rusqlite::{Connection, Transaction, TransactionBehavior};

/// A hook run after a migration's SQL, inside its transaction.
pub type MigrationHook = fn(&Transaction) -> rusqlite::Result<()>;

/// One `fork.db` migration: SQL, plus an optional Rust hook.
#[derive(Clone, Copy)]
pub struct Migration {
    pub(crate) sql: &'static str,
    pub(crate) hook: Option<MigrationHook>,
}

impl fmt::Debug for Migration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Migration")
            .field("sql", &self.sql.lines().next().unwrap_or_default())
            .field("hook", &self.hook.is_some())
            .finish()
    }
}

impl Migration {
    /// A migration of SQL statements.
    pub const fn up(sql: &'static str) -> Self {
        Self { sql, hook: None }
    }

    /// SQL followed by `hook` in the same transaction.
    pub const fn up_with_hook(sql: &'static str, hook: MigrationHook) -> Self {
        Self {
            sql,
            hook: Some(hook),
        }
    }
}

/// A column as `pragma_table_info` reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Column {
    pub decl_type: String,
    pub not_null: bool,
    pub default: Option<String>,
    pub pk: i64,
}

/// The schema of a database: tables with their columns (by lowercase name),
/// and the names of indexes, triggers and views (lowercase).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Schema {
    pub tables: BTreeMap<String, BTreeMap<String, Column>>,
    pub indexes: BTreeSet<String>,
    pub triggers: BTreeSet<String>,
    pub views: BTreeSet<String>,
}

impl Schema {
    /// Reads the schema of `conn`'s main database.
    pub fn read(conn: &Connection) -> rusqlite::Result<Self> {
        let mut schema = Schema::default();
        let objects: Vec<(String, String)> = conn
            .prepare(
                "SELECT type, name FROM sqlite_schema \
                 WHERE name NOT LIKE 'sqlite_%' AND sql IS NOT NULL",
            )?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        for (kind, name) in objects {
            let key = name.to_lowercase();
            match kind.as_str() {
                "table" => {
                    let columns = conn
                        .prepare(
                            "SELECT name, type, \"notnull\", dflt_value, pk \
                             FROM pragma_table_info(?1)",
                        )?
                        .query_map([&name], |r| {
                            Ok((
                                r.get::<_, String>(0)?.to_lowercase(),
                                Column {
                                    decl_type: r.get::<_, String>(1)?.to_uppercase(),
                                    not_null: r.get(2)?,
                                    default: r.get(3)?,
                                    pk: r.get(4)?,
                                },
                            ))
                        })?
                        .collect::<Result<_, _>>()?;
                    schema.tables.insert(key, columns);
                }
                "index" => {
                    schema.indexes.insert(key);
                }
                "trigger" => {
                    schema.triggers.insert(key);
                }
                "view" => {
                    schema.views.insert(key);
                }
                _ => {}
            }
        }
        Ok(schema)
    }

    /// What `self` (the expected schema) has that `actual` lacks or has
    /// differently, as readable names (`table.column`, `index x`).
    pub fn missing_in(&self, actual: &Schema) -> Vec<String> {
        let mut out = Vec::new();
        for (table, columns) in &self.tables {
            match actual.tables.get(table) {
                None => out.push(format!("table {table}")),
                Some(have) => {
                    for (name, column) in columns {
                        match have.get(name) {
                            None => out.push(format!("{table}.{name}")),
                            Some(c) if c != column => out.push(format!("{table}.{name} (differs)")),
                            Some(_) => {}
                        }
                    }
                }
            }
        }
        for (kind, mine, theirs) in [
            ("index", &self.indexes, &actual.indexes),
            ("trigger", &self.triggers, &actual.triggers),
            ("view", &self.views, &actual.views),
        ] {
            out.extend(mine.difference(theirs).map(|n| format!("{kind} {n}")));
        }
        out
    }
}

/// What one migration changes: everything in the schema after it that was
/// not the same before it, and what it removed.
#[derive(Debug, Default)]
struct Delta {
    /// Tables added or changed, with the columns added or changed.
    columns: BTreeMap<String, BTreeMap<String, Column>>,
    /// Columns removed from a table that still exists.
    removed_columns: Vec<(String, String)>,
    removed_tables: Vec<String>,
    added_indexes: Vec<String>,
    added_triggers: Vec<String>,
    added_views: Vec<String>,
}

impl Delta {
    fn between(before: &Schema, after: &Schema) -> Self {
        let mut d = Delta::default();
        for (table, columns) in &after.tables {
            let old = before.tables.get(table);
            let changed: BTreeMap<String, Column> = columns
                .iter()
                .filter(|(name, column)| old.and_then(|o| o.get(*name)) != Some(*column))
                .map(|(n, c)| (n.clone(), c.clone()))
                .collect();
            if !changed.is_empty() || old.is_none() {
                d.columns.insert(table.clone(), changed);
            }
            if let Some(old) = old {
                for name in old.keys().filter(|n| !columns.contains_key(*n)) {
                    d.removed_columns.push((table.clone(), name.clone()));
                }
            }
        }
        d.removed_tables = before
            .tables
            .keys()
            .filter(|t| !after.tables.contains_key(*t))
            .cloned()
            .collect();
        d.added_indexes = after.indexes.difference(&before.indexes).cloned().collect();
        d.added_triggers = after
            .triggers
            .difference(&before.triggers)
            .cloned()
            .collect();
        d.added_views = after.views.difference(&before.views).cloned().collect();
        d
    }

    fn is_empty(&self) -> bool {
        self.columns.is_empty()
            && self.removed_columns.is_empty()
            && self.removed_tables.is_empty()
            && self.added_indexes.is_empty()
            && self.added_triggers.is_empty()
            && self.added_views.is_empty()
    }

    /// `true` when `actual` already has every change of this migration.
    fn present_in(&self, actual: &Schema) -> bool {
        self.columns.iter().all(|(table, columns)| {
            actual
                .tables
                .get(table)
                .is_some_and(|have| columns.iter().all(|(n, c)| have.get(n) == Some(c)))
        }) && self.removed_columns.iter().all(|(table, column)| {
            actual
                .tables
                .get(table)
                .is_none_or(|have| !have.contains_key(column))
        }) && self
            .removed_tables
            .iter()
            .all(|t| !actual.tables.contains_key(t))
            && self
                .added_indexes
                .iter()
                .all(|i| actual.indexes.contains(i))
            && self
                .added_triggers
                .iter()
                .all(|t| actual.triggers.contains(t))
            && self.added_views.iter().all(|v| actual.views.contains(v))
    }
}

/// The schema after each version: element `n` is the schema at
/// `user_version = n` (element 0 is empty).
pub(crate) fn expected_schemas(migrations: &[Migration]) -> rusqlite::Result<Vec<Schema>> {
    let mut conn = Connection::open_in_memory()?;
    let mut out = vec![Schema::read(&conn)?];
    for m in migrations {
        let tx = conn.transaction()?;
        tx.execute_batch(m.sql)?;
        if let Some(hook) = m.hook {
            hook(&tx)?;
        }
        out.push(Schema::read(&tx)?);
        tx.commit()?;
    }
    Ok(out)
}

/// Splits SQL into complete statements (`sqlite3_complete`, so a trigger's
/// `BEGIN … END;` stays whole). Comment-only pieces are dropped.
pub(crate) fn split_statements(sql: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, _) in sql.match_indices(';') {
        let piece = &sql[start..=i];
        if is_complete(piece) {
            if !strip_comments(piece)
                .trim_matches([';', ' ', '\n', '\r', '\t'])
                .is_empty()
            {
                out.push(piece.trim());
            }
            start = i + 1;
        }
    }
    let rest = &sql[start..];
    if !strip_comments(rest).trim().is_empty() {
        out.push(rest.trim());
    }
    out
}

fn is_complete(sql: &str) -> bool {
    let Ok(c) = CString::new(sql) else {
        return false;
    };
    // SAFETY: `c` is a valid NUL-terminated string that outlives the call;
    // `sqlite3_complete` only reads it.
    unsafe { rusqlite::ffi::sqlite3_complete(c.as_ptr()) != 0 }
}

/// `sql` with `--` and `/* */` comments replaced by spaces (string literals
/// and quoted names kept).
fn strip_comments(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '-' if chars.peek() == Some(&'-') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
                out.push('\n');
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut prev = ' ';
                for c in chars.by_ref() {
                    if prev == '*' && c == '/' {
                        break;
                    }
                    prev = c;
                }
                out.push(' ');
            }
            '\'' | '"' | '`' | '[' => {
                let close = if c == '[' { ']' } else { c };
                out.push(c);
                for c in chars.by_ref() {
                    out.push(c);
                    if c == close {
                        break;
                    }
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// Words and quoted names of a statement, quotes removed.
fn tokens(sql: &str) -> Vec<String> {
    let clean = strip_comments(sql);
    let mut out = Vec::new();
    let mut chars = clean.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if matches!(c, '"' | '`' | '[') {
            chars.next();
            let close = if c == '[' { ']' } else { c };
            let mut word = String::new();
            for c in chars.by_ref() {
                if c == close {
                    break;
                }
                word.push(c);
            }
            out.push(word);
        } else if c.is_alphanumeric() || c == '_' {
            let mut word = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_alphanumeric() || c == '_' || c == '$' {
                    word.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            out.push(word);
        } else {
            out.push(c.to_string());
            chars.next();
        }
        if out.len() >= 12 {
            break;
        }
    }
    out
}

/// What a statement creates or removes, as far as the guard cares.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Target {
    AddColumn { table: String, column: String },
    Create { kind: &'static str, name: String },
    Drop { kind: &'static str, name: String },
    Other,
}

/// Recognises `ALTER TABLE t ADD [COLUMN] c`, `CREATE [TEMP] [UNIQUE|VIRTUAL]
/// TABLE|INDEX|TRIGGER|VIEW [IF NOT EXISTS] n` and `DROP … [IF EXISTS] n`.
pub(crate) fn classify(sql: &str) -> Target {
    let t = tokens(sql);
    let up: Vec<String> = t.iter().map(|w| w.to_uppercase()).collect();
    let word = |i: usize| up.get(i).map(String::as_str).unwrap_or_default();
    // `schema.name` → name.
    let name_at = |i: usize| -> (String, usize) {
        if t.get(i + 1).map(String::as_str) == Some(".") {
            (t.get(i + 2).cloned().unwrap_or_default(), i + 3)
        } else {
            (t.get(i).cloned().unwrap_or_default(), i + 1)
        }
    };
    match word(0) {
        "ALTER" if word(1) == "TABLE" => {
            let (table, next) = name_at(2);
            if word(next) != "ADD" {
                return Target::Other;
            }
            let at = if word(next + 1) == "COLUMN" {
                next + 2
            } else {
                next + 1
            };
            match t.get(at) {
                Some(column) => Target::AddColumn {
                    table: table.to_lowercase(),
                    column: column.to_lowercase(),
                },
                None => Target::Other,
            }
        }
        "CREATE" => {
            let mut i = 1;
            while matches!(word(i), "TEMP" | "TEMPORARY" | "UNIQUE" | "VIRTUAL") {
                i += 1;
            }
            let kind = match word(i) {
                "TABLE" => "table",
                "INDEX" => "index",
                "TRIGGER" => "trigger",
                "VIEW" => "view",
                _ => return Target::Other,
            };
            i += 1;
            if word(i) == "IF" && word(i + 1) == "NOT" && word(i + 2) == "EXISTS" {
                i += 3;
            }
            Target::Create {
                kind,
                name: name_at(i).0.to_lowercase(),
            }
        }
        "DROP" => {
            let kind = match word(1) {
                "TABLE" => "table",
                "INDEX" => "index",
                "TRIGGER" => "trigger",
                "VIEW" => "view",
                _ => return Target::Other,
            };
            let mut i = 2;
            if word(i) == "IF" && word(i + 1) == "EXISTS" {
                i += 2;
            }
            Target::Drop {
                kind,
                name: name_at(i).0.to_lowercase(),
            }
        }
        _ => Target::Other,
    }
}

fn object_exists(conn: &Connection, kind: &str, name: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_schema WHERE type = ?1 AND name = ?2 COLLATE NOCASE)",
        [kind, name],
        |r| r.get(0),
    )
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM pragma_table_info(?1) WHERE name = ?2 COLLATE NOCASE)",
        [table, column],
        |r| r.get(0),
    )
}

/// `true` when running the statement would fail only because its work is
/// already done (or there is nothing to drop).
fn already_done(conn: &Connection, target: &Target) -> rusqlite::Result<bool> {
    Ok(match target {
        Target::AddColumn { table, column } => column_exists(conn, table, column)?,
        Target::Create { kind, name } => object_exists(conn, kind, name)?,
        Target::Drop { kind, name } => !object_exists(conn, kind, name)?,
        Target::Other => false,
    })
}

/// A migration that failed: its version and the SQLite error.
#[derive(Debug)]
pub struct MigrationFailure {
    pub version: usize,
    pub error: rusqlite::Error,
}

/// What [`run`] did.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Report {
    /// Versions whose changes were already all there and were only stamped.
    pub skipped: Vec<usize>,
    /// Statements skipped because their target already existed.
    pub statements_skipped: usize,
    /// Versions applied (wholly or in part).
    pub applied: Vec<usize>,
}

/// Applies migrations `from + 1 ..= migrations.len()` to `conn`, each in its
/// own immediate transaction with its `user_version` bump (never lowered).
/// Safe to repeat: a migration already applied, by this process or another
/// one that held the lock first, finds its changes present and is skipped.
pub(crate) fn run(
    conn: &mut Connection,
    migrations: &[Migration],
    expected: &[Schema],
    from: usize,
) -> Result<Report, MigrationFailure> {
    let mut report = Report::default();
    for (index, m) in migrations.iter().enumerate().skip(from) {
        let version = index + 1;
        let fail = |error| MigrationFailure { version, error };
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(fail)?;
        let current: i64 = tx
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(fail)?;
        let actual = Schema::read(&tx).map_err(fail)?;
        let delta = Delta::between(&expected[index], &expected[version]);
        if !delta.is_empty() && delta.present_in(&actual) {
            report.skipped.push(version);
        } else {
            for stmt in split_statements(m.sql) {
                if already_done(&tx, &classify(stmt)).map_err(fail)? {
                    report.statements_skipped += 1;
                    continue;
                }
                tx.execute_batch(stmt).map_err(fail)?;
            }
            if let Some(hook) = m.hook {
                hook(&tx).map_err(fail)?;
            }
            report.applied.push(version);
        }
        if current < version as i64 {
            tx.pragma_update(None, "user_version", version as i64)
                .map_err(fail)?;
        }
        tx.commit().map_err(fail)?;
    }
    Ok(report)
}
