// SPDX-License-Identifier: GPL-3.0-or-later
//
// The tests marked "BetterWispr" are ports of
// Tests/BetterWisprCoreTests/CorrectionLearningTests.swift and the
// correction tests of CoreTests.swift (BetterWispr, commit
// 304335a073a98f7439e5f7b4fbc2c31b810b807e; the watcher and decision files
// are GPL-3.0, VoiceInk-derived; the learner's are Apache-2.0 over
// OpenWhispr's MIT learner). Modified by Wispr Bro contributors, 2026-11-02:
// ported to Rust; decisions are checked against our suggestion rules.

use super::diff::{comparable, corrections, edit_of, Correction};
use super::review::{self, MAX_PENDING, MAX_PER_EDIT};
use super::watcher::{is_commit_key, Commit, Observation, Outcome, Reason, Session, WINDOW_MS};
use crate::fork::db::ForkDb;
use crate::fork::dictionary::{store, EntryKind, EntrySource, EntryStatus};

fn c(heard: &str, corrected: &str) -> Correction {
    Correction::new(heard, corrected)
}

// ---------------------------------------------------------------------------
// diff: anchors and comparison (BetterWispr)

#[test]
fn edit_is_found_between_unique_anchors() {
    let before =
        "Dear team, as discussed earlier today: ping kv now. Thanks so much for the help, Sam";
    let after =
        "Dear crew, as discussed earlier today: ping KV now. Thanks so much for the help, Samantha";
    assert_eq!(
        edit_of("ping kv now", before, after).as_deref(),
        Some("ping KV now")
    );
    assert_eq!(
        edit_of(
            "ping kv now",
            "Status update: ping kv now",
            "Status update: ping KV now. Status update: done"
        ),
        None
    );
    assert_eq!(
        edit_of(
            "ping kv now",
            "ping kv now, see you soon",
            "ping KV now, see you soon, see you soon"
        ),
        None
    );
    assert_eq!(
        edit_of(
            "ping kv now",
            "left ping kv now right",
            " right then left again"
        ),
        None
    );
}

#[test]
fn a_missing_side_is_the_field_boundary() {
    assert_eq!(
        edit_of(
            "ping kv now",
            "ping kv now and then reply to Sam",
            "ping KV now and then reply to Sam"
        )
        .as_deref(),
        Some("ping KV now")
    );
    assert_eq!(
        edit_of(
            "ping kv now",
            "ping kv now and then reply to Sam",
            "Hi, ping KV now and then reply to Sam"
        )
        .as_deref(),
        Some("Hi, ping KV now")
    );
    assert_eq!(
        edit_of(
            "ping kv now",
            "Reply to Sam: ping kv now",
            "Reply to Sam: ping KV now"
        )
        .as_deref(),
        Some("ping KV now")
    );
}

#[test]
fn field_text_compares_the_way_it_was_pasted() {
    let field =
        comparable("  ping\u{00A0}kv\r\nnow\u{200B}\u{FEFF}\u{2028} \t\nthen cafe\u{0301}  ");
    assert_eq!(field, "ping kv\nnow\n\nthen caf\u{e9}");
    assert_eq!(
        comparable("ping\u{00A0}kv now\r"),
        comparable("ping kv now")
    );
}

#[test]
fn anchors_cut_on_character_boundaries() {
    let before = "Ünïcödé ✓ prefix — Siobhan said hi — süffix ✓ ënd";
    let after = "Ünïcödé ✓ prefix — Siobhán said hi — süffix ✓ ënd";
    assert_eq!(
        edit_of("Siobhan said hi", before, after).as_deref(),
        Some("Siobhán said hi")
    );
}

// ---------------------------------------------------------------------------
// diff: what is learned (BetterWispr CoreTests)

#[test]
fn spelling_fixes_are_learned_and_rewrites_are_not() {
    assert_eq!(
        corrections("Thanks kumr, see you", "Thanks Kumar, see you"),
        vec![c("kumr", "Kumar")]
    );
    assert_eq!(
        corrections("try assist able today", "try Assistable today"),
        vec![c("assist able", "Assistable")]
    );
    assert_eq!(
        corrections("ask kv about it", "ask KV about it"),
        vec![c("kv", "KV")]
    );
    assert!(corrections("why is it late", "what is it late").is_empty());
    assert!(corrections("hello there", "hello there!").is_empty());
    assert!(corrections("send the report today", "please call me tomorrow instead").is_empty());
}

#[test]
fn several_hunks_in_one_edit_are_each_learned() {
    let heard = "I dictate with better whisper and then paste it into cloud code. Open the superbase dashboard and check the post hog events.";
    let fixed = "I dictate with BetterWispr and then paste it into Claude Code. Open the Supabase dashboard and check the PostHog events.";
    let learned = corrections(heard, fixed);
    let corrected: Vec<&str> = learned.iter().map(|c| c.corrected.as_str()).collect();
    assert_eq!(
        corrected,
        ["BetterWispr", "Claude Code", "Supabase", "PostHog"]
    );
    assert_eq!(learned[0].heard, "better whisper");
}

#[test]
fn the_slice_example_is_learned() {
    assert_eq!(
        corrections(
            "Deploy it to Kubernetees today.",
            "Deploy it to Kubernetes today."
        ),
        vec![c("Kubernetees", "Kubernetes")]
    );
}

#[test]
fn a_capital_at_the_start_or_an_everyday_word_is_not_learned() {
    assert!(corrections("hello world", "Hello world").is_empty());
    assert!(corrections("send it to tim", "send it to the").is_empty());
    // Too different to be the same word.
    assert!(corrections("meet at the cafe", "meet at the Bellagio").is_empty());
}

#[test]
fn the_same_corrected_term_is_learned_once() {
    assert_eq!(
        corrections(
            "ask kumr and then kumr again today",
            "ask Kumar and then Kumar again today"
        ),
        vec![c("kumr", "Kumar")]
    );
}

#[test]
fn voiceink_checks_refuse_commas_controls_and_overlong_terms() {
    // More than three words a side is not a word fix.
    assert!(corrections(
        "one two three four five six",
        "one Twoo Threee Fourr Fivee six"
    )
    .is_empty());
    let long = "a".repeat(61);
    assert!(corrections("see foo now", &format!("see {long} now")).is_empty());
    // A compact script term longer than 32 characters.
    let cjk = "漢".repeat(33);
    assert!(corrections(
        &format!("see {} now", "漢".repeat(32)),
        &format!("see {cjk} now")
    )
    .is_empty());
}

#[test]
fn a_huge_edit_learns_nothing() {
    let words = vec!["word"; 1_100].join(" ");
    let edited = format!("{words} Kubernetes");
    assert!(corrections(&format!("{words} Kubernetees"), &edited).is_empty());
}

// ---------------------------------------------------------------------------
// watcher: the session

fn value(s: &str) -> Observation {
    Observation::Value(s.to_string())
}

#[test]
fn a_fix_committed_by_return_is_an_edit() {
    let mut s = Session::new("Deploy it to Kubernetees today.").unwrap();
    assert_eq!(
        s.observe(250, value("Hi! Deploy it to Kubernetees today."), false),
        None
    );
    assert_eq!(
        s.observe(500, value("Hi! Deploy it to Kubernet today."), false),
        None
    );
    assert_eq!(
        s.observe(750, value("Hi! Deploy it to Kubernetes today."), true),
        Some(Outcome::Edited {
            commit: Commit::Key,
            original: "Deploy it to Kubernetees today.".into(),
            edited: "Deploy it to Kubernetes today.".into(),
        })
    );
}

#[test]
fn a_sent_message_keeps_the_text_it_had() {
    let mut s = Session::new("ask kv").unwrap();
    s.observe(250, value("ask kv"), false);
    s.observe(500, value("ask KV"), false);
    // Return read too late: the composer is already empty.
    assert_eq!(
        s.observe(750, value(""), true),
        Some(Outcome::Edited {
            commit: Commit::Emptied,
            original: "ask kv".into(),
            edited: "ask KV".into(),
        })
    );
}

#[test]
fn focus_moving_away_commits() {
    let mut s = Session::new("ask kv").unwrap();
    s.observe(250, value("ask kv"), false);
    s.observe(500, value("ask KV"), false);
    assert!(matches!(
        s.observe(750, Observation::Moved, false),
        Some(Outcome::Edited {
            commit: Commit::Focus,
            ..
        })
    ));
}

#[test]
fn no_commit_within_thirty_seconds_learns_nothing() {
    let mut s = Session::new("ask kv").unwrap();
    s.observe(250, value("ask kv"), false);
    assert_eq!(s.observe(10_000, value("ask KV"), false), None);
    assert_eq!(
        s.observe(WINDOW_MS, value("ask KV"), false),
        Some(Outcome::Nothing(Reason::NoCommit))
    );
}

#[test]
fn unchanged_or_missing_dictations_learn_nothing() {
    let mut s = Session::new("ask kv").unwrap();
    s.observe(250, value("ask kv"), false);
    assert_eq!(
        s.observe(500, value("ask kv"), true),
        Some(Outcome::Nothing(Reason::NoEdit))
    );
    let mut s = Session::new("ask kv").unwrap();
    assert_eq!(s.observe(250, value("something else"), false), None);
    assert_eq!(
        s.observe(3_000, value("something else"), false),
        Some(Outcome::Nothing(Reason::NotFound))
    );
    let mut s = Session::new("ask kv").unwrap();
    assert_eq!(
        s.observe(250, Observation::Moved, false),
        Some(Outcome::Nothing(Reason::NotFound))
    );
}

#[test]
fn secure_input_and_overlong_fields_end_the_watch() {
    let mut s = Session::new("ask kv").unwrap();
    s.observe(250, value("ask kv"), false);
    assert_eq!(
        s.observe(500, Observation::SecureInput, false),
        Some(Outcome::Nothing(Reason::SecureInput))
    );
    let mut s = Session::new("ask kv").unwrap();
    assert_eq!(
        s.observe(250, Observation::TooLong, false),
        Some(Outcome::Nothing(Reason::TooLong))
    );
    assert!(Session::new("   ").is_none());
    assert!(Session::new(&"x".repeat(12_001)).is_none());
}

#[test]
fn unreadable_polls_end_the_watch_but_a_key_still_commits() {
    let mut s = Session::new("ask kv").unwrap();
    s.observe(250, value("ask kv"), false);
    s.observe(500, value("ask KV"), false);
    assert!(matches!(
        s.observe(750, Observation::Unreadable, true),
        Some(Outcome::Edited { .. })
    ));
    let mut s = Session::new("ask kv").unwrap();
    s.observe(250, value("ask kv"), false);
    let mut end = None;
    for i in 0..8 {
        end = s.observe(500 + i * 250, Observation::Unreadable, false);
    }
    assert_eq!(end, Some(Outcome::Nothing(Reason::Unreadable)));
}

#[test]
fn commit_keys() {
    use handy_keys::{Key, Modifiers};
    assert!(is_commit_key(Key::Return, Modifiers::empty()));
    assert!(is_commit_key(Key::KeypadEnter, Modifiers::empty()));
    assert!(is_commit_key(Key::Return, Modifiers::CMD_LEFT));
    assert!(is_commit_key(Key::Tab, Modifiers::empty()));
    assert!(!is_commit_key(Key::Return, Modifiers::SHIFT_LEFT));
    assert!(!is_commit_key(Key::Return, Modifiers::OPT_RIGHT));
    assert!(!is_commit_key(Key::Tab, Modifiers::CMD_LEFT));
    assert!(!is_commit_key(Key::Escape, Modifiers::empty()));
}

// ---------------------------------------------------------------------------
// review: the approval queue

fn db() -> (tempfile::TempDir, ForkDb) {
    let dir = tempfile::tempdir().unwrap();
    let db = ForkDb::in_dir(dir.path());
    (dir, db)
}

fn suggested(db: &ForkDb) -> Vec<crate::fork::dictionary::DictionaryEntry> {
    db.with_conn(|c| store::list(c))
        .unwrap()
        .into_iter()
        .filter(|e| e.status == EntryStatus::Suggested)
        .collect()
}

#[test]
fn ascii_fixes_become_words_and_others_replacements() {
    let w = review::entry_for(&c("Kubernetees", "Kubernetes")).unwrap();
    assert_eq!((w.kind, w.phrase.as_str()), (EntryKind::Word, "Kubernetes"));
    let r = review::entry_for(&c("Siobhan", "Siobhán")).unwrap();
    assert_eq!(r.kind, EntryKind::Rule);
    assert_eq!(
        (r.phrase.as_str(), r.replacement.as_deref()),
        ("Siobhan", Some("Siobhán"))
    );
    // An everyday word is never rewritten (BetterWispr).
    assert!(review::entry_for(&c("good", "Goode")).is_none());
}

#[test]
fn suggestions_wait_and_are_counted() {
    let (_d, db) = db();
    let added = db
        .with_conn(|conn| review::suggest(conn, &[c("Kubernetees", "Kubernetes")], 10))
        .unwrap();
    assert_eq!(added, 1);
    let s = suggested(&db);
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].phrase, "Kubernetes");
    assert_eq!(s[0].source, EntrySource::Learn);
    let counts = db.with_conn(|c| store::counts(c)).unwrap();
    assert_eq!((counts.words, counts.suggested), (0, 1));
    // Suggestions are not words yet: Handy's list does not get them.
    assert!(db.with_conn(|c| store::active_words(c)).unwrap().is_empty());
}

#[test]
fn known_terms_are_not_suggested_again() {
    let (_d, db) = db();
    db.with_conn(|conn| {
        let e = crate::fork::dictionary::clean(&crate::fork::dictionary::EntryInput {
            phrase: "post hog".into(),
            replacement: Some("PostHog".into()),
        })
        .unwrap();
        store::add(conn, &e, EntrySource::User, 1)?;
        let n = review::suggest(
            conn,
            &[c("Posthog", "posthog"), c("pax", "Pax"), c("Pax", "pax")],
            2,
        )?;
        assert_eq!(n, 1, "posthog is known, Pax only once");
        // Waiting suggestions count as known too.
        assert_eq!(review::suggest(conn, &[c("paks", "Pax")], 3)?, 0);
        Ok(())
    })
    .unwrap();
}

#[test]
fn rejected_suggestions_never_return() {
    let (_d, db) = db();
    db.with_conn(|conn| {
        review::suggest(conn, &[c("Kubernetees", "Kubernetes")], 1)?;
        let id = store::list(conn)?[0].id;
        assert_eq!(review::reject(conn, &[id], 2)?, 1);
        assert!(store::list(conn)?.is_empty());
        assert_eq!(store::counts(conn)?.suggested, 0);
        assert_eq!(
            review::suggest(conn, &[c("Kubernetis", "Kubernetes")], 3)?,
            0
        );
        assert_eq!(
            review::suggest(conn, &[c("kubernetees", "kubernetes")], 3)?,
            0
        );
        // Its corrections row went with it.
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM corrections", [], |r| r.get(0))?;
        assert_eq!(n, 0);
        Ok(())
    })
    .unwrap();
}

#[test]
fn accepted_suggestions_become_learned_words_and_a_deleted_one_never_returns() {
    let (_d, db) = db();
    db.with_conn(|conn| {
        review::suggest(
            conn,
            &[c("Kubernetees", "Kubernetes"), c("Siobhan", "Siobhán")],
            1,
        )?;
        let ids: Vec<i64> = store::list(conn)?.iter().map(|e| e.id).collect();
        let out = review::accept(conn, &ids, 2)?;
        assert_eq!((out.accepted, out.full), (2, 0));
        assert_eq!(store::active_words(conn)?, vec!["Kubernetes".to_string()]);
        let all = store::list(conn)?;
        assert!(all
            .iter()
            .all(|e| e.status == EntryStatus::Active && e.source == EntrySource::Learn));
        // The corrections rows stay for Insights (S-25).
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM corrections WHERE source = 'learn'",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(n, 2);
        // Accepting twice changes nothing.
        assert_eq!(review::accept(conn, &ids, 3)?.accepted, 0);

        let word = all
            .iter()
            .find(|e| e.kind == EntryKind::Word)
            .unwrap()
            .clone();
        let gone = store::delete(conn, &[word.id])?;
        assert_eq!(gone.len(), 1);
        assert_eq!(
            review::suggest(conn, &[c("Kubernetis", "Kubernetes")], 4)?,
            0
        );
        // Undo brings it back as it was.
        assert_eq!(store::restore(conn, &gone)?, 1);
        assert_eq!(
            store::get(conn, word.id)?.unwrap().status,
            EntryStatus::Active
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn the_user_adding_a_rejected_word_makes_it_active() {
    let (_d, db) = db();
    db.with_conn(|conn| {
        review::suggest(conn, &[c("Kubernetees", "Kubernetes")], 1)?;
        let id = store::list(conn)?[0].id;
        review::reject(conn, &[id], 2)?;
        let e = crate::fork::dictionary::clean(&crate::fork::dictionary::EntryInput {
            phrase: "Kubernetes".into(),
            replacement: None,
        })
        .unwrap();
        assert!(matches!(
            store::add(conn, &e, EntrySource::User, 3)?,
            store::SaveOutcome::Saved { .. }
        ));
        Ok(())
    })
    .unwrap();
}

#[test]
fn suggestions_are_capped() {
    let (_d, db) = db();
    db.with_conn(|conn| {
        let many: Vec<Correction> = (0..10)
            .map(|i| c(&format!("Termx{i}"), &format!("Termz{i}")))
            .collect();
        assert_eq!(review::suggest(conn, &many, 1)?, MAX_PER_EDIT);
        for round in 0..30 {
            let more: Vec<Correction> = (0..MAX_PER_EDIT)
                .map(|i| c(&format!("Wordx{round}q{i}"), &format!("Wordz{round}q{i}")))
                .collect();
            review::suggest(conn, &more, 2)?;
        }
        assert_eq!(store::counts(conn)?.suggested as usize, MAX_PENDING);
        Ok(())
    })
    .unwrap();
}

// ---------------------------------------------------------------------------
// Exit criterion: no field text persisted.

#[test]
fn only_the_word_pair_reaches_the_database() {
    let (_d, db) = db();
    let secret = "Private note: the door code is 4471 and rent is due";
    let pasted = "Deploy it to Kubernetees today.";
    let mut s = Session::new(pasted).unwrap();
    s.observe(250, value(&format!("{secret}. {pasted} Ciao")), false);
    let end = s
        .observe(
            500,
            value(&format!("{secret}. Deploy it to Kubernetes today. Ciao")),
            true,
        )
        .unwrap();
    let Outcome::Edited {
        original, edited, ..
    } = end
    else {
        panic!("expected an edit");
    };
    assert_eq!(edited, "Deploy it to Kubernetes today.");
    let found = corrections(&original, &edited);
    db.with_conn(|conn| review::suggest(conn, &found, 1))
        .unwrap();

    // Every text value in every table: nothing but the pair.
    let dump: Vec<String> = db
        .with_conn(|conn| {
            let tables: Vec<String> = conn
                .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")?
                .query_map([], |r| r.get(0))?
                .collect::<Result<_, _>>()?;
            let mut out = Vec::new();
            for t in tables {
                let mut stmt = conn.prepare(&format!("SELECT * FROM \"{t}\""))?;
                let n = stmt.column_count();
                let mut rows = stmt.query([])?;
                while let Some(row) = rows.next()? {
                    for i in 0..n {
                        if let Ok(Some(v)) = row.get::<_, Option<String>>(i) {
                            out.push(v);
                        }
                    }
                }
            }
            Ok(out)
        })
        .unwrap();
    for needle in ["door", "4471", "rent", "Deploy", "today", "Ciao", "Private"] {
        assert!(
            !dump.iter().any(|v| v.contains(needle)),
            "{needle:?} reached the database"
        );
    }
    assert!(dump.iter().any(|v| v == "Kubernetes"));
    assert!(dump.iter().any(|v| v == "Kubernetees"));
}

// ---------------------------------------------------------------------------
// End to end: a dictation, the user's fix, a waiting word

/// The whole learning path without AX: the watch sees the paste and the
/// user's fix committed with Return, the diff finds the correction, and it
/// waits in Dictionary › Suggested (with its `corrections` row) for the
/// bar's note or the Dictionary page to accept it.
#[test]
fn a_fix_after_a_dictation_becomes_a_waiting_word() {
    let pasted = "We moved the cluster to kubernetees last week.";
    let mut s = Session::new(pasted).unwrap();
    assert_eq!(s.observe(250, value(pasted), false), None);
    assert_eq!(
        s.observe(
            1_000,
            value("We moved the cluster to Kubernetes last week."),
            false
        ),
        None
    );
    let Some(Outcome::Edited {
        original, edited, ..
    }) = s.observe(
        1_250,
        value("We moved the cluster to Kubernetes last week."),
        true,
    )
    else {
        panic!("the fix was not seen as an edit");
    };
    let found = corrections(&original, &edited);
    assert_eq!(found, vec![c("kubernetees", "Kubernetes")]);

    let (_d, db) = db();
    let added = db
        .with_conn(|conn| review::suggest_entries(conn, &found, 42))
        .unwrap();
    assert_eq!(added.len(), 1);
    assert_eq!(added[0].word, "Kubernetes");
    let waiting = suggested(&db);
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0].id, added[0].id);
    assert_eq!(waiting[0].phrase, "Kubernetes");
    assert_eq!(waiting[0].source, EntrySource::Learn);
    let pairs: Vec<(String, String, String)> = db
        .with_conn(|conn| {
            let mut st = conn.prepare("SELECT from_text, to_text, source FROM corrections")?;
            let rows = st
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .unwrap();
    assert_eq!(
        pairs,
        vec![("kubernetees".into(), "Kubernetes".into(), "learn".into())]
    );

    // The note's Add accepts it into the dictionary.
    let out = db
        .with_conn(|conn| review::accept(conn, &[added[0].id], 43))
        .unwrap();
    assert_eq!(out.accepted, 1);
    assert!(suggested(&db).is_empty());
    assert_eq!(
        db.with_conn(|c| store::active_words(c)).unwrap(),
        vec!["Kubernetes".to_string()]
    );
}

/// A replacement suggestion's note names the corrected spelling.
#[test]
fn a_replacement_suggestion_names_the_corrected_spelling() {
    let (_d, db) = db();
    let added = db
        .with_conn(|conn| review::suggest_entries(conn, &[c("Siobhan", "Siobhán")], 1))
        .unwrap();
    assert_eq!(added.len(), 1);
    assert_eq!(added[0].word, "Siobhán");
}
