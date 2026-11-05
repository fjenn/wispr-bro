// SPDX-License-Identifier: GPL-3.0-or-later
//! S-02: what Handy's Carbon fallback shadows under Secure Input
//! (`secure_input_bindings`), and the shadow ids it routes back.

use std::collections::HashMap;

use crate::fork::settings::default_hotkeys;
use crate::fork::shortcut::bindings::compile;
use crate::fork::shortcut::secure::*;
use crate::settings::{get_default_settings, ShortcutBinding};

fn handy() -> HashMap<String, ShortcutBinding> {
    let mut b = get_default_settings().bindings;
    b.insert(
        "test".into(),
        ShortcutBinding {
            id: "test".into(),
            name: "Test".into(),
            description: String::new(),
            default_binding: "ctrl+shift+t".into(),
            current_binding: "ctrl+shift+t".into(),
        },
    );
    b
}

fn spec_with_paste_last() -> ShadowSpec {
    let profile = compile(&default_hotkeys(), |id| {
        id == "paste_last" || id == "copy_last"
    });
    ShadowSpec::from_profile(&profile)
}

#[test]
fn plain_handy_builds_shadow_handys_own_bindings() {
    let plan = plan(&handy(), false, Some(&spec_with_paste_last()), true);
    let ids: Vec<&str> = plan.keys().map(String::as_str).collect();
    assert_eq!(
        ids,
        vec![
            "cancel",
            "test",
            "transcribe",
            "transcribe_with_post_process"
        ]
    );
}

#[test]
fn product_builds_drop_fork_owned_ids_and_add_fork_keyed_bindings() {
    let plan = plan(&handy(), true, Some(&spec_with_paste_last()), false);
    for owned in ["transcribe", "transcribe_with_post_process", "cancel"] {
        assert!(!plan.contains_key(owned), "{owned}");
    }
    assert!(plan.contains_key("test"), "Handy's other bindings stay");
    let paste = plan.get("fork:paste_last").expect("⌃⌘V shadowed");
    assert_eq!(paste.id, "fork:paste_last", "the callback id is the key");
    assert_eq!(paste.current_binding, "ctrl+command+v");
    assert_eq!(plan["fork:copy_last"].current_binding, "ctrl+command+c");
    // The binding strings parse the way Handy's fallback parses them.
    for b in plan.values() {
        assert!(
            b.current_binding.parse::<handy_keys::Hotkey>().is_ok(),
            "{}",
            b.current_binding
        );
    }
}

#[test]
fn esc_is_shadowed_only_while_a_take_is_active() {
    let spec = spec_with_paste_last();
    assert!(spec.esc_cancels);
    let idle = plan(&handy(), true, Some(&spec), false);
    assert!(
        !idle.contains_key("fork:cancel"),
        "an idle Esc in a password field is never swallowed"
    );
    let active = plan(&handy(), true, Some(&spec), true);
    assert_eq!(active["fork:cancel"].current_binding, "escape");
}

#[test]
fn without_the_engine_nothing_of_the_fork_is_shadowed() {
    let plan = plan(&handy(), true, None, true);
    assert_eq!(plan.keys().collect::<Vec<_>>(), vec!["test"]);
}

#[test]
fn modifier_only_and_fn_bindings_are_listed_for_handys_planner() {
    // fn+key cannot be expressed through Carbon: Handy's planner marks it
    // uncovered, so the diagnostic lists it. The fork passes it through.
    let mut hotkeys = default_hotkeys();
    hotkeys.insert(
        "paste_last".into(),
        vec![crate::fork::settings::HotkeyBinding {
            chord: "fn+v".into(),
            activation: crate::fork::settings::HotkeyActivation::Press,
        }],
    );
    let spec = ShadowSpec::from_profile(&compile(&hotkeys, |id| id == "paste_last"));
    let plan = plan(&handy(), true, Some(&spec), false);
    assert_eq!(plan["fork:paste_last"].current_binding, "fn+v");
}

#[test]
fn shadow_ids_route_back_to_fork_actions() {
    assert_eq!(shadow_action("fork:cancel"), Some("cancel"));
    assert_eq!(
        shadow_action("fork:transform.polish"),
        Some("transform.polish")
    );
    assert_eq!(shadow_action("cancel"), None);
    assert_eq!(shadow_action("transcribe"), None);
    // Handy's own ids are not taken by the hook.
    assert!(!on_shadow_event("transcribe", true));
    assert!(on_shadow_event("fork:paste_last", false));
}

#[test]
fn the_holder_is_named_only_when_it_means_something() {
    assert_eq!(holder_name(None, Some("Terminal")), Some("Terminal".into()));
    assert_eq!(holder_name(None, Some("loginwindow")), None);
    assert_eq!(holder_name(None, Some("(process no longer running)")), None);
    assert_eq!(holder_name(None, None), None);
    assert_eq!(holder_name(None, Some("  ")), None);
}

#[test]
fn the_hooks_are_in_handys_files() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let secure = std::fs::read_to_string(root.join("secure_input.rs")).unwrap();
    assert!(secure.contains("crate::fork::shortcut::secure_input_bindings(&settings.bindings)"));
    let handler = std::fs::read_to_string(root.join("shortcut/handler.rs")).unwrap();
    assert!(
        handler.contains("crate::fork::shortcut::secure::on_shadow_event(binding_id, is_pressed)")
    );
    let tray = std::fs::read_to_string(root.join("tray.rs")).unwrap();
    assert!(tray.contains("crate::fork::shortcut::health::tray_warning()"));
}
