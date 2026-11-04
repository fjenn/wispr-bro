// SPDX-License-Identifier: GPL-3.0-or-later
//! The text pipeline (R5 §4.10, S-11): newline-safe, ordered stages between
//! the recognizer and the field.
//!
//! Before the LLM, from the `managers/transcription.rs` hook ([`process`],
//! in `finish_text`, for every transcription):
//!
//! | Stage                                 | Code                              |
//! | ------------------------------------- | --------------------------------- |
//! | 1 verbatim ASR text kept              | ledger → `entry_meta.asr_text`    |
//! | 3 exact dictionary rules, protected   | [`rules`] + [`placeholders`]      |
//! | 2, 4, 5 Handy, per line               | Handy's stages through [`lines`]  |
//! | 7 voice commands, "scratch that"      | [`commands`], [`scratch`]         |
//! | 12 line-safe whitespace               | [`whitespace`]                    |
//!
//! Stage 3 runs ahead of Handy's script conversion (stage 2) because the
//! hook wraps Handy's stages as one unit, per line; its outputs are
//! Private Use Area placeholders while Handy's stages run, so neither the
//! script conversion nor fuzzy custom words nor filler removal can touch
//! them (R2 M-03). Stage 6 (ITN) is not shipped (RS-13, DEC-40).
//!
//! After the LLM, inside `fork::insert::deliver` ([`finish`]): stage 9 puts
//! back rule outputs the LLM reverted (for a rewrite registered with
//! [`note_rewrite`]), stage 12 runs again, then stage 11 expands the
//! snippets ([`snippets`], S-17) and nothing touches the text after it, so
//! a saved text is inserted exactly as written and never reaches the LLM.
//! Stage 10 (style rules, [`style_rules`], S-19a) runs in the LLM step
//! (`fork::llm::post_process::run`, after stage 9) because it needs the
//! take's app and language; stage 13 (Shift+Return when typing) is S-04's
//! typing path.
//!
//! Every stage is fail-open: a panic in a fork stage falls back to Handy's
//! own result for the whole text. Per take, the stage order and change
//! counts and the rules that applied go to the history row
//! (`entry_meta.text_stages`, `fix_count_dictionary`, `corrections`).

pub mod commands;
pub mod counts;
pub mod diff;
pub mod file_tags;
pub mod fold;
pub mod lines;
pub mod placeholders;
pub mod rules;
pub mod scratch;
pub mod snippets;
pub mod style_rules;
pub mod whitespace;

#[cfg(test)]
mod tests;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex, RwLock};

use crate::audio_toolkit::OutputLanguageEvidence;
use crate::fork::context::take::ledger;
use counts::{changed_words, RuleHit, Stage, TextRecord};
use placeholders::Placeholders;
use rules::{Provenance, Rule, RuleSet};
use snippets::{Snippet, SnippetHit, SnippetSet};

type RuleSource = Arc<dyn Fn() -> Vec<Rule> + Send + Sync>;
type SnippetSource = Arc<dyn Fn() -> Vec<Snippet> + Send + Sync>;
type SnippetSink = Arc<dyn Fn(SnippetUse) + Send + Sync>;

type WordSource = Arc<dyn Fn() -> Vec<String> + Send + Sync>;

static SOURCE: RwLock<Option<RuleSource>> = RwLock::new(None);
static WORD_SOURCE: RwLock<Option<WordSource>> = RwLock::new(None);
static COMPILED: Mutex<Option<(Vec<Rule>, Arc<RuleSet>)>> = Mutex::new(None);
static SNIPPET_SOURCE: RwLock<Option<(SnippetSource, SnippetSink)>> = RwLock::new(None);
static SNIPPETS_COMPILED: Mutex<Option<(Vec<Snippet>, Arc<SnippetSet>)>> = Mutex::new(None);

/// What stage 11 did to one delivery, for its history row (S-17).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnippetUse {
    /// The history row being delivered, if known.
    pub entry_id: Option<i64>,
    pub hits: Vec<SnippetHit>,
    /// The text inserted, when the stages after the LLM changed it.
    pub inserted: Option<String>,
}

/// Where the snippets come from (`fork.db`) and where their use is
/// recorded (hit counts, the history row), set by `fork::on_ready`.
/// Without them no snippet expands (headless CLI).
pub fn install_snippets(
    source: impl Fn() -> Vec<Snippet> + Send + Sync + 'static,
    sink: impl Fn(SnippetUse) + Send + Sync + 'static,
) {
    *SNIPPET_SOURCE.write().unwrap_or_else(|e| e.into_inner()) =
        Some((Arc::new(source), Arc::new(sink)));
}

/// The snippets, compiled; recompiled only when they changed.
pub fn active_snippets() -> (Arc<SnippetSet>, Option<SnippetSink>) {
    let installed = SNIPPET_SOURCE
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let Some((source, sink)) = installed else {
        return (Arc::new(SnippetSet::empty()), None);
    };
    let list = source();
    let mut cache = SNIPPETS_COMPILED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((cached, set)) = cache.as_ref() {
        if *cached == list {
            return (Arc::clone(set), Some(sink));
        }
    }
    let set = Arc::new(SnippetSet::new(list.clone()));
    *cache = Some((list, Arc::clone(&set)));
    (set, Some(sink))
}

/// Where the active dictionary rules come from (`fork.db`, set by
/// `fork::on_ready`). Without a source no rule applies (headless CLI).
pub fn install_rule_source(source: impl Fn() -> Vec<Rule> + Send + Sync + 'static) {
    *SOURCE.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(source));
}

/// Where the dictionary's active words come from (`fork.db`, set by
/// `fork::on_ready`), for [`dictionary_accent_stage`]. Without a source no
/// word applies (headless CLI).
pub fn install_word_source(source: impl Fn() -> Vec<String> + Send + Sync + 'static) {
    *WORD_SOURCE.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(source));
}

/// The dictionary words [`dictionary_accent_stage`] handles: the active
/// words with a letter outside ASCII whose accent-free key is plain ASCII
/// (`Siobhán` → `siobhan`, `Zoë`, `Ægir`). Handy's fuzzy fix already covers
/// ASCII words, and scripts without an ASCII key (CJK, Cyrillic) are left
/// to the recognizer's vocabulary, as Handy leaves them.
pub fn accent_words(words: &[String]) -> Vec<String> {
    words
        .iter()
        .filter(|w| !w.is_ascii())
        .filter(|w| {
            let k = crate::fork::context::hints::key(w);
            k.chars().count() >= crate::fork::context::hints::MIN_HINT_CHARS
                && k.chars().all(|c| c.is_ascii_alphanumeric())
        })
        .cloned()
        .collect()
}

fn active_accent_words() -> Vec<String> {
    let source = WORD_SOURCE
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    source.map(|s| accent_words(&s())).unwrap_or_default()
}

/// The active rules, compiled; recompiled only when they changed.
pub fn active_rules() -> Arc<RuleSet> {
    let source = SOURCE.read().unwrap_or_else(|e| e.into_inner()).clone();
    let rules = source.map(|s| s()).unwrap_or_default();
    let mut cache = COMPILED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((cached, set)) = cache.as_ref() {
        if *cached == rules {
            return Arc::clone(set);
        }
    }
    let set = Arc::new(RuleSet::new(rules.clone()));
    *cache = Some((rules, Arc::clone(&set)));
    set
}

/// The `managers/transcription.rs` hook (`finish_text`). `handy` is Handy's
/// own deterministic stage function; it runs on each line. Returns the text
/// Handy keeps (and saves to History); the verbatim ASR text and the
/// pipeline's record go to the take ledger for the history row.
pub fn process(
    raw: String,
    language: &OutputLanguageEvidence,
    mut handy: impl FnMut(String) -> String,
) -> String {
    let rules = active_rules();
    let lang = language.language();
    // S-20: the armed take's context hints, if any (never for a file or a
    // retry outside a dictation).
    let hints = crate::fork::context::awareness::hints();
    // S-36: the code and file names a set-up editor showed, if any.
    let identifiers = crate::fork::context::awareness::identifiers();
    let file_names = crate::fork::context::awareness::file_names();
    let accent_words = active_accent_words();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let (text, mut record) = run(&raw, lang, &rules, &mut handy);
        let text = if accent_words.is_empty() {
            text
        } else {
            dictionary_accent_stage(text, &accent_words, &mut record)
        };
        let text = match identifiers.as_deref() {
            Some(ids) => identifier_stage(text, ids, &mut record),
            None => text,
        };
        let text = match hints.as_deref() {
            Some(h) => context_stage(text, h, &mut record),
            None => text,
        };
        let text = match file_names.as_deref() {
            Some(names) => file_tag_stage(text, names, &mut record),
            None => text,
        };
        (text, record)
    }));
    let (kept, record) = match outcome {
        Ok(done) => done,
        Err(_) => {
            log::error!("text: a pipeline stage panicked; using Handy's stages alone");
            (handy(raw.clone()), TextRecord::default())
        }
    };
    log::info!(
        "text: {} (language {}, {} dictionary rule(s) active)",
        record.describe(),
        lang.unwrap_or("?"),
        rules.rules().len()
    );
    // Debug builds only (Mac test S-11): the texts themselves, which release
    // logs never carry.
    #[cfg(debug_assertions)]
    if std::env::var_os(TRACE_ENV).is_some() {
        log::info!("text trace: {raw:?} -> {kept:?}");
    }
    let context_fixed = record.stages.iter().any(|s| {
        matches!(
            s.stage,
            Stage::Context | Stage::Identifiers | Stage::FileTags
        ) && s.changes > 0
    });
    ledger().note_processed(raw, &kept, record);
    if hints.is_some() && (context_fixed || crate::fork::context::awareness::vocabulary_sent()) {
        crate::fork::context::awareness::note_used();
        ledger().note_context_used(&kept);
    }
    kept
}

/// S-38's stage (S-16 finding): a dictionary word with accents
/// ([`accent_words`]) that the transcript has without them, or with other
/// ones, takes the dictionary's spelling: `Siobhan` and `siobhan` →
/// `Siobhán`. It is S-20's spelling pass ([`crate::fork::context::hints`])
/// with the dictionary's words as hints, so its safety rules hold: a
/// lowercase word changes only when its letters are the same apart from
/// accents, a capitalized one also by Handy's fuzzy score and threshold,
/// and only letters change. Dictionary rule outputs are never touched and
/// stay findable for stage 9.
pub fn dictionary_accent_stage(text: String, words: &[String], record: &mut TextRecord) -> String {
    let c = crate::fork::context::hints::correct_outside(&text, words, &protected_spans(record));
    for p in &mut record.provenance {
        p.target_start = c.map_offset(p.target_start);
    }
    record.push(Stage::DictionaryAccents, c.fixes);
    c.text
}

/// S-20's stage: transcript words that are a near-caret name misspelled
/// take its spelling ([`crate::fork::context::hints::correct`]).
/// The dictionary rules' outputs are never touched, and stay findable for
/// stage 9 (their offsets follow the edits).
pub fn context_stage(text: String, hints: &[String], record: &mut TextRecord) -> String {
    let c = crate::fork::context::hints::correct_outside(&text, hints, &protected_spans(record));
    for p in &mut record.provenance {
        p.target_start = c.map_offset(p.target_start);
    }
    record.push(Stage::Context, c.fixes);
    c.text
}

fn protected_spans(record: &TextRecord) -> Vec<(usize, usize)> {
    record
        .provenance
        .iter()
        .map(|p| (p.target_start, p.target_start + p.target_text.len()))
        .collect()
}

/// S-36's code-name stage: a spoken dotfile becomes its name
/// ([`crate::fork::context::ide::dotfiles`]), then the exact spoken form of
/// an identifier the editor shows becomes its spelling
/// ([`crate::fork::context::ide::correct`]). Dictionary rule outputs are
/// never touched and stay findable for stage 9.
pub fn identifier_stage(text: String, identifiers: &[String], record: &mut TextRecord) -> String {
    use crate::fork::context::ide;
    let d = ide::dotfiles(&text, identifiers, &protected_spans(record));
    for p in &mut record.provenance {
        p.target_start = d.map_offset(p.target_start);
    }
    let c = ide::correct(&d.text, identifiers, &protected_spans(record));
    for p in &mut record.provenance {
        p.target_start = c.map_offset(p.target_start);
    }
    record.push(Stage::Identifiers, d.fixes + c.fixes);
    c.text
}

/// S-36's file-tag stage: a spoken or written file name the editor shows
/// becomes `@name` ([`file_tags::rewrite`]), outside dictionary rule
/// outputs.
pub fn file_tag_stage(text: String, names: &[String], record: &mut TextRecord) -> String {
    let c = file_tags::rewrite_outside(&text, names, &protected_spans(record));
    for p in &mut record.provenance {
        p.target_start = c.map_offset(p.target_start);
    }
    record.push(Stage::FileTags, c.fixes);
    c.text
}

/// Debug builds log each take's verbatim and kept text when this is set.
#[cfg(debug_assertions)]
pub const TRACE_ENV: &str = "WB_DEBUG_TEXT_TRACE";

/// Stages 3 → 2/4/5 → 7 → 12 on `raw`.
pub fn run(
    raw: &str,
    language: Option<&str>,
    rules: &RuleSet,
    handy: &mut impl FnMut(String) -> String,
) -> (String, TextRecord) {
    let mut record = TextRecord {
        language: language.map(str::to_owned),
        ..TextRecord::default()
    };
    let text = rules::sanitize(raw);

    let mut handy_changes = 0u32;
    let out = lines::per_line_at(&text, |line, offset| {
        let done = rules_then_handy(line, rules, handy);
        handy_changes = handy_changes.saturating_add(done.handy_changes);
        for (hit, at) in done.hits {
            record.provenance.push(Provenance {
                source_form: hit.source_form,
                target_text: hit.to_text.clone(),
                target_start: offset + at,
            });
            record.rule_hits.push(RuleHit {
                rule_id: hit.rule_id,
                from_text: hit.from_text,
                to_text: hit.to_text,
            });
        }
        done.text
    });
    let hits = u32::try_from(record.rule_hits.len()).unwrap_or(u32::MAX);
    record.push(Stage::Rules, hits);
    record.push(Stage::Handy, handy_changes);

    let matcher = commands::Matcher::for_language(language);
    let (text, applied) = if matcher.is_empty() {
        (out, commands::Applied::default())
    } else {
        commands::apply(&matcher, &out)
    };
    record.push(Stage::Commands, applied.commands);
    record.push(Stage::Scratch, applied.scratches);

    let (text, changed) = whitespace::normalize(&text);
    record.push(Stage::Whitespace, changed);
    (text, record)
}

struct Hit {
    rule_id: i64,
    from_text: String,
    source_form: String,
    to_text: String,
}

struct LineDone {
    text: String,
    /// Each rule hit and where its output starts in `text`.
    hits: Vec<(Hit, usize)>,
    handy_changes: u32,
}

fn hit_of(a: &rules::Applied) -> Hit {
    Hit {
        rule_id: a.rule_id,
        from_text: a.matched.clone(),
        source_form: a.source_form.clone(),
        to_text: a.target_text.clone(),
    }
}

/// One line: rules (outputs protected), then Handy's stages, then the
/// outputs restored. If a placeholder does not survive Handy's stages
/// intact, the line is redone with the rule outputs inserted as plain text.
fn rules_then_handy(
    line: &str,
    rules: &RuleSet,
    handy: &mut impl FnMut(String) -> String,
) -> LineDone {
    let unprotected = |handy: &mut dyn FnMut(String) -> String| {
        let (ruled, applied) = rules::apply(rules, line, |r| r.to.clone());
        let text = handy(ruled.clone());
        LineDone {
            handy_changes: changed_words(&ruled, &text),
            hits: applied
                .iter()
                .map(|a| (hit_of(a), a.target_start.min(text.len())))
                .collect(),
            text,
        }
    };
    if rules.is_empty() || placeholders::collides(line) {
        return unprotected(handy);
    }

    let mut ph = Placeholders::new();
    let (protected, applied) = rules::apply(rules, line, |r| {
        ph.protect(&r.to)
            .map_or_else(|| r.to.clone(), |c| c.to_string())
    });
    if applied.is_empty() {
        let text = handy(line.to_string());
        return LineDone {
            handy_changes: changed_words(line, &text),
            hits: Vec::new(),
            text,
        };
    }
    let processed = handy(protected.clone());
    match ph.restore(&protected, &processed) {
        Some((text, restored)) => {
            let mut hits: Vec<(Hit, usize)> = Vec::with_capacity(applied.len());
            for r in restored {
                if let Some(a) = applied.get(r.index) {
                    hits.push((hit_of(a), r.start));
                }
            }
            hits.sort_by_key(|(_, at)| *at);
            LineDone {
                handy_changes: changed_words(&protected, &processed),
                hits,
                text,
            }
        }
        None => {
            log::debug!("text: a protected rule output moved in Handy's stages; applying the rules unprotected");
            unprotected(handy)
        }
    }
}

/// What the stages after the LLM did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Finished {
    /// Stage 9: rule outputs put back.
    pub restored: u32,
    /// Stage 12: lines whose whitespace changed.
    pub whitespace: u32,
    /// Stage 11: the snippets that expanded.
    pub snippet_hits: Vec<SnippetHit>,
}

/// Stages 9 → 12 → 11 on the text the LLM step returned. Pure (the tests
/// call it directly); [`finish`] supplies the provenance and the snippets.
///
/// Stage 12 runs before the snippets so the triggers meet tidy spacing and
/// the saved texts are inserted as written (their own spacing and blank
/// lines kept, never rescanned).
pub fn finish_with(
    text: &str,
    provenance: Option<&[Provenance]>,
    snippets: &SnippetSet,
) -> (String, Finished) {
    let mut done = Finished::default();
    let restored_text = match provenance {
        Some(p) => {
            let (t, n) = rules::restore(text, p);
            done.restored = n;
            t
        }
        None => text.to_owned(),
    };
    let (tidy, changed) = whitespace::normalize(&restored_text);
    done.whitespace = changed;
    if snippets.is_empty() {
        return (tidy, done);
    }
    match catch_unwind(AssertUnwindSafe(|| snippets::expand(snippets, &tidy))) {
        Ok((out, hits)) => {
            done.snippet_hits = hits;
            (out, done)
        }
        Err(_) => {
            log::error!("text: the snippet stage panicked; inserting the text without snippets");
            (tidy, done)
        }
    }
}

/// Stages after the LLM, inside `fork::insert::deliver`: stage 9 for a
/// rewrite registered with [`note_rewrite`], stage 12, then stage 11 (the
/// snippets). Returns the text to insert. The only caller that may expand
/// snippets: it runs once per delivery, so a saved text is never scanned
/// twice and never saved to History as the spoken words.
pub fn finish(text: String) -> String {
    finish_stages(text, true)
}

/// Stages 9 and 12 only, for the LLM step itself (S-12a's
/// `restore_spellings`, before History saves the take): the snippets wait
/// for [`finish`] in `deliver`.
pub fn after_llm(text: String) -> String {
    finish_stages(text, false)
}

fn finish_stages(text: String, expand_snippets: bool) -> String {
    let (provenance, entry_id) = {
        let mut l = ledger();
        (l.take_rewrite(&text), l.awaiting_entry(&text))
    };
    let (snippets, sink) = if expand_snippets {
        active_snippets()
    } else {
        (Arc::new(SnippetSet::empty()), None)
    };
    let (out, done) = finish_with(&text, provenance.as_deref(), &snippets);
    // S-36: after the LLM step (not in `deliver`, where snippet text must
    // stay as saved), tag again the file names the rewrite may have
    // written out ("index.ts") or untagged. Idempotent on tagged text.
    let out = match (!expand_snippets)
        .then(crate::fork::context::awareness::file_names)
        .flatten()
    {
        Some(names) => {
            let tagged = file_tags::rewrite(&out, &names);
            if tagged.fixes > 0 {
                log::info!(
                    "text: after the LLM {}={}",
                    Stage::FileTags.as_str(),
                    tagged.fixes
                );
            }
            tagged.text
        }
        None => out,
    };
    if provenance.is_some() || done.whitespace > 0 || !done.snippet_hits.is_empty() {
        log::info!(
            "text: after the LLM {}={} {}={} {}={}",
            Stage::RulesRestored.as_str(),
            done.restored,
            Stage::Whitespace.as_str(),
            done.whitespace,
            Stage::Snippets.as_str(),
            done.snippet_hits.len()
        );
    }
    if out != text {
        ledger().rekey_awaiting(&text, &out);
    }
    if let Some(sink) = sink {
        if !done.snippet_hits.is_empty() || out != text {
            sink(SnippetUse {
                entry_id,
                hits: done.snippet_hits,
                inserted: (out != text).then(|| out.clone()),
            });
        }
    }
    out
}

/// For the LLM step (S-12a): `output` is the LLM's rewrite of `input` (the
/// text [`process`] kept). [`after_llm`] (or [`finish`]) then restores the dictionary rule
/// outputs the rewrite undid.
pub fn note_rewrite(input: &str, output: &str) {
    let mut l = ledger();
    if let Some(provenance) = l.provenance_for(input) {
        l.note_rewrite(output, provenance);
    }
}
