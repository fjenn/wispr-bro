// SPDX-License-Identifier: GPL-3.0-or-later
//! The fn grammar (R5 §S-01 rules 1-7, DEC-09, DEC-47) as a pure state
//! machine. No clock, no I/O, no handy-keys types: the caller feeds it
//! timestamped inputs and applies the effects it returns. Written so S-01 can
//! lift this file into `fork/shortcut/processor.rs` unchanged.
//!
//! Timing rules (constants in [`Timings::DEFAULT`]; RS-01 round 2 also runs
//! `T_show` = 100 ms through [`Timings::with_show_ms`]):
//! - `T` = 500 ms (Wispr's tap window), `T_show` = 150 ms (DEC-47).
//! - fn down alone → pending. After `T_show` of uninterrupted hold the take
//!   starts and the mic opens (rule 1).
//! - Another non-modifier key except Space while fn is held in a
//!   push-to-talk press → the press is a chord: discarded silently (nothing
//!   inserted), the key passes through (rule 2). This holds at any time in
//!   the press, not just within `T` (DEC-54, RS-01 round 2: the owner held fn
//!   1.1-1.7 s before the first chord key). Keys while locked pass through.
//! - fn up before `T` → tap: wait `T` (from the release) for a second press.
//!   A second press → hands-free lock, the mic opens at once; no second press
//!   → cancel, nothing saved (rule 3).
//! - fn up at or after `T` → stop and insert (push to talk, rule 4).
//! - Locked: a fn press stops and inserts; a press within `T` of locking
//!   cancels (rule 5).
//! - fn+Space while fn is held → lock, mic opens at once, Space swallowed
//!   (rule 6).
//! - Esc while a take is active → cancel, with or without fn (rule 7).
//!
//! Not modelled here (out of RS-01's scope): the warm-stream pre-roll of
//! rule 1, Command Mode on Ctrl (rule 2, DEC-31), the silence and length
//! limits (rule 8) and stuck-fn recovery (rule 9). Modifier keys other than
//! fn are not inputs at all: the caller drops them, so Ctrl/Shift/Cmd/Opt
//! with fn neither interrupt nor stop a take.

use serde::Serialize;

/// Milliseconds on the caller's monotonic clock.
pub type Ms = u64;

/// The grammar's two timings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Timings {
    /// `T`: the tap window (Wispr's 0.5 s rules).
    pub tap_window_ms: Ms,
    /// `T_show`: uninterrupted hold before a take starts and the mic opens.
    pub show_ms: Ms,
}

impl Timings {
    pub const DEFAULT: Timings = Timings {
        tap_window_ms: 500,
        show_ms: 150,
    };

    /// The default timings with another `T_show` (RS-01 round 2: 100 ms).
    pub const fn with_show_ms(show_ms: Ms) -> Timings {
        Timings {
            tap_window_ms: Self::DEFAULT.tap_window_ms,
            show_ms,
        }
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LockVia {
    DoubleTap,
    FnSpace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StopVia {
    /// fn released at or after `T` (push to talk).
    Release,
    /// fn pressed while locked.
    LockedPress,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelWhy {
    Escape,
    /// A press held ≥ `T_show` but released before `T`, with no second press.
    NoSecondPress,
    /// A fn press within `T` of locking (third quick press).
    TripleTap,
    /// The caller reset the machine (end of a test block, shutdown).
    Reset,
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
    /// Rule 2: the fn press was a chord. Discarded silently (never inserted);
    /// the key passes through. `mic_was_open` is true when the chord came
    /// after `T_show` (the take had started and its audio is dropped).
    ChordIgnored {
        key: String,
        mic_was_open: bool,
    },
    /// A quick tap (< `T_show`) with no second press: nothing happened.
    TapIgnored,
}

/// The §4.8 blocking set. `{FN}` (modifier-only) is blocked in every state
/// while fn is the active binding, unless the grammar was built without the
/// fn block (R5 §7 RS-01 fallback: "stop blocking the fn press, keep
/// blocking fn+Space and Esc").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Default)]
pub struct Blocking {
    /// `{FN}`: the bare fn press (constant for a given grammar).
    pub fn_press: bool,
    /// `{FN, Space}`: only while fn is physically down.
    pub fn_space: bool,
    /// `{Esc}` and `{FN, Esc}`: only while a take is active, so idle Esc is
    /// never swallowed.
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
    /// Hands-free take.
    Locked {
        at: Ms,
        fn_down: bool,
    },
    /// fn still down after a chord, stop or cancel; nothing active. The
    /// release returns to idle.
    Draining,
}

#[derive(Clone, Debug)]
pub struct Grammar {
    timings: Timings,
    /// Whether `{FN}` is in the blocking set (default true).
    block_fn: bool,
    state: State,
    fn_down: bool,
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
        }
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
        match self.state {
            State::Pending { down } => Some(down + self.timings.show_ms),
            State::TapWait { up, .. } => Some(up + self.timings.tap_window_ms),
            _ => None,
        }
    }

    /// Fires every timer due at or before `now`.
    pub fn advance(&mut self, now: Ms) -> Vec<Effect> {
        let mut out = Vec::new();
        loop {
            match self.state {
                State::Pending { down } if now >= down + self.timings.show_ms => {
                    self.state = State::Holding { down };
                    out.push(Effect::MicOpen);
                    out.push(Effect::TakeStarted);
                }
                State::TapWait { up, take } if now >= up + self.timings.tap_window_ms => {
                    self.state = State::Idle;
                    if take {
                        out.push(Effect::Cancelled {
                            why: CancelWhy::NoSecondPress,
                        });
                        out.push(Effect::MicClose);
                    } else {
                        out.push(Effect::TapIgnored);
                    }
                }
                _ => return out,
            }
        }
    }

    /// Feeds one input at time `now` (after firing timers due by then).
    pub fn step(&mut self, now: Ms, input: &Input) -> Vec<Effect> {
        let mut out = self.advance(now);
        match input {
            Input::FnDown => self.fn_down(now, &mut out),
            Input::FnUp => self.fn_up(now, &mut out),
            Input::KeyDown(key) => self.key_down(now, key, &mut out),
        }
        out
    }

    /// Cancels whatever is active (end of a test block, shutdown).
    pub fn reset(&mut self, now: Ms) -> Vec<Effect> {
        let mut out = self.advance(now);
        if self.take_active() {
            out.push(Effect::Cancelled {
                why: CancelWhy::Reset,
            });
            out.push(Effect::MicClose);
        }
        self.state = if self.fn_down {
            State::Draining
        } else {
            State::Idle
        };
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
                    out.push(Effect::MicOpen);
                }
                out.push(Effect::Locked {
                    via: LockVia::DoubleTap,
                });
                self.state = State::Locked {
                    at: now,
                    fn_down: true,
                };
            }
            State::Locked { at, .. } => {
                if now.saturating_sub(at) < self.timings.tap_window_ms {
                    out.push(Effect::Cancelled {
                        why: CancelWhy::TripleTap,
                    });
                } else {
                    out.push(Effect::Stopped {
                        via: StopVia::LockedPress,
                    });
                }
                out.push(Effect::MicClose);
                self.state = State::Draining;
            }
            // fn was up, so these cannot be the state; keep it unchanged.
            State::Pending { .. } | State::Holding { .. } | State::Draining => {}
        }
    }

    fn fn_up(&mut self, now: Ms, out: &mut Vec<Effect>) {
        if !self.fn_down {
            return; // up without a down we saw (e.g. tool started mid-press)
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
                    out.push(Effect::MicClose);
                    self.state = State::Idle;
                } else {
                    self.state = State::TapWait {
                        up: now,
                        take: true,
                    };
                }
            }
            State::Locked { at, .. } => {
                self.state = State::Locked { at, fn_down: false };
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
                out.push(Effect::MicClose);
                self.state = if self.fn_down {
                    State::Draining
                } else {
                    State::Idle
                };
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
                    };
                }
                // Pending, or Draining after a chord/stop/cancel: rule 6 says
                // "at any time while fn is held", so this starts a locked take.
                _ => {
                    out.push(Effect::MicOpen);
                    out.push(Effect::Locked {
                        via: LockVia::FnSpace,
                    });
                    self.state = State::Locked {
                        at: now,
                        fn_down: true,
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
                        });
                        self.state = State::Draining;
                    }
                    State::Holding { .. } => {
                        out.push(Effect::ChordIgnored {
                            key: name,
                            mic_was_open: true,
                        });
                        out.push(Effect::MicClose);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::{run_scenario, scenarios};

    #[test]
    fn every_scripted_scenario_passes() {
        let mut failures = Vec::new();
        for s in scenarios() {
            if let Err(e) = run_scenario(&s) {
                failures.push(format!("{}: {e}", s.name));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
        assert!(scenarios().len() >= 30);
    }

    fn other(name: &str) -> Input {
        Input::KeyDown(Key::Other(name.into()))
    }

    #[test]
    fn blocking_set_follows_the_state() {
        for block_fn in [true, false] {
            let b = |fn_space, esc| Blocking {
                fn_press: block_fn,
                fn_space,
                esc,
            };
            let mut g = Grammar::with_fn_block(Timings::DEFAULT, block_fn);
            assert_eq!(g.blocking(), b(false, false));
            g.step(0, &Input::FnDown);
            assert_eq!(g.blocking(), b(true, false));
            g.advance(150);
            assert_eq!(g.blocking(), b(true, true));
            g.step(300, &Input::FnUp); // tap with take: Esc still cancels
            assert_eq!(g.blocking(), b(false, true));
            g.step(400, &Input::FnDown); // double-tap lock
            g.step(450, &Input::FnUp);
            assert_eq!(g.blocking(), b(false, true));
            g.step(2000, &Input::KeyDown(Key::Escape));
            assert_eq!(g.blocking(), b(false, false));
        }
    }

    #[test]
    fn default_grammar_blocks_fn() {
        assert!(Grammar::default().blocks_fn());
        assert!(Grammar::new(Timings::DEFAULT).blocking().fn_press);
        assert!(!Grammar::with_fn_block(Timings::DEFAULT, false).blocks_fn());
    }

    #[test]
    fn without_the_fn_block_every_scenario_gives_the_same_effects() {
        // The no-fn-block option changes only the blocking set, never a
        // decision: replay every scripted scenario through both grammars.
        for s in scenarios() {
            let mut a = Grammar::default();
            let mut b = Grammar::with_fn_block(Timings::DEFAULT, false);
            for (t, input) in &s.inputs {
                assert_eq!(a.step(*t, input), b.step(*t, input), "{}", s.name);
                assert_eq!(a.state(), b.state(), "{}", s.name);
                assert!(!b.blocking().fn_press);
            }
            assert_eq!(a.advance(s.end), b.advance(s.end), "{}", s.name);
        }
    }

    #[test]
    fn show_100_ms_opens_the_mic_at_100_and_keeps_quick_taps_closed() {
        let t = Timings::with_show_ms(100);
        assert_eq!(t.tap_window_ms, Timings::DEFAULT.tap_window_ms);
        assert_eq!(t.show_ms, 100);
        // Hold: the take starts at 100 ms, not 150.
        let mut g = Grammar::new(t);
        g.step(0, &Input::FnDown);
        assert_eq!(g.next_deadline(), Some(100));
        assert!(g.advance(99).is_empty());
        assert_eq!(g.advance(100), vec![Effect::MicOpen, Effect::TakeStarted]);
        // A 90 ms tap never opens the mic.
        let mut g = Grammar::new(t);
        g.step(1000, &Input::FnDown);
        assert!(g.step(1090, &Input::FnUp).is_empty());
        assert_eq!(g.advance(1590), vec![Effect::TapIgnored]);
        // A chord at 120 ms now comes after T_show: the take had started and
        // is dropped (with 150 ms it would have been discarded before the mic).
        let mut g = Grammar::new(t);
        g.step(0, &Input::FnDown);
        assert_eq!(
            g.step(120, &other("Home")),
            vec![
                Effect::MicOpen,
                Effect::TakeStarted,
                Effect::ChordIgnored {
                    key: "Home".into(),
                    mic_was_open: true
                },
                Effect::MicClose
            ]
        );
        let mut g = Grammar::default();
        g.step(0, &Input::FnDown);
        assert_eq!(
            g.step(120, &other("Home")),
            vec![Effect::ChordIgnored {
                key: "Home".into(),
                mic_was_open: false
            }]
        );
    }

    #[test]
    fn quick_tap_without_take_does_not_block_esc() {
        let mut g = Grammar::default();
        g.step(0, &Input::FnDown);
        g.step(60, &Input::FnUp);
        assert_eq!(
            g.state(),
            State::TapWait {
                up: 60,
                take: false
            }
        );
        assert!(!g.blocking().esc);
        // Esc now is plain typing: nothing happens and the tap still resolves.
        assert!(g.step(100, &Input::KeyDown(Key::Escape)).is_empty());
        assert_eq!(g.advance(560), vec![Effect::TapIgnored]);
    }

    #[test]
    fn deadlines() {
        let mut g = Grammar::default();
        assert_eq!(g.next_deadline(), None);
        g.step(1000, &Input::FnDown);
        assert_eq!(g.next_deadline(), Some(1150));
        g.step(1100, &Input::FnUp);
        assert_eq!(g.next_deadline(), Some(1600));
        g.step(1200, &Input::FnDown);
        assert_eq!(g.next_deadline(), None); // locked
    }

    #[test]
    fn custom_timings_are_honoured() {
        let mut g = Grammar::new(Timings {
            tap_window_ms: 300,
            show_ms: 100,
        });
        g.step(0, &Input::FnDown);
        assert_eq!(g.advance(99), vec![]);
        assert_eq!(g.advance(100), vec![Effect::MicOpen, Effect::TakeStarted]);
        assert_eq!(
            g.step(300, &Input::FnUp),
            vec![
                Effect::Stopped {
                    via: StopVia::Release
                },
                Effect::MicClose
            ]
        );
    }

    #[test]
    fn reset_cancels_an_active_take_and_waits_for_release() {
        let mut g = Grammar::default();
        g.step(0, &Input::FnDown);
        g.advance(200);
        assert_eq!(
            g.reset(300),
            vec![
                Effect::Cancelled {
                    why: CancelWhy::Reset
                },
                Effect::MicClose
            ]
        );
        assert_eq!(g.state(), State::Draining);
        assert!(g.step(400, &Input::FnUp).is_empty());
        assert!(g.is_idle());
        // Reset when idle is a no-op.
        assert!(g.reset(500).is_empty());
    }

    #[test]
    fn reset_fires_due_timers_first() {
        let mut g = Grammar::default();
        g.step(0, &Input::FnDown);
        g.step(50, &Input::FnUp);
        assert_eq!(g.reset(1000), vec![Effect::TapIgnored]);
    }

    #[test]
    fn mic_is_open_exactly_while_a_take_is_active_under_random_input() {
        // Deterministic pseudo-random walk over inputs and times: the mic
        // effects must always agree with take_active(), and MicOpen/MicClose
        // must alternate.
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        for _run in 0..200 {
            let mut g = Grammar::default();
            let mut t: Ms = 0;
            let mut mic = false;
            for _ in 0..60 {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                t += seed % 700;
                let input = match (seed >> 32) % 5 {
                    0 | 1 => {
                        if g.fn_is_down() {
                            Input::FnUp
                        } else {
                            Input::FnDown
                        }
                    }
                    2 => Input::KeyDown(Key::Space),
                    3 => Input::KeyDown(Key::Escape),
                    _ => other("Home"),
                };
                for e in g.step(t, &input) {
                    match e {
                        Effect::MicOpen => {
                            assert!(!mic, "double MicOpen at {t}");
                            mic = true;
                        }
                        Effect::MicClose => {
                            assert!(mic, "MicClose without MicOpen at {t}");
                            mic = false;
                        }
                        _ => {}
                    }
                }
                assert_eq!(mic, g.take_active(), "state {:?} at {t}", g.state());
                assert_eq!(g.blocking().esc, g.take_active());
                assert_eq!(g.blocking().fn_space, g.fn_is_down());
                assert!(g.blocking().fn_press);
            }
        }
    }
}
