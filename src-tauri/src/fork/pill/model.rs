// SPDX-License-Identifier: GPL-3.0-or-later
//! What the Flow Bar shows, as a pure state machine (S-03).
//!
//! Inputs come from the hotkey engine (take started, locked, stopped,
//! cancelled, warnings), Handy's coordinator events and `overlay.rs` hooks
//! (processing phases, hide), Handy's completion and error events
//! (messages), and the fork settings (whether the resting bar may show).
//! Inputs may arrive twice or out of order (the engine and the coordinator
//! both report a start); every transition is idempotent. The controller
//! (`pill/mod.rs`) renders [`Model::view`] after every input and at
//! [`Model::next_deadline`].
//!
//! Visibility rules:
//! - Active states (held, hands-free, processing, messages) always show,
//!   also during onboarding practice (DEC-38) and while the bar is hidden
//!   for an hour or "Always show the bar" is off.
//! - The resting bar shows only when nothing is active, setup has ended
//!   (DEC-38), "Always show the bar" is on (DEC-14) and it is not hidden
//!   for an hour.

use serde::{Deserialize, Serialize};
use specta::Type;

use super::geometry::Shape;

/// Milliseconds on the controller's monotonic clock.
pub type Ms = u64;

/// How long the microphone name shows at the start of a take.
pub const MIC_LABEL_MS: Ms = 1_500;
/// How long an information message stays.
pub const INFO_MS: Ms = 2_500;
/// How long an error stays (one with a Fix action stays longer).
pub const ERROR_MS: Ms = 6_000;
pub const ERROR_WITH_FIX_MS: Ms = 9_000;
/// How long the "stops at 30 minutes" warning stays.
pub const LENGTH_WARNING_MS: Ms = 10_000;
/// After Handy hides its overlay, the bar keeps its last state this long:
/// Handy hides first and reports an empty or failed take right after, so
/// the message replaces the bar without a resting frame in between.
pub const HIDE_GRACE_MS: Ms = 150;

/// What the active bar is doing (the web view's `state`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PillPhase {
    /// Push to talk: the key is held.
    Held,
    /// Hands-free: ✕, waveform, ✓.
    Locked,
    Transcribing,
    CleaningUp,
    Message,
}

/// A message or an error after (or instead of) a take. The English copy is
/// in `fork.json` > `pill.message.<kind>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PillMessage {
    /// Nothing was transcribed.
    NoSpeech,
    /// The microphone sent only digital silence.
    MicMuted,
    /// Cleanup failed or was too slow; the plain transcript went in (S-12a).
    InsertedWithoutCleanup,
    /// The text was not pasted (sleep, lock, paste failed); it is in History.
    NotPasted,
    /// Transcribing failed.
    TranscriptionFailed,
    /// Microphone access is off.
    MicDenied,
    /// No microphone was found.
    NoMicrophone,
    /// The microphone could not be opened.
    MicFailed,
}

/// Information or error (changes the colour and the VoiceOver priority).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PillTone {
    Info,
    Error,
}

/// What a message's Fix button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PillFix {
    /// Opens System Settings › Privacy & Security › Microphone.
    MicrophonePrivacy,
    /// Opens System Settings › Sound › Input.
    SoundInput,
}

impl PillMessage {
    pub fn tone(self) -> PillTone {
        match self {
            PillMessage::NoSpeech
            | PillMessage::InsertedWithoutCleanup
            | PillMessage::NotPasted => PillTone::Info,
            PillMessage::MicMuted
            | PillMessage::TranscriptionFailed
            | PillMessage::MicDenied
            | PillMessage::NoMicrophone
            | PillMessage::MicFailed => PillTone::Error,
        }
    }

    pub fn fix(self) -> Option<PillFix> {
        match self {
            PillMessage::MicDenied => Some(PillFix::MicrophonePrivacy),
            PillMessage::MicMuted | PillMessage::NoMicrophone | PillMessage::MicFailed => {
                Some(PillFix::SoundInput)
            }
            _ => None,
        }
    }

    /// Errors that stop a take from starting: they replace the active bar.
    /// The others report on a finished take and never interrupt a new one.
    pub fn at_start(self) -> bool {
        matches!(
            self,
            PillMessage::MicDenied | PillMessage::NoMicrophone | PillMessage::MicFailed
        )
    }

    pub fn duration(self) -> Ms {
        match (self.tone(), self.fix()) {
            (PillTone::Info, _) => INFO_MS,
            (PillTone::Error, None) => ERROR_MS,
            (PillTone::Error, Some(_)) => ERROR_WITH_FIX_MS,
        }
    }

    /// The `fork.json` key suffix (`pill.message.<key>`).
    pub fn key(self) -> &'static str {
        match self {
            PillMessage::NoSpeech => "no_speech",
            PillMessage::MicMuted => "mic_muted",
            PillMessage::InsertedWithoutCleanup => "inserted_without_cleanup",
            PillMessage::NotPasted => "not_pasted",
            PillMessage::TranscriptionFailed => "transcription_failed",
            PillMessage::MicDenied => "mic_denied",
            PillMessage::NoMicrophone => "no_microphone",
            PillMessage::MicFailed => "mic_failed",
        }
    }
}

/// A take is about to stop on its own (S-01 rule 8, DEC-32).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PillWarning {
    /// Hands-free and silent for 100 s: stops after 120 s of silence.
    Silence,
    /// The take stops at 30 minutes.
    Length,
}

/// What may show while nothing is active (refreshed by the controller).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prefs {
    /// Setup (onboarding) has ended (DEC-38).
    pub setup_done: bool,
    /// "Always show the bar" (DEC-14).
    pub always_visible: bool,
    /// "Hide for an hour" is running.
    pub hidden_for_now: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            setup_done: false,
            always_visible: true,
            hidden_for_now: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input {
    /// A take started. `locked`: hands-free from the start (pill click,
    /// double-tap, fn+Space).
    TakeStarted {
        locked: bool,
    },
    Locked,
    /// Recording ended; transcription follows.
    Stopped,
    Cancelled,
    /// Handy's processing phases (`show_overlay_state`).
    Transcribing,
    CleaningUp,
    /// Handy hid its overlay: the take is over.
    Hide,
    Message(PillMessage),
    Warning(PillWarning),
    /// Speech-level sound (clears the silence warning).
    Voice,
    /// The microphone's name for the label.
    MicName(String),
    /// The user dismissed a message (✕) or acted on it (Fix).
    Dismiss,
    Prefs(Prefs),
}

/// The active bar, as sent to the web view (`ForkPillState` minus `seq`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveView {
    pub phase: PillPhase,
    pub shape: Shape,
    pub mic: Option<String>,
    pub message: Option<PillMessage>,
    pub warning: Option<PillWarning>,
}

/// Everything on screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    pub resting: bool,
    pub active: Option<ActiveView>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Phase {
    #[default]
    Idle,
    Held,
    Locked,
    Transcribing,
    CleaningUp,
    Message,
}

#[derive(Clone, Debug, Default)]
pub struct Model {
    phase: Phase,
    prefs: Prefs,
    /// End of the microphone label (set at the take start).
    label_until: Option<Ms>,
    mic: Option<String>,
    message: Option<(PillMessage, Ms)>,
    warning: Option<(PillWarning, Option<Ms>)>,
    /// Handy hid its overlay; back to rest at this time ([`HIDE_GRACE_MS`]).
    closing_until: Option<Ms>,
}

impl Model {
    pub fn new(prefs: Prefs) -> Model {
        Model {
            prefs,
            ..Model::default()
        }
    }

    fn phase(&self) -> Phase {
        self.phase
    }

    fn set(&mut self, p: Phase) {
        self.phase = p;
        self.closing_until = None;
    }

    pub fn prefs(&self) -> Prefs {
        self.prefs
    }

    /// `true` while the take records (held or hands-free).
    pub fn recording(&self) -> bool {
        matches!(self.phase(), Phase::Held | Phase::Locked)
    }

    /// `true` while anything but the resting bar is shown.
    pub fn active(&self) -> bool {
        self.phase() != Phase::Idle
    }

    fn start_take(&mut self, now: Ms, locked: bool) {
        self.set(if locked { Phase::Locked } else { Phase::Held });
        self.label_until = Some(now + MIC_LABEL_MS);
        self.mic = None;
        self.message = None;
        self.warning = None;
    }

    fn go_idle(&mut self) {
        self.set(Phase::Idle);
        self.label_until = None;
        self.message = None;
        self.warning = None;
    }

    /// Applies one input at `now`.
    pub fn step(&mut self, now: Ms, input: Input) {
        self.expire(now);
        match input {
            Input::TakeStarted { locked } => match self.phase() {
                // The engine and the coordinator both report a start.
                Phase::Held if locked => self.set(Phase::Locked),
                Phase::Held | Phase::Locked => {}
                _ => self.start_take(now, locked),
            },
            Input::Locked => match self.phase() {
                Phase::Held => self.set(Phase::Locked),
                Phase::Locked => {}
                // A lock with no start seen (CLI toggle): a hands-free take.
                _ => self.start_take(now, true),
            },
            Input::Stopped => {
                if self.recording() {
                    self.set(Phase::Transcribing);
                    self.label_until = None;
                    self.warning = None;
                }
            }
            Input::Transcribing => match self.phase() {
                Phase::Message | Phase::CleaningUp => {}
                _ => {
                    self.set(Phase::Transcribing);
                    self.label_until = None;
                    self.warning = None;
                }
            },
            Input::CleaningUp => {
                if self.phase() != Phase::Message {
                    self.set(Phase::CleaningUp);
                    self.label_until = None;
                    self.warning = None;
                }
            }
            Input::Cancelled => {
                if self.phase() != Phase::Message {
                    self.go_idle();
                }
            }
            Input::Hide => {
                if self.phase() != Phase::Message && self.phase() != Phase::Idle {
                    self.closing_until.get_or_insert(now + HIDE_GRACE_MS);
                }
            }
            Input::Message(kind) => {
                if self.recording() && !kind.at_start() {
                    return; // a late report of an earlier take
                }
                self.set(Phase::Message);
                self.label_until = None;
                self.warning = None;
                self.message = Some((kind, now + kind.duration()));
            }
            Input::Warning(kind) => {
                if self.recording() {
                    let until = match kind {
                        PillWarning::Silence => None,
                        PillWarning::Length => Some(now + LENGTH_WARNING_MS),
                    };
                    self.warning = Some((kind, until));
                }
            }
            Input::Voice => {
                if matches!(self.warning, Some((PillWarning::Silence, _))) {
                    self.warning = None;
                }
            }
            Input::MicName(name) => {
                if self.recording() {
                    let name = name.trim();
                    self.mic = (!name.is_empty()).then(|| name.to_string());
                }
            }
            Input::Dismiss => {
                if self.phase() == Phase::Message {
                    self.go_idle();
                }
            }
            Input::Prefs(p) => self.prefs = p,
        }
    }

    /// Drops whatever timed out by `now`.
    pub fn expire(&mut self, now: Ms) {
        if self.closing_until.is_some_and(|t| now >= t) {
            self.go_idle();
        }
        if self.label_until.is_some_and(|t| now >= t) {
            self.label_until = None;
        }
        if let Some((_, Some(t))) = self.warning {
            if now >= t {
                self.warning = None;
            }
        }
        if let Some((_, t)) = self.message {
            if now >= t {
                self.message = None;
                if self.phase() == Phase::Message {
                    self.set(Phase::Idle);
                }
            }
        }
    }

    /// When something next times out.
    pub fn next_deadline(&self) -> Option<Ms> {
        [
            self.label_until,
            self.message.map(|(_, t)| t),
            self.warning.and_then(|(_, t)| t),
            self.closing_until,
        ]
        .into_iter()
        .flatten()
        .min()
    }

    /// What is on screen at `now` (call [`Model::expire`] first, or use
    /// [`Model::view_at`]).
    pub fn view(&self) -> View {
        let label = self.label_until.is_some() && self.mic.is_some();
        let warning = self.warning.map(|(w, _)| w);
        let active = match self.phase() {
            Phase::Idle => None,
            Phase::Held | Phase::Locked => {
                let locked = self.phase() == Phase::Locked;
                let wide = label || warning.is_some();
                Some(ActiveView {
                    phase: if locked {
                        PillPhase::Locked
                    } else {
                        PillPhase::Held
                    },
                    shape: match (locked, wide) {
                        (false, false) => Shape::Held,
                        (false, true) => Shape::HeldLabel,
                        (true, false) => Shape::Locked,
                        (true, true) => Shape::LockedLabel,
                    },
                    // A warning takes the label's place.
                    mic: if label && warning.is_none() {
                        self.mic.clone()
                    } else {
                        None
                    },
                    message: None,
                    warning,
                })
            }
            Phase::Transcribing | Phase::CleaningUp => Some(ActiveView {
                phase: if self.phase() == Phase::Transcribing {
                    PillPhase::Transcribing
                } else {
                    PillPhase::CleaningUp
                },
                shape: Shape::Status,
                mic: None,
                message: None,
                warning: None,
            }),
            Phase::Message => self.message.map(|(kind, _)| ActiveView {
                phase: PillPhase::Message,
                shape: Shape::Message,
                mic: None,
                message: Some(kind),
                warning: None,
            }),
        };
        let p = self.prefs;
        View {
            resting: active.is_none() && p.setup_done && p.always_visible && !p.hidden_for_now,
            active,
        }
    }

    /// [`Model::expire`] then [`Model::view`].
    pub fn view_at(&mut self, now: Ms) -> View {
        self.expire(now);
        self.view()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> Prefs {
        Prefs {
            setup_done: true,
            always_visible: true,
            hidden_for_now: false,
        }
    }

    fn shape(m: &mut Model, now: Ms) -> Option<Shape> {
        m.view_at(now).active.map(|a| a.shape)
    }

    fn phase(m: &mut Model, now: Ms) -> Option<PillPhase> {
        m.view_at(now).active.map(|a| a.phase)
    }

    #[test]
    fn push_to_talk_take_from_rest_to_rest() {
        let mut m = Model::new(ready());
        assert_eq!(
            m.view_at(0),
            View {
                resting: true,
                active: None
            }
        );
        m.step(1_000, Input::TakeStarted { locked: false });
        // The coordinator reports the same start again.
        m.step(1_020, Input::TakeStarted { locked: false });
        let v = m.view_at(1_020);
        assert!(!v.resting);
        assert_eq!(v.active.as_ref().unwrap().phase, PillPhase::Held);
        // No name yet: the plain waveform.
        assert_eq!(v.active.unwrap().shape, Shape::Held);
        m.step(1_100, Input::MicName("MacBook Pro Microphone".into()));
        let v = m.view_at(1_100).active.unwrap();
        assert_eq!(v.shape, Shape::HeldLabel);
        assert_eq!(v.mic.as_deref(), Some("MacBook Pro Microphone"));
        assert_eq!(m.next_deadline(), Some(1_000 + MIC_LABEL_MS));
        // The label goes after 1.5 s.
        assert_eq!(shape(&mut m, 2_499), Some(Shape::HeldLabel));
        assert_eq!(shape(&mut m, 2_500), Some(Shape::Held));
        m.step(4_000, Input::Stopped);
        assert_eq!(phase(&mut m, 4_000), Some(PillPhase::Transcribing));
        m.step(4_001, Input::Transcribing);
        m.step(4_300, Input::CleaningUp);
        assert_eq!(phase(&mut m, 4_300), Some(PillPhase::CleaningUp));
        assert_eq!(shape(&mut m, 4_300), Some(Shape::Status));
        // A late "transcribing" never steps back from cleanup.
        m.step(4_301, Input::Transcribing);
        assert_eq!(phase(&mut m, 4_301), Some(PillPhase::CleaningUp));
        m.step(5_000, Input::Hide);
        // The last state stays for the grace time, then the bar rests.
        assert_eq!(phase(&mut m, 5_000), Some(PillPhase::CleaningUp));
        assert_eq!(m.next_deadline(), Some(5_000 + HIDE_GRACE_MS));
        assert!(m.view_at(5_000 + HIDE_GRACE_MS).resting);
    }

    #[test]
    fn hands_free_take_and_the_label_with_buttons() {
        let mut m = Model::new(ready());
        m.step(0, Input::TakeStarted { locked: true });
        m.step(10, Input::MicName("USB Mic".into()));
        assert_eq!(shape(&mut m, 10), Some(Shape::LockedLabel));
        assert_eq!(shape(&mut m, 1_600), Some(Shape::Locked));
        // A held take that locks (double-tap, fn+Space) keeps its label time.
        let mut m = Model::new(ready());
        m.step(0, Input::TakeStarted { locked: false });
        m.step(5, Input::MicName("USB Mic".into()));
        m.step(300, Input::Locked);
        let v = m.view_at(300).active.unwrap();
        assert_eq!((v.phase, v.shape), (PillPhase::Locked, Shape::LockedLabel));
        // The engine's later "started, locked" is the same take.
        m.step(310, Input::TakeStarted { locked: true });
        assert_eq!(m.next_deadline(), Some(MIC_LABEL_MS));
    }

    #[test]
    fn a_lock_without_a_start_is_a_hands_free_take() {
        let mut m = Model::new(ready());
        m.step(0, Input::Locked);
        assert_eq!(phase(&mut m, 0), Some(PillPhase::Locked));
    }

    #[test]
    fn cancel_returns_to_rest() {
        let mut m = Model::new(ready());
        m.step(0, Input::TakeStarted { locked: false });
        m.step(500, Input::Cancelled);
        assert_eq!(
            m.view_at(500),
            View {
                resting: true,
                active: None
            }
        );
    }

    #[test]
    fn messages_outlive_the_hide_and_time_out() {
        let mut m = Model::new(ready());
        m.step(0, Input::TakeStarted { locked: false });
        m.step(1_000, Input::Stopped);
        // Handy hides first, then reports the empty take.
        m.step(1_200, Input::Hide);
        // No resting frame between the hide and the message.
        assert!(!m.view_at(1_201).resting);
        m.step(1_201, Input::Message(PillMessage::NoSpeech));
        m.step(1_202, Input::Hide);
        let v = m.view_at(1_202);
        assert!(!v.resting);
        let a = v.active.unwrap();
        assert_eq!(a.shape, Shape::Message);
        assert_eq!(a.message, Some(PillMessage::NoSpeech));
        assert_eq!(m.next_deadline(), Some(1_201 + INFO_MS));
        assert!(m.view_at(1_201 + INFO_MS).resting);
    }

    #[test]
    fn errors_with_fix_stay_longer_and_can_be_dismissed() {
        let mut m = Model::new(ready());
        m.step(0, Input::Message(PillMessage::MicDenied));
        assert_eq!(
            PillMessage::MicDenied.fix(),
            Some(PillFix::MicrophonePrivacy)
        );
        assert_eq!(m.next_deadline(), Some(ERROR_WITH_FIX_MS));
        assert!(m.view_at(ERROR_MS).active.is_some());
        m.step(ERROR_MS, Input::Dismiss);
        assert!(m.view_at(ERROR_MS).resting);
    }

    #[test]
    fn a_start_error_replaces_the_take_but_late_reports_do_not() {
        let mut m = Model::new(ready());
        m.step(0, Input::TakeStarted { locked: false });
        // A report about the previous take arrives during this one.
        m.step(10, Input::Message(PillMessage::NotPasted));
        assert_eq!(phase(&mut m, 10), Some(PillPhase::Held));
        // The microphone failed to open for this take.
        m.step(20, Input::Message(PillMessage::NoMicrophone));
        assert_eq!(phase(&mut m, 20), Some(PillPhase::Message));
        // A new take replaces a message.
        m.step(100, Input::TakeStarted { locked: false });
        assert_eq!(phase(&mut m, 100), Some(PillPhase::Held));
    }

    #[test]
    fn warnings_take_the_label_place() {
        let mut m = Model::new(ready());
        m.step(0, Input::TakeStarted { locked: true });
        m.step(1, Input::MicName("Mic".into()));
        m.step(100_000, Input::Warning(PillWarning::Silence));
        let a = m.view_at(100_000).active.unwrap();
        assert_eq!(
            (a.shape, a.warning, a.mic),
            (Shape::LockedLabel, Some(PillWarning::Silence), None)
        );
        // Silence warnings stay until speech returns.
        assert_eq!(m.next_deadline(), None);
        m.step(105_000, Input::Voice);
        assert_eq!(shape(&mut m, 105_000), Some(Shape::Locked));
        // Length warnings time out.
        m.step(28 * 60_000, Input::Warning(PillWarning::Length));
        assert_eq!(m.next_deadline(), Some(28 * 60_000 + LENGTH_WARNING_MS));
        assert_eq!(
            shape(&mut m, 28 * 60_000 + LENGTH_WARNING_MS),
            Some(Shape::Locked)
        );
        // Warnings never show outside a take.
        m.step(29 * 60_000, Input::Stopped);
        m.step(29 * 60_000, Input::Warning(PillWarning::Length));
        assert_eq!(shape(&mut m, 29 * 60_000), Some(Shape::Status));
    }

    #[test]
    fn resting_visibility_rules() {
        let mut m = Model::new(Prefs::default());
        // DEC-38: hidden until setup ends …
        assert!(!m.view_at(0).resting);
        // … while active states show during practice.
        m.step(0, Input::TakeStarted { locked: false });
        assert!(m.view_at(0).active.is_some());
        m.step(1, Input::Cancelled);
        m.step(2, Input::Prefs(ready()));
        assert!(m.view_at(2).resting);
        // "Always show the bar" off: only active states.
        m.step(
            3,
            Input::Prefs(Prefs {
                always_visible: false,
                ..ready()
            }),
        );
        assert!(!m.view_at(3).resting);
        m.step(4, Input::TakeStarted { locked: true });
        assert!(m.view_at(4).active.is_some());
        m.step(5, Input::Cancelled);
        // Hidden for an hour: same.
        m.step(
            6,
            Input::Prefs(Prefs {
                hidden_for_now: true,
                ..ready()
            }),
        );
        assert!(!m.view_at(6).resting);
        m.step(7, Input::Message(PillMessage::MicMuted));
        assert!(m.view_at(7).active.is_some());
    }

    #[test]
    fn mic_names_only_apply_to_a_running_take() {
        let mut m = Model::new(ready());
        m.step(0, Input::MicName("Mic".into()));
        m.step(1, Input::TakeStarted { locked: false });
        assert_eq!(shape(&mut m, 1), Some(Shape::Held));
        m.step(2, Input::MicName("   ".into()));
        assert_eq!(shape(&mut m, 2), Some(Shape::Held));
    }

    #[test]
    fn every_message_has_a_key_tone_and_duration() {
        use PillMessage::*;
        for k in [
            NoSpeech,
            MicMuted,
            InsertedWithoutCleanup,
            NotPasted,
            TranscriptionFailed,
            MicDenied,
            NoMicrophone,
            MicFailed,
        ] {
            assert!(!k.key().is_empty());
            assert!(k.duration() >= INFO_MS);
            if k.at_start() {
                assert_eq!(k.tone(), PillTone::Error);
            }
            // The serde name is the fork.json key.
            assert_eq!(serde_json::to_value(k).unwrap(), serde_json::json!(k.key()));
        }
    }
}
