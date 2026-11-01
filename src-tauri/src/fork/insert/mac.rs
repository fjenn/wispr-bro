// SPDX-License-Identifier: GPL-3.0-or-later
//! The app side of a delivery on macOS: the [`Platform`] on AppKit (main
//! thread where AppKit or TIS need it, the calling thread otherwise), the
//! target-app class, Handy's settings, and the restore thread.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use objc2_app_kit::NSWorkspace;
use tauri::{AppHandle, Manager};

use super::deliver::{self, Gate, Platform, PublishedText, Request};
use super::verify::{self, FieldState};
use super::{
    discard_target, hook_result, interruption, lock, next_take, publish_outcome, route, Kind, LINE,
    SLOT,
};
use crate::fork::clipboard::restore::{TargetClass, Timing};
use crate::fork::clipboard::snapshot::Snapshot;
use crate::fork::clipboard::{keys, nspb, target};
use crate::settings::{AppSettings, AutoSubmitKey, ClipboardHandling};

/// Delivers on the paste thread Handy spawned for this take (or on the
/// recovery thread for Undo and paste-last, S-06).
pub(super) fn deliver(
    text: String,
    app: AppHandle,
    settings: &AppSettings,
    kind: Kind,
    auto_send: Option<crate::fork::context::app_rules::Armed>,
) -> (Result<(), String>, Option<deliver::PasteOutcome>) {
    let text = if settings.append_trailing_space {
        format!("{text} ")
    } else {
        text
    };
    let take = next_take();
    let _slot = lock(&SLOT);

    let rm = app
        .try_state::<Arc<crate::managers::audio::AudioRecordingManager>>()
        .map(|s| Arc::clone(&s));
    let generation = rm.as_ref().map(|r| r.cancel_generation());
    let cancelled = || match (&rm, generation) {
        (Some(r), Some(g)) => r.was_cancelled_since(g),
        _ => false,
    };

    // S-05: the key-up target still has focus and is a place to type.
    if cancelled() || interruption().is_some() {
        // `paste` reports the cancel or the interruption.
        if kind == Kind::Take {
            discard_target();
        }
    } else if let Some(reason) = route(take, kind) {
        let delivery = deliver::Delivery::Card { reason };
        publish_outcome(&app, take, delivery, &text, kind);
        return (hook_result(delivery), delivery.paste_outcome());
    }

    let (bundle, class) = frontmost_class();
    let mut req = Request::new(&text, class);
    // Paste-last puts the text back; it never sends it (S-06).
    req.submit = (settings.auto_submit && kind != Kind::Again)
        .then(|| submit_flags(settings.auto_submit_key));
    // S-19a "Press Enter after dictating": plain Return, only into the app
    // it was armed for and only after a confirmed paste.
    if let Some(armed) = auto_send {
        if bundle
            .as_deref()
            .is_some_and(|b| b.eq_ignore_ascii_case(&armed.bundle_id))
        {
            req.submit = Some(0);
            req.submit_strict = true;
        } else {
            log::info!("insert: take {take} no Return (the app in front changed)");
        }
    }
    req.keep_transcript = settings.clipboard_handling == ClipboardHandling::CopyToClipboard;

    let platform = AppPlatform { app: app.clone() };
    let gate = Gate {
        cancelled: &cancelled,
        interrupted: &interruption,
    };
    let (report, pending) = deliver::paste(&platform, &LINE, &req, &gate);
    log::info!(
        "insert: take {take} ({kind:?}) {} via {:?} into {} ({}): {}",
        report
            .delivery
            .paste_outcome()
            .map_or("cancelled", |o| o.as_str()),
        report.method,
        bundle.as_deref().unwrap_or("?"),
        class.describe(),
        serde_json::to_string(&report).unwrap_or_default()
    );

    if let Some(pending) = pending {
        let spawned = std::thread::Builder::new()
            .name("fork-clipboard-restore".into())
            .spawn(move || {
                let settled = deliver::settle(&platform, &LINE, pending, &Timing::R5);
                log::info!(
                    "insert: take {take} clipboard {}: {}",
                    settled.decision,
                    serde_json::to_string(&settled).unwrap_or_default()
                );
            });
        if let Err(e) = spawned {
            // Without the waiter the transcript stays on the clipboard
            // (concealed); the next take's guard carries the snapshot.
            log::error!("insert: could not start the restore thread: {e}");
        }
    }
    publish_outcome(&app, take, report.delivery, &text, kind);
    (
        hook_result(report.delivery),
        report.delivery.paste_outcome(),
    )
}

/// S-21: one ⌘V into the element captured when the transform was invoked.
/// Focus is checked before Select All, again before the paste, and by the
/// delivery gate while it waits. There is no typing fallback and no retry.
pub(super) fn deliver_transform<R: tauri::Runtime>(
    app: AppHandle<R>,
    text: String,
    target: u64,
    select_all: bool,
    cancelled: &(dyn Fn() -> bool + Send + Sync),
) -> Result<deliver::Report, String> {
    let _slot = lock(&SLOT);
    let focus_problem = || {
        let focus = crate::fork::ax::focus_now(Some(target), crate::fork::ax::READ_BUDGET);
        match crate::fork::insert::target::plan(true, focus.as_ref()) {
            crate::fork::insert::target::Route::Card(reason) => Some(reason),
            crate::fork::insert::target::Route::Paste { .. } => None,
        }
    };
    if let Some(reason) = focus_problem() {
        return Ok(empty_transform_report(deliver::Delivery::Card { reason }));
    }
    if select_all {
        let platform = AppPlatform { app: app.clone() };
        platform.on_main(|| keys::post_command_letter(b'a'))?;
        if let Some(reason) = focus_problem() {
            return Ok(empty_transform_report(deliver::Delivery::Card { reason }));
        }
    }

    let (_, class) = frontmost_class();
    let mut req = Request::new(&text, class);
    req.method = deliver::Method::CommandV;
    let platform = AppPlatform { app: app.clone() };
    let focus_cancelled = || cancelled() || focus_problem().is_some();
    let no_interruption = || None;
    let gate = Gate {
        cancelled: &focus_cancelled,
        interrupted: &no_interruption,
    };
    let (mut report, pending) = deliver::paste(&platform, &LINE, &req, &gate);
    if report.delivery == deliver::Delivery::Cancelled && !cancelled() {
        report.delivery = deliver::Delivery::Card {
            reason: deliver::CardReason::FocusMoved,
        };
    }
    if let Some(pending) = pending {
        let spawned = std::thread::Builder::new()
            .name("fork-transform-clipboard-restore".into())
            .spawn(move || {
                let settled = deliver::settle(&platform, &LINE, pending, &Timing::R5);
                log::info!("transforms: clipboard {} (restore)", settled.decision);
            });
        if let Err(error) = spawned {
            // The paste already landed; the concealed transcript remains for
            // the next delivery's snapshot guard, exactly as for a take.
            log::error!("transforms: could not start the clipboard restore: {error}");
        }
    }
    Ok(report)
}

fn empty_transform_report(delivery: deliver::Delivery) -> deliver::Report {
    deliver::Report {
        delivery,
        method: deliver::Method::CommandV,
        class: TargetClass::Native,
        modifier_wait_ms: None,
        modifier_wait_timed_out: false,
        previous_read_wait_ms: None,
        snapshot_ms: None,
        snapshot_bytes: 0,
        skipped_types: 0,
        unreadable_types: 0,
        to_paste_ms: None,
        receipt_ms: None,
        verification: None,
        submitted: false,
        error: None,
    }
}

fn submit_flags(key: AutoSubmitKey) -> u64 {
    match key {
        AutoSubmitKey::Enter => 0,
        AutoSubmitKey::CtrlEnter => keys::CONTROL_FLAG,
        AutoSubmitKey::CmdEnter => keys::COMMAND_FLAG,
    }
}

// ---------------------------------------------------------------------------
// Target class

type ClassKey = (PathBuf, Option<SystemTime>);
static CLASSES: Mutex<Option<HashMap<ClassKey, TargetClass>>> = Mutex::new(None);

/// The frontmost app's bundle id and class. Cached per bundle path and
/// `Info.plist` time, so a browser update is noticed (reading an Electron
/// framework for its Chromium version takes a while, once).
fn frontmost_class() -> (Option<String>, TargetClass) {
    let Some(app) = NSWorkspace::sharedWorkspace().frontmostApplication() else {
        return (None, TargetClass::Native);
    };
    let bundle = app.bundleIdentifier().map(|s| s.to_string());
    let Some(path) = app
        .bundleURL()
        .and_then(|u| u.path())
        .map(|p| PathBuf::from(p.to_string()))
    else {
        return (bundle, TargetClass::Native);
    };
    let stamp = std::fs::metadata(path.join("Contents/Info.plist"))
        .and_then(|m| m.modified())
        .ok();
    let key = (path, stamp);
    if let Some(c) = lock(&CLASSES).as_ref().and_then(|m| m.get(&key)) {
        return (bundle, *c);
    }
    let class = target::classify(&key.0);
    lock(&CLASSES)
        .get_or_insert_with(HashMap::new)
        .insert(key, class);
    (bundle, class)
}

// ---------------------------------------------------------------------------
// Platform

#[derive(Clone)]
struct AppPlatform<R: tauri::Runtime> {
    app: AppHandle<R>,
}

impl<R: tauri::Runtime> AppPlatform<R> {
    /// Runs `f` on the main thread and waits for it (AppKit pasteboard
    /// writes with a data provider, and TIS for the ⌘V key code).
    fn on_main<T: Send + 'static>(
        &self,
        f: impl FnOnce() -> Result<T, String> + Send + 'static,
    ) -> Result<T, String> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.app
            .run_on_main_thread(move || {
                let _ = tx.send(f());
            })
            .map_err(|e| format!("could not reach the main thread: {e}"))?;
        rx.recv()
            .map_err(|_| "the main thread dropped the request".to_string())?
    }
}

impl PublishedText for nspb::Published {
    fn change_count(&self) -> i64 {
        self.change_count
    }
    fn reads(&self) -> Vec<Instant> {
        nspb::Published::reads(self)
    }
}

impl<R: tauri::Runtime> Platform for AppPlatform<R> {
    type Published = nspb::Published;

    fn now(&self) -> Instant {
        Instant::now()
    }
    fn wait(&self, d: Duration) {
        std::thread::sleep(d);
    }
    fn modifier_flags(&self) -> u64 {
        keys::flags_now()
    }
    fn change_count(&self) -> i64 {
        nspb::change_count()
    }
    /// Off the main thread on purpose: if our previous transcript is still
    /// on the clipboard, AppKit asks our provider for it on the main thread.
    fn read(&self) -> (Snapshot, Vec<String>) {
        nspb::read()
    }
    fn write(&self, snap: &Snapshot) -> Result<i64, String> {
        let snap = snap.clone();
        self.on_main(move || nspb::write(&snap))
    }
    fn write_plain_text(&self, text: &str) -> Result<i64, String> {
        let text = text.to_string();
        self.on_main(move || nspb::write_plain_text(&text))
    }
    fn publish(&self, text: &str) -> Result<nspb::Published, String> {
        let text = text.to_string();
        self.on_main(move || nspb::publish_transcript(&text, true))
    }
    fn post_paste(&self) -> Result<(), String> {
        self.on_main(|| keys::post_command_v().map(|_| ()))
    }
    fn type_text(&self, text: &str) -> Result<(), String> {
        keys::type_text(text)
    }
    fn post_return(&self, flags: u64) -> Result<(), String> {
        keys::post_return(flags)
    }
    fn field(&self) -> Option<FieldState> {
        verify::focused_field()
    }
}
