// SPDX-License-Identifier: GPL-3.0-or-later
//! Fork LLM layer. F-08: the provider list ([`providers`]) and the request
//! guard Handy's `llm_client` builds every HTTP client through
//! ([`client_builder`]). S-10 adds the sidecar supervisor and the real
//! `local` transport; S-12a the cleanup pipeline.

pub mod providers;

#[cfg(test)]
mod tests;

pub use providers::default_providers;

use reqwest::header::{HeaderMap, REFERER, USER_AGENT};

use crate::settings::PostProcessProvider;

/// Headers Handy adds that name the app to the endpoint. Removed from every
/// request (R5 §4.5: neutral headers). Generic enough to send upstream as an
/// opt-out; it lives here so `llm_client.rs` keeps a one-line hook.
const APP_IDENTIFYING_HEADERS: &[&str] = &["x-title", "http-referer"];

/// Message of a refused request (DEC-04).
pub fn refusal_message(provider: &PostProcessProvider) -> String {
    let host = crate::fork::net::host_of(&provider.base_url).unwrap_or_else(|| "?".into());
    format!(
        "Request refused: provider '{}' points at {host}, which is not on this Mac. Wispr Bro only talks to loopback LLM endpoints.",
        provider.id
    )
}

/// Checks a provider before any network call: `Err` for the `local`
/// placeholder (no sidecar until S-10) and for any base URL that is not
/// loopback.
pub fn check_request(provider: &PostProcessProvider) -> Result<(), String> {
    if provider.id == providers::LOCAL_PROVIDER_ID {
        return Err("The local LLM is not available in this build yet (S-10).".into());
    }
    if !crate::fork::net::is_loopback(&provider.base_url) {
        return Err(refusal_message(provider));
    }
    Ok(())
}

/// Drops the headers that identify the app (`Referer`, `User-Agent`,
/// `X-Title`, `HTTP-Referer`). Auth and content headers stay.
pub fn neutral_headers(mut headers: HeaderMap) -> HeaderMap {
    headers.remove(REFERER);
    headers.remove(USER_AGENT);
    for name in APP_IDENTIFYING_HEADERS {
        headers.remove(*name);
    }
    headers
}

/// Hook in `llm_client::create_client` (replaces
/// `reqwest::Client::builder().default_headers(headers)`): refuses
/// non-loopback providers before any request ([`check_request`]), sends
/// neutral headers, and follows a redirect only to another loopback URL, so a
/// local server cannot bounce a transcript to a remote host.
pub fn client_builder(
    provider: &PostProcessProvider,
    headers: HeaderMap,
) -> Result<reqwest::ClientBuilder, String> {
    check_request(provider)?;
    Ok(reqwest::Client::builder()
        .default_headers(neutral_headers(headers))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() < 5 && crate::fork::net::is_loopback(attempt.url().as_str())
            {
                attempt.follow()
            } else {
                attempt.stop()
            }
        })))
}
