// SPDX-License-Identifier: GPL-3.0-or-later
//! The guided Terminal session: plain-English steps for the owner,
//! permission checks, the test blocks, the y/n questions, and the result
//! files.

use serde::Serialize;
use serde_json::json;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::engine::{BlockRun, BlockSpec, Engine, Msg, UiMsg};
use crate::globe::{self, FnUsage, GlobeSetting};
use crate::grammar::{Grammar, Timings};
use crate::log::Log;
use crate::mic::MicCmd;
use crate::summary::{self, Ans, Answer, HealthCheck};
use crate::system::{self, Pane};
use crate::taps::{self, TapMsg, Taps};
use crate::verdict::{BlockKind, Outcome};
use crate::{mic, Options};

const RULE: &str = "------------------------------------------------------------------";

pub fn intro() {
    println!(
        "\n{RULE}\n  Wispr Bro: fn / 🌐 key test (RS-01)\n{RULE}\n\
         This checks how the fn key (the 🌐 globe key, bottom left) behaves when\n\
         Wispr Bro is listening to it. You will hold, tap and double-tap fn, try\n\
         a few fn shortcuts in TextEdit, and answer simple yes/no questions about\n\
         what you saw on screen.\n\n\
         It takes about 15-20 minutes. Nothing leaves this Mac. The only audio kept\n\
         is 10 short clips of you saying \"testing one two three\" (4 seconds each),\n\
         saved next to the results. You can stop at any time with Control+C;\n\
         everything up to that point is kept.\n"
    );
}

// ---------------------------------------------------------------------------
// Terminal input

pub struct Ui {
    pub(crate) rx: Receiver<UiMsg>,
    /// Stdin ended (once seen, every later read returns None at once).
    pub(crate) eof: std::cell::Cell<bool>,
}

pub enum BlockEnd {
    Done,
    Enter,
    Skip,
    Eof,
}

impl Ui {
    fn line(&self) -> Option<String> {
        loop {
            if self.eof.get() {
                return None;
            }
            match self.rx.recv() {
                Ok(UiMsg::Line(l)) => return Some(l.trim().to_string()),
                Ok(UiMsg::BlockDone) | Ok(UiMsg::FnSeen) | Ok(UiMsg::EpisodeDone { .. }) => {
                    continue
                }
                Ok(UiMsg::Eof) | Err(_) => {
                    self.eof.set(true);
                    return None;
                }
            }
        }
    }

    /// Throws away typed-ahead input and stale notices.
    pub(crate) fn drain(&self) {
        system::flush_stdin();
        while let Ok(m) = self.rx.try_recv() {
            if matches!(m, UiMsg::Eof) {
                self.eof.set(true);
            }
        }
    }

    pub(crate) fn enter(&self, prompt: &str) -> Option<String> {
        print!("{prompt} ");
        let _ = std::io::stdout().flush();
        self.line()
    }

    pub(crate) fn yes_no(&self, q: &str) -> Option<Ans> {
        loop {
            print!("  ? {q}\n    (y = yes, n = no, u = not sure) ");
            let _ = std::io::stdout().flush();
            let l = self.line()?;
            match l.to_lowercase().as_str() {
                "y" | "yes" => return Some(Ans::Yes),
                "n" | "no" => return Some(Ans::No),
                "u" | "?" | "not sure" | "unsure" => return Some(Ans::Unsure),
                _ => println!("    Please type y, n or u and press Enter."),
            }
        }
    }

    pub(crate) fn choice(&self, q: &str, options: &[&str]) -> Option<usize> {
        loop {
            println!("  ? {q}");
            for (i, o) in options.iter().enumerate() {
                println!("      {}  {o}", i + 1);
            }
            print!("    Type a number and press Enter: ");
            let _ = std::io::stdout().flush();
            let l = self.line()?;
            if let Ok(n) = l.parse::<usize>() {
                if (1..=options.len()).contains(&n) {
                    return Some(n - 1);
                }
            }
        }
    }

    /// Waits for the block to end. With a `cue`, prints a ready cue and then
    /// the cue line before the first attempt and after every attempt until
    /// the block's target is reached (round 2's clipping takes).
    fn wait_block(&self, cue: Option<&str>, target: u32) -> BlockEnd {
        let show_cue = |n: u32, target: u32| {
            if let Some(c) = cue {
                println!("\n    Take {n} of {target}: get ready...");
                std::thread::sleep(Duration::from_millis(1200));
                println!("    >>> NOW: {c}");
            }
        };
        let mut cued = false;
        loop {
            if self.eof.get() {
                return BlockEnd::Eof;
            }
            if cue.is_some() && !cued {
                cued = true;
                show_cue(1, target);
            }
            match self.rx.recv() {
                Ok(UiMsg::BlockDone) => return BlockEnd::Done,
                Ok(UiMsg::EpisodeDone { valid, target }) => {
                    if valid < target {
                        std::thread::sleep(Duration::from_millis(600));
                        show_cue(valid + 1, target);
                    }
                }
                Ok(UiMsg::Line(l)) if l.trim().eq_ignore_ascii_case("s") => return BlockEnd::Skip,
                Ok(UiMsg::Line(l)) if l.trim().is_empty() => return BlockEnd::Enter,
                Ok(UiMsg::Line(_)) | Ok(UiMsg::FnSeen) => continue,
                Ok(UiMsg::Eof) | Err(_) => {
                    self.eof.set(true);
                    return BlockEnd::Eof;
                }
            }
        }
    }

    pub(crate) fn wait_fn(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(UiMsg::FnSeen) => return true,
                Ok(UiMsg::Eof) => {
                    self.eof.set(true);
                    return false;
                }
                Ok(_) => continue,
                Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => {
                    return false
                }
            }
        }
    }
}

pub(crate) fn spawn_stdin(tx: Sender<UiMsg>) {
    std::thread::Builder::new()
        .name("wb-spike-fn-stdin".into())
        .spawn(move || {
            let stdin = std::io::stdin();
            let mut line = String::new();
            loop {
                line.clear();
                match stdin.read_line(&mut line) {
                    Ok(0) | Err(_) => {
                        let _ = tx.send(UiMsg::Eof);
                        return;
                    }
                    Ok(_) => {
                        if tx.send(UiMsg::Line(line.clone())).is_err() {
                            return;
                        }
                    }
                }
            }
        })
        .expect("spawn stdin thread");
}

// ---------------------------------------------------------------------------
// Permissions

#[derive(Clone, Debug, Serialize)]
pub struct Permissions {
    pub accessibility: bool,
    pub input_monitoring: bool,
    pub microphone: String,
    pub secure_input: bool,
}

pub fn read_permissions() -> Permissions {
    Permissions {
        accessibility: system::accessibility_granted(),
        input_monitoring: system::input_monitoring_granted(),
        microphone: format!("{:?}", handy_recorder::permission_status()),
        secure_input: system::secure_input_on(),
    }
}

pub fn print_permissions(p: &Permissions, term: &str) {
    let word = |b: bool| if b { "on" } else { "OFF" };
    println!("Permissions for {term}:");
    println!(
        "  Accessibility ......... {}   (needed: lets the test see and hold back the fn key)",
        word(p.accessibility)
    );
    println!(
        "  Input Monitoring ...... {}   (may be needed on some Macs; checked with a test press)",
        word(p.input_monitoring)
    );
    println!(
        "  Microphone ............ {}   (needed: the orange-dot test opens the mic)",
        p.microphone
    );
    if p.secure_input {
        println!("  !! Secure Input is ON (see below): the test cannot see most keys while it is.");
    }
}

fn wait_until(what: &str, check: impl Fn() -> bool, term: &str) {
    let start = Instant::now();
    let mut hinted = false;
    print!("  Waiting for {what} to be switched on");
    let _ = std::io::stdout().flush();
    while !check() {
        std::thread::sleep(Duration::from_secs(1));
        print!(".");
        let _ = std::io::stdout().flush();
        if !hinted && start.elapsed() > Duration::from_secs(90) {
            hinted = true;
            println!(
                "\n  Still not seeing it. If the switch is already on: quit {term} completely\n  \
                 (press ⌘Q in {term}), open it again, and run the same command again."
            );
        }
    }
    println!(" thanks, it's on.");
}

pub(crate) fn ensure_accessibility(ui: &Ui, term: &str) -> Option<()> {
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
    if ui.yes_no("Shall I open that page of System Settings for you?")? == Ans::Yes {
        system::open_pane(Pane::Accessibility);
    }
    wait_until("Accessibility", system::accessibility_granted, term);
    Some(())
}

pub(crate) fn ensure_secure_input_off(term: &str) {
    if !system::secure_input_on() {
        return;
    }
    println!(
        "\nSecure Input is on. While it is, no app can see key presses other than fn.\n\
         Usually this is {term}'s \"Secure Keyboard Entry\": click the {term} menu\n\
         (top left, next to ) and untick \"Secure Keyboard Entry\". A password box\n\
         that has the cursor in it (in any app) does the same: click away from it."
    );
    wait_until_off();
}

fn wait_until_off() {
    print!("  Waiting for Secure Input to turn off");
    let _ = std::io::stdout().flush();
    while system::secure_input_on() {
        std::thread::sleep(Duration::from_secs(1));
        print!(".");
        let _ = std::io::stdout().flush();
    }
    println!(" thanks.");
}

pub(crate) fn ensure_microphone(ui: &Ui, term: &str) -> Option<String> {
    use handy_recorder::Permission;
    match handy_recorder::permission_status() {
        Permission::Denied => {
            println!(
                "\n{term} is not allowed to use the microphone.\n\
                 \n  1. Open System Settings → Privacy & Security → Microphone.\n  \
                 2. Switch \"{term}\" ON.\n  \
                 3. macOS will ask to quit and reopen {term}: click \"Quit & Reopen\",\n     \
                 then run the same command again (the test starts over).\n"
            );
            if ui.yes_no("Shall I open that page of System Settings for you?")? == Ans::Yes {
                system::open_pane(Pane::Microphone);
            }
            wait_until(
                "Microphone",
                || handy_recorder::permission_status() == Permission::Granted,
                term,
            );
        }
        Permission::NotDetermined => {
            println!(
                "\nNext, macOS will ask whether {term} may use the microphone.\n\
                 Please click \"Allow\". (Press Enter to continue.)"
            );
            ui.line()?;
        }
        _ => {}
    }
    println!("\nChecking the microphone (1 second)...");
    match mic::probe(Duration::from_secs(1)) {
        Ok((device, peak, _)) => {
            println!("  Microphone: {device}");
            if peak == 0.0 {
                println!(
                    "  !! The microphone sent pure silence. If macOS just asked for permission,\n  \
                     that's fine. Otherwise check System Settings → Sound → Input."
                );
            }
            if device.to_lowercase().contains("airpods") {
                println!(
                    "  Tip: AirPods are the microphone right now. For this test the Mac's own\n  \
                     microphone is better: take the AirPods out, or pick \"MacBook Microphone\"\n  \
                     in System Settings → Sound → Input, then continue."
                );
            }
            Some(device)
        }
        Err(e) => {
            println!("  !! Could not open the microphone: {e}");
            println!("  The test continues, but the orange-dot checks will not work.");
            Some(format!("error: {e}"))
        }
    }
}

// ---------------------------------------------------------------------------
// The session

pub(crate) struct Q {
    pub(crate) id: &'static str,
    pub(crate) text: &'static str,
    pub(crate) expected: Option<Ans>,
}

pub(crate) const fn q(id: &'static str, text: &'static str, expected: Ans) -> Q {
    Q {
        id,
        text,
        expected: Some(expected),
    }
}

pub(crate) const POPUP: &str = "Did anything else pop up or change while you did that: the emoji picker,\n    the keyboard language / input source switching, or Apple's own Dictation?";

pub(crate) struct BlockText {
    pub(crate) kind: BlockKind,
    pub(crate) title: &'static str,
    pub(crate) target: u32,
    pub(crate) auto_end: bool,
    pub(crate) steps: &'static str,
    pub(crate) questions: Vec<Q>,
    /// Printed (after a ready cue) before each attempt; round 2's takes.
    pub(crate) cue: Option<&'static str>,
}

fn block_texts() -> Vec<BlockText> {
    use Ans::*;
    vec![
        BlockText {
            kind: BlockKind::Hold,
            cue: None,
            title: "Hold fn",
            target: 10,
            auto_end: true,
            steps: "Hold the fn key (🌐, bottom left) for about one second, then let go.\n\
                    Do it 10 times, with a short pause in between. You can stay in this window.\n\
                    Watch the top right of the screen: an orange dot should show while you\n\
                    hold and disappear after you let go.",
            questions: vec![
                q("popup", POPUP, No),
                q("dot", "Did the orange microphone dot appear (top right) while you held fn?", Yes),
                q("dot_gone", "Did the dot go away within a second or two after you let go?", Yes),
            ],
        },
        BlockText {
            kind: BlockKind::QuickTap,
            cue: None,
            title: "Quick taps",
            target: 10,
            auto_end: true,
            steps: "Tap fn very quickly: press and let go at once, like tapping a key.\n\
                    Do it 10 times, waiting about one second between taps\n\
                    (two taps close together would count as a double-tap).\n\
                    This time the orange dot should NOT appear.",
            questions: vec![
                q("popup", POPUP, No),
                q("dot", "Did the orange dot appear at any point during the quick taps?", No),
            ],
        },
        BlockText {
            kind: BlockKind::DoubleTap,
            cue: None,
            title: "Double-tap",
            target: 10,
            auto_end: true,
            steps: "Press fn twice quickly (like a double-click). The orange dot should appear\n\
                    and stay. Wait about two seconds, then press fn once to stop.\n\
                    Do that 10 times.",
            questions: vec![
                q("popup", POPUP, No),
                q(
                    "dot",
                    "Did the orange dot appear after the double-tap and stay until your single press?",
                    Yes,
                ),
            ],
        },
        BlockText {
            kind: BlockKind::FnSpace,
            cue: None,
            title: "fn + Space",
            target: 5,
            auto_end: true,
            steps: "First click into the TextEdit window (on the text).\n\
                    Then: hold fn, press Space once while still holding fn, let go of both.\n\
                    Wait about two seconds, then press fn once to stop.\n\
                    Do that 5 times, then come back to this window.\n\
                    No spaces should appear in TextEdit.",
            questions: vec![
                q("space_typed", "Did any spaces get typed into TextEdit?", No),
                q("popup", POPUP, No),
                q("dot", "Did the orange dot appear after each fn+Space?", Yes),
            ],
        },
        BlockText {
            kind: BlockKind::EscHold,
            cue: None,
            title: "Esc while holding fn",
            target: 5,
            auto_end: true,
            steps: "Hold fn. When the orange dot appears (after about a second), press Esc\n\
                    (top left) while still holding fn, then let go.\n\
                    Do that 5 times. You can stay in this window.",
            questions: vec![q(
                "dot_gone",
                "Did the orange dot go away (within a second or two) when you pressed Esc?",
                Yes,
            )],
        },
        BlockText {
            kind: BlockKind::Chords,
            cue: None,
            title: "fn shortcuts in TextEdit (30 presses)",
            target: 30,
            auto_end: false,
            steps: "Click into the TextEdit window, somewhere in the middle of the text.\n\
                    Hold fn and press the other key, at your normal speed, then let go:\n\
                    \n  fn + ←       5 times   (jumps to the top of the document)\n  \
                    fn + →       5 times   (jumps to the bottom)\n  \
                    fn + ↑       5 times   (scrolls up one page)\n  \
                    fn + ↓       5 times   (scrolls down one page)\n  \
                    fn + delete  5 times   (deletes the letter to the RIGHT of the cursor;\n                          \
                    click into a line of text first)\n  \
                    fn + F11     4 times   (shows the desktop, then brings the windows back;\n                          \
                    if your Mac uses F1, F2… as standard keys it changes\n                          \
                    the volume instead, which is fine)\n  \
                    fn + F1      1 time\n\
                    \nWhen you're done, click back into this window and press Enter.\n\
                    (RS-01 asks for 100 such presses; 30 is enough for a first run.)",
            questions: vec![
                q("arrows", "Did fn+← and fn+→ jump to the top and bottom of the document?", Yes),
                q("pages", "Did fn+↑ and fn+↓ scroll up and down by a page?", Yes),
                q(
                    "fwd_delete",
                    "Did fn+delete delete the letter to the RIGHT of the cursor?",
                    Yes,
                ),
                q("fkeys", "Did fn+F11 do what F11 normally does on your Mac?", Yes),
                q("dot", "Did the orange dot appear at ANY point during these shortcuts?", No),
                q("popup", POPUP, No),
            ],
        },
        BlockText {
            kind: BlockKind::Clipping,
            cue: None,
            title: "First words (10 takes)",
            target: 10,
            auto_end: true,
            steps: "Hold fn and, at the very same moment, start saying\n\
                    \"testing one two three\" in your normal voice. Keep holding until you've\n\
                    finished, then let go. Wait two seconds and repeat: 10 times.\n\
                    You can stay in this window. Only the first 2 seconds of each take are\n\
                    saved, on this Mac. (RS-01 asks for 50 takes; 10 is enough for a first run.)",
            questions: vec![q("dot", "Did the orange dot appear for each take?", Yes)],
        },
    ]
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct GlobeRound {
    pub(crate) round: String,
    pub(crate) expected: Option<FnUsage>,
    pub(crate) read: GlobeSetting,
    /// Owner's confirmation that System Settings shows what we read.
    pub(crate) owner_confirms: Option<Ans>,
    pub(crate) owner_says: Option<FnUsage>,
}

pub(crate) struct Session {
    pub(crate) opts: Options,
    pub(crate) dir: PathBuf,
    pub(crate) log: Arc<Log>,
    pub(crate) ui: Ui,
    pub(crate) tx: Sender<Msg>,
    pub(crate) taps: Option<Taps>,
    pub(crate) mic_tx: Option<Sender<MicCmd>>,
    pub(crate) term: String,
    pub(crate) started_local: String,
    pub(crate) machine: system::MachineInfo,
    pub(crate) permissions: Permissions,
    pub(crate) mic_device: Option<String>,
    pub(crate) runs: Vec<BlockRun>,
    pub(crate) answers: Vec<Answer>,
    pub(crate) health: Vec<HealthCheck>,
    pub(crate) globe_rounds: Vec<GlobeRound>,
    pub(crate) original_globe: Option<FnUsage>,
    pub(crate) builtin_keyboard: Option<Ans>,
    /// The grammar's timings and fn-block option for this run.
    pub(crate) timings: Timings,
    pub(crate) block_fn: bool,
    /// Round 2's extra results (per-take analysis, chord timing, notes).
    pub(crate) extra: serde_json::Map<String, serde_json::Value>,
}

pub fn results_dir(opts: &Options) -> PathBuf {
    let root = opts.out.clone().unwrap_or_else(|| {
        PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
            .join("wb-bench/results/rs01")
    });
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    root.join(if opts.round == 2 {
        format!("{stamp}-r2")
    } else {
        stamp
    })
}

pub(crate) fn write_practice_text(path: &Path) -> std::io::Result<()> {
    let mut s = String::from(
        "Wispr Bro fn test: practice text. Nothing here is saved anywhere; you can\n\
         close this window without saving when the test is done.\n\n",
    );
    for i in 1..=400 {
        s.push_str(&format!(
            "Line {i:03}: The quick brown fox jumps over the lazy dog, then naps in the sun.\n"
        ));
    }
    std::fs::write(path, s)
}

pub fn run(opts: Options) -> i32 {
    let round2 = opts.round == 2;
    if round2 {
        crate::round2::intro();
    } else {
        intro();
    }
    let (timings, block_fn) = if round2 {
        (crate::round2::TIMINGS, false)
    } else {
        (Timings::DEFAULT, true)
    };
    let term = system::terminal_name(std::env::var("TERM_PROGRAM").ok().as_deref());
    let dir = results_dir(&opts);
    if let Err(e) = std::fs::create_dir_all(dir.join("takes")) {
        eprintln!("Could not create {}: {e}", dir.display());
        return 1;
    }
    let epoch = Instant::now();
    let log = match Log::create(&dir.join("events.jsonl"), epoch) {
        Ok(l) => Arc::new(l),
        Err(e) => {
            eprintln!("Could not create the log: {e}");
            return 1;
        }
    };
    let (ui_tx, ui_rx) = mpsc::channel();
    spawn_stdin(ui_tx.clone());
    let (tx, rx) = mpsc::channel::<Msg>();
    let machine = system::machine_info();
    let permissions = read_permissions();
    log.write(
        "session_start",
        Instant::now(),
        json!({"machine": machine, "permissions": permissions, "timings": timings,
               "block_fn": block_fn, "round": opts.round,
               "tool_version": env!("CARGO_PKG_VERSION"), "git": option_env!("WB_GIT_SHA")}),
    );
    let mut s = Session {
        opts,
        dir,
        log,
        ui: Ui {
            rx: ui_rx,
            eof: std::cell::Cell::new(false),
        },
        tx,
        taps: None,
        mic_tx: None,
        term,
        started_local: chrono::Local::now().to_rfc3339(),
        machine,
        permissions,
        mic_device: None,
        runs: Vec::new(),
        answers: Vec::new(),
        health: Vec::new(),
        globe_rounds: Vec::new(),
        original_globe: None,
        builtin_keyboard: None,
        timings,
        block_fn,
        extra: serde_json::Map::new(),
    };
    let done = if round2 {
        s.guided_round2(rx, ui_tx)
    } else {
        s.guided(rx, ui_tx)
    };
    let code = match done {
        Some(()) => 0,
        None => {
            println!(
                "\nStopped early. What was done so far is saved in:\n  {}",
                s.dir.display()
            );
            0
        }
    };
    s.save();
    code
}

impl Session {
    pub(crate) fn ask(&mut self, round: &str, kind: BlockKind, question: &Q) -> Option<Ans> {
        let a = self.ui.yes_no(question.text)?;
        self.answers.push(Answer {
            round: round.to_string(),
            block: kind,
            id: question.id.to_string(),
            question: question.text.replace("\n    ", " "),
            expected: question.expected,
            answer: a,
        });
        self.log.write(
            "answer",
            Instant::now(),
            json!({"round": round, "block": kind.id(), "id": question.id, "answer": a, "expected": question.expected}),
        );
        Some(a)
    }

    pub(crate) fn health(&mut self, label: &str) {
        let Some(taps) = &self.taps else { return };
        let h = HealthCheck {
            label: label.to_string(),
            taps: taps.label(&system::own_taps()),
            secure_input: system::secure_input_on(),
        };
        self.log.write("tap_health", Instant::now(), json!(h));
        if !h.ok() {
            println!("  !! A keyboard tap is missing or switched off ({label}). This is a finding; carry on.");
        }
        self.health.push(h);
    }

    fn guided(&mut self, rx: Receiver<Msg>, ui_tx: Sender<UiMsg>) -> Option<()> {
        self.permissions_step()?;

        // 2. The Globe setting.
        let read = globe::read();
        println!("\n{RULE}\nYour \"Press 🌐 key to\" setting\n{RULE}");
        println!("I read it from macOS as: {}", read.describe());
        println!(
            "Please check: open System Settings → Keyboard and look at \"Press 🌐 key to\"\n\
             (on some keyboards it says \"Press fn key to\"). Don't change it yet."
        );
        if self
            .ui
            .yes_no("Shall I open the Keyboard page of System Settings for you?")?
            == Ans::Yes
        {
            system::open_pane(Pane::Keyboard);
        }
        let labels: Vec<&str> = FnUsage::ALL.iter().map(|u| u.label()).collect();
        let (confirm, says) = match read.usage() {
            Some(u) => {
                let a = self.ui.yes_no(&format!("Does it say \"{}\"?", u.label()))?;
                let says = if a == Ans::No {
                    let i = self.ui.choice("What does it say?", &labels)?;
                    Some(FnUsage::ALL[i])
                } else {
                    Some(u)
                };
                (Some(a), says)
            }
            None => {
                let i = self.ui.choice("What does it say?", &labels)?;
                (None, Some(FnUsage::ALL[i]))
            }
        };
        self.original_globe = says;
        let round1 = says
            .map(|u| u.label().to_string())
            .unwrap_or("unknown".into());
        self.globe_rounds.push(GlobeRound {
            round: round1.clone(),
            expected: says,
            read,
            owner_confirms: confirm,
            owner_says: says,
        });
        self.builtin_keyboard = self
            .ui
            .yes_no("Are you typing on the Mac's own keyboard (not a separate one)?");
        self.builtin_keyboard?;

        // 3. Start both taps, the mic thread and the engine.
        self.start_listeners(rx, ui_tx)?;

        // 4. Prove the tap sees fn.
        self.prove_fn()?;

        // 5. TextEdit with practice text.
        self.open_textedit()?;

        // 6. The main round.
        let texts = block_texts();
        let total = texts.len();
        for (i, b) in texts.iter().enumerate() {
            self.block(&round1, b, &format!("Test {} of {total}", i + 1), true)?;
        }

        // 7. The other Globe settings.
        let others: Vec<FnUsage> = FnUsage::ALL
            .into_iter()
            .filter(|u| Some(*u) != self.original_globe)
            .collect();
        println!(
            "\n{RULE}\nMain tests done. Thank you!\n{RULE}\n\
             Optional (about 2 minutes each, 3 settings): repeat the first three tests\n\
             (hold, quick taps, double-tap) with each of the other \"Press 🌐 key to\"\n\
             settings. This shows whether the 🌐 key's own action ever leaks through."
        );
        if self.ui.yes_no("Do the extra rounds now?")? == Ans::Yes {
            for u in others {
                self.globe_round(u, &texts)?;
            }
            if let Some(orig) = self.original_globe {
                println!(
                    "\nAll rounds done. Please set \"Press 🌐 key to\" back to \"{}\"\n\
                     (System Settings → Keyboard), the way it was before.",
                    orig.label()
                );
                self.ui.enter("Press Enter when done.")?;
                let back = globe::read();
                if back.usage() != Some(orig) {
                    println!(
                        "  (macOS now reports {}; please double-check it.)",
                        back.describe()
                    );
                }
                self.log
                    .write("globe_restored", Instant::now(), json!({"read": back}));
            }
        }

        self.health("end");
        self.finish();
        Some(())
    }

    /// Step 1 of both rounds: show and fix the permissions.
    pub(crate) fn permissions_step(&mut self) -> Option<()> {
        let term = self.term.clone();
        println!("Results will be saved in:\n  {}\n", self.dir.display());
        print_permissions(&self.permissions, &term);
        self.ui.enter("\nPress Enter to begin.")?;
        ensure_accessibility(&self.ui, &term)?;
        ensure_secure_input_off(&term);
        self.mic_device = ensure_microphone(&self.ui, &term);
        self.permissions = read_permissions();
        self.log
            .write("permissions", Instant::now(), json!(self.permissions));
        Some(())
    }

    /// Starts both taps, the mic thread and the engine with this run's
    /// grammar settings.
    pub(crate) fn start_listeners(
        &mut self,
        rx: Receiver<Msg>,
        ui_tx: Sender<UiMsg>,
    ) -> Option<()> {
        let term = self.term.clone();
        let grammar = Grammar::with_fn_block(self.timings, self.block_fn);
        println!(
            "\nStarting the keyboard listeners{}...",
            if grammar.blocks_fn() {
                ""
            } else {
                " (fn itself not held back)"
            }
        );
        let fwd = self.tx.clone();
        let taps = match taps::start(
            move |m: TapMsg| {
                let _ = fwd.send(Msg::Tap(m));
            },
            grammar.blocks_fn(),
        ) {
            Ok(t) => t,
            Err(e) => {
                println!(
                    "  !! Could not start: {e}\n  Check that {term} has Accessibility switched on,\n  \
                     quit {term} (⌘Q), open it again and run the command again."
                );
                self.log
                    .write("tap_error", Instant::now(), json!({"error": e}));
                return None;
            }
        };
        for (b, r) in &taps.handy_registered {
            self.log.write(
                "handy_register",
                Instant::now(),
                json!({"binding": b, "ok": r.is_ok(), "error": r.as_ref().err()}),
            );
        }
        let blocking = Arc::clone(&taps.blocking);
        self.taps = Some(taps);
        let mic_tx = {
            let tx = self.tx.clone();
            mic::spawn(Arc::clone(&self.log), move |r| {
                let _ = tx.send(Msg::Mic(r));
            })
        };
        self.mic_tx = Some(mic_tx.clone());
        let engine = Engine::new(Arc::clone(&self.log), blocking, mic_tx, ui_tx, grammar);
        std::thread::Builder::new()
            .name("wb-spike-fn-engine".into())
            .spawn(move || engine.run(rx))
            .expect("spawn engine");
        self.health("start");
        Some(())
    }

    /// Asks for one fn press and helps with Input Monitoring if none comes.
    pub(crate) fn prove_fn(&mut self) -> Option<()> {
        let term = self.term.clone();
        loop {
            println!("\nQuick check: press and let go of the fn key once.");
            self.ui.drain();
            let _ = self.tx.send(Msg::ArmFnNotice);
            if self.ui.wait_fn(Duration::from_secs(20)) {
                println!("  Got it, the fn key is visible to the test.");
                break;
            }
            println!(
                "\n  I did not see the fn key. Two things to check:\n  \
                 1. System Settings → Privacy & Security → Input Monitoring: switch \"{term}\" ON.\n     \
                 If macOS asks to quit and reopen {term}, do that and run the command again.\n  \
                 2. {term} menu (top left): \"Secure Keyboard Entry\" must NOT be ticked."
            );
            if !system::input_monitoring_granted() {
                system::request_input_monitoring();
            }
            if self
                .ui
                .yes_no("Shall I open the Input Monitoring page for you?")?
                == Ans::Yes
            {
                system::open_pane(Pane::InputMonitoring);
            }
            self.ui.enter("Press Enter to try again.")?;
        }
        Some(())
    }

    /// Opens TextEdit with the practice text.
    pub(crate) fn open_textedit(&mut self) -> Option<()> {
        let practice = self.dir.join("practice-text.txt");
        if write_practice_text(&practice).is_ok() {
            let _ = std::process::Command::new("/usr/bin/open")
                .args(["-a", "TextEdit"])
                .arg(&practice)
                .status();
        }
        println!(
            "\nI opened a TextEdit window with some practice text (used in two tests).\n\
             Please place it next to this window so you can see both, then click back\n\
             into this window."
        );
        self.ui.enter("Press Enter when ready.")?;
        Some(())
    }

    fn globe_round(&mut self, u: FnUsage, texts: &[BlockText]) -> Option<()> {
        println!(
            "\n{RULE}\nRound: \"{}\"\n{RULE}\n\
             In System Settings → Keyboard, set \"Press 🌐 key to\" to \"{}\".",
            u.label(),
            u.label()
        );
        if u == FnUsage::StartDictation {
            println!(
                "(If macOS asks whether to turn on Dictation, you can say yes or no; tell me below.)"
            );
        }
        self.ui.enter("Press Enter once it is set.")?;
        let read = globe::read();
        let ok = read.usage() == Some(u);
        let confirm = if ok {
            println!("  macOS now reports \"{}\". Good.", u.label());
            None
        } else {
            println!("  macOS reports {}.", read.describe());
            self.ui.yes_no(&format!(
                "Does System Settings show \"{}\" right now?",
                u.label()
            ))
        };
        self.globe_rounds.push(GlobeRound {
            round: u.label().to_string(),
            expected: Some(u),
            read,
            owner_confirms: confirm,
            owner_says: None,
        });
        let round = u.label().to_string();
        for b in texts.iter().take(3) {
            self.block(&round, b, &format!("\"{}\" round", u.label()), false)?;
        }
        Some(())
    }

    pub(crate) fn block(
        &mut self,
        round: &str,
        b: &BlockText,
        heading: &str,
        all_questions: bool,
    ) -> Option<()> {
        println!("\n{RULE}\n{heading}: {}\n{RULE}\n{}", b.title, b.steps);
        let a = self
            .ui
            .enter("\nPress Enter to start (or type s and Enter to skip this test).")?;
        if a.eq_ignore_ascii_case("s") {
            println!("  Skipped.");
            self.log.write(
                "block_skipped",
                Instant::now(),
                json!({"block": b.kind.id(), "round": round}),
            );
            return Some(());
        }
        let label = format!("{round} / {}", b.kind.id());
        self.health(&format!("before {label}"));
        if system::secure_input_on() {
            ensure_secure_input_off(&self.term);
        }
        let save_dir = (b.kind == BlockKind::Clipping).then(|| {
            let d = if self.opts.round == 2 {
                self.dir.join("takes").join(crate::round2::slug(round))
            } else {
                self.dir.join("takes")
            };
            let _ = std::fs::create_dir_all(&d);
            d
        });
        let _ = self.tx.send(Msg::Begin(BlockSpec {
            kind: b.kind,
            round: round.to_string(),
            target: b.target,
            auto_end: b.auto_end,
            save_dir,
        }));
        if b.auto_end {
            println!(
                "Go! (It moves on by itself after {}. Press Enter to stop early.)",
                b.target
            );
        } else {
            println!("Go! (Press Enter here when you're done.)");
        }
        let end = self.ui.wait_block(b.cue, b.target);
        let (reply_tx, reply_rx) = mpsc::channel();
        let _ = self.tx.send(Msg::End(reply_tx));
        let run = reply_rx.recv_timeout(Duration::from_secs(15)).ok();
        self.ui.drain();
        if matches!(end, BlockEnd::Eof) {
            if let Some(r) = run {
                self.runs.push(r);
            }
            return None;
        }
        self.health(&format!("after {label}"));
        if let Some(r) = run {
            println!(
                "\nDone: {} ok, {} problem(s), {} not counted{}.",
                r.count(Outcome::Pass),
                r.count(Outcome::Fail),
                r.count(Outcome::Invalid),
                if b.kind == BlockKind::Chords {
                    format!(", {} fn shortcut presses seen", r.chord_keys)
                } else {
                    String::new()
                }
            );
            self.runs.push(r);
        }
        if matches!(end, BlockEnd::Skip) {
            return Some(());
        }
        println!("A few questions about what you saw:");
        for question in &b.questions {
            if all_questions || matches!(question.id, "popup" | "dot") {
                self.ask(round, b.kind, question)?;
            }
        }
        self.save();
        Some(())
    }

    pub(crate) fn summary_json(&self) -> serde_json::Value {
        if self.opts.round == 2 {
            return self.summary_json_round2();
        }
        let crits = summary::criteria(&self.runs, &self.answers, &self.health);
        json!({
            "tool": "wb-spike-fn",
            "spike": "RS-01",
            "tool_version": env!("CARGO_PKG_VERSION"),
            "git": option_env!("WB_GIT_SHA"),
            "started": self.started_local,
            "machine": self.machine,
            "terminal": self.term,
            "builtin_keyboard": self.builtin_keyboard,
            "permissions": self.permissions,
            "mic_device": self.mic_device,
            "timings": self.timings,
            "handy_bindings": self.taps.as_ref().map(|t| t.handy_registered.iter().map(|(b, r)| json!({"binding": b, "ok": r.is_ok()})).collect::<Vec<_>>()),
            "tap_ids": self.taps.as_ref().map(|t| json!({"handy": t.handy_tap_ids, "fork": t.fork_tap_ids})),
            "globe": {"original": self.original_globe, "rounds": self.globe_rounds},
            "criteria": crits,
            "answers": self.answers,
            "health": self.health,
            "blocks": self.runs,
            "options": {"out": self.opts.out},
        })
    }

    pub(crate) fn save(&self) {
        let v = self.summary_json();
        let _ = std::fs::write(
            self.dir.join("summary.json"),
            serde_json::to_string_pretty(&v).unwrap_or_default(),
        );
        let _ = std::fs::write(self.dir.join("summary.txt"), self.summary_text());
    }

    pub(crate) fn summary_text(&self) -> String {
        if self.opts.round == 2 {
            return self.summary_text_round2();
        }
        let crits = summary::criteria(&self.runs, &self.answers, &self.health);
        summary::render_text(&crits, &self.answers)
    }

    pub(crate) fn finish(&self) {
        self.save();
        println!("\n{}", self.summary_text());
        println!(
            "All done, thank you! Everything is saved in:\n  {}\n\
             You can close TextEdit without saving. Tell Claude the test is finished;\n\
             it will read the results from that folder.",
            self.dir.display()
        );
    }
}
