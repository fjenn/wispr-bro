// SPDX-License-Identifier: GPL-3.0-or-later
//! The guided Terminal session (and the unattended `--auto` run): opens each
//! app, asks the owner to click into a short list of places, notices when
//! focus moves there, records one focus event, and moves on. Writes
//! `events.jsonl` as it goes and the summary at the end (or on Ctrl+C).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use serde_json::json;

use crate::apps::{self, AppSpec, Expect};
use crate::ax::element as el;
use crate::ax::probe::{self, Poll};
use crate::ax::thread::{self, AxThread};
use crate::log::Log;
use crate::record::{self, Meta, Plan};
use crate::score::Scored;
use crate::summary::{self, RunInfo};
use crate::system;
use crate::testpage;
use crate::Options;

const RULE: &str = "------------------------------------------------------------------";
/// After this long without noticing the click, remind the owner of Enter.
const HINT_AFTER: Duration = Duration::from_secs(20);
/// Give up on one step after this long.
const STEP_LIMIT: Duration = Duration::from_secs(120);

pub fn intro() {
    println!(
        "\n{RULE}\n  Wispr Bro: text field test (RS-04)\n{RULE}\n\
         This checks how well Wispr Bro can tell where your cursor is: in a text\n\
         field (where dictated text should go) or somewhere else. For each app the\n\
         test opens it and asks you to click into a few places, one at a time.\n\
         It notices the click by itself and moves on (you'll hear a short beep).\n\n\
         It takes about 15 minutes. Nothing leaves this Mac and nothing you type is\n\
         saved (only things like \"text field, 12 characters selected\"). In a few\n\
         practice fields the test briefly types [wb] and removes it again.\n\n\
         Tips:\n  \
         - Make this Terminal window narrow and put it at the right edge of the\n    \
           screen, so you can read it while you click in the other apps.\n  \
         - If it doesn't notice your click, come back here and press Enter.\n  \
         - Type s and Enter to skip a step, q and Enter to skip the rest of an app.\n  \
         - Control+C stops the test; everything up to then is kept.\n"
    );
}

// ---------------------------------------------------------------------------
// Terminal input

pub enum Key {
    Enter,
    Skip,
    Quit,
    Other,
}

pub struct Ui {
    rx: Receiver<Option<String>>,
    eof: bool,
}

impl Ui {
    pub fn new() -> Ui {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("wb-ax-stdin".into())
            .spawn(move || {
                let stdin = std::io::stdin();
                let mut line = String::new();
                loop {
                    line.clear();
                    match stdin.read_line(&mut line) {
                        Ok(0) | Err(_) => {
                            let _ = tx.send(None);
                            return;
                        }
                        Ok(_) => {
                            if tx.send(Some(line.trim().to_string())).is_err() {
                                return;
                            }
                        }
                    }
                }
            })
            .expect("spawn stdin thread");
        Ui { rx, eof: false }
    }

    fn key(l: &str) -> Key {
        match l.to_lowercase().as_str() {
            "" => Key::Enter,
            "s" | "skip" => Key::Skip,
            "q" | "quit" => Key::Quit,
            _ => Key::Other,
        }
    }

    /// A line typed since the last call, if any (never blocks).
    fn try_key(&mut self) -> Option<Key> {
        match self.rx.try_recv() {
            Ok(Some(l)) => Some(Self::key(&l)),
            Ok(None) | Err(TryRecvError::Disconnected) => {
                self.eof = true;
                Some(Key::Quit)
            }
            Err(TryRecvError::Empty) => None,
        }
    }

    /// Waits for a line, running the run loop meanwhile. None at EOF.
    fn line(&mut self) -> Option<String> {
        if self.eof {
            return None;
        }
        loop {
            match self.rx.try_recv() {
                Ok(Some(l)) => return Some(l),
                Ok(None) | Err(TryRecvError::Disconnected) => {
                    self.eof = true;
                    return None;
                }
                Err(TryRecvError::Empty) => {
                    system::tick(Duration::from_millis(50));
                }
            }
        }
    }

    fn drain(&mut self) {
        system::flush_stdin();
        while let Ok(m) = self.rx.try_recv() {
            if m.is_none() {
                self.eof = true;
            }
        }
    }

    fn yes_no(&mut self, q: &str) -> Option<bool> {
        loop {
            print!("  ? {q}\n    (y = yes, n = no) ");
            let _ = std::io::stdout().flush();
            match self.line()?.to_lowercase().as_str() {
                "y" | "yes" => return Some(true),
                "n" | "no" => return Some(false),
                _ => println!("    Please type y or n and press Enter."),
            }
        }
    }

    fn enter(&mut self, prompt: &str) -> Option<Key> {
        print!("{prompt} ");
        let _ = std::io::stdout().flush();
        self.line().map(|l| Self::key(&l))
    }
}

// ---------------------------------------------------------------------------
// The session

pub fn results_dir(opts: &Options) -> PathBuf {
    let root = opts.out.clone().unwrap_or_else(|| {
        PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
            .join("wb-bench/results/rs04")
    });
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    root.join(if opts.auto {
        format!("{stamp}-auto")
    } else {
        stamp
    })
}

pub struct Session {
    ax: AxThread,
    log: Log,
    dir: PathBuf,
    page: PathBuf,
    ui: Option<Ui>,
    rows: Vec<Scored>,
    info: RunInfo,
}

enum Waited {
    Changed(&'static str),
    Enter,
    Skip,
    Quit,
}

fn beep() {
    print!("\x07");
    let _ = std::io::stdout().flush();
}

fn plain(s: &Scored) -> &'static str {
    match s.delivery.as_str() {
        "paste" => "looks like a text field",
        "paste_fail_open" => "can't tell for sure (would paste anyway)",
        "card" => "not a text field",
        "refuse" => "a password field",
        _ => "recorded",
    }
}

impl Session {
    fn new(opts: &Options) -> Result<Session, String> {
        let dir = results_dir(opts);
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        let log = Log::create(&dir.join("events.jsonl"), Instant::now())
            .map_err(|e| format!("cannot write events.jsonl: {e}"))?;
        let page = testpage::write(&dir).map_err(|e| format!("cannot write the test page: {e}"))?;
        let ax = AxThread::spawn();
        let stop_dir = dir.clone();
        thread::on_stop(move || {
            let (rows, info) = summary::read_rows(&stop_dir.join("events.jsonl"));
            if summary::write(&stop_dir, &info.unwrap_or_default(), &rows).is_ok() {
                println!("Results so far: {}", stop_dir.display());
            }
        });
        let machine = serde_json::to_value(system::machine_info()).unwrap_or_default();
        let info = RunInfo {
            tool: format!("wb-spike-ax {}", env!("CARGO_PKG_VERSION")),
            git_sha: option_env!("WB_GIT_SHA").unwrap_or("unknown").to_string(),
            started: chrono::Local::now().to_rfc3339(),
            mode: if opts.auto { "auto" } else { "guided" }.into(),
            machine,
            accessibility: system::accessibility_granted(),
            ..RunInfo::default()
        };
        log.write(
            "run",
            Instant::now(),
            json!({
                "tool": info.tool, "git_sha": info.git_sha, "started": info.started,
                "mode": info.mode, "machine": info.machine,
                "accessibility": info.accessibility, "options": opts,
            }),
        );
        Ok(Session {
            ax,
            log,
            dir,
            page,
            ui: (!opts.auto).then(Ui::new),
            rows: Vec::new(),
            info,
        })
    }

    fn skip(&mut self, app: &str, why: &str) {
        self.log
            .write("skip", Instant::now(), json!({"app": app, "why": why}));
        self.info.skipped.push((app.into(), why.into()));
    }

    fn poll(&self, mark: bool) -> Option<Poll> {
        self.ax
            .run(Duration::from_secs(2), move |c| probe::poll(c, mark, true))
    }

    #[allow(clippy::too_many_arguments)]
    fn capture(
        &mut self,
        app: &str,
        target: &str,
        expect: Option<Expect>,
        source: &str,
        confirmed: &str,
        plan: Plan,
        web_page: bool,
        expected_selection: Option<&'static str>,
        must_title: Option<String>,
    ) -> Option<Scored> {
        let front = system::tick(Duration::from_millis(5));
        let probes = self
            .ax
            .run(Duration::from_secs(30), move |ctx| record::run(ctx, &plan))?;
        if let Some(want) = must_title {
            let all = [
                Some(&probes.primary),
                probes.fast.as_ref(),
                probes.write.as_ref(),
            ];
            if all
                .iter()
                .flatten()
                .any(|p| p.window_title.as_deref() != Some(want.as_str()))
            {
                self.log.write(
                    "stale",
                    Instant::now(),
                    json!({"app": app, "target": target, "why": "the page moved on during the read"}),
                );
                return None;
            }
        }
        let meta = Meta {
            app,
            target,
            expect,
            source,
            web_page,
            expected_selection,
            front_pid: front,
        };
        let scored = record::scored(&meta, &probes);
        self.log.write(
            "event",
            Instant::now(),
            json!({
                "app": app, "target": target, "expect": expect, "source": source,
                "confirmed": confirmed, "front_pid": front,
                "scored": scored, "probes": probes,
            }),
        );
        self.rows.push(scored.clone());
        Some(scored)
    }

    fn activate(&mut self, spec: &AppSpec, path: &'static str, file: Option<&Path>) {
        let started = Instant::now();
        let ok = match file {
            Some(f) => system::open_with(spec.app, f),
            None => system::open_app(spec.app),
        };
        let a = system::wait_activation(&self.ax, path, started, Duration::from_secs(10));
        let v = json!({
            "app": spec.key, "opened": ok, "workspace_ms": a.workspace_ms,
            "ax_ms": a.ax_ms, "ax_failed": a.ax_failed,
        });
        self.log.write("activation", Instant::now(), v.clone());
        self.info.activation.push(v);
        system::tick(Duration::from_millis(600));
    }

    /// Waits until focus (or, for selections, the selected range) changes
    /// inside the app at `path`, then until it is stable.
    fn wait_target(&mut self, path: &'static str, expect: Expect) -> Waited {
        let base = self.poll(true);
        let base_sel = base.as_ref().and_then(|p| p.sel_range);
        let start = Instant::now();
        let mut hinted = false;
        loop {
            system::tick(Duration::from_millis(120));
            if let Some(ui) = self.ui.as_mut() {
                match ui.try_key() {
                    Some(Key::Enter) => return Waited::Enter,
                    Some(Key::Skip) => return Waited::Skip,
                    Some(Key::Quit) => return Waited::Quit,
                    Some(Key::Other) | None => {}
                }
            }
            let Some(p) = self.poll(false) else { continue };
            let in_app = p.app_pid.is_some_and(|pid| system::pid_is_app(pid, path));
            if in_app {
                let selected = p.sel_range.is_some_and(|r| r[1] > 0);
                let focus_moved = p.same_as_baseline == Some(false);
                let sel_moved = selected && p.sel_range != base_sel;
                let why = match expect {
                    Expect::Select if sel_moved => Some("selection_change"),
                    Expect::Select if focus_moved && selected => Some("focus_change"),
                    Expect::Select => None,
                    _ if focus_moved => Some("focus_change"),
                    _ => None,
                };
                if let Some(why) = why {
                    if self.settle(path, expect) {
                        return Waited::Changed(why);
                    }
                }
            }
            if !hinted && start.elapsed() > HINT_AFTER {
                hinted = true;
                println!(
                    "    (Not noticed yet. If you've done it, click here in Terminal and \
                     press Enter. s = skip this step.)"
                );
            }
            if start.elapsed() > STEP_LIMIT {
                return Waited::Skip;
            }
        }
    }

    /// The change must hold for 600 ms (multi-step prompts pass through
    /// other elements on the way).
    fn settle(&mut self, path: &'static str, expect: Expect) -> bool {
        for _ in 0..4 {
            let _ = self.poll(true);
            system::tick(Duration::from_millis(600));
            let Some(p) = self.poll(false) else {
                return false;
            };
            if !p.app_pid.is_some_and(|pid| system::pid_is_app(pid, path)) {
                return false;
            }
            let ok_sel = expect != Expect::Select || p.sel_range.is_some_and(|r| r[1] > 0);
            if p.same_as_baseline == Some(true) && ok_sel {
                return true;
            }
        }
        false
    }

    fn test_page(&mut self, spec: &AppSpec, path: &'static str) {
        let page = self.page.clone();
        self.activate(spec, path, Some(&page));
        let total = testpage::STEPS.len() as u32 * testpage::ROUNDS;
        let limit = Duration::from_millis((testpage::STEP_MS * (total + 4)) as u64)
            + Duration::from_secs(20);
        let start = Instant::now();
        let mut last: Option<String> = None;
        let mut seen = false;
        let mut warned = false;
        let mut n = 0;
        while start.elapsed() < limit {
            system::tick(Duration::from_millis(80));
            if let Some(ui) = self.ui.as_mut() {
                if matches!(ui.try_key(), Some(Key::Skip) | Some(Key::Quit)) {
                    break;
                }
            }
            let Some(p) = self.poll(false) else { continue };
            let Some(title) = p.test_title else {
                if !seen && !warned && start.elapsed() > Duration::from_secs(20) {
                    warned = true;
                    println!(
                        "    I can't see the test page. Is {} in front, showing it? If it \
                         doesn't start, type s and Enter to skip.",
                        spec.title
                    );
                }
                continue;
            };
            seen = true;
            if last.as_deref() == Some(title.as_str()) {
                continue;
            }
            last = Some(title.clone());
            if title == "WB-RS04 done" {
                break;
            }
            let Some(step) = testpage::parse_title(&title) else {
                continue;
            };
            system::tick(Duration::from_millis(350));
            let plan = Plan {
                allow_text: true,
                write_test: testpage::set_test_step(&step.name, step.round),
                menu_copy: step.expect == Expect::Select,
                single: false,
                lease_off_only: false,
            };
            let target = format!("page-r{}-{}", step.round, step.name);
            if let Some(s) = self.capture(
                spec.key,
                &target,
                Some(step.expect),
                "test_page",
                "title",
                plan,
                true,
                step.selection,
                Some(title.clone()),
            ) {
                n += 1;
                println!(
                    "    page step {:>2}/{total}: {:<16} → {}",
                    (step.round - 1) * testpage::STEPS.len() as u32 + step.n,
                    step.name,
                    plain(&s)
                );
            }
        }
        println!("    {n} test page steps recorded in {}.", spec.title);
    }

    fn guided_app(&mut self, spec: &'static AppSpec, k: usize, of: usize) -> bool {
        let Some(path) = apps::installed_path(spec) else {
            self.skip(spec.key, "not installed");
            return true;
        };
        println!("\n{RULE}\n  App {k} of {of}: {}\n{RULE}", spec.title);
        let Some(ui) = self.ui.as_mut() else {
            return true;
        };
        if let Some(q) = spec.ask {
            match ui.yes_no(q) {
                Some(true) => {}
                Some(false) => {
                    self.skip(spec.key, "owner said no (not signed in or not wanted)");
                    return true;
                }
                None => return false,
            }
        }
        println!("{}", spec.intro);
        match ui.enter("Press Enter to start (s = skip this app):") {
            Some(Key::Skip) | Some(Key::Quit) => {
                self.skip(spec.key, "skipped by the owner");
                return true;
            }
            None => return false,
            _ => {}
        }
        let doc = if spec.scratch_doc {
            let f = self.dir.join("rs04-practice.txt");
            let _ = std::fs::write(&f, apps::PRACTICE_TEXT);
            Some(f)
        } else {
            None
        };
        if spec.test_page {
            self.activate(spec, path, None);
            self.cold(spec);
            self.test_page(spec, path);
            println!("  Now a few clicks in {}.", spec.title);
        } else {
            self.activate(spec, path, doc.as_deref());
            self.cold(spec);
        }
        let n = spec.targets.len();
        let browser = matches!(spec.key, "safari" | "chrome" | "gmail" | "gdocs");
        for (i, t) in spec.targets.iter().enumerate() {
            if let Some(ui) = self.ui.as_mut() {
                ui.drain();
            }
            beep();
            println!("\n  Step {} of {n} in {}: {}", i + 1, spec.title, t.prompt);
            let waited = self.wait_target(path, t.expect);
            let confirmed = match waited {
                Waited::Changed(why) => why,
                Waited::Enter => {
                    self.activate(spec, path, None);
                    "enter"
                }
                Waited::Skip => {
                    println!("    skipped.");
                    self.log.write(
                        "step_skipped",
                        Instant::now(),
                        json!({"app": spec.key, "target": t.id}),
                    );
                    continue;
                }
                Waited::Quit => {
                    println!("    skipping the rest of {}.", spec.title);
                    break;
                }
            };
            let plan = Plan {
                allow_text: spec.scratch_doc && t.scratch,
                write_test: t.scratch,
                menu_copy: t.expect == Expect::Select,
                single: false,
                lease_off_only: false,
            };
            let web = browser && t.id != "address";
            if let Some(s) = self.capture(
                spec.key,
                t.id,
                Some(t.expect),
                "guided",
                confirmed,
                plan,
                web,
                None,
                None,
            ) {
                println!("    ✓ recorded: {}", plain(&s));
            }
        }
        beep();
        println!("\n  {}", spec.outro);
        if let Some(ui) = self.ui.as_mut() {
            if ui
                .enter("  Press Enter when you're ready for the next app.")
                .is_none()
            {
                return false;
            }
        }
        true
    }

    /// A first look at a Chromium/Electron app before our lease touches it:
    /// the clean "without lease" reading (not scored; focus is wherever the
    /// app put it).
    fn cold(&mut self, spec: &AppSpec) {
        let builds_on_request = apps::installed_path(spec)
            .is_some_and(|p| probe::family_of(p).builds_tree_on_request());
        if !builds_on_request {
            return;
        }
        let plan = Plan {
            allow_text: false,
            write_test: false,
            menu_copy: false,
            single: true,
            lease_off_only: false,
        };
        let _ = self.capture(
            spec.key,
            "cold",
            None,
            "cold",
            "activation",
            plan,
            false,
            None,
            None,
        );
    }

    fn finish(&mut self) -> i32 {
        self.ax.shutdown();
        self.log.write("end", Instant::now(), json!({}));
        self.info.interrupted = false;
        match summary::write(&self.dir, &self.info, &self.rows) {
            Ok(txt) => {
                println!("\n{RULE}\n{txt}{RULE}");
                println!(
                    "Saved in {}\n(summary.txt, summary.json, events.jsonl, the test page and \
                     practice file).\nTell Claude you're done; it reads the results from there.",
                    self.dir.display()
                );
                0
            }
            Err(e) => {
                eprintln!("could not write the summary: {e}");
                1
            }
        }
    }
}

fn ensure_accessibility(ui: &mut Ui, term: &str) -> Option<()> {
    if system::accessibility_granted() {
        return Some(());
    }
    println!(
        "\n{term} needs the Accessibility permission for this test.\n\
         \n  1. Open System Settings (Apple menu  → System Settings).\n  \
         2. Click \"Privacy & Security\" in the sidebar, then \"Accessibility\".\n  \
         3. Find \"{term}\" in the list and switch it ON. If it is not in the list,\n     \
         click the + button, choose Applications → Utilities → {term}, and switch it on.\n  \
         4. macOS may ask for your password or Touch ID. That's expected.\n"
    );
    if ui.yes_no("Shall I open that page of System Settings for you?")? {
        system::open_accessibility_pane();
    }
    print!("  Waiting for Accessibility to be switched on");
    let _ = std::io::stdout().flush();
    let start = Instant::now();
    let mut hinted = false;
    while !system::accessibility_granted() {
        system::tick(Duration::from_secs(1));
        print!(".");
        let _ = std::io::stdout().flush();
        if !hinted && start.elapsed() > Duration::from_secs(90) {
            hinted = true;
            println!(
                "\n  Still not seeing it. If the switch is already on: quit {term} completely\n  \
                 (⌘Q), open it again, and run the same command again."
            );
        }
    }
    println!(" thanks, it's on.");
    Some(())
}

fn wait_secure_input_off() {
    if !el::secure_input_on() {
        return;
    }
    println!(
        "\nSecure Input is on, which makes every field look like a password field.\n\
         Usually this is Terminal's \"Secure Keyboard Entry\": click the Terminal menu\n\
         (top left) and untick \"Secure Keyboard Entry\". A password box with the\n\
         cursor in it (in any app) does the same: click away from it."
    );
    print!("  Waiting for Secure Input to turn off");
    let _ = std::io::stdout().flush();
    while el::secure_input_on() {
        system::tick(Duration::from_secs(1));
        print!(".");
        let _ = std::io::stdout().flush();
    }
    println!(" thanks.");
}

pub fn selected(opts: &Options) -> Vec<&'static AppSpec> {
    apps::APPS
        .iter()
        .filter(|a| opts.apps.is_empty() || opts.apps.iter().any(|k| k == a.key))
        .collect()
}

pub fn run(opts: Options) -> i32 {
    intro();
    let term = system::terminal_name(std::env::var("TERM_PROGRAM").ok().as_deref());
    let mut s = match Session::new(&opts) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    {
        let ui = s.ui.as_mut().expect("guided session has a terminal");
        if ensure_accessibility(ui, &term).is_none() {
            return 1;
        }
        s.info.accessibility = true;
        wait_secure_input_off();
        if ui.enter("Press Enter to begin.").is_none() {
            return 1;
        }
    }
    let list = selected(&opts);
    let of = list.len();
    for (k, spec) in list.into_iter().enumerate() {
        if !s.guided_app(spec, k + 1, of) {
            println!("\nInput ended; finishing.");
            break;
        }
    }
    s.finish()
}

/// `--auto`: what can run without the owner. TextEdit (the tool moves the
/// caret and selection itself through AX) and the test page in Safari and
/// Chrome (the page moves focus itself). No clicks, no prompts.
pub fn auto(opts: Options) -> i32 {
    let mut s = match Session::new(&opts) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    println!(
        "wb-spike-ax --auto: results in {}\nAccessibility permission: {}",
        s.dir.display(),
        if s.info.accessibility { "on" } else { "OFF" }
    );
    for spec in selected(&opts) {
        let Some(path) = apps::installed_path(spec) else {
            s.skip(spec.key, "not installed");
            continue;
        };
        match spec.key {
            "textedit" => s.auto_textedit(spec, path),
            "safari" | "chrome" => {
                s.activate(spec, path, None);
                s.cold(spec);
                println!(
                    "{}: test page ({} steps)...",
                    spec.title,
                    testpage::STEPS.len() as u32 * testpage::ROUNDS
                );
                s.test_page(spec, path);
            }
            "cursor" | "obsidian" | "claude" | "chatgpt" => {
                // No clicks possible: one look at whatever has focus after
                // activation, first without our lease, then with it (not
                // scored: there is no ground truth).
                s.activate(spec, path, None);
                s.cold(spec);
                let plan = Plan {
                    allow_text: false,
                    write_test: false,
                    menu_copy: false,
                    single: false,
                    lease_off_only: false,
                };
                if let Some(r) = s.capture(
                    spec.key,
                    "as-opened",
                    None,
                    "auto",
                    "activation",
                    plan,
                    false,
                    None,
                    None,
                ) {
                    println!(
                        "{}: focused {} → {} (without lease: {})",
                        spec.title,
                        r.role.as_deref().unwrap_or("nothing"),
                        r.delivery,
                        r.delivery_without_lease.as_deref().unwrap_or("–")
                    );
                }
            }
            _ => s.skip(spec.key, "needs the owner (guided only)"),
        }
    }
    s.finish()
}

impl Session {
    fn auto_textedit(&mut self, spec: &'static AppSpec, path: &'static str) {
        let f = self.dir.join("rs04-practice.txt");
        if std::fs::write(&f, apps::PRACTICE_TEXT).is_err() {
            return;
        }
        self.activate(spec, path, Some(&f));
        let text = apps::PRACTICE_TEXT;
        let units = |s: &str| s.encode_utf16().count() as isize;
        let at = |w: &str| text.find(w).map(|i| units(&text[..i])).unwrap_or(0);
        let words: [&'static str; 4] = ["brown fox", "lazy dog", "second line", "results folder"];
        let mut steps: Vec<AutoStep> = Vec::new();
        for (i, w) in words.iter().enumerate() {
            steps.push((format!("caret-{i}"), at(w), 0, Expect::Text, None, i == 0));
            steps.push((
                format!("select-{i}"),
                at(w),
                units(w),
                Expect::Select,
                Some(*w),
                false,
            ));
        }
        steps.push(("end".into(), units(text), 0, Expect::Text, None, true));
        println!("TextEdit: {} steps driven through AX...", steps.len());
        for (name, loc, len, expect, sel, write) in steps {
            let moved = self
                .ax
                .run(Duration::from_secs(2), move |ctx| {
                    let pid = thread::front_pid()?;
                    let app = ctx.app(pid);
                    el::set_timeout(&app, 0.25);
                    let f = el::attr_element(&app, "AXFocusedUIElement")?;
                    let r = objc2_core_foundation::CFRange {
                        location: loc,
                        length: len,
                    };
                    Some(
                        el::set_range(&f, "AXSelectedTextRange", r)
                            == objc2_application_services::AXError::Success,
                    )
                })
                .flatten();
            system::tick(Duration::from_millis(300));
            if moved != Some(true) {
                println!("  {name}: could not move the selection");
                continue;
            }
            let plan = Plan {
                allow_text: true,
                write_test: write,
                menu_copy: expect == Expect::Select,
                single: false,
                lease_off_only: false,
            };
            if let Some(s) = self.capture(
                spec.key,
                &name,
                Some(expect),
                "auto",
                "ax_driven",
                plan,
                false,
                sel,
                None,
            ) {
                println!("  {name:<10} → {}", plain(&s));
            }
        }
    }
}

/// (name, location, length, expectation, selected text, write test).
type AutoStep = (String, isize, isize, Expect, Option<&'static str>, bool);
