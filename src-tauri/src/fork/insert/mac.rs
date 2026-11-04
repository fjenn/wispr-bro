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
    let mut fail_open = false;
    // The text as pasted: with the smart leading space when the field is
    // read and the paste goes ahead. History, the card and S-34 keep `text`.
    let mut pasted = None;
    if cancelled() || interruption().is_some() {
        // `paste` reports the cancel or the interruption.
        if kind == Kind::Take {
            discard_target();
        }
    } else {
        match route(take, kind) {
            super::target::Route::Card(reason) => {
                let delivery = deliver::Delivery::Card { reason };
                let end = publish_outcome(&app, take, delivery, &text, kind);
                return (hook_result(delivery, end), delivery.paste_outcome());
            }
            super::target::Route::Paste { fail_open: f } => {
                fail_open = f;
                pasted = Some(super::with_leading_space(take, &text));
            }
        }
    }

    let (bundle, class) = frontmost_class();
    let mut req = Request::new(pasted.as_deref().unwrap_or(&text), class);
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
    // S-36: a take into an editor's chat input types its file tags through
    // the input's own `@` picker, so they become real tags.
    if kind == Kind::Take {
        req.mentions = chat_mentions(bundle.as_deref(), &req.text);
    }

    let platform = AppPlatform { app: app.clone() };
    let gate = Gate {
        cancelled: &cancelled,
        interrupted: &interruption,
    };
    let (report, mut pending) = deliver::paste(&platform, &LINE, &req, &gate);
    // OT2-A follow-up: AX could not tell whether a field had focus (a web
    // page, an Electron view, a list) and the field did not visibly change.
    // If nothing reads the ⌘V by the restore deadline, it landed nowhere:
    // the text stays on the clipboard and the bar says so.
    if fail_open
        && report.delivery
            == (deliver::Delivery::Pasted {
                outcome: deliver::PasteOutcome::Unverified,
            })
        && !text.trim().is_empty()
    {
        if let Some(p) = pending.as_mut() {
            p.copy_if_unread(text.clone());
        }
    }
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
                if settled.copied_unread {
                    log::info!(
                        "insert: take {take} copied to the clipboard (nothing read the paste)"
                    );
                    crate::fork::pill::show_message(
                        crate::fork::pill::model::PillMessage::CopiedToClipboard,
                    );
                }
            });
        if let Err(e) = spawned {
            // Without the waiter the transcript stays on the clipboard
            // (concealed); the next take's guard carries the snapshot.
            log::error!("insert: could not start the restore thread: {e}");
        }
    }
    let end = publish_outcome(&app, take, report.delivery, &text, kind);
    // S-34: a dictation left in the field (not sent by a Return) is watched
    // for the user's corrections when auto-learn is on.
    if kind == Kind::Take && report.delivery.inserted() && req.submit.is_none() {
        crate::fork::dictionary::learn::on_pasted(&app, &text);
    }
    (
        hook_result(report.delivery, end),
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
        match crate::fork::insert::target::plan_now(true, focus.as_ref()) {
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
        mention_tags: None,
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
// S-36: file tags in an editor's chat

/// The file names whose `@name` this take's text has and types through the
/// `@` picker (any file of the project, or one visible in the editor): only
/// when the take's context read found that editor's chat input focused and
/// that editor is still in front.
fn chat_mentions(bundle: Option<&str>, text: &str) -> Vec<String> {
    use crate::fork::context::{awareness, ide::Editor};
    let Some(editor) = awareness::chat_editor() else {
        return Vec::new();
    };
    if !editor.tags_files() || Editor::from_bundle(bundle) != Some(editor) {
        return Vec::new();
    }
    awareness::file_names()
        .map(|names| names.mentioned_in(text))
        .unwrap_or_default()
}

/// Whether the focused input's `@` picker highlights the file `name`. The
/// input names its open picker in `AXLinkedUIElements` (`aria-controls`:
/// Cursor 3.28's `typeahead-menu`); the highlighted entry is the selected
/// one, else the first (`typeahead-item-0`); one of its texts must be the
/// name exactly. Only that small subtree is read, never the field's text.
fn mention_offered(name: &str) -> bool {
    use crate::fork::ax::element as el;
    use objc2_application_services::AXUIElement;
    use objc2_core_foundation::{CFArray, CFRetained, CFType};

    let Some(app) = NSWorkspace::sharedWorkspace().frontmostApplication() else {
        return false;
    };
    let pid = app.processIdentifier();
    if pid <= 0 || crate::fork::ax::is_own_pid(pid) || !crate::fork::ax::accessibility_trusted() {
        return false;
    }
    // SAFETY: plain constructor.
    let app = unsafe { AXUIElement::new_application(pid) };
    el::set_timeout(&app, 0.05);
    let Some(field) = el::attr_element(&app, "AXFocusedUIElement") else {
        return false;
    };
    el::set_timeout(&field, 0.05);
    let elements = |e: &AXUIElement, attribute: &str| -> Vec<CFRetained<AXUIElement>> {
        el::attr(e, attribute)
            .ok()
            .and_then(|v| v.downcast::<CFArray>().ok())
            .map(|a| {
                // SAFETY: an array of CFTypes; every entry is checked.
                let a: &CFArray<CFType> = unsafe { a.cast_unchecked() };
                (0..a.len().min(64))
                    .filter_map(|i| a.get(i))
                    .filter_map(|v| v.downcast::<AXUIElement>().ok())
                    .collect()
            })
            .unwrap_or_default()
    };
    for menu in elements(&field, "AXLinkedUIElements").into_iter().take(4) {
        // The menu's entries: a bounded walk for `…-item-N` ids.
        let mut stack = vec![(menu, 0usize)];
        let mut visited = 0;
        let mut first = None;
        let mut selected = None;
        while let Some((e, depth)) = stack.pop() {
            visited += 1;
            if visited > 300 {
                break;
            }
            el::set_timeout(&e, 0.05);
            let id = el::attr_string(&e, "AXDOMIdentifier").unwrap_or_default();
            if id.ends_with("item-0") && first.is_none() {
                first = Some(e.clone());
            }
            if id.contains("item-") && el::attr_bool(&e, "AXSelected") == Some(true) {
                selected = Some(e.clone());
                break;
            }
            if depth < 12 {
                for child in elements(&e, "AXChildren").into_iter().rev() {
                    stack.push((child, depth + 1));
                }
            }
        }
        let Some(entry) = selected.or(first) else {
            continue;
        };
        let mut stack = vec![(entry, 0usize)];
        let mut visited = 0;
        while let Some((e, depth)) = stack.pop() {
            visited += 1;
            if visited > 60 {
                break;
            }
            el::set_timeout(&e, 0.05);
            if el::attr_string(&e, "AXRole").as_deref() == Some("AXStaticText")
                && el::attr_string(&e, "AXValue").is_some_and(|v| v.trim() == name)
            {
                return true;
            }
            if depth < 6 {
                for child in elements(&e, "AXChildren") {
                    stack.push((child, depth + 1));
                }
            }
        }
    }
    false
}

/// Live check against a running editor (Mac only, never in CI): see
/// `insert::mac::live_tests`.
#[cfg(test)]
pub(crate) fn mention_offered_for_test(name: &str) -> bool {
    mention_offered(name)
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
    fn mention_offered(&self, name: &str) -> bool {
        mention_offered(name)
    }
    fn post_backspace(&self) -> Result<(), String> {
        keys::post_key(keys::BACKSPACE_KEY_CODE)
    }
}

/// S-36 live check of the `@` picker path against a running editor whose
/// chat input has focus (never in CI: `#[ignore]`, and only when
/// `WB_S36_LIVE_PID` names the frontmost app, a throwaway editor instance).
/// `cargo test --lib live_mention -- --ignored --nocapture`
#[cfg(test)]
mod live_tests {
    use std::time::{Duration, Instant};

    use objc2_app_kit::NSWorkspace;

    use crate::fork::clipboard::keys;

    fn front_pid() -> i32 {
        NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .map_or(-1, |a| a.processIdentifier())
    }

    #[test]
    #[ignore = "needs a throwaway editor in front (WB_S36_LIVE_PID)"]
    fn live_mention_through_the_picker() {
        let Some(pid) = std::env::var("WB_S36_LIVE_PID")
            .ok()
            .and_then(|p| p.parse::<i32>().ok())
        else {
            eprintln!("WB_S36_LIVE_PID not set: skipped");
            return;
        };
        let name = std::env::var("WB_S36_LIVE_FILE").unwrap_or_else(|_| "user_service.ts".into());
        assert_eq!(front_pid(), pid, "the throwaway editor must be in front");
        assert!(!super::mention_offered_for_test(&name), "no picker yet");
        keys::type_text("check ").unwrap();
        for c in format!("@{name}").chars() {
            // Keys only ever go to the throwaway editor.
            assert_eq!(front_pid(), pid, "the throwaway editor left the front");
            keys::type_text(&c.to_string()).unwrap();
            std::thread::sleep(crate::fork::insert::deliver::MENTION_KEY_GAP);
        }
        let start = Instant::now();
        let mut offered = false;
        while start.elapsed() < Duration::from_millis(1_500) {
            if super::mention_offered_for_test(&name) {
                offered = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        eprintln!("offered after {:?}: {offered}", start.elapsed());
        assert!(!super::mention_offered_for_test("not-a-visible-file.rs"));
        assert!(offered, "the picker offered {name}");
        assert_eq!(front_pid(), pid);
        keys::post_return(0).unwrap();
        std::thread::sleep(Duration::from_millis(80));
        keys::post_key(keys::BACKSPACE_KEY_CODE).unwrap();
        keys::type_text(", please.").unwrap();
    }
}
