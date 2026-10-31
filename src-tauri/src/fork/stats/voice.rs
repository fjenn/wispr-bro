// SPDX-License-Identifier: GPL-3.0-or-later
//! Insights › Your voice (S-25, C-57): the voice profile, computed on this
//! Mac from the user's own history. Locked until [`VOICE_PROFILE_WORDS`]
//! lifetime words (DEC-50, the same count as the Home bar); then four cards:
//!
//! - **Your phrase:** the 3-5 word phrase used in the most dictations
//!   (each dictation counts once), at least [`MIN_PHRASE_DICTATIONS`];
//!   score = dictations × words, so "let me know if you" (4 dictations)
//!   beats "let me know" (5). A phrase never crosses punctuation and is
//!   never only hesitations or fillers.
//! - **Peak time and app:** the local hour of day with the most words, and
//!   the app with the most words. Wispr shows a place; that needs location,
//!   so the app stands in (R2 C-57).
//! - **Most used word:** the most frequent word that is not a stopword
//!   (stopwords-iso, MIT data, for the languages in the history plus
//!   English), a hesitation or filler (S-12a's tables), a number or a
//!   single letter.
//! - **Most corrected word:** the replacement the app made most often
//!   between what was heard (`entry_meta.asr_text`) and what was inserted
//!   (`final_text`): dictionary rules, Handy's word fixes and AI cleanup
//!   alike, as word-level substitutions of 1-3 words each side (removed
//!   fillers are not corrections). Plus `corrections` rows a later fix wrote
//!   without changing the text (S-15b flags, S-34 auto-learn). S-12a left
//!   LLM edits out of `corrections`; S-25 reads them from the two texts
//!   instead, so the write path stays as it is.
//!
//! The profile reads the newest [`MAX_ENTRIES`] counted dictations still in
//! History (text retention can remove older ones; the stats store keeps the
//! totals). Pure functions; `fork::db::insights` reads the rows.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use chrono::{DateTime, TimeZone, Timelike};
use serde::{Deserialize, Serialize};
use specta::Type;
use unicode_segmentation::UnicodeSegmentation;

/// Lifetime words that unlock the voice profile (DEC-50).
pub const VOICE_PROFILE_WORDS: u64 = 1_700;

/// Newest counted dictations the profile reads.
pub const MAX_ENTRIES: usize = 10_000;

/// A phrase must appear in at least this many dictations.
pub const MIN_PHRASE_DICTATIONS: u32 = 3;

/// Phrase lengths considered, in words.
const PHRASE_WORDS: std::ops::RangeInclusive<usize> = 3..=5;

/// Longest side of a substitution that counts as a correction, in words.
const MAX_CORRECTION_WORDS: usize = 3;

/// Texts longer than this (words) are not diffed for corrections.
const MAX_DIFF_WORDS: usize = 600;

/// One counted dictation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VoiceEntry {
    /// Unix ms at record start.
    pub created_at: i64,
    pub final_text: String,
    /// Verbatim ASR text (D-06).
    pub asr_text: Option<String>,
    pub app_name: Option<String>,
    pub word_count: u64,
    pub language: Option<String>,
}

/// A `corrections` row that is not visible in the texts (flag, learn).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoggedCorrection {
    pub from_text: String,
    pub to_text: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct PhraseCard {
    pub phrase: String,
    /// Dictations that contain it.
    pub dictations: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct PeakCard {
    /// Local hour of day, 0-23 (the hour from `hour:00` to `hour+1:00`).
    pub hour: u8,
    pub hour_words: u64,
    /// The hour's share of the words read, 0..=1.
    pub hour_share: f64,
    /// The app with the most words (`None` when no app was recorded).
    pub app_name: Option<String>,
    pub app_words: u64,
    pub app_share: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct WordCard {
    pub word: String,
    pub uses: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct CorrectedCard {
    /// What the recognizer heard.
    pub heard: String,
    /// What was written instead.
    pub written: String,
    pub times: u32,
}

/// Everything on the Voice tab.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct VoiceProfile {
    pub unlocked: bool,
    /// Lifetime words (`daily_stats`).
    pub total_words: u64,
    /// [`VOICE_PROFILE_WORDS`].
    pub goal: u64,
    /// Dictations the cards were computed from (0 while locked).
    pub dictations_read: u32,
    pub phrase: Option<PhraseCard>,
    pub peak: Option<PeakCard>,
    pub word: Option<WordCard>,
    pub corrected: Option<CorrectedCard>,
}

/// stopwords-iso 1.1.0 (MIT, `tables/stopwords-iso.json`, unmodified).
pub(crate) fn stopword_table() -> &'static HashMap<String, HashSet<String>> {
    static SW: OnceLock<HashMap<String, HashSet<String>>> = OnceLock::new();
    SW.get_or_init(|| {
        let raw: HashMap<String, Vec<String>> =
            serde_json::from_str(include_str!("tables/stopwords-iso.json")).unwrap_or_default();
        raw.into_iter()
            .map(|(k, v)| (k, v.into_iter().map(|w| w.to_lowercase()).collect()))
            .collect()
    })
}

/// Hesitations and fillers of every language (S-12a `markers.json`):
/// single words, and the multi-word phrases.
fn fillers() -> &'static (HashSet<String>, Vec<Vec<String>>) {
    static F: OnceLock<(HashSet<String>, Vec<Vec<String>>)> = OnceLock::new();
    F.get_or_init(|| {
        let all = crate::fork::llm::markers::TABLES.for_language(None);
        let mut single = HashSet::new();
        let mut multi = Vec::new();
        for p in all.hesitations.iter().chain(all.fillers.iter()) {
            if p.len() == 1 {
                single.insert(p[0].clone());
            } else {
                multi.push(p.clone());
            }
        }
        (single, multi)
    })
}

/// ISO 639-1 part of a language tag (`zh-Hans` → `zh`).
fn primary(lang: &str) -> String {
    lang.split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// The stopword set for the languages in the history, English always in.
fn stopword_set(entries: &[VoiceEntry]) -> HashSet<&'static str> {
    let mut langs: HashSet<String> = entries
        .iter()
        .filter_map(|e| e.language.as_deref())
        .map(primary)
        .collect();
    langs.insert("en".into());
    let sw = stopword_table();
    langs
        .iter()
        .filter_map(|l| sw.get(l))
        .flat_map(|set| set.iter().map(String::as_str))
        .collect()
}

/// A word of a text: its comparison key and its text as written.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Token<'a> {
    key: String,
    text: &'a str,
}

fn key(word: &str) -> String {
    word.replace(['\u{2019}', '\u{2018}'], "'").to_lowercase()
}

/// The text's clauses (split at punctuation and line breaks), each a list
/// of words.
fn clauses(text: &str) -> Vec<Vec<Token<'_>>> {
    let mut out = Vec::new();
    let mut cur = Vec::new();
    for seg in text.split_word_bounds() {
        if seg.chars().any(char::is_alphanumeric) {
            cur.push(Token {
                key: key(seg),
                text: seg,
            });
        } else if seg.chars().all(|c| c.is_whitespace() && c != '\n') {
            continue;
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn words(text: &str) -> Vec<Token<'_>> {
    clauses(text).into_iter().flatten().collect()
}

/// The most frequent written form of each key.
#[derive(Default)]
struct Forms(HashMap<String, HashMap<String, u32>>);

impl Forms {
    fn add(&mut self, key: &str, form: &str) {
        *self
            .0
            .entry(key.to_owned())
            .or_default()
            .entry(form.to_owned())
            .or_insert(0) += 1;
    }

    fn best(&self, key: &str) -> String {
        self.0
            .get(key)
            .and_then(|forms| {
                forms
                    .iter()
                    .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
                    .map(|(f, _)| f.clone())
            })
            .unwrap_or_else(|| key.to_owned())
    }
}

fn is_filler_word(k: &str) -> bool {
    fillers().0.contains(k)
}

/// A phrase made only of hesitations and fillers, or that starts or ends
/// with a hesitation or filler.
fn is_filler_phrase(keys: &[&str]) -> bool {
    if keys.iter().all(|k| is_filler_word(k)) {
        return true;
    }
    if is_filler_word(keys[0]) || is_filler_word(keys[keys.len() - 1]) {
        return true;
    }
    fillers().1.iter().any(|f| {
        f.len() <= keys.len()
            && (keys[..f.len()].iter().zip(f).all(|(a, b)| *a == b)
                || keys[keys.len() - f.len()..]
                    .iter()
                    .zip(f)
                    .all(|(a, b)| *a == b))
    })
}

/// "Your phrase".
pub fn phrase(entries: &[VoiceEntry]) -> Option<PhraseCard> {
    // Words are interned so an n-gram is a small array, not a String: the
    // profile reads up to 10,000 dictations.
    type Gram = (u8, [u32; 5]);
    let mut vocab: HashMap<String, u32> = HashMap::new();
    let mut names: Vec<String> = Vec::new();
    let mut docs: HashMap<Gram, u32> = HashMap::new();
    let mut seen: HashSet<Gram> = HashSet::new();
    for e in entries {
        seen.clear();
        for clause in clauses(&e.final_text) {
            let ids: Vec<u32> = clause
                .iter()
                .map(|t| {
                    *vocab.entry(t.key.clone()).or_insert_with(|| {
                        names.push(t.key.clone());
                        (names.len() - 1) as u32
                    })
                })
                .collect();
            for n in PHRASE_WORDS {
                for w in ids.windows(n) {
                    let mut gram: Gram = (n as u8, [u32::MAX; 5]);
                    gram.1[..n].copy_from_slice(w);
                    if seen.insert(gram) {
                        *docs.entry(gram).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    let text = |g: &Gram| -> Vec<&str> {
        g.1[..usize::from(g.0)]
            .iter()
            .map(|&i| names[i as usize].as_str())
            .collect()
    };
    let (best, dictations) = docs
        .into_iter()
        .filter(|(g, d)| *d >= MIN_PHRASE_DICTATIONS && !is_filler_phrase(&text(g)))
        .map(|(g, d)| (text(&g).join(" "), g.0, d))
        .max_by(|(ka, na, da), (kb, nb, db)| {
            (*da as usize * usize::from(*na))
                .cmp(&(*db as usize * usize::from(*nb)))
                .then_with(|| na.cmp(nb))
                .then_with(|| kb.cmp(ka))
        })
        .map(|(k, _, d)| (k, d))?;
    // The written form most used for the winner.
    let n = best.split(' ').count();
    let mut forms = Forms::default();
    for e in entries {
        for clause in clauses(&e.final_text) {
            for w in clause.windows(n) {
                if w.iter().map(|t| t.key.as_str()).eq(best.split(' ')) {
                    let form = w.iter().map(|t| t.text).collect::<Vec<_>>().join(" ");
                    forms.add(&best, &form);
                }
            }
        }
    }
    Some(PhraseCard {
        phrase: forms.best(&best),
        dictations,
    })
}

/// The local hour (0-23) of a Unix ms time in `tz`.
fn hour_in<Tz: TimeZone>(unix_ms: i64, tz: &Tz) -> u8 {
    let utc = DateTime::from_timestamp_millis(unix_ms).unwrap_or_default();
    utc.with_timezone(tz).hour() as u8
}

/// "Peak time and app", hours in `tz`.
pub fn peak<Tz: TimeZone>(entries: &[VoiceEntry], tz: &Tz) -> Option<PeakCard> {
    let total: u64 = entries.iter().map(|e| e.word_count).sum();
    if total == 0 {
        return None;
    }
    let mut hours = [0u64; 24];
    let mut apps: HashMap<&str, u64> = HashMap::new();
    for e in entries {
        hours[usize::from(hour_in(e.created_at, tz))] += e.word_count;
        if let Some(app) = e.app_name.as_deref().filter(|a| !a.trim().is_empty()) {
            *apps.entry(app).or_insert(0) += e.word_count;
        }
    }
    // Earliest hour wins a tie.
    let (hour, hour_words) =
        hours.iter().enumerate().fold(
            (0usize, 0u64),
            |best, (h, &w)| {
                if w > best.1 {
                    (h, w)
                } else {
                    best
                }
            },
        );
    let app = apps
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(a.0)));
    Some(PeakCard {
        hour: hour as u8,
        hour_words,
        hour_share: hour_words as f64 / total as f64,
        app_name: app.map(|(a, _)| a.to_owned()),
        app_words: app.map_or(0, |(_, w)| w),
        app_share: app.map_or(0.0, |(_, w)| w as f64 / total as f64),
    })
}

/// "Most used word".
pub fn most_used_word(entries: &[VoiceEntry]) -> Option<WordCard> {
    let stop = stopword_set(entries);
    let mut counts: HashMap<String, u32> = HashMap::new();
    for e in entries {
        for t in words(&e.final_text) {
            if let Some(n) = counts.get_mut(&t.key) {
                *n += 1;
                continue;
            }
            let k = t.key.as_str();
            if k.chars().count() < 2
                || !k.chars().any(char::is_alphabetic)
                || stop.contains(k)
                || is_filler_word(k)
            {
                continue;
            }
            counts.insert(t.key, 1);
        }
    }
    let (best, uses) = counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))?;
    let mut forms = Forms::default();
    for e in entries {
        for t in words(&e.final_text).iter().filter(|t| t.key == best) {
            forms.add(&best, t.text);
        }
    }
    Some(WordCard {
        word: forms.best(&best),
        uses,
    })
}

/// Word-level substitutions from `heard` to `written` (both sides 1-3
/// words, compared case-insensitively), in order.
pub fn substitutions(heard: &str, written: &str) -> Vec<(String, String)> {
    let a = words(heard);
    let b = words(written);
    if a.len() > MAX_DIFF_WORDS || b.len() > MAX_DIFF_WORDS {
        return Vec::new();
    }
    let (n, m) = (a.len(), b.len());
    let mut t = vec![vec![0u16; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            t[i][j] = if a[i].key == b[j].key {
                t[i + 1][j + 1] + 1
            } else {
                t[i + 1][j].max(t[i][j + 1])
            };
        }
    }
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    let (mut del, mut ins): (Vec<&Token>, Vec<&Token>) = (Vec::new(), Vec::new());
    let mut flush = |del: &mut Vec<&Token>, ins: &mut Vec<&Token>| {
        let del_content: Vec<&&Token> = del.iter().filter(|t| !is_filler_word(&t.key)).collect();
        if !del_content.is_empty()
            && !ins.is_empty()
            && del.len() <= MAX_CORRECTION_WORDS
            && ins.len() <= MAX_CORRECTION_WORDS
        {
            let from = del_content
                .iter()
                .map(|t| t.key.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let to = ins.iter().map(|t| t.text).collect::<Vec<_>>().join(" ");
            out.push((from, to));
        }
        del.clear();
        ins.clear();
    };
    while i < n || j < m {
        if i < n && j < m && a[i].key == b[j].key {
            flush(&mut del, &mut ins);
            i += 1;
            j += 1;
        } else if j >= m || (i < n && t[i + 1][j] >= t[i][j + 1]) {
            del.push(&a[i]);
            i += 1;
        } else {
            ins.push(&b[j]);
            j += 1;
        }
    }
    flush(&mut del, &mut ins);
    out
}

/// "Most corrected word".
pub fn most_corrected(
    entries: &[VoiceEntry],
    logged: &[LoggedCorrection],
) -> Option<CorrectedCard> {
    let mut counts: HashMap<(String, String), u32> = HashMap::new();
    let mut forms = Forms::default();
    let mut add = |from: String, to: &str| {
        let to_key = key(to);
        if from == to_key || from.is_empty() || to_key.is_empty() {
            return;
        }
        forms.add(&to_key, to);
        *counts.entry((from, to_key)).or_insert(0) += 1;
    };
    for e in entries {
        let Some(heard) = e.asr_text.as_deref() else {
            continue;
        };
        if heard == e.final_text {
            continue;
        }
        for (from, to) in substitutions(heard, &e.final_text) {
            add(from, &to);
        }
    }
    for c in logged {
        add(key(c.from_text.trim()), c.to_text.trim());
    }
    counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
        .map(|((heard, written), times)| CorrectedCard {
            heard,
            written: forms.best(&written),
            times,
        })
}

/// The Voice tab. `entries` newest first, at most [`MAX_ENTRIES`];
/// `total_words` is the lifetime count from `daily_stats`.
pub fn profile<Tz: TimeZone>(
    total_words: u64,
    entries: &[VoiceEntry],
    logged: &[LoggedCorrection],
    tz: &Tz,
) -> VoiceProfile {
    let unlocked = total_words >= VOICE_PROFILE_WORDS;
    if !unlocked {
        return VoiceProfile {
            unlocked,
            total_words,
            goal: VOICE_PROFILE_WORDS,
            dictations_read: 0,
            phrase: None,
            peak: None,
            word: None,
            corrected: None,
        };
    }
    let entries = &entries[..entries.len().min(MAX_ENTRIES)];
    VoiceProfile {
        unlocked,
        total_words,
        goal: VOICE_PROFILE_WORDS,
        dictations_read: u32::try_from(entries.len()).unwrap_or(u32::MAX),
        phrase: phrase(entries),
        peak: peak(entries, tz),
        word: most_used_word(entries),
        corrected: most_corrected(entries, logged),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    fn e(at_hour_utc: i64, text: &str, app: Option<&str>) -> VoiceEntry {
        VoiceEntry {
            // 2026-10-30T00:00:00Z + hours
            created_at: 1_793_318_400_000 + at_hour_utc * 3_600_000,
            final_text: text.into(),
            asr_text: None,
            app_name: app.map(str::to_owned),
            word_count: crate::fork::stats::wordcount::word_count(text) as u64,
            language: Some("en".into()),
        }
    }

    fn utc() -> FixedOffset {
        FixedOffset::east_opt(0).unwrap()
    }

    #[test]
    fn stopword_data_loads() {
        let sw = stopword_table();
        assert_eq!(sw.len(), 58);
        assert!(sw["en"].contains("the"));
        assert!(sw["de"].contains("und"));
        assert!(sw["nl"].contains("het"));
    }

    #[test]
    fn locked_below_the_goal_and_computes_nothing() {
        let rows = vec![e(9, "Let me know if that works for you.", Some("Mail")); 5];
        let p = profile(1_699, &rows, &[], &utc());
        assert!(!p.unlocked);
        assert_eq!(p.goal, 1_700);
        assert_eq!(p.dictations_read, 0);
        assert!(p.phrase.is_none() && p.peak.is_none());
        assert!(p.word.is_none() && p.corrected.is_none());
        assert!(profile(1_700, &rows, &[], &utc()).unlocked);
    }

    #[test]
    fn phrase_counts_dictations_not_repeats_and_prefers_longer() {
        let rows = vec![
            e(9, "Let me know if you need anything.", None),
            e(9, "let me know if you can make it", None),
            e(
                9,
                "Let me know if you agree. Let me know if you agree.",
                None,
            ),
            e(9, "Let me know, thanks.", None),
            e(9, "Please let me know.", None),
        ];
        // "let me know" is in 5 dictations (score 15); "let me know if you"
        // in 3 (score 15) and longer, so it wins; the repeat in row 3 counts once.
        let p = phrase(&rows).unwrap();
        assert_eq!(p.phrase, "Let me know if you");
        assert_eq!(p.dictations, 3);
    }

    #[test]
    fn phrase_needs_three_dictations_and_skips_fillers() {
        let rows = vec![
            e(9, "Um, you know, I mean.", None),
            e(9, "um you know I mean", None),
            e(9, "Um you know I mean so", None),
        ];
        assert_eq!(phrase(&rows), None);
        let two = vec![e(9, "see you at noon", None), e(9, "see you at noon", None)];
        assert_eq!(phrase(&two), None);
    }

    #[test]
    fn phrase_does_not_cross_punctuation() {
        let rows = vec![e(9, "Done. Next step is review.", None); 3];
        let p = phrase(&rows).unwrap();
        assert_eq!(p.phrase, "Next step is review");
    }

    #[test]
    fn peak_hour_and_app_by_words() {
        let rows = vec![
            e(9, "one two three four", Some("Notes")),
            e(9, "one two three four five six", Some("Mail")),
            e(14, "one two three four five six seven eight", Some("Notes")),
            e(14, "x", None),
        ];
        let p = peak(&rows, &utc()).unwrap();
        // 09:00 has 10 words, 14:00 has 9.
        assert_eq!((p.hour, p.hour_words), (9, 10));
        assert!((p.hour_share - 10.0 / 19.0).abs() < 1e-9);
        assert_eq!(p.app_name.as_deref(), Some("Notes"));
        assert_eq!(p.app_words, 12);
        // In UTC+2 the same takes are at 11:00.
        assert_eq!(
            peak(&rows, &FixedOffset::east_opt(7200).unwrap())
                .unwrap()
                .hour,
            11
        );
        assert_eq!(peak(&[], &utc()), None);
    }

    #[test]
    fn most_used_word_skips_stopwords_fillers_numbers() {
        let rows = vec![
            e(9, "The roadmap is the plan, um, the roadmap.", None),
            e(
                9,
                "Roadmap review at 10 10 10 10. Okay okay okay okay.",
                None,
            ),
            e(9, "the the the the the", None),
        ];
        let w = most_used_word(&rows).unwrap();
        assert_eq!(w.word, "roadmap");
        assert_eq!(w.uses, 3);
    }

    #[test]
    fn substitutions_are_word_level_and_ignore_filler_removal() {
        assert_eq!(
            substitutions(
                "um I'm gonna call Sioban tomorrow",
                "I'm going to call Siobhán tomorrow."
            ),
            vec![
                ("gonna".to_string(), "going to".to_string()),
                ("sioban".to_string(), "Siobhán".to_string()),
            ]
        );
        // Removing a filler or changing only case/punctuation is no correction.
        assert!(substitutions("so um hello there", "So hello there.").is_empty());
        // Long rewrites are not word corrections.
        assert!(substitutions("a b c d e", "v w x y z").is_empty());
    }

    #[test]
    fn most_corrected_merges_texts_and_logged_rows() {
        let mut a = e(9, "I'm going to ship the Kubernetes fix.", None);
        a.asr_text = Some("I'm gonna ship the cooper netties fix".into());
        let mut b = e(9, "We are going to meet.", None);
        b.asr_text = Some("We are gonna meet.".into());
        let logged = [LoggedCorrection {
            from_text: "Gonna".into(),
            to_text: "going to".into(),
        }];
        let c = most_corrected(&[a, b], &logged).unwrap();
        assert_eq!(
            (c.heard.as_str(), c.written.as_str(), c.times),
            ("gonna", "going to", 3)
        );
        assert_eq!(most_corrected(&[e(9, "same", None)], &[]), None);
    }

    /// The hand-computed fixture of the exit criterion (R5 S-25).
    #[test]
    fn profile_matches_a_hand_computed_fixture() {
        let mut rows = Vec::new();
        for i in 0..4 {
            rows.push(e(
                15,
                "Thanks for the update, I will review the proposal tonight.",
                Some("Mail"),
            ));
            let mut r = e(
                10 + i,
                "Can we move the proposal review to Friday?",
                Some("Slack"),
            );
            r.asr_text = Some("Can we move the proposal review to friday".into());
            rows.push(r);
        }
        let mut fixed = e(15, "The Kubernetes rollout went well.", Some("Notes"));
        fixed.asr_text = Some("The cooper netties rollout went well.".into());
        rows.push(fixed);
        let p = profile(2_000, &rows, &[], &utc());
        assert!(p.unlocked);
        assert_eq!(p.dictations_read, 9);
        // 4 dictations × 5 words: "I will review the proposal" (and others of
        // the same score); the longest wins, then alphabetical order.
        let phrase = p.phrase.unwrap();
        assert_eq!(phrase.dictations, 4);
        assert_eq!(phrase.phrase.split(' ').count(), 5);
        // 15:00 UTC: 4 × 10 words + 5 = 45 of 77 words.
        let peak = p.peak.unwrap();
        assert_eq!((peak.hour, peak.hour_words), (15, 45));
        assert_eq!(peak.app_name.as_deref(), Some("Mail"));
        assert_eq!(peak.app_words, 40);
        // "proposal" 8 times, "review" 8 times: alphabetical tie-break.
        let word = p.word.unwrap();
        assert_eq!((word.word.as_str(), word.uses), ("proposal", 8));
        let c = p.corrected.unwrap();
        assert_eq!(
            (c.heard.as_str(), c.written.as_str(), c.times),
            ("cooper netties", "Kubernetes", 1)
        );
    }

    /// A full profile ([`MAX_ENTRIES`] dictations of a realistic length)
    /// stays well inside a page load, even in a debug build.
    #[test]
    fn full_profile_is_fast_enough() {
        let texts = [
            "Thanks for the notes, I am going to send the revised budget to the team by Friday.",
            "Can we move the design review to Thursday afternoon so everyone can join?",
            "Let me know if you need anything else before the launch next week.",
            "Remind me to call the dentist and pick up the package from the post office.",
        ];
        let rows: Vec<VoiceEntry> = (0..MAX_ENTRIES)
            .map(|i| {
                let text = texts[i % texts.len()];
                let mut r = e((i % 24) as i64, text, Some("Mail"));
                r.asr_text = Some(text.replace("going to", "gonna"));
                r
            })
            .collect();
        let t = std::time::Instant::now();
        let p = profile(200_000, &rows, &[], &utc());
        let took = t.elapsed();
        assert!(p.phrase.is_some() && p.word.is_some());
        assert_eq!(p.corrected.unwrap().written, "going to");
        assert!(took < std::time::Duration::from_secs(5), "took {took:?}");
        eprintln!("voice profile over {} dictations: {took:?}", rows.len());
    }
}
