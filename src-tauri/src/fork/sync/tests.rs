// SPDX-License-Identifier: GPL-3.0-or-later
//! Sync tests: the file round trip, every merge rule, the Mac export and
//! CSV sources, and the commands on a mock app. All data is synthetic.

use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::Connection;
use serde_json::{json, Value};
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::{App, Manager};

use super::merge::{merge, MergeReport, SyncChoices, SyncCounts};
use super::{bundle, read, settings, store, Incoming, ReadProblem, SourceKind, Stamped};
use crate::fork::commands::sync::{
    fork_sync_apply, fork_sync_export, fork_sync_review, fork_sync_status, SyncReview,
};
use crate::fork::context::categories::UsageCategory;
use crate::fork::context::now_ms;
use crate::fork::db::stats::{apply, summary, Contribution};
use crate::fork::db::{test_migrations, ForkDb};
use crate::fork::dictionary::{self, EntryInput, EntrySource};
use crate::fork::settings::{FakeHandyStore, ForkSettingUpdate, ForkSettingsHandle};
use crate::fork::snippets::{self, SnippetInput, SnippetOrigin};
use crate::settings::get_default_settings;

// ── Helpers ─────────────────────────────────────────────────────────────

/// A device: its own database in its own folder.
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

impl Device {
    fn with<T>(&self, f: impl FnOnce(&mut Connection) -> T) -> T {
        self.db.with_conn(|c| Ok(f(c))).unwrap()
    }

    fn word(&self, phrase: &str, at: i64) {
        self.entry(phrase, None, at);
    }

    fn entry(&self, phrase: &str, replacement: Option<&str>, at: i64) {
        let clean = dictionary::clean(&EntryInput {
            phrase: phrase.into(),
            replacement: replacement.map(Into::into),
        })
        .unwrap();
        self.with(|c| {
            let ids: Vec<i64> = dictionary::store::list(c)
                .unwrap()
                .into_iter()
                .filter(|e| e.kind == clean.kind && dictionary::norm_of(&e.phrase) == clean.norm)
                .map(|e| e.id)
                .collect();
            match ids.first() {
                Some(&id) => {
                    dictionary::store::update(c, id, &clean, at).unwrap();
                }
                None => {
                    dictionary::store::add(c, &clean, EntrySource::User, at).unwrap();
                }
            }
        });
    }

    fn delete_entry(&self, phrase: &str) {
        self.with(|c| {
            let ids: Vec<i64> = dictionary::store::list(c)
                .unwrap()
                .into_iter()
                .filter(|e| e.phrase == phrase)
                .map(|e| e.id)
                .collect();
            assert!(!ids.is_empty(), "{phrase} is not there");
            dictionary::store::delete(c, &ids).unwrap();
        });
    }

    fn snippet(&self, trigger: &str, body: &str, at: i64) {
        let clean = snippets::clean(&SnippetInput {
            trigger: trigger.into(),
            body: body.into(),
        })
        .unwrap();
        self.with(|c| {
            let existing = snippets::store::list(c)
                .unwrap()
                .into_iter()
                .find(|s| snippets::norm_of(&s.trigger) == clean.norm);
            match existing {
                Some(s) => {
                    snippets::store::update(c, s.id, &clean, at).unwrap();
                }
                None => {
                    snippets::store::add(c, &clean, SnippetOrigin::User, at).unwrap();
                }
            }
        });
    }

    fn delete_snippet(&self, trigger: &str) {
        self.with(|c| {
            let ids: Vec<i64> = snippets::store::list(c)
                .unwrap()
                .into_iter()
                .filter(|s| s.trigger == trigger)
                .map(|s| s.id)
                .collect();
            snippets::store::delete(c, &ids).unwrap();
        });
    }

    fn take(&self, day: &str, words: i64, speech_ms: i64) {
        self.with(|c| {
            apply(
                c,
                None,
                Some(&Contribution {
                    day: day.into(),
                    usage_category: UsageCategory::Email,
                    words,
                    speech_ms,
                    fixes_dictionary: 0,
                    fixes_llm: 0,
                }),
            )
            .unwrap();
        });
    }

    fn setting(&self, key: &str, value: Value, at: i64) {
        let current = BTreeMap::from([(key.to_owned(), value)]);
        self.with(|c| settings::observe(c, &current, &BTreeMap::new(), at).unwrap());
    }

    /// This device's file, written to disk and read back.
    fn file(&self) -> Incoming {
        let path = self.dir.path().join(format!("{}.wisprbro", self.name));
        let (manifest, payload) =
            self.with(|c| store::collect(c, self.name, "1.0.0-test", now_ms()).unwrap());
        bundle::write(&path, &manifest, &payload).unwrap();
        read(&path, now_ms()).unwrap()
    }

    fn merge(&self, incoming: &Incoming) -> MergeReport {
        self.merge_with(incoming, SyncChoices::ALL)
    }

    fn merge_with(&self, incoming: &Incoming, choices: SyncChoices) -> MergeReport {
        self.with(|c| {
            let tx = c.transaction().unwrap();
            let outcome = merge(&tx, incoming, choices, now_ms()).unwrap();
            tx.commit().unwrap();
            outcome.report
        })
    }

    fn words(&self) -> Vec<String> {
        let mut words: Vec<String> = self.with(|c| {
            dictionary::store::list(c)
                .unwrap()
                .into_iter()
                .map(|e| match e.replacement {
                    Some(to) => format!("{} → {to}", e.phrase),
                    None => e.phrase,
                })
                .collect()
        });
        words.sort();
        words
    }

    fn snippet_bodies(&self) -> Vec<(String, String)> {
        let mut all: Vec<_> = self.with(|c| {
            snippets::store::list(c)
                .unwrap()
                .into_iter()
                .map(|s| (s.trigger, s.body))
                .collect()
        });
        all.sort();
        all
    }

    /// Everything a merge is supposed to make equal on two devices.
    fn state(&self) -> Vec<String> {
        self.with(|c| {
            let rows = |sql: &str| -> Vec<String> {
                let mut stmt = c.prepare(sql).unwrap();
                let n = stmt.column_count();
                let rows = stmt
                    .query_map([], |r| {
                        Ok((0..n)
                            .map(|i| format!("{:?}", r.get_ref(i).unwrap()))
                            .collect::<Vec<_>>()
                            .join("|"))
                    })
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();
                rows
            };
            let mut all = rows(
                "SELECT 'entry', kind, norm, term, from_text, to_text, status, updated_at
                 FROM dictionary_entries WHERE status <> 'suggested' ORDER BY kind, norm",
            );
            all.extend(rows(
                r#"SELECT 'snippet', trigger_norm, "trigger", body, updated_at
                   FROM snippets ORDER BY trigger_norm"#,
            ));
            all.extend(rows(
                "SELECT 'gone', kind, norm, deleted_at FROM sync_tombstones ORDER BY kind, norm",
            ));
            all.extend(rows(
                "SELECT 'setting', key, value, updated_at FROM sync_settings
                 WHERE updated_at > 0 ORDER BY key",
            ));
            all
        })
    }

    fn total_words(&self) -> u64 {
        self.with(|c| summary(c, day("2026-11-10")).unwrap().total_words)
    }
}

fn day(s: &str) -> chrono::NaiveDate {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

/// One clock reading for the whole test run, so `past(1) < past(2)`
/// whenever they are called.
fn base() -> i64 {
    static BASE: std::sync::OnceLock<i64> = std::sync::OnceLock::new();
    *BASE.get_or_init(now_ms)
}

/// A time well before now (deletions are stamped with the real clock).
fn past(n: i64) -> i64 {
    base() - 10_000_000 + n
}

/// A time after any deletion made in this test.
fn later(n: i64) -> i64 {
    base() + 10_000_000 + n
}

const NOTHING: SyncCounts = SyncCounts {
    added: 0,
    changed: 0,
    removed: 0,
    skipped: 0,
};

// ── The file ────────────────────────────────────────────────────────────

#[test]
fn a_file_round_trips_and_carries_no_history() {
    let mac = device("Finn's MacBook");
    mac.word("Siobhán", past(1));
    mac.entry("gonna", Some("going to"), past(2));
    mac.snippet("my address", "1 Example Street\nSpringfield", past(3));
    mac.take("2026-11-08", 120, 60_000);
    mac.setting("text_retention_days", json!(30), past(4));
    mac.word("Zeta", past(5));
    mac.delete_entry("Zeta");

    let path = mac.dir.path().join("out.wisprbro");
    let (manifest, payload) = mac.with(|c| store::collect(c, mac.name, "1.2.3", 42).unwrap());
    bundle::write(&path, &manifest, &payload).unwrap();
    let back = read(&path, 0).unwrap();
    assert_eq!(back.kind, SourceKind::Bundle);
    assert_eq!(back.manifest, manifest);
    assert_eq!(back.payload, payload);
    assert_eq!(manifest.device_name, "Finn's MacBook");
    assert_eq!(manifest.platform, "macos");
    assert_eq!(manifest.app_version, "1.2.3");
    assert_eq!(manifest.created_at, 42);
    assert_eq!(manifest.device_id.len(), 32);
    assert_eq!(payload.dictionary.len(), 2);
    assert_eq!(payload.snippets.len(), 1);
    assert_eq!(payload.stats.len(), 1);
    assert_eq!(payload.app_stats.len(), 1);
    assert_eq!(payload.tombstones.len(), 1);
    assert_eq!(payload.tombstones[0].norm, "zeta");
    assert_eq!(payload.settings["text_retention_days"].value, json!(30));

    let archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
    let mut names: Vec<&str> = archive.file_names().collect();
    names.sort_unstable();
    assert_eq!(names, ["manifest.json", "sync.json"]);
}

#[test]
fn files_that_are_not_ours_or_too_new_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let text = dir.path().join("notes.wisprbro");
    std::fs::write(&text, "hello").unwrap();
    assert_eq!(read(&text, 0), Err(ReadProblem::NotWisprBro));
    assert_eq!(
        read(&dir.path().join("missing.wisprbro"), 0),
        Err(ReadProblem::Unreadable)
    );

    let mac = device("Mac");
    let (mut manifest, payload) = mac.with(|c| store::collect(c, "Mac", "9.9.9", 1).unwrap());
    manifest.version = super::FORMAT_VERSION + 1;
    let newer = dir.path().join("newer.wisprbro");
    bundle::write(&newer, &manifest, &payload).unwrap();
    assert_eq!(read(&newer, 0), Err(ReadProblem::TooNew));

    // Some other zip.
    let other = dir.path().join("other.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&other).unwrap());
    zip.start_file("readme.txt", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.finish().unwrap();
    assert_eq!(read(&other, 0), Err(ReadProblem::NotWisprBro));
}

// ── Merge rules ─────────────────────────────────────────────────────────

#[test]
fn new_entries_are_added_and_a_second_merge_changes_nothing() {
    let mac = device("Mac");
    let phone = device("iPhone");
    mac.word("Siobhán", past(1));
    mac.entry("gonna", Some("going to"), past(2));
    mac.snippet("sign off", "Best wishes", past(3));
    phone.word("Kubernetes", past(4));

    let file = mac.file();
    let report = phone.merge(&file);
    assert_eq!(report.dictionary.added, 2);
    assert_eq!(report.snippets.added, 1);
    assert_eq!(phone.words(), ["Kubernetes", "Siobhán", "gonna → going to"]);
    assert_eq!(
        phone.snippet_bodies(),
        [("sign off".to_owned(), "Best wishes".to_owned())]
    );

    let before = phone.state();
    let again = phone.merge(&file);
    assert_eq!(again.dictionary, NOTHING);
    assert_eq!(again.snippets, NOTHING);
    assert!(again.settings.is_empty());
    assert_eq!(phone.state(), before);
}

#[test]
fn the_newer_edit_wins_whichever_way_the_file_travels() {
    let mac = device("Mac");
    let phone = device("iPhone");
    mac.entry("gonna", Some("going to"), past(1));
    phone.entry("gonna", Some("gonna be"), past(2));
    mac.snippet("sign off", "Best wishes", past(4));
    phone.snippet("Sign Off", "Cheers", past(3));

    let (from_mac, from_phone) = (mac.file(), phone.file());
    let on_mac = mac.merge(&from_phone);
    let on_phone = phone.merge(&from_mac);
    assert_eq!(on_mac.dictionary.changed, 1, "the phone's edit is newer");
    assert_eq!(on_mac.snippets, NOTHING, "the Mac's snippet is newer");
    assert_eq!(on_phone.dictionary, NOTHING);
    assert_eq!(on_phone.snippets.changed, 1);
    assert_eq!(mac.words(), ["gonna → gonna be"]);
    assert_eq!(phone.words(), ["gonna → gonna be"]);
    assert_eq!(mac.snippet_bodies(), phone.snippet_bodies());
    assert_eq!(mac.snippet_bodies()[0].1, "Best wishes");
    assert_eq!(mac.state(), phone.state());
}

#[test]
fn edits_at_the_same_instant_still_end_the_same_on_both() {
    let mac = device("Mac");
    let phone = device("iPhone");
    let at = past(1);
    mac.entry("gonna", Some("going to"), at);
    phone.entry("gonna", Some("gonna be"), at);
    let (from_mac, from_phone) = (mac.file(), phone.file());
    mac.merge(&from_phone);
    phone.merge(&from_mac);
    assert_eq!(mac.words(), phone.words());
    assert_eq!(mac.state(), phone.state());
}

#[test]
fn a_deleted_word_does_not_come_back() {
    let mac = device("Mac");
    let phone = device("iPhone");
    mac.word("Zeta", past(1));
    mac.snippet("old", "text", past(1));
    phone.merge(&mac.file());
    assert_eq!(phone.words(), ["Zeta"]);

    mac.delete_entry("Zeta");
    mac.delete_snippet("old");
    // The phone still has both and sends them; the Mac keeps them deleted.
    let report = mac.merge(&phone.file());
    assert_eq!(report.dictionary, NOTHING);
    assert_eq!(report.snippets, NOTHING);
    assert!(mac.words().is_empty());

    // The Mac's deletions reach the phone.
    let report = phone.merge(&mac.file());
    assert_eq!(report.dictionary.removed, 1);
    assert_eq!(report.snippets.removed, 1);
    assert!(phone.words().is_empty());
    assert!(phone.snippet_bodies().is_empty());
    assert_eq!(mac.state(), phone.state());

    // And a third device that never had the word does not get it either.
    let ipad = device("iPad");
    ipad.merge(&phone.file());
    assert!(ipad.words().is_empty());
}

#[test]
fn an_edit_after_a_deletion_survives_it() {
    let mac = device("Mac");
    let phone = device("iPhone");
    mac.entry("gonna", Some("going to"), past(1));
    phone.merge(&mac.file());
    mac.delete_entry("gonna");
    phone.entry("gonna", Some("going to be"), later(1));

    let (from_mac, from_phone) = (mac.file(), phone.file());
    let on_phone = phone.merge(&from_mac);
    let on_mac = mac.merge(&from_phone);
    assert_eq!(on_phone.dictionary, NOTHING, "edited after the deletion");
    assert_eq!(on_mac.dictionary.added, 1);
    assert_eq!(mac.words(), ["gonna → going to be"]);
    assert_eq!(mac.state(), phone.state());
}

#[test]
fn adding_a_deleted_word_again_clears_its_tombstone() {
    let mac = device("Mac");
    mac.word("Zeta", past(1));
    mac.delete_entry("Zeta");
    let gone = |d: &Device| -> i64 {
        d.with(|c| {
            c.query_row("SELECT COUNT(*) FROM sync_tombstones", [], |r| r.get(0))
                .unwrap()
        })
    };
    assert_eq!(gone(&mac), 1);
    mac.word("zeta", later(1));
    assert_eq!(gone(&mac), 0);
    // A rename leaves the old key behind as deleted.
    mac.with(|c| {
        let id = dictionary::store::list(c).unwrap()[0].id;
        let clean = dictionary::clean(&EntryInput {
            phrase: "Eta".into(),
            replacement: None,
        })
        .unwrap();
        dictionary::store::update(c, id, &clean, later(2)).unwrap();
    });
    let keys: Vec<String> = mac
        .file()
        .payload
        .tombstones
        .into_iter()
        .map(|t| t.norm)
        .collect();
    assert_eq!(keys, ["zeta"]);
    let phone = device("iPhone");
    phone.word("Zeta", past(1));
    phone.merge(&mac.file());
    assert_eq!(phone.words(), ["Eta"]);
}

#[test]
fn two_devices_that_changed_many_things_converge() {
    let mac = device("Mac");
    let phone = device("iPhone");
    // A shared start.
    for (i, w) in ["Alpha", "Beta", "Gamma", "Delta"].iter().enumerate() {
        mac.word(w, past(i as i64));
    }
    mac.entry("wanna", Some("want to"), past(10));
    mac.snippet("addr", "1 Example Street", past(11));
    mac.snippet("sig", "Best", past(12));
    phone.merge(&mac.file());

    // Both change things on their own.
    mac.delete_entry("Alpha");
    mac.entry("wanna", Some("would like to"), later(1));
    mac.word("Epsilon", later(2));
    mac.snippet("sig", "Best wishes", later(3));
    mac.setting("text_retention_days", json!(30), later(4));
    mac.setting("greeting_name", json!("Finn"), later(5));
    phone.delete_entry("Beta");
    phone.entry("wanna", Some("wish to"), later(6));
    phone.word("Zeta", later(7));
    phone.delete_snippet("addr");
    phone.snippet("sig", "Cheers", later(2));
    phone.setting("text_retention_days", json!(90), later(8));
    phone.setting("typing_wpm", json!(55), later(9));

    let (from_mac, from_phone) = (mac.file(), phone.file());
    mac.merge(&from_phone);
    phone.merge(&from_mac);
    assert_eq!(mac.state(), phone.state());
    assert_eq!(
        mac.words(),
        ["Delta", "Epsilon", "Gamma", "Zeta", "wanna → wish to"]
    );
    assert_eq!(
        mac.snippet_bodies(),
        [("sig".to_owned(), "Best wishes".to_owned())]
    );

    // Another round both ways changes nothing.
    let (from_mac, from_phone) = (mac.file(), phone.file());
    let before = mac.state();
    assert_eq!(mac.merge(&from_phone).dictionary, NOTHING);
    assert_eq!(phone.merge(&from_mac).snippets, NOTHING);
    assert_eq!(mac.state(), before);
    assert_eq!(phone.state(), before);
}

#[test]
fn nothing_in_a_file_is_trusted() {
    let mac = device("Mac");
    let phone = device("iPhone");
    let mut file = mac.file();
    file.payload.dictionary = vec![
        super::DictItem {
            phrase: "  ".into(),
            replacement: None,
            source: "nonsense".into(),
            status: "active".into(),
            created_at: 1,
            updated_at: 1,
        },
        super::DictItem {
            phrase: "ok\u{7}".into(),
            replacement: None,
            source: String::new(),
            status: "weird".into(),
            created_at: 1,
            updated_at: 1,
        },
        super::DictItem {
            phrase: "Fine".into(),
            replacement: None,
            source: "nonsense".into(),
            status: "weird".into(),
            created_at: 5,
            updated_at: 1,
        },
    ];
    file.payload.snippets = vec![super::SnippetItem {
        trigger: "!!!".into(),
        body: "x".into(),
        source: String::new(),
        created_at: 1,
        updated_at: 1,
    }];
    file.payload.stats = vec![
        super::DayStats {
            day: "not-a-day!".into(),
            words: 5,
            dictations: 1,
            ..Default::default()
        },
        super::DayStats {
            day: "2026-11-01".into(),
            words: -50,
            dictations: 1,
            ..Default::default()
        },
    ];
    file.payload.app_stats = vec![super::AppDayStats {
        day: "2026-11-01".into(),
        usage_category: "hacking".into(),
        words: 5,
        dictations: 1,
    }];
    file.payload.settings = BTreeMap::from([
        (
            "show_in_dock".to_owned(),
            Stamped {
                value: json!(false),
                updated_at: 9,
            },
        ),
        (
            "text_retention_days".to_owned(),
            Stamped {
                value: json!(0),
                updated_at: 9,
            },
        ),
        (
            "typing_wpm".to_owned(),
            Stamped {
                value: json!(100_000),
                updated_at: 9,
            },
        ),
        (
            "spoken_languages".to_owned(),
            Stamped {
                value: json!(["en", "../../etc"]),
                updated_at: 9,
            },
        ),
    ]);
    let report = phone.merge(&file);
    assert_eq!(report.dictionary.added, 1);
    assert_eq!(report.dictionary.skipped, 2);
    assert_eq!(report.snippets.skipped, 1);
    assert!(report.settings.is_empty(), "{:?}", report.settings);
    assert_eq!(report.stats_days, 1);
    assert_eq!(report.stats_words, 0);
    assert_eq!(phone.words(), ["Fine"]);
}

// ── Stats ───────────────────────────────────────────────────────────────

#[test]
fn stats_add_up_across_devices_and_never_double() {
    let mac = device("Mac");
    let phone = device("iPhone");
    mac.take("2026-11-08", 300, 120_000);
    mac.take("2026-11-09", 100, 60_000);
    phone.take("2026-11-09", 40, 30_000);
    phone.take("2026-11-10", 60, 30_000);

    let report = phone.merge(&mac.file());
    assert_eq!(report.stats_days, 2);
    assert_eq!(report.stats_words, 400);
    assert!(report.stats_changed);
    assert_eq!(phone.total_words(), 500);

    // The same file again, and then twice more: still 500.
    for _ in 0..3 {
        let again = phone.merge(&mac.file());
        assert!(!again.stats_changed);
        assert_eq!(phone.total_words(), 500);
    }

    // The Mac dictates more: its new file replaces its old numbers.
    mac.take("2026-11-10", 50, 30_000);
    phone.merge(&mac.file());
    assert_eq!(phone.total_words(), 550);

    // The other way: the Mac counts the phone once, and never its own
    // numbers coming back.
    mac.merge(&phone.file());
    mac.merge(&phone.file());
    assert_eq!(mac.total_words(), 550);

    let s = phone.with(|c| summary(c, day("2026-11-10")).unwrap());
    assert_eq!(s.other_devices, 1);
    assert_eq!(s.today_words, 110);
    assert_eq!(s.active_days, 3, "the union of both devices' days");
    assert_eq!(s.current_streak_days, 3);
    // 550 words in 270 s of speech, whoever spoke them.
    let wpm = s.words_per_minute.unwrap();
    assert!((wpm - 550.0 * 60_000.0 / 270_000.0).abs() < 1e-9, "{wpm}");
    let own = phone.with(|c| {
        c.query_row("SELECT SUM(words) FROM daily_stats", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap()
    });
    assert_eq!(own, 100, "this device's own numbers are untouched");
    let categories = phone.with(|c| crate::fork::db::insights::categories(c).unwrap());
    assert_eq!(categories.iter().map(|c| c.words).sum::<u64>(), 550);
}

#[test]
fn a_device_never_merges_its_own_numbers_from_a_third() {
    let mac = device("Mac");
    let phone = device("iPhone");
    let ipad = device("iPad");
    mac.take("2026-11-08", 100, 60_000);
    phone.take("2026-11-08", 10, 6_000);
    phone.merge(&mac.file());
    // The phone's file carries the phone's numbers only.
    assert_eq!(phone.file().payload.stats[0].words, 10);
    ipad.merge(&phone.file());
    ipad.merge(&mac.file());
    assert_eq!(ipad.total_words(), 110);
}

#[test]
fn leaving_a_part_out_leaves_it_alone() {
    let mac = device("Mac");
    let phone = device("iPhone");
    mac.word("Alpha", past(1));
    mac.snippet("sig", "Best", past(1));
    mac.take("2026-11-08", 100, 60_000);
    mac.setting("text_retention_days", json!(30), past(2));
    let report = phone.merge_with(
        &mac.file(),
        SyncChoices {
            settings: false,
            dictionary: true,
            snippets: false,
            stats: false,
        },
    );
    assert_eq!(report.dictionary.added, 1);
    assert_eq!(report.snippets, NOTHING);
    assert!(report.settings.is_empty());
    assert_eq!(phone.total_words(), 0);
    assert!(phone.snippet_bodies().is_empty());
    // The device is still remembered as synced with.
    assert_eq!(phone.with(|c| store::devices(c).unwrap()).len(), 1);
}

// ── Settings ────────────────────────────────────────────────────────────

#[test]
fn settings_merge_per_key_and_untouched_defaults_are_not_sent() {
    let mac = device("Mac");
    let phone = device("iPhone");
    let defaults = BTreeMap::from([
        ("text_retention_days".to_owned(), json!(60)),
        ("typing_wpm".to_owned(), Value::Null),
        ("greeting_name".to_owned(), Value::Null),
    ]);
    // First sight: a default is "not chosen", anything else is.
    mac.with(|c| {
        settings::observe(
            c,
            &BTreeMap::from([
                ("text_retention_days".to_owned(), json!(60)),
                ("typing_wpm".to_owned(), json!(70)),
                ("greeting_name".to_owned(), Value::Null),
            ]),
            &defaults,
            past(1),
        )
        .unwrap();
    });
    let sent = mac.file().payload.settings;
    assert_eq!(sent.keys().collect::<Vec<_>>(), ["typing_wpm"]);

    // The phone chose a cleanup level earlier and a typing speed later.
    phone.setting("text_retention_days", json!(90), past(0));
    phone.setting("typing_wpm", json!(55), past(5));
    mac.setting("text_retention_days", json!(30), past(3));
    mac.setting("greeting_name", json!("Finn"), past(4));

    let changes = phone.with(|c| settings::merge(c, &mac.file().payload.settings).unwrap());
    assert_eq!(
        changes,
        BTreeMap::from([
            ("text_retention_days".to_owned(), json!(30)),
            ("greeting_name".to_owned(), json!("Finn")),
        ]),
        "the Mac's later cleanup level and its name; the phone's later typing speed stays"
    );
    let changes = mac.with(|c| settings::merge(c, &phone.file().payload.settings).unwrap());
    assert_eq!(
        changes,
        BTreeMap::from([("typing_wpm".to_owned(), json!(55))])
    );
    assert_eq!(mac.state(), phone.state());

    // A later change on the phone is seen and stamped.
    phone.setting("greeting_name", json!("Finn J"), later(1));
    assert_eq!(
        phone.file().payload.settings["greeting_name"],
        Stamped {
            value: json!("Finn J"),
            updated_at: later(1)
        }
    );
}

#[test]
fn only_cross_device_settings_are_synced() {
    for key in settings::FORK_KEYS {
        assert!(settings::is_synced(key));
    }
    for key in [
        "hotkeys",
        "bar_position",
        "show_in_dock",
        "update_channel",
        "onboarding",
        "selected_microphone",
        "theme",
    ] {
        assert!(!settings::is_synced(key), "{key}");
    }
    // Every synced fork key is a real field with an update of its name.
    let fork = serde_json::to_value(crate::fork::settings::ForkSettings::default()).unwrap();
    let handy = serde_json::to_value(get_default_settings()).unwrap();
    let picked = settings::pick(&fork, Some(&handy));
    assert_eq!(
        picked.len(),
        settings::FORK_KEYS.len() + settings::HANDY_KEYS.len()
    );
    for (key, value) in &picked {
        assert!(settings::valid(key, value), "{key}: {value}");
    }
    assert!(settings::valid("styles", &fork["styles"]));
    assert!(!settings::valid("text_retention_days", &json!(0)));
    assert!(!settings::valid("audio_feedback_volume", &json!(4.0)));
}

// ── The Mac app's "Export my data" ──────────────────────────────────────

/// A Mac data folder as the shipped app (schema 9, no sync tables) leaves
/// it, exported by the real `fork::export::export_data`.
fn mac_export(dir: &Path) -> std::path::PathBuf {
    let data = dir.join("Data");
    std::fs::create_dir_all(data.join("recordings")).unwrap();
    let db = ForkDb::with_migrations(data.join(crate::fork::db::DB_FILE_NAME), test_migrations(9));
    db.with_conn(|c| {
        let word = |phrase: &str, to: Option<&str>| {
            dictionary::clean(&EntryInput {
                phrase: phrase.into(),
                replacement: to.map(Into::into),
            })
            .unwrap()
        };
        dictionary::store::add(c, &word("Siobhán", None), EntrySource::User, 1_000)?;
        dictionary::store::add(
            c,
            &word("gonna", Some("going to")),
            EntrySource::Flag,
            2_000,
        )?;
        dictionary::store::add(c, &word("Learned", None), EntrySource::Learn, 3_000)?;
        let learned = dictionary::store::list(c)?
            .into_iter()
            .find(|e| e.phrase == "Learned")
            .unwrap();
        dictionary::store::delete(c, &[learned.id])?;
        let snippet = snippets::clean(&SnippetInput {
            trigger: "my address".into(),
            body: "1 Example Street\nSpringfield".into(),
        })
        .unwrap();
        snippets::store::add(c, &snippet, SnippetOrigin::User, 4_000)?;
        for (day, words) in [("2026-11-07", 200), ("2026-11-08", 300)] {
            apply(
                c,
                None,
                Some(&Contribution {
                    day: day.into(),
                    usage_category: UsageCategory::Work,
                    words,
                    speech_ms: 60_000,
                    fixes_dictionary: 1,
                    fixes_llm: 2,
                }),
            )?;
        }
        Ok(())
    })
    .unwrap();
    db.close();

    let mut fork = serde_json::to_value(crate::fork::settings::ForkSettings::default()).unwrap();
    fork["text_retention_days"] = json!(30);
    fork["greeting_name"] = json!("Finn");
    fork["typing_wpm"] = json!(72);
    fork["show_in_dock"] = json!(false);
    std::fs::write(
        data.join("fork-settings.json"),
        json!({ "settings": fork }).to_string(),
    )
    .unwrap();
    let mut handy = serde_json::to_value(get_default_settings()).unwrap();
    handy["spoken_languages"] = json!(["en", "nl"]);
    handy["audio_feedback"] = json!(true);
    handy["selected_microphone"] = json!("Studio Mic");
    std::fs::write(
        data.join("settings_store.json"),
        json!({ "settings": handy }).to_string(),
    )
    .unwrap();
    std::fs::write(data.join("recordings/take.wav"), b"RIFF-audio").unwrap();

    let history = vec![crate::fork::db::handy::fake::entry(
        1,
        1_000,
        "a synthetic take",
    )];
    let zip = dir.join("Wispr Bro data.zip");
    crate::fork::export::export_data(&data, &history, &zip).unwrap();
    zip
}

/// The two export tests unpack a database to the temporary folder; one of
/// them checks that nothing is left there.
static EXPORT_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn the_shipped_macs_export_syncs_to_the_phone() {
    let _one_at_a_time = EXPORT_TESTS.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let zip = mac_export(dir.path());
    let incoming = read(&zip, now_ms()).unwrap();
    assert_eq!(incoming.kind, SourceKind::MacExport);
    assert_eq!(incoming.manifest.device_id, super::LEGACY_MAC_ID);
    assert_eq!(incoming.manifest.platform, "macos");
    assert_eq!(incoming.manifest.app_version, env!("CARGO_PKG_VERSION"));
    assert!(incoming.manifest.created_at > 1_700_000_000_000);
    assert!(incoming.payload.tombstones.is_empty());
    // Only the cross-device settings, stamped with the export's time.
    let keys: Vec<&str> = incoming
        .payload
        .settings
        .keys()
        .map(String::as_str)
        .collect();
    let mut expected: Vec<&str> = settings::FORK_KEYS
        .iter()
        .chain(settings::HANDY_KEYS)
        .copied()
        .collect();
    expected.sort_unstable();
    assert_eq!(keys, expected);
    assert!(incoming
        .payload
        .settings
        .values()
        .all(|s| s.updated_at == incoming.manifest.created_at));

    let phone = device("iPhone");
    phone.word("Kubernetes", past(1));
    phone.take("2026-11-08", 25, 10_000);
    let outcome = phone.with(|c| {
        let tx = c.transaction().unwrap();
        let outcome = merge(&tx, &incoming, SyncChoices::ALL, now_ms()).unwrap();
        tx.commit().unwrap();
        outcome
    });
    let report = &outcome.report;
    assert_eq!(
        report.dictionary.added, 2,
        "the rejected learned word is not a word"
    );
    assert_eq!(report.snippets.added, 1);
    assert_eq!((report.stats_days, report.stats_words), (2, 500));
    assert_eq!(outcome.settings["text_retention_days"], json!(30));
    assert_eq!(outcome.settings["greeting_name"], json!("Finn"));
    assert_eq!(outcome.settings["typing_wpm"], json!(72));
    assert_eq!(outcome.settings["spoken_languages"], json!(["en", "nl"]));
    assert_eq!(outcome.settings["audio_feedback"], json!(true));
    assert!(!outcome.settings.contains_key("show_in_dock"));
    assert!(!outcome.settings.contains_key("selected_microphone"));
    assert_eq!(phone.words(), ["Kubernetes", "Siobhán", "gonna → going to"]);
    assert_eq!(phone.total_words(), 525);
    let rejected: i64 = phone.with(|c| {
        c.query_row(
            "SELECT COUNT(*) FROM dictionary_entries WHERE status = 'rejected' AND term = 'Learned'",
            [],
            |r| r.get(0),
        )
        .unwrap()
    });
    assert_eq!(
        rejected, 1,
        "auto-learn will not suggest it on the phone either"
    );
    let devices = phone.with(|c| store::devices(c).unwrap());
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].words, 500);

    // The same export again: nothing new, nothing counted twice.
    let again = phone.merge(&read(&zip, now_ms()).unwrap());
    assert_eq!(again.dictionary, NOTHING);
    assert_eq!(again.snippets, NOTHING);
    assert!(!again.stats_changed);
    assert_eq!(phone.total_words(), 525);

    // No unpacked database is left behind.
    let left: Vec<_> = std::fs::read_dir(std::env::temp_dir())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.starts_with(&format!("wispr-bro-sync-{}-", std::process::id()))
        })
        .collect();
    assert!(left.is_empty(), "{left:?}");
}

#[test]
fn a_mac_that_names_itself_replaces_its_unnamed_export() {
    let _one_at_a_time = EXPORT_TESTS.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let export = read(&mac_export(dir.path()), now_ms()).unwrap();
    let phone = device("iPhone");
    phone.merge(&export);
    assert_eq!(phone.total_words(), 500);

    // The Mac updates and sends a sync file with the same numbers.
    let mac = device("Finn's MacBook");
    mac.take("2026-11-07", 200, 60_000);
    mac.take("2026-11-08", 300, 60_000);
    phone.merge(&mac.file());
    assert_eq!(phone.total_words(), 500, "not 1000");
    let devices = phone.with(|c| store::devices(c).unwrap());
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].name, "Finn's MacBook");

    // Its old-style export afterwards adds nothing.
    let report = phone.merge(&export);
    assert_eq!(report.stats_days, 0);
    assert_eq!(phone.total_words(), 500);
    assert_eq!(phone.with(|c| store::devices(c).unwrap()).len(), 1);
}

// ── CSV ─────────────────────────────────────────────────────────────────

#[test]
fn the_pages_csv_exports_are_read() {
    let mac = device("Mac");
    mac.word("Siobhán", past(1));
    mac.entry("gonna", Some("going to"), past(2));
    mac.entry("=sum", Some("total"), past(3));
    mac.snippet("my address", "1 Example Street\nSpringfield", past(4));
    let words = mac.dir.path().join("Dictionary.csv");
    let texts = mac.dir.path().join("Snippets.csv");
    mac.with(|c| {
        dictionary::export::write_file(&words, &dictionary::store::list(c).unwrap()).unwrap();
        snippets::export::write_file(&texts, &snippets::store::list(c).unwrap()).unwrap();
    });

    let phone = device("iPhone");
    phone.entry("gonna", Some("gonna be"), past(9));
    let dictionary_csv = read(&words, now_ms()).unwrap();
    assert_eq!(dictionary_csv.kind, SourceKind::DictionaryCsv);
    assert_eq!(dictionary_csv.manifest.device_name, "Dictionary.csv");
    let report = phone.merge(&dictionary_csv);
    assert_eq!((report.dictionary.added, report.dictionary.changed), (2, 1));
    assert_eq!(
        phone.words(),
        ["=sum → total", "Siobhán", "gonna → going to"]
    );
    let snippets_csv = read(&texts, now_ms()).unwrap();
    assert_eq!(snippets_csv.kind, SourceKind::SnippetsCsv);
    assert_eq!(phone.merge(&snippets_csv).snippets.added, 1);
    assert_eq!(phone.snippet_bodies()[0].1, "1 Example Street\nSpringfield");
    // A CSV is not a device.
    assert!(phone.with(|c| store::devices(c).unwrap()).is_empty());

    let other = mac.dir.path().join("other.csv");
    std::fs::write(&other, "name,age\nSam,3\n").unwrap();
    assert_eq!(read(&other, 0), Err(ReadProblem::NotWisprBro));
}

// ── Commands on a mock app ──────────────────────────────────────────────

fn mock_app(dir: &Path) -> App<MockRuntime> {
    mock_builder()
        .manage(ForkDb::in_dir(dir))
        .manage(FakeHandyStore::with(get_default_settings()))
        .manage(ForkSettingsHandle::new(dir.join("fork_settings.json")))
        .plugin(crate::fork::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .build(mock_context(noop_assets()))
        .expect("mock app")
}

fn block<T>(f: impl std::future::Future<Output = T>) -> T {
    tauri::async_runtime::block_on(f)
}

#[test]
fn export_review_and_apply_through_the_commands() {
    let mac_dir = tempfile::tempdir().unwrap();
    let phone_dir = tempfile::tempdir().unwrap();
    let mac = mock_app(mac_dir.path());
    let phone = mock_app(phone_dir.path());
    let (mac_app, phone_app) = (mac.handle().clone(), phone.handle().clone());

    // The Mac: a word, a snippet, a take, and two chosen settings.
    mac.state::<ForkDb>()
        .with_conn(|c| {
            let word = dictionary::clean(&EntryInput {
                phrase: "Siobhán".into(),
                replacement: None,
            })
            .unwrap();
            dictionary::store::add(c, &word, EntrySource::User, past(1))?;
            let snippet = snippets::clean(&SnippetInput {
                trigger: "sig".into(),
                body: "Best wishes".into(),
            })
            .unwrap();
            snippets::store::add(c, &snippet, SnippetOrigin::User, past(2))?;
            apply(
                c,
                None,
                Some(&Contribution {
                    day: "2026-11-08".into(),
                    usage_category: UsageCategory::Email,
                    words: 321,
                    speech_ms: 90_000,
                    fixes_dictionary: 0,
                    fixes_llm: 0,
                }),
            )?;
            Ok(())
        })
        .unwrap();
    fork_sync_status(mac_app.clone()).unwrap();
    crate::fork::commands::settings::fork_set_setting(
        mac_app.clone(),
        ForkSettingUpdate::TextRetentionDays(Some(30)),
    )
    .unwrap();
    crate::fork::commands::settings::fork_set_setting(
        mac_app.clone(),
        ForkSettingUpdate::GreetingName(Some("Finn".into())),
    )
    .unwrap();
    {
        let store = mac.state::<FakeHandyStore>();
        let mut handy = store.get().unwrap();
        handy.spoken_languages = vec!["en".into(), "nl".into()];
        store.set(handy);
    }

    let target = mac_dir.path().join("To iPhone.wisprbro");
    let export = block(fork_sync_export(
        mac_app.clone(),
        Some(target.to_string_lossy().into_owned()),
        true,
    ))
    .unwrap();
    assert_eq!(export.paths.len(), 3);
    assert_eq!((export.words, export.snippets), (1, 1));
    assert!(export.paths.iter().all(|p| Path::new(p).is_file()));
    assert!(fork_sync_status(mac_app.clone())
        .unwrap()
        .last_export_at
        .is_some());

    // The Mac refuses its own file.
    let own = block(fork_sync_review(
        mac_app.clone(),
        export.paths[0].clone(),
        SyncChoices::ALL,
    ))
    .unwrap();
    assert_eq!(
        own,
        SyncReview::Problem {
            problem: ReadProblem::OwnFile
        }
    );

    // The phone reviews: nothing changes yet.
    let review = block(fork_sync_review(
        phone_app.clone(),
        export.paths[0].clone(),
        SyncChoices::ALL,
    ))
    .unwrap();
    let SyncReview::Ready {
        source,
        platform,
        has_settings,
        has_stats,
        report,
        ..
    } = &review
    else {
        panic!("{review:?}");
    };
    assert_eq!(*source, SourceKind::Bundle);
    assert_eq!(platform, "macos");
    assert!(*has_settings && *has_stats);
    assert_eq!(report.dictionary.added, 1);
    assert_eq!(report.snippets.added, 1);
    assert_eq!(
        report.settings,
        ["greeting_name", "spoken_languages", "text_retention_days"]
    );
    assert_eq!(report.stats_words, 321);
    let status = fork_sync_status(phone_app.clone()).unwrap();
    assert!(status.devices.is_empty(), "a review merges nothing");
    assert!(phone
        .state::<ForkDb>()
        .with_conn(|c| dictionary::store::list(c))
        .unwrap()
        .is_empty());

    // Apply: the same numbers, now real, in the database and both stores.
    let applied = block(fork_sync_apply(
        phone_app.clone(),
        export.paths[0].clone(),
        SyncChoices::ALL,
    ))
    .unwrap();
    assert_eq!(applied, review);
    let settings = crate::fork::commands::settings::fork_get_settings(phone_app.clone()).unwrap();
    assert_eq!(settings.text_retention_days, Some(30));
    assert_eq!(settings.greeting_name.as_deref(), Some("Finn"));
    let handy = phone.state::<FakeHandyStore>().get().unwrap();
    assert_eq!(handy.spoken_languages, ["en", "nl"]);
    assert_eq!(handy.custom_words, ["Siobhán"], "Handy's word list follows");
    let status = fork_sync_status(phone_app.clone()).unwrap();
    assert_eq!(status.devices.len(), 1);
    assert_eq!(status.devices[0].words, 321);
    assert!(status.devices[0].last_import_at.is_some());

    // Again: nothing to do.
    let again = block(fork_sync_apply(
        phone_app.clone(),
        export.paths[0].clone(),
        SyncChoices::ALL,
    ))
    .unwrap();
    let SyncReview::Ready { report, .. } = again else {
        panic!()
    };
    assert_eq!(report.dictionary, NOTHING);
    assert!(report.settings.is_empty());
    assert!(!report.stats_changed);

    // The phone's own later choice goes back to the Mac; the CSVs the phone
    // writes are the ones the shipped Mac's import wizard reads.
    crate::fork::commands::settings::fork_set_setting(
        phone_app.clone(),
        ForkSettingUpdate::TextRetentionDays(Some(90)),
    )
    .unwrap();
    let back = block(fork_sync_export(
        phone_app.clone(),
        Some(
            phone_dir
                .path()
                .join("To Mac.wisprbro")
                .to_string_lossy()
                .into_owned(),
        ),
        true,
    ))
    .unwrap();
    let csv = std::fs::read(&back.paths[1]).unwrap();
    assert!(String::from_utf8_lossy(&csv).contains("phrase,replacement"));
    let on_mac = block(fork_sync_apply(
        mac_app.clone(),
        back.paths[0].clone(),
        SyncChoices::ALL,
    ))
    .unwrap();
    let SyncReview::Ready { report, .. } = on_mac else {
        panic!()
    };
    assert_eq!(report.settings, ["text_retention_days"]);
    assert_eq!(report.dictionary, NOTHING);
    assert_eq!(
        crate::fork::commands::settings::fork_get_settings(mac_app)
            .unwrap()
            .text_retention_days,
        Some(90)
    );
}

/// The cleanup level decides whether a device downloads and loads the
/// cleanup model: a sync never carries it, in either direction.
#[test]
fn the_cleanup_level_is_per_device() {
    assert!(!settings::is_synced("cleanup_level"));
    let fork = json!({ "cleanup_level": "medium", "greeting_name": "Finn" });
    let picked = settings::pick(&fork, None);
    assert_eq!(picked.keys().collect::<Vec<_>>(), ["greeting_name"]);
    // `merge` skips every key that is not synced, so a file from a build
    // that still sent a level changes nothing here.
}
