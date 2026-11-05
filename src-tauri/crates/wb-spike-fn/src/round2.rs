// SPDX-License-Identifier: GPL-3.0-or-later
//! RS-01 round 2 (`wb-spike-fn --round 2`, about 5-8 minutes): tests the
//! candidate fixes for what round 1 found (docs/fork/spikes/RS-01-fn-globe.md).
//!
//! - **Part A, fn not blocked.** With "Press 🌐 key to" at Do Nothing, the
//!   fork's tap runs *without* `{FN}` in its blocking set (fn+Space and Esc
//!   are still blocked as usual): quick taps, double-taps and holds (did
//!   anything pop up?), then fn+F11 and fn+←/→ (did they do their usual
//!   thing?), recording fn-down → chord-key times per press.
//! - **Part B, first words.** `T_show` = 100 ms for the whole round. Ten takes
//!   with a cold mic (variant a), then ten with the prepared mic (variant b,
//!   `mic.rs`): the input unit is built and initialised before the takes and
//!   only started/stopped per take; the owner says whether the orange dot
//!   shows while it is merely prepared. Each take is judged from the data
//!   (`clipcheck.rs`): fn-down, first captured sample, speech onset, and the
//!   RS-09 bench transcript.
//!
//! Results: `~/wb-bench/results/rs01/<timestamp>-r2/`, same files as round 1.

use serde_json::{json, Value};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use crate::clipcheck::{self, TakeInput};
use crate::engine::{Msg, UiMsg};
use crate::globe::{self, FnUsage};
use crate::grammar::Timings;
use crate::mic::MicCmd;
use crate::session::{q, BlockText, GlobeRound, Session, POPUP};
use crate::summary::{
    self, answers_criterion, crit, outcome_criterion, percentile, Ans, Answer, Criterion, Status,
};
use crate::system::{self, Pane};
use crate::verdict::{BlockKind, Outcome};

/// Round 2 runs the grammar with `T_show` = 100 ms (R5 §7 RS-01 fallback).
pub const TIMINGS: Timings = Timings::with_show_ms(100);

pub const PART_A: &str = "fn not blocked";
pub const VAR_A: &str = "a: cold mic";
pub const VAR_B: &str = "b: prepared mic";

const RULE: &str = "------------------------------------------------------------------";

const CUE: &str = "hold fn and IMMEDIATELY say \"testing one two three\"; let go after \"three\".";

pub fn intro() {
    println!(
        "\n{RULE}\n  Wispr Bro: fn / 🌐 key test, round 2 (RS-01)\n{RULE}\n\
         Round 1 found two things to fix. This round tries the fixes. It takes\n\
         about 5-8 minutes. Nothing leaves this Mac.\n\n\
         Part A: the fn key with Wispr Bro NOT holding it back (your 🌐 setting\n\
         must be \"Do Nothing\" for this; I check it and tell you if not).\n\
         Part B: 20 short takes of you saying \"testing one two three\", to see\n\
         whether the first word gets cut off. The takes are saved on this Mac\n\
         (4 seconds each) and transcribed on this Mac afterwards.\n\
         You can stop at any time with Control+C; everything so far is kept.\n"
    );
}

/// A file-name-safe form of a round label ("a: cold mic" → "a-cold-mic").
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

fn part_a_texts() -> Vec<BlockText> {
    use Ans::*;
    vec![
        BlockText {
            kind: BlockKind::QuickTap,
            cue: None,
            title: "Quick taps (5)",
            target: 5,
            auto_end: true,
            steps: "Tap fn very quickly: press and let go at once, like tapping a key.\n\
                    Do it 5 times, about one second apart. Watch the whole screen.",
            questions: vec![
                q("popup", POPUP, No),
                q(
                    "dot",
                    "Did the orange dot appear at any point during the quick taps?",
                    No,
                ),
            ],
        },
        BlockText {
            kind: BlockKind::DoubleTap,
            cue: None,
            title: "Double-taps (5)",
            target: 5,
            auto_end: true,
            steps: "Press fn twice quickly (like a double-click). The orange dot appears.\n\
                    Wait about one second, then press fn once to stop. Do that 5 times.",
            questions: vec![q("popup", POPUP, No)],
        },
        BlockText {
            kind: BlockKind::Hold,
            cue: None,
            title: "Holds (5)",
            target: 5,
            auto_end: true,
            steps: "Hold fn for about one second, then let go. Do it 5 times.",
            questions: vec![q("popup", POPUP, No)],
        },
        BlockText {
            kind: BlockKind::Chords,
            cue: None,
            title: "fn shortcuts in TextEdit (10)",
            target: 10,
            auto_end: true,
            steps: "Click into the TextEdit window, in the middle of the text. Then:\n\
                    \n  fn + F11   5 times\n  \
                    fn + ←     then fn + →, alternating, 5 times in all\n\
                    \nIMPORTANT: one shortcut at a time. Press fn, press the other key, and\n\
                    LET GO OF BOTH before the next one. Don't keep fn held between them.\n\
                    It moves on by itself after 10.",
            questions: vec![
                q(
                    "f11_usual",
                    "Did fn+F11 do what F11 / fn+F11 normally does on your Mac?\n    \
                     (If it normally does nothing, answer y if nothing happened.)",
                    Yes,
                ),
                q(
                    "arrows",
                    "Did fn+← and fn+→ jump to the top and bottom of the document?",
                    Yes,
                ),
                q(
                    "dot",
                    "Did the orange dot appear at ANY point during these shortcuts?",
                    No,
                ),
                q("popup", POPUP, No),
            ],
        },
    ]
}

fn clip_text(prepared: bool) -> BlockText {
    BlockText {
        kind: BlockKind::Clipping,
        cue: Some(CUE),
        title: if prepared {
            "First words, variant b: prepared mic (10 takes)"
        } else {
            "First words, variant a: cold mic (10 takes)"
        },
        target: 10,
        auto_end: true,
        steps: "Each take: wait for the line that says >>> NOW, then press fn and start\n\
                saying \"testing one two three\" AT ONCE, in your normal voice. Don't wait\n\
                for the orange dot. Keep holding fn until you've said \"three\", then let go.\n\
                The next cue comes by itself. You can stay in this window.",
        questions: if prepared {
            vec![
                q("dot", "Did the orange dot appear for each take?", Ans::Yes),
                q(
                    "dot_gone",
                    "Did the dot go away within a second or two after each take?",
                    Ans::Yes,
                ),
            ]
        } else {
            vec![q(
                "dot",
                "Did the orange dot appear for each take?",
                Ans::Yes,
            )]
        },
    }
}

impl Session {
    pub(crate) fn guided_round2(&mut self, rx: Receiver<Msg>, ui_tx: Sender<UiMsg>) -> Option<()> {
        self.permissions_step()?;
        self.globe_do_nothing()?;

        // The F11 question, once, before anything is pressed.
        println!("\n{RULE}\nOne question before we start\n{RULE}");
        let f11 = self.ask(
            "setup",
            BlockKind::Chords,
            &crate::session::Q {
                id: "f11_normally",
                text:
                    "Does F11 or fn+F11 normally do anything on your Mac (e.g. show the desktop)?",
                expected: None,
            },
        )?;
        if f11 == Ans::Yes {
            let what = ["Shows the desktop", "Changes the volume", "Something else"];
            let i = self.ui.choice("What does it do?", &what)?;
            self.extra.insert("f11_does".into(), json!(what[i]));
        }

        self.start_listeners(rx, ui_tx)?;
        self.prove_fn()?;
        self.open_textedit()?;

        // Part A.
        println!(
            "\n{RULE}\nPart A: fn not held back\n{RULE}\n\
             From now on Wispr Bro does NOT hold back the fn key itself (it still\n\
             holds back fn+Space and Esc). Watch for anything popping up."
        );
        let texts = part_a_texts();
        let total = texts.len();
        for (i, b) in texts.iter().enumerate() {
            self.block(
                PART_A,
                b,
                &format!("Part A, test {} of {total}", i + 1),
                true,
            )?;
        }

        // Part B.
        println!("\n{RULE}\nPart B: first words (20 takes, two ways of starting the mic)\n{RULE}");
        self.block(VAR_A, &clip_text(false), "Part B, 1 of 2", true)?;
        self.prepared_variant()?;

        // Objective clipping analysis + transcripts.
        println!("\nChecking the 20 takes (speech start and a transcript of each)...");
        self.analyse_takes();
        self.save();

        self.restore_globe_reminder()?;
        self.health("end");
        self.finish();
        Some(())
    }

    fn globe_do_nothing(&mut self) -> Option<()> {
        let start = globe::read();
        self.log
            .write("globe_start", Instant::now(), json!({"read": start}));
        self.extra.insert("globe_at_start".into(), json!(start));
        self.original_globe = start.usage();
        println!("\n{RULE}\nYour \"Press 🌐 key to\" setting\n{RULE}");
        println!("macOS reports: {}", start.describe());
        let mut read = start.clone();
        let mut confirm = None;
        if start.usage() != Some(FnUsage::DoNothing) {
            println!(
                "For this round it must be \"Do Nothing\". Open System Settings → Keyboard and\n\
                 set \"Press 🌐 key to\" (or \"Press fn key to\") to \"Do Nothing\".\n\
                 I'll remind you to set it back at the end."
            );
            if self
                .ui
                .yes_no("Shall I open the Keyboard page of System Settings for you?")?
                == Ans::Yes
            {
                system::open_pane(Pane::Keyboard);
            }
            loop {
                self.ui.enter("Press Enter once it says \"Do Nothing\".")?;
                std::thread::sleep(Duration::from_millis(500));
                read = globe::read();
                if read.usage() == Some(FnUsage::DoNothing) {
                    println!("  macOS now reports \"Do Nothing\". Good.");
                    break;
                }
                println!("  macOS still reports {}.", read.describe());
                let a = self
                    .ui
                    .yes_no("Does System Settings show \"Do Nothing\" right now?")?;
                if a == Ans::Yes {
                    println!(
                        "  OK, carrying on (the mismatch is noted in the results; it happened\n  \
                         once in round 1 too)."
                    );
                    confirm = Some(a);
                    break;
                }
                println!("  Please pick \"Do Nothing\" in that menu, then press Enter.");
            }
        } else {
            println!("  That's what this round needs. Good.");
        }
        self.globe_rounds.push(GlobeRound {
            round: PART_A.into(),
            expected: Some(FnUsage::DoNothing),
            read,
            owner_confirms: confirm,
            owner_says: None,
        });
        Some(())
    }

    fn restore_globe_reminder(&mut self) -> Option<()> {
        let back_to = match self.original_globe {
            Some(FnUsage::DoNothing) => return Some(()),
            Some(u) => u.label(),
            // Never changed before: the Mac's default (Show Emoji & Symbols
            // in round 1 on this Mac).
            None => FnUsage::ShowEmoji.label(),
        };
        println!(
            "\nPlease set \"Press 🌐 key to\" back to \"{back_to}\" (System Settings → Keyboard),\n\
             the way it was before this round."
        );
        self.ui.enter("Press Enter when done.")?;
        let back = globe::read();
        self.log
            .write("globe_restored", Instant::now(), json!({"read": back}));
        Some(())
    }

    fn prepared_variant(&mut self) -> Option<()> {
        let Some(mic) = self.mic_tx.clone() else {
            return Some(());
        };
        println!("\nPreparing the microphone for variant b (not switching it on)...");
        let (reply_tx, reply_rx) = mpsc::channel();
        let _ = mic.send(MicCmd::Prepare { reply: reply_tx });
        match reply_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok((device, ms))) => {
                println!("  Prepared: {device} ({ms:.0} ms).");
                self.extra
                    .insert("prepared".into(), json!({"device": device, "build_ms": ms}));
            }
            Ok(Err(e)) => {
                println!("  !! Could not prepare the microphone: {e}\n  Skipping variant b.");
                self.extra.insert("prepared_error".into(), json!(e));
                return Some(());
            }
            Err(_) => {
                println!("  !! The microphone did not answer. Skipping variant b.");
                self.extra
                    .insert("prepared_error".into(), json!("no reply within 10 s"));
                return Some(());
            }
        }
        std::thread::sleep(Duration::from_secs(2));
        let idle = crate::session::Q {
            id: "dot_idle",
            text: "Look at the top right of the screen. You're not pressing anything.\n    \
                   Is the orange microphone dot showing right now?",
            expected: Some(Ans::No),
        };
        self.ask(VAR_B, BlockKind::Clipping, &idle)?;
        self.block(VAR_B, &clip_text(true), "Part B, 2 of 2", true)?;
        std::thread::sleep(Duration::from_secs(2));
        let idle_after = crate::session::Q {
            id: "dot_idle_after",
            text: "And now, a few seconds after the last take: is the orange dot showing?",
            expected: Some(Ans::No),
        };
        self.ask(VAR_B, BlockKind::Clipping, &idle_after)?;
        let _ = mic.send(MicCmd::Unprepare);
        Some(())
    }

    fn take_inputs(&self) -> Vec<TakeInput> {
        self.runs
            .iter()
            .filter(|r| r.kind == BlockKind::Clipping)
            .flat_map(|r| {
                r.episodes
                    .iter()
                    .filter(|e| e.verdict.outcome != Outcome::Invalid)
                    .filter_map(move |e| {
                        let m = e.episode.mic.first()?;
                        Some(TakeInput {
                            variant: r.round.clone(),
                            n: e.episode.n,
                            wav: m.wav.as_ref().map(Into::into),
                            fn_down_ms: e.episode.presses.first()?.down_ms as f64,
                            first_audio_ms: m.first_audio_ms,
                        })
                    })
            })
            .collect()
    }

    fn analyse_takes(&mut self) {
        let takes = self.take_inputs();
        let (results, note) = clipcheck::analyse(&takes, &self.dir.join("analysis"));
        print!("{}", clipcheck::render_takes(&results));
        if let Some(n) = &note {
            println!("  Transcripts skipped: {n}");
        }
        self.extra
            .insert("take_stats".into(), json!(clipcheck::stats(&results)));
        self.extra.insert("takes".into(), json!(results));
        self.extra.insert("asr_note".into(), json!(note));
    }

    /// fn-down → first chord key, once per press, from Part A's chord block.
    fn chord_times(&self) -> Vec<u64> {
        let mut v: Vec<u64> = self
            .runs
            .iter()
            .filter(|r| r.kind == BlockKind::Chords)
            .flat_map(|r| r.episodes.iter())
            .flat_map(|e| e.episode.first_chord_key_per_press())
            .collect();
        v.sort_unstable();
        v
    }

    pub(crate) fn criteria_round2(&self) -> Vec<Criterion> {
        let mut out = Vec::new();
        let show = TIMINGS.show_ms;

        // Globe at Do Nothing.
        let g = self.globe_rounds.iter().find(|g| g.round == PART_A);
        out.push(match g {
            Some(g) if g.read.usage() == Some(FnUsage::DoNothing) => crit(
                "globe_do_nothing",
                "\"Press 🌐 key to\" was \"Do Nothing\" for Part A",
                Status::Pass,
                "macOS reported value 0".into(),
            ),
            Some(g) => crit(
                "globe_do_nothing",
                "\"Press 🌐 key to\" was \"Do Nothing\" for Part A",
                Status::Info,
                format!(
                    "macOS reported {}, the owner {} it showed Do Nothing",
                    g.read.describe(),
                    if g.owner_confirms == Some(Ans::Yes) {
                        "confirmed"
                    } else {
                        "did not confirm"
                    }
                ),
            ),
            None => crit(
                "globe_do_nothing",
                "\"Press 🌐 key to\" was \"Do Nothing\" for Part A",
                Status::NotRun,
                String::new(),
            ),
        });

        // Part A: grammar outcomes with T_show = 100 ms and fn not blocked.
        out.push(outcome_criterion(
            &self.runs,
            BlockKind::QuickTap,
            "quick_tap_no_mic",
            "The mic never opens on a quick tap (< 100 ms, T_show = 100 ms)",
        ));
        out.push(outcome_criterion(
            &self.runs,
            BlockKind::DoubleTap,
            "double_tap_locks",
            "Every double-tap locks hands-free (fn not blocked)",
        ));
        out.push(outcome_criterion(
            &self.runs,
            BlockKind::Hold,
            "hold_starts",
            "Every 1 s hold starts a take (at 100 ms) and stops on release (fn not blocked)",
        ));
        let part_a = |a: &Answer| a.round == PART_A;
        out.push(answers_criterion(
            &self.answers,
            "no_globe_action_unblocked",
            "Do Nothing + fn not blocked: nothing pops up on tap, double-tap, hold or chords",
            |a| part_a(a) && a.id == "popup",
        ));
        out.push(answers_criterion(
            &self.answers,
            "chords_native_unblocked",
            "fn not blocked: fn+F11 and fn+←/→ keep their usual function",
            |a| part_a(a) && matches!(a.id.as_str(), "f11_usual" | "arrows"),
        ));
        out.push(answers_criterion(
            &self.answers,
            "no_indicator_taps_chords",
            "Orange dot never shown during quick taps or chords (T_show = 100 ms)",
            |a| part_a(a) && a.id == "dot",
        ));
        let f11 = self.answers.iter().find(|a| a.id == "f11_normally");
        out.push(crit(
            "f11_normally",
            "Does F11 / fn+F11 normally do anything on this Mac? (owner)",
            if f11.is_some() {
                Status::Info
            } else {
                Status::NotRun
            },
            match (f11.map(|a| a.answer), self.extra.get("f11_does")) {
                (Some(a), Some(Value::String(w))) => format!("{a:?}: {w}"),
                (Some(a), _) => format!("{a:?}"),
                (None, _) => String::new(),
            },
        ));

        // Chords: starts and the timing distribution.
        let times = self.chord_times();
        let mut opens = Vec::new();
        for r in self.runs.iter().filter(|r| r.kind == BlockKind::Chords) {
            for e in &r.episodes {
                if e.episode.mic_opened() {
                    opens.push(
                        e.episode
                            .chord_keys
                            .first()
                            .map(|k| format!("{} after {} ms", k.key, k.after_fn_ms))
                            .unwrap_or_else(|| "fn alone".into()),
                    );
                }
            }
        }
        out.push(crit(
            "chords_no_mic",
            "The mic never opens during fn chords (T_show = 100 ms)",
            if times.is_empty() {
                Status::NotRun
            } else if opens.is_empty() {
                Status::Pass
            } else {
                Status::Fail
            },
            format!(
                "{} mic open(s){}",
                opens.len(),
                if opens.is_empty() {
                    String::new()
                } else {
                    format!(": {}", opens.join(", "))
                }
            ),
        ));
        out.push(crit(
            "chord_timing",
            "fn press → chord key, first key of each press",
            if times.is_empty() {
                Status::NotRun
            } else {
                Status::Info
            },
            chord_timing_text(&times, show),
        ));

        // Part B per variant.
        let stats: Vec<clipcheck::VariantStats> = self
            .extra
            .get("take_stats")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        for v in [VAR_A, VAR_B] {
            let id = slug(v);
            let Some(s) = stats.iter().find(|s| s.variant == v) else {
                out.push(crit(
                    &format!("clipping_{id}"),
                    &format!("{v}: first words"),
                    Status::NotRun,
                    "no takes".into(),
                ));
                continue;
            };
            out.push(match (s.latency_p50_ms, s.latency_p95_ms) {
                (Some(p50), Some(p95)) => crit(
                    &format!("capture_latency_{id}"),
                    &format!("{v}: fn press → first audio (§5.1: p50 ≤ 250 ms, p95 ≤ 350 ms)"),
                    if p50 <= 250.0 && p95 <= 350.0 {
                        Status::Pass
                    } else {
                        Status::Fail
                    },
                    format!("p50 {p50:.0} ms, p95 {p95:.0} ms over {} take(s)", s.takes),
                ),
                _ => crit(
                    &format!("capture_latency_{id}"),
                    &format!("{v}: fn press → first audio"),
                    Status::NotRun,
                    "no audio".into(),
                ),
            });
            let judged = s.clipped_by_us + s.not_clipped;
            out.push(crit(
                &format!("clipped_by_us_{id}"),
                &format!("{v}: no take clipped by us (speech already on when the mic went live)"),
                if judged == 0 {
                    Status::NotRun
                } else if s.clipped_by_us == 0 {
                    Status::Pass
                } else {
                    Status::Fail
                },
                format!(
                    "clipped by us {}, not clipped {}, no speech {}, no audio {}; speech began p50 {} ms (earliest {} ms) after the press",
                    s.clipped_by_us,
                    s.not_clipped,
                    s.no_speech,
                    s.no_audio,
                    s.speech_after_fn_p50_ms.map(|x| format!("{x:.0}")).unwrap_or("-".into()),
                    s.speech_after_fn_min_ms.map(|x| format!("{x:.0}")).unwrap_or("-".into()),
                ),
            ));
            out.push(crit(
                &format!("testing_kept_{id}"),
                &format!("{v}: \"testing\" in the transcript (parakeet-unified-en)"),
                if s.transcribed == 0 {
                    Status::NotRun
                } else {
                    Status::Info
                },
                format!(
                    "{} of {} transcribed take(s)",
                    s.testing_survived, s.transcribed
                ),
            ));
        }
        out.push(answers_criterion(
            &self.answers,
            "prepared_no_idle_dot",
            "Prepared mic: no orange dot while it is only prepared",
            |a| a.round == VAR_B && matches!(a.id.as_str(), "dot_idle" | "dot_idle_after"),
        ));
        out.push(answers_criterion(
            &self.answers,
            "indicator_when_recording",
            "Orange dot shown during takes and gone after",
            |a| {
                matches!(a.round.as_str(), VAR_A | VAR_B)
                    && matches!(a.id.as_str(), "dot" | "dot_gone")
            },
        ));
        if let Some(Value::String(e)) = self.extra.get("prepared_error") {
            out.push(crit(
                "prepared_error",
                "Prepared mic could not be set up",
                Status::Fail,
                e.clone(),
            ));
        }

        // Taps alive.
        let bad: Vec<&str> = self
            .health
            .iter()
            .filter(|h| !h.ok())
            .map(|h| h.label.as_str())
            .collect();
        out.push(crit(
            "taps_alive",
            "Both taps (Handy's and the fork's) present and enabled at every check",
            if self.health.is_empty() {
                Status::NotRun
            } else if bad.is_empty() {
                Status::Pass
            } else {
                Status::Fail
            },
            if bad.is_empty() {
                format!("{} check(s)", self.health.len())
            } else {
                format!("failed at: {}", bad.join(", "))
            },
        ));
        out
    }

    pub(crate) fn summary_json_round2(&self) -> Value {
        json!({
            "tool": "wb-spike-fn",
            "spike": "RS-01",
            "round": 2,
            "tool_version": env!("CARGO_PKG_VERSION"),
            "git": option_env!("WB_GIT_SHA"),
            "started": self.started_local,
            "machine": self.machine,
            "terminal": self.term,
            "permissions": self.permissions,
            "mic_device": self.mic_device,
            "timings": self.timings,
            "block_fn": self.block_fn,
            "handy_bindings": self.taps.as_ref().map(|t| t.handy_registered.iter().map(|(b, r)| json!({"binding": b, "ok": r.is_ok()})).collect::<Vec<_>>()),
            "tap_ids": self.taps.as_ref().map(|t| json!({"handy": t.handy_tap_ids, "fork": t.fork_tap_ids})),
            "globe": {"original": self.original_globe, "rounds": self.globe_rounds},
            "criteria": self.criteria_round2(),
            "chord_first_key_ms": self.chord_times(),
            "answers": self.answers,
            "health": self.health,
            "blocks": self.runs,
            "round2": self.extra,
            "options": {"out": self.opts.out},
        })
    }

    pub(crate) fn summary_text_round2(&self) -> String {
        let mut s = summary::render_criteria(
            "RS-01 round 2 results (what this tool can judge, plus your answers)",
            &self.criteria_round2(),
        );
        if let Some(takes) = self.extra.get("takes") {
            if let Ok(rs) = serde_json::from_value::<Vec<clipcheck::TakeResult>>(takes.clone()) {
                s.push_str("\nFirst-word takes (clipped by us = speech already on in the first captured frames):\n");
                s.push_str(&clipcheck::render_takes(&rs));
            }
        }
        if let Some(Value::String(n)) = self.extra.get("asr_note") {
            s.push_str(&format!("\nTranscripts skipped: {n}\n"));
        }
        s.push_str(
            "\nNot covered here: macOS 14 and 15, Finder, an external keyboard, the F5 dictation\n\
             key, sleep/wake. 10 takes per variant (RS-01 asks for 50).\n",
        );
        s
    }
}

/// "n presses: min / p25 / p50 / p75 / max; k under T_show, …".
pub fn chord_timing_text(sorted: &[u64], show_ms: u64) -> String {
    if sorted.is_empty() {
        return "no chord presses".into();
    }
    let f: Vec<f64> = sorted.iter().map(|&x| x as f64).collect();
    let p = |q| percentile(&f, q).unwrap_or(0.0);
    let under = |t: u64| sorted.iter().filter(|&&x| x < t).count();
    format!(
        "{} press(es): min {} / p25 {:.0} / p50 {:.0} / p75 {:.0} / max {} ms; {} under {show_ms} ms, {} under 150 ms, {} under 500 ms",
        sorted.len(),
        sorted[0],
        p(25.0),
        p(50.0),
        p(75.0),
        sorted[sorted.len() - 1],
        under(show_ms),
        under(150),
        under(500),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        let (_ui_tx, ui_rx) = mpsc::channel();
        let (tx, _rx) = mpsc::channel();
        Session {
            opts: crate::Options {
                out: None,
                round: 2,
            },
            dir: std::env::temp_dir(),
            log: std::sync::Arc::new(crate::log::Log::null(Instant::now())),
            ui: crate::session::Ui {
                rx: ui_rx,
                eof: std::cell::Cell::new(true),
            },
            tx,
            taps: None,
            mic_tx: None,
            term: "Terminal".into(),
            started_local: String::new(),
            machine: crate::system::MachineInfo {
                macos_version: "26.6.2".into(),
                macos_build: String::new(),
                model: String::new(),
                cpu: String::new(),
                term_program: None,
            },
            permissions: crate::session::Permissions {
                accessibility: true,
                input_monitoring: true,
                microphone: "Granted".into(),
                secure_input: false,
            },
            mic_device: None,
            runs: Vec::new(),
            answers: Vec::new(),
            health: Vec::new(),
            globe_rounds: Vec::new(),
            original_globe: None,
            builtin_keyboard: None,
            timings: TIMINGS,
            block_fn: false,
            extra: serde_json::Map::new(),
        }
    }

    fn answer(round: &str, id: &str, expected: Option<Ans>, answer: Ans) -> Answer {
        Answer {
            round: round.into(),
            block: BlockKind::Clipping,
            id: id.into(),
            question: id.into(),
            expected,
            answer,
        }
    }

    #[test]
    fn summary_of_an_empty_run_does_not_panic() {
        let s = session();
        let crits = s.criteria_round2();
        assert!(crits.iter().any(|c| c.id == "clipping_a-cold-mic"));
        assert!(s.summary_text_round2().contains("RS-01 round 2 results"));
        assert_eq!(s.summary_json_round2()["round"], 2);
    }

    #[test]
    fn summary_judges_variants_and_the_idle_dot() {
        let mut s = session();
        s.globe_rounds.push(GlobeRound {
            round: PART_A.into(),
            expected: Some(FnUsage::DoNothing),
            read: globe::GlobeSetting::Value {
                raw: 0,
                usage: FnUsage::DoNothing,
            },
            owner_confirms: None,
            owner_says: None,
        });
        s.answers
            .push(answer(VAR_B, "dot_idle", Some(Ans::No), Ans::Yes));
        s.answers
            .push(answer(PART_A, "popup", Some(Ans::No), Ans::No));
        let st = |v: &str, p50, clipped| clipcheck::VariantStats {
            variant: v.into(),
            takes: 10,
            clipped_by_us: clipped,
            not_clipped: 10 - clipped,
            latency_p50_ms: Some(p50),
            latency_p95_ms: Some(p50 + 30.0),
            ..Default::default()
        };
        s.extra.insert(
            "take_stats".into(),
            json!([st(VAR_A, 212.0, 1), st(VAR_B, 120.0, 0)]),
        );
        let c = s.criteria_round2();
        let find = |id: &str| c.iter().find(|x| x.id == id).unwrap().status;
        assert_eq!(find("globe_do_nothing"), Status::Pass);
        assert_eq!(find("capture_latency_a-cold-mic"), Status::Pass);
        assert_eq!(find("clipped_by_us_a-cold-mic"), Status::Fail);
        assert_eq!(find("clipped_by_us_b-prepared-mic"), Status::Pass);
        assert_eq!(find("prepared_no_idle_dot"), Status::Fail);
        assert_eq!(find("no_globe_action_unblocked"), Status::Pass);
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("a: cold mic"), "a-cold-mic");
        assert_eq!(slug("b: prepared mic"), "b-prepared-mic");
        assert_eq!(slug("Show Emoji & Symbols"), "show-emoji-symbols");
        assert_eq!(slug("--x--"), "x");
    }

    #[test]
    fn round_two_runs_show_100_and_the_default_tap_window() {
        assert_eq!(TIMINGS.show_ms, 100);
        assert_eq!(TIMINGS.tap_window_ms, Timings::DEFAULT.tap_window_ms);
    }

    #[test]
    fn chord_timing_summary() {
        let t = chord_timing_text(&[80, 120, 160, 300, 1256], 100);
        assert!(
            t.starts_with("5 press(es): min 80 / p25 120 / p50 160 / p75 300 / max 1256 ms"),
            "{t}"
        );
        assert!(
            t.contains("1 under 100 ms, 2 under 150 ms, 4 under 500 ms"),
            "{t}"
        );
        assert_eq!(chord_timing_text(&[], 100), "no chord presses");
    }

    #[test]
    fn part_b_blocks_cue_every_take_and_part_a_does_not() {
        assert!(part_a_texts().iter().all(|b| b.cue.is_none()));
        assert!(clip_text(false).cue.is_some());
        let b = clip_text(true);
        assert_eq!(b.target, 10);
        assert!(b.questions.iter().any(|q| q.id == "dot_gone"));
    }
}
