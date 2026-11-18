// SPDX-License-Identifier: GPL-3.0-or-later
//! This device's side of a sync: who it is, what it sends ([`collect`]),
//! and the devices it has heard from.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::{
    AppDayStats, DayStats, DictItem, Manifest, Payload, SnippetItem, Tombstone, FORMAT,
    FORMAT_VERSION,
};
use crate::fork::db::DbError;

/// `macos` or `ios`: the platform this build runs on.
pub fn platform() -> &'static str {
    if cfg!(target_os = "ios") {
        "ios"
    } else {
        "macos"
    }
}

/// This device's id (made by migration 0010).
pub fn self_id(conn: &Connection) -> Result<String, DbError> {
    Ok(conn.query_row(
        "SELECT device_id FROM sync_devices WHERE is_self = 1",
        [],
        |r| r.get(0),
    )?)
}

/// A device a file was merged from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SyncDevice {
    pub device_id: String,
    pub name: String,
    pub platform: String,
    pub app_version: String,
    /// When its last file was merged here (Unix ms).
    pub last_import_at: Option<i64>,
    /// Words in its stats snapshot here.
    pub words: u64,
}

/// The other devices, the most recently merged first.
pub fn devices(conn: &Connection) -> Result<Vec<SyncDevice>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT d.device_id, d.name, d.platform, d.app_version, d.last_import_at,
                COALESCE((SELECT SUM(words) FROM sync_device_stats s
                          WHERE s.device_id = d.device_id), 0)
         FROM sync_devices d WHERE d.is_self = 0
         ORDER BY d.last_import_at DESC, d.device_id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(SyncDevice {
                device_id: r.get(0)?,
                name: r.get(1)?,
                platform: r.get(2)?,
                app_version: r.get(3)?,
                last_import_at: r.get(4)?,
                words: u64::try_from(r.get::<_, i64>(5)?).unwrap_or(0),
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// When this device last made a file (Unix ms).
pub fn last_export_at(conn: &Connection) -> Result<Option<i64>, DbError> {
    Ok(conn.query_row(
        "SELECT last_export_at FROM sync_devices WHERE is_self = 1",
        [],
        |r| r.get(0),
    )?)
}

pub fn mark_exported(conn: &Connection, now_ms: i64) -> Result<(), DbError> {
    conn.execute(
        "UPDATE sync_devices SET last_export_at = ?1 WHERE is_self = 1",
        [now_ms],
    )?;
    Ok(())
}

/// Whether Home and Insights show more than this device's own numbers.
pub fn other_device_count(conn: &Connection) -> Result<u32, DbError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT device_id) FROM sync_device_stats",
        [],
        |r| r.get(0),
    )?;
    Ok(u32::try_from(n).unwrap_or(0))
}

/// Dictionary entries as a file carries them: active ones, and the
/// `rejected` rows deleted learned words leave. Suggestions stay here.
pub fn dictionary(conn: &Connection) -> Result<Vec<DictItem>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT CASE kind WHEN 'word' THEN term ELSE from_text END, to_text, source, status,
                created_at, COALESCE(updated_at, created_at)
         FROM dictionary_entries
         WHERE status IN ('active', 'rejected') AND norm IS NOT NULL
         ORDER BY created_at, id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(DictItem {
                phrase: r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                replacement: r.get(1)?,
                source: r.get(2)?,
                status: r.get(3)?,
                created_at: r.get(4)?,
                updated_at: r.get(5)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn snippets(conn: &Connection) -> Result<Vec<SnippetItem>, DbError> {
    let mut stmt = conn.prepare(
        r#"SELECT "trigger", body, source, created_at, updated_at FROM snippets
           ORDER BY created_at, id"#,
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(SnippetItem {
                trigger: r.get(0)?,
                body: r.get(1)?,
                source: r.get(2)?,
                created_at: r.get(3)?,
                updated_at: r.get(4)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// This device's own daily numbers (never the other devices' snapshots).
pub fn stats(conn: &Connection) -> Result<(Vec<DayStats>, Vec<AppDayStats>), DbError> {
    let mut stmt = conn.prepare(
        "SELECT day, words, dictations, speech_ms, timed_words, fixes_dictionary, fixes_llm
         FROM daily_stats ORDER BY day",
    )?;
    let days = stmt
        .query_map([], |r| {
            Ok(DayStats {
                day: r.get(0)?,
                words: r.get(1)?,
                dictations: r.get(2)?,
                speech_ms: r.get(3)?,
                timed_words: r.get(4)?,
                fixes_dictionary: r.get(5)?,
                fixes_llm: r.get(6)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    let mut stmt = conn.prepare(
        "SELECT day, usage_category, words, dictations FROM daily_app_stats
         ORDER BY day, usage_category",
    )?;
    let apps = stmt
        .query_map([], |r| {
            Ok(AppDayStats {
                day: r.get(0)?,
                usage_category: r.get(1)?,
                words: r.get(2)?,
                dictations: r.get(3)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok((days, apps))
}

fn tombstones(conn: &Connection) -> Result<Vec<Tombstone>, DbError> {
    let mut stmt =
        conn.prepare("SELECT kind, norm, deleted_at FROM sync_tombstones ORDER BY kind, norm")?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Tombstone {
                kind: r.get(0)?,
                norm: r.get(1)?,
                deleted_at: r.get(2)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// What this device sends. Run [`super::settings::observe`] first so the
/// settings' times are current.
pub fn collect(
    conn: &Connection,
    device_name: &str,
    app_version: &str,
    now_ms: i64,
) -> Result<(Manifest, Payload), DbError> {
    let (stats, app_stats) = stats(conn)?;
    // This device's own name, for the title of a note conflict copy.
    conn.execute(
        "UPDATE sync_devices SET name = ?1, platform = ?2
         WHERE is_self = 1 AND (name <> ?1 OR platform <> ?2)",
        params![device_name, platform()],
    )?;
    let (notes, note_tombstones) = super::notes::collect(conn)?;
    let manifest = Manifest {
        format: FORMAT.to_owned(),
        version: FORMAT_VERSION,
        device_id: self_id(conn)?,
        device_name: device_name.to_owned(),
        platform: platform().to_owned(),
        app_version: app_version.to_owned(),
        created_at: now_ms,
        minor: super::secure::MINOR,
    };
    let payload = Payload {
        settings: super::settings::chosen(conn)?,
        dictionary: dictionary(conn)?,
        snippets: snippets(conn)?,
        tombstones: tombstones(conn)?,
        stats,
        app_stats,
        notes,
        note_tombstones,
    };
    Ok((manifest, payload))
}

/// Records that a file from `manifest`'s device was merged.
pub fn remember_device(conn: &Connection, manifest: &Manifest, now_ms: i64) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO sync_devices (device_id, name, platform, app_version, last_import_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (device_id) DO UPDATE SET
             name = excluded.name, platform = excluded.platform,
             app_version = excluded.app_version, last_import_at = excluded.last_import_at",
        params![
            manifest.device_id,
            manifest.device_name,
            manifest.platform,
            manifest.app_version,
            now_ms
        ],
    )?;
    Ok(())
}
