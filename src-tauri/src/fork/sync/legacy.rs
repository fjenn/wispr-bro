// SPDX-License-Identifier: GPL-3.0-or-later
//! The Mac app's "Export my data" zip (`fork::export`), read as a sync
//! source, so a Mac that cannot make a sync file yet (1.0.0-alpha.6 and
//! older) still syncs to the iPhone.
//!
//! Of the export's contents only three entries are read:
//! `databases/fork.db` (dictionary, snippets, daily stats) and the two
//! settings files. History (`history.csv`, `history.json`) and the
//! recordings are never opened. The database snapshot also holds history
//! rows; it is unpacked to a private temporary file, read, and deleted.
//!
//! Such a file has no device id and no change times: the Mac is
//! [`LEGACY_MAC_ID`], its settings carry the time the export was made, and
//! it knows no deletions (a word deleted on the Mac but still on the phone
//! stays on the phone).

use std::collections::BTreeMap;
use std::io::{Read, Seek};
use std::path::PathBuf;

use serde_json::Value;

use super::bundle::{entry_bytes, MAX_ENTRY_BYTES};
use super::{
    settings, store, Incoming, Manifest, Payload, ReadProblem, SourceKind, Stamped, FORMAT,
    LEGACY_MAC_ID,
};

pub const DATABASE_NAME: &str = "databases/fork.db";
const FORK_SETTINGS_NAME: &str = "settings/fork-settings.json";
const HANDY_SETTINGS_NAME: &str = "settings/settings_store.json";
/// A database larger than this is not unpacked.
const MAX_DATABASE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// The unpacked database; removed when dropped.
struct Unpacked(PathBuf);

impl Drop for Unpacked {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn unpack_database<R: Read + Seek>(archive: &mut zip::ZipArchive<R>) -> Option<Unpacked> {
    let mut entry = archive.by_name(DATABASE_NAME).ok()?;
    if entry.size() > MAX_DATABASE_BYTES {
        return None;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("wispr-bro-sync-{}-{nanos}.db", std::process::id()));
    let out = Unpacked(path);
    let mut file = std::fs::File::create_new(&out.0).ok()?;
    std::io::copy(&mut (&mut entry).take(MAX_DATABASE_BYTES), &mut file).ok()?;
    Some(out)
}

/// The `settings` object of a store file (`{"settings": {...}}`).
fn store_settings<R: Read + Seek>(archive: &mut zip::ZipArchive<R>, name: &str) -> Option<Value> {
    let bytes = entry_bytes(archive, name, MAX_ENTRY_BYTES)?;
    let mut value: Value = serde_json::from_slice(&bytes).ok()?;
    Some(value.get_mut("settings")?.take())
}

/// Reads a Mac export archive.
pub fn read<R: Read + Seek>(archive: &mut zip::ZipArchive<R>) -> Result<Incoming, ReadProblem> {
    let manifest: Value = entry_bytes(archive, "manifest.json", MAX_ENTRY_BYTES)
        .and_then(|b| serde_json::from_slice(&b).ok())
        .ok_or(ReadProblem::NotWisprBro)?;
    if manifest.get("product").and_then(Value::as_str) != Some("Wispr Bro") {
        return Err(ReadProblem::NotWisprBro);
    }
    if manifest.get("schema_version").and_then(Value::as_u64)
        > Some(u64::from(crate::fork::export::EXPORT_SCHEMA_VERSION))
    {
        return Err(ReadProblem::TooNew);
    }
    let created_at = manifest
        .get("created_at")
        .and_then(Value::as_str)
        .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
        .map_or(1, |t| t.timestamp_millis().max(1));

    let fork = store_settings(archive, FORK_SETTINGS_NAME);
    let handy = store_settings(archive, HANDY_SETTINGS_NAME);
    let settings: BTreeMap<String, Stamped> =
        settings::pick(fork.as_ref().unwrap_or(&Value::Null), handy.as_ref())
            .into_iter()
            .map(|(key, value)| {
                (
                    key,
                    Stamped {
                        value,
                        updated_at: created_at,
                    },
                )
            })
            .collect();

    let database = unpack_database(archive).ok_or(ReadProblem::Unreadable)?;
    let conn = rusqlite::Connection::open(&database.0).map_err(|_| ReadProblem::Unreadable)?;
    // A table an older Mac did not have yet reads as empty.
    let dictionary = store::dictionary(&conn).unwrap_or_default();
    let snippets = store::snippets(&conn).unwrap_or_default();
    let (stats, app_stats) = store::stats(&conn).unwrap_or_default();
    drop(conn);

    Ok(Incoming {
        kind: SourceKind::MacExport,
        manifest: Manifest {
            format: FORMAT.to_owned(),
            version: 0,
            device_id: LEGACY_MAC_ID.to_owned(),
            device_name: String::new(),
            platform: "macos".to_owned(),
            app_version: manifest
                .get("app_version")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            created_at,
            minor: 0,
        },
        payload: Payload {
            settings,
            dictionary,
            snippets,
            tombstones: Vec::new(),
            stats,
            app_stats,
            ..Payload::default()
        },
    })
}
