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
use crate::fork::events::ForkCardReason;

/// Milliseconds on the controller's monotonic clock.
pub type Ms = u64;

/// How long the microphone name shows at the start of a take.
pub const MIC_LABEL_MS: Ms = 2_000;
/// How long an information message stays.
pub const INFO_MS: Ms = 2_500;
/// How long an error stays (one with a Fix action stays longer).
pub const ERROR_MS: Ms = 6_000;
pub const ERROR_WITH_FIX_MS: Ms = 9_000;
/// How long the "stops at 30 minutes" warning stays.
pub const LENGTH_WARNING_MS: Ms = 10_000;
/// How long the Secure Input notice stays at the start of a take (S-02).
pub const SECURE_INPUT_MS: Ms = 5_000;
/// After Handy hides its overlay, the bar keeps its last state this long:
/// Handy hides first and reports an empty or failed take right after, so
/// the message replaces the bar without a resting frame in between.
pub const HIDE_GRACE_MS: Ms = 150;
/// How long "Copied" shows on the card before the bar rests.
pub const COPIED_MS: Ms = 1_200;
/// A message that stays until the user acts (setup problems with Retry).
pub const STICKY: Ms = Ms::MAX;

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
    /// A take is waiting for the first speech model (S-07a).
    SettingUp,
    /// A queued take's text, with Copy (S-07a).
    Card,
    /// A transform's word-level before/after view (S-22).
    Diff,
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
    /// The first speech model could not be downloaded (S-07a).
    DownloadFailed,
    /// A Wi-Fi login page or a proxy blocks the download (S-07a).
    CheckConnection,
    /// Not enough free disk space for the speech model (S-07a).
    NoSpace,
    /// A take was cancelled; Undo brings it back for
    /// [`crate::fork::recovery::UNDO_WINDOW_MS`] (S-06).
    Cancelled,
    /// Paste-last or copy-last found no transcript in History (S-06).
    NoTranscript,
    /// Copy-last put the last transcript on the clipboard (S-06).
    CopiedLast,
    /// OT2-A: no text field had focus at the release (or focus moved on), so
    /// the take's text went on the clipboard instead.
    CopiedToClipboard,
    /// A transform finished; S-22 will present its before/after record.
    TransformDone,
    CommandDone,
    CommandNeedsText,
    SelectTextFirst,
    SelectionTooLong,
    TransformFocusMoved,
    TransformFailed,
    /// OT2-D: the transform left the text as it was (exact edits matched
    /// nothing, or Polish found nothing to fix); nothing was pasted.
    TransformNoChange,
    /// OT2-D: the model returned the text without carrying out a custom
    /// transform's instruction; nothing was pasted.
    TransformNotApplied,
    /// OT2-H: the instruction model is still downloading (or its download
    /// failed); the transform did not run.
    TransformGettingReady,
    /// S-02: the shortcut stopped working: Accessibility is off (or the
    /// app's grant no longer matches it). Fix opens the Fix sheet. Stays
    /// until the shortcut works again, ✕ or the next take.
    ShortcutLost,
    /// S-02: the keyboard listener stopped although the permission is
    /// there (it is being rebuilt, or needs a relaunch).
    ShortcutStopped,
    /// S-02: the shortcut works again (after a loss was shown).
    ShortcutBack,
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
    /// Starts the speech model download again (S-07a).
    RetryDownload,
    /// Brings the cancelled take back (S-06).
    Undo,
    /// Sends the latest transform run to S-22's future changes view.
    ViewChanges,
    /// S-02: opens the Fix sheet in the Hub.
    FixShortcut,
}

impl PillMessage {
    pub fn tone(self) -> PillTone {
        match self {
            PillMessage::NoSpeech
            | PillMessage::InsertedWithoutCleanup
            | PillMessage::NotPasted
            | PillMessage::Cancelled
            | PillMessage::NoTranscript
            | PillMessage::CopiedLast
            | PillMessage::CopiedToClipboard
            | PillMessage::TransformDone
            | PillMessage::CommandDone
            | PillMessage::CommandNeedsText
            | PillMessage::SelectTextFirst
            | PillMessage::SelectionTooLong
            | PillMessage::TransformFocusMoved
            | PillMessage::TransformNoChange
            | PillMessage::ShortcutBack => PillTone::Info,
            PillMessage::MicMuted
            | PillMessage::TranscriptionFailed
            | PillMessage::MicDenied
            | PillMessage::NoMicrophone
            | PillMessage::MicFailed
            | PillMessage::DownloadFailed
            | PillMessage::CheckConnection
            | PillMessage::NoSpace
            | PillMessage::TransformFailed
            | PillMessage::TransformNotApplied
            | PillMessage::TransformGettingReady
            | PillMessage::ShortcutLost
            | PillMessage::ShortcutStopped => PillTone::Error,
        }
    }

    pub fn fix(self) -> Option<PillFix> {
        match self {
            PillMessage::MicDenied => Some(PillFix::MicrophonePrivacy),
            PillMessage::MicMuted | PillMessage::NoMicrophone | PillMessage::MicFailed => {
                Some(PillFix::SoundInput)
            }
            PillMessage::DownloadFailed | PillMessage::CheckConnection | PillMessage::NoSpace => {
                Some(PillFix::RetryDownload)
            }
            PillMessage::Cancelled => Some(PillFix::Undo),
            PillMessage::TransformDone => Some(PillFix::ViewChanges),
            PillMessage::ShortcutLost | PillMessage::ShortcutStopped => Some(PillFix::FixShortcut),
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

    /// Setup problems: they stay until Retry, ✕ or the next take.
    pub fn sticky(self) -> bool {
        self.fix() == Some(PillFix::RetryDownload)
    }

    /// S-02: a lost shortcut stays until it works again (or ✕).
    pub fn until_resolved(self) -> bool {
        matches!(
            self,
            PillMessage::ShortcutLost | PillMessage::ShortcutStopped
        )
    }

    pub fn duration(self) -> Ms {
        if self.sticky() || self.until_resolved() {
            return STICKY;
        }
        if self == PillMessage::Cancelled {
            // The bar offers Undo exactly as long as the audio is kept.
            return crate::fork::recovery::UNDO_WINDOW_MS;
        }
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
            PillMessage::DownloadFailed => "download_failed",
            PillMessage::CheckConnection => "check_connection",
            PillMessage::NoSpace => "no_space",
            PillMessage::Cancelled => "cancelled",
            PillMessage::NoTranscript => "no_transcript",
            PillMessage::CopiedLast => "copied_last",
            PillMessage::CopiedToClipboard => "copied_to_clipboard",
            PillMessage::TransformDone => "transform_done",
            PillMessage::CommandDone => "command_done",
            PillMessage::CommandNeedsText => "command_needs_text",
            PillMessage::SelectTextFirst => "select_text_first",
            PillMessage::SelectionTooLong => "selection_too_long",
            PillMessage::TransformFocusMoved => "transform_focus_moved",
            PillMessage::TransformFailed => "transform_failed",
            PillMessage::TransformNoChange => "transform_no_change",
            PillMessage::TransformNotApplied => "transform_not_applied",
            PillMessage::TransformGettingReady => "transform_getting_ready",
            PillMessage::ShortcutLost => "shortcut_lost",
            PillMessage::ShortcutStopped => "shortcut_stopped",
            PillMessage::ShortcutBack => "shortcut_back",
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
    /// The take's microphone disappeared (unplugged, Bluetooth dropped);
    /// what it heard before is kept (S-08). Stays until the take ends.
    MicLost,
    /// S-02: the take started while an app holds Secure Input: fn+Space
    /// cannot lock; Esc (through the Carbon shadow) or ✕ cancels. Names the
    /// holder when known. Shown for [`SECURE_INPUT_MS`].
    SecureInput,
}

/// The speech model setup as the bar sees it (S-07a).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SetupView {
    /// Download progress, 0-99.
    pub percent: Option<u8>,
    /// What to tell a waiting user instead of the progress (a setup
    /// problem with Retry).
    pub problem: Option<PillMessage>,
    /// A speech model is ready.
    pub ready: bool,
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
    /// The app in front has "Press Enter after dictating" on (S-19a): the
    /// label says "Sends automatically" instead of the microphone.
    SendsAutomatically(bool),
    /// The user dismissed a message (✕) or acted on it (Fix).
    Dismiss,
    Prefs(Prefs),
    /// A take stopped before any speech model was ready (S-07a).
    Queued,
    /// The speech setup changed.
    Setup(SetupView),
    /// The queued takes are being transcribed.
    QueueTranscribing,
    /// A take's text for the card: a queued take (S-07a) or one that was
    /// not pasted (S-05, with its reason).
    Card(CardContent),
    /// The card's text was copied.
    Copied,
    /// Undo is transcribing a cancelled take (S-06): "Transcribing…" from
    /// whatever the bar shows ("Cancelled · Undo"), unless a take records.
    Undoing,
    /// S-20: the take used text near the caret (spelling hints).
    ContextUsed,
    /// S-22: show or replace the transform diff.
    Diff(crate::fork::transforms::record::ForkTransformViewChanges),
    DiffRating(Option<i64>),
    /// S-02: the take runs under Secure Input; the holder's app name.
    SecureInput(Option<String>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ForkTransformDiff {
    pub run: crate::fork::transforms::record::ForkTransformViewChanges,
    pub diff: crate::fork::text::diff::WordDiff,
    /// The card's window height in points: it follows the text
    /// (`geometry::diff_height`), so a short change gets a short card.
    pub height: u32,
}

impl ForkTransformDiff {
    pub fn new(run: crate::fork::transforms::record::ForkTransformViewChanges) -> Self {
        let diff = crate::fork::text::diff::diff(&run.source_text, &run.result_text);
        let height = super::geometry::diff_height(&diff.ops);
        Self { run, diff, height }
    }
}

/// What the card shows (one card for S-07a's queued takes and S-05's
/// takes that were not pasted).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardContent {
    pub text: String,
    /// Why it was not pasted (S-05); `None` for a queued take (S-07a).
    pub reason: Option<ForkCardReason>,
}

impl CardContent {
    /// A password field's text is never shown (Copy still copies it,
    /// concealed from clipboard managers).
    pub fn hidden(&self) -> bool {
        self.reason == Some(ForkCardReason::SecureField)
    }
}

impl From<String> for CardContent {
    /// A queued take's text (S-07a).
    fn from(text: String) -> CardContent {
        CardContent { text, reason: None }
    }
}

impl From<&str> for CardContent {
    fn from(text: &str) -> CardContent {
        CardContent::from(text.to_string())
    }
}

/// The active bar, as sent to the web view (`ForkPillState` minus `seq`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveView {
    pub phase: PillPhase,
    pub shape: Shape,
    pub mic: Option<String>,
    pub message: Option<PillMessage>,
    pub warning: Option<PillWarning>,
    /// Setting up: the download progress.
    pub progress: Option<u8>,
    /// The card's text (`None` for a password field's, S-05).
    pub text: Option<String>,
    /// Why the card's take was not pasted (S-05); `None` for a queued take.
    pub card_reason: Option<ForkCardReason>,
    /// Takes finished before the one on the card (they are in History).
    pub more: u32,
    /// "Copied" shows on the card.
    pub copied: bool,
    /// Held/hands-free: the label says "Sends automatically" (S-19a).
    pub sends_automatically: bool,
    /// S-20: the context glyph (processing states only).
    pub context: bool,
    /// S-22: the transform run and its bounded word diff.
    pub diff: Option<ForkTransformDiff>,
    /// S-02: the app holding Secure Input (with the `secure_input`
    /// warning).
    pub holder: Option<String>,
}

impl ActiveView {
    /// The shape at its window size (the diff card's follows its text).
    pub fn fit(&self) -> super::geometry::Fit {
        match (&self.diff, self.shape) {
            (Some(diff), Shape::Diff) => super::geometry::Fit::diff(diff.height),
            _ => super::geometry::Fit::of(self.shape),
        }
    }
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
    SettingUp,
    Card,
    Diff,
}

#[derive(Clone, Debug, Default)]
pub struct Model {
    phase: Phase,
    prefs: Prefs,
    /// End of the microphone label (set at the take start).
    label_until: Option<Ms>,
    mic: Option<String>,
    /// S-19a: this take will press Return after its paste.
    sends: bool,
    message: Option<(PillMessage, Ms)>,
    warning: Option<(PillWarning, Option<Ms>)>,
    /// Handy hid its overlay; back to rest at this time ([`HIDE_GRACE_MS`]).
    closing_until: Option<Ms>,
    /// S-07a: the speech setup, whether a queued take waits for it, and
    /// the card.
    setup: SetupView,
    waiting: bool,
    card: Option<CardContent>,
    more: u32,
    copied_until: Option<Ms>,
    /// A card that arrived during a take; shown when the take is over.
    pending_card: Option<CardContent>,
    /// S-20: the take in progress used text near the caret.
    context: bool,
    diff: Option<ForkTransformDiff>,
    /// S-02: the Secure Input holder for the take's notice.
    holder: Option<String>,
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

    /// The last speech setup the bar heard of.
    pub fn setup(&self) -> SetupView {
        self.setup
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
        self.sends = false;
        self.message = None;
        self.warning = None;
        // The card's text is in History.
        self.card = None;
        self.more = 0;
        self.copied_until = None;
        self.context = false;
        self.diff = None;
        self.holder = None;
    }

    fn go_idle(&mut self) {
        self.set(Phase::Idle);
        self.label_until = None;
        self.sends = false;
        self.message = None;
        self.warning = None;
        self.card = None;
        self.more = 0;
        self.copied_until = None;
        self.diff = None;
        if let Some(text) = self.pending_card.take() {
            self.show_card(text);
        }
    }

    /// The card on screen, if any (Copy reads its full text).
    pub fn card(&self) -> Option<&CardContent> {
        match self.phase() {
            Phase::Card => self.card.as_ref(),
            _ => None,
        }
    }

    pub fn diff(&self) -> Option<&ForkTransformDiff> {
        match self.phase() {
            Phase::Diff => self.diff.as_ref(),
            _ => None,
        }
    }

    fn show_card(&mut self, text: CardContent) {
        self.more = if self.phase() == Phase::Card {
            self.more + 1
        } else {
            0
        };
        self.set(Phase::Card);
        self.card = Some(text);
        self.copied_until = None;
        self.message = None;
        self.label_until = None;
        self.warning = None;
    }

    /// The bar shows the setup (or its problem) to a waiting user.
    fn setup_screen(&mut self, now: Ms) {
        match self.setup.problem {
            Some(kind) => {
                self.set(Phase::Message);
                self.message = Some((kind, now.saturating_add(kind.duration())));
            }
            None => {
                self.set(Phase::SettingUp);
                self.message = None;
            }
        }
        self.label_until = None;
        self.warning = None;
    }

    /// Showing the setup or one of its problems.
    fn on_setup_screen(&self) -> bool {
        self.phase() == Phase::SettingUp
            || (self.phase() == Phase::Message && self.message.is_some_and(|(k, _)| k.sticky()))
    }

    /// States that outlive Handy's overlay (it hides after every take).
    fn lingers(&self) -> bool {
        matches!(
            self.phase(),
            Phase::Message | Phase::Idle | Phase::SettingUp | Phase::Card | Phase::Diff
        )
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
                Phase::Message
                | Phase::CleaningUp
                | Phase::SettingUp
                | Phase::Card
                | Phase::Diff => {}
                _ => {
                    self.set(Phase::Transcribing);
                    self.label_until = None;
                    self.warning = None;
                }
            },
            Input::CleaningUp => {
                if !self.lingers() {
                    self.set(Phase::CleaningUp);
                    self.label_until = None;
                    self.warning = None;
                }
            }
            Input::ContextUsed => {
                // Only the take being processed (a late report after the
                // next take started is dropped by that start).
                if matches!(
                    self.phase(),
                    Phase::Held | Phase::Locked | Phase::Transcribing | Phase::CleaningUp
                ) {
                    self.context = true;
                }
            }
            Input::Cancelled => {
                if !self.lingers() {
                    self.go_idle();
                }
            }
            Input::Hide => {
                if !self.lingers() {
                    self.closing_until.get_or_insert(now + HIDE_GRACE_MS);
                }
            }
            Input::Message(kind) => {
                if self.recording() && !kind.at_start() {
                    return; // a late report of an earlier take
                }
                if self.phase() == Phase::Card && !kind.at_start() {
                    // S-05: Handy reports a take that went to the card as
                    // failed (its paste contract); the card already says why.
                    return;
                }
                if kind == PillMessage::NotPasted
                    && self.phase() == Phase::Message
                    && self
                        .message
                        .is_some_and(|(current, _)| current == PillMessage::CopiedToClipboard)
                {
                    // OT2-A: the outcome event of a take that was copied
                    // (outcome `card`) does not replace "Copied to clipboard".
                    return;
                }
                if kind == PillMessage::ShortcutBack
                    && !(self.phase() == Phase::Message
                        && self
                            .message
                            .is_some_and(|(current, _)| current.until_resolved()))
                {
                    // S-02: "works again" only answers a loss on screen.
                    return;
                }
                if kind == PillMessage::NoSpeech
                    && self.phase() == Phase::Message
                    && self.message.is_some_and(|(current, _)| {
                        matches!(
                            current,
                            PillMessage::CommandDone | PillMessage::CommandNeedsText
                        )
                    })
                {
                    // S-23 deliberately returns no ordinary dictation to paste.
                    return;
                }
                self.set(Phase::Message);
                self.label_until = None;
                self.warning = None;
                self.message = Some((kind, now.saturating_add(kind.duration())));
            }
            Input::Warning(kind) => {
                // A lost mic outranks "stops soon": nothing else replaces it.
                let mic_lost = matches!(self.warning, Some((PillWarning::MicLost, _)));
                if self.recording() && (!mic_lost || kind == PillWarning::MicLost) {
                    let until = match kind {
                        PillWarning::Silence | PillWarning::MicLost => None,
                        PillWarning::Length => Some(now + LENGTH_WARNING_MS),
                        PillWarning::SecureInput => Some(now + SECURE_INPUT_MS),
                    };
                    self.warning = Some((kind, until));
                }
            }
            Input::SecureInput(holder) => {
                // Only at the start of a take, never over a real warning.
                if self.recording() && self.warning.is_none() {
                    self.warning = Some((PillWarning::SecureInput, Some(now + SECURE_INPUT_MS)));
                    self.holder = holder;
                    // The notice takes the microphone label's place.
                    self.label_until = None;
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
            Input::SendsAutomatically(on) => {
                if self.recording() {
                    self.sends = on;
                }
            }
            Input::Dismiss => {
                if matches!(
                    self.phase(),
                    Phase::Message | Phase::SettingUp | Phase::Card | Phase::Diff
                ) {
                    self.go_idle();
                }
            }
            Input::Prefs(p) => self.prefs = p,
            Input::Queued => {
                self.waiting = true;
                if !self.recording() {
                    self.setup_screen(now);
                }
            }
            Input::Setup(v) => {
                let before = self.setup;
                self.setup = v;
                if v.ready {
                    self.waiting = false;
                    if self.on_setup_screen() {
                        self.set(Phase::Transcribing);
                        self.message = None;
                    }
                } else if self.waiting && self.on_setup_screen() && before.problem != v.problem {
                    // A new problem, or a retry clearing one.
                    self.setup_screen(now);
                }
            }
            Input::QueueTranscribing => {
                if self.on_setup_screen() {
                    self.set(Phase::Transcribing);
                    self.message = None;
                }
            }
            Input::Card(text) => {
                if self.recording() {
                    self.pending_card = Some(text);
                } else {
                    self.show_card(text);
                }
            }
            Input::Copied => {
                if matches!(self.phase(), Phase::Card | Phase::Diff) {
                    self.copied_until = Some(now + COPIED_MS);
                }
            }
            Input::Diff(run) => {
                self.set(Phase::Diff);
                self.diff = Some(ForkTransformDiff::new(run));
                self.card = None;
                self.message = None;
                self.warning = None;
                self.copied_until = None;
            }
            Input::DiffRating(rating) => {
                if let Some(diff) = self.diff.as_mut() {
                    diff.run.rating = rating;
                }
            }
            Input::Undoing => {
                if !self.recording() {
                    self.set(Phase::Transcribing);
                    self.message = None;
                    self.label_until = None;
                    self.warning = None;
                    self.card = None;
                    self.more = 0;
                    self.copied_until = None;
                }
            }
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
                self.holder = None;
            }
        }
        if self.copied_until.is_some_and(|t| now >= t) {
            self.go_idle();
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
            self.copied_until,
        ]
        .into_iter()
        .flatten()
        .filter(|&t| t != STICKY)
        .min()
    }

    /// What is on screen at `now` (call [`Model::expire`] first, or use
    /// [`Model::view_at`]).
    pub fn view(&self) -> View {
        // S-19a: "Sends automatically" takes the microphone name's place.
        let label = self.label_until.is_some() && (self.mic.is_some() || self.sends);
        let warning = self.warning.map(|(w, _)| w);
        let active = match self.phase() {
            Phase::Idle => None,
            Phase::Held | Phase::Locked => {
                let locked = self.phase() == Phase::Locked;
                let wide = label || warning.is_some();
                let notice = warning == Some(PillWarning::SecureInput);
                let sends = label && self.sends && warning.is_none();
                Some(ActiveView {
                    phase: if locked {
                        PillPhase::Locked
                    } else {
                        PillPhase::Held
                    },
                    shape: match (locked, wide, notice) {
                        (false, _, true) => Shape::HeldNotice,
                        (true, _, true) => Shape::LockedNotice,
                        (false, false, _) => Shape::Held,
                        (false, true, _) => Shape::HeldLabel,
                        (true, false, _) => Shape::Locked,
                        (true, true, _) => Shape::LockedLabel,
                    },
                    // A warning takes the label's place.
                    mic: if label && warning.is_none() && !sends {
                        self.mic.clone()
                    } else {
                        None
                    },
                    message: None,
                    warning,
                    progress: None,
                    text: None,
                    card_reason: None,
                    more: 0,
                    copied: false,
                    sends_automatically: sends,
                    context: false,
                    diff: None,
                    holder: if notice { self.holder.clone() } else { None },
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
                progress: None,
                text: None,
                card_reason: None,
                more: 0,
                copied: false,
                sends_automatically: false,
                context: self.context,
                diff: None,
                holder: None,
            }),
            Phase::Message => self.message.map(|(kind, _)| ActiveView {
                phase: PillPhase::Message,
                shape: Shape::Message,
                mic: None,
                message: Some(kind),
                warning: None,
                progress: None,
                text: None,
                card_reason: None,
                more: 0,
                copied: false,
                sends_automatically: false,
                context: false,
                diff: None,
                holder: None,
            }),
            Phase::SettingUp => Some(ActiveView {
                phase: PillPhase::SettingUp,
                shape: Shape::Setup,
                mic: None,
                message: None,
                warning: None,
                progress: self.setup.percent,
                text: None,
                card_reason: None,
                more: 0,
                copied: false,
                sends_automatically: false,
                context: false,
                diff: None,
                holder: None,
            }),
            Phase::Card => self.card.as_ref().map(|card| ActiveView {
                phase: PillPhase::Card,
                shape: Shape::Card,
                mic: None,
                message: None,
                warning: None,
                progress: None,
                text: (!card.hidden()).then(|| card.text.clone()),
                card_reason: card.reason,
                more: self.more,
                copied: self.copied_until.is_some(),
                sends_automatically: false,
                context: false,
                diff: None,
                holder: None,
            }),
            Phase::Diff => self.diff.as_ref().map(|diff| ActiveView {
                phase: PillPhase::Diff,
                shape: Shape::Diff,
                mic: None,
                message: None,
                warning: None,
                progress: None,
                text: None,
                card_reason: None,
                more: 0,
                copied: self.copied_until.is_some(),
                sends_automatically: false,
                context: false,
                diff: Some(diff.clone()),
                holder: None,
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
    fn the_context_glyph_shows_only_while_its_take_is_processed() {
        let ctx = |m: &mut Model, now: Ms| m.view_at(now).active.map(|a| a.context);
        let mut m = Model::new(ready());
        m.step(1_000, Input::TakeStarted { locked: false });
        // Recording: the glyph is for processing states only.
        m.step(1_050, Input::ContextUsed);
        assert_eq!(ctx(&mut m, 1_050), Some(false));
        m.step(2_000, Input::Stopped);
        assert_eq!(ctx(&mut m, 2_000), Some(true));
        m.step(2_100, Input::CleaningUp);
        assert_eq!(ctx(&mut m, 2_100), Some(true));
        // A message after the take carries no glyph.
        m.step(2_500, Input::Message(PillMessage::InsertedWithoutCleanup));
        assert_eq!(ctx(&mut m, 2_500), Some(false));
        // The next take starts without it.
        m.step(9_000, Input::TakeStarted { locked: false });
        m.step(9_500, Input::Stopped);
        assert_eq!(ctx(&mut m, 9_500), Some(false));
        // A report while resting is ignored.
        m.step(9_600, Input::Hide);
        m.step(9_600 + HIDE_GRACE_MS, Input::ContextUsed);
        assert!(m.view_at(9_600 + HIDE_GRACE_MS).active.is_none());
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
        // The label goes after 2 s.
        assert_eq!(shape(&mut m, 2_999), Some(Shape::HeldLabel));
        assert_eq!(shape(&mut m, 3_000), Some(Shape::Held));
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
        assert_eq!(shape(&mut m, 2_100), Some(Shape::Locked));
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
    fn command_guidance_outlives_the_deliberate_empty_delivery() {
        let mut m = Model::new(ready());
        m.step(0, Input::TakeStarted { locked: false });
        m.step(600, Input::Stopped);
        m.step(900, Input::Message(PillMessage::CommandNeedsText));
        m.step(1_000, Input::Hide);
        m.step(1_001, Input::Message(PillMessage::NoSpeech));
        assert_eq!(
            view_of(&mut m, 1_001).message,
            Some(PillMessage::CommandNeedsText)
        );
        m.step(1_100, Input::Message(PillMessage::CommandDone));
        m.step(1_101, Input::Message(PillMessage::NoSpeech));
        assert_eq!(
            view_of(&mut m, 1_101).message,
            Some(PillMessage::CommandDone)
        );
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

    /// S-19a: "Sends automatically" takes the microphone label's place for
    /// the label's time, never outranks a warning, and ends with the take.
    #[test]
    fn sends_automatically_replaces_the_mic_label() {
        let mut m = Model::new(ready());
        m.step(0, Input::SendsAutomatically(true));
        m.step(1, Input::TakeStarted { locked: false });
        assert!(!m.view_at(1).active.unwrap().sends_automatically);
        m.step(2, Input::SendsAutomatically(true));
        m.step(3, Input::MicName("USB Mic".into()));
        let v = m.view_at(3).active.unwrap();
        assert!(v.sends_automatically);
        assert_eq!((v.mic, v.shape), (None, Shape::HeldLabel));
        let v = m.view_at(1 + MIC_LABEL_MS).active.unwrap();
        assert!(!v.sends_automatically);
        assert_eq!(v.shape, Shape::Held);
        // A new take starts without it.
        m.step(2_000, Input::Cancelled);
        m.step(2_001, Input::TakeStarted { locked: true });
        m.step(2_002, Input::MicName("USB Mic".into()));
        let v = m.view_at(2_002).active.unwrap();
        assert!(!v.sends_automatically);
        assert_eq!(v.mic.as_deref(), Some("USB Mic"));
        // A warning wins.
        m.step(2_003, Input::SendsAutomatically(true));
        m.step(2_004, Input::Warning(PillWarning::MicLost));
        assert!(!m.view_at(2_004).active.unwrap().sends_automatically);
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
            DownloadFailed,
            CheckConnection,
            NoSpace,
            Cancelled,
            NoTranscript,
            CopiedLast,
            CopiedToClipboard,
            TransformDone,
            CommandDone,
            CommandNeedsText,
            SelectTextFirst,
            SelectionTooLong,
            TransformFocusMoved,
            TransformFailed,
            TransformGettingReady,
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

    // S-07a -----------------------------------------------------------------

    fn setup(percent: u8) -> Input {
        Input::Setup(SetupView {
            percent: Some(percent),
            problem: None,
            ready: false,
        })
    }

    fn view_of(m: &mut Model, now: Ms) -> ActiveView {
        m.view_at(now).active.expect("active")
    }

    #[test]
    fn a_take_before_the_model_shows_setting_up_then_the_card() {
        let mut m = Model::new(ready());
        m.step(0, setup(10));
        m.step(100, Input::TakeStarted { locked: false });
        m.step(2_000, Input::Stopped);
        assert_eq!(phase(&mut m, 2_000), Some(PillPhase::Transcribing));
        // The hook queued the take; Handy hides its overlay and reports an
        // empty take, which the controller swallows.
        m.step(2_010, Input::Queued);
        m.step(2_020, Input::Hide);
        let v = view_of(&mut m, 2_200);
        assert_eq!(
            (v.phase, v.shape, v.progress),
            (PillPhase::SettingUp, Shape::Setup, Some(10))
        );
        // Progress moves; nothing times out.
        m.step(30_000, setup(40));
        assert_eq!(view_of(&mut m, 30_000).progress, Some(40));
        assert_eq!(m.next_deadline(), None);
        assert!(m.view_at(600_000).active.is_some());
        // Ready: the queue transcribes, then the card with the text.
        m.step(
            600_001,
            Input::Setup(SetupView {
                percent: None,
                problem: None,
                ready: true,
            }),
        );
        assert_eq!(phase(&mut m, 600_001), Some(PillPhase::Transcribing));
        m.step(600_500, Input::Card("hello world".into()));
        let v = view_of(&mut m, 600_500);
        assert_eq!((v.phase, v.shape), (PillPhase::Card, Shape::Card));
        assert_eq!(v.text.as_deref(), Some("hello world"));
        assert!(!v.copied);
        // A card stays until the user acts.
        assert!(m.view_at(3_600_000).active.is_some());
        m.step(3_600_000, Input::Copied);
        assert!(view_of(&mut m, 3_600_000).copied);
        assert!(m.view_at(3_600_000 + COPIED_MS).resting);
    }

    #[test]
    fn a_second_card_counts_the_first_and_dismiss_or_a_new_take_clears_it() {
        let mut m = Model::new(ready());
        m.step(0, Input::Card("one".into()));
        m.step(1, Input::Card("two".into()));
        let v = view_of(&mut m, 1);
        assert_eq!((v.text.as_deref(), v.more), (Some("two"), 1));
        m.step(2, Input::Dismiss);
        assert!(m.view_at(2).resting);
        m.step(3, Input::Card("three".into()));
        assert_eq!(view_of(&mut m, 3).more, 0);
        m.step(4, Input::TakeStarted { locked: false });
        assert_eq!(phase(&mut m, 4), Some(PillPhase::Held));
        // Handy's hide after a cancelled take does not bring it back.
        m.step(5, Input::Cancelled);
        assert!(m.view_at(5).resting);
    }

    #[test]
    fn a_card_during_a_take_waits_for_the_take_to_end() {
        let mut m = Model::new(ready());
        m.step(0, Input::TakeStarted { locked: true });
        m.step(10, Input::Card("queued text".into()));
        assert_eq!(phase(&mut m, 10), Some(PillPhase::Locked));
        m.step(500, Input::Stopped);
        m.step(900, Input::Hide);
        let v = view_of(&mut m, 900 + HIDE_GRACE_MS);
        assert_eq!(v.text.as_deref(), Some("queued text"));
    }

    #[test]
    fn setup_problems_replace_the_progress_and_retry_brings_it_back() {
        let mut m = Model::new(ready());
        m.step(0, setup(5));
        m.step(1, Input::Queued);
        assert_eq!(phase(&mut m, 1), Some(PillPhase::SettingUp));
        let failed = Input::Setup(SetupView {
            percent: Some(5),
            problem: Some(PillMessage::CheckConnection),
            ready: false,
        });
        m.step(2, failed);
        let v = view_of(&mut m, 2);
        assert_eq!(v.message, Some(PillMessage::CheckConnection));
        assert_eq!(
            PillMessage::CheckConnection.fix(),
            Some(PillFix::RetryDownload)
        );
        // Sticky: still there an hour later.
        assert!(m.view_at(3_600_000).active.is_some());
        assert_eq!(m.next_deadline(), None);
        // Retry: back to the progress.
        m.step(3_600_001, setup(5));
        assert_eq!(phase(&mut m, 3_600_001), Some(PillPhase::SettingUp));
        // No queued take: setup changes never pop the bar up.
        let mut m = Model::new(ready());
        m.step(0, setup(50));
        assert!(m.view_at(0).resting);
        m.step(
            1,
            Input::Setup(SetupView {
                percent: None,
                problem: Some(PillMessage::DownloadFailed),
                ready: false,
            }),
        );
        assert!(m.view_at(1).resting);
        // A take queued while the download is stopped shows the problem.
        m.step(2, Input::Queued);
        assert_eq!(
            view_of(&mut m, 2).message,
            Some(PillMessage::DownloadFailed)
        );
    }

    #[test]
    fn setting_up_can_be_dismissed_and_a_queued_take_mid_recording_waits() {
        let mut m = Model::new(ready());
        m.step(0, Input::Queued);
        m.step(1, Input::Dismiss);
        assert!(m.view_at(1).resting);
        // Progress does not bring it back; the card later does.
        m.step(2, setup(70));
        assert!(m.view_at(2).resting);
        m.step(3, Input::Card("x".into()));
        assert_eq!(phase(&mut m, 3), Some(PillPhase::Card));
        // A queue report during a new take is held until after it.
        let mut m = Model::new(ready());
        m.step(0, Input::TakeStarted { locked: false });
        m.step(1, Input::Queued);
        assert_eq!(phase(&mut m, 1), Some(PillPhase::Held));
    }

    // S-06 ------------------------------------------------------------------

    #[test]
    fn a_cancel_offers_undo_for_exactly_the_undo_window() {
        let mut m = Model::new(ready());
        m.step(0, Input::TakeStarted { locked: false });
        // The engine's cancel, then the recovery hook's offer.
        m.step(2_000, Input::Cancelled);
        m.step(2_001, Input::Message(PillMessage::Cancelled));
        let v = view_of(&mut m, 2_001);
        assert_eq!(
            (v.phase, v.shape, v.message),
            (
                PillPhase::Message,
                Shape::Message,
                Some(PillMessage::Cancelled)
            )
        );
        assert_eq!(PillMessage::Cancelled.fix(), Some(PillFix::Undo));
        assert_eq!(PillMessage::Cancelled.tone(), PillTone::Info);
        // Handy's "cancelled" phase arrives after the offer: it stays.
        m.step(2_002, Input::Cancelled);
        m.step(2_003, Input::Hide);
        assert_eq!(phase(&mut m, 2_200), Some(PillPhase::Message));
        assert_eq!(
            m.next_deadline(),
            Some(2_001 + crate::fork::recovery::UNDO_WINDOW_MS)
        );
        assert!(
            m.view_at(2_001 + crate::fork::recovery::UNDO_WINDOW_MS)
                .resting
        );
    }

    #[test]
    fn a_cli_cancel_mid_take_still_shows_the_offer() {
        // No engine effect: the offer itself arrives during "recording".
        let mut m = Model::new(ready());
        m.step(0, Input::TakeStarted { locked: true });
        m.step(500, Input::Cancelled);
        m.step(500, Input::Message(PillMessage::Cancelled));
        assert_eq!(view_of(&mut m, 500).message, Some(PillMessage::Cancelled));
    }

    #[test]
    fn undo_shows_transcribing_then_the_card_or_rest() {
        let mut m = Model::new(ready());
        m.step(0, Input::Message(PillMessage::Cancelled));
        m.step(1_000, Input::Undoing);
        assert_eq!(phase(&mut m, 1_000), Some(PillPhase::Transcribing));
        // Pasted: Handy-style hide, then rest.
        m.step(1_500, Input::Hide);
        assert!(m.view_at(1_500 + HIDE_GRACE_MS).resting);
        // Focus moved: the card.
        m.step(2_000, Input::Undoing);
        m.step(
            2_500,
            Input::Card(CardContent {
                text: "back again".into(),
                reason: Some(ForkCardReason::FocusMoved),
            }),
        );
        let v = view_of(&mut m, 2_500);
        assert_eq!(
            (v.phase, v.card_reason, v.text.as_deref()),
            (
                PillPhase::Card,
                Some(ForkCardReason::FocusMoved),
                Some("back again")
            )
        );
        // An undo never interrupts a take that records.
        let mut m = Model::new(ready());
        m.step(0, Input::TakeStarted { locked: false });
        m.step(1, Input::Undoing);
        assert_eq!(phase(&mut m, 1), Some(PillPhase::Held));
    }

    #[test]
    fn copied_to_clipboard_is_information_and_not_replaced_by_not_pasted() {
        let k = PillMessage::CopiedToClipboard;
        assert_eq!(
            (k.tone(), k.fix(), k.duration(), k.key()),
            (PillTone::Info, None, INFO_MS, "copied_to_clipboard")
        );
        let mut m = Model::new(ready());
        m.step(0, Input::TakeStarted { locked: false });
        m.step(500, Input::Stopped);
        m.step(900, Input::Message(PillMessage::CopiedToClipboard));
        // The take's outcome event (`card`) arrives right after.
        m.step(901, Input::Message(PillMessage::NotPasted));
        let a = m.view_at(901).active.unwrap();
        assert_eq!(a.phase, PillPhase::Message);
        assert_eq!(a.message, Some(PillMessage::CopiedToClipboard));
    }

    #[test]
    fn recovery_messages_are_short_information() {
        for k in [PillMessage::NoTranscript, PillMessage::CopiedLast] {
            assert_eq!(
                (k.tone(), k.fix(), k.duration()),
                (PillTone::Info, None, INFO_MS)
            );
        }
    }

    // S-02 -------------------------------------------------------------

    #[test]
    fn a_lost_shortcut_stays_with_fix_until_it_works_again() {
        let mut m = Model::new(ready());
        m.step(1_000, Input::Message(PillMessage::ShortcutLost));
        let a = m.view_at(1_000).active.unwrap();
        assert_eq!(a.phase, PillPhase::Message);
        assert_eq!(a.message, Some(PillMessage::ShortcutLost));
        assert_eq!(PillMessage::ShortcutLost.fix(), Some(PillFix::FixShortcut));
        assert_eq!(PillMessage::ShortcutLost.tone(), PillTone::Error);
        // Not a setup screen: a ready speech model does not replace it.
        m.step(
            2_000,
            Input::Setup(SetupView {
                ready: true,
                ..SetupView::default()
            }),
        );
        // Hours later it is still there.
        assert_eq!(
            m.view_at(3_600_000).active.unwrap().message,
            Some(PillMessage::ShortcutLost)
        );
        m.step(3_600_100, Input::Message(PillMessage::ShortcutBack));
        let a = m.view_at(3_600_100).active.unwrap();
        assert_eq!(a.message, Some(PillMessage::ShortcutBack));
        assert_eq!(a.message.unwrap().tone(), PillTone::Info);
        assert!(m.view_at(3_600_100 + INFO_MS).active.is_none());
    }

    #[test]
    fn works_again_only_answers_a_loss_on_screen() {
        let mut m = Model::new(ready());
        m.step(0, Input::Message(PillMessage::ShortcutBack));
        assert!(m.view_at(0).active.is_none());
        // Dismissed loss: recovery stays silent too.
        m.step(10, Input::Message(PillMessage::ShortcutStopped));
        m.step(20, Input::Dismiss);
        m.step(30, Input::Message(PillMessage::ShortcutBack));
        assert!(m.view_at(30).active.is_none());
    }

    #[test]
    fn a_take_under_secure_input_shows_the_notice_with_the_holder() {
        let mut m = Model::new(ready());
        m.step(1_000, Input::TakeStarted { locked: false });
        m.step(1_010, Input::MicName("MacBook Pro Microphone".into()));
        m.step(1_020, Input::SecureInput(Some("Terminal".into())));
        let a = m.view_at(1_020).active.unwrap();
        assert_eq!(a.warning, Some(PillWarning::SecureInput));
        assert_eq!(a.holder.as_deref(), Some("Terminal"));
        assert_eq!(a.shape, Shape::HeldNotice);
        assert_eq!(a.mic, None, "the notice takes the label's place");
        // Hands-free: the wide locked shape.
        m.step(1_100, Input::Locked);
        assert_eq!(m.view_at(1_100).active.unwrap().shape, Shape::LockedNotice);
        // Gone after SECURE_INPUT_MS.
        let a = m.view_at(1_020 + SECURE_INPUT_MS).active.unwrap();
        assert_eq!(a.warning, None);
        assert_eq!(a.holder, None);
        assert_eq!(a.shape, Shape::Locked);
    }

    #[test]
    fn the_secure_input_notice_never_hides_a_real_warning_or_shows_at_rest() {
        let mut m = Model::new(ready());
        m.step(0, Input::SecureInput(None));
        assert!(m.view_at(0).active.is_none());
        m.step(100, Input::TakeStarted { locked: true });
        m.step(200, Input::Warning(PillWarning::MicLost));
        m.step(300, Input::SecureInput(None));
        assert_eq!(
            m.view_at(300).active.unwrap().warning,
            Some(PillWarning::MicLost)
        );
    }
}
