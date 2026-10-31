// SPDX-License-Identifier: GPL-3.0-or-later
//! Context awareness for one take (S-20, DEC-24): the hints of the take in
//! flight, in memory only, and when they go.
//!
//! 1. Record start ([`on_record_start`]): with the setting on, one read is
//!    queued on the AX thread ([`super::caret`]); it never waits. The read's
//!    hints land in the slot for this take only (a newer take's slot is
//!    never overwritten by an older read).
//! 2. Key-up ([`on_stopped`], from `fork::take_file_name`): the slot is
//!    armed. Only an armed slot gives hints, so a file transcription or a
//!    retry outside a dictation never sees any.
//! 3. The take uses them: the recognizer's vocabulary
//!    ([`vocabulary`], `managers/transcription.rs` hook), the spelling stage
//!    (`fork::text::process`), the cleanup prompt (`fork::llm::post_process`).
//!    Any use marks the take ([`note_used`]): the pill shows the context
//!    glyph and the history row says "Context used".
//! 4. The take ends ([`on_take_over`]: Handy hid its bar after the paste,
//!    an empty take, an error or a cancel): the slot is cleared. A slot left
//!    armed longer than [`ARMED_TTL`] is never used and is dropped.
//!
//! Nothing here is written to disk or logged: the log gets counts, times
//! and skip reasons only.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::caret::{Read, Skip};

/// An armed slot older than this is dropped (a take that never ended).
pub const ARMED_TTL: Duration = Duration::from_secs(120);

#[derive(Debug, Default)]
struct Slot {
    generation: u64,
    /// `None` while the read runs (or when it found nothing).
    hints: Option<Vec<String>>,
    armed_at: Option<Instant>,
    used: bool,
    vocabulary_sent: bool,
}

static SLOT: Mutex<Option<Slot>> = Mutex::new(None);
static GENERATION: Mutex<u64> = Mutex::new(0);

fn slot() -> std::sync::MutexGuard<'static, Option<Slot>> {
    SLOT.lock().unwrap_or_else(|e| e.into_inner())
}

/// A new take: its slot replaces the last one. Returns its generation.
pub fn begin() -> u64 {
    let generation = {
        let mut g = GENERATION.lock().unwrap_or_else(|e| e.into_inner());
        *g += 1;
        *g
    };
    *slot() = Some(Slot {
        generation,
        ..Slot::default()
    });
    generation
}

/// The read for take `generation` finished.
pub fn land(generation: u64, read: Read) {
    log::info!("{}", log_line(&read));
    let mut s = slot();
    if let Some(s) = s.as_mut().filter(|s| s.generation == generation) {
        s.hints = (!read.hints.is_empty()).then_some(read.hints);
    }
}

/// The one log line of a read: the outcome, counts and time, never a hint
/// or any of the text.
pub fn log_line(read: &Read) -> String {
    format!(
        "context: read {} ({} char(s), {} hint(s), {:.1} ms)",
        read.skip.map(Skip::as_str).unwrap_or("ok"),
        read.chars,
        read.hints.len(),
        read.ms
    )
}

/// Key-up of the running take.
pub fn on_stopped() {
    if let Some(s) = slot().as_mut() {
        s.armed_at = Some(Instant::now());
    }
}

/// The take ended: its hints go.
pub fn on_take_over() {
    *slot() = None;
}

/// The armed take's hints (a copy), if any.
pub fn hints() -> Option<Vec<String>> {
    let mut guard = slot();
    let s = guard.as_ref()?;
    let armed = s.armed_at?;
    if armed.elapsed() > ARMED_TTL {
        *guard = None;
        return None;
    }
    s.hints.clone()
}

/// The armed take used its hints: the pill shows the glyph (once).
pub fn note_used() {
    let first = {
        let mut g = slot();
        match g.as_mut() {
            Some(s) if !s.used => {
                s.used = true;
                true
            }
            _ => false,
        }
    };
    if first {
        log::info!("context: this take used its hints");
        crate::fork::pill::context_used();
    }
}

/// Whether the armed take used its hints so far.
pub fn used() -> bool {
    slot().as_ref().is_some_and(|s| s.used)
}

/// Whether the recognizer got hints for this take.
pub fn vocabulary_sent() -> bool {
    slot().as_ref().is_some_and(|s| s.vocabulary_sent)
}

/// The `managers/transcription.rs` hook: the vocabulary for a model that
/// takes one (`supports`), with the armed take's hints after the user's
/// words. Unchanged when the model takes none or there are no hints.
pub fn vocabulary(supports: bool, custom_words: Vec<String>) -> Vec<String> {
    if !supports {
        return custom_words;
    }
    let Some(hints) = hints() else {
        return custom_words;
    };
    let out = super::hints::vocabulary_terms(&custom_words, &hints);
    if out.len() > custom_words.len() {
        if let Some(s) = slot().as_mut() {
            s.vocabulary_sent = true;
        }
        note_used();
    }
    out
}

/// Record start (`fork::on_record_start`): queues the read when context
/// awareness is on (`enabled`). `languages`: the user's spoken languages.
pub fn on_record_start(enabled: bool, languages: Vec<String>) {
    let generation = begin();
    if !enabled {
        land(generation, Read::skipped(Skip::Off, 0.0));
        return;
    }
    #[cfg(target_os = "macos")]
    {
        if !crate::fork::ax::accessibility_trusted() {
            land(generation, Read::skipped(Skip::NoAccess, 0.0));
            return;
        }
        crate::fork::ax::thread::post(move |ctx| {
            let read = super::caret::mac::read(ctx, &languages);
            land(generation, read);
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = languages;
        land(generation, Read::skipped(Skip::NoAccess, 0.0));
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The slot is process-wide; tests that use it hold this.
    pub(crate) static LOCK: Mutex<()> = Mutex::new(());

    pub(crate) fn lock() -> std::sync::MutexGuard<'static, ()> {
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn read(hints: &[&str]) -> Read {
        Read {
            hints: hints.iter().map(|h| h.to_string()).collect(),
            chars: 120,
            ms: 3.0,
            skip: None,
        }
    }

    /// A take whose read found `hints`, stopped.
    pub(crate) fn armed_take(hints: &[&str]) {
        let g = begin();
        land(g, read(hints));
        on_stopped();
    }

    #[test]
    fn hints_are_given_only_between_key_up_and_the_end_of_the_take() {
        let _l = lock();
        let g = begin();
        land(g, read(&["Siobhán"]));
        // Not armed yet: a retry or a file job during the recording sees
        // nothing.
        assert_eq!(hints(), None);
        on_stopped();
        assert_eq!(hints(), Some(vec!["Siobhán".to_string()]));
        on_take_over();
        assert_eq!(hints(), None);
        assert!(!used());
    }

    #[test]
    fn an_older_read_never_fills_a_newer_take() {
        let _l = lock();
        let old = begin();
        let new = begin();
        land(old, read(&["Old"]));
        on_stopped();
        assert_eq!(hints(), None);
        land(new, read(&["New"]));
        assert_eq!(hints(), Some(vec!["New".to_string()]));
        on_take_over();
    }

    #[test]
    fn a_skipped_or_disabled_read_gives_nothing() {
        let _l = lock();
        on_record_start(false, Vec::new());
        on_stopped();
        assert_eq!(hints(), None);
        let g = begin();
        land(g, Read::skipped(Skip::Timeout, 140.0));
        on_stopped();
        assert_eq!(hints(), None);
        on_take_over();
    }

    #[test]
    fn vocabulary_only_for_models_that_take_one() {
        let _l = lock();
        armed_take(&["Kowalczyk"]);
        let words = vec!["Wispr Bro".to_string()];
        assert_eq!(vocabulary(false, words.clone()), words);
        assert!(!used());
        assert_eq!(vocabulary(true, words.clone()), ["Wispr Bro", "Kowalczyk"]);
        assert!(used() && vocabulary_sent());
        on_take_over();
        // No take: Handy's words unchanged.
        assert_eq!(vocabulary(true, words.clone()), words);
    }

    #[test]
    fn the_log_line_has_counts_only() {
        let line = log_line(&read(&["Siobhán", "Kowalczyk"]));
        assert_eq!(line, "context: read ok (120 char(s), 2 hint(s), 3.0 ms)");
        assert_eq!(
            log_line(&Read::skipped(Skip::PasswordManager, 0.4)),
            "context: read password_manager (0 char(s), 0 hint(s), 0.4 ms)"
        );
    }

    #[test]
    fn a_stale_armed_slot_is_dropped() {
        let _l = lock();
        armed_take(&["Kowalczyk"]);
        if let Some(s) = slot().as_mut() {
            s.armed_at = Instant::now().checked_sub(ARMED_TTL + Duration::from_secs(1));
        }
        assert_eq!(hints(), None);
        assert!(slot().is_none(), "the slot is gone, not just hidden");
    }
}
