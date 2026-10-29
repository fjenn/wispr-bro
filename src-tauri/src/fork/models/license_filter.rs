// SPDX-License-Identifier: GPL-3.0-or-later
//! Model licence filter (DEC-27, R4 §5 licence register).
//!
//! - **Hidden** from our catalog view: the 12 non-English Moonshine models
//!   (`moonshine-{tiny,base}-{ar,ja,ko,uk,vi,zh}`, Moonshine Community
//!   License, non-commercial; Handy's `catalog.json` mislabels them `mit`),
//!   and every catalog model whose licence is non-commercial (`cc-by-nc*`).
//! - **Never default**: the hidden ones, plus `other`-licensed models and the
//!   NVIDIA Open Model License models (Sortformer, multitalker-parakeet).
//!   DEC-27 keeps them out of every silent choice; F-08 also keeps them out of
//!   the catalog view, so nothing the fork shows can lead to them.
//!
//! The catalog view keeps a model the user already downloaded, so an
//! existing install never loses its working model.
//!
//! Where it applies: the fork's own model surfaces (S-07a/S-07b silent model
//! policy, the S-14 Advanced list) call [`catalog_view`] on Handy's
//! `ModelManager::get_available_models()`. Handy's own model picker is not
//! hooked (no hook is listed for it in R5 §4.2, and S-13 replaces that UI).

use std::collections::HashMap;

use once_cell::sync::Lazy;
use serde::Deserialize;

use crate::managers::model::{ModelInfo, ModelSource};

/// The 12 non-English Moonshine models (catalog ids).
pub const NON_COMMERCIAL_MOONSHINE: &[&str] = &[
    "moonshine-tiny-ar",
    "moonshine-tiny-ja",
    "moonshine-tiny-ko",
    "moonshine-tiny-uk",
    "moonshine-tiny-vi",
    "moonshine-tiny-zh",
    "moonshine-base-ar",
    "moonshine-base-ja",
    "moonshine-base-ko",
    "moonshine-base-uk",
    "moonshine-base-vi",
    "moonshine-base-zh",
];

/// Models under the NVIDIA Open Model License (R4 §5), by catalog id prefix.
pub const NVIDIA_OPEN_MODEL_LICENSE: &[&str] =
    &["diar_streaming_sortformer", "multitalker-parakeet"];

#[derive(Deserialize)]
struct Catalog {
    models: Vec<CatalogEntry>,
}

#[derive(Deserialize)]
struct CatalogEntry {
    id: String,
    repository: String,
    #[serde(default)]
    license: String,
}

/// Catalog id and licence, keyed by HF repository (`handy-computer/…-gguf`).
static LICENSES: Lazy<HashMap<String, (String, String)>> = Lazy::new(|| {
    let catalog: Catalog = serde_json::from_str(include_str!("../../catalog/catalog.json"))
        .unwrap_or(Catalog { models: Vec::new() });
    catalog
        .models
        .into_iter()
        .map(|m| (m.repository, (m.id, m.license.to_ascii_lowercase())))
        .collect()
});

/// Why a catalog model is filtered, if it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restriction {
    /// Non-commercial licence: hidden and never default.
    NonCommercial,
    /// `other` or NVIDIA licence: never default (and not shown, see module
    /// docs).
    NeverDefault,
}

/// Restriction for a catalog id with a catalog licence string.
pub fn restriction_for(catalog_id: &str, license: &str) -> Option<Restriction> {
    let license = license.to_ascii_lowercase();
    if NON_COMMERCIAL_MOONSHINE.contains(&catalog_id) || license.contains("-nc") {
        Some(Restriction::NonCommercial)
    } else if license == "other"
        || NVIDIA_OPEN_MODEL_LICENSE
            .iter()
            .any(|p| catalog_id.starts_with(p))
    {
        Some(Restriction::NeverDefault)
    } else {
        None
    }
}

/// Restriction for a model in Handy's registry (catalog models only; legacy,
/// on-disk and custom models are not in the licence register and pass).
pub fn restriction(model: &ModelInfo) -> Option<Restriction> {
    let ModelSource::HuggingFace { repo_id, .. } = &model.source else {
        return None;
    };
    let (id, license) = LICENSES.get(repo_id)?;
    restriction_for(id, license)
}

/// `true` when a model may be chosen silently (S-07a/S-07b).
pub fn may_be_default(model: &ModelInfo) -> bool {
    restriction(model).is_none()
}

/// The fork's catalog view of Handy's model list: drops restricted models
/// unless already downloaded.
pub fn catalog_view(models: Vec<ModelInfo>) -> Vec<ModelInfo> {
    models
        .into_iter()
        .filter(|m| m.is_downloaded || restriction(m).is_none())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> Vec<ModelInfo> {
        // Same conversion as Handy's `ModelManager::seed_catalog_models`.
        let status = crate::managers::model::DiskStatus::default();
        crate::catalog::CATALOG
            .iter()
            .map(|d| d.to_model_info(&status))
            .collect()
    }

    fn catalog_id(m: &ModelInfo) -> Option<String> {
        match &m.source {
            ModelSource::HuggingFace { repo_id, .. } => {
                LICENSES.get(repo_id).map(|(id, _)| id.clone())
            }
            _ => None,
        }
    }

    #[test]
    fn the_twelve_moonshine_models_are_in_the_catalog_and_hidden() {
        let all = registry();
        let view = catalog_view(all.clone());
        for id in NON_COMMERCIAL_MOONSHINE {
            assert!(
                all.iter().any(|m| catalog_id(m).as_deref() == Some(id)),
                "{id} is not offered by Handy's catalog any more; update the list"
            );
            assert!(
                !view.iter().any(|m| catalog_id(m).as_deref() == Some(id)),
                "{id} is visible"
            );
        }
        assert_eq!(NON_COMMERCIAL_MOONSHINE.len(), 12);
    }

    #[test]
    fn english_moonshine_and_permissive_models_stay_visible() {
        let view = catalog_view(registry());
        let ids: Vec<_> = view.iter().filter_map(catalog_id).collect();
        for id in [
            "moonshine-base",
            "moonshine-tiny",
            "whisper-small",
            "parakeet-tdt-0.6b-v3",
        ] {
            assert!(ids.iter().any(|i| i == id), "{id} hidden; view {ids:?}");
        }
    }

    #[test]
    fn never_default_models_are_flagged_and_hidden() {
        for (id, license) in [
            ("nemotron-3.5-asr-streaming-0.6b", "other"),
            ("SenseVoiceSmall", "other"),
            ("multitalker-parakeet-streaming-0.6b-v1", "other"),
            ("diar_streaming_sortformer_4spk-v2.1", "other"),
        ] {
            assert_eq!(
                restriction_for(id, license),
                Some(Restriction::NeverDefault),
                "{id}"
            );
        }
        assert_eq!(
            restriction_for("canary-1b", "cc-by-nc-4.0"),
            Some(Restriction::NonCommercial)
        );
        assert_eq!(restriction_for("whisper-small", "apache-2.0"), None);
        assert_eq!(restriction_for("parakeet-tdt-0.6b-v3", "cc-by-4.0"), None);
        let view = catalog_view(registry());
        assert!(view.iter().all(may_be_default));
    }

    #[test]
    fn a_downloaded_restricted_model_stays_in_the_view() {
        let mut all = registry();
        let m = all
            .iter_mut()
            .find(|m| catalog_id(m).as_deref() == Some("moonshine-base-ja"))
            .unwrap();
        m.is_downloaded = true;
        let view = catalog_view(all);
        assert!(view
            .iter()
            .any(|m| catalog_id(m).as_deref() == Some("moonshine-base-ja")));
    }
}
