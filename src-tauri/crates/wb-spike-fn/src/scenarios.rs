// SPDX-License-Identifier: GPL-3.0-or-later
//! Scripted event sequences for the grammar, with the exact effects (and
//! their times) each must produce. Shared by the unit tests and
//! `wb-spike-fn --self-test`. The driver wakes at every grammar deadline the
//! way the live engine does, so timer effects carry the deadline's time.

use crate::grammar::{CancelWhy, Effect, Grammar, Input, Key, LockVia, Ms, StopVia};

pub struct Scenario {
    pub name: &'static str,
    /// (time, input) in order.
    pub inputs: Vec<(Ms, Input)>,
    /// Run timers up to this time after the last input.
    pub end: Ms,
    pub expect: Vec<(Ms, Effect)>,
}

/// Collects effects and checks that the mic effects alternate and match
/// `take_active()` after every step.
#[derive(Default)]
struct Trace {
    got: Vec<(Ms, Effect)>,
    mic: bool,
}

impl Trace {
    fn record(&mut self, t: Ms, effects: Vec<Effect>, g: &Grammar) -> Result<(), String> {
        for e in effects {
            match e {
                Effect::MicOpen if self.mic => return Err(format!("double MicOpen at {t}")),
                Effect::MicOpen => self.mic = true,
                Effect::MicClose if !self.mic => {
                    return Err(format!("MicClose while closed at {t}"))
                }
                Effect::MicClose => self.mic = false,
                _ => {}
            }
            self.got.push((t, e));
        }
        if self.mic != g.take_active() {
            return Err(format!("mic {} but state {:?} at {t}", self.mic, g.state()));
        }
        Ok(())
    }

    fn fire_until(&mut self, g: &mut Grammar, t: Ms) -> Result<(), String> {
        while let Some(d) = g.next_deadline() {
            if d > t {
                break;
            }
            let effects = g.advance(d);
            self.record(d, effects, g)?;
        }
        Ok(())
    }
}

/// Runs inputs through a fresh grammar; returns the effects with their times.
pub fn drive(inputs: &[(Ms, Input)], end: Ms) -> Result<Vec<(Ms, Effect)>, String> {
    let mut g = Grammar::default();
    let mut trace = Trace::default();
    for (t, input) in inputs {
        trace.fire_until(&mut g, *t)?;
        let effects = g.step(*t, input);
        trace.record(*t, effects, &g)?;
    }
    trace.fire_until(&mut g, end)?;
    Ok(trace.got)
}

pub fn run_scenario(s: &Scenario) -> Result<(), String> {
    let got = drive(&s.inputs, s.end)?;
    if got != s.expect {
        return Err(format!("\n  expected {:?}\n  got      {:?}", s.expect, got));
    }
    Ok(())
}

fn d() -> Input {
    Input::FnDown
}
fn u() -> Input {
    Input::FnUp
}
fn key(name: &str) -> Input {
    Input::KeyDown(Key::Other(name.into()))
}
fn space() -> Input {
    Input::KeyDown(Key::Space)
}
fn esc() -> Input {
    Input::KeyDown(Key::Escape)
}
const OPEN: Effect = Effect::MicOpen;
const CLOSE: Effect = Effect::MicClose;
const START: Effect = Effect::TakeStarted;
const TAP: Effect = Effect::TapIgnored;
const RELEASE: Effect = Effect::Stopped {
    via: StopVia::Release,
};
const PRESS_STOP: Effect = Effect::Stopped {
    via: StopVia::LockedPress,
};
const DOUBLE: Effect = Effect::Locked {
    via: LockVia::DoubleTap,
};
const FNSPACE: Effect = Effect::Locked {
    via: LockVia::FnSpace,
};
fn cancel(why: CancelWhy) -> Effect {
    Effect::Cancelled { why }
}
fn chord(k: &str, mic_was_open: bool) -> Effect {
    Effect::ChordIgnored {
        key: k.into(),
        mic_was_open,
    }
}

pub fn scenarios() -> Vec<Scenario> {
    use CancelWhy::*;
    vec![
        // --- Push to talk (rules 1 and 4) ---
        Scenario {
            name: "hold 1 s: take starts at T_show, stops on release",
            inputs: vec![(0, d()), (1000, u())],
            end: 3000,
            expect: vec![(150, OPEN), (150, START), (1000, RELEASE), (1000, CLOSE)],
        },
        Scenario {
            name: "hold released exactly at T counts as push to talk",
            inputs: vec![(0, d()), (500, u())],
            end: 2000,
            expect: vec![(150, OPEN), (150, START), (500, RELEASE), (500, CLOSE)],
        },
        Scenario {
            name: "long hold (30 s) still one take",
            inputs: vec![(0, d()), (30_000, u())],
            end: 31_000,
            expect: vec![
                (150, OPEN),
                (150, START),
                (30_000, RELEASE),
                (30_000, CLOSE),
            ],
        },
        Scenario {
            name: "two holds in a row are two takes",
            inputs: vec![(0, d()), (900, u()), (2000, d()), (2800, u())],
            end: 4000,
            expect: vec![
                (150, OPEN),
                (150, START),
                (900, RELEASE),
                (900, CLOSE),
                (2150, OPEN),
                (2150, START),
                (2800, RELEASE),
                (2800, CLOSE),
            ],
        },
        // --- Taps (rule 3) ---
        Scenario {
            name: "quick tap (< T_show) never opens the mic",
            inputs: vec![(0, d()), (80, u())],
            end: 2000,
            expect: vec![(580, TAP)],
        },
        Scenario {
            name: "tap released exactly at T_show has started the take",
            inputs: vec![(0, d()), (150, u())],
            end: 2000,
            expect: vec![
                (150, OPEN),
                (150, START),
                (650, cancel(NoSecondPress)),
                (650, CLOSE),
            ],
        },
        Scenario {
            name: "tap of 149 ms is still quick",
            inputs: vec![(0, d()), (149, u())],
            end: 2000,
            expect: vec![(649, TAP)],
        },
        Scenario {
            name: "press of 300 ms (between T_show and T) with no second press cancels",
            inputs: vec![(0, d()), (300, u())],
            end: 2000,
            expect: vec![
                (150, OPEN),
                (150, START),
                (800, cancel(NoSecondPress)),
                (800, CLOSE),
            ],
        },
        Scenario {
            name: "press of 499 ms is a tap, not push to talk",
            inputs: vec![(0, d()), (499, u())],
            end: 2000,
            expect: vec![
                (150, OPEN),
                (150, START),
                (999, cancel(NoSecondPress)),
                (999, CLOSE),
            ],
        },
        Scenario {
            name: "ten quick taps 1 s apart: ten ignored taps, mic never opens",
            inputs: (0..10)
                .flat_map(|i| [(i * 1000, d()), (i * 1000 + 70, u())])
                .collect(),
            end: 11_000,
            expect: (0..10).map(|i| (i * 1000 + 570, TAP)).collect(),
        },
        // --- Double-tap lock (rules 3 and 5) ---
        Scenario {
            name: "double-tap locks; the mic opens on the second press",
            inputs: vec![
                (0, d()),
                (80, u()),
                (200, d()),
                (280, u()),
                (1500, d()),
                (1580, u()),
            ],
            end: 3000,
            expect: vec![
                (200, OPEN),
                (200, DOUBLE),
                (1500, PRESS_STOP),
                (1500, CLOSE),
            ],
        },
        Scenario {
            name: "double-tap after a first press held past T_show keeps that audio",
            inputs: vec![
                (0, d()),
                (200, u()),
                (400, d()),
                (450, u()),
                (2000, d()),
                (2050, u()),
            ],
            end: 3000,
            expect: vec![
                (150, OPEN),
                (150, START),
                (400, DOUBLE),
                (2000, PRESS_STOP),
                (2000, CLOSE),
            ],
        },
        Scenario {
            name: "second press 1 ms before the window closes still locks",
            inputs: vec![(0, d()), (50, u()), (549, d()), (600, u())],
            end: 1000,
            expect: vec![(549, OPEN), (549, DOUBLE)],
        },
        Scenario {
            name: "second press exactly at T after release is a new press, not a lock",
            inputs: vec![(0, d()), (50, u()), (550, d()), (600, u())],
            end: 2000,
            expect: vec![(550, TAP), (1100, TAP)],
        },
        Scenario {
            name: "third quick press (within T of locking) cancels",
            inputs: vec![
                (0, d()),
                (60, u()),
                (180, d()),
                (240, u()),
                (400, d()),
                (460, u()),
            ],
            end: 2000,
            expect: vec![
                (180, OPEN),
                (180, DOUBLE),
                (400, cancel(TripleTap)),
                (400, CLOSE),
            ],
        },
        Scenario {
            name: "holding the second press does not stop the locked take",
            inputs: vec![
                (0, d()),
                (60, u()),
                (180, d()),
                (2000, u()),
                (4000, d()),
                (4100, u()),
            ],
            end: 5000,
            expect: vec![
                (180, OPEN),
                (180, DOUBLE),
                (4000, PRESS_STOP),
                (4000, CLOSE),
            ],
        },
        Scenario {
            name: "stop press held long does not start a new take",
            inputs: vec![
                (0, d()),
                (60, u()),
                (180, d()),
                (240, u()),
                (1500, d()),
                (3000, u()),
            ],
            end: 4000,
            expect: vec![
                (180, OPEN),
                (180, DOUBLE),
                (1500, PRESS_STOP),
                (1500, CLOSE),
            ],
        },
        // --- fn+Space (rule 6) ---
        Scenario {
            name: "fn+Space before T_show locks and opens the mic at once",
            inputs: vec![
                (0, d()),
                (50, space()),
                (300, u()),
                (1500, d()),
                (1600, u()),
            ],
            end: 3000,
            expect: vec![(50, OPEN), (50, FNSPACE), (1500, PRESS_STOP), (1500, CLOSE)],
        },
        Scenario {
            name: "fn+Space during a push-to-talk take turns it hands-free",
            inputs: vec![
                (0, d()),
                (400, space()),
                (600, u()),
                (2000, d()),
                (2100, u()),
            ],
            end: 3000,
            expect: vec![
                (150, OPEN),
                (150, START),
                (400, FNSPACE),
                (2000, PRESS_STOP),
                (2000, CLOSE),
            ],
        },
        Scenario {
            name: "fn+Space after T also locks (no stop on release)",
            inputs: vec![
                (0, d()),
                (800, space()),
                (900, u()),
                (3000, d()),
                (3050, u()),
            ],
            end: 4000,
            expect: vec![
                (150, OPEN),
                (150, START),
                (800, FNSPACE),
                (3000, PRESS_STOP),
                (3000, CLOSE),
            ],
        },
        Scenario {
            name: "Space again while locked with fn held does nothing",
            inputs: vec![(0, d()), (50, space()), (100, space()), (300, u())],
            end: 1000,
            expect: vec![(50, OPEN), (50, FNSPACE)],
        },
        Scenario {
            name: "Space without fn does nothing",
            inputs: vec![(0, space()), (100, space())],
            end: 1000,
            expect: vec![],
        },
        Scenario {
            name: "fn press within T of an fn+Space lock cancels (rule 5)",
            inputs: vec![(0, d()), (50, space()), (100, u()), (300, d()), (350, u())],
            end: 1000,
            expect: vec![
                (50, OPEN),
                (50, FNSPACE),
                (300, cancel(TripleTap)),
                (300, CLOSE),
            ],
        },
        // --- Esc (rule 7) ---
        Scenario {
            name: "Esc while holding cancels; release afterwards does nothing",
            inputs: vec![(0, d()), (900, esc()), (1200, u())],
            end: 2000,
            expect: vec![
                (150, OPEN),
                (150, START),
                (900, cancel(Escape)),
                (900, CLOSE),
            ],
        },
        Scenario {
            name: "Esc while locked (fn up) cancels",
            inputs: vec![(0, d()), (60, u()), (200, d()), (260, u()), (1500, esc())],
            end: 2000,
            expect: vec![
                (200, OPEN),
                (200, DOUBLE),
                (1500, cancel(Escape)),
                (1500, CLOSE),
            ],
        },
        Scenario {
            name: "Esc during the tap window after a long-enough press cancels",
            inputs: vec![(0, d()), (300, u()), (400, esc())],
            end: 2000,
            expect: vec![
                (150, OPEN),
                (150, START),
                (400, cancel(Escape)),
                (400, CLOSE),
            ],
        },
        Scenario {
            name: "Esc when idle is not ours",
            inputs: vec![(0, esc()), (500, esc())],
            end: 1000,
            expect: vec![],
        },
        Scenario {
            name: "fn+Esc before T_show is a chord (no take to cancel)",
            inputs: vec![(0, d()), (60, esc()), (200, u())],
            end: 1000,
            expect: vec![(60, chord("Escape", false))],
        },
        // --- Chords (rule 2) ---
        Scenario {
            name: "fn+Home within T_show: chord, mic never opens",
            inputs: vec![(0, d()), (60, key("Home")), (200, u())],
            end: 2000,
            expect: vec![(60, chord("Home", false))],
        },
        Scenario {
            name: "auto-repeat or several keys in one chord: one discard only",
            inputs: vec![
                (0, d()),
                (40, key("End")),
                (90, key("End")),
                (140, key("PageUp")),
                (700, key("PageDown")),
                (900, u()),
            ],
            end: 2000,
            expect: vec![(40, chord("End", false))],
        },
        Scenario {
            name: "slow chord (key after T_show, before T): take discarded, mic was open",
            inputs: vec![(0, d()), (300, key("ForwardDelete")), (400, u())],
            end: 2000,
            expect: vec![
                (150, OPEN),
                (150, START),
                (300, chord("ForwardDelete", true)),
                (300, CLOSE),
            ],
        },
        Scenario {
            name: "key after T is still a chord: take dropped, nothing inserted (DEC-54)",
            inputs: vec![
                (0, d()),
                (1100, key("PageUp")),
                (1300, key("PageDown")),
                (2000, u()),
            ],
            end: 3000,
            expect: vec![
                (150, OPEN),
                (150, START),
                (1100, chord("PageUp", true)),
                (1100, CLOSE),
            ],
        },
        Scenario {
            name: "keys while locked pass through",
            inputs: vec![
                (0, d()),
                (60, u()),
                (200, d()),
                (260, u()),
                (900, key("A")),
                (2000, d()),
            ],
            end: 3000,
            expect: vec![
                (200, OPEN),
                (200, DOUBLE),
                (2000, PRESS_STOP),
                (2000, CLOSE),
            ],
        },
        Scenario {
            name: "plain typing without fn never starts anything",
            inputs: vec![(0, key("A")), (50, key("B")), (100, key("Home"))],
            end: 1000,
            expect: vec![],
        },
        Scenario {
            name: "chord then quick re-press is a fresh tap",
            inputs: vec![
                (0, d()),
                (50, key("F11")),
                (100, u()),
                (300, d()),
                (350, u()),
            ],
            end: 2000,
            expect: vec![(50, chord("F11", false)), (850, TAP)],
        },
        Scenario {
            name: "fn+Space after a chord with fn still held locks (rule 6, see notes)",
            inputs: vec![(0, d()), (50, key("Home")), (300, space()), (400, u())],
            end: 1000,
            expect: vec![(50, chord("Home", false)), (300, OPEN), (300, FNSPACE)],
        },
        // --- Robustness ---
        Scenario {
            name: "duplicate fn down and stray fn up are ignored",
            inputs: vec![(0, u()), (10, d()), (20, d()), (1000, u()), (1010, u())],
            end: 2000,
            expect: vec![(160, OPEN), (160, START), (1000, RELEASE), (1000, CLOSE)],
        },
        Scenario {
            name: "Esc after cancel with fn still held is not swallowed again",
            inputs: vec![(0, d()), (600, esc()), (700, esc()), (800, u())],
            end: 1500,
            expect: vec![
                (150, OPEN),
                (150, START),
                (600, cancel(Escape)),
                (600, CLOSE),
            ],
        },
    ]
}
