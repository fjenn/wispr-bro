// SPDX-License-Identifier: GPL-3.0-or-later
//
// Ported checks (each marked below; docs/fork/PORTS.md, NOTICE §3):
//
// - SpeakoFlow `src-tauri/src/actions.rs` at
//   9818a17fcd124e5f1ea6d34780ddea9b154322d8 (https://github.com/AbhishekBarali/SpeakoFlow):
//   `transcription_allows_empty_output`, `is_leaked_channel_name`,
//   `echo_words`, `prompt_echo`, `is_implausibly_long`.
//   Copyright (c) 2026 Abhishek Barali, Copyright (c) 2025 CJ Pais.
//   Licence: MIT (LICENSES/MIT-SpeakoFlow.txt).
// - FreeFlow `Sources/TranscriptTextCore.swift` at
//   4ebd9366e9e2920928c65c879185e5f98af38806 (https://github.com/zachlatta/freeflow):
//   `appearsToHaveExecutedInstruction`, `significantTokens`.
//   Copyright (c) 2026 Zach Latta. Licence: MIT (LICENSES/MIT-FreeFlow.txt).
// - BetterWispr `Sources/BetterWisprCore/Transcription/TranscriptPolisher.swift`
//   at 304335a073a98f7439e5f7b4fbc2c31b810b807e (https://github.com/opennookorg/betterwispr):
//   `accepted()`. That file adapts VoiceInk (https://github.com/Beingpax/VoiceInk,
//   c09cc1f677f40f2ee665843a61f07670210f012e) and is GPL-3.0.
// - Muesli `native/MuesliNative/Sources/MuesliNativeApp/Qwen3PostProcessor.swift`
//   at e1b52117ffb1ab6980f7266b72d9cb9e706a5afb (https://github.com/Muesli-HQ/muesli):
//   the assistant-marker list of `shouldFallbackToInput` (in tables/markers.json).
//   Copyright (c) 2026 Pranav Hari. Licence: MIT (LICENSES/MIT-Muesli.txt).
//
// Modified by Wispr Bro contributors, 2026-10-31: ported from Swift/Rust to
// one Rust chain; multilingual word tables instead of English-only lists;
// the word band and the content-word check know the filler policy and
// self-repairs; entity, line-count, content-word and language checks are our
// own.
//
//! The guard chain (R5 §4.5, S-12a): decides whether a model reply may
//! replace the deterministic text. Never paste unparsed output; any hit
//! means the user gets the deterministic text ("Inserted without cleanup").
//!
//! Checks, in R5 order (all are run, so the eval sees every reason):
//!
//! 1. `json`: the reply is `{"text": string}`.
//! 2. SpeakoFlow validators: `empty` (unless the input was only fillers),
//!    `punctuation_only`, `channel` (a bare channel name or `<|…|>` token),
//!    `prompt_echo` (6-word runs of the prompt the speaker never said, or the
//!    user-turn framing), `too_long` (> max(80, 3 × input chars)).
//! 3. FreeFlow `executed_instruction` (English markers).
//! 4. BetterWispr `accepted()`: `word_band` (0.4× … 1.2× + 2 words),
//!    `question_mark` (kept when the input has one), `new_tokens` (≤ 25 %
//!    new meaningful words).
//! 5. `assistant_marker`: Muesli's markers and assistant openers that the
//!    input does not have.
//! 6. `entity`: digits, URLs, e-mails, code spans and dictionary terms of the
//!    input survive (a number inside a self-repair may go), no new digits;
//!    `line_count`: the same number of non-empty lines; `formatting`: no
//!    bullets, numbering, headings or bold the input did not have.
//! 7. `content_words`: the input's content words survive (stopwords, the
//!    filler policy and self-repair reparanda excluded; RS-07
//!    recommendation 5). Skipped for unspaced scripts.
//! 8. `language`: Handy's `lang_id` of the output is the input's language.

use std::collections::HashSet;

use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;

use super::markers::{self, Table, Word, TABLES};
use crate::fork::settings::CleanupLevel;

/// Why a reply was rejected (stable names: logs, eval reports).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    Json,
    Empty,
    PunctuationOnly,
    Channel,
    PromptEcho,
    TooLong,
    ExecutedInstruction,
    WordBand,
    QuestionMark,
    NewTokens,
    AssistantMarker,
    Entity,
    LineCount,
    Formatting,
    ContentWords,
    Language,
}

impl Reason {
    pub fn as_str(self) -> &'static str {
        match self {
            Reason::Json => "json",
            Reason::Empty => "empty",
            Reason::PunctuationOnly => "punctuation_only",
            Reason::Channel => "channel",
            Reason::PromptEcho => "prompt_echo",
            Reason::TooLong => "too_long",
            Reason::ExecutedInstruction => "executed_instruction",
            Reason::WordBand => "word_band",
            Reason::QuestionMark => "question_mark",
            Reason::NewTokens => "new_tokens",
            Reason::AssistantMarker => "assistant_marker",
            Reason::Entity => "entity",
            Reason::LineCount => "line_count",
            Reason::Formatting => "formatting",
            Reason::ContentWords => "content_words",
            Reason::Language => "language",
        }
    }
}

/// What the guards compare a reply with.
#[derive(Clone, Debug)]
pub struct Context<'a> {
    /// The deterministic text the model was given.
    pub input: &'a str,
    pub level: CleanupLevel,
    /// The take's language (ISO 639-1) when known; otherwise the input's
    /// language is detected.
    pub language: Option<&'a str>,
    /// The system prompt that was sent (prompt-echo check).
    pub system_prompt: &'a str,
    /// The user's dictionary terms (Handy custom words).
    pub terms: &'a [String],
}

/// The chain's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    /// The reply's text (quotes stripped), when it parsed.
    pub text: Option<String>,
    pub reasons: Vec<Reason>,
}

impl Verdict {
    pub fn accepted(&self) -> bool {
        self.text.is_some() && self.reasons.is_empty()
    }

    /// The text to insert: the reply when accepted, else `None`.
    pub fn accepted_text(&self) -> Option<&str> {
        self.accepted().then_some(self.text.as_deref()).flatten()
    }
}

/// Runs the whole chain on `raw` (the model's reply).
pub fn check(raw: &str, ctx: &Context<'_>) -> Verdict {
    let Some(parsed) = super::prompt::parse_reply(raw) else {
        return Verdict {
            text: None,
            reasons: vec![Reason::Json],
        };
    };
    let text = strip_quotes(&parsed, ctx.input);
    let reasons = check_text(&text, ctx);
    Verdict {
        text: Some(text),
        reasons,
    }
}

/// The input language: the take's, else a confident detection.
pub fn input_language(ctx: &Context<'_>) -> Option<String> {
    ctx.language
        .map(|l| l.split(['-', '_']).next().unwrap_or(l).to_ascii_lowercase())
        .or_else(|| detect(ctx.input))
}

fn detect(text: &str) -> Option<String> {
    crate::audio_toolkit::detect_output_language(text, &[])
}

/// Every check on an already parsed reply.
pub fn check_text(output: &str, ctx: &Context<'_>) -> Vec<Reason> {
    let mut r: Vec<Reason> = Vec::new();
    let lang = input_language(ctx);
    let table = TABLES.for_language(lang.as_deref());
    let input = ctx.input;
    let in_words = markers::words(input);
    let out_words = markers::words(output);

    // 2. SpeakoFlow validators.
    if output.trim().is_empty() {
        if !markers::only_fillers(input, table) {
            r.push(Reason::Empty);
        }
        // Nothing else to check on an allowed empty output.
        return r;
    }
    if !output.chars().any(char::is_alphanumeric) && input.chars().any(char::is_alphanumeric) {
        r.push(Reason::PunctuationOnly);
    }
    if is_leaked_channel_name(input, output) || CHANNEL_TOKEN.is_match(output) {
        r.push(Reason::Channel);
    }
    if prompt_echo(ctx.system_prompt, input, output) || framing_echo(input, output) {
        r.push(Reason::PromptEcho);
    }
    if is_implausibly_long(input, output) {
        r.push(Reason::TooLong);
    }

    // 3. FreeFlow.
    if appears_to_have_executed_instruction(input, output) {
        r.push(Reason::ExecutedInstruction);
    }

    // 4. BetterWispr accepted().
    if !in_word_band(input, output, &in_words, table) {
        r.push(Reason::WordBand);
    }
    if input.contains('?') && !output.contains('?') && !output.contains('？') {
        r.push(Reason::QuestionMark);
    }
    if too_many_new_tokens(&in_words, &out_words) {
        r.push(Reason::NewTokens);
    }

    // 5. Assistant markers.
    if has_assistant_marker(input, output, table) {
        r.push(Reason::AssistantMarker);
    }

    // 6. Entities and lines.
    if !entities_survive(input, output, &in_words, table, ctx.terms) {
        r.push(Reason::Entity);
    }
    if non_empty_lines(input) != non_empty_lines(output) {
        r.push(Reason::LineCount);
    }
    if adds_formatting(input, output) {
        r.push(Reason::Formatting);
    }

    // 7. Content words.
    if !markers::is_unspaced_script(input)
        && !content_words_survive(input, &out_words, table, ctx.level)
    {
        r.push(Reason::ContentWords);
    }

    // 8. Language.
    if let Some(want) = &lang {
        if out_words.len() >= LANGUAGE_MIN_WORDS {
            if let Some(got) = detect(output) {
                if got != *want {
                    r.push(Reason::Language);
                }
            }
        }
    }
    r
}

/// The language check needs this many output words (whatlang is unreliable
/// below; the eval's `LANGUAGE_MIN_WORDS`).
pub const LANGUAGE_MIN_WORDS: usize = 5;

/// BetterWispr `accepted()`: a reply wrapped in quotes the input did not
/// start with loses them. FreeFlow's `EMPTY` sentinel means empty.
pub fn strip_quotes(reply: &str, input: &str) -> String {
    let mut edited = reply.trim().to_string();
    let first = edited.chars().next();
    let last = edited.chars().last();
    if edited.chars().count() > 1 {
        if let (Some(f), Some(l)) = (first, last) {
            if "\"“".contains(f) && "\"”".contains(l) && !input.trim_start().starts_with(f) {
                let inner: String = {
                    let mut cs = edited.chars();
                    cs.next();
                    cs.next_back();
                    cs.collect()
                };
                edited = inner.trim().to_string();
            }
        }
    }
    if edited == "EMPTY" && !input.contains("EMPTY") {
        return String::new();
    }
    edited
}

// --- SpeakoFlow ------------------------------------------------------------

/// gpt-oss channel names returned as the whole answer (SpeakoFlow).
fn is_leaked_channel_name(input: &str, output: &str) -> bool {
    let word = output
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase();
    matches!(word.as_str(), "final" | "analysis" | "commentary")
        && !input.to_lowercase().contains(&word)
}

static CHANNEL_TOKEN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"<\|[a-z_]+\|>|assistantfinal").expect("static regex"));

/// Lower-cased words, punctuation at their edges removed (SpeakoFlow).
fn echo_words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|word| {
            word.trim_matches(|c: char| {
                c.is_ascii_punctuation()
                    || matches!(
                        c,
                        '“' | '”'
                            | '‘'
                            | '’'
                            | '…'
                            | '«'
                            | '»'
                            | '¿'
                            | '¡'
                            | '।'
                            | '—'
                            | '–'
                    )
            })
            .replace('’', "'")
            .to_lowercase()
        })
        .filter(|word| !word.is_empty())
        .collect()
}

/// SpeakoFlow `prompt_echo`: the reply copies runs of the prompt that the
/// speaker never said.
fn prompt_echo(system_prompt: &str, input: &str, output: &str) -> bool {
    const N: usize = 6;
    const ECHO_MIN_WORDS: usize = 8;
    const ECHO_ABSOLUTE_WORDS: usize = 24;

    let output_words = echo_words(output);
    if output_words.len() < N || system_prompt.trim().is_empty() {
        return false;
    }
    let prompt_words = echo_words(system_prompt);
    if prompt_words.len() < N {
        return false;
    }
    let prompt_ngrams: HashSet<&[String]> = prompt_words.windows(N).collect();
    let spoken: HashSet<String> = echo_words(input).into_iter().collect();
    let unsaid = |word: &String| {
        let numeric = word.chars().any(|c| c.is_ascii_digit())
            && word
                .chars()
                .all(|c| c.is_ascii_digit() || c == '.' || c == ',');
        !numeric && !spoken.contains(word)
    };
    let mut copied = vec![false; output_words.len()];
    for (start, window) in output_words.windows(N).enumerate() {
        if prompt_ngrams.contains(window) && window.iter().filter(|w| unsaid(w)).count() * 2 >= N {
            for flag in &mut copied[start..start + N] {
                *flag = true;
            }
        }
    }
    let echoed = copied.iter().filter(|f| **f).count();
    let substantial = echoed >= ECHO_ABSOLUTE_WORDS || echoed * 3 >= output_words.len();
    echoed >= ECHO_MIN_WORDS && substantial
}

/// Our addition: the reply repeats the user-turn framing (`<transcript>`,
/// "Output only the cleaned transcript."), which is too short for
/// [`prompt_echo`]. RS-07: Muesli answered "ok" with the framing line.
fn framing_echo(input: &str, output: &str) -> bool {
    let o = output.to_lowercase();
    let i = input.to_lowercase();
    [
        "<transcript",
        "</transcript",
        "cleaned transcript",
        "begin transcript",
        "end transcript",
    ]
    .iter()
    .any(|m| o.contains(m) && !i.contains(m))
}

/// SpeakoFlow `is_implausibly_long`: more than max(80, 3 × input) chars.
fn is_implausibly_long(input: &str, output: &str) -> bool {
    const RATIO: usize = 3;
    const FLOOR: usize = 80;
    let budget = FLOOR.max(input.chars().count().saturating_mul(RATIO));
    output.chars().count() > budget
}

// --- FreeFlow --------------------------------------------------------------

/// FreeFlow `significantTokens`.
fn significant_tokens(text: &str) -> HashSet<String> {
    const STOP: &[&str] = &[
        "a", "an", "and", "are", "as", "at", "be", "but", "by", "can", "could", "for", "from",
        "had", "has", "have", "he", "her", "him", "his", "i", "if", "in", "into", "is", "it",
        "its", "just", "me", "my", "of", "on", "or", "our", "please", "she", "so", "that", "the",
        "their", "them", "then", "there", "this", "to", "um", "uh", "was", "we", "were", "what",
        "when", "where", "who", "with", "would", "you", "your",
    ];
    text.to_lowercase()
        .split(|c: char| !c.is_alphabetic() && !c.is_numeric())
        .filter(|t| t.chars().count() > 1 && !STOP.contains(t))
        .map(str::to_owned)
        .collect()
}

static ASSISTANT_PREAMBLE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)^\s*(sure|certainly|absolutely|here(?:'s| is)|i(?:'d| would) be happy to|i can)\b",
    )
    .expect("static regex")
});

/// FreeFlow `appearsToHaveExecutedInstruction` (no translation target).
pub fn appears_to_have_executed_instruction(raw: &str, cleaned: &str) -> bool {
    let raw_tokens = significant_tokens(raw);
    let cleaned_tokens = significant_tokens(cleaned);
    if raw_tokens.is_empty() || cleaned_tokens.is_empty() {
        return false;
    }
    const MARKERS: &[&str] = &[
        "ask",
        "answer",
        "compose",
        "create",
        "draft",
        "email",
        "generate",
        "make",
        "message",
        "prompt",
        "reply",
        "respond",
        "response",
        "summarize",
        "tell",
        "translate",
        "write",
        "claude",
        "chatgpt",
        "ai",
        "llm",
    ];
    let raw_markers: HashSet<&String> = raw_tokens
        .iter()
        .filter(|t| MARKERS.contains(&t.as_str()))
        .collect();
    if raw_markers.is_empty() {
        return false;
    }
    let preserved = raw_markers.iter().any(|m| cleaned_tokens.contains(*m));
    let overlap = raw_tokens.intersection(&cleaned_tokens).count();
    let ratio = overlap as f64 / raw_tokens.len().max(1) as f64;
    let cleaned_preamble = ASSISTANT_PREAMBLE.is_match(cleaned);
    let raw_preamble = ASSISTANT_PREAMBLE.is_match(raw);
    (cleaned_preamble && !raw_preamble) || (!preserved && ratio < 0.35)
}

// --- BetterWispr accepted() ------------------------------------------------

static LIST_PREFIX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^[ \t]*(?:\d+[.)]|[-*•])[ \t]+").expect("static regex"));

/// Words removable by a correct cleanup (fillers, repairs and reparanda),
/// counted on whitespace words like BetterWispr.
fn droppable_whitespace_words(words: &[Word], table: &Table) -> usize {
    markers::droppable(words, table)
        .iter()
        .filter(|d| **d)
        .count()
}

/// BetterWispr's word band: at least 2/5 of the words that must stay (the
/// input minus fillers and reparanda, our refinement) and at most the input
/// plus max(2, 1/5).
fn in_word_band(input: &str, output: &str, in_words: &[Word], table: &Table) -> bool {
    let before = input.split_whitespace().count();
    let stripped = LIST_PREFIX.replace_all(output, "");
    let after = stripped.split_whitespace().count();
    if markers::is_unspaced_script(input) {
        // One "word" per run: compare characters instead.
        let b = input.chars().filter(|c| c.is_alphanumeric()).count();
        let a = output.chars().filter(|c| c.is_alphanumeric()).count();
        return a * 5 >= b * 2 && a <= b + usize::max(2, b / 5);
    }
    let keep = before.saturating_sub(droppable_whitespace_words(in_words, table));
    after >= usize::max(1, keep * 2 / 5) && after <= before + usize::max(2, before / 5)
}

fn fold(s: &str) -> String {
    // Diacritics do not make a word new ("deja" → "déjà" is a fix).
    s.chars()
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'ç' => 'c',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ñ' => 'n',
            'ò' | 'ó' | 'ô' | 'ö' | 'õ' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            other => other,
        })
        .collect()
}

/// BetterWispr: more than a quarter of the reply's meaningful words (longer
/// than 3 characters, or with a digit) were never said.
fn too_many_new_tokens(in_words: &[Word], out_words: &[Word]) -> bool {
    let said: HashSet<String> = in_words.iter().map(|w| fold(&w.text)).collect();
    let squashed = squash(in_words);
    let meaningful: Vec<String> = out_words
        .iter()
        .filter(|w| w.text.chars().count() > 3 || w.text.chars().any(|c| c.is_ascii_digit()))
        .map(|w| fold(&w.text))
        .collect();
    // Our refinement: a joined word ("e mail" → "email") is not a new word.
    let fresh = meaningful
        .iter()
        .filter(|w| !kept(w, &said, &squashed) && !same_stem(w, &said))
        .count();
    fresh * 4 > meaningful.len()
}

/// The word with an ending added or dropped is in `said` ("meeting" ->
/// "meetings"): not a new word.
fn same_stem(word: &str, said: &HashSet<String>) -> bool {
    word.chars().count() >= 4
        && said.iter().any(|s| {
            s.chars().count() >= 4
                && (word.starts_with(s.as_str()) || s.starts_with(word))
                && word.chars().count().abs_diff(s.chars().count()) <= 2
        })
}

/// The words joined with nothing between them (letters and digits only).
fn squash(words: &[Word]) -> String {
    words
        .iter()
        .flat_map(|w| fold(&w.text).chars().collect::<Vec<_>>())
        .filter(|c| c.is_alphanumeric())
        .collect()
}

// --- Assistant markers -----------------------------------------------------

fn lead(text: &str) -> String {
    text.trim()
        .trim_start_matches(|c: char| {
            matches!(c, '*' | '_' | '#' | '>' | '"' | '“' | '\'') || c.is_whitespace()
        })
        .to_lowercase()
}

/// The opener of `table` (or English) that `text` starts with.
fn opener<'t>(text: &str, table: &'t Table) -> Option<&'t str> {
    let t = lead(text);
    let en = TABLES.for_language(Some("en"));
    table
        .openers
        .iter()
        .chain(en.openers.iter())
        .find(|m| {
            t.starts_with(m.as_str())
                && t[m.len()..].chars().next().is_none_or(|c| {
                    !c.is_alphanumeric() || !m.ends_with(|e: char| e.is_alphanumeric())
                })
        })
        .map(String::as_str)
}

fn has_assistant_marker(input: &str, output: &str, table: &Table) -> bool {
    let o = output.to_lowercase();
    let i = input.to_lowercase();
    if TABLES
        .assistant_markers
        .iter()
        .any(|m| o.contains(m.as_str()) && !i.contains(m.as_str()))
    {
        return true;
    }
    match opener(output, table) {
        Some(m) => opener(input, table) != Some(m),
        None => false,
    }
}

// --- Entities --------------------------------------------------------------

static URL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)\b(?:https?://|www\.)[^\s<>"']+"#).expect("static regex"));
static EMAIL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"[\p{L}\p{N}._%+-]+@[\p{L}\p{N}.-]+\.\p{L}{2,}").expect("static regex")
});
static CODE: Lazy<Regex> = Lazy::new(|| Regex::new(r"`[^`\n]+`").expect("static regex"));
static DIGITS: Lazy<Regex> = Lazy::new(|| Regex::new(r"\p{Nd}+").expect("static regex"));

fn trim_trailing_punct(s: &str) -> &str {
    s.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']'])
}

/// Byte offsets of input words a self-repair may drop.
fn repair_spans(in_words: &[Word], table: &Table) -> Vec<(usize, usize)> {
    let drop = markers::droppable(in_words, table);
    in_words
        .iter()
        .zip(drop)
        .filter(|(_, d)| *d)
        .map(|(w, _)| (w.start, w.end))
        .collect()
}

/// Digits, URLs, e-mails, code spans and dictionary terms survive; a digit
/// run inside a self-repair may go; the reply adds no digit run the input
/// lacks (no ITN at Light).
fn entities_survive(
    input: &str,
    output: &str,
    in_words: &[Word],
    table: &Table,
    terms: &[String],
) -> bool {
    for re in [&*URL, &*EMAIL, &*CODE] {
        for m in re.find_iter(input) {
            if !output.contains(trim_trailing_punct(m.as_str())) {
                return false;
            }
        }
    }
    let repairs = repair_spans(in_words, table);
    let out_digits: Vec<&str> = DIGITS.find_iter(output).map(|m| m.as_str()).collect();
    let in_digits: Vec<&str> = DIGITS.find_iter(input).map(|m| m.as_str()).collect();
    for m in DIGITS.find_iter(input) {
        let in_repair = repairs
            .iter()
            .any(|(s, e)| m.start() >= *s && m.end() <= *e);
        if !in_repair && !out_digits.contains(&m.as_str()) {
            return false;
        }
    }
    if out_digits.iter().any(|d| !in_digits.contains(d)) {
        return false;
    }
    let in_lower = input.to_lowercase();
    let out_lower = output.to_lowercase();
    for term in terms {
        let t = term.trim();
        if t.is_empty() {
            continue;
        }
        if contains_whole(input, t) {
            if !contains_whole(output, t) {
                return false;
            }
        } else if contains_whole(&in_lower, &t.to_lowercase())
            && !contains_whole(&out_lower, &t.to_lowercase())
        {
            return false;
        }
    }
    true
}

/// `needle` occurs in `hay` with no letter or digit on either side.
fn contains_whole(hay: &str, needle: &str) -> bool {
    hay.match_indices(needle).any(|(i, _)| {
        let before = hay[..i].chars().next_back();
        let after = hay[i + needle.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}

static MARKUP_LINE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^[ \t]*(?:[-*•][ \t]+|\d+[.)][ \t]+|#{1,6}[ \t]+)|\*\*[^*\n]+\*\*")
        .expect("static regex")
});

/// Our addition: bullets, numbered items, headings or bold the input did not
/// have ("never over-format short dictations"; S-12a eval: a three-line
/// take came back as a bulleted list).
fn adds_formatting(input: &str, output: &str) -> bool {
    MARKUP_LINE.is_match(output) && !MARKUP_LINE.is_match(input)
}

fn non_empty_lines(s: &str) -> usize {
    s.lines().filter(|l| !l.trim().is_empty()).count()
}

// --- Content words ---------------------------------------------------------

/// A content word is kept when the reply has it (case and diacritics
/// aside) or has it joined with a neighbour ("e mail" → "email").
fn kept(word: &str, out: &HashSet<String>, squashed: &str) -> bool {
    let w = fold(word);
    if out.contains(&w) {
        return true;
    }
    let plain: String = w.chars().filter(|c| c.is_alphanumeric()).collect();
    if plain.chars().count() >= 4 && squashed.contains(&plain) {
        return true;
    }
    // No edit-distance allowance: a recognizer writes real words, so a
    // "spelling fix" is a different word ("negeer" → "Neger", "thanks" →
    // "tanks" in the S-12a eval and RS-07).
    false
}

/// Light lets at most 1 in 10 content words go (none below 10), Medium 1 in
/// 4 (rewording).
fn content_words_survive(
    input: &str,
    out_words: &[Word],
    table: &Table,
    level: CleanupLevel,
) -> bool {
    // One-letter words ("e" of "e mail") say too little to require.
    let required: Vec<String> = markers::required_words(input, table)
        .into_iter()
        .filter(|w| w.chars().count() > 1 || w.chars().all(|c| c.is_ascii_digit()))
        .collect();
    if required.is_empty() {
        return true;
    }
    let out: HashSet<String> = out_words.iter().map(|w| fold(&w.text)).collect();
    let squashed = squash(out_words);
    let missing: Vec<&String> = required
        .iter()
        .filter(|w| !kept(w, &out, &squashed))
        .collect();
    let allowed = match level {
        CleanupLevel::Medium => required.len() / 4,
        _ => required.len() / 10,
    };
    missing.len() <= allowed && !missing.iter().any(|w| altered(w, &out))
}

/// A word that came back as a different word that looks like it ("negeer"
/// → "neger", "thanks" → "tanks", "envoie" → "envoyer"): never a cleanup,
/// at any level. An added or dropped ending ("meeting" → "meetings", "goed"
/// → "goede") is rewording, which Medium may do.
fn altered(word: &str, out: &HashSet<String>) -> bool {
    let w = fold(word);
    let n = w.chars().count();
    if n < 4 {
        return false;
    }
    out.iter().any(|o| {
        let d = strsim::osa_distance(o, &w);
        // An added or dropped ending keeps the shorter word as a prefix.
        let ending_only = o.starts_with(w.as_str()) || w.starts_with(o.as_str());
        (1..=2).contains(&d) && !ending_only
    })
}

#[cfg(test)]
mod tests;
