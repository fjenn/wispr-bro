// SPDX-License-Identifier: GPL-3.0-or-later
//! Handy's `custom_words` and the dictionary's words (S-16, C-58, C-68).
//!
//! The dictionary is the source of truth; Handy's setting is a mirror of
//! its active words, because Handy's recognizer path reads its own setting
//! (decoder vocabulary, else the fuzzy word fix). No Handy file changes:
//! the list is written through Handy's settings store, like Handy's own
//! `update_custom_words` command does, and Handy's settings window is told
//! to re-read it.
//!
//! Handy's own editor (Settings › Advanced, until S-14 replaces the hosted
//! settings) can still change the list, so a sync is a three-way merge
//! against the list the fork last wrote (`dictionary_handy_mirror`):
//!
//! - a word in Handy's list but not in the mirror was added there and is
//!   taken into the dictionary (source `migrated`). On the first sync the
//!   mirror is empty, so this is the one-time migration of a Handy user's
//!   existing words; afterwards a word deleted on the Dictionary page is
//!   never taken back in;
//! - a word in the mirror but no longer in Handy's list was removed there
//!   and leaves the dictionary too;
//! - then Handy's list is set to the dictionary's active words, read back,
//!   and what Handy really holds becomes the new mirror (a failed write
//!   can therefore never look like a removal).

use std::collections::HashSet;
use std::sync::Mutex;

use rusqlite::{params, Connection};
use tauri::{AppHandle, Manager};

use super::{clean, store, EntryInput, EntryKind, EntrySource};
use crate::fork::db::{DbError, ForkDb};
use crate::fork::ForkRuntime;

/// One sync at a time (it reads, merges and writes Handy's settings).
static SYNC: Mutex<()> = Mutex::new(());

/// What a sync did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    /// Words taken over from Handy's list.
    pub taken_in: u32,
    /// Dictionary words removed because Handy's list dropped them.
    pub removed: u32,
    /// Handy's list was rewritten.
    pub wrote_handy: bool,
}

impl SyncReport {
    /// The dictionary itself changed.
    pub fn dictionary_changed(&self) -> bool {
        self.taken_in > 0 || self.removed > 0
    }
}

/// The list last written into Handy's setting.
pub fn mirror(conn: &Connection) -> Result<Vec<String>, DbError> {
    let mut stmt =
        conn.prepare_cached("SELECT word FROM dictionary_handy_mirror ORDER BY position")?;
    let words = stmt
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(words)
}

fn set_mirror(conn: &Connection, words: &[String]) -> Result<(), DbError> {
    conn.execute("DELETE FROM dictionary_handy_mirror", [])?;
    let mut insert = conn
        .prepare_cached("INSERT INTO dictionary_handy_mirror (position, word) VALUES (?1, ?2)")?;
    for (i, w) in words.iter().enumerate() {
        insert.execute(params![i64::try_from(i).unwrap_or(i64::MAX), w])?;
    }
    Ok(())
}

/// Step 1 of a sync, on the database: merges Handy's current list into the
/// dictionary. Returns the words Handy should hold now.
pub fn merge_from_handy(
    conn: &mut Connection,
    handy: &[String],
    now_ms: i64,
    report: &mut SyncReport,
) -> Result<Vec<String>, DbError> {
    let tx = conn.transaction()?;
    store::ensure_norms(&tx)?;
    let last: Vec<String> = mirror(&tx)?;
    let last_set: HashSet<&str> = last.iter().map(String::as_str).collect();
    let handy_set: HashSet<&str> = handy.iter().map(String::as_str).collect();

    for word in handy.iter().filter(|w| !last_set.contains(w.as_str())) {
        let input = EntryInput {
            phrase: word.clone(),
            replacement: None,
        };
        let Ok(entry) = clean(&input) else {
            log::debug!("dictionary: skipped an invalid Handy custom word");
            continue;
        };
        if let store::SaveOutcome::Saved { .. } =
            store::add(&tx, &entry, EntrySource::Migrated, now_ms)?
        {
            report.taken_in += 1;
        }
    }
    for word in last.iter().filter(|w| !handy_set.contains(w.as_str())) {
        let n = tx.execute(
            "DELETE FROM dictionary_entries
             WHERE kind = ?1 AND status = 'active' AND term = ?2",
            params![EntryKind::Word.as_sql(), word],
        )?;
        report.removed += u32::try_from(n).unwrap_or(u32::MAX);
    }
    let desired = store::active_words(&tx)?;
    tx.commit()?;
    Ok(desired)
}

/// Records what Handy holds after the write.
pub fn record_mirror(conn: &Connection, handy_now: &[String]) -> Result<(), DbError> {
    set_mirror(conn, handy_now)
}

/// Syncs the dictionary's words with Handy's `custom_words`. Returns `None`
/// when Handy's settings or `fork.db` are not available (headless, tests
/// without a Handy store).
pub fn sync<R: ForkRuntime>(app: &AppHandle<R>) -> Option<SyncReport> {
    let _one = SYNC.lock().unwrap_or_else(|e| e.into_inner());
    let db = app.try_state::<ForkDb>()?;
    let before = R::handy_settings(app)?;
    let handy_words = before.0.custom_words.clone();
    let mut report = SyncReport::default();
    let now = crate::fork::context::now_ms();
    let desired = match db.with_conn(|c| merge_from_handy(c, &handy_words, now, &mut report)) {
        Ok(d) => d,
        Err(e) => {
            log::warn!("dictionary: could not merge Handy's custom words: {e}");
            return None;
        }
    };
    let mut held = handy_words;
    if desired != held {
        let mut after = crate::fork::settings::HandySettings(before.0.clone());
        after.0.custom_words = desired.clone();
        R::write_handy_settings(
            app,
            &before,
            after,
            &crate::fork::settings::ProductReport::default(),
        );
        held = R::handy_settings(app)
            .map(|s| s.0.custom_words)
            .unwrap_or_default();
        report.wrote_handy = true;
        if held != desired {
            log::warn!("dictionary: Handy's custom words did not take the new list");
        }
    }
    if let Err(e) = db.with_conn(|c| record_mirror(c, &held)) {
        log::warn!("dictionary: could not record the custom-words mirror: {e}");
    }
    if report.dictionary_changed() || report.wrote_handy {
        log::info!(
            "dictionary: synced with Handy's custom words (taken in {}, removed {}, {} word(s) now)",
            report.taken_in,
            report.removed,
            held.len()
        );
    }
    if report.dictionary_changed() {
        super::notify_changed(app);
    }
    Some(report)
}
