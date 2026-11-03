// SPDX-License-Identifier: GPL-3.0-or-later
//! User-data export (S-27): a schema-versioned zip made on this Mac.
//!
//! Contents: `manifest.json` (schema version), History as `history.csv` and
//! `history.json` (rows read through Handy's `HistoryManager`, R5 §4.3), a
//! `VACUUM INTO` snapshot of the fork's own `fork.db` (consistent while the
//! app runs), both settings stores and the recordings. Models and logs are
//! not user data and are excluded.

use std::io::Write;
use std::path::{Path, PathBuf};

use rusqlite::OpenFlags;
use serde::Serialize;
use zip::write::SimpleFileOptions;

use crate::fork::db::DB_FILE_NAME;
use crate::managers::history::HistoryEntry;

pub const EXPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportReport {
    pub files: u32,
    pub bytes: u64,
}

#[derive(Serialize)]
struct Manifest<'a> {
    schema_version: u32,
    product: &'a str,
    app_version: &'a str,
    created_at: String,
}

struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new() -> Result<Self, String> {
        let unique = format!(
            "wispr-bro-export-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        std::fs::create_dir(&path)
            .map_err(|e| format!("could not create export workspace: {e}"))?;
        Ok(Self(path))
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn options() -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)
}

fn check_output(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("choose an absolute export path".into());
    }
    if !path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
    {
        return Err("the export file name must end in .zip".into());
    }
    if !path.parent().is_some_and(Path::is_dir) {
        return Err("the export folder does not exist".into());
    }
    Ok(())
}

fn snapshot_database(source: &Path, destination: &Path) -> Result<bool, String> {
    if !source.is_file() {
        return Ok(false);
    }
    let conn = rusqlite::Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("could not open {}: {e}", source.display()))?;
    let quoted = destination.to_string_lossy().replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{quoted}'"))
        .map_err(|e| format!("could not snapshot {}: {e}", source.display()))?;
    Ok(true)
}

fn add_bytes<W: Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    name: &str,
    bytes: &[u8],
) -> Result<(), String> {
    zip.start_file(name, options())
        .map_err(|e| format!("could not add {name} to the export: {e}"))?;
    zip.write_all(bytes)
        .map_err(|e| format!("could not add {name} to the export: {e}"))
}

fn add_file<W: Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    name: &str,
    path: &Path,
) -> Result<bool, String> {
    if !path.is_file() {
        return Ok(false);
    }
    // Streamed: recordings can add up to gigabytes.
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    zip.start_file(name, options())
        .map_err(|e| format!("could not add {name} to the export: {e}"))?;
    std::io::copy(&mut file, zip)
        .map_err(|e| format!("could not add {name} to the export: {e}"))?;
    Ok(true)
}

/// History rows as CSV (one row per entry, oldest first), from Handy's
/// `HistoryManager` (R5 §4.3: the fork never runs SQL on Handy's database).
fn history_csv(entries: &[HistoryEntry]) -> Result<Vec<u8>, String> {
    let mut rows: Vec<&HistoryEntry> = entries.iter().collect();
    rows.sort_by_key(|e| (e.timestamp, e.id));
    let mut csv = csv::Writer::from_writer(Vec::new());
    csv.write_record([
        "id",
        "timestamp",
        "title",
        "file_name",
        "transcription_text",
        "post_processed_text",
        "source_name",
        "language",
        "saved",
    ])
    .map_err(|e| e.to_string())?;
    for e in rows {
        csv.write_record([
            e.id.to_string(),
            e.timestamp.to_string(),
            e.title.clone(),
            e.file_name.clone(),
            e.transcription_text.clone(),
            e.post_processed_text.clone().unwrap_or_default(),
            e.source_name.clone().unwrap_or_default(),
            e.language.clone().unwrap_or_default(),
            e.saved.to_string(),
        ])
        .map_err(|e| e.to_string())?;
    }
    csv.into_inner().map_err(|e| e.to_string())
}

fn add_recordings<W: Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    root: &Path,
    current: &Path,
    count: &mut u32,
) -> Result<(), String> {
    let Ok(entries) = std::fs::read_dir(current) else {
        return Ok(());
    };
    let mut entries = entries
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let file_type = entry.file_type().map_err(|e| e.to_string())?;
        let path = entry.path();
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            add_recordings(zip, root, &path, count)?;
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            if add_file(zip, &format!("recordings/{relative}"), &path)? {
                *count += 1;
            }
        }
    }
    Ok(())
}

fn write_archive(
    data_dir: &Path,
    history: &[HistoryEntry],
    scratch: &ScratchDir,
    output: &Path,
) -> Result<ExportReport, String> {
    let fork = scratch.join(DB_FILE_NAME);
    snapshot_database(&data_dir.join(DB_FILE_NAME), &fork)?;

    let file = std::fs::File::create_new(output).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let manifest = Manifest {
        schema_version: EXPORT_SCHEMA_VERSION,
        product: "Wispr Bro",
        app_version: env!("CARGO_PKG_VERSION"),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    let manifest = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    add_bytes(&mut zip, "manifest.json", &manifest)?;
    let mut files = 1;

    add_bytes(&mut zip, "history.csv", &history_csv(history)?)?;
    let json = serde_json::to_vec_pretty(history).map_err(|e| e.to_string())?;
    add_bytes(&mut zip, "history.json", &json)?;
    files += 2;
    for (name, path) in [
        ("databases/fork.db", fork.as_path()),
        (
            "settings/settings_store.json",
            data_dir.join("settings_store.json").as_path(),
        ),
        (
            "settings/fork-settings.json",
            data_dir.join("fork-settings.json").as_path(),
        ),
    ] {
        if add_file(&mut zip, name, path)? {
            files += 1;
        }
    }
    add_recordings(
        &mut zip,
        &data_dir.join("recordings"),
        &data_dir.join("recordings"),
        &mut files,
    )?;
    let file = zip.finish().map_err(|e| e.to_string())?;
    let bytes = file.metadata().map_err(|e| e.to_string())?.len();
    Ok(ExportReport { files, bytes })
}

/// Exports the app-owned data under `data_dir` plus Handy's `history` rows
/// (read by the caller through `fork::db::handy`) to `output`, atomically via
/// a sibling temporary file. Nothing touches the network.
pub fn export_data(
    data_dir: &Path,
    history: &[HistoryEntry],
    output: &Path,
) -> Result<ExportReport, String> {
    check_output(output)?;
    let scratch = ScratchDir::new()?;
    let temporary = output.with_extension(format!(
        "zip.{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    ));
    match write_archive(data_dir, history, &scratch, &temporary) {
        Ok(report) => {
            std::fs::rename(&temporary, output).map_err(|e| {
                let _ = std::fs::remove_file(&temporary);
                format!("could not finish the export: {e}")
            })?;
            Ok(report)
        }
        Err(error) => {
            let _ = std::fs::remove_file(&temporary);
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Read;

    fn history() -> Vec<HistoryEntry> {
        let mut e = crate::fork::db::handy::fake::entry(7, 1234, "private, \"quoted\" words");
        e.post_processed_text = Some("clean words".into());
        e.language = Some("en".into());
        vec![e]
    }

    #[test]
    fn export_is_schema_versioned_complete_and_excludes_models_and_logs() {
        let temp = tempfile::tempdir().unwrap();
        let data = temp.path().join("Data");
        std::fs::create_dir_all(data.join("recordings/nested")).unwrap();
        rusqlite::Connection::open(data.join(DB_FILE_NAME))
            .unwrap()
            .execute_batch("CREATE TABLE marker (value TEXT); INSERT INTO marker VALUES ('fork');")
            .unwrap();
        std::fs::write(data.join("settings_store.json"), "{\"settings\":{}}").unwrap();
        std::fs::write(data.join("fork-settings.json"), "{\"settings\":{}}").unwrap();
        std::fs::write(data.join("recordings/nested/take.wav"), b"RIFF-audio").unwrap();
        std::fs::create_dir(data.join("models")).unwrap();
        std::fs::write(data.join("models/model.gguf"), "not exported").unwrap();
        std::fs::create_dir(data.join("logs")).unwrap();
        std::fs::write(data.join("logs/wispr-bro.log"), "not exported").unwrap();
        let output = temp.path().join("my-data.zip");

        let report = export_data(&data, &history(), &output).unwrap();
        assert_eq!(report.files, 7);
        assert_eq!(report.bytes, std::fs::metadata(&output).unwrap().len());
        let mut archive = zip::ZipArchive::new(std::fs::File::open(&output).unwrap()).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_owned())
            .collect();
        for expected in [
            "manifest.json",
            "history.csv",
            "history.json",
            "databases/fork.db",
            "settings/settings_store.json",
            "settings/fork-settings.json",
            "recordings/nested/take.wav",
        ] {
            assert!(names.contains(&expected.to_owned()), "{names:?}");
        }
        assert!(!names
            .iter()
            .any(|n| n.contains("model") || n.contains("logs")));

        let mut manifest = String::new();
        archive
            .by_name("manifest.json")
            .unwrap()
            .read_to_string(&mut manifest)
            .unwrap();
        let manifest: serde_json::Value = serde_json::from_str(&manifest).unwrap();
        assert_eq!(manifest["schema_version"], EXPORT_SCHEMA_VERSION);
        assert_eq!(manifest["product"], "Wispr Bro");
        let mut csv = String::new();
        archive
            .by_name("history.csv")
            .unwrap()
            .read_to_string(&mut csv)
            .unwrap();
        let mut reader = csv::Reader::from_reader(csv.as_bytes());
        let row = reader.records().next().unwrap().unwrap();
        assert_eq!(&row[4], "private, \"quoted\" words");
        assert!(csv.contains("clean words"), "{csv}");
        let mut audio = Vec::new();
        archive
            .by_name("recordings/nested/take.wav")
            .unwrap()
            .read_to_end(&mut audio)
            .unwrap();
        assert_eq!(audio, b"RIFF-audio");

        let mut json = String::new();
        archive
            .by_name("history.json")
            .unwrap()
            .read_to_string(&mut json)
            .unwrap();
        let json: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(json[0]["id"], 7);

        let snapshot = temp.path().join("snapshot.db");
        std::fs::write(&snapshot, {
            let mut bytes = Vec::new();
            archive
                .by_name("databases/fork.db")
                .unwrap()
                .read_to_end(&mut bytes)
                .unwrap();
            bytes
        })
        .unwrap();
        let value: String = rusqlite::Connection::open(snapshot)
            .unwrap()
            .query_row("SELECT value FROM marker", [], |r| r.get(0))
            .unwrap();
        assert_eq!(value, "fork");
    }

    #[test]
    fn export_requires_a_zip_in_an_existing_folder() {
        let temp = tempfile::tempdir().unwrap();
        assert!(export_data(temp.path(), &[], &temp.path().join("data.txt")).is_err());
        assert!(export_data(temp.path(), &[], &temp.path().join("missing/data.zip")).is_err());
        assert!(export_data(temp.path(), &[], Path::new("relative.zip")).is_err());
    }
}
