// SPDX-License-Identifier: GPL-3.0-or-later
//! The engine end to end with a fake coordinator and a fake clock: raw
//! events in, coordinator commands, blocking sets and observations out.

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use handy_keys::{Hotkey, Key as HkKey, KeyEvent, Modifiers};

use crate::fork::settings::default_hotkeys;
use crate::fork::shortcut::actions::{EscContext, Registry};
use crate::fork::shortcut::bindings::{compile, fallback_hotkeys};
use crate::fork::shortcut::engine::*;
use crate::fork::shortcut::processor::{Effect, Ms, Timings, WarnKind};
use crate::fork::shortcut::{owns_in, ForkHotkeyEvent};
use crate::transcription_coordinator::ExternalCommand;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Idle,
    Recording,
    Processing,
}

/// A coordinator with Handy's busy rules: one start remembered while
/// processing, Stop forgets it, Cancel clears everything.
struct Fake {
    stage: Stage,
    queued: bool,
    commands: Vec<ExternalCommand>,
    blocking: HashSet<Hotkey>,
    observed: Vec<Observed>,
    prepared: usize,
    physical: Option<bool>,
    warnings: Vec<WarnKind>,
    /// Starts that actually began recording.
    recordings: usize,
}

impl Fake {
    fn new() -> Fake {
        Fake {
            stage: Stage::Idle,
            queued: false,
            commands: vec![],
            blocking: HashSet::new(),
            observed: vec![],
            prepared: 0,
            physical: None,
            warnings: vec![],
            recordings: 0,
        }
    }

    /// The pipeline finished transcribing.
    fn finish(&mut self) {
        assert_eq!(self.stage, Stage::Processing);
        self.stage = Stage::Idle;
        if std::mem::take(&mut self.queued) {
            self.stage = Stage::Recording;
            self.recordings += 1;
        }
    }

    fn names(&self) -> Vec<&'static str> {
        self.commands
            .iter()
            .map(|c| match c {
                ExternalCommand::Start { .. } => "start",
                ExternalCommand::Stop => "stop",
                ExternalCommand::Lock => "lock",
                ExternalCommand::Cancel => "cancel",
            })
            .collect()
    }
}

impl Host for Fake {
    fn command(&mut self, command: ExternalCommand) {
        match &command {
            ExternalCommand::Start { binding_id } => {
                assert!(binding_id == BINDING || binding_id == COMMAND_BINDING);
                match self.stage {
                    Stage::Idle => {
                        self.stage = Stage::Recording;
                        self.recordings += 1;
                    }
                    Stage::Processing => self.queued = true,
                    Stage::Recording => {}
                }
            }
            ExternalCommand::Stop => match self.stage {
                Stage::Recording => self.stage = Stage::Processing,
                Stage::Processing => self.queued = false,
                Stage::Idle => {}
            },
            ExternalCommand::Lock => {}
            ExternalCommand::Cancel => {
                self.stage = Stage::Idle;
                self.queued = false;
            }
        }
        self.commands.push(command);
    }

    fn busy(&self) -> bool {
        self.stage != Stage::Idle || self.queued
    }

    fn set_blocking(&mut self, set: HashSet<Hotkey>) {
        self.blocking = set;
    }

    fn prepare_mic(&mut self) {
        self.prepared += 1;
    }

    fn trigger_physically_down(&self) -> Option<bool> {
        self.physical
    }

    fn warn(&mut self, kind: WarnKind) {
        self.warnings.push(kind);
    }

    fn observe(&mut self, event: &Observed) {
        self.observed.push(event.clone());
    }
}

struct Rig {
    engine: Engine,
    host: Fake,
    now: Ms,
}

impl Rig {
    fn new() -> Rig {
        Self::with(compile(&default_hotkeys(), |_| false), Registry::default())
    }

    fn with(profile: crate::fork::shortcut::bindings::Profile, registry: Registry) -> Rig {
        let mut host = Fake::new();
        let mut engine = Engine::new(profile, registry, Timings::DEFAULT, false);
        engine.tick(0, &mut host);
        Rig {
            engine,
            host,
            now: 0,
        }
    }

    /// Advances the clock to `t`, waking the engine at every deadline on
    /// the way, as the live loop does.
    fn at(&mut self, t: Ms) -> &mut Self {
        while let Some(w) = self.engine.next_wake(self.now) {
            if w > t {
                break;
            }
            self.now = w.max(self.now);
            self.engine.tick(self.now, &mut self.host);
        }
        self.now = t;
        self
    }

    fn ev(&mut self, t: Ms, ev: KeyEvent) -> &mut Self {
        self.at(t);
        self.engine.event(t, &ev, &mut self.host);
        self
    }

    fn fn_down(&mut self, t: Ms) -> &mut Self {
        self.ev(t, flags(Modifiers::FN, true, Modifiers::FN))
    }

    fn fn_up(&mut self, t: Ms) -> &mut Self {
        self.ev(t, flags(Modifiers::empty(), false, Modifiers::FN))
    }

    fn key(&mut self, t: Ms, m: Modifiers, k: HkKey) -> &mut Self {
        self.ev(t, key_ev(m, k, true));
        self.ev(t + 5, key_ev(m, k, false))
    }
}

fn flags(modifiers: Modifiers, down: bool, changed: Modifiers) -> KeyEvent {
    KeyEvent {
        modifiers,
        key: None,
        is_key_down: down,
        changed_modifier: Some(changed),
    }
}

fn key_ev(modifiers: Modifiers, key: HkKey, down: bool) -> KeyEvent {
    KeyEvent {
        modifiers,
        key: Some(key),
        is_key_down: down,
        changed_modifier: None,
    }
}

fn hk(s: &str) -> Hotkey {
    s.parse().unwrap()
}

#[test]
fn push_to_talk_starts_at_t_show_and_stops_on_release() {
    let mut r = Rig::new();
    r.fn_down(1000).at(1149);
    assert!(r.host.commands.is_empty());
    r.at(1150);
    assert_eq!(r.host.names(), vec!["start"]);
    r.fn_up(2000);
    assert_eq!(r.host.names(), vec!["start", "stop"]);
    assert_eq!(r.host.prepared, 1);
}

#[test]
fn fn_ctrl_routes_a_command_only_when_enabled_and_latches_the_take() {
    use crate::fork::settings::hotkey_actions::COMMAND_MODE;
    let on = compile(&default_hotkeys(), |id| id == COMMAND_MODE);
    assert!(on.command_mode);
    let mut r = Rig::with(on, Registry::default());
    r.ev(0, flags(Modifiers::CTRL_LEFT, true, Modifiers::CTRL_LEFT));
    r.ev(
        20,
        flags(Modifiers::FN | Modifiers::CTRL_LEFT, true, Modifiers::FN),
    );
    r.at(170);
    assert!(matches!(
        r.host.commands.first(),
        Some(ExternalCommand::Start { binding_id }) if binding_id == COMMAND_BINDING
    ));
    // Releasing Ctrl during the instruction does not change its mode.
    r.ev(500, flags(Modifiers::FN, false, Modifiers::CTRL_LEFT));
    r.fn_up(900);
    assert_eq!(r.host.names(), vec!["start", "stop"]);

    let off = compile(&default_hotkeys(), |_| false);
    assert!(!off.command_mode);
    let mut r = Rig::with(off, Registry::default());
    r.ev(0, flags(Modifiers::CTRL_LEFT, true, Modifiers::CTRL_LEFT));
    r.ev(
        20,
        flags(Modifiers::FN | Modifiers::CTRL_LEFT, true, Modifiers::FN),
    );
    r.at(170);
    assert!(matches!(
        r.host.commands.first(),
        Some(ExternalCommand::Start { binding_id }) if binding_id == BINDING
    ));
}

#[test]
fn a_quick_tap_sends_nothing() {
    let mut r = Rig::new();
    r.fn_down(0).fn_up(90).at(2000);
    assert!(r.host.commands.is_empty());
    assert_eq!(r.host.recordings, 0);
}

#[test]
fn double_tap_starts_locked_and_the_next_press_stops() {
    let mut r = Rig::new();
    r.fn_down(0).fn_up(90).fn_down(250).fn_up(330);
    assert_eq!(r.host.names(), vec!["start", "lock"]);
    r.at(5000).fn_down(5000);
    assert_eq!(r.host.names(), vec!["start", "lock", "stop"]);
    r.fn_up(5080).at(6000);
    assert_eq!(r.host.names().len(), 3);
}

#[test]
fn a_first_press_held_past_t_show_then_a_second_press_locks_the_same_take() {
    let mut r = Rig::new();
    r.fn_down(0).fn_up(300).fn_down(500);
    assert_eq!(r.host.names(), vec!["start", "lock"]);
    assert_eq!(r.host.recordings, 1);
}

#[test]
fn fn_space_locks_and_space_is_blocked_only_while_fn_is_down() {
    let mut r = Rig::new();
    assert!(
        r.host.blocking.is_empty(),
        "idle: nothing blocked (Globe ≠ Dictation)"
    );
    r.fn_down(0);
    assert!(r.host.blocking.contains(&hk("fn+space")));
    r.key(60, Modifiers::FN, HkKey::Space);
    assert_eq!(r.host.names(), vec!["start", "lock"]);
    r.fn_up(200);
    assert!(!r.host.blocking.contains(&hk("fn+space")));
    assert!(
        r.host.blocking.contains(&hk("escape")),
        "Esc cancels while locked"
    );
}

#[test]
fn a_fluent_chord_never_starts_a_take() {
    for k in [
        HkKey::LeftArrow,
        HkKey::RightArrow,
        HkKey::UpArrow,
        HkKey::DownArrow,
        HkKey::Delete,
        HkKey::F11,
        HkKey::F1,
    ] {
        let mut r = Rig::new();
        r.fn_down(0).key(70, Modifiers::FN, k).fn_up(130).at(2000);
        assert!(r.host.commands.is_empty(), "{k:?}");
        assert_eq!(r.host.recordings, 0, "{k:?}: the mic never opened");
    }
}

#[test]
fn a_late_chord_cancels_the_take_and_never_stops_it() {
    let mut r = Rig::new();
    r.fn_down(0)
        .at(1200)
        .key(1200, Modifiers::FN, HkKey::Home)
        .fn_up(1400)
        .at(3000);
    assert_eq!(r.host.names(), vec!["start", "cancel"]);
    let kept = r.host.observed.iter().find_map(|o| match o {
        Observed::Effect(Effect::ChordIgnored { keep, .. }) => Some(*keep),
        _ => None,
    });
    assert_eq!(kept, Some(true), "DEC-54: the take ran ≥ T");
}

#[test]
fn esc_with_fn_held_cancels() {
    let mut r = Rig::new();
    r.fn_down(0).at(800);
    assert!(r.host.blocking.contains(&hk("fn+escape")));
    r.key(800, Modifiers::FN, HkKey::Escape);
    assert_eq!(r.host.names(), vec!["start", "cancel"]);
    r.fn_up(1000);
    assert_eq!(r.host.names().len(), 2, "the release does nothing");
}

#[test]
fn esc_while_processing_cancels_the_transcription() {
    let mut r = Rig::new();
    r.fn_down(0).fn_up(900);
    assert_eq!(r.host.stage, Stage::Processing);
    r.at(920);
    assert!(
        r.host.blocking.contains(&hk("escape")),
        "Esc blocked while processing"
    );
    r.key(950, Modifiers::empty(), HkKey::Escape);
    assert_eq!(r.host.names(), vec!["start", "stop", "cancel"]);
    assert!(r.host.observed.contains(&Observed::EscCancelledBusy));
    r.at(1100);
    assert!(
        !r.host.blocking.contains(&hk("escape")),
        "idle again: Esc free"
    );
}

#[test]
fn idle_esc_is_neither_blocked_nor_a_command_and_observers_still_see_it() {
    let reg = Registry::default();
    let seen = Arc::new(AtomicUsize::new(0));
    let s = seen.clone();
    reg.observe_esc(Arc::new(move |ctx: EscContext| {
        assert!(!ctx.cancelled_take);
        s.fetch_add(1, Ordering::SeqCst);
    }));
    let mut r = Rig::with(compile(&default_hotkeys(), |_| false), reg);
    assert!(!r.host.blocking.contains(&hk("escape")));
    r.key(10, Modifiers::empty(), HkKey::Escape);
    assert!(r.host.commands.is_empty());
    assert_eq!(seen.load(Ordering::SeqCst), 1);
}

#[test]
fn esc_observers_learn_that_esc_cancelled_a_take() {
    let reg = Registry::default();
    let cancelled = Arc::new(AtomicUsize::new(0));
    let c = cancelled.clone();
    reg.observe_esc(Arc::new(move |ctx: EscContext| {
        if ctx.cancelled_take {
            c.fetch_add(1, Ordering::SeqCst);
        }
    }));
    let mut r = Rig::with(compile(&default_hotkeys(), |_| false), reg);
    r.fn_down(0).fn_up(90).fn_down(200).fn_up(260);
    r.key(3000, Modifiers::empty(), HkKey::Escape);
    assert_eq!(cancelled.load(Ordering::SeqCst), 1);
    assert_eq!(r.host.names(), vec!["start", "lock", "cancel"]);
}

#[test]
fn a_press_while_processing_queues_the_next_dictation() {
    // A-206: the second hold starts while the first is transcribing; the
    // coordinator runs it when the pipeline drains.
    let mut r = Rig::new();
    r.fn_down(0).fn_up(900);
    assert_eq!(r.host.stage, Stage::Processing);
    r.fn_down(1000).fn_up(1100).fn_down(1200);
    // tap then a second press: hands-free lock, queued behind processing
    assert_eq!(r.host.names(), vec!["start", "stop", "start", "lock"]);
    assert!(r.host.queued);
    r.host.finish();
    assert_eq!(r.host.stage, Stage::Recording);
    assert_eq!(r.host.recordings, 2);
}

#[test]
fn a_hold_released_while_still_processing_forgets_the_queued_start() {
    let mut r = Rig::new();
    r.fn_down(0).fn_up(900).fn_down(1000).at(1150);
    assert!(r.host.queued);
    r.fn_up(1800);
    assert!(!r.host.queued, "Stop forgets the remembered start");
    r.host.finish();
    assert_eq!(r.host.stage, Stage::Idle);
}

#[test]
fn a_keyed_action_fires_its_handler_without_touching_the_grammar() {
    let reg = Registry::default();
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();
    reg.register(
        "paste_last",
        Arc::new(move || {
            h.fetch_add(1, Ordering::SeqCst);
        }),
    )
    .unwrap();
    let profile = compile(&default_hotkeys(), |id| reg.handles(id));
    let mut r = Rig::with(profile, reg);
    assert!(r.host.blocking.contains(&hk("ctrl+command+v")));
    r.key(10, Modifiers::CTRL_LEFT | Modifiers::CMD_LEFT, HkKey::V);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert!(r.host.commands.is_empty());
    assert!(r
        .host
        .observed
        .contains(&Observed::Action("paste_last".into())));
    // Shift+Ctrl+Cmd+V does not match exactly.
    r.key(
        100,
        Modifiers::CTRL_LEFT | Modifiers::CMD_LEFT | Modifiers::SHIFT_LEFT,
        HkKey::V,
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[test]
fn press_enter_on_a_mouse_button_fires_and_never_blocks_the_click() {
    let reg = Registry::default();
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();
    reg.register(
        "press_enter",
        Arc::new(move || {
            h.fetch_add(1, Ordering::SeqCst);
        }),
    )
    .unwrap();
    let mut map = default_hotkeys();
    map.insert(
        "press_enter".into(),
        vec![crate::fork::settings::HotkeyBinding {
            chord: "mousex1".into(),
            activation: crate::fork::settings::HotkeyActivation::Press,
        }],
    );
    let profile = compile(&map, |id| reg.handles(id));
    let mut r = Rig::with(profile, reg);
    // In the set like any keyed binding; handy-keys' tap never consults it
    // for mouse buttons (RS-11), so the click still reaches the app.
    assert!(r.host.blocking.contains(&hk("mousex1")));
    r.key(10, Modifiers::empty(), HkKey::MouseX1);
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "fires on the button-down only"
    );
    // Another button, or the button with a modifier, does not.
    r.key(100, Modifiers::empty(), HkKey::MouseX2);
    r.key(200, Modifiers::OPT_LEFT, HkKey::MouseX1);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert!(r.host.commands.is_empty(), "no take started or stopped");
}

#[test]
fn a_stuck_fn_is_released_after_one_second() {
    let mut r = Rig::new();
    r.host.physical = Some(true);
    r.fn_down(0).at(700);
    r.host.physical = Some(false); // the release event was lost
    r.at(1600);
    assert_eq!(
        r.host.names(),
        vec!["start"],
        "not yet: 900 ms of disagreement"
    );
    r.at(2000);
    assert_eq!(r.host.names(), vec!["start", "stop"]);
    assert!(r.host.observed.contains(&Observed::StuckReleased));
    // The real (late) release is then a no-op.
    r.fn_up(2100).at(3000);
    assert_eq!(r.host.names().len(), 2);
}

#[test]
fn a_real_long_hold_is_never_released_by_the_guard() {
    let mut r = Rig::new();
    r.host.physical = Some(true);
    r.fn_down(0).at(20_000);
    assert_eq!(r.host.names(), vec!["start"]);
    r.fn_up(20_000);
    assert_eq!(r.host.names(), vec!["start", "stop"]);
}

#[test]
fn changing_the_profile_mid_take_cancels_it() {
    let mut r = Rig::new();
    r.fn_down(0).at(400);
    let p = compile(&fallback_hotkeys(), |_| false);
    r.engine.set_profile(400, p, &mut r.host);
    assert_eq!(r.host.names(), vec!["start", "cancel"]);
    // fn is no longer a trigger.
    r.fn_up(500).fn_down(1000).at(2000);
    assert_eq!(r.host.names().len(), 2);
}

#[test]
fn the_ctrl_opt_profile_runs_the_same_grammar() {
    let mut r = Rig::with(compile(&fallback_hotkeys(), |_| false), Registry::default());
    let co = Modifiers::CTRL_LEFT | Modifiers::OPT_LEFT;
    r.ev(0, flags(Modifiers::CTRL_LEFT, true, Modifiers::CTRL_LEFT))
        .ev(20, flags(co, true, Modifiers::OPT_LEFT))
        .at(1000)
        .ev(
            1000,
            flags(Modifiers::CTRL_LEFT, false, Modifiers::OPT_LEFT),
        );
    assert_eq!(r.host.names(), vec!["start", "stop"]);
    // Ctrl+Opt+Space locks.
    r.ev(
        3000,
        flags(Modifiers::OPT_RIGHT, true, Modifiers::OPT_RIGHT),
    )
    .ev(
        3010,
        flags(
            Modifiers::OPT_RIGHT | Modifiers::CTRL_RIGHT,
            true,
            Modifiers::CTRL_RIGHT,
        ),
    );
    assert!(r.host.blocking.contains(&hk("ctrl+option+space")));
    r.key(
        3050,
        Modifiers::OPT_RIGHT | Modifiers::CTRL_RIGHT,
        HkKey::Space,
    );
    assert_eq!(r.host.names(), vec!["start", "stop", "start", "lock"]);
    // fn does nothing in this profile.
    let before = r.host.names().len();
    r.fn_down(9000).at(9500).fn_up(9500);
    assert_eq!(r.host.names().len(), before);
}

#[test]
fn ctrl_with_fn_does_not_change_a_dictation_take() {
    let mut r = Rig::new();
    r.fn_down(0)
        .at(300)
        .ev(
            300,
            flags(
                Modifiers::FN | Modifiers::CTRL_LEFT,
                true,
                Modifiers::CTRL_LEFT,
            ),
        )
        .ev(400, flags(Modifiers::FN, false, Modifiers::CTRL_LEFT))
        .fn_up(900);
    assert_eq!(r.host.names(), vec!["start", "stop"]);
}

#[test]
fn space_is_a_chord_when_hands_free_has_no_fn_space_binding() {
    let mut map = default_hotkeys();
    map.insert(
        "hands_free".into(),
        vec![crate::fork::settings::HotkeyBinding {
            chord: "fn".into(),
            activation: crate::fork::settings::HotkeyActivation::DoubleTap,
        }],
    );
    let mut r = Rig::with(compile(&map, |_| false), Registry::default());
    r.fn_down(0);
    assert!(
        !r.host.blocking.contains(&hk("fn+space")),
        "Space not swallowed"
    );
    r.key(50, Modifiers::FN, HkKey::Space).fn_up(100).at(2000);
    assert!(r.host.commands.is_empty());
}

#[test]
fn hands_free_silence_stop_reaches_the_coordinator_as_stop() {
    let mut r = Rig::new();
    r.fn_down(0).fn_up(90).fn_down(200).fn_up(260).at(200_000);
    assert_eq!(r.host.names(), vec!["start", "lock", "stop"]);
    assert_eq!(r.host.warnings, vec![WarnKind::Silence]);
}

#[test]
fn voice_keeps_a_hands_free_take_alive() {
    let mut r = Rig::new();
    r.fn_down(0).fn_up(90).fn_down(200).fn_up(260);
    let mut t = 1000;
    while t < 400_000 {
        r.at(t);
        r.engine.voice(t, &mut r.host);
        t += 60_000;
    }
    assert_eq!(r.host.names(), vec!["start", "lock"]);
    assert!(r.host.warnings.is_empty());
}

#[test]
fn next_wake_covers_deadlines_the_stuck_guard_and_the_busy_poll() {
    let mut r = Rig::new();
    assert_eq!(r.engine.next_wake(0), None, "idle: sleep until an event");
    r.fn_down(0);
    assert_eq!(r.engine.next_wake(0), Some(150));
    r.at(150);
    assert_eq!(
        r.engine.next_wake(150),
        Some(250),
        "stuck poll while fn is down, on an absolute schedule"
    );
    r.fn_up(900);
    assert_eq!(
        r.engine.next_wake(900),
        Some(900 + BUSY_POLL_MS),
        "busy poll"
    );
    r.host.finish();
    r.at(1000);
    assert_eq!(r.engine.next_wake(1000), None);
}

#[test]
fn the_fn_press_is_blocked_only_when_asked() {
    let mut host = Fake::new();
    let mut e = Engine::new(
        compile(&default_hotkeys(), |_| false),
        Registry::default(),
        Timings::DEFAULT,
        true,
    );
    e.tick(0, &mut host);
    assert_eq!(host.blocking, HashSet::from([hk("fn")]));
    e.set_fn_block(false, &mut host);
    assert!(host.blocking.is_empty());
}

#[test]
fn reset_cancels_an_active_take() {
    let mut r = Rig::new();
    r.fn_down(0).fn_up(90).fn_down(200).fn_up(260);
    r.engine.reset(1000, &mut r.host);
    assert_eq!(r.host.names(), vec!["start", "lock", "cancel"]);
}

#[test]
fn hotkey_events_for_listeners() {
    let e = |o: Observed| ForkHotkeyEvent::from_observed(&o);
    assert_eq!(
        e(Observed::Effect(Effect::TakeStarted)).unwrap().kind,
        "take_started"
    );
    let stopped = e(Observed::Effect(Effect::Stopped {
        via: crate::fork::shortcut::processor::StopVia::Silence,
    }))
    .unwrap();
    assert_eq!(
        (stopped.kind.as_str(), stopped.detail.as_deref()),
        ("stopped", Some("silence"))
    );
    let chord = e(Observed::Effect(Effect::ChordIgnored {
        key: "Home".into(),
        mic_was_open: true,
        keep: true,
    }))
    .unwrap();
    assert_eq!(
        (chord.kind.as_str(), chord.keep, chord.detail),
        ("chord", true, None)
    );
    assert!(e(Observed::Effect(Effect::MicOpen)).is_none());
    assert!(
        e(Observed::Effect(Effect::ChordIgnored {
            key: "Home".into(),
            mic_was_open: false,
            keep: false,
        }))
        .is_none(),
        "a fluent chord is not news"
    );
    assert_eq!(
        e(Observed::Action("paste_last".into()))
            .unwrap()
            .detail
            .as_deref(),
        Some("paste_last")
    );
}

#[test]
fn the_fork_owns_handys_dictation_bindings_only_in_product_builds() {
    for id in ["transcribe", "transcribe_with_post_process", "cancel"] {
        assert!(owns_in(true, id), "{id}");
        assert!(!owns_in(false, id), "{id}");
    }
    assert!(!owns_in(true, "test"));
}

#[test]
fn cli_toggle_and_cancel_bypass_the_shortcut_registry() {
    // A-67, E-86: `--toggle-transcription` and `--cancel` reach the
    // coordinator directly, never through register_shortcut, so owns()
    // cannot disable them.
    let lib = include_str!("../../../lib.rs");
    assert!(lib.contains("signal_handle::send_transcription_input(app, \"transcribe\", \"CLI\");"));
    assert!(lib.contains("} else if args.iter().any(|a| a == \"--cancel\") {\n                crate::utils::cancel_current_operation(app);"));
    let signal = include_str!("../../../signal_handle.rs");
    assert!(signal.contains("c.send_external_input(binding_id, source);"));
    assert!(!signal.contains("register_shortcut"));
    // And the hook sits first in both backends' register and unregister,
    // which every path goes through: startup (`init_shortcuts`), binding
    // changes, cancel reconcile, backend switches and the Carbon shadows.
    let hook = "\n    if crate::fork::shortcut::owns(&binding.id) {\n        return Ok(());\n    }";
    let tauri = include_str!("../../../shortcut/tauri_impl.rs");
    for f in ["register_shortcut", "unregister_shortcut"] {
        let head = format!(
            "pub fn {f}(app: &AppHandle, binding: ShortcutBinding) -> Result<(), String> {{{hook}"
        );
        assert!(tauri.contains(&head), "tauri_impl::{f}");
    }
    let keys = include_str!("../../../shortcut/handy_keys.rs");
    let indented = hook.replace("\n", "\n    ");
    for f in ["register", "unregister"] {
        let head = format!(
            "    pub fn {f}(&self, binding: &ShortcutBinding) -> Result<(), String> {{{indented}"
        );
        assert!(keys.contains(&head), "HandyKeysState::{f}");
    }
    // handy-keys' startup registers through HandyKeysState::register.
    assert!(keys.contains("if let Err(e) = state.register(&binding) {"));
}

// --- S-03: the Flow Bar ---

#[test]
fn bar_click_starts_a_locked_take_and_stop_inserts_it() {
    let mut r = Rig::new();
    r.at(1_000);
    r.engine.pill(1_000, PillAction::Click, &mut r.host);
    assert_eq!(r.host.names(), vec!["start", "lock"]);
    assert_eq!(r.host.recordings, 1);
    // Esc now cancels (blocked while the take runs).
    assert!(r.host.blocking.iter().any(|h| h.key == Some(HkKey::Escape)));
    r.at(3_000);
    r.engine.pill(3_000, PillAction::Stop, &mut r.host);
    assert_eq!(r.host.names(), vec!["start", "lock", "stop"]);
    assert!(r
        .host
        .observed
        .iter()
        .any(|o| matches!(o, Observed::Effect(Effect::Locked { .. }))));
}

#[test]
fn bar_click_on_a_running_bar_take_stops_it_and_cancel_cancels() {
    let mut r = Rig::new();
    r.engine.pill(10, PillAction::Click, &mut r.host);
    r.engine.pill(2_000, PillAction::Click, &mut r.host);
    assert_eq!(r.host.names(), vec!["start", "lock", "stop"]);
    r.host.finish();
    r.host.commands.clear();
    r.engine.pill(3_000, PillAction::Click, &mut r.host);
    r.engine.pill(3_500, PillAction::Cancel, &mut r.host);
    assert_eq!(r.host.names(), vec!["start", "lock", "cancel"]);
}

#[test]
fn fn_press_stops_a_bar_take() {
    let mut r = Rig::new();
    r.engine.pill(10, PillAction::Click, &mut r.host);
    r.fn_down(200).fn_up(260);
    assert_eq!(r.host.names(), vec!["start", "lock", "stop"]);
}

#[test]
fn bar_buttons_reach_a_take_the_grammar_does_not_own() {
    // A take started by the CLI (`--toggle-transcription`).
    let mut r = Rig::new();
    r.host.stage = Stage::Recording;
    r.engine.pill(100, PillAction::Stop, &mut r.host);
    assert_eq!(r.host.names(), vec!["stop"]);
    r.host.stage = Stage::Recording;
    r.engine.pill(200, PillAction::Cancel, &mut r.host);
    assert_eq!(r.host.names(), vec!["stop", "cancel"]);
    // A click while the pipeline is busy starts a take the coordinator
    // remembers (Handy's queued start, A-206).
    r.host.stage = Stage::Processing;
    r.engine.pill(300, PillAction::Click, &mut r.host);
    assert_eq!(r.host.names(), vec!["stop", "cancel", "start", "lock"]);
    assert!(r.host.queued);
}

// ---------------------------------------------------------------------------
// S-02: Secure Input shadows (Carbon presses routed into the engine)

fn paste_last_rig() -> (Rig, Arc<AtomicUsize>) {
    let reg = Registry::default();
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();
    reg.register(
        "paste_last",
        Arc::new(move || {
            h.fetch_add(1, Ordering::SeqCst);
        }),
    )
    .unwrap();
    let profile = compile(&default_hotkeys(), |id| reg.handles(id));
    (Rig::with(profile, reg), hits)
}

#[test]
fn a_shadow_press_fires_the_keyed_action_under_secure_input() {
    let (mut r, hits) = paste_last_rig();
    r.at(10);
    r.engine.shadow(10, "paste_last", &mut r.host);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert!(r
        .host
        .observed
        .contains(&Observed::Action("paste_last".into())));
    // An unbound action is ignored.
    r.engine.shadow(20, "transform.unknown", &mut r.host);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[test]
fn tap_and_shadow_reporting_one_press_fire_once() {
    let (mut r, hits) = paste_last_rig();
    // The tap saw it first: the shadow is dropped.
    r.key(10, Modifiers::CTRL_LEFT | Modifiers::CMD_LEFT, HkKey::V);
    r.engine.shadow(60, "paste_last", &mut r.host);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    // The shadow came first: the tap's report is dropped.
    r.at(2_000);
    r.engine.shadow(2_000, "paste_last", &mut r.host);
    r.key(2_040, Modifiers::CTRL_LEFT | Modifiers::CMD_LEFT, HkKey::V);
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    // Two presses far apart are two presses.
    r.at(4_000);
    r.engine.shadow(4_000, "paste_last", &mut r.host);
    assert_eq!(hits.load(Ordering::SeqCst), 3);
}

#[test]
fn shadowed_esc_cancels_a_held_take() {
    let mut r = Rig::new();
    r.fn_down(0).at(800);
    assert_eq!(r.host.stage, Stage::Recording);
    r.engine.shadow(800, "cancel", &mut r.host);
    assert_eq!(r.host.names(), vec!["start", "cancel"]);
    r.fn_up(1_000);
    assert_eq!(r.host.names().len(), 2, "the release does nothing");
}

#[test]
fn shadowed_esc_cancels_processing_and_a_hands_free_take() {
    let mut r = Rig::new();
    r.fn_down(0).fn_up(900);
    assert_eq!(r.host.stage, Stage::Processing);
    r.at(950);
    r.engine.shadow(950, "cancel", &mut r.host);
    assert_eq!(r.host.names(), vec!["start", "stop", "cancel"]);
    assert!(r.host.observed.contains(&Observed::EscCancelledBusy));

    let mut r = Rig::new();
    r.fn_down(0).fn_up(90).fn_down(200).fn_up(260);
    r.at(3_000);
    r.engine.shadow(3_000, "cancel", &mut r.host);
    assert_eq!(r.host.names(), vec!["start", "lock", "cancel"]);
}

#[test]
fn a_tap_esc_and_its_shadow_cancel_once() {
    let mut r = Rig::new();
    r.fn_down(0).fn_up(90).fn_down(200).fn_up(260);
    r.key(3_000, Modifiers::empty(), HkKey::Escape);
    r.engine.shadow(3_050, "cancel", &mut r.host);
    assert_eq!(r.host.names(), vec!["start", "lock", "cancel"]);
    assert!(!r.host.observed.contains(&Observed::EscCancelledBusy));
}

#[test]
fn an_idle_shadowed_esc_sends_nothing() {
    let mut r = Rig::new();
    r.at(10);
    r.engine.shadow(10, "cancel", &mut r.host);
    assert!(r.host.commands.is_empty());
}

// OT2-A -----------------------------------------------------------------

/// The owner's rc3 report: switching windows while holding fn cancelled
/// the take. ⌘-Tab, a click into another app, ⌃-→ to another Space and
/// ⌘-1 to another tab all keep it recording; the release stops it.
#[test]
fn a_held_take_survives_app_window_tab_and_space_switches() {
    let mut r = Rig::new();
    r.fn_down(0).at(200);
    assert_eq!(r.host.names(), vec!["start"]);
    // ⌘-Tab (twice, through the switcher) with fn held.
    let fc = Modifiers::FN | Modifiers::CMD_LEFT;
    r.ev(700, flags(fc, true, Modifiers::CMD_LEFT));
    r.key(720, fc, HkKey::Tab);
    r.key(760, fc, HkKey::Tab);
    r.key(780, fc, HkKey::Num1);
    r.ev(800, flags(Modifiers::FN, false, Modifiers::CMD_LEFT));
    // A click into another window.
    r.key(1_000, Modifiers::FN, HkKey::MouseLeft);
    // ⌃-→ to the next Space.
    let fx = Modifiers::FN | Modifiers::CTRL_LEFT;
    r.ev(1_200, flags(fx, true, Modifiers::CTRL_LEFT));
    r.key(1_220, fx, HkKey::RightArrow);
    r.ev(1_260, flags(Modifiers::FN, false, Modifiers::CTRL_LEFT));
    // The stuck guard still sees fn physically down.
    r.host.physical = Some(true);
    r.at(3_000);
    assert_eq!(r.host.names(), vec!["start"], "nothing ended the take");
    assert!(r.engine.grammar().take_active());
    r.fn_up(3_100);
    assert_eq!(r.host.names(), vec!["start", "stop"]);
}

/// Esc still cancels a held take after a switch; an ordinary fn+key chord
/// still drops it (rule 2 is unchanged for plain keys).
#[test]
fn after_a_switch_esc_cancels_and_a_plain_chord_still_drops() {
    let mut r = Rig::new();
    r.fn_down(0).at(200);
    let fc = Modifiers::FN | Modifiers::CMD_LEFT;
    r.key(300, fc, HkKey::Tab);
    r.key(400, Modifiers::FN, HkKey::Escape);
    assert_eq!(r.host.names(), vec!["start", "cancel"]);
    r.fn_up(500);

    let mut r = Rig::new();
    r.fn_down(0).at(200);
    r.key(300, Modifiers::FN, HkKey::MouseLeft);
    r.key(400, Modifiers::FN, HkKey::F);
    assert_eq!(r.host.names(), vec!["start", "cancel"]);
}

/// Hands-free already let every key pass (the owner: "fine there"); still so.
#[test]
fn hands_free_keeps_recording_through_switches() {
    let mut r = Rig::new();
    r.fn_down(0).fn_up(80).fn_down(200).fn_up(260);
    assert_eq!(r.host.names(), vec!["start", "lock"]);
    r.key(800, Modifiers::CMD_LEFT, HkKey::Tab);
    r.key(900, Modifiers::empty(), HkKey::MouseLeft);
    r.at(5_000);
    assert_eq!(r.host.names(), vec!["start", "lock"]);
}
