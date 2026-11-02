#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-26a (docs/fork/MAC-TEST-SCRIPT.md): menus and tray on the
# running test app (identity com.wisprbro.s26acheck). Each step holds
# ~/wb-bench/gui.lock, posts events only while the app is in front
# (scripts/wb-s25/ui.swift) and reads menus through Accessibility
# (scripts/wb-s26a/menus.swift). The user's clipboard is saved first and
# restored after the Edit step. No audio.
#
#   menubar <pid> <out dir>             S-26a.0: structure dump + checks
#   edit <pid> <x> <y> <out dir>        S-26a.1: ⌘X/⌘V/⌘Z/⌘C in the Hub text
#                                       field at window point (x, y)
#   help <pid> <query> <out dir>        S-26a.2: ⌘? + query; lists the rows
#   counts <pid> <out dir>              S-26a.4: Vocabulary › Add word… adds a
#                                       word; the count item follows live
#   tray <pid> <out dir>                S-26a.3: restart ControlCenter, the
#                                       status item must come back (A-69)
#
# Usage: scripts/wb-s26a/run.sh <step> … ; WBUI / WBMENUS name the compiled
# drivers (default: <scratch>/wbui, <scratch>/wbmenus).
set -u
STEP="$1"; shift
if [ "${S26A_LOCKED:-}" != 1 ]; then
  S26A_LOCKED=1 exec /usr/bin/lockf -k -t 600 "$HOME/wb-bench/gui.lock" "$0" "$STEP" "$@"
fi
UI="${WBUI:?set WBUI to the compiled scripts/wb-s25/ui.swift}"
MENUS="${WBMENUS:?set WBMENUS to the compiled scripts/wb-s26a/menus.swift}"
fail=0
check() { # $1 = label, $2 = 0 for pass
  if [ "$2" = 0 ]; then echo "PASS $1"; else echo "FAIL $1"; fail=1; fi
}

case "$STEP" in
  menubar)
    PID="$1"; OUT="$2"; mkdir -p "$OUT"
    "$MENUS" "$PID" menubar > "$OUT/menubar.txt" || exit 1
    cat "$OUT/menubar.txt"
    for k in "Undo [⌘z]" "Cut [⌘x]" "Copy [⌘c]" "Paste [⌘v]" "Select All [⌘a]"; do
      grep -qF "$k" "$OUT/menubar.txt"; check "Edit has $k" $?
    done
    for m in Dictation Vocabulary Help; do
      grep -qx "$m" "$OUT/menubar.txt"; check "menu $m" $?
    done
    grep -q "words\? dictated" "$OUT/menubar.txt"; check "Dictation shows the word count" $?
    grep -q "Copy last transcript" "$OUT/menubar.txt"; check "Copy last transcript" $?
    for i in Documentation Shortcuts Microphone Languages "Add word…" "New snippet…"; do
      grep -qF "$i" "$OUT/menubar.txt"; check "item $i" $?
    done
    ;;
  edit)
    PID="$1"; X="$2"; Y="$3"; OUT="$4"; mkdir -p "$OUT"
    "$MENUS" 0 clip-save "$OUT/clipboard.json" || exit 1
    trap '"$MENUS" 0 clip-restore "$OUT/clipboard.json"' EXIT
    "$UI" "$PID" "front; click $X $Y; key cmd-a; type s26a probe; key cmd-a; key cmd-x; wait 300" || exit 1
    v=$("$MENUS" "$PID" focused); echo "after ⌘X: $v"
    [ "$("$MENUS" 0 clip-text)" = "s26a probe" ]; check "⌘X cut into the clipboard" $?
    "$UI" "$PID" "key cmd-v; wait 200; key cmd-v; wait 300" || exit 1
    v=$("$MENUS" "$PID" focused); echo "after ⌘V ⌘V: $v"
    [[ "$v" == *"s26a probes26a probe" ]]; check "⌘V pasted twice" $?
    "$UI" "$PID" "key cmd-z; wait 300" || exit 1
    v=$("$MENUS" "$PID" focused); echo "after ⌘Z: $v"
    [[ "$v" == *" s26a probe" && "$v" != *"probes26a"* ]]; check "⌘Z undid one paste" $?
    "$UI" "$PID" "key cmd-a; key cmd-c; wait 200" || exit 1
    [ "$("$MENUS" 0 clip-text)" = "s26a probe" ]; check "⌘C copied the selection" $?
    "$UI" "$PID" "key cmd-a; key cmd-x" > /dev/null
    ;;
  help)
    PID="$1"; Q="$2"; OUT="$3"; mkdir -p "$OUT"
    "$UI" "$PID" "front; key cmd-shift-slash; wait 800; type $Q; wait 1500" || exit 1
    "$MENUS" "$PID" help-results | tee "$OUT/help-results.txt"
    screencapture -x "$OUT/help-search.png"
    "$UI" "$PID" "key escape; key escape" > /dev/null
    [ "$(grep -c . "$OUT/help-results.txt")" -gt 1 ]; check "Help menu lists results for \"$Q\" (see help-search.png)" $?
    ;;
  counts)
    PID="$1"; OUT="$2"; mkdir -p "$OUT"
    before=$("$MENUS" "$PID" menubar | grep -o "[0-9,]* dictionary words\?" | head -1)
    "$MENUS" "$PID" press "Vocabulary/Add word…"
    sleep 1
    "$UI" "$PID" "front; type S26a$RANDOM; key return; wait 1200; key escape" || exit 1
    after=$("$MENUS" "$PID" menubar | grep -o "[0-9,]* dictionary words\?" | head -1)
    echo "before: $before; after: $after"
    b=${before%% *}; a=${after%% *}
    [ "${a//,/}" = $(( ${b//,/} + 1 )) ]; check "dictionary count followed live" $?
    ;;
  tray)
    PID="$1"; OUT="$2"; mkdir -p "$OUT"
    [ "$("$MENUS" "$PID" tray)" -gt 0 ]; check "status item present" $?
    killall ControlCenter
    sleep 2
    "$MENUS" "$PID" wait-tray 30; check "status item back after ControlCenter restart" $?
    screencapture -x -R 0,0,3000,40 "$OUT/menu-bar.png"
    ;;
  *)
    echo "unknown step $STEP"; exit 2 ;;
esac
exit $fail
