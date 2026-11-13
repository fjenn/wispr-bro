// SPDX-License-Identifier: GPL-3.0-or-later
//! The publik edition (docs/fork/EDITIONS.md).
//!
//! Compiled only with the `publik` Cargo feature, which
//! `src-tauri/tauri.publik.conf.json` turns on. The normal Wispr Bro build
//! never compiles this module, and `scripts/wb-guard-edition.ts` fails CI if
//! anything from it reaches a normal build.
//!
//! Everything the publik edition adds lives here (and in `src/fork/publik/`
//! on the frontend). Code elsewhere reaches it only behind
//! `#[cfg(feature = "publik")]`.
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
/// The version publik is told about.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Whether the app uses publik's AI models. Off until the optional cloud
/// mode (step 4); with it on, signed-in people choose a publik plan or their
/// own key after signing in.
pub const USES_AI: bool = false;
/// publik's models, for the cloud mode.
pub const MODELS: [&str; 3] = ["publik-fast", "publik-balanced", "publik-smart"];

/// The publik edition's stable update feed, next to the normal `latest.json`
/// on the same GitHub release, so an update never swaps one edition for the
/// other (`fork::update::channel`).
pub const STABLE_PATH: &str = "/releases/latest/download/latest-publik.json";
/// The publik edition's feed on the rolling `beta` prerelease.
pub const BETA_PATH: &str = "/releases/download/beta/latest-publik.json";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feeds_are_the_publik_files_next_to_the_normal_ones() {
        assert_eq!(
            STABLE_PATH.replace("latest-publik.json", "latest.json"),
            "/releases/latest/download/latest.json"
        );
        assert_eq!(
            BETA_PATH.replace("latest-publik.json", "latest.json"),
            "/releases/download/beta/latest.json"
        );
    }

    /// The update channels read these feeds, and the publik overlay points the
    /// updater at the same stable feed and turns this feature on.
    #[test]
    fn the_publik_edition_reads_its_own_feeds() {
        use crate::fork::settings::UpdateChannel;
        use crate::fork::update::channel::{endpoints_on, stable_endpoint, GITHUB_BASE};
        assert_eq!(
            endpoints_on(GITHUB_BASE, UpdateChannel::Beta),
            vec![
                "https://github.com/fjenn/wispr-bro/releases/download/beta/latest-publik.json",
                "https://github.com/fjenn/wispr-bro/releases/latest/download/latest-publik.json",
            ]
        );
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.publik.conf.json");
        let overlay: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(
            overlay["plugins"]["updater"]["endpoints"],
            serde_json::json!([stable_endpoint()])
        );
        assert_eq!(overlay["build"]["features"], serde_json::json!(["publik"]));
    }
}
