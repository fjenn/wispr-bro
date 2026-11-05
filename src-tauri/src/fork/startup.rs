// SPDX-License-Identifier: GPL-3.0-or-later
//! Startup timing (R5 §5.3, S-38): "launch to hotkey ready and pill
//! visible ≤ 2 s (p95 ≤ 3 s) on Apple Silicon".
//!
//! Two milestones are marked once per process, in milliseconds since the
//! kernel started it (`proc_pidinfo` `PROC_PIDTBSDINFO`, so dyld, Tauri and
//! WebKit set-up count too): the fork's hotkey tap running
//! (`fork::shortcut`) and the resting bar first on screen
//! (`fork::pill::native`). When both are in, one info line goes to the log:
//!
//! ```text
//! startup: ready 612 ms after launch (hotkeys 430 ms, bar 612 ms)
//! ```
//!
//! `scripts/wb-perf/startup.py` launches the app repeatedly and reads that
//! line. The line carries timings only.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// A startup milestone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Milestone {
    /// The fork's event tap runs: the hotkey works.
    Hotkeys,
    /// The resting bar is on screen.
    Bar,
}

/// "Not reached yet".
const UNSET: u64 = u64::MAX;

static HOTKEYS_MS: AtomicU64 = AtomicU64::new(UNSET);
static BAR_MS: AtomicU64 = AtomicU64::new(UNSET);
static LOGGED: AtomicBool = AtomicBool::new(false);

/// The process start as an `Instant`, read once (the first call).
fn launch() -> Instant {
    static LAUNCH: OnceLock<Instant> = OnceLock::new();
    *LAUNCH.get_or_init(|| {
        let now = Instant::now();
        process_age()
            .and_then(|age| now.checked_sub(age))
            .unwrap_or(now)
    })
}

/// How long ago the kernel started this process.
#[cfg(target_os = "macos")]
fn process_age() -> Option<Duration> {
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    // SAFETY: the buffer is a `proc_bsdinfo` of `size` bytes, which is what
    // PROC_PIDTBSDINFO writes.
    let n = unsafe {
        libc::proc_pidinfo(
            std::process::id() as libc::c_int,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if n != size {
        return None;
    }
    // SAFETY: filled by the successful call above.
    let info = unsafe { info.assume_init() };
    let started = UNIX_EPOCH
        + Duration::from_secs(info.pbi_start_tvsec)
        + Duration::from_micros(info.pbi_start_tvusec);
    SystemTime::now().duration_since(started).ok()
}

#[cfg(not(target_os = "macos"))]
fn process_age() -> Option<Duration> {
    None
}

/// Reads the launch time now, so later marks measure from the real start
/// even when the first mark comes late. Called from `fork::on_ready`.
pub fn init() {
    let _ = launch();
}

/// Marks `m` reached now (only the first time counts).
pub fn mark(m: Milestone) {
    let ms = u64::try_from(launch().elapsed().as_millis()).unwrap_or(UNSET - 1);
    record(m, ms);
}

fn slot(m: Milestone) -> &'static AtomicU64 {
    match m {
        Milestone::Hotkeys => &HOTKEYS_MS,
        Milestone::Bar => &BAR_MS,
    }
}

/// Stores `ms` for `m` if it was not set; logs the ready line once, when
/// both are in. Returns the line from the call that logged it.
fn record(m: Milestone, ms: u64) -> Option<String> {
    if slot(m)
        .compare_exchange(UNSET, ms, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return None;
    }
    let (hotkeys, bar) = (
        HOTKEYS_MS.load(Ordering::Acquire),
        BAR_MS.load(Ordering::Acquire),
    );
    if hotkeys == UNSET || bar == UNSET || LOGGED.swap(true, Ordering::AcqRel) {
        return None;
    }
    let line = line(hotkeys, bar);
    log::info!("{line}");
    Some(line)
}

/// The log line `scripts/wb-perf/startup.py` parses.
pub fn line(hotkeys_ms: u64, bar_ms: u64) -> String {
    format!(
        "startup: ready {} ms after launch (hotkeys {hotkeys_ms} ms, bar {bar_ms} ms)",
        hotkeys_ms.max(bar_ms)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ready_line_comes_once_when_both_are_in() {
        // Process-wide slots: this is the only test that writes them.
        assert_eq!(record(Milestone::Bar, 512), None);
        assert_eq!(record(Milestone::Bar, 900), None, "first mark counts");
        assert_eq!(
            record(Milestone::Hotkeys, 430).as_deref(),
            Some("startup: ready 512 ms after launch (hotkeys 430 ms, bar 512 ms)")
        );
        assert_eq!(record(Milestone::Hotkeys, 1000), None, "logged once");
        assert_eq!(BAR_MS.load(Ordering::Acquire), 512);
        assert_eq!(HOTKEYS_MS.load(Ordering::Acquire), 430);
    }

    #[test]
    fn the_launch_is_before_now() {
        let age = process_age();
        if cfg!(target_os = "macos") {
            let age = age.expect("proc_pidinfo");
            assert!(age < Duration::from_secs(24 * 3600));
        }
        assert!(launch() <= Instant::now());
    }

    /// E-85 (regression protected by S-38): one instance. A second launch
    /// hands its arguments to the running app (the CLI toggles, the soak's
    /// `--toggle-transcription`, a relaunch that rebuilds a parked Hub)
    /// instead of starting a second app with its own tap and bar.
    #[test]
    fn single_instance_stays_registered_with_the_cli_routes() {
        let lib = include_str!("../lib.rs");
        let at = lib
            .find("tauri_plugin_single_instance::init(")
            .expect("single-instance plugin registered");
        let end = at + lib[at..].find("}));").expect("handler end");
        let handler = &lib[at..end];
        for route in [
            "--toggle-transcription",
            "--toggle-post-process",
            "--cancel",
            "show_main_window",
        ] {
            assert!(handler.contains(route), "{route}");
        }
        let manifest = include_str!("../../Cargo.toml");
        assert!(manifest.contains("tauri-plugin-single-instance"));
    }
}
