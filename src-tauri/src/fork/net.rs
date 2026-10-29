// SPDX-License-Identifier: GPL-3.0-or-later
//! Network allowlist (R5 §1 privacy promise, DEC-04, DEC-16).
//!
//! The app may contact exactly two places: GitHub releases (update check and
//! download) and Hugging Face (model downloads). Everything else is either
//! loopback (the user's own Ollama through the custom provider, the fork's
//! `local` LLM sidecar from S-10) or refused.
//!
//! - [`is_allowed_host`]: an `https` URL on one of the §1 hosts.
//! - [`is_loopback`]: an `http(s)` URL on `localhost` or a loopback IP.
//! - [`mirror_base`]: the hook in Handy's mirror URL builder
//!   (`catalog::mirror_files`). Returns `None` for every mirror that is not on
//!   the allowlist, so `https://blob.handy.computer/...` URLs are never built.
//! - [`checked_download_url`]: the hook in `ModelManager::download_model` for
//!   Handy's legacy URL-sourced models, which point at the same mirror.

use reqwest::Url;

/// Hosts the app may contact, matched exactly (R5 §1).
pub const ALLOWED_HOSTS: &[&str] = &[
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
    "huggingface.co",
];

/// Host suffixes the app may contact: Hugging Face's download CDNs
/// (`cdn-lfs.hf.co`, `cas-bridge.xethub.hf.co`, `cdn-lfs-us-1.huggingface.co`, …).
pub const ALLOWED_HOST_SUFFIXES: &[&str] = &[".hf.co", ".huggingface.co"];

/// Handy's static model mirror. DEC-16 drops it; nothing may build a URL on it.
pub const HANDY_MIRROR_HOST: &str = "blob.handy.computer";

/// Lower-cased host of an absolute URL, or `None` if it does not parse or has
/// no host.
pub fn host_of(url: &str) -> Option<String> {
    let parsed = Url::parse(url.trim()).ok()?;
    parsed.host_str().map(|h| h.to_ascii_lowercase())
}

/// `true` when `host` is on the §1 allowlist.
pub fn is_allowed_hostname(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    ALLOWED_HOSTS.contains(&host.as_str())
        || ALLOWED_HOST_SUFFIXES
            .iter()
            .any(|suffix| host.ends_with(suffix) && host.len() > suffix.len())
}

/// `true` for an `https` URL, without credentials, whose host is on the §1
/// allowlist (GitHub releases, Hugging Face and its CDNs).
pub fn is_allowed_host(url: &str) -> bool {
    let Ok(parsed) = Url::parse(url.trim()) else {
        return false;
    };
    parsed.scheme() == "https"
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.host_str().is_some_and(is_allowed_hostname)
}

/// `true` for an `http(s)` URL, without credentials, whose host is
/// `localhost` or a loopback IP (`127.0.0.0/8`, `::1`).
///
/// Two independent parsers must agree: a plain split of the authority (no
/// normalisation) and the WHATWG parser reqwest uses to connect. A URL the two
/// read differently (`http://0x7f000001/`, tabs, backslashes, `user@host`) is
/// not loopback, so the guard can only err on the side of refusing.
pub fn is_loopback(url: &str) -> bool {
    literal_loopback(url) && whatwg_loopback(url)
}

fn literal_loopback(url: &str) -> bool {
    let Some((scheme, rest)) = url.trim().split_once("://") else {
        return false;
    };
    if !matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https") {
        return false;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.contains(['@', '\\']) || authority.chars().any(char::is_whitespace) {
        return false;
    }
    let host = if let Some(v6) = authority.strip_prefix('[') {
        match v6.split_once(']') {
            Some((h, _)) => h,
            None => return false,
        }
    } else {
        authority.split(':').next().unwrap_or_default()
    };
    let host = host.to_ascii_lowercase();
    host == "localhost"
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

fn whatwg_loopback(url: &str) -> bool {
    let Ok(parsed) = Url::parse(url.trim()) else {
        return false;
    };
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return false;
    }
    // The WHATWG parser normalises IPv4 forms to dotted decimal and keeps
    // IPv6 hosts in brackets.
    let Some(host) = parsed.host_str() else {
        return false;
    };
    let host = host.trim_start_matches('[').trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

/// Hook in Handy's mirror URL builder (`catalog::mirror_files`): the mirror
/// base URL if it is on the allowlist, else `None`. Handy's only mirror,
/// `https://blob.handy.computer`, is not, so no mirror URL is ever built and
/// a failed Hugging Face download is reported instead of retried elsewhere.
pub fn mirror_base(base: &str) -> Option<&str> {
    is_allowed_host(base).then_some(base)
}

/// Hook in `ModelManager::download_model` for URL-sourced models: the URL if
/// it is on the allowlist, else an error before any request. Handy's five
/// legacy Whisper/Breeze entries point at `blob.handy.computer` and fail here;
/// the same models are offered from Hugging Face by the catalog.
pub fn checked_download_url(url: &str) -> anyhow::Result<String> {
    if is_allowed_host(url) {
        Ok(url.to_string())
    } else {
        let host = host_of(url).unwrap_or_else(|| "an unparseable URL".into());
        Err(anyhow::anyhow!(
            "Download refused: {host} is not an allowed host (Wispr Bro downloads models only from Hugging Face)"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_matches_the_privacy_promise() {
        for url in [
            "https://github.com/owner/wispr-bro/releases/latest/download/latest.json",
            "https://objects.githubusercontent.com/x",
            "https://release-assets.githubusercontent.com/x",
            "https://huggingface.co/handy-computer/whisper-small-gguf/resolve/abc/f.gguf",
            "https://cdn-lfs.hf.co/repos/x",
            "https://cas-bridge.xethub.hf.co/x",
            "https://cdn-lfs-us-1.huggingface.co/x",
            "https://HuggingFace.co/x",
        ] {
            assert!(is_allowed_host(url), "{url}");
        }
        for url in [
            "http://huggingface.co/x",
            "https://blob.handy.computer/ggml-small.bin",
            "https://handy.computer/",
            "https://api.openai.com/v1",
            "https://hf.co.evil.com/x",
            "https://evilhuggingface.co/x",
            "https://hf.co/x",
            "https://user:pw@huggingface.co/x",
            "https://github.com.evil.com/",
            "ftp://github.com/",
            "not a url",
            "",
        ] {
            assert!(!is_allowed_host(url), "{url}");
        }
    }

    #[test]
    fn loopback_requires_both_parsers_to_agree() {
        for url in [
            "http://localhost:11434/v1",
            "http://LOCALHOST/v1",
            "http://127.0.0.1:8080",
            "https://127.1.2.3/v1",
            "http://[::1]:11434/v1",
        ] {
            assert!(is_loopback(url), "{url}");
        }
        for url in [
            "https://api.openai.com/v1",
            "http://localhost.evil.com/v1",
            "http://127.0.0.1.nip.io/v1",
            "http://0x7f000001:11434/v1",
            "http://2130706433/v1",
            "http://user@127.0.0.1/v1",
            "http://127.0.0.1\\@evil.com/v1",
            "http://local\thost/v1",
            "http://[::ffff:8.8.8.8]/v1",
            "http://10.0.0.5:11434/v1",
            "file:///etc/passwd",
            "apple-intelligence://local",
            "",
        ] {
            assert!(!is_loopback(url), "{url}");
        }
    }

    #[test]
    fn the_handy_mirror_is_never_a_mirror_base() {
        assert_eq!(mirror_base("https://blob.handy.computer"), None);
        assert_eq!(mirror_base("https://blob.handy.computer/"), None);
        assert_eq!(
            mirror_base("https://huggingface.co"),
            Some("https://huggingface.co")
        );
        assert!(checked_download_url("https://blob.handy.computer/ggml-small.bin").is_err());
        assert!(checked_download_url("https://huggingface.co/a/b").is_ok());
    }
}
