// SPDX-License-Identifier: GPL-3.0-or-later
//! Dictionary commands (S-16) for the Dictionary page: list, add, edit,
//! delete with undo, CSV import (read, review, commit) and export.
//!
//! Every change syncs the words into Handy's `custom_words`
//! (`fork::dictionary::migrate`) and emits `ForkDictionaryChanged`. The
//! replacements need no signal: the text pipeline reads them per take.

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};

use super::ForkError;
use crate::fork::db::{DbError, ForkDb};
use crate::fork::dictionary::import::{ImportFile, ImportOutcome, ImportReview, ImportRow};
use crate::fork::dictionary::store::{self, SaveOutcome};
use crate::fork::dictionary::{
    self, clean, migrate, DictionaryCounts, DictionaryEntry, EntryInput, EntrySource,
};
use crate::fork::ForkRuntime;

fn db<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<tauri::State<'_, ForkDb>, ForkError> {
    app.try_state::<ForkDb>().ok_or(ForkError::NotReady)
}

fn internal(e: DbError) -> ForkError {
    ForkError::Internal(e.to_string())
}

/// Syncs Handy's word list and tells every window.
pub(crate) fn changed<R: ForkRuntime>(app: &AppHandle<R>) {
    migrate::sync(app);
    dictionary::notify_changed(app);
}

/// The page's data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct DictionaryList {
    /// Active and suggested entries, newest first.
    pub entries: Vec<DictionaryEntry>,
    pub counts: DictionaryCounts,
}

/// Every active and suggested entry. Takes over words added in Handy's own
/// settings first, so the page shows them.
#[tauri::command]
#[specta::specta]
pub fn fork_dictionary_list<R: ForkRuntime>(
    app: AppHandle<R>,
) -> Result<DictionaryList, ForkError> {
    migrate::sync(&app);
    db(&app)?
        .with_conn(|c| {
            store::ensure_norms(c)?;
            Ok(DictionaryList {
                entries: store::list(c)?,
                counts: store::counts(c)?,
            })
        })
        .map_err(internal)
}

/// Adds a word, or a replacement when `input.replacement` is set.
#[tauri::command]
#[specta::specta]
pub fn fork_dictionary_add<R: ForkRuntime>(
    app: AppHandle<R>,
    input: EntryInput,
) -> Result<SaveOutcome, ForkError> {
    let entry = match clean(&input) {
        Ok(e) => e,
        Err(problem) => return Ok(SaveOutcome::Invalid { problem }),
    };
    let now = crate::fork::context::now_ms();
    let outcome = db(&app)?
        .with_conn(|c| {
            store::ensure_norms(c)?;
            store::add(c, &entry, EntrySource::User, now)
        })
        .map_err(internal)?;
    if matches!(outcome, SaveOutcome::Saved { .. }) {
        changed(&app);
    }
    Ok(outcome)
}

/// Changes entry `id` (a word may become a replacement and back).
#[tauri::command]
#[specta::specta]
pub fn fork_dictionary_update<R: ForkRuntime>(
    app: AppHandle<R>,
    id: i64,
    input: EntryInput,
) -> Result<SaveOutcome, ForkError> {
    let entry = match clean(&input) {
        Ok(e) => e,
        Err(problem) => return Ok(SaveOutcome::Invalid { problem }),
    };
    let now = crate::fork::context::now_ms();
    let outcome = db(&app)?
        .with_conn(|c| {
            store::ensure_norms(c)?;
            store::update(c, id, &entry, now)
        })
        .map_err(internal)?;
    if matches!(outcome, SaveOutcome::Saved { .. }) {
        changed(&app);
    }
    Ok(outcome)
}

/// Deletes entries. Returns them, so the page can offer Undo
/// ([`fork_dictionary_restore`]).
#[tauri::command]
#[specta::specta]
pub fn fork_dictionary_delete<R: ForkRuntime>(
    app: AppHandle<R>,
    ids: Vec<i64>,
) -> Result<Vec<DictionaryEntry>, ForkError> {
    let gone = db(&app)?
        .with_conn(|c| store::delete(c, &ids))
        .map_err(internal)?;
    if !gone.is_empty() {
        changed(&app);
    }
    Ok(gone)
}

/// Puts deleted entries back (Undo). Returns how many came back.
#[tauri::command]
#[specta::specta]
pub fn fork_dictionary_restore<R: ForkRuntime>(
    app: AppHandle<R>,
    entries: Vec<DictionaryEntry>,
) -> Result<u32, ForkError> {
    let n = db(&app)?
        .with_conn(|c| store::restore(c, &entries))
        .map_err(internal)?;
    if n > 0 {
        changed(&app);
    }
    Ok(n)
}

/// Reads a `.csv` file (picked or dropped) for the import wizard.
#[tauri::command]
#[specta::specta]
pub fn fork_dictionary_read_file(path: String) -> ImportFile {
    dictionary::import::read_file(std::path::Path::new(&path))
}

/// The import wizard's review step.
#[tauri::command]
#[specta::specta]
pub fn fork_dictionary_import_review<R: ForkRuntime>(
    app: AppHandle<R>,
    rows: Vec<ImportRow>,
) -> Result<ImportReview, ForkError> {
    db(&app)?
        .with_conn(|c| dictionary::import::review(c, &rows))
        .map_err(internal)
}

/// Imports the rows. Conflicts listed in `replace_rows` (row numbers) take
/// the file's version.
#[tauri::command]
#[specta::specta]
pub fn fork_dictionary_import<R: ForkRuntime>(
    app: AppHandle<R>,
    rows: Vec<ImportRow>,
    replace_rows: Vec<u32>,
) -> Result<ImportOutcome, ForkError> {
    let now = crate::fork::context::now_ms();
    let outcome = db(&app)?
        .with_conn(|c| dictionary::import::commit(c, &rows, &replace_rows, now))
        .map_err(internal)?;
    if outcome.added > 0 || outcome.replaced > 0 {
        changed(&app);
    }
    log::info!(
        "dictionary: import added {}, replaced {}, skipped {}, kept {}, invalid {}, over the limit {}",
        outcome.added,
        outcome.replaced,
        outcome.skipped,
        outcome.kept,
        outcome.invalid,
        outcome.over_limit
    );
    Ok(outcome)
}

/// Writes the active entries to `path` (from the save panel) as CSV.
/// Returns how many rows were written.
#[tauri::command]
#[specta::specta]
pub fn fork_dictionary_export<R: ForkRuntime>(
    app: AppHandle<R>,
    path: String,
) -> Result<u32, ForkError> {
    let entries = db(&app)?.with_conn(|c| store::list(c)).map_err(internal)?;
    dictionary::export::write_file(std::path::Path::new(&path), &entries).map_err(|e| {
        log::warn!("dictionary: export failed: {e}");
        ForkError::Internal(e.to_string())
    })
}
