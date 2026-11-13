// SPDX-License-Identifier: GPL-3.0-or-later
//! Update channel → updater endpoints (F-02, DEC-33).
//!
//! The stable endpoint is the one in the overlay
//! (`plugins.updater.endpoints`, `releases/latest/download/latest.json`):
//! GitHub's `releases/latest` never points at a prerelease or a draft, so a
//! stable install never sees a beta. The beta channel reads the rolling `beta`
//! prerelease's `latest.json` **and** the stable one and takes the newer
//! release, so a beta tester is never left behind when a stable release
//! overtakes the last beta (semver: `1.0.0-beta.3 < 1.0.0`).
//!
//! Debug builds accept a loopback stand-in for GitHub
//! (`WISPRBRO_UPDATE_ENDPOINT=http://127.0.0.1:PORT/fjenn/wispr-bro`), used
//! by `scripts/wb-f02/` for the 0.x → 0.x+1 update test. Release builds
//! ignore it. The signature is checked against the overlay pubkey either way.

use reqwest::Url;

use crate::fork::settings::UpdateChannel;

/// GitHub owner and repository of our releases (R5 §1 row 1).
pub const OWNER: &str = "fjenn";
pub const REPO: &str = "wispr-bro";
/// Base of every update URL.
pub const GITHUB_BASE: &str = "https://github.com/fjenn/wispr-bro";
/// `latest.json` of the newest published (non-pre-)release.
#[cfg(not(feature = "publik"))]
pub const STABLE_PATH: &str = "/releases/latest/download/latest.json";
/// `latest.json` of the rolling `beta` prerelease.
#[cfg(not(feature = "publik"))]
pub const BETA_PATH: &str = "/releases/download/beta/latest.json";
/// The publik edition reads its own feeds on the same releases
/// (docs/fork/EDITIONS.md), so it never updates to the normal build.
#[cfg(feature = "publik")]
pub use crate::fork::publik::{BETA_PATH, STABLE_PATH};
/// minisign key id of our updater public key (`plugins.updater.pubkey`). The
/// key must never change after the first public build (CONTEXT.md "Do not").
pub const PUBKEY_KEY_ID: &str = "89E085217ACC46CF";
/// Debug builds only: loopback stand-in for [`GITHUB_BASE`].
pub const ENDPOINT_OVERRIDE_ENV: &str = "WISPRBRO_UPDATE_ENDPOINT";

/// The stable endpoint (equal to the overlay's only endpoint).
pub fn stable_endpoint() -> String {
    format!("{GITHUB_BASE}{STABLE_PATH}")
}

/// The beta endpoint.
pub fn beta_endpoint() -> String {
    format!("{GITHUB_BASE}{BETA_PATH}")
}

/// The endpoints a channel reads, in order, on `base` (a GitHub repo URL or
/// its loopback stand-in). Beta reads both; the caller takes the newer.
pub fn endpoints_on(base: &str, channel: UpdateChannel) -> Vec<String> {
    let base = base.trim_end_matches('/');
    match channel {
        UpdateChannel::Stable => vec![format!("{base}{STABLE_PATH}")],
        UpdateChannel::Beta => vec![format!("{base}{BETA_PATH}"), format!("{base}{STABLE_PATH}")],
    }
}

/// The base URL to use: [`GITHUB_BASE`], or in debug builds a loopback
/// `override_base` (anything else is ignored with a warning).
pub fn base(override_base: Option<&str>) -> String {
    if cfg!(debug_assertions) {
        if let Some(candidate) = override_base.map(str::trim).filter(|s| !s.is_empty()) {
            if crate::fork::net::is_loopback(candidate) {
                return candidate.trim_end_matches('/').to_string();
            }
            log::warn!("update: {ENDPOINT_OVERRIDE_ENV} ignored (not a loopback URL)");
        }
    }
    GITHUB_BASE.to_string()
}

/// The parsed endpoints for `channel`, honouring the debug-only override.
pub fn endpoints(channel: UpdateChannel) -> Result<Vec<Url>, String> {
    let override_base = std::env::var(ENDPOINT_OVERRIDE_ENV).ok();
    endpoints_on(&base(override_base.as_deref()), channel)
        .iter()
        .map(|u| Url::parse(u).map_err(|e| format!("bad update endpoint {u}: {e}")))
        .collect()
}

/// `true` when `url` may serve an update: our GitHub repo over https, or (in
/// debug builds) loopback.
pub fn is_update_url(url: &str) -> bool {
    let ours = url.starts_with(&format!("{GITHUB_BASE}/releases/"))
        && crate::fork::net::is_allowed_host(url);
    ours || (cfg!(debug_assertions) && crate::fork::net::is_loopback(url))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use std::path::Path;

    fn overlay() -> serde_json::Value {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.fork.conf.json");
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    #[cfg(not(feature = "publik"))]
    fn stable_reads_only_releases_latest() {
        assert_eq!(
            endpoints_on(GITHUB_BASE, UpdateChannel::Stable),
            vec!["https://github.com/fjenn/wispr-bro/releases/latest/download/latest.json"]
        );
    }

    #[test]
    #[cfg(not(feature = "publik"))]
    fn beta_reads_the_beta_prerelease_then_stable() {
        assert_eq!(
            endpoints_on(GITHUB_BASE, UpdateChannel::Beta),
            vec![
                "https://github.com/fjenn/wispr-bro/releases/download/beta/latest.json",
                "https://github.com/fjenn/wispr-bro/releases/latest/download/latest.json",
            ]
        );
    }

    #[test]
    #[cfg(not(feature = "publik"))]
    fn the_overlay_endpoint_is_the_stable_channel() {
        let endpoints = overlay()["plugins"]["updater"]["endpoints"].clone();
        assert_eq!(endpoints, serde_json::json!([stable_endpoint()]));
    }

    #[test]
    fn the_overlay_pubkey_is_ours() {
        let key = overlay()["plugins"]["updater"]["pubkey"]
            .as_str()
            .unwrap()
            .to_string();
        let text = String::from_utf8(
            base64::engine::general_purpose::STANDARD
                .decode(key)
                .expect("pubkey is base64"),
        )
        .unwrap();
        assert_eq!(
            text.lines().next(),
            Some(format!("untrusted comment: minisign public key: {PUBKEY_KEY_ID}").as_str())
        );
        // The key line itself: "RW" + base64 of Ed + key id (LE) + 32-byte key.
        let raw = base64::engine::general_purpose::STANDARD
            .decode(text.lines().nth(1).unwrap())
            .unwrap();
        assert_eq!(raw.len(), 42);
        assert_eq!(&raw[..2], b"Ed");
        let id: String = raw[2..10]
            .iter()
            .rev()
            .map(|b| format!("{b:02X}"))
            .collect();
        assert_eq!(id, PUBKEY_KEY_ID);
    }

    #[test]
    fn only_loopback_overrides_the_base_in_debug_builds() {
        assert_eq!(base(None), GITHUB_BASE);
        assert_eq!(base(Some("")), GITHUB_BASE);
        assert_eq!(
            base(Some("https://evil.example/fjenn/wispr-bro")),
            GITHUB_BASE
        );
        let local = base(Some("http://127.0.0.1:18090/fjenn/wispr-bro/"));
        if cfg!(debug_assertions) {
            assert_eq!(local, "http://127.0.0.1:18090/fjenn/wispr-bro");
        } else {
            assert_eq!(local, GITHUB_BASE);
        }
    }

    #[test]
    fn update_urls_are_our_releases_or_loopback() {
        assert!(is_update_url(&stable_endpoint()));
        assert!(is_update_url(&beta_endpoint()));
        assert!(!is_update_url(
            "https://github.com/cjpais/Handy/releases/latest/download/latest.json"
        ));
        assert!(!is_update_url(
            "http://github.com/fjenn/wispr-bro/releases/x"
        ));
        assert_eq!(
            is_update_url("http://127.0.0.1:1/fjenn/wispr-bro/releases/x"),
            cfg!(debug_assertions)
        );
    }
}
