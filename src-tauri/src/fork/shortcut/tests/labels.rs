// SPDX-License-Identifier: GPL-3.0-or-later
//! Layout-aware labels: US, AZERTY, QWERTZ, Dvorak.

use std::collections::HashMap;

use crate::fork::shortcut::labels::*;

/// A layout from (keycode, character) overrides on top of US ANSI.
struct Fake(HashMap<u16, char>);

impl Layout for Fake {
    fn char_for(&self, keycode: u16) -> Option<char> {
        self.0
            .get(&keycode)
            .copied()
            .or_else(|| Ansi.char_for(keycode))
    }
}

fn azerty() -> Fake {
    // French AZERTY: A↔Q, Z↔W, M on the semicolon key, comma on M.
    Fake(HashMap::from([
        (0, 'q'),
        (12, 'a'),
        (13, 'z'),
        (6, 'w'),
        (41, 'm'),
        (46, ','),
    ]))
}

fn qwertz() -> Fake {
    // German QWERTZ: Y↔Z.
    Fake(HashMap::from([(6, 'y'), (16, 'z')]))
}

fn dvorak() -> Fake {
    // US Dvorak, the keys used below.
    Fake(HashMap::from([
        (9, 'k'),
        (8, 'j'),
        (0, 'a'),
        (1, 'o'),
        (12, '\''),
        (16, 'f'),
        (35, 'l'),
    ]))
}

fn text(chord: &str, layout: &dyn Layout) -> String {
    label(chord, layout, "en").unwrap().text
}

#[test]
fn modifier_glyphs_in_apple_order() {
    assert_eq!(text("command+ctrl+v", &Ansi), "⌃⌘V");
    assert_eq!(text("shift+option+ctrl+command+k", &Ansi), "⌃⌥⇧⌘K");
    assert_eq!(text("ctrl+option", &Ansi), "⌃⌥");
}

#[test]
fn fn_and_space_read_as_words() {
    assert_eq!(text("fn", &Ansi), "fn");
    assert_eq!(text("fn+space", &Ansi), "fn Space");
    assert_eq!(text("ctrl+option+space", &Ansi), "⌃⌥ Space");
    assert_eq!(text("escape", &Ansi), "esc");
}

#[test]
fn named_keys_use_glyphs() {
    assert_eq!(text("fn+leftarrow", &Ansi), "fn ←");
    assert_eq!(text("fn+delete", &Ansi), "fn ⌫");
    assert_eq!(text("command+return", &Ansi), "⌘↩");
    assert_eq!(text("fn+f11", &Ansi), "fn F11");
}

#[test]
fn azerty_labels_follow_the_layout() {
    let l = azerty();
    assert_eq!(text("command+q", &l), "⌘A");
    assert_eq!(text("command+a", &l), "⌘Q");
    assert_eq!(text("ctrl+z", &l), "⌃W");
    assert_eq!(text("ctrl+semicolon", &l), "⌃M");
    assert_eq!(text("ctrl+v", &l), "⌃V", "unchanged keys keep their name");
}

#[test]
fn qwertz_swaps_y_and_z() {
    let l = qwertz();
    assert_eq!(text("command+z", &l), "⌘Y");
    assert_eq!(text("command+y", &l), "⌘Z");
}

#[test]
fn dvorak_labels_follow_the_layout() {
    let l = dvorak();
    assert_eq!(text("ctrl+command+v", &l), "⌃⌘K");
    assert_eq!(text("command+c", &l), "⌘J");
    assert_eq!(text("command+s", &l), "⌘O");
    assert_eq!(text("command+p", &l), "⌘L");
}

#[test]
fn spoken_form_for_screen_readers() {
    let s = |c: &str| label(c, &Ansi, "en").unwrap().spoken;
    assert_eq!(s("command+ctrl+v"), "Control Command V");
    assert_eq!(s("fn+space"), "fn Space");
    assert_eq!(s("ctrl+option"), "Control Option");
    assert_eq!(s("escape"), "Escape");
    assert_eq!(s("fn+leftarrow"), "fn Left Arrow");
    assert_eq!(
        label("command+q", &azerty(), "en").unwrap().spoken,
        "Command A"
    );
}

#[test]
fn every_layout_key_has_a_keycode() {
    for k in [
        "a", "q", "z", "m", "1", "0", "minus", "slash", "grave", "section",
    ] {
        let key: handy_keys::Key = k.parse().unwrap();
        assert!(keycode(key).is_some(), "{k}");
    }
    assert_eq!(keycode(handy_keys::Key::F5), None);
}

#[test]
fn an_unknown_chord_has_no_label() {
    assert!(label("ctrl+nosuchkey", &Ansi, "en").is_none());
}

#[test]
fn labels_use_the_locale_strings() {
    // Locales without a translation fall back to English.
    let l = label("fn+space", &Ansi, "de").unwrap();
    assert!(l.text.starts_with("fn "));
    assert!(!l.spoken.contains("rust.hotkeys"));
}

#[test]
fn status_lists_every_binding_with_its_label_and_the_skipped_ones() {
    use crate::fork::settings::default_hotkeys;
    use crate::fork::shortcut::bindings::compile;
    use crate::fork::shortcut::blocking::GlobeUsage;
    let map = default_hotkeys();
    let profile = compile(&map, |_| false);
    let mut flat = Vec::new();
    let mut actions = Vec::new();
    for (a, list) in &map {
        for b in list {
            flat.push(b.clone());
            actions.push(a.clone());
        }
    }
    let s = crate::fork::shortcut::status_for_test(
        &profile,
        GlobeUsage::DoNothing,
        &flat,
        &actions,
        &dvorak(),
    );
    assert_eq!(s.trigger.as_deref(), Some("fn"));
    assert_eq!(s.globe, "do_nothing");
    assert!(!s.fn_blocked);
    assert_eq!(
        s.issues.len(),
        6,
        "Command Mode, recovery and disabled transform actions have no handler here"
    );
    let texts: Vec<_> = s
        .bindings
        .iter()
        .map(|b| (b.action.as_str(), b.label.text.as_str()))
        .collect();
    assert!(texts.contains(&("dictate", "fn")));
    assert!(texts.contains(&("hands_free", "fn Space")));
    assert!(texts.contains(&("cancel", "esc")));
    assert!(texts.contains(&("paste_last", "⌃⌘K")), "Dvorak: {texts:?}");
    let blocked = crate::fork::shortcut::status_for_test(
        &profile,
        GlobeUsage::Dictation,
        &flat,
        &actions,
        &Ansi,
    );
    assert!(blocked.fn_blocked);
}

#[test]
fn mouse_buttons_have_spoken_names() {
    // S-06: Press Enter may be a mouse button.
    for (chord, text) in [
        ("mousemiddle", "Middle mouse button"),
        ("mousex1", "Back mouse button"),
        ("mousex2", "Forward mouse button"),
        ("option+mousex1", "⌥ Back mouse button"),
    ] {
        let l = crate::fork::shortcut::labels::label(chord, &Ansi, "en").unwrap();
        assert_eq!(l.text, text, "{chord}");
    }
    let l = crate::fork::shortcut::labels::label("option+mousex2", &Ansi, "en").unwrap();
    assert_eq!(l.spoken, "Option Forward mouse button");
}
