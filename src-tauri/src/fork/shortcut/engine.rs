// SPDX-License-Identifier: GPL-3.0-or-later
//! The live hotkey loop: raw tap events → [`Translator`] → [`Grammar`] →
//! coordinator commands, blocking set, mic preparation, Esc observers. The
//! loop owns no Tauri or OS objects: everything outside goes through
//! [`Host`], so tests drive it with a fake coordinator and a fake clock.
//!
//! Effects → coordinator (`TranscriptionCoordinator::external_command`):
//! - `TakeStarted`, or `Locked` without a running take → `Start`
//!   (`Locked` then also → `Lock`);
//! - `Locked` on a running take → `Lock`;
//! - `Stopped` (release, locked press, silence, length) → `Stop`;
//! - `Cancelled` (Esc, no second press, triple tap, reset) → `Cancel`;
//! - `ChordIgnored` with the mic open → `Cancel`. With `keep` (DEC-54: the
//!   take ran ≥ `T`) the transcript should go to History without pasting;
//!   until the S-04 delivery hook can skip the paste, the take is cancelled
//!   (never pasted) and the event says `keep`;
//! - `Warning` → [`Host::warn`] (the pill shows it from S-03).
//!
//! Esc with no take of ours while the coordinator is busy (a take started
//! by the CLI, or one being transcribed) → `Cancel` (rule 7).
//!
//! The Flow Bar (S-03) arrives as [`PillAction`]s: a click on the resting
//! bar starts a hands-free take through the grammar (so fn and Esc then
//! behave as for any hands-free take); ✓ and ✕ on a take the grammar does
//! not own (started by the CLI) go straight to the coordinator.

use std::collections::HashSet;

use handy_keys::{Hotkey, KeyEvent, Modifiers};

use super::actions::{EscContext, Registry};
use super::bindings::Profile;
use super::blocking::{self, Extra};
use super::decision::{compound, key_name, Decision, StuckGuard, Translator};
use super::processor::{Effect, Grammar, Input, Key, Ms, State, Timings, WarnKind};
use crate::transcription_coordinator::ExternalCommand;

/// The coordinator binding the fork drives.
pub const BINDING: &str = "transcribe";
/// The existing post-processing action carries an S-23 instruction take.
pub const COMMAND_BINDING: &str = "transcribe_with_post_process";

/// Origin tag for the coordinator's log.
pub const ORIGIN: &str = "fork:hotkey";

/// How often the engine re-reads the coordinator while it is busy and the
/// grammar is idle (to drop Esc from the blocking set when it finishes).
pub const BUSY_POLL_MS: Ms = 50;

/// What the Flow Bar asks for (S-03).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PillAction {
    /// A click on the bar: start hands-free when idle, stop when hands-free.
    Click,
    /// ✓: stop and insert.
    Stop,
    /// ✕: cancel.
    Cancel,
}

/// Everything the engine touches outside itself.
pub trait Host {
    fn command(&mut self, command: ExternalCommand);
    /// The coordinator is recording, transcribing or holding a queued start.
    fn busy(&self) -> bool;
    fn set_blocking(&mut self, set: HashSet<Hotkey>);
    /// The trigger went down: make sure the prepared mic is ready by
    /// `T_show` (re-prepares a failed unit; DEC-53).
    fn prepare_mic(&mut self) {}
    /// The trigger as the system sees it (rule 9), `None` if unknown.
    fn trigger_physically_down(&self) -> Option<bool> {
        None
    }
    fn warn(&mut self, _kind: WarnKind) {}
    /// A decision for the log and for listeners (`ForkHotkeyEvent`).
    fn observe(&mut self, _event: &Observed) {}
}

/// What happened, for logs and listeners.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observed {
    Effect(Effect),
    /// A keyed binding fired its action.
    Action(String),
    /// Esc cancelled a take the grammar did not own (busy coordinator).
    EscCancelledBusy,
    /// Rule 9: the trigger was released by the guard.
    StuckReleased,
}

pub struct Engine {
    grammar: Grammar,
    translator: Option<Translator>,
    profile: Profile,
    registry: Registry,
    stuck: StuckGuard,
    applied: Option<HashSet<Hotkey>>,
    /// The coordinator has a take we started (between Start and its end).
    ours: bool,
    /// Whether fn+Ctrl is held now; latched into the next take's binding.
    command_chord: bool,
    last_busy: bool,
    /// Absolute time of the next stuck-guard / busy poll, so a stream of
    /// events never postpones it.
    poll_at: Option<Ms>,
    /// The latest time the engine was given.
    clock: Ms,
}

impl Engine {
    pub fn new(profile: Profile, registry: Registry, timings: Timings, block_fn: bool) -> Engine {
        Engine {
            grammar: Grammar::with_fn_block(timings, block_fn),
            translator: profile.trigger.map(Translator::new),
            profile,
            registry,
            stuck: StuckGuard::default(),
            applied: None,
            ours: false,
            command_chord: false,
            last_busy: false,
            poll_at: None,
            clock: 0,
        }
    }

    pub fn grammar(&self) -> &Grammar {
        &self.grammar
    }

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// Swaps the profile (settings changed). An active take is cancelled.
    pub fn set_profile(&mut self, now: Ms, profile: Profile, host: &mut dyn Host) {
        self.at(now);
        let effects = self.grammar.reset(now);
        self.apply(effects, host);
        self.translator = profile.trigger.map(Translator::new);
        self.profile = profile;
        self.command_chord = false;
        self.stuck = StuckGuard::default();
        self.sync_blocking(host);
    }

    /// The Globe setting was re-read.
    pub fn set_fn_block(&mut self, block_fn: bool, host: &mut dyn Host) {
        self.grammar.set_fn_block(block_fn);
        self.sync_blocking(host);
    }

    /// When [`Engine::tick`] next has work: a grammar deadline, the stuck
    /// guard while the trigger is down, or the busy poll.
    pub fn next_wake(&self, _now: Ms) -> Option<Ms> {
        match (self.grammar.next_deadline(), self.poll_at) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Schedules the next poll from the state just reached.
    fn rearm(&mut self) {
        let interval = if self.last_busy && !self.grammar.take_active() {
            Some(BUSY_POLL_MS)
        } else if self.grammar.fn_is_down() {
            Some(StuckGuard::POLL_MS)
        } else {
            None
        };
        self.poll_at = match (interval, self.poll_at) {
            (None, _) => None,
            (Some(i), Some(at)) if at <= self.clock + i && at > self.clock => Some(at),
            (Some(i), _) => Some(self.clock + i),
        };
    }

    fn at(&mut self, now: Ms) {
        self.clock = self.clock.max(now);
    }

    /// Timers, stuck guard, busy poll.
    pub fn tick(&mut self, now: Ms, host: &mut dyn Host) {
        self.at(now);
        if self.poll_at.is_some_and(|p| p <= now) {
            self.poll_at = None;
        }
        let effects = self.grammar.advance(now);
        self.apply(effects, host);
        if self.grammar.fn_is_down()
            && self
                .stuck
                .observe(now, true, host.trigger_physically_down())
        {
            host.observe(&Observed::StuckReleased);
            if let Some(t) = self.translator.as_mut() {
                t.force_up();
            }
            let effects = self.grammar.step(now, &Input::FnUp);
            self.apply(effects, host);
        }
        self.sync_blocking(host);
    }

    /// Speech-level sound from the mic (rule 8).
    pub fn voice(&mut self, now: Ms, host: &mut dyn Host) {
        self.at(now);
        let effects = self.grammar.step(now, &Input::Voice);
        self.apply(effects, host);
        self.sync_blocking(host);
    }

    /// The Flow Bar was clicked (S-03).
    pub fn pill(&mut self, now: Ms, action: PillAction, host: &mut dyn Host) {
        self.at(now);
        let effects = self.grammar.advance(now);
        self.apply(effects, host);
        let state = self.grammar.state();
        let locked = matches!(state, State::Locked { .. });
        let input = match action {
            PillAction::Click if self.grammar.is_idle() || locked => Some(Input::PillPress),
            PillAction::Stop if locked => Some(Input::PillPress),
            PillAction::Cancel if self.grammar.take_active() => Some(Input::PillCancel),
            _ => None,
        };
        match input {
            Some(input) => {
                let effects = self.grammar.step(now, &input);
                self.apply(effects, host);
            }
            // A take the grammar does not own (CLI): ✓ / ✕ still work.
            None if host.busy() && !self.grammar.take_active() => match action {
                PillAction::Stop => host.command(ExternalCommand::Stop),
                PillAction::Cancel => {
                    host.command(ExternalCommand::Cancel);
                    self.ours = false;
                }
                PillAction::Click => {}
            },
            None => {}
        }
        self.sync_blocking(host);
    }

    /// One raw event from the fork tap.
    pub fn event(&mut self, now: Ms, ev: &KeyEvent, host: &mut dyn Host) {
        self.at(now);
        if ev.key.is_none() {
            self.command_chord = self.profile.command_mode
                && compound(ev.modifiers) == Modifiers::FN.union(Modifiers::CTRL);
        }
        let effects = self.grammar.advance(now);
        self.apply(effects, host);

        // S-34: Return, keypad Enter and Tab commit what the user typed.
        if ev.is_key_down
            && ev.key.is_some_and(|k| {
                crate::fork::dictionary::learn::watcher::is_commit_key(k, ev.modifiers)
            })
        {
            self.registry.notify_commit_key();
        }

        // Keyed action bindings (exact modifier match, key-down only).
        if ev.is_key_down {
            if let Some(key) = ev.key {
                let hit = self
                    .profile
                    .keyed
                    .iter()
                    .find(|b| b.hotkey.key == Some(key) && b.hotkey.modifiers.matches(ev.modifiers))
                    .map(|b| b.action.clone());
                if let Some(action) = hit {
                    if self.registry.fire(&action) {
                        host.observe(&Observed::Action(action));
                    }
                    self.sync_blocking(host);
                    return;
                }
            }
        }

        let Some(translator) = self.translator.as_mut() else {
            self.esc_without_trigger(ev, host);
            return;
        };
        let decision = translator.translate(ev);
        let Decision::Input(input) = decision else {
            self.sync_blocking(host);
            return;
        };
        if input == Input::FnDown && self.grammar.is_idle() {
            host.prepare_mic();
        }
        let input = self.filter(input);
        let Some(input) = input else {
            self.sync_blocking(host);
            return;
        };
        let esc = matches!(input, Input::KeyDown(Key::Escape));
        let take_was_active = self.grammar.take_active();
        let effects = self.grammar.step(now, &input);
        self.apply(effects, host);
        if esc {
            self.esc_side_effects(take_was_active, host);
        }
        self.sync_blocking(host);
    }

    /// Bindings the profile leaves out turn their grammar inputs off.
    fn filter(&self, input: Input) -> Option<Input> {
        match &input {
            // fn+Space only locks when `hands_free` is bound to it; otherwise
            // Space is an ordinary chord key.
            Input::KeyDown(Key::Space)
                if !self.profile.trigger_space && self.grammar.fn_is_down() =>
            {
                Some(Input::KeyDown(Key::Other(key_name(handy_keys::Key::Space))))
            }
            Input::KeyDown(Key::Escape) if !self.profile.esc_cancels => Some(Input::KeyDown(
                Key::Other(key_name(handy_keys::Key::Escape)),
            )),
            _ => Some(input),
        }
    }

    fn esc_without_trigger(&mut self, ev: &KeyEvent, host: &mut dyn Host) {
        if ev.is_key_down && ev.key == Some(handy_keys::Key::Escape) {
            self.esc_side_effects(false, host);
            self.sync_blocking(host);
        }
    }

    /// Rule 7 for takes the grammar does not own, then the Esc observers.
    fn esc_side_effects(&mut self, take_was_active: bool, host: &mut dyn Host) {
        let mut cancelled = take_was_active && self.profile.esc_cancels;
        if !take_was_active && self.profile.esc_cancels && host.busy() {
            host.command(ExternalCommand::Cancel);
            host.observe(&Observed::EscCancelledBusy);
            self.ours = false;
            cancelled = true;
        }
        self.registry.notify_esc(EscContext {
            cancelled_take: cancelled,
        });
    }

    fn apply(&mut self, effects: Vec<Effect>, host: &mut dyn Host) {
        for e in effects {
            host.observe(&Observed::Effect(e.clone()));
            match e {
                Effect::TakeStarted => self.start(host),
                Effect::Locked { .. } => {
                    if !self.ours {
                        self.start(host);
                    }
                    host.command(ExternalCommand::Lock);
                }
                Effect::Stopped { .. } => {
                    host.command(ExternalCommand::Stop);
                    self.ours = false;
                }
                Effect::Cancelled { .. } => {
                    host.command(ExternalCommand::Cancel);
                    self.ours = false;
                }
                Effect::ChordIgnored {
                    mic_was_open: true, ..
                } => {
                    host.command(ExternalCommand::Cancel);
                    self.ours = false;
                }
                Effect::Warning { kind } => host.warn(kind),
                Effect::ChordIgnored { .. }
                | Effect::MicOpen
                | Effect::MicClose
                | Effect::TapIgnored => {}
            }
        }
    }

    fn start(&mut self, host: &mut dyn Host) {
        let binding_id = if self.profile.command_mode && self.command_chord {
            COMMAND_BINDING
        } else {
            BINDING
        };
        host.command(ExternalCommand::Start {
            binding_id: binding_id.to_string(),
        });
        self.ours = true;
    }

    fn sync_blocking(&mut self, host: &mut dyn Host) {
        let busy = host.busy();
        self.last_busy = busy;
        let set = blocking::hotkeys(&self.profile, self.grammar.blocking(), Extra { busy });
        if self.applied.as_ref() != Some(&set) {
            host.set_blocking(set.clone());
            self.applied = Some(set);
        }
        self.rearm();
    }

    /// Shutdown or tap restart: cancels anything active.
    pub fn reset(&mut self, now: Ms, host: &mut dyn Host) {
        let effects = self.grammar.reset(now);
        self.apply(effects, host);
        if let Some(t) = self.translator.as_mut() {
            t.force_up();
        }
        self.sync_blocking(host);
    }
}

/// The modifier flags a trigger needs, as `CGEventFlags` bits (rule 9).
pub fn trigger_flag_mask(modifiers: Modifiers) -> u64 {
    const SHIFT: u64 = 0x0002_0000;
    const CONTROL: u64 = 0x0004_0000;
    const ALTERNATE: u64 = 0x0008_0000;
    const COMMAND: u64 = 0x0010_0000;
    const SECONDARY_FN: u64 = 0x0080_0000;
    let mut mask = 0;
    for (bit, flag) in [
        (Modifiers::SHIFT, SHIFT),
        (Modifiers::CTRL, CONTROL),
        (Modifiers::OPT, ALTERNATE),
        (Modifiers::CMD, COMMAND),
        (Modifiers::FN, SECONDARY_FN),
    ] {
        if modifiers.intersects(bit) {
            mask |= flag;
        }
    }
    mask
}
