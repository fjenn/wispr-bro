// SPDX-License-Identifier: GPL-3.0-or-later
//! Snippet commands (S-17) for the Snippets page: list, add, edit, delete
//! with undo, CSV import (review, commit) and export. The file itself is
//! read by S-16's `fork_dictionary_read_file` (same rules, same wizard).
//!
//! Every change emits `ForkSnippetsChanged`. The text pipeline needs no
//! signal: it reads the snippets per delivery.

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};

use super::ForkError;
use crate::fork::db::{DbError, ForkDb};
use crate::fork::snippets::import::{SnippetImportOutcome, SnippetImportReview, SnippetImportRow};
use crate::fork::snippets::store::{self, SnippetSaveOutcome};
use crate::fork::snippets::{
    self, clean, SnippetCounts, SnippetEntry, SnippetInput, SnippetOrigin,
};
use crate::fork::ForkRuntime;

fn db<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<tauri::State<'_, ForkDb>, ForkError> {
    app.try_state::<ForkDb>().ok_or(ForkError::NotReady)
}

fn internal(e: DbError) -> ForkError {
    ForkError::Internal(e.to_string())
}

/// The page's data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SnippetList {
    /// Newest first.
    pub snippets: Vec<SnippetEntry>,
    pub counts: SnippetCounts,
}

/// Every snippet.
#[tauri::command]
#[specta::specta]
pub fn fork_snippets_list<R: ForkRuntime>(app: AppHandle<R>) -> Result<SnippetList, ForkError> {
    db(&app)?
        .with_conn(|c| {
            Ok(SnippetList {
                snippets: store::list(c)?,
                counts: store::counts(c)?,
            })
        })
        .map_err(internal)
}

/// Adds a snippet.
#[tauri::command]
#[specta::specta]
pub fn fork_snippets_add<R: ForkRuntime>(
    app: AppHandle<R>,
    input: SnippetInput,
) -> Result<SnippetSaveOutcome, ForkError> {
    let s = match clean(&input) {
        Ok(s) => s,
        Err(problem) => return Ok(SnippetSaveOutcome::Invalid { problem }),
    };
    let now = crate::fork::context::now_ms();
    let outcome = db(&app)?
        .with_conn(|c| store::add(c, &s, SnippetOrigin::User, now))
        .map_err(internal)?;
    if matches!(outcome, SnippetSaveOutcome::Saved { .. }) {
        snippets::notify_changed(&app);
    }
    Ok(outcome)
}

/// Changes snippet `id`.
#[tauri::command]
#[specta::specta]
pub fn fork_snippets_update<R: ForkRuntime>(
    app: AppHandle<R>,
    id: i64,
    input: SnippetInput,
) -> Result<SnippetSaveOutcome, ForkError> {
    let s = match clean(&input) {
        Ok(s) => s,
        Err(problem) => return Ok(SnippetSaveOutcome::Invalid { problem }),
    };
    let now = crate::fork::context::now_ms();
    let outcome = db(&app)?
        .with_conn(|c| store::update(c, id, &s, now))
        .map_err(internal)?;
    if matches!(outcome, SnippetSaveOutcome::Saved { .. }) {
        snippets::notify_changed(&app);
    }
    Ok(outcome)
}

/// Deletes snippets. Returns them, so the page can offer Undo
/// ([`fork_snippets_restore`]).
#[tauri::command]
#[specta::specta]
pub fn fork_snippets_delete<R: ForkRuntime>(
    app: AppHandle<R>,
    ids: Vec<i64>,
) -> Result<Vec<SnippetEntry>, ForkError> {
    let gone = db(&app)?
        .with_conn(|c| store::delete(c, &ids))
        .map_err(internal)?;
    if !gone.is_empty() {
        snippets::notify_changed(&app);
    }
    Ok(gone)
}

/// Puts deleted snippets back (Undo). Returns how many came back.
#[tauri::command]
#[specta::specta]
pub fn fork_snippets_restore<R: ForkRuntime>(
    app: AppHandle<R>,
    snippets: Vec<SnippetEntry>,
) -> Result<u32, ForkError> {
    let n = db(&app)?
        .with_conn(|c| store::restore(c, &snippets))
        .map_err(internal)?;
    if n > 0 {
        snippets::notify_changed(&app);
    }
    Ok(n)
}

/// The import wizard's review step.
#[tauri::command]
#[specta::specta]
pub fn fork_snippets_import_review<R: ForkRuntime>(
    app: AppHandle<R>,
    rows: Vec<SnippetImportRow>,
) -> Result<SnippetImportReview, ForkError> {
    db(&app)?
        .with_conn(|c| snippets::import::review(c, &rows))
        .map_err(internal)
}

/// Imports the rows. Conflicts listed in `replace_rows` (row numbers) take
/// the file's text.
#[tauri::command]
#[specta::specta]
pub fn fork_snippets_import<R: ForkRuntime>(
    app: AppHandle<R>,
    rows: Vec<SnippetImportRow>,
    replace_rows: Vec<u32>,
) -> Result<SnippetImportOutcome, ForkError> {
    let now = crate::fork::context::now_ms();
    let outcome = db(&app)?
        .with_conn(|c| snippets::import::commit(c, &rows, &replace_rows, now))
        .map_err(internal)?;
    if outcome.added > 0 || outcome.replaced > 0 {
        snippets::notify_changed(&app);
    }
    log::info!(
        "snippets: import added {}, replaced {}, skipped {}, kept {}, invalid {}, over the limit {}",
        outcome.added,
        outcome.replaced,
        outcome.skipped,
        outcome.kept,
        outcome.invalid,
        outcome.over_limit
    );
    Ok(outcome)
}

/// Writes every snippet to `path` (from the save panel) as CSV. Returns how
/// many rows were written.
#[tauri::command]
#[specta::specta]
pub fn fork_snippets_export<R: ForkRuntime>(
    app: AppHandle<R>,
    path: String,
) -> Result<u32, ForkError> {
    let list = db(&app)?.with_conn(|c| store::list(c)).map_err(internal)?;
    snippets::export::write_file(std::path::Path::new(&path), &list).map_err(|e| {
        log::warn!("snippets: export failed: {e}");
        ForkError::Internal(e.to_string())
    })
}
