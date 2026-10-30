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
//! ```
//! See docs/fork/spikes/RS-05-procedure.md.

mod apps;
mod clip;
mod fidelity;
mod log;
mod macos;
mod owner;
mod report;
mod session;
mod sources;
mod take;
mod ui;

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
                 --out DIR            save results under DIR instead of ~/wb-bench/results/rs05"
            );
            0
        }
        Ok((Mode::DryRun, opts)) => session::dry_run(&opts),
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
    }
}
