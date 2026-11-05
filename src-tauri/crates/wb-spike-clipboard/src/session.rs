// SPDX-License-Identifier: GPL-3.0-or-later
//! The guided session: plain-English steps for the owner, the automated
//! checks, the paste matrix across the installed apps, the held-⌃⌘ test,
//! the optional CJK input step, and the result files.

use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::apps::{self, AppSpec, Installed, Open, Verify};
use crate::clip::keys::{self, COMMAND_FLAG, CONTROL_FLAG, HOTKEY_MODIFIERS};
use crate::clip::nspb;
use crate::clip::restore::{RestoreGuard, TargetClass, Timing};
use crate::clip::target;
use crate::fidelity;
use crate::log::Log;
use crate::macos::{self, Front};
use crate::owner;
use crate::report::{
    self, AppRun, AppTake, HeldTest, HeldVariant, ImeTest, Landed, Results, SourceInfo,
};
use crate::sources::{self, Files, Kind, Source, SOURCE_MARK};
use crate::take::{self, Method, TakeSpec};
use crate::ui::{Ans, Ui};
use crate::Options;

const RULE: &str = "------------------------------------------------------------------";
const WINDOW_TITLE: &str = "Wispr Bro RS-05 test";

pub fn results_root(opts: &Options) -> PathBuf {
    opts.out.clone().unwrap_or_else(|| {
        PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
            .join("wb-bench/results/rs05")
    })
}

pub fn intro() {
    println!(
        "\n{RULE}\n  Wispr Bro: clipboard and paste test (RS-05)\n{RULE}\n\
         When Wispr Bro pastes your words, it briefly puts them on the clipboard,\n\
         presses ⌘V for you, and then puts back whatever you had copied before.\n\
         This test checks that this works in your apps, and that what you had\n\
         copied (text, styled text, a web page, files, a picture) comes back\n\
         exactly.\n\n\
         It takes about 10 minutes. Nothing leaves this Mac. The test types\n\
         \"[dictated text 1]\", \"[dictated text 2]\" and so on into the apps; it\n\
         never sends anything. Whatever is on your clipboard right now is saved\n\
         first and put back at the end, even if you stop early with Control+C.\n\n\
         Tip: make this window narrow and keep it at one side of the screen, so\n\
         you can read it while other apps are in front.\n"
    );
}

struct Ctx {
    dir: PathBuf,
    log: Log,
    ui: Option<Ui>,
    home: Option<Front>,
    r: Results,
    files: Option<Files>,
    /// The source used for each kind in the paste matrix.
    matrix_sources: BTreeMap<String, Source>,
    guard: RestoreGuard,
    token: u32,
    tidy: Vec<String>,
    cat_pidfile: Option<PathBuf>,
    /// (input source to go back to, enabled ids before, saved list before)
    ime_restore: Option<(String, Vec<String>, String)>,
}

fn key(k: Kind) -> String {
    format!("{k:?}")
}

impl Ctx {
    fn ev(&self, kind: &str, v: serde_json::Value) {
        self.log.write(kind, Instant::now(), v);
    }

    fn next_text(&mut self) -> String {
        self.token += 1;
        format!("[dictated text {}] ", self.token)
    }

    fn ui(&self) -> &Ui {
        self.ui.as_ref().expect("interactive session")
    }

    /// Brings the owner's terminal window back to the front.
    fn come_back(&self) {
        if let Some(h) = &self.home {
            if !h.bundle_id.is_empty() {
                macos::open(&["-b", &h.bundle_id]);
            }
            macos::pump(Duration::from_millis(300));
            macos::raise_window(h.pid, WINDOW_TITLE);
            macos::pump(Duration::from_millis(200));
        }
    }
}

fn machine() -> report::Machine {
    let run = |cmd: &str, args: &[&str]| {
        std::process::Command::new(cmd)
            .args(args)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default()
    };
    report::Machine {
        model: run("/usr/sbin/sysctl", &["-n", "hw.model"]),
        macos_version: run("/usr/bin/sw_vers", &["-productVersion"]),
        macos_build: run("/usr/bin/sw_vers", &["-buildVersion"]),
    }
}

fn perms() -> report::Perms {
    report::Perms {
        accessibility: macos::accessibility_trusted(),
        post_events: macos::can_post_events(),
        listen_events: macos::can_listen_events(),
    }
}

fn environment(r: &mut Results) {
    r.machine = machine();
    r.permissions = perms();
    r.terminal = std::env::var("TERM_PROGRAM").unwrap_or_default();
    r.apps = apps::APPS.iter().filter_map(apps::installed).collect();
    if let Some(c) = r.apps.iter().find(|a| a.key == "chrome") {
        r.chrome_version = c.version.clone();
        r.chrome_major = c.class.chromium_major();
    }
}

pub fn dry_run(opts: &Options) -> i32 {
    intro();
    let mut r = Results::default();
    environment(&mut r);
    println!(
        "Mac: {}, macOS {} ({})",
        r.machine.model, r.machine.macos_version, r.machine.macos_build
    );
    println!(
        "Permissions for this terminal: Accessibility {}, post key events {}, Input Monitoring {}",
        r.permissions.accessibility, r.permissions.post_events, r.permissions.listen_events
    );
    println!("Apps the test would use:");
    for a in &r.apps {
        println!(
            "  {:<14} {:<12} {:<28} {}",
            a.name,
            a.version.clone().unwrap_or_default(),
            a.class.describe(),
            a.bundle_id
        );
    }
    let missing: Vec<&str> = apps::APPS
        .iter()
        .filter(|s| !r.apps.iter().any(|a| a.key == s.key))
        .map(|s| s.name)
        .collect();
    if !missing.is_empty() {
        println!("Not installed (skipped): {}", missing.join(", "));
    }
    let on: Vec<String> = macos::input_sources(false)
        .into_iter()
        .map(|s| s.id)
        .collect();
    let ime: Vec<String> = macos::input_sources(true)
        .into_iter()
        .filter(|s| IME_CANDIDATES.contains(&s.id.as_str()))
        .map(|s| format!("{} (on: {})", s.id, on.contains(&s.id)))
        .collect();
    println!("CJK input sources found: {ime:?}");
    println!(
        "⌘V key code on the current layout: {}",
        keys::command_v_keycode()
    );
    println!(
        "Results would go to: {}/<date-time>/\n\nDry run: the clipboard was not touched and nothing was written.",
        results_root(opts).display()
    );
    0
}

// ---------------------------------------------------------------------------
// Entry points

pub fn run(opts: Options, mode: RunMode) -> i32 {
    let interactive = mode == RunMode::Session;
    let root = results_root(&opts);
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let dir = root.join(if interactive {
        stamp.clone()
    } else {
        format!("{stamp}-{}", mode.name())
    });
    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!("Could not create {}: {e}", dir.display());
        return 1;
    }
    let log = match Log::create(&dir.join("events.jsonl"), Instant::now()) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Could not create the log: {e}");
            return 1;
        }
    };
    let ui = interactive.then(Ui::new);
    let mut ctx = Ctx {
        dir: dir.clone(),
        log,
        home: macos::frontmost(),
        ui,
        r: Results {
            tool: "wb-spike-clipboard",
            version: env!("CARGO_PKG_VERSION"),
            git_sha: option_env!("WB_GIT_SHA").unwrap_or("unknown").into(),
            mode: mode.name().into(),
            started: chrono::Local::now().to_rfc3339(),
            ..Results::default()
        },
        files: None,
        matrix_sources: BTreeMap::new(),
        guard: RestoreGuard::default(),
        token: 0,
        tidy: Vec::new(),
        cat_pidfile: None,
        ime_restore: None,
    };
    match &ctx.ui {
        Some(ui) => owner::install_signals(ui.waker()),
        None => owner::install_signals(|| {}),
    }
    print!("\x1b]0;{WINDOW_TITLE}\x07");
    let _ = std::io::stdout().flush();
    if interactive {
        intro();
    }

    // A backup left by a run that did not finish.
    if interactive {
        let left = owner::leftover_backups(&root, Some(&dir));
        if let Some(p) = left.first() {
            println!(
                "An earlier run of this test stopped before it could put your clipboard back.\n\
                 Its copy of your clipboard is still saved."
            );
            if ctx.ui().yes_no("Put that saved clipboard back now?") == Some(Ans::Yes) {
                match owner::restore_file(p) {
                    Ok(r) => println!(
                        "  Done ({}).\n",
                        if r.identical { "exactly" } else { &r.detail }
                    ),
                    Err(e) => println!("  Could not: {e}\n"),
                }
            }
        }
    }

    match owner::save(&dir) {
        Ok(c) => {
            ctx.r.owner_saved = Some(c);
            ctx.ev("owner_saved", json!({"items": c.0, "types": c.1}));
        }
        Err(e) => {
            eprintln!("Could not save your clipboard first ({e}); stopping without touching it.");
            return 1;
        }
    }
    environment(&mut ctx.r);
    ctx.ev(
        "environment",
        json!({"machine": ctx.r.machine, "permissions": ctx.r.permissions, "apps": ctx.r.apps}),
    );

    body(&mut ctx, mode);
    finish(&mut ctx)
}

fn body(ctx: &mut Ctx, mode: RunMode) {
    let interactive = mode == RunMode::Session;
    let files = match sources::make_files(&ctx.dir) {
        Ok(f) => f,
        Err(e) => {
            println!("Could not create the test files: {e}");
            return;
        }
    };
    for k in Kind::MATRIX {
        ctx.matrix_sources
            .insert(key(k), sources::synthetic(k, &files));
    }
    ctx.files = Some(files);

    let can_post = match mode {
        RunMode::Session => permissions(ctx),
        RunMode::SelfCheck => macos::accessibility_trusted() && macos::can_post_events(),
        RunMode::Auto => false,
    };
    if owner::interrupted() {
        return;
    }

    // Real copies from TextEdit, Safari and Finder.
    let mut real = Vec::new();
    if can_post {
        println!(
            "\n{RULE}\nStep 1 of 4: copying from TextEdit, Safari and Finder (about 30 seconds)\n{RULE}\n\
             The test now opens a TextEdit document, a Safari page and a Finder window by\n\
             itself and copies from them. Please keep your hands off the keyboard and\n\
             mouse until this window comes back to the front."
        );
        if interactive && ctx.ui().enter("Press Enter to start:").is_none() {
            return;
        }
        let f = ctx.files.as_ref().expect("files");
        for (k, res) in sources::real_copies(f) {
            match res {
                Ok(src) => {
                    ctx.ev(
                        "real_copy",
                        json!({"kind": k, "origin": src.origin, "types": src.snapshot.types()}),
                    );
                    ctx.matrix_sources.insert(key(k), src.clone());
                    real.push(src);
                }
                Err(e) => {
                    ctx.ev("real_copy_failed", json!({"kind": k, "error": e}));
                    ctx.r.real_copy_errors.push((k, e));
                }
            }
            if owner::interrupted() {
                return;
            }
        }
        ctx.tidy
            .push("close the TextEdit, Safari and Finder windows named \"WB-SOURCE …\"".into());
        ctx.come_back();
        println!(
            "  Copied {} of 4 from the apps{}.",
            real.len(),
            if ctx.r.real_copy_errors.is_empty() {
                String::new()
            } else {
                " (the rest use the test's own copies)".to_string()
            }
        );
    }

    // Automated round trips and guard checks.
    if interactive {
        println!("\nChecking the clipboard save and restore by itself (a few seconds)...");
    }
    let f = ctx.files.as_ref().expect("files");
    let mut all: Vec<Source> = Kind::MATRIX
        .iter()
        .map(|k| sources::synthetic(*k, f))
        .collect();
    all.push(sources::synthetic(Kind::BigImage, f));
    all.extend(real.iter().cloned());
    for s in ctx.matrix_sources.values() {
        ctx.r.sources.push(SourceInfo {
            kind: s.kind,
            origin: s.origin.clone(),
            types: s.snapshot.types(),
            bytes: s.snapshot.bytes(),
        });
    }
    ctx.r.fidelity = fidelity::run_fidelity(&all);
    for row in &ctx.r.fidelity {
        ctx.log.write("fidelity", Instant::now(), json!(row));
    }
    let plain = sources::synthetic(Kind::Plain, f);
    let (guards, _) = fidelity::run_guards(&plain);
    for g in &guards {
        ctx.ev("guard", json!(g));
    }
    ctx.r.guards = guards;
    let count = |b: fidelity::Backend| {
        let rows: Vec<_> = ctx.r.fidelity.iter().filter(|r| r.backend == b).collect();
        let bad = rows
            .iter()
            .filter(|r| !r.fidelity.as_ref().is_some_and(|f| f.byte_identical()))
            .count();
        (rows.len(), bad)
    };
    let (n, bad) = count(fidelity::Backend::Objc2);
    let (cn, cbad) = count(fidelity::Backend::Carbon);
    let gbad = ctx.r.guards.iter().filter(|g| !g.pass).count();
    println!(
        "  Save and restore the way Wispr Bro will do it: {n} tries, {}.\n  \
         (An older method, kept for comparison: {cbad} of {cn} differ.)\n  \
         Safety checks: {} of {} passed.",
        if bad == 0 {
            "all came back exactly".to_string()
        } else {
            format!("{bad} did NOT come back exactly")
        },
        ctx.r.guards.len() - gbad,
        ctx.r.guards.len()
    );
    if mode == RunMode::Auto || owner::interrupted() {
        return;
    }
    if !can_post {
        println!(
            "\nThe remaining steps press keys for you, which needs the Accessibility\n\
             permission. Skipping them."
        );
        return;
    }

    matrix(ctx);
    if owner::interrupted() {
        return;
    }
    if interactive {
        ctx.r.held = Some(held(ctx));
        if owner::interrupted() {
            return;
        }
    }
    ctx.r.ime = ime(ctx);
}

/// `Session`: the owner's guided run. `Auto`: clipboard checks only (no key
/// presses, no windows). `SelfCheck`: Auto plus the parts that need no
/// person: real copies, and pastes into fresh TextEdit, Terminal, Safari and
/// Chrome windows the tool opens itself, and the IME step if a CJK input
/// source is already on (switched back afterwards).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunMode {
    Session,
    Auto,
    SelfCheck,
}

impl RunMode {
    fn name(&self) -> &'static str {
        match self {
            RunMode::Session => "session",
            RunMode::Auto => "auto",
            RunMode::SelfCheck => "selfcheck",
        }
    }
}

/// Apps the self-check can use without anyone clicking.
const SELF_CHECK_APPS: &[&str] = &["textedit", "terminal", "safari", "chrome"];

fn permissions(ctx: &mut Ctx) -> bool {
    let term = ctx
        .home
        .as_ref()
        .map(|h| h.name.clone())
        .unwrap_or_else(|| "Terminal".into());
    for attempt in 0..3 {
        let ax = macos::accessibility_trusted();
        let post = macos::can_post_events();
        if ax && post {
            if attempt > 0 {
                println!("  Thanks, that worked.");
            }
            ctx.r.permissions = perms();
            return true;
        }
        if attempt == 0 {
            println!(
                "\nThis test needs one permission: {term} must be allowed to control your Mac\n\
                 (System Settings → Privacy & Security → Accessibility → {term}: on).\n\
                 macOS may show its own dialog now; the settings page opens too."
            );
            macos::request_post_events();
            macos::open_accessibility_pane();
        } else {
            println!(
                "  It is still off. If {term} is already switched on in that list, macOS may\n\
                 need {term} to restart: quit it (⌘Q), open it again and run the same command."
            );
        }
        let a = ctx.ui().enter(&format!(
            "Switch on {term} in that list, then press Enter (or type s to skip the app tests):"
        ));
        match a.as_deref() {
            None => return false,
            Some("s") | Some("S") => break,
            _ => {}
        }
    }
    ctx.r.permissions = perms();
    ctx.r.permissions.accessibility && ctx.r.permissions.post_events
}

// ---------------------------------------------------------------------------
// The paste matrix

fn target_text(verify: Verify, pid: i32, file: Option<&Path>) -> Option<String> {
    match verify {
        Verify::FieldText => macos::focus(pid).value,
        Verify::WindowTitle => macos::focus(pid).window_title,
        Verify::File => file.and_then(|f| fs::read_to_string(f).ok()),
        Verify::Owner => None,
    }
}

fn prepare(ctx: &mut Ctx, spec: &AppSpec, inst: &Installed) -> Result<Option<PathBuf>, String> {
    let targets = ctx.dir.join("targets");
    fs::create_dir_all(&targets).map_err(|e| e.to_string())?;
    let app = inst.path.to_string_lossy().into_owned();
    match spec.open {
        Open::NewFile => {
            let f = targets.join(format!("wb-paste-target-{}.txt", spec.key));
            fs::write(&f, b"").map_err(|e| e.to_string())?;
            macos::open(&["-a", &app, &f.to_string_lossy()]);
            Ok(None)
        }
        Open::TestPage => {
            let f = targets.join(format!("wb-paste-test-{}.html", spec.key));
            fs::write(&f, apps::TEST_PAGE).map_err(|e| e.to_string())?;
            macos::open(&["-a", &app, &f.to_string_lossy()]);
            Ok(None)
        }
        Open::NewMail => {
            macos::open(&[
                "-b",
                &inst.bundle_id,
                "mailto:?subject=Wispr%20Bro%20paste%20test",
            ]);
            Ok(None)
        }
        Open::TerminalCat => {
            let out = targets.join("terminal-received.txt");
            let pid = targets.join("terminal-cat.pid");
            let script = targets.join("wb-paste-target.command");
            fs::write(&script, apps::terminal_script(&out, &pid)).map_err(|e| e.to_string())?;
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
            let _ = fs::remove_file(&out);
            macos::open(&["-a", &app, &script.to_string_lossy()]);
            ctx.cat_pidfile = Some(pid);
            Ok(Some(out))
        }
        Open::Activate => {
            macos::open(&["-a", &app]);
            Ok(None)
        }
    }
}

enum Ready {
    Go(Front),
    Skip,
    TimedOut,
}

/// Waits for the owner to click into the target (or, for a fresh document
/// the tool opened, for the text field to have focus).
fn wait_ready(ctx: &Ctx, spec: &AppSpec, inst: &Installed) -> Ready {
    let auto = matches!(
        spec.open,
        Open::NewFile | Open::TestPage | Open::TerminalCat
    ) && spec.verify != Verify::Owner;
    if ctx.ui.is_none() && !auto {
        return Ready::Skip;
    }
    let end = Instant::now() + Duration::from_secs(if ctx.ui.is_some() { 120 } else { 20 });
    let mut was_down = false;
    let mut clicked_at: Option<Instant> = None;
    let mut auto_since: Option<Instant> = None;
    while Instant::now() < end {
        if owner::interrupted() {
            return Ready::Skip;
        }
        if let Some(l) = ctx.ui.as_ref().and_then(|u| u.poll_line()) {
            if l.eq_ignore_ascii_case("s") {
                return Ready::Skip;
            }
        }
        let front = macos::frontmost();
        let in_target = front
            .as_ref()
            .is_some_and(|f| f.bundle_id == inst.bundle_id);
        let down = macos::mouse_down();
        if in_target {
            if was_down && !down {
                clicked_at = Some(Instant::now());
            }
            let f = front.clone().expect("front");
            let ok_auto = auto && {
                let foc = macos::focus(f.pid);
                if spec.open == Open::TerminalCat {
                    foc.window_title
                        .is_some_and(|t| t.contains("wb-paste-target"))
                } else {
                    matches!(
                        foc.role.as_deref(),
                        Some("AXTextArea") | Some("AXTextField")
                    )
                }
            };
            if spec.open == Open::TerminalCat && !ok_auto {
                // Our own window is Terminal too: only the target window counts.
                clicked_at = None;
            }
            auto_since = if ok_auto {
                auto_since.or(Some(Instant::now()))
            } else {
                None
            };
            let ready = clicked_at.is_some_and(|t| t.elapsed() >= Duration::from_millis(1200))
                || auto_since.is_some_and(|t| t.elapsed() >= Duration::from_millis(1500));
            if ready && !down {
                return Ready::Go(f);
            }
        } else {
            clicked_at = None;
            auto_since = None;
        }
        was_down = down;
        macos::pump(Duration::from_millis(20));
    }
    Ready::TimedOut
}

fn matrix(ctx: &mut Ctx) {
    let self_check = ctx.ui.is_none();
    let list: Vec<(&'static AppSpec, Installed)> = apps::APPS
        .iter()
        .filter(|s| !self_check || SELF_CHECK_APPS.contains(&s.key))
        .filter_map(|s| {
            ctx.r
                .apps
                .iter()
                .find(|a| a.key == s.key)
                .map(|a| (s, a.clone()))
        })
        .collect();
    if self_check {
        println!("Pasting into {} apps the tool opens itself...", list.len());
    } else {
        println!(
        "\n{RULE}\nStep 2 of 4: pasting into your apps ({} apps, about 30 seconds each)\n{RULE}\n\
         For each app: press Enter, and the app comes to the front (with a new, empty\n\
         document where possible). Click where you would type, as the line below\n\
         says, then take your hands off the keyboard and mouse. The test pastes six\n\
         times, \"[dictated text …]\", each time with something different on the\n\
         clipboard. A sound plays and this window comes back when it is done.\n\
         Nothing is ever sent. To skip an app, type s and press Enter.",
            list.len()
        );
    }
    let n = list.len();
    for (i, (spec, inst)) in list.into_iter().enumerate() {
        if owner::interrupted() {
            return;
        }
        let run = app(ctx, spec, &inst, i + 1, n);
        ctx.ev("app_done", json!({"app": spec.key, "status": run.status}));
        ctx.r.matrix.push(run);
    }
}

fn app(ctx: &mut Ctx, spec: &AppSpec, inst: &Installed, i: usize, n: usize) -> AppRun {
    let mut run = AppRun {
        key: spec.key.into(),
        name: spec.name.into(),
        version: inst.version.clone(),
        class: inst.class.describe(),
        status: "skipped".into(),
        takes: Vec::new(),
        owner_count: None,
        owner_saw_other: None,
        note: None,
    };
    println!("\n  App {i} of {n}: {}", spec.name);
    if let Some(ui) = &ctx.ui {
        println!("  When it opens, click into {}.", spec.click);
        match ui
            .enter("  Press Enter to open it (or s to skip):")
            .as_deref()
        {
            None => return run,
            Some(s) if s.eq_ignore_ascii_case("s") => return run,
            _ => {}
        }
    }
    let file = match prepare(ctx, spec, inst) {
        Ok(f) => f,
        Err(e) => {
            run.status = "error".into();
            run.note = Some(e);
            return run;
        }
    };
    if ctx.ui.is_some() {
        println!(
            "  Waiting for your click in {}... (s + Enter here to skip)",
            spec.name
        );
    }
    let front = match wait_ready(ctx, spec, inst) {
        Ready::Go(f) => f,
        Ready::Skip => {
            ctx.come_back();
            return run;
        }
        Ready::TimedOut => {
            run.status = "not_ready".into();
            run.note = Some("no click in the app within 2 minutes".into());
            ctx.come_back();
            return run;
        }
    };
    ctx.tidy.push(spec.tidy.into());
    run.status = "done".into();
    let first = ctx.token + 1;
    // Chromium 155/156: the product types instead of pasting (hex). Paste
    // anyway, to see whether this Mac's Chrome drops the ⌘V, then add one
    // take on the typing path the product would use.
    let race = target::has_clipboard_race(inst.class);
    let mut plan: Vec<(Kind, Method)> = Kind::MATRIX
        .iter()
        .map(|k| (*k, Method::CommandV))
        .collect();
    if race {
        plan.push((Kind::Plain, Method::Typing));
    }
    for (k, method) in plan {
        if owner::interrupted() {
            break;
        }
        let src = ctx.matrix_sources[&key(k)].clone();
        let text = ctx.next_text();
        let token = text.trim().to_string();
        if let Err(e) = nspb::write(&src.snapshot) {
            run.note = Some(format!("could not set the source: {e}"));
            continue;
        }
        macos::pump(Duration::from_millis(150));
        let before = target_text(spec.verify, front.pid, file.as_deref());
        let marks_before = before.as_deref().map(|b| b.matches(SOURCE_MARK).count());
        let mut tspec = TakeSpec::new(&text, inst.class);
        tspec.method = method;
        tspec.expect_bundle = Some(inst.bundle_id.clone());
        if spec.open == Open::TerminalCat {
            tspec.expect_window = Some("wb-paste-target".into());
        }
        let verify = spec.verify;
        let pid = front.pid;
        let fpath = file.clone();
        let tok = token.clone();
        let mut probe =
            move || target_text(verify, pid, fpath.as_deref()).is_some_and(|t| t.contains(&tok));
        let rep = take::run_probed(&mut ctx.guard, &tspec, &Timing::R5, &mut probe);
        let after = target_text(spec.verify, front.pid, file.as_deref());
        let landed = if rep.decision == "not_pasted" {
            Landed::NotPasted
        } else {
            match (&after, spec.verify) {
                (_, Verify::Owner) | (None, _) => Landed::Unverified,
                (Some(t), _) if t.contains(&token) => Landed::Confirmed,
                (Some(_), _) => Landed::Missing,
            }
        };
        let old = match (marks_before, after.as_deref()) {
            (Some(b), Some(a)) => Some(a.matches(SOURCE_MARK).count() > b),
            _ => None,
        };
        ctx.log.write(
            "take",
            Instant::now(),
            json!({"app": spec.key, "kind": k, "origin": src.origin, "landed": landed, "old_clipboard_pasted": old, "take": rep}),
        );
        let not_pasted = rep.decision == "not_pasted";
        run.takes.push(AppTake {
            kind: k,
            origin: src.origin.clone(),
            race_probe: race && method == Method::CommandV,
            confirm_ms: rep.landed_ms,
            take: rep,
            landed,
            old_clipboard_pasted: old,
        });
        if not_pasted {
            run.status = "focus_moved".into();
            run.note = Some("another app came to the front; stopped pasting here".into());
            break;
        }
        macos::pump(Duration::from_millis(250));
    }
    macos::play("Glass");
    if let Some(p) = ctx.cat_pidfile.take() {
        kill_pidfile(&p);
    }
    ctx.come_back();
    let last = ctx.token;
    for t in &run.takes {
        println!(
            "    {:<16} {:<9} {:<7} {:<11} clipboard {}",
            t.kind.label(),
            t.origin,
            match t.take.method {
                Method::Typing => "typed",
                _ => "⌘V",
            },
            match t.landed {
                Landed::Confirmed => "arrived",
                Landed::Missing => "NOT SEEN",
                Landed::NotPasted => "not pasted",
                _ => "(see below)",
            },
            t.take
                .fidelity
                .as_ref()
                .map(|f| if f.byte_identical() {
                    "restored exactly".to_string()
                } else {
                    format!("restore problem: {}", f.describe())
                })
                .unwrap_or_else(|| t.take.decision.clone())
        );
    }
    if ctx.ui.is_some()
        && run.status == "done"
        && run.takes.iter().any(|t| ask_owner(spec.key, t.landed))
    {
        let c = ctx.ui().choice(
            &format!(
                "Look at {}: how many of \"[dictated text {first}]\" to \"[dictated text {last}]\" arrived?",
                spec.name
            ),
            &["all of them", "some, not all", "none"],
        );
        run.owner_count = c.map(|c| ["all", "some", "none"][c].to_string());
        let mapped = match c {
            Some(0) => Landed::OwnerYes,
            Some(1) => Landed::OwnerSome,
            Some(2) => Landed::OwnerNo,
            _ => Landed::Unverified,
        };
        for t in run
            .takes
            .iter_mut()
            .filter(|t| ask_owner(spec.key, t.landed))
        {
            t.landed = mapped;
        }
        run.owner_saw_other = ctx.ui().yes_no(
            "Did anything else appear there too, such as text starting with \"WB-SOURCE\", a picture or a file?",
        );
    }
    run
}

/// Apps where the tool's own check was shown to work in the self-check.
/// Elsewhere a "not seen" from Accessibility may only mean the field's text
/// is not exposed, so the owner is asked as well.
const READ_PROVEN: &[&str] = &["textedit", "terminal", "safari", "chrome"];

fn ask_owner(app: &str, landed: Landed) -> bool {
    landed == Landed::Unverified || (landed == Landed::Missing && !READ_PROVEN.contains(&app))
}

fn kill_pidfile(p: &Path) {
    if let Some(pid) = fs::read_to_string(p)
        .ok()
        .and_then(|s| s.trim().parse::<i32>().ok())
    {
        // SAFETY: plain kill(2) of the `cat` this run started.
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
    }
    let _ = fs::remove_file(p);
}

// ---------------------------------------------------------------------------
// Held ⌃⌘ (S-06 paste-last)

fn open_textedit(ctx: &Ctx, name: &str) -> Option<Front> {
    let targets = ctx.dir.join("targets");
    fs::create_dir_all(&targets).ok()?;
    let f = targets.join(name);
    fs::write(&f, b"").ok()?;
    macos::open(&["-a", "TextEdit", &f.to_string_lossy()]);
    let front = macos::wait_front("com.apple.TextEdit", Duration::from_secs(10))?;
    let end = Instant::now() + Duration::from_secs(10);
    while Instant::now() < end {
        let foc = macos::focus(front.pid);
        let stem = name.trim_end_matches(".txt");
        if foc.window_title.is_some_and(|t| t.contains(stem))
            && foc.role.as_deref() == Some("AXTextArea")
        {
            macos::pump(Duration::from_millis(400));
            return Some(front);
        }
        macos::pump(Duration::from_millis(100));
    }
    None
}

fn wait_held(want: u64, timeout: Duration) -> Option<u64> {
    let end = Instant::now() + timeout;
    while Instant::now() < end && !owner::interrupted() {
        let f = keys::flags_now();
        if f & want == want {
            return Some(f & HOTKEY_MODIFIERS);
        }
        macos::pump(Duration::from_millis(15));
    }
    None
}

fn wait_released(timeout: Duration) {
    let end = Instant::now() + timeout;
    while Instant::now() < end && keys::flags_now() & HOTKEY_MODIFIERS != 0 {
        macos::pump(Duration::from_millis(15));
    }
}

fn held_variant(ctx: &mut Ctx, front: &Front, wait: bool) -> HeldVariant {
    let want = CONTROL_FLAG | COMMAND_FLAG;
    let mut v = HeldVariant {
        held_flags: "none".into(),
        take: None,
        landed: Landed::NotPasted,
        v_arrived_with: Vec::new(),
    };
    let Some(held) = wait_held(want, Duration::from_secs(45)) else {
        return v;
    };
    v.held_flags = report::flags_label(held);
    if wait {
        macos::play("Pop"); // "let go now"
    } else {
        macos::pump(Duration::from_millis(400));
    }
    let text = ctx.next_text();
    let token = text.trim().to_string();
    let mut spec = TakeSpec::new(&text, TargetClass::Native);
    spec.wait_modifiers = wait;
    spec.max_modifier_wait = Duration::from_secs(5);
    spec.expect_bundle = Some("com.apple.TextEdit".into());
    let since = Instant::now();
    let pid = front.pid;
    let tok = token.clone();
    let mut probe = move || macos::focus(pid).value.is_some_and(|t| t.contains(&tok));
    let rep = take::run_probed(&mut ctx.guard, &spec, &Timing::R5, &mut probe);
    if !wait {
        macos::pump(Duration::from_millis(500));
        macos::play("Tink"); // "let go now"
    }
    let code = rep.v_keycode;
    v.v_arrived_with = macos::tapped_since(since)
        .into_iter()
        .filter(|(c, _)| Some(*c) == code)
        .map(|(_, f)| report::flags_label(f & HOTKEY_MODIFIERS))
        .collect();
    v.landed = match macos::focus(front.pid).value {
        _ if rep.decision == "not_pasted" => Landed::NotPasted,
        Some(t) if t.contains(&token) => Landed::Confirmed,
        Some(_) => Landed::Missing,
        None => Landed::Unverified,
    };
    ctx.ev("held", json!({"wait": wait, "variant": v, "take": rep}));
    v.take = Some(rep);
    wait_released(Duration::from_secs(15));
    v
}

fn held(ctx: &mut Ctx) -> HeldTest {
    let tap = ctx.r.permissions.listen_events && macos::start_key_tap();
    let mut h = HeldTest {
        tap_available: tap,
        no_wait: None,
        with_wait: None,
        owner_saw_other: None,
        note: None,
    };
    println!(
        "\n{RULE}\nStep 3 of 4: holding Control and Command (about 1 minute)\n{RULE}\n\
         Wispr Bro's \"paste last\" shortcut is Control+Command+V. People often still\n\
         hold Control and Command when the paste happens. This checks what arrives.\n\n\
         A TextEdit window opens. Then, twice:\n\
         \x20 1. Press and HOLD the Control (⌃) and Command (⌘) keys together.\n\
         \x20    Keep holding until you hear a sound, then let go.\n\
         \x20 2. Press and hold them again; let go as soon as you hear the sound.\n\
         Do not press any other key."
    );
    if !tap {
        println!(
            "  (The test cannot watch the keys arriving: Input Monitoring is off for this\n\
             \x20 terminal. It still checks what lands in TextEdit.)"
        );
    }
    match ctx
        .ui()
        .enter("Press Enter to start (or s to skip):")
        .as_deref()
    {
        None => return h,
        Some(s) if s.eq_ignore_ascii_case("s") => {
            h.note = Some("skipped by the owner".into());
            return h;
        }
        _ => {}
    }
    let Some(front) = open_textedit(ctx, "wb-held-test.txt") else {
        h.note = Some("TextEdit did not open with the cursor in the document".into());
        ctx.come_back();
        return h;
    };
    ctx.tidy
        .push("close the TextEdit window \"wb-held-test\" (no need to save)".into());
    println!("  Now: press and HOLD ⌃ and ⌘, and keep holding until you hear a sound.");
    h.no_wait = Some(held_variant(ctx, &front, false));
    if owner::interrupted() {
        return h;
    }
    println!("  Again: press and hold ⌃ and ⌘. Let go as soon as you hear the sound.");
    h.with_wait = Some(held_variant(ctx, &front, true));
    ctx.come_back();
    h.owner_saw_other = ctx.ui().yes_no(
        "While you held the keys the first time, did anything besides text happen in TextEdit (a beep, a menu, a different window)?",
    );
    h
}

// ---------------------------------------------------------------------------
// CJK input source (optional)

const IME_CANDIDATES: &[&str] = &[
    "com.apple.inputmethod.Kotoeri.RomajiTyping.Japanese",
    "com.apple.inputmethod.Kotoeri.KanaTyping.Japanese",
    "com.apple.inputmethod.SCIM.ITABC",
];
/// The input method a mode belongs to (it has to be on for the mode).
fn ime_parent(mode: &str) -> &str {
    mode.rsplit_once('.').map(|(p, _)| p).unwrap_or(mode)
}

/// The enabled input sources as macOS saved them. Read from the
/// preferences, because this process's TIS list goes stale after
/// enable/disable (seen while building RS-05: modes kept reporting
/// "enabled").
fn saved_input_sources() -> String {
    std::process::Command::new("/usr/bin/defaults")
        .args(["read", "com.apple.HIToolbox", "AppleEnabledInputSources"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

fn ime(ctx: &mut Ctx) -> Option<ImeTest> {
    let enabled_now: Vec<String> = macos::input_sources(false)
        .into_iter()
        .map(|s| s.id)
        .collect();
    let installed = macos::input_sources(true);
    let pick = IME_CANDIDATES
        .iter()
        .find(|id| enabled_now.iter().any(|e| e == **id))
        .or_else(|| {
            IME_CANDIDATES
                .iter()
                .find(|id| installed.iter().any(|s| s.id == **id))
        })
        .copied();
    let Some(id) = pick else {
        println!("\n(Step 4 skipped: no Japanese or Chinese input method found on this Mac.)");
        return None;
    };
    let enabled = enabled_now.iter().any(|e| e == id);
    let name = if id.contains("Kotoeri") {
        "Japanese"
    } else {
        "Chinese (Pinyin)"
    };
    // The self-check never turns on an input source by itself (a developer
    // can allow it with WB_RS05_SELFCHECK_IME=1; it is turned off again).
    if ctx.ui.is_none() && !enabled && std::env::var_os("WB_RS05_SELFCHECK_IME").is_none() {
        return None;
    }
    if let Some(ui) = &ctx.ui {
        println!(
            "\n{RULE}\nStep 4 of 4 (optional): pasting with a {name} keyboard (30 seconds)\n{RULE}\n\
             People who type Japanese or Chinese use an \"input method\". This checks that\n\
             a paste still lands while one is on.\n{}\n\
             It pastes into a TextEdit window twice: once normally, and once while a\n\
             word is half typed (the tool types \"nihon\" itself). Hands off meanwhile.",
            if enabled {
                format!("The test switches your keyboard to {name} for a few seconds, then back.")
            } else {
                format!(
                    "{name} is not switched on, so the test turns it on for a few seconds\n\
                     and removes it again afterwards (you may see it in the menu bar)."
                )
            }
        );
        if ui.yes_no("Run this step?") != Some(Ans::Yes) {
            return None;
        }
    }
    let original = macos::current_input_source_id();
    let saved_before = saved_input_sources();
    let mut t = ImeTest {
        input_source: id.into(),
        enabled_by_tool: false,
        selected: false,
        plain: None,
        composing: None,
        composing_field_tail: None,
        input_sources_restored: None,
        note: None,
    };
    ctx.ime_restore = Some((original, enabled_now.clone(), saved_before));
    if !enabled {
        for want in [ime_parent(id), id] {
            if let Some(s) = installed.iter().find(|s| s.id == want) {
                t.enabled_by_tool |= s.enable();
            }
        }
        macos::pump(Duration::from_millis(500));
    }
    let Some(front) = open_textedit(ctx, "wb-ime-test.txt") else {
        t.note = Some("TextEdit did not open with the cursor in the document".into());
        t.input_sources_restored = restore_ime(ctx);
        ctx.come_back();
        return Some(t);
    };
    ctx.tidy
        .push("close the TextEdit window \"wb-ime-test\" (no need to save)".into());
    let src = macos::input_sources(false).into_iter().find(|s| s.id == id);
    t.selected = src.as_ref().is_some_and(|s| s.select());
    macos::pump(Duration::from_millis(500));
    let now_id = macos::current_input_source_id();
    if now_id != id {
        t.note = Some(format!(
            "could not switch to {id}; the keyboard is {now_id}"
        ));
        t.selected = false;
    }
    if t.selected {
        for composing in [false, true] {
            if composing {
                // n i h o n on the ANSI positions (the IME's Latin layout).
                for code in [45u16, 34, 4, 31, 45] {
                    let _ = keys::post_key(code);
                    macos::pump(Duration::from_millis(60));
                }
                macos::pump(Duration::from_millis(300));
            }
            let text = ctx.next_text();
            let token = text.trim().to_string();
            let mut spec = TakeSpec::new(&text, TargetClass::Native);
            spec.expect_bundle = Some("com.apple.TextEdit".into());
            let pid = front.pid;
            let tok = token.clone();
            let mut probe = move || macos::focus(pid).value.is_some_and(|v| v.contains(&tok));
            let rep = take::run_probed(&mut ctx.guard, &spec, &Timing::R5, &mut probe);
            let value = macos::focus(front.pid).value;
            let landed = match &value {
                _ if rep.decision == "not_pasted" => Landed::NotPasted,
                Some(v) if v.contains(&token) => Landed::Confirmed,
                Some(_) => Landed::Missing,
                None => Landed::Unverified,
            };
            ctx.ev(
                "ime_take",
                json!({"composing": composing, "landed": landed, "take": rep}),
            );
            if composing {
                t.composing_field_tail = value.map(|v| {
                    let chars: Vec<char> = v.chars().collect();
                    chars[chars.len().saturating_sub(60)..].iter().collect()
                });
                // Leave no half-typed word behind.
                let _ = keys::post_key(53);
                t.composing = Some((landed, rep));
            } else {
                t.plain = Some((landed, rep));
            }
            macos::pump(Duration::from_millis(300));
        }
    }
    t.input_sources_restored = restore_ime(ctx);
    macos::play("Glass");
    ctx.come_back();
    println!(
        "  Paste with {name} on: {:?}; while a word was half typed: {:?}. Your keyboard is back to {}.",
        t.plain.as_ref().map(|x| x.0),
        t.composing.as_ref().map(|x| x.0),
        macos::current_input_source_id()
    );
    if t.input_sources_restored == Some(false) {
        println!(
            "  Your list of keyboards may still show {name}. To remove it: System Settings →\n\
             \x20 Keyboard → Text Input → Edit…, select {name} and click −."
        );
    }
    Some(t)
}

/// Selects the original input source again and turns off everything that
/// was not on before. Returns whether the saved list is as before.
fn restore_ime(ctx: &mut Ctx) -> Option<bool> {
    let (original, before, saved_before) = ctx.ime_restore.take()?;
    if let Some(s) = macos::input_sources(false)
        .iter()
        .find(|s| s.id == original)
    {
        s.select();
    }
    macos::pump(Duration::from_millis(200));
    let mut disabled = Vec::new();
    for _ in 0..2 {
        let all = macos::input_sources(true);
        for s in macos::input_sources(false) {
            if !before.contains(&s.id) {
                if let Some(x) = all.iter().find(|x| x.id == s.id) {
                    if x.disable() {
                        disabled.push(s.id.clone());
                    }
                }
                // The parent input method too (it switches on its siblings).
                let parent = ime_parent(&s.id).to_string();
                if !before.contains(&parent) {
                    if let Some(x) = all.iter().find(|x| x.id == parent) {
                        x.disable();
                    }
                }
            }
        }
        macos::pump(Duration::from_millis(300));
    }
    let same = saved_input_sources() == saved_before;
    ctx.ev(
        "ime_restored",
        json!({"input_source": macos::current_input_source_id(), "disabled": disabled, "saved_list_unchanged": same}),
    );
    Some(same)
}

// ---------------------------------------------------------------------------
// The end

fn finish(ctx: &mut Ctx) -> i32 {
    if let Some(p) = ctx.cat_pidfile.take() {
        kill_pidfile(&p);
    }
    restore_ime(ctx);
    ctx.r.interrupted = owner::interrupted();
    let restored = owner::restore();
    if let Some(r) = &restored {
        ctx.ev("owner_restored", json!(r));
    }
    ctx.r.owner_restore = restored;
    ctx.r.finished = chrono::Local::now().to_rfc3339();
    ctx.r.verdicts = report::verdicts(&ctx.r);
    let text = report::text(&ctx.r);
    let json_ok = serde_json::to_string_pretty(&ctx.r)
        .map_err(|e| e.to_string())
        .and_then(|j| fs::write(ctx.dir.join("summary.json"), j).map_err(|e| e.to_string()));
    let txt_ok = fs::write(ctx.dir.join("summary.txt"), &text);
    println!("\n{RULE}\nDone.\n{RULE}");
    match &ctx.r.owner_restore {
        Some(o) if o.identical => {
            println!("Your clipboard is back exactly as it was before the test.")
        }
        Some(o) => println!(
            "Your clipboard was put back, but not exactly ({}). A copy is kept in\n{}",
            o.detail,
            ctx.dir.join(owner::BACKUP_NAME).display()
        ),
        None => println!("Your clipboard was not changed."),
    }
    if ctx.ui.is_some() {
        println!("\nShort result:");
        for v in &ctx.r.verdicts {
            println!("  {}: {}", v.status.word(), v.criterion);
        }
        if !ctx.tidy.is_empty() {
            println!("\nYou can tidy up now:");
            ctx.tidy.dedup();
            for t in &ctx.tidy {
                println!("  - {t}");
            }
        }
    } else {
        print!("{text}");
    }
    println!(
        "\nResults: {}\n(summary.txt and summary.json; tell Claude you are done.)",
        ctx.dir.display()
    );
    if let Err(e) = json_ok {
        eprintln!("Could not write summary.json: {e}");
    }
    if let Err(e) = txt_ok {
        eprintln!("Could not write summary.txt: {e}");
    }
    if ctx.r.interrupted {
        130
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_is_asked_when_the_tool_cannot_tell() {
        assert!(ask_owner("cursor", Landed::Unverified));
        assert!(ask_owner("notes", Landed::Missing));
        assert!(!ask_owner("textedit", Landed::Missing));
        assert!(!ask_owner("notes", Landed::Confirmed));
    }

    #[test]
    fn input_method_parent() {
        assert_eq!(
            ime_parent("com.apple.inputmethod.Kotoeri.RomajiTyping.Japanese"),
            "com.apple.inputmethod.Kotoeri.RomajiTyping"
        );
        assert_eq!(
            ime_parent("com.apple.inputmethod.SCIM.ITABC"),
            "com.apple.inputmethod.SCIM"
        );
    }
}
