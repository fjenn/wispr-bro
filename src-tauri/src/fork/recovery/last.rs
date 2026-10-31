// SPDX-License-Identifier: GPL-3.0-or-later
//! The last transcript (R5 S-06), for paste-last and copy-last: the newest
//! dictation in History with text, as History shows it.
//!
//! - Only dictations: practice takes (onboarding), transcribed files,
//!   commands and transforms are skipped (`entry_meta.source`).
//! - Failed and empty takes are skipped (no text; a take still waiting for
//!   its speech model or for Undo's transcription has none yet either).
//! - Deleted entries are skipped (`deleted_at`, S-15b's undoable delete).
//! - The shown variant: `raw` shows the verbatim speech-model text
//!   (`asr_text`); `final` shows what was inserted (`inserted_text`, the
//!   text after a snippet expanded, S-17) or else the final text.
//!
//! Newest = latest record start (`created_at`), then the higher History id
//! (two takes started in the same millisecond).

use rusqlite::{Connection, OptionalExtension};
use tauri::{AppHandle, Manager, Runtime};

use crate::fork::db::{DbError, ForkDb};

/// The text to paste or copy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LastTranscript {
    pub entry_id: i64,
    pub text: String,
    /// The take was refused for a password field (S-05): a copy is
    /// concealed from clipboard managers, as its card's Copy was.
    pub concealed: bool,
}

const QUERY: &str = "
    SELECT entry_id, text, paste_outcome FROM (
        SELECT entry_id, created_at, paste_outcome,
               CASE WHEN shown_variant = 'raw' AND trim(coalesce(asr_text, '')) != ''
                    THEN asr_text
                    ELSE coalesce(nullif(inserted_text, ''), final_text)
               END AS text
          FROM entry_meta
         WHERE deleted_at IS NULL AND source = 'dictation'
    )
     WHERE trim(coalesce(text, '')) != ''
     ORDER BY created_at DESC, entry_id DESC
     LIMIT 1";

/// The last transcript, if History has one.
pub fn select(conn: &Connection) -> Result<Option<LastTranscript>, DbError> {
    let row = conn
        .query_row(QUERY, [], |r| {
            Ok(LastTranscript {
                entry_id: r.get(0)?,
                text: r.get(1)?,
                concealed: r.get::<_, Option<String>>(2)?.as_deref() == Some("refused"),
            })
        })
        .optional()?;
    Ok(row)
}

/// [`select`] on the app's `fork.db`; `None` when there is none or the
/// database cannot be read (logged).
pub fn find<R: Runtime>(app: &AppHandle<R>) -> Option<LastTranscript> {
    let db = app.try_state::<ForkDb>()?;
    match db.with_conn(|c| select(c)) {
        Ok(found) => found,
        Err(e) => {
            log::warn!("recovery: cannot read the last transcript: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::types::ToSql;

    fn db() -> (tempfile::TempDir, ForkDb) {
        let dir = tempfile::tempdir().unwrap();
        let db = ForkDb::in_dir(dir.path());
        (dir, db)
    }

    #[derive(Default)]
    struct Row<'a> {
        id: i64,
        at: i64,
        asr: Option<&'a str>,
        final_text: Option<&'a str>,
        inserted: Option<&'a str>,
        shown: &'a str,
        source: &'a str,
        deleted: bool,
        outcome: Option<&'a str>,
    }

    fn put(db: &ForkDb, r: Row) {
        let deleted = r.deleted.then_some(r.at + 1);
        let shown = if r.shown.is_empty() { "final" } else { r.shown };
        let source = if r.source.is_empty() {
            "dictation"
        } else {
            r.source
        };
        db.with_conn(|c| {
            let fields: [(&str, &dyn ToSql); 8] = [
                ("created_at", &r.at),
                ("asr_text", &r.asr),
                ("final_text", &r.final_text),
                ("inserted_text", &r.inserted),
                ("shown_variant", &shown),
                ("source", &source),
                ("deleted_at", &deleted),
                ("paste_outcome", &r.outcome),
            ];
            crate::fork::db::upsert_entry_meta(c, r.id, &fields)
        })
        .unwrap();
    }

    fn last(db: &ForkDb) -> Option<LastTranscript> {
        db.with_conn(|c| select(c)).unwrap()
    }

    #[test]
    fn none_when_history_is_empty() {
        let (_d, db) = db();
        assert_eq!(last(&db), None);
    }

    #[test]
    fn the_newest_dictation_with_text_wins() {
        let (_d, db) = db();
        put(
            &db,
            Row {
                id: 1,
                at: 100,
                final_text: Some("first"),
                ..Row::default()
            },
        );
        put(
            &db,
            Row {
                id: 2,
                at: 200,
                final_text: Some("second"),
                ..Row::default()
            },
        );
        let got = last(&db).unwrap();
        assert_eq!((got.entry_id, got.text.as_str()), (2, "second"));
        assert!(!got.concealed);
    }

    #[test]
    fn practice_files_failed_empty_and_deleted_takes_are_skipped() {
        let (_d, db) = db();
        put(
            &db,
            Row {
                id: 1,
                at: 100,
                final_text: Some("the real one"),
                ..Row::default()
            },
        );
        for (id, row) in [
            (
                2,
                Row {
                    final_text: Some("practice"),
                    source: "practice",
                    ..Row::default()
                },
            ),
            (
                3,
                Row {
                    final_text: Some("a file"),
                    source: "file",
                    ..Row::default()
                },
            ),
            // A failed take: Handy saves an empty text.
            (
                4,
                Row {
                    final_text: Some(""),
                    ..Row::default()
                },
            ),
            (
                5,
                Row {
                    final_text: Some("   "),
                    ..Row::default()
                },
            ),
            (6, Row::default()),
            (
                7,
                Row {
                    final_text: Some("deleted"),
                    deleted: true,
                    ..Row::default()
                },
            ),
        ] {
            put(
                &db,
                Row {
                    id,
                    at: 100 + id,
                    ..row
                },
            );
        }
        assert_eq!(last(&db).unwrap().text, "the real one");
    }

    #[test]
    fn the_shown_variant_and_the_inserted_text() {
        let (_d, db) = db();
        // A snippet expanded: the inserted text is what goes in again.
        put(
            &db,
            Row {
                id: 1,
                at: 100,
                asr: Some("my address"),
                final_text: Some("My address."),
                inserted: Some("1 Main St\nSpringfield"),
                ..Row::default()
            },
        );
        assert_eq!(last(&db).unwrap().text, "1 Main St\nSpringfield");
        // History shows the raw variant: the verbatim speech-model text.
        put(
            &db,
            Row {
                id: 2,
                at: 200,
                asr: Some("um hello there"),
                final_text: Some("Hello there."),
                shown: "raw",
                ..Row::default()
            },
        );
        assert_eq!(last(&db).unwrap().text, "um hello there");
        // Raw with no verbatim text kept: the final text.
        put(
            &db,
            Row {
                id: 3,
                at: 300,
                final_text: Some("Fallback."),
                shown: "raw",
                ..Row::default()
            },
        );
        assert_eq!(last(&db).unwrap().text, "Fallback.");
    }

    #[test]
    fn same_start_time_takes_the_higher_id_and_refused_is_concealed() {
        let (_d, db) = db();
        for id in [4, 9] {
            put(
                &db,
                Row {
                    id,
                    at: 500,
                    final_text: Some(if id == 9 { "secret" } else { "older" }),
                    outcome: (id == 9).then_some("refused"),
                    ..Row::default()
                },
            );
        }
        let got = last(&db).unwrap();
        assert_eq!((got.entry_id, got.concealed), (9, true));
    }
}
