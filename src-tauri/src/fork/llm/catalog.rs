// SPDX-License-Identifier: GPL-3.0-or-later
//! The cleanup models the app may download (DEC-03, DEC-34, S-10).
//!
//! Every entry is pinned to an upstream Hugging Face repo **revision** and the
//! file's **sha256** (the LFS oid HF reports for that revision). The app
//! never asks Hugging Face for "latest", and a file whose hash differs is
//! deleted instead of loaded. The user never picks a model (R5 §1): the tier
//! rule below picks one by RAM.
//!
//! Values verified 2026-10-30 against `huggingface.co/api/models/<repo>/
//! paths-info/<revision>` (size and `lfs.oid`) and against the files used by
//! the RS-07 bake-off (`docs/fork/spikes/RS-07-cleanup-bakeoff.md`, same
//! sha256).

use serde::{Deserialize, Serialize};
use specta::Type;

/// How the model expects its prompt. Only the chat template is used today;
/// the field keeps tier changes (DEC-03 fallbacks) data-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PromptStyle {
    /// OpenAI chat messages through `/v1/chat/completions` with the GGUF's
    /// Jinja chat template, JSON schema output (RS-07 winner).
    ChatJsonSchema,
}

/// One downloadable GGUF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LlmModel {
    /// Stable id stored in logs and the status event.
    pub id: &'static str,
    /// Name shown in Settings › Advanced › AI engine.
    pub display_name: &'static str,
    /// Upstream Hugging Face repo (no mirror, DEC-34 as amended in CONTEXT).
    pub repo: &'static str,
    /// Full commit hash of the pinned revision.
    pub revision: &'static str,
    /// File name inside the repo.
    pub file: &'static str,
    /// Lower-case hex sha256 of the file.
    pub sha256: &'static str,
    /// File size in bytes.
    pub size_bytes: u64,
    /// SPDX id of the weights' licence (licence register).
    pub license: &'static str,
    /// Peak resident memory of `llama-server` with this model and the spawn
    /// line of [`super::process`] (`-c 4096 --cache-ram 256`), in MiB, as
    /// measured in RS-06 (phys_footprint). Feeds the memory policy (D-213).
    pub resident_mib: u64,
    pub prompt_style: PromptStyle,
}

/// Muesli 2B v12, Q4_K_M: the cleanup model on every Apple Silicon tier
/// (DEC-03 update 2026-10-29, owner decision after RS-07).
pub const MUESLI_2B_V12: LlmModel = LlmModel {
    id: "muesli-2b-v12-q4km",
    display_name: "Muesli 2B v12",
    repo: "Muesli-HQ/qwen35-2b-muesli-cleanup-v12-gguf",
    revision: "e35af097be4f17d3673515fb321ce19aa300faf6",
    file: "qwen35-2b-muesli-v12-Q4_K_M.gguf",
    sha256: "2d59d2118170e0170fa294d71eba0a701c4f26063f284f68a8bed880322c4143",
    size_bytes: 1_274_396_096,
    license: "Apache-2.0",
    resident_mib: 2_100,
    prompt_style: PromptStyle::ChatJsonSchema,
};

/// Qwen3.5-2B, Q4_K_M: the DEC-03 fallback (same RAM class). Not selected
/// by the tier rule today; kept pinned so a tier change is data only.
pub const QWEN35_2B: LlmModel = LlmModel {
    id: "qwen3.5-2b-q4km",
    display_name: "Qwen3.5 2B",
    repo: "unsloth/Qwen3.5-2B-GGUF",
    revision: "f6d5376be1edb4d416d56da11e5397a961aca8ae",
    file: "Qwen3.5-2B-Q4_K_M.gguf",
    sha256: "aaf42c8b7c3cab2bf3d69c355048d4a0ee9973d48f16c731c0520ee914699223",
    size_bytes: 1_280_835_840,
    license: "Apache-2.0",
    resident_mib: 2_050,
    prompt_style: PromptStyle::ChatJsonSchema,
};

/// Every model the app knows. The licence register
/// (`docs/fork/LICENSE-REGISTER.md`) lists each one.
pub const ALL: &[LlmModel] = &[MUESLI_2B_V12, QWEN35_2B];

/// CPU family the tier rule distinguishes (DEC-29).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cpu {
    AppleSilicon,
    Other,
}

impl Cpu {
    /// The CPU this binary runs on.
    pub fn current() -> Self {
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Cpu::AppleSilicon
        } else {
            Cpu::Other
        }
    }
}

/// The model for a Mac with `physical_ram_bytes` of memory, or `None` when
/// the local LLM is off for this machine (Intel, DEC-29, and anything under
/// 8 GB, which macOS no longer ships on Apple Silicon).
///
/// DEC-03 (update 2026-10-29): Muesli 2B v12 on every Apple Silicon tier,
/// pending RS-06 on an M1 8 GB. If RS-06 shows the 8 GB tier misses its
/// budget, R5 §5.1 says to use the next smaller model for that tier; that is
/// a change to this function only.
pub fn select(physical_ram_bytes: u64, cpu: Cpu) -> Option<&'static LlmModel> {
    const MIN_RAM: u64 = 8 * 1024 * 1024 * 1024;
    match cpu {
        Cpu::AppleSilicon if physical_ram_bytes >= MIN_RAM - 64 * 1024 * 1024 => {
            Some(&MUESLI_2B_V12)
        }
        _ => None,
    }
}

/// Looks a model up by id.
pub fn by_id(id: &str) -> Option<&'static LlmModel> {
    ALL.iter().find(|m| m.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1024 * 1024 * 1024;

    #[test]
    fn every_entry_is_pinned_by_revision_and_sha256() {
        for m in ALL {
            assert_eq!(m.revision.len(), 40, "{}: full commit hash", m.id);
            assert!(m.revision.chars().all(|c| c.is_ascii_hexdigit()));
            assert_eq!(m.sha256.len(), 64, "{}: sha256", m.id);
            assert!(m
                .sha256
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
            assert!(m.file.ends_with(".gguf"));
            assert!(m.repo.contains('/') && !m.repo.contains(".."));
            assert!(m.size_bytes > 100 * 1024 * 1024);
            // DEC-27: a silent default needs a licence the filter passes.
            assert_eq!(
                crate::fork::models::license_filter::restriction_for(m.id, m.license),
                None,
                "{}: licence {}",
                m.id,
                m.license
            );
            assert_eq!(m.license, "Apache-2.0", "{}: licence register", m.id);
            assert_eq!(by_id(m.id), Some(m));
        }
        let mut ids: Vec<_> = ALL.iter().map(|m| m.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), ALL.len());
    }

    #[test]
    fn dec03_muesli_on_every_apple_silicon_tier() {
        for ram in [8 * GB, 16 * GB, 24 * GB, 36 * GB, 128 * GB] {
            assert_eq!(select(ram, Cpu::AppleSilicon), Some(&MUESLI_2B_V12));
        }
        // macOS reports a few MB less than the marketing size on some Macs.
        assert!(select(8 * GB - 16 * 1024 * 1024, Cpu::AppleSilicon).is_some());
        assert_eq!(select(4 * GB, Cpu::AppleSilicon), None);
    }

    #[test]
    fn dec29_no_local_llm_on_intel() {
        assert_eq!(select(64 * GB, Cpu::Other), None);
    }

    #[test]
    fn the_hf_url_of_every_entry_is_on_the_allowlist() {
        for m in ALL {
            let url = super::super::download::resolve_url(m);
            assert!(crate::fork::net::is_allowed_host(&url), "{url}");
            assert!(url.contains(m.revision), "{url}");
        }
    }
}
