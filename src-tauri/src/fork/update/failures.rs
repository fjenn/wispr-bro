// SPDX-License-Identifier: GPL-3.0-or-later
//! Failed update installs and the 24-hour pause (S-28).
//!
//! A download whose signature does not verify, or an install that cannot
//! replace the bundle, counts as one failed install. After
//! [`MAX_FAILURES`] in a row the background scheduler stops trying for
//! [`PAUSE`]; a manual "Retry" still works. The log is kept in
//! `update-state.json` in the app data dir (deleted with everything else by
//! Delete all, S-27), so a relaunch does not restart the count.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Failed installs in a row before the pause.
pub const MAX_FAILURES: u32 = 3;
/// How long background updates pause after [`MAX_FAILURES`].
pub const PAUSE: Duration = Duration::from_secs(24 * 60 * 60);
/// File name in the app data dir.
pub const STATE_FILE: &str = "update-state.json";

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
#[serde(default)]
pub struct FailureLog {
    /// The version that failed last.
    pub version: Option<String>,
    /// Failed installs in a row.
    pub count: u32,
    /// Unix ms until which background updates pause.
    pub paused_until_ms: Option<i64>,
}

impl FailureLog {
    /// Records one failed install of `version`; returns `true` when this one
    /// started the pause.
    pub fn record(&mut self, version: &str, now_ms: i64) -> bool {
        self.expire(now_ms);
        self.version = Some(version.to_string());
        self.count = self.count.saturating_add(1);
        if self.count >= MAX_FAILURES && self.paused_until_ms.is_none() {
            let pause = i64::try_from(PAUSE.as_millis()).unwrap_or(i64::MAX);
            self.paused_until_ms = Some(now_ms.saturating_add(pause));
            return true;
        }
        false
    }

    /// Whether background updates are paused at `now_ms`.
    pub fn is_paused(&self, now_ms: i64) -> bool {
        self.paused_until_ms.is_some_and(|until| now_ms < until)
    }

    /// Ends an elapsed pause (the count starts again from zero). Returns
    /// whether anything changed.
    pub fn expire(&mut self, now_ms: i64) -> bool {
        if self.paused_until_ms.is_some_and(|until| now_ms >= until) {
            *self = FailureLog::default();
            return true;
        }
        false
    }

    /// Forgets everything (an install succeeded).
    pub fn clear(&mut self) -> bool {
        let changed = *self != FailureLog::default();
        *self = FailureLog::default();
        changed
    }

    /// At launch: once the running app is at or past the version that
    /// failed, that version evidently installed (by hand, or a later
    /// attempt), so the log is stale.
    pub fn on_launch(&mut self, current_version: &str) -> bool {
        let landed = match (&self.version, semver::Version::parse(current_version)) {
            (Some(failed), Ok(current)) => {
                semver::Version::parse(failed).is_ok_and(|failed| current >= failed)
            }
            _ => false,
        };
        landed && self.clear()
    }
}

/// `<dir>/update-state.json`.
pub fn state_path(dir: &Path) -> PathBuf {
    dir.join(STATE_FILE)
}

/// Reads the log; a missing or unreadable file is an empty log.
pub fn load(path: &Path) -> FailureLog {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            log::warn!("update: {STATE_FILE} unreadable ({e}); starting over");
            FailureLog::default()
        }),
        Err(_) => FailureLog::default(),
    }
}

/// Writes the log atomically (temp file, then rename).
pub fn save(path: &Path, log: &FailureLog) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(log).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3_600_000;

    #[test]
    fn three_failures_pause_background_updates_for_a_day() {
        let mut log = FailureLog::default();
        assert!(!log.record("1.0.1", 0));
        assert!(!log.record("1.0.1", HOUR));
        assert!(!log.is_paused(HOUR));
        assert!(log.record("1.0.1", 2 * HOUR));
        assert_eq!(log.count, 3);
        assert!(log.is_paused(2 * HOUR));
        assert!(log.is_paused(25 * HOUR));
        assert!(!log.is_paused(26 * HOUR));
        // A further failure during the pause does not extend it.
        assert!(!log.record("1.0.1", 3 * HOUR));
        assert_eq!(log.paused_until_ms, Some(26 * HOUR));
    }

    #[test]
    fn the_pause_expires_and_the_count_starts_again() {
        let mut log = FailureLog::default();
        for i in 0..3 {
            log.record("1.0.1", i);
        }
        assert!(log.expire(30 * HOUR));
        assert_eq!(log, FailureLog::default());
        assert!(!log.record("1.0.2", 30 * HOUR));
        assert_eq!(log.count, 1);
    }

    #[test]
    fn a_landed_version_clears_the_log() {
        let mut log = FailureLog::default();
        log.record("1.0.1", 0);
        assert!(!log.on_launch("1.0.0"));
        assert_eq!(log.count, 1);
        assert!(log.on_launch("1.0.1"));
        assert_eq!(log, FailureLog::default());
        log.record("1.0.0-beta.2", 0);
        assert!(log.on_launch("1.0.0"));
    }

    #[test]
    fn the_log_round_trips_and_a_bad_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = state_path(dir.path());
        assert_eq!(load(&path), FailureLog::default());
        let mut log = FailureLog::default();
        log.record("1.0.1", 5);
        save(&path, &log).unwrap();
        assert_eq!(load(&path), log);
        std::fs::write(&path, b"{not json").unwrap();
        assert_eq!(load(&path), FailureLog::default());
    }
}
