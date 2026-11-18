// SPDX-License-Identifier: GPL-3.0-or-later
//! Sync commands (`fork::sync`), the same on the Mac and the iPhone:
//!
//! - `fork_sync_status`: this device, the devices merged from, and a file
//!   another app handed over (AirDrop, Files) that waits for a review;
//! - `fork_sync_review`: reads a file and says what merging it would do
//!   (the merge itself, rolled back);
//! - `fork_sync_apply`: merges it;
//! - `fork_sync_export`: writes this device's `.wisprbro` file (and, when
//!   asked, the Dictionary and Snippets CSVs a Mac without sync imports
//!   through its own wizard). The Mac writes where the save panel said and
//!   shows the file in Finder; the iPhone writes to its temporary folder
//!   and opens the share sheet;
//! - `fork_sync_discard`: forgets the waiting file.
//!
//! Nothing here touches the network.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use super::ForkError;
use crate::fork::db::{DbError, ForkDb};
use crate::fork::settings::{ForkSettings, ForkSettingsHandle, HandySettings, ProductReport};
use crate::fork::sync::merge::{MergeReport, SyncChoices};
use crate::fork::sync::store::SyncDevice;
use crate::fork::sync::{self, Incoming, ReadProblem, SourceKind, BUNDLE_EXTENSION};
use crate::fork::{dictionary, snippets, ForkRuntime};

fn db<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<tauri::State<'_, ForkDb>, ForkError> {
    app.try_state::<ForkDb>().ok_or(ForkError::NotReady)
}

fn internal(e: DbError) -> ForkError {
    ForkError::Internal(e.to_string())
}

// ── This device ─────────────────────────────────────────────────────────

/// The name other devices show for this one: the Mac's host name as
/// words ("Finns-MacBook-Pro.local" → "Finns MacBook Pro"). An iPhone does
/// not tell apps its name; it is "iPhone".
fn device_name() -> String {
    if cfg!(target_os = "ios") {
        return "iPhone".into();
    }
    host_words(&sysinfo::System::host_name().unwrap_or_default()).unwrap_or_else(|| "Mac".into())
}

fn host_words(host: &str) -> Option<String> {
    let name = host.split('.').next().unwrap_or_default().replace('-', " ");
    let name: String = name.trim().chars().take(64).collect();
    (!name.is_empty() && !name.chars().any(char::is_control)).then_some(name)
}

type SettingValues = BTreeMap<String, Value>;

/// The synced settings as they are now, and their defaults.
fn settings_now<R: ForkRuntime>(
    app: &AppHandle<R>,
) -> Result<(SettingValues, SettingValues), ForkError> {
    let fork = app
        .try_state::<ForkSettingsHandle>()
        .ok_or(ForkError::NotReady)?
        .get(app)
        .map_err(ForkError::Internal)?;
    let handy = R::handy_settings(app).map(|HandySettings(s)| json(&s));
    let current = sync::settings::pick(&json(&fork), handy.as_ref());
    let defaults = sync::settings::pick(
        &json(&ForkSettings::default()),
        Some(&json(&crate::settings::get_default_settings())),
    );
    Ok((current, defaults))
}

fn json<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

/// Stamps the synced settings that changed since they were last seen
/// (`sync::settings::observe`). Called at start, after a fork setting
/// change and around every sync; failures are logged, never raised.
pub fn observe_settings<R: ForkRuntime>(app: &AppHandle<R>) {
    let Ok((current, defaults)) = settings_now(app) else {
        return;
    };
    let now = crate::fork::context::now_ms();
    let Some(db) = app.try_state::<ForkDb>() else {
        return;
    };
    if let Err(e) = db.with_conn(|c| sync::settings::observe(c, &current, &defaults, now)) {
        log::warn!("sync: could not record the settings' change times: {e}");
    }
}

/// Writes merged setting values to the two settings stores, through the
/// same paths the Settings pages use (validation, side effects, events).
fn write_settings<R: ForkRuntime>(app: &AppHandle<R>, changes: &BTreeMap<String, Value>) {
    for (key, value) in changes {
        if let Some(update) = sync::settings::fork_update(key, value) {
            if let Err(e) = super::settings::fork_set_setting(app.clone(), update) {
                log::warn!("sync: could not set {key}: {e}");
            }
        }
    }
    let handy_changes: Vec<_> = changes
        .iter()
        .filter(|(key, _)| sync::settings::HANDY_KEYS.contains(&key.as_str()))
        .collect();
    if handy_changes.is_empty() {
        return;
    }
    let Some(HandySettings(before)) = R::handy_settings(app) else {
        return;
    };
    // Field by field (never a JSON round trip of all of Handy's settings).
    let mut after = before.clone();
    for (key, value) in handy_changes {
        match key.as_str() {
            "spoken_languages" => {
                if let Ok(languages) = serde_json::from_value(value.clone()) {
                    after.spoken_languages = languages;
                }
            }
            "audio_feedback" => {
                if let Some(on) = value.as_bool() {
                    after.audio_feedback = on;
                }
            }
            "audio_feedback_volume" => {
                if let Some(volume) = value.as_f64() {
                    after.audio_feedback_volume = volume as f32;
                }
            }
            _ => {}
        }
    }
    let languages_changed = before.spoken_languages != after.spoken_languages;
    R::write_handy_settings(
        app,
        &HandySettings(before),
        HandySettings(after),
        &ProductReport::default(),
    );
    if languages_changed {
        // As a language change in Settings does: the speech model follows.
        crate::fork::models::setup::refresh();
        crate::fork::app_menu::request_refresh(crate::fork::app_menu::Refresh::Force);
    }
}

// ── A file handed over by another app ───────────────────────────────────

static PENDING: Mutex<Option<String>> = Mutex::new(None);

/// A file was opened in Wispr Bro from outside (AirDrop, Files, "Open
/// in…"). The Sync page reviews it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkSyncFileOpened {
    pub path: String,
}

/// Remembers the file and tells the page (if it is up).
pub fn file_opened<R: tauri::Runtime>(app: Option<&AppHandle<R>>, path: String) {
    log::info!("sync: a file was opened in the app");
    *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some(path.clone());
    if let Some(app) = app {
        if let Err(e) = (ForkSyncFileOpened { path }).emit(app) {
            log::debug!("sync: could not announce the opened file: {e}");
        }
    }
}

/// A path as the page has it: a plain path (the Mac's panels, AirDrop), or
/// the `file://` URL the iOS document picker returns.
fn local_path(text: &str) -> PathBuf {
    if text.starts_with("file://") {
        if let Some(path) = tauri::Url::parse(text)
            .ok()
            .and_then(|url| url.to_file_path().ok())
        {
            return path;
        }
    }
    PathBuf::from(text)
}

/// `/private/var/…` and `/var/…` are the same place on iOS.
fn without_private(path: &Path) -> PathBuf {
    match path.strip_prefix("/private") {
        Ok(rest) => Path::new("/").join(rest),
        Err(_) => path.to_path_buf(),
    }
}

/// Whether `path` is a copy iOS made for this app (the document picker's
/// copy, or the Inbox of "Open in…"), which the app deletes after use. A
/// Mac export holds history and recordings; it must not stay behind.
fn is_own_copy(path: &Path) -> bool {
    if !cfg!(target_os = "ios") {
        return false;
    }
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return false;
    };
    let (path, home) = (without_private(path), without_private(&home));
    path.starts_with(home.join("tmp")) || path.starts_with(home.join("Documents/Inbox"))
}

fn forget_file(path: &Path) {
    let mut pending = PENDING.lock().unwrap_or_else(|e| e.into_inner());
    if pending.as_deref().map(Path::new) == Some(path) {
        *pending = None;
    }
    drop(pending);
    if is_own_copy(path) {
        let _ = std::fs::remove_file(path);
    }
}

// ── Commands ────────────────────────────────────────────────────────────

/// The Sync page's data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SyncStatus {
    pub device_name: String,
    /// `macos` or `ios`.
    pub platform: String,
    /// The devices a file was merged from, the latest first.
    pub devices: Vec<SyncDevice>,
    /// When this device last made a file (Unix ms).
    pub last_export_at: Option<i64>,
    /// A file opened in the app from outside, waiting for a review.
    pub pending_file: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub fn fork_sync_status<R: ForkRuntime>(app: AppHandle<R>) -> Result<SyncStatus, ForkError> {
    observe_settings(&app);
    let (devices, last_export_at) = db(&app)?
        .with_conn(|c| Ok((sync::store::devices(c)?, sync::store::last_export_at(c)?)))
        .map_err(internal)?;
    Ok(SyncStatus {
        device_name: device_name(),
        platform: sync::store::platform().to_owned(),
        devices,
        last_export_at,
        pending_file: PENDING.lock().unwrap_or_else(|e| e.into_inner()).clone(),
    })
}

/// A file, read, and what merging it does.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SyncReview {
    Ready {
        source: SourceKind,
        /// The other device's name; empty when the file does not say.
        device_name: String,
        platform: String,
        app_version: String,
        /// When the file was made (Unix ms).
        created_at: i64,
        /// What the file holds at all (the review's switches).
        has_settings: bool,
        has_dictionary: bool,
        has_snippets: bool,
        has_stats: bool,
        has_notes: bool,
        report: MergeReport,
    },
    Problem {
        problem: ReadProblem,
    },
}

fn review_of(incoming: &Incoming, report: MergeReport) -> SyncReview {
    let p = &incoming.payload;
    SyncReview::Ready {
        source: incoming.kind,
        device_name: incoming.manifest.device_name.clone(),
        platform: incoming.manifest.platform.clone(),
        app_version: incoming.manifest.app_version.clone(),
        created_at: incoming.manifest.created_at,
        has_settings: !p.settings.is_empty(),
        has_dictionary: !p.dictionary.is_empty()
            || p.tombstones.iter().any(|t| t.kind != "snippet"),
        has_snippets: !p.snippets.is_empty() || p.tombstones.iter().any(|t| t.kind == "snippet"),
        has_stats: !p.stats.is_empty(),
        has_notes: !p.notes.is_empty() || !p.note_tombstones.is_empty(),
        report,
    }
}

/// Merges `incoming` into this device's database; `keep` commits it,
/// otherwise it is rolled back (a review). `None`: it is this device's own.
fn merge_incoming<R: ForkRuntime>(
    app: &AppHandle<R>,
    incoming: &Incoming,
    choices: SyncChoices,
    keep: bool,
) -> Result<Option<sync::merge::Outcome>, ForkError> {
    let now = crate::fork::context::now_ms();
    observe_settings(app);
    db(app)?
        .with_conn(|conn| {
            if sync::store::self_id(conn)? == incoming.manifest.device_id {
                return Ok(None);
            }
            let tx = conn.transaction()?;
            let outcome = sync::merge::merge(&tx, incoming, choices, now)?;
            if keep {
                tx.commit()?;
            }
            Ok(Some(outcome))
        })
        .map_err(internal)
}

/// What a committed merge leaves to do outside the database: the settings
/// stores, Handy's word list, and telling the pages.
fn after_merge<R: ForkRuntime>(app: &AppHandle<R>, outcome: &sync::merge::Outcome) {
    write_settings(app, &outcome.settings);
    let report = &outcome.report;
    let touched = |c: &sync::merge::SyncCounts| c.added + c.changed + c.removed > 0;
    if touched(&report.dictionary) {
        super::dictionary::changed(app);
    }
    if touched(&report.snippets) {
        snippets::notify_changed(app);
    }
    if touched(&report.notes) {
        crate::fork::scratchpad::changed(app);
    }
    if report.stats_changed {
        let day = crate::fork::stats::local_day(crate::fork::context::now_ms());
        let _ = (crate::fork::events::ForkStatsUpdated { day }).emit(app);
    }
}

/// Reads the file and runs the merge; `keep` commits it, otherwise it is
/// rolled back (the review).
fn run<R: ForkRuntime>(
    app: &AppHandle<R>,
    path: &Path,
    choices: SyncChoices,
    keep: bool,
) -> Result<(SyncReview, Option<sync::merge::Outcome>), ForkError> {
    let now = crate::fork::context::now_ms();
    let incoming = match sync::read(path, now) {
        Ok(incoming) => incoming,
        Err(problem) => return Ok((SyncReview::Problem { problem }, None)),
    };
    let outcome = merge_incoming(app, &incoming, choices, keep)?;
    Ok(match outcome {
        None => (
            SyncReview::Problem {
                problem: ReadProblem::OwnFile,
            },
            None,
        ),
        Some(outcome) => (review_of(&incoming, outcome.report.clone()), Some(outcome)),
    })
}

/// What merging the file at `path` would do. Changes nothing.
#[tauri::command]
#[specta::specta]
pub async fn fork_sync_review<R: ForkRuntime>(
    app: AppHandle<R>,
    path: String,
    choices: SyncChoices,
) -> Result<SyncReview, ForkError> {
    tauri::async_runtime::spawn_blocking(move || {
        run(&app, &local_path(&path), choices, false).map(|(review, _)| review)
    })
    .await
    .map_err(|e| ForkError::Internal(e.to_string()))?
}

/// Merges the file at `path`. The reply's report is what changed.
#[tauri::command]
#[specta::specta]
pub async fn fork_sync_apply<R: ForkRuntime>(
    app: AppHandle<R>,
    path: String,
    choices: SyncChoices,
) -> Result<SyncReview, ForkError> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = local_path(&path);
        let (review, outcome) = run(&app, &path, choices, true)?;
        let Some(outcome) = outcome else {
            return Ok(review);
        };
        after_merge(&app, &outcome);
        let report = &outcome.report;
        forget_file(&path);
        log::info!(
            "sync: merged a file (dictionary +{} ~{} -{}, snippets +{} ~{} -{}, notes +{} ~{} -{}, {} settings, {} stats days)",
            report.dictionary.added,
            report.dictionary.changed,
            report.dictionary.removed,
            report.snippets.added,
            report.snippets.changed,
            report.snippets.removed,
            report.notes.added,
            report.notes.changed,
            report.notes.removed,
            report.settings.len(),
            report.stats_days
        );
        Ok(review)
    })
    .await
    .map_err(|e| ForkError::Internal(e.to_string()))?
}

/// Forgets a file that was reviewed and not merged (and deletes the copy
/// iOS made of it).
#[tauri::command]
#[specta::specta]
pub fn fork_sync_discard(path: String) {
    forget_file(&local_path(&path));
}

/// The files `fork_sync_export` wrote.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SyncExport {
    /// The `.wisprbro` file first, then the CSVs.
    pub paths: Vec<String>,
    pub words: u32,
    pub snippets: u32,
}

/// A file name without characters a file system or a share sheet trips on.
fn file_stem(device: &str) -> String {
    let clean: String = device
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .collect();
    let clean = clean.trim();
    if clean.is_empty() {
        "Wispr Bro".into()
    } else {
        format!("Wispr Bro ({clean})")
    }
}

/// Writes this device's sync file into `folder` (or to `file`), plus the
/// CSVs when asked.
fn export_to<R: ForkRuntime>(
    app: &AppHandle<R>,
    file: Option<&Path>,
    folder: &Path,
    with_csv: bool,
) -> Result<SyncExport, ForkError> {
    observe_settings(app);
    let now = crate::fork::context::now_ms();
    let name = device_name();
    let version = app.package_info().version.to_string();
    let (manifest, payload, entries, list) = db(app)?
        .with_conn(|c| {
            let (manifest, payload) = sync::store::collect(c, &name, &version, now)?;
            Ok((
                manifest,
                payload,
                dictionary::store::list(c)?,
                snippets::store::list(c)?,
            ))
        })
        .map_err(internal)?;
    let bundle = match file {
        Some(file) => file.with_extension(BUNDLE_EXTENSION),
        None => folder.join(format!("{}.{BUNDLE_EXTENSION}", file_stem(&name))),
    };
    sync::bundle::write(&bundle, &manifest, &payload).map_err(ForkError::Internal)?;
    let mut paths = vec![bundle];
    if with_csv {
        let io = |e: std::io::Error| ForkError::Internal(e.to_string());
        let words = folder.join("Wispr Bro Dictionary.csv");
        dictionary::export::write_file(&words, &entries).map_err(io)?;
        let texts = folder.join("Wispr Bro Snippets.csv");
        snippets::export::write_file(&texts, &list).map_err(io)?;
        paths.extend([words, texts]);
    }
    db(app)?
        .with_conn(|c| sync::store::mark_exported(c, now))
        .map_err(internal)?;
    Ok(SyncExport {
        paths: paths
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
        words: u32::try_from(
            payload
                .dictionary
                .iter()
                .filter(|d| d.status == "active")
                .count(),
        )
        .unwrap_or(u32::MAX),
        snippets: u32::try_from(payload.snippets.len()).unwrap_or(u32::MAX),
    })
}

/// Makes this device's sync file. With `path` (the Mac's save panel) it is
/// written there and shown in Finder; without (the iPhone) it is written
/// to the temporary folder and handed to the share sheet. `with_csv` adds
/// the Dictionary and Snippets CSVs next to it.
#[tauri::command]
#[specta::specta]
pub async fn fork_sync_export<R: ForkRuntime>(
    app: AppHandle<R>,
    path: Option<String>,
    with_csv: bool,
) -> Result<SyncExport, ForkError> {
    tauri::async_runtime::spawn_blocking(move || {
        let file = path.map(PathBuf::from);
        let folder = match &file {
            Some(file) => file
                .parent()
                .filter(|p| p.is_dir())
                .map(Path::to_path_buf)
                .ok_or_else(|| ForkError::Internal("the folder does not exist".into()))?,
            None => {
                let folder = std::env::temp_dir().join("wispr-bro-sync");
                // Yesterday's files are not today's.
                let _ = std::fs::remove_dir_all(&folder);
                std::fs::create_dir_all(&folder).map_err(|e| ForkError::Internal(e.to_string()))?;
                folder
            }
        };
        let export = export_to(&app, file.as_deref(), &folder, with_csv)?;
        log::info!("sync: wrote {} file(s)", export.paths.len());
        present(&app, &export, file.is_some());
        Ok(export)
    })
    .await
    .map_err(|e| ForkError::Internal(e.to_string()))?
}

/// Shows what was written: Finder on the Mac, the share sheet on iPhone.
#[cfg(not(test))]
fn present<R: ForkRuntime>(app: &AppHandle<R>, export: &SyncExport, chosen: bool) {
    #[cfg(target_os = "ios")]
    {
        let _ = (app, chosen);
        sync::ios::share_files(&export.paths);
    }
    #[cfg(not(target_os = "ios"))]
    if chosen {
        use tauri_plugin_opener::OpenerExt;
        if let Some(first) = export.paths.first() {
            if let Err(e) = app.opener().reveal_item_in_dir(first) {
                log::warn!("sync: could not show the file in Finder: {e}");
            }
        }
    }
}

#[cfg(test)]
fn present<R: ForkRuntime>(_app: &AppHandle<R>, _export: &SyncExport, _chosen: bool) {}

// ── Automatic sync (fork::sync::auto) ───────────────────────────────────

/// The automatic-sync status changed; the page reads it again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkSyncAutoChanged {}

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod auto {
    use std::sync::Arc;

    use super::*;
    use crate::fork::sync::auto::{self, AutoStatus, SyncRole};
    use crate::fork::sync::secure::Identity;
    use crate::fork::sync::session::Host;
    use crate::fork::sync::{Manifest, Payload, SourceKind};

    /// The app as the sync engine sees it.
    struct AppSide<R: ForkRuntime>(AppHandle<R>);

    impl<R: ForkRuntime> Host for AppSide<R> {
        fn collect(&self) -> Option<(Manifest, Payload)> {
            observe_settings(&self.0);
            let version = self.0.package_info().version.to_string();
            let now = crate::fork::context::now_ms();
            db(&self.0)
                .ok()?
                .with_conn(|c| sync::store::collect(c, &device_name(), &version, now))
                .ok()
        }

        fn merge(&self, manifest: Manifest, payload: Payload) -> bool {
            let incoming = Incoming {
                kind: SourceKind::Bundle,
                manifest,
                payload,
            };
            match merge_incoming(&self.0, &incoming, SyncChoices::ALL, true) {
                Ok(Some(outcome)) => {
                    after_merge(&self.0, &outcome);
                    true
                }
                Ok(None) => false,
                Err(e) => {
                    log::warn!("sync: could not merge the other device's state: {e}");
                    false
                }
            }
        }
    }

    impl<R: ForkRuntime> auto::App for AppSide<R> {
        fn changed(&self) {
            let _ = (ForkSyncAutoChanged {}).emit(&self.0);
        }

        fn attention(&self) {
            // The code must be seen: show the Hub (the code is on a dialog
            // there, whatever page is up).
            #[cfg(not(target_os = "ios"))]
            crate::fork::hub::park::show_main_then(&self.0, || {});
        }

        fn app_state(&self) -> &'static str {
            #[cfg(target_os = "ios")]
            {
                crate::fork::ios::host::app_state()
            }
            #[cfg(not(target_os = "ios"))]
            {
                "active"
            }
        }
    }

    /// From `fork::on_ready`: starts automatic sync. It does nothing on the
    /// network until the user turns it on (Mac) or looks for a Mac (iPhone).
    pub fn start<R: ForkRuntime>(app: &AppHandle<R>) {
        let Ok(data) = crate::portable::data_dir()
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| app.path().app_data_dir())
        else {
            return;
        };
        let Some(device_id) = db(app)
            .ok()
            .and_then(|db| db.with_conn(|c| sync::store::self_id(c)).ok())
        else {
            log::warn!("sync: automatic sync is off (the database is not available)");
            return;
        };
        let mut role = if cfg!(target_os = "ios") {
            SyncRole::Phone
        } else {
            SyncRole::Mac
        };
        // Debug builds: a Mac can play the phone in a two-Mac check.
        if cfg!(debug_assertions) && std::env::var("WB_SYNC_ROLE").as_deref() == Ok("phone") {
            role = SyncRole::Phone;
        }
        let me = Identity {
            device_id,
            name: device_name(),
            platform: sync::store::platform().to_owned(),
        };
        auto::start(Arc::new(AppSide(app.clone())), role, me, &data);
    }

    pub fn status() -> Option<AutoStatus> {
        auto::engine().map(|e| e.status())
    }
}

/// From `fork::on_ready`.
pub fn start_auto<R: ForkRuntime>(app: &AppHandle<R>) {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    auto::start(app);
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    let _ = app;
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
use crate::fork::sync::auto::{engine, AutoStatus};

/// Automatic sync as the Sync pages show it; `None` before the app is ready.
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[tauri::command]
#[specta::specta]
pub fn fork_sync_auto_status() -> Option<AutoStatus> {
    auto::status()
}

/// The Mac's switch: advertise to and accept paired iPhones.
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[tauri::command]
#[specta::specta]
pub fn fork_sync_auto_enable(on: bool) {
    if let Some(engine) = engine() {
        engine.set_enabled(on);
    }
}

/// iPhone: look for Macs ("Find my Mac"), or stop looking.
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[tauri::command]
#[specta::specta]
pub fn fork_sync_search(on: bool) {
    if let Some(engine) = engine() {
        engine.search(on);
    }
}

/// iPhone: start pairing with the Mac found as `service`.
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[tauri::command]
#[specta::specta]
pub fn fork_sync_pair(service: String) {
    if let Some(engine) = engine() {
        engine.pair(service);
    }
}

/// iPhone: the code typed by the user; `None` cancels or dismisses.
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[tauri::command]
#[specta::specta]
pub fn fork_sync_pair_code(code: Option<String>) {
    if let Some(engine) = engine() {
        engine.pair_code(code.filter(|c| c.len() == 6 && c.chars().all(|c| c.is_ascii_digit())));
    }
}

/// Removes the pairing with `device_id`, on both devices when connected.
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[tauri::command]
#[specta::specta]
pub fn fork_sync_unpair(device_id: String) {
    if let Some(engine) = engine() {
        engine.unpair(&device_id);
    }
}

/// "Sync now": send this device's state at once and retry a lost Mac.
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[tauri::command]
#[specta::specta]
pub fn fork_sync_now() {
    if let Some(engine) = engine() {
        engine.sync_now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_host_name_reads_as_a_device_name() {
        assert_eq!(
            host_words("Finns-MacBook-Pro.local").as_deref(),
            Some("Finns MacBook Pro")
        );
        assert_eq!(host_words("studio").as_deref(), Some("studio"));
        assert_eq!(host_words(""), None);
        assert_eq!(host_words(".local"), None);
        assert!(!device_name().is_empty());
    }

    #[test]
    fn picker_urls_and_plain_paths_are_both_paths() {
        assert_eq!(
            local_path("file:///private/var/mobile/tmp/My%20Mac.zip"),
            PathBuf::from("/private/var/mobile/tmp/My Mac.zip")
        );
        assert_eq!(
            local_path("/Users/test/a file.wisprbro"),
            PathBuf::from("/Users/test/a file.wisprbro")
        );
        assert_eq!(
            without_private(Path::new("/private/var/mobile/x")),
            PathBuf::from("/var/mobile/x")
        );
        // On a Mac nothing is "the app's own copy": files are never deleted.
        assert!(!is_own_copy(Path::new("/private/var/mobile/tmp/x.zip")));
    }
}
