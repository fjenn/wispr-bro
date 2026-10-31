// SPDX-License-Identifier: GPL-3.0-or-later
//! F-07 tests: defaults, migrations, salvage, validation, product settings
//! (ENFORCED / FIRST_RUN_DEFAULTS) and the commands on a mock Tauri app.

use std::path::Path;

use serde_json::{json, Value};

use super::*;
use crate::settings::{get_default_settings, PostProcessApiFormat, PostProcessProvider};

fn ctx() -> ProductContext {
    ProductContext {
        autostart_allowed: true,
    }
}

/// A fresh Handy settings object as Handy would create it on first launch.
fn fresh_handy() -> AppSettings {
    get_default_settings()
}

/// Simulates a relaunch: both stores go through JSON, the way they are
/// written to and read from disk.
fn relaunch(handy: &AppSettings, fork: &ForkSettings) -> (AppSettings, ForkSettings) {
    let handy: AppSettings =
        serde_json::from_value(serde_json::to_value(handy).unwrap()).expect("Handy round trip");
    let stored = serde_json::to_value(fork).unwrap();
    let (fork, status) = decode_stored(Some(&stored));
    assert_eq!(status, LoadStatus::Loaded);
    (handy, fork)
}

fn provider(id: &str, base_url: &str) -> PostProcessProvider {
    PostProcessProvider {
        id: id.into(),
        label: id.into(),
        base_url: base_url.into(),
        allow_base_url_edit: false,
        models_endpoint: None,
        supports_structured_output: false,
        api_format: PostProcessApiFormat::OpenAi,
    }
}

// --- defaults ---------------------------------------------------------------

#[test]
fn defaults_match_the_product_decisions() {
    let s = ForkSettings::default();
    assert_eq!(s.schema_version, CURRENT_SCHEMA_VERSION);
    assert_eq!(s.cleanup_level, CleanupLevel::Light); // DEC-03
    assert!(s.context_awareness); // DEC-24
    assert!(!s.transforms_enabled && !s.command_mode_enabled); // DEC-31
    assert!(s.bar_always_visible); // DEC-14
    assert_eq!(s.bar_position, BarPosition::Bottom);
    assert_eq!(s.bar_hidden_until_ms, None);
    assert!(!s.live_text_in_pill); // DEC-30
    assert_eq!(s.text_retention_days, None); // DEC-07: text forever
    assert_eq!(s.audio_retention_days, Some(14)); // DEC-07: audio 14 days
    assert!(s.update_checks_enabled); // DEC-14
    assert_eq!(s.update_channel, UpdateChannel::Stable); // DEC-33
    assert!(s.show_in_dock); // DEC-14
    assert_eq!(s.notifications, NotificationSettings::default());
    assert!(s.notifications.system && s.notifications.errors);
    assert_eq!(s.onboarding.status, OnboardingStatus::NotStarted);
    assert!(!s.auto_learn_enabled); // DEC-20
    assert!(!s.vibe_coding.enabled && !s.vibe_coding.setup_completed); // DEC-41
    assert_eq!(s.typing_wpm, None);
    assert_eq!(s.typing_hours_per_day, 2.0); // DEC-50
    assert_eq!(s.greeting_name, None);
    assert!(s.applied_first_run_defaults.is_empty());

    // DEC-09 bindings, each within the per-action limit.
    let fn_hold = HotkeyBinding::new("fn", HotkeyActivation::Hold);
    assert_eq!(s.hotkeys[hotkey_actions::DICTATE], vec![fn_hold]);
    assert_eq!(s.hotkeys[hotkey_actions::HANDS_FREE].len(), 2);
    assert_eq!(s.hotkeys[hotkey_actions::CANCEL][0].chord, "escape");
    for (action, bindings) in &s.hotkeys {
        validate_bindings(action, bindings).unwrap();
    }
}

#[test]
fn an_empty_or_missing_store_gives_defaults() {
    assert_eq!(
        decode_stored(None),
        (ForkSettings::default(), LoadStatus::Fresh)
    );
    let (s, status) = decode_stored(Some(&json!({ "schema_version": 1 })));
    assert_eq!(s, ForkSettings::default());
    assert_eq!(status, LoadStatus::Loaded);
}

#[test]
fn defaults_round_trip_through_json() {
    let value = serde_json::to_value(ForkSettings::default()).unwrap();
    assert_eq!(
        decode_stored(Some(&value)),
        (ForkSettings::default(), LoadStatus::Loaded)
    );
}

// --- migrations -------------------------------------------------------------

#[test]
fn migrates_a_v0_object_keeping_known_fields() {
    let stored = json!({
        "cleanup_level": "medium",
        "greeting_name": "Sam",
        "some_retired_key": true,
    });
    let (s, status) = decode_stored(Some(&stored));
    assert_eq!(status, LoadStatus::Migrated { from: 0 });
    assert_eq!(s.schema_version, CURRENT_SCHEMA_VERSION);
    assert_eq!(s.cleanup_level, CleanupLevel::Medium);
    assert_eq!(s.greeting_name.as_deref(), Some("Sam"));
    // Everything else is default.
    assert_eq!(s.audio_retention_days, Some(14));
}

#[test]
fn migrations_run_in_order_from_the_stored_version() {
    // A future schema 3 with two migrations: v1→v2 renames a key, v2→v3
    // reshapes it. A v1 file must go through both; a v2 file only the last.
    fn v1_to_v2(map: &mut Map<String, Value>) {
        if let Some(v) = map.remove("old_name") {
            map.insert("greeting_name".into(), v);
        }
    }
    fn v2_to_v3(map: &mut Map<String, Value>) {
        if let Some(Value::String(name)) = map.get("greeting_name").cloned() {
            map.insert("greeting_name".into(), Value::String(name.to_uppercase()));
        }
    }
    let migrations = [
        Migration {
            from: 0,
            apply: migrate_v0_to_v1,
        },
        Migration {
            from: 1,
            apply: v1_to_v2,
        },
        Migration {
            from: 2,
            apply: v2_to_v3,
        },
    ];
    let (s, status) = decode_with(
        Some(&json!({ "schema_version": 1, "old_name": "ana" })),
        &migrations,
        3,
    );
    assert_eq!(status, LoadStatus::Migrated { from: 1 });
    assert_eq!(s.schema_version, 3);
    assert_eq!(s.greeting_name.as_deref(), Some("ANA"));

    let (s, _) = decode_with(
        Some(&json!({ "schema_version": 2, "greeting_name": "bo" })),
        &migrations,
        3,
    );
    assert_eq!(s.greeting_name.as_deref(), Some("BO"));
}

#[test]
fn a_newer_schema_keeps_readable_fields_and_asks_for_a_backup() {
    let (s, status) = decode_stored(Some(&json!({
        "schema_version": 99,
        "cleanup_level": "none",
        "field_from_the_future": 1,
    })));
    assert_eq!(status, LoadStatus::Newer { version: 99 });
    assert!(status.needs_backup());
    assert_eq!(s.cleanup_level, CleanupLevel::None);
    assert_eq!(s.schema_version, CURRENT_SCHEMA_VERSION);
}

#[test]
fn every_migration_targets_a_known_version() {
    for (i, m) in MIGRATIONS.iter().enumerate() {
        assert_eq!(m.from as usize, i, "migrations must be contiguous from 0");
    }
    assert_eq!(MIGRATIONS.len() as u32, CURRENT_SCHEMA_VERSION);
}

// --- salvage ----------------------------------------------------------------

#[test]
fn salvage_keeps_valid_fields_and_resets_broken_ones() {
    let stored = json!({
        "schema_version": 1,
        "cleanup_level": 42,               // wrong type
        "bar_position": "sideways",        // unknown variant
        "greeting_name": "Kim",            // fine
        "typing_wpm": 250,                 // fine
        "audio_retention_days": 30,        // fine
        "typing_hours_per_day": 99.0,      // out of range (sanitize)
        "hotkeys": {
            "dictate": [ { "chord": "fn", "activation": "hold" } ],
            "cancel": [                     // 5 bindings: over the limit
                { "chord": "a", "activation": "press" },
                { "chord": "b", "activation": "press" },
                { "chord": "c", "activation": "press" },
                { "chord": "d", "activation": "press" },
                { "chord": "e", "activation": "press" }
            ]
        }
    });
    let (s, status) = decode_stored(Some(&stored));
    let LoadStatus::Salvaged { dropped } = &status else {
        panic!("expected Salvaged, got {status:?}");
    };
    for field in [
        "cleanup_level",
        "bar_position",
        "typing_hours_per_day",
        "hotkeys.cancel",
    ] {
        assert!(dropped.iter().any(|d| d == field), "{field} in {dropped:?}");
    }
    assert!(status.needs_backup());
    assert_eq!(s.cleanup_level, CleanupLevel::Light);
    assert_eq!(s.bar_position, BarPosition::Bottom);
    assert_eq!(s.typing_hours_per_day, 2.0);
    assert_eq!(s.hotkeys["cancel"], default_hotkeys()["cancel"]);
    assert_eq!(s.greeting_name.as_deref(), Some("Kim"));
    assert_eq!(s.typing_wpm, Some(250));
    assert_eq!(s.audio_retention_days, Some(30));
}

#[test]
fn a_non_object_settings_value_falls_back_to_defaults() {
    let (s, status) = decode_stored(Some(&json!([1, 2, 3])));
    assert_eq!(s, ForkSettings::default());
    assert!(status.needs_backup());
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn an_unreadable_file_is_backed_up_and_replaced_by_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(FORK_SETTINGS_FILE);
    std::fs::write(&path, b"{\"settings\": {\"cleanup_level\": \"medi").unwrap();

    let (s, status) = load_from_path(&path);
    assert_eq!(s, ForkSettings::default());
    assert!(status.needs_backup());

    let names = files_in(dir.path());
    let backup = names
        .iter()
        .find(|n| n.starts_with("fork-settings.corrupt-") && n.ends_with(".json"))
        .unwrap_or_else(|| panic!("no backup in {names:?}"));
    assert_eq!(
        std::fs::read(dir.path().join(backup)).unwrap(),
        b"{\"settings\": {\"cleanup_level\": \"medi"
    );
}

#[test]
fn a_partly_broken_file_is_salvaged_and_backed_up() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(FORK_SETTINGS_FILE);
    let original = json!({ "settings": {
        "schema_version": 1, "greeting_name": "Lee", "cleanup_level": [1]
    }})
    .to_string();
    std::fs::write(&path, &original).unwrap();

    let (s, status) = load_from_path(&path);
    assert!(matches!(status, LoadStatus::Salvaged { .. }));
    assert_eq!(s.greeting_name.as_deref(), Some("Lee"));
    assert_eq!(s.cleanup_level, CleanupLevel::Light);
    let backups: Vec<_> = files_in(dir.path())
        .into_iter()
        .filter(|n| n.contains(".corrupt-"))
        .collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(
        std::fs::read_to_string(dir.path().join(&backups[0])).unwrap(),
        original
    );
}

#[test]
fn a_good_file_is_loaded_without_a_backup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(FORK_SETTINGS_FILE);
    assert_eq!(load_from_path(&path).1, LoadStatus::Fresh);
    let value = json!({ "settings": ForkSettings::default() });
    std::fs::write(&path, value.to_string()).unwrap();
    assert_eq!(load_from_path(&path).1, LoadStatus::Loaded);
    assert_eq!(files_in(dir.path()), vec![FORK_SETTINGS_FILE.to_string()]);
}

// --- validation -------------------------------------------------------------

fn rejects(update: ForkSettingUpdate) {
    let mut s = ForkSettings::default();
    let before = s.clone();
    assert!(
        apply_update(&mut s, update.clone()).is_err(),
        "accepted {update:?}"
    );
    assert_eq!(s, before, "a rejected update changed the settings");
}

fn press(chord: &str) -> HotkeyBinding {
    HotkeyBinding::new(chord, HotkeyActivation::Press)
}

#[test]
fn set_setting_rejects_invalid_values() {
    use ForkSettingUpdate as U;
    rejects(U::Hotkey {
        action: "cancel".into(),
        bindings: ["a", "b", "c", "d", "e"].map(press).to_vec(),
    });
    rejects(U::Hotkey {
        action: "cancel".into(),
        bindings: vec![press("escape"), press("escape")],
    });
    rejects(U::Hotkey {
        action: "cancel".into(),
        bindings: vec![press("")],
    });
    rejects(U::Hotkey {
        action: "cancel".into(),
        bindings: vec![press("ctrl++v")],
    });
    rejects(U::Hotkey {
        action: "Bad Action!".into(),
        bindings: vec![],
    });
    rejects(U::TypingWpm(Some(0)));
    rejects(U::TypingWpm(Some(MAX_TYPING_WPM + 1)));
    rejects(U::TypingHoursPerDay(-1.0));
    rejects(U::TypingHoursPerDay(24.5));
    rejects(U::TypingHoursPerDay(f32::NAN));
    rejects(U::GreetingName(Some("   ".into())));
    rejects(U::GreetingName(Some("x".repeat(MAX_GREETING_NAME_LEN + 1))));
    rejects(U::GreetingName(Some("a\u{7}b".into())));
    rejects(U::TextRetentionDays(Some(0)));
    rejects(U::TextRetentionDays(Some(MAX_RETENTION_DAYS + 1)));
    rejects(U::AudioRetentionDays(Some(MAX_RETENTION_DAYS + 1)));
    rejects(U::BarHiddenUntilMs(Some(-5)));
    rejects(U::FirstUseChoice(Some("Has Spaces".into())));
    rejects(U::AutoApplyTransform(Some(String::new())));
    rejects(U::DismissedBanners(vec!["ok".into(), "NOT OK".into()]));
    rejects(U::SeenNewTags(vec!["x".into(); MAX_LIST_ITEMS + 1]));
    rejects(U::Styles(StyleSettings {
        personal: StylePreset::Upbeat,
        ..StyleSettings::default()
    }));
    rejects(U::Styles(StyleSettings {
        work: StylePreset::VeryRelaxed,
        ..StyleSettings::default()
    }));
    rejects(U::Onboarding(OnboardingState {
        current_step: Some("Step 1".into()),
        ..OnboardingState::default()
    }));
}

#[test]
fn set_setting_rejects_wrong_types_unknown_keys_and_internal_fields() {
    for bad in [
        json!({ "key": "cleanup_level", "value": "extreme" }),
        json!({ "key": "cleanup_level", "value": 3 }),
        json!({ "key": "context_awareness", "value": "yes" }),
        json!({ "key": "typing_wpm", "value": -3 }),
        json!({ "key": "no_such_setting", "value": true }),
        json!({ "key": "schema_version", "value": 7 }),
        json!({ "key": "applied_first_run_defaults", "value": [] }),
        json!({ "value": true }),
    ] {
        assert!(
            serde_json::from_value::<ForkSettingUpdate>(bad.clone()).is_err(),
            "accepted {bad}"
        );
    }
}

#[test]
fn set_setting_applies_valid_values() {
    let mut s = ForkSettings::default();
    for (raw, check) in [
        (
            json!({ "key": "cleanup_level", "value": "medium" }),
            (|s: &ForkSettings| s.cleanup_level == CleanupLevel::Medium)
                as fn(&ForkSettings) -> bool,
        ),
        (
            json!({ "key": "hotkey", "value": { "action": "paste_last", "bindings": [] } }),
            |s| s.hotkeys["paste_last"].is_empty(),
        ),
        (
            json!({ "key": "hotkey", "value": { "action": "transform.polish", "bindings": [
                { "chord": "option+1", "activation": "press" }
            ] } }),
            |s| s.hotkeys["transform.polish"].len() == 1,
        ),
        (json!({ "key": "greeting_name", "value": "  Ana  " }), |s| {
            s.greeting_name.as_deref() == Some("Ana")
        }),
        (
            json!({ "key": "text_retention_days", "value": null }),
            |s| s.text_retention_days.is_none(),
        ),
        (json!({ "key": "audio_retention_days", "value": 0 }), |s| {
            s.audio_retention_days == Some(0)
        }),
        (json!({ "key": "update_channel", "value": "beta" }), |s| {
            s.update_channel == UpdateChannel::Beta
        }),
    ] {
        let update: ForkSettingUpdate = serde_json::from_value(raw.clone()).unwrap();
        assert_eq!(update.key(), raw["key"].as_str().unwrap());
        apply_update(&mut s, update).unwrap();
        assert!(check(&s), "{raw}");
    }
}

// --- product settings -------------------------------------------------------

#[test]
fn apply_product_settings_sets_both_lists_on_first_run() {
    let mut handy = fresh_handy();
    let mut fork = ForkSettings::default();
    let report = apply_product_settings(&mut handy, &mut fork, &ctx());

    // FIRST_RUN_DEFAULTS
    assert!(handy.reliable_paste);
    assert!(handy.audio_feedback);
    assert_eq!(handy.log_level, LogLevel::Info);
    assert_eq!(handy.paste_method, PasteMethod::CtrlV);
    assert_eq!(
        handy.keyboard_implementation,
        KeyboardImplementation::HandyKeys
    );
    assert_eq!(handy.theme, Theme::System);
    assert!(handy.autostart_enabled);
    let mut recorded = fork.applied_first_run_defaults.clone();
    recorded.sort();
    let mut all: Vec<String> = FIRST_RUN_DEFAULTS
        .iter()
        .map(|d| d.key.to_owned())
        .collect();
    all.sort();
    assert_eq!(recorded, all);
    assert_eq!(report.first_run.len(), FIRST_RUN_DEFAULTS.len());

    // ENFORCED
    assert_eq!(
        handy.recording_retention_period,
        RecordingRetentionPeriod::Never
    );
    assert!(!handy.post_process_enabled);
    assert!(!handy.experimental_enabled);
    assert_eq!(handy.post_process_provider_id, LOCAL_PROVIDER_ID); // was "openai"
    assert!(handy.update_checks_enabled);
    assert!(report.enforced.contains(&"post_process_provider_id"));
    assert!(report.enforced.contains(&"recording_retention_period"));
}

#[test]
fn apply_product_settings_is_idempotent() {
    let mut handy = fresh_handy();
    let mut fork = ForkSettings::default();
    apply_product_settings(&mut handy, &mut fork, &ctx());
    let (handy_after, fork_after) = (serde_json::to_value(&handy).unwrap(), fork.clone());

    let report = apply_product_settings(&mut handy, &mut fork, &ctx());
    assert_eq!(report, ProductReport::default());
    assert!(!report.handy_changed());
    assert_eq!(serde_json::to_value(&handy).unwrap(), handy_after);
    assert_eq!(fork, fork_after);

    // And after a relaunch.
    let (mut handy, mut fork) = relaunch(&handy, &fork);
    assert_eq!(
        apply_product_settings(&mut handy, &mut fork, &ctx()),
        ProductReport::default()
    );
}

#[test]
fn enforced_values_are_reapplied_after_a_manual_change() {
    let mut handy = fresh_handy();
    let mut fork = ForkSettings::default();
    apply_product_settings(&mut handy, &mut fork, &ctx());

    // The user (or Handy's own UI) changes every enforced value.
    handy.recording_retention_period = RecordingRetentionPeriod::Days3;
    handy.post_process_enabled = true;
    handy.experimental_enabled = true;
    handy.post_process_provider_id = "anthropic".into();
    handy.update_checks_enabled = false;
    let (mut handy, mut fork) = relaunch(&handy, &fork);

    let report = apply_product_settings(&mut handy, &mut fork, &ctx());
    let mut enforced = report.enforced.clone();
    enforced.sort();
    assert_eq!(
        enforced,
        vec![
            "experimental_enabled",
            "post_process_enabled",
            "post_process_provider_id",
            "recording_retention_period",
            "update_checks_enabled",
        ]
    );
    assert!(report.first_run.is_empty());
    assert_eq!(
        handy.recording_retention_period,
        RecordingRetentionPeriod::Never
    );
    assert!(!handy.post_process_enabled && !handy.experimental_enabled);
    assert_eq!(handy.post_process_provider_id, LOCAL_PROVIDER_ID);
    assert!(handy.update_checks_enabled);
}

#[test]
fn update_checks_follow_the_fork_setting() {
    let mut handy = fresh_handy();
    let mut fork = ForkSettings::default();
    apply_product_settings(&mut handy, &mut fork, &ctx());
    apply_update(&mut fork, ForkSettingUpdate::UpdateChecksEnabled(false)).unwrap();
    let report = apply_product_settings(&mut handy, &mut fork, &ctx());
    assert_eq!(report.enforced, vec!["update_checks_enabled"]);
    assert!(!handy.update_checks_enabled);
}

#[test]
fn each_first_run_default_changed_by_the_user_persists_across_relaunch() {
    let mut handy = fresh_handy();
    let mut fork = ForkSettings::default();
    apply_product_settings(&mut handy, &mut fork, &ctx());

    // The user changes every first-run value in Advanced.
    handy.reliable_paste = false;
    handy.audio_feedback = false;
    handy.log_level = LogLevel::Warn;
    handy.paste_method = PasteMethod::Direct;
    handy.keyboard_implementation = KeyboardImplementation::Tauri;
    handy.theme = Theme::Dark;
    handy.autostart_enabled = false;
    // Every first-run key is covered by this test.
    assert_eq!(FIRST_RUN_DEFAULTS.len(), 7);

    for _ in 0..3 {
        let (h, f) = relaunch(&handy, &fork);
        handy = h;
        fork = f;
        let report = apply_product_settings(&mut handy, &mut fork, &ctx());
        assert!(report.first_run.is_empty(), "{report:?}");
    }
    assert!(!handy.reliable_paste);
    assert!(!handy.audio_feedback);
    assert_eq!(handy.log_level, LogLevel::Warn);
    assert_eq!(handy.paste_method, PasteMethod::Direct);
    assert_eq!(handy.keyboard_implementation, KeyboardImplementation::Tauri);
    assert_eq!(handy.theme, Theme::Dark);
    assert!(!handy.autostart_enabled);
}

#[test]
fn autostart_waits_for_a_build_that_may_set_it() {
    let mut handy = fresh_handy();
    let mut fork = ForkSettings::default();
    let dev = ProductContext {
        autostart_allowed: false,
    };
    let report = apply_product_settings(&mut handy, &mut fork, &dev);
    assert!(!report.first_run.contains(&"autostart_enabled"));
    assert!(!handy.autostart_enabled);
    assert!(!fork
        .applied_first_run_defaults
        .iter()
        .any(|k| k == "autostart_enabled"));
    // The first run of a build that may set it applies it once.
    let report = apply_product_settings(&mut handy, &mut fork, &ctx());
    assert_eq!(report.first_run, vec!["autostart_enabled"]);
    assert!(handy.autostart_enabled);
}

#[test]
fn a_new_first_run_key_is_applied_once_to_an_existing_install() {
    let mut handy = fresh_handy();
    let mut fork = ForkSettings::default();
    apply_product_settings(&mut handy, &mut fork, &ctx());
    // An install from before "audio_feedback" existed in the list.
    fork.applied_first_run_defaults
        .retain(|k| k != "audio_feedback");
    handy.audio_feedback = false;
    let report = apply_product_settings(&mut handy, &mut fork, &ctx());
    assert_eq!(report.first_run, vec!["audio_feedback"]);
    assert!(handy.audio_feedback);
}

#[test]
fn a_cloud_provider_becomes_local_and_a_loopback_custom_provider_stays() {
    // F-08: the built-in list always has `local`, so a cloud choice becomes
    // `local`.
    for cloud in [
        "openai",
        "anthropic",
        "groq",
        "openrouter",
        "zai",
        "bedrock_mantle",
    ] {
        let mut handy = fresh_handy();
        handy.post_process_provider_id = cloud.into();
        let mut fork = ForkSettings::default();
        apply_product_settings(&mut handy, &mut fork, &ctx());
        assert_eq!(handy.post_process_provider_id, LOCAL_PROVIDER_ID, "{cloud}");
    }

    // Without a `local` entry (a list edited by hand), the provider rule
    // restores it first, so the choice still becomes `local`.
    let mut handy = fresh_handy();
    handy
        .post_process_providers
        .retain(|p| p.id != LOCAL_PROVIDER_ID);
    handy.post_process_provider_id = "openai".into();
    let mut fork = ForkSettings::default();
    apply_product_settings(&mut handy, &mut fork, &ctx());
    assert_eq!(handy.post_process_provider_id, LOCAL_PROVIDER_ID);

    // `local` and a loopback custom provider stay.
    for (id, url) in [
        (LOCAL_PROVIDER_ID, "http://127.0.0.1:0/v1"),
        (CUSTOM_PROVIDER_ID, "http://localhost:11434/v1"),
        (CUSTOM_PROVIDER_ID, "http://127.0.0.1:8080/v1"),
        (CUSTOM_PROVIDER_ID, "http://[::1]:11434/v1"),
    ] {
        let mut handy = fresh_handy();
        if id == LOCAL_PROVIDER_ID {
            let _ = url; // the built-in `local` entry is used as is
        } else {
            handy
                .post_process_providers
                .iter_mut()
                .find(|p| p.id == CUSTOM_PROVIDER_ID)
                .unwrap()
                .base_url = url.into();
        }
        handy.post_process_provider_id = id.into();
        let mut fork = ForkSettings::default();
        let report = apply_product_settings(&mut handy, &mut fork, &ctx());
        assert_eq!(handy.post_process_provider_id, id, "{url}");
        assert!(
            !report.enforced.contains(&"post_process_provider_id"),
            "{url}"
        );
        let custom = handy
            .post_process_providers
            .iter()
            .find(|p| p.id == CUSTOM_PROVIDER_ID)
            .unwrap();
        assert!(is_loopback_url(&custom.base_url));
    }

    // Apple Intelligence runs on device and stays.
    let mut handy = fresh_handy();
    handy
        .post_process_providers
        .retain(|p| p.id != crate::settings::APPLE_INTELLIGENCE_PROVIDER_ID);
    handy.post_process_providers.push(provider(
        crate::settings::APPLE_INTELLIGENCE_PROVIDER_ID,
        "apple-intelligence://local",
    ));
    handy.post_process_provider_id = crate::settings::APPLE_INTELLIGENCE_PROVIDER_ID.into();
    let mut fork = ForkSettings::default();
    apply_product_settings(&mut handy, &mut fork, &ctx());
    assert_eq!(
        handy.post_process_provider_id,
        crate::settings::APPLE_INTELLIGENCE_PROVIDER_ID
    );
}

#[test]
fn a_custom_provider_pointing_at_the_network_is_reset_to_loopback() {
    let mut handy = fresh_handy();
    let custom = handy
        .post_process_providers
        .iter_mut()
        .find(|p| p.id == CUSTOM_PROVIDER_ID)
        .unwrap();
    custom.base_url = "https://my-llm.example.com/v1".into();
    handy.post_process_provider_id = CUSTOM_PROVIDER_ID.into();
    let mut fork = ForkSettings::default();
    let report = apply_product_settings(&mut handy, &mut fork, &ctx());
    assert!(report.enforced.contains(&"post_process_provider_id"));
    let custom = handy
        .post_process_providers
        .iter()
        .find(|p| p.id == CUSTOM_PROVIDER_ID)
        .unwrap();
    assert_eq!(custom.base_url, LOOPBACK_CUSTOM_BASE_URL);
    assert_eq!(handy.post_process_provider_id, CUSTOM_PROVIDER_ID);
}

#[test]
fn loopback_detection() {
    for ok in [
        "http://localhost:11434/v1",
        "http://LOCALHOST/v1",
        "https://127.0.0.1:8443",
        "http://127.8.9.10/x",
        "http://[::1]:8080/v1",
        "http://localhost",
    ] {
        assert!(is_loopback_url(ok), "{ok}");
    }
    for bad in [
        "https://api.openai.com/v1",
        "http://localhost.example.com/v1",
        "http://10.0.0.2:11434/v1",
        "http://user@localhost:11434/v1",
        "ftp://localhost/",
        "localhost:11434",
        "http://[::1/v1",
        "apple-intelligence://local",
        "",
    ] {
        assert!(!is_loopback_url(bad), "{bad}");
    }
}

#[test]
fn handy_overlay_is_turned_off_now_that_the_bar_replaces_it() {
    // S-03: the fork's Flow Bar replaces Handy's overlay.
    const { assert!(HANDY_OVERLAY_REPLACED) };
    let mut handy = fresh_handy();
    handy.overlay_style = crate::settings::OverlayStyle::Minimal;
    let mut fork = ForkSettings::default();
    let report = apply_product_settings(&mut handy, &mut fork, &ctx());
    assert_eq!(handy.overlay_style, crate::settings::OverlayStyle::None);
    assert!(report.enforced.contains(&"overlay_style"));
    assert_eq!(enforced_snapshot(&handy)["overlay_enforced"], json!(true));
    // Idempotent: the next start changes nothing.
    let report = apply_product_settings(&mut handy, &mut fork, &ctx());
    assert!(!report.enforced.contains(&"overlay_style"));
}

#[test]
fn bar_positions_are_validated() {
    use std::collections::BTreeMap;
    let ok = BTreeMap::from([("built-in-retina-display-1512x982".to_string(), 0.25)]);
    assert!(validate(&ForkSettingUpdate::BarXByDisplay(ok.clone())).is_ok());
    for bad in [
        BTreeMap::from([("Bad Key".to_string(), 0.5)]),
        BTreeMap::from([("k".to_string(), 1.5)]),
        BTreeMap::from([("k".to_string(), f64::NAN)]),
        (0..=MAX_BAR_DISPLAYS)
            .map(|i| (format!("d{i}"), 0.5))
            .collect(),
    ] {
        assert!(validate(&ForkSettingUpdate::BarXByDisplay(bad)).is_err());
    }
    let mut s = ForkSettings::default();
    apply_update(&mut s, ForkSettingUpdate::BarXByDisplay(ok.clone())).unwrap();
    apply_update(&mut s, ForkSettingUpdate::BarHideInRecordings(true)).unwrap();
    assert_eq!(s.bar_x_by_display, ok);
    assert!(s.bar_hide_in_recordings);
    // Defaults: centred everywhere, visible in recordings.
    let d = ForkSettings::default();
    assert!(d.bar_x_by_display.is_empty());
    assert!(!d.bar_hide_in_recordings);
}

#[test]
fn the_enforced_list_matches_r5() {
    let keys: Vec<_> = ENFORCED.iter().map(|r| r.key).collect();
    assert_eq!(
        keys,
        vec![
            "overlay_style",
            "recording_retention_period",
            "post_process_enabled",
            "experimental_enabled",
            "post_process_providers",
            "post_process_provider_id",
            "update_checks_enabled",
        ]
    );
}

/// F-08: a settings file written by an older build (Handy's cloud list, an
/// OpenAI key, OpenAI selected) comes out with no cloud provider, no cloud
/// key slot and `local` selected.
#[test]
fn a_stored_openai_provider_is_migrated_to_local() {
    let mut stored = serde_json::to_value(fresh_handy()).unwrap();
    let cloud = [
        ("openai", "https://api.openai.com/v1"),
        ("anthropic", "https://api.anthropic.com/v1"),
        ("openrouter", "https://openrouter.ai/api/v1"),
        (
            "bedrock_mantle",
            "https://bedrock-mantle.us-east-1.api.aws/v1",
        ),
    ];
    stored["post_process_providers"]
        .as_array_mut()
        .unwrap()
        .retain(|p| p["id"] != LOCAL_PROVIDER_ID);
    for (id, url) in cloud {
        stored["post_process_providers"]
            .as_array_mut()
            .unwrap()
            .insert(0, serde_json::to_value(provider(id, url)).unwrap());
        stored["post_process_api_keys"][id] = json!("sk-proj-secret-key-12345");
        stored["post_process_models"][id] = json!("gpt-x");
    }
    stored["post_process_provider_id"] = json!("openai");
    let mut handy: AppSettings = serde_json::from_value(stored).unwrap();
    let mut fork = ForkSettings::default();
    let report = apply_product_settings(&mut handy, &mut fork, &ctx());

    assert!(report.enforced.contains(&"post_process_providers"));
    assert_eq!(handy.post_process_provider_id, LOCAL_PROVIDER_ID);
    for p in &handy.post_process_providers {
        assert!(
            crate::fork::llm::providers::is_on_device(p),
            "{} ({}) survived",
            p.id,
            p.base_url
        );
    }
    for (id, _) in cloud {
        assert!(
            !handy.post_process_api_keys.contains_key(id),
            "{id} key kept"
        );
        assert!(
            !handy.post_process_models.contains_key(id),
            "{id} model kept"
        );
    }
    let ids: Vec<_> = handy
        .post_process_providers
        .iter()
        .map(|p| p.id.as_str())
        .collect();
    let builtin: Vec<_> = crate::fork::llm::default_providers()
        .into_iter()
        .map(|p| p.id)
        .collect();
    assert_eq!(ids, builtin);
    // The read side: what Handy's `get_app_settings` would now return.
    let json = serde_json::to_string(&handy).unwrap();
    for host in [
        "api.openai.com",
        "api.anthropic.com",
        "openrouter.ai",
        "api.aws",
    ] {
        assert!(!json.contains(host), "{host} still in settings");
    }
    // Idempotent.
    assert_eq!(
        apply_product_settings(&mut handy, &mut fork, &ctx()),
        ProductReport::default()
    );
}

#[test]
fn the_custom_provider_keeps_the_users_loopback_url_and_format() {
    let mut handy = fresh_handy();
    let custom = handy
        .post_process_providers
        .iter_mut()
        .find(|p| p.id == CUSTOM_PROVIDER_ID)
        .unwrap();
    custom.base_url = "http://127.0.0.1:8080/v1".into();
    custom.api_format = PostProcessApiFormat::Anthropic;
    let mut fork = ForkSettings::default();
    apply_product_settings(&mut handy, &mut fork, &ctx());
    let custom = handy
        .post_process_providers
        .iter()
        .find(|p| p.id == CUSTOM_PROVIDER_ID)
        .unwrap();
    assert_eq!(custom.base_url, "http://127.0.0.1:8080/v1");
    assert_eq!(custom.api_format, PostProcessApiFormat::Anthropic);
}

// --- Tauri: commands, event, on_ready, relaunch ------------------------------

mod app {
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::Duration;

    use serde_json::{json, Value};
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime};
    use tauri::webview::InvokeRequest;
    use tauri::{App, Manager, RunEvent, WebviewWindow, WebviewWindowBuilder};
    use tauri_specta::Event;

    use super::super::*;
    use crate::fork;
    use crate::fork::events::ForkSettingsChanged;
    use crate::settings::get_default_settings;

    /// A mock app wired like `lib.rs` (fork plugin, store plugin, composed
    /// handler) whose fork settings file is at `path` and whose Handy store
    /// is a [`FakeHandyStore`].
    fn build(path: &Path, handy: AppSettings, identifier: &str) -> App<MockRuntime> {
        let mut context = mock_context(noop_assets());
        context.config_mut().identifier = identifier.into();
        mock_builder()
            // Managed before the plugin's setup runs, so `register` keeps it.
            .manage(ForkSettingsHandle::new(path))
            .manage(FakeHandyStore::with(handy))
            .plugin(fork::init())
            .plugin(tauri_plugin_store::Builder::default().build())
            .invoke_handler(fork::compose_invoke(tauri::generate_handler![]))
            .build(context)
            .expect("failed to build mock app")
    }

    fn window(app: &App<MockRuntime>) -> WebviewWindow<MockRuntime> {
        WebviewWindowBuilder::new(app, "main", Default::default())
            .build()
            .expect("mock window")
    }

    fn invoke(w: &WebviewWindow<MockRuntime>, cmd: &str, body: Value) -> Result<Value, Value> {
        get_ipc_response(
            w,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "tauri://localhost".parse().unwrap(),
                body: InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.to_string(),
            },
        )
        .map(|b| b.deserialize::<Value>().unwrap())
    }

    /// Runs until `RunEvent::Ready` (so `fork::on_ready` has run), calls `f`,
    /// and returns its result plus Handy's settings afterwards.
    fn run<T: Send + 'static>(
        app: App<MockRuntime>,
        f: impl Fn(&WebviewWindow<MockRuntime>) -> T + 'static,
    ) -> (T, AppSettings) {
        let out: Arc<Mutex<Option<(T, AppSettings)>>> = Arc::new(Mutex::new(None));
        let sink = out.clone();
        app.run(move |handle, event| {
            if let RunEvent::Ready = event {
                let w = handle.get_webview_window("main").expect("main window");
                let value = f(&w);
                let handy = handle.state::<FakeHandyStore>().get().unwrap();
                *sink.lock().unwrap() = Some((value, handy));
                w.destroy().unwrap();
            }
        });
        let result = out.lock().unwrap().take();
        result.expect("Ready was never delivered")
    }

    fn stored(path: &Path) -> Value {
        let raw: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        raw[STORE_KEY].clone()
    }

    #[test]
    fn commands_read_update_validate_and_emit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FORK_SETTINGS_FILE);
        let app = build(&path, get_default_settings(), PRODUCT_IDENTIFIER);
        let _w = window(&app);

        let (tx, rx) = mpsc::channel();
        ForkSettingsChanged::listen(&app, move |e| {
            let _ = tx.send(e.payload);
        });
        assert_eq!(ForkSettingsChanged::NAME, "fork-settings-changed");

        let (results, handy) = run(app, |w| {
            let got = invoke(w, "fork_get_settings", json!({}));
            let set = invoke(
                w,
                "fork_set_setting",
                json!({ "update": { "key": "cleanup_level", "value": "medium" } }),
            );
            let invalid = invoke(
                w,
                "fork_set_setting",
                json!({ "update": { "key": "typing_wpm", "value": 100000 } }),
            );
            let wrong_type = invoke(
                w,
                "fork_set_setting",
                json!({ "update": { "key": "cleanup_level", "value": 7 } }),
            );
            let mirror = invoke(
                w,
                "fork_set_setting",
                json!({ "update": { "key": "update_checks_enabled", "value": false } }),
            );
            let after = invoke(w, "fork_get_settings", json!({}));
            (got, set, invalid, wrong_type, mirror, after)
        });
        let (got, set, invalid, wrong_type, mirror, after) = results;

        let got = got.unwrap();
        assert_eq!(got["cleanup_level"], json!("light"));
        assert_eq!(set.unwrap()["cleanup_level"], json!("medium"));
        let invalid = invalid.unwrap_err();
        assert_eq!(invalid["kind"], json!("InvalidSetting"));
        assert!(invalid["message"].as_str().unwrap().contains("typing_wpm"));
        assert!(wrong_type.is_err());
        assert_eq!(mirror.unwrap()["update_checks_enabled"], json!(false));
        let after = after.unwrap();
        assert_eq!(after["cleanup_level"], json!("medium"));
        assert_eq!(after["typing_wpm"], Value::Null);

        // Persisted to the store file, and mirrored to Handy at once.
        assert_eq!(stored(&path)["cleanup_level"], json!("medium"));
        assert_eq!(stored(&path)["update_checks_enabled"], json!(false));
        assert!(!handy.update_checks_enabled);

        // One event per successful set, none for rejected ones.
        let events: Vec<ForkSettingsChanged> =
            std::iter::from_fn(|| rx.recv_timeout(Duration::from_secs(2)).ok()).collect();
        let keys: Vec<_> = events.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["cleanup_level", "update_checks_enabled"]);
        assert_eq!(events[0].settings.cleanup_level, CleanupLevel::Medium);
    }

    #[test]
    fn on_ready_applies_product_settings_and_a_relaunch_keeps_user_choices() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FORK_SETTINGS_FILE);

        // First launch: both lists applied, first-run keys recorded on disk.
        let mut handy = get_default_settings();
        handy.post_process_provider_id = "openai".into();
        handy.experimental_enabled = true;
        let app = build(&path, handy, PRODUCT_IDENTIFIER);
        let _w = window(&app);
        let ((), handy) = run(app, |_| ());
        assert!(handy.audio_feedback);
        assert_eq!(handy.log_level, LogLevel::Info);
        assert!(!handy.experimental_enabled);
        assert_eq!(handy.post_process_provider_id, LOCAL_PROVIDER_ID);
        let recorded = stored(&path)["applied_first_run_defaults"].clone();
        assert!(recorded
            .as_array()
            .unwrap()
            .contains(&json!("audio_feedback")));

        // The user turns sounds off and re-enables Experimental in Handy's UI.
        let mut handy = handy;
        handy.audio_feedback = false;
        handy.experimental_enabled = true;
        let app = build(&path, handy, PRODUCT_IDENTIFIER);
        let _w = window(&app);
        let ((), handy) = run(app, |_| ());
        assert!(!handy.audio_feedback, "first-run default re-applied");
        assert!(!handy.experimental_enabled, "enforced value not re-applied");
    }

    #[test]
    fn another_identity_never_touches_handy_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FORK_SETTINGS_FILE);
        let mut handy = get_default_settings();
        handy.experimental_enabled = true;
        let app = build(&path, handy, "com.pais.handy");
        let _w = window(&app);
        let (got, handy) = run(app, |w| invoke(w, "fork_get_settings", json!({})));
        assert!(handy.experimental_enabled);
        assert!(!handy.audio_feedback);
        // Fork settings themselves still work.
        assert_eq!(got.unwrap()["cleanup_level"], json!("light"));
    }

    #[test]
    fn a_corrupt_file_is_salvaged_through_the_store() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FORK_SETTINGS_FILE);
        std::fs::write(&path, b"not json at all").unwrap();
        let app = build(&path, get_default_settings(), PRODUCT_IDENTIFIER);
        let _w = window(&app);
        let (got, _) = run(app, |w| invoke(w, "fork_get_settings", json!({})));
        assert_eq!(
            got.unwrap()["schema_version"],
            json!(CURRENT_SCHEMA_VERSION)
        );
        // Rewritten with valid settings, the bad bytes kept in a backup.
        assert_eq!(
            stored(&path)["schema_version"],
            json!(CURRENT_SCHEMA_VERSION)
        );
        let backups: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".corrupt-"))
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(
            std::fs::read(backups[0].path()).unwrap(),
            b"not json at all"
        );
    }
}
