// SPDX-License-Identifier: GPL-3.0-or-later
//! RS-01 fn / Globe spike (R5 §7 RS-01, §4.8, DEC-09, DEC-44, DEC-47). Not
//! product code; never built into the app.
//!
//! Runs the exact two-tap setup RS-01 specifies (Handy's `HotkeyManager` as
//! shipped plus the fork's own `KeyboardListener::new_with_blocking` with the
//! §4.8 dynamic blocking set), drives the fn grammar of S-01 (`grammar.rs`, a
//! pure state machine S-01 can lift), opens the mic cold at `T_show` and
//! closes it on release/cancel, and walks the owner through the test blocks
//! in plain English. Results: `~/wb-bench/results/rs01/<timestamp>/`
//! (`events.jsonl`, `summary.json`, `summary.txt`, `takes/*.wav`).
//!
//! ```text
//! wb-spike-fn               the guided session
//! wb-spike-fn --dry-run     intro, permission check, Globe setting; no taps, no prompts
//! wb-spike-fn --self-test   run the grammar against its scripted event sequences
//! wb-spike-fn --out DIR     results under DIR/<timestamp>/ instead
//! ```
//! See docs/fork/spikes/RS-01-procedure.md.

mod engine;
mod globe;
mod grammar;
mod log;
mod mic;
mod scenarios;
mod session;
mod summary;
mod system;
mod taps;
mod verdict;

use std::path::PathBuf;

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Options {
    pub out: Option<PathBuf>,
}

enum Mode {
    Session,
    DryRun,
    SelfTest,
    Help,
}

fn parse(args: &[String]) -> Result<(Mode, Options), String> {
    let mut mode = Mode::Session;
    let mut opts = Options::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--dry-run" => mode = Mode::DryRun,
            "--self-test" => mode = Mode::SelfTest,
            "-h" | "--help" => mode = Mode::Help,
            "--out" => {
                let dir = it.next().ok_or("--out needs a folder")?;
                opts.out = Some(PathBuf::from(dir));
            }
            other => return Err(format!("unknown option {other} (try --help)")),
        }
    }
    Ok((mode, opts))
}

fn self_test() -> i32 {
    use grammar::Timings;
    let t = Timings::DEFAULT;
    println!(
        "wb-spike-fn self-test: fn grammar with T = {} ms, T_show = {} ms\n",
        t.tap_window_ms, t.show_ms
    );
    let mut failed = 0;
    let all = scenarios::scenarios();
    for s in &all {
        match scenarios::run_scenario(s) {
            Ok(()) => println!("  ok    {}", s.name),
            Err(e) => {
                failed += 1;
                println!("  FAIL  {}{e}", s.name);
            }
        }
    }
    println!(
        "\n{} of {} scenarios passed.",
        all.len() - failed,
        all.len()
    );
    i32::from(failed > 0)
}

fn dry_run(opts: &Options) -> i32 {
    session::intro();
    let term = system::terminal_name(std::env::var("TERM_PROGRAM").ok().as_deref());
    let p = session::read_permissions();
    session::print_permissions(&p, &term);
    println!(
        "\n\"Press 🌐 key to\" setting, as macOS reports it: {}",
        globe::read().describe()
    );
    let m = system::machine_info();
    println!(
        "Mac: {} ({}), macOS {} ({})",
        m.model, m.cpu, m.macos_version, m.macos_build
    );
    println!(
        "Handy's shortcut listener would register: {:?}",
        taps::handy_bindings(false)
    );
    println!(
        "Results would go to: {}\n\nDry run: nothing was started, prompted or written.",
        session::results_dir(opts).display()
    );
    0
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match parse(&args) {
        Ok((Mode::Help, _)) => {
            println!(
                "wb-spike-fn: RS-01 fn / Globe key test\n\n  \
                 (no options)   guided session (about 15-20 minutes)\n  \
                 --dry-run      intro + permission check, then exit\n  \
                 --self-test    check the fn grammar against scripted presses\n  \
                 --out DIR      save results under DIR instead of ~/wb-bench/results/rs01"
            );
            0
        }
        Ok((Mode::SelfTest, _)) => self_test(),
        Ok((Mode::DryRun, opts)) => dry_run(&opts),
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
        assert!(matches!(
            parse(&args(&["--self-test"])),
            Ok((Mode::SelfTest, _))
        ));
        let (_, o) = parse(&args(&["--out", "/tmp/x"])).unwrap();
        assert_eq!(o.out, Some(PathBuf::from("/tmp/x")));
        assert!(parse(&args(&["--out"])).is_err());
        assert!(parse(&args(&["--bogus"])).is_err());
    }

    #[test]
    fn self_test_passes() {
        assert_eq!(self_test(), 0);
    }
}
