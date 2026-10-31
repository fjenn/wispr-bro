// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) the Voicetypr authors (https://github.com/ideaplexa/voicetypr)
// Modified by Wispr Bro contributors, 2026-10-30.
//
// Ported from Voicetypr `src-tauri/src/writing/library_rules.rs`
// (https://github.com/ideaplexa/voicetypr, commit
// f86e571df5bd5c0a4d8950095167b45185fa59d4), licensed AGPL-3.0. Kept: the
// word-boundary rule (`candidate_has_boundaries`), protected URL and e-mail
// tokens (`span_in_protected_token`), candidate selection (leftmost, then
// longest, no overlaps), per-application provenance, the final restoration
// guard with its shifted-offset fallback, `sanitize_transcript`, and the
// tests for those parts. Changed: one `aho-corasick` automaton over Unicode
// case-folded rules instead of a regex per rule (Wispr's dictionary rule:
// whole word, case-insensitive, literal, longest wins, at most 60
// characters); the replacement is chosen by the caller (the pipeline protects
// it with a placeholder); no snippets, custom-word spoken forms or language
// scopes (Wispr Bro keeps snippets in S-17 and has no per-rule language);
// boundaries are not required next to scripts without spaces. See
// docs/fork/PORTS.md.

//! Stage 3 and stage 9 of the text pipeline (R5 §4.10): exact dictionary
//! rules "from → to" (S-16 manages them in `dictionary_entries`), and their
//! re-application after the LLM so cleanup cannot undo a spelling.

use std::borrow::Cow;

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};

use super::fold::{fold_str, whole_words, Folded};

/// Longest "from" a rule may have, in characters (Wispr's dictionary limit).
pub const MAX_FROM_CHARS: usize = 60;

/// One active rule (`dictionary_entries` row of kind `rule`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub id: i64,
    pub from: String,
    pub to: String,
}

impl Rule {
    /// Whether the rule can apply: a non-blank "from" of at most
    /// [`MAX_FROM_CHARS`] characters that differs from its "to".
    pub fn is_valid(&self) -> bool {
        let from = self.from.trim();
        !from.is_empty() && from.chars().count() <= MAX_FROM_CHARS && self.from != self.to
    }
}

/// The active rules, compiled once.
#[derive(Debug)]
pub struct RuleSet {
    rules: Vec<Rule>,
    matcher: Option<AhoCorasick>,
}

impl RuleSet {
    pub fn empty() -> RuleSet {
        RuleSet {
            rules: Vec::new(),
            matcher: None,
        }
    }

    /// Compiles `rules`; invalid ones are dropped. Of two rules with the
    /// same "from", the older one (lower id) wins.
    pub fn new(rules: Vec<Rule>) -> RuleSet {
        let mut rules: Vec<Rule> = rules
            .into_iter()
            .filter(Rule::is_valid)
            .map(|r| Rule {
                from: r.from.trim().to_string(),
                ..r
            })
            .collect();
        rules.sort_by_key(|r| r.id);
        let patterns: Vec<String> = rules.iter().map(|r| fold_str(&r.from)).collect();
        let matcher = if patterns.is_empty() {
            None
        } else {
            match AhoCorasickBuilder::new()
                .match_kind(MatchKind::Standard)
                .build(&patterns)
            {
                Ok(m) => Some(m),
                Err(e) => {
                    log::error!(
                        "text: could not compile {} dictionary rules: {e}",
                        patterns.len()
                    );
                    None
                }
            }
        };
        RuleSet { rules, matcher }
    }

    pub fn is_empty(&self) -> bool {
        self.matcher.is_none()
    }

    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }
}

/// One rule applied to the text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Applied {
    pub rule_id: i64,
    /// The text that was replaced, as it was written.
    pub matched: String,
    /// The rule's "from" and "to".
    pub source_form: String,
    pub target_text: String,
    /// Byte span of `matched` in the input.
    pub start: usize,
    pub end: usize,
    /// Byte span of what the caller inserted, in the output.
    pub target_start: usize,
    pub target_end: usize,
}

/// A rule application to restore after the LLM (stage 9).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Provenance {
    pub source_form: String,
    pub target_text: String,
    /// Where `target_text` was in the text handed to the LLM.
    pub target_start: usize,
}

#[derive(Clone, Copy, Debug)]
struct Candidate {
    start: usize,
    end: usize,
    rule: usize,
}

/// Voicetypr `span_in_protected_token`: inside a URL or an e-mail address.
fn span_in_protected_token(text: &str, start: usize, end: usize) -> bool {
    let token_start = text[..start]
        .char_indices()
        .rev()
        .find_map(|(index, ch)| ch.is_whitespace().then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    let token_end = text[end..]
        .char_indices()
        .find_map(|(index, ch)| ch.is_whitespace().then_some(end + index))
        .unwrap_or(text.len());
    let token = &text[token_start..token_end];

    token.contains("://")
        || token
            .match_indices('@')
            .any(|(index, _)| index > 0 && index + 1 < token.len())
}

fn candidates(set: &RuleSet, text: &str) -> Vec<Candidate> {
    let Some(matcher) = &set.matcher else {
        return Vec::new();
    };
    let folded = Folded::new(text);
    let mut out: Vec<Candidate> = matcher
        .find_overlapping_iter(&folded.text)
        .filter_map(|m| {
            let (start, end) = folded.original_span(m.start(), m.end())?;
            (start < end
                && whole_words(text, start, end)
                && !span_in_protected_token(text, start, end))
            .then_some(Candidate {
                start,
                end,
                rule: m.pattern().as_usize(),
            })
        })
        .collect();
    // Leftmost first, then the longest, then the older rule.
    out.sort_by(|a, b| {
        a.start
            .cmp(&b.start)
            .then((b.end - b.start).cmp(&(a.end - a.start)))
            .then(a.rule.cmp(&b.rule))
    });
    out
}

/// Applies the rules once, left to right, never overlapping and never
/// rescanning what was inserted. `insert` gives the text that replaces each
/// match (the rule's "to", or a placeholder protecting it).
pub fn apply(
    set: &RuleSet,
    text: &str,
    mut insert: impl FnMut(&Rule) -> String,
) -> (String, Vec<Applied>) {
    let mut selected = Vec::new();
    let mut cursor = 0usize;
    for c in candidates(set, text) {
        if c.start < cursor {
            continue;
        }
        cursor = c.end;
        selected.push(c);
    }
    if selected.is_empty() {
        return (text.to_string(), Vec::new());
    }

    let mut output = String::with_capacity(text.len());
    let mut applied = Vec::with_capacity(selected.len());
    let mut last = 0usize;
    for c in selected {
        let rule = &set.rules[c.rule];
        output.push_str(&text[last..c.start]);
        let target_start = output.len();
        output.push_str(&insert(rule));
        applied.push(Applied {
            rule_id: rule.id,
            matched: text[c.start..c.end].to_string(),
            source_form: rule.from.clone(),
            target_text: rule.to.clone(),
            start: c.start,
            end: c.end,
            target_start,
            target_end: output.len(),
        });
        last = c.end;
    }
    output.push_str(&text[last..]);
    (output, applied)
}

#[derive(Clone)]
struct GuardCandidate {
    start: usize,
    end: usize,
    replacement: String,
}

fn guard_candidates(text: &str, provenance: &[Provenance]) -> Vec<GuardCandidate> {
    let folded = Folded::new(text);
    let mut candidates = Vec::new();
    for application in provenance {
        if application.source_form == application.target_text {
            continue;
        }
        let needle = fold_str(&application.source_form);
        if needle.is_empty() {
            continue;
        }

        // Primary: the exact recorded offset (the LLM left the term in
        // place). Fallback: the nearest boundary-valid, unprotected
        // occurrence; every boundary-valid source form was already consumed
        // by the rule stage, so one in the text was put back by the LLM. The
        // fallback only applies to a plausible in-text offset.
        let mut exact: Option<(usize, usize)> = None;
        let mut shifted: Option<(usize, usize)> = None;
        for (at, _) in folded.text.match_indices(needle.as_str()) {
            let Some((start, end)) = folded.original_span(at, at + needle.len()) else {
                continue;
            };
            if !whole_words(text, start, end) || span_in_protected_token(text, start, end) {
                continue;
            }
            if start == application.target_start {
                exact = Some((start, end));
            } else if application.target_start < text.len()
                && shifted.is_none_or(|(s, _)| {
                    start.abs_diff(application.target_start) < s.abs_diff(application.target_start)
                })
            {
                shifted = Some((start, end));
            }
        }

        if let Some((start, end)) = exact.or(shifted) {
            candidates.push(GuardCandidate {
                start,
                end,
                replacement: application.target_text.clone(),
            });
        }
    }
    candidates.sort_by(|left, right| {
        left.start
            .cmp(&right.start)
            .then((right.end - right.start).cmp(&(left.end - left.start)))
    });
    candidates
}

/// Stage 9: puts back every rule output the LLM reverted to the rule's
/// "from". Returns the text and how many were restored.
pub fn restore(text: &str, provenance: &[Provenance]) -> (String, u32) {
    if provenance.is_empty() {
        return (text.to_string(), 0);
    }
    let mut selected = Vec::new();
    let mut cursor = 0usize;
    for candidate in guard_candidates(text, provenance) {
        if candidate.start < cursor {
            continue;
        }
        cursor = candidate.end;
        selected.push(candidate);
    }
    if selected.is_empty() {
        return (text.to_string(), 0);
    }

    let mut output = String::with_capacity(text.len());
    let mut last = 0usize;
    let restored = u32::try_from(selected.len()).unwrap_or(u32::MAX);
    for candidate in selected {
        output.push_str(&text[last..candidate.start]);
        output.push_str(&candidate.replacement);
        last = candidate.end;
    }
    output.push_str(&text[last..]);
    (output, restored)
}

/// Voicetypr `sanitize_transcript`: mechanical clean-up only. Trims the
/// ends, turns `\r\n` and `\r` into `\n`, drops control characters other
/// than `\n` and `\t`.
pub fn sanitize(text: &str) -> Cow<'_, str> {
    let trimmed = text.trim();
    let needs_cleanup = trimmed.len() != text.len()
        || trimmed
            .chars()
            .any(|ch| ch == '\r' || (ch.is_control() && ch != '\n' && ch != '\t'));

    if !needs_cleanup {
        return Cow::Borrowed(text);
    }

    let mut output = String::with_capacity(trimmed.len());
    let mut chars = trimmed.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            output.push('\n');
        } else if !ch.is_control() || ch == '\n' || ch == '\t' {
            output.push(ch);
        }
    }

    Cow::Owned(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(id: i64, from: &str, to: &str) -> Rule {
        Rule {
            id,
            from: from.into(),
            to: to.into(),
        }
    }

    fn literal(set: &RuleSet, text: &str) -> (String, Vec<Applied>) {
        apply(set, text, |r| r.to.clone())
    }

    // Voicetypr tests, kept.

    #[test]
    fn test_replacements_require_boundaries() {
        let set = RuleSet::new(vec![rule(1, "react", "React")]);
        let (identifier_text, identifier_ops) = literal(&set, "createReactiveStore");
        assert_eq!(identifier_text, "createReactiveStore");
        assert!(identifier_ops.is_empty());
    }

    #[test]
    fn test_replacements_protect_word_tokens_urls_and_emails() {
        let set = RuleSet::new(vec![rule(1, "foo", "bar")]);
        let (protected_text, protected_ops) =
            literal(&set, "foo_bar https://example.com/foo user@foo.com");
        assert_eq!(
            protected_text,
            "foo_bar https://example.com/foo user@foo.com"
        );
        assert!(protected_ops.is_empty());

        let (boundary_text, boundary_ops) = literal(&set, "foo foo, (foo) end foo");
        assert_eq!(boundary_text, "bar bar, (bar) end bar");
        assert_eq!(boundary_ops.len(), 4);
    }

    #[test]
    fn test_replacement_provenance_records_selected_rules() {
        let set = RuleSet::new(vec![
            rule(1, "voice typer", "Voicetypr"),
            rule(2, "react", "React"),
        ]);
        let (text, applied) = literal(&set, "voice typer uses react");
        assert_eq!(text, "Voicetypr uses React");
        assert_eq!(applied.len(), 2);
        assert_eq!(applied[0].source_form, "voice typer");
        assert_eq!(applied[0].target_text, "Voicetypr");
        assert_eq!((applied[0].start, applied[0].end), (0, 11));
        assert_eq!((applied[0].target_start, applied[0].target_end), (0, 9));
        assert_eq!(applied[1].source_form, "react");
        assert_eq!(applied[1].target_text, "React");
    }

    #[test]
    fn test_replacement_provenance_tracks_only_applied_rules() {
        let set = RuleSet::new(vec![
            rule(1, "voice typer", "Voicetypr"),
            rule(2, "hello", "Bonjour"),
        ]);
        let (text, applied) = literal(&set, "voice typer");
        assert_eq!(text, "Voicetypr");
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].rule_id, 1);
    }

    fn prov(source: &str, target: &str, target_start: usize) -> Provenance {
        Provenance {
            source_form: source.into(),
            target_text: target.into(),
            target_start,
        }
    }

    #[test]
    fn test_final_guard_restores_only_provenance_sources() {
        let set = RuleSet::new(vec![rule(1, "voice typer", "Voicetypr")]);
        let (_, applied) = literal(&set, "voice typer is fast");
        let provenance: Vec<Provenance> = applied
            .iter()
            .map(|a| prov(&a.source_form, &a.target_text, a.target_start))
            .collect();
        let (guarded, n) = restore("voice typer is fast", &provenance);
        assert_eq!(guarded, "Voicetypr is fast");
        assert_eq!(n, 1);

        let (unproven, n) = restore("voice typer is fast", &[]);
        assert_eq!(unproven, "voice typer is fast");
        assert_eq!(n, 0);
    }

    #[test]
    fn test_final_guard_respects_boundaries() {
        let provenance = vec![prov("react", "React", 20)];
        let (guarded, n) = restore("createReactiveStore react", &provenance);
        assert_eq!(guarded, "createReactiveStore React");
        assert_eq!(n, 1);

        let (shifted, n) = restore("please react", &provenance);
        assert_eq!(shifted, "please react");
        assert_eq!(n, 0);
    }

    #[test]
    fn test_final_guard_restores_after_position_shift() {
        let provenance = vec![prov("voice typer", "Voicetypr", 0)];
        let (guarded, n) = restore("well, voice typer", &provenance);
        assert_eq!(guarded, "well, Voicetypr");
        assert_eq!(n, 1);

        // An out-of-range recorded offset is left alone.
        let out_of_range = vec![prov("voice typer", "Voicetypr", 50)];
        let (untouched, n) = restore("well, voice typer", &out_of_range);
        assert_eq!(untouched, "well, voice typer");
        assert_eq!(n, 0);
    }

    #[test]
    fn test_final_guard_skips_protected_tokens() {
        let (url_text, n) = restore("https://example.com/foo", &[prov("foo", "bar", 20)]);
        assert_eq!(url_text, "https://example.com/foo");
        assert_eq!(n, 0);
        let (email_text, n) = restore("user@foo.com", &[prov("foo", "bar", 5)]);
        assert_eq!(email_text, "user@foo.com");
        assert_eq!(n, 0);
    }

    #[test]
    fn test_sanitize_transcript_is_mechanical_only() {
        let cleaned = sanitize(" \r\nhello\rworld\tthere\0\u{0008} ");
        assert_eq!(cleaned.as_ref(), "hello\nworld\tthere");

        let semantic_text = "um I mean send it to Bob no Alice period";
        let untouched = sanitize(semantic_text);
        assert!(matches!(untouched, Cow::Borrowed(_)));
        assert_eq!(untouched.as_ref(), semantic_text);
    }

    // Wispr's dictionary rule (R5 S-11 acceptance): whole word,
    // case-insensitive, literal, longest wins, at most 60 characters.

    #[test]
    fn whole_word_only() {
        let set = RuleSet::new(vec![rule(1, "cat", "Kat")]);
        assert_eq!(
            literal(&set, "cat concat cats cat.").0,
            "Kat concat cats Kat."
        );
    }

    #[test]
    fn case_insensitive_including_non_ascii() {
        let set = RuleSet::new(vec![rule(1, "müller", "Mueller"), rule(2, "jira", "JIRA")]);
        assert_eq!(literal(&set, "MÜLLER and Jira").0, "Mueller and JIRA");
        // Typographic apostrophes match a straight one.
        let set = RuleSet::new(vec![rule(1, "o'brien", "O'Brien")]);
        assert_eq!(literal(&set, "ask o’brien").0, "ask O'Brien");
    }

    #[test]
    fn literal_output_keeps_the_rule_case_and_never_rescans() {
        // The output is inserted exactly as written, whatever the spoken case.
        let set = RuleSet::new(vec![rule(1, "wispr bro", "Wispr Bro")]);
        assert_eq!(literal(&set, "WISPR BRO rocks").0, "Wispr Bro rocks");
        // An output that contains its own "from" is not replaced again.
        let set = RuleSet::new(vec![rule(1, "dr", "Dr."), rule(2, "dr.", "Doctor")]);
        assert_eq!(literal(&set, "dr smith").0, "Dr. smith");
        // Chained rules do not chain: a → b and b → c turn "a b" into "b c".
        let set = RuleSet::new(vec![rule(1, "a", "b"), rule(2, "b", "c")]);
        assert_eq!(literal(&set, "a b").0, "b c");
    }

    #[test]
    fn longest_wins_and_a_failed_long_match_leaves_the_short_one() {
        let set = RuleSet::new(vec![
            rule(1, "new york", "NY"),
            rule(2, "new york city", "NYC"),
        ]);
        assert_eq!(literal(&set, "new york city and new york").0, "NYC and NY");
        // "foo b" fails its boundary in "foo bar"; "foo" still applies.
        let set = RuleSet::new(vec![rule(1, "foo b", "X"), rule(2, "foo", "Y")]);
        assert_eq!(literal(&set, "foo bar").0, "Y bar");
    }

    #[test]
    fn rules_over_sixty_chars_blank_or_identity_are_ignored() {
        let long = "a".repeat(61);
        let ok = "b".repeat(60);
        let set = RuleSet::new(vec![
            rule(1, &long, "x"),
            rule(2, &ok, "y"),
            rule(3, "  ", "z"),
            rule(4, "same", "same"),
        ]);
        assert_eq!(set.rules().len(), 1);
        assert_eq!(
            literal(&set, &format!("{long} {ok} same")).0,
            format!("{long} y same")
        );
    }

    #[test]
    fn duplicate_from_keeps_the_older_rule() {
        let set = RuleSet::new(vec![rule(9, "gpt", "GPT-5"), rule(3, "gpt", "GPT")]);
        let (text, applied) = literal(&set, "gpt");
        assert_eq!(text, "GPT");
        assert_eq!(applied[0].rule_id, 3);
    }

    #[test]
    fn spaceless_scripts_match_inside_runs() {
        let set = RuleSet::new(vec![rule(1, "东京", "東京")]);
        assert_eq!(literal(&set, "我在东京工作").0, "我在東京工作");
    }

    #[test]
    fn newlines_are_kept_and_can_be_inserted() {
        let set = RuleSet::new(vec![rule(1, "my sig", "Best,\nFinn")]);
        assert_eq!(literal(&set, "thanks\nmy sig").0, "thanks\nBest,\nFinn");
    }

    #[test]
    fn empty_set_is_a_no_op() {
        let set = RuleSet::empty();
        assert!(set.is_empty());
        assert_eq!(literal(&set, "anything").0, "anything");
    }
}
