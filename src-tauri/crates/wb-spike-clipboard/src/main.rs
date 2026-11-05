// SPDX-License-Identifier: GPL-3.0-or-later
//! RS-05 clipboard fidelity and paste hygiene spike (R5 §7 RS-05, §4 W-07,
//! S-04, S-06). Not product code; never built into the app.
//!
//! `src/clip/` is the prototype of `fork/clipboard/` (snapshot every item
//! and type with the skip policy, transcript with the nspasteboard markers,
//! ⌘V, modifier-release wait, restore guard and wait rule). The rest drives
//! it on the owner's Mac: automated save/restore round trips with two
//! backends, guard checks on the real pasteboard, the paste matrix across
//! the installed apps, the held-⌃⌘ test and an optional CJK input step.
//! Results: `~/wb-bench/results/rs05/<timestamp>/` (`summary.txt`,
//! `summary.json`, `events.jsonl`).
//!
//! ```text
//! wb-spike-clipboard                    the guided session (about 10 minutes)
//! wb-spike-clipboard --auto             automated checks only: no key presses, no windows
//! wb-spike-clipboard --self-check       --auto, plus real copies and pastes into windows it opens
//! wb-spike-clipboard --dry-run          what the session would use; touches nothing
//! wb-spike-clipboard --restore-owner F  put back a clipboard saved by a run that crashed
//! wb-spike-clipboard --out DIR          results under DIR/<timestamp>/ instead
//! wb-spike-clipboard --s04-soak         S-04 acceptance: 10 takes in each of 10 apps
//!     [--per-app N] [--only a,b] [--keep-open]   through the product's fork::insert
//! ```
//! See docs/fork/spikes/RS-05-procedure.md.

mod apps;
mod clip;
/// `crate::clipboard` for the product's `fork::insert` files, which reach
/// the clipboard layer as `super::super::clipboard`.
use clip as clipboard;
mod fidelity;
mod log;
mod macos;
mod owner;
mod report;
mod session;
mod soak;
mod sources;
mod take;
mod ui;

mod insert;

use std::path::PathBuf;

#[derive(Clone, Debug, Default)]
pub struct Options {
    pub out: Option<PathBuf>,
}

#[derive(Debug)]
enum Mode {
    Session,
    Auto,
    SelfCheck,
    DryRun,
    RestoreOwner(PathBuf),
    Soak {
        per_app: usize,
        only: Vec<String>,
        keep_open: bool,
    },
    Leftovers,
    /// S-04.5: paste with the Japanese input method on.
    Ime,
    /// Cleanup by hand: press a button in an app's focused window.
    Press(String, String),
    /// S-04.3/4 wiring: the product's session observer and gate.
    Session04,
    WaitQuiet(u64, u64),
    /// Empty the front field of an app if it holds only soak text.
    ClearField(String),
    StripTail(String),
    ClickBackspace(f64, f64, usize),
    TerminalBackspace(String, usize),
    /// Cleanup by hand: bring `bundle`'s window titled `title` to the front
    /// and post the given keys (`cmd+a`, `delete`, `cmd+w`, `return` …).
    Keys(String, String, Vec<String>),
    /// Window titles of a running app (to check a run left nothing behind).
    Titles(String),
    Help,
}

fn parse(args: &[String]) -> Result<(Mode, Options), String> {
    let mut mode = Mode::Session;
    let mut opts = Options::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--auto" => mode = Mode::Auto,
            "--self-check" => mode = Mode::SelfCheck,
            "--dry-run" => mode = Mode::DryRun,
            "--s04-leftovers" => mode = Mode::Leftovers,
            "--s04-ime" => mode = Mode::Ime,
            "--s04-press" => {
                let b = it.next().ok_or("needs BUNDLE TITLE")?.clone();
                let t = it.next().ok_or("needs BUNDLE TITLE")?.clone();
                mode = Mode::Press(b, t);
            }
            "--s04-session" => mode = Mode::Session04,
            "--s04-wait-quiet" => {
                let n = |v: Option<&String>| v.and_then(|v| v.parse::<u64>().ok());
                let (Some(q), Some(m)) = (n(it.next()), n(it.next())) else {
                    return Err("--s04-wait-quiet needs QUIET_S MAX_S".into());
                };
                mode = Mode::WaitQuiet(q, m);
            }
            "--s04-keys" => {
                let b = it.next().ok_or("needs BUNDLE TITLE KEYS…")?.clone();
                let t = it.next().ok_or("needs BUNDLE TITLE KEYS…")?.clone();
                let keys: Vec<String> = it.by_ref().cloned().collect();
                mode = Mode::Keys(b, t, keys);
            }
            "--s04-terminal-backspace" => {
                let title = it.next().ok_or("needs TITLE COUNT")?.clone();
                let n = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or("needs TITLE COUNT")?;
                mode = Mode::TerminalBackspace(title, n);
            }
            "--s04-click-backspace" => {
                let n = |v: Option<&String>| v.and_then(|v| v.parse::<f64>().ok());
                let (Some(x), Some(y), Some(k)) = (n(it.next()), n(it.next()), n(it.next())) else {
                    return Err("--s04-click-backspace needs X Y COUNT".into());
                };
                mode = Mode::ClickBackspace(x, y, k as usize);
            }
            "--s04-strip-tail" => {
                mode = Mode::StripTail(
                    it.next()
                        .ok_or("--s04-strip-tail needs a bundle id")?
                        .clone(),
                )
            }
            "--s04-clear-field" => {
                mode = Mode::ClearField(
                    it.next()
                        .ok_or("--s04-clear-field needs a bundle id")?
                        .clone(),
                )
            }
            "--s04-titles" => {
                mode = Mode::Titles(it.next().ok_or("--s04-titles needs a bundle id")?.clone())
            }
            "--s04-soak" => {
                mode = Mode::Soak {
                    per_app: 10,
                    only: Vec::new(),
                    keep_open: false,
                }
            }
            "--per-app" | "--only" | "--keep-open" => {
                let Mode::Soak {
                    per_app,
                    only,
                    keep_open,
                } = &mut mode
                else {
                    return Err(format!("{a} goes after --s04-soak"));
                };
                match a.as_str() {
                    "--per-app" => {
                        *per_app = it
                            .next()
                            .and_then(|n| n.parse().ok())
                            .ok_or("--per-app needs a number")?
                    }
                    "--only" => {
                        *only = it
                            .next()
                            .ok_or("--only needs app keys")?
                            .split(',')
                            .map(str::to_owned)
                            .collect()
                    }
                    _ => *keep_open = true,
                }
            }
            "-h" | "--help" => mode = Mode::Help,
            "--out" => {
                let dir = it.next().ok_or("--out needs a folder")?;
                opts.out = Some(PathBuf::from(dir));
            }
            "--restore-owner" => {
                let f = it.next().ok_or("--restore-owner needs a file")?;
                mode = Mode::RestoreOwner(PathBuf::from(f));
            }
            other => return Err(format!("unknown option {other} (try --help)")),
        }
    }
    Ok((mode, opts))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match parse(&args) {
        Ok((Mode::Help, _)) => {
            println!(
                "wb-spike-clipboard: RS-05 clipboard and paste test\n\n  \
                 (no options)         guided session (about 10 minutes)\n  \
                 --auto               automated checks only (no key presses, no windows)\n  \
                 --self-check         --auto plus pastes into windows the tool opens (no person needed)\n  \
                 --dry-run            show what the session would use; touches nothing\n  \
                 --restore-owner FILE put back a clipboard saved by an unfinished run\n  \
                 --out DIR            save results under DIR instead of ~/wb-bench/results/rs05\n  \
                 --s04-soak           S-04 acceptance run through the product's fork::insert\n  \
                   [--per-app N] [--only textedit,notes,…] [--keep-open]"
            );
            0
        }
        Ok((Mode::DryRun, opts)) => session::dry_run(&opts),
        Ok((Mode::Titles(b), _)) => {
            let titles = macos::running_pid(&b)
                .map(macos::window_titles)
                .unwrap_or_default();
            println!("{titles:#?}");
            if std::env::var_os("WB_TEXTS").is_some() {
                if let Some(pid) = macos::running_pid(&b) {
                    for (title, text) in macos::window_texts(pid) {
                        let tail: String = text
                            .chars()
                            .rev()
                            .take(400)
                            .collect::<Vec<_>>()
                            .into_iter()
                            .rev()
                            .collect();
                        println!(
                            "== {title}\n   soak text: {}\n   tail: {tail:?}",
                            text.matches("[S-04 soak ").count()
                        );
                        for (i, _) in text.match_indices("[S-04 soak ") {
                            let around: String = text
                                .chars()
                                .skip(text[..i].chars().count().saturating_sub(300))
                                .take(400)
                                .collect();
                            println!("   around: {around:?}");
                        }
                    }
                }
            }
            if std::env::var_os("WB_OUTLINE").is_some() {
                if let Some(pid) = macos::running_pid(&b) {
                    for l in macos::window_outline(pid, 3) {
                        println!("{l}");
                    }
                }
            }
            0
        }
        Ok((Mode::ClearField(b), _)) => soak::clear_field(&b),
        Ok((Mode::StripTail(b), _)) => soak::strip_tail(&b),
        Ok((Mode::Keys(bundle, title, keys), _)) => {
            let Some(pid) = macos::running_pid(&bundle) else {
                std::process::exit(1);
            };
            macos::open(&["-b", &bundle]);
            macos::pump(std::time::Duration::from_millis(500));
            macos::raise_window(pid, &title);
            macos::pump(std::time::Duration::from_millis(500));
            let front = macos::focus(pid).window_title.unwrap_or_default();
            if macos::frontmost().map(|f| f.pid) != Some(pid) || !front.contains(&title) {
                eprintln!("not the right window in front ({front:?}); nothing typed");
                std::process::exit(1);
            }
            for k in keys {
                let r = match k.as_str() {
                    "cmd+a" => clip::keys::post_command_letter(b'a'),
                    "cmd+w" => clip::keys::post_command_letter(b'w'),
                    "cmd+d" => clip::keys::post_command_letter(b'd'),
                    "delete" => clip::keys::post_key(51),
                    "return" => clip::keys::post_key(36),
                    "escape" => clip::keys::post_key(53),
                    other => Err(format!("unknown key {other}")),
                };
                if let Err(e) = r {
                    eprintln!("{e}");
                }
                macos::pump(std::time::Duration::from_millis(400));
            }
            0
        }
        Ok((Mode::TerminalBackspace(title, n), _)) => {
            // Cleanup by hand: delete `n` characters at the prompt of the
            // Terminal window whose title contains `title`, only if that
            // window is the focused one.
            let bundle = "com.apple.Terminal";
            let Some(pid) = macos::running_pid(bundle) else {
                std::process::exit(1);
            };
            macos::open(&["-b", bundle]);
            macos::pump(std::time::Duration::from_millis(500));
            macos::raise_window(pid, &title);
            macos::pump(std::time::Duration::from_millis(500));
            let front = macos::focus(pid).window_title.unwrap_or_default();
            let front_app = macos::frontmost().map(|f| f.pid);
            if front_app != Some(pid) || !front.contains(&title) {
                eprintln!("not the right window in front ({front:?}); nothing typed");
                1
            } else {
                for _ in 0..n {
                    let _ = clip::keys::post_key(51);
                    macos::pump(std::time::Duration::from_millis(60));
                }
                0
            }
        }
        Ok((Mode::ClickBackspace(x, y, k), _)) => {
            // Cleanup by hand: click at the end of our text, delete k chars.
            macos::click_at(x, y);
            macos::pump(std::time::Duration::from_millis(300));
            for _ in 0..k {
                let _ = clip::keys::post_key(51);
                macos::pump(std::time::Duration::from_millis(40));
            }
            0
        }
        Ok((Mode::WaitQuiet(q, m), _)) => soak::wait_quiet(q, m),
        Ok((Mode::Ime, _)) => soak::ime_check(),
        Ok((Mode::Press(b, t), _)) => {
            let pressed = macos::running_pid(&b).is_some_and(|pid| macos::press_button(pid, &t));
            println!("pressed {t:?}: {pressed}");
            i32::from(!pressed)
        }
        Ok((Mode::Session04, _)) => soak::session_check(),
        Ok((Mode::Leftovers, _)) => {
            let left = soak::close_leftovers();
            println!("{left:#?}");
            i32::from(!left.is_empty())
        }
        Ok((
            Mode::Soak {
                per_app,
                only,
                keep_open,
            },
            opts,
        )) => soak::run(soak::SoakOptions {
            out: opts.out,
            per_app,
            only,
            keep_open,
        }),
        Ok((Mode::Auto, opts)) => session::run(opts, session::RunMode::Auto),
        Ok((Mode::SelfCheck, opts)) => session::run(opts, session::RunMode::SelfCheck),
        Ok((Mode::Session, opts)) => session::run(opts, session::RunMode::Session),
        Ok((Mode::RestoreOwner(f), _)) => match owner::restore_file(&f) {
            Ok(r) => {
                println!(
                    "Clipboard put back ({}).",
                    if r.identical {
                        "exactly".to_string()
                    } else {
                        r.detail
                    }
                );
                0
            }
            Err(e) => {
                eprintln!("{e}");
                1
            }
        },
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
        assert!(matches!(parse(&args(&["--auto"])), Ok((Mode::Auto, _))));
        assert!(matches!(
            parse(&args(&["--dry-run"])),
            Ok((Mode::DryRun, _))
        ));
        let (_, o) = parse(&args(&["--out", "/tmp/x"])).unwrap();
        assert_eq!(o.out, Some(PathBuf::from("/tmp/x")));
        assert!(parse(&args(&["--out"])).is_err());
        assert!(matches!(
            parse(&args(&["--restore-owner", "/tmp/f"])),
            Ok((Mode::RestoreOwner(_), _))
        ));
        assert!(parse(&args(&["--bogus"])).is_err());
        assert!(matches!(
            parse(&args(&["--s04-soak", "--per-app", "3", "--only", "notes,mail"])),
            Ok((Mode::Soak { per_app: 3, ref only, keep_open: false }, _)) if only.len() == 2
        ));
        assert!(parse(&args(&["--per-app", "3"])).is_err());
    }
}
