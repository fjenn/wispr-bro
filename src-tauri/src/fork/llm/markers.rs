// SPDX-License-Identifier: GPL-3.0-or-later
//! Words the cleanup gate and the guards need to know (S-12a): hesitations,
//! the owner's filler policy, self-repair markers, stopwords and assistant
//! openers, per language (`tables/markers.json`).
//!
//! The first eight `assistant_markers` are Muesli's
//! (`Qwen3PostProcessor.shouldFallbackToInput`, MIT, see
//! docs/fork/PORTS.md); everything else is our own data.
//!
//! Matching works on [`words`]: UAX #29 word segments that contain a letter
//! or digit, lower-cased. A phrase matches a run of whole words.

use std::collections::HashSet;

use once_cell::sync::Lazy;
use serde::Deserialize;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Deserialize)]
struct RawTable {
    hesitations: Vec<String>,
    fillers: Vec<String>,
    repairs: Vec<String>,
    stopwords: String,
    openers: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawFile {
    languages: std::collections::BTreeMap<String, RawTable>,
    assistant_markers: Vec<String>,
}

/// One language's tables, phrases split into words.
#[derive(Debug, Default)]
pub struct Table {
    pub hesitations: Vec<Vec<String>>,
    pub fillers: Vec<Vec<String>>,
    pub repairs: Vec<Vec<String>>,
    pub stopwords: HashSet<String>,
    pub openers: Vec<String>,
}

pub struct Tables {
    by_language: Vec<(String, Table)>,
    /// Every language merged (unknown language).
    all: Table,
    pub assistant_markers: Vec<String>,
}

fn phrases(list: &[String]) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = list
        .iter()
        .map(|p| words(p).into_iter().map(|w| w.text).collect::<Vec<_>>())
        .filter(|p: &Vec<String>| !p.is_empty())
        .collect();
    // Longest first, so "no wait" wins over "no".
    out.sort_by_key(|p| std::cmp::Reverse(p.len()));
    out.dedup();
    out
}

fn table(raw: &RawTable) -> Table {
    Table {
        hesitations: phrases(&raw.hesitations),
        fillers: phrases(&raw.fillers),
        repairs: phrases(&raw.repairs),
        stopwords: raw
            .stopwords
            .split_whitespace()
            .map(str::to_owned)
            .collect(),
        openers: raw.openers.clone(),
    }
}

fn merge(into: &mut Table, t: &Table) {
    into.hesitations.extend(t.hesitations.iter().cloned());
    into.fillers.extend(t.fillers.iter().cloned());
    into.repairs.extend(t.repairs.iter().cloned());
    into.stopwords.extend(t.stopwords.iter().cloned());
    into.openers.extend(t.openers.iter().cloned());
}

fn sort_dedup(list: &mut Vec<Vec<String>>) {
    list.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    list.dedup();
}

pub static TABLES: Lazy<Tables> = Lazy::new(|| {
    let raw: RawFile = serde_json::from_str(include_str!("tables/markers.json"))
        .expect("fork/llm/tables/markers.json is valid (tested)");
    let by_language: Vec<(String, Table)> = raw
        .languages
        .iter()
        .map(|(k, v)| (k.clone(), table(v)))
        .collect();
    let mut all = Table::default();
    for (_, t) in &by_language {
        merge(&mut all, t);
    }
    sort_dedup(&mut all.hesitations);
    sort_dedup(&mut all.fillers);
    sort_dedup(&mut all.repairs);
    all.openers.sort();
    all.openers.dedup();
    Tables {
        by_language,
        all,
        assistant_markers: raw.assistant_markers,
    }
});

impl Tables {
    /// The table for `language` (ISO 639-1, region ignored); every language
    /// merged when it is unknown or has no table.
    pub fn for_language(&self, language: Option<&str>) -> &Table {
        let primary = language
            .and_then(|l| l.split(['-', '_']).next())
            .map(str::to_ascii_lowercase);
        primary
            .and_then(|p| self.by_language.iter().find(|(k, _)| *k == p))
            .map(|(_, t)| t)
            .unwrap_or(&self.all)
    }

    pub fn languages(&self) -> impl Iterator<Item = &str> {
        self.by_language.iter().map(|(k, _)| k.as_str())
    }
}

/// One word of a text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Word {
    /// Lower-cased, curly apostrophes made straight.
    pub text: String,
    /// Byte range in the original text.
    pub start: usize,
    pub end: usize,
}

/// The words of `text` (see the module docs).
pub fn words(text: &str) -> Vec<Word> {
    text.unicode_word_indices()
        .filter(|(_, w)| w.chars().any(char::is_alphanumeric))
        .map(|(start, w)| Word {
            text: w.to_lowercase().replace('\u{2019}', "'"),
            start,
            end: start + w.len(),
        })
        .collect()
}

/// `true` when `phrase` matches `words` starting at `at`.
pub fn matches_at(words: &[Word], at: usize, phrase: &[String]) -> bool {
    at + phrase.len() <= words.len()
        && phrase
            .iter()
            .zip(&words[at..at + phrase.len()])
            .all(|(p, w)| *p == w.text)
}

/// The longest phrase of `list` that starts at `at`, as a word count.
pub fn phrase_at(words: &[Word], at: usize, list: &[Vec<String>]) -> Option<usize> {
    list.iter()
        .find(|p| matches_at(words, at, p))
        .map(|p| p.len())
}

/// `true` when any phrase of `list` occurs in `words`.
pub fn contains_any(words: &[Word], list: &[Vec<String>]) -> bool {
    (0..words.len()).any(|i| phrase_at(words, i, list).is_some())
}

/// `true` when `text` has no words, or only hesitations and fillers (an
/// input that may clean to nothing; SpeakoFlow
/// `transcription_allows_empty_output`, with our multilingual lists).
pub fn only_fillers(text: &str, table: &Table) -> bool {
    let ws = words(text);
    let mut i = 0;
    while i < ws.len() {
        match phrase_at(&ws, i, &table.hesitations).or_else(|| phrase_at(&ws, i, &table.fillers)) {
            Some(n) => i += n,
            None => return false,
        }
    }
    true
}

/// Two identical words in a row ("the the"), open-flow's stutter test.
pub fn has_stutter(words: &[Word]) -> bool {
    words.windows(2).any(|w| {
        w[0].text == w[1].text
            && w[0].text.chars().any(char::is_alphabetic)
            // "that that" and "had had" can be grammatical; still worth a look,
            // but very short repeats of digits are numbers ("1 1").
            && !w[0].text.chars().all(|c| c.is_ascii_digit())
    })
}

/// Which words of `words` a correct cleanup may drop: hesitations, fillers,
/// repair markers and up to [`REPAIR_REACH`] words before a repair marker
/// (the reparandum, "the blue **no** the green"). `true` = may drop.
pub fn droppable(words: &[Word], table: &Table) -> Vec<bool> {
    let mut drop = vec![false; words.len()];
    let mut i = 0;
    while i < words.len() {
        if let Some(n) = phrase_at(words, i, &table.repairs) {
            for d in drop.iter_mut().skip(i).take(n) {
                *d = true;
            }
            for d in drop.iter_mut().take(i).skip(i.saturating_sub(REPAIR_REACH)) {
                *d = true;
            }
            i += n;
        } else if let Some(n) =
            phrase_at(words, i, &table.hesitations).or_else(|| phrase_at(words, i, &table.fillers))
        {
            for d in drop.iter_mut().skip(i).take(n) {
                *d = true;
            }
            i += n;
        } else {
            i += 1;
        }
    }
    drop
}

/// How far back a repair may reach.
pub const REPAIR_REACH: usize = 4;

/// The content words of `text` a cleanup must keep: words that are not
/// stopwords and not droppable (see [`droppable`]), first occurrence each.
pub fn required_words(text: &str, table: &Table) -> Vec<String> {
    let ws = words(text);
    let drop = droppable(&ws, table);
    let mut seen = HashSet::new();
    ws.iter()
        .zip(drop)
        .filter(|(w, d)| !d && !table.stopwords.contains(&w.text))
        .filter(|(w, _)| seen.insert(w.text.clone()))
        .map(|(w, _)| w.text.clone())
        .collect()
}

/// `true` for text in a script written without spaces (Han, kana, Thai…),
/// where word-level checks say little.
pub fn is_unspaced_script(text: &str) -> bool {
    let mut cjk = 0usize;
    let mut letters = 0usize;
    for c in text.chars().filter(|c| c.is_alphabetic()) {
        letters += 1;
        let cp = c as u32;
        if matches!(cp, 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x0E00..=0x0EFF | 0x1000..=0x109F | 0x1780..=0x17FF)
        {
            cjk += 1;
        }
    }
    letters > 0 && cjk * 2 >= letters
}

#[cfg(test)]
mod tests {
    use super::*;

    fn en() -> &'static Table {
        TABLES.for_language(Some("en"))
    }

    #[test]
    fn the_tables_load_and_cover_the_owner_languages() {
        let langs: Vec<&str> = TABLES.languages().collect();
        for l in [
            "en", "nl", "fr", "de", "es", "it", "pt", "sv", "pl", "ja", "zh", "ko",
        ] {
            assert!(langs.contains(&l), "{l}");
            let t = TABLES.for_language(Some(l));
            assert!(!t.hesitations.is_empty() && !t.fillers.is_empty() && !t.repairs.is_empty());
            assert!(!t.stopwords.is_empty() && !t.openers.is_empty());
        }
        // Region tags and unknown languages.
        assert!(std::ptr::eq(
            TABLES.for_language(Some("nl-BE")),
            TABLES.for_language(Some("nl"))
        ));
        assert!(std::ptr::eq(
            TABLES.for_language(Some("xx")),
            TABLES.for_language(None)
        ));
        assert!(TABLES.for_language(None).stopwords.contains("het"));
        assert!(TABLES
            .assistant_markers
            .iter()
            .any(|m| m == "the user is asking"));
    }

    #[test]
    fn the_filler_lists_follow_the_owner_policy() {
        // eval/cleanup/README.md "Discourse markers": the table per language.
        let has = |l: &str, p: &str| {
            let want: Vec<String> = words(p).into_iter().map(|w| w.text).collect();
            TABLES.for_language(Some(l)).fillers.contains(&want)
                || TABLES.for_language(Some(l)).hesitations.contains(&want)
        };
        for (l, p) in [
            ("en", "basically"),
            ("en", "let me think"),
            ("en", "you know"),
            ("en", "hmm"),
            ("nl", "nou"),
            ("nl", "dus"),
            ("nl", "zeg maar"),
            ("nl", "ehm"),
            ("fr", "alors"),
            ("fr", "du coup"),
            ("fr", "euh"),
            ("fr", "genre"),
            ("de", "also"),
            ("de", "halt"),
            ("de", "ähm"),
            ("es", "o sea"),
            ("pt", "né"),
            ("it", "cioè"),
            ("sv", "liksom"),
            ("pl", "yyy"),
            ("ja", "えーと"),
            ("zh", "那个"),
            ("ko", "그러니까"),
        ] {
            assert!(has(l, p), "{l}: {p}");
        }
    }

    #[test]
    fn words_are_lowercased_segments_with_offsets() {
        let ws = words("Don’t send it, Anna-Lena! 3.5 km");
        let t: Vec<&str> = ws.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(t, ["don't", "send", "it", "anna", "lena", "3.5", "km"]);
        assert_eq!(&"Don’t send"[ws[1].start..ws[1].end], "send");
    }

    #[test]
    fn only_fillers_allows_an_empty_cleanup() {
        assert!(only_fillers("", en()));
        assert!(only_fillers("um uh, you know", en()));
        assert!(only_fillers("Hmm. Okay.", en()));
        assert!(!only_fillers("um send it", en()));
        assert!(only_fillers("euh bon", TABLES.for_language(Some("fr"))));
        assert!(!only_fillers(
            "euh bonjour",
            TABLES.for_language(Some("fr"))
        ));
    }

    #[test]
    fn stutters_are_found() {
        assert!(has_stutter(&words("refactor the the login page")));
        assert!(has_stutter(&words("it is is broken")));
        assert!(!has_stutter(&words("Send the report to Sarah.")));
        assert!(!has_stutter(&words("room 1 1 b")));
    }

    #[test]
    fn repairs_make_their_reparandum_droppable() {
        let req = required_words("we need the blue no the green folder", en());
        assert!(req.contains(&"green".to_string()));
        assert!(req.contains(&"folder".to_string()));
        // "blue" sits before the repair marker "no".
        assert!(!req.contains(&"blue".to_string()));
        let req = required_words("coffee at 2 actually 3", en());
        assert!(!req.contains(&"2".to_string()));
        assert!(req.contains(&"3".to_string()));
        // Fillers and stopwords are never required.
        let req = required_words("um so basically the client wants a refund", en());
        assert_eq!(req, ["client", "wants", "refund"]);
    }

    #[test]
    fn dutch_and_french_repairs_and_fillers() {
        let nl = TABLES.for_language(Some("nl"));
        let req = required_words("nou ik kom om drie uur nee wacht om vier uur", nl);
        assert!(req.contains(&"vier".to_string()) && req.contains(&"uur".to_string()));
        assert!(!req.contains(&"nou".to_string()) && !req.contains(&"drie".to_string()));
        let fr = TABLES.for_language(Some("fr"));
        let req = required_words("alors euh on se voit mardi non attends mercredi", fr);
        assert!(req.contains(&"mercredi".to_string()) && !req.contains(&"mardi".to_string()));
        assert!(!req.contains(&"alors".to_string()));
    }

    #[test]
    fn unspaced_scripts() {
        assert!(is_unspaced_script("東京駅で待ち合わせしましょう。"));
        assert!(is_unspaced_script("我们明天见"));
        assert!(!is_unspaced_script("See you tomorrow"));
        assert!(!is_unspaced_script("내일 봐요"));
    }
}
