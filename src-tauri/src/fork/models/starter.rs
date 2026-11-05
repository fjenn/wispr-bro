// SPDX-License-Identifier: GPL-3.0-or-later
//! The first speech model (S-07a, DEC-01, DEC-52, RS-09).
//!
//! A new install downloads a small model first so the first dictation
//! works within a minute; S-07b later swaps in the best model for this Mac.
//! Which one comes from `catalog.fork.json` (the candidates in order of
//! preference) and the languages the user speaks:
//!
//! 1. the first **small** candidate (under [`STARTER_CAP_BYTES`], §5.3) that
//!    is usable for every language the user speaks;
//! 2. else a **partial starter**: the small candidate usable for the first
//!    of the user's languages that has one (the owner's rc9 install: English
//!    and Dutch get parakeet-110m first, not 3.5 minutes of nothing). Takes
//!    in a language it does not hear wait for the full model, which S-07b
//!    downloads right after it ([`needs_full_model`], DEC-52: Dutch is never
//!    transcribed with a weak model);
//! 3. else the first candidate (any size) usable for every language;
//! 4. else the first one usable for their first language;
//! 5. else the last resort (whisper-base: 99 languages).
//!
//! With RS-09's table that gives: English → parakeet-110m (90 MB), French
//! and the other whisper languages → whisper-base (85 MB), English + Dutch
//! → parakeet-110m (Dutch waits for Parakeet v3), French + Dutch →
//! whisper-base (Dutch waits), Dutch alone → Parakeet TDT 0.6B v3 (740 MB,
//! DEC-52's "no starter": takes are queued until it is ready).
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

    /// Small enough to be a true starter (§5.3: ready within a minute).
    pub fn is_starter_sized(&self) -> bool {
        self.size < STARTER_CAP_BYTES
    }
}

/// §5.3's starter cap: what downloads within the first-run budget.
pub const STARTER_CAP_BYTES: u64 = 150_000_000;

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
    /// This candidate's quant, when not the table's (S-07b).
    #[serde(default)]
    quant: Option<String>,
    #[serde(default)]
    unusable_for: Vec<String>,
}

#[derive(Deserialize)]
pub(crate) struct HandyCatalog {
    models: Vec<HandyModel>,
}

#[derive(Deserialize)]
struct HandyModel {
    id: String,
    repository: String,
    revision: String,
    /// The default quant.
    #[serde(default)]
    quant: String,
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

/// Pins catalog model `id` at `quant` from Handy's parsed catalog.
pub(crate) fn pin_in(
    handy: &HandyCatalog,
    id: &str,
    quant: &str,
    unusable_for: Vec<String>,
) -> Result<Pinned, String> {
    let m = handy
        .models
        .iter()
        .find(|m| m.id == id)
        .ok_or_else(|| format!("{id} is not in Handy's catalog"))?;
    let f = m
        .files
        .get(quant)
        .ok_or_else(|| format!("{id} has no {quant} file"))?;
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
}

pub(crate) fn parse_handy(handy_json: &str) -> Result<HandyCatalog, String> {
    serde_json::from_str(handy_json).map_err(|e| format!("catalog.json: {e}"))
}

/// Handy's bundled catalog, parsed once for the fork's tables.
pub(crate) static HANDY: Lazy<Option<HandyCatalog>> = Lazy::new(|| {
    parse_handy(include_str!("../../catalog/catalog.json"))
        .map_err(|e| log::error!("models: {e}"))
        .ok()
});

/// Catalog model `id` pinned at `quant` (its default quant when `None`).
pub fn pinned(id: &str, quant: Option<&str>) -> Option<Pinned> {
    let handy = HANDY.as_ref()?;
    let quant = match quant {
        Some(q) => q.to_string(),
        None => handy.models.iter().find(|m| m.id == id)?.quant.clone(),
    };
    pin_in(handy, id, &quant, Vec::new()).ok()
}

fn load(fork_json: &str, handy_json: &str) -> Result<Starters, String> {
    let fork: ForkCatalog =
        serde_json::from_str(fork_json).map_err(|e| format!("catalog.fork.json: {e}"))?;
    let handy = parse_handy(handy_json)?;
    let pin = |id: &str, unusable_for: Vec<String>| pin_in(&handy, id, &fork.quant, unusable_for);
    let candidates = fork
        .candidates
        .iter()
        .map(|c| {
            let quant = c.quant.as_deref().unwrap_or(&fork.quant);
            pin_in(&handy, &c.id, quant, c.unusable_for.clone())
        })
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

/// No languages counts as English.
fn or_english(languages: &[String]) -> std::borrow::Cow<'_, [String]> {
    if languages.is_empty() {
        std::borrow::Cow::Owned(vec!["en".to_string()])
    } else {
        std::borrow::Cow::Borrowed(languages)
    }
}

/// The model to download first for these languages (rules in the module
/// docs). No languages counts as English.
pub fn pick<'a>(starters: &'a Starters, languages: &[String]) -> &'a Pinned {
    let languages = or_english(languages);
    let c = &starters.candidates;
    let small = |p: &&Pinned| p.is_starter_sized();
    c.iter()
        .filter(small)
        .find(|p| languages.iter().all(|l| p.usable_for(l)))
        .or_else(|| {
            languages
                .iter()
                .find_map(|l| c.iter().filter(small).find(|p| p.usable_for(l)))
        })
        .or_else(|| c.iter().find(|p| languages.iter().all(|l| p.usable_for(l))))
        .or_else(|| c.iter().find(|p| p.usable_for(&languages[0])))
        .unwrap_or(&starters.last_resort)
}

/// Whether `starter` is a partial one: some language the user speaks must
/// wait for the full model.
pub fn is_partial(starter: &Pinned, languages: &[String]) -> bool {
    !or_english(languages).iter().all(|l| starter.usable_for(l))
}

/// The user's languages `starter` hears, in the user's order (the practice
/// text uses the first, so the practice take can run on the starter).
pub fn heard_by(starter: &Pinned, languages: &[String]) -> Vec<String> {
    or_english(languages)
        .iter()
        .filter(|l| starter.usable_for(l))
        .cloned()
        .collect()
}

/// Whether a take must wait for the full model while the partial starter
/// `starter` is selected. `take_language` is the take's language when it
/// is known before transcribing (Handy's language set to one language);
/// with Auto it is unknown, and only the onboarding practice take (whose
/// text is in a language the starter hears) runs on the starter. Anything
/// else waits: DEC-52, a weak model never gets Dutch.
pub fn needs_full_model(starter: &Pinned, take_language: Option<&str>, practice: bool) -> bool {
    match take_language
        .map(base_language)
        .filter(|l| !l.is_empty() && l != "auto")
    {
        Some(l) => !starter.usable_for(&l),
        None => !practice,
    }
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
        // S-07b: the Q4_K_M file (S-07b bench, docs/fork/test-runs/…-S-07b.md).
        assert_eq!(
            by("parakeet-tdt_ctc-110m").sha256,
            "486414fd90185a8c8a4ced7c123cfb133ff4f7958426c6b8bd9049946b56b448"
        );
        assert_eq!(
            by("parakeet-tdt_ctc-110m").file,
            "parakeet-tdt_ctc-110m-Q4_K_M.gguf"
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
            // A file of Handy's catalog (a default, or another quant the
            // fork downloads itself, S-07b).
            assert!(
                crate::catalog::file_in_catalog(&c.file, Some(&c.repo)).is_some(),
                "{} not in Handy's catalog",
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
        // The §5.3 starter cap (150 MB) holds for the true starters, and
        // S-07b: both transfer within 80 s at 10 Mbit/s (1.25 MB/s), so
        // "ready" fits the 90 s budget with launch and checksum.
        for id in ["parakeet-tdt_ctc-110m", "whisper-base"] {
            assert!(by(id).size < 100_000_000, "{id}");
        }
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
    fn dutch_alone_gets_no_starter() {
        assert_eq!(pick_id(&["nl"]), "parakeet-tdt-0.6b-v3");
        let s = pick(starters(), &langs(&["nl"]));
        assert!(!is_partial(s, &langs(&["nl"])));
    }

    #[test]
    fn a_second_language_with_a_starter_gets_a_partial_starter() {
        // The owner's rc9 install: English + Dutch, in either order.
        for l in [&["en", "nl"][..], &["nl", "en"][..]] {
            assert_eq!(pick_id(l), "parakeet-tdt_ctc-110m", "{l:?}");
            let s = pick(starters(), &langs(l));
            assert!(s.is_starter_sized());
            assert!(is_partial(s, &langs(l)));
            assert_eq!(heard_by(s, &langs(l)), langs(&["en"]));
        }
        // French + Dutch: whisper-base for French, never for Dutch.
        for l in [&["fr", "nl"][..], &["nl", "fr"][..]] {
            assert_eq!(pick_id(l), "whisper-base", "{l:?}");
            let s = pick(starters(), &langs(l));
            assert!(is_partial(s, &langs(l)));
            assert!(!s.usable_for("nl"));
            assert_eq!(heard_by(s, &langs(l)), langs(&["fr"]));
        }
        // English + French: one small model hears both, not partial.
        let s = pick(starters(), &langs(&["en", "fr"]));
        assert!(!is_partial(s, &langs(&["en", "fr"])));
        // The first language with a starter wins.
        assert_eq!(pick_id(&["nl", "ja"]), "whisper-base");
        assert_eq!(pick_id(&["ja", "nl"]), "whisper-base");
    }

    #[test]
    fn english_alone_is_never_partial() {
        let s = pick(starters(), &langs(&["en"]));
        assert!(!is_partial(s, &langs(&["en"])));
        assert!(!is_partial(s, &[]));
        assert_eq!(heard_by(s, &[]), langs(&["en"]));
    }

    #[test]
    fn dutch_takes_wait_for_the_full_model() {
        let s = pick(starters(), &langs(&["en", "nl"]));
        // Known language.
        assert!(needs_full_model(s, Some("nl"), false));
        assert!(needs_full_model(s, Some("nl-NL"), true));
        assert!(!needs_full_model(s, Some("en"), false));
        assert!(!needs_full_model(s, Some("en-GB"), true));
        // Auto: only the practice take (English text) uses the starter.
        assert!(!needs_full_model(s, None, true));
        assert!(!needs_full_model(s, Some("auto"), true));
        assert!(needs_full_model(s, None, false));
        assert!(needs_full_model(s, Some("auto"), false));
        // French + Dutch: whisper-base never gets Dutch.
        let s = pick(starters(), &langs(&["fr", "nl"]));
        assert!(needs_full_model(s, Some("nl"), true));
        assert!(!needs_full_model(s, Some("fr"), false));
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
