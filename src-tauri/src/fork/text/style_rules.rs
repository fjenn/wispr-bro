// SPDX-License-Identifier: GPL-3.0-or-later
//
// The preset rules are a port of BetterWispr's `StyleFormatter`:
//   Source: BetterWispr, https://github.com/opennookorg/betterwispr
//   Commit: 304335a073a98f7439e5f7b4fbc2c31b810b807e
//   File:   Sources/BetterWisprCore/Transcription/StyleFormatter.swift
//           (tests: Tests/BetterWisprCoreTests/StyleTests.swift)
//   Copyright 2026 Kartik Labhshetwar. Licence: Apache-2.0
//   (LICENSES/Apache-2.0-BetterWispr.txt; this file is not one of the
//   VoiceInk-derived files listed in BetterWispr's docs/oss-reuse.md).
//   Modified by Wispr Bro contributors, 2026-10-31: ported from Swift to
//   Rust; the greeting join of the casual tone is not ported (it removed the
//   line breaks after a greeting, and no stage may remove a `\n` the user
//   dictated, R5 §4.10); sentence starts that are dictionary words keep
//   their spelling; the English gate is our own.
//
//! Stage 10 of the text pipeline (R5 §4.10, S-19a): the deterministic part
//! of a style preset, after the LLM and stage 9, before the snippets.
//!
//! | Preset       | Rule (BetterWispr's tone)                                   |
//! | ------------ | ----------------------------------------------------------- |
//! | Formal       | the text as cleaned (`formal`)                              |
//! | Relaxed      | commas and the final period dropped (`casual`)              |
//! | Very relaxed | Relaxed, and sentence starts in lower case (`veryCasual`)   |
//! | Upbeat       | the final period becomes "!" (`excited`)                    |
//!
//! Numbers keep their commas ("1,200"), a comma that ends a line stays (a
//! greeting), ellipses, abbreviations ("p.m.") and decimals ("2.0") are never
//! a final period, and a question keeps its question mark. Words with a
//! capital inside ("iPhone", "NASA") and the user's dictionary words keep
//! their case. No rule adds or removes a word or a line break.
//!
//! Styles apply to English only (Wispr's note; R2 D-26): [`is_english`].

use crate::fork::settings::StylePreset;

/// Applies `preset` to `text`. `keep` are spellings that never change case
/// (the user's dictionary words).
pub fn apply(preset: StylePreset, text: &str, keep: &[String]) -> String {
    match preset {
        StylePreset::Formal => text.to_owned(),
        StylePreset::Relaxed => casual(text),
        StylePreset::VeryRelaxed => lowercasing_sentence_starts(&casual(text), keep),
        StylePreset::Upbeat => excited(text),
    }
}

/// BetterWispr `casual`: commas go, except inside a number or at the end of
/// a line; a final period that ends the text goes. Ours: a comma inside
/// brackets or a backtick code span stays (code is never restyled:
/// `f(a, b)`, `[1, 2]`; the S-12a eval's `code-009`).
fn casual(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut kept = String::with_capacity(text.len());
    let mut depth = 0usize;
    let mut in_code = false;
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '`' => in_code = !in_code,
            '(' | '[' | '{' if !in_code => depth += 1,
            ')' | ']' | '}' if !in_code => depth = depth.saturating_sub(1),
            '\n' => {
                // A bracket or code span never runs past the line.
                depth = 0;
                in_code = false;
            }
            _ => {}
        }
        let next = chars.get(i + 1).copied();
        let inside_number =
            i > 0 && chars[i - 1].is_numeric() && next.is_some_and(char::is_numeric);
        let ends_line = next.is_some_and(|n| n == '\n' || n == '\r');
        let protected = depth > 0 || in_code;
        if c == ',' && !inside_number && !ends_line && !protected {
            continue;
        }
        kept.push(c);
    }
    if let Some(at) = final_period(&kept) {
        if at + 1 == kept.len() {
            kept.remove(at);
        }
    }
    kept
}

/// BetterWispr `excited`: the final period becomes an exclamation mark.
fn excited(text: &str) -> String {
    match final_period(text) {
        Some(at) => {
            let mut out = String::with_capacity(text.len());
            out.push_str(&text[..at]);
            out.push('!');
            out.push_str(&text[at + 1..]);
            out
        }
        None => text.to_owned(),
    }
}

/// Byte index of the period that ends the last sentence, skipping
/// ellipses, abbreviations like "p.m." and decimals (BetterWispr
/// `finalPeriod`).
fn final_period(text: &str) -> Option<usize> {
    let (end, c) = text
        .char_indices()
        .rev()
        .find(|(_, c)| matches!(c, '.' | '!' | '?'))?;
    if c != '.' {
        return None;
    }
    let after = text[end + 1..].chars().next();
    if after.is_some_and(|a| !a.is_whitespace()) {
        return None;
    }
    // The word in front of the period, back to the last whitespace.
    let word: String = text[..end]
        .chars()
        .rev()
        .take_while(|c| !c.is_whitespace())
        .collect();
    let first = word.chars().next()?;
    if first.is_alphabetic() || first.is_numeric() {
        (!word.contains('.')).then_some(end)
    } else {
        None
    }
}

/// BetterWispr `lowercasingSentenceStarts`: the first word of every
/// sentence (and line) in lower case, unless it has a capital after its
/// first letter ("iPhone", "NASA") or is one of `keep`.
fn lowercasing_sentence_starts(text: &str, keep: &[String]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at_start = true;
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if at_start && c.is_alphabetic() {
            let len: usize = rest
                .chars()
                .take_while(|&ch| ch.is_alphabetic() || ch == '\'' || ch == '’')
                .map(char::len_utf8)
                .sum();
            let word = &rest[..len];
            let inner_capital = word.chars().skip(1).any(char::is_uppercase);
            if inner_capital || is_kept(rest, keep) {
                out.push_str(word);
            } else {
                out.push_str(&word.to_lowercase());
            }
            at_start = false;
            rest = &rest[len..];
            continue;
        }
        if c.is_alphanumeric() {
            at_start = false;
        }
        if matches!(c, '.' | '!' | '?' | '\n') {
            at_start = true;
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// `rest` starts with one of `keep`, exactly, ending at a word boundary.
fn is_kept(rest: &str, keep: &[String]) -> bool {
    keep.iter().any(|k| {
        !k.is_empty()
            && rest.starts_with(k.as_str())
            && rest[k.len()..]
                .chars()
                .next()
                .is_none_or(|c| !c.is_alphanumeric())
    })
}

/// Whether a take is English for the style gate. The recognizer's language
/// wins, then the guess from the user's spoken languages; without either,
/// Handy's detector decides, and a text too short to tell counts as English
/// unless the user speaks only other languages.
pub fn is_english(
    language: Option<&str>,
    hint: Option<&str>,
    text: &str,
    spoken: &[String],
) -> bool {
    fn en(code: &str) -> bool {
        code.split(['-', '_'])
            .next()
            .unwrap_or(code)
            .trim()
            .eq_ignore_ascii_case("en")
    }
    if let Some(l) = language.filter(|l| !l.trim().is_empty()) {
        return en(l);
    }
    if let Some(h) = hint.filter(|h| !h.trim().is_empty()) {
        return en(h);
    }
    match crate::audio_toolkit::detect_output_language(text, &[]) {
        Some(code) => en(&code),
        None => spoken.is_empty() || spoken.iter().any(|s| en(s)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use StylePreset::*;

    fn s(p: StylePreset, t: &str) -> String {
        apply(p, t, &[])
    }

    // BetterWispr StyleTests `personalTonesMatchTheirSamples`.
    #[test]
    fn personal_presets() {
        let spoken = "Hey, are you free for lunch tomorrow? Let's do 12 if that works for you.";
        assert_eq!(s(Formal, spoken), spoken);
        assert_eq!(
            s(Relaxed, spoken),
            "Hey are you free for lunch tomorrow? Let's do 12 if that works for you"
        );
        assert_eq!(
            s(VeryRelaxed, spoken),
            "hey are you free for lunch tomorrow? let's do 12 if that works for you"
        );
    }

    // `workAndEmailTonesMatchTheirSamples`, without the greeting join.
    #[test]
    fn work_email_and_other_presets() {
        let work = "Hey, if you're free, let's chat about the great results.";
        assert_eq!(
            s(Relaxed, work),
            "Hey if you're free let's chat about the great results"
        );
        assert_eq!(
            s(Upbeat, work),
            "Hey, if you're free, let's chat about the great results!"
        );
        let email = "Hi Alex,\n\nIt was great talking with you today. Looking forward to our next chat.\n\nBest,\nMary";
        assert_eq!(s(Formal, email), email);
        // The greeting and sign-off keep their lines (no `\n` is removed).
        assert_eq!(
            s(Relaxed, email),
            "Hi Alex,\n\nIt was great talking with you today. Looking forward to our next chat.\n\nBest,\nMary"
        );
        assert_eq!(
            s(Upbeat, email),
            "Hi Alex,\n\nIt was great talking with you today. Looking forward to our next chat!\n\nBest,\nMary"
        );
        let other = "So far, I am enjoying the new workout routine.\n\nI am excited for tomorrow's workout, especially after a full night of rest.";
        assert_eq!(
            s(Relaxed, other),
            "So far I am enjoying the new workout routine.\n\nI am excited for tomorrow's workout especially after a full night of rest"
        );
        assert!(s(Upbeat, other).ends_with("full night of rest!"));
    }

    // `tonesKeepNumbersNamesAndEllipses`.
    #[test]
    fn numbers_names_and_ellipses() {
        assert_eq!(
            s(Relaxed, "It costs 1,200 dollars, sadly."),
            "It costs 1,200 dollars sadly"
        );
        assert_eq!(
            s(
                VeryRelaxed,
                "I met iPhone fans. NASA called. Kartik said hi"
            ),
            "i met iPhone fans. NASA called. kartik said hi"
        );
        assert_eq!(s(Upbeat, "Wait for it..."), "Wait for it...");
        assert_eq!(s(Upbeat, "Is it done?"), "Is it done?");
        assert_eq!(s(Relaxed, "See you at 5 p.m."), "See you at 5 p.m.");
        assert_eq!(s(Upbeat, "Version 2.0"), "Version 2.0");
        assert_eq!(s(Upbeat, "Version 2.0."), "Version 2.0.");
        assert_eq!(
            s(Relaxed, "Hi team,\n\nI shipped it."),
            "Hi team,\n\nI shipped it"
        );
        assert_eq!(s(Upbeat, ""), "");
        assert_eq!(s(Relaxed, "."), ".");
        assert_eq!(s(Upbeat, "Done. "), "Done! ");
    }

    #[test]
    fn code_keeps_its_commas() {
        assert_eq!(
            s(
                Relaxed,
                "Use items.reduce((a, b) => a + b, 0), then log it, please."
            ),
            "Use items.reduce((a, b) => a + b, 0) then log it please"
        );
        assert_eq!(s(Relaxed, "Run `cp a, b` now, ok."), "Run `cp a, b` now ok");
        assert_eq!(s(Relaxed, "A list [1, 2], done."), "A list [1, 2] done");
        // An unclosed bracket protects only its own line.
        assert_eq!(s(Relaxed, "(a, b\nc, d."), "(a, b\nc d");
    }

    #[test]
    fn dictionary_words_keep_their_case() {
        let keep = vec!["Wispr Bro".to_string(), "Siobhán".to_string()];
        assert_eq!(
            apply(VeryRelaxed, "Wispr Bro is great. Siobhán agrees.", &keep),
            "Wispr Bro is great. Siobhán agrees"
        );
        // Only a whole match counts.
        assert_eq!(
            apply(VeryRelaxed, "Wispr is a word.", &keep),
            "wispr is a word"
        );
    }

    #[test]
    fn no_rule_adds_or_removes_a_word_or_a_line() {
        let samples = [
            "Hi Sam,\n\nThanks, that works.\nSee you then.",
            "One, two, three.\n\n\nFour!",
            "Ünïcödé, ok. Ça va?",
        ];
        for t in samples {
            for p in [Formal, Relaxed, VeryRelaxed, Upbeat] {
                let out = s(p, t);
                assert_eq!(
                    out.matches('\n').count(),
                    t.matches('\n').count(),
                    "{p:?} {t:?}"
                );
                let words = |x: &str| {
                    x.split_whitespace()
                        .map(|w| {
                            w.chars()
                                .filter(|c| c.is_alphanumeric())
                                .collect::<String>()
                                .to_lowercase()
                        })
                        .collect::<Vec<_>>()
                };
                assert_eq!(words(&out), words(t), "{p:?} {t:?}");
            }
        }
    }

    /// The Style page's before/after cards (`src/fork/style/presets.ts`)
    /// show what these rules really insert: every preset's text is the
    /// rules applied to the group's Formal text.
    #[test]
    fn page_examples_match_the_rules() {
        let ts = include_str!("../../../../src/fork/style/presets.ts");
        let examples = ts.split("export const EXAMPLES").nth(1).expect("EXAMPLES");
        let entry = regex::Regex::new(r#"(\w+):\s*"([^"]*)""#).unwrap();
        let mut checked = 0;
        for group in ["personal", "work", "email", "other"] {
            let block = examples
                .split(&format!("  {group}: {{"))
                .nth(1)
                .and_then(|b| b.split("\n  },").next())
                .unwrap_or_else(|| panic!("no example for {group}"));
            let texts: std::collections::HashMap<String, String> = entry
                .captures_iter(block)
                .map(|c| (c[1].to_string(), c[2].to_string()))
                .collect();
            let formal = &texts["formal"];
            for (key, preset) in [
                ("relaxed", Relaxed),
                ("very_relaxed", VeryRelaxed),
                ("upbeat", Upbeat),
            ] {
                if let Some(expected) = texts.get(key) {
                    assert_eq!(&s(preset, formal), expected, "{group} {key}");
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 8);
    }

    #[test]
    fn english_gate() {
        let none: Vec<String> = vec![];
        let en = vec!["en".to_string()];
        let nl = vec!["nl".to_string()];
        // The recognizer's language wins.
        assert!(is_english(
            Some("en-US"),
            None,
            "Bonjour tout le monde",
            &none
        ));
        assert!(!is_english(Some("fr"), None, "Hello there", &en));
        // Then the guess.
        assert!(is_english(None, Some("en"), "x", &nl));
        assert!(!is_english(None, Some("nl"), "Hello there my friend", &en));
        // Then the detector.
        assert!(!is_english(
            None,
            None,
            "Bonjour à tous, je voudrais réserver une table pour ce soir.",
            &none
        ));
        assert!(is_english(
            None,
            None,
            "Thanks a lot for the update, I will look at the numbers tomorrow morning.",
            &none
        ));
        // Too short to tell: English unless the user speaks only others.
        assert!(is_english(None, None, "ok", &none));
        assert!(is_english(None, None, "ok", &en));
        assert!(!is_english(None, None, "ok", &nl));
    }
}
