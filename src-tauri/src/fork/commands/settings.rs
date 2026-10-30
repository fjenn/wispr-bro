// SPDX-License-Identifier: GPL-3.0-or-later
//! Fork settings commands (F-07): `fork_get_settings`, `fork_set_setting`.

use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use super::ForkError;
use crate::fork::events::ForkSettingsChanged;
use crate::fork::settings::{
    self, apply_update, ForkSettingUpdate, ForkSettings, ForkSettingsHandle, PRODUCT_IDENTIFIER,
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
    settings::validate(&update).map_err(ForkError::InvalidSetting)?;
    let (next, ()) = handle(&app)?
        .update(&app, |s| apply_update(s, update))
        .map_err(ForkError::Internal)?;

    if key == "update_checks_enabled" {
        mirror_update_checks(&app, next.update_checks_enabled);
    }
    if key == "hotkey" {
        crate::fork::shortcut::reload(&app);
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

/// Mirrors the fork's update-check switch into Handy's setting at once (it is
/// also re-applied on every start, `ENFORCED`).
fn mirror_update_checks<R: ForkRuntime>(app: &AppHandle<R>, enabled: bool) {
    if app.config().identifier != PRODUCT_IDENTIFIER {
        return;
    }
    let Some(settings::HandySettings(handy)) = R::handy_settings(app) else {
        return;
    };
    if handy.update_checks_enabled == enabled {
        return;
    }
    let before = handy.clone();
    let mut after = handy;
    after.update_checks_enabled = enabled;
    let report = settings::ProductReport {
        enforced: vec!["update_checks_enabled"],
        first_run: vec![],
    };
    R::write_handy_settings(
        app,
        &settings::HandySettings(before),
        settings::HandySettings(after),
        &report,
    );
}
