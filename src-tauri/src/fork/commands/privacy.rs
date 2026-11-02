// SPDX-License-Identifier: GPL-3.0-or-later
//! Data and Privacy commands (S-27): local paths, export, delete all and
//! sanitized diagnostics. Destructive erase is accepted only in a ready Wispr
//! Bro product/test identity and only with the exact typed confirmation.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
#[cfg(not(test))]
use tauri_plugin_opener::OpenerExt;

use super::ForkError;
use crate::fork::diagnostics::bundle;
use crate::fork::erase::{self, EraseReport};
use crate::fork::export::{self, ExportReport};
use crate::fork::{ForkRuntime, ForkState};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ForkDataLocationKind {
    Data,
    Models,
    Recordings,
    Logs,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ForkDataLocations {
    pub data: String,
    pub models: String,
    pub recordings: String,
    pub logs: String,
    pub portable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ForkExportResult {
    pub path: String,
    pub files: u32,
    pub bytes: u64,
}

fn data_dir<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<PathBuf, ForkError> {
    crate::portable::data_dir()
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| app.path().app_data_dir())
        .map_err(|e| ForkError::Internal(e.to_string()))
}

/// Where each kind of data lives. Models are the Hugging Face cache
/// (`HF_HOME`, set under app data by `early_init` / portable mode, DEC-39);
/// without one, the folder it would be. Portable mode keeps every folder
/// under its `Data/` (E-88).
pub fn locations_for(
    data: &Path,
    logs: &Path,
    hf_home: Option<&Path>,
    portable: bool,
) -> ForkDataLocations {
    let text = |p: &Path| p.to_string_lossy().into_owned();
    let models = hf_home
        .map(Path::to_path_buf)
        .unwrap_or_else(|| data.join(crate::fork::models::HF_HOME_DIR));
    ForkDataLocations {
        data: text(data),
        models: text(&models),
        recordings: text(&data.join("recordings")),
        logs: text(logs),
        portable,
    }
}

fn locations<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<ForkDataLocations, ForkError> {
    let data = data_dir(app)?;
    let logs = diagnostics_dir(app)?;
    let hf_home = std::env::var_os("HF_HOME").map(PathBuf::from);
    Ok(locations_for(
        &data,
        &logs,
        hf_home.as_deref(),
        crate::portable::is_portable(),
    ))
}

#[tauri::command]
#[specta::specta]
pub fn fork_data_locations<R: ForkRuntime>(
    app: AppHandle<R>,
) -> Result<ForkDataLocations, ForkError> {
    locations(&app)
}

#[tauri::command]
#[specta::specta]
pub fn fork_show_data_location<R: ForkRuntime>(
    app: AppHandle<R>,
    kind: ForkDataLocationKind,
) -> Result<(), ForkError> {
    let locations = locations(&app)?;
    let path = match kind {
        ForkDataLocationKind::Data => locations.data,
        ForkDataLocationKind::Models => locations.models,
        ForkDataLocationKind::Recordings => locations.recordings,
        ForkDataLocationKind::Logs => locations.logs,
    };
    std::fs::create_dir_all(&path).map_err(|e| ForkError::Internal(e.to_string()))?;
    #[cfg(not(test))]
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| ForkError::Internal(e.to_string()))?;
    #[cfg(test)]
    let _ = path;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn fork_export_data<R: ForkRuntime>(
    app: AppHandle<R>,
    path: String,
) -> Result<ForkExportResult, ForkError> {
    let output = PathBuf::from(path);
    let data = data_dir(&app)?;
    let history = R::handy_history(&app);
    let target = output.clone();
    let ExportReport { files, bytes } = tauri::async_runtime::spawn_blocking(move || {
        // History rows through Handy's manager (R5 §4.3); its calls block.
        let entries = match history {
            Some(history) => history.entries()?,
            None => Vec::new(),
        };
        export::export_data(&data, &entries, &target)
    })
    .await
    .map_err(|e| ForkError::Internal(e.to_string()))?
    .map_err(ForkError::Internal)?;
    #[cfg(not(test))]
    app.opener()
        .reveal_item_in_dir(&output)
        .map_err(|e| ForkError::Internal(e.to_string()))?;
    log::info!("export: wrote {files} files ({bytes} bytes), revealed in Finder");
    Ok(ForkExportResult {
        path: output.to_string_lossy().into_owned(),
        files,
        bytes,
    })
}

fn diagnostics_dir<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<PathBuf, ForkError> {
    crate::fork::app_log_dir(app)
        .ok_or_else(|| ForkError::Internal("could not resolve the log folder".into()))
}

/// Puts the sanitized diagnostics text on the clipboard (native pasteboard,
/// like Scratchpad's copy); returns its length in bytes.
#[tauri::command]
#[specta::specta]
pub fn fork_copy_diagnostics<R: ForkRuntime>(app: AppHandle<R>) -> Result<u32, ForkError> {
    let text = bundle::copy_text(
        &diagnostics_dir(&app)?,
        &app.package_info().version.to_string(),
    );
    #[cfg(target_os = "macos")]
    crate::fork::clipboard::nspb::write_text(&text, false)
        .map_err(|error| ForkError::Internal(error.to_string()))?;
    #[cfg(not(target_os = "macos"))]
    return Err(ForkError::Internal(
        "copy diagnostics is available on macOS".into(),
    ));
    #[cfg(target_os = "macos")]
    Ok(u32::try_from(text.len()).unwrap_or(u32::MAX))
}

#[tauri::command]
#[specta::specta]
pub async fn fork_save_diagnostics<R: ForkRuntime>(
    app: AppHandle<R>,
    path: String,
) -> Result<u64, ForkError> {
    let logs = diagnostics_dir(&app)?;
    let version = app.package_info().version.to_string();
    let report = tauri::async_runtime::spawn_blocking(move || {
        bundle::save_bundle(&logs, &version, Path::new(&path))
    })
    .await
    .map_err(|e| ForkError::Internal(e.to_string()))?
    .map_err(ForkError::Internal)?;
    log::info!("diagnostics: saved {} sanitized files", report.files);
    Ok(report.bytes)
}

#[tauri::command]
#[specta::specta]
pub fn fork_erase_all_data<R: ForkRuntime>(
    app: AppHandle<R>,
    confirmation: String,
    delete_models: bool,
) -> Result<(), ForkError> {
    if !erase::valid_confirmation(&confirmation) {
        return Err(ForkError::InvalidSetting(
            "delete-all confirmation does not match".into(),
        ));
    }
    if !crate::fork::is_product_identity(&app.config().identifier) {
        return Err(ForkError::Internal(
            "delete all is disabled outside the Wispr Bro identity".into(),
        ));
    }
    if !app
        .try_state::<ForkState>()
        .is_some_and(|state| state.is_ready())
    {
        return Err(ForkError::NotReady);
    }
    let data = data_dir(&app)?;
    let logs = diagnostics_dir(&app)?;
    erase::erase_in_order(
        || R::stop_jobs_for_erase(&app).map_err(ForkError::Internal),
        || R::unload_models_for_erase(&app).map_err(ForkError::Internal),
        || {
            R::release_for_erase(&app);
            erase::erase_files(&data, &logs, delete_models)
                .map(|EraseReport { removed }| {
                    log::info!("erase: removed {removed} app-owned entries");
                })
                .map_err(ForkError::Internal)
        },
        || {
            std::thread::sleep(erase::WEBVIEW_CLEAR_GRACE);
            app.restart()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn location_kind_uses_stable_ipc_names() {
        for (kind, expected) in [
            (ForkDataLocationKind::Data, "data"),
            (ForkDataLocationKind::Models, "models"),
            (ForkDataLocationKind::Recordings, "recordings"),
            (ForkDataLocationKind::Logs, "logs"),
        ] {
            assert_eq!(serde_json::to_value(kind).unwrap(), expected);
        }
    }

    #[test]
    fn portable_mode_keeps_every_location_under_its_data_folder() {
        // E-88: portable sets HF_HOME to `Data/huggingface` and logs to
        // `Data/logs`; every folder the page shows is inside `Data/`.
        let data = Path::new("/Volumes/Stick/Wispr Bro/Data");
        let hf = data.join("huggingface");
        let l = locations_for(data, &data.join("logs"), Some(&hf), true);
        assert!(l.portable);
        for path in [&l.models, &l.recordings, &l.logs] {
            assert!(Path::new(path).starts_with(data), "{path}");
        }
        assert_eq!(l.models, hf.to_string_lossy());
    }

    #[test]
    fn installed_locations_use_app_support_logs_and_the_hf_cache() {
        let data = Path::new("/Users/a/Library/Application Support/com.wisprbro.desktop");
        let logs = Path::new("/Users/a/Library/Logs/com.wisprbro.desktop");
        let l = locations_for(data, logs, None, false);
        assert!(!l.portable);
        assert_eq!(l.models, data.join("huggingface").to_string_lossy());
        assert_eq!(l.recordings, data.join("recordings").to_string_lossy());
        assert_eq!(l.logs, logs.to_string_lossy());
    }

    /// S-27 exit criterion: every statement on the page is true. This pins
    /// the English page text to the code it describes; the rest is covered
    /// by `export`, `erase`, `diagnostics::bundle` tests and the egress check.
    #[test]
    fn page_statements_match_the_code() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/i18n/locales/en/fork.json");
        let en: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let page = &en["settingsModal"]["privacy"];
        let text = |a: &str, b: &str| page[a][b].as_str().unwrap().to_owned();

        // "the network only for these two things": Hugging Face and GitHub.
        assert!(text("promise", "models").contains("huggingface.co"));
        assert!(text("promise", "updates").contains("github.com"));
        let ours = |h: &str| {
            [
                "github.com",
                "githubusercontent.com",
                "huggingface.co",
                "hf.co",
            ]
            .iter()
            .any(|d| h == *d || h.ends_with(&format!(".{d}")))
        };
        for host in crate::fork::net::ALLOWED_HOSTS {
            assert!(ours(host), "{host}");
        }
        for suffix in crate::fork::net::ALLOWED_HOST_SUFFIXES {
            assert!(ours(suffix.trim_start_matches('.')), "{suffix}");
        }

        // Retention defaults: text forever, audio 14 days; updates on.
        let defaults = crate::fork::settings::ForkSettings::default();
        assert_eq!(defaults.text_retention_days, None);
        assert!(text("retention", "textHint").contains("forever by default"));
        assert_eq!(defaults.audio_retention_days, Some(14));
        assert!(text("retention", "audioHint").contains("14 days by default"));
        assert!(defaults.update_checks_enabled);

        // Export excludes models and logs (checked by `export` tests).
        assert!(text("export", "hint").contains("Models and logs are not included"));
        // Delete-all keeps models only when asked (checked by `erase` tests).
        assert!(text("delete", "modelsHint").contains("Keep this off"));
    }
}
