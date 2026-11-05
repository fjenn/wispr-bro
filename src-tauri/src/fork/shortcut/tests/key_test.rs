// SPDX-License-Identifier: GPL-3.0-or-later
//! S-29a: onboarding's shortcut test lights the key for the trigger only,
//! and the test mode expires on its own.

use handy_keys::Modifiers as M;

use super::super::decision::Trigger;
use super::super::{key_test_active, trigger_held, KEY_TEST_TTL_MS};

#[test]
fn fn_lights_only_with_fn_held() {
    assert!(trigger_held(Trigger::Fn, M::FN));
    assert!(trigger_held(Trigger::Fn, M::FN | M::SHIFT));
    assert!(!trigger_held(Trigger::Fn, M::CTRL | M::OPT));
    assert!(!trigger_held(Trigger::Fn, M::empty()));
}

#[test]
fn the_fallback_needs_both_modifiers_from_either_side() {
    let fallback = Trigger::Mods(M::CTRL | M::OPT);
    assert!(trigger_held(fallback, M::CTRL | M::OPT));
    assert!(trigger_held(fallback, M::CTRL_LEFT | M::OPT_RIGHT));
    assert!(!trigger_held(fallback, M::CTRL));
    assert!(!trigger_held(fallback, M::OPT | M::CMD));
}

#[test]
fn the_test_mode_expires() {
    assert!(key_test_active(1_000 + KEY_TEST_TTL_MS, 1_000));
    assert!(!key_test_active(1_000, 1_000));
    assert!(!key_test_active(0, 5));
}

#[test]
fn a_combo_lights_only_while_its_key_is_held_with_its_modifiers() {
    use super::super::key_test_down;
    use handy_keys::{Key, KeyEvent};
    let combo = Trigger::Combo(M::OPT, Key::Space);
    let ev = |modifiers, key, down| KeyEvent {
        modifiers,
        key,
        is_key_down: down,
        changed_modifier: None,
    };
    assert!(!key_test_down(combo, false, &ev(M::OPT_LEFT, None, true)));
    assert!(key_test_down(
        combo,
        false,
        &ev(M::OPT_LEFT, Some(Key::Space), true)
    ));
    assert!(key_test_down(
        combo,
        true,
        &ev(M::OPT_LEFT, Some(Key::A), true)
    ));
    assert!(!key_test_down(
        combo,
        true,
        &ev(M::OPT_LEFT, Some(Key::Space), false)
    ));
    assert!(!key_test_down(combo, true, &ev(M::empty(), None, false)));
}
