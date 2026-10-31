// SPDX-License-Identifier: GPL-3.0-or-later
//! RS-04 Accessibility (AX) matrix spike (R5 §7 RS-04, §4.6, S-05, S-19b,
//! S-20, S-21). Not product code; never built into the app.
//!
//! `src/ax/` is the prototype of the fork's AX layer (one AX thread,
//! messaging timeouts, the `AXManualAccessibility` lease, the editable
//! verdict and fail-open rule, the selection chain), written so S-05 can
//! lift it. The rest walks the owner through the installed apps, records
//! one focus event per click and scores each app against the RS-04
//! criteria. Results: `~/wb-bench/results/rs04/<timestamp>/`
//! (`events.jsonl`, `summary.json`, `summary.txt`).
//!
//! ```text
//! wb-spike-ax                  the guided session (about 15 minutes)
//! wb-spike-ax --auto           unattended: TextEdit + the test page in Safari and Chrome
//! wb-spike-ax --dry-run        permission check and plan; nothing opened or written
//! wb-spike-ax --apps a,b       only these apps (keys: see --dry-run)
//! wb-spike-ax --out DIR        results under DIR/<timestamp>/ instead
//! wb-spike-ax --summarize DIR  rebuild summary.* from a run's events.jsonl
//! ```
//! See docs/fork/spikes/RS-04-procedure.md.

mod apps;
mod ax;
mod log;
mod record;
mod score;
mod session;
mod summary;
mod system;
mod testpage;

use std::path::PathBuf;

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Options {
    pub out: Option<PathBuf>,
    pub apps: Vec<String>,
    pub auto: bool,
}

enum Mode {
    Session,
    Auto,
    DryRun,
    Summarize(PathBuf),
    LeaseCheck(i32),
    Watch(u64),
    Help,
}

fn parse(args: &[String]) -> Result<(Mode, Options), String> {
    let mut mode = Mode::Session;
    let mut opts = Options::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--dry-run" => mode = Mode::DryRun,
            "--auto" => {
                mode = Mode::Auto;
                opts.auto = true;
            }
            "-h" | "--help" => mode = Mode::Help,
            "--out" => {
                let dir = it.next().ok_or("--out needs a folder")?;
                opts.out = Some(PathBuf::from(dir));
            }
            "--apps" => {
                let list = it
                    .next()
                    .ok_or("--apps needs a list, e.g. textedit,safari")?;
                for k in list.split(',').map(str::trim).filter(|k| !k.is_empty()) {
                    if apps::find(k).is_none() {
                        return Err(format!("unknown app {k} (try --dry-run for the list)"));
                    }
                    opts.apps.push(k.to_string());
                }
            }
            "--lease-check" => {
                let pid = it
                    .next()
                    .and_then(|p| p.parse().ok())
                    .ok_or("--lease-check needs a process id")?;
                mode = Mode::LeaseCheck(pid);
            }
            "--watch" => {
                let secs = it
                    .next()
                    .and_then(|p| p.parse().ok())
                    .ok_or("--watch needs a number of seconds")?;
                mode = Mode::Watch(secs);
            }
            "--summarize" => {
                let dir = it.next().ok_or("--summarize needs a results folder")?;
                mode = Mode::Summarize(PathBuf::from(dir));
            }
            other => return Err(format!("unknown option {other} (try --help)")),
        }
    }
    Ok((mode, opts))
}

fn dry_run(opts: &Options) -> i32 {
    session::intro();
    let term = system::terminal_name(std::env::var("TERM_PROGRAM").ok().as_deref());
    println!(
        "Accessibility permission for {term}: {}",
        if system::accessibility_granted() {
            "on"
        } else {
            "OFF (the session will show how to switch it on)"
        }
    );
    println!(
        "Secure Input: {}",
        if ax::element::secure_input_on() {
            "ON (the session will ask you to switch it off)"
        } else {
            "off"
        }
    );
    let m = system::machine_info();
    println!(
        "Mac: {} ({}), macOS {} ({})\n\nApps, in order:",
        m.model, m.cpu, m.macos_version, m.macos_build
    );
    let mut guided = 0;
    for a in session::selected(opts) {
        let at = apps::installed_path(a);
        guided += a.targets.len();
        println!(
            "  {:<9} {:<24} {:>2} steps{}{}  {}",
            a.key,
            a.title,
            a.targets.len(),
            if a.test_page { " + test page" } else { "" },
            if a.ask.is_some() { " (asks first)" } else { "" },
            at.unwrap_or("NOT INSTALLED: skipped")
        );
    }
    println!(
        "\n{guided} guided steps. Not tested (not installed): {}.",
        apps::NOT_TESTED.join(", ")
    );
    println!(
        "Results would go to: {}\n\nDry run: nothing was opened, prompted or written.",
        session::results_dir(opts).display()
    );
    0
}

/// `--lease-check PID`: read `AXManualAccessibility`, switch it on, read it
/// back, and put the old value back. Diagnostics for the lease rows.
fn lease_check(pid: i32) -> i32 {
    use ax::element as el;
    // SAFETY: plain constructor.
    let app = unsafe { objc2_application_services::AXUIElement::new_application(pid) };
    el::set_timeout(&app, 0.25);
    let read = |label: &str| match el::attr(&app, "AXManualAccessibility") {
        Ok(v) => println!("  {label}: {:?}", v),
        Err(e) => println!("  {label}: error {}", el::error_name(e)),
    };
    println!(
        "pid {pid}: {}",
        el::app_bundle_path(pid).unwrap_or_else(|| "?".into())
    );
    println!(
        "  AXManualAccessibility settable: {:?}",
        el::is_settable(&app, "AXManualAccessibility").map_err(el::error_name)
    );
    read("before");
    println!(
        "  set true: {}",
        el::error_name(el::set_manual_accessibility(&app, true))
    );
    read("after");
    let before = el::attr_bool(&app, "AXManualAccessibility");
    if before != Some(true) {
        println!("  (value did not stick)");
    }
    println!(
        "  set false (restore): {}",
        el::error_name(el::set_manual_accessibility(&app, false))
    );
    0
}

/// `--watch SECS` (S-05): every 0.4 s, the product's verdict for the
/// focused element of the frontmost app, one JSON line per change (roles,
/// verdict and delivery only; never text). Used by the S-05 real-Mac run to
/// check the desktop, Finder and each app's fields.
fn watch(secs: u64) -> i32 {
    use ax::probe::{self, LeaseMode, ProbeOpts};
    let ax = ax::thread::AxThread::spawn();
    let end = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    let mut last = String::new();
    while std::time::Instant::now() < end {
        system::tick(std::time::Duration::from_millis(400));
        let line = ax.run(std::time::Duration::from_secs(2), |ctx| {
            let web = probe::focused_family(ctx).is_some_and(|f| f.builds_tree_on_request());
            let p = probe::probe(
                ctx,
                &ProbeOpts {
                    timeout: 0.2,
                    lease: if web { LeaseMode::On } else { LeaseMode::NotNeeded },
                    allow_text: false,
                    set_test: false,
                    menu_copy: false,
                },
            );
            serde_json::json!({
                "app": p.app,
                "family": p.family,
                "role": p.facts.role,
                "subrole": p.facts.subrole,
                "window": p.facts.has_focused_window,
                "element": p.facts.has_focused_element,
                "secure_input": p.facts.secure_input,
                "verdict": p.verdict,
                "delivery": p.delivery,
                "ms": p.total_ms,
            })
            .to_string()
        });
        if let Some(line) = line {
            // Print on change only (the timing field aside).
            let key = line.split(",\"ms\"").next().unwrap_or("").to_string();
            if key != last {
                println!("{line}");
                last = key;
            }
        }
    }
    ax.shutdown();
    0
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match parse(&args) {
        Ok((Mode::Help, _)) => {
            println!(
                "wb-spike-ax: RS-04 text field (Accessibility) test\n\n  \
                 (no options)     guided session (about 15 minutes)\n  \
                 --auto           unattended part only (TextEdit, test page in Safari/Chrome)\n  \
                 --dry-run        permission check and plan, then exit\n  \
                 --apps a,b       only these apps\n  \
                 --out DIR        save results under DIR instead of ~/wb-bench/results/rs04\n  \
                 --summarize DIR  rebuild summary.txt/json from a run's events.jsonl\n  \
                 --watch SECS     print the product's verdict for the focused element (S-05)"
            );
            0
        }
        Ok((Mode::DryRun, opts)) => dry_run(&opts),
        Ok((Mode::Summarize(dir), _)) => summary::summarize_dir(&dir),
        Ok((Mode::LeaseCheck(pid), _)) => lease_check(pid),
        Ok((Mode::Watch(secs), _)) => watch(secs),
        Ok((Mode::Auto, opts)) => session::auto(opts),
        Ok((Mode::Session, opts)) => session::run(opts),
        Err(e) => {
            eprintln!("{e}");
            2
        }
    };
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn options() {
        assert!(matches!(parse(&args(&[])), Ok((Mode::Session, _))));
        assert!(matches!(
            parse(&args(&["--dry-run"])),
            Ok((Mode::DryRun, _))
        ));
        let (m, o) = parse(&args(&["--auto", "--apps", "textedit, safari"])).unwrap();
        assert!(matches!(m, Mode::Auto));
        assert!(o.auto);
        assert_eq!(o.apps, vec!["textedit", "safari"]);
        assert!(parse(&args(&["--apps", "slack"])).is_err());
        assert!(parse(&args(&["--out"])).is_err());
        assert!(matches!(
            parse(&args(&["--summarize", "/tmp/r"])),
            Ok((Mode::Summarize(_), _))
        ));
        assert!(parse(&args(&["--bogus"])).is_err());
    }
}
