#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# S-05 real-Mac check (docs/fork/MAC-TEST-SCRIPT.md S-05.3): the product's
# AX verdict in Finder. Opens a Finder window on a scratch folder (our own
# window), watches the verdict with `wb-spike-ax --watch`, closes the window
# and watches again (Finder frontmost with our window gone). Posts no key
# events. Run under the GUI lock:
#   /usr/bin/lockf -k ~/wb-bench/gui.lock scripts/wb-s05/finder-probe.sh <out>
set -u
OUT="${1:?out file}"
HERE="$(cd "$(dirname "$0")" && pwd)"
SPIKE="$HERE/../../src-tauri/crates/wb-spike-ax/target/release/wb-spike-ax"
DIR="${TMPDIR:-/tmp}/wb-s05-finder"
mkdir -p "$DIR"
touch "$DIR/one.txt" "$DIR/two.txt"

osascript -e "tell application \"Finder\"" -e "activate" \
  -e "set w to make new Finder window to (POSIX file \"$DIR\" as alias)" \
  -e "end tell" >/dev/null
sleep 1.5
echo "# own Finder window (file list)" >"$OUT"
"$SPIKE" --watch 3 >>"$OUT"
osascript -e 'tell application "Finder" to close Finder window "wb-s05-finder"' >/dev/null
sleep 1
echo "# Finder frontmost, our window closed" >>"$OUT"
"$SPIKE" --watch 3 >>"$OUT"
cat "$OUT"
