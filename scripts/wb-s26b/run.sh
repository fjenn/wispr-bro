#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-26b (docs/fork/MAC-TEST-SCRIPT.md): Help's Report a problem
# (⌘/) and the Share with a friend sheet on the running test app (identity
# com.wisprbro.s26bcheck). Each step holds ~/wb-bench/gui.lock; key presses
# go only to the test app while it is in front (scripts/wb-s25/ui.swift);
# menus and the picker are read through Accessibility
# (scripts/wb-s26a/menus.swift, scripts/wb-s26b/share.swift). No audio.
# Nothing is submitted on GitHub: the browser tab is left for a look and
# closed by the operator.
#
#   menus <pid> <out dir>              Help and tray carry the two items, ⌘/
#   issue <pid> <log file> <out dir>   S-26b.1: ⌘/ opens the prefilled issue
#   share <pid> <x> <y> <out dir>      S-26b.2: Share… at window point (x, y)
#                                      shows the share sheet
#
# Usage: scripts/wb-s26b/run.sh <step> … ; WBUI / WBMENUS / WBSHARE name the
# compiled drivers.
set -u
STEP="$1"; shift
if [ "${S26B_LOCKED:-}" != 1 ]; then
  S26B_LOCKED=1 exec /usr/bin/lockf -k -t 600 "$HOME/wb-bench/gui.lock" "$0" "$STEP" "$@"
fi
UI="${WBUI:?set WBUI to the compiled scripts/wb-s25/ui.swift}"
MENUS="${WBMENUS:?set WBMENUS to the compiled scripts/wb-s26a/menus.swift}"
SHARE="${WBSHARE:?set WBSHARE to the compiled scripts/wb-s26b/share.swift}"
fail=0
check() { # $1 = label, $2 = 0 for pass
  if [ "$2" = 0 ]; then echo "PASS $1"; else echo "FAIL $1"; fail=1; fi
}

case "$STEP" in
  menus)
    PID="$1"; OUT="$2"; mkdir -p "$OUT"
    "$MENUS" "$PID" menubar > "$OUT/menubar.txt" || exit 1
    grep -qF "Report a problem… [⌘/]" "$OUT/menubar.txt"; check "Help › Report a problem… with ⌘/" $?
    grep -qF "Send feedback…" "$OUT/menubar.txt"; check "Help › Send feedback…" $?
    ;;
  issue)
    PID="$1"; LOG="$2"; OUT="$3"; mkdir -p "$OUT"
    before=$(grep -c "help: opened the Problem form on GitHub" "$LOG" 2>/dev/null || echo 0)
    "$UI" "$PID" "front; key cmd-slash; wait 500" || exit 1
    "$SHARE" 0 frontmost 15 | tee "$OUT/browser.txt"
    check "a browser came to the front" "${PIPESTATUS[0]}"
    screencapture -x "$OUT/issue.png"
    after=$(grep -c "help: opened the Problem form on GitHub" "$LOG" 2>/dev/null || echo 0)
    [ "$after" -gt "$before" ]; check "the app opened the Problem form (log)" $?
    # Safari exposes the page URL (AXDocument); other browsers show the title.
    if grep -q "issues/new" "$OUT/browser.txt"; then
      grep -q "title=Problem" "$OUT/browser.txt"; check "the URL is the prefilled form" $?
    else
      echo "NOTE the browser hides its URL from AX: confirm on issue.png that the form shows \"Problem: \" and the About this Mac block"
    fi
    ;;
  share)
    PID="$1"; X="$2"; Y="$3"; OUT="$4"; mkdir -p "$OUT"
    "$UI" "$PID" "front; click $X $Y; wait 300" || exit 1
    "$SHARE" "$PID" picker 10 | tee "$OUT/picker.txt"
    check "the share sheet shows" "${PIPESTATUS[0]}"
    screencapture -x "$OUT/share-sheet.png"
    "$UI" "$PID" "key escape" > /dev/null
    ;;
  *)
    echo "unknown step $STEP"; exit 2 ;;
esac
exit $fail
