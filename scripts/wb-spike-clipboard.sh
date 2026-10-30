#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# RS-05 clipboard and paste test: builds src-tauri/crates/wb-spike-clipboard
# (release, its own workspace and lockfile, so the app build is untouched)
# and starts the guided session. Extra arguments go to the tool (--auto,
# --self-check, --dry-run, --restore-owner FILE, --out DIR). See
# docs/fork/spikes/RS-05-procedure.md.
#
#   ./scripts/wb-spike-clipboard.sh

set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
crate="$repo/src-tauri/crates/wb-spike-clipboard"

if ! command -v cargo >/dev/null 2>&1; then
  echo "Rust (cargo) is not installed. Install it from rustup.rs, then run this again." >&2
  exit 1
fi

echo "Preparing the clipboard test (the first time this takes a minute)..."
WB_GIT_SHA="$(git -C "$repo" rev-parse --short HEAD 2>/dev/null || echo unknown)" \
  cargo build --release --locked --quiet --manifest-path "$crate/Cargo.toml"

exec "$crate/target/release/wb-spike-clipboard" "$@"
