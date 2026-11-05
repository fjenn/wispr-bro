// SPDX-License-Identifier: GPL-3.0-or-later
//
// A port of BetterWispr's correction learning:
//   Source: BetterWispr, https://github.com/opennookorg/betterwispr
//   Commit: 304335a073a98f7439e5f7b4fbc2c31b810b807e
//   Files:  Sources/BetterWisprCore/Transcription/CorrectionLearner.swift
//           (`corrections(from:to:)`, `tokenize`, the common-word list,
//           `isVocabulary`, the word LCS and the edit distance), which
//           ports OpenWhispr `src/utils/correctionLearner.js`
//           (https://github.com/OpenWhispr/openwhispr, commit
//           4881a74d7273e3d12ca91a0b46204ad80ea03e9a,
//           Copyright (c) 2024 OpenWhispr Team, MIT,
//           LICENSES/MIT-OpenWhispr.txt); BetterWispr's additions are
//           Copyright 2026 Kartik Labhshetwar, Apache-2.0
//           (LICENSES/Apache-2.0-BetterWispr.txt);
//           Sources/BetterWisprCore/Integrations/CorrectionWatcher.swift
//           (`edit(of:from:to:)`, `comparable(_:)`), GPL-3.0: it adapts
//           VoiceInk's `FinalSnapshotDiffEngine.swift` and
//           `AutoLearnTypes.swift` (`accessibilityComparable`).
//   And VoiceInk, https://github.com/Beingpax/VoiceInk
//   Commit: c09cc1f677f40f2ee665843a61f07670210f012e
//   File:   VoiceInk/Features/Dictionary/AutoLearn/CorrectionDiffEngine.swift
//           (the candidate checks: no comma in the source, no control
//           characters, the length caps, the compact-script cap, the edge
//           punctuation trim), GPL-3.0 (© the VoiceInk authors).
//   Modified by Wispr Bro contributors, 2026-11-02: ported from Swift to
//   Rust; the length caps are the dictionary's (60 characters for a word or
//   "from"); a diff of more than 1,024 words on a side learns nothing (the
//   LCS table stays small); comparison text is NFC through icu_normalizer.
//
//! The pure half of auto-learn (S-34, W-27): what the user changed in the
//! text we pasted, and which of those changes are words worth learning.
//!
//! 1. [`comparable`]: field text and pasted text evened out the same way
//!    (line endings, non-breaking and zero-width spaces, blank lines, NFC).
//! 2. [`edit_of`]: the pasted text as it reads now, found between the 16
//!    characters on each side of it in the field before the edit (each
//!    side must appear exactly once afterwards).
//! 3. [`corrections`]: the word-level substitutions between the two (1-3
//!    words a side, several hunks per edit), keeping only spelling fixes
//!    of names and jargon: not a rewrite, not an everyday word, not a
//!    different word.
//!
//! Nothing here keeps or logs text; callers hold it in memory only.

use crate::fork::dictionary::MAX_PHRASE_CHARS;

/// Characters of field text kept on each side of the dictation as its
/// anchors (VoiceInk's 16).
pub const ANCHOR_CHARS: usize = 16;

/// A diff side longer than this many words learns nothing.
pub const MAX_WORDS: usize = 1_024;

/// Longest learned term in a script written without spaces (VoiceInk
/// `maximumUnspacedCandidateCharacters`).
pub const MAX_UNSPACED_CHARS: usize = 32;

/// One fix: what we pasted (`heard`) and what the user made of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Correction {
    pub heard: String,
    pub corrected: String,
}

impl Correction {
    pub fn new(heard: &str, corrected: &str) -> Correction {
        Correction {
            heard: heard.to_string(),
            corrected: corrected.to_string(),
        }
    }
}

/// OpenWhispr's everyday words (BetterWispr `commonWords`): never learned,
/// never rewritten.
const COMMON_WORDS: &str = "the and for not with you this but his from they say her she will one all \
     would there their what out about who get which when make can like time just him know take into \
     year your good some could them see other than then now look only come over think also back \
     after use two how our work first well way even new want because any these give day most are \
     was were been has had did does said went made got came took saw knew thought where why here \
     very much many still too again off down never every own same another both each few more less \
     last next while before through under between should might must being have that its yes okay";

/// Whether `phrase` is an everyday word (any case).
pub fn is_common(phrase: &str) -> bool {
    let p = phrase.to_lowercase();
    COMMON_WORDS.split_whitespace().any(|w| w == p)
}

/// The text as a field shows it, evened out for comparison (BetterWispr
/// `comparable`, after VoiceInk `accessibilityComparable`).
pub fn comparable(text: &str) -> String {
    let text = text
        .replace("\r\n", "\n")
        .replace(['\r', '\u{2028}', '\u{2029}'], "\n")
        .replace('\u{00A0}', " ")
        .replace(['\u{FEFF}', '\u{200B}'], "");
    let lines: Vec<&str> = text
        .split('\n')
        .map(|l| {
            if l.trim_matches(|c: char| c == ' ' || c == '\t').is_empty() {
                ""
            } else {
                l
            }
        })
        .collect();
    let joined = lines.join("\n");
    crate::fork::text::snippets::nfc(joined.trim()).into_owned()
}

fn unique_find(hay: &str, needle: &str) -> Option<(usize, usize)> {
    let first = hay.find(needle)?;
    // A second match may overlap the first (VoiceInk searches from the
    // character after the first match's start).
    let next = first + hay[first..].chars().next().map_or(1, char::len_utf8);
    if hay[next..].contains(needle) {
        return None;
    }
    Some((first, first + needle.len()))
}

fn last_chars(s: &str, n: usize) -> &str {
    let count = s.chars().count();
    match s.char_indices().nth(count.saturating_sub(n)) {
        Some((i, _)) if count > n => &s[i..],
        _ => s,
    }
}

fn first_chars(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

/// The pasted text `inserted` as it reads in `after`, the field's text
/// once the user was done, given the field's text `before` (with the paste
/// in it). `None` unless each anchor appears exactly once in `after` and
/// the right one does not come before the left one; an empty side is the
/// field's edge. All three are [`comparable`] text.
pub fn edit_of(inserted: &str, before: &str, after: &str) -> Option<String> {
    if inserted.is_empty() {
        return None;
    }
    let at = before.rfind(inserted)?;
    let left = last_chars(&before[..at], ANCHOR_CHARS);
    let right = first_chars(&before[at + inserted.len()..], ANCHOR_CHARS);
    let mut start = 0;
    let mut end = after.len();
    if !left.is_empty() {
        start = unique_find(after, left)?.1;
    }
    if !right.is_empty() {
        let (lo, _) = unique_find(after, right)?;
        if lo < start {
            return None;
        }
        end = lo;
    }
    Some(after[start..end].to_string())
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || is_mark(c)
}

fn is_mark(c: char) -> bool {
    icu_normalizer::properties::CanonicalCombiningClassMapBorrowed::new().get_u8(c) != 0
}

/// Whitespace-separated words with punctuation trimmed from their edges
/// (BetterWispr `tokenize`).
pub fn tokenize(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| w.trim_matches(|c: char| !is_word_char(c)))
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

fn edit_distance(a: &[char], b: &[char]) -> usize {
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, x) in a.iter().enumerate() {
        let mut current = Vec::with_capacity(b.len() + 1);
        current.push(i + 1);
        for (j, y) in b.iter().enumerate() {
            let v = if x == y {
                previous[j]
            } else {
                1 + previous[j].min(previous[j + 1]).min(current[j])
            };
            current.push(v);
        }
        previous = current;
    }
    previous[b.len()]
}

fn is_compact_script(c: char) -> bool {
    matches!(c as u32,
        0x0E00..=0x0EFF // Thai and Lao
        | 0x1000..=0x109F // Myanmar
        | 0x1780..=0x17FF // Khmer
        | 0x1100..=0x11FF // Hangul Jamo
        | 0x3040..=0x30FF // Hiragana and Katakana
        | 0x3130..=0x318F // Hangul Compatibility Jamo
        | 0x3400..=0x4DBF // CJK Extension A
        | 0x4E00..=0x9FFF // CJK Unified Ideographs
        | 0xAC00..=0xD7AF // Hangul Syllables
        | 0x20000..=0x2FA1F) // Additional CJK ideographs
}

/// VoiceInk's candidate checks, with the dictionary's length cap.
fn fits(c: &Correction) -> bool {
    let fits_side = |s: &str| {
        let n = s.chars().count();
        n > 0 && n <= MAX_PHRASE_CHARS && !s.chars().any(char::is_control)
    };
    if !fits_side(&c.heard) || !fits_side(&c.corrected) || c.heard.contains(',') {
        return false;
    }
    let unspaced = |s: &str| !s.chars().any(char::is_whitespace);
    let compact =
        c.heard.chars().any(is_compact_script) || c.corrected.chars().any(is_compact_script);
    if unspaced(&c.heard) && unspaced(&c.corrected) && compact {
        return c.heard.chars().count() <= MAX_UNSPACED_CHARS
            && c.corrected.chars().count() <= MAX_UNSPACED_CHARS;
    }
    true
}

/// BetterWispr `isVocabulary`: a spelling fix of a name or a term, not a
/// different word.
fn is_vocabulary(c: &Correction) -> bool {
    let heard = c.heard.to_lowercase();
    let corrected = c.corrected.to_lowercase();
    if !corrected.chars().any(char::is_alphabetic) {
        return false;
    }
    if heard == corrected {
        // A case-only fix counts when it is distinctive ("kv" → "KV",
        // "posthog" → "PostHog"), not a capital at the start.
        return c.corrected.chars().skip(1).any(char::is_uppercase);
    }
    if c.corrected.chars().count() < 3 && !c.corrected.chars().any(char::is_uppercase) {
        return false;
    }
    if is_common(&corrected) {
        return false;
    }
    let compact = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<Vec<_>>();
    let (h, k) = (compact(&heard), compact(&corrected));
    let distance = edit_distance(&h, &k);
    distance as f64 / h.len().max(k.len()) as f64 <= 0.65
}

/// The word-level substitutions from `before` to `after` (1-3 words a
/// side), by a longest-common-subsequence walk (BetterWispr
/// `substitutions`).
fn substitutions(before: &[String], after: &[String]) -> Vec<Correction> {
    let (m, n) = (before.len(), after.len());
    let width = n + 1;
    let mut lcs = vec![0u16; (m + 1) * width];
    for i in (0..m).rev() {
        for j in (0..n).rev() {
            lcs[i * width + j] = if before[i] == after[j] {
                lcs[(i + 1) * width + j + 1] + 1
            } else {
                lcs[(i + 1) * width + j].max(lcs[i * width + j + 1])
            };
        }
    }
    let mut out = Vec::new();
    let (mut removed, mut added): (Vec<&str>, Vec<&str>) = (Vec::new(), Vec::new());
    let mut flush = |removed: &mut Vec<&str>, added: &mut Vec<&str>| {
        if (1..=3).contains(&removed.len()) && (1..=3).contains(&added.len()) {
            out.push(Correction {
                heard: removed.join(" "),
                corrected: added.join(" "),
            });
        }
        removed.clear();
        added.clear();
    };
    let (mut i, mut j) = (0, 0);
    while i < m || j < n {
        if i < m && j < n && before[i] == after[j] {
            flush(&mut removed, &mut added);
            i += 1;
            j += 1;
        } else if j < n && (i == m || lcs[i * width + j + 1] > lcs[(i + 1) * width + j]) {
            added.push(&after[j]);
            j += 1;
        } else {
            removed.push(&before[i]);
            i += 1;
        }
    }
    flush(&mut removed, &mut added);
    out
}

/// The spelling fixes the user made to dictated text `original` (as it
/// was pasted) to get `edited`: word substitutions in any number of hunks,
/// none if the edit rewrote more than half the words; each corrected term
/// once (BetterWispr `corrections(from:to:)`, VoiceInk's checks).
pub fn corrections(original: &str, edited: &str) -> Vec<Correction> {
    let before = tokenize(original);
    let after = tokenize(edited);
    if before.is_empty()
        || after.is_empty()
        || before == after
        || before.len() > MAX_WORDS
        || after.len() > MAX_WORDS
    {
        return Vec::new();
    }
    let subs = substitutions(&before, &after);
    if subs.len() > (before.len() / 2).max(1) {
        return Vec::new();
    }
    let mut seen = std::collections::HashSet::new();
    subs.into_iter()
        .filter(|c| fits(c) && seen.insert(c.corrected.to_lowercase()) && is_vocabulary(c))
        .collect()
}
