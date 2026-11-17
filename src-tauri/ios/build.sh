#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Wispr Bro (iOS): build the iPhone app.
#   src-tauri/ios/build.sh sim      debug build for the simulator (arm64)
#   src-tauri/ios/build.sh device   debug build for an iPhone, automatic signing
# Extra arguments go to `tauri ios build` (e.g. `--release` is the default
# there; this script passes --debug unless WB_IOS_RELEASE=1).
set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(cd "$here/../.." && pwd)
. "$here/env.sh"
kind=${1:-sim}
shift || true
profile=(--debug)
[ "${WB_IOS_RELEASE:-0}" = "1" ] && profile=()
case "$kind" in
  sim) target=aarch64-sim ;;
  device) target=aarch64 ;;
  *) echo "usage: $0 sim|device [tauri ios build args]" >&2; exit 2 ;;
esac
# The CLI moves the built app here and fails if the last one is still there.
case "$target" in
  aarch64-sim) rm -rf "$here/../gen/apple/build/arm64-sim" ;;
  aarch64) rm -rf "$here/../gen/apple/build/arm64" ;;
esac
cd "$root"
exec bun tauri ios build \
  --config src-tauri/tauri.fork.conf.json \
  --config src-tauri/tauri.fork.ios.conf.json \
  --target "$target" "${profile[@]}" "$@"
