// SPDX-License-Identifier: GPL-3.0-or-later
//! Scratchpad notes in a sync ([`super::notes`]): create, edit, pin and
//! delete both ways, concurrent edits, idempotent re-syncs, the file route
//! and an app from before notes synced. All data is synthetic.

use rusqlite::Connection;

use super::merge::{merge, MergeReport, SyncChoices, SyncCounts};
use super::{bundle, read, store, Incoming, Payload, SourceKind};
use crate::fork::context::now_ms;
use crate::fork::db::ForkDb;
use crate::fork::scratchpad::store as pad;

struct Device {
    dir: tempfile::TempDir,
    db: ForkDb,
    name: &'static str,
}

fn device(name: &'static str) -> Device {
    let dir = tempfile::tempdir().unwrap();
    let db = ForkDb::in_dir(dir.path());
    db.open_now().unwrap();
    Device { dir, db, name }
}

/// Times well before the wall clock (a pin is stamped with it).
fn at(n: i64) -> i64 {
    static BASE: std::sync::OnceLock<i64> = std::sync::OnceLock::new();
    *BASE.get_or_init(now_ms) - 10_000_000 + n
}

/// A note as both devices must end up with it.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
struct Seen {
    id: String,
    title: String,
    body: String,
    pinned: bool,
}

impl Device {
    fn with<T>(&self, f: impl FnOnce(&mut Connection) -> T) -> T {
        self.db.with_conn(|c| Ok(f(c))).unwrap()
    }

    /// This device's state as a session sends it.
    fn state(&self) -> Incoming {
        let (manifest, payload) =
            self.with(|c| store::collect(c, self.name, "1.0.0-test", now_ms()).unwrap());
        Incoming {
            kind: SourceKind::Bundle,
            manifest,
            payload,
        }
    }

    /// This device's `.wisprbro` file, written and read back.
    fn file(&self) -> Incoming {
        let path = self.dir.path().join(format!("{}.wisprbro", self.name));
        let state = self.state();
        bundle::write(&path, &state.manifest, &state.payload).unwrap();
        read(&path, now_ms()).unwrap()
    }

    fn merge(&self, incoming: &Incoming) -> SyncCounts {
        self.merge_report(incoming).notes
    }

    fn merge_report(&self, incoming: &Incoming) -> MergeReport {
        self.with(|c| {
            let tx = c.transaction().unwrap();
            let outcome = merge(&tx, incoming, SyncChoices::ALL, now_ms()).unwrap();
            tx.commit().unwrap();
            outcome.report
        })
    }

    fn write(&self, body: &str, n: i64) -> i64 {
        self.with(|c| {
            let note = pad::create(c, at(n)).unwrap();
            pad::save(c, note.id, body, at(n)).unwrap();
            note.id
        })
    }

    fn edit(&self, rowid: i64, body: &str, n: i64) {
        self.with(|c| pad::save(c, rowid, body, at(n)).unwrap().unwrap());
    }

    fn pin(&self, rowid: i64, on: bool) {
        self.with(|c| pad::set_pinned(c, rowid, on).unwrap().unwrap());
    }

    fn delete(&self, rowid: i64, n: i64) {
        assert!(self.with(|c| pad::delete(c, rowid, at(n)).unwrap()));
    }

    /// The row id of the live note with `body`.
    fn row(&self, body: &str) -> i64 {
        self.with(|c| {
            c.query_row(
                "SELECT id FROM notes WHERE body = ?1 AND deleted_at IS NULL",
                [body],
                |r| r.get(0),
            )
            .unwrap()
        })
    }

    fn notes(&self) -> Vec<Seen> {
        let mut all: Vec<Seen> = self.with(|c| {
            let mut stmt = c
                .prepare(
                    "SELECT sync_id, title, body, pinned FROM notes
                     WHERE deleted_at IS NULL ORDER BY sync_id",
                )
                .unwrap();
            let rows = stmt
                .query_map([], |r| {
                    Ok(Seen {
                        id: r.get(0)?,
                        title: r.get(1)?,
                        body: r.get(2)?,
                        pinned: r.get(3)?,
                    })
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            rows
        });
        all.sort();
        all
    }

    fn bodies(&self) -> Vec<String> {
        let mut b: Vec<String> = self.notes().into_iter().map(|n| n.body).collect();
        b.sort();
        b
    }

    /// The search index agrees with the notes.
    fn finds(&self, word: &str) -> usize {
        self.with(|c| pad::list(c, Some(word)).unwrap().len())
    }
}

/// Sends states both ways until neither changes (a session does the same).
fn settle(a: &Device, b: &Device) {
    for _ in 0..4 {
        b.merge(&a.state());
        a.merge(&b.state());
    }
    assert_eq!(a.notes(), b.notes(), "the two devices differ");
}

fn with_note(note: &super::NoteItem, f: impl FnOnce(&mut super::NoteItem)) -> super::NoteItem {
    let mut note = note.clone();
    f(&mut note);
    note
}

const NOTHING: SyncCounts = SyncCounts {
    added: 0,
    changed: 0,
    removed: 0,
    skipped: 0,
};

#[test]
fn a_new_note_travels_and_a_second_merge_changes_nothing() {
    let mac = device("Studio");
    let phone = device("iPhone");
    mac.write("Groceries\nmilk and eggs", 1);
    let counts = phone.merge(&mac.state());
    assert_eq!(counts.added, 1);
    assert_eq!(phone.notes(), mac.notes());
    assert_eq!(phone.notes()[0].title, "Groceries");
    assert_eq!(phone.finds("eggs"), 1);
    // Again, and the echo back: nothing.
    assert_eq!(phone.merge(&mac.state()), NOTHING);
    assert_eq!(mac.merge(&phone.state()), NOTHING);
    // A note on each side.
    phone.write("Ideas\na new app", 2);
    assert_eq!(mac.merge(&phone.state()).added, 1);
    settle(&mac, &phone);
    assert_eq!(mac.bodies().len(), 2);
}

#[test]
fn edits_travel_both_ways() {
    let mac = device("Studio");
    let phone = device("iPhone");
    let on_mac = mac.write("draft", 1);
    phone.merge(&mac.state());
    let on_phone = phone.row("draft");
    phone.edit(on_phone, "draft, edited on the phone", 2);
    assert_eq!(mac.merge(&phone.state()).changed, 1);
    assert_eq!(mac.bodies(), ["draft, edited on the phone"]);
    // The Mac's edit is made from the phone's version: no conflict.
    mac.edit(on_mac, "draft, edited on both, one after the other", 3);
    assert_eq!(phone.merge(&mac.state()).changed, 1);
    settle(&mac, &phone);
    assert_eq!(mac.bodies(), ["draft, edited on both, one after the other"]);
    assert_eq!(phone.finds("one after"), 1);
    assert_eq!(phone.finds("phone"), 0);
}

#[test]
fn a_stale_echo_never_undoes_a_newer_edit() {
    let mac = device("Studio");
    let phone = device("iPhone");
    let row = mac.write("v1", 1);
    phone.merge(&mac.state());
    let phone_state_with_v1 = phone.state();
    mac.edit(row, "v2", 2);
    // The phone's older state (holding the Mac's own v1) arrives late.
    assert_eq!(mac.merge(&phone_state_with_v1), NOTHING);
    assert_eq!(mac.bodies(), ["v2"]);
    settle(&mac, &phone);
    assert_eq!(phone.bodies(), ["v2"]);
}

#[test]
fn pins_travel_both_ways() {
    let mac = device("Studio");
    let phone = device("iPhone");
    let row = mac.write("pin me", 1);
    phone.merge(&mac.state());
    phone.pin(phone.row("pin me"), true);
    assert_eq!(mac.merge(&phone.state()).changed, 1);
    assert!(mac.notes()[0].pinned);
    mac.pin(row, false);
    phone.merge(&mac.state());
    assert!(!phone.notes()[0].pinned);
    settle(&mac, &phone);
}

#[test]
fn deletions_travel_both_ways_and_stay_deleted() {
    let mac = device("Studio");
    let phone = device("iPhone");
    let a = mac.write("first", 1);
    mac.write("second", 2);
    phone.merge(&mac.state());
    let old_mac_state = mac.state();
    mac.delete(a, 3);
    assert_eq!(phone.merge(&mac.state()).removed, 1);
    assert_eq!(phone.bodies(), ["second"]);
    assert_eq!(phone.finds("first"), 0);
    // An old state with the note does not bring it back.
    assert_eq!(phone.merge(&old_mac_state), NOTHING);
    assert_eq!(phone.bodies(), ["second"]);
    // And the other way.
    phone.delete(phone.row("second"), 4);
    assert_eq!(mac.merge(&phone.state()).removed, 1);
    settle(&mac, &phone);
    assert!(mac.bodies().is_empty());
}

#[test]
fn concurrent_edits_keep_both_texts() {
    let mac = device("Studio");
    let phone = device("iPhone");
    let on_mac = mac.write("Plan\nshared start", 1);
    phone.merge(&mac.state());
    let on_phone = phone.row("Plan\nshared start");
    // Both edit before either hears of the other's edit; the phone's is later.
    mac.edit(on_mac, "Plan\nthe Mac's words", 2);
    phone.edit(on_phone, "Plan\nthe phone's words", 3);
    let phone_state = phone.state();
    let mac_state = mac.state();
    let on_mac_side = mac.merge(&phone_state);
    let on_phone_side = phone.merge(&mac_state);
    assert_eq!((on_mac_side.added, on_mac_side.changed), (1, 1));
    assert_eq!((on_phone_side.added, on_phone_side.changed), (1, 0));
    // The newer stays as the note (same row: an open note updates in
    // place); the other is a new note naming where it came from.
    assert_eq!(
        mac.with(|c| pad::get(c, on_mac).unwrap().unwrap().body),
        "Plan\nthe phone's words"
    );
    let copy = mac
        .notes()
        .into_iter()
        .find(|n| n.body == "Plan\nthe Mac's words")
        .unwrap();
    assert_eq!(copy.title, "Plan (from Studio)");
    assert_eq!(mac.notes(), phone.notes(), "both made the same copy");
    // Re-syncing changes nothing and makes no second copy.
    assert_eq!(mac.merge(&phone_state), NOTHING);
    assert_eq!(phone.merge(&mac_state), NOTHING);
    settle(&mac, &phone);
    assert_eq!(mac.bodies().len(), 2);
    // Deleting the copy on one side deletes it on the other, for good.
    phone.delete(phone.row("Plan\nthe Mac's words"), 4);
    settle(&mac, &phone);
    assert_eq!(mac.bodies(), ["Plan\nthe phone's words"]);
    assert_eq!(mac.merge(&phone_state), NOTHING);
    assert_eq!(phone.merge(&mac_state), NOTHING);
}

#[test]
fn an_edit_the_deleting_device_never_saw_survives_the_deletion() {
    let mac = device("Studio");
    let phone = device("iPhone");
    let row = mac.write("keep this?", 1);
    phone.merge(&mac.state());
    phone.edit(phone.row("keep this?"), "keep this! new words", 2);
    mac.delete(row, 3);
    // The Mac's deletion reaches the phone: the edit stays.
    assert_eq!(phone.merge(&mac.state()).removed, 0);
    // The edit reaches the Mac: the note is back there.
    assert_eq!(mac.merge(&phone.state()).added, 1);
    settle(&mac, &phone);
    assert_eq!(mac.bodies(), ["keep this! new words"]);
    assert_eq!(mac.finds("new words"), 1);
}

#[test]
fn typing_over_a_note_a_sync_just_changed_keeps_both_texts() {
    let mac = device("Studio");
    let phone = device("iPhone");
    let row = mac.write("note", 1);
    let opened_at = mac.with(|c| pad::get(c, row).unwrap().unwrap().version);
    phone.merge(&mac.state());
    phone.edit(phone.row("note"), "note, from the phone", 2);
    mac.merge(&phone.state());
    // The Mac's editor still shows "note" and the user types on.
    let saved = mac.with(|c| {
        pad::save_from(c, row, "note, typed on the Mac", at(3), Some(opened_at))
            .unwrap()
            .unwrap()
    });
    assert_eq!(saved.body, "note, typed on the Mac");
    assert_eq!(
        mac.bodies(),
        ["note, from the phone", "note, typed on the Mac"]
    );
    settle(&mac, &phone);
    assert_eq!(phone.bodies().len(), 2);
    // With the current version as base, a save is a plain save.
    let now = mac.with(|c| pad::get(c, row).unwrap().unwrap().version);
    mac.with(|c| pad::save_from(c, row, "plain", at(4), Some(now)).unwrap());
    assert_eq!(mac.bodies().len(), 2);
}

#[test]
fn the_file_route_carries_notes() {
    let mac = device("Studio");
    let phone = device("iPhone");
    mac.write("by file", 1);
    let gone = mac.write("deleted before export", 2);
    phone.merge(&mac.state());
    mac.delete(gone, 3);
    let file = mac.file();
    assert_eq!(file.payload.notes.len(), 1);
    assert_eq!(file.payload.note_tombstones.len(), 1);
    let report = phone.merge_report(&file);
    assert_eq!(report.notes.removed, 1);
    assert_eq!(phone.bodies(), ["by file"]);
    // Leaving notes out of a review's choices leaves them alone.
    let other = device("iPad");
    other.with(|c| {
        let tx = c.transaction().unwrap();
        let choices = SyncChoices {
            notes: false,
            ..SyncChoices::ALL
        };
        merge(&tx, &file, choices, now_ms()).unwrap();
        tx.commit().unwrap();
    });
    assert!(other.bodies().is_empty());
}

#[test]
fn an_app_from_before_notes_synced_still_syncs_the_rest() {
    let mac = device("Studio");
    let phone = device("iPhone");
    mac.write("a note", 1);
    // The newer app's state, as the older app reads it: version-1 fields
    // only (serde skips the rest), and no tombstone of a kind it checks.
    let state = mac.state();
    let json = serde_json::to_value(&state.payload).unwrap();
    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct OldPayload {
        settings: serde_json::Value,
        dictionary: Vec<serde_json::Value>,
        snippets: Vec<serde_json::Value>,
        tombstones: Vec<super::Tombstone>,
        stats: Vec<serde_json::Value>,
        app_stats: Vec<serde_json::Value>,
    }
    let old: OldPayload = serde_json::from_value(json).unwrap();
    assert!(old.tombstones.is_empty());
    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct OldManifest {
        format: String,
        version: u32,
        device_id: String,
    }
    serde_json::from_value::<OldManifest>(serde_json::to_value(&state.manifest).unwrap()).unwrap();
    assert_eq!(state.manifest.minor, super::secure::MINOR);

    // The older app's state: no notes, no minor. Merging it here changes no
    // note and the manifest reads as minor 0 (the page asks for an update).
    let mut manifest = serde_json::to_value(&phone.state().manifest).unwrap();
    manifest.as_object_mut().unwrap().remove("minor");
    let mut payload = serde_json::to_value(&phone.state().payload).unwrap();
    payload.as_object_mut().unwrap().remove("notes");
    payload.as_object_mut().unwrap().remove("note_tombstones");
    let old_state = Incoming {
        kind: SourceKind::Bundle,
        manifest: serde_json::from_value(manifest).unwrap(),
        payload: serde_json::from_value::<Payload>(payload).unwrap(),
    };
    assert_eq!(old_state.manifest.minor, 0);
    assert_eq!(mac.merge(&old_state), NOTHING);
    assert_eq!(mac.bodies(), ["a note"]);
}

#[test]
fn nothing_in_a_note_is_trusted() {
    let mac = device("Studio");
    let phone = device("iPhone");
    mac.write("fine", 1);
    let mut state = mac.state();
    let good = state.payload.notes[0].clone();
    state.payload.notes = vec![
        with_note(&good, |n| n.id = "../../etc".into()),
        with_note(&good, |n| {
            n.id = "00000000-0000-4000-8000-000000000001".into();
            n.body = "x".repeat(pad::MAX_BODY_CHARS + 1);
        }),
        with_note(&good, |n| {
            n.id = "00000000-0000-4000-8000-000000000002".into();
            n.title = "bad\u{7}title".into();
        }),
        good,
    ];
    let counts = phone.merge(&state);
    assert_eq!((counts.added, counts.skipped), (2, 2));
    assert!(phone.notes().iter().any(|n| n.title == "badtitle"));
}

#[test]
fn typing_in_a_note_another_device_deleted_brings_it_back() {
    let mac = device("Studio");
    let phone = device("iPhone");
    let row = mac.write("open on the Mac", 1);
    let opened_at = mac.with(|c| pad::get(c, row).unwrap().unwrap().version);
    phone.merge(&mac.state());
    phone.delete(phone.row("open on the Mac"), 2);
    assert_eq!(mac.merge(&phone.state()).removed, 1);
    // The Mac's editor saves what the user typed meanwhile.
    let saved = mac.with(|c| {
        pad::save_from(
            c,
            row,
            "open on the Mac, and typed on",
            at(3),
            Some(opened_at),
        )
        .unwrap()
    });
    assert_eq!(saved.unwrap().body, "open on the Mac, and typed on");
    settle(&mac, &phone);
    assert_eq!(phone.bodies(), ["open on the Mac, and typed on"]);
    // Without an editor's base (or unchanged text) a deleted note stays deleted.
    phone.delete(phone.row("open on the Mac, and typed on"), 4);
    mac.merge(&phone.state());
    assert!(mac
        .with(|c| pad::save(c, row, "other", at(5)).unwrap())
        .is_none());
}
