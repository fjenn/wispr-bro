// SPDX-License-Identifier: GPL-3.0-or-later
//! Fork commands for the Scratchpad Hub page and floating note window.

use tauri::{AppHandle, Manager};

use super::ForkError;
use crate::fork::db::{DbError, ForkDb};
use crate::fork::scratchpad::{store, window};
use crate::fork::ForkRuntime;

fn db<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<tauri::State<'_, ForkDb>, ForkError> {
    app.try_state::<ForkDb>().ok_or(ForkError::NotReady)
}

fn internal(error: DbError) -> ForkError {
    ForkError::Internal(error.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn fork_scratchpad_list<R: ForkRuntime>(
    app: AppHandle<R>,
    query: Option<String>,
) -> Result<Vec<store::Note>, ForkError> {
    db(&app)?
        .with_conn(|conn| store::list(conn, query.as_deref()))
        .map_err(internal)
}

#[tauri::command]
#[specta::specta]
pub fn fork_scratchpad_get<R: ForkRuntime>(
    app: AppHandle<R>,
    id: i64,
) -> Result<Option<store::Note>, ForkError> {
    db(&app)?
        .with_conn(|conn| store::get(conn, id))
        .map_err(internal)
}

#[tauri::command]
#[specta::specta]
pub fn fork_scratchpad_save<R: ForkRuntime>(
    app: AppHandle<R>,
    id: i64,
    body: String,
) -> Result<store::Note, ForkError> {
    if !store::valid_body(&body) {
        return Err(ForkError::InvalidSetting(format!(
            "notes are limited to {} characters",
            store::MAX_BODY_CHARS
        )));
    }
    let note = db(&app)?
        .with_conn(|conn| store::save(conn, id, &body, crate::fork::context::now_ms()))
        .map_err(internal)?
        .ok_or_else(|| ForkError::InvalidSetting("note not found or deleted".into()))?;
    crate::fork::scratchpad::changed(&app);
    Ok(note)
}

#[tauri::command]
#[specta::specta]
pub fn fork_scratchpad_set_pinned<R: ForkRuntime>(
    app: AppHandle<R>,
    id: i64,
    pinned: bool,
) -> Result<store::Note, ForkError> {
    let note = db(&app)?
        .with_conn(|conn| store::set_pinned(conn, id, pinned))
        .map_err(internal)?
        .ok_or_else(|| ForkError::InvalidSetting("note not found or deleted".into()))?;
    crate::fork::scratchpad::changed(&app);
    Ok(note)
}

#[tauri::command]
#[specta::specta]
pub fn fork_scratchpad_delete<R: ForkRuntime>(app: AppHandle<R>, id: i64) -> Result<(), ForkError> {
    db(&app)?
        .with_conn(|conn| store::delete(conn, id, crate::fork::context::now_ms()))
        .map_err(internal)?;
    crate::fork::scratchpad::changed(&app);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn fork_scratchpad_open<R: ForkRuntime>(app: AppHandle<R>) -> Result<store::Note, ForkError> {
    window::open_shortcut(&app).map_err(ForkError::Internal)
}

#[tauri::command]
#[specta::specta]
pub fn fork_scratchpad_new<R: ForkRuntime>(app: AppHandle<R>) -> Result<store::Note, ForkError> {
    window::open(&app, window::Open::New).map_err(ForkError::Internal)
}

#[tauri::command]
#[specta::specta]
pub fn fork_scratchpad_show_note<R: ForkRuntime>(
    app: AppHandle<R>,
    id: i64,
) -> Result<store::Note, ForkError> {
    window::open(&app, window::Open::Note(id)).map_err(ForkError::Internal)
}

#[tauri::command]
#[specta::specta]
pub fn fork_scratchpad_close<R: ForkRuntime>(app: AppHandle<R>) -> Result<(), ForkError> {
    window::close(&app).map_err(ForkError::Internal)
}

#[tauri::command]
#[specta::specta]
pub fn fork_scratchpad_expand<R: ForkRuntime>(app: AppHandle<R>) -> Result<bool, ForkError> {
    window::expand(&app).map_err(ForkError::Internal)
}

#[tauri::command]
#[specta::specta]
pub fn fork_scratchpad_copy<R: ForkRuntime>(
    _app: AppHandle<R>,
    text: String,
) -> Result<(), ForkError> {
    if !store::valid_body(&text) {
        return Err(ForkError::InvalidSetting(
            "note is too large to copy".into(),
        ));
    }
    #[cfg(target_os = "macos")]
    crate::fork::clipboard::nspb::write_text(&text, false)
        .map(|_| ())
        .map_err(|error| ForkError::Internal(error.to_string()))?;
    #[cfg(not(target_os = "macos"))]
    return Err(ForkError::InvalidSetting(
        "native note copy is available on macOS".into(),
    ));
    #[cfg(target_os = "macos")]
    Ok(())
}

/// Polish is the merged S-21 engine and built-in definition, with no AX
/// selection, paste, clipboard write, or transform-run side effects.
#[tauri::command]
#[specta::specta]
pub async fn fork_scratchpad_polish<R: ForkRuntime>(
    app: AppHandle<R>,
    source: String,
) -> Result<String, ForkError> {
    if source.trim().is_empty() {
        return Err(ForkError::InvalidSetting(
            "there is no note to polish".into(),
        ));
    }
    if !store::valid_body(&source) {
        return Err(ForkError::InvalidSetting(
            "note is too large to polish".into(),
        ));
    }
    let definition = db(&app)?
        .with_conn(|conn| {
            crate::fork::transforms::record::definition(
                conn,
                crate::fork::transforms::TransformKind::Polish,
            )
        })
        .map_err(internal)?
        .ok_or_else(|| ForkError::Internal("the Polish transform is missing".into()))?;
    crate::fork::transforms::run::preview(&app, definition, &source)
        .await
        .map_err(ForkError::Internal)
}
