// SPDX-License-Identifier: GPL-3.0-or-later
//! Help and Feedback and Share with a friend (S-26b).
//!
//! Report a problem and Send feedback open a prefilled GitHub issue in the
//! browser ([`crate::fork::diagnostics::issue_url`]); the Hub, the Help menu
//! (⌘/) and the tray share [`open_issue`]. Share with a friend shows the
//! macOS share sheet through the runtime.

use tauri::AppHandle;

use crate::fork::diagnostics::issue_url::{self, IssueKind};
use crate::fork::share::{self, ShareAnchor};
use crate::fork::{i18n, ForkRuntime};

use super::ForkError;

/// The prefilled issue URL for this app (versions and ids only).
pub fn issue_link<R: ForkRuntime>(app: &AppHandle<R>, kind: IssueKind) -> String {
    issue_url::issue_url(kind, &issue_url::gather(app))
}

/// Opens the prefilled issue in the browser, off the calling thread (the
/// machine probe and the browser hand-off never block a menu click).
pub fn open_issue<R: ForkRuntime>(app: &AppHandle<R>, kind: IssueKind) {
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("wb-help-issue".into())
        .spawn(move || {
            let url = issue_link(&app, kind);
            use tauri_plugin_opener::OpenerExt;
            match app.opener().open_url(url, None::<&str>) {
                Ok(()) => log::info!("help: opened the {kind:?} form on GitHub"),
                Err(error) => log::warn!("help: could not open the {kind:?} form: {error}"),
            }
        });
    if let Err(error) = spawned {
        log::warn!("help: could not start the issue opener: {error}");
    }
}

/// Help › Report a problem / Send feedback: opens the prefilled GitHub issue
/// in the browser. Nothing is sent by the app.
#[tauri::command]
#[specta::specta]
pub fn fork_open_issue<R: ForkRuntime>(
    app: AppHandle<R>,
    kind: IssueKind,
) -> Result<(), ForkError> {
    open_issue(&app, kind);
    Ok(())
}

/// Shows the macOS share sheet next to `anchor` (the Share button).
#[tauri::command]
#[specta::specta]
pub fn fork_share_sheet<R: ForkRuntime>(
    app: AppHandle<R>,
    anchor: ShareAnchor,
) -> Result<(), ForkError> {
    let language = R::handy_app_language(&app);
    let locale = i18n::current_locale(language.as_deref());
    R::show_share_sheet(&app, share::request(locale, anchor)).map_err(ForkError::Internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_app() -> tauri::App<tauri::test::MockRuntime> {
        tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app")
    }

    #[test]
    fn the_share_sheet_command_reaches_the_runtime_with_the_link() {
        let app = mock_app();
        share::recorded::take();
        let anchor = ShareAnchor {
            x: 40.0,
            y: 220.0,
            width: 120.0,
            height: 36.0,
        };
        fork_share_sheet(app.handle().clone(), anchor).unwrap();
        let requests = share::recorded::take();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].url, share::DOWNLOAD_URL);
        assert_eq!(requests[0].anchor, anchor);
        assert!(requests[0].message.contains("Wispr Bro"));
    }

    #[test]
    fn the_issue_link_carries_no_dictation_from_the_app() {
        let app = mock_app();
        let link = issue_link(app.handle(), IssueKind::Problem);
        let parsed = reqwest::Url::parse(&link).unwrap();
        assert_eq!(parsed.host_str(), Some("github.com"));
        assert_eq!(parsed.path(), "/fjenn/wispr-bro/issues/new");
        let body: String = parsed
            .query_pairs()
            .find(|(k, _)| k == "body")
            .map(|(_, v)| v.into_owned())
            .unwrap();
        assert!(body.contains(&format!(
            "- Wispr Bro: {}",
            app.handle().package_info().version
        )));
    }
}
