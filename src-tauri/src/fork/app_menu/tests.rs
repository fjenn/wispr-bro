// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

fn sample(locale: &str) -> MenuSnapshot {
    MenuSnapshot {
        locale: locale.into(),
        total_words: 12,
        dictionary_words: 3,
        snippets: 2,
        last: Some(LastMenuTranscript {
            preview: Some("A short dictation".into()),
            concealed: false,
        }),
        update_checks_enabled: true,
        ..MenuSnapshot::default()
    }
}

fn submenu_items(spec: &MenuSpec) -> &[MenuSpec] {
    match spec {
        MenuSpec::Submenu { items, .. } => items,
        _ => panic!("expected a submenu"),
    }
}

fn id(spec: &MenuSpec) -> &str {
    match spec {
        MenuSpec::Item { id, .. } | MenuSpec::Submenu { id, .. } => id,
        MenuSpec::Separator => "separator",
    }
}

fn ids(specs: &[MenuSpec]) -> Vec<&str> {
    specs.iter().map(id).collect()
}

fn item<'a>(specs: &'a [MenuSpec], wanted: &str) -> &'a MenuSpec {
    specs
        .iter()
        .find(|spec| id(spec) == wanted)
        .unwrap_or_else(|| panic!("missing {wanted}"))
}

#[test]
fn app_menu_structure_snapshot() {
    let snapshot = sample("en");
    assert_eq!(
        ids(submenu_items(&dictation_spec(&snapshot))),
        [
            "fork.dictation.words",
            COPY_LAST_ID,
            "separator",
            "fork.dictation.microphone",
            "fork.dictation.languages",
        ]
    );
    assert_eq!(
        ids(submenu_items(&vocabulary_spec(&snapshot))),
        [
            "fork.vocabulary.dictionary_count",
            "fork.vocabulary.snippet_count",
            "separator",
            ADD_WORD_ID,
            NEW_SNIPPET_ID,
        ]
    );
    assert_eq!(
        ids(submenu_items(&help_spec(&snapshot))),
        [
            DOCUMENTATION_ID,
            "separator",
            SHORTCUTS_ID,
            HELP_MICROPHONE_ID,
            HELP_LANGUAGES_ID,
            "separator",
            REPORT_PROBLEM_ID,
            SEND_FEEDBACK_ID,
        ]
    );
    // Menu::default supplies the predefined Edit/Window menus and keyboard
    // actions; the installer changes only their labels.
    let source = include_str!("../app_menu.rs");
    assert!(source.contains("Menu::default(app)?"));
    for action in ["undo", "redo", "cut", "copy", "paste"] {
        assert!(source.contains(&format!("\"rust.menu.{action}\"")));
    }
    assert!(source.contains("help.set_as_help_menu_for_nsapp()?"));
}

#[test]
fn tray_structure_has_no_model_or_old_copy_items() {
    let specs = tray_specs(&sample("en"), false, false);
    assert_eq!(
        ids(&specs),
        [
            TRAY_OPEN_ID,
            TRAY_UPDATES_ID,
            PASTE_LAST_ID,
            "separator",
            SHORTCUTS_ID,
            "fork.tray.microphone",
            "fork.tray.languages",
            "separator",
            DOCUMENTATION_ID,
            REPORT_PROBLEM_ID,
            SEND_FEEDBACK_ID,
            "separator",
            TRAY_QUIT_ID,
        ]
    );
    let all = format!("{specs:?}");
    for absent in [
        "model_submenu",
        "unload_model",
        "copy_last_transcript",
        "version",
    ] {
        assert!(!all.contains(absent), "{absent}");
    }
}

#[test]
fn disabled_states_follow_transcript_and_update_availability() {
    let mut snapshot = sample("en");
    snapshot.last = None;
    snapshot.update_checks_enabled = false;
    let dictation = dictation_spec(&snapshot);
    let app = submenu_items(&dictation);
    assert!(matches!(
        item(app, COPY_LAST_ID),
        MenuSpec::Item { enabled: false, .. }
    ));
    let tray = tray_specs(&snapshot, false, false);
    assert!(matches!(
        item(&tray, PASTE_LAST_ID),
        MenuSpec::Item { enabled: false, .. }
    ));
    // S-28: honest instead of greyed out while checks are off.
    assert!(!ids(&tray).contains(&"check_updates"));
    assert!(matches!(
        item(&tray, TRAY_UPDATES_OFF_ID),
        MenuSpec::Item { enabled: true, .. }
    ));

    snapshot.last = sample("en").last;
    snapshot.update_checks_enabled = true;
    assert!(matches!(
        item(&tray_specs(&snapshot, false, false), PASTE_LAST_ID),
        MenuSpec::Item { enabled: true, .. }
    ));
    snapshot.updates_forced_off = true;
    assert!(!ids(&tray_specs(&snapshot, false, false)).contains(&"check_updates"));
}

#[test]
fn counts_and_preview_change_with_the_live_snapshot() {
    let first = dictation_spec(&sample("en"));
    let mut changed = sample("en");
    changed.total_words = 13;
    changed.last = Some(LastMenuTranscript {
        preview: Some("Newer words".into()),
        concealed: false,
    });
    assert_ne!(first, dictation_spec(&changed));
    let rendered = format!("{:?}", dictation_spec(&changed));
    assert!(rendered.contains("13 words dictated"));
    assert!(rendered.contains("Newer words"));

    changed.dictionary_words = 4;
    changed.snippets = 5;
    let vocabulary = format!("{:?}", vocabulary_spec(&changed));
    assert!(vocabulary.contains("4 dictionary words"));
    assert!(vocabulary.contains("5 snippets"));
}

#[test]
fn preview_is_one_line_short_and_never_reveals_a_concealed_take() {
    assert_eq!(preview("  one\n two\tthree "), Some("one two three".into()));
    assert_eq!(preview(" \n\t"), None);
    let long = preview(&"abcdefgh ".repeat(10)).unwrap();
    assert!(long.ends_with('…'));
    assert!(long.chars().count() <= 48);

    let mut snapshot = sample("en");
    snapshot.last = Some(LastMenuTranscript {
        preview: None,
        concealed: true,
    });
    let rendered = format!("{:?}", dictation_spec(&snapshot));
    assert!(rendered.contains("Copy last transcript (text hidden)"));
    assert!(!rendered.contains("A short dictation"));
}

#[test]
fn strings_resolve_in_three_locales() {
    for locale in ["en", "de", "fr"] {
        let snapshot = sample(locale);
        for spec in [
            dictation_spec(&snapshot),
            vocabulary_spec(&snapshot),
            help_spec(&snapshot),
        ] {
            let rendered = format!("{spec:?}");
            assert!(!rendered.contains("rust.menu."), "{locale}: {rendered}");
            assert!(!rendered.contains("{{"), "{locale}: {rendered}");
        }
        assert!(text(locale, "rust.menu.dictation").len() > 2);
    }
}

#[test]
fn languages_show_the_users_choice_and_microphones_the_selected_device() {
    let mut snapshot = sample("en");
    snapshot.languages = ["en", "de"]
        .iter()
        .map(|code| MenuLanguage {
            code: (*code).into(),
            name: language_name(code),
        })
        .collect();
    let spec = language_spec(&snapshot, "languages");
    let items = submenu_items(&spec);
    assert_eq!(
        ids(items),
        [
            "fork.language:en",
            "fork.language:de",
            "separator",
            MORE_LANGUAGES_ID
        ]
    );
    assert!(format!("{items:?}").contains("Deutsch"));
    assert!(items[..2].iter().all(|spec| matches!(
        spec,
        MenuSpec::Item {
            checked: Some(true),
            ..
        }
    )));
    // No languages yet: only the way to Settings.
    snapshot.languages.clear();
    assert_eq!(
        ids(submenu_items(&language_spec(&snapshot, "languages"))),
        [MORE_LANGUAGES_ID]
    );

    let mic = |value: &str, kind| ForkMicEntry {
        value: value.into(),
        name: if value == "auto" {
            String::new()
        } else {
            value.into()
        },
        kind,
        recommended: false,
        wireless: false,
        system_default: false,
        transport: transport::Transport::Unknown,
    };
    snapshot.microphones = vec![
        mic("auto", crate::fork::audio::choice::MicEntryKind::Auto),
        mic("USB Mic", crate::fork::audio::choice::MicEntryKind::Device),
    ];
    snapshot.selected_microphone = "USB Mic".into();
    let spec = microphone_spec(&snapshot, "microphones");
    let microphones = submenu_items(&spec);
    assert!(
        matches!(&microphones[0], MenuSpec::Item { checked: Some(false), label, .. } if label == "Auto-detect (system setting)")
    );
    assert!(matches!(
        microphones[1],
        MenuSpec::Item {
            checked: Some(true),
            ..
        }
    ));
    snapshot.microphones.clear();
    let spec = microphone_spec(&snapshot, "microphones");
    assert!(matches!(
        submenu_items(&spec),
        [MenuSpec::Item { enabled: false, .. }]
    ));
}

#[test]
fn a_language_click_removes_it_but_never_the_last_one() {
    let two = vec!["en".to_string(), "nl".to_string()];
    assert_eq!(toggle_language(&two, "nl"), Some(vec!["en".to_string()]));
    assert_eq!(toggle_language(&two, "fr"), None);
    assert_eq!(toggle_language(&["en".to_string()], "en"), None);
    assert_eq!(toggle_language(&[], "en"), None);
}

#[test]
fn every_catalog_language_has_a_native_name() {
    // Handy's picker offers the speech models' languages (as
    // `languageIdentity` codes); a menu must never show a bare code.
    let catalog: serde_json::Value =
        serde_json::from_str(include_str!("../../catalog/catalog.json")).unwrap();
    let mut missing = std::collections::BTreeSet::new();
    for model in catalog["models"].as_array().unwrap() {
        if model["role"] != "asr" {
            continue;
        }
        for code in model["languages"].as_array().unwrap() {
            let code = crate::catalog::language_identity(code.as_str().unwrap());
            if language_name(&code) == code {
                missing.insert(code);
            }
        }
    }
    assert!(missing.is_empty(), "no native name for {missing:?}");
}

#[test]
fn default_menu_titles_map_to_localized_keys() {
    // muda 0.19.3's macOS titles for Menu::default's predefined items.
    for title in [
        "Undo",
        "Redo",
        "Cut",
        "Copy",
        "Paste",
        "Select All",
        "Minimize",
        "Zoom",
        "Close Window",
        "Toggle Full Screen",
        "Services",
        "Hide Others",
        "About Wispr Bro",
        "Hide Wispr Bro",
        "Quit Wispr Bro",
    ] {
        let key = predefined_key(title).unwrap_or_else(|| panic!("{title}"));
        for locale in ["en", "de", "ja"] {
            let s = app_text(locale, key);
            assert!(!s.is_empty() && !s.contains("{{"), "{locale} {key}");
        }
    }
    assert_eq!(predefined_key("Hide Others"), Some("rust.menu.hideOthers"));
    for title in ["File", "Edit", "View", "Window", "Help"] {
        assert!(submenu_key(title).is_some(), "{title}");
    }
    assert_eq!(app_text("en", "rust.menu.quitApp"), "Quit Wispr Bro");
}

#[test]
fn busy_and_secure_input_keep_their_existing_recovery_actions() {
    let busy_specs = tray_specs(&sample("en"), true, false);
    let busy = ids(&busy_specs);
    assert!(busy.contains(&"cancel"));
    let warned_specs = tray_specs(&sample("en"), false, true);
    let warned = ids(&warned_specs);
    assert_eq!(warned[0], "secure_input_warning");
}

#[test]
fn the_tray_update_item_follows_the_fork_update_state() {
    let mut snapshot = sample("en");
    let tray = tray_specs(&snapshot, false, false);
    assert!(
        matches!(item(&tray, TRAY_UPDATES_ID), MenuSpec::Item { enabled: true, label, .. } if label == "Check for updates…")
    );
    snapshot.update_ready = Some("1.0.1".into());
    let tray = tray_specs(&snapshot, false, false);
    assert!(!ids(&tray).contains(&TRAY_UPDATES_ID));
    assert!(
        matches!(item(&tray, TRAY_RESTART_UPDATE_ID), MenuSpec::Item { enabled: true, label, .. } if label == "Restart to update to 1.0.1")
    );
    snapshot.update_checks_enabled = false;
    let tray = tray_specs(&snapshot, false, false);
    assert!(
        matches!(item(&tray, TRAY_UPDATES_OFF_ID), MenuSpec::Item { label, .. } if label == "Update checks are off…")
    );
    snapshot.updates_forced_off = true;
    let tray = tray_specs(&snapshot, false, false);
    for id in [TRAY_UPDATES_ID, TRAY_UPDATES_OFF_ID, TRAY_RESTART_UPDATE_ID] {
        assert!(!ids(&tray).contains(&id), "{id}");
    }
}

#[test]
fn tray_update_clicks_are_routed_to_fork_update() {
    // Handy's `check_updates` handler only shows the Hub (its checker is
    // not mounted); the check itself must run in `on_menu`.
    let source = include_str!("../app_menu.rs");
    let on_menu = &source[source.find("fn on_menu<").unwrap()..];
    let on_menu = &on_menu[..on_menu.find("\n}\n").unwrap()];
    assert!(on_menu.contains("TRAY_UPDATES_ID => {"));
    assert!(on_menu.contains("update::scheduler::check_now(app)"));
    assert!(on_menu.contains("update::scheduler::restart_now(app)"));
    assert!(
        on_menu.contains("TRAY_UPDATES_OFF_ID => open_hub(app, \"settings\", Some(\"privacy\"))")
    );
    let footer = include_str!("../../../../src/fork/hub/Footer.tsx");
    assert!(!footer.contains("<UpdateChecker"));
    assert!(footer.contains("<UpdateStatus />"));
}

#[test]
fn report_a_problem_has_the_command_slash_key_and_both_menus_carry_it() {
    let expected = if cfg!(target_os = "macos") {
        "Cmd+/"
    } else {
        "Ctrl+/"
    };
    assert_eq!(accelerator(REPORT_PROBLEM_ID), Some(expected));
    assert_eq!(accelerator(SEND_FEEDBACK_ID), None);
    // muda 0.19.3 parses "/" as `Code::Slash` (`accelerator.rs`, `"SLASH" |
    // "/"`); Tauri does not re-export its parser, so MAC-TEST-SCRIPT S-26b.1
    // checks the key on the real menu.
    let snapshot = sample("en");
    for specs in [
        submenu_items(&help_spec(&snapshot)).to_vec(),
        tray_specs(&snapshot, false, false),
    ] {
        for (wanted, label) in [
            (REPORT_PROBLEM_ID, "Report a problem…"),
            (SEND_FEEDBACK_ID, "Send feedback…"),
        ] {
            assert!(matches!(
                item(&specs, wanted),
                MenuSpec::Item { label: l, enabled: true, checked: None, .. } if l == label
            ));
        }
    }
    let source = include_str!("../app_menu.rs");
    assert!(source.contains("REPORT_PROBLEM_ID => super::commands::help::open_issue("));
    assert!(source.contains("SEND_FEEDBACK_ID => super::commands::help::open_issue("));
}
