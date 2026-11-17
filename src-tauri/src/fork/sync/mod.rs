// SPDX-License-Identifier: GPL-3.0-or-later
//! Sync between the user's devices (Mac and iPhone) without an account, a
//! server or a cloud: one file, carried by the user (AirDrop, Files, a
//! cable), merged on the other side.
//!
//! ```text
//!   device A                     file                     device B
//!   store::collect ─► bundle::write ─►  .wisprbro  ─► bundle::read ─► merge::merge
//!                                       Mac export ─► legacy::read ─┘
//!                                       CSV        ─► csv_file::read ┘
//! ```
//!
//! What a file carries ([`Payload`]) and how it merges ([`merge`]):
//!
//! | Data       | Key                 | Rule                                              |
//! | ---------- | ------------------- | ------------------------------------------------- |
//! | Dictionary | kind + folded word  | newest `updated_at` wins; a deletion is a tombstone |
//! | Snippets   | folded trigger      | the same                                          |
//! | Settings   | setting name        | newest change wins, per setting ([`settings`])    |
//! | Stats      | device id + day     | each device's days are kept as that device's snapshot and replaced whole, so totals add up and never double |
//!
//! History text and audio are never in a `.wisprbro` file. The Mac's
//! "Export my data" zip does contain them; [`legacy`] reads only its
//! database snapshot and the two settings files.
//!
//! The merge is transport-agnostic: it takes a [`Payload`] and a
//! [`Manifest`], whatever brought them. The next transport (the local
//! network) is sketched at the end of `docs/fork/IOS.md`.

pub mod bundle;
pub mod csv_file;
#[cfg(target_os = "ios")]
pub mod ios;
pub mod legacy;
pub mod merge;
pub mod settings;
pub mod store;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use specta::Type;

/// File extension of a sync file.
pub const BUNDLE_EXTENSION: &str = "wisprbro";
/// `Manifest::format` of a sync file.
pub const FORMAT: &str = "wispr-bro-sync";
/// The format this build writes; a larger one is refused.
pub const FORMAT_VERSION: u32 = 1;
/// The device id given to a Mac whose file has none ("Export my data" of
/// the Mac app before it could sync).
pub const LEGACY_MAC_ID: &str = "mac-export";

/// Who made a file, and when.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Manifest {
    pub format: String,
    pub version: u32,
    pub device_id: String,
    pub device_name: String,
    /// `macos` or `ios`.
    pub platform: String,
    pub app_version: String,
    /// Unix ms.
    pub created_at: i64,
}

/// A setting's value and when it last changed (Unix ms).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stamped {
    pub value: serde_json::Value,
    pub updated_at: i64,
}

/// A dictionary word (`replacement` empty) or replacement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DictItem {
    pub phrase: String,
    #[serde(default)]
    pub replacement: Option<String>,
    /// `user`, `import`, `flag`, `learn`, `migrated`.
    #[serde(default)]
    pub source: String,
    /// `active`, or `rejected` (a learned word the user deleted).
    #[serde(default)]
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnippetItem {
    pub trigger: String,
    pub body: String,
    #[serde(default)]
    pub source: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A deleted key: `kind` is `word`, `rule` or `snippet`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tombstone {
    pub kind: String,
    pub norm: String,
    pub deleted_at: i64,
}

/// One `daily_stats` row of the device that made the file.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayStats {
    pub day: String,
    pub words: i64,
    pub dictations: i64,
    #[serde(default)]
    pub speech_ms: i64,
    #[serde(default)]
    pub timed_words: i64,
    #[serde(default)]
    pub fixes_dictionary: i64,
    #[serde(default)]
    pub fixes_llm: i64,
}

/// One `daily_app_stats` row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppDayStats {
    pub day: String,
    pub usage_category: String,
    pub words: i64,
    pub dictations: i64,
}

/// Everything a file carries.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Payload {
    #[serde(default)]
    pub settings: BTreeMap<String, Stamped>,
    #[serde(default)]
    pub dictionary: Vec<DictItem>,
    #[serde(default)]
    pub snippets: Vec<SnippetItem>,
    #[serde(default)]
    pub tombstones: Vec<Tombstone>,
    #[serde(default)]
    pub stats: Vec<DayStats>,
    #[serde(default)]
    pub app_stats: Vec<AppDayStats>,
}

/// What kind of file was read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// A `.wisprbro` sync file.
    Bundle,
    /// The Mac app's "Export my data" zip.
    MacExport,
    DictionaryCsv,
    SnippetsCsv,
}

/// A file, read.
#[derive(Clone, Debug, PartialEq)]
pub struct Incoming {
    pub kind: SourceKind,
    pub manifest: Manifest,
    pub payload: Payload,
}

/// Why a file was not read. The page has one sentence for each.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "problem", rename_all = "snake_case")]
pub enum ReadProblem {
    /// Missing or unreadable.
    Unreadable,
    /// Not a sync file, a Mac export or a Wispr Bro CSV.
    NotWisprBro,
    /// Written by a newer Wispr Bro.
    TooNew,
    /// Made on this device.
    OwnFile,
}

/// Reads any file the importer takes: a `.wisprbro` file, the Mac's export
/// zip, or a Dictionary / Snippets CSV. `now_ms` stamps what has no time of
/// its own (CSV rows).
pub fn read(path: &std::path::Path, now_ms: i64) -> Result<Incoming, ReadProblem> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension == "csv" {
        return csv_file::read(path, now_ms);
    }
    let file = std::fs::File::open(path).map_err(|_| ReadProblem::Unreadable)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| ReadProblem::NotWisprBro)?;
    if archive.index_for_name(bundle::PAYLOAD_NAME).is_some() {
        bundle::read(&mut archive)
    } else if archive.index_for_name(legacy::DATABASE_NAME).is_some() {
        legacy::read(&mut archive)
    } else {
        Err(ReadProblem::NotWisprBro)
    }
}
