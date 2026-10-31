// SPDX-License-Identifier: GPL-3.0-or-later
//! What the fork learns about a take before Handy saves it, and how it finds
//! that take again when the save hook runs.
//!
//! Handy's flow for one dictation (`actions.rs`):
//!
//! 1. `TranscribeAction::start` → [`super::on_record_start`]: the frontmost
//!    app, the time and local day are noted as the *current* take.
//! 2. The stop task gets the samples and names the WAV through
//!    `fork::history_file_name(sample_count)`: the current take is *bound*
//!    to that file name with its speech time.
//! 3. Handy transcribes; the text pipeline (`fork::text::process`, from
//!    `finish_text`) hands the verbatim ASR text, the text Handy keeps and
//!    what the pipeline did to [`Ledger::note_processed`].
//! 4. `HistoryManager::save_entry` → `fork::db::on_saved` finds the bound
//!    take by the entry's `file_name` and its verbatim text and pipeline
//!    record by the entry's `transcription_text`, then registers the entry
//!    for its paste outcome.
//! 5. `fork::insert` delivers the text and reports the outcome, matched to
//!    the entry by its final text.
//!
//! Everything is bounded (the last [`KEEP`] of each) and keyed, never "the
//! latest", so a cancelled take or a retry running beside a dictation cannot
//! give one entry another take's app or text. A take that started while the
//! previous one was still recording is the one known gap: Handy does not
//! allow it, and if it ever did, the binding falls back to the frontmost app
//! at stop.

use std::collections::VecDeque;
use std::sync::Mutex;

use super::categories::{self, AppClass};
use super::frontmost::FrontApp;
use crate::fork::text::counts::TextRecord;
use crate::fork::text::rules::Provenance;

/// How many entries each queue keeps.
pub const KEEP: usize = 16;

/// Where a take's text came from (`entry_meta.source`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TakeSource {
    #[default]
    Dictation,
    File,
    Practice,
    Command,
    Transform,
}

impl TakeSource {
    pub fn as_str(self) -> &'static str {
        match self {
            TakeSource::Dictation => "dictation",
            TakeSource::File => "file",
            TakeSource::Practice => "practice",
            TakeSource::Command => "command",
            TakeSource::Transform => "transform",
        }
    }

    pub fn parse(s: &str) -> Option<TakeSource> {
        Some(match s {
            "dictation" => TakeSource::Dictation,
            "file" => TakeSource::File,
            "practice" => TakeSource::Practice,
            "command" => TakeSource::Command,
            "transform" => TakeSource::Transform,
            _ => return None,
        })
    }

    /// Only plain dictations feed the stats (DEC-11, C-212): files, practice
    /// takes, commands and transforms do not.
    pub fn counts_in_stats(self) -> bool {
        self == TakeSource::Dictation
    }
}

/// A take as known at record start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TakeContext {
    /// Unix ms at record start (`entry_meta.created_at`).
    pub started_at_ms: i64,
    /// Local day at record start (`entry_meta.day`).
    pub day: String,
    pub app: Option<FrontApp>,
    pub class: AppClass,
    pub source: TakeSource,
}

impl TakeContext {
    pub fn new(started_at_ms: i64, day: String, app: Option<FrontApp>) -> Self {
        let class = match &app {
            Some(a) => categories::classify(a.bundle_id.as_deref(), a.name.as_deref()),
            None => AppClass::OTHER,
        };
        Self {
            started_at_ms,
            day,
            app,
            class,
            source: TakeSource::Dictation,
        }
    }
}

/// A take whose recording stopped and whose WAV has a name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundTake {
    pub file_name: String,
    pub context: TakeContext,
    /// Speech time Handy kept (VAD-trimmed samples / 16 kHz).
    pub audio_ms: u64,
    /// The WAV could not be written (disk full): the row keeps the text only.
    pub audio_missing: bool,
}

/// One transcription as the text pipeline left it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Processed {
    /// The text Handy keeps (the history row's `transcription_text`).
    pub kept: String,
    /// The verbatim ASR text.
    pub verbatim: String,
    pub record: TextRecord,
}

#[derive(Debug, Default)]
pub struct Ledger {
    current: Option<TakeContext>,
    bound: VecDeque<BoundTake>,
    processed: VecDeque<Processed>,
    /// (LLM output, rule provenance of its input) for stage 9.
    rewrites: VecDeque<(String, Vec<Provenance>)>,
    /// (entry id, final text) waiting for their paste outcome.
    awaiting: VecDeque<(i64, String)>,
}

fn push_bounded<T>(q: &mut VecDeque<T>, v: T) {
    if q.len() == KEEP {
        q.pop_front();
    }
    q.push_back(v);
}

impl Ledger {
    pub const fn new() -> Self {
        Self {
            current: None,
            bound: VecDeque::new(),
            processed: VecDeque::new(),
            rewrites: VecDeque::new(),
            awaiting: VecDeque::new(),
        }
    }

    /// Step 1: a recording started.
    pub fn record_started(&mut self, ctx: TakeContext) {
        self.current = Some(ctx);
    }

    /// Step 2: the recording that just stopped gets its WAV name. Without a
    /// current take (the hook did not run), `fallback` builds one.
    pub fn bind(&mut self, file_name: &str, audio_ms: u64, fallback: impl FnOnce() -> TakeContext) {
        let context = self.current.take().unwrap_or_else(fallback);
        push_bounded(
            &mut self.bound,
            BoundTake {
                file_name: file_name.to_owned(),
                context,
                audio_ms,
                audio_missing: false,
            },
        );
    }

    /// The WAV of a bound take could not be written.
    pub fn mark_audio_missing(&mut self, file_name: &str) -> bool {
        match self.bound.iter_mut().find(|b| b.file_name == file_name) {
            Some(b) => {
                b.audio_missing = true;
                true
            }
            None => false,
        }
    }

    /// Step 4: the bound take for a saved entry (consumed).
    pub fn take_bound(&mut self, file_name: &str) -> Option<BoundTake> {
        let at = self.bound.iter().position(|b| b.file_name == file_name)?;
        self.bound.remove(at)
    }

    /// Step 3: the pipeline turned verbatim ASR text `verbatim` into `kept`.
    pub fn note_processed(&mut self, verbatim: String, kept: &str, record: TextRecord) {
        push_bounded(
            &mut self.processed,
            Processed {
                kept: kept.to_owned(),
                verbatim,
                record,
            },
        );
    }

    /// [`Ledger::note_processed`] without a pipeline record.
    pub fn note_verbatim(&mut self, verbatim: String, kept: &str) {
        self.note_processed(verbatim, kept, TextRecord::default());
    }

    /// The newest transcription kept as `kept` (consumed).
    pub fn take_processed(&mut self, kept: &str) -> Option<Processed> {
        let at = self.processed.iter().rposition(|p| p.kept == kept)?;
        self.processed.remove(at)
    }

    /// The verbatim text for the newest transcription Handy kept as `kept`
    /// (consumed).
    pub fn take_verbatim(&mut self, kept: &str) -> Option<String> {
        self.take_processed(kept).map(|p| p.verbatim)
    }

    /// The rule provenance of the newest transcription kept as `kept`, if
    /// any rule applied (not consumed).
    pub fn provenance_for(&self, kept: &str) -> Option<Vec<Provenance>> {
        self.processed
            .iter()
            .rev()
            .find(|p| p.kept == kept)
            .map(|p| p.record.provenance.clone())
            .filter(|p| !p.is_empty())
    }

    /// The LLM rewrote a kept text into `output`; stage 9 restores
    /// `provenance` in it.
    pub fn note_rewrite(&mut self, output: &str, provenance: Vec<Provenance>) {
        push_bounded(&mut self.rewrites, (output.to_owned(), provenance));
    }

    /// The provenance registered for `output` (consumed).
    pub fn take_rewrite(&mut self, output: &str) -> Option<Vec<Provenance>> {
        let at = self.rewrites.iter().rposition(|(o, _)| o == output)?;
        self.rewrites.remove(at).map(|(_, p)| p)
    }

    /// The text of an entry waiting for its paste outcome changed on its way
    /// to the field (stages after the LLM).
    pub fn rekey_awaiting(&mut self, old: &str, new: &str) {
        if let Some(slot) = self.awaiting.iter_mut().find(|(_, t)| t == old) {
            slot.1 = new.to_owned();
        }
    }

    /// Step 4: `entry_id` will be pasted as `final_text`.
    pub fn await_outcome(&mut self, entry_id: i64, final_text: &str) {
        push_bounded(&mut self.awaiting, (entry_id, final_text.to_owned()));
    }

    /// Step 5: the oldest entry waiting with this text (consumed). The paste
    /// may carry Handy's trailing space.
    pub fn take_awaiting(&mut self, pasted: &str) -> Option<i64> {
        let pasted = pasted.trim_end();
        let at = self
            .awaiting
            .iter()
            .position(|(_, text)| text.trim_end() == pasted)?;
        self.awaiting.remove(at).map(|(id, _)| id)
    }
}

/// The process's ledger.
pub static LEDGER: Mutex<Ledger> = Mutex::new(Ledger::new());

pub fn ledger() -> std::sync::MutexGuard<'static, Ledger> {
    LEDGER.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(ms: i64, bundle: &str) -> TakeContext {
        TakeContext::new(
            ms,
            "2026-10-30".into(),
            Some(FrontApp {
                bundle_id: Some(bundle.into()),
                name: None,
            }),
        )
    }

    #[test]
    fn context_classifies_its_app() {
        let c = ctx(1, "com.apple.mail");
        assert_eq!(c.class.style_group, categories::StyleGroup::Email);
        let none = TakeContext::new(1, "2026-10-30".into(), None);
        assert_eq!(none.class, AppClass::OTHER);
    }

    #[test]
    fn a_take_binds_to_its_file_and_is_found_once() {
        let mut l = Ledger::new();
        l.record_started(ctx(1, "com.apple.mail"));
        l.bind("a.wav", 1500, || panic!("no fallback needed"));
        let b = l.take_bound("a.wav").unwrap();
        assert_eq!(b.context.started_at_ms, 1);
        assert_eq!(b.audio_ms, 1500);
        assert!(!b.audio_missing);
        assert!(l.take_bound("a.wav").is_none());
    }

    #[test]
    fn a_cancelled_take_is_replaced_by_the_next_one() {
        let mut l = Ledger::new();
        l.record_started(ctx(1, "com.apple.mail"));
        // Cancelled: never bound. The next take starts in Slack.
        l.record_started(ctx(2, "com.tinyspeck.slackmacgap"));
        l.bind("b.wav", 10, || unreachable!());
        assert_eq!(l.take_bound("b.wav").unwrap().context.started_at_ms, 2);
    }

    #[test]
    fn binding_without_a_start_uses_the_fallback() {
        let mut l = Ledger::new();
        l.bind("c.wav", 10, || ctx(9, "com.apple.Notes"));
        assert_eq!(l.take_bound("c.wav").unwrap().context.started_at_ms, 9);
    }

    #[test]
    fn two_takes_in_flight_keep_their_own_context() {
        let mut l = Ledger::new();
        l.record_started(ctx(1, "com.apple.mail"));
        l.bind("1.wav", 10, || unreachable!());
        l.record_started(ctx(2, "com.apple.MobileSMS"));
        l.bind("2.wav", 20, || unreachable!());
        // Saved out of order.
        assert_eq!(l.take_bound("2.wav").unwrap().audio_ms, 20);
        assert_eq!(l.take_bound("1.wav").unwrap().audio_ms, 10);
    }

    #[test]
    fn missing_audio_is_marked_on_the_bound_take() {
        let mut l = Ledger::new();
        l.bind("d.wav", 10, || ctx(1, "x"));
        assert!(l.mark_audio_missing("d.wav"));
        assert!(!l.mark_audio_missing("nope.wav"));
        assert!(l.take_bound("d.wav").unwrap().audio_missing);
    }

    #[test]
    fn verbatim_is_matched_by_the_kept_text() {
        let mut l = Ledger::new();
        l.note_verbatim("um so hello".into(), "So hello");
        l.note_verbatim("retry text uh".into(), "Retry text");
        assert_eq!(l.take_verbatim("So hello").as_deref(), Some("um so hello"));
        assert_eq!(l.take_verbatim("So hello"), None);
        assert_eq!(l.take_verbatim("unknown"), None);
        assert_eq!(
            l.take_verbatim("Retry text").as_deref(),
            Some("retry text uh")
        );
    }

    #[test]
    fn processed_takes_carry_their_record_and_rewrites_their_provenance() {
        use crate::fork::text::counts::{RuleHit, Stage};
        let mut l = Ledger::new();
        let mut record = TextRecord::default();
        record.push(Stage::Rules, 1);
        record.rule_hits.push(RuleHit {
            rule_id: 4,
            from_text: "voice typer".into(),
            to_text: "Voicetypr".into(),
        });
        record.provenance.push(Provenance {
            source_form: "voice typer".into(),
            target_text: "Voicetypr".into(),
            target_start: 0,
        });
        l.note_processed(
            "voice typer rocks".into(),
            "Voicetypr rocks",
            record.clone(),
        );
        let p = l.provenance_for("Voicetypr rocks").unwrap();
        l.note_rewrite("Voice typer rocks!", p.clone());
        assert_eq!(l.take_rewrite("Voice typer rocks!"), Some(p));
        assert_eq!(l.take_rewrite("Voice typer rocks!"), None);
        let taken = l.take_processed("Voicetypr rocks").unwrap();
        assert_eq!(taken.verbatim, "voice typer rocks");
        assert_eq!(taken.record, record);
        // A take without rule hits has no provenance to restore.
        l.note_verbatim("plain".into(), "Plain");
        assert!(l.provenance_for("Plain").is_none());
    }

    #[test]
    fn an_awaiting_entry_follows_its_text() {
        let mut l = Ledger::new();
        l.await_outcome(3, "a  b");
        l.rekey_awaiting("a  b", "a b");
        assert_eq!(l.take_awaiting("a b"), Some(3));
    }

    #[test]
    fn outcomes_are_matched_by_text_ignoring_the_trailing_space() {
        let mut l = Ledger::new();
        l.await_outcome(7, "Hello there.");
        l.await_outcome(8, "Second");
        assert_eq!(l.take_awaiting("Second "), Some(8));
        assert_eq!(l.take_awaiting("Hello there."), Some(7));
        assert_eq!(l.take_awaiting("Hello there."), None);
    }

    #[test]
    fn queues_are_bounded() {
        let mut l = Ledger::new();
        for i in 0..(KEEP as i64 + 5) {
            l.await_outcome(i, &format!("t{i}"));
            l.bind(&format!("{i}.wav"), 0, || ctx(i, "x"));
            l.note_verbatim(format!("v{i}"), &format!("k{i}"));
        }
        assert_eq!(l.take_awaiting("t0"), None);
        assert_eq!(l.take_awaiting("t5"), Some(5));
        assert!(l.take_bound("0.wav").is_none());
        assert!(l.take_bound("5.wav").is_some());
        assert!(l.take_verbatim("k0").is_none());
    }

    #[test]
    fn source_names_match_the_db_check() {
        let sql = include_str!("../db/migrations/0001_init.sql");
        for s in [
            TakeSource::Dictation,
            TakeSource::File,
            TakeSource::Practice,
            TakeSource::Command,
            TakeSource::Transform,
        ] {
            assert!(sql.contains(&format!("'{}'", s.as_str())));
            assert_eq!(TakeSource::parse(s.as_str()), Some(s));
        }
        assert!(TakeSource::Dictation.counts_in_stats());
        assert!(!TakeSource::File.counts_in_stats());
        assert!(!TakeSource::Practice.counts_in_stats());
    }
}
