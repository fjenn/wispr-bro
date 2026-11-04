// SPDX-License-Identifier: GPL-3.0-or-later
//! Hub window commands (S-38).
//!
//! - `fork_hub_ready`: the main page's event listeners are up. Events that
//!   waited for a rebuilt Hub are delivered now, and a Hub that started
//!   hidden may be parked (`fork::hub::park`).

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Manager};

use crate::fork::hub::park;
use crate::fork::ForkRuntime;

/// The main window's page is listening.
#[tauri::command]
#[specta::specta]
pub fn fork_hub_ready<R: ForkRuntime>(app: AppHandle<R>) {
    park::hub_ready(&app);
    apply_test_route(&app);
}

/// Mac wave 6: `WISPRBRO_HUB_ROUTE=<hash>` opens that route once, the first
/// time the Hub's page is ready (e.g. `#/fork/gallery`, which a person
/// reaches through Web Inspector, or `#/hub/insights`), so a scripted pass
/// over the real WebKit screens needs no console. Debug builds of test
/// identities (`com.wisprbro.<slice>check`) only; a product build never
/// reads it.
fn apply_test_route<R: ForkRuntime>(app: &AppHandle<R>) {
    static DONE: AtomicBool = AtomicBool::new(false);
    if !cfg!(debug_assertions) || !crate::fork::is_test_identity(&app.config().identifier) {
        return;
    }
    let Some(route) = std::env::var("WISPRBRO_HUB_ROUTE")
        .ok()
        .and_then(|r| test_route(&r))
    else {
        return;
    };
    if DONE.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Some(window) = app.get_webview_window(park::MAIN) {
        match window.eval(format!("window.location.hash = \"{route}\";")) {
            Ok(()) => log::info!("hub: test route {route}"),
            Err(e) => log::warn!("hub: test route failed: {e}"),
        }
    }
}

/// The route if it is a plain app hash (`#/` then page ids, sections and
/// percent escapes), else `None`; nothing that could leave the string.
fn test_route(raw: &str) -> Option<String> {
    let rest = raw.strip_prefix("#/")?;
    let ok = !rest.is_empty()
        && rest.len() <= 200
        && rest
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.' | '%'));
    ok.then(|| raw.to_owned())
}

#[cfg(test)]
mod tests {
    use super::test_route;

    #[test]
    fn test_routes_are_plain_hashes() {
        assert_eq!(
            test_route("#/fork/gallery").as_deref(),
            Some("#/fork/gallery")
        );
        assert_eq!(
            test_route("#/hub/style/work%20email").as_deref(),
            Some("#/hub/style/work%20email")
        );
        for bad in [
            "",
            "#/",
            "/hub/x",
            "#/hub/\";alert(1)//",
            "#/a b",
            "#/a'b",
            "#/a\\b",
        ] {
            assert_eq!(test_route(bad), None, "{bad}");
        }
    }
}
