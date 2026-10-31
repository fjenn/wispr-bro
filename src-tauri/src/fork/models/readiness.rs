// SPDX-License-Identifier: GPL-3.0-or-later
// Ported from SpeakoFlow `src-tauri/src/speech_readiness.rs`
// (https://github.com/AbhishekBarali/SpeakoFlow, commit
// 9818a17fcd124e5f1ea6d34780ddea9b154322d8), MIT, Copyright (c) 2026
// Abhishek Barali (LICENSES/MIT-SpeakoFlow.txt).
// Modified by Wispr Bro contributors, 2026-10-31: no cloud engine; a press
// that cannot be transcribed yet is recorded and queued instead of refused
// (DEC-01), so the decision returns what to do rather than drawing the pill;
// a stale selection names the model to switch to; retired models come from
// Handy's ONNX list. See docs/fork/PORTS.md.
//! Whether dictation can be turned into text right now (S-07a).
//!
//! SpeakoFlow refuses a press while the speech model downloads; Wispr Bro
//! records it and transcribes it when a model is ready (DEC-01), so this
//! only answers "is there a model to transcribe with, and if not, is one
//! coming". The decision is a pure function over a snapshot ([`resolve`]),
//! so every case is a unit test; `fork::models::setup` reads the snapshot
//! from Handy's model manager and acts on the answer.

/// What the fork does about speech models right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpeechReadiness {
    /// The selected model is on disk: transcribe as Handy does.
    Ready { model_id: String },
    /// The selection is missing or stale but another usable model is on
    /// disk: switch to it (no download needed).
    UseOther { model_id: String },
    /// A speech model is on its way.
    Downloading { model_id: String },
    /// No usable speech model on disk and none coming: download one.
    Missing,
}

/// The parts of one of Handy's models the decision reads.
#[derive(Debug, Clone, Copy)]
pub struct ModelFacts<'a> {
    pub id: &'a str,
    pub is_downloaded: bool,
    pub is_downloading: bool,
    /// Its engine is gone (Handy's retired ONNX models): never usable,
    /// whatever is on disk.
    pub is_retired: bool,
}

/// Decide from a snapshot. `selected` is Handy's `selected_model` setting.
///
/// Order matters (SpeakoFlow's rules without the cloud case):
/// 1. A downloaded, non-retired selection is the normal case.
/// 2. A download in flight wins over "another model is on disk": the setup
///    selects a model only once its file has landed.
/// 3. Any other downloaded, non-retired model means the selection is stale:
///    switch to it.
/// 4. Nothing: download one.
pub fn resolve(selected: &str, models: &[ModelFacts<'_>]) -> SpeechReadiness {
    let usable = || models.iter().filter(|m| !m.is_retired);

    if !selected.is_empty()
        && usable().any(|m| m.id == selected && m.is_downloaded && !m.is_downloading)
    {
        return SpeechReadiness::Ready {
            model_id: selected.to_string(),
        };
    }
    let downloading = usable()
        .find(|m| m.id == selected && m.is_downloading)
        .or_else(|| usable().find(|m| m.is_downloading));
    if let Some(model) = downloading {
        return SpeechReadiness::Downloading {
            model_id: model.id.to_string(),
        };
    }
    if let Some(model) = usable().find(|m| m.is_downloaded) {
        return SpeechReadiness::UseOther {
            model_id: model.id.to_string(),
        };
    }
    SpeechReadiness::Missing
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(id: &str, downloaded: bool, downloading: bool) -> ModelFacts<'_> {
        ModelFacts {
            id,
            is_downloaded: downloaded,
            is_downloading: downloading,
            is_retired: false,
        }
    }

    fn retired(id: &str) -> ModelFacts<'_> {
        ModelFacts {
            is_retired: true,
            ..model(id, true, false)
        }
    }

    fn ready(id: &str) -> SpeechReadiness {
        SpeechReadiness::Ready {
            model_id: id.into(),
        }
    }

    fn downloading(id: &str) -> SpeechReadiness {
        SpeechReadiness::Downloading {
            model_id: id.into(),
        }
    }

    fn use_other(id: &str) -> SpeechReadiness {
        SpeechReadiness::UseOther {
            model_id: id.into(),
        }
    }

    #[test]
    fn downloaded_selection_is_ready() {
        let models = [model("parakeet", true, false)];
        assert_eq!(resolve("parakeet", &models), ready("parakeet"));
    }

    #[test]
    fn selection_still_downloading_is_reported() {
        let models = [model("parakeet", false, true)];
        assert_eq!(resolve("parakeet", &models), downloading("parakeet"));
    }

    #[test]
    fn a_selection_mid_redownload_is_not_ready() {
        // Handy keeps `is_downloaded` from the last scan while a download of
        // the same file runs; the file is not complete.
        let models = [model("parakeet", true, true)];
        assert_eq!(resolve("parakeet", &models), downloading("parakeet"));
    }

    #[test]
    fn first_run_download_behind_an_empty_selection() {
        let models = [
            model("parakeet", false, false),
            model("starter", false, true),
        ];
        assert_eq!(resolve("", &models), downloading("starter"));
    }

    #[test]
    fn the_selected_download_is_preferred_over_another() {
        let models = [
            model("nemotron", false, true),
            model("parakeet", false, true),
        ];
        assert_eq!(resolve("parakeet", &models), downloading("parakeet"));
    }

    #[test]
    fn a_download_wins_over_a_stale_selection() {
        let models = [
            model("whisper-small", true, false),
            model("parakeet", false, true),
        ];
        assert_eq!(resolve("parakeet", &models), downloading("parakeet"));
    }

    #[test]
    fn a_stale_selection_switches_to_the_model_on_disk() {
        // Handy clears a selection whose file is gone; any model on disk is
        // better than a download.
        let models = [
            model("parakeet", false, false),
            model("whisper-small", true, false),
        ];
        assert_eq!(resolve("parakeet", &models), use_other("whisper-small"));
        assert_eq!(resolve("", &models), use_other("whisper-small"));
    }

    #[test]
    fn nothing_on_disk_and_nothing_coming_is_missing() {
        let models = [
            model("parakeet", false, false),
            model("nemotron", false, false),
        ];
        assert_eq!(resolve("parakeet", &models), SpeechReadiness::Missing);
        assert_eq!(resolve("", &[]), SpeechReadiness::Missing);
    }

    #[test]
    fn a_retired_selection_is_never_ready() {
        let models = [retired("old-onnx")];
        assert_eq!(resolve("old-onnx", &models), SpeechReadiness::Missing);
        // …and is not a model to switch to either.
        let models = [retired("old-onnx"), model("whisper-base", true, false)];
        assert_eq!(resolve("old-onnx", &models), use_other("whisper-base"));
    }
}
