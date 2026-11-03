// SPDX-License-Identifier: GPL-3.0-or-later
//! The quiet relaunch after an update (S-28).
//!
//! Tauri's restart starts the new process with the old arguments, so a
//! relaunch would show the Hub (and take focus) whenever the app was not
//! started with `--start-hidden`. Before restarting, the scheduler leaves a
//! small marker in the log dir; `fork::cli_args` (the `main.rs` hook) reads
//! and deletes it at the next start and sets `start_hidden` when the Hub was
//! hidden, so the app comes back to the bar without a window.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};

/// File name in the log dir.
pub const MARKER: &str = "update-relaunch.json";
/// A marker older than this is stale (a crash between writing it and the
/// restart) and ignored.
pub const MAX_AGE_MS: i64 = 10 * 60 * 1000;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
struct Marker {
    hidden: bool,
    at_ms: i64,
}

/// Writes the marker for the next start.
pub fn write_marker(dir: &Path, hidden: bool, now_ms: i64) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(&Marker {
        hidden,
        at_ms: now_ms,
    })
    .map_err(|e| e.to_string())?;
    std::fs::write(dir.join(MARKER), bytes).map_err(|e| e.to_string())
}

/// Reads and deletes the marker: `Some(hidden)` when this start is the
/// relaunch after an update.
pub fn take_marker(dir: &Path, now_ms: i64) -> Option<bool> {
    let path = dir.join(MARKER);
    let bytes = std::fs::read(&path).ok()?;
    let _ = std::fs::remove_file(&path);
    let marker: Marker = serde_json::from_slice(&bytes).ok()?;
    let age = now_ms.saturating_sub(marker.at_ms);
    (0..=MAX_AGE_MS).contains(&age).then_some(marker.hidden)
}

static RELAUNCHED: OnceLock<bool> = OnceLock::new();

/// At startup (before Tauri): applies a marker in `dir` to the CLI flags.
pub fn apply_at_start(dir: Option<PathBuf>, start_hidden: &mut bool) {
    let Some(dir) = dir else { return };
    if let Some(hidden) = take_marker(&dir, chrono::Utc::now().timestamp_millis()) {
        if hidden {
            *start_hidden = true;
        }
        let _ = RELAUNCHED.set(hidden);
    }
}

/// `Some(hidden)` when this process is the relaunch after an update.
pub fn relaunched() -> Option<bool> {
    RELAUNCHED.get().copied()
}

/// Writes the marker and restarts through Tauri (the exit events run first,
/// so the single-instance socket is gone before the new process starts).
pub fn relaunch<R: Runtime>(app: &AppHandle<R>, hidden: bool) {
    match crate::fork::app_log_dir(app) {
        Some(dir) => {
            if let Err(e) = write_marker(&dir, hidden, chrono::Utc::now().timestamp_millis()) {
                log::warn!("update: could not write the relaunch marker: {e}");
            }
        }
        None => log::warn!("update: no log dir for the relaunch marker"),
    }
    app.request_restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_marker_is_read_once_and_only_while_fresh() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(take_marker(dir.path(), 0), None);
        write_marker(dir.path(), true, 1_000).unwrap();
        assert_eq!(take_marker(dir.path(), 2_000), Some(true));
        assert_eq!(
            take_marker(dir.path(), 2_000),
            None,
            "deleted after reading"
        );
        write_marker(dir.path(), false, 0).unwrap();
        assert_eq!(take_marker(dir.path(), 5_000), Some(false));
        write_marker(dir.path(), true, 0).unwrap();
        assert_eq!(take_marker(dir.path(), MAX_AGE_MS + 1), None, "stale");
        assert!(!dir.path().join(MARKER).exists());
    }

    #[test]
    fn a_hidden_marker_sets_start_hidden_and_a_shown_one_keeps_the_flag() {
        let dir = tempfile::tempdir().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        write_marker(dir.path(), false, now).unwrap();
        let mut hidden = false;
        apply_at_start(Some(dir.path().to_path_buf()), &mut hidden);
        assert!(!hidden);
        write_marker(dir.path(), true, now).unwrap();
        apply_at_start(Some(dir.path().to_path_buf()), &mut hidden);
        assert!(hidden);
        let mut untouched = false;
        apply_at_start(None, &mut untouched);
        assert!(!untouched);
    }
}
