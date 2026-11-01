// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for the History list (S-15b): paging, search, variants, audio
//! states, flags and retries.

use std::time::Instant;

use super::*;
use crate::fork::commands::history::{copy_atomically, save_flag, FlagKind};
use crate::fork::db::entry_meta::{self, EntryWrite};
use crate::fork::db::handy::fake::{entry, FakeHistory};
use crate::fork::dictionary::store::SaveOutcome;
use crate::fork::dictionary::EntryKind;

fn setup() -> (tempfile::TempDir, ForkDb, FakeHistory) {
    let dir = tempfile::tempdir().unwrap();
    let db = ForkDb::in_dir(dir.path());
    let rec = dir.path().join("recordings");
    std::fs::create_dir_all(&rec).unwrap();
    let handy = FakeHistory::new(&rec);
    (dir, db, handy)
}

/// A Handy row and its `entry_meta`, as the save hook writes them.
fn add(db: &ForkDb, handy: &FakeHistory, e: HistoryEntry, wav: bool) {
    let w = EntryWrite::for_saved(&e, None, None, |_| "2026-10-30".into());
    db.with_conn(|c| entry_meta::write(c, &w, false)).unwrap();
    handy.add(e, wav.then_some(50));
}

fn dictation(id: i64, raw: &str, cleaned: Option<&str>) -> HistoryEntry {
    let mut e = entry(id, 1_793_400_000 + id, raw);
    e.post_processed_text = cleaned.map(str::to_owned);
    e
}

#[test]
fn pages_follow_handys_cursor_and_skip_soft_deleted_rows() {
    let (_d, db, handy) = setup();
    for id in 1..=250 {
        add(
            &db,
            &handy,
            dictation(id, &format!("take {id}"), None),
            false,
        );
    }
    // 30 soft-deleted rows inside the first page must not shorten it.
    let hidden: Vec<i64> = (221..=250).collect();
    db.with_conn(|c| entry_meta::soft_delete(c, &hidden, 1))
        .unwrap();

    let first = page(&db, &handy, None, 100).unwrap();
    assert_eq!(first.rows.len(), 100);
    assert_eq!(first.rows[0].id, 220);
    assert_eq!(first.rows[99].id, 121);
    assert_eq!(first.next, Some(121));
    let second = page(&db, &handy, first.next, 100).unwrap();
    assert_eq!(second.rows[0].id, 120);
    let third = page(&db, &handy, second.next, 100).unwrap();
    assert_eq!(third.rows.len(), 20);
    assert_eq!(third.next, None);
    // A limit over Handy's page size is capped, never an error.
    assert_eq!(page(&db, &handy, None, 10_000).unwrap().rows.len(), 100);
}

#[test]
fn a_row_shows_the_final_text_and_copies_what_was_inserted() {
    let (_d, db, handy) = setup();
    add(
        &db,
        &handy,
        dictation(1, "um send my address", Some("Send my address.")),
        true,
    );
    db.with_conn(|c| {
        c.execute(
            "UPDATE entry_meta SET inserted_text = 'Send 1 Main St.' WHERE entry_id = 1",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let r = &rows(&db, &handy, &[1]).unwrap()[0];
    assert_eq!(r.text, "Send my address.");
    assert_eq!(r.copy_text, "Send 1 Main St.");
    assert_eq!(r.variant, ShownVariant::Final);
    assert!(r.ai_edited);
    assert_eq!(r.audio, AudioState::Available);
    assert_eq!(r.source, "dictation");
}

#[test]
fn undo_ai_edit_shows_and_copies_the_text_before_cleanup_and_keeps_the_stats() {
    let (_d, db, handy) = setup();
    add(
        &db,
        &handy,
        dictation(1, "so um coffee at 2 actually 3", Some("Coffee at 3.")),
        true,
    );
    let stats = |db: &ForkDb| {
        db.with_conn(|c| {
            Ok(c.query_row("SELECT SUM(words) FROM daily_stats", [], |r| {
                r.get::<_, i64>(0)
            })?)
        })
        .unwrap()
    };
    let before = stats(&db);
    assert!(db
        .with_conn(|c| set_variant(c, 1, ShownVariant::Raw))
        .unwrap());
    let r = &rows(&db, &handy, &[1]).unwrap()[0];
    assert_eq!(r.variant, ShownVariant::Raw);
    assert_eq!(r.text, "so um coffee at 2 actually 3");
    assert_eq!(r.copy_text, "so um coffee at 2 actually 3");
    assert_eq!(stats(&db), before);
    db.with_conn(|c| set_variant(c, 1, ShownVariant::Final))
        .unwrap();
    assert_eq!(rows(&db, &handy, &[1]).unwrap()[0].text, "Coffee at 3.");
    // Unknown rows change nothing.
    assert!(!db
        .with_conn(|c| set_variant(c, 99, ShownVariant::Raw))
        .unwrap());
}

#[test]
fn a_row_without_an_ai_edit_never_shows_raw() {
    let (_d, db, handy) = setup();
    add(&db, &handy, dictation(1, "Hello there.", None), false);
    add(
        &db,
        &handy,
        dictation(2, "Same text.", Some("Same text.")),
        false,
    );
    db.with_conn(|c| {
        set_variant(c, 1, ShownVariant::Raw)?;
        set_variant(c, 2, ShownVariant::Raw)
    })
    .unwrap();
    for r in rows(&db, &handy, &[1, 2]).unwrap() {
        assert!(!r.ai_edited);
        assert_eq!(r.variant, ShownVariant::Final);
    }
}

#[test]
fn a_retry_shows_the_new_text_as_made() {
    let (_d, db, handy) = setup();
    add(
        &db,
        &handy,
        dictation(1, "old raw", Some("Old clean.")),
        true,
    );
    db.with_conn(|c| {
        set_variant(c, 1, ShownVariant::Raw)?;
        c.execute(
            "UPDATE entry_meta SET inserted_text = 'expanded' WHERE entry_id = 1",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let retried = dictation(1, "new raw", Some("New clean."));
    let w = EntryWrite::for_saved(&retried, None, None, |_| "2026-10-30".into());
    db.with_conn(|c| entry_meta::write(c, &w, true)).unwrap();
    handy.add(retried, None);
    let r = &rows(&db, &handy, &[1]).unwrap()[0];
    assert_eq!(r.variant, ShownVariant::Final);
    assert_eq!(r.text, "New clean.");
    assert_eq!(r.copy_text, "New clean.");
    let previous: Option<String> = db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT previous_text FROM entry_meta WHERE entry_id = 1",
                [],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(previous.as_deref(), Some("Old clean."));
}

#[test]
fn audio_is_available_expired_or_missing() {
    let (_d, db, handy) = setup();
    add(&db, &handy, dictation(1, "with audio", None), true);
    add(&db, &handy, dictation(2, "expired", None), false);
    add(&db, &handy, dictation(3, "disk full", None), false);
    add(&db, &handy, dictation(4, "file gone", None), false);
    db.with_conn(|c| {
        // Retention runs later than the take; a full disk marks the start.
        c.execute(
            "UPDATE entry_meta SET audio_deleted_at = created_at + 1209600000 WHERE entry_id = 2",
            [],
        )?;
        c.execute(
            "UPDATE entry_meta SET audio_deleted_at = created_at WHERE entry_id = 3",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let states: Vec<AudioState> = rows(&db, &handy, &[1, 2, 3, 4])
        .unwrap()
        .into_iter()
        .map(|r| r.audio)
        .collect();
    assert_eq!(
        states,
        vec![
            AudioState::Available,
            AudioState::Expired,
            AudioState::Missing,
            AudioState::Missing
        ]
    );
}

#[test]
fn file_transcriptions_are_tagged_and_rows_without_meta_still_show() {
    let (_d, db, handy) = setup();
    let mut f = dictation(1, "meeting notes", None);
    f.source_name = Some("call.m4a".into());
    add(&db, &handy, f, true);
    // A row the reconcile has not seen yet.
    handy.add(dictation(2, "not reconciled", None), None);
    let r = rows(&db, &handy, &[1, 2]).unwrap();
    assert_eq!(r[0].source, "file");
    assert_eq!(r[0].source_name.as_deref(), Some("call.m4a"));
    assert_eq!(r[1].source, "dictation");
    assert_eq!(r[1].text, "not reconciled");
    assert_eq!(r[1].created_at, (1_793_400_000 + 2) * 1000);
}

#[test]
fn queued_takes_expose_waiting_and_failed_states_in_history() {
    let _batch = crate::fork::models::queue::TEST_BATCH_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let (_d, db, handy) = setup();
    add(&db, &handy, dictation(1, "", None), true);
    db.with_conn(|c| {
        crate::fork::models::queue::enqueue(c, "wispr-bro-1.wav", 1)?;
        crate::fork::models::queue::bind_entry(c, "wispr-bro-1.wav", 1)
    })
    .unwrap();
    assert_eq!(
        rows(&db, &handy, &[1]).unwrap()[0].queue_status,
        Some(crate::fork::models::queue::QueueStatus::Waiting)
    );
    db.with_conn(|c| {
        let started = crate::fork::models::queue::start_ready(c)?;
        crate::fork::models::queue::begin_batch(&started);
        crate::fork::models::queue::finish(c, 1, false)
    })
    .unwrap();
    assert_eq!(
        rows(&db, &handy, &[1]).unwrap()[0].queue_status,
        Some(crate::fork::models::queue::QueueStatus::Failed)
    );
}

#[test]
fn search_pages_by_offset_newest_first() {
    let (_d, db, handy) = setup();
    for id in 1..=130 {
        let text = if id % 2 == 0 { "coffee order" } else { "tea" };
        add(&db, &handy, dictation(id, text, None), false);
    }
    let first = find(&db, &handy, "coffee", 0, 50).unwrap();
    assert_eq!(first.rows.len(), 50);
    assert_eq!(first.rows[0].id, 130);
    assert_eq!(first.next, Some(50));
    let second = find(&db, &handy, "coffee", 50, 50).unwrap();
    assert_eq!(second.rows.len(), 15);
    assert_eq!(second.next, None);
    assert!(find(&db, &handy, "zebra", 0, 50).unwrap().rows.is_empty());
    // Under three characters: the LIKE fallback.
    assert_eq!(find(&db, &handy, "te", 0, 100).unwrap().rows.len(), 65);
}

#[test]
fn flag_adds_a_replacement_or_a_word_and_records_the_correction() {
    let (_d, db, handy) = setup();
    add(
        &db,
        &handy,
        dictation(1, "call Sioban tomorrow", None),
        true,
    );
    let outcome = db
        .with_conn(|c| save_flag(c, 1, "Sioban", "Siobhán", FlagKind::Rule, None, 5))
        .unwrap();
    let SaveOutcome::Saved { entry } = outcome else {
        panic!("not saved: {outcome:?}");
    };
    assert_eq!(entry.kind, EntryKind::Rule);
    assert_eq!(entry.phrase, "Sioban");
    assert_eq!(entry.replacement.as_deref(), Some("Siobhán"));
    assert_eq!(entry.source, crate::fork::dictionary::EntrySource::Flag);
    assert!(rows(&db, &handy, &[1]).unwrap()[0].flagged);
    let flags: i64 = db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM corrections WHERE source = 'flag' AND entry_id = 1
                 AND from_text = 'Sioban' AND to_text = 'Siobhán'",
                [],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(flags, 1);

    // The same flag again: already there, the row stays flagged.
    let again = db
        .with_conn(|c| save_flag(c, 1, "Sioban", "Siobhán", FlagKind::Rule, None, 6))
        .unwrap();
    assert!(matches!(again, SaveOutcome::Duplicate { .. }));
    // Another replacement for the same word: a conflict, nothing recorded.
    let conflict = db
        .with_conn(|c| save_flag(c, 1, "Sioban", "Siobhan", FlagKind::Rule, None, 7))
        .unwrap();
    let SaveOutcome::Conflict { existing } = conflict else {
        panic!("no conflict: {conflict:?}");
    };
    // "Replace it" edits the existing entry.
    let replaced = db
        .with_conn(|c| {
            save_flag(
                c,
                1,
                "Sioban",
                "Siobhan",
                FlagKind::Rule,
                Some(existing.id),
                8,
            )
        })
        .unwrap();
    assert!(matches!(replaced, SaveOutcome::Saved { .. }));

    // A word: the corrected spelling is taught.
    let word = db
        .with_conn(|c| save_flag(c, 1, "tomorow", "Kubernetes", FlagKind::Word, None, 9))
        .unwrap();
    let SaveOutcome::Saved { entry } = word else {
        panic!("word not saved");
    };
    assert_eq!(entry.kind, EntryKind::Word);
    assert_eq!(entry.phrase, "Kubernetes");
    // An empty correction is invalid and records nothing.
    let invalid = db
        .with_conn(|c| save_flag(c, 1, "call", "  ", FlagKind::Rule, None, 10))
        .unwrap();
    assert!(matches!(invalid, SaveOutcome::Invalid { .. }));
    let total: i64 = db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM corrections WHERE source = 'flag'",
                [],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(total, 4);
}

#[test]
fn save_audio_copies_whole_files_only() {
    let dir = tempfile::tempdir().unwrap();
    let from = dir.path().join("take.wav");
    std::fs::write(&from, b"RIFF....WAVE").unwrap();
    let to = dir.path().join("out").join("saved.wav");
    // Missing folder: an error, no partial file left.
    assert!(copy_atomically(&from, &to).is_err());
    std::fs::create_dir_all(to.parent().unwrap()).unwrap();
    copy_atomically(&from, &to).unwrap();
    assert_eq!(std::fs::read(&to).unwrap(), b"RIFF....WAVE");
    assert!(!dir.path().join("out").join("saved.wav.partial").exists());
}

#[test]
fn retry_levels_apply_to_one_retry_of_one_entry() {
    use crate::fork::llm::post_process::{set_retry_level, take_retry_level};
    use crate::fork::settings::CleanupLevel;
    set_retry_level(9_001, Some(CleanupLevel::Medium));
    set_retry_level(9_002, Some(CleanupLevel::None));
    set_retry_level(9_001, Some(CleanupLevel::None));
    assert_eq!(take_retry_level(9_001), Some(CleanupLevel::None));
    assert_eq!(take_retry_level(9_001), None);
    set_retry_level(9_002, None);
    assert_eq!(take_retry_level(9_002), None);
}

#[test]
fn ten_thousand_rows_page_quickly() {
    let (_d, db, handy) = setup();
    db.with_conn(|c| {
        let tx = c.transaction()?;
        for id in 1..=10_000_i64 {
            crate::fork::db::upsert_entry_meta(
                &tx,
                id,
                &[
                    ("created_at", &(id * 1000)),
                    ("final_text", &format!("take number {id}")),
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    })
    .unwrap();
    for id in 1..=10_000 {
        handy.add(dictation(id, &format!("take number {id}"), None), None);
    }
    let start = Instant::now();
    let mut cursor = None;
    let mut seen = 0;
    loop {
        let p = page(&db, &handy, cursor, 100).unwrap();
        seen += p.rows.len();
        match p.next {
            Some(n) => cursor = Some(n),
            None => break,
        }
    }
    assert_eq!(seen, 10_000);
    // 100 pages; a page is one screenful and more, far under a frame each
    // in release (a debug build on CI is the bound here).
    assert!(start.elapsed().as_secs() < 20, "{:?}", start.elapsed());
}
