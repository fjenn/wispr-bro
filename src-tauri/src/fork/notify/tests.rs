// SPDX-License-Identifier: GPL-3.0-or-later
//! S-18 tests: policy, inbox CRUD, catalog rules, and the whole path on a
//! mock app (toggles filter creation, macOS only for the four kinds and only
//! while away, no permission request before the first real need, clicks and
//! buttons route to the right page or action).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use rusqlite::Connection;

use super::catalog::{self, Notice};
use super::inbox::{self, NotificationFilter};
use super::policy::{self, Args, NotificationAction, NotificationButton, NotificationKind};
use super::*;
use crate::fork::db::ForkDb;
use crate::fork::settings::NotificationSettings;

fn args(pairs: &[(&str, &str)]) -> Args {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect::<BTreeMap<_, _>>()
}

// ---------------------------------------------------------------------------
// Policy

#[test]
fn only_the_four_dec21_kinds_can_reach_macos() {
    let all_on = NotificationSettings::default();
    let system: Vec<_> = NotificationKind::ALL
        .into_iter()
        .filter(|k| policy::system_allowed(*k, &all_on))
        .collect();
    assert_eq!(
        system,
        vec![
            NotificationKind::ModelReady,
            NotificationKind::UpdateReady,
            NotificationKind::QueuedTakesReady,
            NotificationKind::ShortcutProblem,
        ]
    );
    let off = NotificationSettings {
        system: false,
        ..Default::default()
    };
    assert!(NotificationKind::ALL
        .into_iter()
        .all(|k| !policy::system_allowed(k, &off)));
    let no_errors = NotificationSettings {
        errors: false,
        ..Default::default()
    };
    assert!(!policy::system_allowed(
        NotificationKind::ShortcutProblem,
        &no_errors
    ));
    assert!(policy::system_allowed(
        NotificationKind::ModelReady,
        &no_errors
    ));
}

#[test]
fn toggles_filter_creation_by_group() {
    let s = NotificationSettings {
        suggestions: false,
        announcements: false,
        milestones: false,
        ..Default::default()
    };
    for kind in NotificationKind::ALL {
        let expect = !matches!(
            kind,
            NotificationKind::Tip | NotificationKind::Announcement | NotificationKind::Milestone
        );
        assert_eq!(policy::inbox_allowed(kind, &s), expect, "{kind:?}");
        assert!(policy::inbox_allowed(
            kind,
            &NotificationSettings::default()
        ));
    }
}

#[test]
fn kinds_round_trip_their_stored_names() {
    for kind in NotificationKind::ALL {
        assert_eq!(NotificationKind::parse(kind.as_str()), Some(kind));
        let json = serde_json::to_value(kind).unwrap();
        assert_eq!(json, serde_json::json!(kind.as_str()));
    }
    for b in NotificationButton::ALL {
        assert_eq!(NotificationButton::parse(b.id()), Some(b));
        assert_eq!(serde_json::to_value(b).unwrap(), serde_json::json!(b.id()));
    }
}

#[test]
fn categories_match_the_buttons_each_kind_shows() {
    let permission = args(&[("reason", policy::REASON_PERMISSION)]);
    let cases = [
        (NotificationKind::QueuedTakesReady, Args::new()),
        (NotificationKind::ShortcutProblem, permission.clone()),
    ];
    for (kind, a) in cases {
        let id = policy::category_id(kind, &a).expect("has a category");
        let (_, registered) = policy::CATEGORIES.iter().find(|(c, _)| *c == id).unwrap();
        assert_eq!(policy::buttons(kind, &a), registered.to_vec(), "{kind:?}");
    }
    // A shortcut problem that is not about the permission has no button.
    assert!(policy::buttons(NotificationKind::ShortcutProblem, &Args::new()).is_empty());
    assert_eq!(
        policy::category_id(NotificationKind::ShortcutProblem, &Args::new()),
        None
    );
    for kind in NotificationKind::ALL {
        if !matches!(
            kind,
            NotificationKind::QueuedTakesReady | NotificationKind::ShortcutProblem
        ) {
            assert!(policy::buttons(kind, &permission).is_empty(), "{kind:?}");
        }
    }
}

#[test]
fn resolve_routes_clicks_and_buttons() {
    let page = NotificationAction::page("settings", Some("history"));
    let k = NotificationKind::QueuedTakesReady;
    assert_eq!(
        policy::resolve(k, &Args::new(), Some(&page), None),
        Some(page.clone())
    );
    assert_eq!(
        policy::resolve(k, &Args::new(), Some(&page), Some(NotificationButton::Open)),
        Some(page.clone())
    );
    assert_eq!(
        policy::resolve(
            k,
            &Args::new(),
            Some(&page),
            Some(NotificationButton::CopyLatest)
        ),
        Some(NotificationAction::CopyLatest)
    );
    // A button the entry does not offer falls back to the click.
    assert_eq!(
        policy::resolve(
            NotificationKind::ModelReady,
            &Args::new(),
            Some(&page),
            Some(NotificationButton::CopyLatest)
        ),
        Some(page.clone())
    );
    let permission = args(&[("reason", "permission")]);
    assert_eq!(
        policy::resolve(
            NotificationKind::ShortcutProblem,
            &permission,
            Some(&page),
            Some(NotificationButton::OpenAccessibility)
        ),
        Some(NotificationAction::OpenAccessibilitySettings)
    );
    assert_eq!(
        policy::resolve(NotificationKind::Tip, &Args::new(), None, None),
        None
    );
}

#[test]
fn notices_point_at_the_right_pages() {
    use NotificationAction as A;
    assert_eq!(
        Notice::model_ready().action,
        Some(A::page("dictation", None))
    );
    assert_eq!(
        Notice::update_ready("1.2.3").action,
        Some(A::page("settings", Some("about")))
    );
    assert_eq!(
        Notice::queued_takes_ready(3).action,
        Some(A::page("settings", Some("history")))
    );
    assert_eq!(
        Notice::queued_takes_ready(1).title_key,
        "rust.notify.queuedTakesReady.title_one"
    );
    assert_eq!(
        Notice::queued_takes_ready(3).title_key,
        "rust.notify.queuedTakesReady.title_other"
    );
    let p = Notice::shortcut_problem(true);
    assert_eq!(p.args.get("reason").map(String::as_str), Some("permission"));
    assert_eq!(
        p.body_key.as_deref(),
        Some("rust.notify.shortcutProblem.bodyPermission")
    );
    assert!(Notice::shortcut_problem(false).args.is_empty());
    assert_eq!(
        Notice::crash_report("crash-1.txt").action,
        Some(A::RevealCrashReport {
            file_name: "crash-1.txt".into()
        })
    );
    assert_eq!(
        Notice::announcement(&catalog::FEATURES[0]).action,
        Some(A::OpenNotifications)
    );
    assert_eq!(
        Notice::tip(&catalog::TIPS[2]).action,
        Some(A::page("settings", Some("history")))
    );
}

/// Every key a notice or button uses exists in English `fork.json`, with
/// the placeholders the notice fills.
#[test]
fn every_notification_string_exists_in_english() {
    let mut notices = vec![
        Notice::model_ready(),
        Notice::update_ready("1.0.0"),
        Notice::queued_takes_ready(1),
        Notice::queued_takes_ready(2),
        Notice::shortcut_problem(true),
        Notice::shortcut_problem(false),
        Notice::crash_report("crash-1.txt"),
        Notice::data_recovered(1),
        Notice::words_milestone(1000),
        Notice::streak_milestone(7, "2026-10-01"),
    ];
    notices.extend(catalog::TIPS.iter().map(Notice::tip));
    notices.extend(catalog::FEATURES.iter().map(Notice::announcement));
    for n in &notices {
        for key in std::iter::once(&n.title_key).chain(n.body_key.iter()) {
            let template = crate::fork::i18n::get("en", key)
                .unwrap_or_else(|| panic!("missing English string {key}"));
            let rendered = render("en", key, &n.args);
            assert!(
                !rendered.contains("{{"),
                "{key}: unfilled placeholder in {template:?}"
            );
        }
    }
    for b in NotificationButton::ALL {
        assert!(
            crate::fork::i18n::get("en", b.label_key()).is_some(),
            "{b:?}"
        );
    }
    // The other half of each plural pair exists too (i18next picks one).
    for key in [
        "rust.notify.queuedTakesReady.title_one",
        "rust.notify.queuedTakesReady.title_other",
    ] {
        assert!(crate::fork::i18n::get("en", key).is_some(), "{key}");
    }
}

// ---------------------------------------------------------------------------
// Catalog

#[test]
fn milestones_pick_the_highest_reached() {
    assert_eq!(
        catalog::highest_reached(&catalog::WORD_MILESTONES, 999),
        None
    );
    assert_eq!(
        catalog::highest_reached(&catalog::WORD_MILESTONES, 1_000),
        Some(1_000)
    );
    assert_eq!(
        catalog::highest_reached(&catalog::WORD_MILESTONES, 12_345),
        Some(10_000)
    );
    assert_eq!(
        catalog::highest_reached(&catalog::STREAK_MILESTONES, 2),
        None
    );
    assert_eq!(
        catalog::highest_reached(&catalog::STREAK_MILESTONES, 8),
        Some(7)
    );
}

#[test]
fn tips_come_in_order_at_most_one_a_day() {
    let none = |_: &str| false;
    let day = catalog::TIP_INTERVAL_MS;
    assert_eq!(catalog::next_tip(0, &none, None, 0), None);
    assert_eq!(
        catalog::next_tip(2, &none, None, 0).map(|t| t.id),
        Some("hands_free")
    );
    let first_done = |id: &str| id == "hands_free";
    assert_eq!(
        catalog::next_tip(100, &first_done, Some(0), day).map(|t| t.id),
        Some("cancel")
    );
    // Less than a day after the last tip: nothing.
    assert_eq!(catalog::next_tip(100, &first_done, Some(0), day - 1), None);
    let all_done = |_: &str| true;
    assert_eq!(catalog::next_tip(1_000, &all_done, None, 0), None);
    // Ids are unique (they are dedupe keys).
    let mut ids: Vec<_> = catalog::TIPS.iter().map(|t| t.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), catalog::TIPS.len());
}

#[test]
fn the_current_version_announces_the_inbox() {
    let f = catalog::feature_for_version(env!("CARGO_PKG_VERSION"));
    assert_eq!(f.map(|f| f.id), Some("inbox"));
    assert_eq!(catalog::feature_for_version("0.0.1"), None);
    assert_eq!(feature_dismiss_id("inbox"), "feature.inbox");
}

#[test]
fn crash_file_names_are_checked() {
    assert!(is_crash_file_name("crash-20261030-101010.txt"));
    for bad in [
        "crash-../x.txt",
        "x/crash-1.txt",
        "crash-1.log",
        "other.txt",
    ] {
        assert!(!is_crash_file_name(bad), "{bad}");
    }
}

#[test]
fn only_a_recent_crash_file_is_announced() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(recent_crash_file(dir.path(), 0), None);
    std::fs::write(dir.path().join("crash-1.txt"), "a").unwrap();
    std::fs::write(dir.path().join("crash-2.txt"), "b").unwrap();
    std::fs::write(dir.path().join("notes.txt"), "c").unwrap();
    let now = crate::fork::context::now_ms();
    assert_eq!(
        recent_crash_file(dir.path(), now).as_deref(),
        Some("crash-2.txt")
    );
    // A week later neither is recent.
    assert_eq!(
        recent_crash_file(dir.path(), now + CRASH_RECENT_MS + 60_000),
        None
    );
}

#[test]
fn demo_values_parse() {
    assert_eq!(
        parse_demo("4000:model_ready, queued_takes_ready"),
        Some((
            4000,
            vec![
                NotificationKind::ModelReady,
                NotificationKind::QueuedTakesReady
            ]
        ))
    );
    for bad in ["", "4000", "x:model_ready", "10:", "10:nope"] {
        assert_eq!(parse_demo(bad), None, "{bad}");
    }
}

#[test]
fn model_ids_exclude_the_language_detector() {
    assert!(is_speech_model_id("\"whisper-base\""));
    assert!(!is_speech_model_id("\"language-detector\""));
    assert!(!is_speech_model_id(""));
}

// ---------------------------------------------------------------------------
// Inbox CRUD

fn temp_db() -> (tempfile::TempDir, ForkDb) {
    let dir = tempfile::tempdir().unwrap();
    let db = ForkDb::in_dir(dir.path());
    (dir, db)
}

fn on<T>(db: &ForkDb, f: impl FnOnce(&mut Connection) -> Result<T, DbError>) -> T {
    db.with_conn(f).unwrap()
}

#[test]
fn inbox_insert_list_read_and_dedupe() {
    let (_dir, db) = temp_db();
    let a = on(&db, |c| inbox::insert(c, &Notice::model_ready(), 1_000)).unwrap();
    let b = on(&db, |c| {
        inbox::insert(c, &Notice::words_milestone(1000), 2_000)
    })
    .unwrap();
    // Same dedupe key: nothing new.
    assert_eq!(
        on(&db, |c| inbox::insert(
            c,
            &Notice::words_milestone(1000),
            3_000
        )),
        None
    );
    // No key: repeats are kept.
    let c2 = on(&db, |c| inbox::insert(c, &Notice::model_ready(), 4_000)).unwrap();

    let page = on(&db, |c| inbox::list(c, NotificationFilter::All, 50));
    assert_eq!(page.total, 3);
    assert_eq!(page.unread, 3);
    assert_eq!(
        page.items.iter().map(|i| i.id).collect::<Vec<_>>(),
        vec![c2, b, a],
        "newest first"
    );
    let milestone = &page.items[1];
    assert_eq!(milestone.kind, NotificationKind::Milestone);
    assert_eq!(
        milestone.args.get("words").map(String::as_str),
        Some("1000")
    );
    assert_eq!(
        milestone.action,
        Some(NotificationAction::page("insights", None))
    );
    assert!(!milestone.read);

    assert_eq!(on(&db, |c| inbox::mark_read(c, &[b, b, 999], 5_000)), 1);
    assert_eq!(on(&db, |c| inbox::unread_count(c)), 2);
    let unread = on(&db, |c| inbox::list(c, NotificationFilter::Unread, 50));
    assert_eq!(
        unread.items.iter().map(|i| i.id).collect::<Vec<_>>(),
        vec![c2, a]
    );
    assert_eq!(unread.unread, 2);
    assert_eq!(unread.total, 3);

    assert_eq!(on(&db, |c| inbox::mark_all_read(c, 6_000)), 2);
    assert_eq!(on(&db, |c| inbox::unread_count(c)), 0);
    assert!(on(&db, |c| inbox::get(c, a)).unwrap().read);
    assert!(on(&db, |c| inbox::has_key(c, "milestone:words:1000")));
    assert_eq!(
        on(&db, |c| inbox::last_created(
            c,
            NotificationKind::ModelReady
        )),
        Some(4_000)
    );
    assert_eq!(on(&db, |c| inbox::clear(c)), 3);
    assert_eq!(
        on(&db, |c| inbox::list(c, NotificationFilter::All, 50)).total,
        0
    );
}

#[test]
fn inbox_keeps_the_newest_entries() {
    let (_dir, db) = temp_db();
    let extra = 5;
    for i in 0..(inbox::MAX_ENTRIES + extra) {
        on(&db, |c| inbox::insert(c, &Notice::model_ready(), i));
    }
    let page = on(&db, |c| inbox::list(c, NotificationFilter::All, 500));
    assert_eq!(i64::from(page.total), inbox::MAX_ENTRIES);
    assert_eq!(page.items.last().unwrap().created_at, extra);
}

#[test]
fn inbox_skips_rows_of_unknown_kinds() {
    let (_dir, db) = temp_db();
    on(&db, |c| {
        c.execute(
            "INSERT INTO notifications (kind, title_key, created_at) VALUES ('from_the_future', 'x', 1)",
            [],
        )?;
        Ok(())
    });
    on(&db, |c| inbox::insert(c, &Notice::model_ready(), 2));
    let page = on(&db, |c| inbox::list(c, NotificationFilter::All, 50));
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].kind, NotificationKind::ModelReady);
}

// ---------------------------------------------------------------------------
// The whole path on a mock app

/// `TEST_CLIPBOARD` is one process-wide slot: tests that read it run one at
/// a time.
static CLIPBOARD_TESTS: Mutex<()> = Mutex::new(());

#[derive(Default)]
struct FakeSink {
    delivered: Mutex<Vec<SystemNotification>>,
    permission_reads: AtomicUsize,
}

impl SystemSink for FakeSink {
    fn available(&self) -> bool {
        true
    }
    fn deliver(&self, n: SystemNotification) {
        self.delivered.lock().unwrap().push(n);
    }
    fn permission(&self) -> NotificationPermission {
        self.permission_reads.fetch_add(1, Ordering::SeqCst);
        NotificationPermission::NotDetermined
    }
}

impl FakeSink {
    fn kinds(&self) -> Vec<NotificationKind> {
        self.delivered
            .lock()
            .unwrap()
            .iter()
            .map(|n| n.kind)
            .collect()
    }
}

mod app {
    use super::*;
    use serde_json::{json, Value};
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime};
    use tauri::webview::InvokeRequest;
    use tauri::{App, Emitter, Manager, RunEvent, WebviewWindow, WebviewWindowBuilder};
    use tauri_specta::Event;

    use crate::fork;
    use crate::fork::db::handy::fake::{entry, FakeHistory};
    use crate::fork::settings::{FakeHandyStore, ForkSettingsHandle};

    pub struct Harness {
        pub dir: tempfile::TempDir,
        pub sink: Arc<FakeSink>,
        pub active: Arc<AtomicBool>,
        pub history: Arc<FakeHistory>,
    }

    pub fn build(onboarded: bool) -> (App<MockRuntime>, Harness) {
        let dir = tempfile::tempdir().unwrap();
        let sink = Arc::new(FakeSink::default());
        let active = Arc::new(AtomicBool::new(false));
        let history = Arc::new(FakeHistory::new(dir.path()));
        let flag = active.clone();
        let notifier = Notifier::with_activity(
            sink.clone() as Arc<dyn SystemSink>,
            Box::new(move || Some(flag.load(Ordering::SeqCst))),
        );
        let mut handy = crate::settings::get_default_settings();
        handy.onboarding_completed = onboarded;
        let mut context = mock_context(noop_assets());
        context.config_mut().identifier = "com.wisprbro.s18test".into();
        let app = mock_builder()
            .manage(ForkSettingsHandle::new(
                dir.path().join("fork-settings.json"),
            ))
            .manage(ForkDb::in_dir(dir.path()))
            .manage(FakeHandyStore::with(handy))
            .manage(history.clone())
            .manage(notifier)
            .plugin(fork::init())
            .plugin(tauri_plugin_store::Builder::default().build())
            .invoke_handler(fork::compose_invoke(tauri::generate_handler![]))
            .build(context)
            .expect("mock app");
        WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("mock window");
        (
            app,
            Harness {
                dir,
                sink,
                active,
                history,
            },
        )
    }

    pub fn invoke(w: &WebviewWindow<MockRuntime>, cmd: &str, body: Value) -> Result<Value, Value> {
        get_ipc_response(
            w,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "tauri://localhost".parse().unwrap(),
                body: InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.to_string(),
            },
        )
        .map(|b| b.deserialize::<Value>().unwrap())
    }

    /// Runs `f` once Ready has run (so `fork::on_ready` wired everything).
    pub fn run<T: Send + 'static>(
        app: App<MockRuntime>,
        f: impl Fn(&tauri::AppHandle<MockRuntime>, &WebviewWindow<MockRuntime>) -> T + 'static,
    ) -> T {
        let out: Arc<Mutex<Option<T>>> = Arc::new(Mutex::new(None));
        let sink = out.clone();
        app.run(move |handle, event| {
            if let RunEvent::Ready = event {
                let w = handle.get_webview_window("main").expect("main window");
                *sink.lock().unwrap() = Some(f(handle, &w));
                w.destroy().unwrap();
            }
        });
        let result = out.lock().unwrap().take();
        result.expect("Ready was never delivered")
    }

    fn list(w: &WebviewWindow<MockRuntime>) -> Value {
        invoke(
            w,
            "fork_notifications_list",
            json!({ "filter": "all", "limit": null }),
        )
        .unwrap()
    }

    fn kinds(page: &Value) -> Vec<String> {
        page["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["kind"].as_str().unwrap().to_string())
            .collect()
    }

    fn set_notifications(
        handle: &tauri::AppHandle<MockRuntime>,
        f: impl FnOnce(&mut NotificationSettings),
    ) {
        handle
            .state::<ForkSettingsHandle>()
            .update(handle, |s| {
                f(&mut s.notifications);
                Ok(())
            })
            .unwrap();
    }

    /// S-18 exit criterion: launching (Ready, the announcement, crash scan,
    /// milestones) and inbox-only kinds never reach macOS, so macOS never
    /// gets to ask for permission; the first system kind while away does.
    #[test]
    fn no_permission_request_before_the_first_real_need() {
        let (app, h) = build(true);
        let sink = h.sink.clone();
        let active = h.active.clone();
        let (at_ready, after_inbox_only, while_active, away) = run(app, move |handle, _w| {
            let at_ready = sink.delivered.lock().unwrap().len();
            post(handle, Notice::words_milestone(1000));
            post(handle, Notice::tip(&catalog::TIPS[0]));
            post(handle, Notice::crash_report("crash-x.txt"));
            post(handle, Notice::data_recovered(1));
            let after_inbox_only = sink.delivered.lock().unwrap().len();
            active.store(true, Ordering::SeqCst);
            post(handle, Notice::model_ready());
            let while_active = sink.delivered.lock().unwrap().len();
            active.store(false, Ordering::SeqCst);
            post(handle, Notice::model_ready());
            (at_ready, after_inbox_only, while_active, sink.kinds())
        });
        assert_eq!(at_ready, 0, "Ready posted nothing to macOS");
        assert_eq!(after_inbox_only, 0);
        assert_eq!(while_active, 0, "app in front: Hub toast only");
        assert_eq!(away, vec![NotificationKind::ModelReady]);
        assert_eq!(
            h.sink.permission_reads.load(Ordering::SeqCst),
            0,
            "nothing even read the permission"
        );
    }

    /// Only the real sink's `deliver` may ask macOS; nothing at Ready does.
    #[test]
    fn only_delivery_requests_authorization() {
        let src = include_str!("macos.rs");
        let calls: Vec<_> = src
            .match_indices("requestAuthorizationWithOptions_completionHandler(")
            .collect();
        assert_eq!(calls.len(), 1);
        let request_fn = src.find("fn request_then_add(").unwrap();
        assert!(calls[0].0 > request_fn, "inside request_then_add");
        let install = &src[src.find("pub fn install(").unwrap()..];
        let install = &install[..install.find("\n}\n").unwrap()];
        assert!(!install.contains("requestAuthorization"));
        assert!(!install.contains("getNotificationSettings"));
        let root = include_str!("../notify.rs");
        let ready = &root[root.find("pub fn on_ready<").unwrap()..];
        assert!(!ready.contains(".deliver(") && !ready.contains(".permission()"));
    }

    #[test]
    fn toggles_filter_creation_and_system_toggles_filter_macos() {
        let (app, h) = build(true);
        let sink = h.sink.clone();
        let page = run(app, move |handle, w| {
            invoke(w, "fork_notifications_mark_all_read", json!({})).unwrap();
            set_notifications(handle, |n| {
                n.suggestions = false;
                n.milestones = false;
                n.announcements = false;
                n.model_ready = false;
            });
            assert!(post(handle, Notice::tip(&catalog::TIPS[1])).is_none());
            assert!(post(handle, Notice::words_milestone(5000)).is_none());
            assert!(post(handle, Notice::announcement(&catalog::FEATURES[0])).is_none());
            // Still recorded, but not sent to macOS (its toggle is off).
            assert!(post(handle, Notice::model_ready()).is_some());
            post(handle, Notice::queued_takes_ready(2));
            set_notifications(handle, |n| n.system = false);
            post(handle, Notice::update_ready("9.9.9"));
            invoke(
                w,
                "fork_notifications_list",
                json!({ "filter": "unread", "limit": 10 }),
            )
            .unwrap()
        });
        assert_eq!(
            kinds(&page),
            vec!["update_ready", "queued_takes_ready", "model_ready"]
        );
        assert_eq!(h.sink.kinds(), vec![NotificationKind::QueuedTakesReady]);
        let sent = sink.delivered.lock().unwrap()[0].clone();
        assert_eq!(sent.title, "2 dictations ready");
        assert_eq!(sent.category, Some("wb.queued_takes"));
        let (_, labels) = sent
            .categories
            .iter()
            .find(|(id, _)| *id == "wb.queued_takes")
            .unwrap();
        assert_eq!(
            labels,
            &vec![
                (NotificationButton::CopyLatest, "Copy latest".to_string()),
                (NotificationButton::Open, "Open".to_string())
            ]
        );
    }

    #[test]
    fn ready_posts_the_announcement_and_model_events_post_model_ready() {
        let (app, h) = build(true);
        let changed = Arc::new(Mutex::new(Vec::new()));
        let seen = changed.clone();
        ForkNotificationsChanged::listen(&app, move |e| seen.lock().unwrap().push(e.payload));
        let page = run(app, move |handle, w| {
            handle
                .emit("model-download-complete", "language-detector")
                .unwrap();
            handle
                .emit("model-download-complete", "whisper-base")
                .unwrap();
            let feature = invoke(w, "fork_notifications_feature", json!({})).unwrap();
            assert_eq!(feature["id"], json!("inbox"));
            assert_eq!(feature["dismiss_id"], json!("feature.inbox"));
            assert_eq!(feature["action"], json!({ "type": "open_notifications" }));
            assert!(feature["entry_id"].is_i64());
            list(w)
        });
        assert_eq!(kinds(&page), vec!["model_ready", "announcement"]);
        assert_eq!(page["unread"], json!(2));
        let events = changed.lock().unwrap();
        let posted: Vec<_> = events
            .iter()
            .filter_map(|e| e.posted.as_ref().map(|p| p.kind))
            .collect();
        assert_eq!(
            posted,
            vec![NotificationKind::Announcement, NotificationKind::ModelReady]
        );
        // Away from the app: the model-ready one went to macOS too.
        assert_eq!(h.sink.kinds(), vec![NotificationKind::ModelReady]);
    }

    #[test]
    fn announcements_off_hides_the_popup() {
        let (app, _h) = build(true);
        let feature = run(app, |handle, w| {
            set_notifications(handle, |n| n.announcements = false);
            invoke(w, "fork_notifications_feature", json!({})).unwrap()
        });
        assert_eq!(feature, Value::Null);
    }

    #[test]
    fn hub_clicks_and_buttons_route_to_pages_and_actions() {
        let _clipboard = CLIPBOARD_TESTS.lock().unwrap_or_else(|e| e.into_inner());
        let (app, h) = build(true);
        h.history.add(entry(1, 100, "older dictation"), None);
        h.history.add(entry(2, 200, "the latest dictation"), None);
        *TEST_CLIPBOARD.lock().unwrap() = None;
        let (click, copy, accessibility, missing, page) = run(app, move |handle, w| {
            let queued = post(handle, Notice::queued_takes_ready(3)).unwrap();
            let problem = post(handle, Notice::shortcut_problem(true)).unwrap();
            let activate = |id: i64, button: Value| {
                invoke(
                    w,
                    "fork_notifications_activate",
                    json!({ "id": id, "button": button }),
                )
                .unwrap()
            };
            let click = activate(queued.id, Value::Null);
            let copy = activate(queued.id, json!("copy_latest"));
            let accessibility = activate(problem.id, json!("open_accessibility"));
            let missing = activate(99_999, Value::Null);
            (click, copy, accessibility, missing, list(w))
        });
        assert_eq!(
            click,
            json!({ "type": "open_page", "page": "settings", "section": "history" })
        );
        assert_eq!(copy, json!({ "type": "copy_latest" }));
        assert_eq!(
            TEST_CLIPBOARD.lock().unwrap().as_deref(),
            Some("the latest dictation")
        );
        assert_eq!(
            accessibility,
            json!({ "type": "open_accessibility_settings" })
        );
        assert_eq!(missing, Value::Null);
        // Both activated entries are read now; the others are not (the
        // launch reconcile of the fake history may have earned a tip).
        let read: Vec<_> = page["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["read"] == json!(true))
            .map(|i| i["kind"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(read, vec!["shortcut_problem", "queued_takes_ready"]);
    }

    #[test]
    fn macos_clicks_show_the_hub_on_the_right_page() {
        let _clipboard = CLIPBOARD_TESTS.lock().unwrap_or_else(|e| e.into_inner());
        let (app, h) = build(true);
        h.history.add(entry(5, 500, "copied from the banner"), None);
        let opened = Arc::new(Mutex::new(Vec::new()));
        let seen = opened.clone();
        ForkNotificationOpened::listen(&app, move |e| seen.lock().unwrap().push(e.payload));
        *TEST_CLIPBOARD.lock().unwrap() = None;
        run(app, move |handle, _w| {
            let model = post(handle, Notice::model_ready()).unwrap();
            let queued = post(handle, Notice::queued_takes_ready(1)).unwrap();
            on_system_response(
                handle,
                SystemResponse::Activated {
                    id: model.id,
                    button: None,
                },
            );
            // "Copy latest" runs in the background: no window, no event.
            on_system_response(
                handle,
                SystemResponse::Activated {
                    id: queued.id,
                    button: Some(NotificationButton::CopyLatest),
                },
            );
            on_system_response(
                handle,
                SystemResponse::Activated {
                    id: queued.id,
                    button: Some(NotificationButton::Open),
                },
            );
            on_system_response(handle, SystemResponse::OpenSettings);
        });
        let opened = opened.lock().unwrap();
        let actions: Vec<_> = opened.iter().map(|o| o.action.clone()).collect();
        assert_eq!(
            actions,
            vec![
                NotificationAction::page("dictation", None),
                NotificationAction::page("settings", Some("history")),
                NotificationAction::OpenNotifications,
            ]
        );
        assert_eq!(
            TEST_CLIPBOARD.lock().unwrap().as_deref(),
            Some("copied from the banner")
        );
    }

    #[test]
    fn stats_earn_milestones_and_tips_once() {
        let (app, _h) = build(true);
        let (first, second) = run(app, move |handle, w| {
            let today = chrono::Local::now().date_naive();
            handle
                .state::<ForkDb>()
                .with_conn(|c| {
                    for back in 0..8 {
                        let day = (today - chrono::Duration::days(back)).format("%Y-%m-%d");
                        c.execute(
                            "INSERT INTO daily_stats (day, words, dictations, speech_ms, timed_words)
                             VALUES (?1, 1300, 2, 60000, 1300)",
                            [day.to_string()],
                        )?;
                    }
                    Ok(())
                })
                .unwrap();
            check_progress(handle);
            let first = list(w);
            check_progress(handle);
            (first, list(w))
        });
        // 10,400 words → the 10,000 milestone only; an 8-day streak → 7.
        let items = first["items"].as_array().unwrap();
        let milestones: Vec<_> = items
            .iter()
            .filter(|i| i["kind"] == json!("milestone"))
            .map(|i| i["args"].clone())
            .collect();
        assert_eq!(
            milestones,
            vec![json!({ "days": "7" }), json!({ "words": "10000" })]
        );
        let tips: Vec<_> = items.iter().filter(|i| i["kind"] == json!("tip")).collect();
        assert_eq!(tips.len(), 1, "one tip a day");
        assert_eq!(
            tips[0]["title_key"],
            json!("rust.notify.tips.handsFree.title")
        );
        assert_eq!(second["total"], first["total"], "nothing posted twice");
    }

    #[test]
    fn shortcut_problems_wait_for_onboarding() {
        let (app, h) = build(false);
        run(app, |handle, _w| {
            shortcut_problem(handle, true);
        });
        assert!(h.sink.kinds().is_empty());
        let (app, h) = build(true);
        let page = run(app, |handle, w| {
            shortcut_problem(handle, true);
            list(w)
        });
        assert_eq!(kinds(&page)[0], "shortcut_problem");
        assert_eq!(page["items"][0]["buttons"], json!(["open_accessibility"]));
        assert_eq!(h.sink.kinds(), vec![NotificationKind::ShortcutProblem]);
    }

    #[test]
    fn a_recovered_database_is_reported_once() {
        let (app, h) = build(true);
        // Not a database: the first open moves it aside and recreates it.
        std::fs::write(
            h.dir.path().join(crate::fork::db::DB_FILE_NAME),
            "not sqlite",
        )
        .unwrap();
        let (first, again) = run(app, |handle, w| {
            let first = list(w);
            drain_data_recovered(handle);
            (first, list(w))
        });
        assert_eq!(
            kinds(&first)
                .iter()
                .filter(|k| *k == "data_recovered")
                .count(),
            1
        );
        assert_eq!(again["total"], first["total"]);
    }

    #[test]
    fn commands_mark_read_and_emit_the_unread_count() {
        let (app, _h) = build(true);
        let counts = Arc::new(Mutex::new(Vec::new()));
        let seen = counts.clone();
        ForkNotificationsChanged::listen(&app, move |e| {
            if e.payload.posted.is_none() {
                seen.lock().unwrap().push(e.payload.unread);
            }
        });
        let (one, all, page) = run(app, |handle, w| {
            let a = post(handle, Notice::model_ready()).unwrap();
            post(handle, Notice::model_ready());
            let one = invoke(w, "fork_notifications_mark_read", json!({ "ids": [a.id] })).unwrap();
            let all = invoke(w, "fork_notifications_mark_all_read", json!({})).unwrap();
            (one, all, list(w))
        });
        assert_eq!(one, json!(2), "announcement + one model entry left");
        assert_eq!(all, json!(0));
        assert_eq!(page["unread"], json!(0));
        assert_eq!(*counts.lock().unwrap(), vec![2, 0]);
    }

    #[test]
    fn commands_are_not_ready_before_ready() {
        let (app, _h) = build(true);
        let w = app.get_webview_window("main").unwrap();
        let err = invoke(
            &w,
            "fork_notifications_list",
            json!({ "filter": "all", "limit": 5 }),
        )
        .unwrap_err();
        assert_eq!(err, json!({ "kind": "NotReady" }));
    }

    #[test]
    fn debug_post_uses_the_real_path() {
        let (app, h) = build(true);
        let sink = h.sink.clone();
        run(app, move |_handle, w| {
            invoke(
                w,
                "fork_notifications_debug_post",
                json!({ "kind": "update_ready", "delay_ms": 0 }),
            )
            .unwrap();
            for _ in 0..100 {
                if !sink.kinds().is_empty() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        assert_eq!(h.sink.kinds(), vec![NotificationKind::UpdateReady]);
    }

    /// Samples (debug panel, Mac test demo) repeat even when their dedupe
    /// key was earned already.
    #[test]
    fn samples_repeat_past_their_dedupe_key() {
        let (app, _h) = build(true);
        let (first, second) = run(app, |handle, _w| {
            let post = |seq| {
                crate::fork::commands::notifications::post_sample(
                    handle,
                    NotificationKind::Milestone,
                    seq,
                )
            };
            (post(0), post(1))
        });
        assert!(first.is_some() && second.is_some(), "posted both times");
        assert_eq!(first.unwrap().kind, NotificationKind::Milestone);
    }

    /// Esc observed by the hotkey tap closes toasts (and is never blocked:
    /// the registry has no way to block).
    #[test]
    fn esc_dismisses_toasts() {
        let (app, _h) = build(true);
        let (tx, rx) = mpsc::channel();
        ForkToastsDismiss::listen(&app, move |e| {
            let _ = tx.send(e.payload);
        });
        run(app, |_handle, _w| {
            crate::fork::shortcut::registry().notify_esc(
                crate::fork::shortcut::actions::EscContext {
                    cancelled_take: false,
                },
            );
        });
        let got = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(!got.cancelled_take);
    }
}
