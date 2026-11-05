// SPDX-License-Identifier: GPL-3.0-or-later
//
// What a correction becomes follows BetterWispr's `CorrectionDecision`:
//   Source: BetterWispr, https://github.com/opennookorg/betterwispr
//   Commit: 304335a073a98f7439e5f7b4fbc2c31b810b807e
//   File:   Sources/BetterWisprCore/Transcription/CorrectionReviewer.swift
//           (`CorrectionDecision.entry()`, `entries(for:in:)`: everyday
//           words are never rewritten, terms the vocabulary already has
//           are skipped, a cap on what is added; tests:
//           Tests/BetterWisprCoreTests/CorrectionLearningTests.swift)
//   That file adapts VoiceInk (https://github.com/Beingpax/VoiceInk,
//   c09cc1f677f40f2ee665843a61f07670210f012e) and is GPL-3.0
//   (BetterWispr docs/oss-reuse.md).
//   Modified by Wispr Bro contributors, 2026-11-02: ported to Rust; the
//   notes-model review is not ported (DEC-20: the user approves every
//   suggestion instead); a fix of an everyday word is dropped rather than
//   turned into a word (the dictionary's words are fuzzy-applied by
//   Handy); a suggestion waits in `dictionary_entries` (status
//   `suggested`), a rejected one stays as a `rejected` row so it never
//   comes back; the cap is on waiting suggestions.
//
//! The approval queue (S-34, DEC-20): learned corrections wait in
//! Dictionary › Suggested until the user accepts or rejects them.
//!
//! A correction becomes one suggestion:
//!
//! - a **word** (the corrected spelling) when it is plain ASCII: Handy's
//!   fuzzy word fix and the recognizer's vocabulary then spell it right
//!   however the recognizer mangles it next time;
//! - a **replacement** "heard → corrected" otherwise, since Handy's fuzzy
//!   fix is ASCII-only (S-16: "Siobhán" is not fixed as a word).
//!
//! Never suggested: a fix of an everyday word ([`super::diff::is_common`]),
//! anything the dictionary already has in any state (active, waiting or
//! rejected: rejected words never return), more than [`MAX_PENDING`]
//! waiting suggestions. Only the word pair is stored; `corrections` gets
//! the same pair (source `learn`, S-25's "most corrected word").

use rusqlite::{params, Connection, OptionalExtension};

use super::diff::{is_common, Correction};
use crate::fork::db::DbError;
use crate::fork::dictionary::{clean, norm_of, CleanEntry, EntryInput, EntryKind, MAX_ENTRIES};

/// Most suggestions waiting for approval; later ones are dropped.
pub const MAX_PENDING: usize = 100;

/// Most suggestions one edit adds.
pub const MAX_PER_EDIT: usize = 5;

/// The entry a correction would suggest, if any.
pub fn entry_for(c: &Correction) -> Option<CleanEntry> {
    if is_common(&c.heard) {
        return None;
    }
    let input = if c.corrected.is_ascii() {
        EntryInput {
            phrase: c.corrected.clone(),
            replacement: None,
        }
    } else {
        EntryInput {
            phrase: c.heard.clone(),
            replacement: Some(c.corrected.clone()),
        }
    };
    clean(&input).ok()
}

/// Whether the dictionary already has `entry`'s terms in any state: a
/// word or "from" with the same key, or a replacement that already writes
/// the corrected spelling.
fn known(conn: &Connection, entry: &CleanEntry) -> Result<bool, DbError> {
    let mut terms = vec![entry.norm.clone()];
    if let Some(to) = &entry.replacement {
        terms.push(norm_of(to));
    }
    let mut stmt = conn.prepare_cached(
        "SELECT norm, to_text FROM dictionary_entries WHERE norm IS NOT NULL OR to_text IS NOT NULL",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, Option<String>>(0)?,
            r.get::<_, Option<String>>(1)?,
        ))
    })?;
    for row in rows {
        let (norm, to) = row?;
        let hit = |t: &str| terms.iter().any(|x| x == t);
        if norm.as_deref().is_some_and(hit)
            || to.as_deref().map(norm_of).as_deref().is_some_and(hit)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn pending(conn: &Connection) -> Result<usize, DbError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM dictionary_entries WHERE status = 'suggested'",
        [],
        |r| r.get(0),
    )?;
    Ok(usize::try_from(n).unwrap_or(usize::MAX))
}

/// A suggestion [`suggest_entries`] added: its row and the word the bar's
/// note names (the corrected spelling).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggested {
    pub id: i64,
    pub word: String,
}

/// Adds the suggestions `corrections` give (in one transaction). Returns
/// how many were added.
pub fn suggest(
    conn: &mut Connection,
    corrections: &[Correction],
    now_ms: i64,
) -> Result<usize, DbError> {
    suggest_entries(conn, corrections, now_ms).map(|s| s.len())
}

/// [`suggest`], returning what was added.
pub fn suggest_entries(
    conn: &mut Connection,
    corrections: &[Correction],
    now_ms: i64,
) -> Result<Vec<Suggested>, DbError> {
    let tx = conn.transaction()?;
    let mut added = Vec::new();
    for c in corrections {
        if added.len() == MAX_PER_EDIT || pending(&tx)? >= MAX_PENDING {
            break;
        }
        let Some(entry) = entry_for(c) else { continue };
        if known(&tx, &entry)? {
            continue;
        }
        tx.execute(
            "INSERT INTO dictionary_entries (kind, term, from_text, to_text, norm, source,
                                             status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'learn', 'suggested', ?6, ?6)",
            params![
                entry.kind.as_sql(),
                (entry.kind == EntryKind::Word).then_some(entry.phrase.as_str()),
                (entry.kind == EntryKind::Rule).then_some(entry.phrase.as_str()),
                entry.replacement.as_deref(),
                entry.norm,
                now_ms
            ],
        )?;
        let id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO corrections (entry_id, from_text, to_text, source, created_at)
             VALUES (NULL, ?1, ?2, 'learn', ?3)",
            params![c.heard, c.corrected, now_ms],
        )?;
        added.push(Suggested {
            id,
            word: entry.replacement.clone().unwrap_or(entry.phrase.clone()),
        });
    }
    tx.commit()?;
    Ok(added)
}

/// What accepting suggestions did.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type,
)]
pub struct AcceptOutcome {
    /// Now active.
    pub accepted: u32,
    /// Left waiting because the dictionary is full.
    pub full: u32,
}

/// Makes waiting suggestions active (their source stays `learn`).
pub fn accept(conn: &mut Connection, ids: &[i64], now_ms: i64) -> Result<AcceptOutcome, DbError> {
    let tx = conn.transaction()?;
    let mut out = AcceptOutcome::default();
    for &id in ids {
        let waiting: Option<i64> = tx
            .query_row(
                "SELECT id FROM dictionary_entries WHERE id = ?1 AND status = 'suggested'",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        if waiting.is_none() {
            continue;
        }
        let active: i64 = tx.query_row(
            "SELECT COUNT(*) FROM dictionary_entries WHERE status = 'active'",
            [],
            |r| r.get(0),
        )?;
        if usize::try_from(active).unwrap_or(usize::MAX) >= MAX_ENTRIES {
            out.full += 1;
            continue;
        }
        tx.execute(
            "UPDATE dictionary_entries SET status = 'active', updated_at = ?2 WHERE id = ?1",
            params![id, now_ms],
        )?;
        out.accepted += 1;
    }
    tx.commit()?;
    Ok(out)
}

/// Rejects waiting suggestions: they stay as `rejected` rows (never shown,
/// never suggested again) and their `corrections` rows go. Returns how
/// many were rejected.
pub fn reject(conn: &mut Connection, ids: &[i64], now_ms: i64) -> Result<u32, DbError> {
    let tx = conn.transaction()?;
    let mut n = 0;
    for &id in ids {
        let row: Option<(Option<String>, Option<String>)> = tx
            .query_row(
                "SELECT term, to_text FROM dictionary_entries
                 WHERE id = ?1 AND status = 'suggested'",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((term, to)) = row else { continue };
        tx.execute(
            "UPDATE dictionary_entries SET status = 'rejected', updated_at = ?2 WHERE id = ?1",
            params![id, now_ms],
        )?;
        if let Some(corrected) = to.or(term) {
            tx.execute(
                "DELETE FROM corrections WHERE source = 'learn' AND to_text = ?1",
                [corrected],
            )?;
        }
        n += 1;
    }
    tx.commit()?;
    Ok(n)
}
