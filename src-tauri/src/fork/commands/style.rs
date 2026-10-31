// SPDX-License-Identifier: GPL-3.0-or-later
//! Style page commands (S-19a): the apps of each group (built-in table,
//! the user's rules, apps dictated into), the "+" picker's list of
//! installed apps, app icons, and the changes (move, remove, default group,
//! "Press Enter after dictating"). The presets per group are the fork
//! setting `styles` (`fork_set_setting`).
//!
//! Every change reloads the take path's cached rules
//! (`fork::context::app_rules::reload`) and emits `ForkAppRulesChanged`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};
use tauri_specta::Event;

use super::ForkError;
use crate::fork::context::app_rules::{self, AppRule, RuleKind};
use crate::fork::context::apps::{self, InstalledApp};
use crate::fork::context::categories::{self, StyleGroup};
use crate::fork::db::{DbError, ForkDb};
use crate::fork::ForkRuntime;

/// One app on the Style page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct StyleApp {
    pub bundle_id: String,
    pub name: String,
    /// Installed on this Mac (an uninstalled app shows only while a rule
    /// names it).
    pub installed: bool,
    /// The group it is in now.
    pub group: StyleGroup,
    /// The group the built-in table gives it.
    pub builtin_group: StyleGroup,
    /// The group the user chose, if any.
    pub user_group: Option<StyleGroup>,
    /// "Press Enter after dictating".
    pub auto_send: bool,
    /// An AI chat: its text goes in as cleaned, without a style (S-12a).
    pub ai_chat: bool,
}

/// The Style page's apps.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct StyleOverview {
    /// Every listed app, by name. The page groups them by `group`.
    pub apps: Vec<StyleApp>,
}

/// One entry of the "+" picker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct StylePickerApp {
    pub bundle_id: String,
    pub name: String,
    pub group: StyleGroup,
}

/// The user's app rules changed (another window refreshes its Style page).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkAppRulesChanged {
    pub rules: u32,
}

fn db<R: Runtime>(app: &AppHandle<R>) -> Result<tauri::State<'_, ForkDb>, ForkError> {
    app.try_state::<ForkDb>().ok_or(ForkError::NotReady)
}

fn internal(e: DbError) -> ForkError {
    ForkError::Internal(e.to_string())
}

/// This app itself never gets a style row.
fn is_self(bundle_id: &str) -> bool {
    let id = bundle_id.to_ascii_lowercase();
    id.starts_with("com.wisprbro.") || id == "com.pais.handy"
}

/// Apps the user dictated into (`entry_meta`), most used first.
fn dictated_apps(conn: &rusqlite::Connection) -> Result<Vec<(String, Option<String>)>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT app_bundle_id, MAX(app_name) FROM entry_meta
         WHERE app_bundle_id IS NOT NULL AND app_bundle_id <> '' AND deleted_at IS NULL
         GROUP BY lower(app_bundle_id) ORDER BY COUNT(*) DESC LIMIT 200",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Builds the page's list: the user's rules, the installed apps of the
/// built-in table and the installed apps the user dictated into.
pub fn overview(
    rules: &[AppRule],
    installed: &[InstalledApp],
    dictated: &[(String, Option<String>)],
) -> StyleOverview {
    let by_id: HashMap<String, &InstalledApp> = installed
        .iter()
        .map(|a| (a.bundle_id.to_lowercase(), a))
        .collect();
    // (bundle id, fallback name), first occurrence wins.
    let mut candidates: Vec<(String, Option<String>)> = Vec::new();
    for r in rules.iter().filter(|r| r.kind == RuleKind::App) {
        candidates.push((r.key.clone(), r.app_name.clone()));
    }
    for (id, _) in categories::known_apps() {
        if by_id.contains_key(&id.to_lowercase()) {
            candidates.push((id.clone(), None));
        }
    }
    for (id, name) in dictated {
        if by_id.contains_key(&id.to_lowercase()) {
            candidates.push((id.clone(), name.clone()));
        }
    }
    let mut seen = std::collections::HashSet::new();
    let mut apps: Vec<StyleApp> = Vec::new();
    for (id, fallback) in candidates {
        if is_self(&id) || !seen.insert(id.to_lowercase()) {
            continue;
        }
        let installed = by_id.get(&id.to_lowercase()).copied();
        let name = installed
            .map(|a| a.name.clone())
            .or(fallback)
            .unwrap_or_else(|| id.clone());
        let resolved = app_rules::resolve(rules, Some(&id), Some(&name), None);
        let class = categories::classify(Some(&id), Some(&name));
        let user_group = rules
            .iter()
            .find(|r| r.kind == RuleKind::App && r.key.eq_ignore_ascii_case(&id))
            .and_then(|r| r.style_group);
        apps.push(StyleApp {
            bundle_id: installed.map_or(id.clone(), |a| a.bundle_id.clone()),
            name,
            installed: installed.is_some(),
            group: resolved.group,
            builtin_group: resolved.builtin_group,
            user_group,
            auto_send: resolved.auto_send,
            ai_chat: class.is_ai_chat,
        });
    }
    apps.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.bundle_id.cmp(&b.bundle_id))
    });
    StyleOverview { apps }
}

fn current_overview<R: Runtime>(app: &AppHandle<R>) -> Result<StyleOverview, ForkError> {
    let (rules, dictated) = db(app)?
        .with_conn(|c| Ok((app_rules::list(c)?, dictated_apps(c)?)))
        .map_err(internal)?;
    Ok(overview(&rules, &apps::installed(), &dictated))
}

/// After a change: the take path's cache, then the other windows.
fn changed<R: Runtime>(app: &AppHandle<R>) -> Result<StyleOverview, ForkError> {
    let rules = db(app)?
        .with_conn(|c| app_rules::reload(c))
        .map_err(internal)?;
    let event = ForkAppRulesChanged {
        rules: u32::try_from(rules.len()).unwrap_or(u32::MAX),
    };
    if let Err(e) = event.emit(app) {
        log::debug!("style: could not emit the change event: {e}");
    }
    current_overview(app)
}

fn checked_id(bundle_id: &str) -> Result<String, ForkError> {
    let id = bundle_id.trim();
    let valid = !id.is_empty()
        && id.len() <= 255
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    if !valid || is_self(id) {
        return Err(ForkError::InvalidSetting(format!(
            "not an app's bundle id: {bundle_id:?}"
        )));
    }
    Ok(id.to_string())
}

/// The name to store: the installed app's, else what the page sent.
fn name_for(bundle_id: &str, sent: Option<&str>) -> Option<String> {
    apps::locate(bundle_id)
        .map(|a| a.name)
        .or_else(|| sent.map(|s| s.trim().chars().take(120).collect()))
}

/// The Style page's apps.
#[tauri::command]
#[specta::specta]
pub fn fork_style_overview<R: ForkRuntime>(app: AppHandle<R>) -> Result<StyleOverview, ForkError> {
    current_overview(&app)
}

/// Installed apps for the "+" picker, by name, with their current group.
#[tauri::command]
#[specta::specta]
pub fn fork_style_installed_apps<R: ForkRuntime>(
    app: AppHandle<R>,
) -> Result<Vec<StylePickerApp>, ForkError> {
    let rules = db(&app)?
        .with_conn(|c| app_rules::list(c))
        .map_err(internal)?;
    apps::forget_scan();
    Ok(apps::installed()
        .into_iter()
        .filter(|a| !is_self(&a.bundle_id))
        .map(|a| StylePickerApp {
            group: app_rules::resolve(&rules, Some(&a.bundle_id), Some(&a.name), None).group,
            bundle_id: a.bundle_id,
            name: a.name,
        })
        .collect())
}

/// An app's icon as a PNG `data:` URL (`None` if it is not installed).
#[tauri::command]
#[specta::specta]
pub async fn fork_style_app_icon(bundle_id: String) -> Result<Option<String>, ForkError> {
    tauri::async_runtime::spawn_blocking(move || apps::icon_data_url(&bundle_id))
        .await
        .map_err(|e| ForkError::Internal(e.to_string()))
}

/// Puts an app in `group`; `None` returns it to its built-in group.
#[tauri::command]
#[specta::specta]
pub fn fork_style_set_app_group<R: ForkRuntime>(
    app: AppHandle<R>,
    bundle_id: String,
    name: Option<String>,
    group: Option<StyleGroup>,
) -> Result<StyleOverview, ForkError> {
    let id = checked_id(&bundle_id)?;
    let name = name_for(&id, name.as_deref());
    let now = crate::fork::context::now_ms();
    db(&app)?
        .with_conn(|c| app_rules::set_group(c, &id, name.as_deref(), group, now))
        .map_err(internal)?;
    log::info!(
        "style: {id} → {}",
        group.map_or("its default group", |g| g.as_str())
    );
    changed(&app)
}

/// Takes an app out of its group (it joins "all other apps").
#[tauri::command]
#[specta::specta]
pub fn fork_style_remove_app<R: ForkRuntime>(
    app: AppHandle<R>,
    bundle_id: String,
    name: Option<String>,
) -> Result<StyleOverview, ForkError> {
    let id = checked_id(&bundle_id)?;
    let name = name_for(&id, name.as_deref());
    let now = crate::fork::context::now_ms();
    db(&app)?
        .with_conn(|c| app_rules::remove_from_group(c, &id, name.as_deref(), now))
        .map_err(internal)?;
    log::info!("style: {id} removed from its group");
    changed(&app)
}

/// "Press Enter after dictating" for one app.
#[tauri::command]
#[specta::specta]
pub fn fork_style_set_auto_send<R: ForkRuntime>(
    app: AppHandle<R>,
    bundle_id: String,
    name: Option<String>,
    on: bool,
) -> Result<StyleOverview, ForkError> {
    let id = checked_id(&bundle_id)?;
    let name = name_for(&id, name.as_deref());
    let now = crate::fork::context::now_ms();
    db(&app)?
        .with_conn(|c| app_rules::set_auto_send(c, &id, name.as_deref(), on, now))
        .map_err(internal)?;
    log::info!("style: {id} press Enter after dictating {on}");
    changed(&app)
}

/// Loads the rules into the take path's cache (`fork::on_ready`).
pub fn on_ready<R: Runtime>(app: &AppHandle<R>) {
    let Some(db) = app.try_state::<ForkDb>() else {
        return;
    };
    match db.with_conn(|c| app_rules::reload(c)) {
        Ok(r) => log::info!("style: {} app rule(s)", r.len()),
        Err(e) => log::warn!("style: could not load the app rules: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inst(id: &str, name: &str) -> InstalledApp {
        InstalledApp {
            bundle_id: id.into(),
            name: name.into(),
            path: format!("/Applications/{name}.app"),
        }
    }

    #[test]
    fn overview_lists_rules_known_and_dictated_apps() {
        let installed = vec![
            inst("com.tinyspeck.slackmacgap", "Slack"),
            inst("com.apple.mail", "Mail"),
            inst("com.apple.MobileSMS", "Messages"),
            inst("com.example.editor", "Editor"),
            inst("com.example.unused", "Unused"),
            inst("com.openai.chat", "ChatGPT"),
            inst("com.wisprbro.desktop", "Wispr Bro"),
        ];
        let rules = vec![
            AppRule {
                key: "com.apple.MobileSMS".into(),
                kind: RuleKind::App,
                style_group: Some(StyleGroup::Work),
                auto_send: true,
                app_name: Some("Messages".into()),
                updated_at: 1,
            },
            AppRule {
                key: "com.example.gone".into(),
                kind: RuleKind::App,
                style_group: Some(StyleGroup::Email),
                auto_send: false,
                app_name: Some("Gone".into()),
                updated_at: 2,
            },
        ];
        let dictated = vec![
            ("com.example.editor".to_string(), Some("Editor".to_string())),
            (
                "com.example.uninstalled".to_string(),
                Some("Old".to_string()),
            ),
            ("com.wisprbro.desktop".to_string(), None),
        ];
        let o = overview(&rules, &installed, &dictated);
        let find = |id: &str| o.apps.iter().find(|a| a.bundle_id == id);
        let slack = find("com.tinyspeck.slackmacgap").unwrap();
        assert_eq!((slack.group, slack.user_group), (StyleGroup::Work, None));
        let messages = find("com.apple.MobileSMS").unwrap();
        assert_eq!(messages.group, StyleGroup::Work);
        assert_eq!(messages.builtin_group, StyleGroup::Personal);
        assert!(messages.auto_send);
        assert_eq!(find("com.apple.mail").unwrap().group, StyleGroup::Email);
        let gone = find("com.example.gone").unwrap();
        assert!(!gone.installed);
        assert_eq!(
            (gone.name.as_str(), gone.group),
            ("Gone", StyleGroup::Email)
        );
        assert_eq!(find("com.example.editor").unwrap().group, StyleGroup::Other);
        assert!(find("com.openai.chat").unwrap().ai_chat);
        // Not listed: installed but never used or known, uninstalled and
        // never named by a rule, and this app itself.
        assert!(find("com.example.unused").is_none());
        assert!(find("com.example.uninstalled").is_none());
        assert!(find("com.wisprbro.desktop").is_none());
        // By name.
        let names: Vec<&str> = o.apps.iter().map(|a| a.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort_by_key(|n| n.to_lowercase());
        assert_eq!(names, sorted);
    }

    #[test]
    fn bundle_ids_are_checked() {
        assert!(checked_id(" com.apple.mail ").is_ok());
        assert!(checked_id("").is_err());
        assert!(checked_id("a b").is_err());
        assert!(checked_id("x'); DROP TABLE app_rules; --").is_err());
        assert!(checked_id("com.wisprbro.desktop").is_err());
    }
}
