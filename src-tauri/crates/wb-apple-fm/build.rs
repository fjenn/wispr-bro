// SPDX-License-Identifier: GPL-3.0-or-later
//
// The Swift build follows Handy's `build_apple_intelligence_bridge`
// (src-tauri/build.rs; Copyright (c) 2025 CJ Pais; MIT, LICENSES/MIT-Handy.txt):
// the same SDK and `swiftc` lookup, `-parse-as-library`, a static library,
// weak-linked FoundationModels. Modified by Wispr Bro contributors, 2026-10-31:
// its own crate and symbols (`wb_apple_fm_`), and no stub Swift file: without
// the macOS 26 SDK and full Xcode the crate builds its Rust stub instead
// (`cfg(wb_apple_fm_stub)`), so no Swift is compiled at all.
//
//! Builds `swift/fork_apple_fm.swift` into `libwb_apple_fm.a`, or sets
//! `cfg(wb_apple_fm_stub)` when it cannot (not macOS on Apple Silicon, no
//! FoundationModels in the SDK, a Command Line Tools-only toolchain, or
//! `WB_APPLE_FM_STUB=1`).

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

const SWIFT_FILE: &str = "swift/fork_apple_fm.swift";

fn main() {
    println!("cargo::rustc-check-cfg=cfg(wb_apple_fm_stub)");
    println!("cargo:rerun-if-changed={SWIFT_FILE}");
    println!("cargo:rerun-if-env-changed=WB_APPLE_FM_STUB");
    println!("cargo:rerun-if-env-changed=SDKROOT");
    println!("cargo:rerun-if-env-changed=SWIFTC");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if target_os != "macos" || target_arch != "aarch64" {
        return stub("not macOS on Apple Silicon");
    }
    if env::var("WB_APPLE_FM_STUB").as_deref() == Ok("1") {
        return stub("WB_APPLE_FM_STUB=1");
    }
    let Some(sdk) = env::var("SDKROOT").ok().or_else(|| {
        run("xcrun", &["--sdk", "macosx", "--show-sdk-path"])
    }) else {
        return stub("no macOS SDK");
    };
    if !Path::new(&sdk)
        .join("System/Library/Frameworks/FoundationModels.framework")
        .exists()
    {
        return stub("the SDK has no FoundationModels (needs the macOS 26 SDK)");
    }
    // The CLT `swiftc` has no FoundationModelsMacros plugin (`@Generable`).
    if env::var("SWIFTC").is_err()
        && run("xcode-select", &["-p"]).is_some_and(|p| p.ends_with("CommandLineTools"))
    {
        return stub("Command Line Tools only; FoundationModels needs full Xcode");
    }
    let Some(swiftc) = env::var("SWIFTC")
        .ok()
        .or_else(|| run("xcrun", &["--find", "swiftc"]))
    else {
        return stub("no swiftc");
    };

    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let object = out.join("wb_apple_fm.o");
    let lib = out.join("libwb_apple_fm.a");
    let status = Command::new(&swiftc)
        .args([
            "-parse-as-library",
            "-module-name",
            "WbAppleFm",
            "-target",
            "arm64-apple-macosx11.0",
            "-sdk",
            &sdk,
            "-O",
            "-c",
            SWIFT_FILE,
            "-o",
        ])
        .arg(&object)
        .status()
        .expect("swiftc");
    assert!(status.success(), "swiftc failed on {SWIFT_FILE}");
    let status = Command::new("libtool")
        .arg("-static")
        .arg("-o")
        .arg(&lib)
        .arg(&object)
        .status()
        .expect("libtool");
    assert!(status.success(), "libtool failed for {SWIFT_FILE}");

    let toolchain_swift = Path::new(&swiftc)
        .parent()
        .and_then(Path::parent)
        .map(|root| root.join("lib/swift/macosx"))
        .expect("swift toolchain lib dir");
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=wb_apple_fm");
    println!("cargo:rustc-link-search=native={}", toolchain_swift.display());
    println!(
        "cargo:rustc-link-search=native={}",
        Path::new(&sdk).join("usr/lib/swift").display()
    );
    println!("cargo:rustc-link-lib=framework=Foundation");
    // Weak, so the app still launches on macOS 13-15.
    println!("cargo:rustc-link-arg=-weak_framework");
    println!("cargo:rustc-link-arg=FoundationModels");
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
}

fn stub(why: &str) {
    println!("cargo:rustc-cfg=wb_apple_fm_stub");
    // Only worth a warning where a real build was possible.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:warning=wb-apple-fm: Apple Foundation Models stub ({why})");
    }
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!s.is_empty()).then_some(s)
}
