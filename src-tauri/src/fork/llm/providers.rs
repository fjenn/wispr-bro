// SPDX-License-Identifier: GPL-3.0-or-later
//! The post-processing provider list (DEC-04, R5 §4.5 W-02).
//!
//! Hooked into Handy's `settings::default_post_process_providers`, so it is
//! the list Handy merges into every settings read. It holds no cloud
//! provider:
//!
//! - `local`: the fork's own LLM. A placeholder until S-10 ships the
//!   `llama-server` sidecar; the request guard ([`super::client_builder`])
//!   refuses it before any network call, and Handy's post-processing falls
//!   back to the raw transcript.
//! - Apple Intelligence (macOS on Apple Silicon): on device, never reaches
//!   `llm_client`.
//! - `custom`: Handy's OpenAI-compatible endpoint, loopback only (Ollama).
//!
//! Providers already stored by an older build (OpenAI, Anthropic, …) are
//! dropped by `fork::settings::apply_product_settings` (`ENFORCED`), which
//! also moves a cloud selection to `local`.

use crate::settings::{PostProcessApiFormat, PostProcessProvider};

/// Id of the fork's local LLM provider.
pub const LOCAL_PROVIDER_ID: &str = "local";
/// Id of Handy's custom (OpenAI-compatible) provider.
pub const CUSTOM_PROVIDER_ID: &str = "custom";
/// Placeholder base URL of the `local` provider until S-10 sets the real
/// transport (Unix socket or a random loopback port, DEC-49). Loopback, so
/// even a future bug in the refusal could not leave the Mac.
pub const LOCAL_PLACEHOLDER_BASE_URL: &str = "http://127.0.0.1/v1";
/// Handy's default for the custom provider: Ollama on loopback.
pub const LOOPBACK_CUSTOM_BASE_URL: &str = "http://localhost:11434/v1";

/// The built-in provider list (Handy hook: `default_post_process_providers`).
pub fn default_providers() -> Vec<PostProcessProvider> {
    let mut providers = vec![PostProcessProvider {
        id: LOCAL_PROVIDER_ID.to_string(),
        label: "Local".to_string(),
        base_url: LOCAL_PLACEHOLDER_BASE_URL.to_string(),
        allow_base_url_edit: false,
        models_endpoint: None,
        supports_structured_output: true,
        api_format: PostProcessApiFormat::OpenAi,
    }];

    // Same entry as Handy's (no availability check at startup; see the note
    // in Handy's `default_post_process_providers`).
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    providers.push(PostProcessProvider {
        id: crate::settings::APPLE_INTELLIGENCE_PROVIDER_ID.to_string(),
        label: "Apple Intelligence".to_string(),
        base_url: "apple-intelligence://local".to_string(),
        allow_base_url_edit: false,
        models_endpoint: None,
        supports_structured_output: true,
        api_format: PostProcessApiFormat::OpenAi,
    });

    // Custom provider always comes last (Handy's order).
    providers.push(PostProcessProvider {
        id: CUSTOM_PROVIDER_ID.to_string(),
        label: "Custom".to_string(),
        base_url: LOOPBACK_CUSTOM_BASE_URL.to_string(),
        allow_base_url_edit: true,
        models_endpoint: Some("/models".to_string()),
        supports_structured_output: false,
        api_format: PostProcessApiFormat::OpenAi,
    });

    providers
}

/// `true` for an id in [`default_providers`].
pub fn is_builtin(id: &str) -> bool {
    default_providers().iter().any(|p| p.id == id)
}

/// `true` for a provider that keeps text on this Mac: `local`, Apple
/// Intelligence (on device), or any provider whose base URL is loopback.
pub fn is_on_device(provider: &PostProcessProvider) -> bool {
    provider.id == LOCAL_PROVIDER_ID
        || provider.id == crate::settings::APPLE_INTELLIGENCE_PROVIDER_ID
        || crate::fork::net::is_loopback(&provider.base_url)
}
