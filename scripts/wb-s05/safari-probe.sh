#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# S-05 diagnostics: opens fields.html#<field> in a new Safari window (ours),
# prints the frontmost app, Safari's front window and the product verdict
# for 3 s (`wb-spike-ax --watch`), then closes the window. Posts no events.
# Needs the page server (run.sh starts one on 127.0.0.1:47815).
#   /usr/bin/lockf -k ~/wb-bench/gui.lock /bin/bash scripts/wb-s05/safari-probe.sh input
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
SPIKE="$HERE/../../src-tauri/crates/wb-spike-ax/target/release/wb-spike-ax"
osascript -e 'tell application "Safari"' \
  -e "make new document with properties {URL:\"http://127.0.0.1:47815/fields.html#${1:-input}\"}" \
  -e 'activate' -e 'end tell' >/dev/null
sleep 3
osascript -e 'tell application "Safari" to get {name, index, visible} of every window'
"$SPIKE" --watch 3
osascript -e 'tell application "Safari" to close (every document whose URL contains "47815/fields.html")'
