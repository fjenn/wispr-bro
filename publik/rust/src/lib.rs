// SPDX-License-Identifier: GPL-3.0-or-later
//! The publik edition's Rust code (publik/README.md).
//!
//! A crate of its own, which the app (src-tauri) depends on only with its
//! `publik` Cargo feature, which `publik/tauri.conf.json` turns on. The
//! normal Wispr Bro build never compiles it, and `publik/guard.ts` fails CI
//! if anything from it reaches a normal build. The app reaches it only behind
//! `#[cfg(feature = "publik")]`, in three places: [`start`] at launch,
//! [`gate::hold`] before the app's `on_ready`, and the gate's commands.
//!
//! - [`client`]: publik's app protocol (install, wallet, usage events, the
//!   release self test), the same requests, files and words as publik's own
//!   Rust module;
//! - [`gate`]: the "Sign in with publik" window that stands in front of the
//!   app until the person is signed in;
//! - the edition's own update feeds (below).

pub mod client;
#[cfg(test)]
mod fake;
pub mod gate;

/// This app on publik (its listing's slug).
pub const APP_SLUG: &str = "wispr-bro";
/// The app's version, given by [`start`] (this crate has its own version).
static APP_VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// The version publik is told about: the app's.
pub fn app_version() -> &'static str {
    APP_VERSION.get().map(String::as_str).unwrap_or("0.0.0")
}

/// First thing at launch: keeps the app's version and answers publik's
/// release self test (`PUBLIK_SELF_TEST=1`), which exits before anything
/// else starts.
pub fn start(app_version: &str) {
    let _ = APP_VERSION.set(app_version.to_string());
    client::self_test_if_requested();
}
/// Whether the app uses publik's AI models. Off until the optional cloud
/// mode (step 4); with it on, signed-in people choose a publik plan or their
/// own key after signing in.
pub const USES_AI: bool = false;
/// publik's models, for the cloud mode.
pub const MODELS: [&str; 3] = ["publik-fast", "publik-balanced", "publik-smart"];

/// The publik edition's stable update feed, on its own rolling `publik`
/// release (never the latest release), so the normal release, its
/// `latest.json` and every site that reads `releases/latest` only ever see
/// the normal build, and an update never swaps one edition for the other
/// (the app's `fork::update::channel`). Its downloads are on
/// `publik-v<version>`.
pub const STABLE_PATH: &str = "/releases/download/publik/latest-publik.json";
/// The publik edition's feed on its own rolling `publik-beta` prerelease.
pub const BETA_PATH: &str = "/releases/download/publik-beta/latest-publik.json";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feeds_are_on_the_publik_releases_never_the_normal_ones() {
        assert_eq!(STABLE_PATH, "/releases/download/publik/latest-publik.json");
        assert_eq!(
            BETA_PATH,
            "/releases/download/publik-beta/latest-publik.json"
        );
        for path in [STABLE_PATH, BETA_PATH] {
            assert!(!path.contains("/releases/latest/"));
            assert!(!path.contains("/download/beta/"));
        }
    }

    /// The publik overlay points the updater at the stable feed and turns
    /// the app's `publik` feature on.
    #[test]
    fn the_overlay_reads_the_publik_feed() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tauri.conf.json");
        let overlay: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(
            overlay["plugins"]["updater"]["endpoints"],
            serde_json::json!([format!("https://github.com/fjenn/wispr-bro{STABLE_PATH}")])
        );
        assert_eq!(overlay["build"]["features"], serde_json::json!(["publik"]));
    }
}
