// SPDX-License-Identifier: GPL-3.0-or-later
//! What one attempt ("episode": from fn leaving idle until the grammar is idle
//! again) looked like, and whether it meets the RS-01 pass criteria for the
//! block it was made in. Pure code, unit-tested.

use serde::Serialize;

use crate::grammar::{CancelWhy, Effect, LockVia, Ms, StopVia, Timings};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockKind {
    Hold,
    QuickTap,
    DoubleTap,
    FnSpace,
    EscHold,
    Chords,
    Clipping,
}

impl BlockKind {
    pub fn id(self) -> &'static str {
        match self {
            BlockKind::Hold => "hold",
            BlockKind::QuickTap => "quick_tap",
            BlockKind::DoubleTap => "double_tap",
            BlockKind::FnSpace => "fn_space",
            BlockKind::EscHold => "esc_hold",
            BlockKind::Chords => "chords",
            BlockKind::Clipping => "clipping",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Press {
    pub down_ms: Ms,
    pub up_ms: Option<Ms>,
}

impl Press {
    pub fn held_ms(&self) -> Option<Ms> {
        self.up_ms.map(|u| u.saturating_sub(self.down_ms))
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ChordKey {
    pub key: String,
    /// Time from the fn press to this key.
    pub after_fn_ms: Ms,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct MicSpan {
    /// Take id (one per mic open).
    pub take: u64,
    /// Session ms when the grammar asked for the mic.
    pub requested_ms: f64,
    /// When `Recorder::open` returned.
    pub opened_ms: Option<f64>,
    /// First audio chunk delivered (capture running).
    pub first_audio_ms: Option<f64>,
    pub closed_ms: Option<f64>,
    pub wav: Option<String>,
    /// Loudest sample of the take (0.0 = digital silence: muted or denied).
    pub peak: Option<f32>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Episode {
    /// 1-based, within its block.
    pub n: u32,
    pub start_ms: Ms,
    pub end_ms: Ms,
    pub presses: Vec<Press>,
    /// Non-modifier keys pressed while fn was down (repeats excluded).
    pub chord_keys: Vec<ChordKey>,
    pub effects: Vec<(Ms, Effect)>,
    pub mic: Vec<MicSpan>,
}

impl Episode {
    pub fn has(&self, pred: impl Fn(&Effect) -> bool) -> bool {
        self.effects.iter().any(|(_, e)| pred(e))
    }
    pub fn mic_opened(&self) -> bool {
        self.has(|e| *e == Effect::MicOpen)
    }
    pub fn started(&self) -> bool {
        self.has(|e| matches!(e, Effect::TakeStarted | Effect::Locked { .. }))
    }
    pub fn locked_via(&self, via: LockVia) -> bool {
        self.has(|e| *e == Effect::Locked { via })
    }
    /// A take that started and was then dropped by the chord rule.
    pub fn chord_discarded_take(&self) -> bool {
        self.has(|e| {
            matches!(
                e,
                Effect::ChordIgnored {
                    mic_was_open: true,
                    ..
                }
            )
        })
    }
    pub fn first_held_ms(&self) -> Option<Ms> {
        self.presses.first().and_then(Press::held_ms)
    }
    /// fn press → first audio chunk (DEC-47 / §5.1 "capture running").
    pub fn fn_to_first_audio_ms(&self) -> Option<f64> {
        let down = self.presses.first()?.down_ms as f64;
        Some(self.mic.first()?.first_audio_ms? - down)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Pass,
    Fail,
    /// The owner did a different gesture than the block asked for; not
    /// counted against the criteria (but reported).
    Invalid,
}

#[derive(Clone, Debug, Serialize)]
pub struct Verdict {
    pub outcome: Outcome,
    pub note: String,
}

fn v(outcome: Outcome, note: impl Into<String>) -> Verdict {
    Verdict {
        outcome,
        note: note.into(),
    }
}

fn secs(ms: Ms) -> String {
    format!("{:.2} s", ms as f64 / 1000.0)
}

/// Judges one episode against what its block asked for.
pub fn judge(kind: BlockKind, ep: &Episode, t: Timings) -> Verdict {
    use Outcome::*;
    let held = ep.first_held_ms();
    let chord = ep.has(|e| matches!(e, Effect::ChordIgnored { .. }));
    match kind {
        BlockKind::Hold | BlockKind::Clipping => {
            if chord {
                return v(Invalid, "another key was pressed with fn");
            }
            match held {
                Some(h) if h < t.tap_window_ms => v(
                    Invalid,
                    format!(
                        "let go after {} (shorter than 0.5 s, so it counts as a tap)",
                        secs(h)
                    ),
                ),
                _ if ep.has(|e| matches!(e, Effect::Locked { .. })) => {
                    v(Invalid, "became hands-free (double-tap or fn+Space)")
                }
                _ => {
                    let started = ep.has(|e| *e == Effect::TakeStarted);
                    let stopped = ep.has(|e| {
                        *e == Effect::Stopped {
                            via: StopVia::Release,
                        }
                    });
                    let wav_ok = kind != BlockKind::Clipping
                        || ep.mic.first().is_some_and(|m| m.wav.is_some());
                    match (started, stopped, wav_ok) {
                        (true, true, true) => {
                            v(Pass, format!("held {}", held.map(secs).unwrap_or_default()))
                        }
                        (true, true, false) => v(Fail, "take ran but no audio was saved"),
                        (false, _, _) => v(Fail, "the take never started"),
                        (true, false, _) => v(Fail, "the take did not stop on release"),
                    }
                }
            }
        }
        BlockKind::QuickTap => {
            if ep.presses.len() > 1 {
                return v(Invalid, "two presses close together (that is a double-tap)");
            }
            if chord {
                return v(Invalid, "another key was pressed with fn");
            }
            match held {
                Some(h) if h >= t.show_ms => v(
                    Invalid,
                    format!("press lasted {} (a quick tap is under 0.15 s)", secs(h)),
                ),
                _ if ep.mic_opened() || ep.started() => v(Fail, "the mic opened on a quick tap"),
                Some(h) => v(Pass, format!("tap of {} ms, mic stayed closed", h)),
                None => v(Invalid, "fn was not released"),
            }
        }
        BlockKind::DoubleTap => {
            if ep.locked_via(LockVia::DoubleTap) {
                let stop = if ep.has(|e| {
                    *e == Effect::Stopped {
                        via: StopVia::LockedPress,
                    }
                }) {
                    "locked, stopped with one press"
                } else if ep.has(|e| {
                    *e == Effect::Cancelled {
                        why: CancelWhy::TripleTap,
                    }
                }) {
                    "locked, then the stop press came within 0.5 s and cancelled"
                } else {
                    "locked (not stopped by a press)"
                };
                v(Pass, stop)
            } else if ep.presses.len() < 2 {
                v(Invalid, "only one press")
            } else {
                v(Fail, "two presses but no hands-free lock")
            }
        }
        BlockKind::FnSpace => {
            if ep.locked_via(LockVia::FnSpace) {
                v(Pass, "locked by fn+Space")
            } else if !ep.chord_keys.iter().any(|k| k.key == "Space") {
                v(Invalid, "Space was not pressed while fn was held")
            } else {
                v(Fail, "fn+Space did not lock")
            }
        }
        BlockKind::EscHold => {
            let esc = ep.chord_keys.iter().any(|k| k.key == "Escape");
            let cancelled = ep.has(|e| {
                *e == Effect::Cancelled {
                    why: CancelWhy::Escape,
                }
            });
            if cancelled && !ep.has(|e| matches!(e, Effect::Stopped { .. })) {
                v(Pass, "Esc cancelled the take")
            } else if !esc {
                v(Invalid, "Esc was not pressed while fn was held")
            } else if chord {
                v(Invalid, "Esc came before the take started (under 0.15 s)")
            } else {
                v(Fail, "Esc did not cancel")
            }
        }
        BlockKind::Chords => {
            if ep.chord_discarded_take() {
                v(
                    Fail,
                    format!(
                        "fn was held {} before the key, so the mic opened (DEC-47 T_show) and the take was dropped",
                        ep.chord_keys
                            .first()
                            .map(|k| format!("{} ms", k.after_fn_ms))
                            .unwrap_or_default()
                    ),
                )
            } else if ep.started() || ep.mic_opened() {
                v(Fail, "a take started during a chord")
            } else if ep.chord_keys.is_empty() {
                v(Invalid, "fn alone, no chord key")
            } else {
                v(
                    Pass,
                    format!("{} chord key(s), no start", ep.chord_keys.len()),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::Input;
    use crate::scenarios::drive;

    const T: Timings = Timings::DEFAULT;

    /// Builds an episode from scripted inputs, the way the engine does.
    fn episode(inputs: &[(Ms, Input)], chord_keys: &[(&str, Ms)]) -> Episode {
        let effects = drive(inputs, 10_000).expect("drive");
        let mut presses: Vec<Press> = Vec::new();
        for (t, i) in inputs {
            match i {
                Input::FnDown => presses.push(Press {
                    down_ms: *t,
                    up_ms: None,
                }),
                Input::FnUp => {
                    if let Some(p) = presses.last_mut() {
                        p.up_ms = Some(*t);
                    }
                }
                _ => {}
            }
        }
        let mic = effects
            .iter()
            .filter(|(_, e)| *e == Effect::MicOpen)
            .map(|(t, _)| MicSpan {
                requested_ms: *t as f64,
                first_audio_ms: Some(*t as f64 + 40.0),
                wav: Some("take.wav".into()),
                ..Default::default()
            })
            .collect();
        Episode {
            n: 1,
            presses,
            chord_keys: chord_keys
                .iter()
                .map(|(k, ms)| ChordKey {
                    key: k.to_string(),
                    after_fn_ms: *ms,
                })
                .collect(),
            effects,
            mic,
            ..Default::default()
        }
    }

    fn outcome(kind: BlockKind, ep: &Episode) -> Outcome {
        judge(kind, ep, T).outcome
    }

    #[test]
    fn hold() {
        let ok = episode(&[(0, Input::FnDown), (1000, Input::FnUp)], &[]);
        assert_eq!(outcome(BlockKind::Hold, &ok), Outcome::Pass);
        assert_eq!(ok.fn_to_first_audio_ms(), Some(190.0));
        let short = episode(&[(0, Input::FnDown), (300, Input::FnUp)], &[]);
        assert_eq!(outcome(BlockKind::Hold, &short), Outcome::Invalid);
    }

    #[test]
    fn clipping_needs_a_wav() {
        let mut ep = episode(&[(0, Input::FnDown), (2500, Input::FnUp)], &[]);
        assert_eq!(outcome(BlockKind::Clipping, &ep), Outcome::Pass);
        ep.mic[0].wav = None;
        assert_eq!(outcome(BlockKind::Clipping, &ep), Outcome::Fail);
    }

    #[test]
    fn quick_tap() {
        let ok = episode(&[(0, Input::FnDown), (90, Input::FnUp)], &[]);
        assert_eq!(outcome(BlockKind::QuickTap, &ok), Outcome::Pass);
        let slow = episode(&[(0, Input::FnDown), (200, Input::FnUp)], &[]);
        assert_eq!(outcome(BlockKind::QuickTap, &slow), Outcome::Invalid);
        let double = episode(
            &[
                (0, Input::FnDown),
                (60, Input::FnUp),
                (200, Input::FnDown),
                (260, Input::FnUp),
            ],
            &[],
        );
        assert_eq!(outcome(BlockKind::QuickTap, &double), Outcome::Invalid);
        // A quick tap that somehow opened the mic fails.
        let mut bad = ok.clone();
        bad.effects.push((50, Effect::MicOpen));
        assert_eq!(outcome(BlockKind::QuickTap, &bad), Outcome::Fail);
    }

    #[test]
    fn double_tap() {
        let ok = episode(
            &[
                (0, Input::FnDown),
                (60, Input::FnUp),
                (200, Input::FnDown),
                (260, Input::FnUp),
                (1500, Input::FnDown),
                (1560, Input::FnUp),
            ],
            &[],
        );
        let verdict = judge(BlockKind::DoubleTap, &ok, T);
        assert_eq!(verdict.outcome, Outcome::Pass);
        assert!(verdict.note.contains("stopped"));
        let slow = episode(
            &[
                (0, Input::FnDown),
                (60, Input::FnUp),
                (800, Input::FnDown),
                (860, Input::FnUp),
            ],
            &[],
        );
        // The engine would split this into two episodes; as one it is a fail.
        assert_eq!(outcome(BlockKind::DoubleTap, &slow), Outcome::Fail);
        let one = episode(&[(0, Input::FnDown), (60, Input::FnUp)], &[]);
        assert_eq!(outcome(BlockKind::DoubleTap, &one), Outcome::Invalid);
    }

    #[test]
    fn fn_space() {
        let ok = episode(
            &[
                (0, Input::FnDown),
                (80, Input::KeyDown(crate::grammar::Key::Space)),
                (300, Input::FnUp),
                (1500, Input::FnDown),
            ],
            &[("Space", 80)],
        );
        assert_eq!(outcome(BlockKind::FnSpace, &ok), Outcome::Pass);
        let none = episode(&[(0, Input::FnDown), (300, Input::FnUp)], &[]);
        assert_eq!(outcome(BlockKind::FnSpace, &none), Outcome::Invalid);
    }

    #[test]
    fn esc_hold() {
        let ok = episode(
            &[
                (0, Input::FnDown),
                (900, Input::KeyDown(crate::grammar::Key::Escape)),
                (1200, Input::FnUp),
            ],
            &[("Escape", 900)],
        );
        assert_eq!(outcome(BlockKind::EscHold, &ok), Outcome::Pass);
        let early = episode(
            &[
                (0, Input::FnDown),
                (50, Input::KeyDown(crate::grammar::Key::Escape)),
                (300, Input::FnUp),
            ],
            &[("Escape", 50)],
        );
        assert_eq!(outcome(BlockKind::EscHold, &early), Outcome::Invalid);
        let none = episode(&[(0, Input::FnDown), (900, Input::FnUp)], &[]);
        assert_eq!(outcome(BlockKind::EscHold, &none), Outcome::Invalid);
    }

    #[test]
    fn chords() {
        let k = |n: &str| Input::KeyDown(crate::grammar::Key::Other(n.into()));
        let ok = episode(
            &[(0, Input::FnDown), (70, k("Home")), (200, Input::FnUp)],
            &[("Home", 70)],
        );
        assert_eq!(outcome(BlockKind::Chords, &ok), Outcome::Pass);
        let slow = episode(
            &[(0, Input::FnDown), (250, k("End")), (400, Input::FnUp)],
            &[("End", 250)],
        );
        let verdict = judge(BlockKind::Chords, &slow, T);
        assert_eq!(verdict.outcome, Outcome::Fail);
        assert!(verdict.note.contains("250 ms"), "{}", verdict.note);
        let held = episode(
            &[(0, Input::FnDown), (800, k("End")), (1000, Input::FnUp)],
            &[("End", 800)],
        );
        assert_eq!(outcome(BlockKind::Chords, &held), Outcome::Fail);
        let alone = episode(&[(0, Input::FnDown), (60, Input::FnUp)], &[]);
        assert_eq!(outcome(BlockKind::Chords, &alone), Outcome::Invalid);
    }
}
