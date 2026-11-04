// SPDX-License-Identifier: GPL-3.0-or-later
//! Full-text search over `fork.db` (R5 §4.3).
//!
//! Both indexes are contentless FTS5 tables with the `trigram` tokenizer,
//! maintained by the app (no triggers). The FTS rowid is the key of the
//! content row, so a hit maps straight back to `entry_meta.entry_id` or
//! `notes.id`.
//!
//! Matching is substring matching, case-insensitive. The trigram tokenizer
//! cannot match fewer than three characters, so shorter queries fall back to
//! `LIKE` on the content table.

use rusqlite::{params_from_iter, Connection, ToSql};

use super::DbError;

/// Queries with fewer characters (Unicode scalar values) than this use the
/// `LIKE` fallback.
pub const MIN_TRIGRAM_CHARS: usize = 3;

/// One FTS index and the content table it mirrors. All names are fork
/// constants, never user input.
#[derive(Debug)]
pub struct FtsIndex {
    /// The FTS5 virtual table.
    pub fts_table: &'static str,
    /// The table that holds the text (used by the `LIKE` fallback and to skip
    /// soft-deleted rows).
    pub content_table: &'static str,
    /// Integer key in the content table; equals the FTS rowid.
    pub key_column: &'static str,
    /// Indexed columns, same names and order in both tables.
    pub columns: &'static [&'static str],
    /// Newest-first ordering of results.
    pub order_column: &'static str,
}

/// History search: `history_fts` over `entry_meta.final_text`.
pub const HISTORY: FtsIndex = FtsIndex {
    fts_table: "history_fts",
    content_table: "entry_meta",
    key_column: "entry_id",
    columns: &["final_text"],
    order_column: "created_at",
};

/// Scratchpad search: `notes_fts` over `notes.title` and `notes.body`.
pub const NOTES: FtsIndex = FtsIndex {
    fts_table: "notes_fts",
    content_table: "notes",
    key_column: "id",
    columns: &["title", "body"],
    order_column: "updated_at",
};

/// Adds or replaces the document for `key`. `values` match
/// [`FtsIndex::columns`] in order.
pub fn upsert_document(
    conn: &Connection,
    index: &FtsIndex,
    key: i64,
    values: &[&str],
) -> Result<(), DbError> {
    if values.len() != index.columns.len() {
        return Err(DbError::UnknownColumn(format!(
            "{}: expected {} values, got {}",
            index.fts_table,
            index.columns.len(),
            values.len()
        )));
    }
    delete_document(conn, index, key)?;
    let placeholders: Vec<String> = (2..=values.len() + 1).map(|i| format!("?{i}")).collect();
    let sql = format!(
        "INSERT INTO {} (rowid, {}) VALUES (?1, {})",
        index.fts_table,
        index.columns.join(", "),
        placeholders.join(", ")
    );
    let params: Vec<&dyn ToSql> = std::iter::once(&key as &dyn ToSql)
        .chain(values.iter().map(|v| v as &dyn ToSql))
        .collect();
    conn.prepare_cached(&sql)?
        .execute(params_from_iter(params))?;
    Ok(())
}

/// Removes the document for `key`. Idempotent.
pub fn delete_document(conn: &Connection, index: &FtsIndex, key: i64) -> Result<(), DbError> {
    let sql = format!("DELETE FROM {} WHERE rowid = ?1", index.fts_table);
    conn.prepare_cached(&sql)?.execute([key])?;
    Ok(())
}

/// Keys of the rows whose text contains `query`, newest first, skipping
/// soft-deleted rows. A blank query returns nothing.
pub fn search(
    conn: &Connection,
    index: &FtsIndex,
    query: &str,
    limit: u32,
) -> Result<Vec<i64>, DbError> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let (sql, pattern) = if query.chars().count() >= MIN_TRIGRAM_CHARS {
        (
            format!(
                "SELECT c.{key} FROM {fts} JOIN {content} AS c ON c.{key} = {fts}.rowid \
                 WHERE {fts} MATCH ?1 AND c.deleted_at IS NULL \
                 ORDER BY c.{order} DESC, c.{key} DESC LIMIT ?2",
                key = index.key_column,
                fts = index.fts_table,
                content = index.content_table,
                order = index.order_column,
            ),
            fts_phrase(query),
        )
    } else {
        let any_column: Vec<String> = index
            .columns
            .iter()
            .map(|c| format!("c.{c} LIKE ?1 ESCAPE '\\'"))
            .collect();
        (
            format!(
                "SELECT c.{key} FROM {content} AS c \
                 WHERE ({any}) AND c.deleted_at IS NULL \
                 ORDER BY c.{order} DESC, c.{key} DESC LIMIT ?2",
                key = index.key_column,
                content = index.content_table,
                any = any_column.join(" OR "),
                order = index.order_column,
            ),
            like_pattern(query),
        )
    };
    let mut stmt = conn.prepare_cached(&sql)?;
    let keys = stmt
        .query_map(rusqlite::params![pattern, limit], |row| row.get(0))?
        .collect::<Result<Vec<i64>, _>>()?;
    Ok(keys)
}

/// Indexes the final text of one history entry.
pub fn index_history_entry(
    conn: &Connection,
    entry_id: i64,
    final_text: &str,
) -> Result<(), DbError> {
    upsert_document(conn, &HISTORY, entry_id, &[final_text])
}

/// Re-indexes one history entry from its `entry_meta` row: the final text
/// (the spoken words) plus, when a snippet expanded, the text that was
/// inserted, so History finds what it shows (audit OT2). No row or no text
/// removes the document.
pub fn reindex_history_entry(conn: &Connection, entry_id: i64) -> Result<(), DbError> {
    use rusqlite::OptionalExtension;
    let texts: Option<(Option<String>, Option<String>)> = conn
        .prepare_cached("SELECT final_text, inserted_text FROM entry_meta WHERE entry_id = ?1")?
        .query_row([entry_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    let (final_text, inserted) = texts.unwrap_or_default();
    let final_text = final_text.unwrap_or_default();
    let document = match inserted.filter(|t| !t.trim().is_empty() && *t != final_text) {
        Some(inserted) if final_text.trim().is_empty() => inserted,
        Some(inserted) => format!("{final_text}\n{inserted}"),
        None => final_text,
    };
    if document.trim().is_empty() {
        delete_document(conn, &HISTORY, entry_id)
    } else {
        index_history_entry(conn, entry_id, &document)
    }
}

/// Searches history entries; see [`search`].
pub fn search_history(conn: &Connection, query: &str, limit: u32) -> Result<Vec<i64>, DbError> {
    search(conn, &HISTORY, query, limit)
}

/// The query as one FTS5 string (phrase), so operators and punctuation in
/// user input are matched literally.
fn fts_phrase(query: &str) -> String {
    format!("\"{}\"", query.replace('"', "\"\""))
}

/// `%query%` with `LIKE` wildcards escaped by `\`.
fn like_pattern(query: &str) -> String {
    let mut out = String::with_capacity(query.len() + 2);
    out.push('%');
    for ch in query.chars() {
        if matches!(ch, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(ch);
    }
    out.push('%');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::db::{upsert_entry_meta, ForkDb};

    fn db() -> (tempfile::TempDir, ForkDb) {
        let dir = tempfile::tempdir().unwrap();
        let db = ForkDb::in_dir(dir.path());
        (dir, db)
    }

    fn add_entry(conn: &Connection, id: i64, text: &str) {
        upsert_entry_meta(
            conn,
            id,
            &[("created_at", &(1_000 + id)), ("final_text", &text)],
        )
        .unwrap();
        index_history_entry(conn, id, text).unwrap();
    }

    #[test]
    fn trigram_query_finds_substrings_case_insensitively() {
        let (_dir, db) = db();
        db.with_conn(|conn| {
            add_entry(conn, 1, "The quick brown fox");
            add_entry(conn, 2, "jumps over the lazy dog");
            add_entry(conn, 3, "Quickly, please");
            assert_eq!(search_history(conn, "quick", 10)?, vec![3, 1]);
            assert_eq!(search_history(conn, "AZY D", 10)?, vec![2]);
            assert!(search_history(conn, "zebra", 10)?.is_empty());
            // FTS5 syntax in user input is matched literally.
            assert!(search_history(conn, "fox OR dog", 10)?.is_empty());
            assert!(search_history(conn, "\"quick", 10)?.is_empty());
            assert_eq!(search_history(conn, "quick", 1)?, vec![3]);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn cjk_uses_trigram_from_three_chars_and_like_below() {
        let (_dir, db) = db();
        db.with_conn(|conn| {
            add_entry(conn, 1, "今天天气很好，我们去公园");
            add_entry(conn, 2, "你好世界");
            add_entry(conn, 3, "東京は晴れです");
            // Three or more characters: trigram MATCH.
            assert_eq!(search_history(conn, "天气很", 10)?, vec![1]);
            assert_eq!(search_history(conn, "你好世界", 10)?, vec![2]);
            // Fewer than three: LIKE fallback.
            assert_eq!(search_history(conn, "你好", 10)?, vec![2]);
            assert_eq!(search_history(conn, "晴", 10)?, vec![3]);
            assert_eq!(search_history(conn, "公", 10)?, vec![1]);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn short_ascii_queries_use_like_with_escaped_wildcards() {
        let (_dir, db) = db();
        db.with_conn(|conn| {
            add_entry(conn, 1, "Ok then");
            add_entry(conn, 2, "100% sure");
            add_entry(conn, 3, "snake_case");
            add_entry(conn, 4, "nothing");
            assert_eq!(search_history(conn, "ok", 10)?, vec![1]);
            assert_eq!(search_history(conn, "%", 10)?, vec![2]);
            assert_eq!(search_history(conn, "_", 10)?, vec![3]);
            assert!(search_history(conn, "  ", 10)?.is_empty());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn update_and_delete_by_entry_id() {
        let (_dir, db) = db();
        db.with_conn(|conn| {
            add_entry(conn, 7, "first draft");
            assert_eq!(search_history(conn, "draft", 10)?, vec![7]);
            // Re-indexing replaces the old document.
            index_history_entry(conn, 7, "final version")?;
            assert!(search_history(conn, "draft", 10)?.is_empty());
            assert_eq!(search_history(conn, "version", 10)?, vec![7]);
            delete_document(conn, &HISTORY, 7)?;
            delete_document(conn, &HISTORY, 7)?;
            assert!(search_history(conn, "version", 10)?.is_empty());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn soft_deleted_rows_are_not_returned() {
        let (_dir, db) = db();
        db.with_conn(|conn| {
            add_entry(conn, 1, "hello there");
            upsert_entry_meta(conn, 1, &[("deleted_at", &5_000_i64)])?;
            assert!(search_history(conn, "hello", 10)?.is_empty());
            assert!(search_history(conn, "he", 10)?.is_empty());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn notes_index_covers_title_and_body() {
        let (_dir, db) = db();
        db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO notes (id, title, body, created_at, updated_at) \
                 VALUES (1, 'Groceries', 'milk and eggs', 1, 1), (2, 'Ideas', 'a new app', 2, 2)",
                [],
            )?;
            upsert_document(conn, &NOTES, 1, &["Groceries", "milk and eggs"])?;
            upsert_document(conn, &NOTES, 2, &["Ideas", "a new app"])?;
            assert_eq!(search(conn, &NOTES, "grocer", 10)?, vec![1]);
            assert_eq!(search(conn, &NOTES, "eggs", 10)?, vec![1]);
            assert_eq!(search(conn, &NOTES, "id", 10)?, vec![2]);
            assert!(upsert_document(conn, &NOTES, 3, &["only one"]).is_err());
            Ok(())
        })
        .unwrap();
    }
}
