// SPDX-License-Identifier: GPL-3.0-or-later
//! CSV import (S-17, C-71) through the shared four-step wizard (S-16): the
//! webview parses the file (papaparse) and maps two columns, trigger and
//! text; the rows come here for the review ([`review`]: new, duplicates,
//! conflicts, invalid) and the commit ([`commit`], one transaction that
//! re-checks everything). The file is read by S-16's
//! `fork::dictionary::import::read_file` (`.csv`, ≤ 3 MB, encodings).
//! At most [`MAX_IMPORT_ROWS`] rows per file (Wispr's import limit).

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use specta::Type;

use super::store::{self, SnippetSaveOutcome};
use super::{clean, CleanSnippet, SnippetEntry, SnippetInput, SnippetOrigin, SnippetProblem};
use crate::fork::db::DbError;
use crate::fork::dictionary::import::unguard;

/// Most rows one import takes.
pub const MAX_IMPORT_ROWS: usize = super::MAX_SNIPPETS;

/// One mapped row of the file. `row` is its 1-based record number.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SnippetImportRow {
    pub row: u32,
    pub trigger: String,
    pub body: String,
}

/// A valid row as it would be saved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SnippetReviewRow {
    pub row: u32,
    pub trigger: String,
    pub body: String,
}

/// A row whose trigger is already used with another text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SnippetImportConflict {
    pub incoming: SnippetReviewRow,
    pub existing: SnippetEntry,
}

/// A row that already is a snippet, or repeats an earlier row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SnippetImportDuplicate {
    pub incoming: SnippetReviewRow,
    /// `true`: an earlier row of the same file; `false`: a saved snippet.
    pub in_file: bool,
}

/// A row that is not a valid snippet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SnippetInvalidRow {
    pub row: u32,
    /// The trigger as written, cut to 80 characters.
    pub trigger: String,
    pub problem: SnippetProblem,
}

/// The review step.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SnippetImportReview {
    pub new_snippets: Vec<SnippetReviewRow>,
    pub duplicates: Vec<SnippetImportDuplicate>,
    pub conflicts: Vec<SnippetImportConflict>,
    pub invalid: Vec<SnippetInvalidRow>,
    /// How many more snippets fit ([`super::MAX_SNIPPETS`]).
    pub room: u32,
    /// Rows after the first [`MAX_IMPORT_ROWS`], left out.
    pub ignored_rows: u32,
}

/// What an import did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SnippetImportOutcome {
    pub added: u32,
    /// Conflicts the user chose to take from the file.
    pub replaced: u32,
    /// Duplicates skipped.
    pub skipped: u32,
    /// Conflicts kept as they were.
    pub kept: u32,
    pub invalid: u32,
    /// New rows left out because the page was full (or past the row limit).
    pub over_limit: u32,
}

enum Classified {
    New(CleanSnippet),
    Duplicate {
        snippet: CleanSnippet,
        in_file: bool,
    },
    Conflict {
        snippet: CleanSnippet,
        existing: SnippetEntry,
    },
    Invalid(SnippetProblem),
}

fn review_row(row: u32, s: &CleanSnippet) -> SnippetReviewRow {
    SnippetReviewRow {
        row,
        trigger: s.trigger.clone(),
        body: s.body.clone(),
    }
}

fn input_of(r: &SnippetImportRow) -> SnippetInput {
    SnippetInput {
        trigger: unguard(&r.trigger).to_owned(),
        body: unguard(&r.body).to_owned(),
    }
}

/// Sorts the rows into new / duplicate / conflict / invalid against the
/// snippets as they are now. A trigger repeated in the file counts once
/// (the first row wins).
fn classify(
    conn: &Connection,
    rows: &[SnippetImportRow],
) -> Result<Vec<(u32, Classified)>, DbError> {
    let existing: HashMap<String, SnippetEntry> = store::list(conn)?
        .into_iter()
        .map(|s| (super::norm_of(&s.trigger), s))
        .collect();
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(rows.len().min(MAX_IMPORT_ROWS));
    for r in rows.iter().take(MAX_IMPORT_ROWS) {
        let class = match clean(&input_of(r)) {
            Err(problem) => Classified::Invalid(problem),
            Ok(s) if !seen.insert(s.norm.clone()) => Classified::Duplicate {
                snippet: s,
                in_file: true,
            },
            Ok(s) => match existing.get(&s.norm) {
                None => Classified::New(s),
                Some(old) if old.body == s.body => Classified::Duplicate {
                    snippet: s,
                    in_file: false,
                },
                Some(old) => Classified::Conflict {
                    snippet: s,
                    existing: old.clone(),
                },
            },
        };
        out.push((r.row, class));
    }
    Ok(out)
}

fn room(conn: &Connection) -> Result<u32, DbError> {
    let used = store::counts(conn)?.snippets as usize;
    Ok(u32::try_from(super::MAX_SNIPPETS.saturating_sub(used)).unwrap_or(0))
}

fn ignored(rows: &[SnippetImportRow]) -> u32 {
    u32::try_from(rows.len().saturating_sub(MAX_IMPORT_ROWS)).unwrap_or(u32::MAX)
}

/// The review step: what importing `rows` would do.
pub fn review(
    conn: &Connection,
    rows: &[SnippetImportRow],
) -> Result<SnippetImportReview, DbError> {
    let mut review = SnippetImportReview {
        room: room(conn)?,
        ignored_rows: ignored(rows),
        ..Default::default()
    };
    for (r, (row, class)) in rows.iter().zip(classify(conn, rows)?) {
        match class {
            Classified::New(s) => review.new_snippets.push(review_row(row, &s)),
            Classified::Duplicate { snippet, in_file } => {
                review.duplicates.push(SnippetImportDuplicate {
                    incoming: review_row(row, &snippet),
                    in_file,
                })
            }
            Classified::Conflict { snippet, existing } => {
                review.conflicts.push(SnippetImportConflict {
                    incoming: review_row(row, &snippet),
                    existing,
                })
            }
            Classified::Invalid(problem) => review.invalid.push(SnippetInvalidRow {
                row,
                trigger: r.trigger.chars().take(80).collect(),
                problem,
            }),
        }
    }
    Ok(review)
}

/// Imports `rows`. Conflicts whose row number is in `replace_rows` take the
/// file's text; the others keep the saved one.
pub fn commit(
    conn: &mut Connection,
    rows: &[SnippetImportRow],
    replace_rows: &[u32],
    now_ms: i64,
) -> Result<SnippetImportOutcome, DbError> {
    let tx = conn.transaction()?;
    let mut outcome = SnippetImportOutcome {
        over_limit: ignored(rows),
        ..Default::default()
    };
    for (row, class) in classify(&tx, rows)? {
        match class {
            Classified::Invalid(_) => outcome.invalid += 1,
            Classified::Duplicate { .. } => outcome.skipped += 1,
            Classified::New(s) => match store::add(&tx, &s, SnippetOrigin::Import, now_ms)? {
                SnippetSaveOutcome::Saved { .. } => outcome.added += 1,
                SnippetSaveOutcome::Invalid {
                    problem: SnippetProblem::Full,
                } => outcome.over_limit += 1,
                _ => outcome.skipped += 1,
            },
            Classified::Conflict { snippet, existing } if replace_rows.contains(&row) => {
                match store::update(&tx, existing.id, &snippet, now_ms)? {
                    SnippetSaveOutcome::Saved { .. } => outcome.replaced += 1,
                    _ => outcome.kept += 1,
                }
            }
            Classified::Conflict { .. } => outcome.kept += 1,
        }
    }
    tx.commit()?;
    Ok(outcome)
}
