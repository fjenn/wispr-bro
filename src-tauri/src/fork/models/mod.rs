// SPDX-License-Identifier: GPL-3.0-or-later
//! Fork model layer. F-08: the licence filter ([`license_filter`]), the
//! `HF_HOME` location (DEC-39) and the proof that no download path builds a
//! `blob.handy.computer` URL (DEC-16). S-07a: the first speech model and
//! the mic gate ([`setup`], [`starter`], [`readiness`], [`download`],
//! [`probe`], [`queue_session`]). S-07b: the silent model policy
//! ([`policy`], [`machine`], [`speed`]), upgrades ([`upgrade`]) and the
//! deletion of superseded models ([`cleanup`]).
//!
//! Neither slice needs a catalog overlay or a `managers/model.rs` hook:
//! every model the fork picks is a default file of Handy's catalog, and
//! Handy's downloader moves the bytes. Handy's stale `HANDY_RANKING` is
//! bypassed, not edited: the fork never asks Handy which model to use.

pub mod cleanup;
pub mod download;
pub mod license_filter;
pub mod machine;
pub mod policy;
pub mod probe;
pub mod queue_session;
pub mod readiness;
pub mod setup;
pub mod speed;
pub mod starter;
pub mod upgrade;

use std::path::{Path, PathBuf};

/// Sub-folder of the app data dir that holds the Hugging Face cache, the same
/// name Handy's portable mode uses (`portable.rs`).
pub const HF_HOME_DIR: &str = "huggingface";

/// DEC-39: `HF_HOME` for a normal (non-portable) macOS install,
/// `<home>/Library/Application Support/<identifier>/huggingface`. This is
/// Tauri's `app_data_dir()` on macOS, computed by hand because `early_init`
/// runs before Tauri resolves paths. `None` off macOS (S-07a decides the
/// Windows/Linux layout).
pub fn hf_home_for(home: &Path, identifier: &str) -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        Some(
            home.join("Library/Application Support")
                .join(identifier)
                .join(HF_HOME_DIR),
        )
    } else {
        None
    }
}

/// What `early_init` does with `HF_HOME`: the value to set, or `None` to
/// leave the environment alone (plain Handy identity, portable mode, which
/// sets its own `HF_HOME`, no `$HOME`, or not macOS).
pub fn hf_home_to_set(
    ours: bool,
    portable: bool,
    home: Option<&Path>,
    identifier: &str,
) -> Option<PathBuf> {
    if !ours || portable {
        return None;
    }
    hf_home_for(home?, identifier)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::net::HANDY_MIRROR_HOST;

    #[test]
    fn hf_home_only_for_our_identity_and_never_in_portable_mode() {
        let home = Path::new("/Users/alice");
        let id = crate::fork::PRODUCT_IDENTIFIER;
        if cfg!(target_os = "macos") {
            assert_eq!(
                hf_home_to_set(true, false, Some(home), id),
                Some(PathBuf::from(
                    "/Users/alice/Library/Application Support/com.wisprbro.desktop/huggingface"
                ))
            );
        }
        assert_eq!(hf_home_to_set(false, false, Some(home), id), None);
        assert_eq!(hf_home_to_set(true, true, Some(home), id), None);
        assert_eq!(hf_home_to_set(true, false, None, id), None);
    }

    /// DEC-16 / `ci/inert-hosts.txt` proof for `blob.handy.computer`: no
    /// download path of Handy's model manager produces a URL on that host.
    ///
    /// - Mirror fallback (`FileTransfer::mirrors`): built only by
    ///   `catalog::mirror_fallbacks` / `catalog::language_detector_mirrors`,
    ///   both through `catalog::mirror_files`, whose mirror list passes
    ///   `fork::net::mirror_base` (hook). Checked for every catalog model and
    ///   every quant file, and for the language detector.
    /// - Legacy URL models (`ModelSource::Url`): `download_model` passes the
    ///   URL through `fork::net::checked_download_url` (hook), which refuses
    ///   it before any request.
    /// - Hugging Face downloads go through hf-hub to `huggingface.co`.
    #[test]
    fn downloader_never_builds_a_blob_handy_computer_url() {
        let mut checked = 0;
        for desc in crate::catalog::CATALOG.iter() {
            let mut ids = vec![desc.id.clone()];
            if let crate::managers::model::ModelSource::HuggingFace { repo_id, .. } = &desc.source {
                ids.extend(
                    desc.files
                        .iter()
                        .map(|f| format!("{repo_id}/{}", f.filename)),
                );
            }
            for id in ids {
                for mirror in crate::catalog::mirror_fallbacks(&id) {
                    assert!(
                        crate::fork::net::is_allowed_host(&mirror.url),
                        "{id}: mirror {}",
                        mirror.url
                    );
                }
                checked += 1;
            }
        }
        assert!(checked > 50, "catalog unexpectedly small ({checked})");
        for mirror in crate::catalog::language_detector_mirrors() {
            assert!(!mirror.url.contains(HANDY_MIRROR_HOST), "{}", mirror.url);
        }

        // Legacy URL entries are refused before any request.
        let model_rs = include_str!("../../managers/model.rs");
        let legacy: Vec<&str> = model_rs
            .split('"')
            .filter(|s| s.starts_with("https://") && s.contains(HANDY_MIRROR_HOST))
            .collect();
        assert!(!legacy.is_empty());
        for url in legacy {
            assert!(
                crate::fork::net::checked_download_url(url).is_err(),
                "{url}"
            );
        }

        // The hooks are in place.
        let catalog_rs = include_str!("../../catalog/mod.rs");
        assert!(catalog_rs.contains(".filter_map(|base| crate::fork::net::mirror_base(base))"));
        assert!(model_rs.contains("crate::fork::net::checked_download_url(url)?"));
        assert_eq!(
            model_rs
                .matches("ModelSource::Url { url, sha256 } =>")
                .count(),
            1
        );
    }
}
