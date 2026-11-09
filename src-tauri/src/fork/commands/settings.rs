// SPDX-License-Identifier: GPL-3.0-or-later
//! Fork settings commands (F-07): `fork_get_settings`, `fork_set_setting`.

use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use super::ForkError;
use crate::fork::events::ForkSettingsChanged;
use crate::fork::settings::{
    self, apply_update, ForkSettingUpdate, ForkSettings, ForkSettingsHandle,
};
use crate::fork::ForkRuntime;

fn handle<R: tauri::Runtime>(
    app: &AppHandle<R>,
) -> Result<tauri::State<'_, ForkSettingsHandle>, ForkError> {
    app.try_state::<ForkSettingsHandle>()
        .ok_or_else(|| ForkError::Internal("fork settings are not managed".into()))
}

/// All fork settings.
#[tauri::command]
#[specta::specta]
pub fn fork_get_settings<R: ForkRuntime>(app: AppHandle<R>) -> Result<ForkSettings, ForkError> {
    handle(&app)?.get(&app).map_err(ForkError::Internal)
}

/// Changes one fork setting. The value is type-checked by the IPC layer and
/// range-checked by `fork::settings::validate`; an invalid value returns
/// `InvalidSetting` and changes nothing. On success the new settings are
/// saved, returned and broadcast as `ForkSettingsChanged`.
#[tauri::command]
#[specta::specta]
pub fn fork_set_setting<R: ForkRuntime>(
    app: AppHandle<R>,
    update: ForkSettingUpdate,
) -> Result<ForkSettings, ForkError> {
    let key = update.key();
    // Giving a transform a shortcut means the user wants it to work: a
    // transform shortcut does nothing while Transforms is off, and the key
    // then types its own character (⌥M → µ). Turn Transforms on with it.
    let wants_transforms = matches!(
        &update,
        ForkSettingUpdate::Hotkey { action, bindings }
            if action.starts_with("transform.") && action != "transform.view_changes" && !bindings.is_empty()
    );
    settings::validate(&update).map_err(ForkError::InvalidSetting)?;
    if update == ForkSettingUpdate::ShowInDock(false) && !tray_visible(&app) {
        return Err(ForkError::InvalidSetting(NO_WAY_BACK.into()));
    }
    let (next, ()) = handle(&app)?
        .update(&app, |s| apply_update(s, update))
        .map_err(ForkError::Internal)?;

    if key == "update_checks_enabled" {
        mirror_to_handy(&app, &next, "update_checks_enabled");
    }
    if key == "cleanup_level" {
        mirror_to_handy(&app, &next, "filler_word_removal_enabled");
    }
    if key == "transforms_enabled" {
        crate::fork::transforms::sync_registration(&app, next.transforms_enabled);
        crate::fork::shortcut::reload(&app);
    }
    if key == "apple_dictation_shortcut_off" {
        crate::fork::shortcut::reconcile_apple_dictation(&app);
    }
    // OT2-H: custom transforms and Command Mode run on the instruction model.
    if (key == "transforms_enabled" || key == "command_mode_enabled")
        && crate::fork::llm::instruct::wanted(&app)
    {
        crate::fork::llm::instruct::ensure_downloaded(&app);
    }
    if wants_transforms && !next.transforms_enabled {
        return fork_set_setting(app, ForkSettingUpdate::TransformsEnabled(true));
    }
    if key == "hotkey" || key == "command_mode_enabled" {
        if key == "command_mode_enabled" && !next.command_mode_enabled {
            crate::fork::transforms::command::cancel();
        }
        crate::fork::shortcut::reload(&app);
    }
    #[cfg(target_os = "macos")]
    if key == "show_in_dock" {
        let policy = if next.show_in_dock || !tray_visible(&app) {
            tauri::ActivationPolicy::Regular
        } else {
            tauri::ActivationPolicy::Accessory
        };
        if let Err(error) = app.set_activation_policy(policy) {
            log::error!("fork: could not change the Dock activation policy: {error}");
        }
    }
    if key == "vibe_coding" || key == "context_awareness" {
        crate::fork::context::ide::sync_warm(&next);
    }
    if key == "text_retention_days" || key == "audio_retention_days" {
        crate::fork::db::jobs::wake();
    }
    if key.starts_with("bar_") || key == "scratchpad_on_bar" {
        crate::fork::pill::settings_changed(&next);
    }

    let event = ForkSettingsChanged {
        key,
        settings: next.clone(),
    };
    if let Err(e) = event.emit(&app) {
        log::warn!("fork: failed to emit ForkSettingsChanged: {e}");
    }
    Ok(next)
}

/// Audit OT2: with neither a Dock icon nor a menu bar icon the app has no
/// way back in but Spotlight, so "Show app in Dock" and "Show menu bar icon"
/// are never both off. Settings disables the last one that is on; this is
/// the backend's half of the rule.
pub const NO_WAY_BACK: &str =
    "show_in_dock: the menu bar icon is off, so the Dock icon is the way back into the app";

/// The menu bar icon is shown (Handy's setting, and no `--no-tray`).
fn tray_visible<R: ForkRuntime>(app: &AppHandle<R>) -> bool {
    R::handy_settings(app)
        .map(|settings::HandySettings(settings)| settings.show_tray_icon)
        .unwrap_or(true)
        && !app
            .try_state::<crate::CliArgs>()
            .map(|args| args.no_tray)
            .unwrap_or(false)
}

/// After Handy's own "Show menu bar icon" command (observed in
/// `fork::compose_invoke`): if that left neither icon, the Dock icon comes
/// back. Settings never offers that state; this covers any other caller.
/// Returns whether it turned the Dock icon back on.
pub fn keep_a_way_back<R: ForkRuntime>(app: &AppHandle<R>) -> bool {
    let dock = handle(app)
        .and_then(|h| h.get(app).map_err(ForkError::Internal))
        .map(|s| s.show_in_dock)
        .unwrap_or(true);
    if dock || tray_visible(app) {
        return false;
    }
    log::info!("fork: menu bar icon off with the Dock icon off; the Dock icon stays");
    match fork_set_setting(app.clone(), ForkSettingUpdate::ShowInDock(true)) {
        Ok(_) => true,
        Err(e) => {
            log::warn!("fork: could not turn the Dock icon back on: {e:?}");
            false
        }
    }
}

/// Applies one `ENFORCED` rule to Handy's settings at once (the update-check
/// switch, the filler removal of level None); every rule is also re-applied
/// on every start.
fn mirror_to_handy<R: ForkRuntime>(
    app: &AppHandle<R>,
    fork: &ForkSettings,
    handy_key: &'static str,
) {
    if !crate::fork::is_product_identity(&app.config().identifier) {
        return;
    }
    let Some(rule) = settings::ENFORCED.iter().find(|r| r.key == handy_key) else {
        return;
    };
    let Some(settings::HandySettings(handy)) = R::handy_settings(app) else {
        return;
    };
    let before = handy.clone();
    let mut after = handy;
    if !(rule.apply)(&mut after, fork) {
        return;
    }
    let report = settings::ProductReport {
        enforced: vec![handy_key],
        first_run: vec![],
    };
    R::write_handy_settings(
        app,
        &settings::HandySettings(before),
        settings::HandySettings(after),
        &report,
    );
}

#[cfg(test)]
mod tests {
    use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
    use tauri::App;

    use super::*;
    use crate::fork::settings::FakeHandyStore;

    fn app(show_tray_icon: bool) -> App<MockRuntime> {
        let dir = tempfile::tempdir().unwrap().keep();
        let mut handy = crate::settings::get_default_settings();
        handy.show_tray_icon = show_tray_icon;
        mock_builder()
            .manage(ForkSettingsHandle::new(dir.join("fork-settings.json")))
            .manage(FakeHandyStore::with(handy))
            // The fork plugin mounts the events `fork_set_setting` emits.
            .plugin(crate::fork::init())
            .plugin(tauri_plugin_store::Builder::default().build())
            .build(mock_context(noop_assets()))
            .expect("mock app")
    }

    fn set_tray(app: &App<MockRuntime>, on: bool) {
        let store = app.state::<FakeHandyStore>();
        let mut handy = store.get().unwrap();
        handy.show_tray_icon = on;
        store.set(handy);
    }

    fn dock(app: &App<MockRuntime>) -> bool {
        fork_get_settings(app.handle().clone())
            .unwrap()
            .show_in_dock
    }

    /// Audit OT2: the Dock icon cannot go while the menu bar icon is off.
    #[test]
    fn the_dock_icon_stays_while_the_menu_bar_icon_is_off() {
        let app = app(false);
        let refused = fork_set_setting(app.handle().clone(), ForkSettingUpdate::ShowInDock(false));
        assert_eq!(refused, Err(ForkError::InvalidSetting(NO_WAY_BACK.into())));
        assert!(dock(&app), "nothing changed");

        set_tray(&app, true);
        fork_set_setting(app.handle().clone(), ForkSettingUpdate::ShowInDock(false))
            .expect("with the menu bar icon on, the Dock icon may go");
        assert!(!dock(&app));
        // Turning the Dock icon back on is always allowed.
        fork_set_setting(app.handle().clone(), ForkSettingUpdate::ShowInDock(true)).unwrap();
        assert!(dock(&app));
    }

    /// Handy's own tray command cannot be refused, so the fork restores the
    /// Dock icon after it if both would be off.
    #[test]
    fn turning_the_menu_bar_icon_off_brings_the_dock_icon_back() {
        let app = app(true);
        fork_set_setting(app.handle().clone(), ForkSettingUpdate::ShowInDock(false)).unwrap();
        assert!(!keep_a_way_back(app.handle()), "menu bar icon still on");
        assert!(!dock(&app));

        set_tray(&app, false);
        assert!(keep_a_way_back(app.handle()));
        assert!(dock(&app));
        assert!(!keep_a_way_back(app.handle()), "already safe");
    }

    #[test]
    fn handy_commands_the_fork_reacts_to_still_exist() {
        let lib = include_str!("../../lib.rs");
        for (command, path) in [
            ("update_custom_words", "shortcut::update_custom_words"),
            (
                "change_show_tray_icon_setting",
                "shortcut::change_show_tray_icon_setting",
            ),
        ] {
            assert!(
                crate::fork::observe_handy_command::<MockRuntime>(command).is_some(),
                "{command}"
            );
            assert!(lib.contains(path), "{path}");
        }
    }

    /// Reopening the running app from Finder, Spotlight or the Dock always
    /// shows the Hub (Handy's `RunEvent::Reopen`; a parked Hub is rebuilt
    /// by `show_main_window`'s first line).
    #[test]
    fn reopening_the_app_shows_the_hub() {
        let lib = include_str!("../../lib.rs");
        let at = lib.find("tauri::RunEvent::Reopen").expect("Reopen handled");
        let end = at + lib[at..].find("\n        }").expect("arm end");
        assert!(lib[at..end].contains("show_main_window(app)"));
        let show = lib.find("fn show_main_window(").expect("show_main_window");
        assert!(lib[show..show + 200].contains("crate::fork::hub::park::ensure_main(app)"));
    }
}
