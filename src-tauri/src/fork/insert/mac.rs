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
use super::{hook_result, interruption, lock, next_take, publish_outcome, LINE, SLOT};
use crate::fork::clipboard::restore::{TargetClass, Timing};
use crate::fork::clipboard::snapshot::Snapshot;
use crate::fork::clipboard::{keys, nspb, target};
use crate::settings::{AppSettings, AutoSubmitKey, ClipboardHandling};

/// Delivers on the paste thread Handy spawned for this take.
pub(super) fn deliver(text: String, app: AppHandle, settings: &AppSettings) -> Result<(), String> {
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

    let (bundle, class) = frontmost_class();
    let mut req = Request::new(&text, class);
    req.submit = settings
        .auto_submit
        .then(|| submit_flags(settings.auto_submit_key));
    req.keep_transcript = settings.clipboard_handling == ClipboardHandling::CopyToClipboard;

    let platform = AppPlatform { app: app.clone() };
    let gate = Gate {
        cancelled: &cancelled,
        interrupted: &interruption,
    };
    let (report, pending) = deliver::paste(&platform, &LINE, &req, &gate);
    log::info!(
        "insert: take {take} {} via {:?} into {} ({}): {}",
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
    publish_outcome(&app, take, report.delivery, &text);
    hook_result(report.delivery)
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
struct AppPlatform {
    app: AppHandle,
}

impl AppPlatform {
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

impl Platform for AppPlatform {
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
