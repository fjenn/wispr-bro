// SPDX-License-Identifier: GPL-3.0-or-later
//! Raw tap events → grammar inputs, per trigger, and the stuck guard.

use handy_keys::{Hotkey, Key as HkKey, KeyEvent, Modifiers};

use crate::fork::shortcut::decision::*;
use crate::fork::shortcut::processor::{Input, Key};

fn flags(modifiers: Modifiers, down: bool, changed: Modifiers) -> KeyEvent {
    KeyEvent {
        modifiers,
        key: None,
        is_key_down: down,
        changed_modifier: Some(changed),
    }
}

fn key(modifiers: Modifiers, key: HkKey, down: bool) -> KeyEvent {
    KeyEvent {
        modifiers,
        key: Some(key),
        is_key_down: down,
        changed_modifier: None,
    }
}

fn input(i: Input) -> Decision {
    Decision::Input(i)
}

#[test]
fn fn_edges_become_fn_down_and_up() {
    let mut t = Translator::new(Trigger::Fn);
    assert_eq!(
        t.translate(&flags(Modifiers::FN, true, Modifiers::FN)),
        input(Input::FnDown)
    );
    assert!(t.is_down());
    assert_eq!(
        t.translate(&flags(Modifiers::empty(), false, Modifiers::FN)),
        input(Input::FnUp)
    );
    assert!(!t.is_down());
}

#[test]
fn a_duplicate_fn_edge_is_absorbed() {
    let mut t = Translator::new(Trigger::Fn);
    t.translate(&flags(Modifiers::FN, true, Modifiers::FN));
    assert_eq!(
        t.translate(&flags(Modifiers::FN, true, Modifiers::FN)),
        Decision::Nothing
    );
}

#[test]
fn shift_with_fn_held_is_not_an_input() {
    let mut t = Translator::new(Trigger::Fn);
    t.translate(&flags(Modifiers::FN, true, Modifiers::FN));
    let shift = Modifiers::FN | Modifiers::SHIFT_LEFT;
    assert_eq!(
        t.translate(&flags(shift, true, Modifiers::SHIFT_LEFT)),
        Decision::Nothing
    );
    assert_eq!(
        t.translate(&flags(Modifiers::FN, false, Modifiers::SHIFT_LEFT)),
        Decision::Nothing
    );
    assert!(t.is_down(), "fn is still held");
}

#[test]
fn ctrl_with_fn_passes_through_while_command_mode_is_off() {
    // DEC-31: Ctrl is a modifier, not an input; the take stays dictation.
    let mut t = Translator::new(Trigger::Fn);
    t.translate(&flags(Modifiers::FN, true, Modifiers::FN));
    let ctrl = Modifiers::FN | Modifiers::CTRL_LEFT;
    assert_eq!(
        t.translate(&flags(ctrl, true, Modifiers::CTRL_LEFT)),
        Decision::Nothing
    );
    assert!(t.is_down());
}

#[test]
fn shift_alone_never_reads_as_fn() {
    let mut t = Translator::new(Trigger::Fn);
    assert_eq!(
        t.translate(&flags(Modifiers::SHIFT_LEFT, true, Modifiers::SHIFT_LEFT)),
        Decision::Nothing
    );
    assert!(!t.is_down());
}

#[test]
fn key_downs_are_inputs_and_key_ups_are_not() {
    let mut t = Translator::new(Trigger::Fn);
    assert_eq!(
        t.translate(&key(Modifiers::FN, HkKey::LeftArrow, true)),
        input(Input::KeyDown(Key::Other(key_name(HkKey::LeftArrow))))
    );
    assert_eq!(
        t.translate(&key(Modifiers::FN, HkKey::LeftArrow, false)),
        Decision::Nothing
    );
    assert_eq!(
        t.translate(&key(Modifiers::FN, HkKey::Space, true)),
        input(Input::KeyDown(Key::Space))
    );
    assert_eq!(
        t.translate(&key(Modifiers::empty(), HkKey::Escape, true)),
        input(Input::KeyDown(Key::Escape))
    );
}

#[test]
fn a_mouse_click_with_fn_is_a_navigation_key() {
    // OT2-A: a click into another app or window while holding fn.
    let mut t = Translator::new(Trigger::Fn);
    assert_eq!(
        t.translate(&key(Modifiers::FN, HkKey::MouseLeft, true)),
        input(Input::KeyDown(Key::Nav("MouseLeft".into())))
    );
    assert_eq!(
        t.translate(&key(Modifiers::FN, HkKey::MouseRight, true)),
        input(Input::KeyDown(Key::Nav("MouseRight".into())))
    );
}

#[test]
fn app_window_tab_and_space_switches_are_navigation_keys() {
    let mut t = Translator::new(Trigger::Fn);
    let nav = |m: Modifiers, k: HkKey| {
        Decision::Input(Input::KeyDown(Key::Nav(key_name(k))))
            == Translator::new(Trigger::Fn).translate(&key(m, k, true))
    };
    // ⌘-Tab, ⌘⇧-Tab, ⌘-`, ⌘-1, ⌃-Tab, ⌃-→ (Spaces), ⌘⌥-→ (browser tabs).
    assert!(nav(Modifiers::FN | Modifiers::CMD_LEFT, HkKey::Tab));
    assert!(nav(
        Modifiers::FN | Modifiers::CMD_LEFT | Modifiers::SHIFT_LEFT,
        HkKey::Tab
    ));
    assert!(nav(Modifiers::FN | Modifiers::CMD_RIGHT, HkKey::Grave));
    assert!(nav(Modifiers::FN | Modifiers::CMD_LEFT, HkKey::Num1));
    assert!(nav(Modifiers::FN | Modifiers::CTRL_LEFT, HkKey::Tab));
    assert!(nav(Modifiers::FN | Modifiers::CTRL_LEFT, HkKey::RightArrow));
    assert!(nav(
        Modifiers::FN | Modifiers::CMD_LEFT | Modifiers::OPT_LEFT,
        HkKey::RightArrow
    ));
    // Plain fn+key, Shift or Option combinations stay chords.
    for (m, k) in [
        (Modifiers::FN, HkKey::F),
        (Modifiers::FN, HkKey::LeftArrow),
        (Modifiers::FN | Modifiers::SHIFT_LEFT, HkKey::A),
        (Modifiers::FN | Modifiers::OPT_LEFT, HkKey::Tab),
    ] {
        assert_eq!(
            t.translate(&key(m, k, true)),
            input(Input::KeyDown(Key::Other(key_name(k)))),
            "{m:?} {k:?}"
        );
    }
    // ⌘ with Space and Esc keep their own meaning.
    assert_eq!(
        t.translate(&key(
            Modifiers::FN | Modifiers::CMD_LEFT,
            HkKey::Escape,
            true
        )),
        input(Input::KeyDown(Key::Escape))
    );
}

#[test]
fn a_modifier_pair_trigger_does_not_count_its_own_ctrl() {
    // Ctrl+Opt held: Ctrl is the trigger's, so Ctrl+Opt+A is a chord; ⌘-Tab
    // with the pair held still switches apps.
    let mut t = Translator::new(Trigger::CTRL_OPT);
    let co = Modifiers::CTRL_LEFT | Modifiers::OPT_LEFT;
    assert_eq!(
        t.translate(&key(co, HkKey::A, true)),
        input(Input::KeyDown(Key::Other("A".into())))
    );
    assert_eq!(
        t.translate(&key(co | Modifiers::CMD_LEFT, HkKey::Tab, true)),
        input(Input::KeyDown(Key::Nav(key_name(HkKey::Tab))))
    );
}

#[test]
fn ctrl_opt_goes_down_when_both_are_held_either_order_either_side() {
    for (first, second) in [
        (Modifiers::CTRL_LEFT, Modifiers::OPT_LEFT),
        (Modifiers::OPT_RIGHT, Modifiers::CTRL_LEFT),
        (Modifiers::CTRL_RIGHT, Modifiers::OPT_RIGHT),
    ] {
        let mut t = Translator::new(Trigger::CTRL_OPT);
        assert_eq!(t.translate(&flags(first, true, first)), Decision::Nothing);
        assert_eq!(
            t.translate(&flags(first | second, true, second)),
            input(Input::FnDown)
        );
        assert_eq!(
            t.translate(&flags(second, false, first)),
            input(Input::FnUp)
        );
    }
}

#[test]
fn ctrl_opt_ignores_a_third_modifier() {
    let mut t = Translator::new(Trigger::CTRL_OPT);
    let co = Modifiers::CTRL_LEFT | Modifiers::OPT_LEFT;
    t.translate(&flags(co, true, Modifiers::OPT_LEFT));
    assert_eq!(
        t.translate(&flags(co | Modifiers::CMD_LEFT, true, Modifiers::CMD_LEFT)),
        Decision::Nothing
    );
    assert_eq!(
        t.translate(&flags(co, false, Modifiers::CMD_LEFT)),
        Decision::Nothing
    );
    assert!(t.is_down());
}

#[test]
fn ctrl_opt_does_not_react_to_fn() {
    let mut t = Translator::new(Trigger::CTRL_OPT);
    assert_eq!(
        t.translate(&flags(Modifiers::FN, true, Modifiers::FN)),
        Decision::Nothing
    );
}

#[test]
fn triggers_from_hotkeys() {
    let h = |s: &str| s.parse::<Hotkey>().unwrap();
    assert_eq!(Trigger::from_hotkey(&h("fn")), Some(Trigger::Fn));
    assert_eq!(
        Trigger::from_hotkey(&h("ctrl+option")),
        Some(Trigger::CTRL_OPT)
    );
    assert_eq!(
        Trigger::from_hotkey(&h("option+ctrl")),
        Some(Trigger::CTRL_OPT)
    );
    assert_eq!(
        Trigger::from_hotkey(&h("option")),
        None,
        "one modifier is not a trigger"
    );
    assert_eq!(Trigger::from_hotkey(&h("fn+space")), None, "keyed");
    assert_eq!(
        Trigger::from_hotkey(&h("fn+shift")),
        None,
        "fn with another modifier"
    );
}

#[test]
fn typing_is_never_logged_by_name() {
    assert_eq!(logged_key_name(HkKey::A, false), "(typing)");
    assert_eq!(logged_key_name(HkKey::Num1, false), "(typing)");
    assert_eq!(logged_key_name(HkKey::A, true), "A");
    assert_eq!(logged_key_name(HkKey::Escape, false), "Escape");
    assert_eq!(logged_key_name(HkKey::F11, false), "F11");
}

#[test]
fn stuck_guard_releases_after_one_second_of_disagreement() {
    let mut g = StuckGuard::default();
    assert!(!g.observe(0, true, Some(false)));
    assert!(!g.observe(999, true, Some(false)));
    assert!(g.observe(1000, true, Some(false)));
    // Re-arms after a release.
    assert!(!g.observe(1250, true, Some(false)));
}

#[test]
fn stuck_guard_resets_when_the_system_agrees_again() {
    let mut g = StuckGuard::default();
    g.observe(0, true, Some(false));
    assert!(!g.observe(600, true, Some(true)));
    assert!(!g.observe(1200, true, Some(false)));
    assert!(!g.observe(2100, true, Some(false)));
    assert!(g.observe(2200, true, Some(false)));
}

#[test]
fn stuck_guard_never_fires_without_system_state_or_with_fn_up() {
    let mut g = StuckGuard::default();
    for t in (0..5000).step_by(250) {
        assert!(!g.observe(t, true, None));
        assert!(!g.observe(t, false, Some(false)));
    }
}

#[test]
fn modifier_key_combos_are_triggers_but_shift_or_fn_combos_and_esc_are_not() {
    let t = |s: &str| Trigger::from_hotkey(&s.parse::<Hotkey>().unwrap());
    assert_eq!(
        t("option+space"),
        Some(Trigger::Combo(Modifiers::OPT, HkKey::Space))
    );
    assert_eq!(
        t("ctrl+option+d"),
        Some(Trigger::Combo(Modifiers::CTRL | Modifiers::OPT, HkKey::D))
    );
    assert!(t("command+shift+k").is_some());
    assert_eq!(t("shift+a"), None);
    assert_eq!(t("fn+space"), None);
    assert_eq!(t("option+escape"), None);
    assert_eq!(t("space"), None);
}

#[test]
fn a_combo_trigger_goes_down_on_its_key_and_up_on_key_or_modifier_release() {
    let combo = Trigger::Combo(Modifiers::OPT, HkKey::Space);
    let mut t = Translator::new(combo);
    let opt = Modifiers::OPT_RIGHT;
    assert_eq!(t.translate(&flags(opt, true, opt)), Decision::Nothing);
    assert_eq!(
        t.translate(&key(opt, HkKey::Space, true)),
        input(Input::FnDown)
    );
    assert_eq!(
        t.translate(&key(opt, HkKey::Space, true)),
        Decision::Nothing
    );
    assert!(matches!(
        t.translate(&key(opt, HkKey::A, true)),
        Decision::Input(Input::KeyDown(_))
    ));
    assert_eq!(
        t.translate(&key(opt, HkKey::Space, false)),
        input(Input::FnUp)
    );
    t.translate(&key(opt, HkKey::Space, true));
    assert_eq!(
        t.translate(&flags(Modifiers::empty(), false, opt)),
        input(Input::FnUp)
    );
    // Space with Shift too is a key, not the trigger.
    assert!(matches!(
        t.translate(&key(opt | Modifiers::SHIFT_LEFT, HkKey::Space, true)),
        Decision::Input(Input::KeyDown(Key::Space))
    ));
}
