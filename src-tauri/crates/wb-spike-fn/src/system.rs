// SPDX-License-Identifier: GPL-3.0-or-later
//! macOS facts the session needs: permissions of the terminal app, Secure
//! Input, the event taps this process owns (and whether each is enabled),
//! and a few machine details for the report. Public APIs only.

use serde::Serialize;
use std::process::Command;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightListenEventAccess() -> bool;
    fn CGRequestListenEventAccess() -> bool;
    fn CGGetEventTapList(max: u32, list: *mut CGEventTapInformation, count: *mut u32) -> i32;
}

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    fn IsSecureEventInputEnabled() -> u8;
}

/// `CGEventTapInformation` (CoreGraphics/CGEventTypes.h).
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGEventTapInformation {
    event_tap_id: u32,
    tap_point: u32,
    options: u32,
    events_of_interest: u64,
    tapping_process: i32,
    process_being_tapped: i32,
    enabled: bool,
    min_usec_latency: f32,
    avg_usec_latency: f32,
    max_usec_latency: f32,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct TapInfo {
    pub id: u32,
    pub enabled: bool,
    /// 0 = active filter (can block), 1 = listen only.
    pub options: u32,
    pub events_of_interest: u64,
    pub avg_usec_latency: f32,
    pub max_usec_latency: f32,
}

/// Event taps owned by this process, from `CGGetEventTapList`. Called only
/// at block boundaries (never polled), cf. handy-keys #1827.
pub fn own_taps() -> Vec<TapInfo> {
    let pid = std::process::id() as i32;
    let mut count: u32 = 0;
    // SAFETY: a null list asks only for the count.
    if unsafe { CGGetEventTapList(0, std::ptr::null_mut(), &mut count) } != 0 {
        return Vec::new();
    }
    let mut list = vec![CGEventTapInformation::default(); count as usize + 8];
    let mut got: u32 = 0;
    // SAFETY: `list` has room for `list.len()` entries.
    if unsafe { CGGetEventTapList(list.len() as u32, list.as_mut_ptr(), &mut got) } != 0 {
        return Vec::new();
    }
    list.truncate(got as usize);
    list.into_iter()
        .filter(|t| t.tapping_process == pid)
        .map(|t| TapInfo {
            id: t.event_tap_id,
            enabled: t.enabled,
            options: t.options,
            events_of_interest: t.events_of_interest,
            avg_usec_latency: t.avg_usec_latency,
            max_usec_latency: t.max_usec_latency,
        })
        .collect()
}

pub fn accessibility_granted() -> bool {
    handy_keys::check_accessibility()
}

pub fn input_monitoring_granted() -> bool {
    // SAFETY: no arguments; reads TCC state.
    unsafe { CGPreflightListenEventAccess() }
}

/// Adds the terminal to the Input Monitoring list (macOS may show a prompt).
pub fn request_input_monitoring() -> bool {
    // SAFETY: no arguments.
    unsafe { CGRequestListenEventAccess() }
}

pub fn secure_input_on() -> bool {
    // SAFETY: no arguments.
    unsafe { IsSecureEventInputEnabled() != 0 }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    Accessibility,
    InputMonitoring,
    Microphone,
    Keyboard,
}

/// Opens a System Settings pane (local `open`, no network).
pub fn open_pane(pane: Pane) {
    let url = match pane {
        Pane::Accessibility => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
        }
        Pane::InputMonitoring => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
        }
        Pane::Microphone => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
        }
        Pane::Keyboard => "x-apple.systempreferences:com.apple.Keyboard-Settings.extension",
    };
    let _ = Command::new("/usr/bin/open").arg(url).status();
}

/// The app that owns the permissions (the one the command was typed into).
pub fn terminal_name(term_program: Option<&str>) -> String {
    match term_program {
        Some("Apple_Terminal") => "Terminal".into(),
        Some("iTerm.app") => "iTerm".into(),
        Some("vscode") => "Visual Studio Code".into(),
        Some("WarpTerminal") => "Warp".into(),
        Some("ghostty") => "Ghostty".into(),
        Some("WezTerm") => "WezTerm".into(),
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

/// Discards anything typed into the terminal that has not been read yet
/// (stray Esc or arrow presses during a block).
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
    fn tap_info_layout_matches_the_c_struct() {
        // u32 ×3, pad, u64, i32 ×2, bool, pad, f32 ×3, pad → 48 bytes.
        assert_eq!(std::mem::size_of::<CGEventTapInformation>(), 48);
        assert_eq!(std::mem::align_of::<CGEventTapInformation>(), 8);
    }

    #[test]
    fn terminal_names() {
        assert_eq!(terminal_name(Some("Apple_Terminal")), "Terminal");
        assert_eq!(terminal_name(Some("iTerm.app")), "iTerm");
        assert!(terminal_name(None).contains("Terminal"));
    }

    #[test]
    fn tap_list_call_is_safe_without_taps() {
        // This test process owns no taps.
        assert!(own_taps().is_empty());
    }
}
