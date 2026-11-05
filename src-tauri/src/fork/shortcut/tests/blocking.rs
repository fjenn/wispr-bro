// SPDX-License-Identifier: GPL-3.0-or-later
//! The §4.8 blocking set per state, per profile and per Globe setting.

use std::collections::{BTreeMap, HashSet};

use handy_keys::{Hotkey, Key, Modifiers};

use crate::fork::settings::default_hotkeys;
use crate::fork::shortcut::bindings::{compile, fallback_hotkeys, Profile};
use crate::fork::shortcut::blocking::*;
use crate::fork::shortcut::processor::{Blocking, Grammar, Input, Timings};

fn hk(s: &str) -> Hotkey {
    s.parse().unwrap()
}

fn default_profile() -> Profile {
    compile(&default_hotkeys(), |_| false)
}

fn set(items: &[&str]) -> HashSet<Hotkey> {
    items.iter().map(|s| hk(s)).collect()
}

fn esc_set() -> Vec<&'static str> {
    vec!["escape", "fn+escape"]
}

#[test]
fn idle_with_fn_block_holds_only_fn() {
    let b = Blocking {
        fn_press: true,
        ..Default::default()
    };
    assert_eq!(
        hotkeys(&default_profile(), b, Extra::default()),
        set(&["fn"])
    );
}

#[test]
fn idle_without_fn_block_is_empty() {
    assert!(hotkeys(&default_profile(), Blocking::default(), Extra::default()).is_empty());
}

#[test]
fn fn_down_adds_fn_space() {
    let b = Blocking {
        fn_press: false,
        fn_space: true,
        esc: false,
    };
    assert_eq!(
        hotkeys(&default_profile(), b, Extra::default()),
        set(&["fn+space"])
    );
}

#[test]
fn a_take_adds_esc_with_and_without_fn() {
    let b = Blocking {
        esc: true,
        ..Default::default()
    };
    assert_eq!(
        hotkeys(&default_profile(), b, Extra::default()),
        set(&esc_set())
    );
}

#[test]
fn processing_adds_esc_even_with_the_grammar_idle() {
    let got = hotkeys(
        &default_profile(),
        Blocking::default(),
        Extra { busy: true },
    );
    assert_eq!(got, set(&esc_set()));
}

#[test]
fn esc_is_never_blocked_when_cancel_is_unbound() {
    let mut map = default_hotkeys();
    map.insert("cancel".into(), vec![]);
    let p = compile(&map, |_| false);
    let b = Blocking {
        esc: true,
        ..Default::default()
    };
    assert!(hotkeys(&p, b, Extra { busy: true }).is_empty());
}

#[test]
fn ctrl_opt_profile_never_blocks_its_trigger_but_blocks_its_space_and_esc() {
    let p = compile(&fallback_hotkeys(), |_| false);
    let b = Blocking {
        fn_press: true,
        fn_space: true,
        esc: true,
    };
    let got = hotkeys(&p, b, Extra::default());
    assert_eq!(
        got,
        set(&[
            "ctrl+option+space",
            "ctrl+option+escape",
            "escape",
            "fn+escape"
        ])
    );
}

#[test]
fn keyed_bindings_with_a_handler_are_always_blocked() {
    let p = compile(&default_hotkeys(), |id| id == "paste_last");
    let got = hotkeys(&p, Blocking::default(), Extra::default());
    assert_eq!(got, set(&["ctrl+command+v"]));
}

#[test]
fn the_set_follows_the_grammar_through_a_take() {
    let p = default_profile();
    let mut g = Grammar::with_fn_block(Timings::DEFAULT, false);
    let at = |g: &Grammar| hotkeys(&p, g.blocking(), Extra::default());
    assert!(at(&g).is_empty());
    g.step(0, &Input::FnDown);
    assert_eq!(at(&g), set(&["fn+space"]));
    g.advance(150);
    assert_eq!(at(&g), set(&["fn+space", "escape", "fn+escape"]));
    g.step(900, &Input::FnUp);
    assert!(
        at(&g).is_empty(),
        "idle Esc is never swallowed after the take"
    );
}

#[test]
fn globe_values_map_to_the_fn_block_rule() {
    assert_eq!(GlobeUsage::from_value(Some(0)), GlobeUsage::DoNothing);
    assert_eq!(GlobeUsage::from_value(Some(1)), GlobeUsage::InputSource);
    assert_eq!(GlobeUsage::from_value(Some(2)), GlobeUsage::Emoji);
    assert_eq!(GlobeUsage::from_value(Some(3)), GlobeUsage::Dictation);
    assert_eq!(GlobeUsage::from_value(None), GlobeUsage::Default);
    assert_eq!(GlobeUsage::from_value(Some(9)), GlobeUsage::Default);
    // DEC-44 update: {FN} only for Start Dictation.
    let blocked: Vec<_> = [0, 1, 2, 3]
        .into_iter()
        .map(|v| GlobeUsage::from_value(Some(v)).blocks_fn())
        .collect();
    assert_eq!(blocked, vec![false, false, false, true]);
    assert!(!GlobeUsage::Default.blocks_fn());
}

#[test]
fn would_block_mirrors_handy_keys_exact_matching() {
    let s = set(&["fn", "fn+space", "escape"]);
    assert!(would_block(&s, Modifiers::FN, None, true));
    assert!(
        !would_block(&s, Modifiers::FN, None, false),
        "releases pass"
    );
    assert!(!would_block(
        &s,
        Modifiers::FN | Modifiers::SHIFT_LEFT,
        None,
        true
    ));
    assert!(would_block(&s, Modifiers::FN, Some(Key::Space), true));
    assert!(
        !would_block(&s, Modifiers::FN, Some(Key::LeftArrow), true),
        "chords pass"
    );
    assert!(would_block(&s, Modifiers::empty(), Some(Key::Escape), true));
    assert!(!would_block(&s, Modifiers::FN, Some(Key::Escape), true));
}

#[test]
fn no_trigger_blocks_nothing_but_keyed_actions() {
    let mut map: BTreeMap<_, _> = default_hotkeys();
    map.insert("dictate".into(), vec![]);
    map.insert("hands_free".into(), vec![]);
    let p = compile(&map, |_| false);
    assert_eq!(p.trigger, None);
    let b = Blocking {
        fn_press: true,
        fn_space: true,
        esc: true,
    };
    assert_eq!(hotkeys(&p, b, Extra::default()), set(&esc_set()));
}

#[test]
fn reading_the_globe_setting_does_not_panic() {
    // Whatever this machine has; only checks the FFI path.
    let _ = read_globe_usage();
}
