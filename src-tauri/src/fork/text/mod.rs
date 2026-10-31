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
//! [`note_rewrite`]), and stage 12 runs again. Stages 10 (style rules) and
//! 11 (snippets) are S-19a and S-17; stage 13 (Shift+Return when typing)
//! is S-04's typing path.
//!
//! Every stage is fail-open: a panic in a fork stage falls back to Handy's
//! own result for the whole text. Per take, the stage order and change
//! counts and the rules that applied go to the history row
//! (`entry_meta.text_stages`, `fix_count_dictionary`, `corrections`).

pub mod commands;
pub mod counts;
pub mod fold;
pub mod lines;
pub mod placeholders;
pub mod rules;
pub mod scratch;
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

type RuleSource = Arc<dyn Fn() -> Vec<Rule> + Send + Sync>;

static SOURCE: RwLock<Option<RuleSource>> = RwLock::new(None);
static COMPILED: Mutex<Option<(Vec<Rule>, Arc<RuleSet>)>> = Mutex::new(None);

/// Where the active dictionary rules come from (`fork.db`, set by
/// `fork::on_ready`). Without a source no rule applies (headless CLI).
pub fn install_rule_source(source: impl Fn() -> Vec<Rule> + Send + Sync + 'static) {
    *SOURCE.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(source));
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
    let outcome = catch_unwind(AssertUnwindSafe(|| run(&raw, lang, &rules, &mut handy)));
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
    ledger().note_processed(raw, &kept, record);
    kept
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
    let mut record = TextRecord::default();
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

/// Stages after the LLM, inside `fork::insert::deliver`: stage 9 for a
/// rewrite registered with [`note_rewrite`], then stage 12. Returns the
/// text to insert.
pub fn finish(text: String) -> String {
    let provenance = ledger().take_rewrite(&text);
    let (restored_text, restored) = match &provenance {
        Some(p) => rules::restore(&text, p),
        None => (text.clone(), 0),
    };
    let (out, changed) = whitespace::normalize(&restored_text);
    if provenance.is_some() || changed > 0 {
        log::info!(
            "text: after the LLM {}={restored} {}={changed}",
            Stage::RulesRestored.as_str(),
            Stage::Whitespace.as_str()
        );
    }
    if out != text {
        ledger().rekey_awaiting(&text, &out);
    }
    out
}

/// For the LLM step (S-12a): `output` is the LLM's rewrite of `input` (the
/// text [`process`] kept). [`finish`] then restores the dictionary rule
/// outputs the rewrite undid.
pub fn note_rewrite(input: &str, output: &str) {
    let mut l = ledger();
    if let Some(provenance) = l.provenance_for(input) {
        l.note_rewrite(output, provenance);
    }
}
