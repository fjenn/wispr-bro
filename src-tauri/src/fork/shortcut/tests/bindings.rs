// SPDX-License-Identifier: GPL-3.0-or-later
//! Bindings → profile: defaults, fallback, unset, per-binding activation,
//! ≤ 4 per action, conflicts and unsupported bindings.

use std::collections::BTreeMap;

use crate::fork::settings::{
    default_hotkeys, validate_bindings, HotkeyActivation, HotkeyBinding, MAX_BINDINGS_PER_ACTION,
};
use crate::fork::shortcut::bindings::*;
use crate::fork::shortcut::decision::Trigger;

fn b(chord: &str, activation: HotkeyActivation) -> HotkeyBinding {
    HotkeyBinding {
        chord: chord.into(),
        activation,
    }
}

use HotkeyActivation::*;

#[test]
fn the_default_bindings_compile_to_the_fn_grammar() {
    let p = compile(&default_hotkeys(), |_| false);
    assert_eq!(p.trigger, Some(Trigger::Fn));
    assert!(p.hold && p.double_tap && p.trigger_space && p.esc_cancels);
    assert!(p.keyed.is_empty());
    // Keyed actions with no handler (S-06 registers them in the app):
    // reported, not registered.
    let no_handler = |action: &str, chord: &str| Issue::Unsupported {
        action: action.into(),
        chord: chord.into(),
        reason: "no handler for this action yet".into(),
    };
    assert_eq!(
        p.issues,
        vec![
            no_handler("copy_last", "ctrl+command+c"),
            no_handler("paste_last", "ctrl+command+v"),
            no_handler("transform.polish", "option+1"),
            no_handler("transform.prompt_engineer", "option+2"),
            no_handler("transform.view_changes", "option+o"),
        ]
    );
}

#[test]
fn a_handled_keyed_action_is_registered() {
    let p = compile(&default_hotkeys(), |id| id == "paste_last");
    assert_eq!(
        p.issues.len(),
        4,
        "copy_last and disabled transforms have no handler here"
    );
    assert_eq!(p.keyed.len(), 1);
    assert_eq!(p.keyed[0].action, "paste_last");
    assert_eq!(p.keyed[0].hotkey, "ctrl+command+v".parse().unwrap());
}

// S-06 ------------------------------------------------------------------

#[test]
fn the_recovery_defaults_paste_and_copy_and_leave_the_rest_unset() {
    let map = default_hotkeys();
    assert_eq!(map["paste_last"], vec![b("ctrl+command+v", Press)]);
    assert_eq!(map["copy_last"], vec![b("ctrl+command+c", Press)]);
    assert!(!map.contains_key("press_enter"));
    assert!(!map.contains_key("undo_cancel"));
    let p = compile(&map, |_| true);
    assert!(p.issues.is_empty(), "{:?}", p.issues);
    let mut actions: Vec<_> = p.keyed.iter().map(|k| k.action.as_str()).collect();
    actions.sort();
    assert_eq!(
        actions,
        [
            "copy_last",
            "paste_last",
            "transform.polish",
            "transform.prompt_engineer",
            "transform.view_changes"
        ]
    );
}

#[test]
fn press_enter_takes_a_key_or_a_mouse_button_but_never_return() {
    let with = |chord: &str| {
        let mut map = default_hotkeys();
        map.insert("press_enter".into(), vec![b(chord, Press)]);
        compile(&map, |_| true)
    };
    for chord in [
        "mousex1",
        "mousex2",
        "mousemiddle",
        "option+mousex1",
        "f13",
        "ctrl+option+return",
    ] {
        let p = with(chord);
        let keyed = p.keyed.iter().find(|k| k.action == "press_enter");
        if chord.ends_with("return") {
            assert!(keyed.is_none(), "{chord}");
            assert!(matches!(
                &p.issues[..],
                [Issue::Unsupported { action, .. }] if action == "press_enter"
            ));
        } else {
            let keyed = keyed.unwrap_or_else(|| panic!("{chord}: {:?}", p.issues));
            assert_eq!(keyed.hotkey, normalize(chord).unwrap().1);
        }
    }
    for chord in ["return", "enter", "keypadenter"] {
        let p = with(chord);
        assert!(p.keyed.iter().all(|k| k.action != "press_enter"), "{chord}");
        assert_eq!(p.issues.len(), 1, "{chord}");
    }
    // The recorder's side-specific spelling normalises.
    assert_eq!(
        normalize("option_left+mousex1").unwrap().0,
        "option+mousex1"
    );
}

#[test]
fn undo_cancel_can_be_bound_like_any_keyed_action() {
    let mut map = default_hotkeys();
    map.insert("undo_cancel".into(), vec![b("ctrl+option+z", Press)]);
    let p = compile(&map, |_| true);
    assert!(p.keyed.iter().any(|k| k.action == "undo_cancel"));
    // The same chord as paste-last is a conflict.
    map.insert("undo_cancel".into(), vec![b("ctrl+command+v", Press)]);
    let p = compile(&map, |_| true);
    assert!(p
        .issues
        .iter()
        .any(|i| matches!(i, Issue::Conflict { action, .. } if action == "undo_cancel" || action == "paste_last")));
}

#[test]
fn the_fallback_profile_runs_on_ctrl_opt() {
    let p = compile(&fallback_hotkeys(), |_| false);
    assert_eq!(p.trigger, Some(Trigger::CTRL_OPT));
    assert!(p.hold && p.double_tap && p.trigger_space && p.esc_cancels);
}

#[test]
fn the_fallback_is_not_the_default() {
    assert_ne!(fallback_hotkeys(), default_hotkeys());
    assert_eq!(default_hotkeys()["dictate"], vec![b("fn", Hold)]);
}

#[test]
fn every_action_may_be_unset() {
    let empty: BTreeMap<String, Vec<HotkeyBinding>> =
        default_hotkeys().into_keys().map(|k| (k, vec![])).collect();
    let p = compile(&empty, |_| true);
    assert_eq!(p, Profile::default());
}

#[test]
fn hands_free_alone_still_gives_a_trigger() {
    let mut map = default_hotkeys();
    map.insert("dictate".into(), vec![]);
    let p = compile(&map, |_| false);
    assert_eq!(p.trigger, Some(Trigger::Fn));
    assert!(!p.hold);
    assert!(p.double_tap);
}

#[test]
fn the_same_chord_in_two_actions_conflicts() {
    let mut map = default_hotkeys();
    map.insert("paste_last".into(), vec![b("escape", Press)]);
    let p = compile(&map, |_| true);
    assert!(p.issues.contains(&Issue::Conflict {
        action: "paste_last".into(),
        other: "cancel".into(),
        chord: "escape".into(),
    }));
}

#[test]
fn hold_and_double_tap_on_one_trigger_do_not_conflict() {
    let p = compile(&default_hotkeys(), |_| false);
    assert!(!p.issues.iter().any(|i| matches!(i, Issue::Conflict { .. })));
}

#[test]
fn chord_spelling_does_not_hide_a_conflict() {
    let mut map = default_hotkeys();
    map.insert("transform.shout".into(), vec![b("command+ctrl+v", Press)]);
    map.insert("paste_last".into(), vec![b("ctrl+command+v", Press)]);
    let p = compile(&map, |_| true);
    assert_eq!(
        p.issues
            .iter()
            .filter(|i| matches!(i, Issue::Conflict { .. }))
            .count(),
        1
    );
}

#[test]
fn a_second_hold_trigger_is_unsupported() {
    let mut map = default_hotkeys();
    map.insert(
        "dictate".into(),
        vec![b("fn", Hold), b("ctrl+option", Hold)],
    );
    let p = compile(&map, |_| false);
    assert_eq!(p.trigger, Some(Trigger::Fn));
    assert!(p.issues.iter().any(|i| matches!(
        i,
        Issue::Unsupported { chord, .. } if chord == "ctrl+option"
    )));
}

#[test]
fn hold_on_a_keyed_chord_is_unsupported() {
    let mut map = default_hotkeys();
    map.insert("dictate".into(), vec![b("ctrl+d", Hold)]);
    let p = compile(&map, |_| false);
    assert_eq!(
        p.trigger,
        Some(Trigger::Fn),
        "hands-free double-tap remains"
    );
    assert!(!p.hold);
    assert!(p.issues.iter().any(|i| matches!(
        i,
        Issue::Unsupported { action, .. } if action == "dictate"
    )));
}

#[test]
fn a_single_modifier_cannot_be_the_trigger() {
    let mut map = default_hotkeys();
    map.insert("dictate".into(), vec![b("option", Hold)]);
    map.insert("hands_free".into(), vec![]);
    let p = compile(&map, |_| false);
    assert_eq!(p.trigger, None);
}

#[test]
fn an_invalid_chord_is_reported() {
    let mut map = default_hotkeys();
    map.insert("paste_last".into(), vec![b("ctrl+nosuchkey", Press)]);
    let p = compile(&map, |_| true);
    assert!(p.issues.contains(&Issue::Invalid {
        action: "paste_last".into(),
        chord: "ctrl+nosuchkey".into(),
    }));
}

#[test]
fn per_binding_activation_matters() {
    // fn+space as a double-tap is not the lock chord.
    let mut map = default_hotkeys();
    map.insert("hands_free".into(), vec![b("fn+space", DoubleTap)]);
    let p = compile(&map, |_| false);
    assert!(!p.trigger_space);
    assert!(!p.double_tap);
}

#[test]
fn at_most_four_bindings_per_action() {
    let four: Vec<_> = ["a", "b", "c", "d"]
        .iter()
        .map(|k| b(&format!("ctrl+{k}"), Press))
        .collect();
    assert!(validate_bindings("paste_last", &four).is_ok());
    let mut five = four.clone();
    five.push(b("ctrl+e", Press));
    assert_eq!(MAX_BINDINGS_PER_ACTION, 4);
    assert!(validate_bindings("paste_last", &five).is_err());
}

#[test]
fn normalize_gives_one_spelling() {
    assert_eq!(normalize("Option+Ctrl").unwrap().0, "ctrl+option");
    assert_eq!(normalize("fn+Space").unwrap().0, "fn+space");
    assert_eq!(normalize("command+ctrl+v").unwrap().0, "ctrl+command+v");
    assert_eq!(normalize("escape").unwrap().0, "escape");
    assert!(normalize("").is_none());
}
