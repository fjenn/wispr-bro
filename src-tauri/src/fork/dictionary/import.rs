// SPDX-License-Identifier: GPL-3.0-or-later
//! CSV import (S-16, C-64): the wizard parses the file in the webview
//! (papaparse, R4), the user maps its columns, and the rows come here twice:
//! once for the review step ([`review`]: new, duplicates, conflicts,
//! invalid) and once to commit ([`commit`], one transaction that re-checks
//! everything, so a change made in between cannot slip through).
//!
//! The file itself is read by [`read_file`]: `.csv` only, at most 3 MB
//! (checked before reading), UTF-8 (a BOM is dropped), UTF-16 with a BOM,
//! or else Windows-1252, which is what older spreadsheet exports write.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use specta::Type;

use super::store::{self, SaveOutcome};
use super::{clean, CleanEntry, DictionaryEntry, EntryInput, EntryKind, EntryProblem, EntrySource};
use crate::fork::db::DbError;

/// Largest file the wizard takes (Wispr's limit, C-64).
pub const MAX_FILE_BYTES: u64 = 3 * 1024 * 1024;

/// A file read for the wizard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ImportFile {
    Read {
        /// File name without the folder.
        name: String,
        text: String,
    },
    /// Not a `.csv` file.
    NotCsv,
    /// Larger than [`MAX_FILE_BYTES`].
    TooLarge { bytes: u64 },
    /// Missing or unreadable.
    Unreadable,
}

/// Decodes file bytes as text (see the module doc).
pub fn decode(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    for (bom, encoding) in [
        (&[0xFF, 0xFE][..], encoding_rs::UTF_16LE),
        (&[0xFE, 0xFF][..], encoding_rs::UTF_16BE),
    ] {
        if bytes.starts_with(bom) {
            let (text, _, _) = encoding.decode(bytes);
            return text.into_owned();
        }
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => encoding_rs::WINDOWS_1252.decode(bytes).0.into_owned(),
    }
}

/// Reads the file at `path` for the wizard.
pub fn read_file(path: &Path) -> ImportFile {
    let is_csv = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("csv"));
    if !is_csv {
        return ImportFile::NotCsv;
    }
    let Ok(meta) = std::fs::metadata(path) else {
        return ImportFile::Unreadable;
    };
    if !meta.is_file() {
        return ImportFile::Unreadable;
    }
    if meta.len() > MAX_FILE_BYTES {
        return ImportFile::TooLarge { bytes: meta.len() };
    }
    let Ok(file) = std::fs::File::open(path) else {
        return ImportFile::Unreadable;
    };
    // The cap again while reading: the file may grow after the check.
    let mut bytes = Vec::new();
    if file
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return ImportFile::Unreadable;
    }
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return ImportFile::TooLarge {
            bytes: bytes.len() as u64,
        };
    }
    ImportFile::Read {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        text: decode(&bytes),
    }
}

/// Undoes the export's formula guard ([`super::export::guard`]): a cell
/// that starts with `'` followed by `= + - @` loses the `'`.
pub fn unguard(cell: &str) -> &str {
    match cell.strip_prefix('\'') {
        Some(rest) if rest.starts_with(['=', '+', '-', '@']) => rest,
        _ => cell,
    }
}

/// One mapped row of the file. `row` is its 1-based line in the file, for
/// messages.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ImportRow {
    pub row: u32,
    pub phrase: String,
    pub replacement: Option<String>,
}

/// A valid row as it would be saved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ReviewRow {
    pub row: u32,
    pub kind: EntryKind,
    pub phrase: String,
    pub replacement: Option<String>,
}

/// A row whose word (or "from") is already in the dictionary with another
/// spelling or replacement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ImportConflict {
    pub incoming: ReviewRow,
    pub existing: DictionaryEntry,
}

/// A row that already is in the dictionary, or repeats an earlier row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ImportDuplicate {
    pub incoming: ReviewRow,
    /// `true`: an earlier row of the same file; `false`: the dictionary.
    pub in_file: bool,
}

/// A row that is not a valid entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct InvalidRow {
    pub row: u32,
    pub phrase: String,
    pub problem: EntryProblem,
}

/// The review step.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ImportReview {
    pub new_entries: Vec<ReviewRow>,
    pub duplicates: Vec<ImportDuplicate>,
    pub conflicts: Vec<ImportConflict>,
    pub invalid: Vec<InvalidRow>,
    /// How many more entries fit ([`super::MAX_ENTRIES`]).
    pub room: u32,
}

/// What an import did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ImportOutcome {
    pub added: u32,
    /// Conflicts the user chose to take from the file.
    pub replaced: u32,
    /// Duplicates skipped.
    pub skipped: u32,
    /// Conflicts kept as they were.
    pub kept: u32,
    pub invalid: u32,
    /// New rows left out because the dictionary was full.
    pub over_limit: u32,
}

enum Classified {
    New(CleanEntry),
    Duplicate {
        entry: CleanEntry,
        in_file: bool,
    },
    Conflict {
        entry: CleanEntry,
        existing: DictionaryEntry,
    },
    Invalid(EntryProblem),
}

fn review_row(row: u32, e: &CleanEntry) -> ReviewRow {
    ReviewRow {
        row,
        kind: e.kind,
        phrase: e.phrase.clone(),
        replacement: e.replacement.clone(),
    }
}

/// Sorts every row into new / duplicate / conflict / invalid against the
/// dictionary as it is now. A key repeated in the file counts once (the
/// first row wins).
fn classify(conn: &Connection, rows: &[ImportRow]) -> Result<Vec<(u32, Classified)>, DbError> {
    let existing: HashMap<(EntryKind, String), DictionaryEntry> = store::list(conn)?
        .into_iter()
        .filter(|e| e.status == super::EntryStatus::Active)
        .map(|e| ((e.kind, super::norm_of(&e.phrase)), e))
        .collect();
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let input = EntryInput {
            phrase: unguard(&r.phrase).to_owned(),
            replacement: r.replacement.as_deref().map(|s| unguard(s).to_owned()),
        };
        let class = match clean(&input) {
            Err(problem) => Classified::Invalid(problem),
            Ok(e) => {
                let key = (e.kind, e.norm.clone());
                if !seen.insert(key.clone()) {
                    Classified::Duplicate {
                        entry: e,
                        in_file: true,
                    }
                } else {
                    match existing.get(&key) {
                        None => Classified::New(e),
                        Some(old) => {
                            let same = match e.kind {
                                EntryKind::Word => old.phrase == e.phrase,
                                EntryKind::Rule => {
                                    old.replacement.as_deref() == e.replacement.as_deref()
                                }
                            };
                            if same {
                                Classified::Duplicate {
                                    entry: e,
                                    in_file: false,
                                }
                            } else {
                                Classified::Conflict {
                                    entry: e,
                                    existing: old.clone(),
                                }
                            }
                        }
                    }
                }
            }
        };
        out.push((r.row, class));
    }
    Ok(out)
}

fn room(conn: &Connection) -> Result<u32, DbError> {
    let c = store::counts(conn)?;
    let used = c.words as usize + c.rules as usize;
    Ok(u32::try_from(super::MAX_ENTRIES.saturating_sub(used)).unwrap_or(0))
}

/// The review step: what importing `rows` would do.
pub fn review(conn: &Connection, rows: &[ImportRow]) -> Result<ImportReview, DbError> {
    store::ensure_norms(conn)?;
    let mut review = ImportReview {
        room: room(conn)?,
        ..Default::default()
    };
    for (r, (row, class)) in rows.iter().zip(classify(conn, rows)?) {
        match class {
            Classified::New(e) => review.new_entries.push(review_row(row, &e)),
            Classified::Duplicate { entry, in_file } => review.duplicates.push(ImportDuplicate {
                incoming: review_row(row, &entry),
                in_file,
            }),
            Classified::Conflict { entry, existing } => review.conflicts.push(ImportConflict {
                incoming: review_row(row, &entry),
                existing,
            }),
            Classified::Invalid(problem) => review.invalid.push(InvalidRow {
                row,
                phrase: r.phrase.chars().take(80).collect(),
                problem,
            }),
        }
    }
    Ok(review)
}

/// Imports `rows`. Conflicts whose row number is in `replace_rows` take the
/// file's version; the others keep the dictionary's.
pub fn commit(
    conn: &mut Connection,
    rows: &[ImportRow],
    replace_rows: &[u32],
    now_ms: i64,
) -> Result<ImportOutcome, DbError> {
    let tx = conn.transaction()?;
    store::ensure_norms(&tx)?;
    let mut outcome = ImportOutcome::default();
    for (row, class) in classify(&tx, rows)? {
        match class {
            Classified::Invalid(_) => outcome.invalid += 1,
            Classified::Duplicate { .. } => outcome.skipped += 1,
            Classified::New(e) => match store::add(&tx, &e, EntrySource::Import, now_ms)? {
                SaveOutcome::Saved { .. } => outcome.added += 1,
                SaveOutcome::Invalid {
                    problem: EntryProblem::Full,
                } => outcome.over_limit += 1,
                _ => outcome.skipped += 1,
            },
            Classified::Conflict { entry, existing } if replace_rows.contains(&row) => {
                let saved = matches!(
                    store::update(&tx, existing.id, &entry, now_ms)?,
                    SaveOutcome::Saved { .. }
                );
                if saved {
                    outcome.replaced += 1;
                } else {
                    outcome.kept += 1;
                }
            }
            Classified::Conflict { .. } => outcome.kept += 1,
        }
    }
    tx.commit()?;
    Ok(outcome)
}
