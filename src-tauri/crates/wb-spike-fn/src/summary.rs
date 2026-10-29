// SPDX-License-Identifier: GPL-3.0-or-later
//! Turns the block runs, the owner's answers and the tap health checks into
//! the RS-01 criteria table (summary.json and the printed summary). Pure.

use serde::Serialize;

use crate::engine::BlockRun;
use crate::system::TapInfo;
use crate::verdict::{BlockKind, Outcome};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Ans {
    Yes,
    No,
    Unsure,
}

#[derive(Clone, Debug, Serialize)]
pub struct Answer {
    pub round: String,
    pub block: BlockKind,
    /// Stable id: `popup`, `dot`, `space_typed`, `arrows`, …
    pub id: String,
    pub question: String,
    /// The answer that means "pass" (None: informational).
    pub expected: Option<Ans>,
    pub answer: Ans,
}

impl Answer {
    pub fn matches(&self) -> Option<bool> {
        match (self.expected, self.answer) {
            (None, _) | (_, Ans::Unsure) => None,
            (Some(e), a) => Some(e == a),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct HealthCheck {
    pub label: String,
    /// (tap owner "handy"/"fork"/"other", info)
    pub taps: Vec<(String, TapInfo)>,
    pub secure_input: bool,
}

impl HealthCheck {
    pub fn ok(&self) -> bool {
        let has = |who: &str| self.taps.iter().any(|(w, t)| w == who && t.enabled);
        has("handy") && has("fork") && self.taps.iter().all(|(_, t)| t.enabled)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pass,
    Fail,
    /// Not enough data (block skipped, or every answer "not sure").
    NotRun,
    /// Numbers only; a person must judge (e.g. listening to the WAVs).
    Info,
}

impl Status {
    pub fn word(self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
            Status::NotRun => "not run",
            Status::Info => "info",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Criterion {
    pub id: String,
    pub text: String,
    pub status: Status,
    pub detail: String,
}

fn crit(id: &str, text: &str, status: Status, detail: String) -> Criterion {
    Criterion {
        id: id.into(),
        text: text.into(),
        status,
        detail,
    }
}

fn runs_of(runs: &[BlockRun], kind: BlockKind) -> impl Iterator<Item = &BlockRun> {
    runs.iter().filter(move |r| r.kind == kind)
}

fn outcome_criterion(runs: &[BlockRun], kind: BlockKind, id: &str, text: &str) -> Criterion {
    let (mut pass, mut fail, mut invalid) = (0, 0, 0);
    let mut fails = Vec::new();
    for r in runs_of(runs, kind) {
        pass += r.count(Outcome::Pass);
        fail += r.count(Outcome::Fail);
        invalid += r.count(Outcome::Invalid);
        for e in &r.episodes {
            if e.verdict.outcome == Outcome::Fail {
                fails.push(format!("[{}] #{} {}", r.round, e.episode.n, e.verdict.note));
            }
        }
    }
    let status = if fail > 0 {
        Status::Fail
    } else if pass == 0 {
        Status::NotRun
    } else {
        Status::Pass
    };
    let mut detail = format!("{pass} ok, {fail} problem(s), {invalid} not counted");
    if !fails.is_empty() {
        detail.push_str(": ");
        detail.push_str(&fails.join("; "));
    }
    crit(id, text, status, detail)
}

pub fn percentile(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let rank = (p / 100.0 * (sorted.len() - 1) as f64).round() as usize;
    sorted.get(rank.min(sorted.len() - 1)).copied()
}

fn answers_criterion(
    answers: &[Answer],
    id: &str,
    text: &str,
    pick: impl Fn(&Answer) -> bool,
) -> Criterion {
    let picked: Vec<&Answer> = answers.iter().filter(|a| pick(a)).collect();
    let bad: Vec<String> = picked
        .iter()
        .filter(|a| a.matches() == Some(false))
        .map(|a| format!("[{}/{}] {}", a.round, a.block.id(), a.question))
        .collect();
    let judged = picked.iter().filter(|a| a.matches().is_some()).count();
    let status = if !bad.is_empty() {
        Status::Fail
    } else if judged == 0 {
        Status::NotRun
    } else {
        Status::Pass
    };
    let detail = if bad.is_empty() {
        format!("{judged} answer(s) as expected")
    } else {
        format!("unexpected answer to: {}", bad.join("; "))
    };
    crit(id, text, status, detail)
}

pub fn criteria(runs: &[BlockRun], answers: &[Answer], health: &[HealthCheck]) -> Vec<Criterion> {
    let mut out = vec![
        outcome_criterion(
            runs,
            BlockKind::Hold,
            "hold_starts",
            "Every 1 s hold starts a take (at 150 ms) and stops on release",
        ),
        outcome_criterion(
            runs,
            BlockKind::DoubleTap,
            "double_tap_locks",
            "Every double-tap locks hands-free",
        ),
        outcome_criterion(
            runs,
            BlockKind::FnSpace,
            "fn_space_locks",
            "Every fn+Space locks hands-free",
        ),
        outcome_criterion(
            runs,
            BlockKind::EscHold,
            "esc_cancels",
            "Esc while holding fn cancels",
        ),
        outcome_criterion(
            runs,
            BlockKind::QuickTap,
            "quick_tap_no_mic",
            "The mic never opens on a quick tap (< 150 ms)",
        ),
    ];

    // Chords.
    let chord_runs: Vec<&BlockRun> = runs_of(runs, BlockKind::Chords).collect();
    let chord_keys: u32 = chord_runs.iter().map(|r| r.chord_keys).sum();
    let mut starts = 0;
    let mut mic_opens = 0;
    let mut slow = Vec::new();
    for r in &chord_runs {
        for e in &r.episodes {
            let ep = &e.episode;
            if ep.started() && !ep.chord_discarded_take() {
                starts += 1;
            }
            if ep.mic_opened() {
                mic_opens += 1;
                if let Some(k) = ep.chord_keys.first() {
                    slow.push(format!("{} after {} ms", k.key, k.after_fn_ms));
                }
            }
        }
    }
    let ran = !chord_runs.is_empty() && chord_keys > 0;
    let st = |bad: usize| {
        if !ran {
            Status::NotRun
        } else if bad > 0 {
            Status::Fail
        } else {
            Status::Pass
        }
    };
    out.push(crit(
        "chords_no_start",
        "0 takes start during fn chords",
        st(starts),
        format!("{starts} start(s) in {chord_keys} chord key(s)"),
    ));
    out.push(crit(
        "chords_no_mic",
        "The mic never opens during fn chords",
        st(mic_opens),
        if slow.is_empty() {
            format!("{mic_opens} mic open(s)")
        } else {
            format!(
                "{mic_opens} mic open(s), each a chord where fn was held longer than 150 ms first: {}",
                slow.join(", ")
            )
        },
    ));
    out.push(crit(
        "chord_count",
        "Chord count (RS-01 asks for 100; this session does about 30)",
        Status::Info,
        format!("{chord_keys} chord key(s)"),
    ));

    // Both taps alive.
    let bad: Vec<&str> = health
        .iter()
        .filter(|h| !h.ok())
        .map(|h| h.label.as_str())
        .collect();
    out.push(crit(
        "taps_alive",
        "Both taps (Handy's and the fork's) present and enabled at every check",
        if health.is_empty() {
            Status::NotRun
        } else if bad.is_empty() {
            Status::Pass
        } else {
            Status::Fail
        },
        if bad.is_empty() {
            format!("{} check(s)", health.len())
        } else {
            format!("failed at: {}", bad.join(", "))
        },
    ));
    let secure: Vec<&str> = health
        .iter()
        .filter(|h| h.secure_input)
        .map(|h| h.label.as_str())
        .collect();
    if !secure.is_empty() {
        out.push(crit(
            "secure_input",
            "Secure Input was on at some checks (keys other than fn are invisible then)",
            Status::Info,
            secure.join(", "),
        ));
    }

    // fn-down → capture running (cold stream), §5.1: p50 ≤ 250 ms, p95 ≤ 350 ms.
    let mut lat: Vec<f64> = runs
        .iter()
        .filter(|r| matches!(r.kind, BlockKind::Hold | BlockKind::Clipping))
        .flat_map(|r| r.episodes.iter())
        .filter(|e| e.verdict.outcome == Outcome::Pass)
        .filter_map(|e| e.episode.fn_to_first_audio_ms())
        .collect();
    lat.sort_by(|a, b| a.total_cmp(b));
    out.push(match (percentile(&lat, 50.0), percentile(&lat, 95.0)) {
        (Some(p50), Some(p95)) => crit(
            "capture_latency",
            "fn press → first audio, cold mic (§5.1: p50 ≤ 250 ms, p95 ≤ 350 ms)",
            if p50 <= 250.0 && p95 <= 350.0 {
                Status::Pass
            } else {
                Status::Fail
            },
            format!(
                "p50 {p50:.0} ms, p95 {p95:.0} ms over {} take(s)",
                lat.len()
            ),
        ),
        _ => crit(
            "capture_latency",
            "fn press → first audio, cold mic (§5.1: p50 ≤ 250 ms, p95 ≤ 350 ms)",
            Status::NotRun,
            "no takes with audio".into(),
        ),
    });

    // Clipping WAVs (listening/transcription is a later step).
    let clips: Vec<&BlockRun> = runs_of(runs, BlockKind::Clipping).collect();
    let saved = clips
        .iter()
        .flat_map(|r| r.episodes.iter())
        .filter(|e| e.episode.mic.iter().any(|m| m.wav.is_some()))
        .count();
    out.push(crit(
        "clipping_saved",
        "First-word clipping takes saved (≤ 1 clipped first word allowed; judged later by listening)",
        if clips.is_empty() {
            Status::NotRun
        } else {
            Status::Info
        },
        format!("{saved} WAV(s) saved (RS-01 asks for 50 takes; this session does 10)"),
    ));

    // The owner's eyes.
    out.push(answers_criterion(
        answers,
        "no_globe_action",
        "No Globe action (emoji picker, input switch, Dictation) on hold, tap or double-tap",
        |a| a.id == "popup",
    ));
    out.push(answers_criterion(
        answers,
        "no_indicator_taps_chords",
        "Orange mic dot never shown during quick taps or chords",
        |a| a.id == "dot" && matches!(a.block, BlockKind::QuickTap | BlockKind::Chords),
    ));
    out.push(answers_criterion(
        answers,
        "chords_native",
        "fn chords keep their normal function",
        |a| a.block == BlockKind::Chords && a.id != "dot" && a.id != "popup",
    ));
    out.push(answers_criterion(
        answers,
        "indicator_when_recording",
        "Orange mic dot shown while recording and gone after",
        |a| {
            a.id == "dot" && !matches!(a.block, BlockKind::QuickTap | BlockKind::Chords)
                || a.id == "dot_gone"
        },
    ));
    out.push(answers_criterion(
        answers,
        "space_swallowed",
        "fn+Space types no space",
        |a| a.id == "space_typed",
    ));
    out
}

/// Per-Globe-setting table of the owner's "did anything pop up" answers.
pub fn globe_table(answers: &[Answer]) -> Vec<(String, String, String)> {
    let mut rows: Vec<(String, String, String)> = Vec::new();
    for a in answers.iter().filter(|a| a.id == "popup") {
        let word = match a.answer {
            Ans::Yes => "POPPED UP",
            Ans::No => "nothing",
            Ans::Unsure => "not sure",
        };
        rows.push((a.round.clone(), a.block.id().to_string(), word.to_string()));
    }
    rows
}

pub fn render_text(crits: &[Criterion], answers: &[Answer]) -> String {
    let mut s = String::new();
    s.push_str("RS-01 results (what this tool can judge, plus your answers)\n");
    s.push_str("-----------------------------------------------------------\n");
    for c in crits {
        s.push_str(&format!(
            "[{:>7}] {}\n          {}\n",
            c.status.word(),
            c.text,
            c.detail
        ));
    }
    let table = globe_table(answers);
    if !table.is_empty() {
        s.push_str("\nGlobe action by setting (your answers):\n");
        for (round, block, word) in table {
            s.push_str(&format!("  {round:<28} {block:<11} {word}\n"));
        }
    }
    s.push_str(
        "\nNot covered here: macOS 14 and 15, Finder, fn+F1..F12 in full, the F5 dictation key,\n\
         Ctrl+Opt on an external keyboard, sleep/wake, and listening to the clipping takes.\n",
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::JudgedEpisode;
    use crate::grammar::Effect;
    use crate::verdict::{ChordKey, Episode, MicSpan, Press, Verdict};

    fn run(kind: BlockKind, eps: Vec<(Outcome, Episode)>, chord_keys: u32) -> BlockRun {
        BlockRun {
            kind,
            round: "Show Emoji & Symbols".into(),
            target: 10,
            started_ms: 0.0,
            ended_ms: 1.0,
            episodes: eps
                .into_iter()
                .map(|(o, episode)| JudgedEpisode {
                    episode,
                    verdict: Verdict {
                        outcome: o,
                        note: "n".into(),
                    },
                })
                .collect(),
            chord_keys,
            fork_events: 0,
            handy_events: 0,
        }
    }

    fn hold_ep(first_audio: f64) -> Episode {
        Episode {
            presses: vec![Press {
                down_ms: 0,
                up_ms: Some(1000),
            }],
            mic: vec![MicSpan {
                first_audio_ms: Some(first_audio),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn find<'a>(c: &'a [Criterion], id: &str) -> &'a Criterion {
        c.iter().find(|c| c.id == id).unwrap()
    }

    #[test]
    fn outcome_criteria() {
        let runs = vec![
            run(
                BlockKind::Hold,
                vec![
                    (Outcome::Pass, hold_ep(200.0)),
                    (Outcome::Invalid, hold_ep(0.0)),
                ],
                0,
            ),
            run(
                BlockKind::QuickTap,
                vec![(Outcome::Fail, Episode::default())],
                0,
            ),
        ];
        let c = criteria(&runs, &[], &[]);
        assert_eq!(find(&c, "hold_starts").status, Status::Pass);
        assert_eq!(find(&c, "quick_tap_no_mic").status, Status::Fail);
        assert_eq!(find(&c, "double_tap_locks").status, Status::NotRun);
        assert_eq!(find(&c, "capture_latency").status, Status::Pass);
        assert_eq!(find(&c, "taps_alive").status, Status::NotRun);
    }

    #[test]
    fn slow_capture_fails_latency() {
        let runs = vec![run(
            BlockKind::Hold,
            (0..10)
                .map(|i| (Outcome::Pass, hold_ep(240.0 + i as f64 * 20.0)))
                .collect(),
            0,
        )];
        let c = criteria(&runs, &[], &[]);
        assert_eq!(find(&c, "capture_latency").status, Status::Fail);
    }

    #[test]
    fn chord_mic_open_is_reported_with_latency() {
        let slow = Episode {
            chord_keys: vec![ChordKey {
                key: "End".into(),
                after_fn_ms: 230,
            }],
            effects: vec![
                (150, Effect::MicOpen),
                (150, Effect::TakeStarted),
                (
                    230,
                    Effect::ChordIgnored {
                        key: "End".into(),
                        mic_was_open: true,
                    },
                ),
                (230, Effect::MicClose),
            ],
            ..Default::default()
        };
        let runs = vec![run(BlockKind::Chords, vec![(Outcome::Fail, slow)], 30)];
        let c = criteria(&runs, &[], &[]);
        assert_eq!(find(&c, "chords_no_start").status, Status::Pass);
        let mic = find(&c, "chords_no_mic");
        assert_eq!(mic.status, Status::Fail);
        assert!(mic.detail.contains("End after 230 ms"));
    }

    #[test]
    fn answers_drive_owner_criteria() {
        let a = |block, id: &str, expected, answer| Answer {
            round: "r".into(),
            block,
            id: id.into(),
            question: "q".into(),
            expected: Some(expected),
            answer,
        };
        let answers = vec![
            a(BlockKind::Hold, "popup", Ans::No, Ans::No),
            a(BlockKind::DoubleTap, "popup", Ans::No, Ans::Yes),
            a(BlockKind::QuickTap, "dot", Ans::No, Ans::Unsure),
            a(BlockKind::Chords, "arrows", Ans::Yes, Ans::Yes),
        ];
        let c = criteria(&[], &answers, &[]);
        assert_eq!(find(&c, "no_globe_action").status, Status::Fail);
        assert_eq!(find(&c, "no_indicator_taps_chords").status, Status::NotRun);
        assert_eq!(find(&c, "chords_native").status, Status::Pass);
        let text = render_text(&c, &answers);
        assert!(text.contains("POPPED UP"));
    }

    #[test]
    fn health() {
        let tap = |id, enabled| TapInfo {
            id,
            enabled,
            options: 0,
            events_of_interest: 0,
            avg_usec_latency: 0.0,
            max_usec_latency: 0.0,
        };
        let ok = HealthCheck {
            label: "start".into(),
            taps: vec![
                ("handy".into(), tap(1, true)),
                ("fork".into(), tap(2, true)),
            ],
            secure_input: false,
        };
        let off = HealthCheck {
            label: "after chords".into(),
            taps: vec![
                ("handy".into(), tap(1, true)),
                ("fork".into(), tap(2, false)),
            ],
            secure_input: true,
        };
        assert!(ok.ok());
        assert!(!off.ok());
        let c = criteria(&[], &[], &[ok, off]);
        assert_eq!(find(&c, "taps_alive").status, Status::Fail);
        assert!(find(&c, "taps_alive").detail.contains("after chords"));
        assert_eq!(find(&c, "secure_input").status, Status::Info);
    }

    #[test]
    fn percentiles() {
        assert_eq!(percentile(&[], 50.0), None);
        assert_eq!(percentile(&[1.0], 95.0), Some(1.0));
        let v: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(percentile(&v, 50.0), Some(51.0));
        assert_eq!(percentile(&v, 95.0), Some(95.0));
    }
}
