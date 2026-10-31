// SPDX-License-Identifier: GPL-3.0-or-later
// Ported from SpeakoFlow `src-tauri/src/dictation_recovery.rs`
// (https://github.com/AbhishekBarali/SpeakoFlow, commit
// 9818a17fcd124e5f1ea6d34780ddea9b154322d8), MIT, Copyright (c) 2026
// Abhishek Barali (LICENSES/MIT-SpeakoFlow.txt).
// Modified by Wispr Bro contributors, 2026-10-31: only cancels are offered
// (failed takes are retried from History); the offer lives a fixed 4 s on a
// caller-driven clock instead of the overlay epoch; what is left to do is the
// audio in memory, Handy's WAV, or a saved row; the row is learned from the
// save hook instead of a watch channel; Handy's short-take padding. See
// docs/fork/PORTS.md.
//! The undo window, pure (R5 S-06): a cancelled take's audio is kept for
//! exactly [`UNDO_WINDOW_MS`], then dropped. Undo takes it out of the window
//! once; a newer cancel replaces it (one bar, one offer, as in SpeakoFlow's
//! `OfferSlot`). Driven by the caller's monotonic clock, so the tests run on
//! a fake one.

/// Milliseconds on the caller's monotonic clock.
pub type Ms = u64;

/// How long a cancelled take can be brought back (and how long the bar
/// shows "Cancelled · Undo").
pub const UNDO_WINDOW_MS: Ms = 4_000;

const SAMPLE_RATE: usize = 16_000;

/// Less speech than this is not worth offering back (SpeakoFlow's
/// `MIN_RECOVERABLE_SAMPLES`): Handy's recorder keeps only what its VAD
/// called speech, and under half a second of voice a cancel is almost always
/// a key pressed by mistake and cancelled on purpose.
pub const MIN_UNDO_SAMPLES: usize = SAMPLE_RATE / 2;

/// Whether a take cancelled while recording, with `samples` of speech, is
/// kept for Undo.
pub fn worth_keeping(samples: usize) -> bool {
    samples >= MIN_UNDO_SAMPLES
}

/// Where a cancelled take's audio is.
#[derive(Clone, Debug, PartialEq)]
pub enum Audio {
    /// Cancelled while recording: the samples (16 kHz mono, padded the way
    /// Handy pads a short take), in memory only. Nothing is on disk.
    Samples(Vec<f32>),
    /// Cancelled while transcribing: Handy already wrote the WAV before it
    /// started transcribing (R5 §5.4); it is reused.
    File,
}

/// A cancelled take that can still be brought back.
#[derive(Clone, Debug, PartialEq)]
pub struct Kept {
    /// The take's WAV name (already bound to the take's app and day).
    pub file_name: String,
    pub audio: Audio,
    /// Handy saved the History row before the cancel landed (a cancel just
    /// before the paste): Undo delivers its text without transcribing.
    pub entry_id: Option<i64>,
    /// AX capture of the element focused at the cancel (S-05): Undo pastes
    /// there only if it still has focus.
    pub target: Option<u64>,
}

impl Kept {
    /// What to do with it when the window closes: drop the samples, or
    /// delete a WAV no History row points at.
    pub fn leftover_file(&self) -> Option<&str> {
        match (&self.audio, self.entry_id) {
            (Audio::File, None) => Some(&self.file_name),
            _ => None,
        }
    }
}

/// What Undo does with a kept take.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plan {
    /// The row exists with its text: deliver it.
    Deliver { entry_id: i64 },
    /// Write the samples to the WAV, save an empty row, transcribe it.
    WriteAndTranscribe,
    /// The WAV is there: save an empty row and transcribe it.
    Transcribe,
}

pub fn plan(kept: &Kept) -> Plan {
    match (kept.entry_id, &kept.audio) {
        (Some(entry_id), _) => Plan::Deliver { entry_id },
        (None, Audio::Samples(_)) => Plan::WriteAndTranscribe,
        (None, Audio::File) => Plan::Transcribe,
    }
}

/// At most one cancelled take is kept: a newer cancel replaces it.
#[derive(Debug, Default)]
pub struct Window {
    kept: Option<(Kept, Ms)>,
    seq: u64,
}

impl Window {
    pub const fn new() -> Window {
        Window { kept: None, seq: 0 }
    }

    /// Keeps `kept` from `now` for [`UNDO_WINDOW_MS`]. Returns its sequence
    /// number (for [`Window::expire_seq`]) and the take it replaced.
    pub fn keep(&mut self, now: Ms, kept: Kept) -> (u64, Option<Kept>) {
        self.seq += 1;
        let old = self.kept.replace((kept, now + UNDO_WINDOW_MS));
        (self.seq, old.map(|(k, _)| k))
    }

    /// The kept take, if still within its window at `now`.
    pub fn peek(&self, now: Ms) -> Option<&Kept> {
        match &self.kept {
            Some((k, until)) if now < *until => Some(k),
            _ => None,
        }
    }

    /// Undo: takes the kept take out, once, if still within its window.
    pub fn take(&mut self, now: Ms) -> Option<Kept> {
        match &self.kept {
            Some((_, until)) if now < *until => self.kept.take().map(|(k, _)| k),
            _ => None,
        }
    }

    /// Drops the kept take if its window closed by `now`.
    pub fn expire(&mut self, now: Ms) -> Option<Kept> {
        match &self.kept {
            Some((_, until)) if now >= *until => self.kept.take().map(|(k, _)| k),
            _ => None,
        }
    }

    /// The timer of keep number `seq` fired: drops that take (not a newer
    /// one) if its window closed.
    pub fn expire_seq(&mut self, now: Ms, seq: u64) -> Option<Kept> {
        if seq == self.seq {
            self.expire(now)
        } else {
            None
        }
    }

    /// Dismissed (✕, Esc on the message) or a new take started.
    pub fn discard(&mut self) -> Option<Kept> {
        self.kept.take().map(|(k, _)| k)
    }

    /// Handy saved the History row of the kept take after the cancel was
    /// seen (its paste was then skipped).
    pub fn note_saved(&mut self, file_name: &str, entry_id: i64) {
        if let Some((k, _)) = self.kept.as_mut() {
            if k.file_name == file_name {
                k.entry_id = Some(entry_id);
            }
        }
    }

    /// When the kept take's window closes.
    pub fn deadline(&self) -> Option<Ms> {
        self.kept.as_ref().map(|(_, until)| *until)
    }
}

/// Pads a short take the way Handy's `stop_recording` does (under 1 s of
/// 16 kHz audio → 1.25 s), so Undo transcribes what a normal stop would.
pub fn pad(mut samples: Vec<f32>) -> Vec<f32> {
    if !samples.is_empty() && samples.len() < SAMPLE_RATE {
        samples.resize(SAMPLE_RATE * 5 / 4, 0.0);
    }
    samples
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(n: usize) -> Kept {
        Kept {
            file_name: "wispr-bro-1.wav".into(),
            audio: Audio::Samples(vec![0.1; n]),
            entry_id: None,
            target: Some(7),
        }
    }

    fn file(name: &str) -> Kept {
        Kept {
            file_name: name.into(),
            audio: Audio::File,
            entry_id: None,
            target: None,
        }
    }

    #[test]
    fn samples_are_kept_exactly_four_seconds_then_dropped() {
        let mut w = Window::new();
        let (seq, replaced) = w.keep(10_000, samples(16_000));
        assert!(replaced.is_none());
        assert_eq!(w.deadline(), Some(10_000 + UNDO_WINDOW_MS));
        // Not yet: the last millisecond of the window still has it.
        assert!(w.expire_seq(13_999, seq).is_none());
        assert!(w.peek(13_999).is_some());
        // At 4 s it is gone, and Undo finds nothing.
        let dropped = w.expire_seq(14_000, seq).expect("dropped at 4 s");
        assert_eq!(dropped, samples(16_000));
        assert!(w.peek(14_000).is_none());
        assert!(w.take(14_000).is_none());
        assert_eq!(w.deadline(), None);
    }

    #[test]
    fn undo_takes_it_once_within_the_window() {
        let mut w = Window::new();
        w.keep(0, samples(100));
        assert!(w.take(UNDO_WINDOW_MS - 1).is_some());
        assert!(w.take(UNDO_WINDOW_MS - 1).is_none());
        // Its timer later finds nothing to drop.
        assert!(w.expire_seq(UNDO_WINDOW_MS, 1).is_none());
    }

    #[test]
    fn undo_after_the_window_fails_even_before_the_timer_ran() {
        let mut w = Window::new();
        w.keep(0, samples(100));
        assert!(w.take(UNDO_WINDOW_MS).is_none());
        // Still there for the timer to drop (and clean up).
        assert!(w.expire(UNDO_WINDOW_MS).is_some());
    }

    #[test]
    fn a_newer_cancel_replaces_the_kept_take_and_the_old_timer_is_ignored() {
        let mut w = Window::new();
        let (first, _) = w.keep(0, file("a.wav"));
        let (second, replaced) = w.keep(3_000, file("b.wav"));
        assert_eq!(replaced, Some(file("a.wav")));
        // The first take's timer fires at 4 s: the second one is younger.
        assert!(w.expire_seq(UNDO_WINDOW_MS, first).is_none());
        assert_eq!(w.peek(4_000).map(|k| k.file_name.as_str()), Some("b.wav"));
        assert!(w.expire_seq(3_000 + UNDO_WINDOW_MS, second).is_some());
    }

    #[test]
    fn discard_and_saved_rows() {
        let mut w = Window::new();
        w.keep(0, file("a.wav"));
        // Handy saved the row of another take: ignored.
        w.note_saved("other.wav", 3);
        assert_eq!(w.peek(1).unwrap().entry_id, None);
        w.note_saved("a.wav", 9);
        assert_eq!(w.peek(1).unwrap().entry_id, Some(9));
        assert_eq!(plan(w.peek(1).unwrap()), Plan::Deliver { entry_id: 9 });
        assert!(w.discard().is_some());
        assert!(w.take(2).is_none());
    }

    #[test]
    fn plans_and_leftovers() {
        assert_eq!(plan(&samples(10)), Plan::WriteAndTranscribe);
        assert_eq!(plan(&file("a.wav")), Plan::Transcribe);
        // Samples leave nothing on disk; a WAV without a row is deleted; a
        // WAV with a row belongs to History.
        assert_eq!(samples(10).leftover_file(), None);
        assert_eq!(file("a.wav").leftover_file(), Some("a.wav"));
        let saved = Kept {
            entry_id: Some(1),
            ..file("a.wav")
        };
        assert_eq!(saved.leftover_file(), None);
    }

    #[test]
    fn a_stray_press_is_not_worth_an_undo() {
        assert!(!worth_keeping(0));
        assert!(!worth_keeping(MIN_UNDO_SAMPLES - 1));
        assert!(worth_keeping(MIN_UNDO_SAMPLES));
        assert!(worth_keeping(SAMPLE_RATE * 60));
    }

    #[test]
    fn short_takes_are_padded_like_handy() {
        assert_eq!(pad(vec![0.5; 8_000]).len(), 20_000);
        assert_eq!(pad(vec![0.5; 16_000]).len(), 16_000);
        assert!(pad(Vec::new()).is_empty());
        // The audio itself is unchanged, the tail is silence.
        let p = pad(vec![0.5; 10]);
        assert_eq!(&p[..10], &[0.5; 10]);
        assert!(p[10..].iter().all(|s| *s == 0.0));
    }
}
