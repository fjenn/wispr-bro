// SPDX-License-Identifier: GPL-3.0-or-later
//! CSV export (S-17, C-71), the format the import reads back
//! (`docs/fork/user/import-format.md`, "Snippets"):
//!
//! ```text
//! trigger,text
//! my sign-off,"Best wishes,
//! Finn"
//! ```
//!
//! The dictionary export's conventions (S-16): UTF-8 with a BOM, CRLF
//! between records, oldest first, the formula guard on every cell, an
//! atomic write. Line breaks inside a text stay `\n`, inside quotes.

use std::io::Write;
use std::path::Path;

use super::SnippetEntry;
use crate::fork::dictionary::export::{guard, write_atomic};

/// Header row; also what the import's column guess looks for.
pub const HEADER: [&str; 2] = ["trigger", "text"];

/// Writes the snippets as CSV. Returns how many rows were written.
pub fn write_csv<W: Write>(out: W, snippets: &[SnippetEntry]) -> Result<u32, csv::Error> {
    let mut out = out;
    out.write_all(&[0xEF, 0xBB, 0xBF])?;
    let mut w = csv::WriterBuilder::new()
        .terminator(csv::Terminator::CRLF)
        .from_writer(out);
    w.write_record(HEADER)?;
    let mut sorted: Vec<&SnippetEntry> = snippets.iter().collect();
    sorted.sort_by_key(|s| (s.created_at, s.id));
    for s in &sorted {
        w.write_record([guard(&s.trigger).as_ref(), guard(&s.body).as_ref()])?;
    }
    w.flush()?;
    Ok(u32::try_from(sorted.len()).unwrap_or(u32::MAX))
}

/// Writes the export file atomically.
pub fn write_file(path: &Path, snippets: &[SnippetEntry]) -> std::io::Result<u32> {
    write_atomic(path, |file| write_csv(file, snippets))
}
