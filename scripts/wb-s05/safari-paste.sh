#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# S-05 diagnostics: does a synthetic ⌘V reach a field of fields.html in
# Safari at all (outside Wispr Bro)? Saves the clipboard, puts a marker
# text on it, opens the page in a new Safari window (ours), posts ⌘V only
# while Safari is frontmost, prints the page title, closes the window and
# restores the clipboard. Run under the GUI lock with WB_S05_BIN set.
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
BIN="${WB_S05_BIN:?}"
D="${TMPDIR:-/tmp}/wb-s05-safari-paste"
"$BIN/clip" save "$D" >/dev/null
printf 'wb s05 marker' | pbcopy
osascript -e 'tell application "Safari"' \
  -e "make new document with properties {URL:\"http://127.0.0.1:47815/fields.html#${1:-input}\"}" \
  -e 'activate' -e 'end tell' >/dev/null
sleep 3
front=$("$BIN/drive" front)
case "$front" in
  *com.apple.Safari*) osascript -e 'tell application "System Events" to keystroke "v" using command down' ;;
  *) echo "front is not Safari: $front" ;;
esac
sleep 1
osascript -e 'tell application "Safari" to get name of (first document whose URL contains "47815/fields.html")'
osascript -e 'tell application "Safari" to close (every document whose URL contains "47815/fields.html")'
"$BIN/clip" restore "$D"
