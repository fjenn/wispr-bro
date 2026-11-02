// SPDX-License-Identifier: GPL-3.0-or-later
//! Delete-all checklist (S-27). Runtime shutdown is kept separate from the
//! path-scoped eraser so tests never stop managers, restart, or touch a real
//! profile. The command runs the four phases in the required order.

use std::path::Path;

pub const CONFIRMATION: &str = "DELETE";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EraseReport {
    pub removed: u32,
}

pub fn valid_confirmation(value: &str) -> bool {
    value == CONFIRMATION
}

/// Stops recording, live/background transcription and model downloads. Every
/// queued job names a History row, so cancelling those ids empties both queues
/// and marks a running job cancelled before its next chunk.
pub fn stop_jobs(app: &tauri::AppHandle) -> Result<(), String> {
    use crate::fork::db::handy::HandyHistory;
    use tauri::Manager;

    crate::utils::cancel_current_operation(app);
    crate::fork::models::setup::on_exit(app);
    let Some(jobs) = app
        .try_state::<std::sync::Arc<crate::managers::transcription_jobs::TranscriptionJobManager>>(
        )
    else {
        return Ok(());
    };
    // Rows come through Handy's manager (R5 §4.3), never SQL on its file.
    let Some(history) = app.try_state::<std::sync::Arc<crate::managers::history::HistoryManager>>()
    else {
        return Ok(());
    };
    let ids: Vec<i64> = crate::fork::db::handy::HandyHistoryManager(history.inner().clone())
        .entries()?
        .into_iter()
        .map(|e| e.id)
        .collect();
    for id in ids {
        jobs.cancel(id);
    }
    Ok(())
}

/// Unloads the speech/language models synchronously, then stops the cleanup
/// sidecar and cancels its downloads. No files are removed until this returns.
pub fn unload_models(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;

    if let Some(manager) =
        app.try_state::<std::sync::Arc<crate::managers::transcription::TranscriptionManager>>()
    {
        manager.unload_model().map_err(|e| e.to_string())?;
    }
    crate::fork::llm::runtime::on_exit(app);
    Ok(())
}

/// Just before the files go: closes Handy's and the fork's settings stores
/// (flushing any pending debounced save first, so nothing rewrites them on
/// exit) and clears the webviews' storage (theme, sidebar and onboarding
/// progress in `localStorage`). WebKit clears asynchronously, so the caller
/// waits [`WEBVIEW_CLEAR_GRACE`] before relaunching.
pub fn release_for_erase(app: &tauri::AppHandle) {
    use tauri::Manager;
    use tauri_plugin_store::StoreExt;

    for path in [
        crate::settings::SETTINGS_STORE_PATH,
        crate::fork::settings::FORK_SETTINGS_FILE,
    ] {
        if let Some(store) = app.get_store(crate::portable::store_path(path)) {
            store.close_resource();
        }
    }
    for window in app.webview_windows().values() {
        if let Err(e) = window.clear_all_browsing_data() {
            log::warn!("erase: could not clear webview storage: {e}");
        }
    }
}

/// Time given to WebKit to finish clearing website data before relaunch.
pub const WEBVIEW_CLEAR_GRACE: std::time::Duration = std::time::Duration::from_millis(500);

fn safe_root(path: &Path) -> bool {
    if !path.is_absolute() || path.parent().is_none() {
        return false;
    }
    if std::env::var_os("HOME").is_some_and(|home| Path::new(&home) == path) {
        return false;
    }
    std::fs::symlink_metadata(path)
        .map(|m| m.is_dir() && !m.file_type().is_symlink())
        .unwrap_or(false)
}

fn remove_path(path: &Path, report: &mut EraseReport) -> Result<(), String> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("could not inspect {}: {e}", path.display())),
    };
    let result = if metadata.is_dir() && !metadata.file_type().is_symlink() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    result.map_err(|e| format!("could not delete {}: {e}", path.display()))?;
    report.removed += 1;
    Ok(())
}

/// App-data entries kept when the user keeps downloaded models: Handy's
/// `models/` folder, the Hugging Face cache (`HF_HOME`, DEC-39) and the
/// cleanup model's hash markers (so a kept model is not re-hashed).
pub const MODEL_PATHS: &[&str] = &["models", "huggingface", "llm/verified"];

/// Removes everything under `dir` except the `keep` paths (relative to `dir`,
/// `/`-separated). A parent of a kept path is descended into, never removed.
fn erase_dir_except(dir: &Path, keep: &[&str], report: &mut EraseReport) -> Result<(), String> {
    let mut entries = std::fs::read_dir(dir)
        .map_err(|e| format!("could not read {}: {e}", dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            remove_path(&entry.path(), report)?;
            continue;
        };
        if keep.contains(&name) {
            continue;
        }
        let nested: Vec<&str> = keep
            .iter()
            .filter_map(|k| k.strip_prefix(name).and_then(|rest| rest.strip_prefix('/')))
            .collect();
        let is_real_dir = entry
            .file_type()
            .is_ok_and(|t| t.is_dir() && !t.is_symlink());
        if !nested.is_empty() && is_real_dir {
            erase_dir_except(&entry.path(), &nested, report)?;
        } else {
            remove_path(&entry.path(), report)?;
        }
    }
    Ok(())
}

/// Erases only the contents of the supplied app data directory and its log
/// directory. With models kept, exactly [`MODEL_PATHS`] survive in app data.
/// Symlinks are unlinked, never followed.
pub fn erase_files(
    data_dir: &Path,
    log_dir: &Path,
    delete_models: bool,
) -> Result<EraseReport, String> {
    if !safe_root(data_dir) {
        return Err("refusing to erase an unsafe app data path".into());
    }
    let separate_logs = !log_dir.starts_with(data_dir);
    if separate_logs && log_dir.exists() && !safe_root(log_dir) {
        return Err("refusing to erase an unsafe log path".into());
    }
    let mut report = EraseReport::default();
    let keep: &[&str] = if delete_models { &[] } else { MODEL_PATHS };
    erase_dir_except(data_dir, keep, &mut report)?;
    if separate_logs {
        remove_path(log_dir, &mut report)?;
    }
    Ok(report)
}

/// The S-27 stop/unload/erase/relaunch sequence. A failed phase stops the
/// checklist; relaunch is never attempted after a partial erase error.
pub fn erase_in_order<E>(
    stop_jobs: impl FnOnce() -> Result<(), E>,
    unload_models: impl FnOnce() -> Result<(), E>,
    erase: impl FnOnce() -> Result<(), E>,
    relaunch: impl FnOnce(),
) -> Result<(), E> {
    stop_jobs()?;
    unload_models()?;
    erase()?;
    relaunch();
    Ok(())
}

#[cfg(test)]
#[path = "erase_tests.rs"]
mod tests;
