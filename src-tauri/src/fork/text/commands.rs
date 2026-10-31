// SPDX-License-Identifier: GPL-3.0-or-later
//! Stage 7 of the text pipeline (R5 §4.10): voice commands at clause edges.
//!
//! | Command        | Effect                                              |
//! | -------------- | --------------------------------------------------- |
//! | new line       | `\n`; the next word starts with a capital           |
//! | new paragraph  | `\n\n`; the next word starts with a capital         |
//! | scratch that   | deletes the clause before it ([`super::scratch`])   |
//! | question mark… | the mark, replacing the recognizer's punctuation    |
//!
//! The phrases live in `tables/commands.<lang>.json` (one table per
//! language; the spoken punctuation phrases and literal prefixes are
//! MacParakeet's data). The clause-edge rule keeps ordinary words ordinary:
//!
//! - line, paragraph and scratch commands need a clause edge on **both**
//!   sides (start or end of the text, a line break, clause punctuation, or
//!   another command), so "a new line of credit" and "I need a new line."
//!   stay text, while "Thanks, new line, Finn" or "Hello. New paragraph.
//!   How are you?" are commands. A recognizer puts punctuation where the
//!   speaker paused, which is how a spoken command is said;
//! - punctuation commands need only the right edge (they close a clause)
//!   and must not follow a blocker word ("what is a question mark?" stays);
//! - in scripts without spaces (Chinese, Japanese) only the right edge is
//!   checked: the next character must not be a letter (MacParakeet's rule,
//!   so 请问号码 stays);
//! - a literal prefix ("literally new line") keeps the phrase as words and
//!   drops the prefix.
//!
//! The commands run once over the whole text, left to right; what a command
//! inserts is never scanned again. Newlines are only ever added.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use serde::Deserialize;

use super::fold::{fold_str, is_spaceless, is_word_char, whole_words, Folded};
use super::scratch;

/// The tables shipped with the app, by language.
const TABLES: &[(&str, &str)] = &[
    ("de", include_str!("tables/commands.de.json")),
    ("en", include_str!("tables/commands.en.json")),
    ("es", include_str!("tables/commands.es.json")),
    ("fr", include_str!("tables/commands.fr.json")),
    ("it", include_str!("tables/commands.it.json")),
    ("ja", include_str!("tables/commands.ja.json")),
    ("pl", include_str!("tables/commands.pl.json")),
    ("pt", include_str!("tables/commands.pt.json")),
    ("zh", include_str!("tables/commands.zh.json")),
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Table {
    pub language: String,
    pub new_line: Vec<String>,
    pub new_paragraph: Vec<String>,
    pub scratch_that: Vec<String>,
    pub punctuation: Vec<(String, String)>,
    pub literal: Vec<String>,
    pub blockers: Vec<String>,
}

/// What a phrase does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Break(&'static str),
    Scratch,
    Mark(String),
}

#[derive(Debug)]
struct Phrase {
    kind: Kind,
    /// Written in a script without spaces: only the right edge counts.
    spaceless: bool,
}

/// The compiled commands of one or more languages.
#[derive(Debug)]
pub struct Matcher {
    automaton: Option<AhoCorasick>,
    phrases: Vec<Phrase>,
    literal: Vec<String>,
    blockers: HashSet<String>,
}

/// How many commands one pass applied.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Applied {
    /// Line breaks, paragraphs, marks and literal escapes.
    pub commands: u32,
    /// "scratch that" deletions.
    pub scratches: u32,
}

/// Every table, parsed once.
pub fn tables() -> &'static [Table] {
    static PARSED: OnceLock<Vec<Table>> = OnceLock::new();
    PARSED.get_or_init(|| {
        TABLES
            .iter()
            .filter_map(|(lang, json)| match serde_json::from_str::<Table>(json) {
                Ok(t) => Some(t),
                Err(e) => {
                    log::error!("text: commands table {lang} is invalid: {e}");
                    None
                }
            })
            .collect()
    })
}

impl Matcher {
    fn new<'a>(tables: impl IntoIterator<Item = &'a Table>) -> Matcher {
        let mut patterns = Vec::new();
        let mut phrases = Vec::new();
        let mut literal = Vec::new();
        let mut blockers = HashSet::new();
        let mut add = |phrase: &str, kind: Kind| {
            let folded = fold_str(phrase.trim());
            if folded.is_empty() {
                return;
            }
            phrases.push(Phrase {
                spaceless: folded.chars().any(is_spaceless),
                kind,
            });
            patterns.push(folded);
        };
        for t in tables {
            for p in &t.new_line {
                add(p, Kind::Break("\n"));
            }
            for p in &t.new_paragraph {
                add(p, Kind::Break("\n\n"));
            }
            for p in &t.scratch_that {
                add(p, Kind::Scratch);
            }
            for (p, mark) in &t.punctuation {
                add(p, Kind::Mark(mark.clone()));
            }
            literal.extend(t.literal.iter().map(|w| fold_str(w)));
            blockers.extend(t.blockers.iter().map(|w| fold_str(w)));
        }
        let automaton = if patterns.is_empty() {
            None
        } else {
            AhoCorasickBuilder::new()
                .match_kind(MatchKind::Standard)
                .build(&patterns)
                .map_err(|e| log::error!("text: could not compile the voice commands: {e}"))
                .ok()
        };
        Matcher {
            automaton,
            phrases,
            literal,
            blockers,
        }
    }

    /// The commands for an output language: its own table; every table when
    /// the language is unknown; none for a language without a table.
    pub fn for_language(language: Option<&str>) -> Arc<Matcher> {
        static CACHE: OnceLock<HashMap<String, Arc<Matcher>>> = OnceLock::new();
        let cache = CACHE.get_or_init(|| {
            let mut m: HashMap<String, Arc<Matcher>> = tables()
                .iter()
                .map(|t| (t.language.clone(), Arc::new(Matcher::new([t]))))
                .collect();
            m.insert(String::new(), Arc::new(Matcher::new(tables())));
            m
        });
        let key = language
            .map(|l| {
                l.split(['-', '_'])
                    .next()
                    .unwrap_or_default()
                    .to_ascii_lowercase()
            })
            .unwrap_or_default();
        cache.get(&key).cloned().unwrap_or_else(|| {
            static NONE: OnceLock<Arc<Matcher>> = OnceLock::new();
            NONE.get_or_init(|| Arc::new(Matcher::new([]))).clone()
        })
    }

    pub fn is_empty(&self) -> bool {
        self.automaton.is_none()
    }
}

/// Punctuation that ends a clause (an edge for the commands).
pub fn is_clause_punct(c: char) -> bool {
    matches!(
        c,
        ',' | ';'
            | ':'
            | '.'
            | '!'
            | '?'
            | '…'
            | '—'
            | '–'
            | '，'
            | '。'
            | '！'
            | '？'
            | '；'
            | '：'
            | '、'
    )
}

/// Punctuation a recognizer puts around a spoken command, consumed with it.
pub fn is_engine_punct(c: char) -> bool {
    matches!(
        c,
        ',' | '.' | ';' | ':' | '!' | '?' | '…' | '，' | '。' | '！' | '？' | '；' | '：' | '、'
    )
}

fn is_blank(c: char) -> bool {
    c == ' ' || c == '\t'
}

/// Uppercases the first character of `s` if it is a lowercase letter.
pub fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_lowercase() => c.to_uppercase().chain(chars).collect(),
        _ => s.to_string(),
    }
}

#[derive(Clone, Copy, Debug)]
struct Hit {
    start: usize,
    end: usize,
    phrase: usize,
}

/// Every phrase occurrence in `text` that stands on word boundaries,
/// leftmost first, then longest.
fn hits(m: &Matcher, text: &str) -> Vec<Hit> {
    let Some(automaton) = &m.automaton else {
        return Vec::new();
    };
    let folded = Folded::new(text);
    let mut out: Vec<Hit> = automaton
        .find_overlapping_iter(&folded.text)
        .filter_map(|x| {
            let (start, end) = folded.original_span(x.start(), x.end())?;
            let phrase = x.pattern().as_usize();
            let bounded = if m.phrases[phrase].spaceless {
                // MacParakeet: the next character must not be a letter or digit.
                text[end..]
                    .chars()
                    .next()
                    .is_none_or(|c| !c.is_alphanumeric())
            } else {
                whole_words(text, start, end)
            };
            bounded.then_some(Hit { start, end, phrase })
        })
        .collect();
    out.sort_by(|a, b| {
        a.start
            .cmp(&b.start)
            .then((b.end - b.start).cmp(&(a.end - a.start)))
    });
    out
}

/// The word right before `at` (after skipping blanks), with its start.
fn word_before(text: &str, at: usize) -> Option<(usize, &str)> {
    let head = text[..at].trim_end_matches(is_blank);
    if head.len() == at {
        // Not separated by a blank: no separate word.
        return None;
    }
    let start = head
        .char_indices()
        .rev()
        .find(|(_, c)| !(is_word_char(*c) || *c == '\''))
        .map_or(0, |(i, c)| i + c.len_utf8());
    let word = &head[start..];
    (!word.is_empty()).then_some((start, word))
}

fn left_edge(text: &str, at: usize) -> bool {
    let head = text[..at].trim_end_matches(is_blank);
    head.chars()
        .next_back()
        .is_none_or(|c| c == '\n' || is_clause_punct(c))
}

fn right_edge(text: &str, at: usize, next_is_command: bool) -> bool {
    let tail = text[at..].trim_start_matches(is_blank);
    next_is_command
        || tail
            .chars()
            .next()
            .is_none_or(|c| c == '\n' || is_clause_punct(c))
}

/// Applies the commands to `text`.
pub fn apply(m: &Matcher, text: &str) -> (String, Applied) {
    let mut text = text.to_string();
    let mut applied = Applied::default();
    let mut cursor = 0usize;
    'scan: loop {
        let all = hits(m, &text);
        let pending: Vec<Hit> = all.iter().copied().filter(|h| h.start >= cursor).collect();
        for (i, hit) in pending.iter().enumerate() {
            let phrase = &m.phrases[hit.phrase];

            // "literally new line": keep the words, drop the prefix.
            if !phrase.spaceless {
                if let Some((wstart, word)) = word_before(&text, hit.start) {
                    if m.literal.iter().any(|l| fold_str(word) == *l) {
                        let kept = text[hit.start..hit.end].to_string();
                        let next = format!("{}{}{}", &text[..wstart], kept, &text[hit.end..]);
                        cursor = wstart + kept.len();
                        text = next;
                        applied.commands += 1;
                        continue 'scan;
                    }
                }
            }

            let after_ws = hit.end
                + (text[hit.end..].len() - text[hit.end..].trim_start_matches(is_blank).len());
            let next_is_command = pending[i + 1..].iter().any(|h| h.start == after_ws);
            let fires = match &phrase.kind {
                _ if phrase.spaceless => true,
                Kind::Break(_) | Kind::Scratch => {
                    left_edge(&text, hit.start) && right_edge(&text, hit.end, next_is_command)
                }
                Kind::Mark(_) => {
                    right_edge(&text, hit.end, next_is_command)
                        && !word_before(&text, hit.start)
                            .is_some_and(|(_, w)| m.blockers.contains(&fold_str(w)))
                }
            };
            if !fires {
                continue;
            }

            let (next, at) = match &phrase.kind {
                Kind::Break(b) => line_break(&text, hit.start, hit.end, b),
                Kind::Mark(mark) => punctuation(&text, hit.start, hit.end, mark),
                Kind::Scratch => scratch::apply(&text, hit.start, hit.end),
            };
            if phrase.kind == Kind::Scratch {
                applied.scratches += 1;
            } else {
                applied.commands += 1;
            }
            text = next;
            cursor = at;
            continue 'scan;
        }
        break;
    }
    (text, applied)
}

/// `before` + the break + the rest with its leading punctuation dropped and
/// its first letter capitalized. Returns the text and where scanning goes on.
fn line_break(text: &str, start: usize, end: usize, br: &str) -> (String, usize) {
    let before = text[..start].trim_end_matches(is_blank);
    let after = text[end..]
        .trim_start_matches(is_blank)
        .trim_start_matches(is_engine_punct)
        .trim_start_matches(is_blank);
    let at = before.len() + br.len();
    (format!("{before}{br}{}", capitalize_first(after)), at)
}

/// The mark replaces the phrase and the recognizer's punctuation around it
/// (MacParakeet: a comma before, any terminators after).
fn punctuation(text: &str, start: usize, end: usize, mark: &str) -> (String, usize) {
    let before = text[..start]
        .trim_end_matches(is_blank)
        .trim_end_matches([',', '.', ';', ':', '，', '。', '；', '：', '、'])
        .trim_end_matches(is_blank);
    let after = text[end..]
        .trim_start_matches(is_blank)
        .trim_start_matches(is_engine_punct)
        .trim_start_matches(is_blank);
    let fullwidth = !mark.is_ascii();
    let gap = if after.is_empty() || after.starts_with('\n') || fullwidth {
        ""
    } else {
        " "
    };
    let at = before.len() + mark.len();
    (format!("{before}{mark}{gap}{after}"), at)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn en(text: &str) -> String {
        apply(&Matcher::for_language(Some("en")), text).0
    }

    #[test]
    fn every_table_parses_and_is_unique() {
        assert_eq!(tables().len(), TABLES.len());
        for t in tables() {
            let mut seen = HashSet::new();
            for p in t
                .new_line
                .iter()
                .chain(&t.new_paragraph)
                .chain(&t.scratch_that)
                .chain(t.punctuation.iter().map(|(p, _)| p))
            {
                assert!(seen.insert(fold_str(p)), "{}: {p} twice", t.language);
                assert_eq!(p.trim(), p, "{}: {p:?} has outer spaces", t.language);
            }
        }
    }

    #[test]
    fn new_line_and_paragraph_at_clause_edges() {
        assert_eq!(en("Hello. New line. How are you?"), "Hello.\nHow are you?");
        assert_eq!(
            en("Hi Sarah, new paragraph, I wanted to ask."),
            "Hi Sarah,\n\nI wanted to ask."
        );
        assert_eq!(en("Thanks, new line, Finn"), "Thanks,\nFinn");
        assert_eq!(en("New line. Hello"), "\nHello");
        assert_eq!(en("Done, new line."), "Done,\n");
        assert_eq!(
            en("One. Next line. Two. Line break. Three"),
            "One.\nTwo.\nThree"
        );
    }

    #[test]
    fn ordinary_words_stay_text() {
        for s in [
            "We launched a new line of shoes.",
            "I need a new line.",
            "Hello new line how are you",
            "the period of time",
            "What is a question mark?",
            "Draw a question mark.",
            "Scratch that itch, please.",
            "It literally broke.",
        ] {
            assert_eq!(en(s), s);
        }
    }

    #[test]
    fn consecutive_commands() {
        assert_eq!(en("Title. New line new line. Body"), "Title.\n\nBody");
        assert_eq!(en("A, new paragraph, new paragraph, B"), "A,\n\n\n\nB");
    }

    #[test]
    fn spoken_punctuation_macparakeet_cases() {
        assert_eq!(en("are you coming question mark."), "are you coming?");
        assert_eq!(en("Are you coming question mark?"), "Are you coming?");
        assert_eq!(en("ship it exclamation mark"), "ship it!");
        assert_eq!(en("ship it exclamation point"), "ship it!");
        assert_eq!(en("hello, question mark"), "hello?");
        assert_eq!(en("hello question mark!"), "hello?");
        assert_eq!(en("Really question mark. Yes."), "Really? Yes.");
        let all = |s: &str| apply(&Matcher::for_language(None), s).0;
        assert_eq!(all("fertig fragezeichen"), "fertig?");
        assert_eq!(all("vamos signo de interrogación"), "vamos?");
        assert_eq!(all("c'est fait point d'exclamation"), "c'est fait!");
        assert_eq!(all("c’est fait point d’exclamation"), "c’est fait!");
        assert_eq!(all("pronto ponto de interrogação"), "pronto?");
        assert_eq!(all("koniec znak zapytania"), "koniec?");
        assert_eq!(all("你好问号"), "你好？");
        assert_eq!(all("完成感叹号"), "完成！");
        assert_eq!(all("请问号码是多少"), "请问号码是多少");
    }

    #[test]
    fn literal_escape_keeps_the_words() {
        assert_eq!(
            en("say literal question mark please"),
            "say question mark please"
        );
        assert_eq!(
            en("Type literally new line, then stop."),
            "Type new line, then stop."
        );
        assert_eq!(en("Literally scratch that."), "scratch that.");
        let de = |s: &str| apply(&Matcher::for_language(Some("de")), s).0;
        assert_eq!(de("wörtlich Fragezeichen"), "Fragezeichen");
    }

    #[test]
    fn languages_select_their_table() {
        let de = |s: &str| apply(&Matcher::for_language(Some("de-DE")), s).0;
        assert_eq!(de("Hallo, neue Zeile, wie geht's?"), "Hallo,\nWie geht's?");
        // English phrases are text in German output.
        assert_eq!(de("Hallo. New line. Ja"), "Hallo. New line. Ja");
        // A language without a table has no commands.
        let ru = apply(&Matcher::for_language(Some("ru")), "Привет. New line. Да");
        assert_eq!(ru.0, "Привет. New line. Да");
        assert!(Matcher::for_language(Some("ru")).is_empty());
        let ja = |s: &str| apply(&Matcher::for_language(Some("ja")), s).0;
        assert_eq!(ja("こんにちは。改行。元気です"), "こんにちは。\n元気です");
        assert_eq!(ja("改行してください"), "改行してください");
    }

    #[test]
    fn counts() {
        let (_, a) = apply(
            &Matcher::for_language(Some("en")),
            "A. New line. B question mark. C, scratch that.",
        );
        assert_eq!(a.commands, 2);
        assert_eq!(a.scratches, 1);
    }

    #[test]
    fn never_removes_a_newline() {
        for s in [
            "A\nnew line\nB",
            "x\n\nscratch that\ny",
            "q\nquestion mark\n",
        ] {
            let out = en(s);
            assert!(
                out.matches('\n').count() >= s.matches('\n').count(),
                "{s:?} → {out:?}"
            );
        }
    }
}
