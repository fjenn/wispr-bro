// SPDX-License-Identifier: GPL-3.0-or-later
//! CSV export (S-16, C-65), written with the `csv` crate (R4): the format
//! the import reads back (`docs/fork/user/import-format.md`):
//!
//! ```text
//! phrase,replacement
//! Siobhán,
//! gonna,going to
//! ```
//!
//! UTF-8 with a BOM (spreadsheet apps then read accents right), one row per
//! active entry, oldest first; a word has an empty replacement. Cells that
//! a spreadsheet would run as a formula (`= + - @`, or a tab / carriage
//! return first) get a leading `'` (formula-injection guard, R4); the
//! import removes it again.

use std::io::Write;
use std::path::{Path, PathBuf};

use super::{DictionaryEntry, EntryStatus};

/// Header row; also what the import's column guess looks for.
pub const HEADER: [&str; 2] = ["phrase", "replacement"];

/// The cell as written: a leading `'` when a spreadsheet would treat it as
/// a formula.
pub fn guard(cell: &str) -> std::borrow::Cow<'_, str> {
    if cell.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{cell}").into()
    } else {
        cell.into()
    }
}

/// Writes the active entries as CSV. Returns how many rows were written.
pub fn write_csv<W: Write>(out: W, entries: &[DictionaryEntry]) -> Result<u32, csv::Error> {
    let mut out = out;
    out.write_all(&[0xEF, 0xBB, 0xBF])?;
    let mut w = csv::WriterBuilder::new()
        .terminator(csv::Terminator::CRLF)
        .from_writer(out);
    w.write_record(HEADER)?;
    let mut active: Vec<&DictionaryEntry> = entries
        .iter()
        .filter(|e| e.status == EntryStatus::Active)
        .collect();
    active.sort_by_key(|e| (e.created_at, e.id));
    for e in &active {
        let replacement = e.replacement.as_deref().unwrap_or("");
        w.write_record([guard(&e.phrase).as_ref(), guard(replacement).as_ref()])?;
    }
    w.flush()?;
    Ok(u32::try_from(active.len()).unwrap_or(u32::MAX))
}

/// `path` with a `.csv` extension (the save panel may drop it).
pub fn with_csv_extension(path: &Path) -> PathBuf {
    let is_csv = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("csv"));
    if is_csv {
        path.to_path_buf()
    } else {
        let mut name = path.as_os_str().to_owned();
        name.push(".csv");
        PathBuf::from(name)
    }
}

/// Writes the export file atomically (a hidden temp file next to it,
/// renamed over it).
pub fn write_file(path: &Path, entries: &[DictionaryEntry]) -> std::io::Result<u32> {
    let path = with_csv_extension(path);
    let name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("no file name"))?;
    let mut tmp_name = std::ffi::OsString::from(".");
    tmp_name.push(name);
    tmp_name.push(format!(".{}.tmp", std::process::id()));
    let tmp = path.with_file_name(tmp_name);
    let result = (|| {
        let mut file = std::fs::File::create(&tmp)?;
        let n = write_csv(&mut file, entries).map_err(std::io::Error::other)?;
        file.sync_all()?;
        std::fs::rename(&tmp, &path)?;
        Ok(n)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}
