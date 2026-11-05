// SPDX-License-Identifier: GPL-3.0-or-later
//! The post-processing provider list (DEC-04, R5 §4.5 W-02).
//!
//! Hooked into Handy's `settings::default_post_process_providers`, so it is
//! the list Handy merges into every settings read. It holds no cloud
//! provider:
//!
//! - `local`: the fork's own LLM, the bundled `llama-server` sidecar (S-10).
//!   Its base URL is [`LOCAL_BASE_URL`]; [`super::client_builder`] routes the
//!   request to the sidecar's Unix socket or loopback port (DEC-49) and
//!   refuses it before any network call while the sidecar is not running, so
//!   Handy's post-processing falls back to the raw transcript.
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
/// Base URL of the `local` provider. The host is a name only: the client is
/// bound to the sidecar's Unix socket, or resolves `localhost` to the
/// sidecar's random loopback port (`transport::Transport::configure`).
/// Loopback either way, so a bug in the routing still could not leave the Mac.
pub const LOCAL_BASE_URL: &str = "http://localhost/v1";
/// Handy's default for the custom provider: Ollama on loopback.
pub const LOOPBACK_CUSTOM_BASE_URL: &str = "http://localhost:11434/v1";

/// The built-in provider list (Handy hook: `default_post_process_providers`).
pub fn default_providers() -> Vec<PostProcessProvider> {
    let mut providers = vec![PostProcessProvider {
        id: LOCAL_PROVIDER_ID.to_string(),
        label: "Local".to_string(),
        base_url: LOCAL_BASE_URL.to_string(),
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
