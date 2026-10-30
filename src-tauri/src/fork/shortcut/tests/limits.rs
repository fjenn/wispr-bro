// SPDX-License-Identifier: GPL-3.0-or-later
//! S-01 additions to the grammar: DEC-54's History flag on late chords and
//! rule 8's take limits (DEC-32), on a fake clock.

use super::scenarios::drive;
use crate::fork::shortcut::processor::*;

const OPEN: Effect = Effect::MicOpen;
const CLOSE: Effect = Effect::MicClose;
const START: Effect = Effect::TakeStarted;

fn chord(key: &str, open: bool, keep: bool) -> Effect {
    Effect::ChordIgnored {
        key: key.into(),
        mic_was_open: open,
        keep,
    }
}

fn key(name: &str) -> Input {
    Input::KeyDown(Key::Other(name.into()))
}

/// Double-tap lock at 0/100/200/300 ms; the take starts at 200 ms.
fn locked_at_200() -> Vec<(Ms, Input)> {
    vec![
        (0, Input::FnDown),
        (100, Input::FnUp),
        (200, Input::FnDown),
        (300, Input::FnUp),
    ]
}

#[test]
fn chord_before_t_show_never_opens_the_mic_and_keeps_nothing() {
    let got = drive(
        &[
            (0, Input::FnDown),
            (80, key("LeftArrow")),
            (120, Input::FnUp),
        ],
        2000,
    )
    .unwrap();
    assert_eq!(got, vec![(80, chord("LeftArrow", false, false))]);
}

#[test]
fn chord_between_t_show_and_t_drops_without_history() {
    let got = drive(
        &[(0, Input::FnDown), (300, key("F11")), (400, Input::FnUp)],
        2000,
    )
    .unwrap();
    assert_eq!(
        got,
        vec![
            (150, OPEN),
            (150, START),
            (300, chord("F11", true, false)),
            (300, CLOSE)
        ]
    );
}

#[test]
fn chord_at_t_keeps_the_transcript_for_history() {
    let got = drive(
        &[(0, Input::FnDown), (500, key("Home")), (900, Input::FnUp)],
        2000,
    )
    .unwrap();
    assert_eq!(got[2], (500, chord("Home", true, true)));
}

#[test]
fn chord_one_ms_before_t_does_not_keep() {
    let got = drive(
        &[(0, Input::FnDown), (499, key("Home")), (900, Input::FnUp)],
        2000,
    )
    .unwrap();
    assert_eq!(got[2], (499, chord("Home", true, false)));
}

#[test]
fn chord_after_a_long_hold_keeps_and_the_release_does_nothing() {
    let got = drive(
        &[
            (0, Input::FnDown),
            (1700, key("Delete")),
            (2500, Input::FnUp),
        ],
        4000,
    )
    .unwrap();
    assert_eq!(
        got,
        vec![
            (150, OPEN),
            (150, START),
            (1700, chord("Delete", true, true)),
            (1700, CLOSE)
        ]
    );
}

#[test]
fn keys_while_locked_pass_through_without_a_chord() {
    let mut inputs = locked_at_200();
    inputs.push((900, Input::FnDown));
    inputs.push((950, key("LeftArrow")));
    let got = drive(&inputs, 3000).unwrap();
    assert!(!got
        .iter()
        .any(|(_, e)| matches!(e, Effect::ChordIgnored { .. })));
}

#[test]
fn hands_free_silence_warns_at_100_s_and_stops_at_120_s() {
    let got = drive(&locked_at_200(), 200_000).unwrap();
    assert_eq!(
        got,
        vec![
            (200, OPEN),
            (
                200,
                Effect::Locked {
                    via: LockVia::DoubleTap
                }
            ),
            (
                100_200,
                Effect::Warning {
                    kind: WarnKind::Silence
                }
            ),
            (
                120_200,
                Effect::Stopped {
                    via: StopVia::Silence
                }
            ),
            (120_200, CLOSE),
        ]
    );
}

#[test]
fn voice_resets_the_silence_timer_and_the_warning() {
    let mut g = Grammar::default();
    for (t, i) in locked_at_200() {
        g.step(t, &i);
    }
    assert_eq!(
        g.advance(100_200),
        vec![Effect::Warning {
            kind: WarnKind::Silence
        }]
    );
    g.step(110_000, &Input::Voice);
    // No stop at 120.2 s; a new warning 100 s after the voice.
    assert!(g.advance(120_200).is_empty());
    assert_eq!(g.next_deadline(), Some(210_000));
    assert_eq!(
        g.advance(210_000),
        vec![Effect::Warning {
            kind: WarnKind::Silence
        }]
    );
    assert_eq!(
        g.advance(230_000),
        vec![
            Effect::Stopped {
                via: StopVia::Silence
            },
            CLOSE
        ]
    );
    assert!(g.is_idle());
}

#[test]
fn length_cap_fires_on_time_when_driven_by_deadlines() {
    let mut g = Grammar::default();
    for (t, i) in locked_at_200() {
        g.step(t, &i);
    }
    // Speech every 50 s keeps silence away; walk the deadlines.
    let mut t = 300;
    let mut effects = Vec::new();
    while !g.is_idle() {
        let next = g.next_deadline().unwrap();
        let voice = t + 50_000;
        if voice < next {
            t = voice;
            g.step(t, &Input::Voice);
        } else {
            t = next;
            effects.extend(g.advance(t).into_iter().map(|e| (t, e)));
        }
    }
    assert_eq!(
        effects,
        vec![
            (
                28 * 60_000 + 200,
                Effect::Warning {
                    kind: WarnKind::Length
                }
            ),
            (
                30 * 60_000 + 200,
                Effect::Stopped {
                    via: StopVia::MaxLength
                }
            ),
            (30 * 60_000 + 200, CLOSE),
        ]
    );
}

#[test]
fn push_to_talk_has_no_silence_stop_but_has_the_length_cap() {
    let got = drive(
        &[(0, Input::FnDown), (31 * 60_000, Input::FnUp)],
        32 * 60_000,
    )
    .unwrap();
    assert_eq!(
        got,
        vec![
            (150, OPEN),
            (150, START),
            (
                28 * 60_000 + 150,
                Effect::Warning {
                    kind: WarnKind::Length
                }
            ),
            (
                30 * 60_000 + 150,
                Effect::Stopped {
                    via: StopVia::MaxLength
                }
            ),
            (30 * 60_000 + 150, CLOSE),
        ]
    );
}

#[test]
fn after_a_length_stop_with_fn_held_the_release_does_nothing() {
    let mut g = Grammar::default();
    g.step(0, &Input::FnDown);
    g.advance(30 * 60_000 + 150);
    assert_eq!(g.state(), State::Draining);
    assert!(g.step(30 * 60_000 + 500, &Input::FnUp).is_empty());
    assert!(g.is_idle());
}

#[test]
fn voice_outside_a_take_is_ignored() {
    let mut g = Grammar::default();
    assert!(g.step(10, &Input::Voice).is_empty());
    assert!(g.is_idle());
    assert_eq!(g.next_deadline(), None);
}

#[test]
fn fn_space_lock_also_gets_the_silence_timer() {
    let got = drive(
        &[
            (0, Input::FnDown),
            (60, Input::KeyDown(Key::Space)),
            (200, Input::FnUp),
        ],
        130_000,
    )
    .unwrap();
    assert_eq!(
        got,
        vec![
            (60, OPEN),
            (
                60,
                Effect::Locked {
                    via: LockVia::FnSpace
                }
            ),
            (
                100_060,
                Effect::Warning {
                    kind: WarnKind::Silence
                }
            ),
            (
                120_060,
                Effect::Stopped {
                    via: StopVia::Silence
                }
            ),
            (120_060, CLOSE),
        ]
    );
}

#[test]
fn esc_cancels_after_a_silence_warning() {
    let mut g = Grammar::default();
    for (t, i) in locked_at_200() {
        g.step(t, &i);
    }
    g.advance(100_200);
    assert_eq!(
        g.step(105_000, &Input::KeyDown(Key::Escape)),
        vec![
            Effect::Cancelled {
                why: CancelWhy::Escape
            },
            CLOSE
        ]
    );
    assert_eq!(g.next_deadline(), None);
}

#[test]
fn custom_limits_are_honoured() {
    let t = Timings {
        silence_warn_ms: 1_000,
        silence_stop_ms: 2_000,
        ..Timings::DEFAULT
    };
    let mut g = Grammar::new(t);
    for (ms, i) in locked_at_200() {
        g.step(ms, &i);
    }
    assert_eq!(
        g.advance(1_200),
        vec![Effect::Warning {
            kind: WarnKind::Silence
        }]
    );
    assert_eq!(
        g.advance(2_200),
        vec![
            Effect::Stopped {
                via: StopVia::Silence
            },
            CLOSE
        ]
    );
}

#[test]
fn default_timings_are_the_spec_values() {
    let t = Timings::DEFAULT;
    assert_eq!((t.tap_window_ms, t.show_ms), (500, 150));
    assert_eq!((t.silence_warn_ms, t.silence_stop_ms), (100_000, 120_000));
    assert_eq!(
        (t.length_warn_ms, t.length_stop_ms),
        (28 * 60_000, 30 * 60_000)
    );
    assert_eq!(Timings::with_show_ms(100).silence_stop_ms, 120_000);
}
