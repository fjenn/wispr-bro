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

// ------------------------------------------------------------ S-28 -------

mod s28 {
    use super::super::failures::{FailureLog, MAX_FAILURES};
    use super::super::idle_gate::Activity;
    use super::super::scheduler::*;
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::time::Duration;

    const MIN: i64 = 60_000;

    fn timing() -> Timing {
        Timing::default()
    }

    fn inputs(now_ms: i64, busy: bool, last: i64) -> Inputs {
        Inputs {
            checks_enabled: true,
            automatic: true,
            channel: UpdateChannel::Stable,
            activity: Activity {
                now_ms,
                busy,
                last_activity_ms: last,
                window_focused: false,
            },
            silent_install: true,
        }
    }

    /// A machine with 1.0.1 downloaded at t = 0 (launch at t = 0).
    fn ready_machine() -> Machine {
        let mut m = Machine::new(0, FailureLog::default(), timing());
        assert_eq!(m.tick(&inputs(MIN, false, 0)), Step::Check);
        assert_eq!(
            m.on_checked(MIN, UpdateChannel::Stable, Ok(Some("1.0.1".into()))),
            Step::Download
        );
        assert!(m.on_downloaded(MIN, Ok(())));
        assert_eq!(m.phase, ForkUpdatePhase::Ready);
        m
    }

    #[test]
    fn timing_env_overrides_parse_and_ignore_junk() {
        let t = Timing::parse("first=3,period=60,retry=5,quiet=30,tick=1,bogus=9,quiet");
        assert_eq!(t.first_check, Duration::from_secs(3));
        assert_eq!(t.period, Duration::from_secs(60));
        assert_eq!(t.retry_check, Duration::from_secs(5));
        assert_eq!(t.retry_install, Duration::from_secs(5));
        assert_eq!(t.quiet, Duration::from_secs(30));
        assert_eq!(t.tick, Duration::from_secs(1));
        assert_eq!(Timing::parse(""), Timing::default());
        assert_eq!(Timing::default().quiet, Duration::from_secs(20 * 60));
    }

    #[test]
    fn checks_start_after_the_first_delay_and_repeat_periodically() {
        let mut m = Machine::new(0, FailureLog::default(), timing());
        assert_eq!(m.tick(&inputs(59_000, false, 0)), Step::Nothing);
        assert_eq!(m.tick(&inputs(MIN, false, 0)), Step::Check);
        assert_eq!(m.tick(&inputs(MIN, false, 0)), Step::Nothing, "in flight");
        assert_eq!(
            m.on_checked(MIN, UpdateChannel::Stable, Ok(None)),
            Step::Nothing
        );
        assert_eq!(m.phase, ForkUpdatePhase::UpToDate);
        assert_eq!(m.tick(&inputs(6 * 60 * MIN, false, 0)), Step::Nothing);
        assert_eq!(m.tick(&inputs(6 * 60 * MIN + MIN, false, 0)), Step::Check);
        // A failed check retries after 30 minutes.
        m.on_checked(361 * MIN, UpdateChannel::Stable, Err("offline".into()));
        assert_eq!(m.phase, ForkUpdatePhase::CheckFailed);
        assert_eq!(m.tick(&inputs(390 * MIN, false, 0)), Step::Nothing);
        assert_eq!(m.tick(&inputs(391 * MIN, false, 0)), Step::Check);
    }

    // Guard case "skips dev builds": no automatic check, a manual one works.
    #[test]
    fn dev_builds_never_check_on_their_own_but_check_on_request() {
        let mut m = Machine::new(0, FailureLog::default(), timing());
        let mut i = inputs(10 * 60 * MIN, false, 0);
        i.automatic = false;
        assert_eq!(m.tick(&i), Step::Nothing);
        assert_eq!(m.request_check(), Step::Check);
        assert_eq!(m.request_check(), Step::Nothing, "already checking");
        m.on_checked(0, UpdateChannel::Stable, Ok(Some("1.0.1".into())));
        assert!(m.on_downloaded(0, Ok(())));
        // Ready, but a dev build installs only on Restart now.
        assert_eq!(m.tick(&i), Step::Nothing);
        assert_eq!(m.request_restart(false), Ok(Step::Install));
    }

    // Guard case "blocks during recording" and the 20-minute quiet window.
    #[test]
    fn a_ready_update_installs_only_when_idle_for_twenty_minutes() {
        let mut m = ready_machine();
        assert_eq!(
            m.tick(&inputs(30 * MIN, true, 0)),
            Step::Nothing,
            "mid-take"
        );
        assert_eq!(m.phase, ForkUpdatePhase::Ready);
        // The take was seen at 30 min: nothing before 50 min.
        assert_eq!(m.tick(&inputs(49 * MIN, false, 30 * MIN)), Step::Nothing);
        let mut focused = inputs(50 * MIN, false, 30 * MIN);
        focused.activity.window_focused = true;
        assert_eq!(m.tick(&focused), Step::Nothing, "the user is in the Hub");
        assert_eq!(m.tick(&inputs(50 * MIN, false, 30 * MIN)), Step::Install);
        assert_eq!(m.phase, ForkUpdatePhase::Installing);
        m.on_installed(50 * MIN, Ok(()));
        assert_eq!(m.phase, ForkUpdatePhase::Installed);
    }

    // Guard case "vetoes relaunch mid-take".
    #[test]
    fn an_installed_update_never_relaunches_mid_take() {
        let mut m = ready_machine();
        assert_eq!(m.tick(&inputs(40 * MIN, false, 0)), Step::Install);
        m.on_installed(40 * MIN, Ok(()));
        assert_eq!(m.tick(&inputs(40 * MIN, true, 40 * MIN)), Step::Nothing);
        assert_eq!(
            m.request_restart(true),
            Err(RestartRefused::Busy),
            "Restart now waits too"
        );
        // Right after the take: no quiet window, the files are new already.
        assert_eq!(m.tick(&inputs(41 * MIN, false, 41 * MIN)), Step::Relaunch);
        assert_eq!(m.request_restart(false), Ok(Step::Relaunch));
    }

    #[test]
    fn a_bundle_that_needs_a_password_installs_only_on_restart_now() {
        let mut m = ready_machine();
        let mut i = inputs(60 * MIN, false, 0);
        i.silent_install = false;
        assert_eq!(m.tick(&i), Step::Nothing);
        assert_eq!(m.phase, ForkUpdatePhase::Ready);
        assert_eq!(m.request_restart(false), Ok(Step::Install));
    }

    #[test]
    fn restart_now_installs_but_is_refused_mid_take_or_without_an_update() {
        let mut m = Machine::new(0, FailureLog::default(), timing());
        assert_eq!(m.request_restart(false), Err(RestartRefused::NothingReady));
        let mut m = ready_machine();
        assert_eq!(m.request_restart(true), Err(RestartRefused::Busy));
        assert_eq!(m.phase, ForkUpdatePhase::Ready);
        assert_eq!(m.request_restart(false), Ok(Step::Install));
    }

    // "after 3 failed installs it pauses 24 h".
    #[test]
    fn three_failed_installs_pause_background_updates_for_a_day() {
        let mut m = ready_machine();
        let mut now = 30 * MIN;
        for attempt in 1..=MAX_FAILURES {
            if attempt > 1 {
                assert_eq!(m.tick(&inputs(now, false, 0)), Step::Check, "{attempt}");
                m.on_checked(now, UpdateChannel::Stable, Ok(Some("1.0.1".into())));
                assert!(m.on_downloaded(now, Ok(())));
            }
            assert_eq!(m.tick(&inputs(now, false, 0)), Step::Install);
            m.on_installed(now, Err("read-only volume".into()));
            assert_eq!(m.phase, ForkUpdatePhase::InstallFailed);
            assert_eq!(m.failures.count, attempt);
            now += 61 * MIN; // past the 1 h retry
        }
        assert!(m.failures.is_paused(now));
        assert_eq!(m.tick(&inputs(now, false, 0)), Step::Nothing, "paused");
        assert_eq!(
            m.tick(&inputs(now + 22 * 60 * MIN, false, 0)),
            Step::Nothing
        );
        // Retry by hand still works during the pause.
        assert_eq!(m.clone().request_check(), Step::Check);
        // After 24 h the pause ends and the count starts again.
        let later = 30 * MIN + 2 * 61 * MIN + 24 * 60 * MIN;
        assert_eq!(m.tick(&inputs(later, false, 0)), Step::Check);
        assert_eq!(m.failures.count, 0);
    }

    #[test]
    fn a_bad_signature_counts_as_a_failed_install() {
        let mut m = Machine::new(0, FailureLog::default(), timing());
        m.tick(&inputs(MIN, false, 0));
        m.on_checked(MIN, UpdateChannel::Stable, Ok(Some("1.0.1".into())));
        assert!(!m.on_downloaded(MIN, Err("signature".into())));
        assert_eq!(m.phase, ForkUpdatePhase::InstallFailed);
        assert_eq!(m.failures.count, 1);
        assert_eq!(m.failures.version.as_deref(), Some("1.0.1"));
    }

    #[test]
    fn turning_checks_off_drops_the_update_and_on_checks_again() {
        let mut m = ready_machine();
        let mut off = inputs(30 * MIN, false, 0);
        off.checks_enabled = false;
        assert_eq!(m.tick(&off), Step::Nothing);
        assert_eq!(m.phase, ForkUpdatePhase::Off);
        assert_eq!(m.version, None);
        assert!(!m.take_quit_install(false), "nothing installs at quit");
        assert_eq!(m.tick(&inputs(31 * MIN, false, 0)), Step::Check);
    }

    #[test]
    fn a_ready_beta_is_dropped_when_the_user_returns_to_stable() {
        let mut m = Machine::new(0, FailureLog::default(), timing());
        let mut beta = inputs(MIN, false, 0);
        beta.channel = UpdateChannel::Beta;
        assert_eq!(m.tick(&beta), Step::Check);
        m.on_checked(MIN, UpdateChannel::Beta, Ok(Some("1.1.0-beta.1".into())));
        assert!(m.on_downloaded(MIN, Ok(())));
        assert_eq!(m.tick(&inputs(2 * MIN, false, 0)), Step::Nothing);
        assert_eq!(m.phase, ForkUpdatePhase::Idle);
        assert_eq!(m.version, None);
        assert_eq!(m.tick(&inputs(2 * MIN, false, 0)), Step::Check);
    }

    #[test]
    fn at_quit_a_ready_update_installs_once() {
        let mut m = ready_machine();
        assert!(m.take_quit_install(true));
        assert!(!m.take_quit_install(true));
        m.on_installed(MIN, Ok(()));
        assert_eq!(m.phase, ForkUpdatePhase::Installed);
        let mut idle = Machine::new(0, FailureLog::default(), timing());
        assert!(!idle.take_quit_install(true));
    }

    #[test]
    fn late_results_after_a_phase_change_are_ignored() {
        let mut m = Machine::new(0, FailureLog::default(), timing());
        assert_eq!(
            m.on_checked(0, UpdateChannel::Stable, Ok(Some("9.0.0".into()))),
            Step::Nothing
        );
        assert!(!m.on_downloaded(0, Ok(())));
        m.on_installed(0, Ok(()));
        assert_eq!(m.phase, ForkUpdatePhase::Idle);
    }

    // ---------------------------------------------------- executor -------

    static INSTALLS: AtomicU32 = AtomicU32::new(0);
    static FAIL_INSTALL: AtomicBool = AtomicBool::new(false);
    /// The executor tests share the install counter and the activity clock.
    static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    fn fake_install(update: &Update, bytes: Vec<u8>) -> Result<(), String> {
        assert_eq!(bytes, ARTIFACT, "the verified bytes reach the install");
        assert_eq!(update.version, "0.2.0");
        INSTALLS.fetch_add(1, Ordering::SeqCst);
        if FAIL_INSTALL.load(Ordering::SeqCst) {
            Err("read-only volume".into())
        } else {
            Ok(())
        }
    }

    struct Rig {
        app: App<MockRuntime>,
        relaunched: Arc<Mutex<Vec<bool>>>,
        _stand: Stand,
        dir: tempfile::TempDir,
    }

    /// A mock app (0.1.0, our test key) whose scheduler reads a loopback
    /// stand serving 0.2.0, with a fake install and relaunch probe.
    fn rig(quiet_secs: u64) -> Rig {
        let stand = Stand::new();
        stand.release(STABLE_PATH, "0.2.0");
        let app = app(TEST_PUBKEY, true);
        crate::fork::specta_builder::<MockRuntime>().mount_events(app.handle());
        let dir = tempfile::tempdir().unwrap();
        let timing = Timing {
            quiet: Duration::from_secs(quiet_secs),
            ..Timing::default()
        };
        let scheduler = UpdateScheduler::new(
            Machine::new(
                0,
                FailureLog::default(),
                Timing {
                    first_check: Duration::ZERO,
                    ..timing
                },
            ),
            Some(failures::state_path(dir.path())),
            true,
            fake_install,
            Some(stand.endpoints(UpdateChannel::Stable)),
        );
        let relaunched: Arc<Mutex<Vec<bool>>> = Arc::default();
        let probe = relaunched.clone();
        scheduler.set_relaunch(Arc::new(move |hidden| probe.lock().unwrap().push(hidden)));
        app.manage(scheduler);
        app.manage(FakeBusy(AtomicBool::new(false)));
        Rig {
            app,
            relaunched,
            _stand: stand,
            dir,
        }
    }

    fn set_busy(rig: &Rig, busy: bool) {
        rig.app.state::<FakeBusy>().0.store(busy, Ordering::SeqCst);
    }

    #[tokio::test]
    async fn the_scheduler_downloads_waits_for_quiet_installs_and_relaunches_hidden() {
        let _serial = SERIAL.lock().await;
        INSTALLS.store(0, Ordering::SeqCst);
        FAIL_INSTALL.store(false, Ordering::SeqCst);
        let rig = rig(3600);
        let handle = rig.app.handle();
        super::super::idle_gate::note_activity();

        // Check + download + verify; the take just now keeps it waiting.
        tick_once(handle).await;
        let s = state(handle);
        assert_eq!(s.phase, ForkUpdatePhase::Ready, "{s:?}");
        assert_eq!(s.version.as_deref(), Some("0.2.0"));
        assert!(s.automatic);
        set_busy(&rig, true);
        tick_once(handle).await;
        assert!(state(handle).busy);
        assert_eq!(INSTALLS.load(Ordering::SeqCst), 0, "never mid-take");
        set_busy(&rig, false);
        tick_once(handle).await;
        assert_eq!(
            INSTALLS.load(Ordering::SeqCst),
            0,
            "inside the quiet window"
        );

        // Restart now while a take runs is refused; afterwards it installs
        // and relaunches hidden (no Hub window in the mock).
        set_busy(&rig, true);
        assert_eq!(restart_now(handle), Err(RestartRefused::Busy));
        set_busy(&rig, false);
        assert_eq!(restart_now(handle), Ok(()));
        for _ in 0..100 {
            if !rig.relaunched.lock().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(INSTALLS.load(Ordering::SeqCst), 1);
        assert_eq!(*rig.relaunched.lock().unwrap(), vec![true]);
        assert_eq!(state(handle).phase, ForkUpdatePhase::Installed);
        // The quit after that installs nothing more.
        on_exit(handle);
        assert_eq!(INSTALLS.load(Ordering::SeqCst), 1);
        drop(rig.dir);
    }

    #[tokio::test]
    async fn the_quit_install_uses_the_held_bytes_without_a_relaunch() {
        let _serial = SERIAL.lock().await;
        FAIL_INSTALL.store(false, Ordering::SeqCst);
        let rig = rig(3600);
        let handle = rig.app.handle();
        super::super::idle_gate::note_activity();
        tick_once(handle).await;
        assert_eq!(state(handle).phase, ForkUpdatePhase::Ready);
        let s = rig.app.state::<UpdateScheduler>();
        // At quit the held bytes install (fake_install checks them).
        on_exit(handle);
        assert_eq!(s.machine().phase, ForkUpdatePhase::Installed);
        assert!(
            rig.relaunched.lock().unwrap().is_empty(),
            "no relaunch at quit"
        );
        let saved = failures::load(&failures::state_path(rig.dir.path()));
        assert_eq!(saved, FailureLog::default());
    }

    #[tokio::test]
    async fn a_failed_install_is_kept_on_disk_and_nothing_relaunches() {
        let _serial = SERIAL.lock().await;
        FAIL_INSTALL.store(true, Ordering::SeqCst);
        let rig = rig(3600);
        let handle = rig.app.handle();
        super::super::idle_gate::note_activity();
        tick_once(handle).await;
        assert_eq!(state(handle).phase, ForkUpdatePhase::Ready);
        assert_eq!(restart_now(handle), Ok(()));
        for _ in 0..100 {
            if state(handle).phase == ForkUpdatePhase::InstallFailed {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        FAIL_INSTALL.store(false, Ordering::SeqCst);
        let s = state(handle);
        assert_eq!(s.phase, ForkUpdatePhase::InstallFailed);
        assert_eq!(s.failures, 1);
        assert!(rig.relaunched.lock().unwrap().is_empty());
        let saved = failures::load(&failures::state_path(rig.dir.path()));
        assert_eq!(saved.count, 1);
        assert_eq!(saved.version.as_deref(), Some("0.2.0"));
    }

    #[tokio::test]
    async fn checks_off_reports_off_and_refuses_a_manual_check() {
        let app = app(TEST_PUBKEY, false);
        crate::fork::specta_builder::<MockRuntime>().mount_events(app.handle());
        let handle = app.handle();
        assert_eq!(state(handle).phase, ForkUpdatePhase::Off);
        app.manage(UpdateScheduler::new(
            Machine::new(0, FailureLog::default(), Timing::default()),
            None,
            true,
            fake_install,
            None,
        ));
        app.manage(FakeBusy(AtomicBool::new(false)));
        tick_once(handle).await;
        assert_eq!(state(handle).phase, ForkUpdatePhase::Off);
        assert_eq!(check_now(handle).phase, ForkUpdatePhase::Off);
    }
}
