// SPDX-License-Identifier: GPL-3.0-or-later
//! Spelling hints from the text near the caret (S-20, D-58), pure.
//!
//! The read (`caret.rs`) hands its text here once, on the AX thread, and
//! keeps only what [`extract`] returns: the **names and terms** seen near the
//! caret (`Siobhán`, `Kowalczyk`, `McAllister`), never the text itself. A
//! take then uses them three ways, all local:
//!
//! 1. the recognizer's vocabulary, for models that take one (Whisper;
//!    transcribe-cpp `Feature::Vocabulary`), through the
//!    `managers/transcription.rs` hook ([`vocabulary_terms`]);
//! 2. a spelling pass on the transcript ([`correct`]), the fallback for
//!    models without a vocabulary, as Handy's custom words fall back to
//!    fuzzy correction (`audio_toolkit::text::apply_custom_words`, whose
//!    scoring this follows: normalized Levenshtein with a Soundex boost,
//!    the 25 % length rule, n-grams of joined words);
//! 3. the cleanup prompt's CONTEXT line ([`relevant`]: only the hints the
//!    transcript has or nearly has; none means the S-12a prompt, unchanged).
//!
//! Safety rules (a name must never replace an ordinary word):
//! - a hint is name-like: capitalized and not just a sentence's first word,
//!   or with letters outside ASCII, or with a capital inside (`DeShawn`);
//!   stopwords of the user's languages and all-caps words are never hints;
//! - a lowercase transcript word changes only when it is the same word with
//!   other accents or joins (`siobhan` → `Siobhán`), never by fuzzy match;
//! - a transcript word that already is a hint is left alone;
//! - only letters change: punctuation, digits, line breaks and spaces stay
//!   byte for byte.

use std::collections::HashSet;

use natural::phonetics::soundex;
use strsim::levenshtein;

/// At most this many hints per take (ranked nearest the caret first).
pub const MAX_HINTS: usize = 25;
/// At most this many hints go to the recognizer's vocabulary.
pub const MAX_VOCABULARY: usize = 20;
/// At most this many hints go to the cleanup prompt.
pub const MAX_PROMPT: usize = 10;
/// Shortest hint, in characters.
pub const MIN_HINT_CHARS: usize = 3;
/// Longest hint, in characters.
pub const MAX_HINT_CHARS: usize = 40;
/// Handy's default `word_correction_threshold`: the combined score a fuzzy
/// match must stay under.
pub const THRESHOLD: f64 = 0.18;
/// A looser score: the hint is "nearly" in the transcript (prompt only).
pub const RELEVANT_THRESHOLD: f64 = 0.35;

/// The comparison key of a word: letters and digits only, accents removed
/// (NFD without combining marks, plus the Latin letters NFD keeps whole),
/// lowercase.
pub fn key(word: &str) -> String {
    let nfd = icu_normalizer::DecomposingNormalizerBorrowed::new_nfd().normalize(word);
    let marks = icu_normalizer::properties::CanonicalCombiningClassMapBorrowed::new();
    let mut out = String::with_capacity(word.len());
    for c in nfd.chars() {
        if !c.is_ascii() && marks.get_u8(c) != 0 {
            continue;
        }
        match c {
            'ø' | 'Ø' => out.push('o'),
            'ł' | 'Ł' => out.push('l'),
            'đ' | 'Đ' | 'ð' | 'Ð' => out.push('d'),
            'æ' | 'Æ' => out.push_str("ae"),
            'œ' | 'Œ' => out.push_str("oe"),
            'ß' | 'ẞ' => out.push_str("ss"),
            'þ' | 'Þ' => out.push_str("th"),
            'ı' => out.push('i'),
            c if c.is_alphanumeric() => out.extend(c.to_lowercase()),
            _ => {}
        }
    }
    out
}

/// A word of a text: its byte span (letters and inner `'`/`-` only).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Word {
    start: usize,
    end: usize,
    /// The first word of a sentence, a line or the text.
    initial: bool,
}

fn is_inner_joiner(c: char) -> bool {
    matches!(c, '\'' | '\u{2019}' | '-' | '\u{2010}')
}

fn is_word_char(c: char) -> bool {
    c.is_alphabetic() || (!c.is_ascii() && is_mark(c))
}

fn is_mark(c: char) -> bool {
    icu_normalizer::properties::CanonicalCombiningClassMapBorrowed::new().get_u8(c) != 0
}

/// The words of `text`, with whether each starts a sentence. A word touching
/// a digit, `@`, `/`, `_` or `.` inside (e-mails, URLs, file names, code) is
/// not returned.
fn words(text: &str) -> Vec<Word> {
    let mut out = Vec::new();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;
    // A sentence starts at the beginning, after a line break, and after
    // . ! ? : (and their full-width forms) followed by space.
    let mut initial = true;
    while i < chars.len() {
        let (at, c) = chars[i];
        if !is_word_char(c) {
            let sentence_end = matches!(c, '.' | '!' | '?' | ':' | '。' | '！' | '？')
                && chars.get(i + 1).is_none_or(|(_, n)| n.is_whitespace());
            if matches!(c, '\n' | '\r') || sentence_end {
                initial = true;
            }
            i += 1;
            continue;
        }
        let start = at;
        let mut j = i;
        while j < chars.len() {
            let c = chars[j].1;
            if is_word_char(c) {
                j += 1;
            } else if is_inner_joiner(c)
                && chars.get(j + 1).is_some_and(|(_, n)| is_word_char(*n))
                && j > i
            {
                j += 1;
            } else {
                break;
            }
        }
        let end = chars.get(j).map_or(text.len(), |(a, _)| *a);
        let before = i.checked_sub(1).map(|k| chars[k].1);
        let after = chars.get(j).map(|(_, c)| *c);
        let after2 = chars.get(j + 1).map(|(_, c)| *c);
        let glued = |c: Option<char>| {
            c.is_some_and(|c| c.is_ascii_digit() || matches!(c, '@' | '/' | '_' | '\\' | '#'))
        };
        let dotted = after == Some('.') && after2.is_some_and(is_word_char);
        if !glued(before) && !glued(after) && !dotted && before != Some('.') {
            out.push(Word {
                start,
                end,
                initial,
            });
        }
        initial = false;
        i = j;
    }
    out
}

fn is_capitalized(w: &str) -> bool {
    w.chars().next().is_some_and(char::is_uppercase)
}

fn is_all_caps(w: &str) -> bool {
    let letters: Vec<char> = w.chars().filter(|c| c.is_alphabetic()).collect();
    letters.len() > 1 && letters.iter().all(|c| c.is_uppercase())
}

fn has_inner_capital(w: &str) -> bool {
    w.chars().skip(1).any(char::is_uppercase)
}

/// Stopwords of `languages` (English always in), lower case.
fn stopword_set(languages: &[String]) -> HashSet<&'static str> {
    let sw = crate::fork::stats::voice::stopword_table();
    let mut langs: Vec<String> = languages
        .iter()
        .map(|l| {
            l.split(['-', '_'])
                .next()
                .unwrap_or("")
                .to_ascii_lowercase()
        })
        .collect();
    langs.push("en".into());
    langs
        .iter()
        .filter_map(|l| sw.get(l.as_str()))
        .flat_map(|set| set.iter().map(String::as_str))
        .collect()
}

/// Strips a trailing possessive (`Kowalczyk's` → `Kowalczyk`, `'s`).
fn split_possessive(w: &str) -> (&str, &str) {
    for suffix in ["'s", "\u{2019}s", "'S", "\u{2019}S"] {
        if let Some(base) = w.strip_suffix(suffix) {
            if base.chars().count() >= MIN_HINT_CHARS {
                return (base, &w[base.len()..]);
            }
        }
    }
    (w, "")
}

/// The hints in `sections` (nearest the caret first: the text before the
/// caret read backwards from it, the selection, the text after, then the
/// lines around the field), at most [`MAX_HINTS`].
pub fn extract(sections: &[&str], languages: &[String]) -> Vec<String> {
    let stop = stopword_set(languages);
    let mut out: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    // Words seen only as a sentence's first word wait for a second sighting
    // inside a sentence.
    let candidate = |section: &str, w: &Word| -> Option<String> {
        let (word, _) = split_possessive(&section[w.start..w.end]);
        let chars = word.chars().count();
        ((MIN_HINT_CHARS..=MAX_HINT_CHARS).contains(&chars)
            && is_capitalized(word)
            && !is_all_caps(word)
            && !stop.contains(word.to_lowercase().as_str()))
        .then(|| word.to_owned())
    };
    for section in sections {
        let ws = words(section);
        for (i, w) in ws.iter().enumerate() {
            let Some(word) = candidate(section, w) else {
                continue;
            };
            // A first word followed by another capitalized one ("Priya
            // Sharma", an author line) is a name too.
            let pair = ws.get(i + 1).is_some_and(|n| {
                section[w.end..n.start].chars().all(|c| c == ' ') && candidate(section, n).is_some()
            });
            let name_like = !w.initial || pair || !word.is_ascii() || has_inner_capital(&word);
            if !name_like || !seen.insert(word.to_owned()) {
                continue;
            }
            out.push(word.to_owned());
            if out.len() == MAX_HINTS {
                return out;
            }
        }
    }
    out
}

/// What a take does with its hints, for the history row and the pill.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Corrected {
    pub text: String,
    /// Words the spelling pass changed.
    pub fixes: u32,
    /// Each change: (start, end) of the replaced bytes in the input, and the
    /// byte length of what replaced them.
    pub edits: Vec<(usize, usize, usize)>,
}

impl Corrected {
    /// Where byte offset `at` of the input is in the output (an offset
    /// inside a replaced span is not moved; protected spans never are).
    pub fn map_offset(&self, at: usize) -> usize {
        let mut shift: isize = 0;
        for &(start, end, len) in &self.edits {
            if start >= at {
                break;
            }
            if end <= at {
                shift += len as isize - (end - start) as isize;
            }
        }
        (at as isize + shift).max(0) as usize
    }
}

/// The best hint for candidate key `k` and its score, if any is under
/// `threshold`. `cased`: the transcript wrote the candidate capitalized (the
/// recognizer took it for a name), so a fuzzy match may change it.
fn best<'a>(
    k: &str,
    cased: bool,
    hints: &'a [(String, String)],
    threshold: f64,
) -> Option<(&'a str, f64)> {
    if k.chars().count() < MIN_HINT_CHARS {
        return None;
    }
    let mut best: Option<(&str, f64)> = None;
    for (hint, hk) in hints {
        let score = if hk == k {
            0.0
        } else if !cased {
            continue;
        } else {
            let (a, b) = (k.chars().count(), hk.chars().count());
            let max = a.max(b) as f64;
            if (a.abs_diff(b) as f64) > (max * 0.25).max(2.0) {
                continue;
            }
            let lev = levenshtein(k, hk) as f64 / max;
            let phonetic = k.chars().all(|c| c.is_ascii_alphabetic())
                && hk.chars().all(|c| c.is_ascii_alphabetic())
                && soundex(k, hk);
            if phonetic {
                lev * 0.3
            } else {
                lev
            }
        };
        if score < threshold && best.is_none_or(|(_, s)| score < s) {
            best = Some((hint.as_str(), score));
        }
    }
    best
}

fn keyed(hints: &[String]) -> Vec<(String, String)> {
    hints
        .iter()
        .map(|h| (h.clone(), key(h)))
        .filter(|(_, k)| !k.is_empty())
        .collect()
}

/// A span of one or two transcript words that may be one hint.
struct Candidate<'t> {
    start: usize,
    end: usize,
    /// The words as written, joined by what was between them.
    text: &'t str,
    key: String,
    cased: bool,
    words: usize,
}

fn candidates<'t>(text: &'t str, ws: &[Word], i: usize) -> Vec<Candidate<'t>> {
    let mut out = Vec::new();
    for n in [2usize, 1] {
        if i + n > ws.len() {
            continue;
        }
        let first = ws[i];
        let last = ws[i + n - 1];
        // Two words join only across plain spaces (no punctuation, no line
        // break).
        if n == 2 && !text[first.end..last.start].chars().all(|c| c == ' ') {
            continue;
        }
        let span = &text[first.start..last.end];
        // A possessive after the last word stays as written.
        let (base, _) = split_possessive(span);
        let k: String = text[first.start..first.start + base.len()]
            .split(' ')
            .map(key)
            .collect();
        out.push(Candidate {
            start: first.start,
            end: first.start + base.len(),
            text: base,
            key: k,
            cased: is_capitalized(&text[first.start..first.end]),
            words: n,
        });
    }
    out
}

/// Stage "context" (R5 §4.10, after Handy's stages): transcript words that
/// are a hint misspelled get the hint's spelling.
pub fn correct(text: &str, hints: &[String]) -> Corrected {
    correct_outside(text, hints, &[])
}

/// [`correct`], never touching the byte spans in `protected` (the
/// dictionary rules' outputs, which stage 9 must find again).
pub fn correct_outside(text: &str, hints: &[String], protected: &[(usize, usize)]) -> Corrected {
    let keyed = keyed(hints);
    if keyed.is_empty() || text.is_empty() {
        return Corrected {
            text: text.to_owned(),
            ..Corrected::default()
        };
    }
    let overlaps = |s: usize, e: usize| protected.iter().any(|&(ps, pe)| s < pe && ps < e);
    let mut edits = Vec::new();
    let exact: HashSet<&str> = hints.iter().map(String::as_str).collect();
    let ws = words(text);
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    let mut fixes = 0;
    let mut i = 0;
    while i < ws.len() {
        let mut chosen: Option<(Candidate<'_>, &str, f64)> = None;
        for c in candidates(text, &ws, i) {
            if exact.contains(c.text) {
                // Already a hint, spelled right: leave it (and its pair).
                chosen = None;
                if c.words == 1 {
                    break;
                }
                continue;
            }
            if overlaps(c.start, c.end) {
                continue;
            }
            if let Some((hint, score)) = best(&c.key, c.cased, &keyed, THRESHOLD) {
                // Same letters but only the case differs on a lowercase
                // word ("mark" vs "Mark"): an ordinary word, keep it.
                let lowercase_same = !c.cased && c.text.to_lowercase() == hint.to_lowercase();
                if lowercase_same || c.text == hint {
                    continue;
                }
                if chosen.as_ref().is_none_or(|(_, _, s)| score < *s) {
                    chosen = Some((c, hint, score));
                }
            }
        }
        match chosen {
            Some((c, hint, _)) => {
                out.push_str(&text[copied..c.start]);
                out.push_str(hint);
                edits.push((c.start, c.end, hint.len()));
                copied = c.end;
                fixes += 1;
                i += c.words;
            }
            None => i += 1,
        }
    }
    out.push_str(&text[copied..]);
    Corrected {
        text: out,
        fixes,
        edits,
    }
}

/// The hints this transcript has (spelled as the hint) or nearly has: what
/// the cleanup prompt may name. At most [`MAX_PROMPT`], in hint order.
pub fn relevant(text: &str, hints: &[String]) -> Vec<String> {
    let keyed = keyed(hints);
    if keyed.is_empty() {
        return Vec::new();
    }
    let ws = words(text);
    let mut found: HashSet<&str> = HashSet::new();
    for i in 0..ws.len() {
        for c in candidates(text, &ws, i) {
            if let Some(h) = hints.iter().find(|h| h.as_str() == c.text) {
                found.insert(h.as_str());
                continue;
            }
            if let Some((hint, _)) = best(&c.key, c.cased, &keyed, RELEVANT_THRESHOLD) {
                found.insert(hint);
            }
        }
    }
    hints
        .iter()
        .filter(|h| found.contains(h.as_str()))
        .take(MAX_PROMPT)
        .cloned()
        .collect()
}

/// The hints for the recognizer's vocabulary, after the user's dictionary
/// words (which keep priority).
pub fn vocabulary_terms(custom_words: &[String], hints: &[String]) -> Vec<String> {
    let mut out = custom_words.to_vec();
    for h in hints.iter().take(MAX_VOCABULARY) {
        if !out.iter().any(|w| w.eq_ignore_ascii_case(h)) {
            out.push(h.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn keys_fold_accents_case_and_joins() {
        assert_eq!(key("Siobhán"), "siobhan");
        assert_eq!(key("Bjørn"), "bjorn");
        assert_eq!(key("Łukasz"), "lukasz");
        assert_eq!(key("O'Brien"), "obrien");
        assert_eq!(key("Anne-Marie"), "annemarie");
        assert_eq!(key("Dvořák"), "dvorak");
        assert_eq!(key("Strauß"), "strauss");
        assert_eq!(key("Nguyễn"), "nguyen");
    }

    #[test]
    fn names_are_hints_ordinary_words_are_not() {
        let hints = extract(
            &["Thanks for the notes. I'll ask Siobhán Kowalczyk and DeShawn to review the OKR doc with Priya."],
            &[],
        );
        assert_eq!(hints, s(&["Siobhán", "Kowalczyk", "DeShawn", "Priya"]));
    }

    #[test]
    fn a_sentence_first_word_is_not_a_hint_unless_it_looks_like_a_name() {
        // "Review" starts a sentence; "Zoë" has a letter outside ASCII;
        // "McAllister" a capital inside.
        let hints = extract(&["Review it.\nZoë said so. McAllister agreed."], &[]);
        assert_eq!(hints, s(&["Zoë", "McAllister"]));
        // A line of UI labels gives nothing.
        assert!(extract(&["Reply\nSend\nChannels\nThreads"], &[]).is_empty());
    }

    #[test]
    fn stopwords_all_caps_short_words_emails_and_urls_are_skipped() {
        let hints = extract(
            &["Mail it to Jo or ASAP at anna.kowalski@example.com or see https://Example.com/Kowalski and v2Beta The"],
            &[],
        );
        assert!(hints.is_empty(), "{hints:?}");
        // Spoken languages add their stopwords: "Und" is German.
        assert_eq!(extract(&["so Und Mila"], &s(&["de"])), s(&["Mila"]));
    }

    #[test]
    fn hints_are_capped_and_unique() {
        let text: String = (0..60)
            .map(|i| format!("and Zed{} ", "a".repeat(i % 30 + 1)))
            .collect();
        let hints = extract(&[&text], &[]);
        assert_eq!(hints.len(), MAX_HINTS);
        let unique: HashSet<_> = hints.iter().collect();
        assert_eq!(unique.len(), hints.len());
        let possessive = extract(&["ask Kowalczyk's team and Kowalczyk"], &[]);
        assert_eq!(possessive, s(&["Kowalczyk"]));
    }

    #[test]
    fn misspelled_names_get_the_hint_spelling() {
        let hints = s(&["Siobhán", "Kowalczyk", "McAllister", "Thibault"]);
        let c = correct("Send it to Siobhan Kovalczyk and Mcallister.", &hints);
        assert_eq!(c.text, "Send it to Siobhán Kowalczyk and McAllister.");
        assert_eq!(c.fixes, 3);
        // Two words of the recognizer, one name.
        let c = correct("Ask Mac Allister, then Thibo.", &hints);
        assert_eq!(c.text, "Ask McAllister, then Thibo.");
        // A possessive stays.
        let c = correct("That is Kowalcyk's draft.", &hints);
        assert_eq!(c.text, "That is Kowalczyk's draft.");
    }

    #[test]
    fn ordinary_words_are_never_turned_into_names() {
        let hints = s(&["Mark", "Ward", "Will", "Kowalczyk"]);
        let text = "mark this word as done, will you? The ward was full.";
        assert_eq!(correct(text, &hints).text, text);
        // A lowercase word with other accents is the same name.
        assert_eq!(correct("ask siobhan", &s(&["Siobhán"])).text, "ask Siobhán");
        // Already spelled as a hint: unchanged.
        let c = correct("Mark and Marc", &s(&["Mark", "Marc"]));
        assert_eq!((c.text.as_str(), c.fixes), ("Mark and Marc", 0));
    }

    #[test]
    fn only_letters_change() {
        let hints = s(&["Kowalczyk"]);
        let text = "Line one\n\nKovalczyk:  42 items\t(ok)\n";
        let c = correct(text, &hints);
        assert_eq!(c.text, "Line one\n\nKowalczyk:  42 items\t(ok)\n");
        // Code, e-mails and digits are not words.
        let text = "see kovalczyk@example.com and Kovalczyk2 and src/Kovalczyk";
        assert_eq!(correct(text, &hints).text, text);
        assert_eq!(correct("", &hints).text, "");
        assert_eq!(correct("anything", &[]).text, "anything");
    }

    #[test]
    fn protected_spans_stay_and_offsets_follow_the_edits() {
        let hints = s(&["Siobhán", "Kowalczyk"]);
        let text = "Siobhan met Kovalczyk";
        // The dictionary put "Kovalczyk" there: it is not touched.
        let c = correct_outside(text, &hints, &[(12, 21)]);
        assert_eq!(c.text, "Siobhán met Kovalczyk");
        // "Siobhán" is one byte longer: later offsets move by one.
        assert_eq!(c.map_offset(12), 13);
        assert_eq!(c.map_offset(0), 0);
        assert_eq!(&c.text[c.map_offset(12)..], "Kovalczyk");
    }

    #[test]
    fn relevant_hints_are_the_ones_the_transcript_has_or_nearly_has() {
        let hints = s(&["Siobhán", "Kowalczyk", "Priya", "Thibault"]);
        assert_eq!(
            relevant("Siobhán and Kovalchek met.", &hints),
            s(&["Siobhán", "Kowalczyk"])
        );
        assert!(relevant("Nothing here.", &hints).is_empty());
        assert!(relevant("x", &[]).is_empty());
    }

    #[test]
    fn vocabulary_keeps_the_dictionary_first() {
        let v = vocabulary_terms(
            &s(&["Wispr Bro", "kowalczyk"]),
            &s(&["Kowalczyk", "Siobhán"]),
        );
        assert_eq!(v, s(&["Wispr Bro", "kowalczyk", "Siobhán"]));
        let many: Vec<String> = (0..40).map(|i| format!("Name{i}")).collect();
        assert_eq!(vocabulary_terms(&[], &many).len(), MAX_VOCABULARY);
    }
}
