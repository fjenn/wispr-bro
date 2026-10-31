// SPDX-License-Identifier: MIT AND GPL-3.0-or-later
// Copyright (c) 2024 OpenWhispr Team (snippet matching)
// Modified by Wispr Bro contributors, 2026-10-31 (S-17).
//
// The matching rules are ported from OpenWhispr `src/utils/snippets.ts`
// (`buildMatcher`, `expandSnippets`; OpenWhispr/openwhispr on GitHub, commit
// fc786f41c91de8177ce7d9333d2834d973cc3c19, MIT): triggers matched
// case-insensitively in one pass over the finished text, longest first,
// never inside a word (Unicode-aware boundaries), NFC on both sides, the
// Turkish dotted and dotless i folded so "İmza" and "imza" meet, and the
// saved text inserted as written. Changed: one `aho-corasick` automaton over
// case-folded triggers instead of a regex alternation; Wispr Flow's
// documented rules on top (a trigger spoken alone ignores its punctuation,
// symbols and spacing; possessives expand, "n't" and other contractions do
// not; underscores and combining marks count as word characters; trigger
// ≤ 60 characters, text ≤ 4,000); no boundary next to scripts written
// without spaces; overlapping matches resolved by length over the whole
// text. MIT text: LICENSES/MIT-OpenWhispr.txt. See docs/fork/PORTS.md.

//! Stage 11 of the text pipeline (R5 §4.10, S-17): snippet expansion.
//!
//! A snippet is a spoken trigger ("my address") and the plain text it
//! stands for (DEC-12; line breaks kept). It runs last, inside
//! `fork::insert::deliver`, after the LLM step: the saved text never passes
//! through the LLM, and nothing after this stage rewrites it.
//!
//! Matching (Wispr Flow's snippet rules, R3 §2.5):
//!
//! - **Alone:** when the whole take is the trigger, its punctuation,
//!   symbols, spacing and case are ignored ("My e-mail sig." says
//!   "my email sig") and the take becomes the saved text, nothing else.
//! - **Inline:** inside a sentence the trigger must appear as written, but
//!   in any case, and must not touch a letter, digit, underscore or
//!   combining mark on either side. A possessive `'s` after it is kept
//!   ("my address's" → saved text + "'s"); any other contraction ("can't",
//!   "it'll") is not a match.
//! - The **longest** trigger wins where matches overlap (then the earlier
//!   one, then the older snippet); each part of the text is used once and
//!   the inserted text is never scanned again.
//! - The saved text keeps its own casing. A sentence mark right after an
//!   inline match is dropped when the saved text already ends with it.

use std::borrow::Cow;
use std::collections::HashMap;

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use icu_normalizer::properties::CanonicalCombiningClassMapBorrowed;
use icu_normalizer::ComposingNormalizerBorrowed;

use super::fold::{fold_char, is_spaceless, Folded};

/// Longest trigger, in characters (Wispr's limit).
pub const MAX_TRIGGER_CHARS: usize = 60;

/// Longest saved text, in characters (Wispr's desktop limit).
pub const MAX_BODY_CHARS: usize = 4_000;

/// One snippet as the matcher needs it (`snippets` row).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snippet {
    pub id: i64,
    pub trigger: String,
    pub body: String,
}

/// One expansion in a take.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnippetHit {
    pub snippet_id: i64,
    /// The words as they were in the text.
    pub matched: String,
    /// Whether the take was the trigger alone.
    pub alone: bool,
}

/// `s` in Unicode NFC.
pub fn nfc(s: &str) -> Cow<'_, str> {
    ComposingNormalizerBorrowed::new_nfc().normalize(s)
}

/// The fold snippets match on: the pipeline's fold (Unicode lowercase,
/// typographic apostrophes as `'`), plus Turkish `İ` and `ı` read as `i`
/// (OpenWhispr's `foldCapitalIDot`), so a trigger saved in either script
/// matches the other.
pub fn snippet_fold_char(c: char, out: &mut String) {
    match c {
        '\u{0130}' | '\u{0131}' => out.push('i'),
        c => fold_char(c, out),
    }
}

/// [`snippet_fold_char`] over a whole string.
pub fn snippet_fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        snippet_fold_char(c, &mut out);
    }
    out
}

/// A combining mark (canonical combining class above 0): part of the
/// letter before it, so a trigger cannot end in front of one.
fn is_mark(c: char) -> bool {
    !c.is_ascii() && CanonicalCombiningClassMapBorrowed::new().get_u8(c) != 0
}

/// A character a trigger must not touch: letter, digit, `_` or combining
/// mark (Wispr: "letters, digits or underscores").
pub fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || is_mark(c)
}

fn is_apostrophe(c: char) -> bool {
    matches!(c, '\'' | '\u{2019}' | '\u{02BC}')
}

/// The key of a trigger spoken alone: folded, with everything that is not
/// a letter, digit or mark removed (punctuation, symbols and spacing are
/// ignored).
pub fn alone_key(s: &str) -> String {
    snippet_fold(&nfc(s))
        .chars()
        .filter(|&c| c.is_alphanumeric() || is_mark(c))
        .collect()
}

/// The snippets, compiled once.
#[derive(Debug)]
pub struct SnippetSet {
    snippets: Vec<Snippet>,
    /// Trigger length in characters, per snippet.
    lengths: Vec<usize>,
    matcher: Option<AhoCorasick>,
    /// [`alone_key`] → snippet index (the older snippet wins a shared key).
    alone: HashMap<String, usize>,
}

impl SnippetSet {
    pub fn empty() -> SnippetSet {
        SnippetSet {
            snippets: Vec::new(),
            lengths: Vec::new(),
            matcher: None,
            alone: HashMap::new(),
        }
    }

    /// Compiles `snippets`. A trigger that is blank, too long, has no
    /// letter or digit, or a body that is empty or too long is dropped.
    pub fn new(snippets: Vec<Snippet>) -> SnippetSet {
        let mut snippets: Vec<Snippet> = snippets
            .into_iter()
            .filter_map(|s| {
                let trigger = nfc(&s.trigger)
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                let chars = trigger.chars().count();
                let usable = chars > 0
                    && chars <= MAX_TRIGGER_CHARS
                    && trigger.chars().any(char::is_alphanumeric)
                    && !s.body.is_empty()
                    && s.body.chars().count() <= MAX_BODY_CHARS;
                usable.then_some(Snippet { trigger, ..s })
            })
            .collect();
        snippets.sort_by_key(|s| s.id);
        let lengths = snippets.iter().map(|s| s.trigger.chars().count()).collect();
        let patterns: Vec<String> = snippets.iter().map(|s| snippet_fold(&s.trigger)).collect();
        let matcher = if patterns.is_empty() {
            None
        } else {
            match AhoCorasickBuilder::new()
                .match_kind(MatchKind::Standard)
                .build(&patterns)
            {
                Ok(m) => Some(m),
                Err(e) => {
                    log::error!("text: could not compile {} snippets: {e}", patterns.len());
                    None
                }
            }
        };
        let mut alone = HashMap::new();
        for (i, s) in snippets.iter().enumerate() {
            alone.entry(alone_key(&s.trigger)).or_insert(i);
        }
        alone.remove("");
        SnippetSet {
            snippets,
            lengths,
            matcher,
            alone,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.snippets.is_empty()
    }

    pub fn snippets(&self) -> &[Snippet] {
        &self.snippets
    }
}

#[derive(Clone, Copy, Debug)]
struct Candidate {
    start: usize,
    end: usize,
    snippet: usize,
}

/// Whether the left edge at `start` is a boundary.
fn left_ok(text: &str, start: usize) -> bool {
    let mut before = text[..start].chars().rev();
    let Some(outer) = before.next() else {
        return true;
    };
    let inner = text[start..].chars().next();
    if is_spaceless(outer) || inner.is_some_and(is_spaceless) {
        return true;
    }
    if is_word_char(outer) {
        return false;
    }
    // "o'brien": an apostrophe between two letters is inside a word.
    !(is_apostrophe(outer) && before.next().is_some_and(is_word_char))
}

/// Whether the right edge at `end` is a boundary.
fn right_ok(text: &str, end: usize) -> bool {
    let mut after = text[end..].chars();
    let Some(outer) = after.next() else {
        return true;
    };
    let inner = text[..end].chars().next_back();
    if is_spaceless(outer) || inner.is_some_and(is_spaceless) {
        return true;
    }
    if is_word_char(outer) {
        return false;
    }
    if !is_apostrophe(outer) {
        return true;
    }
    // A possessive expands ("my address's"); "n't", "'ll", "'re", "'d" …
    // do not: the apostrophe is then inside a word.
    match after.next() {
        Some(c) if is_word_char(c) => {
            matches!(c, 's' | 'S') && !after.next().is_some_and(is_word_char)
        }
        _ => true,
    }
}

fn candidates(set: &SnippetSet, text: &str) -> Vec<Candidate> {
    let Some(matcher) = &set.matcher else {
        return Vec::new();
    };
    let folded = Folded::with(text, snippet_fold_char);
    let mut out: Vec<Candidate> = matcher
        .find_overlapping_iter(&folded.text)
        .filter_map(|m| {
            let (start, end) = folded.original_span(m.start(), m.end())?;
            (start < end && left_ok(text, start) && right_ok(text, end)).then_some(Candidate {
                start,
                end,
                snippet: m.pattern().as_usize(),
            })
        })
        .collect();
    // Longest trigger first, then the earliest, then the older snippet.
    out.sort_by(|a, b| {
        set.lengths[b.snippet]
            .cmp(&set.lengths[a.snippet])
            .then(a.start.cmp(&b.start))
            .then(a.snippet.cmp(&b.snippet))
    });
    out
}

/// Sentence marks that are not doubled after an inline expansion.
fn is_sentence_mark(c: char) -> bool {
    matches!(c, '.' | '!' | '?' | '。' | '！' | '？')
}

/// Expands the snippets in `text` once. Returns the text and what expanded.
pub fn expand(set: &SnippetSet, text: &str) -> (String, Vec<SnippetHit>) {
    if set.is_empty() || text.trim().is_empty() {
        return (text.to_owned(), Vec::new());
    }
    let original = text;
    let normalized = nfc(text);
    let text: &str = &normalized;

    // Alone: the take is the trigger and nothing else.
    if let Some(&i) = set.alone.get(&alone_key(text)) {
        let s = &set.snippets[i];
        return (
            s.body.clone(),
            vec![SnippetHit {
                snippet_id: s.id,
                matched: text.trim().to_owned(),
                alone: true,
            }],
        );
    }

    // Inline: non-overlapping matches, longest first.
    let mut selected: Vec<Candidate> = Vec::new();
    for c in candidates(set, text) {
        if selected
            .iter()
            .all(|s| c.end <= s.start || c.start >= s.end)
        {
            selected.push(c);
        }
    }
    if selected.is_empty() {
        // Untouched, not even normalized.
        return (original.to_owned(), Vec::new());
    }
    selected.sort_by_key(|c| c.start);

    let mut out = String::with_capacity(text.len());
    let mut hits = Vec::with_capacity(selected.len());
    let mut cursor = 0usize;
    for c in selected {
        let s = &set.snippets[c.snippet];
        // (A dropped sentence mark may be where this match starts.)
        out.push_str(&text[cursor.min(c.start)..c.start]);
        out.push_str(&s.body);
        cursor = c.end;
        if let (Some(last), Some(next)) =
            (s.body.chars().next_back(), text[cursor..].chars().next())
        {
            if is_sentence_mark(last) && last == next {
                cursor += next.len_utf8();
            }
        }
        hits.push(SnippetHit {
            snippet_id: s.id,
            matched: text[c.start..c.end].to_owned(),
            alone: false,
        });
    }
    out.push_str(&text[cursor..]);
    (out, hits)
}

#[cfg(test)]
#[path = "snippets_tests.rs"]
mod tests;
