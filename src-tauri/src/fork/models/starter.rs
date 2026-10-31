// SPDX-License-Identifier: GPL-3.0-or-later
//! The first speech model (S-07a, DEC-01, DEC-52, RS-09).
//!
//! A new install downloads a small model first so the first dictation
//! works within a minute; S-07b later swaps in the best model for this Mac.
//! Which one comes from `catalog.fork.json` (the candidates in order of
//! preference) and the languages the user speaks:
//!
//! 1. the first candidate that is usable for **every** language the user
//!    speaks (so an English + Dutch speaker never gets an English-only
//!    model, and nobody's Dutch goes through whisper-base, DEC-52);
//! 2. else the first one usable for their first language;
//! 3. else the last resort (whisper-base: 99 languages).
//!
//! With RS-09's table that gives: English → parakeet-110m (135 MB), French
//! and the other whisper languages → whisper-base (85 MB), Dutch (alone or
//! with others) → Parakeet TDT 0.6B v3 (740 MB, DEC-52's "no starter":
//! takes are queued until it is ready).
//!
//! Pins (repository, revision, file, size, sha256) are read from Handy's
//! bundled catalog (`catalog/catalog.json`), the same pinned upstream
//! Hugging Face repos Handy downloads from; nothing is duplicated here.

use once_cell::sync::Lazy;
use serde::Deserialize;

/// One downloadable speech model, pinned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pinned {
    /// Catalog id (`parakeet-tdt_ctc-110m`).
    pub catalog_id: String,
    /// Hugging Face repository (`handy-computer/…-gguf`).
    pub repo: String,
    pub revision: String,
    pub file: String,
    pub size: u64,
    pub sha256: String,
    /// The catalog's licence id (`cc-by-4.0`, `apache-2.0`).
    pub license: String,
    /// Languages the catalog says it hears (base codes).
    pub languages: Vec<String>,
    /// Languages RS-09 measured as unusable on it.
    pub unusable_for: Vec<String>,
}

impl Pinned {
    /// Handy's model id for it (`<repo>/<file>`, `catalog/mod.rs`).
    pub fn handy_id(&self) -> String {
        format!("{}/{}", self.repo, self.file)
    }

    /// The immutable resolve URL of the pinned file (what the probe asks).
    pub fn resolve_url(&self) -> String {
        format!(
            "https://huggingface.co/{}/resolve/{}/{}",
            self.repo, self.revision, self.file
        )
    }

    /// Whether it is good enough for `language` (a base code).
    pub fn usable_for(&self, language: &str) -> bool {
        self.languages.iter().any(|l| l == language)
            && !self.unusable_for.iter().any(|l| l == language)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForkCatalog {
    quant: String,
    candidates: Vec<Candidate>,
    last_resort: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Candidate {
    id: String,
    #[serde(default)]
    unusable_for: Vec<String>,
}

#[derive(Deserialize)]
struct HandyCatalog {
    models: Vec<HandyModel>,
}

#[derive(Deserialize)]
struct HandyModel {
    id: String,
    repository: String,
    revision: String,
    #[serde(default)]
    license: String,
    #[serde(default)]
    languages: Vec<String>,
    files: std::collections::HashMap<String, HandyFile>,
}

#[derive(Deserialize)]
struct HandyFile {
    filename: String,
    size: u64,
    sha256: String,
}

/// The candidates, pinned, in order of preference, and the last resort.
pub struct Starters {
    pub candidates: Vec<Pinned>,
    pub last_resort: Pinned,
}

fn load(fork_json: &str, handy_json: &str) -> Result<Starters, String> {
    let fork: ForkCatalog =
        serde_json::from_str(fork_json).map_err(|e| format!("catalog.fork.json: {e}"))?;
    let handy: HandyCatalog =
        serde_json::from_str(handy_json).map_err(|e| format!("catalog.json: {e}"))?;
    let pin = |id: &str, unusable_for: Vec<String>| -> Result<Pinned, String> {
        let m = handy
            .models
            .iter()
            .find(|m| m.id == id)
            .ok_or_else(|| format!("{id} is not in Handy's catalog"))?;
        let f = m
            .files
            .get(&fork.quant)
            .ok_or_else(|| format!("{id} has no {} file", fork.quant))?;
        Ok(Pinned {
            catalog_id: m.id.clone(),
            repo: m.repository.clone(),
            revision: m.revision.clone(),
            file: f.filename.clone(),
            size: f.size,
            sha256: f.sha256.to_ascii_lowercase(),
            license: m.license.to_ascii_lowercase(),
            languages: m.languages.iter().map(|l| base_language(l)).collect(),
            unusable_for,
        })
    };
    let candidates = fork
        .candidates
        .iter()
        .map(|c| pin(&c.id, c.unusable_for.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    let last_resort = candidates
        .iter()
        .find(|c| c.catalog_id == fork.last_resort)
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| pin(&fork.last_resort, Vec::new()))?;
    Ok(Starters {
        candidates,
        last_resort,
    })
}

/// The bundled table. A broken table is a build error caught by the tests;
/// at runtime it means no starter (the app then behaves like Handy).
pub static STARTERS: Lazy<Option<Starters>> = Lazy::new(|| {
    load(
        include_str!("catalog.fork.json"),
        include_str!("../../catalog/catalog.json"),
    )
    .map_err(|e| log::error!("models: starter table unusable: {e}"))
    .ok()
});

/// `en-US` → `en`, `zh-Hans-CN` → `zh`, `pt_BR` → `pt`; lower case.
pub fn base_language(tag: &str) -> String {
    tag.split(['-', '_'])
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}

/// The user's languages as base codes, in order, without duplicates or
/// empties: Handy's `spoken_languages` when set (Welcome saves them), else
/// the macOS preferred languages.
pub fn user_languages(spoken: &[String], os_preferred: &[String]) -> Vec<String> {
    let source = if spoken.iter().any(|s| !s.trim().is_empty()) {
        spoken
    } else {
        os_preferred
    };
    let mut out: Vec<String> = Vec::new();
    for l in source.iter().map(|l| base_language(l)) {
        if !l.is_empty() && !out.contains(&l) {
            out.push(l);
        }
    }
    out
}

/// The model to download first for these languages (rules in the module
/// docs). No languages counts as English.
pub fn pick<'a>(starters: &'a Starters, languages: &[String]) -> &'a Pinned {
    let english = ["en".to_string()];
    let languages = if languages.is_empty() {
        &english[..]
    } else {
        languages
    };
    starters
        .candidates
        .iter()
        .find(|c| languages.iter().all(|l| c.usable_for(l)))
        .or_else(|| {
            starters
                .candidates
                .iter()
                .find(|c| c.usable_for(&languages[0]))
        })
        .unwrap_or(&starters.last_resort)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn starters() -> &'static Starters {
        STARTERS.as_ref().expect("the bundled starter table loads")
    }

    fn langs(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| s.to_string()).collect()
    }

    fn pick_id(l: &[&str]) -> String {
        pick(starters(), &langs(l)).catalog_id.clone()
    }

    #[test]
    fn the_table_pins_the_rs09_files() {
        let s = starters();
        let ids: Vec<_> = s.candidates.iter().map(|c| c.catalog_id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "parakeet-tdt_ctc-110m",
                "whisper-base",
                "parakeet-tdt-0.6b-v3"
            ]
        );
        // RS-09's sha256 values (docs/fork/spikes/RS-09-asr-bench.md).
        let by = |id: &str| s.candidates.iter().find(|c| c.catalog_id == id).unwrap();
        assert_eq!(
            by("parakeet-tdt_ctc-110m").sha256,
            "7dd44c74a331d788a4e5f8b16913b3feb29ced22cf5613aad0e0f6cd30516296"
        );
        assert_eq!(
            by("whisper-base").sha256,
            "81c069428bc8a24551a8169cf31cf09bcfd9d4cf50389ae281323c9aa9648c81"
        );
        assert_eq!(
            by("parakeet-tdt-0.6b-v3").sha256,
            "5859f77944efcd8eafa23a6350731960b2b55b2203df51f319665c807d802cc7"
        );
        for c in &s.candidates {
            assert_eq!(c.sha256.len(), 64);
            assert!(c.size > 50_000_000, "{}", c.catalog_id);
            assert_eq!(c.revision.len(), 40, "{}: pinned commit", c.catalog_id);
            assert!(c.repo.starts_with("handy-computer/"));
            assert!(crate::fork::net::is_allowed_host(&c.resolve_url()));
            // Handy's catalog offers it under this id.
            assert!(
                crate::catalog::CATALOG.iter().any(|d| d.id == c.handy_id()),
                "{} not offered by Handy",
                c.handy_id()
            );
            // DEC-27: never a non-commercial or never-default model.
            assert!(
                crate::fork::models::license_filter::restriction_for(&c.catalog_id, &c.license)
                    .is_none(),
                "{}",
                c.catalog_id
            );
        }
        assert_eq!(s.last_resort.catalog_id, "whisper-base");
        // The §5.3 starter cap (150 MB) holds for the true starters.
        assert!(by("parakeet-tdt_ctc-110m").size < 150_000_000);
        assert!(by("whisper-base").size < 150_000_000);
    }

    #[test]
    fn english_gets_parakeet_110m() {
        assert_eq!(pick_id(&["en"]), "parakeet-tdt_ctc-110m");
        assert_eq!(pick_id(&[]), "parakeet-tdt_ctc-110m");
    }

    #[test]
    fn french_and_other_whisper_languages_get_whisper_base() {
        assert_eq!(pick_id(&["fr"]), "whisper-base");
        assert_eq!(pick_id(&["ja"]), "whisper-base");
        assert_eq!(pick_id(&["de", "es"]), "whisper-base");
        // A second language the English model does not hear.
        assert_eq!(pick_id(&["en", "fr"]), "whisper-base");
    }

    #[test]
    fn dutch_never_goes_through_whisper_base() {
        assert_eq!(pick_id(&["nl"]), "parakeet-tdt-0.6b-v3");
        assert_eq!(pick_id(&["en", "nl"]), "parakeet-tdt-0.6b-v3");
        assert_eq!(pick_id(&["nl", "fr"]), "parakeet-tdt-0.6b-v3");
        // Nothing hears Dutch and Japanese: the first language decides.
        assert_eq!(pick_id(&["nl", "ja"]), "parakeet-tdt-0.6b-v3");
        assert_eq!(pick_id(&["ja", "nl"]), "whisper-base");
    }

    #[test]
    fn a_language_no_candidate_hears_gets_the_last_resort() {
        assert_eq!(pick_id(&["xx"]), "whisper-base");
    }

    #[test]
    fn languages_come_from_handy_then_the_os() {
        assert_eq!(
            user_languages(&langs(&["nl", "en"]), &langs(&["fr-FR"])),
            langs(&["nl", "en"])
        );
        assert_eq!(
            user_languages(&[], &langs(&["en-GB", "en-US", "zh-Hans-CN", "pt_BR"])),
            langs(&["en", "zh", "pt"])
        );
        assert_eq!(user_languages(&langs(&[" "]), &[]), Vec::<String>::new());
    }

    #[test]
    fn a_broken_table_is_an_error() {
        let handy = include_str!("../../catalog/catalog.json");
        assert!(load("{", handy).is_err());
        assert!(load(
            r#"{"quant":"Q8_0","candidates":[{"id":"nope"}],"lastResort":"nope"}"#,
            handy
        )
        .is_err());
        assert!(load(
            r#"{"quant":"Q1","candidates":[{"id":"whisper-base"}],"lastResort":"whisper-base"}"#,
            handy
        )
        .is_err());
    }
}
