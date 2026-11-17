// SPDX-License-Identifier: GPL-3.0-or-later
//! The `.wisprbro` file: a zip of `manifest.json` ([`Manifest`]) and
//! `sync.json` ([`Payload`]). No history, no audio.

use std::io::{Read, Seek, Write};
use std::path::Path;

use zip::write::SimpleFileOptions;

use super::{Incoming, Manifest, Payload, ReadProblem, SourceKind, FORMAT, FORMAT_VERSION};

pub const MANIFEST_NAME: &str = "manifest.json";
pub const PAYLOAD_NAME: &str = "sync.json";

/// Largest entry read from any file (a dictionary and snippets at their
/// limits are a few MB).
pub const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;

/// One zip entry, whole, refused above `limit` bytes.
pub fn entry_bytes<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
    limit: u64,
) -> Option<Vec<u8>> {
    let entry = archive.by_name(name).ok()?;
    if entry.size() > limit {
        return None;
    }
    let mut bytes = Vec::new();
    entry.take(limit + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= limit).then_some(bytes)
}

/// Writes the file at `path` (replacing one that is there), through a
/// sibling temporary file.
pub fn write(path: &Path, manifest: &Manifest, payload: &Payload) -> Result<(), String> {
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    let result = (|| {
        let file = std::fs::File::create(&temporary).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, bytes) in [
            (
                MANIFEST_NAME,
                serde_json::to_vec_pretty(manifest).map_err(|e| e.to_string())?,
            ),
            (
                PAYLOAD_NAME,
                serde_json::to_vec(payload).map_err(|e| e.to_string())?,
            ),
        ] {
            zip.start_file(name, options).map_err(|e| e.to_string())?;
            zip.write_all(&bytes).map_err(|e| e.to_string())?;
        }
        zip.finish().map_err(|e| e.to_string())?;
        std::fs::rename(&temporary, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result.map_err(|e| format!("could not write the sync file: {e}"))
}

/// Reads a `.wisprbro` archive.
pub fn read<R: Read + Seek>(archive: &mut zip::ZipArchive<R>) -> Result<Incoming, ReadProblem> {
    let manifest = entry_bytes(archive, MANIFEST_NAME, MAX_ENTRY_BYTES)
        .and_then(|b| serde_json::from_slice::<Manifest>(&b).ok())
        .filter(|m| m.format == FORMAT && !m.device_id.trim().is_empty())
        .ok_or(ReadProblem::NotWisprBro)?;
    if manifest.version > FORMAT_VERSION {
        return Err(ReadProblem::TooNew);
    }
    let payload = entry_bytes(archive, PAYLOAD_NAME, MAX_ENTRY_BYTES)
        .and_then(|b| serde_json::from_slice::<Payload>(&b).ok())
        .ok_or(ReadProblem::NotWisprBro)?;
    Ok(Incoming {
        kind: SourceKind::Bundle,
        manifest,
        payload,
    })
}
