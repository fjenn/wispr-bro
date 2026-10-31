// SPDX-License-Identifier: GPL-3.0-or-later
//! Fork additions to Handy's tray (F-01; S-26a adds and removes items).
//!
//! Hooks in `tray.rs` (R5 §4.2):
//! - `build_menu`: `crate::fork::tray::extend(app, &menu, &mut tooltip, &inputs.locale)?;`
//!   right before it returns. It replaces Handy's title/version item
//!   (`"Handy v…"`, id `version`) and the tooltip with localized fork strings.
//!   `build_menu` runs inside Handy's single tray applier, so the tooltip is
//!   still set only through the applier (see `tray.rs` module docs).
//! - `get_icon_path`: first line `if let Some(p) = crate::fork::tray::icon_path(…)`
//!   returns the fork's tray glyphs (`resources/fork/tray/`).
//!
//! The initial tooltip passed to `TrayIconBuilder` in `lib.rs` is still
//! Handy's; Handy's first `update_tray_menu` during setup replaces it before
//! the event loop shows anything.

use std::sync::Mutex;

use tauri::menu::Menu;
use tauri::{AppHandle, Runtime};

use super::i18n;
use crate::tray::{AppTheme, TrayIconState};

/// Tooltip from the last [`extend`] call (read by the startup self-check).
static LAST_TOOLTIP: Mutex<Option<String>> = Mutex::new(None);

/// The tooltip the tray hook last produced, if the tray menu was built.
pub fn last_tooltip() -> Option<String> {
    LAST_TOOLTIP.lock().ok().and_then(|g| g.clone())
}

/// Version label for the tray title item and tooltip, e.g.
/// `"Wispr Bro v1.0.0-alpha.1"` (`"… (Dev)"` in debug builds).
pub fn version_label(locale: &str) -> String {
    let key = if cfg!(debug_assertions) {
        "rust.tray.versionDev"
    } else {
        "rust.tray.version"
    };
    i18n::t(
        i18n::current_locale(Some(locale)),
        key,
        &[("version", env!("CARGO_PKG_VERSION"))],
    )
}

/// The label Handy's `tray::version_label` produces. Mirrors it so the tooltip
/// rewrite can keep whatever Handy appends after it (the Secure Input
/// warning).
fn handy_version_label() -> String {
    if cfg!(debug_assertions) {
        format!("Handy v{} (Dev)", env!("CARGO_PKG_VERSION"))
    } else {
        format!("Handy v{}", env!("CARGO_PKG_VERSION"))
    }
}

/// Replaces Handy's version label at the start of `tooltip` with `fork_label`
/// and keeps the rest (`" — <warning>"`). If Handy changed its label format,
/// the whole tooltip becomes `fork_label` rather than leaking the old brand.
pub(crate) fn rewrite_tooltip(tooltip: &str, fork_label: &str) -> String {
    match tooltip.strip_prefix(&handy_version_label()) {
        Some(rest) => format!("{fork_label}{rest}"),
        None if tooltip.contains("Handy") => {
            log::warn!("fork tray: unexpected Handy tooltip format {tooltip:?}");
            fork_label.to_string()
        }
        None => tooltip.to_string(),
    }
}

/// The `tray.rs` `build_menu` hook. Relabels the `version` item and rewrites
/// the tooltip.
pub fn extend<R: Runtime>(
    _app: &AppHandle<R>,
    menu: &Menu<R>,
    tooltip: &mut String,
    locale: &str,
) -> tauri::Result<()> {
    let label = version_label(locale);
    match menu
        .get("version")
        .and_then(|item| item.as_menuitem().cloned())
    {
        Some(item) => item.set_text(&label)?,
        None => log::warn!("fork tray: Handy's `version` menu item not found"),
    }
    *tooltip = rewrite_tooltip(tooltip, &label);
    if crate::fork::is_product_build() {
        remove_model_items(menu)?;
    }
    if let Ok(mut last) = LAST_TOOLTIP.lock() {
        *last = Some(tooltip.clone());
    }
    Ok(())
}

/// Handy's tray items that name or manage the speech model (S-07b, B-207:
/// the user never picks a model; models live in Settings › Advanced).
pub const MODEL_ITEM_IDS: [&str; 2] = ["model_submenu", "unload_model"];

/// Which positions to remove from a menu whose item ids are `ids` (`None`
/// for a separator): the model items, and the separator after them when it
/// would leave two separators in a row.
pub fn model_item_positions(ids: &[Option<&str>]) -> Vec<usize> {
    let mut out: Vec<usize> = ids
        .iter()
        .enumerate()
        .filter(|(_, id)| id.is_some_and(|id| MODEL_ITEM_IDS.contains(&id)))
        .map(|(i, _)| i)
        .collect();
    if let Some(&last) = out.last() {
        let before_is_sep = out
            .first()
            .and_then(|&f| f.checked_sub(1))
            .is_some_and(|b| ids[b].is_none());
        if before_is_sep && ids.get(last + 1).is_some_and(|n| n.is_none()) {
            out.push(last + 1);
        }
    }
    out
}

/// Removes Handy's Model ▸ submenu and Unload item (and a doubled
/// separator). Absent items (the busy layout) are a no-op.
fn remove_model_items<R: Runtime>(menu: &Menu<R>) -> tauri::Result<()> {
    use tauri::menu::MenuItemKind;
    let items = menu.items()?;
    let ids: Vec<Option<String>> = items
        .iter()
        .map(|i| match i {
            MenuItemKind::Predefined(_) => None,
            other => Some(other.id().0.clone()),
        })
        .collect();
    let refs: Vec<Option<&str>> = ids.iter().map(|i| i.as_deref()).collect();
    let positions = model_item_positions(&refs);
    for pos in positions.iter().rev() {
        menu.remove_at(*pos)?;
    }
    if !positions.is_empty() {
        log::debug!("fork tray: model items removed ({})", positions.len());
    }
    Ok(())
}

/// The `tray.rs` `get_icon_path` hook: the fork's tray glyph for a theme and
/// state, or `None` to keep Handy's (Linux "Colored" theme, until Linux
/// ships). Paths are resources bundled by Handy's `resources/**/*` glob.
pub fn icon_path(theme: &AppTheme, state: TrayIconState, warning: bool) -> Option<&'static str> {
    let warning = warning && state == TrayIconState::Idle;
    Some(match (theme, state, warning) {
        (AppTheme::Colored, _, _) => return None,
        (AppTheme::Dark, _, true) => "resources/fork/tray/tray_idle_warning.png",
        (AppTheme::Light, _, true) => "resources/fork/tray/tray_idle_warning_dark.png",
        // Dark menu bars use the light (white) glyphs, as in Handy.
        (AppTheme::Dark, TrayIconState::Idle, _) => "resources/fork/tray/tray_idle.png",
        (AppTheme::Dark, TrayIconState::Recording, _) => "resources/fork/tray/tray_recording.png",
        (AppTheme::Dark, TrayIconState::Transcribing, _) => {
            "resources/fork/tray/tray_transcribing.png"
        }
        (AppTheme::Light, TrayIconState::Idle, _) => "resources/fork/tray/tray_idle_dark.png",
        (AppTheme::Light, TrayIconState::Recording, _) => {
            "resources/fork/tray/tray_recording_dark.png"
        }
        (AppTheme::Light, TrayIconState::Transcribing, _) => {
            "resources/fork/tray/tray_transcribing_dark.png"
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // `extend` itself needs a native menu; muda builds menus only on the
    // macOS main thread, which `cargo test` does not own. Its logic is the
    // two pure functions tested here; the wiring is checked by
    // `fork::identity_tests::file_prefix_hooks_are_in_place` and by running
    // the app (tray label in the log of `bun run wb:dev`).
    #[test]
    fn the_model_items_and_their_separator_go() {
        // Handy's idle layout (tray.rs build_menu).
        let idle = [
            Some("version"),
            None,
            Some("copy_last_transcript"),
            None,
            Some("model_submenu"),
            Some("unload_model"),
            None,
            Some("settings"),
            Some("check_updates"),
            None,
            Some("quit"),
        ];
        assert_eq!(model_item_positions(&idle), vec![4, 5, 6]);
        // The busy layout has no model items.
        let busy = [Some("version"), None, Some("cancel"), None, Some("quit")];
        assert!(model_item_positions(&busy).is_empty());
        // Handy's tray.rs still builds these ids.
        let tray = include_str!("../tray.rs");
        for id in MODEL_ITEM_IDS {
            assert!(tray.contains(&format!("\"{id}\"")), "{id}");
        }
    }

    #[test]
    fn version_label_is_ours_in_every_locale() {
        for code in i18n::locales() {
            let label = version_label(code);
            assert!(!label.contains("Handy"), "{code}: {label}");
            assert!(label.contains(env!("CARGO_PKG_VERSION")), "{code}: {label}");
            assert!(label.contains("Wispr Bro"), "{code}: {label}");
        }
    }

    #[test]
    fn tooltip_rewrite_keeps_the_warning_suffix() {
        let ours = version_label("en");
        assert_eq!(rewrite_tooltip(&handy_version_label(), &ours), ours);
        let warned = format!("{} — Shortcuts blocked", handy_version_label());
        assert_eq!(
            rewrite_tooltip(&warned, &ours),
            format!("{ours} — Shortcuts blocked")
        );
        assert_eq!(rewrite_tooltip("Handy 2.0 beta", &ours), ours);
        assert_eq!(rewrite_tooltip("something else", &ours), "something else");
    }

    #[test]
    fn tray_icons_exist_for_every_macos_state() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        for theme in [AppTheme::Dark, AppTheme::Light] {
            for state in [
                TrayIconState::Idle,
                TrayIconState::Recording,
                TrayIconState::Transcribing,
            ] {
                for warning in [false, true] {
                    let path = icon_path(&theme, state, warning).unwrap();
                    assert!(path.starts_with("resources/fork/tray/"), "{path}");
                    assert!(root.join(path).is_file(), "missing {path}");
                }
            }
        }
        assert_eq!(
            icon_path(&AppTheme::Colored, TrayIconState::Idle, false),
            None
        );
        // Busy states never show the warning badge (Handy's rule).
        assert_eq!(
            icon_path(&AppTheme::Dark, TrayIconState::Recording, true),
            icon_path(&AppTheme::Dark, TrayIconState::Recording, false)
        );
    }
}
