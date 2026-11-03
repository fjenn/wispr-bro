// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for `fork::hub::park` (S-38). The decision is pure; the window
//! side runs on `tauri::test::MockRuntime`, whose windows always report
//! visible, so destroying is driven through `mark_main_parked`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::{App, Manager, WebviewWindowBuilder};

use super::*;

const HIDDEN: Option<WindowShown> = Some(WindowShown {
    visible: false,
    minimized: false,
});

fn app() -> App<MockRuntime> {
    mock_builder()
        .manage(Parking::with_delay(Some(Duration::from_millis(10))))
        .build(mock_context(noop_assets()))
        .expect("mock app")
}

#[test]
fn a_hidden_window_is_parked_only_in_its_own_generation() {
    let parking = Parking::with_delay(Some(PARK_AFTER));
    let scratch = crate::fork::scratchpad::WINDOW_LABEL;
    let g = parking.bump(scratch);
    assert_eq!(parking.decide(scratch, g, HIDDEN), Ok(()));
    // Shown (or hidden again) since: that park is stale.
    let g2 = parking.bump(scratch);
    assert_eq!(parking.decide(scratch, g, HIDDEN), Err(Kept::Superseded));
    assert_eq!(parking.decide(scratch, g2, HIDDEN), Ok(()));
    // Generations are per window.
    assert_eq!(parking.decide(MAIN, 0, None), Err(Kept::Gone));
}

#[test]
fn visible_minimized_and_missing_windows_are_kept() {
    let parking = Parking::with_delay(Some(PARK_AFTER));
    let scratch = crate::fork::scratchpad::WINDOW_LABEL;
    let g = parking.bump(scratch);
    let visible = WindowShown {
        visible: true,
        minimized: false,
    };
    // A minimized NSWindow is not "visible"; it must stay in the Dock.
    let minimized = WindowShown {
        visible: false,
        minimized: true,
    };
    assert_eq!(
        parking.decide(scratch, g, Some(visible)),
        Err(Kept::Visible)
    );
    assert_eq!(
        parking.decide(scratch, g, Some(minimized)),
        Err(Kept::Minimized)
    );
    assert_eq!(parking.decide(scratch, g, None), Err(Kept::Gone));
}

#[test]
fn the_hub_is_parked_only_after_its_page_reported_ready() {
    // Handy's App.tsx initializes Enigo and the shortcuts from the page.
    let app = app();
    let parking = app.state::<Parking>();
    let g = parking.bump(MAIN);
    assert_eq!(parking.decide(MAIN, g, HIDDEN), Err(Kept::NotReady));
    hub_ready(app.handle());
    assert!(parking.main_ready());
    assert_eq!(parking.decide(MAIN, g, HIDDEN), Ok(()));
}

#[test]
fn delay_parsing() {
    assert_eq!(parse_delay("off"), None);
    assert_eq!(parse_delay("0"), None);
    assert_eq!(parse_delay(" 5 "), Some(Duration::from_secs(5)));
    assert_eq!(parse_delay("soon"), Some(PARK_AFTER));
    assert!(Parking::with_delay(None).after.is_none());
}

#[test]
fn only_codeless_exit_requests_are_refused_and_only_while_parked() {
    let app = app();
    let handle = app.handle();
    assert!(!keeps_running(handle, None));
    app.state::<Parking>().mark_main_parked(None);
    assert!(keeps_running(handle, None));
    // Quit (tray, ⌘Q, updater restart) always carries a code.
    assert!(!keeps_running(handle, Some(0)));
}

#[test]
fn a_parked_hub_is_rebuilt_with_its_frame_and_deliveries_wait_for_its_page() {
    let app = app();
    let handle = app.handle().clone();
    app.state::<Parking>().mark_main_parked(Some(Frame {
        position: (40.0, 60.0),
        size: (1000.0, 700.0),
    }));
    assert!(app.get_webview_window(MAIN).is_none());

    let delivered = Arc::new(AtomicUsize::new(0));
    let d = delivered.clone();
    show_main_then(&handle, move || {
        d.fetch_add(1, Ordering::SeqCst);
    });
    assert!(app.get_webview_window(MAIN).is_some(), "rebuilt");
    let parking = app.state::<Parking>();
    assert!(!parking.main_parked());
    assert!(!keeps_running(&handle, None));
    // The new page has not reported ready: the delivery waits.
    assert_eq!(delivered.load(Ordering::SeqCst), 0);
    hub_ready(&handle);
    assert_eq!(delivered.load(Ordering::SeqCst), 1);
    // Ready page: the next one goes at once.
    let d = delivered.clone();
    show_main_then(&handle, move || {
        d.fetch_add(1, Ordering::SeqCst);
    });
    assert_eq!(delivered.load(Ordering::SeqCst), 2);
}

#[test]
fn ensure_main_leaves_an_existing_window_and_builds_nothing_unparked() {
    let app = app();
    // Not parked and no window (headless CLI): nothing is built.
    assert!(ensure_main(app.handle()).is_none());
    assert!(app.get_webview_window(MAIN).is_none());
    let built = WebviewWindowBuilder::new(&app, MAIN, Default::default())
        .build()
        .unwrap();
    let parking = app.state::<Parking>();
    let before = parking.current(MAIN);
    let got = ensure_main(app.handle()).expect("main");
    assert_eq!(got.label(), built.label());
    // Showing cancels a scheduled park.
    assert_eq!(parking.current(MAIN), before + 1);
}

#[test]
fn only_the_hub_and_the_scratchpad_are_parkable() {
    assert_eq!(parkable("main"), Some(MAIN));
    assert_eq!(parkable("fork-scratchpad"), Some("fork-scratchpad"));
    // The bar's web view has its own lifecycle (S-03: destroyed after a
    // take); Handy's overlay is never built.
    assert_eq!(parkable("fork-pill"), None);
    assert_eq!(parkable("recording_overlay"), None);
}

#[test]
fn the_show_main_window_hook_is_in_place() {
    let lib = include_str!("../../lib.rs");
    let at = lib.find("fn show_main_window(app: &AppHandle) {").unwrap();
    let body = &lib[at..at + 200];
    assert!(
        body.contains("crate::fork::hub::park::ensure_main(app);"),
        "{body}"
    );
}
