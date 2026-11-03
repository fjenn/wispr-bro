// SPDX-License-Identifier: GPL-3.0-or-later
//! Our update check (F-02, E-54, DEC-33).
//!
//! `tauri-plugin-updater` 2.10.1 does the work: it fetches `latest.json`,
//! compares versions, downloads the `.app.tar.gz` and verifies its minisign
//! signature against `plugins.updater.pubkey` from the overlay (our key; the
//! identity guard and `channel::tests` pin it) before anything is installed.
//! This module only chooses the endpoints for the user's channel
//! ([`channel`], through `updater_builder().endpoints(…)`), refuses any
//! download that is not one of our GitHub release assets, and, on the beta
//! channel, takes the newer of the beta and stable releases.
//!
//! Callers: the fork debug panel ("Check now" / "Install") and the
//! debug-only `WISPRBRO_UPDATE_SELFTEST` hook the Mac update test drives
//! (F-02); S-28's background [`scheduler`] (idle gate, failures, quiet
//! relaunch) behind the Hub footer, the About page and the tray.

pub mod channel;
pub mod failures;
pub mod idle_gate;
pub mod relaunch;
pub mod scheduler;
#[cfg(test)]
mod tests;

use std::time::Duration;

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::fork::settings::{ForkSettingsHandle, UpdateChannel};

/// Network timeout of one `latest.json` request or download.
pub const TIMEOUT: Duration = Duration::from_secs(60);
/// Debug builds only: check, download, verify and install at startup, then
/// relaunch (the scripted 0.x → 0.x+1 test, `scripts/wb-f02/`).
pub const SELFTEST_ENV: &str = "WISPRBRO_UPDATE_SELFTEST";

/// A release newer than the running app.
#[derive(Serialize, Debug, Clone, PartialEq, Eq, Type)]
pub struct ForkAvailableUpdate {
    pub version: String,
    /// `pub_date` from `latest.json` (RFC 3339), when present.
    pub date: Option<String>,
    /// Release notes from `latest.json`.
    pub notes: Option<String>,
    /// The `latest.json` it came from.
    pub endpoint: String,
}

/// Result of one check.
#[derive(Serialize, Debug, Clone, PartialEq, Eq, Type)]
pub struct ForkUpdateCheck {
    pub current_version: String,
    pub channel: UpdateChannel,
    /// `None`: up to date.
    pub available: Option<ForkAvailableUpdate>,
}

/// What the debug panel shows before a check.
#[derive(Serialize, Debug, Clone, PartialEq, Eq, Type)]
pub struct ForkUpdateStatus {
    pub current_version: String,
    pub channel: UpdateChannel,
    pub endpoints: Vec<String>,
    /// Handy's effective setting (`update_checks_enabled` and the
    /// `HANDY_DISABLE_UPDATER` / `WISPRBRO_DISABLE_UPDATER` kill switches).
    pub checks_enabled: bool,
    /// minisign key id downloads are verified against.
    pub pubkey_id: String,
}

/// Why a check or install did not run or failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateError {
    /// Update checks are off (setting or kill switch).
    Disabled,
    /// Network, HTTP, JSON or signature failure (message for logs and the
    /// debug panel; never user text).
    Failed(String),
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateError::Disabled => write!(f, "update checks are off"),
            UpdateError::Failed(m) => write!(f, "{m}"),
        }
    }
}

/// The user's channel (stable when the fork settings are not loaded).
pub fn current_channel<R: Runtime>(app: &AppHandle<R>) -> UpdateChannel {
    app.try_state::<ForkSettingsHandle>()
        .and_then(|h| h.get(app).ok())
        .map(|s| s.update_channel)
        .unwrap_or_default()
}

/// One endpoint's answer, reduced to what [`pick_newest`] compares.
pub(crate) struct Found<T> {
    pub version: semver::Version,
    pub item: T,
}

/// Picks the newest release over the channel's endpoints. One failing
/// endpoint is fine while another answered (a beta channel before the first
/// beta release exists); all failing is an error (the first one's).
pub(crate) fn pick_newest<T>(
    answers: Vec<Result<Option<Found<T>>, String>>,
) -> Result<Option<T>, String> {
    let mut first_error = None;
    let mut answered = false;
    let mut best: Option<Found<T>> = None;
    for answer in answers {
        match answer {
            Ok(found) => {
                answered = true;
                if let Some(found) = found {
                    if best.as_ref().is_none_or(|b| found.version > b.version) {
                        best = Some(found);
                    }
                }
            }
            Err(e) => {
                first_error.get_or_insert(e);
            }
        }
    }
    match (answered, first_error) {
        (false, Some(e)) => Err(e),
        _ => Ok(best.map(|b| b.item)),
    }
}

/// Checks the channel's endpoints. Returns the newest update newer than the
/// running app, or `None`.
async fn find<R: Runtime>(
    app: &AppHandle<R>,
    channel: UpdateChannel,
) -> Result<Option<(Update, String)>, String> {
    find_on(app, channel::endpoints(channel)?).await
}

/// [`find`] on explicit endpoints (tests serve them on loopback).
pub(crate) async fn find_on<R: Runtime>(
    app: &AppHandle<R>,
    endpoints: Vec<reqwest::Url>,
) -> Result<Option<(Update, String)>, String> {
    let mut answers = Vec::with_capacity(endpoints.len());
    for endpoint in endpoints {
        let shown = endpoint.to_string();
        let answer = async {
            let updater = app
                .updater_builder()
                .endpoints(vec![endpoint])
                .map_err(|e| e.to_string())?
                .timeout(TIMEOUT)
                .build()
                .map_err(|e| e.to_string())?;
            match updater.check().await {
                Ok(Some(update)) => {
                    let version = semver::Version::parse(&update.version)
                        .map_err(|e| format!("bad version {:?}: {e}", update.version))?;
                    Ok(Some(Found {
                        version,
                        item: (update, shown.clone()),
                    }))
                }
                Ok(None) => Ok(None),
                Err(e) => Err(format!("{shown}: {e}")),
            }
        }
        .await;
        if let Err(e) = &answer {
            log::warn!("update: check failed: {e}");
        }
        answers.push(answer);
    }
    pick_newest(answers)
}

fn checks_enabled<R: crate::fork::ForkRuntime>(app: &AppHandle<R>) -> bool {
    R::handy_settings(app)
        .map(|h| crate::settings::update_checks_effectively_enabled(&h.0))
        .unwrap_or(false)
}

/// Status for the debug panel.
pub fn status<R: crate::fork::ForkRuntime>(app: &AppHandle<R>) -> ForkUpdateStatus {
    let channel = current_channel(app);
    ForkUpdateStatus {
        current_version: app.package_info().version.to_string(),
        channel,
        endpoints: channel::endpoints(channel)
            .map(|v| v.into_iter().map(|u| u.to_string()).collect())
            .unwrap_or_default(),
        checks_enabled: checks_enabled(app),
        pubkey_id: channel::PUBKEY_KEY_ID.to_string(),
    }
}

/// Checks for an update on the user's channel.
pub async fn check<R: crate::fork::ForkRuntime>(
    app: &AppHandle<R>,
) -> Result<ForkUpdateCheck, UpdateError> {
    if !checks_enabled(app) {
        return Err(UpdateError::Disabled);
    }
    let channel = current_channel(app);
    let available = find(app, channel)
        .await
        .map_err(UpdateError::Failed)?
        .map(|(update, endpoint)| available(&update, endpoint));
    log::info!(
        "update: checked {channel:?}: {}",
        available
            .as_ref()
            .map(|a| format!("{} available", a.version))
            .unwrap_or_else(|| "up to date".into())
    );
    Ok(ForkUpdateCheck {
        current_version: app.package_info().version.to_string(),
        channel,
        available,
    })
}

fn available(update: &Update, endpoint: String) -> ForkAvailableUpdate {
    ForkAvailableUpdate {
        version: update.version.clone(),
        date: update
            .raw_json
            .get("pub_date")
            .and_then(|d| d.as_str())
            .map(str::to_owned),
        notes: update.body.clone(),
        endpoint,
    }
}

/// Downloads `update`, verifies its signature against our pubkey and returns
/// the verified bytes. Refuses a download URL that is not one of our release
/// assets (R5 §1: GitHub releases only).
pub async fn download_verified(update: &Update) -> Result<Vec<u8>, String> {
    let url = update.download_url.as_str();
    if !channel::is_update_url(url) {
        return Err(format!("refusing update download from {url}"));
    }
    update
        .download(|_, _| {}, || {})
        .await
        .map_err(|e| format!("download or signature check failed: {e}"))
}

/// Checks, downloads, verifies and installs the newest update, then
/// relaunches the app. Returns `Ok(None)` when already up to date.
pub async fn install<R: crate::fork::ForkRuntime>(
    app: &AppHandle<R>,
) -> Result<Option<String>, UpdateError> {
    if !checks_enabled(app) {
        return Err(UpdateError::Disabled);
    }
    let channel = current_channel(app);
    let Some((update, _)) = find(app, channel).await.map_err(UpdateError::Failed)? else {
        return Ok(None);
    };
    let bytes = download_verified(&update)
        .await
        .map_err(UpdateError::Failed)?;
    log::info!(
        "update: {} verified ({} bytes), installing",
        update.version,
        bytes.len()
    );
    update
        .install(bytes)
        .map_err(|e| UpdateError::Failed(format!("install failed: {e}")))?;
    log::info!("update: installed {}, relaunching", update.version);
    Ok(Some(update.version.clone()))
}

/// `WISPRBRO_UPDATE_SELFTEST=1` (debug builds only): one check + install at
/// startup, logged as `fork update selftest: …`, then a relaunch.
pub fn start_selftest<R: crate::fork::ForkRuntime>(app: &AppHandle<R>) {
    if !cfg!(debug_assertions) || !crate::utils::env_flag_enabled(SELFTEST_ENV) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        log::info!(
            "fork update selftest: v{} on {:?}",
            app.package_info().version,
            current_channel(&app)
        );
        match install(&app).await {
            Ok(Some(version)) => {
                log::info!("fork update selftest: INSTALLED {version}");
                app.restart();
            }
            Ok(None) => log::info!("fork update selftest: UP-TO-DATE"),
            Err(e) => log::warn!("fork update selftest: FAIL {e}"),
        }
    });
}
