// SPDX-License-Identifier: GPL-3.0-or-later
//! Pipeline tests (R5 S-11): golden texts through every stage with Handy's
//! real `text.rs` stages, the "no stage removes `\n`" property, protection
//! of rule outputs, fail-open, and the per-take record.

use super::counts::Stage;
use super::rules::{Rule, RuleSet};
use super::*;
use crate::audio_toolkit::{
    apply_custom_words, normalize_transcription_output, remove_filler_words, OutputLanguageEvidence,
};

/// Handy's `post_process_transcription_text` for English output, built from
/// the same public `text.rs` functions (script conversion is a no-op for
/// English).
fn handy(custom_words: &[&str]) -> impl FnMut(String) -> String {
    let words: Vec<String> = custom_words.iter().map(|w| w.to_string()).collect();
    move |text: String| {
        let corrected = if words.is_empty() {
            text
        } else {
            apply_custom_words(&text, &words, 0.18)
        };
        let without = remove_filler_words(
            &corrected,
            &OutputLanguageEvidence::UserSelected("en".into()),
            &None,
            true,
        );
        normalize_transcription_output(&without)
    }
}

fn rule(id: i64, from: &str, to: &str) -> Rule {
    Rule {
        id,
        from: from.into(),
        to: to.into(),
    }
}

fn en(raw: &str, rules: &RuleSet, words: &[&str]) -> (String, counts::TextRecord) {
    run(raw, Some("en"), rules, &mut handy(words))
}

#[test]
fn handy_alone_flattens_every_newline() {
    // AF-06, the reason for the wrapper.
    assert_eq!(handy(&[])("one\ntwo\n\nthree".into()), "one two three");
}

#[test]
fn golden_three_paragraph_email() {
    let rules = RuleSet::new(vec![rule(1, "wispr bro", "Wispr Bro")]);
    let raw = "Hi Anna, new paragraph, um, thanks for the notes on wispr bro. \
               I'll send the draft Monday, scratch that, I'll send it Tuesday. \
               New paragraph. Best, new line, Finn";
    let (text, record) = en(raw, &rules, &[]);
    assert_eq!(
        text,
        "Hi Anna,\n\nThanks for the notes on Wispr Bro. I'll send it Tuesday.\n\nBest,\nFinn"
    );
    assert_eq!(
        record
            .stages
            .iter()
            .map(|s| (s.stage, s.changes))
            .collect::<Vec<_>>(),
        [
            (Stage::Rules, 1),
            (Stage::Handy, 1),
            (Stage::Commands, 3),
            (Stage::Scratch, 1),
            (Stage::Whitespace, 0),
        ]
    );
    assert_eq!(record.dictionary_fixes(), 1);
    assert_eq!(record.rule_hits[0].from_text, "wispr bro");
}

#[test]
fn golden_two_line_chat_message() {
    let (text, _) = en(
        "Running late. New line. Start without me",
        &RuleSet::empty(),
        &[],
    );
    assert_eq!(text, "Running late.\nStart without me");
}

#[test]
fn multi_line_input_survives_handy_stages() {
    let (text, _) = en(
        "um first line  here\nsecond uh line\n\nthird",
        &RuleSet::empty(),
        &[],
    );
    assert_eq!(text, "first line here\nsecond line\n\nthird");
}

#[test]
fn rule_outputs_are_protected_from_fuzzy_custom_words() {
    // Without protection Handy's fuzzy matcher would turn the rule's "Jon"
    // into the custom word "John".
    assert_eq!(handy(&["John"])("Jon".into()), "John");
    let rules = RuleSet::new(vec![rule(1, "jonn", "Jon")]);
    let (text, record) = en("ask jonn and jon", &rules, &["John"]);
    assert_eq!(text, "ask Jon and John");
    assert_eq!(record.dictionary_fixes(), 1);
}

#[test]
fn rule_outputs_keep_their_newlines_and_fillers() {
    // A rule output is literal: Handy's filler removal never sees it.
    let rules = RuleSet::new(vec![rule(1, "my sign off", "Um, cheers,\nFinn")]);
    let (text, _) = en("thanks, my sign off", &rules, &[]);
    assert_eq!(text, "thanks, Um, cheers,\nFinn");
}

#[test]
fn a_moved_placeholder_falls_back_to_unprotected_rules() {
    // A stage that glues the placeholder to the word before it.
    let mut glue = |t: String| t.replacen(" \u{E000}", "\u{E000}", 1);
    let rules = RuleSet::new(vec![rule(1, "foo", "Bar")]);
    let (text, record) = run("say foo now", Some("en"), &rules, &mut glue);
    // The fallback ran the stage on the unprotected text: no placeholder
    // leaked, the rule still applied.
    assert_eq!(text, "say Bar now");
    assert!(!placeholders::collides(&text));
    assert_eq!(record.dictionary_fixes(), 1);
}

#[test]
fn text_with_private_use_characters_is_not_protected_but_still_ruled() {
    let rules = RuleSet::new(vec![rule(1, "foo", "Bar")]);
    let (text, _) = en("foo \u{E000}", &rules, &[]);
    assert_eq!(text, "Bar \u{E000}");
}

#[test]
fn literal_escape_and_clause_edges_end_to_end() {
    let (text, _) = en(
        "We launched a new line of shoes. Type literally new line, please.",
        &RuleSet::empty(),
        &[],
    );
    assert_eq!(
        text,
        "We launched a new line of shoes. Type new line, please."
    );
}

#[test]
fn unknown_language_uses_every_table_and_none_for_untabled_ones() {
    let (de, _) = run(
        "Hallo. Neue Zeile. Tschüss",
        None,
        &RuleSet::empty(),
        &mut |t| t,
    );
    assert_eq!(de, "Hallo.\nTschüss");
    let (ru, _) = run(
        "Привет. New line. Пока",
        Some("ru"),
        &RuleSet::empty(),
        &mut |t| t,
    );
    assert_eq!(ru, "Привет. New line. Пока");
}

/// A small deterministic generator (no extra dev-dependency).
struct Lcg(u64);
impl Lcg {
    fn next(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % n
    }
}

const TOKENS: &[&str] = &[
    "hello",
    "um",
    "uh",
    "new",
    "line",
    "paragraph",
    "scratch",
    "that",
    "question",
    "mark",
    "literally",
    "a",
    "the",
    "I",
    "I",
    "I",
    "wispr",
    "bro",
    ",",
    ".",
    "?",
    "\n",
    "\n",
    "  ",
    "new line",
    "new paragraph",
    "scratch that",
    "Jon",
    "jonn",
    "東京",
    "改行",
    "\t",
];

fn random_text(g: &mut Lcg) -> String {
    let n = 1 + g.next(40);
    let mut s = String::new();
    for _ in 0..n {
        let t = TOKENS[g.next(TOKENS.len())];
        if !s.is_empty() && !t.starts_with([',', '.', '?']) {
            s.push(' ');
        }
        s.push_str(t);
    }
    s
}

fn newlines(s: &str) -> usize {
    s.matches('\n').count()
}

#[test]
fn property_no_stage_removes_a_newline() {
    let rules = RuleSet::new(vec![
        rule(1, "wispr bro", "Wispr Bro"),
        rule(2, "jonn", "Jon\nX"),
    ]);
    let matcher = commands::Matcher::for_language(None);
    // Handy's real stages compile their filler regexes on every call, so
    // they run on every 8th text; the others use the same flattening
    // behaviour (split on whitespace, join with one space) at no cost.
    let flatten = |t: String| t.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut g = Lcg(0x5EED);
    for i in 0..2_000 {
        let raw = random_text(&mut g);
        let mut stage: Box<dyn FnMut(String) -> String> = if i % 8 == 0 {
            Box::new(handy(&["John"]))
        } else {
            Box::new(flatten)
        };
        // Each stage on its own.
        let (ruled, _) = rules::apply(&rules, &raw, |r| r.to.clone());
        assert!(
            newlines(&ruled) >= newlines(&raw),
            "rules: {raw:?} → {ruled:?}"
        );
        let handled = lines::per_line(&ruled, &mut stage);
        assert!(
            newlines(&handled) >= newlines(&ruled),
            "handy: {ruled:?} → {handled:?}"
        );
        let (commanded, _) = commands::apply(&matcher, &handled);
        assert!(
            newlines(&commanded) >= newlines(&handled),
            "commands: {handled:?} → {commanded:?}"
        );
        let (spaced, _) = whitespace::normalize(&commanded);
        assert_eq!(
            newlines(&spaced),
            newlines(&commanded),
            "whitespace: {commanded:?}"
        );
        // The whole pipeline (sanitize trims only the ends of the raw text).
        let (out, _) = run(&raw, None, &rules, &mut stage);
        let inner = newlines(raw.trim());
        assert!(newlines(&out) >= inner, "pipeline: {raw:?} → {out:?}");
        assert!(
            !placeholders::collides(&out),
            "placeholder leaked: {raw:?} → {out:?}"
        );
    }
}

#[test]
fn a_panicking_stage_fails_open_to_handy() {
    let mut calls = 0;
    let flaky = |t: String| {
        calls += 1;
        if calls == 1 {
            panic!("first call fails");
        }
        t.to_uppercase()
    };
    let out = process(
        "unique panic text 7f3a".into(),
        &OutputLanguageEvidence::UserSelected("en".into()),
        flaky,
    );
    assert_eq!(out, "UNIQUE PANIC TEXT 7F3A");
    let p = ledger().take_processed("UNIQUE PANIC TEXT 7F3A").unwrap();
    assert_eq!(p.verbatim, "unique panic text 7f3a");
    assert!(p.record.stages.is_empty());
}

#[test]
fn process_notes_the_record_for_the_history_row() {
    let out = process(
        "  Note e1b2. New line. Done  ".into(),
        &OutputLanguageEvidence::UserSelected("en".into()),
        handy(&[]),
    );
    assert_eq!(out, "Note e1b2.\nDone");
    let p = ledger().take_processed(&out).unwrap();
    assert_eq!(p.verbatim, "  Note e1b2. New line. Done  ");
    assert_eq!(
        p.record.stages.iter().map(|s| s.stage).collect::<Vec<_>>(),
        [
            Stage::Rules,
            Stage::Handy,
            Stage::Commands,
            Stage::Scratch,
            Stage::Whitespace
        ]
    );
}

#[test]
fn finish_restores_rules_after_an_llm_rewrite_and_rekeys_the_outcome() {
    let rules = RuleSet::new(vec![rule(1, "voice typer", "Voicetypr")]);
    let (kept, record) = en("voice typer is fast c4d1", &rules, &[]);
    assert_eq!(kept, "Voicetypr is fast c4d1");
    ledger().note_processed("voice typer is fast c4d1".into(), &kept, record);
    // The LLM reverted the spelling and left two spaces.
    let llm = "Well,  voice typer is fast c4d1.";
    note_rewrite(&kept, llm);
    ledger().await_outcome(991_001, llm);
    let out = finish(llm.to_string());
    assert_eq!(out, "Well, Voicetypr is fast c4d1.");
    assert_eq!(ledger().take_awaiting(&out), Some(991_001));
    // Without a registered rewrite only whitespace is touched.
    assert_eq!(finish("plain  text b7e0".into()), "plain text b7e0");
    let _ = ledger().take_processed(&kept);
}

#[test]
fn rules_come_from_the_installed_source_and_are_cached() {
    // Only this test installs a source; the others call `run` directly.
    install_rule_source(|| {
        vec![Rule {
            id: 1,
            from: "zq9".into(),
            to: "ZQ-9".into(),
        }]
    });
    let a = active_rules();
    let b = active_rules();
    assert!(Arc::ptr_eq(&a, &b));
    assert_eq!(a.rules().len(), 1);
}

#[test]
fn the_fork_stages_stay_far_inside_the_latency_budget() {
    // R5 §5.1: the whole deterministic path has ≤ 500 ms on M1. The fork's
    // stages on a 200-word take with 100 rules (Handy's stages excluded)
    // must cost a few milliseconds even in a debug build.
    let rules = RuleSet::new(
        (0..100)
            .map(|i| rule(i, &format!("term{i} alpha"), &format!("Term{i}")))
            .collect(),
    );
    let sentence = "so I talked to term7 alpha about the plan, new line, and we agreed. ";
    let raw = sentence.repeat(200 / sentence.split_whitespace().count() + 1);
    let started = std::time::Instant::now();
    let rounds = 20;
    for _ in 0..rounds {
        let _ = run(&raw, Some("en"), &rules, &mut |t| t);
    }
    let per_take = started.elapsed() / rounds;
    eprintln!("fork text stages: {per_take:?} per 200-word take with 100 rules");
    assert!(
        per_take < std::time::Duration::from_millis(50),
        "{per_take:?}"
    );
}
