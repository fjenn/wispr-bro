// SPDX-License-Identifier: GPL-3.0-or-later
//! macOS facts and actions the session needs: the Accessibility permission,
//! NSWorkspace's frontmost app (kept fresh by running the main run loop),
//! opening apps and files with `open`, and machine details for the report.
//! Public APIs only; nothing touches the network.

use std::process::Command;
use std::time::{Duration, Instant};

use objc2_app_kit::NSWorkspace;
use objc2_application_services::AXIsProcessTrusted;
use objc2_core_foundation::{kCFRunLoopDefaultMode, CFRunLoop};
use serde::Serialize;

pub fn accessibility_granted() -> bool {
    // SAFETY: no arguments; reads TCC state for this process.
    unsafe { AXIsProcessTrusted() }
}

/// Runs the main thread's run loop for `d`, so NSWorkspace's notifications
/// (frontmost app changes) are delivered, as they would be in the app.
/// Call from the main thread only.
pub fn pump(d: Duration) {
    // SAFETY: the default mode constant is a valid static.
    let mode = unsafe { kCFRunLoopDefaultMode };
    let _ = CFRunLoop::run_in_mode(mode, d.as_secs_f64(), false);
}

/// NSWorkspace's frontmost application PID. Main thread, after `pump`.
pub fn frontmost_pid() -> Option<i32> {
    let ws = NSWorkspace::sharedWorkspace();
    ws.frontmostApplication()
        .map(|a| a.processIdentifier())
        .filter(|p| *p > 0)
}

/// Pumps the run loop for `d` and refreshes the AX thread's frontmost hint.
pub fn tick(d: Duration) -> Option<i32> {
    pump(d);
    let pid = frontmost_pid();
    crate::ax::thread::set_front_pid(pid);
    pid
}

pub fn open_app(app: &str) -> bool {
    Command::new("/usr/bin/open")
        .args(["-a", app])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn open_with(app: &str, file: &std::path::Path) -> bool {
    Command::new("/usr/bin/open")
        .arg("-a")
        .arg(app)
        .arg(file)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn open_accessibility_pane() {
    let _ = Command::new("/usr/bin/open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .status();
}

/// Is `pid` an instance of the app bundle at `path`?
/// Compared by bundle file name: Safari runs from its cryptex
/// (`/System/Volumes/Preboot/Cryptexes/App/...`), not `/Applications`.
pub fn pid_is_app(pid: i32, path: &str) -> bool {
    let name = |p: &str| p.rsplit('/').next().unwrap_or("").to_string();
    crate::ax::element::app_bundle_path(pid).is_some_and(|b| b == path || name(&b) == name(path))
}

/// After `open -a`: when did NSWorkspace and AX each first report the app
/// as frontmost? (RS-04 "frontmostApplication vs AXFocusedApplication
/// freshness".) Polls every 10 ms for up to `limit`.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Activation {
    pub workspace_ms: Option<f64>,
    pub ax_ms: Option<f64>,
    pub ax_failed: bool,
}

pub fn wait_activation(
    ax: &crate::ax::thread::AxThread,
    path: &'static str,
    started: Instant,
    limit: Duration,
) -> Activation {
    let mut a = Activation::default();
    while started.elapsed() < limit && (a.workspace_ms.is_none() || a.ax_ms.is_none()) {
        let ws = tick(Duration::from_millis(10));
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        if a.workspace_ms.is_none() && ws.is_some_and(|p| pid_is_app(p, path)) {
            a.workspace_ms = Some(ms);
        }
        if a.ax_ms.is_none() && !a.ax_failed {
            let ax_pid = ax
                .run(Duration::from_millis(300), |ctx| {
                    let sys = ctx.system.clone();
                    crate::ax::element::set_timeout(&sys, 0.1);
                    crate::ax::element::attr_element(&sys, "AXFocusedApplication")
                        .and_then(|e| crate::ax::element::element_pid(&e))
                })
                .flatten();
            match ax_pid {
                Some(p) if pid_is_app(p, path) => {
                    a.ax_ms = Some(started.elapsed().as_secs_f64() * 1000.0)
                }
                Some(_) => {}
                None => a.ax_failed = true,
            }
        }
    }
    a
}

/// The app that owns the permissions (the one the command was typed into).
pub fn terminal_name(term_program: Option<&str>) -> String {
    match term_program {
        Some("Apple_Terminal") => "Terminal".into(),
        Some("iTerm.app") => "iTerm".into(),
        Some("vscode") => "Visual Studio Code".into(),
        Some("WarpTerminal") => "Warp".into(),
        Some("ghostty") => "Ghostty".into(),
        _ => "the app you typed the command into (for example Terminal)".into(),
    }
}

fn run(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

#[derive(Clone, Debug, Serialize)]
pub struct MachineInfo {
    pub macos_version: String,
    pub macos_build: String,
    pub model: String,
    pub cpu: String,
    pub term_program: Option<String>,
}

pub fn machine_info() -> MachineInfo {
    MachineInfo {
        macos_version: run("/usr/bin/sw_vers", &["-productVersion"]),
        macos_build: run("/usr/bin/sw_vers", &["-buildVersion"]),
        model: run("/usr/sbin/sysctl", &["-n", "hw.model"]),
        cpu: run("/usr/sbin/sysctl", &["-n", "machdep.cpu.brand_string"]),
        term_program: std::env::var("TERM_PROGRAM").ok(),
    }
}

/// Discards anything typed into the terminal that has not been read yet.
pub fn flush_stdin() {
    // SAFETY: tcflush on fd 0; harmless if stdin is not a terminal.
    unsafe {
        libc::tcflush(0, libc::TCIFLUSH);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_names() {
        assert_eq!(terminal_name(Some("Apple_Terminal")), "Terminal");
        assert!(terminal_name(None).contains("Terminal"));
    }
}
