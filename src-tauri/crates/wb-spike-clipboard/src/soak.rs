// SPDX-License-Identifier: GPL-3.0-or-later
//! S-04 acceptance run (R5 S-04 "Real-Mac acceptance", §5.4): takes across
//! the ten apps through the product's `fork::insert` engine (compiled into
//! this tool from `src-tauri/src/fork/insert/` with `#[path]`), unattended.
//!
//! Each take puts one of the six RS-05 sources on the clipboard, delivers a
//! numbered text with `deliver::paste` + `deliver::settle` exactly as the app
//! does (same `Request::new`, so Chromium 155/156 targets are typed), then
//! compares the clipboard with the source byte for byte. After each app the
//! field's text is read back and every token counted: exactly once means no
//! lost and no duplicated insertion; "WB-SOURCE" in the field means the old
//! clipboard was pasted.
//!
//! Safety: the owner's clipboard is saved first (and to a 0600 file) and
//! put back at the end; the run refuses to start if the pointer moved in
//! the last 3 s, and stops if it moves during the run (someone is using the
//! Mac). Only windows, tabs, notes and messages it opens itself are typed
//! into, and each is emptied or closed without saving or sending.

use serde::Serialize;
use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::apps::{self, Installed};
use crate::clip::keys;
use crate::clip::nspb;
use crate::clip::restore::{TargetClass, Timing};
use crate::clip::snapshot::{self, Snapshot};
use crate::insert::deliver::{self, Delivery, Gate, Line, Platform, PublishedText, Request};
use crate::insert::verify::{self, FieldState};
use crate::log::Log;
use crate::macos::{self, Front};
use crate::owner;
use crate::sources::{self, Kind, Source, SOURCE_MARK};

// ---------------------------------------------------------------------------
// The platform: the product's AppKit calls, on this (main) thread, with
// waits that pump the run loop so our lazy text can be served.

struct SoakPlatform;

impl PublishedText for nspb::Published {
    fn change_count(&self) -> i64 {
        self.change_count
    }
    fn reads(&self) -> Vec<Instant> {
        nspb::Published::reads(self)
    }
}

impl Platform for SoakPlatform {
    type Published = nspb::Published;
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn wait(&self, d: Duration) {
        macos::pump(d)
    }
    fn modifier_flags(&self) -> u64 {
        keys::flags_now()
    }
    fn change_count(&self) -> i64 {
        nspb::change_count()
    }
    fn read(&self) -> (Snapshot, Vec<String>) {
        nspb::read()
    }
    fn write(&self, snap: &Snapshot) -> Result<i64, String> {
        nspb::write(snap)
    }
    fn write_plain_text(&self, text: &str) -> Result<i64, String> {
        nspb::write_plain_text(text)
    }
    fn publish(&self, text: &str) -> Result<nspb::Published, String> {
        nspb::publish_transcript(text, true)
    }
    fn post_paste(&self) -> Result<(), String> {
        keys::post_command_v().map(|_| ())
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

// ---------------------------------------------------------------------------
// The apps

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Read {
    /// The focused field's AXValue.
    Field,
    /// The test page mirrors its text box into the window title.
    Title,
    /// Terminal's `cat` writes what it receives to a file.
    CatFile,
    /// Save the document (⌘S) and read the file.
    SaveFile,
    /// Select all and copy in the field, then read the clipboard.
    CopyBack,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct SoakApp {
    key: &'static str,
    /// Stands in for this §5.4 app.
    for_r5: &'static str,
    read: Read,
}

const SOAK_APPS: &[SoakApp] = &[
    SoakApp {
        key: "textedit",
        for_r5: "TextEdit",
        read: Read::Field,
    },
    SoakApp {
        key: "notes",
        for_r5: "Notes",
        read: Read::Field,
    },
    SoakApp {
        key: "mail",
        for_r5: "Mail",
        read: Read::CopyBack,
    },
    SoakApp {
        key: "messages",
        for_r5: "Messages",
        read: Read::Field,
    },
    SoakApp {
        key: "terminal",
        for_r5: "Terminal",
        read: Read::CatFile,
    },
    SoakApp {
        key: "safari",
        for_r5: "Safari",
        read: Read::Title,
    },
    SoakApp {
        key: "chrome",
        for_r5: "Chrome Gmail (local page)",
        read: Read::Title,
    },
    SoakApp {
        key: "cursor",
        for_r5: "VS Code",
        read: Read::SaveFile,
    },
    SoakApp {
        key: "claude",
        for_r5: "Slack (Electron chat box)",
        read: Read::Field,
    },
    SoakApp {
        key: "chatgpt",
        for_r5: "Notion (Chromium app)",
        read: Read::CopyBack,
    },
];

// ---------------------------------------------------------------------------
// Results

#[derive(Clone, Debug, Serialize)]
struct TakeRow {
    n: usize,
    token: String,
    source: Kind,
    report: deliver::Report,
    settled: Option<deliver::Settled>,
    /// Clipboard after the take compared with the source.
    restore: String,
    restore_identical: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
struct AppRow {
    key: String,
    name: String,
    version: Option<String>,
    class: String,
    for_r5: String,
    status: String,
    note: Option<String>,
    takes: Vec<TakeRow>,
    /// Pasted tokens found exactly once / missing / more than once.
    once: usize,
    lost: Vec<String>,
    duplicated: Vec<String>,
    old_clipboard_pasted: bool,
    read_back: Option<String>,
    /// Times another app took the front during the run and the soak
    /// brought the target back before the next take.
    refocused: usize,
}

#[derive(Debug, Default, Serialize)]
struct Summary {
    tool: &'static str,
    started: String,
    git_sha: String,
    per_app: usize,
    apps: Vec<AppRow>,
    owner_clipboard_restored: Option<bool>,
}

// ---------------------------------------------------------------------------

pub struct SoakOptions {
    pub out: Option<PathBuf>,
    pub per_app: usize,
    pub only: Vec<String>,
    pub keep_open: bool,
}

fn hid_idle_secs() -> Option<u64> {
    let out = std::process::Command::new("/usr/sbin/ioreg")
        .args(["-c", "IOHIDSystem", "-d", "4"])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let line = s.lines().find(|l| l.contains("\"HIDIdleTime\""))?;
    let ns: u64 = line.rsplit('=').next()?.trim().parse().ok()?;
    Some(ns / 1_000_000_000)
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn CGEventCreate(source: *const std::ffi::c_void) -> *mut std::ffi::c_void;
    fn CGEventGetLocation(e: *mut std::ffi::c_void) -> CGPoint;
}
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(v: *const std::ffi::c_void);
}

fn pointer() -> CGPoint {
    // SAFETY: a null-source event only to read the pointer; released here.
    unsafe {
        let e = CGEventCreate(std::ptr::null());
        if e.is_null() {
            return CGPoint::default();
        }
        let p = CGEventGetLocation(e);
        CFRelease(e.cast_const());
        p
    }
}

struct Ctx {
    dir: PathBuf,
    log: Log,
    line: Mutex<Line>,
    sources: Vec<Source>,
    token: usize,
    pointer: CGPoint,
    stop: Option<String>,
}

impl Ctx {
    fn ev(&self, kind: &str, v: serde_json::Value) {
        self.log.write(kind, Instant::now(), v);
    }

    /// Someone moved the pointer since our last own click: stop.
    fn owner_active(&mut self) -> bool {
        let p = pointer();
        let moved = (p.x - self.pointer.x).abs() > 2.0 || (p.y - self.pointer.y).abs() > 2.0;
        if moved && self.stop.is_none() {
            self.stop = Some("the pointer moved: someone is using the Mac".into());
        }
        self.stop.is_some() || owner::interrupted()
    }

    fn click(&mut self, pid: i32, frac_y: f64) -> bool {
        let ok = macos::click_in_window(pid, frac_y);
        self.pointer = pointer();
        ok
    }
}

pub fn run(opts: SoakOptions) -> i32 {
    let root = opts.out.clone().unwrap_or_else(|| {
        PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
            .join("wb-bench/results/s04")
    });
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let dir = root.join(&stamp);
    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!("Could not create {}: {e}", dir.display());
        return 1;
    }
    if !macos::accessibility_trusted() || !macos::can_post_events() {
        eprintln!("This tool needs Accessibility (to read fields and post keys).");
        return 1;
    }
    // Someone at the Mac moves the pointer. (HID idle time is no use here:
    // the tool's own launcher resets it.)
    let p0 = pointer();
    macos::pump(Duration::from_secs(3));
    let p1 = pointer();
    if (p0.x - p1.x).abs() > 2.0 || (p0.y - p1.y).abs() > 2.0 {
        eprintln!(
            "The pointer moved in the last 3 s (HID idle {:?} s): someone is using the Mac. Not started.",
            hid_idle_secs()
        );
        return 3;
    }
    let log = match Log::create(&dir.join("events.jsonl"), Instant::now()) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Could not create the log: {e}");
            return 1;
        }
    };
    owner::install_signals(|| {});
    if let Err(e) = owner::save(&dir) {
        eprintln!("Could not save the clipboard first ({e}); stopping without touching it.");
        return 1;
    }
    let files = match sources::make_files(&dir) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Could not create the source files: {e}");
            let _ = owner::restore();
            return 1;
        }
    };
    let mut ctx = Ctx {
        dir: dir.clone(),
        log,
        line: Mutex::new(Line::new()),
        sources: Kind::MATRIX
            .iter()
            .map(|k| sources::synthetic(*k, &files))
            .collect(),
        token: 0,
        pointer: pointer(),
        stop: None,
    };
    let home = macos::frontmost();
    let mut summary = Summary {
        tool: "wb-spike-clipboard --s04-soak",
        started: chrono::Local::now().to_rfc3339(),
        git_sha: option_env!("WB_GIT_SHA").unwrap_or("unknown").into(),
        per_app: opts.per_app,
        ..Summary::default()
    };
    println!(
        "S-04 soak: {} takes per app; results in {}",
        opts.per_app,
        dir.display()
    );
    for app in SOAK_APPS {
        if !opts.only.is_empty() && !opts.only.iter().any(|o| o == app.key) {
            continue;
        }
        if ctx.owner_active() {
            break;
        }
        let Some(spec) = apps::APPS.iter().find(|s| s.key == app.key) else {
            continue;
        };
        let Some(inst) = apps::installed(spec) else {
            summary.apps.push(AppRow {
                key: app.key.into(),
                status: "not_installed".into(),
                ..AppRow::default()
            });
            continue;
        };
        let row = run_app(&mut ctx, app, &inst, opts.per_app, opts.keep_open);
        print_row(&row);
        ctx.ev("app_done", json!({ "app": row }));
        summary.apps.push(row);
        if let Some(h) = &home {
            macos::open(&["-b", &h.bundle_id]);
            macos::pump(Duration::from_millis(400));
        }
    }
    if let Some(why) = &ctx.stop {
        println!("Stopped early: {why}");
    }
    let left = leftovers();
    if !left.is_empty() {
        println!("Left open (close by hand): {left:?}");
    }
    ctx.ev("leftovers", json!({ "windows": left }));
    let restored = owner::restore();
    summary.owner_clipboard_restored = restored.as_ref().map(|r| r.identical);
    println!(
        "Owner's clipboard put back: {}",
        match &restored {
            Some(r) if r.identical => "exactly".to_string(),
            Some(r) => format!("NOT exactly: {}", r.detail),
            None => "nothing saved?".into(),
        }
    );
    let totals = totals(&summary);
    println!("{totals}");
    let _ = fs::write(
        dir.join("summary.json"),
        serde_json::to_string_pretty(&summary).unwrap_or_default(),
    );
    let _ = fs::write(dir.join("summary.txt"), &totals);
    i32::from(ctx.stop.is_some())
}

/// Closes (⌘W, never saving) every window of ours still open in the browsers
/// and TextEdit; returns what is still left.
pub fn close_leftovers() -> Vec<String> {
    for key in ["safari", "chrome", "textedit"] {
        let Some(inst) = apps::APPS
            .iter()
            .find(|s| s.key == key)
            .and_then(apps::installed)
        else {
            continue;
        };
        let Some(pid) = macos::running_pid(&inst.bundle_id) else {
            continue;
        };
        for _ in 0..8 {
            let Some(t) = macos::window_titles(pid).into_iter().find(|t| is_ours(t)) else {
                break;
            };
            macos::open(&["-b", &inst.bundle_id]);
            macos::pump(Duration::from_millis(500));
            macos::raise_window(pid, &t);
            macos::pump(Duration::from_millis(400));
            let front = macos::focus(pid).window_title.unwrap_or_default();
            if macos::frontmost().is_some_and(|f| f.pid == pid) && is_ours(&front) {
                cmd(b'w');
                macos::pump(Duration::from_millis(500));
            } else {
                break;
            }
        }
    }
    leftovers()
}

/// Windows of ours still open in any of the ten apps.
/// Whether `text` holds nothing but soak tokens (and spaces).
fn only_tokens(text: &str) -> bool {
    let mut rest = text.to_string();
    while let Some(i) = rest.find("[S-04 soak ") {
        match rest[i..].find(']') {
            Some(j) => rest.replace_range(i..i + j + 1, ""),
            None => return false,
        }
    }
    rest.trim().is_empty() && text.contains("[S-04 soak ")
}

/// Cleanup after a run went wrong: in the front window of `bundle`, select
/// all and copy; if the selection is nothing but soak tokens, delete it.
/// The owner's clipboard is saved and put back around it.
pub fn clear_field(bundle: &str) -> i32 {
    let dir = std::env::temp_dir().join(format!("wb-s04-clear-{}", std::process::id()));
    let _ = fs::create_dir_all(&dir);
    if owner::save(&dir).is_err() {
        eprintln!("could not save the clipboard; not touching anything");
        return 1;
    }
    macos::open(&["-b", bundle]);
    let code = match macos::wait_front(bundle, Duration::from_secs(10)) {
        None => {
            eprintln!("{bundle} did not come to the front");
            1
        }
        Some(_) => {
            macos::pump(Duration::from_millis(800));
            nspb::write(&Snapshot::default()).ok();
            cmd(b'a');
            cmd(b'c');
            macos::pump(Duration::from_millis(300));
            let (snap, _) = nspb::read();
            let text = snap
                .items
                .first()
                .and_then(|i| i.get(sources::TEXT))
                .map(|b| String::from_utf8_lossy(b).into_owned());
            match text {
                Some(t) if only_tokens(&t) => {
                    key(DELETE);
                    println!("cleared {:?}", t);
                    0
                }
                Some(t) => {
                    // Click away the selection without changing anything.
                    key(124); // right arrow
                    println!(
                        "not only soak text ({} chars); left alone",
                        t.chars().count()
                    );
                    2
                }
                None => {
                    println!("nothing selected/copied; left alone");
                    2
                }
            }
        }
    };
    let r = owner::restore();
    println!(
        "clipboard put back: {}",
        r.map(|r| r.identical).unwrap_or(false)
    );
    let _ = fs::remove_dir_all(&dir);
    code
}

/// The soak tokens (with their spaces) at the end of `text`: how many
/// characters to delete to get back what was there before the run.
fn token_tail(text: &str) -> usize {
    let mut rest = text;
    let mut n = 0;
    loop {
        let t = rest.trim_end_matches(' ');
        let spaces = rest.len() - t.len();
        let Some(start) = t.rfind("[S-04 soak ") else {
            break;
        };
        if !t.ends_with(']') || t[start..].matches(']').count() != 1 {
            break;
        }
        n += spaces + t[start..].chars().count();
        rest = &t[..start];
    }
    if n == 0 {
        0
    } else {
        n
    }
}

/// Cleanup when soak text landed after someone's own draft: in the focused
/// field of `bundle`, delete only the trailing soak tokens (checked through
/// Accessibility before and after).
pub fn strip_tail(bundle: &str) -> i32 {
    macos::open(&["-b", bundle]);
    let Some(front) = macos::wait_front(bundle, Duration::from_secs(10)) else {
        eprintln!("{bundle} did not come to the front");
        return 1;
    };
    macos::pump(Duration::from_millis(800));
    // A field Accessibility cannot read (web areas, Chromium): select all
    // and copy instead, with the owner's clipboard saved around it.
    let readable = macos::focus(front.pid).value.is_some_and(|v| !v.is_empty());
    let result = if readable {
        strip_tokens_in_focus(front.pid)
    } else {
        let dir = std::env::temp_dir().join(format!("wb-s04-strip-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        if owner::save(&dir).is_err() {
            eprintln!("could not save the clipboard; not touching anything");
            return 1;
        }
        let r = strip_tokens_by_copy();
        let back = owner::restore().is_some_and(|r| r.identical);
        println!("clipboard put back: {back}");
        let _ = fs::remove_dir_all(&dir);
        r
    };
    match result {
        Ok(0) => {
            println!("no soak text at the end; left alone");
            0
        }
        Ok(n) => {
            println!("removed {n} chars of soak text; the rest is as before");
            0
        }
        Err(e) => {
            println!("CHECK BY HAND: {e}");
            2
        }
    }
}

fn copy_field() -> String {
    nspb::write(&Snapshot::default()).ok();
    cmd(b'a');
    cmd(b'c');
    macos::pump(Duration::from_millis(200));
    let (snap, _) = nspb::read();
    snap.items
        .first()
        .and_then(|i| i.get(sources::TEXT))
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .unwrap_or_default()
}

/// As [`strip_tokens_in_focus`] for a field Accessibility cannot read:
/// select all + copy before and after (the clipboard is the soak's own).
fn strip_tokens_by_copy() -> Result<usize, String> {
    let before = copy_field();
    let total = token_tail(&before);
    if total == 0 {
        key(124);
        return Ok(0);
    }
    let keep: String = before
        .chars()
        .take(before.chars().count() - total)
        .collect();
    for _ in 0..4 {
        let now = copy_field();
        if now == keep {
            key(124);
            return Ok(total);
        }
        let n = now.chars().count().saturating_sub(keep.chars().count());
        if !now.starts_with(&keep) || n == 0 {
            key(124);
            return Err(format!(
                "the field changed unexpectedly ({} chars, expected {})",
                now.chars().count(),
                keep.chars().count()
            ));
        }
        // Right arrow ends the selection at the end of the text.
        key(124);
        let _ = keys::post_command_key(125);
        macos::pump(Duration::from_millis(150));
        for _ in 0..n {
            let _ = keys::post_key(DELETE);
            macos::pump(Duration::from_millis(80));
        }
        macos::pump(Duration::from_millis(400));
    }
    Err("soak text still at the end of the field".into())
}

/// Deletes the soak tokens at the end of the focused field of `pid`, and
/// nothing else (Accessibility check before and after each pass).
fn strip_tokens_in_focus(pid: i32) -> Result<usize, String> {
    let Some(before) = macos::focus(pid).value else {
        return Err("the focused field has no readable value".into());
    };
    let total = token_tail(&before);
    if total == 0 {
        return Ok(0);
    }
    let keep: String = before
        .chars()
        .take(before.chars().count() - total)
        .collect();
    for _ in 0..4 {
        let now = macos::focus(pid).value.unwrap_or_default();
        if now == keep {
            return Ok(total);
        }
        // What is left of our tail (a pass can drop keys in Electron).
        let n = now.chars().count().saturating_sub(keep.chars().count());
        if !now.starts_with(&keep) || n == 0 {
            return Err(format!(
                "the field changed unexpectedly ({} chars, expected {})",
                now.chars().count(),
                keep.chars().count()
            ));
        }
        // ⌘↓: caret to the end of the field.
        let _ = keys::post_command_key(125);
        macos::pump(Duration::from_millis(200));
        for _ in 0..n {
            let _ = keys::post_key(DELETE);
            macos::pump(Duration::from_millis(80));
        }
        macos::pump(Duration::from_millis(400));
    }
    Err("soak text still at the end of the field".into())
}

// ---------------------------------------------------------------------------
// S-04.5: paste with the Japanese input method on (plain and mid-word)

const IME_ID: &str = "com.apple.inputmethod.Kotoeri.RomajiTyping.Japanese";

fn saved_input_sources() -> String {
    std::process::Command::new("/usr/bin/defaults")
        .args(["read", "com.apple.HIToolbox", "AppleEnabledInputSources"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

/// Two takes into a fresh TextEdit document with Japanese (Romaji) on: one
/// plain, one while "nihon" is being composed. The input sources are put
/// back as they were (the tool turns off whatever it turned on).
pub fn ime_check() -> i32 {
    let dir = std::env::temp_dir().join(format!("wb-s04-ime-{}", std::process::id()));
    let _ = fs::create_dir_all(&dir);
    owner::install_signals(|| {});
    if owner::save(&dir).is_err() {
        eprintln!("could not save the clipboard; not started");
        return 1;
    }
    let before_ids: Vec<String> = macos::input_sources(false)
        .into_iter()
        .map(|s| s.id)
        .collect();
    let saved_before = saved_input_sources();
    let original = macos::current_input_source_id();
    let installed = macos::input_sources(true);
    let parent = IME_ID.rsplit_once('.').map(|(p, _)| p).unwrap_or(IME_ID);
    for want in [parent, IME_ID] {
        if !before_ids.iter().any(|b| b == want) {
            if let Some(s) = installed.iter().find(|s| s.id == want) {
                s.enable();
            }
        }
    }
    macos::pump(Duration::from_millis(500));
    let f = dir.join("wb-s04-ime.txt");
    let _ = fs::write(&f, b"");
    macos::open(&["-a", "TextEdit", &f.to_string_lossy()]);
    let mut code = 1;
    if let Some(front) = macos::wait_front("com.apple.TextEdit", Duration::from_secs(10)) {
        if wait_role(front.pid, &["AXTextArea"], Duration::from_secs(8)).is_some()
            && macos::focus(front.pid)
                .window_title
                .is_some_and(|t| t.contains("wb-s04-ime"))
        {
            let selected = macos::input_sources(false)
                .into_iter()
                .find(|s| s.id == IME_ID)
                .is_some_and(|s| s.select());
            macos::pump(Duration::from_millis(500));
            println!(
                "input source: {} (selected {selected})",
                macos::current_input_source_id()
            );
            let line = Mutex::new(Line::new());
            let open = || false;
            let none = || None;
            let gate = Gate {
                cancelled: &open,
                interrupted: &none,
            };
            let mut ok = 0;
            for (i, composing) in [false, true].into_iter().enumerate() {
                if composing {
                    // n i h o n (ANSI positions) as a word being composed.
                    for k in [45u16, 34, 4, 31, 45] {
                        let _ = keys::post_key(k);
                        macos::pump(Duration::from_millis(60));
                    }
                    macos::pump(Duration::from_millis(300));
                }
                let token = format!("[S-04 ime {}]", i + 1);
                let text = format!("{token} ");
                let (r, pending) = deliver::paste(
                    &SoakPlatform,
                    &line,
                    &Request::new(&text, TargetClass::Native),
                    &gate,
                );
                if let Some(p) = pending {
                    deliver::settle(&SoakPlatform, &line, p, &Timing::R5);
                }
                macos::pump(Duration::from_millis(400));
                let value = macos::focus(front.pid).value.unwrap_or_default();
                let landed = value.matches(token.as_str()).count();
                let tail: String = value
                    .chars()
                    .rev()
                    .take(40)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                println!(
                    "{}: {:?}, verification {:?}, in the field {landed}× (tail {tail:?})",
                    if composing {
                        "while composing"
                    } else {
                        "plain"
                    },
                    r.delivery,
                    r.verification
                );
                if landed == 1 {
                    ok += 1;
                }
                if composing {
                    let _ = keys::post_key(53); // Esc: drop any half-typed word
                }
                macos::pump(Duration::from_millis(300));
            }
            code = i32::from(ok != 2);
            cmd(b'w');
        }
    }
    // Input sources back as they were.
    if let Some(s) = macos::input_sources(false)
        .into_iter()
        .find(|s| s.id == original)
    {
        s.select();
    }
    macos::pump(Duration::from_millis(300));
    for _ in 0..2 {
        let all = macos::input_sources(true);
        for s in macos::input_sources(false) {
            if !before_ids.contains(&s.id) {
                if let Some(x) = all.iter().find(|x| x.id == s.id) {
                    x.disable();
                }
            }
        }
        macos::pump(Duration::from_millis(300));
    }
    let same = saved_input_sources() == saved_before;
    println!(
        "input sources restored: {same} (now {})",
        macos::current_input_source_id()
    );
    let back = owner::restore().is_some_and(|r| r.identical);
    println!("clipboard put back: {back}");
    let _ = fs::remove_dir_all(&dir);
    if same {
        code
    } else {
        2
    }
}

// ---------------------------------------------------------------------------
// S-04.3 / S-04.4 wiring: the product's session observer and the engine's
// gate with real NSWorkspace notifications (posted in this process: the
// tool cannot put the Mac to sleep and wake it, or switch users, unattended)

pub fn session_check() -> i32 {
    use crate::insert::session::{mac, Interruption, SessionLog, Signal};
    use objc2_app_kit::{
        NSWorkspace, NSWorkspaceDidWakeNotification, NSWorkspaceSessionDidBecomeActiveNotification,
        NSWorkspaceSessionDidResignActiveNotification, NSWorkspaceWillSleepNotification,
    };
    static LOG: Mutex<SessionLog> = Mutex::new(SessionLog::new());
    static SEEN: Mutex<Vec<String>> = Mutex::new(Vec::new());
    mac::observe(|s: Signal| {
        if let Ok(mut l) = LOG.lock() {
            l.signal(s, Instant::now());
        }
        if let Ok(mut v) = SEEN.lock() {
            v.push(format!("{s:?}"));
        }
    });
    let center = NSWorkspace::sharedWorkspace().notificationCenter();
    let post = |name: &objc2_foundation::NSNotificationName| {
        // SAFETY: posting a named notification with no object.
        unsafe { center.postNotificationName_object(name, None) };
        macos::pump(Duration::from_millis(100));
    };
    let verdict = || LOG.lock().ok().and_then(|l| l.verdict());
    let cc = nspb::change_count();
    let line = Mutex::new(Line::new());
    let open = || false;
    let deliver_now = |label: &str| {
        let interrupted = || verdict().map(Interruption::card_reason);
        let gate = Gate {
            cancelled: &open,
            interrupted: &interrupted,
        };
        let (r, pending) = deliver::paste(
            &SoakPlatform,
            &line,
            &Request::new("[S-04 session] ", TargetClass::Native),
            &gate,
        );
        let untouched = pending.is_none() && nspb::change_count() == cc;
        println!("{label}: {:?}; clipboard untouched {untouched}", r.delivery);
        matches!(r.delivery, Delivery::Card { .. }) && untouched
    };
    let mut pass = 0;
    // 1. Sleep during recording, wake, release: card.
    LOG.lock().unwrap().take_started(Instant::now());
    // SAFETY: framework statics.
    post(unsafe { NSWorkspaceWillSleepNotification });
    pass += usize::from(deliver_now("asleep (lid closed) during the take"));
    post(unsafe { NSWorkspaceDidWakeNotification });
    LOG.lock().unwrap().take_stopped(Instant::now());
    pass += usize::from(deliver_now("woke, then the take ended"));
    // 2. User switch while processing: card.
    LOG.lock().unwrap().take_started(Instant::now());
    LOG.lock().unwrap().take_stopped(Instant::now());
    post(unsafe { NSWorkspaceSessionDidResignActiveNotification });
    post(unsafe { NSWorkspaceSessionDidBecomeActiveNotification });
    pass += usize::from(deliver_now("user switched away and back while processing"));
    // 3. A quiet take after that: not interrupted.
    LOG.lock().unwrap().take_started(Instant::now());
    LOG.lock().unwrap().take_stopped(Instant::now());
    let quiet = verdict().is_none();
    println!("a later quiet take: interrupted {:?}", verdict());
    pass += usize::from(quiet);
    println!(
        "signals seen: {:?}",
        SEEN.lock().map(|v| v.clone()).unwrap_or_default()
    );
    println!("{pass}/4 as expected");
    i32::from(pass != 4)
}

/// Waits until nobody (a person or another test tool) has moved the
/// pointer or changed the front app for `quiet` seconds; gives up after
/// `max` seconds. Returns 0 when quiet.
pub fn wait_quiet(quiet: u64, max: u64) -> i32 {
    let start = Instant::now();
    let mut last_change = Instant::now();
    let mut p = pointer();
    let mut front = macos::frontmost().map(|f| f.bundle_id);
    while start.elapsed() < Duration::from_secs(max) {
        macos::pump(Duration::from_millis(500));
        let np = pointer();
        let nf = macos::frontmost().map(|f| f.bundle_id);
        if (np.x - p.x).abs() > 2.0 || (np.y - p.y).abs() > 2.0 || nf != front {
            last_change = Instant::now();
            p = np;
            front = nf;
        }
        if last_change.elapsed() >= Duration::from_secs(quiet) {
            println!("quiet for {quiet} s (front: {front:?})");
            return 0;
        }
    }
    println!("never quiet for {quiet} s within {max} s");
    1
}

pub fn leftovers() -> Vec<String> {
    let mut out = Vec::new();
    for app in SOAK_APPS {
        let Some(inst) = apps::APPS
            .iter()
            .find(|s| s.key == app.key)
            .and_then(apps::installed)
        else {
            continue;
        };
        let Some(pid) = macos::running_pid(&inst.bundle_id) else {
            continue;
        };
        for t in macos::window_titles(pid) {
            if is_ours(&t) {
                out.push(format!("{}: {t}", inst.name));
            }
        }
    }
    out
}

fn totals(s: &Summary) -> String {
    let takes: usize = s.apps.iter().map(|a| a.takes.len()).sum();
    let pasted: usize = s
        .apps
        .iter()
        .flat_map(|a| &a.takes)
        .filter(|t| t.report.delivery.inserted())
        .count();
    let confirmed = s
        .apps
        .iter()
        .flat_map(|a| &a.takes)
        .filter(|t| {
            t.report.delivery
                == Delivery::Pasted {
                    outcome: deliver::PasteOutcome::Confirmed,
                }
        })
        .count();
    let once: usize = s.apps.iter().map(|a| a.once).sum();
    let lost: usize = s.apps.iter().map(|a| a.lost.len()).sum();
    let dup: usize = s.apps.iter().map(|a| a.duplicated.len()).sum();
    let restores = s
        .apps
        .iter()
        .flat_map(|a| &a.takes)
        .filter(|t| t.restore_identical)
        .count();
    let old = s.apps.iter().filter(|a| a.old_clipboard_pasted).count();
    let mut to_paste: Vec<f64> = s
        .apps
        .iter()
        .flat_map(|a| &a.takes)
        .filter_map(|t| t.report.to_paste_ms)
        .collect();
    to_paste.sort_by(f64::total_cmp);
    let mut restore_after: Vec<f64> = s
        .apps
        .iter()
        .flat_map(|a| &a.takes)
        .filter_map(|t| t.settled.as_ref()?.restore_after_ms)
        .collect();
    restore_after.sort_by(f64::total_cmp);
    let pct = |v: &[f64], q: f64| {
        v.get(((v.len() as f64 - 1.0) * q).round() as usize)
            .copied()
            .unwrap_or(f64::NAN)
    };
    format!(
        "takes {takes}: pasted {pasted} (confirmed {confirmed}, unverified {}), in the field exactly once {once}, lost {lost}, duplicated {dup}; \
         clipboard byte-identical after {restores}/{takes}; old clipboard pasted in {old} app(s); \
         start→paste p50 {:.1} ms p95 {:.1} ms; ⌘V→restore p50 {:.1} ms p95 {:.1} ms (n={})",
        pasted - confirmed,
        pct(&to_paste, 0.5),
        pct(&to_paste, 0.95),
        pct(&restore_after, 0.5),
        pct(&restore_after, 0.95),
        restore_after.len()
    )
}

fn print_row(r: &AppRow) {
    let conf = r
        .takes
        .iter()
        .filter(|t| t.report.verification.is_some_and(|v| v.confirmed()))
        .count();
    let ident = r.takes.iter().filter(|t| t.restore_identical).count();
    println!(
        "  {:<9} {:<26} {:<10} takes {:>2}  once {:>2}  lost {}  dup {}  confirmed {:>2}  restore-identical {:>2}  refocused {}{}{}",
        r.key,
        r.class,
        r.status,
        r.takes.len(),
        r.once,
        r.lost.len(),
        r.duplicated.len(),
        conf,
        ident,
        r.refocused,
        if r.old_clipboard_pasted { "  OLD CLIPBOARD PASTED" } else { "" },
        r.note.as_deref().map(|n| format!("  ({n})")).unwrap_or_default()
    );
}

// ---------------------------------------------------------------------------
// One app

fn cmd(letter: u8) {
    let _ = keys::post_command_letter(letter);
    macos::pump(Duration::from_millis(250));
}

fn key(code: u16) {
    let _ = keys::post_key(code);
    macos::pump(Duration::from_millis(150));
}

const TAB: u16 = 48;
/// Window titles of ours: the test page (before and after typing), the
/// TextEdit / Cursor files, the Terminal target.
const OURS: [&str; 5] = [
    "paste test",
    "WBTEST",
    "wb-s04-target",
    "wb-paste-target",
    "S-04 soak",
];

fn is_ours(title: &str) -> bool {
    OURS.iter().any(|n| title.contains(n))
}
const DELETE: u16 = 51;

fn wait_role(pid: i32, roles: &[&str], timeout: Duration) -> Option<String> {
    let end = Instant::now() + timeout;
    while Instant::now() < end {
        let f = macos::focus(pid);
        if let Some(r) = f.role.as_deref() {
            if roles.contains(&r) {
                return Some(r.to_string());
            }
        }
        macos::pump(Duration::from_millis(100));
    }
    None
}

struct Opened {
    front: Front,
    /// The field's text when we got it (apps whose field Accessibility can
    /// read): before each take it must be this plus our tokens so far.
    initial: Option<String>,
    /// Terminal `cat` output, or the Cursor file.
    file: Option<PathBuf>,
    pidfile: Option<PathBuf>,
}

fn open_app(ctx: &mut Ctx, app: &SoakApp, inst: &Installed) -> Result<Opened, String> {
    let targets = ctx.dir.join("targets");
    fs::create_dir_all(&targets).map_err(|e| e.to_string())?;
    let path = inst.path.to_string_lossy().into_owned();
    let mut file = None;
    let mut pidfile = None;
    match app.key {
        "textedit" | "cursor" => {
            let f = targets.join(format!("wb-s04-target-{}.txt", app.key));
            fs::write(&f, b"").map_err(|e| e.to_string())?;
            macos::open(&["-a", &path, &f.to_string_lossy()]);
            file = Some(f);
        }
        "safari" | "chrome" => {
            let f = targets.join(format!("wb-s04-test-{}.html", app.key));
            fs::write(&f, apps::TEST_PAGE).map_err(|e| e.to_string())?;
            macos::open(&["-a", &path, &f.to_string_lossy()]);
        }
        "terminal" => {
            let out = targets.join("terminal-received.txt");
            let pid = targets.join("terminal-cat.pid");
            let script = targets.join("wb-paste-target.command");
            fs::write(&script, apps::terminal_script(&out, &pid)).map_err(|e| e.to_string())?;
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
            let _ = fs::remove_file(&out);
            macos::open(&["-a", &path, &script.to_string_lossy()]);
            file = Some(out);
            pidfile = Some(pid);
        }
        "mail" => {
            macos::open(&["-b", &inst.bundle_id, "mailto:"]);
        }
        _ => {
            macos::open(&["-a", &path]);
        }
    }
    let front = macos::wait_front(&inst.bundle_id, Duration::from_secs(20))
        .ok_or("the app did not come to the front")?;
    macos::pump(Duration::from_millis(1200));
    // Put the focus into a fresh, empty field we own.
    match app.key {
        "notes" => {
            cmd(b'n');
            wait_role(front.pid, &["AXTextArea"], Duration::from_secs(5)).ok_or("no new note")?;
        }
        "messages" => {
            cmd(b'n');
            macos::pump(Duration::from_millis(600));
            // New message: focus is in To: (empty, so nothing can be sent);
            // Tab moves to the message field.
            for _ in 0..3 {
                key(TAB);
                macos::pump(Duration::from_millis(400));
                if macos::focus_labels(front.pid).0.as_deref() == Some("Message") {
                    break;
                }
            }
            let f = macos::focus(front.pid);
            let labels = macos::focus_labels(front.pid);
            ctx.ev(
                "messages_focus",
                json!({"role": f.role, "value_len": f.value.as_ref().map(|v| v.len()), "labels": labels}),
            );
            // macOS 26: the message field is an AXTextField described
            // "Message" (the To: field is described "To:").
            let message_field = f.role.as_deref() == Some("AXTextArea")
                || (f.role.as_deref() == Some("AXTextField")
                    && labels.0.as_deref() == Some("Message"));
            if !message_field || f.value.is_some_and(|v| !v.is_empty()) {
                return Err(format!(
                    "not in an empty message field ({:?}, {labels:?})",
                    f.role
                ));
            }
        }
        "mail" => {
            macos::pump(Duration::from_millis(1500));
            ctx.click(front.pid, 0.75);
            macos::pump(Duration::from_millis(400));
        }
        "cursor" => {
            // Cursor 3 opens the file in the right-hand editor panel of its
            // Agents window, whose prompt box keeps the focus: click into
            // the editor, then make sure the focus is in our empty file
            // (select all + copy gives nothing but a line break) and not in
            // the prompt (which holds the owner's text).
            macos::pump(Duration::from_millis(2500));
            let pid = front.pid;
            ctx.pointer = pointer();
            macos::click_in_window_at(pid, 0.80, 0.16);
            ctx.pointer = pointer();
            macos::pump(Duration::from_millis(500));
            nspb::write(&Snapshot::default()).ok();
            cmd(b'a');
            cmd(b'c');
            let (snap, _) = nspb::read();
            let copied = snap
                .items
                .first()
                .and_then(|i| i.get(sources::TEXT))
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .unwrap_or_default();
            if !copied.trim().is_empty() {
                key(124); // right arrow: drop the selection, change nothing
                return Err(format!(
                    "the focus is not in the test file ({} chars selected)",
                    copied.chars().count()
                ));
            }
        }
        "claude" => {
            cmd(b'n');
            macos::pump(Duration::from_millis(1500));
        }
        "chatgpt" => {
            // New chat; its message box (centred, no AX tree without an
            // assistive app) gets the focus from a click at its right end.
            cmd(b'n');
            macos::pump(Duration::from_millis(1500));
            ctx.click(front.pid, 0.483);
            ctx.pointer = pointer();
            macos::click_in_window_at(front.pid, 0.66, 0.483);
            ctx.pointer = pointer();
            macos::pump(Duration::from_millis(400));
        }
        "textedit" => {
            wait_role(front.pid, &["AXTextArea"], Duration::from_secs(5)).ok_or("no document")?;
        }
        "safari" | "chrome" => {
            let end = Instant::now() + Duration::from_secs(10);
            let mut clicks = 0;
            let mut last_click = Instant::now();
            loop {
                let f = macos::focus(front.pid);
                let ours = f
                    .window_title
                    .as_deref()
                    .is_some_and(|t| t.contains("paste test"));
                if ours && f.role.as_deref() == Some("AXTextArea") {
                    break;
                }
                if Instant::now() > end {
                    return Err(format!(
                        "the test page's text box has no focus ({:?})",
                        f.role
                    ));
                }
                // Safari keeps the focus in the address bar after `open`:
                // click into the text box (top third of the page).
                if ours
                    && clicks < 3
                    && (clicks == 0 || last_click.elapsed() > Duration::from_secs(2))
                {
                    macos::pump(Duration::from_millis(500));
                    // The text box sits in the top third of the page; a
                    // little lower on each try.
                    ctx.click(front.pid, 0.28 + 0.04 * clicks as f64);
                    clicks += 1;
                    last_click = Instant::now();
                }
                macos::pump(Duration::from_millis(200));
            }
        }
        "terminal" => {
            let end = Instant::now() + Duration::from_secs(10);
            while !macos::focus(front.pid)
                .window_title
                .is_some_and(|t| t.contains("wb-paste-target"))
            {
                if Instant::now() > end {
                    return Err("the wb-paste-target window is not in front".into());
                }
                macos::pump(Duration::from_millis(200));
            }
            macos::pump(Duration::from_millis(500));
        }
        _ => {}
    }
    let f = macos::focus(front.pid);
    ctx.ev(
        "opened",
        json!({"app": app.key, "role": f.role, "value_len": f.value.as_ref().map(|v| v.chars().count()), "title": f.window_title}),
    );
    let initial = (app.read == Read::Field).then_some(f.value).flatten();
    Ok(Opened {
        front,
        initial,
        file,
        pidfile,
    })
}

fn still_in_target(app: &SoakApp, inst: &Installed, opened: &Opened) -> Result<(), String> {
    let front = macos::frontmost().ok_or("nothing in front")?;
    if front.bundle_id != inst.bundle_id {
        return Err(format!("{} came to the front", front.bundle_id));
    }
    if app.key == "terminal"
        && !macos::focus(opened.front.pid)
            .window_title
            .is_some_and(|t| t.contains("wb-paste-target"))
    {
        return Err("another Terminal window is in front".into());
    }
    Ok(())
}

/// Brings the target back to the front (other apps may activate
/// themselves during a run).
fn refocus(app: &SoakApp, inst: &Installed, opened: &Opened) -> Result<(), String> {
    macos::open(&["-b", &inst.bundle_id]);
    macos::pump(Duration::from_millis(700));
    if app.key == "terminal" {
        macos::raise_window(opened.front.pid, "wb-paste-target");
        macos::pump(Duration::from_millis(400));
    }
    still_in_target(app, inst, opened)
}

/// Before a take: the target app is in front and, where Accessibility can
/// read it, its field holds exactly what it held at the start plus our
/// tokens so far (so the take cannot land anywhere else). Returns how many
/// times the target had to be brought back.
fn ensure_target(
    ctx: &mut Ctx,
    app: &SoakApp,
    inst: &Installed,
    opened: &Opened,
    tokens: &[String],
) -> Result<usize, String> {
    let mut refocused = 0;
    for _ in 0..3 {
        match still_in_target(app, inst, opened) {
            Ok(()) => {
                if let Some(initial) = &opened.initial {
                    let want: String = tokens
                        .iter()
                        .fold(initial.clone(), |acc, t| format!("{acc}{t} "));
                    let have = macos::focus(opened.front.pid).value.unwrap_or_default();
                    if have != want {
                        return Err(format!(
                            "the field is not ours any more ({} chars, expected {})",
                            have.chars().count(),
                            want.chars().count()
                        ));
                    }
                }
                return Ok(refocused);
            }
            Err(e) => {
                ctx.ev("focus_stolen", json!({"app": app.key, "by": e}));
                refocused += 1;
                let _ = refocus(app, inst, opened);
                ctx.pointer = pointer();
            }
        }
    }
    Err("could not bring the target back to the front".into())
}

fn read_back(app: &SoakApp, opened: &Opened) -> Option<String> {
    match app.read {
        Read::Field => macos::focus(opened.front.pid).value,
        Read::Title => macos::focus(opened.front.pid).window_title,
        Read::CatFile => {
            macos::pump(Duration::from_millis(500));
            opened
                .file
                .as_ref()
                .and_then(|f| fs::read_to_string(f).ok())
        }
        Read::SaveFile => {
            cmd(b's');
            macos::pump(Duration::from_millis(1500));
            opened
                .file
                .as_ref()
                .and_then(|f| fs::read_to_string(f).ok())
        }
        Read::CopyBack => {
            // Empty the clipboard first, so a copy that did not happen
            // cannot be mistaken for the field's text (the last source is
            // still on the clipboard).
            let t = copy_field();
            key(124); // drop the selection
            (!t.is_empty()).then_some(t)
        }
    }
}

/// Empties or closes what we opened. Never saves into the owner's files,
/// never sends.
fn tidy(app: &SoakApp, inst: &Installed, opened: &Opened, clean_field: bool) {
    if still_in_target(app, inst, opened).is_err() && refocus(app, inst, opened).is_err() {
        println!(
            "    {}: not in front for the cleanup; check it by hand",
            inst.name
        );
        return;
    }
    match app.key {
        "notes" | "messages" | "claude" => {
            // Only our tokens, even after a draft that was already there.
            if let Err(e) = strip_tokens_in_focus(opened.front.pid) {
                println!("    {}: {e}; check the field by hand", inst.name);
            }
        }
        "chatgpt" => {
            if let Err(e) = strip_tokens_by_copy() {
                println!("    {}: {e}; check the field by hand", inst.name);
            }
        }
        "mail" => {
            if clean_field {
                cmd(b'a');
                key(DELETE);
                cmd(b'w');
                // Mail asks whether to keep the (auto-saved) draft: no.
                macos::pump(Duration::from_millis(900));
                if macos::press_button(opened.front.pid, "Don’t Save")
                    || macos::press_button(opened.front.pid, "Don't Save")
                {
                    macos::pump(Duration::from_millis(600));
                }
            }
        }
        "terminal" => {
            if let Some(p) = &opened.pidfile {
                if let Ok(pid) = fs::read_to_string(p) {
                    let _ = std::process::Command::new("/bin/kill")
                        .arg(pid.trim())
                        .status();
                }
            }
            macos::pump(Duration::from_millis(500));
            cmd(b'w');
        }
        "safari" | "chrome" => {
            // Our tab, and any test tab an earlier, interrupted run left.
            for _ in 0..4 {
                let title = macos::focus(opened.front.pid)
                    .window_title
                    .unwrap_or_default();
                if !is_ours(&title) {
                    break;
                }
                cmd(b'w');
                macos::pump(Duration::from_millis(400));
            }
        }
        _ => cmd(b'w'),
    }
    macos::pump(Duration::from_millis(500));
}

/// After a failed open: close the window or tab only if it is one of ours.
fn close_if_ours(inst: &Installed) {
    let Some(front) = macos::frontmost() else {
        return;
    };
    if front.bundle_id != inst.bundle_id {
        return;
    }
    let title = macos::focus(front.pid).window_title.unwrap_or_default();
    if is_ours(&title) {
        cmd(b'w');
    }
}

fn run_app(ctx: &mut Ctx, app: &SoakApp, inst: &Installed, n: usize, keep_open: bool) -> AppRow {
    let mut row = AppRow {
        key: app.key.into(),
        name: inst.name.into(),
        version: inst.version.clone(),
        class: inst.class.describe(),
        for_r5: app.for_r5.into(),
        status: "error".into(),
        ..AppRow::default()
    };
    println!("  {} …", inst.name);
    let opened = match open_app(ctx, app, inst) {
        Ok(o) => o,
        Err(e) => {
            row.note = Some(e);
            close_if_ours(inst);
            return row;
        }
    };
    ctx.pointer = pointer();
    let open = || false;
    let none = || None;
    let gate = Gate {
        cancelled: &open,
        interrupted: &none,
    };
    let mut tokens = Vec::new();
    for i in 0..n {
        if ctx.owner_active() {
            row.note = ctx.stop.clone();
            break;
        }
        match ensure_target(ctx, app, inst, &opened, &tokens) {
            Ok(n) => row.refocused += n,
            Err(e) => {
                row.status = "focus_moved".into();
                row.note = Some(e);
                break;
            }
        }
        ctx.token += 1;
        let token = format!("[S-04 soak {}]", ctx.token);
        let text = format!("{token} ");
        let src = ctx.sources[i % ctx.sources.len()].clone();
        if let Err(e) = nspb::write(&src.snapshot) {
            row.note = Some(format!("could not set the source: {e}"));
            break;
        }
        macos::pump(Duration::from_millis(150));
        // What is really on the clipboard (AppKit may add derived types to
        // what we wrote): the restore must give back exactly this.
        let (before, _) = nspb::read();
        let req = Request::new(&text, inst.class);
        let (report, pending) = deliver::paste(&SoakPlatform, &ctx.line, &req, &gate);
        let settled = pending.map(|p| deliver::settle(&SoakPlatform, &ctx.line, p, &Timing::R5));
        let (after, _) = nspb::read();
        let fid = snapshot::compare(&before, &after);
        if report.delivery.inserted() {
            tokens.push(token.clone());
        }
        let take = TakeRow {
            n: ctx.token,
            token,
            source: src.kind,
            report,
            settled,
            restore: fid.describe(),
            restore_identical: fid.byte_identical(),
        };
        ctx.ev("take", json!({"app": app.key, "take": take}));
        row.takes.push(take);
        macos::pump(Duration::from_millis(250));
    }
    if row.status == "error" {
        row.status = "done".into();
    }
    // Read the field back and count every token.
    macos::pump(Duration::from_millis(600));
    let text = if still_in_target(app, inst, &opened).is_ok() || refocus(app, inst, &opened).is_ok()
    {
        read_back(app, &opened)
    } else {
        None
    };
    match &text {
        Some(t) => {
            for tok in &tokens {
                match t.matches(tok.as_str()).count() {
                    1 => row.once += 1,
                    0 => row.lost.push(tok.clone()),
                    _ => row.duplicated.push(tok.clone()),
                }
            }
            row.old_clipboard_pasted = t.contains(SOURCE_MARK);
            // Our tokens only (never the owner's text): the last 300 chars.
            row.read_back = Some(
                t.chars()
                    .rev()
                    .take(300)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect(),
            );
        }
        None => {
            row.note
                .get_or_insert_with(|| "could not read the field back".into());
        }
    }
    if !keep_open {
        // Only empty a field whose whole text is ours.
        let ours = text.as_deref().is_some_and(|t| {
            let mut rest = t.to_string();
            for tok in &tokens {
                rest = rest.replace(tok.as_str(), "");
            }
            rest.trim().is_empty() || app.read == Read::Title
        });
        tidy(app, inst, &opened, ours);
    }
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ten_apps_cover_r5_5_4() {
        assert_eq!(SOAK_APPS.len(), 10);
        for a in SOAK_APPS {
            assert!(apps::APPS.iter().any(|s| s.key == a.key), "{}", a.key);
        }
    }

    #[test]
    fn only_tokens_rule() {
        assert!(only_tokens("[S-04 soak 1] "));
        assert!(only_tokens(" [S-04 soak 1] [S-04 soak 22]\n"));
        assert!(!only_tokens(""));
        assert!(!only_tokens("[S-04 soak 1] hello"));
        assert!(!only_tokens("hello"));
        assert!(!only_tokens("[S-04 soak 1"));
    }

    #[test]
    fn token_tail_counts_only_trailing_tokens() {
        assert_eq!(token_tail("hello [S-04 soak 1] [S-04 soak 2] "), 28);
        assert_eq!(token_tail("hello"), 0);
        assert_eq!(token_tail("[S-04 soak 3] tail"), 0);
        assert_eq!(token_tail("x[S-04 soak 10]"), 14);
    }

    #[test]
    fn totals_on_empty() {
        let s = Summary::default();
        assert!(totals(&s).starts_with("takes 0"));
    }
}
