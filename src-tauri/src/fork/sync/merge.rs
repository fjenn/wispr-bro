// SPDX-License-Identifier: GPL-3.0-or-later
//! The merge: what a file from another device changes here.
//!
//! [`merge`] runs inside a transaction the caller owns. The review step
//! runs the same merge and rolls back, so the numbers shown before are the
//! numbers applied after.
//!
//! Rules (the table in [`super`]):
//!
//! - an entry is found by its key (kind + folded word, folded trigger);
//! - the side with the newer `updated_at` wins; at the same time the
//!   larger text does, so both devices end the same;
//! - a deletion is a tombstone with its time: it removes an entry that was
//!   not edited after it, and an entry edited after it survives and clears
//!   it;
//! - merging the same file again changes nothing;
//! - nothing in a file is trusted: every entry goes through the same
//!   `clean` the pages use, and the stores' limits hold.

use std::collections::BTreeMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::{settings, store, Incoming, Payload, SourceKind, LEGACY_MAC_ID};
use crate::fork::context::categories::UsageCategory;
use crate::fork::db::DbError;
use crate::fork::dictionary::{self, EntryInput, EntryKind, EntrySource};
use crate::fork::snippets::{self, SnippetInput, SnippetOrigin};

/// Which parts of a file to take.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SyncChoices {
    pub settings: bool,
    pub dictionary: bool,
    pub snippets: bool,
    pub stats: bool,
}

impl SyncChoices {
    pub const ALL: SyncChoices = SyncChoices {
        settings: true,
        dictionary: true,
        snippets: true,
        stats: true,
    };
}

/// What happened to one list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SyncCounts {
    pub added: u32,
    pub changed: u32,
    pub removed: u32,
    /// Not taken: invalid here, or the list is full.
    pub skipped: u32,
}

/// What a merge does (review) or did (result).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct MergeReport {
    pub dictionary: SyncCounts,
    pub snippets: SyncCounts,
    /// The settings whose value changes here.
    pub settings: Vec<String>,
    /// Days and words in the other device's stats.
    pub stats_days: u32,
    pub stats_words: u64,
    /// Whether those stats are new or different from what is stored.
    pub stats_changed: bool,
}

/// The report plus the setting values the caller has to write.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    pub report: MergeReport,
    pub settings: BTreeMap<String, serde_json::Value>,
}

/// Whether the incoming side replaces the local one.
fn incoming_wins(local_at: i64, local: &str, incoming_at: i64, incoming: &str) -> bool {
    incoming_at > local_at || (incoming_at == local_at && incoming > local)
}

fn tombstone_at(conn: &Connection, kind: &str, norm: &str) -> Result<Option<i64>, DbError> {
    Ok(conn
        .query_row(
            "SELECT deleted_at FROM sync_tombstones WHERE kind = ?1 AND norm = ?2",
            params![kind, norm],
            |r| r.get(0),
        )
        .optional()?)
}

/// Keeps the later of the stored and the given deletion time.
fn put_tombstone(conn: &Connection, kind: &str, norm: &str, at: i64) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO sync_tombstones (kind, norm, deleted_at) VALUES (?1, ?2, ?3)
         ON CONFLICT (kind, norm) DO UPDATE SET deleted_at = MAX(deleted_at, excluded.deleted_at)",
        params![kind, norm, at],
    )?;
    Ok(())
}

/// Sets a deletion time exactly (after a delete whose trigger wrote "now").
fn set_tombstone(conn: &Connection, kind: &str, norm: &str, at: i64) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO sync_tombstones (kind, norm, deleted_at) VALUES (?1, ?2, ?3)
         ON CONFLICT (kind, norm) DO UPDATE SET deleted_at = excluded.deleted_at",
        params![kind, norm, at],
    )?;
    Ok(())
}

// ── Dictionary ──────────────────────────────────────────────────────────

struct LocalEntry {
    id: i64,
    phrase: String,
    replacement: Option<String>,
    status: String,
    updated_at: i64,
}

impl LocalEntry {
    fn text(&self) -> String {
        entry_text(&self.status, &self.phrase, self.replacement.as_deref())
    }
}

/// What two sides are compared by when their times are equal.
fn entry_text(status: &str, phrase: &str, replacement: Option<&str>) -> String {
    format!("{status}\u{1f}{phrase}\u{1f}{}", replacement.unwrap_or(""))
}

fn local_entry(conn: &Connection, kind: &str, norm: &str) -> Result<Option<LocalEntry>, DbError> {
    Ok(conn
        .query_row(
            "SELECT id, CASE kind WHEN 'word' THEN term ELSE from_text END, to_text, status,
                    COALESCE(updated_at, created_at)
             FROM dictionary_entries WHERE kind = ?1 AND norm = ?2",
            params![kind, norm],
            |r| {
                Ok(LocalEntry {
                    id: r.get(0)?,
                    phrase: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    replacement: r.get(2)?,
                    status: r.get(3)?,
                    updated_at: r.get(4)?,
                })
            },
        )
        .optional()?)
}

fn active_entries(conn: &Connection) -> Result<usize, DbError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM dictionary_entries WHERE status = 'active'",
        [],
        |r| r.get(0),
    )?;
    Ok(usize::try_from(n).unwrap_or(usize::MAX))
}

fn merge_dictionary(conn: &Connection, payload: &Payload) -> Result<SyncCounts, DbError> {
    let mut counts = SyncCounts::default();
    for t in &payload.tombstones {
        if EntryKind::from_sql(&t.kind).is_none() {
            continue;
        }
        match local_entry(conn, &t.kind, &t.norm)? {
            Some(local) if t.deleted_at >= local.updated_at => {
                conn.execute("DELETE FROM dictionary_entries WHERE id = ?1", [local.id])?;
                set_tombstone(conn, &t.kind, &t.norm, t.deleted_at)?;
                if local.status == "active" {
                    counts.removed += 1;
                }
            }
            // Edited here after it was deleted there: it stays.
            Some(_) => {}
            None => put_tombstone(conn, &t.kind, &t.norm, t.deleted_at)?,
        }
    }
    let mut active = active_entries(conn)?;
    for item in &payload.dictionary {
        let Ok(clean) = dictionary::clean(&EntryInput {
            phrase: item.phrase.clone(),
            replacement: item.replacement.clone(),
        }) else {
            counts.skipped += 1;
            continue;
        };
        let kind = clean.kind.as_sql();
        let status = if item.status == "rejected" {
            "rejected"
        } else {
            "active"
        };
        let source = EntrySource::from_sql(&item.source).as_sql();
        let (term, from) = match clean.kind {
            EntryKind::Word => (Some(clean.phrase.as_str()), None),
            EntryKind::Rule => (None, Some(clean.phrase.as_str())),
        };
        let theirs = entry_text(status, &clean.phrase, clean.replacement.as_deref());
        match local_entry(conn, kind, &clean.norm)? {
            Some(local) => {
                let ours = local.text();
                if !incoming_wins(local.updated_at, &ours, item.updated_at, &theirs) {
                    continue;
                }
                if ours != theirs {
                    let was_active = local.status == "active";
                    if status == "active" && !was_active && active >= dictionary::MAX_ENTRIES {
                        counts.skipped += 1;
                        continue;
                    }
                    conn.execute(
                        "UPDATE dictionary_entries
                         SET term = ?2, from_text = ?3, to_text = ?4, status = ?5, source = ?6
                         WHERE id = ?1",
                        params![local.id, term, from, clean.replacement, status, source],
                    )?;
                    match (was_active, status == "active") {
                        (true, true) => counts.changed += 1,
                        (true, false) => {
                            counts.removed += 1;
                            active -= 1;
                        }
                        (false, true) => {
                            counts.added += 1;
                            active += 1;
                        }
                        (false, false) => {}
                    }
                }
                // After the row's own triggers: the time is the winner's.
                conn.execute(
                    "UPDATE dictionary_entries SET updated_at = ?2 WHERE id = ?1",
                    params![local.id, item.updated_at],
                )?;
            }
            None => {
                if tombstone_at(conn, kind, &clean.norm)?.is_some_and(|at| at >= item.updated_at) {
                    continue;
                }
                if status == "active" {
                    if active >= dictionary::MAX_ENTRIES {
                        counts.skipped += 1;
                        continue;
                    }
                    active += 1;
                    counts.added += 1;
                }
                conn.execute(
                    "INSERT INTO dictionary_entries (kind, term, from_text, to_text, norm, source,
                                                     status, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        kind,
                        term,
                        from,
                        clean.replacement,
                        clean.norm,
                        source,
                        status,
                        item.created_at.min(item.updated_at),
                        item.updated_at
                    ],
                )?;
            }
        }
    }
    Ok(counts)
}

// ── Snippets ────────────────────────────────────────────────────────────

const SNIPPET: &str = "snippet";

struct LocalSnippet {
    id: i64,
    trigger: String,
    body: String,
    updated_at: i64,
}

fn snippet_text(trigger: &str, body: &str) -> String {
    format!("{trigger}\u{1f}{body}")
}

fn local_snippet(conn: &Connection, norm: &str) -> Result<Option<LocalSnippet>, DbError> {
    Ok(conn
        .query_row(
            r#"SELECT id, "trigger", body, updated_at FROM snippets WHERE trigger_norm = ?1"#,
            [norm],
            |r| {
                Ok(LocalSnippet {
                    id: r.get(0)?,
                    trigger: r.get(1)?,
                    body: r.get(2)?,
                    updated_at: r.get(3)?,
                })
            },
        )
        .optional()?)
}

fn merge_snippets(conn: &Connection, payload: &Payload) -> Result<SyncCounts, DbError> {
    let mut counts = SyncCounts::default();
    for t in payload.tombstones.iter().filter(|t| t.kind == SNIPPET) {
        match local_snippet(conn, &t.norm)? {
            Some(local) if t.deleted_at >= local.updated_at => {
                conn.execute("DELETE FROM snippets WHERE id = ?1", [local.id])?;
                set_tombstone(conn, SNIPPET, &t.norm, t.deleted_at)?;
                counts.removed += 1;
            }
            Some(_) => {}
            None => put_tombstone(conn, SNIPPET, &t.norm, t.deleted_at)?,
        }
    }
    let mut total: i64 = conn.query_row("SELECT COUNT(*) FROM snippets", [], |r| r.get(0))?;
    for item in &payload.snippets {
        let Ok(clean) = snippets::clean(&SnippetInput {
            trigger: item.trigger.clone(),
            body: item.body.clone(),
        }) else {
            counts.skipped += 1;
            continue;
        };
        let source = SnippetOrigin::from_sql(&item.source).as_sql();
        let theirs = snippet_text(&clean.trigger, &clean.body);
        match local_snippet(conn, &clean.norm)? {
            Some(local) => {
                let ours = snippet_text(&local.trigger, &local.body);
                if !incoming_wins(local.updated_at, &ours, item.updated_at, &theirs) {
                    continue;
                }
                if ours != theirs {
                    counts.changed += 1;
                }
                conn.execute(
                    r#"UPDATE snippets SET "trigger" = ?2, body = ?3, source = ?4, updated_at = ?5
                       WHERE id = ?1"#,
                    params![local.id, clean.trigger, clean.body, source, item.updated_at],
                )?;
            }
            None => {
                if tombstone_at(conn, SNIPPET, &clean.norm)?.is_some_and(|at| at >= item.updated_at)
                {
                    continue;
                }
                if usize::try_from(total).unwrap_or(usize::MAX) >= snippets::MAX_SNIPPETS {
                    counts.skipped += 1;
                    continue;
                }
                conn.execute(
                    r#"INSERT INTO snippets ("trigger", trigger_norm, body, source, created_at,
                                             updated_at)
                       VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
                    params![
                        clean.trigger,
                        clean.norm,
                        clean.body,
                        source,
                        item.created_at.min(item.updated_at),
                        item.updated_at
                    ],
                )?;
                total += 1;
                counts.added += 1;
            }
        }
    }
    Ok(counts)
}

// ── Stats ───────────────────────────────────────────────────────────────

fn valid_day(day: &str) -> bool {
    day.len() == 10 && chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d").is_ok()
}

fn forget_stats(conn: &Connection, device_id: &str) -> Result<(), DbError> {
    conn.execute(
        "DELETE FROM sync_device_stats WHERE device_id = ?1",
        [device_id],
    )?;
    conn.execute(
        "DELETE FROM sync_device_app_stats WHERE device_id = ?1",
        [device_id],
    )?;
    Ok(())
}

/// A device's snapshot as one comparable value.
fn snapshot(conn: &Connection, device_id: &str) -> Result<Vec<(String, i64, i64, i64)>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT day, words, dictations, speech_ms FROM sync_device_stats
         WHERE device_id = ?1 ORDER BY day",
    )?;
    let rows = stmt
        .query_map([device_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Whether a Mac that names itself has synced here. Its plain export is
/// then the same Mac again and must not be counted a second time.
fn named_mac(conn: &Connection) -> Result<bool, DbError> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_devices
                       WHERE is_self = 0 AND platform = 'macos' AND device_id <> ?1)",
        [LEGACY_MAC_ID],
        |r| r.get(0),
    )?)
}

/// Replaces the stored snapshot of the file's device with the file's days.
fn merge_stats(
    conn: &Connection,
    incoming: &Incoming,
    report: &mut MergeReport,
) -> Result<(), DbError> {
    let id = incoming.manifest.device_id.as_str();
    let before = snapshot(conn, id)?;
    forget_stats(conn, id)?;
    for d in incoming.payload.stats.iter().filter(|d| valid_day(&d.day)) {
        conn.execute(
            "INSERT OR REPLACE INTO sync_device_stats
                 (device_id, day, words, dictations, speech_ms, timed_words, fixes_dictionary,
                  fixes_llm)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                d.day,
                d.words.max(0),
                d.dictations.max(0),
                d.speech_ms.max(0),
                d.timed_words.max(0),
                d.fixes_dictionary.max(0),
                d.fixes_llm.max(0)
            ],
        )?;
    }
    for a in incoming
        .payload
        .app_stats
        .iter()
        .filter(|a| valid_day(&a.day) && UsageCategory::parse(&a.usage_category).is_some())
    {
        conn.execute(
            "INSERT OR REPLACE INTO sync_device_app_stats
                 (device_id, day, usage_category, words, dictations)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                id,
                a.day,
                a.usage_category,
                a.words.max(0),
                a.dictations.max(0)
            ],
        )?;
    }
    let after = snapshot(conn, id)?;
    report.stats_days = u32::try_from(after.len()).unwrap_or(u32::MAX);
    report.stats_words = after
        .iter()
        .map(|(_, words, _, _)| u64::try_from(*words).unwrap_or(0))
        .sum();
    report.stats_changed = before != after;
    Ok(())
}

// ── The merge ───────────────────────────────────────────────────────────

/// Merges `incoming` into the database behind `conn` (a transaction of the
/// caller). Settings: run [`settings::observe`] first; the returned values
/// are for the caller to write to the settings stores after the commit.
pub fn merge(
    conn: &Connection,
    incoming: &Incoming,
    choices: SyncChoices,
    now_ms: i64,
) -> Result<Outcome, DbError> {
    let mut outcome = Outcome::default();
    if choices.dictionary {
        outcome.report.dictionary = merge_dictionary(conn, &incoming.payload)?;
    }
    if choices.snippets {
        outcome.report.snippets = merge_snippets(conn, &incoming.payload)?;
    }
    if choices.settings {
        outcome.settings = settings::merge(conn, &incoming.payload.settings)?;
        outcome.report.settings = outcome.settings.keys().cloned().collect();
    }
    let device = matches!(incoming.kind, SourceKind::Bundle | SourceKind::MacExport);
    let superseded = incoming.kind == SourceKind::MacExport && named_mac(conn)?;
    if incoming.kind == SourceKind::Bundle && incoming.manifest.platform == "macos" {
        // The Mac now names itself: its unnamed snapshot would count twice.
        forget_stats(conn, LEGACY_MAC_ID)?;
        conn.execute(
            "DELETE FROM sync_devices WHERE device_id = ?1 AND is_self = 0",
            [LEGACY_MAC_ID],
        )?;
    }
    if device && !superseded {
        if choices.stats {
            merge_stats(conn, incoming, &mut outcome.report)?;
        }
        store::remember_device(conn, &incoming.manifest, now_ms)?;
    }
    Ok(outcome)
}
