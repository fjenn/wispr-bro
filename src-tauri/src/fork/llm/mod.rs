// SPDX-License-Identifier: GPL-3.0-or-later
//! Fork LLM layer.
//!
//! - F-08: the provider list ([`providers`]) and the request guard Handy's
//!   `llm_client` builds every HTTP client through ([`client_builder`]).
//! - S-10: the bundled `llama-server` sidecar ([`supervisor`], [`process`],
//!   [`transport`], [`health`]), the pinned cleanup models ([`catalog`],
//!   [`download`]), the memory policy and ordered loads ([`memory`]), the
//!   chat client ([`chat`]), the OpenWhispr prompt seed ([`prompt`]) and the
//!   app wiring ([`runtime`]). The `local` provider now reaches the sidecar
//!   (Unix socket or loopback TCP with a key, DEC-49).
//! - S-12a adds the cleanup pipeline and guards.

pub mod catalog;
pub mod chat;
pub mod download;
pub mod health;
pub mod memory;
pub mod process;
pub mod prompt;
pub mod providers;
#[cfg(test)]
mod rs06;
pub mod runtime;
pub mod supervisor;
pub mod transport;

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

/// The transport of the app's running sidecar, if any. Set by the
/// supervisor ([`supervisor::Supervisor`] with `publish_transport`); read by
/// [`client_builder`] for the `local` provider.
static LOCAL_TRANSPORT: std::sync::Mutex<Option<transport::Transport>> =
    std::sync::Mutex::new(None);

/// Records (or clears) the running sidecar's transport.
pub fn set_local_transport(t: Option<transport::Transport>) {
    *LOCAL_TRANSPORT.lock().unwrap_or_else(|p| p.into_inner()) = t;
}

fn local_transport() -> Option<transport::Transport> {
    LOCAL_TRANSPORT
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
}

/// Message when `local` is selected but the sidecar is not running. Handy's
/// callers turn it into "use the original transcript".
pub const LOCAL_NOT_RUNNING: &str = "The local LLM is not running (no model yet, or stopped).";

/// Checks a provider before any network call: `Err` for any base URL that is
/// not loopback, and for `local` while the sidecar is not running.
pub fn check_request(provider: &PostProcessProvider) -> Result<(), String> {
    if !crate::fork::net::is_loopback(&provider.base_url) {
        return Err(refusal_message(provider));
    }
    if provider.id == providers::LOCAL_PROVIDER_ID && local_transport().is_none() {
        return Err(LOCAL_NOT_RUNNING.into());
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
/// local server cannot bounce a transcript to a remote host. For `local` it
/// points the client at the sidecar (S-10: Unix socket or the loopback port,
/// with the sidecar's key in place of any stored API key).
pub fn client_builder(
    provider: &PostProcessProvider,
    headers: HeaderMap,
) -> Result<reqwest::ClientBuilder, String> {
    check_request(provider)?;
    let mut headers = neutral_headers(headers);
    let mut builder = reqwest::Client::builder();
    if provider.id == providers::LOCAL_PROVIDER_ID {
        let t = local_transport().ok_or_else(|| LOCAL_NOT_RUNNING.to_string())?;
        headers.insert(reqwest::header::AUTHORIZATION, t.auth_header());
        builder = t.configure(builder);
    }
    Ok(builder
        .default_headers(headers)
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
