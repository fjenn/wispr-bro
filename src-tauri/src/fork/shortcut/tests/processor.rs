// SPDX-License-Identifier: GPL-3.0-or-later
//! Unit tests of the grammar (`processor.rs`), lifted from the RS-01 spike
//! with the S-01 additions (DEC-54 History flag, rule 8 limits).

use super::scenarios::{run_scenario, scenarios};
use crate::fork::shortcut::processor::*;

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
                mic_was_open: true,
                keep: false
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
            mic_was_open: false,
            keep: false
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
    // Locked: the next deadline is the hands-free silence warning (rule 8).
    assert_eq!(g.next_deadline(), Some(1200 + 100_000));
}

#[test]
fn custom_timings_are_honoured() {
    let mut g = Grammar::new(Timings {
        tap_window_ms: 300,
        show_ms: 100,
        ..Timings::DEFAULT
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
