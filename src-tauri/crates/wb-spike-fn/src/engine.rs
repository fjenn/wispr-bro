// SPDX-License-Identifier: GPL-3.0-or-later
//! The live loop: raw events from both taps → log → grammar → effects (mic,
//! blocking set, log) → episodes and live progress lines for the block the
//! owner is doing. One thread; it sleeps until the next event or the next
//! grammar deadline, whichever comes first.

use handy_keys::{BlockingHotkeys, Hotkey, Key as HkKey, KeyEvent, Modifiers};
use serde::Serialize;
use serde_json::json;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::grammar::{Blocking, Effect, Grammar, Input, Key, Ms};
use crate::log::Log;
use crate::mic::{MicCmd, MicReport};
use crate::taps::{idle_set, would_block, TapMsg};
use crate::verdict::{judge, BlockKind, ChordKey, Episode, MicSpan, Outcome, Press, Verdict};

pub enum Msg {
    Tap(TapMsg),
    Mic(MicReport),
    Begin(BlockSpec),
    End(Sender<BlockRun>),
    /// Send `UiMsg::FnSeen` at the next fn press (setup check).
    ArmFnNotice,
}

/// Sent to the session (main thread).
pub enum UiMsg {
    Line(String),
    Eof,
    BlockDone,
    FnSeen,
    /// An attempt in the active block finished (`valid` attempts counted so
    /// far, of `target`). Round 2 uses it to cue the next take.
    EpisodeDone {
        valid: u32,
        target: u32,
    },
}

#[derive(Clone, Debug)]
pub struct BlockSpec {
    pub kind: BlockKind,
    /// Which "Press 🌐 key to" round this block belongs to.
    pub round: String,
    pub target: u32,
    /// End the block by itself once `target` valid attempts are in.
    pub auto_end: bool,
    /// Clipping block: save each take's first seconds here.
    pub save_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, Serialize)]
pub struct JudgedEpisode {
    #[serde(flatten)]
    pub episode: Episode,
    pub verdict: Verdict,
}

#[derive(Clone, Debug, Serialize)]
pub struct BlockRun {
    pub kind: BlockKind,
    pub round: String,
    pub target: u32,
    pub started_ms: f64,
    pub ended_ms: f64,
    pub episodes: Vec<JudgedEpisode>,
    /// Non-modifier keys pressed while fn was held, during this block.
    pub chord_keys: u32,
    pub fork_events: u32,
    pub handy_events: u32,
}

impl BlockRun {
    pub fn count(&self, o: Outcome) -> usize {
        self.episodes
            .iter()
            .filter(|e| e.verdict.outcome == o)
            .count()
    }
}

struct Active {
    spec: BlockSpec,
    run: BlockRun,
    valid: u32,
    attempts: u32,
    done_sent: bool,
}

pub struct Engine {
    g: Grammar,
    log: Arc<Log>,
    blocking: BlockingHotkeys,
    applied: Blocking,
    mic: Sender<MicCmd>,
    ui: Sender<UiMsg>,
    take_seq: u64,
    current_take: Option<u64>,
    pending_reports: usize,
    pressed: HashSet<HkKey>,
    fn_down_at: Option<Ms>,
    episode: Option<Episode>,
    block: Option<Active>,
    quiet: bool,
    arm_fn: bool,
}

fn key_name(k: HkKey) -> String {
    match k {
        HkKey::Escape => "Escape".into(),
        HkKey::Space => "Space".into(),
        other => other.to_string(),
    }
}

/// Key name for the log. Ordinary typing (letters, digits, punctuation)
/// while fn is up is not recorded by name: the tap sees every keystroke on
/// the Mac, and the spike only needs to know a key was pressed.
fn logged_key_name(k: HkKey, fn_down: bool) -> String {
    let name = key_name(k);
    let printable =
        name.chars().count() == 1 || name.starts_with("Keypad") || name == "JisUnderscore";
    if fn_down || !printable {
        name
    } else {
        "(typing)".into()
    }
}

fn classify(k: HkKey) -> Key {
    match k {
        HkKey::Space => Key::Space,
        HkKey::Escape => Key::Escape,
        other => Key::Other(key_name(other)),
    }
}

impl Engine {
    pub fn new(
        log: Arc<Log>,
        blocking: BlockingHotkeys,
        mic: Sender<MicCmd>,
        ui: Sender<UiMsg>,
        grammar: Grammar,
    ) -> Engine {
        // The tap starts with `taps::idle_set(grammar.blocks_fn())`.
        let applied = grammar.blocking();
        Engine {
            g: grammar,
            log,
            blocking,
            applied,
            mic,
            ui,
            take_seq: 0,
            current_take: None,
            pending_reports: 0,
            pressed: HashSet::new(),
            fn_down_at: None,
            episode: None,
            block: None,
            quiet: false,
            arm_fn: false,
        }
    }

    fn ms(&self, at: Instant) -> Ms {
        at.saturating_duration_since(self.log.epoch()).as_millis() as Ms
    }

    fn at(&self, t: Ms) -> Instant {
        self.log.epoch() + Duration::from_millis(t)
    }

    pub fn run(mut self, rx: Receiver<Msg>) {
        loop {
            let msg = match self.g.next_deadline() {
                Some(d) => {
                    let due = self.at(d);
                    let now = Instant::now();
                    if due <= now {
                        self.fire(d);
                        continue;
                    }
                    match rx.recv_timeout(due - now) {
                        Ok(m) => m,
                        Err(RecvTimeoutError::Timeout) => {
                            self.fire(d);
                            continue;
                        }
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
                None => match rx.recv() {
                    Ok(m) => m,
                    Err(_) => break,
                },
            };
            match msg {
                Msg::End(reply) => {
                    let run = self.end_block(&rx);
                    let _ = reply.send(run);
                }
                other => self.handle(other),
            }
        }
        let now = self.ms(Instant::now());
        let effects = self.g.reset(now);
        self.apply(now, effects);
    }

    fn handle(&mut self, msg: Msg) {
        match msg {
            Msg::Tap(TapMsg::Fork { at, ev }) => self.on_fork(at, ev),
            Msg::Tap(TapMsg::Handy {
                at,
                binding,
                pressed,
            }) => {
                self.log.write(
                    "handy_event",
                    at,
                    json!({"binding": binding, "pressed": pressed}),
                );
                if let Some(b) = self.block.as_mut() {
                    b.run.handy_events += 1;
                }
            }
            Msg::Tap(TapMsg::TapEnded { which }) => {
                self.log
                    .write("tap_ended", Instant::now(), json!({"tap": which}));
                eprintln!(
                    "\n  !! The {which} keyboard tap stopped. Please tell Claude about this."
                );
            }
            Msg::Mic(report) => self.on_mic(report),
            Msg::Begin(spec) => {
                let now = Instant::now();
                self.log.write(
                    "block_begin",
                    now,
                    json!({"block": spec.kind.id(), "round": spec.round, "target": spec.target}),
                );
                self.block = Some(Active {
                    run: BlockRun {
                        kind: spec.kind,
                        round: spec.round.clone(),
                        target: spec.target,
                        started_ms: self.log.ms(now),
                        ended_ms: 0.0,
                        episodes: Vec::new(),
                        chord_keys: 0,
                        fork_events: 0,
                        handy_events: 0,
                    },
                    spec,
                    valid: 0,
                    attempts: 0,
                    done_sent: false,
                });
            }
            Msg::ArmFnNotice => self.arm_fn = true,
            Msg::End(reply) => {
                // Only reached from inside end_block's wait; answer empty.
                drop(reply);
            }
        }
    }

    fn on_fork(&mut self, at: Instant, ev: KeyEvent) {
        let t = self.ms(at);
        let likely_blocked = self
            .blocking
            .lock()
            .map(|s| would_block(&s, &ev))
            .unwrap_or(false);
        self.log.write(
            "fork_event",
            at,
            json!({
                "modifiers": ev.modifiers.to_string(),
                "key": ev.key.map(|k| logged_key_name(k, self.g.fn_is_down())),
                "down": ev.is_key_down,
                "changed_modifier": ev.changed_modifier.map(|m| m.to_string()),
                "likely_blocked": likely_blocked,
            }),
        );
        if let Some(b) = self.block.as_mut() {
            b.run.fork_events += 1;
        }

        // Translate. Modifier keys other than fn are not grammar inputs.
        let (input, new_key) = if ev.changed_modifier == Some(Modifiers::FN) {
            (
                Some(if ev.is_key_down {
                    Input::FnDown
                } else {
                    Input::FnUp
                }),
                None,
            )
        } else if let Some(k) = ev.key {
            if ev.is_key_down {
                let fresh = self.pressed.insert(k);
                (Some(Input::KeyDown(classify(k))), fresh.then_some(k))
            } else {
                self.pressed.remove(&k);
                (None, None)
            }
        } else {
            (None, None)
        };
        let Some(input) = input else { return };

        self.fire_due(t);
        let was_idle = self.g.is_idle();
        let fn_was_down = self.g.fn_is_down();
        let effects = self.g.step(t, &input);
        if was_idle && !self.g.is_idle() {
            self.episode = Some(Episode {
                start_ms: t,
                ..Default::default()
            });
        }
        match &input {
            Input::FnDown if !fn_was_down => {
                self.fn_down_at = Some(t);
                if std::mem::take(&mut self.arm_fn) {
                    let _ = self.ui.send(UiMsg::FnSeen);
                }
                if let Some(ep) = self.episode.as_mut() {
                    ep.presses.push(Press {
                        down_ms: t,
                        up_ms: None,
                    });
                }
            }
            Input::FnUp if fn_was_down => {
                if let Some(p) = self.episode.as_mut().and_then(|e| e.presses.last_mut()) {
                    p.up_ms = Some(t);
                }
            }
            _ => {}
        }
        if let (Some(k), true) = (new_key, fn_was_down) {
            let after = t.saturating_sub(self.fn_down_at.unwrap_or(t));
            if let Some(ep) = self.episode.as_mut() {
                let press = ep.presses.len().saturating_sub(1);
                ep.chord_keys.push(ChordKey {
                    key: key_name(k),
                    after_fn_ms: after,
                    press,
                });
            }
            if let Some(b) = self.block.as_mut() {
                b.run.chord_keys += 1;
                if b.spec.kind == BlockKind::Chords && !self.quiet {
                    println!(
                        "    chord {}: fn+{} ({after} ms after fn went down)",
                        b.run.chord_keys,
                        key_name(k)
                    );
                }
            }
        }
        self.apply(t, effects);
        self.after_change(t);
    }

    fn fire_due(&mut self, t: Ms) {
        while let Some(d) = self.g.next_deadline() {
            if d > t {
                break;
            }
            self.fire(d);
        }
    }

    fn fire(&mut self, d: Ms) {
        let effects = self.g.advance(d);
        self.apply(d, effects);
        self.after_change(d);
    }

    fn apply(&mut self, t: Ms, effects: Vec<Effect>) {
        for e in effects {
            self.log.write(
                "decision",
                self.at(t),
                json!({"effect": &e, "state": self.g.state()}),
            );
            match &e {
                Effect::MicOpen => {
                    self.take_seq += 1;
                    let take = self.take_seq;
                    self.current_take = Some(take);
                    let save = self.block.as_ref().and_then(|b| {
                        b.spec.save_dir.as_ref().map(|dir| {
                            dir.join(format!("take-{:02}-{}.wav", b.run.episodes.len() + 1, take))
                        })
                    });
                    let requested = Instant::now();
                    if let Some(ep) = self.episode.as_mut() {
                        ep.mic.push(MicSpan {
                            take,
                            requested_ms: self.log.ms(requested),
                            ..Default::default()
                        });
                    }
                    self.pending_reports += 1;
                    let _ = self.mic.send(MicCmd::Open {
                        take,
                        requested,
                        save,
                    });
                }
                Effect::MicClose => {
                    if let Some(take) = self.current_take.take() {
                        let _ = self.mic.send(MicCmd::Close { take });
                    }
                }
                _ => {}
            }
            if let Some(ep) = self.episode.as_mut() {
                ep.effects.push((t, e));
            }
        }
    }

    fn after_change(&mut self, t: Ms) {
        self.sync_blocking(t);
        if self.g.is_idle() {
            if let Some(mut ep) = self.episode.take() {
                ep.end_ms = t;
                self.finish_episode(ep);
            }
        }
    }

    fn sync_blocking(&mut self, t: Ms) {
        let want = self.g.blocking();
        if want == self.applied {
            return;
        }
        if let Ok(mut set) = self.blocking.lock() {
            *set = idle_set(want.fn_press);
            if want.fn_space {
                set.insert(Hotkey::new(Modifiers::FN, HkKey::Space).expect("hotkey"));
            }
            if want.esc {
                set.insert(Hotkey::new(Modifiers::empty(), HkKey::Escape).expect("hotkey"));
                set.insert(Hotkey::new(Modifiers::FN, HkKey::Escape).expect("hotkey"));
            }
        }
        self.applied = want;
        self.log.write(
            "blocking",
            self.at(t),
            json!({"fn": want.fn_press, "fn_space": want.fn_space, "esc": want.esc}),
        );
    }

    fn finish_episode(&mut self, mut ep: Episode) {
        let Some(b) = self.block.as_mut() else {
            self.log.write(
                "episode",
                self.at(ep.end_ms),
                json!({"block": null, "episode": ep}),
            );
            return;
        };
        ep.n = b.run.episodes.len() as u32 + 1;
        // Live verdict; clipping audio is checked again once the WAV is in.
        let live_kind = if b.spec.kind == BlockKind::Clipping {
            BlockKind::Hold
        } else {
            b.spec.kind
        };
        let verdict = judge(live_kind, &ep, self.g.timings());
        b.attempts += 1;
        if verdict.outcome != Outcome::Invalid {
            b.valid += 1;
        }
        if !self.quiet && b.spec.kind != BlockKind::Chords {
            let mark = match verdict.outcome {
                Outcome::Pass => format!("{}/{}  ok", b.valid, b.spec.target),
                Outcome::Fail => format!("{}/{}  PROBLEM", b.valid, b.spec.target),
                Outcome::Invalid => "  not counted".to_string(),
            };
            println!("    {mark}: {}", verdict.note);
        } else if !self.quiet && verdict.outcome == Outcome::Fail {
            println!("    note: {}", verdict.note);
        }
        let done =
            b.spec.auto_end && (b.valid >= b.spec.target || b.attempts >= b.spec.target * 2 + 5);
        b.run.episodes.push(JudgedEpisode {
            episode: ep,
            verdict,
        });
        let _ = self.ui.send(UiMsg::EpisodeDone {
            valid: b.valid,
            target: b.spec.target,
        });
        if done && !b.done_sent {
            b.done_sent = true;
            let _ = self.ui.send(UiMsg::BlockDone);
        }
    }

    fn on_mic(&mut self, report: MicReport) {
        self.pending_reports = self.pending_reports.saturating_sub(1);
        let take = report.take;
        let slot = self
            .episode
            .iter_mut()
            .chain(
                self.block
                    .iter_mut()
                    .flat_map(|b| b.run.episodes.iter_mut().rev().map(|j| &mut j.episode)),
            )
            .flat_map(|ep| ep.mic.iter_mut())
            .find(|m| m.take == take);
        if let Some(m) = slot {
            *m = report.span;
        }
    }

    fn end_block(&mut self, rx: &Receiver<Msg>) -> BlockRun {
        let now = Instant::now();
        let t = self.ms(now);
        let effects = self.g.reset(t);
        self.apply(t, effects);
        self.sync_blocking(t);
        if let Some(mut ep) = self.episode.take() {
            ep.end_ms = t;
            self.finish_episode(ep);
        }
        // Collect the mic reports still in flight (the mic thread closes the
        // device and writes the WAV after the take ends).
        let quiet = self.quiet;
        self.quiet = true;
        let deadline = Instant::now() + Duration::from_secs(4);
        while self.pending_reports > 0 {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            match rx.recv_timeout(left) {
                Ok(Msg::End(reply)) => drop(reply),
                Ok(m) => self.handle(m),
                Err(_) => break,
            }
        }
        self.quiet = quiet;
        let timings = self.g.timings();
        let Some(mut b) = self.block.take() else {
            return BlockRun {
                kind: BlockKind::Hold,
                round: String::new(),
                target: 0,
                started_ms: 0.0,
                ended_ms: 0.0,
                episodes: Vec::new(),
                chord_keys: 0,
                fork_events: 0,
                handy_events: 0,
            };
        };
        for j in &mut b.run.episodes {
            j.verdict = judge(b.spec.kind, &j.episode, timings);
        }
        b.run.ended_ms = self.log.ms(Instant::now());
        self.log.write(
            "block_end",
            Instant::now(),
            json!({
                "block": b.spec.kind.id(),
                "round": b.spec.round,
                "episodes": b.run.episodes.len(),
                "pass": b.run.count(Outcome::Pass),
                "fail": b.run.count(Outcome::Fail),
                "invalid": b.run.count(Outcome::Invalid),
                "chord_keys": b.run.chord_keys,
            }),
        );
        for j in &b.run.episodes {
            self.log.write(
                "episode",
                self.at(j.episode.end_ms),
                json!({"block": b.spec.kind.id(), "round": b.spec.round, "episode": j}),
            );
        }
        b.run
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::Timings;
    use crate::taps::fn_only;
    use std::collections::HashSet;
    use std::sync::mpsc;
    use std::sync::Mutex;

    struct Rig {
        tx: Sender<Msg>,
        ui: Receiver<UiMsg>,
        mic: Receiver<MicCmd>,
        blocking: BlockingHotkeys,
        epoch: Instant,
    }

    fn rig() -> Rig {
        rig_with(Grammar::default())
    }

    fn rig_with(grammar: Grammar) -> Rig {
        let epoch = Instant::now();
        let log = Arc::new(Log::null(epoch));
        let blocking: BlockingHotkeys = Arc::new(Mutex::new(idle_set(grammar.blocks_fn())));
        let (mic_tx, mic_rx) = mpsc::channel();
        let (ui_tx, ui_rx) = mpsc::channel();
        let (tx, rx) = mpsc::channel();
        let mut engine = Engine::new(log, Arc::clone(&blocking), mic_tx, ui_tx, grammar);
        engine.quiet = true;
        std::thread::spawn(move || engine.run(rx));
        Rig {
            tx,
            ui: ui_rx,
            mic: mic_rx,
            blocking,
            epoch,
        }
    }

    impl Rig {
        fn send(&self, ms: u64, ev: KeyEvent) {
            let at = self.epoch + Duration::from_millis(ms);
            // Real time must pass for the engine's timers: wait until `at`.
            if let Some(d) = at.checked_duration_since(Instant::now()) {
                std::thread::sleep(d);
            }
            self.tx.send(Msg::Tap(TapMsg::Fork { at, ev })).unwrap();
        }
        fn fn_down(&self, ms: u64) {
            self.send(
                ms,
                KeyEvent {
                    modifiers: Modifiers::FN,
                    key: None,
                    is_key_down: true,
                    changed_modifier: Some(Modifiers::FN),
                },
            );
        }
        fn fn_up(&self, ms: u64) {
            self.send(
                ms,
                KeyEvent {
                    modifiers: Modifiers::empty(),
                    key: None,
                    is_key_down: false,
                    changed_modifier: Some(Modifiers::FN),
                },
            );
        }
        fn key(&self, ms: u64, k: HkKey, down: bool) {
            self.send(
                ms,
                KeyEvent {
                    modifiers: Modifiers::FN,
                    key: Some(k),
                    is_key_down: down,
                    changed_modifier: None,
                },
            );
        }
        fn end(&self) -> BlockRun {
            let (tx, rx) = mpsc::channel();
            self.tx.send(Msg::End(tx)).unwrap();
            rx.recv_timeout(Duration::from_secs(10)).unwrap()
        }
        fn begin(&self, kind: BlockKind, target: u32) {
            self.tx
                .send(Msg::Begin(BlockSpec {
                    kind,
                    round: "test".into(),
                    target,
                    auto_end: true,
                    save_dir: None,
                }))
                .unwrap();
        }
    }

    #[test]
    fn plain_typing_is_not_logged_by_name() {
        assert_eq!(logged_key_name(HkKey::A, false), "(typing)");
        assert_eq!(logged_key_name(HkKey::Num1, false), "(typing)");
        assert_eq!(logged_key_name(HkKey::Keypad7, false), "(typing)");
        assert_eq!(logged_key_name(HkKey::A, true), "A");
        assert_eq!(logged_key_name(HkKey::Home, false), "Home");
        assert_eq!(logged_key_name(HkKey::Escape, false), "Escape");
        assert_eq!(logged_key_name(HkKey::Space, false), "Space");
    }

    #[test]
    fn hold_opens_and_closes_the_mic_and_counts_the_attempt() {
        let r = rig();
        r.begin(BlockKind::Hold, 1);
        r.fn_down(10);
        std::thread::sleep(Duration::from_millis(80));
        assert!(r
            .blocking
            .lock()
            .unwrap()
            .contains(&Hotkey::new(Modifiers::FN, HkKey::Space).unwrap()));
        r.fn_up(700);
        match r.mic.recv_timeout(Duration::from_secs(2)).unwrap() {
            MicCmd::Open { take, .. } => assert_eq!(take, 1),
            _ => panic!("expected open"),
        }
        assert!(matches!(
            r.mic.recv_timeout(Duration::from_secs(2)).unwrap(),
            MicCmd::Close { take: 1 }
        ));
        assert!(matches!(
            r.ui.recv_timeout(Duration::from_secs(2)).unwrap(),
            UiMsg::EpisodeDone {
                valid: 1,
                target: 1
            }
        ));
        assert!(matches!(
            r.ui.recv_timeout(Duration::from_secs(2)).unwrap(),
            UiMsg::BlockDone
        ));
        // Report the take closed so end() need not wait.
        r.tx.send(Msg::Mic(MicReport {
            take: 1,
            span: MicSpan {
                take: 1,
                requested_ms: 160.0,
                first_audio_ms: Some(200.0),
                ..Default::default()
            },
        }))
        .unwrap();
        let run = r.end();
        assert_eq!(run.episodes.len(), 1);
        assert_eq!(run.count(Outcome::Pass), 1, "{:?}", run.episodes[0].verdict);
        let ep = &run.episodes[0].episode;
        assert_eq!(ep.presses.len(), 1);
        assert_eq!(ep.mic[0].first_audio_ms, Some(200.0));
        // Released: blocking set is back to {FN}.
        assert_eq!(*r.blocking.lock().unwrap(), HashSet::from([fn_only()]));
    }

    #[test]
    fn chord_keys_are_counted_without_repeats_and_never_open_the_mic() {
        let r = rig();
        r.begin(BlockKind::Chords, 30);
        r.fn_down(10);
        r.key(40, HkKey::Home, true);
        r.key(70, HkKey::Home, true); // auto-repeat
        r.key(90, HkKey::Home, false);
        r.key(100, HkKey::End, true);
        r.key(120, HkKey::End, false);
        r.fn_up(300);
        std::thread::sleep(Duration::from_millis(50));
        let run = r.end();
        assert_eq!(run.chord_keys, 2);
        assert_eq!(run.episodes.len(), 1);
        assert_eq!(run.count(Outcome::Pass), 1);
        assert!(r.mic.try_recv().is_err(), "mic must not open");
    }

    #[test]
    fn without_the_fn_block_fn_is_never_in_the_set_but_space_and_esc_are() {
        let r = rig_with(Grammar::with_fn_block(Timings::with_show_ms(100), false));
        let fn_space = Hotkey::new(Modifiers::FN, HkKey::Space).unwrap();
        let esc = Hotkey::new(Modifiers::empty(), HkKey::Escape).unwrap();
        assert!(r.blocking.lock().unwrap().is_empty());
        r.fn_down(10);
        std::thread::sleep(Duration::from_millis(50));
        {
            let set = r.blocking.lock().unwrap();
            assert!(set.contains(&fn_space));
            assert!(!set.contains(&fn_only()));
            assert!(!set.contains(&esc), "no take yet at 40 ms");
        }
        std::thread::sleep(Duration::from_millis(120)); // past T_show = 100
        {
            let set = r.blocking.lock().unwrap();
            assert!(set.contains(&esc));
            assert!(!set.contains(&fn_only()));
        }
        assert!(matches!(
            r.mic.recv_timeout(Duration::from_secs(2)).unwrap(),
            MicCmd::Open { take: 1, .. }
        ));
        r.fn_up(700);
        std::thread::sleep(Duration::from_millis(50));
        assert!(r.blocking.lock().unwrap().is_empty());
    }

    #[test]
    fn esc_is_blocked_only_while_a_take_is_active() {
        let r = rig();
        let esc = Hotkey::new(Modifiers::empty(), HkKey::Escape).unwrap();
        r.fn_down(10);
        std::thread::sleep(Duration::from_millis(60));
        assert!(!r.blocking.lock().unwrap().contains(&esc));
        std::thread::sleep(Duration::from_millis(250)); // past T_show
        assert!(r.blocking.lock().unwrap().contains(&esc));
        r.key(600, HkKey::Escape, true);
        std::thread::sleep(Duration::from_millis(50));
        assert!(!r.blocking.lock().unwrap().contains(&esc));
        r.fn_up(700);
    }
}
