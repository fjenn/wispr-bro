// SPDX-License-Identifier: GPL-3.0-or-later
//! Update channel tests on a loopback stand-in for GitHub releases.
//!
//! The fixture (`testdata/`) is a tiny `.app.tar.gz` signed with a throwaway
//! test key (`test-updater.key.pub`; its private half was discarded). The
//! real plugin code fetches `latest.json`, compares versions, downloads and
//! verifies the signature; nothing is installed.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};

use reqwest::Url;
use serde_json::json;
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::App;

use super::channel::{endpoints_on, BETA_PATH, STABLE_PATH};
use super::*;
use crate::fork::settings::{FakeHandyStore, ForkSettingsHandle};

const ARTIFACT: &[u8] = include_bytes!("testdata/update.app.tar.gz");
const SIGNATURE: &str = include_str!("testdata/update.app.tar.gz.sig");
const TEST_PUBKEY: &str = include_str!("testdata/test-updater.key.pub");

type Routes = Arc<Mutex<HashMap<String, (u16, Vec<u8>)>>>;

/// A loopback HTTP server answering GETs from `routes` (404 otherwise).
fn serve() -> (String, Routes) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let routes: Routes = Arc::default();
    let shared = routes.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { return };
            let routes = shared.clone();
            std::thread::spawn(move || {
                let mut buf = vec![0u8; 8192];
                let n = s.read(&mut buf).unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]);
                let path = head.split_whitespace().nth(1).unwrap_or("/").to_string();
                let (status, body) = routes
                    .lock()
                    .unwrap()
                    .get(&path)
                    .cloned()
                    .unwrap_or((404, b"Not Found".to_vec()));
                let reason = if status == 200 { "OK" } else { "Not Found" };
                let _ = write!(
                    s,
                    "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = s.write_all(&body);
            });
        }
    });
    (base, routes)
}

/// `latest.json` for `version` whose every platform points at `url`.
fn manifest(version: &str, url: &str, signature: &str) -> Vec<u8> {
    let platform = json!({ "signature": signature, "url": url });
    let mut platforms = serde_json::Map::new();
    for key in [
        "darwin-aarch64",
        "darwin-aarch64-app",
        "darwin-x86_64",
        "darwin-x86_64-app",
        "linux-x86_64",
        "windows-x86_64",
    ] {
        platforms.insert(key.into(), platform.clone());
    }
    serde_json::to_vec(&json!({
        "version": version,
        "notes": format!("Wispr Bro {version}"),
        "pub_date": "2026-11-02T12:00:00Z",
        "platforms": platforms,
    }))
    .unwrap()
}

struct Stand {
    repo: String,
    routes: Routes,
}

impl Stand {
    fn new() -> Self {
        let (base, routes) = serve();
        let stand = Stand {
            repo: format!("{base}/fjenn/wispr-bro"),
            routes,
        };
        stand.put("/artifact.app.tar.gz", ARTIFACT.to_vec());
        stand
    }
    fn put(&self, path: &str, body: Vec<u8>) {
        let path = path.strip_prefix(&self.repo).unwrap_or(path).to_string();
        let path = if path.starts_with("/fjenn") {
            path
        } else {
            format!("/fjenn/wispr-bro{path}")
        };
        self.routes.lock().unwrap().insert(path, (200, body));
    }
    fn artifact_url(&self) -> String {
        format!("{}/artifact.app.tar.gz", self.repo)
    }
    fn release(&self, path: &str, version: &str) {
        self.put(path, manifest(version, &self.artifact_url(), SIGNATURE));
    }
    fn endpoints(&self, channel: UpdateChannel) -> Vec<Url> {
        endpoints_on(&self.repo, channel)
            .iter()
            .map(|u| Url::parse(u).unwrap())
            .collect()
    }
}

/// A mock app at version 0.1.0 (mock context) with the updater plugin
/// trusting `pubkey`.
fn app(pubkey: &str, update_checks: bool) -> App<MockRuntime> {
    let dir = tempfile::tempdir().unwrap().keep();
    let mut context = mock_context(noop_assets());
    context.config_mut().plugins.0.insert(
        "updater".into(),
        json!({ "pubkey": pubkey.trim(), "endpoints": [] }),
    );
    let mut handy = crate::settings::get_default_settings();
    handy.update_checks_enabled = update_checks;
    mock_builder()
        .manage(ForkSettingsHandle::new(dir.join("fork-settings.json")))
        .manage(FakeHandyStore::with(handy))
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .build(context)
        .expect("mock app")
}

async fn found_version(app: &App<MockRuntime>, endpoints: Vec<Url>) -> Option<String> {
    find_on(app.handle(), endpoints)
        .await
        .expect("check")
        .map(|(u, _)| u.version)
}

#[tokio::test]
async fn stable_finds_downloads_and_verifies_the_newer_release() {
    let stand = Stand::new();
    stand.release(STABLE_PATH, "0.2.0");
    let app = app(TEST_PUBKEY, true);
    let (update, endpoint) = find_on(app.handle(), stand.endpoints(UpdateChannel::Stable))
        .await
        .unwrap()
        .expect("0.2.0 > 0.1.0");
    assert_eq!(update.version, "0.2.0");
    assert!(endpoint.ends_with(STABLE_PATH));
    let bytes = download_verified(&update)
        .await
        .expect("signature verifies");
    assert_eq!(bytes, ARTIFACT);
    let shown = available(&update, endpoint);
    assert_eq!(shown.date.as_deref(), Some("2026-11-02T12:00:00Z"));
    assert_eq!(shown.notes.as_deref(), Some("Wispr Bro 0.2.0"));
}

#[tokio::test]
async fn up_to_date_when_the_release_is_not_newer() {
    let stand = Stand::new();
    stand.release(STABLE_PATH, "0.1.0");
    let app = app(TEST_PUBKEY, true);
    assert_eq!(
        found_version(&app, stand.endpoints(UpdateChannel::Stable)).await,
        None
    );
}

#[tokio::test]
async fn a_stable_install_ignores_the_beta_prerelease() {
    let stand = Stand::new();
    stand.release(STABLE_PATH, "0.1.0");
    stand.release(BETA_PATH, "0.3.0-beta.1");
    let app = app(TEST_PUBKEY, true);
    assert_eq!(
        found_version(&app, stand.endpoints(UpdateChannel::Stable)).await,
        None
    );
    assert_eq!(
        found_version(&app, stand.endpoints(UpdateChannel::Beta)).await,
        Some("0.3.0-beta.1".into())
    );
}

#[tokio::test]
async fn beta_takes_stable_once_it_overtakes_the_last_beta() {
    let stand = Stand::new();
    stand.release(BETA_PATH, "0.2.0-beta.4");
    stand.release(STABLE_PATH, "0.2.0");
    let app = app(TEST_PUBKEY, true);
    assert_eq!(
        found_version(&app, stand.endpoints(UpdateChannel::Beta)).await,
        Some("0.2.0".into())
    );
}

#[tokio::test]
async fn beta_falls_back_to_stable_before_the_first_beta_exists() {
    let stand = Stand::new();
    stand.release(STABLE_PATH, "0.2.0");
    let app = app(TEST_PUBKEY, true);
    assert_eq!(
        found_version(&app, stand.endpoints(UpdateChannel::Beta)).await,
        Some("0.2.0".into())
    );
}

#[tokio::test]
async fn no_release_at_all_is_an_error() {
    let stand = Stand::new();
    let app = app(TEST_PUBKEY, true);
    assert!(find_on(app.handle(), stand.endpoints(UpdateChannel::Beta))
        .await
        .is_err());
}

#[tokio::test]
async fn a_tampered_download_is_rejected() {
    let stand = Stand::new();
    stand.release(STABLE_PATH, "0.2.0");
    let mut tampered = ARTIFACT.to_vec();
    let last = tampered.len() - 1;
    tampered[last] ^= 0xff;
    stand.put("/artifact.app.tar.gz", tampered);
    let app = app(TEST_PUBKEY, true);
    let (update, _) = find_on(app.handle(), stand.endpoints(UpdateChannel::Stable))
        .await
        .unwrap()
        .unwrap();
    let err = download_verified(&update).await.unwrap_err();
    assert!(err.contains("signature"), "{err}");
}

#[tokio::test]
async fn only_our_pubkey_is_trusted() {
    // The fixture is signed with the test key; an app trusting our real key
    // (the overlay's) must refuse it.
    let overlay: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.fork.conf.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let ours = overlay["plugins"]["updater"]["pubkey"].as_str().unwrap();
    let stand = Stand::new();
    stand.release(STABLE_PATH, "0.2.0");
    let app = app(ours, true);
    let (update, _) = find_on(app.handle(), stand.endpoints(UpdateChannel::Stable))
        .await
        .unwrap()
        .unwrap();
    assert!(download_verified(&update).await.is_err());
}

#[tokio::test]
async fn a_download_outside_our_releases_is_refused() {
    let stand = Stand::new();
    stand.put(
        STABLE_PATH,
        manifest(
            "0.2.0",
            "https://example.com/wispr-bro.app.tar.gz",
            SIGNATURE,
        ),
    );
    let app = app(TEST_PUBKEY, true);
    let (update, _) = find_on(app.handle(), stand.endpoints(UpdateChannel::Stable))
        .await
        .unwrap()
        .unwrap();
    let err = download_verified(&update).await.unwrap_err();
    assert!(err.contains("refusing"), "{err}");
}

#[tokio::test]
async fn checks_are_refused_while_update_checks_are_off() {
    let app = app(TEST_PUBKEY, false);
    assert_eq!(
        check(app.handle()).await.unwrap_err(),
        UpdateError::Disabled
    );
    assert_eq!(
        install(app.handle()).await.unwrap_err(),
        UpdateError::Disabled
    );
    let status = status(app.handle());
    assert!(!status.checks_enabled);
    assert_eq!(status.channel, UpdateChannel::Stable);
    assert_eq!(status.pubkey_id, channel::PUBKEY_KEY_ID);
}

#[test]
fn pick_newest_prefers_the_highest_version_and_tolerates_one_failure() {
    let v = |s: &str| semver::Version::parse(s).unwrap();
    let found = |s: &str| {
        Ok(Some(Found {
            version: v(s),
            item: s.to_string(),
        }))
    };
    assert_eq!(
        pick_newest(vec![found("1.0.0-beta.2"), found("1.0.0")]),
        Ok(Some("1.0.0".into()))
    );
    assert_eq!(
        pick_newest(vec![found("1.1.0-beta.1"), found("1.0.0")]),
        Ok(Some("1.1.0-beta.1".into()))
    );
    assert_eq!(
        pick_newest(vec![Err("404".into()), found("1.0.0")]),
        Ok(Some("1.0.0".into()))
    );
    assert_eq!(
        pick_newest::<String>(vec![Err("404".into()), Ok(None)]),
        Ok(None)
    );
    assert_eq!(
        pick_newest::<String>(vec![Err("a".into()), Err("b".into())]),
        Err("a".into())
    );
}

/// The release acceptance check: a real updater artifact (built and signed by
/// `scripts/wb-release/macos.sh`) is served on loopback and verified by the
/// plugin against the overlay's pubkey.
/// `WB_UPDATER_ARTIFACT=<…app.tar.gz> cargo test --lib real_updater_artifact -- --ignored`
#[tokio::test]
#[ignore = "needs a release artifact in WB_UPDATER_ARTIFACT"]
async fn real_updater_artifact_verifies_against_our_pubkey() {
    let path = std::env::var("WB_UPDATER_ARTIFACT").expect("WB_UPDATER_ARTIFACT");
    let bytes = std::fs::read(&path).unwrap();
    let signature = std::fs::read_to_string(format!("{path}.sig")).unwrap();
    let overlay: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.fork.conf.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let ours = overlay["plugins"]["updater"]["pubkey"].as_str().unwrap();
    let stand = Stand::new();
    stand.put("/artifact.app.tar.gz", bytes.clone());
    stand.put(
        STABLE_PATH,
        manifest("1.0.0", &stand.artifact_url(), signature.trim()),
    );
    let app = app(ours, true);
    let (update, _) = find_on(app.handle(), stand.endpoints(UpdateChannel::Stable))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(download_verified(&update).await.unwrap(), bytes);
}
