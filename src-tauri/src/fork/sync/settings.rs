// SPDX-License-Identifier: GPL-3.0-or-later
//! The settings that follow the user from device to device, and when each
//! last changed.
//!
//! Only what means the same on a Mac and an iPhone is synced ([`FORK_KEYS`]
//! from the fork's settings, [`HANDY_KEYS`] from Handy's). Shortcuts, the
//! bar, the Dock, the microphone, updates and the iPhone's session settings
//! stay where they are.
//!
//! Neither settings store records when a value changed, and the fork does
//! not add fields to them. `sync_settings` keeps the last seen value of
//! each key with a time: [`observe`] compares it with the current value and
//! stamps a difference "now". It runs at start, after every fork setting
//! change, and before a file is made or merged, so a stamp is at most as
//! late as the next of those. A value that was never seen different from
//! the default has time 0, counts as not chosen, and is not sent: a fresh
//! install never overrides a setting someone chose.

use std::collections::BTreeMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

use super::Stamped;
use crate::fork::db::DbError;
use crate::fork::settings::{validate, ForkSettingUpdate};

/// Synced keys of the fork's settings (`ForkSettings` field names; each has
/// a `ForkSettingUpdate` variant of the same name).
pub const FORK_KEYS: &[&str] = &[
    "cleanup_level",
    "styles",
    "text_retention_days",
    "audio_retention_days",
    "typing_wpm",
    "typing_hours_per_day",
    "greeting_name",
];

/// Synced keys of Handy's settings (`AppSettings` field names).
pub const HANDY_KEYS: &[&str] = &[
    "spoken_languages",
    "audio_feedback",
    "audio_feedback_volume",
];

pub fn is_fork_key(key: &str) -> bool {
    FORK_KEYS.contains(&key)
}

pub fn is_synced(key: &str) -> bool {
    is_fork_key(key) || HANDY_KEYS.contains(&key)
}

/// The synced keys of two settings objects (each the serialized struct).
pub fn pick(fork: &Value, handy: Option<&Value>) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for (keys, object) in [(FORK_KEYS, Some(fork)), (HANDY_KEYS, handy)] {
        let Some(object) = object else { continue };
        for key in keys {
            if let Some(value) = object.get(*key) {
                out.insert((*key).to_owned(), value.clone());
            }
        }
    }
    out
}

/// The fork update that sets `key` to `value`, if it is a valid one.
pub fn fork_update(key: &str, value: &Value) -> Option<ForkSettingUpdate> {
    if !is_fork_key(key) {
        return None;
    }
    let update: ForkSettingUpdate =
        serde_json::from_value(serde_json::json!({ "key": key, "value": value })).ok()?;
    validate(&update).ok()?;
    Some(update)
}

/// Whether `value` may be written to `key` here. A file is data from
/// outside: nothing in it is trusted.
pub fn valid(key: &str, value: &Value) -> bool {
    match key {
        "spoken_languages" => value.as_array().is_some_and(|codes| {
            codes.len() <= 64
                && codes.iter().all(|c| {
                    c.as_str().is_some_and(|c| {
                        !c.is_empty()
                            && c.len() <= 16
                            && c.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
                    })
                })
        }),
        "audio_feedback" => value.is_boolean(),
        "audio_feedback_volume" => value.as_f64().is_some_and(|v| (0.0..=1.0).contains(&v)),
        _ => fork_update(key, value).is_some(),
    }
}

fn row(conn: &Connection, key: &str) -> Result<Option<Stamped>, DbError> {
    Ok(conn
        .query_row(
            "SELECT value, updated_at FROM sync_settings WHERE key = ?1",
            [key],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
        )
        .optional()?
        .map(|(value, updated_at)| Stamped {
            value: serde_json::from_str(&value).unwrap_or(Value::Null),
            updated_at,
        }))
}

fn put(conn: &Connection, key: &str, value: &Value, updated_at: i64) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO sync_settings (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value.to_string(), updated_at],
    )?;
    Ok(())
}

/// Records the current values: a key seen for the first time gets time 0
/// when it is the default and `now_ms` otherwise; a key whose value changed
/// since it was last seen gets `now_ms`.
pub fn observe(
    conn: &Connection,
    current: &BTreeMap<String, Value>,
    defaults: &BTreeMap<String, Value>,
    now_ms: i64,
) -> Result<(), DbError> {
    for (key, value) in current {
        match row(conn, key)? {
            None => {
                let stamp = if defaults.get(key) == Some(value) {
                    0
                } else {
                    now_ms
                };
                put(conn, key, value, stamp)?;
            }
            Some(seen) if seen.value != *value => put(conn, key, value, now_ms)?,
            Some(_) => {}
        }
    }
    Ok(())
}

/// What a file from this device carries: every synced key someone chose.
pub fn chosen(conn: &Connection) -> Result<BTreeMap<String, Stamped>, DbError> {
    let mut stmt =
        conn.prepare("SELECT key, value, updated_at FROM sync_settings WHERE updated_at > 0")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
        ))
    })?;
    let mut out = BTreeMap::new();
    for row in rows {
        let (key, value, updated_at) = row?;
        if let (true, Ok(value)) = (is_synced(&key), serde_json::from_str(&value)) {
            out.insert(key, Stamped { value, updated_at });
        }
    }
    Ok(out)
}

/// Whether the incoming value replaces the local one: the newer change
/// wins; at the same time the larger JSON text does, so two devices agree.
fn incoming_wins(local: Option<&Stamped>, incoming: &Stamped) -> bool {
    match local {
        None => true,
        Some(local) if incoming.updated_at == local.updated_at => {
            let (theirs, ours) = (incoming.value.to_string(), local.value.to_string());
            theirs > ours
        }
        Some(local) => incoming.updated_at > local.updated_at,
    }
}

/// Merges `incoming` into `sync_settings` and returns the keys whose value
/// here must change, with the new value. The caller writes them to the
/// settings stores once the transaction is committed. Call [`observe`]
/// first.
pub fn merge(
    conn: &Connection,
    incoming: &BTreeMap<String, Stamped>,
) -> Result<BTreeMap<String, Value>, DbError> {
    let mut changes = BTreeMap::new();
    for (key, theirs) in incoming {
        if !is_synced(key) || theirs.updated_at <= 0 || !valid(key, &theirs.value) {
            continue;
        }
        let ours = row(conn, key)?;
        if !incoming_wins(ours.as_ref(), theirs) {
            continue;
        }
        if ours.as_ref().map(|o| &o.value) != Some(&theirs.value) {
            changes.insert(key.clone(), theirs.value.clone());
        }
        put(conn, key, &theirs.value, theirs.updated_at)?;
    }
    Ok(changes)
}
