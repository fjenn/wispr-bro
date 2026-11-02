// SPDX-License-Identifier: GPL-3.0-or-later
//! The hotkey grammar (R5 §S-01 rules 1-8, DEC-09, DEC-32, DEC-47, DEC-54)
//! as a pure state machine. No clock, no I/O, no handy-keys types: the
//! engine (`engine.rs`) feeds it timestamped inputs and applies the effects
//! it returns. Lifted from the RS-01 spike (`crates/wb-spike-fn/src/grammar.rs`,
//! tested on a real Mac in both RS-01 rounds) and extended for S-01 with the
//! DEC-54 History flag and the DEC-32 take limits. The behaviour follows the
//! anarlog `shortcut-macos` processor and Hex `HotKeyProcessor` (MIT,
//! credited in `docs/fork/PORTS.md`); their test vectors, and those of
//! MacParakeet, FluidVoice, OpenWhispr and hex, are ported in
//! `tests/ported.rs`.
//!
//! "fn" below is the active **trigger**: the fn key in the default profile,
//! or Ctrl+Opt held together in the fallback profile (`bindings.rs`). The
//! grammar does not care which.
//!
//! Timing rules (constants in [`Timings::DEFAULT`], the one place they live):
//! - `T` = 500 ms (Wispr's tap window), `T_show` = 150 ms (DEC-47, DEC-53).
//! - fn down alone → pending. After `T_show` of uninterrupted hold the take
//!   starts and the mic opens (rule 1).
//! - Another non-modifier key except Space while fn is held in a
//!   push-to-talk press → the press is a chord: dropped (nothing inserted),
//!   the key passes through (rule 2). This holds at any time in the press
//!   (DEC-54); when the take had run ≥ `T` its transcript is kept for
//!   History ([`Effect::ChordIgnored::keep`]). Keys while locked pass
//!   through.
//! - fn up before `T` → tap: wait `T` (from the release) for a second press.
//!   A second press → hands-free lock, the mic opens at once; no second press
//!   → cancel, nothing saved (rule 3).
//! - fn up at or after `T` → stop and insert (push to talk, rule 4).
//! - Locked: a fn press stops and inserts; a press within `T` of locking
//!   cancels (rule 5).
//! - fn+Space while fn is held → lock, mic opens at once, Space swallowed
//!   (rule 6).
//! - Esc while a take is active → cancel, with or without fn (rule 7).
//! - The Flow Bar (S-03): a click on the resting bar starts a hands-free
//!   take; a click on ✓ (or the bar) while hands-free stops and inserts; ✕
//!   cancels. A fn press stops a take the bar started, however soon (rule
//!   5's third-press cancel belongs to the double-tap only).
//! - Hands-free: 120 s of continuous silence stops the take (warning at
//!   100 s); any take stops at 30 min (warning at 28 min). An auto-stopped
//!   take is still transcribed (rule 8, DEC-32).
//!
//! Not modelled here: the warm-stream pre-roll of rule 1 (DEC-53 removed the
//! warm stream: the mic is a prepared unit that never runs while idle),
//! Esc during processing (the engine asks the coordinator) and stuck-fn
//! recovery (rule 9, `decision::StuckGuard`). Modifier keys other than the
//! trigger are not grammar inputs: the caller drops them, so Ctrl/Shift/Cmd/Opt
//! with fn neither interrupt nor stop a take. S-23's engine watches fn+Ctrl
//! before this grammar to choose the instruction binding; with Command Mode
//! off, Ctrl leaves ordinary dictation unchanged (DEC-31).

use serde::Serialize;

/// Milliseconds on the caller's monotonic clock.
pub type Ms = u64;

/// The grammar's timings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Timings {
    /// `T`: the tap window (Wispr's 0.5 s rules).
    pub tap_window_ms: Ms,
    /// `T_show`: uninterrupted hold before a take starts and the mic opens.
    pub show_ms: Ms,
    /// Hands-free: silence before the warning.
    pub silence_warn_ms: Ms,
    /// Hands-free: silence before the take stops itself.
    pub silence_stop_ms: Ms,
    /// Any take: length before the warning.
    pub length_warn_ms: Ms,
    /// Any take: length at which it stops itself.
    pub length_stop_ms: Ms,
}

impl Timings {
    pub const DEFAULT: Timings = Timings {
        tap_window_ms: 500,
        show_ms: 150,
        silence_warn_ms: 100_000,
        silence_stop_ms: 120_000,
        length_warn_ms: 28 * 60_000,
        length_stop_ms: 30 * 60_000,
    };

    /// The default timings with another `T_show` (RS-01 round 2: 100 ms).
    pub const fn with_show_ms(show_ms: Ms) -> Timings {
        let mut t = Self::DEFAULT;
        t.show_ms = show_ms;
        t
    }
}

/// A non-modifier key as the grammar sees it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Key {
    Space,
    Escape,
    /// Any other key (or mouse button); the name is only for logs.
    Other(String),
}

/// One input. fn is tracked by its own down/up events; key-up events of
/// other keys are not inputs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Input {
    FnDown,
    FnUp,
    KeyDown(Key),
    /// The microphone heard speech-level sound now (resets the silence
    /// timer of a hands-free take; ignored otherwise).
    Voice,
    /// The Flow Bar was clicked (or its ✓): start hands-free, or stop.
    PillPress,
    /// The Flow Bar's ✕.
    PillCancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LockVia {
    DoubleTap,
    FnSpace,
    /// A click on the Flow Bar (S-03).
    Pill,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StopVia {
    /// fn released at or after `T` (push to talk).
    Release,
    /// fn pressed while locked.
    LockedPress,
    /// Hands-free take silent for `silence_stop_ms` (rule 8).
    Silence,
    /// Take reached `length_stop_ms` (rule 8).
    MaxLength,
    /// The Flow Bar's ✓ (S-03).
    Pill,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelWhy {
    Escape,
    /// A press held ≥ `T_show` but released before `T`, with no second press.
    NoSecondPress,
    /// A fn press within `T` of locking (third quick press).
    TripleTap,
    /// The caller reset the machine (profile change, shutdown).
    Reset,
    /// The Flow Bar's ✕ (S-03).
    Pill,
}

/// What a take warning is about (rule 8).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WarnKind {
    /// The take stops after `silence_stop_ms - silence_warn_ms` more silence.
    Silence,
    /// The take stops at `length_stop_ms`.
    Length,
}

/// What the caller must do (or log). Effects come out in the order they
/// happen; `MicOpen` always precedes the take event it belongs to and
/// `MicClose` follows the event that ended the take.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "effect", rename_all = "snake_case")]
pub enum Effect {
    MicOpen,
    MicClose,
    /// Push-to-talk take started (`T_show` reached); pill and start sound.
    TakeStarted,
    Locked {
        via: LockVia,
    },
    /// Stop and insert.
    Stopped {
        via: StopVia,
    },
    /// Stop and discard.
    Cancelled {
        why: CancelWhy,
    },
    /// Rule 2: the fn press was a chord. Never inserted; the key passes
    /// through. `mic_was_open` is true when the chord came after `T_show`
    /// (the take had started). `keep` is true when the take had run ≥ `T`:
    /// its transcript goes to History, not into the field (DEC-54).
    ChordIgnored {
        key: String,
        mic_was_open: bool,
        keep: bool,
    },
    /// A quick tap (< `T_show`) with no second press: nothing happened.
    TapIgnored,
    /// Rule 8: the take is about to stop on its own.
    Warning {
        kind: WarnKind,
    },
}

/// The §4.8 blocking set as the grammar sees it. Which hotkeys implement
/// each flag is `blocking.rs`'s business.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Default)]
pub struct Blocking {
    /// `{FN}`: the bare trigger press (constant for a given grammar).
    pub fn_press: bool,
    /// `{FN, Space}`: only while the trigger is physically down.
    pub fn_space: bool,
    /// `{Esc}` and `{FN, Esc}`: only while a take is active, so idle Esc is
    /// never swallowed (the engine also adds it while processing).
    pub esc: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum State {
    Idle,
    /// fn down for less than `T_show`; mic closed.
    Pending {
        down: Ms,
    },
    /// fn down, take started (mic open).
    Holding {
        down: Ms,
    },
    /// fn released before `T`; waiting `T` for a second press. `take` is true
    /// when the press lasted ≥ `T_show` (take started, mic open).
    TapWait {
        up: Ms,
        take: bool,
    },
    /// Hands-free take. `quick_cancel`: a fn press within `T` of `at`
    /// cancels (rule 5); false for a take the Flow Bar started.
    Locked {
        at: Ms,
        fn_down: bool,
        quick_cancel: bool,
    },
    /// fn still down after a chord, stop or cancel; nothing active. The
    /// release returns to idle.
    Draining,
}

/// Rule 8 bookkeeping for the take in progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TakeClock {
    /// When the mic opened.
    started: Ms,
    /// Last speech-level sound (or the take start).
    voice: Ms,
    silence_warned: bool,
    length_warned: bool,
}

#[derive(Clone, Debug)]
pub struct Grammar {
    timings: Timings,
    /// Whether `{FN}` is in the blocking set (DEC-44 update: only when the
    /// Globe setting is Start Dictation).
    block_fn: bool,
    state: State,
    fn_down: bool,
    take: Option<TakeClock>,
}

impl Default for Grammar {
    fn default() -> Self {
        Self::new(Timings::DEFAULT)
    }
}

impl Grammar {
    pub fn new(timings: Timings) -> Self {
        Self::with_fn_block(timings, true)
    }

    /// `block_fn = false` leaves the bare fn press out of the blocking set;
    /// the grammar itself behaves exactly the same.
    pub fn with_fn_block(timings: Timings, block_fn: bool) -> Self {
        Self {
            timings,
            block_fn,
            state: State::Idle,
            fn_down: false,
            take: None,
        }
    }

    /// Changes only the `{FN}` flag (the Globe setting was re-read).
    pub fn set_fn_block(&mut self, block_fn: bool) {
        self.block_fn = block_fn;
    }

    pub fn blocks_fn(&self) -> bool {
        self.block_fn
    }

    pub fn timings(&self) -> Timings {
        self.timings
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn is_idle(&self) -> bool {
        self.state == State::Idle
    }

    pub fn fn_is_down(&self) -> bool {
        self.fn_down
    }

    /// A take is active: the mic is open and Esc cancels.
    pub fn take_active(&self) -> bool {
        matches!(
            self.state,
            State::Holding { .. } | State::TapWait { take: true, .. } | State::Locked { .. }
        )
    }

    /// The press is push-to-talk and the take has run ≥ `T`.
    fn held_past_t(&self, now: Ms) -> bool {
        matches!(self.state, State::Holding { down } if now.saturating_sub(down) >= self.timings.tap_window_ms)
    }

    /// What the blocking set must contain right now.
    pub fn blocking(&self) -> Blocking {
        Blocking {
            fn_press: self.block_fn,
            fn_space: self.fn_down,
            esc: self.take_active(),
        }
    }

    /// When `advance` next has something to do.
    pub fn next_deadline(&self) -> Option<Ms> {
        let tap = match self.state {
            State::Pending { down } => Some(down + self.timings.show_ms),
            State::TapWait { up, .. } => Some(up + self.timings.tap_window_ms),
            _ => None,
        };
        let limits = self.take.filter(|_| self.take_active()).map(|c| {
            let t = &self.timings;
            let length = if c.length_warned {
                c.started + t.length_stop_ms
            } else {
                c.started + t.length_warn_ms
            };
            if matches!(self.state, State::Locked { .. }) {
                let silence = if c.silence_warned {
                    c.voice + t.silence_stop_ms
                } else {
                    c.voice + t.silence_warn_ms
                };
                length.min(silence)
            } else {
                length
            }
        });
        match (tap, limits) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    fn open_mic(&mut self, now: Ms, out: &mut Vec<Effect>) {
        out.push(Effect::MicOpen);
        self.take = Some(TakeClock {
            started: now,
            voice: now,
            silence_warned: false,
            length_warned: false,
        });
    }

    fn close_mic(&mut self, out: &mut Vec<Effect>) {
        out.push(Effect::MicClose);
        self.take = None;
    }

    fn after_stop(&mut self) {
        self.state = if self.fn_down {
            State::Draining
        } else {
            State::Idle
        };
    }

    /// Fires every timer due at or before `now`.
    pub fn advance(&mut self, now: Ms) -> Vec<Effect> {
        let mut out = Vec::new();
        loop {
            match self.state {
                State::Pending { down } if now >= down + self.timings.show_ms => {
                    self.state = State::Holding { down };
                    self.open_mic(down + self.timings.show_ms, &mut out);
                    out.push(Effect::TakeStarted);
                }
                State::TapWait { up, take } if now >= up + self.timings.tap_window_ms => {
                    self.state = State::Idle;
                    if take {
                        out.push(Effect::Cancelled {
                            why: CancelWhy::NoSecondPress,
                        });
                        self.close_mic(&mut out);
                    } else {
                        out.push(Effect::TapIgnored);
                    }
                }
                _ => {
                    if !self.fire_limits(now, &mut out) {
                        return out;
                    }
                }
            }
        }
    }

    /// Rule 8. Returns whether anything fired.
    fn fire_limits(&mut self, now: Ms, out: &mut Vec<Effect>) -> bool {
        if !self.take_active() {
            return false;
        }
        let Some(mut c) = self.take else {
            return false;
        };
        let t = self.timings;
        let locked = matches!(self.state, State::Locked { .. });
        if now >= c.started + t.length_stop_ms {
            out.push(Effect::Stopped {
                via: StopVia::MaxLength,
            });
            self.close_mic(out);
            self.after_stop();
            return true;
        }
        if locked && now >= c.voice + t.silence_stop_ms {
            out.push(Effect::Stopped {
                via: StopVia::Silence,
            });
            self.close_mic(out);
            self.after_stop();
            return true;
        }
        let mut fired = false;
        if !c.length_warned && now >= c.started + t.length_warn_ms {
            c.length_warned = true;
            out.push(Effect::Warning {
                kind: WarnKind::Length,
            });
            fired = true;
        }
        if locked && !c.silence_warned && now >= c.voice + t.silence_warn_ms {
            c.silence_warned = true;
            out.push(Effect::Warning {
                kind: WarnKind::Silence,
            });
            fired = true;
        }
        self.take = Some(c);
        fired
    }

    /// Feeds one input at time `now` (after firing timers due by then).
    pub fn step(&mut self, now: Ms, input: &Input) -> Vec<Effect> {
        let mut out = self.advance(now);
        match input {
            Input::FnDown => self.fn_down(now, &mut out),
            Input::FnUp => self.fn_up(now, &mut out),
            Input::KeyDown(key) => self.key_down(now, key, &mut out),
            Input::Voice => {
                if let Some(c) = self.take.as_mut() {
                    c.voice = now;
                    c.silence_warned = false;
                }
            }
            Input::PillPress => self.pill_press(now, &mut out),
            Input::PillCancel => {
                if self.take_active() {
                    out.push(Effect::Cancelled {
                        why: CancelWhy::Pill,
                    });
                    self.close_mic(&mut out);
                    self.after_stop();
                }
            }
        }
        out
    }

    /// A click on the Flow Bar: idle → a hands-free take; hands-free →
    /// stop and insert. Ignored while fn is pending, held or being drained
    /// (the key owns that take).
    fn pill_press(&mut self, now: Ms, out: &mut Vec<Effect>) {
        match self.state {
            State::Idle => {
                self.open_mic(now, out);
                out.push(Effect::Locked { via: LockVia::Pill });
                self.state = State::Locked {
                    at: now,
                    fn_down: false,
                    quick_cancel: false,
                };
            }
            State::Locked { .. } => {
                out.push(Effect::Stopped { via: StopVia::Pill });
                self.close_mic(out);
                self.after_stop();
            }
            State::Pending { .. }
            | State::Holding { .. }
            | State::TapWait { .. }
            | State::Draining => {}
        }
    }

    /// Cancels whatever is active (profile change, shutdown).
    pub fn reset(&mut self, now: Ms) -> Vec<Effect> {
        let mut out = self.advance(now);
        if self.take_active() {
            out.push(Effect::Cancelled {
                why: CancelWhy::Reset,
            });
            self.close_mic(&mut out);
        }
        self.after_stop();
        out
    }

    fn fn_down(&mut self, now: Ms, out: &mut Vec<Effect>) {
        if self.fn_down {
            return; // duplicate down (tap re-enabled after a missed up, …)
        }
        self.fn_down = true;
        match self.state {
            State::Idle => self.state = State::Pending { down: now },
            // `advance` already closed the window if it had expired.
            State::TapWait { take, .. } => {
                if !take {
                    self.open_mic(now, out);
                }
                out.push(Effect::Locked {
                    via: LockVia::DoubleTap,
                });
                self.state = State::Locked {
                    at: now,
                    fn_down: true,
                    quick_cancel: true,
                };
            }
            State::Locked {
                at, quick_cancel, ..
            } => {
                if quick_cancel && now.saturating_sub(at) < self.timings.tap_window_ms {
                    out.push(Effect::Cancelled {
                        why: CancelWhy::TripleTap,
                    });
                } else {
                    out.push(Effect::Stopped {
                        via: StopVia::LockedPress,
                    });
                }
                self.close_mic(out);
                self.state = State::Draining;
            }
            // fn was up, so these cannot be the state; keep it unchanged.
            State::Pending { .. } | State::Holding { .. } | State::Draining => {}
        }
    }

    fn fn_up(&mut self, now: Ms, out: &mut Vec<Effect>) {
        if !self.fn_down {
            return; // up without a down we saw (e.g. started mid-press)
        }
        self.fn_down = false;
        match self.state {
            State::Pending { .. } => {
                self.state = State::TapWait {
                    up: now,
                    take: false,
                }
            }
            State::Holding { down } => {
                if now.saturating_sub(down) >= self.timings.tap_window_ms {
                    out.push(Effect::Stopped {
                        via: StopVia::Release,
                    });
                    self.close_mic(out);
                    self.state = State::Idle;
                } else {
                    self.state = State::TapWait {
                        up: now,
                        take: true,
                    };
                }
            }
            State::Locked {
                at, quick_cancel, ..
            } => {
                self.state = State::Locked {
                    at,
                    fn_down: false,
                    quick_cancel,
                };
            }
            State::Draining => self.state = State::Idle,
            State::Idle | State::TapWait { .. } => {}
        }
    }

    fn key_down(&mut self, now: Ms, key: &Key, out: &mut Vec<Effect>) {
        match key {
            Key::Escape if self.take_active() => {
                out.push(Effect::Cancelled {
                    why: CancelWhy::Escape,
                });
                self.close_mic(out);
                self.after_stop();
            }
            Key::Space if self.fn_down => match self.state {
                State::Locked { .. } => {} // already hands-free; Space swallowed
                State::Holding { .. } => {
                    out.push(Effect::Locked {
                        via: LockVia::FnSpace,
                    });
                    self.state = State::Locked {
                        at: now,
                        fn_down: true,
                        quick_cancel: true,
                    };
                }
                // Pending, or Draining after a chord/stop/cancel: rule 6 says
                // "at any time while fn is held", so this starts a locked take.
                _ => {
                    self.open_mic(now, out);
                    out.push(Effect::Locked {
                        via: LockVia::FnSpace,
                    });
                    self.state = State::Locked {
                        at: now,
                        fn_down: true,
                        quick_cancel: true,
                    };
                }
            },
            // Esc with no take behaves like any other key (a chord when fn is
            // pending); Space and other keys with fn up do nothing.
            _ if self.fn_down => {
                let name = match key {
                    Key::Space => "Space".to_string(),
                    Key::Escape => "Escape".to_string(),
                    Key::Other(n) => n.clone(),
                };
                match self.state {
                    State::Pending { .. } => {
                        out.push(Effect::ChordIgnored {
                            key: name,
                            mic_was_open: false,
                            keep: false,
                        });
                        self.state = State::Draining;
                    }
                    State::Holding { .. } => {
                        let keep = self.held_past_t(now);
                        out.push(Effect::ChordIgnored {
                            key: name,
                            mic_was_open: true,
                            keep,
                        });
                        self.close_mic(out);
                        self.state = State::Draining;
                    }
                    // Locked or draining: keys pass through.
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
