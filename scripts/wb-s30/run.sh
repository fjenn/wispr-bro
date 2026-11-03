#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-30 (docs/fork/MAC-TEST-SCRIPT.md, S-30), for the orchestrator's
# consolidated UI wave. Drives the S-30 test bundle ("Wispr Bro S30 Check",
# com.wisprbro.s30check, portable). The spotlight is started by
# WISPRBRO_S30_SPOTLIGHT=<choice> (test identities only; the chooser's own
# "Try it now" is Playwright-tested and checked by eye in `eye`). Windows,
# clicks and keys go through scripts/wb-s03-pill.swift (window server and
# CGEvent), fn takes through scripts/wb-s01-keys.swift, with one `say` WAV
# played once through the speakers where a take needs the live mic. Every
# GUI case holds ~/wb-bench/gui.lock.
#
# usage: run.sh <case>   cases:
#   panel      the spotlight is on: a window of ours at layer 24 covers the
#              bar's display (`windows`), the resting bar shows at layer 25
#              above it; log "spotlight: on (email)"; screenshot
#   through    with the spotlight on, TextEdit in front: a click in its
#              document and typed keys reach TextEdit (the dim takes no
#              click); Wispr Bro never becomes frontmost
#   fullscreen TextEdit in full screen (⌃⌘F): the dim is still on screen
#              over it (window list + screenshot)
#   esc        Esc → log "spotlight: off (Esc)", the window is gone
#   own        a take into the Scratchpad window does not end it
#   mail       a new Mail message in front (nothing is sent; no screenshot
#              of Mail): fn, the WAV, fn up → the text lands, log
#              "spotlight: off (Dictated)" and "checklist: try_in_app done"
#   APP=<bundle> OUT=<dir> PILL=<compiled wb-s03-pill> KEYS=<compiled wb-s01-keys> WAV=<say -o file>
set -u
: "${APP:?}" "${OUT:?}" "${PILL:?}"
LOCK="$HOME/wb-bench/gui.lock"
HERE="$(cd "$(dirname "$0")" && pwd)"
EXE="$APP/Contents/MacOS/$(/usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "$APP/Contents/Info.plist")"
DATA="$APP/Contents/MacOS/Data"
LOGS="$DATA/logs"
mkdir -p "$OUT"

if [ "${WB_S30_LOCKED:-}" != 1 ]; then
  WB_S30_LOCKED=1 exec /usr/bin/lockf -k -t 1800 "$LOCK" /bin/bash "$0" "$@"
fi

pid() { pgrep -f "$EXE" | head -1; }
log() { echo "$(date +%H:%M:%S) $*" | tee -a "$OUT/run.log"; }
applog() { cat "$LOGS"/*.log 2>/dev/null; }
pill() { WB_PID="$(pid)" "$PILL" -e "$1"; }
quit_app() {
  local p; p=$(pid)
  [ -n "$p" ] && kill "$p"
  for _ in $(seq 1 40); do [ -z "$(pid)" ] && break; sleep 0.5; done
  [ -n "$(pid)" ] && kill -9 "$(pid)"
  log "quit"
}
launch_spotlight() {
  quit_app
  python3 -I "$HERE/profile.py" "$DATA" email | tee -a "$OUT/run.log"
  WISPRBRO_S30_SPOTLIGHT="${1:-email}" "$EXE" --start-hidden >/dev/null 2>&1 &
  for _ in $(seq 1 60); do applog | grep -q "spotlight: on" && break; sleep 0.5; done
  sleep 1
  applog | grep -q "spotlight: on" && log "PASS spotlight on" || log "FAIL no 'spotlight: on' in the log"
}
expect_log() {
  for _ in $(seq 1 ${2:-30}); do applog | grep -q "$1" && { log "PASS log: $1"; return 0; }; sleep 0.5; done
  log "FAIL log missing: $1"; return 1
}

case "${1:-}" in
  panel)
    launch_spotlight email
    pill "windows" | tee "$OUT/panel-windows.json"
    grep -q '"layer" *: *24' "$OUT/panel-windows.json" && log "PASS a layer-24 window" || log "FAIL no layer-24 window"
    grep -q '"layer" *: *25' "$OUT/panel-windows.json" && log "PASS the bar at layer 25" || log "FAIL no resting bar"
    screencapture -x "$OUT/panel.png"
    ;;
  through)
    launch_spotlight email
    open -a TextEdit; sleep 2
    WB_FRONT=TextEdit pill "cmd n; wait 800; click 500 400; wait 300; key a; key s; key a; wait 300; front" | tee "$OUT/through.json"
    grep -q '"front" *: *"TextEdit"' "$OUT/through.json" && log "PASS TextEdit stayed frontmost" || log "FAIL front changed"
    screencapture -x "$OUT/through.png"
    log "CHECK by eye in through.png: 'asa' typed in the TextEdit document under the dim"
    ;;
  fullscreen)
    launch_spotlight note
    open -a TextEdit; sleep 2
    WB_FRONT=TextEdit pill "cmd n; wait 800" >/dev/null
    "$KEYS" -e "mods down ctrl,command; key f; mods up ctrl,command; wait 2500"
    pill "windows" | tee "$OUT/fullscreen-windows.json"
    grep -q '"layer" *: *24' "$OUT/fullscreen-windows.json" && log "PASS dim over the full-screen app" || log "FAIL no dim in full screen"
    screencapture -x "$OUT/fullscreen.png"
    "$KEYS" -e "mods down ctrl,command; key f; mods up ctrl,command; wait 1500"
    ;;
  esc)
    launch_spotlight email
    pill "key escape; wait 800" >/dev/null
    expect_log "spotlight: off (Esc)"
    expect_log "spotlight: panel destroyed"
    ;;
  own)
    : "${KEYS:?}" "${WAV:?}"
    launch_spotlight message
    log "open the Scratchpad (bar button or its shortcut), then take:"
    "$KEYS" -e "fn down; wait 600" && afplay "$WAV" && "$KEYS" -e "wait 400; fn up; wait 6000"
    applog | grep -q "spotlight: off (Dictated)" && log "FAIL the own-app take ended it" || log "PASS still on after the own-app take"
    ;;
  mail)
    : "${KEYS:?}" "${WAV:?}"
    launch_spotlight email
    open -a Mail; sleep 3
    WB_FRONT=Mail pill "cmd n; wait 1500" >/dev/null
    "$KEYS" -e "fn down; wait 600" && afplay "$WAV" && "$KEYS" -e "wait 400; fn up"
    expect_log "spotlight: off (Dictated)" 60
    expect_log "checklist: try_in_app done"
    log "Close the unsent Mail message without saving."
    ;;
  *) echo "usage: $0 panel|through|fullscreen|esc|own|mail"; exit 2 ;;
esac
