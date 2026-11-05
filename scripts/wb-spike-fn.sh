#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# RS-01 fn / Globe key test: builds src-tauri/crates/wb-spike-fn (release,
# its own workspace and lockfile, so the app build is untouched) and starts
# the guided session. Extra arguments go to the tool (--dry-run,
# --self-test, --out DIR). See docs/fork/spikes/RS-01-procedure.md.
#
#   ./scripts/wb-spike-fn.sh

set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
crate="$repo/src-tauri/crates/wb-spike-fn"

if ! command -v cargo >/dev/null 2>&1; then
  echo "Rust (cargo) is not installed. Install it from rustup.rs, then run this again." >&2
  exit 1
fi

echo "Preparing the fn key test (the first time this takes a minute)..."
WB_GIT_SHA="$(git -C "$repo" rev-parse --short HEAD 2>/dev/null || echo unknown)" \
  cargo build --release --locked --quiet --manifest-path "$crate/Cargo.toml"

exec "$crate/target/release/wb-spike-fn" "$@"
