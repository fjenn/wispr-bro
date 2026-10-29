// SPDX-License-Identifier: GPL-3.0-or-later
//! F-08 tests: provider list, backend refusal before any request, neutral
//! headers on the wire.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::providers::{self, CUSTOM_PROVIDER_ID, LOCAL_PROVIDER_ID};
use crate::settings::{PostProcessApiFormat, PostProcessProvider};

fn custom(base_url: &str) -> PostProcessProvider {
    PostProcessProvider {
        id: CUSTOM_PROVIDER_ID.into(),
        label: "Custom".into(),
        base_url: base_url.into(),
        allow_base_url_edit: true,
        models_endpoint: Some("/models".into()),
        supports_structured_output: false,
        api_format: PostProcessApiFormat::OpenAi,
    }
}

/// A loopback listener that records whether anything connected and, if so,
/// the raw request head; answers with one chat completion.
async fn recording_server() -> (u16, tokio::task::JoinHandle<Option<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        let accepted = tokio::time::timeout(Duration::from_millis(1500), listener.accept()).await;
        let Ok(Ok((mut socket, _))) = accepted else {
            return None;
        };
        let mut buf = vec![0u8; 16 * 1024];
        let mut read = 0;
        loop {
            let n = socket.read(&mut buf[read..]).await.unwrap_or(0);
            read += n;
            if n == 0 || buf[..read].windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        let body = r#"{"choices":[{"message":{"content":"ok"},"finish_reason":"stop"}]}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        Some(String::from_utf8_lossy(&buf[..read]).to_string())
    });
    (port, handle)
}

#[test]
fn default_list_has_no_cloud_provider() {
    let list = providers::default_providers();
    assert_eq!(list.first().map(|p| p.id.as_str()), Some(LOCAL_PROVIDER_ID));
    assert_eq!(list.last().map(|p| p.id.as_str()), Some(CUSTOM_PROVIDER_ID));
    for p in &list {
        assert!(providers::is_on_device(p), "{} is not on device", p.id);
    }
    let local = &list[0];
    assert!(local.supports_structured_output);
    assert!(crate::fork::net::is_loopback(&local.base_url));
    assert!(local.models_endpoint.is_none());
}

#[test]
fn handy_settings_defaults_come_from_the_fork_list() {
    let settings = crate::settings::get_default_settings();
    let ids: Vec<_> = settings
        .post_process_providers
        .iter()
        .map(|p| p.id.clone())
        .collect();
    let fork_ids: Vec<_> = providers::default_providers()
        .into_iter()
        .map(|p| p.id)
        .collect();
    assert_eq!(ids, fork_ids);
    for id in [
        "openai",
        "anthropic",
        "openrouter",
        "groq",
        "cerebras",
        "zai",
        "bedrock_mantle",
    ] {
        assert!(!ids.iter().any(|i| i == id), "{id} still offered");
        assert!(
            !settings.post_process_api_keys.contains_key(id),
            "{id} key slot"
        );
    }
    let src = include_str!("../../settings.rs");
    assert!(src.contains("crate::fork::llm::default_providers()"));
}

#[test]
fn refusal_covers_cloud_and_disguised_hosts_and_the_local_placeholder() {
    for url in [
        "https://api.openai.com/v1",
        "https://api.anthropic.com/v1",
        "http://192.168.1.10:11434/v1",
        "http://0x7f000001:11434/v1",
    ] {
        let err = super::check_request(&custom(url)).unwrap_err();
        assert!(err.contains("refused"), "{url}: {err}");
    }
    assert!(super::check_request(&custom("http://localhost:11434/v1")).is_ok());
    assert!(super::check_request(&providers::default_providers()[0]).is_err());
}

#[test]
fn neutral_headers_drop_app_identifying_headers_only() {
    use reqwest::header::{
        HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, REFERER, USER_AGENT,
    };
    let mut h = HeaderMap::new();
    h.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    h.insert(
        REFERER,
        HeaderValue::from_static("https://github.com/cjpais/Handy"),
    );
    h.insert(USER_AGENT, HeaderValue::from_static("Handy/1.0"));
    h.insert("X-Title", HeaderValue::from_static("Handy"));
    h.insert(AUTHORIZATION, HeaderValue::from_static("Bearer k"));
    let h = super::neutral_headers(h);
    assert!(h.get(REFERER).is_none() && h.get(USER_AGENT).is_none());
    assert!(h.get("x-title").is_none());
    assert!(h.get(CONTENT_TYPE).is_some() && h.get(AUTHORIZATION).is_some());
}

/// DEC-04: a non-loopback custom URL is refused by the backend before any
/// request. `0x7f000001` is 127.0.0.1 to reqwest's URL parser, so without the
/// guard the request would land on the listener; it must not.
#[tokio::test]
async fn non_loopback_custom_url_is_refused_before_any_request() {
    let (port, server) = recording_server().await;
    let provider = custom(&format!("http://0x7f000001:{port}/v1"));
    let result = crate::llm_client::send_chat_completion(
        &provider,
        String::new(),
        "m",
        "secret".into(),
        false,
    )
    .await;
    let err = result.unwrap_err();
    assert!(err.contains("refused"), "{err}");
    assert_eq!(
        server.await.unwrap(),
        None,
        "a connection reached the server"
    );

    for base in ["https://api.openai.com/v1", "https://openrouter.ai/api/v1"] {
        let p = custom(base);
        assert!(
            crate::llm_client::send_chat_completion(&p, "k".into(), "m", "x".into(), false)
                .await
                .unwrap_err()
                .contains("refused")
        );
        assert!(crate::llm_client::fetch_models(&p, "k".into())
            .await
            .unwrap_err()
            .contains("refused"));
    }
}

#[tokio::test]
async fn selecting_local_sends_nothing_and_does_not_panic() {
    let local = providers::default_providers().remove(0);
    let err = crate::llm_client::send_chat_completion_with_schema(
        &local,
        String::new(),
        "m",
        "secret".into(),
        None,
        None,
        true,
    )
    .await
    .unwrap_err();
    assert!(err.contains("not available"), "{err}");
}

#[tokio::test]
async fn loopback_requests_carry_neutral_headers() {
    let (port, server) = recording_server().await;
    let provider = custom(&format!("http://127.0.0.1:{port}/v1"));
    let out = crate::llm_client::send_chat_completion(
        &provider,
        "key".into(),
        "m",
        "hello".into(),
        false,
    )
    .await
    .unwrap();
    assert_eq!(out.as_deref(), Some("ok"));
    let head = server.await.unwrap().expect("request reached the server");
    let lower = head.to_ascii_lowercase();
    assert!(!lower.contains("\r\nreferer:"), "{head}");
    assert!(!lower.contains("\r\nx-title:"), "{head}");
    assert!(!lower.contains("\r\nuser-agent:"), "{head}");
    assert!(!lower.contains("handy"), "{head}");
    assert!(lower.contains("authorization: bearer key"));
}
