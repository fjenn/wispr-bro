// SPDX-License-Identifier: GPL-3.0-or-later
//! Handy's history, as the fork may use it (R5 §4.3): read rows and delete
//! them **only through Handy's `HistoryManager` API**, never by SQL on
//! `history.db`. Deleting a WAV file alone (audio retention) is a file
//! operation on Handy's recordings dir; Handy has no API for it and keeps
//! working when a row's audio is missing.

use std::path::PathBuf;

use crate::managers::history::HistoryEntry;

/// The part of Handy's `HistoryManager` the fork's jobs use. Implemented for
/// the real manager below and by `FakeHistory` in tests.
pub trait HandyHistory: Send + Sync {
    /// Every row, newest first.
    fn entries(&self) -> Result<Vec<HistoryEntry>, String>;
    /// Up to `limit` rows older than row `cursor` (all rows without one),
    /// newest first, and whether more follow (S-15b's History list).
    fn page(&self, cursor: Option<i64>, limit: usize) -> Result<(Vec<HistoryEntry>, bool), String>;
    /// One row.
    fn entry(&self, id: i64) -> Result<Option<HistoryEntry>, String>;
    /// Deletes the row and its WAV through Handy (which then calls
    /// `fork::db::on_deleted`).
    fn delete_entry(&self, id: i64) -> Result<(), String>;
    /// Where the row's WAV lives.
    fn audio_path(&self, file_name: &str) -> PathBuf;
}

mod real {
    use std::sync::Arc;

    use super::*;
    use crate::managers::history::HistoryManager;

    /// Handy's manager. Its async methods do no awaiting inside (plain
    /// rusqlite), so they are driven with `block_on`; callers are the fork's
    /// own threads, never an async task.
    pub struct HandyHistoryManager(pub Arc<HistoryManager>);

    impl HandyHistory for HandyHistoryManager {
        fn entries(&self) -> Result<Vec<HistoryEntry>, String> {
            tauri::async_runtime::block_on(self.0.get_history_entries(None, None))
                .map(|page| page.entries)
                .map_err(|e| e.to_string())
        }

        fn page(
            &self,
            cursor: Option<i64>,
            limit: usize,
        ) -> Result<(Vec<HistoryEntry>, bool), String> {
            tauri::async_runtime::block_on(self.0.get_history_entries(cursor, Some(limit)))
                .map(|page| (page.entries, page.has_more))
                .map_err(|e| e.to_string())
        }

        fn entry(&self, id: i64) -> Result<Option<HistoryEntry>, String> {
            tauri::async_runtime::block_on(self.0.get_entry_by_id(id)).map_err(|e| e.to_string())
        }

        fn delete_entry(&self, id: i64) -> Result<(), String> {
            tauri::async_runtime::block_on(self.0.delete_entry(id)).map_err(|e| e.to_string())
        }

        fn audio_path(&self, file_name: &str) -> PathBuf {
            self.0.get_audio_file_path(file_name)
        }
    }
}

pub use real::HandyHistoryManager;

#[cfg(test)]
pub mod fake {
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::sync::Mutex;

    use super::*;

    /// In-memory Handy history with a real recordings dir (so WAV deletion
    /// is observable). Records every call.
    pub struct FakeHistory {
        pub rows: Mutex<BTreeMap<i64, HistoryEntry>>,
        pub dir: PathBuf,
        pub deleted: Mutex<Vec<i64>>,
        pub fail_delete: Mutex<bool>,
    }

    impl FakeHistory {
        pub fn new(dir: &Path) -> Self {
            Self {
                rows: Mutex::new(BTreeMap::new()),
                dir: dir.to_path_buf(),
                deleted: Mutex::new(Vec::new()),
                fail_delete: Mutex::new(false),
            }
        }

        /// A Handy row; writes `file_name` with `wav_ms` of silence when
        /// given.
        pub fn add(&self, entry: HistoryEntry, wav_ms: Option<u32>) {
            if let Some(ms) = wav_ms {
                let samples = vec![0.0f32; (ms as usize) * 16];
                crate::audio_toolkit::save_wav_file(self.dir.join(&entry.file_name), &samples)
                    .unwrap();
            }
            self.rows.lock().unwrap().insert(entry.id, entry);
        }

        pub fn ids(&self) -> Vec<i64> {
            self.rows.lock().unwrap().keys().copied().collect()
        }
    }

    pub fn entry(id: i64, ts_secs: i64, text: &str) -> HistoryEntry {
        HistoryEntry {
            id,
            file_name: format!("wispr-bro-{id}.wav"),
            timestamp: ts_secs,
            saved: false,
            title: String::new(),
            transcription_text: text.to_owned(),
            post_processed_text: None,
            post_process_prompt: None,
            post_process_requested: false,
            source_name: None,
            language: None,
            language_source: None,
        }
    }

    impl HandyHistory for FakeHistory {
        fn entries(&self) -> Result<Vec<HistoryEntry>, String> {
            Ok(self.rows.lock().unwrap().values().rev().cloned().collect())
        }
        fn page(
            &self,
            cursor: Option<i64>,
            limit: usize,
        ) -> Result<(Vec<HistoryEntry>, bool), String> {
            // Handy caps a page at 100 rows.
            let limit = limit.min(100);
            let rows = self.rows.lock().unwrap();
            let mut older: Vec<HistoryEntry> = rows
                .values()
                .rev()
                .filter(|e| cursor.is_none_or(|c| e.id < c))
                .take(limit + 1)
                .cloned()
                .collect();
            let more = older.len() > limit;
            older.truncate(limit);
            Ok((older, more))
        }
        fn entry(&self, id: i64) -> Result<Option<HistoryEntry>, String> {
            Ok(self.rows.lock().unwrap().get(&id).cloned())
        }
        fn delete_entry(&self, id: i64) -> Result<(), String> {
            if *self.fail_delete.lock().unwrap() {
                return Err("delete failed".into());
            }
            if let Some(e) = self.rows.lock().unwrap().remove(&id) {
                let _ = std::fs::remove_file(self.dir.join(e.file_name));
            }
            self.deleted.lock().unwrap().push(id);
            Ok(())
        }
        fn audio_path(&self, file_name: &str) -> PathBuf {
            self.dir.join(file_name)
        }
    }
}
