// SPDX-License-Identifier: GPL-3.0-or-later
//! A Dictionary or Snippets CSV as the Mac's pages export them
//! (`docs/fork/user/import-format.md`), read as a sync source. The header
//! row says which it is: `phrase,replacement` or `trigger,text`. Rows have
//! no time of their own; they count as changed now, so a row replaces a
//! different entry with the same word or trigger.

use std::path::Path;

use super::{DictItem, Incoming, Manifest, Payload, ReadProblem, SnippetItem, SourceKind, FORMAT};
use crate::fork::dictionary::import::{read_file, unguard, ImportFile};
use crate::fork::{dictionary, snippets};

/// Rows read at most (the pages' own limits are lower).
const MAX_ROWS: usize = 10_000;

fn header_is(record: &::csv::StringRecord, header: [&str; 2]) -> bool {
    record.len() >= 2 && (0..2).all(|i| record[i].trim().eq_ignore_ascii_case(header[i]))
}

pub fn read(path: &Path, now_ms: i64) -> Result<Incoming, ReadProblem> {
    let (name, text) = match read_file(path) {
        ImportFile::Read { name, text } => (name, text),
        ImportFile::NotCsv => return Err(ReadProblem::NotWisprBro),
        ImportFile::TooLarge { .. } | ImportFile::Unreadable => {
            return Err(ReadProblem::Unreadable)
        }
    };
    let mut reader = ::csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut records = reader.records().filter_map(Result::ok);
    let header = records.next().ok_or(ReadProblem::NotWisprBro)?;
    let cell = |record: &::csv::StringRecord, i: usize| {
        unguard(record.get(i).unwrap_or_default()).to_owned()
    };
    let mut payload = Payload::default();
    let kind = if header_is(&header, dictionary::export::HEADER) {
        payload.dictionary = records
            .take(MAX_ROWS)
            .map(|r| DictItem {
                phrase: cell(&r, 0),
                replacement: Some(cell(&r, 1)).filter(|c| !c.trim().is_empty()),
                source: "import".into(),
                status: "active".into(),
                created_at: now_ms,
                updated_at: now_ms,
            })
            .collect();
        SourceKind::DictionaryCsv
    } else if header_is(&header, snippets::export::HEADER) {
        payload.snippets = records
            .take(MAX_ROWS)
            .map(|r| SnippetItem {
                trigger: cell(&r, 0),
                body: cell(&r, 1),
                source: "import".into(),
                created_at: now_ms,
                updated_at: now_ms,
            })
            .collect();
        SourceKind::SnippetsCsv
    } else {
        return Err(ReadProblem::NotWisprBro);
    };
    Ok(Incoming {
        kind,
        manifest: Manifest {
            format: FORMAT.to_owned(),
            version: 0,
            device_id: String::new(),
            device_name: name,
            platform: String::new(),
            app_version: String::new(),
            created_at: now_ms,
        },
        payload,
    })
}
