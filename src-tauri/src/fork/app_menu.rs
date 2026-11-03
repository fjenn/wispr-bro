// SPDX-License-Identifier: GPL-3.0-or-later
//! Wispr Bro's application menus and tray contents (S-26a, W-11).
//!
//! One small cached [`MenuSnapshot`] (counts, choices and a shortened
//! last-transcript preview, never the whole transcript) feeds both native
//! menus. It is read on a worker thread; database and device reads never run
//! on the main thread. Menus are described by pure [`MenuSpec`] trees, so the
//! structure is unit-tested without AppKit.
//!
//! - **App menu:** Tauri's `Menu::default` (so Edit keeps ⌘C/⌘V/⌘Z/⌘A and
//!   Window keeps ⌘M), labels localized from `fork/i18n.rs`, plus Dictation and
//!   Vocabulary after Edit and our Help items in Tauri's Help submenu, which
//!   is registered with `set_as_help_menu_for_nsapp()` so macOS adds its
//!   search box.
//! - **Tray:** Handy's `tray.rs` `build_menu` hook ([`super::tray::extend`])
//!   replaces Handy's items with [`tray_specs`]. Handy rebuilds its tray only
//!   when its own inputs change, so count and "last transcript" changes are
//!   applied here to the tray menu that hook last built.
//!
//! Live changes update the existing native items in place (text, enabled,
//! checked); a menu is rebuilt only when its shape changes (a microphone
//! plugged in, a language added) or the app language changes.
//!
//! S-26b adds Report a problem… (⌘/) and Send feedback… to Help and the
//! tray.
//!
//! Later slices add items by putting a [`MenuSpec`] with a stable id into the
//! spec builders below ([`dictation_spec`], [`vocabulary_spec`],
//! [`help_items`], [`tray_specs`]) and handling the id in [`on_menu`].

use std::collections::BTreeMap;
use std::sync::{LazyLock, Mutex, OnceLock};

use serde::Serialize;
use specta::Type;
use tauri::menu::{
    CheckMenuItem, Menu, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu, HELP_SUBMENU_ID,
    WINDOW_SUBMENU_ID,
};
use tauri::{AppHandle, Listener, Manager, Runtime};
use tauri_specta::Event;

use super::audio::transport;
use super::commands::audio::ForkMicEntry;
use super::db::ForkDb;
use super::dictionary::store as dictionary_store;
use super::recovery::last;
use super::settings::{ForkSettingsHandle, HandySettings};
use super::snippets::store as snippets_store;
use super::{i18n, ForkRuntime};

pub const DICTATION_MENU_ID: &str = "fork.dictation";
pub const VOCABULARY_MENU_ID: &str = "fork.vocabulary";
pub const HELP_MENU_ID: &str = HELP_SUBMENU_ID;
pub const WORDS_ID: &str = "fork.dictation.words";
pub const COPY_LAST_ID: &str = "fork.copy_last";
pub const PASTE_LAST_ID: &str = "fork.paste_last";
pub const DOCUMENTATION_ID: &str = "fork.documentation";
pub const SHORTCUTS_ID: &str = "fork.shortcuts";
/// S-26b: Help › Report a problem… (⌘/) and Send feedback… (app menu and
/// tray): a prefilled GitHub issue in the browser.
pub const REPORT_PROBLEM_ID: &str = "fork.report_problem";
pub const SEND_FEEDBACK_ID: &str = "fork.send_feedback";
pub const DICTIONARY_COUNT_ID: &str = "fork.vocabulary.dictionary_count";
pub const SNIPPET_COUNT_ID: &str = "fork.vocabulary.snippet_count";
pub const ADD_WORD_ID: &str = "fork.add_word";
pub const NEW_SNIPPET_ID: &str = "fork.new_snippet";
pub const HELP_MICROPHONE_ID: &str = "fork.help.microphone";
pub const HELP_LANGUAGES_ID: &str = "fork.help.languages";
pub const NO_MICROPHONES_ID: &str = "fork.microphone.none";
pub const MORE_LANGUAGES_ID: &str = "fork.languages.more";
pub const MICROPHONE_PREFIX: &str = "fork.microphone:";
pub const LANGUAGE_PREFIX: &str = "fork.language:";
/// Handy's tray ids, kept so Handy's tray handler still runs them.
pub const TRAY_OPEN_ID: &str = "settings";
pub const TRAY_UPDATES_ID: &str = "check_updates";
/// S-28: shown instead of Check for updates… while update checks are off;
/// opens Settings › Data and Privacy (Handy's handler ignores the id).
pub const TRAY_UPDATES_OFF_ID: &str = "fork.updates_off";
/// S-28: shown instead of Check for updates… once an update is downloaded.
pub const TRAY_RESTART_UPDATE_ID: &str = "fork.restart_update";
pub const TRAY_CANCEL_ID: &str = "cancel";
pub const TRAY_QUIT_ID: &str = "quit";
pub const TRAY_SECURE_INPUT_ID: &str = "secure_input_warning";
/// Help › Documentation: the user docs in the public repository (R2 C-102:
/// hosted help is out; GitHub is one of the two hosts the app may open).
pub const DOCUMENTATION_URL: &str =
    "https://github.com/fjenn/wispr-bro/tree/wispr-bro/docs/fork/user";

/// The last transcript as menus show it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LastMenuTranscript {
    /// One line, at most [`PREVIEW_CHARS`] characters; `None` when concealed.
    pub preview: Option<String>,
    /// A password-field take (S-05): its text is never shown.
    pub concealed: bool,
}

/// One of the user's languages (Handy's `spoken_languages`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuLanguage {
    pub code: String,
    pub name: String,
}

/// Everything both menus show.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuSnapshot {
    pub locale: String,
    pub total_words: u64,
    pub dictionary_words: u32,
    pub snippets: u32,
    pub last: Option<LastMenuTranscript>,
    pub microphones: Vec<ForkMicEntry>,
    pub selected_microphone: String,
    pub languages: Vec<MenuLanguage>,
    pub update_checks_enabled: bool,
    pub updates_forced_off: bool,
    /// S-28: the downloaded update's version (Restart to update).
    pub update_ready: Option<String>,
}

static SNAPSHOT: LazyLock<Mutex<MenuSnapshot>> =
    LazyLock::new(|| Mutex::new(MenuSnapshot::default()));

/// The current snapshot (a clone).
pub fn snapshot() -> MenuSnapshot {
    SNAPSHOT.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

fn store_snapshot(next: MenuSnapshot) -> bool {
    let mut current = SNAPSHOT.lock().unwrap_or_else(|p| p.into_inner());
    if *current == next {
        return false;
    }
    *current = next;
    true
}

// ---------------------------------------------------------------- specs ----

/// A native menu entry, independent of AppKit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuSpec {
    Item {
        id: String,
        label: String,
        enabled: bool,
        /// `Some` for a check item.
        checked: Option<bool>,
    },
    Submenu {
        id: String,
        label: String,
        enabled: bool,
        items: Vec<MenuSpec>,
    },
    Separator,
}

impl MenuSpec {
    pub fn item(id: impl Into<String>, label: String, enabled: bool) -> Self {
        Self::Item {
            id: id.into(),
            label,
            enabled,
            checked: None,
        }
    }

    pub fn check(id: impl Into<String>, label: String, checked: bool) -> Self {
        Self::Item {
            id: id.into(),
            label,
            enabled: true,
            checked: Some(checked),
        }
    }

    pub fn submenu(id: impl Into<String>, label: String, items: Vec<MenuSpec>) -> Self {
        Self::Submenu {
            id: id.into(),
            label,
            enabled: true,
            items,
        }
    }

    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Item { id, .. } | Self::Submenu { id, .. } => Some(id),
            Self::Separator => None,
        }
    }
}

fn text(locale: &str, key: &str) -> String {
    i18n::t(locale, key, &[])
}

fn app_text(locale: &str, key: &str) -> String {
    let app_name = text(locale, "rust.appName");
    i18n::t(locale, key, &[("appName", &app_name)])
}

/// A `{{count}}` plural group (`<key>_one`, `<key>_few`, …), resolved with
/// the locale's CLDR plural rules (`i18n::t_plural`).
fn count_text(locale: &str, key: &str, count: u64) -> String {
    i18n::t_plural(locale, key, count, &[])
}

/// Longest preview in "Copy last transcript: “…”".
pub const PREVIEW_CHARS: usize = 40;

/// A one-line, shortened preview of a transcript; `None` for blank text.
pub fn preview(text: &str) -> Option<String> {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.is_empty() {
        return None;
    }
    if one_line.chars().count() <= PREVIEW_CHARS {
        return Some(one_line);
    }
    let mut short = one_line.chars().take(PREVIEW_CHARS - 1).collect::<String>();
    while short.ends_with(|c: char| c.is_whitespace() || c.is_ascii_punctuation()) {
        short.pop();
    }
    Some(format!("{short}…"))
}

fn microphone_spec(snapshot: &MenuSnapshot, id: &str) -> MenuSpec {
    let locale = snapshot.locale.as_str();
    let mut items: Vec<MenuSpec> = snapshot
        .microphones
        .iter()
        .map(|mic| {
            let label = if mic.kind == super::audio::choice::MicEntryKind::Auto {
                text(locale, "rust.menu.automaticMicrophone")
            } else {
                mic.name.clone()
            };
            MenuSpec::check(
                format!("{MICROPHONE_PREFIX}{}", mic.value),
                label,
                mic.value == snapshot.selected_microphone,
            )
        })
        .collect();
    if items.is_empty() {
        items.push(MenuSpec::item(
            NO_MICROPHONES_ID,
            text(locale, "rust.menu.noMicrophones"),
            false,
        ));
    }
    MenuSpec::submenu(id, text(locale, "rust.menu.microphone"), items)
}

/// The user's languages, checked, then "Add or remove languages…" (Settings
/// › General › Languages). The full list of ~100 languages lives in Settings'
/// searchable picker; a language added from a menu could start a large model
/// download by one stray click.
fn language_spec(snapshot: &MenuSnapshot, id: &str) -> MenuSpec {
    let locale = snapshot.locale.as_str();
    let mut items: Vec<MenuSpec> = snapshot
        .languages
        .iter()
        .map(|language| {
            MenuSpec::check(
                format!("{LANGUAGE_PREFIX}{}", language.code),
                language.name.clone(),
                true,
            )
        })
        .collect();
    if !items.is_empty() {
        items.push(MenuSpec::Separator);
    }
    items.push(MenuSpec::item(
        MORE_LANGUAGES_ID,
        text(locale, "rust.menu.moreLanguages"),
        true,
    ));
    MenuSpec::submenu(id, text(locale, "rust.menu.languages"), items)
}

fn copy_last_label(snapshot: &MenuSnapshot) -> String {
    let locale = snapshot.locale.as_str();
    match &snapshot.last {
        Some(last) if last.concealed => text(locale, "rust.menu.copyLastConcealed"),
        Some(LastMenuTranscript {
            preview: Some(preview),
            ..
        }) => i18n::t(locale, "rust.menu.copyLastPreview", &[("preview", preview)]),
        _ => text(locale, "rust.menu.copyLast"),
    }
}

/// Dictation: "N words dictated", Copy last transcript (with preview),
/// Microphone ▸, Languages ▸.
pub fn dictation_spec(snapshot: &MenuSnapshot) -> MenuSpec {
    let locale = snapshot.locale.as_str();
    MenuSpec::submenu(
        DICTATION_MENU_ID,
        text(locale, "rust.menu.dictation"),
        vec![
            MenuSpec::item(
                WORDS_ID,
                count_text(locale, "rust.menu.wordsDictated", snapshot.total_words),
                false,
            ),
            MenuSpec::item(
                COPY_LAST_ID,
                copy_last_label(snapshot),
                snapshot.last.is_some(),
            ),
            MenuSpec::Separator,
            microphone_spec(snapshot, "fork.dictation.microphone"),
            language_spec(snapshot, "fork.dictation.languages"),
        ],
    )
}

/// Vocabulary: dictionary and snippet counts, Add word…, New snippet….
pub fn vocabulary_spec(snapshot: &MenuSnapshot) -> MenuSpec {
    let locale = snapshot.locale.as_str();
    MenuSpec::submenu(
        VOCABULARY_MENU_ID,
        text(locale, "rust.menu.vocabulary"),
        vec![
            MenuSpec::item(
                DICTIONARY_COUNT_ID,
                count_text(
                    locale,
                    "rust.menu.dictionaryWords",
                    u64::from(snapshot.dictionary_words),
                ),
                false,
            ),
            MenuSpec::item(
                SNIPPET_COUNT_ID,
                count_text(
                    locale,
                    "rust.menu.snippetCount",
                    u64::from(snapshot.snippets),
                ),
                false,
            ),
            MenuSpec::Separator,
            MenuSpec::item(ADD_WORD_ID, text(locale, "rust.menu.addWord"), true),
            MenuSpec::item(NEW_SNIPPET_ID, text(locale, "rust.menu.newSnippet"), true),
        ],
    )
}

/// Help's items (macOS adds its search box above them).
pub fn help_items(snapshot: &MenuSnapshot) -> Vec<MenuSpec> {
    let locale = snapshot.locale.as_str();
    vec![
        MenuSpec::item(
            DOCUMENTATION_ID,
            text(locale, "rust.menu.documentation"),
            true,
        ),
        MenuSpec::Separator,
        MenuSpec::item(SHORTCUTS_ID, text(locale, "rust.menu.shortcuts"), true),
        MenuSpec::item(
            HELP_MICROPHONE_ID,
            text(locale, "rust.menu.microphone"),
            true,
        ),
        MenuSpec::item(HELP_LANGUAGES_ID, text(locale, "rust.menu.languages"), true),
        MenuSpec::Separator,
        MenuSpec::item(
            REPORT_PROBLEM_ID,
            text(locale, "rust.menu.reportProblem"),
            true,
        ),
        MenuSpec::item(
            SEND_FEEDBACK_ID,
            text(locale, "rust.menu.sendFeedback"),
            true,
        ),
    ]
}

pub fn help_spec(snapshot: &MenuSnapshot) -> MenuSpec {
    MenuSpec::submenu(
        HELP_MENU_ID,
        text(snapshot.locale.as_str(), "rust.menu.help"),
        help_items(snapshot),
    )
}

/// The tray menu. `busy`: a take is recording/transcribing (Handy's Cancel
/// stays); `warning`: Secure Input blocks the shortcuts (Handy's warning
/// item stays on top).
pub fn tray_specs(snapshot: &MenuSnapshot, busy: bool, warning: bool) -> Vec<MenuSpec> {
    let locale = snapshot.locale.as_str();
    let mut items = Vec::new();
    if warning {
        // S-02: the same item says so when the shortcut is lost (the
        // health check), not blocked by Secure Input.
        let key = if crate::fork::shortcut::health::tray_warning() {
            "rust.tray.shortcutLost"
        } else {
            "rust.menu.shortcutsBlocked"
        };
        items.push(MenuSpec::item(
            TRAY_SECURE_INPUT_ID,
            text(locale, key),
            true,
        ));
        items.push(MenuSpec::Separator);
    }
    if busy {
        items.push(MenuSpec::item(
            TRAY_CANCEL_ID,
            text(locale, "rust.menu.cancelDictation"),
            true,
        ));
        items.push(MenuSpec::Separator);
    }
    items.push(MenuSpec::item(
        TRAY_OPEN_ID,
        app_text(locale, "rust.menu.openApp"),
        true,
    ));
    // Forced off (HANDY_DISABLE_UPDATER/WISPRBRO_DISABLE_UPDATER): dropped
    // rather than greyed, as Handy does. S-28: the item uses `fork/update`
    // and says so when checks are off instead of greying out.
    if !snapshot.updates_forced_off {
        items.push(
            match (&snapshot.update_ready, snapshot.update_checks_enabled) {
                (Some(version), true) => MenuSpec::item(
                    TRAY_RESTART_UPDATE_ID,
                    i18n::t(locale, "rust.menu.restartToUpdate", &[("version", version)]),
                    true,
                ),
                (_, true) => MenuSpec::item(
                    TRAY_UPDATES_ID,
                    text(locale, "rust.menu.checkUpdates"),
                    true,
                ),
                (_, false) => MenuSpec::item(
                    TRAY_UPDATES_OFF_ID,
                    text(locale, "rust.menu.updatesOff"),
                    true,
                ),
            },
        );
    }
    items.push(MenuSpec::item(
        PASTE_LAST_ID,
        text(locale, "rust.menu.pasteLast"),
        snapshot.last.is_some(),
    ));
    items.push(MenuSpec::Separator);
    items.push(MenuSpec::item(
        SHORTCUTS_ID,
        text(locale, "rust.menu.shortcuts"),
        true,
    ));
    items.push(microphone_spec(snapshot, "fork.tray.microphone"));
    items.push(language_spec(snapshot, "fork.tray.languages"));
    items.push(MenuSpec::Separator);
    items.push(MenuSpec::item(
        DOCUMENTATION_ID,
        text(locale, "rust.menu.documentation"),
        true,
    ));
    items.push(MenuSpec::item(
        REPORT_PROBLEM_ID,
        text(locale, "rust.menu.reportProblem"),
        true,
    ));
    items.push(MenuSpec::item(
        SEND_FEEDBACK_ID,
        text(locale, "rust.menu.sendFeedback"),
        true,
    ));
    items.push(MenuSpec::Separator);
    items.push(MenuSpec::item(
        TRAY_QUIT_ID,
        app_text(locale, "rust.menu.quitApp"),
        true,
    ));
    items
}

// ----------------------------------------------------- native menus -------

/// A menu or submenu, so one code path fills both.
enum Parent<'a, R: Runtime> {
    Menu(&'a Menu<R>),
    Submenu(&'a Submenu<R>),
}

impl<R: Runtime> Parent<'_, R> {
    fn items(&self) -> tauri::Result<Vec<MenuItemKind<R>>> {
        match self {
            Self::Menu(m) => m.items(),
            Self::Submenu(m) => m.items(),
        }
    }

    fn append(&self, item: &dyn tauri::menu::IsMenuItem<R>) -> tauri::Result<()> {
        match self {
            Self::Menu(m) => m.append(item),
            Self::Submenu(m) => m.append(item),
        }
    }

    /// Removes every item.
    fn clear(&self) -> tauri::Result<()> {
        for _ in 0..self.items()?.len() {
            match self {
                Self::Menu(m) => m.remove_at(0)?,
                Self::Submenu(m) => m.remove_at(0)?,
            };
        }
        Ok(())
    }
}

/// Key equivalents: Quit (⌘Q) and Report a problem (⌘/, as Wispr's
/// "Talk to support").
pub fn accelerator(id: &str) -> Option<&'static str> {
    let mac = cfg!(target_os = "macos");
    match id {
        TRAY_QUIT_ID => Some(if mac { "Cmd+Q" } else { "Ctrl+Q" }),
        REPORT_PROBLEM_ID => Some(if mac { "Cmd+/" } else { "Ctrl+/" }),
        _ => None,
    }
}

fn append_spec<R: Runtime>(
    app: &AppHandle<R>,
    parent: &Parent<'_, R>,
    spec: &MenuSpec,
) -> tauri::Result<()> {
    match spec {
        MenuSpec::Item {
            id,
            label,
            enabled,
            checked: Some(checked),
        } => parent.append(&CheckMenuItem::with_id(
            app,
            id,
            label,
            *enabled,
            *checked,
            accelerator(id),
        )?),
        MenuSpec::Item {
            id,
            label,
            enabled,
            checked: None,
        } => parent.append(&MenuItem::with_id(
            app,
            id,
            label,
            *enabled,
            accelerator(id),
        )?),
        MenuSpec::Submenu {
            id,
            label,
            enabled,
            items,
        } => {
            let submenu = Submenu::with_id(app, id, label, *enabled)?;
            for item in items {
                append_spec(app, &Parent::Submenu(&submenu), item)?;
            }
            parent.append(&submenu)
        }
        MenuSpec::Separator => parent.append(&PredefinedMenuItem::separator(app)?),
    }
}

/// Whether `items` have the shape of `specs` (same kinds and ids in order).
fn same_shape<R: Runtime>(items: &[MenuItemKind<R>], specs: &[MenuSpec]) -> tauri::Result<bool> {
    if items.len() != specs.len() {
        return Ok(false);
    }
    for (item, spec) in items.iter().zip(specs) {
        let same = match (item, spec) {
            (MenuItemKind::Predefined(_), MenuSpec::Separator) => true,
            (MenuItemKind::Check(i), MenuSpec::Item { id, checked, .. }) => {
                checked.is_some() && i.id().0 == *id
            }
            (MenuItemKind::MenuItem(i), MenuSpec::Item { id, checked, .. }) => {
                checked.is_none() && i.id().0 == *id
            }
            (MenuItemKind::Submenu(s), MenuSpec::Submenu { id, items, .. }) => {
                s.id().0 == *id && same_shape(&s.items()?, items)?
            }
            _ => false,
        };
        if !same {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Brings items of the same shape up to date (only what differs).
fn update_in_place<R: Runtime>(items: &[MenuItemKind<R>], specs: &[MenuSpec]) -> tauri::Result<()> {
    for (item, spec) in items.iter().zip(specs) {
        match (item, spec) {
            (MenuItemKind::MenuItem(i), MenuSpec::Item { label, enabled, .. }) => {
                if i.text()? != *label {
                    i.set_text(label)?;
                }
                if i.is_enabled()? != *enabled {
                    i.set_enabled(*enabled)?;
                }
            }
            (
                MenuItemKind::Check(i),
                MenuSpec::Item {
                    label,
                    enabled,
                    checked,
                    ..
                },
            ) => {
                if i.text()? != *label {
                    i.set_text(label)?;
                }
                if i.is_enabled()? != *enabled {
                    i.set_enabled(*enabled)?;
                }
                // Always re-asserted: AppKit flips a check item on click,
                // and a refused click (the last language) must flip back.
                i.set_checked(checked.unwrap_or_default())?;
            }
            (
                MenuItemKind::Submenu(s),
                MenuSpec::Submenu {
                    label,
                    enabled,
                    items,
                    ..
                },
            ) => {
                if s.text()? != *label {
                    s.set_text(label)?;
                }
                if s.is_enabled()? != *enabled {
                    s.set_enabled(*enabled)?;
                }
                update_in_place(&s.items()?, items)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Makes `parent` show `specs`: in place when the shape is unchanged,
/// otherwise by replacing its contents.
fn sync_children<R: Runtime>(
    app: &AppHandle<R>,
    parent: &Parent<'_, R>,
    specs: &[MenuSpec],
) -> tauri::Result<()> {
    let items = parent.items()?;
    if same_shape(&items, specs)? {
        return update_in_place(&items, specs);
    }
    parent.clear()?;
    for spec in specs {
        append_spec(app, parent, spec)?;
    }
    Ok(())
}

/// Replaces a menu's contents with `specs` (the tray hook).
pub fn replace_menu<R: Runtime>(
    app: &AppHandle<R>,
    menu: &Menu<R>,
    specs: &[MenuSpec],
) -> tauri::Result<()> {
    let parent = Parent::Menu(menu);
    parent.clear()?;
    for spec in specs {
        append_spec(app, &parent, spec)?;
    }
    Ok(())
}

/// The localization key for one of `Menu::default`'s predefined items, by
/// muda's macOS title (`muda-0.19.3/src/items/predefined.rs`,
/// `platform_impl/macos/mod.rs`).
fn predefined_key(title: &str) -> Option<&'static str> {
    Some(match title {
        "Undo" => "rust.menu.undo",
        "Redo" => "rust.menu.redo",
        "Cut" => "rust.menu.cut",
        "Copy" => "rust.menu.copy",
        "Paste" => "rust.menu.paste",
        "Select All" => "rust.menu.selectAll",
        "Minimize" => "rust.menu.minimize",
        "Zoom" | "Maximize" => "rust.menu.zoom",
        "Close Window" | "Close" => "rust.menu.closeWindow",
        "Toggle Full Screen" => "rust.menu.fullScreen",
        "Services" => "rust.menu.services",
        "Hide Others" => "rust.menu.hideOthers",
        "Show All" => "rust.menu.showAll",
        t if t.starts_with("About ") || t == "About" => "rust.menu.about",
        t if t.starts_with("Hide ") || t == "Hide" => "rust.menu.hide",
        t if t.starts_with("Quit ") || t == "Quit" => "rust.menu.quitApp",
        _ => return None,
    })
}

/// The localization key for one of `Menu::default`'s submenus.
fn submenu_key(title: &str) -> Option<&'static str> {
    Some(match title {
        "File" => "rust.menu.file",
        "Edit" => "rust.menu.edit",
        "View" => "rust.menu.view",
        "Window" => "rust.menu.window",
        "Help" => "rust.menu.help",
        _ => return None,
    })
}

fn localize_predefined<R: Runtime>(
    item: &PredefinedMenuItem<R>,
    locale: &str,
) -> tauri::Result<()> {
    let title = item.text()?;
    if title.is_empty() {
        return Ok(()); // a separator
    }
    match predefined_key(&title) {
        Some(key) => item.set_text(app_text(locale, key)),
        None => {
            log::warn!("app menu: unlocalized predefined item {title:?}");
            Ok(())
        }
    }
}

/// Localizes `Menu::default`'s English labels in place.
fn localize_defaults<R: Runtime>(
    app: &AppHandle<R>,
    menu: &Menu<R>,
    locale: &str,
) -> tauri::Result<()> {
    for item in menu.items()? {
        let MenuItemKind::Submenu(submenu) = item else {
            continue;
        };
        let label = submenu.text()?;
        if let Some(key) = submenu_key(&label) {
            submenu.set_text(text(locale, key))?;
        } else if label == app.package_info().name {
            submenu.set_text(text(locale, "rust.appName"))?;
        }
        for child in submenu.items()? {
            if let MenuItemKind::Predefined(predefined) = child {
                localize_predefined(&predefined, locale)?;
            }
        }
    }
    Ok(())
}

/// The locale the installed app menu was built in (`None`: not installed).
static INSTALLED_LOCALE: Mutex<Option<String>> = Mutex::new(None);

/// Builds and installs the whole app menu. Main thread only.
fn install_app_menu<R: Runtime>(app: &AppHandle<R>, snapshot: &MenuSnapshot) -> tauri::Result<()> {
    let menu = Menu::default(app)?;
    localize_defaults(app, &menu, &snapshot.locale)?;

    // Dictation and Vocabulary go right after Edit.
    let items = menu.items()?;
    let edit_label = text(&snapshot.locale, "rust.menu.edit");
    let after_edit = items
        .iter()
        .position(|item| {
            matches!(item, MenuItemKind::Submenu(s) if s.text().is_ok_and(|t| t == edit_label))
        })
        .map_or(items.len().saturating_sub(2), |i| i + 1);
    for (offset, spec) in [dictation_spec(snapshot), vocabulary_spec(snapshot)]
        .iter()
        .enumerate()
    {
        let MenuSpec::Submenu {
            id,
            label,
            enabled,
            items,
        } = spec
        else {
            continue;
        };
        let submenu = Submenu::with_id(app, id, label, *enabled)?;
        for item in items {
            append_spec(app, &Parent::Submenu(&submenu), item)?;
        }
        menu.insert(&submenu, after_edit + offset)?;
    }

    match menu.get(HELP_MENU_ID) {
        Some(MenuItemKind::Submenu(help)) => {
            sync_children(app, &Parent::Submenu(&help), &help_items(snapshot))?;
            #[cfg(target_os = "macos")]
            help.set_as_help_menu_for_nsapp()?;
        }
        _ => log::warn!("app menu: Tauri's Help submenu was not found"),
    }
    #[cfg(target_os = "macos")]
    if let Some(MenuItemKind::Submenu(window)) = menu.get(WINDOW_SUBMENU_ID) {
        window.set_as_windows_menu_for_nsapp()?;
    }
    app.set_menu(menu)?;
    Ok(())
}

/// Updates the installed app menu's own submenus to `snapshot`.
fn sync_app_menu<R: Runtime>(app: &AppHandle<R>, snapshot: &MenuSnapshot) -> tauri::Result<()> {
    let installed = INSTALLED_LOCALE
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    let menu = app.menu();
    let Some(menu) = menu.filter(|_| installed.as_deref() == Some(snapshot.locale.as_str())) else {
        install_app_menu(app, snapshot)?;
        *INSTALLED_LOCALE.lock().unwrap_or_else(|p| p.into_inner()) = Some(snapshot.locale.clone());
        return Ok(());
    };
    for spec in [dictation_spec(snapshot), vocabulary_spec(snapshot)] {
        let MenuSpec::Submenu { id, items, .. } = &spec else {
            continue;
        };
        if let Some(MenuItemKind::Submenu(submenu)) = menu.get(id.as_str()) {
            sync_children(app, &Parent::Submenu(&submenu), items)?;
        }
    }
    Ok(())
}

/// The tray menu Handy's hook last filled (Handy keeps the `TrayIcon`).
struct TrayMenu<R: Runtime>(Mutex<Option<Menu<R>>>);

/// Updates the tray menu the hook last built to `snapshot`.
fn sync_tray_menu<R: ForkRuntime>(
    app: &AppHandle<R>,
    snapshot: &MenuSnapshot,
) -> tauri::Result<()> {
    let Some(slot) = app.try_state::<TrayMenu<R>>() else {
        return Ok(());
    };
    let menu = slot.0.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let Some(menu) = menu else {
        return Ok(());
    };
    let specs = tray_specs(
        snapshot,
        R::menu_dictation_in_progress(app),
        R::menu_shortcuts_blocked(app),
    );
    sync_children(app, &Parent::Menu(&menu), &specs)
}

/// Main thread: both menus to the current snapshot.
fn apply_on_main<R: ForkRuntime>(app: &AppHandle<R>) {
    let snapshot = snapshot();
    if let Err(error) = sync_app_menu(app, &snapshot) {
        log::warn!("app menu: could not update the app menu: {error}");
    }
    if super::is_product_build() {
        if let Err(error) = sync_tray_menu(app, &snapshot) {
            log::warn!("app menu: could not update the tray menu: {error}");
        }
    }
}

/// The `tray.rs` hook's part (main thread, inside Handy's tray apply): fills
/// Handy's freshly built tray menu from the snapshot and remembers it for
/// live updates. A changed app language is picked up here too, since
/// Handy's language command rebuilds the tray.
pub fn fill_tray<R: ForkRuntime>(
    app: &AppHandle<R>,
    menu: &Menu<R>,
    handy_locale: &str,
) -> tauri::Result<()> {
    let locale = i18n::current_locale(Some(handy_locale)).to_string();
    let mut current = snapshot();
    if current.locale != locale {
        current.locale = locale;
        store_snapshot(current.clone());
        if let Err(error) = sync_app_menu(app, &current) {
            log::warn!("app menu: could not relocalize the app menu: {error}");
        }
    }
    let specs = tray_specs(
        &current,
        R::menu_dictation_in_progress(app),
        R::menu_shortcuts_blocked(app),
    );
    replace_menu(app, menu, &specs)?;
    // Handy builds its first tray inside its setup closure, before
    // [`on_ready`]: the slot is created here so that menu gets live updates.
    if app.try_state::<TrayMenu<R>>().is_none() {
        app.manage(TrayMenu::<R>(Mutex::new(None)));
    }
    if let Some(slot) = app.try_state::<TrayMenu<R>>() {
        *slot.0.lock().unwrap_or_else(|p| p.into_inner()) = Some(menu.clone());
    }
    Ok(())
}

// ------------------------------------------------------------ refresh ------

/// Native names for the language codes Handy's picker offers, so a menu shows
/// "Deutsch", not "de" (the same names macOS shows in its own language lists).
fn language_name(code: &str) -> String {
    static NAMES: LazyLock<BTreeMap<String, String>> = LazyLock::new(|| {
        serde_json::from_str(include_str!("app_menu/languages.json"))
            .expect("native language names parse")
    });
    NAMES.get(code).cloned().unwrap_or_else(|| code.to_string())
}

#[derive(Default)]
struct RefreshState {
    running: bool,
    again: bool,
    /// Re-list the microphones (device or choice changes), not on each take.
    devices: bool,
    /// Re-apply native state even if the snapshot is unchanged (a clicked
    /// check item flips itself).
    force: bool,
}

static REFRESH: LazyLock<Mutex<RefreshState>> =
    LazyLock::new(|| Mutex::new(RefreshState::default()));

/// What a refresh re-reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refresh {
    /// Counts, last transcript and settings.
    Data,
    /// Also the microphone list.
    Devices,
    /// Data, and re-apply native state even when nothing changed.
    Force,
}

fn read_snapshot<R: ForkRuntime>(app: &AppHandle<R>, devices: bool) -> MenuSnapshot {
    let previous = snapshot();
    let handy = R::handy_settings(app).map(|HandySettings(settings)| settings);
    let app_language = R::handy_app_language(app);
    let locale = i18n::current_locale(app_language.as_deref()).to_string();

    let (microphones, selected_microphone) = if devices || previous.microphones.is_empty() {
        let follow_system = app
            .try_state::<ForkSettingsHandle>()
            .and_then(|handle| handle.get(app).ok())
            .is_some_and(|settings| settings.mic_follow_system);
        let selected = handy
            .as_ref()
            .and_then(|settings| settings.selected_microphone.clone());
        let devices = super::commands::audio::recorder_devices().unwrap_or_else(|error| {
            log::warn!("app menu: could not list microphones: {error}");
            Vec::new()
        });
        let list = super::commands::audio::build_list(
            &devices,
            &transport::snapshot(),
            selected.as_deref(),
            follow_system,
        );
        (list.entries, list.selected)
    } else {
        (previous.microphones, previous.selected_microphone)
    };

    let languages = handy
        .as_ref()
        .map(|settings| settings.spoken_languages.clone())
        .unwrap_or_default()
        .into_iter()
        .map(|code| MenuLanguage {
            name: language_name(&code),
            code,
        })
        .collect();

    let mut snapshot = MenuSnapshot {
        locale,
        microphones,
        selected_microphone,
        languages,
        update_checks_enabled: handy
            .as_ref()
            .is_some_and(|settings| settings.update_checks_enabled),
        updates_forced_off: crate::settings::update_checks_forced_disabled(),
        update_ready: {
            let update = super::update::scheduler::state(app);
            (update.phase == super::update::scheduler::ForkUpdatePhase::Ready)
                .then_some(update.version)
                .flatten()
        },
        ..MenuSnapshot::default()
    };
    let Some(db) = app.try_state::<ForkDb>() else {
        return snapshot;
    };
    let read = db.with_conn(|conn| {
        let stats = super::db::stats::summary(conn, chrono::Local::now().date_naive())?;
        let dictionary = dictionary_store::counts(conn)?;
        let snippets = snippets_store::counts(conn)?;
        let last = last::select(conn)?;
        Ok((stats, dictionary, snippets, last))
    });
    match read {
        Ok((stats, dictionary, snippets, latest)) => {
            snapshot.total_words = stats.total_words;
            snapshot.dictionary_words = dictionary.words.saturating_add(dictionary.rules);
            snapshot.snippets = snippets.snippets;
            snapshot.last = latest.map(|entry| LastMenuTranscript {
                preview: (!entry.concealed).then(|| preview(&entry.text)).flatten(),
                concealed: entry.concealed,
            });
        }
        Err(error) => log::warn!("app menu: could not read live counts: {error}"),
    }
    snapshot
}

/// Re-reads the snapshot on a worker and updates both menus when it changed.
/// Bursts coalesce into one read; never blocks the caller.
pub fn refresh<R: ForkRuntime>(app: &AppHandle<R>, kind: Refresh) {
    if !R::native_menus() {
        return;
    }
    {
        let mut state = REFRESH.lock().unwrap_or_else(|p| p.into_inner());
        state.devices |= kind == Refresh::Devices;
        state.force |= kind == Refresh::Force;
        if state.running {
            state.again = true;
            return;
        }
        state.running = true;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("wb-menu-refresh".into())
        .spawn(move || loop {
            let (devices, force) = {
                let mut state = REFRESH.lock().unwrap_or_else(|p| p.into_inner());
                state.again = false;
                (
                    std::mem::take(&mut state.devices),
                    std::mem::take(&mut state.force),
                )
            };
            let changed = store_snapshot(read_snapshot(&app, devices));
            if changed || force {
                let handle = app.clone();
                if let Err(error) = app.run_on_main_thread(move || apply_on_main(&handle)) {
                    log::debug!("app menu: event loop is gone: {error}");
                }
            }
            let mut state = REFRESH.lock().unwrap_or_else(|p| p.into_inner());
            if !state.again {
                state.running = false;
                break;
            }
        });
    if let Err(error) = spawned {
        REFRESH.lock().unwrap_or_else(|p| p.into_inner()).running = false;
        log::warn!("app menu: could not start the refresh worker: {error}");
    }
}

/// [`refresh`] for callers without an `AppHandle` (Handy-command observers,
/// [`super::observe_handy_command`]). A no-op before [`on_ready`].
pub fn request_refresh(kind: Refresh) {
    if let Some(hook) = REFRESH_HOOK.get() {
        hook(kind);
    }
}

type RefreshHook = Box<dyn Fn(Refresh) + Send + Sync>;
static REFRESH_HOOK: OnceLock<RefreshHook> = OnceLock::new();

// ------------------------------------------------------------- actions -----

/// A Hub deep link requested by a native menu: `navigate(page, section)`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type, tauri_specta::Event)]
pub struct ForkMenuAction {
    pub page: String,
    pub section: Option<String>,
}

/// Shows the Hub (rebuilt if S-38 parked it) and asks it to open
/// `page`/`section` once its page listens.
fn open_hub<R: ForkRuntime>(app: &AppHandle<R>, page: &str, section: Option<&str>) {
    let event = ForkMenuAction {
        page: page.to_string(),
        section: section.map(str::to_string),
    };
    let handle = app.clone();
    super::hub::park::show_main_then(app, move || {
        if let Err(error) = event.emit(&handle) {
            log::warn!("app menu: could not send the Hub action: {error}");
        }
    });
}

/// What a click on a language does: the new list, or `None` to keep it.
/// A checked language is removed unless it is the last one; adding happens
/// in Settings (see [`language_spec`]).
pub fn toggle_language(spoken: &[String], code: &str) -> Option<Vec<String>> {
    if spoken.len() <= 1 || !spoken.iter().any(|l| l == code) {
        return None;
    }
    Some(spoken.iter().filter(|l| *l != code).cloned().collect())
}

fn choose_language<R: ForkRuntime>(app: &AppHandle<R>, code: &str) {
    let code = code.to_string();
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("wb-menu-language".into())
        .spawn(move || {
            if let Some(HandySettings(before)) = R::handy_settings(&app) {
                if let Some(languages) = toggle_language(&before.spoken_languages, &code) {
                    let mut after = before.clone();
                    after.spoken_languages = languages;
                    R::write_handy_settings(
                        &app,
                        &HandySettings(before),
                        HandySettings(after),
                        &super::settings::ProductReport::default(),
                    );
                    // As Settings' language change does (S-07b): re-pick.
                    super::models::setup::refresh();
                    log::info!("app menu: a language was removed from the menu");
                }
            }
            refresh(&app, Refresh::Force);
        });
    if let Err(error) = spawned {
        log::warn!("app menu: could not change languages: {error}");
    }
}

fn choose_microphone<R: ForkRuntime>(app: &AppHandle<R>, value: &str) {
    let value = value.to_string();
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("wb-menu-microphone".into())
        .spawn(move || {
            if let Err(error) = super::commands::audio::choose(&app, value) {
                log::warn!("app menu: could not choose the microphone: {error}");
            }
            refresh(&app, Refresh::Devices);
            refresh(&app, Refresh::Force);
        });
    if let Err(error) = spawned {
        log::warn!("app menu: could not choose the microphone: {error}");
    }
}

/// Every menu click (app menu and tray). Handy's tray handler runs its own
/// ids (`settings`, `check_updates`, `cancel`, `quit`, `secure_input_warning`);
/// for `check_updates` it only shows the Hub (its own checker is not
/// mounted), and the check itself runs here through `fork/update` (S-28).
fn on_menu<R: ForkRuntime>(app: &AppHandle<R>, id: &str) {
    match id {
        TRAY_UPDATES_ID => {
            super::update::scheduler::check_now(app);
        }
        TRAY_UPDATES_OFF_ID => open_hub(app, "settings", Some("privacy")),
        TRAY_RESTART_UPDATE_ID => {
            if let Err(refused) = super::update::scheduler::restart_now(app) {
                log::info!("app menu: restart to update refused: {refused:?}");
                open_hub(app, "settings", Some("about"));
            }
        }
        COPY_LAST_ID => R::menu_copy_last(app),
        PASTE_LAST_ID => R::menu_paste_last(app),
        DOCUMENTATION_ID => {
            use tauri_plugin_opener::OpenerExt;
            if let Err(error) = app.opener().open_url(DOCUMENTATION_URL, None::<&str>) {
                log::warn!("app menu: could not open Documentation: {error}");
            }
        }
        SHORTCUTS_ID => open_hub(app, "settings", Some("hotkeys")),
        REPORT_PROBLEM_ID => super::commands::help::open_issue(
            app,
            super::diagnostics::issue_url::IssueKind::Problem,
        ),
        SEND_FEEDBACK_ID => super::commands::help::open_issue(
            app,
            super::diagnostics::issue_url::IssueKind::Feedback,
        ),
        // S-02: Handy shows the Hub for this id; when the shortcut is lost
        // the Fix sheet opens on top.
        TRAY_SECURE_INPUT_ID if crate::fork::shortcut::health::tray_warning() => {
            crate::fork::shortcut::health::open_fix_sheet(app, "tray")
        }
        ADD_WORD_ID => open_hub(app, "dictionary", Some("add")),
        NEW_SNIPPET_ID => open_hub(app, "snippets", Some("add")),
        HELP_MICROPHONE_ID => open_hub(app, "settings", Some("microphone")),
        HELP_LANGUAGES_ID | MORE_LANGUAGES_ID => open_hub(app, "settings", Some("languages")),
        _ => {
            if let Some(value) = id.strip_prefix(MICROPHONE_PREFIX) {
                choose_microphone(app, value);
            } else if let Some(code) = id.strip_prefix(LANGUAGE_PREFIX) {
                choose_language(app, code);
            }
        }
    }
}

/// Installs the menus and their live updates once Handy's managers are in
/// state. Listeners only schedule worker reads.
pub fn on_ready<R: ForkRuntime>(app: &AppHandle<R>) {
    if !R::native_menus() {
        return;
    }
    if app.try_state::<TrayMenu<R>>().is_none() {
        app.manage(TrayMenu::<R>(Mutex::new(None)));
    }
    let handle = app.clone();
    let _ = REFRESH_HOOK.set(Box::new(move |kind| refresh(&handle, kind)));
    app.on_menu_event(|app, event| on_menu(app, event.id.as_ref()));

    let handle = app.clone();
    super::events::ForkStatsUpdated::listen(app, move |_| refresh(&handle, Refresh::Data));
    let handle = app.clone();
    super::dictionary::ForkDictionaryChanged::listen(app, move |_| refresh(&handle, Refresh::Data));
    let handle = app.clone();
    super::snippets::ForkSnippetsChanged::listen(app, move |_| refresh(&handle, Refresh::Data));
    let handle = app.clone();
    super::audio::watch::ForkMicDevicesChanged::listen(app, move |_| {
        refresh(&handle, Refresh::Devices)
    });
    // Handy's history events (a take added, a row deleted) and settings.
    let handle = app.clone();
    app.listen_any("history-update-payload", move |_| {
        refresh(&handle, Refresh::Data)
    });
    let handle = app.clone();
    app.listen_any("settings-changed", move |_| {
        refresh(&handle, Refresh::Devices)
    });
    refresh(app, Refresh::Devices);
}

#[cfg(test)]
mod tests;
