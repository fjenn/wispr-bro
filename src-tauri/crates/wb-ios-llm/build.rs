// SPDX-License-Identifier: GPL-3.0-or-later
//! Builds (or reuses) the native library for the target and links it.
//! `native/build.sh` caches per target triple, not per cargo profile, so a
//! debug and a release build of the app share one llama.cpp build.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let native = manifest.join("native");
    for f in [
        "build.sh",
        "CMakeLists.txt",
        "wb_llm.cpp",
        "wb_llm.h",
        "exports.txt",
    ] {
        println!("cargo:rerun-if-changed={}", native.join(f).display());
    }
    for v in [
        "WB_IOS_LLM_CACHE",
        "WB_LLAMA_SRC_TARBALL",
        "IPHONEOS_DEPLOYMENT_TARGET",
    ] {
        println!("cargo:rerun-if-env-changed={v}");
    }
    let target = std::env::var("TARGET").unwrap();
    if !matches!(
        target.as_str(),
        "aarch64-apple-darwin" | "aarch64-apple-ios" | "aarch64-apple-ios-sim"
    ) {
        // Other targets get the crate's types and an engine that never loads
        // (`Engine::load` returns `Error::Unsupported`).
        println!("cargo:rustc-cfg=wb_ios_llm_stub");
        println!("cargo:rustc-check-cfg=cfg(wb_ios_llm_stub)");
        return;
    }
    println!("cargo:rustc-check-cfg=cfg(wb_ios_llm_stub)");
    let out = Command::new("bash")
        .arg(native.join("build.sh"))
        .arg(&target)
        // Xcode's build phase exports SDKROOT and friends for the app's
        // platform; cmake must see the triple's own.
        .env_remove("SDKROOT")
        .env_remove("MACOSX_DEPLOYMENT_TARGET")
        .output()
        .expect("run native/build.sh");
    if !out.status.success() {
        panic!(
            "native/build.sh {target} failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let dir = stdout
        .lines()
        .last()
        .expect("build.sh prints the output directory");
    println!("cargo:rustc-link-search=native={dir}");
    println!("cargo:rustc-link-lib=static=wb_ios_llm");
    println!("cargo:rustc-link-lib=c++");
    for framework in ["Foundation", "Metal", "MetalKit", "Accelerate"] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
}
