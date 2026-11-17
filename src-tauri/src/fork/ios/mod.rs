// SPDX-License-Identifier: GPL-3.0-or-later
//
//! Wispr Bro for iPhone: what iOS forces to differ from the Mac app, and
//! nothing else. The app is the same crate; this module holds
//!
//! - the entry point ([`start`]);
//! - the take API the Swift session calls ([`core`], `wb_core.h`);
//! - the iPhone-only commands ([`commands`]) over the Swift session's own C
//!   API ([`host`]) and the shared App Group settings ([`defaults`]);
//! - stand-ins for desktop-only pieces (`stubs/`, [`compat`], and the crates
//!   `wb-ios-handy-keys` / `wb-ios-enigo`).
//!
//! What is off on the phone, and where, is listed in `docs/fork/IOS.md`.

pub mod commands;
pub mod compat;
pub mod core;
pub mod defaults;
pub mod host;
pub mod selftest;

use tauri::AppHandle;

extern "C" {
    fn wb_ios_scene_install();
}

/// The iOS entry point (`start_app`, called from the Xcode project's
/// `main.mm`): the same start as the Mac's `main.rs`.
#[tauri::mobile_entry_point]
fn start() {
    // Rust's runtime start-up, which ignores SIGPIPE, does not run under an
    // Objective-C `main`. A write to the engine's pipe after it ended must
    // be an error, not the end of the app.
    // SAFETY: setting a signal's disposition to SIG_IGN has no preconditions.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_IGN);
    }
    // As on the Mac (`lib.rs::run`): no ggml-metal residency sets, whose
    // teardown asserts when an engine outlives the app's shutdown.
    if std::env::var_os("GGML_METAL_NO_RESIDENCY").is_none() {
        std::env::set_var("GGML_METAL_NO_RESIDENCY", "1");
    }
    // The scene life cycle the iOS 27 SDK requires (scene.m), before UIKit
    // starts.
    // SAFETY: no arguments; idempotent.
    unsafe {
        wb_ios_scene_install();
    }
    crate::fork::early_init();
    crate::run(crate::fork::cli_args(crate::CliArgs::default()));
}

/// From `fork::on_ready` (product builds), after the speech model setup
/// started: takes may run from here on.
pub fn on_ready(app: &AppHandle) {
    core::on_ready(app);
    commands::watch(app);
    selftest::start(app);
}
