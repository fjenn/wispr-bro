// SPDX-License-Identifier: GPL-3.0-or-later
//
// The "needs cleanup" gate extends open-flow's `LLMCleanerStage.needsCleanup`
// (https://github.com/Robj1925/open-flow, 7055c59014ffaac3785e25532ee07ce39093490e,
// `Sources/OpenFlowCore/Postprocess/LLMCleanerStage.swift`; Copyright (c) 2026
// Rob J; MIT, LICENSES/MIT-open-flow.txt). Modified by Wispr Bro contributors,
// 2026-10-31: multilingual filler and repair tables, and "not yet punctuated"
// also needs the LLM (Light fixes punctuation and casing).
//
//! Cleanup levels on the local LLM (S-12a, R5 §4.10 stage 8).
//!
//! [`clean`] is the whole decision for one text, independent of Tauri so the
//! eval runs the shipped code path (`eval_run.rs`):
//!
//! 1. Level None, or nothing to clean → the deterministic text (skipped).
//! 2. Light and the text already looks clean (no filler, repair or stutter;
//!    punctuated and capitalized) → skipped, no LLM call (open-flow's gate).
//! 3. Paragraph by paragraph (DEC-51): texts over [`CHUNK_WORDS`] words are
//!    cut at blank lines, then sentence ends, into chunks of about that
//!    size; each chunk is one request with its own guards and deadline.
//! 4. Each reply goes through the guard chain ([`super::guards`]); a
//!    rejected, late or failed chunk keeps its deterministic text.
//!
//! [`post_process`] is the `actions.rs` hook (R5 §4.2): it decides what the
//! take may do (level, AI-chat skip by `is_ai_chat`, model ready, memory
//! rule) and on which engine ([`choose`]: the sidecar, or Apple's model per
//! RS-08's [`super::apple::ROLE`], S-12b), runs [`clean`] on it, puts back dictionary spellings the
//! LLM undid (stage 9 via `fork::text::note_rewrite`), records the outcome
//! for the history row and tells the UI (`ForkCleanupResult`; the pill says
//! "Inserted without cleanup" when `notice` is set, S-03).

use std::future::Future;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};
use tauri_specta::Event;

use super::chat::{CancellationToken, ChatError, ChatRequest};
use super::guards::{self, Verdict};
use super::markers::{self, TABLES};
use super::supervisor::{RequestError, StartError, Supervisor};
use super::{deadline, prompt};
use crate::fork::context::categories::StyleGroup;
use crate::fork::settings::{CleanupLevel, StylePreset, StyleSettings};

/// Longest chunk sent in one request, in words (≈ 1-2 s on an M1).
pub const CHUNK_WORDS: usize = 120;

/// `entry_meta.cleanup_outcome`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CleanupOutcome {
    Applied,
    Skipped,
    Guard,
    Timeout,
    Error,
}

impl CleanupOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            CleanupOutcome::Applied => "applied",
            CleanupOutcome::Skipped => "skipped",
            CleanupOutcome::Guard => "guard",
            CleanupOutcome::Timeout => "timeout",
            CleanupOutcome::Error => "error",
        }
    }
}

/// Why a take was not (fully) cleaned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CleanupWhy {
    /// Level None.
    LevelNone,
    /// Nothing to clean.
    Empty,
    /// Light, and the text needs no cleanup (no LLM call).
    AlreadyClean,
    /// The target is an AI chat (`is_ai_chat`): no LLM formatting.
    AiChat,
    /// No cleanup model on this Mac yet (downloading, unsupported).
    NotReady,
    /// R5 §4.5 memory rule.
    LowMemory,
    Guard,
    Timeout,
    Error,
}

impl CleanupWhy {
    pub fn as_str(self) -> &'static str {
        match self {
            CleanupWhy::LevelNone => "level_none",
            CleanupWhy::Empty => "empty",
            CleanupWhy::AlreadyClean => "already_clean",
            CleanupWhy::AiChat => "ai_chat",
            CleanupWhy::NotReady => "not_ready",
            CleanupWhy::LowMemory => "low_memory",
            CleanupWhy::Guard => "guard",
            CleanupWhy::Timeout => "timeout",
            CleanupWhy::Error => "error",
        }
    }

    /// The user is told "Inserted without cleanup" (R5 S-12a): the LLM was
    /// wanted but could not be used. Level None, empty, clean text and AI
    /// chats are by design.
    pub fn is_notice(self) -> bool {
        matches!(
            self,
            CleanupWhy::NotReady
                | CleanupWhy::LowMemory
                | CleanupWhy::Guard
                | CleanupWhy::Timeout
                | CleanupWhy::Error
        )
    }
}

/// `entry_meta.cleanup_engine`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CleanupEngine {
    None,
    Local,
    /// Apple Foundation Models (S-12b).
    Apple,
}

impl CleanupEngine {
    pub fn as_str(self) -> &'static str {
        match self {
            CleanupEngine::None => "none",
            CleanupEngine::Local => "local",
            CleanupEngine::Apple => "apple",
        }
    }
}

impl CleanupLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            CleanupLevel::None => "none",
            CleanupLevel::Light => "light",
            CleanupLevel::Medium => "medium",
        }
    }
}

/// What a take's cleanup did, for its history row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanupRecord {
    pub level: CleanupLevel,
    pub engine: CleanupEngine,
    pub outcome: CleanupOutcome,
    pub why: Option<CleanupWhy>,
    /// Word edits the LLM made (`entry_meta.fix_count_llm`).
    pub fixes: u32,
}

/// One take's result for the UI (pill, debug panel).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkCleanupResult {
    /// Counter since launch.
    pub take: u32,
    pub level: CleanupLevel,
    pub engine: CleanupEngine,
    pub outcome: CleanupOutcome,
    pub why: Option<CleanupWhy>,
    /// Show "Inserted without cleanup".
    pub notice: bool,
    pub chunks_total: u32,
    pub chunks_applied: u32,
    /// Time in the LLM step (requests and a server start).
    pub llm_ms: u32,
    pub fixes: u32,
    /// Guard checks that rejected a chunk (names, no text).
    pub guard_reasons: Vec<String>,
}

// ---------------------------------------------------------------------------
// The engine

/// Why a request produced no reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EngineError {
    Timeout,
    Failed(String),
}

/// Something that answers a chat request (the sidecar; fakes in tests).
pub trait Engine: Send + Sync {
    fn complete(
        &self,
        request: ChatRequest,
        timeout: Duration,
    ) -> impl Future<Output = Result<String, EngineError>> + Send;
}

/// The bundled `llama-server` through the supervisor. A server that is not
/// running gets [`deadline::START_BUDGET`] to start, outside the request
/// deadline.
pub struct LocalEngine<'a> {
    pub supervisor: &'a Supervisor,
    /// Show "Cleaning up…" on the pill when a request starts (dictations,
    /// not Try it or the eval).
    pub pill: bool,
}

impl Engine for LocalEngine<'_> {
    async fn complete(
        &self,
        request: ChatRequest,
        timeout: Duration,
    ) -> Result<String, EngineError> {
        if self.pill {
            crate::fork::pill::cleaning_up();
        }
        match tokio::time::timeout(deadline::START_BUDGET, self.supervisor.ensure_ready()).await {
            Err(_) => return Err(EngineError::Timeout),
            Ok(Err(e)) => return Err(EngineError::Failed(start_error_kind(&e).into())),
            Ok(Ok(_)) => {}
        }
        match self
            .supervisor
            .complete(&request, timeout, &CancellationToken::new())
            .await
        {
            Ok(reply) => Ok(reply.content),
            Err(RequestError::Chat(ChatError::Timeout)) => Err(EngineError::Timeout),
            Err(RequestError::Chat(e)) => Err(EngineError::Failed(chat_error_kind(&e).into())),
            Err(RequestError::Start(e)) => Err(EngineError::Failed(start_error_kind(&e).into())),
        }
    }
}

/// Error names without any server text (logs).
fn chat_error_kind(e: &ChatError) -> &'static str {
    match e {
        ChatError::Cancelled => "cancelled",
        ChatError::Transport(_) => "transport",
        ChatError::Timeout => "timeout",
        ChatError::InputTooLong => "input_too_long",
        ChatError::Truncated => "truncated",
        ChatError::Server(..) => "server",
        ChatError::Empty => "empty_reply",
        ChatError::Malformed => "malformed",
    }
}

fn start_error_kind(e: &StartError) -> &'static str {
    match e {
        StartError::NoModel => "no_model",
        StartError::Backoff(_) => "backoff",
        StartError::Failed(_) => "start_failed",
    }
}

// ---------------------------------------------------------------------------
// The core

/// What to clean.
#[derive(Clone, Debug)]
pub struct Request<'a> {
    pub text: &'a str,
    pub level: CleanupLevel,
    /// The take's language (ISO 639-1) as the recognizer or the user's
    /// setting gave it: prompt, lead-ins and guards use it.
    pub language: Option<&'a str>,
    /// A guess from the text ([`hint_language`]) when `language` is unknown:
    /// the prompt and the lead-ins use it, the guards do not (a wrong guess
    /// must not reject a correct cleanup).
    pub hint: Option<&'a str>,
    /// Spellings for the prompt's vocabulary: the dictionary's words and its
    /// replacements' outputs.
    pub terms: &'a [String],
    /// Words the guards require back exactly when the input has them: the
    /// dictionary's words (Handy's custom words). Replacement outputs are
    /// not here: stage 9 puts them back after the LLM instead of failing
    /// the whole cleanup.
    pub keep: &'a [String],
    /// The take's style preset (S-19a; English takes outside AI chats):
    /// no preset changes the prompt today (`prompt::style_fragment`). The
    /// deterministic rules (stage 10)
    /// run after [`clean`], in [`run`].
    pub style: Option<StylePreset>,
    /// S-20: names near the caret this text has or nearly has (the
    /// prompt's CONTEXT line); empty without context awareness.
    pub context: &'a [String],
}

impl Request<'_> {
    /// The language the prompt and the lead-in stage work with.
    pub fn prompt_language(&self) -> Option<&str> {
        self.language.or(self.hint)
    }
}

/// One chunk's request, for logs and the eval.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkTrace {
    pub input: String,
    /// The model's reply, when one came back.
    pub raw: Option<String>,
    pub verdict: Option<Verdict>,
    pub ms: u32,
    pub deadline_ms: u32,
    pub error: Option<String>,
    pub timed_out: bool,
    /// A multi-line block whose reply only lost line breaks; its lines were
    /// then sent one by one (their traces follow). Not counted.
    pub superseded: bool,
    /// The reply joined the block's lines and they were put back
    /// ([`realign_lines`]).
    pub realigned: bool,
}

impl ChunkTrace {
    fn applied(&self) -> bool {
        self.verdict.as_ref().is_some_and(Verdict::accepted)
    }

    /// The accepted text, if any.
    fn result(&self) -> Option<&str> {
        self.verdict.as_ref().and_then(Verdict::accepted_text)
    }

    /// The model's text: the parsed reply, else the raw reply, else nothing.
    fn model(&self) -> &str {
        self.verdict
            .as_ref()
            .and_then(|v| v.text.as_deref())
            .or(self.raw.as_deref())
            .unwrap_or_default()
    }

    /// Rejected only for the layout of its lines: line breaks moved, or
    /// the lines came back as a list.
    fn only_lines_moved(&self) -> bool {
        self.verdict.as_ref().is_some_and(|v| {
            v.text.is_some()
                && !v.reasons.is_empty()
                && v.reasons
                    .iter()
                    .all(|r| matches!(r, guards::Reason::LineCount | guards::Reason::Formatting))
        })
    }
}

/// The result of [`clean`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cleaned {
    /// The text to insert (the deterministic text where nothing applied).
    pub text: String,
    pub outcome: CleanupOutcome,
    pub why: Option<CleanupWhy>,
    pub chunks: Vec<ChunkTrace>,
    /// Wall time of the LLM step.
    pub llm_ms: u32,
    /// The text as the model wrote it, before the guards: each chunk's reply
    /// text (its raw reply when unparsed, nothing after a timeout or error)
    /// in place of the chunk. The eval's pre-guard text.
    pub model_text: String,
}

impl Cleaned {
    fn skipped(text: &str, why: CleanupWhy) -> Self {
        Self {
            text: text.to_owned(),
            outcome: CleanupOutcome::Skipped,
            why: Some(why),
            chunks: Vec::new(),
            llm_ms: 0,
            model_text: text.to_owned(),
        }
    }

    /// The requests that count (not superseded by their lines).
    pub fn counted(&self) -> impl Iterator<Item = &ChunkTrace> {
        self.chunks.iter().filter(|c| !c.superseded)
    }

    pub fn chunks_applied(&self) -> u32 {
        u32::try_from(self.counted().filter(|c| c.applied()).count()).unwrap_or(u32::MAX)
    }

    pub fn chunks_sent(&self) -> u32 {
        u32::try_from(self.counted().count()).unwrap_or(u32::MAX)
    }

    pub fn guard_reasons(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for c in self.counted() {
            if let Some(v) = &c.verdict {
                for r in &v.reasons {
                    let s = r.as_str().to_string();
                    if !out.contains(&s) {
                        out.push(s);
                    }
                }
            }
        }
        out
    }
}

const TERMINAL: &[char] = &[
    '.', '!', '?', '…', '。', '！', '？', '"', '”', '»', ')', '」', ':',
];

/// open-flow's gate, extended: `false` when the text has no filler, repair
/// or stutter and is already punctuated and capitalized, so Light has
/// nothing to do and the LLM is skipped.
pub fn needs_cleanup(text: &str, language: Option<&str>) -> bool {
    needs_cleanup_in(text, language, false)
}

/// [`needs_cleanup`] for one line of a longer text (`line_of_many`): such a
/// line ("Best," / "Finn") needs no sentence end of its own.
pub fn needs_cleanup_in(text: &str, language: Option<&str>, line_of_many: bool) -> bool {
    let table = TABLES.for_language(language);
    let ws = markers::words(text);
    if ws.is_empty() {
        return false;
    }
    if markers::contains_any(&ws, &table.hesitations)
        || markers::contains_any(&ws, &table.fillers)
        || markers::contains_any(&ws, &table.repairs)
        || markers::has_stutter(&ws)
    {
        return true;
    }
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let lowercase_start = lines.iter().any(|l| {
        l.chars()
            .find(|c| c.is_alphabetic())
            .is_some_and(char::is_lowercase)
    });
    if lowercase_start {
        return true;
    }
    // A sentence end followed by a lower-case word ("done. then we").
    let lower_after_stop = text.split_inclusive(['.', '!', '?']).skip(1).any(|s| {
        s.chars()
            .find(|c| c.is_alphabetic())
            .is_some_and(char::is_lowercase)
    });
    if lower_after_stop {
        return true;
    }
    let ends = text
        .trim_end()
        .chars()
        .next_back()
        .is_some_and(|c| TERMINAL.contains(&c));
    lines.len() == 1 && !ends && !line_of_many
}

/// A piece of the text and the separator after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Piece {
    pub text: String,
    pub separator: String,
}

fn count_words(s: &str) -> usize {
    markers::words(s).len()
}

/// Cuts `text` into the pieces sent one request each: every line is its own
/// piece (Muesli joins lines into one, which the line-count guard would
/// reject: S-12a eval), and a line over [`CHUNK_WORDS`] words is cut at
/// sentence ends, then every [`CHUNK_WORDS`] words, with neighbouring
/// sentences joined back while they fit. Joining the pieces with their
/// separators gives `text` back.
pub fn chunks(text: &str) -> Vec<Piece> {
    static LINE_BREAKS: once_cell::sync::Lazy<regex::Regex> =
        once_cell::sync::Lazy::new(|| regex::Regex::new(r"[ \t]*\n\s*").expect("static regex"));
    static SENTENCE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"[.!?…。！？][\x22”»)]?[ \t]+").expect("static regex")
    });

    fn split_by(re: &regex::Regex, piece: Piece, keep_mark: bool) -> Vec<Piece> {
        let mut out = Vec::new();
        let mut last = 0;
        let t = &piece.text;
        for m in re.find_iter(t) {
            // Sentence marks stay with their sentence; the space is the separator.
            let cut = if keep_mark {
                m.start() + m.as_str().trim_end().len()
            } else {
                m.start()
            };
            out.push(Piece {
                text: t[last..cut].to_owned(),
                separator: t[cut..m.end()].to_owned(),
            });
            last = m.end();
        }
        out.push(Piece {
            text: t[last..].to_owned(),
            separator: piece.separator,
        });
        out.retain(|p| !(p.text.is_empty() && p.separator.is_empty()));
        out
    }

    fn by_words(piece: Piece) -> Vec<Piece> {
        let ws = markers::words(&piece.text);
        let mut out = Vec::new();
        let mut start = 0;
        let mut i = CHUNK_WORDS;
        while i < ws.len() {
            let cut = ws[i].start;
            let end = piece.text[..cut].trim_end().len();
            out.push(Piece {
                text: piece.text[start..end].to_owned(),
                separator: piece.text[end..cut].to_owned(),
            });
            start = cut;
            i += CHUNK_WORDS;
        }
        out.push(Piece {
            text: piece.text[start..].to_owned(),
            separator: piece.separator,
        });
        out
    }

    /// One line: sentences, then words; neighbours joined while they fit.
    fn line_pieces(line: Piece) -> Vec<Piece> {
        if count_words(&line.text) <= CHUNK_WORDS {
            return vec![line];
        }
        let pieces: Vec<Piece> = split_by(&SENTENCE, line, true)
            .into_iter()
            .flat_map(|p| {
                if count_words(&p.text) > CHUNK_WORDS {
                    by_words(p)
                } else {
                    vec![p]
                }
            })
            .collect();
        let mut joined: Vec<Piece> = Vec::new();
        for p in pieces {
            match joined.last_mut() {
                Some(last) if count_words(&last.text) + count_words(&p.text) <= CHUNK_WORDS => {
                    last.text.push_str(&last.separator);
                    last.text.push_str(&p.text);
                    last.separator = p.separator;
                }
                _ => joined.push(p),
            }
        }
        joined
    }

    split_by(
        &LINE_BREAKS,
        Piece {
            text: text.to_owned(),
            separator: String::new(),
        },
        false,
    )
    .into_iter()
    .flat_map(line_pieces)
    .collect()
}

/// The take's language for the prompt when the recognizer did not say:
/// the user's only spoken language; else the one of the user's spoken
/// languages whose own words the text clearly uses ([`vote`]); else Handy's
/// detector (whatlang) constrained to those languages, with a lower bar than
/// Handy's 0.9 because only these few compete. `None` without spoken
/// languages or when the text is too short to tell.
pub fn hint_language(text: &str, spoken: &[String]) -> Option<String> {
    let primary = |c: &str| {
        c.split(['-', '_'])
            .next()
            .unwrap_or(c)
            .trim()
            .to_ascii_lowercase()
    };
    let mut codes: Vec<String> = spoken
        .iter()
        .map(|c| primary(c))
        .filter(|c| !c.is_empty())
        .collect();
    codes.sort();
    codes.dedup();
    match codes.len() {
        0 => return None,
        1 => return codes.pop(),
        _ => {}
    }
    if let Some(by_words) = vote(text, &codes) {
        return Some(by_words);
    }
    let langs: Vec<whatlang::Lang> = codes
        .iter()
        .filter_map(|c| isolang::Language::from_639_1(c))
        .filter_map(|l| whatlang::Lang::from_code(l.to_639_3()))
        .collect();
    // whatlang needs some text to be right among even a few languages.
    if langs.len() < 2 || markers::words(text).len() < 8 {
        return None;
    }
    let info = whatlang::Detector::with_allowlist(langs).detect(text)?;
    if info.confidence() < HINT_CONFIDENCE {
        return None;
    }
    isolang::Language::from_639_3(info.lang().code())
        .and_then(|l| l.to_639_1())
        .map(str::to_owned)
}

/// The language whose own words (stopwords, fillers, repair markers from
/// `tables/markers.json`) the text uses clearly more than the others'. A word
/// several of the candidates share counts for each of them by 1/n.
fn vote(text: &str, codes: &[String]) -> Option<String> {
    let ws = markers::words(text);
    let sets: Vec<(String, std::collections::HashSet<String>)> = codes
        .iter()
        .map(|c| {
            let t = TABLES.for_language(Some(c));
            let mut set: std::collections::HashSet<String> = t.stopwords.clone();
            // Hesitations are sounds many languages share ("uh", "um").
            for p in t.fillers.iter().chain(&t.repairs) {
                if p.len() == 1 {
                    set.insert(p[0].clone());
                }
            }
            (c.clone(), set)
        })
        .collect();
    let mut scores: Vec<(String, f64)> = sets
        .iter()
        .map(|(c, set)| {
            let score = ws
                .iter()
                .filter(|w| set.contains(&w.text))
                .map(|w| {
                    let shared = sets.iter().filter(|(_, s)| s.contains(&w.text)).count();
                    1.0 / shared.max(1) as f64
                })
                .sum::<f64>();
            (c.clone(), score)
        })
        .collect();
    scores.sort_by(|a, b| b.1.total_cmp(&a.1));
    let (best, top) = scores.first()?.clone();
    let second = scores.get(1).map_or(0.0, |s| s.1);
    (top >= 1.5 && top >= 1.5 * second).then_some(best)
}

/// Confidence [`hint_language`] needs among the user's own languages.
pub const HINT_CONFIDENCE: f64 = 0.8;

fn ms(d: Duration) -> u32 {
    u32::try_from(d.as_millis()).unwrap_or(u32::MAX)
}

/// The system prompt for `req` (also what the prompt-echo guard compares).
pub fn system_prompt(req: &Request<'_>) -> String {
    prompt::system(&prompt::Spec {
        level: req.level,
        language: req.prompt_language(),
        terms: req.terms,
        style: req.style,
        context: req.context,
    })
}

/// Consecutive pieces of [`chunks`] grouped into blocks of at most
/// [`CHUNK_WORDS`] words: a short multi-line text is one block (one
/// request, with context); a long one is several.
pub fn blocks(text: &str) -> Vec<Vec<Piece>> {
    let mut out: Vec<Vec<Piece>> = Vec::new();
    let mut words = 0;
    for p in chunks(text) {
        let n = count_words(&p.text);
        match out.last_mut() {
            Some(b) if words + n <= CHUNK_WORDS => {
                b.push(p);
                words += n;
            }
            _ => {
                out.push(vec![p]);
                words = n;
            }
        }
    }
    out
}

/// The text of a block and the separator after it.
fn block_text(block: &[Piece]) -> (String, &str) {
    let mut t = String::new();
    for (i, p) in block.iter().enumerate() {
        t.push_str(&p.text);
        if i + 1 < block.len() {
            t.push_str(&p.separator);
        }
    }
    (t, block.last().map_or("", |p| p.separator.as_str()))
}

/// Puts the line breaks of `input` back into `output`, a cleanup of it that
/// came back on fewer lines: the words of both are aligned (longest common
/// subsequence, case and punctuation aside) and each input line starts again
/// at the output word aligned with its first kept word, with the input's own
/// separator. `None` when a line has no word left to anchor it, or the
/// output already has as many lines.
pub fn realign_lines(input: &str, output: &str) -> Option<String> {
    let plain =
        |w: &markers::Word| -> String { w.text.chars().filter(|c| c.is_alphanumeric()).collect() };
    let lines: Vec<&str> = input.split('\n').collect();
    let in_lines: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|l| !l.trim().is_empty())
        .collect();
    if in_lines.len() < 2
        || output.lines().filter(|l| !l.trim().is_empty()).count() >= in_lines.len()
    {
        return None;
    }
    let a = markers::words(input);
    let b = markers::words(output);
    let (n, m) = (a.len(), b.len());
    let mut t = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            t[i][j] = if plain(&a[i]) == plain(&b[j]) {
                t[i + 1][j + 1] + 1
            } else {
                t[i + 1][j].max(t[i][j + 1])
            };
        }
    }
    // matched[i] = the output word aligned with input word i.
    let mut matched = vec![None; n];
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if plain(&a[i]) == plain(&b[j]) {
            matched[i] = Some(j);
            i += 1;
            j += 1;
        } else if t[i + 1][j] >= t[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    // The non-empty lines: byte span and the separator before them.
    let mut spans: Vec<(usize, usize, String)> = Vec::new();
    let mut prev_end: Option<usize> = None;
    let mut off = 0;
    for line in &lines {
        let start = off;
        let end = start + line.len();
        off = end + 1;
        if line.trim().is_empty() {
            continue;
        }
        let sep = prev_end.map_or(String::new(), |pe| input[pe..start].to_string());
        spans.push((start, end, sep));
        prev_end = Some(end);
    }
    // Each line after the first starts at the output word aligned with its
    // first kept word.
    let mut cuts: Vec<(usize, String)> = Vec::new();
    for (start, end, sep) in spans.iter().skip(1) {
        let anchor = a
            .iter()
            .enumerate()
            .filter(|(_, w)| w.start >= *start && w.end <= *end)
            .find_map(|(i, _)| matched[i])?;
        cuts.push((
            b[anchor].start,
            sep.trim_start_matches([' ', '\t']).to_string(),
        ));
    }
    if cuts.len() + 1 != in_lines.len() || cuts.windows(2).any(|w| w[0].0 >= w[1].0) {
        return None;
    }
    let mut out = String::with_capacity(output.len() + cuts.len());
    let mut last = 0;
    for (at, sep) in &cuts {
        out.push_str(output[last..*at].trim_end());
        out.push_str(sep);
        last = *at;
    }
    out.push_str(&output[last..]);
    Some(out)
}

/// One request for `core`, guarded.
async fn ask<E: Engine>(
    engine: &E,
    req: &Request<'_>,
    system: &str,
    core: &str,
    timeout: Duration,
) -> ChunkTrace {
    let mut trace = ChunkTrace {
        input: core.to_owned(),
        raw: None,
        verdict: None,
        ms: 0,
        deadline_ms: ms(timeout),
        error: None,
        timed_out: false,
        superseded: false,
        realigned: false,
    };
    if timeout.is_zero() {
        trace.timed_out = true;
        return trace;
    }
    let request = ChatRequest {
        system: system.to_owned(),
        user: prompt::cleanup_user(core),
        max_tokens: prompt::max_tokens(core),
        response_format: Some(prompt::response_format()),
    };
    let t0 = Instant::now();
    // The engine enforces the deadline on the request; this outer bound
    // also covers a slow server start (START_BUDGET).
    let reply = tokio::time::timeout(
        timeout + deadline::START_BUDGET,
        engine.complete(request, timeout),
    )
    .await
    .unwrap_or(Err(EngineError::Timeout));
    trace.ms = ms(t0.elapsed());
    match reply {
        Ok(raw) => {
            trace.verdict = Some(guards::check(
                &raw,
                &guards::Context {
                    input: core,
                    level: req.level,
                    language: req.language,
                    system_prompt: system,
                    terms: req.keep,
                },
            ));
            trace.raw = Some(raw);
        }
        Err(EngineError::Timeout) => trace.timed_out = true,
        Err(EngineError::Failed(e)) => trace.error = Some(e),
    }
    trace
}

/// Appends `text` with its edges and separator to both outputs.
fn push(
    out: &mut String,
    model_text: &mut String,
    lead: &str,
    body: (&str, &str),
    tail: &str,
    sep: &str,
) {
    for (o, b) in [(out, body.0), (model_text, body.1)] {
        o.push_str(lead);
        o.push_str(b);
        o.push_str(tail);
        o.push_str(sep);
    }
}

fn edges(t: &str) -> (&str, &str, &str) {
    let core = t.trim();
    let lead = &t[..t.len() - t.trim_start().len()];
    let tail = &t[t.trim_end().len()..];
    (lead, core, tail)
}

/// Cleans `req.text` (see the module docs) within the R5 take deadline.
pub async fn clean<E: Engine>(engine: &E, req: &Request<'_>) -> Cleaned {
    let words = u32::try_from(count_words(req.text)).unwrap_or(u32::MAX);
    clean_within(engine, req, deadline::take(req.text, words)).await
}

/// [`clean`] with an explicit take deadline (tests).
pub async fn clean_within<E: Engine>(
    engine: &E,
    req: &Request<'_>,
    take_deadline: Duration,
) -> Cleaned {
    let text = req.text;
    if req.level == CleanupLevel::None {
        return Cleaned::skipped(text, CleanupWhy::LevelNone);
    }
    if !text.chars().any(char::is_alphanumeric) {
        return Cleaned::skipped(text, CleanupWhy::Empty);
    }
    // Sentence lead-ins of the take's language go first, without the model.
    let (stripped, _) = super::leadins::strip(text, req.prompt_language());
    let text = stripped.as_str();
    if req.level == CleanupLevel::Light && !needs_cleanup(text, req.prompt_language()) {
        return Cleaned::skipped(text, CleanupWhy::AlreadyClean);
    }

    let system = system_prompt(req);
    let started = Instant::now();
    let mut out = String::with_capacity(text.len());
    let mut model_text = String::with_capacity(text.len());
    let mut traces: Vec<ChunkTrace> = Vec::new();
    let lang = req.prompt_language();
    let multi_line = text.trim().contains('\n');
    let budget_left = |started: Instant| take_deadline.saturating_sub(started.elapsed());

    for block in blocks(text) {
        let (whole, sep) = block_text(&block);
        let (lead, core, tail) = edges(&whole);
        if !core.chars().any(char::is_alphanumeric)
            || (req.level == CleanupLevel::Light
                && multi_line
                && !needs_cleanup_in(core, lang, true))
        {
            // Nothing to clean here: no words, or clean lines of a longer text.
            push(&mut out, &mut model_text, lead, (core, core), tail, sep);
            continue;
        }
        let timeout = deadline::request(core).min(budget_left(started));
        let mut trace = ask(engine, req, &system, core, timeout).await;
        if trace.only_lines_moved() && block.len() > 1 {
            // The model kept the words but joined the lines: put the
            // breaks back where the input had them, then guard again.
            if let Some(text) = trace
                .verdict
                .as_ref()
                .and_then(|v| v.text.as_deref())
                .and_then(|t| realign_lines(core, t))
            {
                let reasons = guards::check_text(
                    &text,
                    &guards::Context {
                        input: core,
                        level: req.level,
                        language: req.language,
                        system_prompt: &system,
                        terms: req.keep,
                    },
                );
                if reasons.is_empty() {
                    trace.verdict = Some(Verdict {
                        text: Some(text),
                        reasons,
                    });
                    trace.realigned = true;
                }
            }
        }
        if !(trace.only_lines_moved() && block.len() > 1) {
            let result = trace
                .result()
                .map(str::to_owned)
                .unwrap_or_else(|| super::leadins::strip_all(core, lang).0);
            let model = trace.model().to_owned();
            push(
                &mut out,
                &mut model_text,
                lead,
                (&result, &model),
                tail,
                sep,
            );
            traces.push(trace);
            continue;
        }
        // The model joined the lines or made a list of them (Muesli does):
        // send them one by one.
        trace.superseded = true;
        traces.push(trace);
        let n = block.len();
        for (i, piece) in block.iter().enumerate() {
            let sep = if i + 1 == n {
                sep
            } else {
                piece.separator.as_str()
            };
            let (lead, core, tail) = edges(&piece.text);
            if !core.chars().any(char::is_alphanumeric)
                || (req.level == CleanupLevel::Light && !needs_cleanup_in(core, lang, true))
            {
                push(&mut out, &mut model_text, lead, (core, core), tail, sep);
                continue;
            }
            let timeout = deadline::request(core).min(budget_left(started));
            let t = ask(engine, req, &system, core, timeout).await;
            let result = t
                .result()
                .map(str::to_owned)
                .unwrap_or_else(|| super::leadins::strip_all(core, lang).0);
            let model = t.model().to_owned();
            push(
                &mut out,
                &mut model_text,
                lead,
                (&result, &model),
                tail,
                sep,
            );
            traces.push(t);
        }
    }

    let failed = traces
        .iter()
        .filter(|t| !t.superseded)
        .find(|t| !t.applied());
    let (outcome, why) = match failed {
        None => (CleanupOutcome::Applied, None),
        Some(t) if t.timed_out => (CleanupOutcome::Timeout, Some(CleanupWhy::Timeout)),
        Some(t) if t.error.is_some() => (CleanupOutcome::Error, Some(CleanupWhy::Error)),
        Some(_) => (CleanupOutcome::Guard, Some(CleanupWhy::Guard)),
    };
    Cleaned {
        text: out,
        outcome,
        why,
        chunks: traces,
        llm_ms: ms(started.elapsed()),
        model_text,
    }
}

// ---------------------------------------------------------------------------
// The app hook

static TAKES: AtomicU32 = AtomicU32::new(0);
static LAST: Mutex<Option<ForkCleanupResult>> = Mutex::new(None);

/// The last take's result (`fork_llm_last_cleanup`).
pub fn last_result() -> Option<ForkCleanupResult> {
    LAST.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

/// Where a text comes from: a dictation goes to the frontmost app (AI-chat
/// skip applies); a retry or a file transcription (Handy's jobs) does not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Dictation,
    Job,
}

/// What the app lets this take do.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// The engine that cleans this take ([`CleanupEngine::None`] when it is
    /// skipped).
    pub engine: CleanupEngine,
    pub level: CleanupLevel,
    /// The dictionary's words (guards and prompt).
    pub words: Vec<String>,
    /// The take's language from Handy's pipeline.
    pub language: Option<String>,
    /// A guess from the text when Handy did not know it.
    pub hint: Option<String>,
    pub terms: Vec<String>,
    /// S-20: the armed take's context hints this text has or nearly has
    /// (none for a job, or with context awareness off).
    pub context: Vec<String>,
    /// Decided before any LLM call.
    pub skip: Option<CleanupWhy>,
    /// The style of this take (S-19a), see [`StyleDecision`].
    pub style: StyleDecision,
    /// S-22: the opt-in transform applied after cleanup, fail-open.
    pub auto_apply: Option<crate::fork::transforms::record::Definition>,
}

/// Which style a take gets (S-19a), decided with the plan.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StyleDecision {
    /// The target app's group.
    pub group: Option<StyleGroup>,
    /// The preset applied: `None` for a job, an AI chat or a take that is
    /// not English (Wispr: styles apply in English only).
    pub preset: Option<StylePreset>,
    /// The target app's bundle id when "Press Enter after dictating" is on
    /// for it (armed after the cleanup unless it fell back).
    pub auto_send: Option<String>,
}

/// The preset of a group.
pub fn preset_for(styles: &StyleSettings, group: StyleGroup) -> StylePreset {
    match group {
        StyleGroup::Personal => styles.personal,
        StyleGroup::Work => styles.work,
        StyleGroup::Email => styles.email,
        StyleGroup::Other => styles.other,
    }
}

/// The style decision for a dictation into `target` (bundle id, name) and
/// its reduced browser `url_host`: the site's group precedes the app's, then
/// the group's preset applies when the take is English and not an AI chat.
pub fn decide_style(
    rules: &[crate::fork::context::app_rules::AppRule],
    target: Option<(Option<&str>, Option<&str>)>,
    url_host: Option<&str>,
    styles: &StyleSettings,
    english: bool,
) -> StyleDecision {
    let Some((bundle_id, name)) = target else {
        return StyleDecision::default();
    };
    let resolved = crate::fork::context::app_rules::resolve(rules, bundle_id, name, url_host);
    let ai_chat = crate::fork::context::categories::classify(bundle_id, name).is_ai_chat;
    StyleDecision {
        group: Some(resolved.group),
        preset: (english && !ai_chat).then(|| preset_for(styles, resolved.group)),
        auto_send: bundle_id.filter(|_| resolved.auto_send).map(str::to_owned),
    }
}

/// The fork settings (`None` if they cannot be read).
fn fork_settings<R: Runtime>(app: &AppHandle<R>) -> Option<crate::fork::settings::ForkSettings> {
    app.try_state::<crate::fork::settings::ForkSettingsHandle>()
        .and_then(|h| h.get(app).ok())
}

/// The cleanup level from the fork settings (Light if they cannot be read).
fn level<R: Runtime>(app: &AppHandle<R>) -> CleanupLevel {
    fork_settings(app)
        .map(|s| s.cleanup_level)
        .unwrap_or_default()
}

/// `true` when the app this take goes to is an AI chat (`is_ai_chat`):
/// the frontmost app now, which is where the text will be inserted.
pub fn target_is_ai_chat() -> bool {
    crate::fork::context::frontmost::frontmost()
        .map(|a| {
            crate::fork::context::categories::classify(a.bundle_id.as_deref(), a.name.as_deref())
                .is_ai_chat
        })
        .unwrap_or(false)
}

/// The plan for `text`, or `None` when the fork does not own cleanup here
/// (headless CLI, or a plain Handy build without the LLM runtime): Handy's
/// own post-processing runs then.
pub async fn plan<R: crate::fork::ForkRuntime>(
    app: &AppHandle<R>,
    text: &str,
    source: Source,
    words: Vec<String>,
    spoken: Vec<String>,
    level_override: Option<CleanupLevel>,
) -> Option<Plan> {
    // The prompt also names the replacements' outputs (S-11 rules).
    let mut terms = words.clone();
    for r in crate::fork::text::active_rules().rules() {
        if !terms.iter().any(|t| t.eq_ignore_ascii_case(&r.to)) {
            terms.push(r.to.clone());
        }
    }
    let rt = super::runtime::runtime(app)?;
    let ready = app
        .try_state::<crate::fork::ForkState>()
        .is_some_and(|s| s.is_ready());
    if !ready {
        return None;
    }
    if rt.status().state == super::runtime::LlmState::Unsupported
        && !crate::fork::is_product_build()
        && !crate::portable::is_portable()
    {
        return None;
    }
    let settings = fork_settings(app).unwrap_or_default();
    let level = level_override.unwrap_or_else(|| level(app));
    let language = crate::fork::context::take::ledger().language_for(text);
    let hint = match language {
        Some(_) => None,
        None => hint_language(text, &spoken),
    };
    // A dictation's style comes from where it goes (the frontmost app now,
    // like the AI-chat skip); a browser page's host precedes that app.
    let style = match source {
        Source::Dictation => {
            let front = crate::fork::context::frontmost::frontmost();
            let url_host = front
                .as_ref()
                .and_then(|a| crate::fork::context::url::current_host(a.bundle_id.as_deref()));
            let english = crate::fork::text::style_rules::is_english(
                language.as_deref(),
                hint.as_deref(),
                text,
                &spoken,
            );
            decide_style(
                &crate::fork::context::app_rules::current(),
                front
                    .as_ref()
                    .map(|a| (a.bundle_id.as_deref(), a.name.as_deref())),
                url_host.as_deref(),
                &settings.styles,
                english,
            )
        }
        Source::Job => StyleDecision::default(),
    };
    let auto_apply = if source == Source::Dictation && settings.transforms_enabled {
        settings.auto_apply_transform.as_deref().and_then(|key| {
            app.try_state::<crate::fork::db::ForkDb>()
                .and_then(|db| {
                    db.with_conn(|conn| {
                        crate::fork::transforms::record::definition_by_key(conn, key)
                    })
                    .ok()
                })
                .flatten()
        })
    } else {
        None
    };
    let mut plan = Plan {
        engine: CleanupEngine::None,
        level,
        words: words.clone(),
        language,
        hint,
        terms,
        context: match source {
            Source::Dictation => crate::fork::context::awareness::hints()
                .map(|h| crate::fork::context::hints::relevant(text, &h))
                .unwrap_or_default(),
            Source::Job => Vec::new(),
        },
        skip: None,
        style,
        auto_apply,
    };
    if level == CleanupLevel::None || !text.chars().any(char::is_alphanumeric) {
        return Some(plan);
    }
    let ai_chat = source == Source::Dictation && target_is_ai_chat();
    let ready = rt.is_ready() && rt.supervisor().is_some();
    // The memory rule only matters once the model could run.
    let memory_ok = !ready || ai_chat || rt.memory_allows_start().await;
    let apple_ok =
        !ai_chat && super::apple::usable_for(plan.language.as_deref().or(plan.hint.as_deref()));
    (plan.skip, plan.engine) = choose(super::apple::ROLE, ai_chat, ready, memory_ok, apple_ok);
    Some(plan)
}

/// The engine for a take that wants cleanup (S-12b), or why it goes without:
/// an AI-chat target never gets one; with [`AppleRole::Default`] Apple's
/// model comes first; the sidecar when its model is ready and the memory rule
/// allows it; with [`AppleRole::Fallback`] Apple's model stands in when the
/// sidecar cannot (model downloading or failed, or not enough free memory:
/// Apple's model runs in a system process the memory rule does not cover);
/// else [`skip_reason`].
///
/// [`AppleRole::Default`]: super::apple::AppleRole::Default
/// [`AppleRole::Fallback`]: super::apple::AppleRole::Fallback
pub fn choose(
    role: super::apple::AppleRole,
    ai_chat: bool,
    ready: bool,
    memory_ok: bool,
    apple_ok: bool,
) -> (Option<CleanupWhy>, CleanupEngine) {
    use super::apple::AppleRole;
    if ai_chat {
        return (Some(CleanupWhy::AiChat), CleanupEngine::None);
    }
    if role == AppleRole::Default && apple_ok {
        return (None, CleanupEngine::Apple);
    }
    if ready && memory_ok {
        return (None, CleanupEngine::Local);
    }
    if apple_ok {
        return (None, CleanupEngine::Apple);
    }
    (skip_reason(false, ready, memory_ok), CleanupEngine::None)
}

/// The last take's system prompt (no text of the take), so the next key-down
/// can prewarm Apple's model with the same instructions (their dictionary
/// terms and level).
static LAST_SYSTEM: Mutex<Option<(CleanupLevel, String)>> = Mutex::new(None);

/// Key-down (S-12b): prewarm Apple's model when it would clean this take
/// ([`super::apple::ROLE`] Default, or the sidecar's model not ready), with
/// the last take's instructions, else the plain prompt for the level.
pub fn prewarm_apple<R: Runtime>(app: &AppHandle<R>) {
    use super::apple::{AppleRole, ROLE};
    let level = level(app);
    if level == CleanupLevel::None {
        return;
    }
    let local_ready = super::runtime::runtime(app).is_some_and(|rt| rt.is_ready());
    if ROLE == AppleRole::Fallback && local_ready {
        return;
    }
    let system = LAST_SYSTEM
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
        .filter(|(l, _)| *l == level)
        .map(|(_, s)| s)
        .unwrap_or_else(|| {
            system_prompt(&Request {
                text: "",
                level,
                language: None,
                hint: None,
                terms: &[],
                keep: &[],
                style: None,
                context: &[],
            })
        });
    super::apple::prewarm(&system);
}

/// Why a take that wants cleanup must go without the LLM, in this order:
/// an AI-chat target (by design, no notice), no model yet, the R5 memory
/// rule.
pub fn skip_reason(ai_chat: bool, ready: bool, memory_ok: bool) -> Option<CleanupWhy> {
    if ai_chat {
        Some(CleanupWhy::AiChat)
    } else if !ready {
        Some(CleanupWhy::NotReady)
    } else if !memory_ok {
        Some(CleanupWhy::LowMemory)
    } else {
        None
    }
}

/// Stage 9 (R5 §4.10) on an LLM rewrite of `input`: the dictionary
/// replacements' outputs the rewrite undid come back
/// (`fork::text::note_rewrite` + `after_llm`; the snippets are expanded
/// later, once, in `deliver`, S-17).
pub fn restore_spellings(input: &str, output: String) -> String {
    if output == input {
        return output;
    }
    crate::fork::text::note_rewrite(input, &output);
    crate::fork::text::after_llm(output)
}

/// The app a take may press Return in after its paste (RV-48): only when
/// the app has "Press Enter after dictating" on and the cleanup did not
/// fall back ("Inserted without cleanup": a guard, a timeout, an error, no
/// model, low memory). The paste then still sends it only once the text
/// demonstrably arrived (`deliver::Request::submit_strict`).
pub fn auto_send_target(style: &StyleDecision, why: Option<CleanupWhy>) -> Option<&str> {
    if why.is_some_and(CleanupWhy::is_notice) {
        return None;
    }
    style.auto_send.as_deref()
}

/// Stage 10 (R5 §4.10, S-19a): the take's preset, if it has one. Logs the
/// preset and group, never the text.
pub fn apply_style(style: &StyleDecision, text: String, keep: &[String]) -> String {
    let Some(preset) = style.preset else {
        return text;
    };
    let styled = crate::fork::text::style_rules::apply(preset, &text, keep);
    log::info!(
        "style: {:?} for {} ({})",
        preset,
        style.group.map_or("?", StyleGroup::as_str),
        if styled == text {
            "no change"
        } else {
            "applied"
        }
    );
    styled
}

/// Word edits between the deterministic text and the cleaned one: runs of
/// changed words, compared case- and punctuation-insensitively (a removed
/// "um" is one fix, "2 actually 3" → "3" is one fix).
pub fn fix_count(before: &str, after: &str) -> u32 {
    crate::fork::text::counts::changed_runs(before, after)
}

/// The guards' keep list: the dictionary's words, plus the context hints
/// `text` already spells exactly (S-20).
pub fn keep_with_context(words: &[String], context: &[String], text: &str) -> Vec<String> {
    let mut keep = words.to_vec();
    for c in context {
        if text.contains(c.as_str()) && !keep.iter().any(|k| k == c) {
            keep.push(c.clone());
        }
    }
    keep
}

/// Runs a plan: [`clean`] on the sidecar, stage 9, the record and the event.
pub async fn run<R: Runtime>(
    app: &AppHandle<R>,
    text: &str,
    plan: &Plan,
) -> (String, CleanupRecord) {
    let cleaned = match plan.skip {
        Some(why) => Cleaned::skipped(text, why),
        None => {
            // S-20: a near-caret name the text already spells right must
            // come back from the LLM exactly (the entity guard).
            let keep = keep_with_context(&plan.words, &plan.context, text);
            let req = Request {
                text,
                level: plan.level,
                language: plan.language.as_deref(),
                hint: plan.hint.as_deref(),
                terms: &plan.terms,
                keep: &keep,
                style: plan.style.preset,
                context: &plan.context,
            };
            if plan.engine == CleanupEngine::Apple {
                // The prewarm prompt outlives the take: never with its
                // context names.
                *LAST_SYSTEM.lock().unwrap_or_else(|p| p.into_inner()) = Some((
                    plan.level,
                    system_prompt(&Request {
                        context: &[],
                        ..req.clone()
                    }),
                ));
                clean(&super::apple::AppleEngine::for_dictation(), &req).await
            } else {
                match super::runtime::runtime(app).and_then(|rt| rt.supervisor().cloned()) {
                    Some(sup) => {
                        clean(
                            &LocalEngine {
                                supervisor: &sup,
                                pill: true,
                            },
                            &req,
                        )
                        .await
                    }
                    None => Cleaned::skipped(text, CleanupWhy::NotReady),
                }
            }
        }
    };
    let engine = if cleaned.chunks.is_empty() {
        CleanupEngine::None
    } else if plan.engine == CleanupEngine::Apple {
        CleanupEngine::Apple
    } else {
        CleanupEngine::Local
    };
    if engine != CleanupEngine::None && !plan.context.is_empty() {
        // S-20: the prompt named near-caret names.
        crate::fork::context::awareness::note_used();
        crate::fork::context::take::ledger().note_context_used(text);
    }
    let restored = restore_spellings(text, cleaned.text.clone());
    let fixes = fix_count(text, &restored);
    // S-22: after cleanup and restored spellings, before deterministic style.
    // Every failure returns `restored`, so the ordinary paste still proceeds.
    let transformed = match &plan.auto_apply {
        Some(definition) => {
            crate::fork::transforms::auto::apply(app, definition, &restored)
                .await
                .text
        }
        None => restored,
    };
    // Stage 10 (S-19a): the preset's deterministic rules, on the cleaned
    // text or, after a fallback, on the deterministic one.
    let final_text = apply_style(&plan.style, transformed, &plan.words);
    match auto_send_target(&plan.style, cleaned.why) {
        Some(bundle_id) => crate::fork::context::app_rules::arm_auto_send(&final_text, bundle_id),
        None if plan.style.auto_send.is_some() => {
            log::info!("style: no Return after this take (inserted without cleanup)");
        }
        None => {}
    }
    let record = CleanupRecord {
        level: plan.level,
        engine,
        outcome: cleaned.outcome,
        why: cleaned.why,
        fixes,
    };
    let result = ForkCleanupResult {
        take: TAKES.fetch_add(1, Ordering::Relaxed) + 1,
        level: plan.level,
        engine,
        outcome: cleaned.outcome,
        why: cleaned.why,
        notice: cleaned.why.is_some_and(CleanupWhy::is_notice),
        chunks_total: cleaned.chunks_sent(),
        chunks_applied: cleaned.chunks_applied(),
        llm_ms: cleaned.llm_ms,
        fixes,
        guard_reasons: cleaned.guard_reasons(),
    };
    log::info!(
        "cleanup: {} {} ({}{}, {}/{} chunk(s), {} ms, {} fix(es){})",
        cleaned.outcome.as_str(),
        plan.level.as_str(),
        engine.as_str(),
        cleaned
            .why
            .map(|w| format!(", {}", w.as_str()))
            .unwrap_or_default(),
        result.chunks_applied,
        result.chunks_total,
        cleaned.llm_ms,
        fixes,
        if result.guard_reasons.is_empty() {
            String::new()
        } else {
            format!(", guard: {}", result.guard_reasons.join(" "))
        }
    );
    #[cfg(debug_assertions)]
    if std::env::var_os(crate::fork::text::TRACE_ENV).is_some() {
        log::info!("cleanup trace: {text:?} -> {final_text:?}");
    }
    if result.notice {
        // The pill (S-03) says "Inserted without cleanup".
        crate::fork::pill::show_message(
            crate::fork::pill::model::PillMessage::InsertedWithoutCleanup,
        );
    }
    *LAST.lock().unwrap_or_else(|p| p.into_inner()) = Some(result.clone());
    if let Err(e) = result.emit(app) {
        log::debug!("cleanup: result event: {e}");
    }
    (final_text, record)
}

/// The `actions.rs` hook (R5 §4.2): replaces Handy's
/// `process_transcription_output` for a dictation. Handy's own chain still
/// runs where the fork does not own cleanup ([`plan`] → `None`).
pub(crate) async fn post_process(
    app: &AppHandle,
    transcription: &str,
    post_process: bool,
) -> crate::actions::ProcessedTranscription {
    processed(app, transcription, post_process, Source::Dictation, None).await
}

/// Cleanup levels asked for one retry each (S-15b "Retry transcript" with
/// another level), by history entry id; taken by the entry's next job.
static RETRY_LEVELS: Mutex<Vec<(i64, CleanupLevel)>> = Mutex::new(Vec::new());

/// The next job for `entry_id` cleans at `level` instead of the setting.
pub fn set_retry_level(entry_id: i64, level: Option<CleanupLevel>) {
    let mut levels = RETRY_LEVELS.lock().unwrap_or_else(|e| e.into_inner());
    levels.retain(|(id, _)| *id != entry_id);
    if let Some(level) = level {
        levels.push((entry_id, level));
    }
}

/// The level asked for `entry_id`'s retry, removed (one retry each).
pub fn take_retry_level(entry_id: i64) -> Option<CleanupLevel> {
    let mut levels = RETRY_LEVELS.lock().unwrap_or_else(|e| e.into_inner());
    let at = levels.iter().position(|(id, _)| *id == entry_id)?;
    Some(levels.remove(at).1)
}

/// The `managers/transcription_jobs.rs` hook: retries and file
/// transcriptions are cleaned at the same level (no AI-chat skip: the text
/// goes to History, not to an app), or at the level the user picked for
/// this retry (S-15b).
pub(crate) async fn post_process_job(
    app: &AppHandle,
    transcription: &str,
    entry: &crate::managers::history::HistoryEntry,
) -> crate::actions::ProcessedTranscription {
    let level = take_retry_level(entry.id);
    processed(
        app,
        transcription,
        entry.post_process_requested,
        Source::Job,
        level,
    )
    .await
}

async fn processed(
    app: &AppHandle,
    transcription: &str,
    post_process: bool,
    source: Source,
    level: Option<CleanupLevel>,
) -> crate::actions::ProcessedTranscription {
    if source == Source::Dictation && crate::fork::transforms::command::take_requested(post_process)
    {
        return crate::fork::transforms::command::process(app, transcription).await;
    }
    let settings = crate::settings::get_settings(app);
    let Some(plan) = plan(
        app,
        transcription,
        source,
        settings.custom_words,
        settings.spoken_languages,
        level,
    )
    .await
    else {
        return crate::actions::process_transcription_output(app, transcription, post_process)
            .await;
    };
    let (final_text, record) = run(app, transcription, &plan).await;
    crate::fork::context::take::ledger().note_cleanup(transcription, record);
    let changed = final_text != transcription;
    crate::actions::ProcessedTranscription {
        post_processed_text: changed.then(|| final_text.clone()),
        final_text,
        post_process_prompt: None,
    }
}

#[cfg(test)]
mod tests;
