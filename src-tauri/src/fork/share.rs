// SPDX-License-Identifier: GPL-3.0-or-later
//! Share with a friend (S-26b, R2 B-30/C-11 replaced): the download link,
//! the Homebrew line and the macOS share sheet.
//!
//! No tracking: the link is the same for everyone (no referral code, no
//! query string), nothing is counted or sent by the app, and the share sheet
//! hands plain text and the link to whichever service the user picks
//! (`NSSharingServicePicker`, objc2-app-kit, already in the tree).

use serde::{Deserialize, Serialize};
use specta::Type;

/// Latest release on GitHub (DMG); the same link for everyone.
pub const DOWNLOAD_URL: &str = "https://github.com/fjenn/wispr-bro/releases/latest";
/// Release notes ("What's New" in Help).
pub const RELEASES_URL: &str = "https://github.com/fjenn/wispr-bro/releases";
/// The Homebrew line. The Share page hides it until S-39 publishes the cask
/// in the `fjenn/tap` tap (`BREW_CASK_PUBLISHED` in `src/fork/hub/links.ts`).
pub const BREW_COMMAND: &str = "brew install --cask fjenn/tap/wispr-bro";

/// Where the Share button is, in the web view's CSS pixels (top-left
/// origin), so the sheet opens next to it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct ShareAnchor {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl ShareAnchor {
    /// A finite, non-negative rect of at least 1×1 (a hostile or broken
    /// value cannot reach AppKit).
    pub fn sanitized(self) -> Self {
        let clean = |v: f64, min: f64| {
            if v.is_finite() {
                v.clamp(min, 100_000.0)
            } else {
                min
            }
        };
        Self {
            x: clean(self.x, 0.0),
            y: clean(self.y, 0.0),
            width: clean(self.width, 1.0),
            height: clean(self.height, 1.0),
        }
    }
}

/// One share sheet request: the message and the link it carries.
#[derive(Clone, Debug, PartialEq)]
pub struct ShareRequest {
    pub message: String,
    pub url: String,
    pub anchor: ShareAnchor,
}

/// The request for `locale`: a one-line message plus the download link.
pub fn request(locale: &str, anchor: ShareAnchor) -> ShareRequest {
    let app_name = super::i18n::t(locale, "rust.appName", &[]);
    ShareRequest {
        message: super::i18n::t(locale, "rust.share.message", &[("appName", &app_name)]),
        url: DOWNLOAD_URL.to_string(),
        anchor: anchor.sanitized(),
    }
}

/// Shows the macOS share sheet anchored in the main web view (main thread,
/// through `with_webview`).
#[cfg(target_os = "macos")]
pub(crate) fn show(
    window: &tauri::WebviewWindow<tauri::Wry>,
    req: ShareRequest,
) -> Result<(), String> {
    window
        .with_webview(move |webview| {
            use objc2::rc::Retained;
            use objc2::runtime::AnyObject;
            use objc2::MainThreadMarker;
            use objc2_app_kit::{NSSharingServicePicker, NSView};
            use objc2_foundation::{NSArray, NSPoint, NSRect, NSRectEdge, NSSize, NSString, NSURL};

            let Some(mtm) = MainThreadMarker::new() else {
                log::warn!("share: not on the main thread");
                return;
            };
            let ptr = webview.inner();
            if ptr.is_null() {
                log::warn!("share: no web view");
                return;
            }
            // SAFETY: `inner()` is the live WKWebView (an NSView) of this
            // window, used on the main thread inside `with_webview`.
            let view: &NSView = unsafe { &*(ptr as *const NSView) };
            let text = NSString::from_str(&format!("{} {}", req.message, req.url));
            let url = NSURL::URLWithString(&NSString::from_str(&req.url));
            let mut items: Vec<&AnyObject> = vec![&text];
            if let Some(url) = url.as_deref() {
                items.push(url);
            }
            let items = NSArray::from_slice(&items);
            // SAFETY: NSString and NSURL conform to NSPasteboardWriting, as
            // `initWithItems:` requires.
            let picker: Retained<NSSharingServicePicker> =
                unsafe { NSSharingServicePicker::initWithItems(mtm.alloc(), &items) };
            let a = req.anchor;
            // WKWebView is flipped: CSS coordinates are its own.
            let rect = NSRect::new(NSPoint::new(a.x, a.y), NSSize::new(a.width, a.height));
            picker.showRelativeToRect_ofView_preferredEdge(rect, view, NSRectEdge::MaxY);
            // AppKit keeps the picker while its menu is up; hold the last
            // one too, so it outlives this closure in any case.
            PICKER.with(|slot| *slot.borrow_mut() = Some(picker));
            log::info!("share: share sheet shown");
        })
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
thread_local! {
    static PICKER: std::cell::RefCell<Option<objc2::rc::Retained<objc2_app_kit::NSSharingServicePicker>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) mod recorded {
    //! MockRuntime's share sheet: requests are recorded for the tests.
    use std::sync::Mutex;

    pub static REQUESTS: Mutex<Vec<super::ShareRequest>> = Mutex::new(Vec::new());

    pub fn take() -> Vec<super::ShareRequest> {
        std::mem::take(&mut *REQUESTS.lock().unwrap_or_else(|p| p.into_inner()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_the_same_for_everyone_and_on_github() {
        for url in [DOWNLOAD_URL, RELEASES_URL] {
            let parsed = reqwest::Url::parse(url).unwrap();
            assert_eq!(parsed.host_str(), Some("github.com"));
            assert_eq!(parsed.query(), None, "no tracking parameters: {url}");
            assert_eq!(parsed.fragment(), None);
        }
        assert!(BREW_COMMAND.starts_with("brew install --cask "));
        // The Hub shows and copies the same strings (`src/fork/hub/links.ts`).
        let links = include_str!("../../../src/fork/hub/links.ts");
        // No tap exists yet (S-39): the line stays hidden until then.
        assert!(links.contains("export const BREW_CASK_PUBLISHED = false;"));
        for value in [DOWNLOAD_URL, RELEASES_URL, BREW_COMMAND] {
            assert!(
                links.contains(&format!("\"{value}\"")),
                "links.ts lacks {value}"
            );
        }
    }

    #[test]
    fn anchors_are_sanitized() {
        let a = ShareAnchor {
            x: f64::NAN,
            y: -5.0,
            width: f64::INFINITY,
            height: 0.0,
        }
        .sanitized();
        assert_eq!(
            a,
            ShareAnchor {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0
            }
        );
        let b = ShareAnchor {
            x: 12.5,
            y: 300.0,
            width: 96.0,
            height: 32.0,
        };
        assert_eq!(b.sanitized(), b);
    }

    #[test]
    fn the_message_is_localized_and_carries_the_link_separately() {
        let anchor = ShareAnchor {
            x: 1.0,
            y: 2.0,
            width: 3.0,
            height: 4.0,
        };
        let req = request("en", anchor);
        assert_eq!(req.url, DOWNLOAD_URL);
        assert!(req.message.contains("Wispr Bro"), "{}", req.message);
        assert!(!req.message.contains("{{"), "{}", req.message);
        assert!(!req.message.contains("http"), "{}", req.message);
        assert_eq!(req.anchor, anchor);
    }
}
