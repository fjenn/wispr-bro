# SPDX-License-Identifier: GPL-3.0-or-later
# Wispr Bro (iOS): environment for every iOS build of the Rust crate. Source
# it from bash or zsh (`. src-tauri/ios/env.sh`) before `bun tauri ios …` or
# a plain `cargo check --target aarch64-apple-ios[-sim]`. The Xcode "Build
# Rust Code" phase sets the same two things itself (gen/apple/project.yml).
_wb_ios_dir=$(cd "$(dirname "${BASH_SOURCE:-$0}")" && pwd)
case ":$PATH:" in
  *":$_wb_ios_dir/bin:"*) ;;
  # bin/swift: see the note in that file (swift-rs and Swift 6.4's SwiftPM).
  *) PATH="$_wb_ios_dir/bin:$PATH" ;;
esac
export PATH
export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-17.0}"
unset _wb_ios_dir
