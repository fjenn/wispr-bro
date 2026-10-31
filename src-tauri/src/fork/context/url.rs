// SPDX-License-Identifier: GPL-3.0-only
// Copyright (c) 2024 @bennobuilder (mado; MIT, notice below)
// Copyright (c) the VoiceInk authors and BetterWispr contributors (GPL-3.0)
// Modified by Wispr Bro contributors, 2026-10-31.
//
// Ported from mado 0.0.21's crates.io tarball, `src-swift/
// BrowserURLExtractor.swift`, `BrowserWebContent.swift` and
// `SupportedBrowsers.swift`, MIT. Kept: the
// supported-browser table, document URL before the address bar, the bounded
// breadth-first search, address-bar identification, and the rule that an
// address bar is read only when it is demonstrably not focused. Changed:
// Swift to Rust, only the five S-19b browsers are enabled, and every result
// is reduced to a host immediately.
//
// The focused-page and first-web-area fallbacks are ported from BetterWispr
// `Sources/BetterWisprCore/Integrations/WebsiteReader.swift`, commit
// 304335a073a98f7439e5f7b4fbc2c31b810b807e (VoiceInk-derived, GPL-3.0).
// Changed: Swift to Rust; the existing AX thread and helpers are reused.
//
// Tambourine's `active_app_context` origin-only reduction (AGPL-3.0,
// kstonekuan/tambourine-voice c54b527d) supplied the reduction idea only;
// this implementation is stricter and keeps no scheme, port or origin.
//
// MIT License
//
// Copyright (c) 2024 @bennobuilder
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! Local browser-page detection for Styles (S-19b). Full URLs and window
//! titles exist only inside one AX read. The only value returned or stored is
//! a lower-case host, with no userinfo, port, path, query or fragment.

use std::time::Duration;

use super::categories;

const READ_BUDGET: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Browser {
    Chromium,
    Safari,
}

/// mado's `SupportedBrowsers.kind`, limited to S-19b's five browsers.
fn browser(bundle_id: &str) -> Option<Browser> {
    match bundle_id.to_ascii_lowercase().as_str() {
        "com.google.chrome"
        | "com.google.chrome.beta"
        | "com.google.chrome.dev"
        | "com.google.chrome.canary"
        | "company.thebrowser.browser"
        | "com.brave.browser"
        | "com.brave.browser.beta"
        | "com.brave.browser.nightly"
        | "com.microsoft.edgemac"
        | "com.microsoft.edgemac.beta"
        | "com.microsoft.edgemac.dev"
        | "com.microsoft.edgemac.canary" => Some(Browser::Chromium),
        "com.apple.safari" | "com.apple.safaritechnologypreview" => Some(Browser::Safari),
        _ => None,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct AddressBar {
    value: Option<String>,
    focused: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PageRead {
    document_url: Option<String>,
    focused_page_url: Option<String>,
    first_page_url: Option<String>,
    address_bar: Option<AddressBar>,
    window_title: Option<String>,
}

/// mado's address-bar normalisation. Non-web schemes may be recognised here,
/// but [`host`] will not return a host for them.
fn normalize_url(value: &str) -> Option<String> {
    let value = value.trim();
    if value.len() < 3 {
        return None;
    }
    const PREFIXES: &[&str] = &[
        "http://",
        "https://",
        "file://",
        "about:",
        "chrome://",
        "edge://",
        "brave://",
        "arc://",
    ];
    let lower = value.to_ascii_lowercase();
    if PREFIXES.iter().any(|p| lower.starts_with(p)) {
        return Some(value.to_string());
    }
    if value.contains('.') && !value.contains(char::is_whitespace) {
        return Some(format!("https://{value}"));
    }
    None
}

/// Reduces a URL or scheme-less address to its host. Nothing else survives.
pub fn host(value: &str) -> Option<String> {
    let value = normalize_url(value)?;
    let (scheme, rest) = value.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next()?;
    let authority = authority.rsplit('@').next()?;
    let host = if let Some(bracketed) = authority.strip_prefix('[') {
        let end = bracketed.find(']')?;
        &bracketed[..end]
    } else {
        authority.split(':').next()?
    }
    .trim_end_matches('.')
    .to_ascii_lowercase();
    let valid = !host.is_empty()
        && host.len() <= 253
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':'));
    valid.then_some(host)
}

/// The extractor's pure decision path, shared by every supported browser.
fn extract_host(bundle_id: &str, read: &PageRead) -> Option<String> {
    browser(bundle_id)?;
    for candidate in [
        read.document_url.as_deref(),
        read.focused_page_url.as_deref(),
        read.first_page_url.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(host) = host(candidate) {
            return Some(host);
        }
    }
    // An unreadable focus state cannot establish that the address is not an
    // uncommitted edit (mado's safeguard).
    if let Some(bar) = read
        .address_bar
        .as_ref()
        .filter(|b| b.focused == Some(false))
    {
        if let Some(host) = bar.value.as_deref().and_then(host) {
            return Some(host);
        }
    }
    read.window_title
        .as_deref()
        .and_then(categories::host_from_title)
}

/// Reads the frontmost supported browser and returns only its page's host.
pub fn current_host(bundle_id: Option<&str>) -> Option<String> {
    let bundle_id = bundle_id?;
    browser(bundle_id)?;
    #[cfg(target_os = "macos")]
    {
        if !crate::fork::ax::accessibility_trusted() {
            return None;
        }
        let bundle_id = bundle_id.to_string();
        crate::fork::ax::thread::run(READ_BUDGET, move |ctx| macos::read(ctx, &bundle_id)).flatten()
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = READ_BUDGET;
        None
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::collections::VecDeque;

    use objc2_app_kit::NSWorkspace;
    use objc2_application_services::AXUIElement;
    use objc2_core_foundation::CFRetained;

    use super::{AddressBar, PageRead};
    use crate::fork::ax::element as el;
    use crate::fork::ax::thread::Ctx;

    fn focused_page_url(app: &AXUIElement) -> Option<String> {
        let mut current = el::attr_element(app, "AXFocusedUIElement");
        let mut url = None;
        let mut hops = 0;
        while let Some(element) = current {
            if hops == 64 {
                break;
            }
            hops += 1;
            if el::attr_string(&element, "AXRole").as_deref() == Some("AXWebArea") {
                if let Some(candidate) = el::attr_url(&element, "AXURL") {
                    url = Some(candidate);
                }
            }
            current = el::attr_element(&element, "AXParent");
        }
        url
    }

    fn first_page_url(window: &CFRetained<AXUIElement>) -> Option<String> {
        let mut pending = VecDeque::from([(window.clone(), 0usize)]);
        let mut visited = 0;
        while let Some((element, depth)) = pending.pop_front() {
            if visited == 500 {
                break;
            }
            visited += 1;
            let role = el::attr_string(&element, "AXRole");
            if matches!(role.as_deref(), Some("AXWebArea" | "AXDocument")) {
                if let Some(url) = el::attr_url(&element, "AXURL") {
                    return Some(url);
                }
                continue;
            }
            if depth < 12 {
                pending.extend(
                    el::children(&element)
                        .into_iter()
                        .map(|child| (child, depth + 1)),
                );
            }
        }
        None
    }

    fn is_address_bar(element: &AXUIElement, role: Option<&str>) -> bool {
        if !matches!(role, Some("AXTextField" | "AXComboBox")) {
            return false;
        }
        let identifier = el::attr_string(element, "AXDOMIdentifier")
            .or_else(|| el::attr_string(element, "AXIdentifier"));
        if identifier.as_deref().is_some_and(|id| {
            [
                "urlbar-input",
                "urlbar",
                "omnibox",
                "urlFieldInput",
                "WEB_BROWSER_ADDRESS_AND_SEARCH_FIELD",
            ]
            .contains(&id)
        }) {
            return true;
        }
        let description = el::attr_string(element, "AXDescription")
            .unwrap_or_default()
            .to_ascii_lowercase();
        let placeholder = el::attr_string(element, "AXPlaceholderValue")
            .unwrap_or_default()
            .to_ascii_lowercase();
        [description, placeholder]
            .iter()
            .any(|text| text.contains("address") || text.contains("url"))
    }

    fn address_bar(window: &CFRetained<AXUIElement>) -> Option<AddressBar> {
        let mut queue = VecDeque::from([(window.clone(), 1usize)]);
        let mut fallback = None;
        let mut visited = 0;
        while let Some((element, depth)) = queue.pop_front() {
            if visited == 500 {
                break;
            }
            visited += 1;
            if depth > 30 {
                continue;
            }
            let role = el::attr_string(&element, "AXRole");
            if matches!(role.as_deref(), Some("AXWebArea" | "AXDocument")) {
                continue;
            }
            if is_address_bar(&element, role.as_deref()) {
                let bar = AddressBar {
                    value: el::attr_string(&element, "AXValue"),
                    focused: el::attr_bool(&element, "AXFocused"),
                };
                if bar
                    .value
                    .as_deref()
                    .and_then(super::normalize_url)
                    .is_some()
                {
                    return Some(bar);
                }
                if fallback.is_none() {
                    fallback = Some(bar);
                }
            }
            queue.extend(
                el::children(&element)
                    .into_iter()
                    .map(|child| (child, depth + 1)),
            );
        }
        fallback
    }

    pub fn read(ctx: &mut Ctx, expected_bundle: &str) -> Option<String> {
        let running = NSWorkspace::sharedWorkspace().frontmostApplication()?;
        if running
            .bundleIdentifier()
            .is_none_or(|id| !id.to_string().eq_ignore_ascii_case(expected_bundle))
        {
            return None;
        }
        let pid = running.processIdentifier();
        if pid <= 0 {
            return None;
        }
        let app = ctx.app(pid);
        let window: CFRetained<AXUIElement> = el::attr_element(&app, "AXFocusedWindow")?;
        let document_url = el::attr_url(&window, "AXDocument");
        let read = PageRead {
            document_url,
            ..PageRead::default()
        };
        if let Some(host) = super::extract_host(expected_bundle, &read) {
            return Some(host);
        }
        let focused_page_url = focused_page_url(&app);
        let read = PageRead {
            focused_page_url,
            ..PageRead::default()
        };
        if let Some(host) = super::extract_host(expected_bundle, &read) {
            return Some(host);
        }
        let first_page_url = first_page_url(&window);
        let read = PageRead {
            first_page_url,
            ..PageRead::default()
        };
        if let Some(host) = super::extract_host(expected_bundle, &read) {
            return Some(host);
        }
        let address_bar = address_bar(&window);
        let read = PageRead {
            address_bar,
            ..PageRead::default()
        };
        if let Some(host) = super::extract_host(expected_bundle, &read) {
            return Some(host);
        }
        let window_title = el::attr_string(&window, "AXTitle");
        let read = PageRead {
            window_title,
            ..PageRead::default()
        };
        super::extract_host(expected_bundle, &read)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(bundle_id: &str, url: &str) -> Option<String> {
        extract_host(
            bundle_id,
            &PageRead {
                document_url: Some(url.into()),
                ..PageRead::default()
            },
        )
    }

    #[test]
    fn chrome_arc_brave_edge_and_safari_extract_hosts() {
        for bundle_id in [
            "com.google.Chrome",
            "company.thebrowser.Browser",
            "com.brave.Browser",
            "com.microsoft.edgemac",
            "com.apple.Safari",
        ] {
            assert_eq!(
                document(
                    bundle_id,
                    "https://user:secret@Example.com:443/a/private/path?q=1#x"
                )
                .as_deref(),
                Some("example.com"),
                "{bundle_id}"
            );
        }
        assert_eq!(document("com.example.editor", "https://example.com"), None);
    }

    #[test]
    fn document_then_web_area_then_unfocused_address_bar() {
        let read = PageRead {
            document_url: Some("https://one.example.com/private".into()),
            focused_page_url: Some("https://two.example.com/private".into()),
            first_page_url: Some("https://three.example.com/private".into()),
            address_bar: Some(AddressBar {
                value: Some("four.example.com/private".into()),
                focused: Some(false),
            }),
            ..PageRead::default()
        };
        assert_eq!(
            extract_host("com.google.Chrome", &read).as_deref(),
            Some("one.example.com")
        );
        let read = PageRead {
            focused_page_url: read.focused_page_url,
            first_page_url: read.first_page_url,
            address_bar: read.address_bar,
            ..PageRead::default()
        };
        assert_eq!(
            extract_host("com.apple.Safari", &read).as_deref(),
            Some("two.example.com")
        );
        let read = PageRead {
            first_page_url: read.first_page_url,
            address_bar: read.address_bar,
            ..PageRead::default()
        };
        assert_eq!(
            extract_host("com.brave.Browser", &read).as_deref(),
            Some("three.example.com")
        );
        let read = PageRead {
            address_bar: read.address_bar,
            ..PageRead::default()
        };
        assert_eq!(
            extract_host("com.microsoft.edgemac", &read).as_deref(),
            Some("four.example.com")
        );
    }

    #[test]
    fn focused_or_unknown_address_bar_is_never_read() {
        for focused in [Some(true), None] {
            let read = PageRead {
                address_bar: Some(AddressBar {
                    value: Some("private.example.com/uncommitted".into()),
                    focused,
                }),
                ..PageRead::default()
            };
            assert_eq!(extract_host("company.thebrowser.Browser", &read), None);
        }
    }

    #[test]
    fn title_fallback_returns_only_a_known_host() {
        for (title, expected) in [
            ("Inbox - Gmail", Some("mail.google.com")),
            ("Team conversation | Slack", Some("app.slack.com")),
            ("A private document title", None),
            ("https://unknown.example.com/private", None),
        ] {
            let read = PageRead {
                window_title: Some(title.into()),
                ..PageRead::default()
            };
            assert_eq!(
                extract_host("com.google.Chrome", &read).as_deref(),
                expected
            );
        }
    }

    #[test]
    fn host_reduction_drops_everything_else() {
        assert_eq!(
            host("HTTPS://User:Pass@WWW.Example.COM:8443/a/b?secret=yes#fragment").as_deref(),
            Some("www.example.com")
        );
        assert_eq!(host("example.com/path").as_deref(), Some("example.com"));
        assert_eq!(host("file:///Users/me/private.html"), None);
        assert_eq!(host("not an address"), None);
        assert_eq!(
            host("https://example.com.evil.example/").as_deref(),
            Some("example.com.evil.example")
        );
    }
}
