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
#   textedit   the same first dictation into a TextEdit document of the
#              driver's own instance (stand-in for Mail when the Mac's Mail
#              is a real account; "another app" is what the check needs)
#
# TextEdit runs as an instance of the driver's own (`open -n`), and every
# key or click is guarded by that instance's pid (WB_FRONT_PID), so nothing
# reaches a TextEdit the owner may have open. TAKE=signal starts and stops
# takes with SIGUSR2 instead of fn (fn also reaches any other Wispr Bro that
# holds it, such as the owner's own instance).
#   APP=<bundle> OUT=<dir> PILL=<compiled wb-s03-pill> KEYS=<compiled wb-s01-keys> WAV=<say -o file>
#   [WBWIN=<compiled wb-window-id> for through] [TAKE=signal]
set -u
: "${APP:?}" "${OUT:?}" "${PILL:?}"
LOCK="$HOME/wb-bench/gui.lock"
HERE="$(cd "$(dirname "$0")" && pwd)"
. "$HERE/../wb-lib/app.sh"
EXE="$APP/Contents/MacOS/$(/usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "$APP/Contents/Info.plist")"
DATA="$APP/Contents/MacOS/Data"
LOGS="$DATA/logs"
mkdir -p "$OUT"
# Pre-recorded audio through the app's test microphone (never the speakers).
[ -n "${WAV:-}" ] && wb_test_mic "$OUT" "$WAV"
# Shortcuts unbound next to the owner's app; fn takes keep fn, the `own`
# case binds ⌥S (pressed only with our TextEdit in front).
keys_needed() { [ "${TAKE:-}" = signal ] || echo dictate,hands_free; }
S30_SET=()

if [ "${WB_S30_LOCKED:-}" != 1 ]; then
  WB_S30_LOCKED=1 exec /usr/bin/lockf -k -t 1800 "$LOCK" /bin/bash "$0" "$@"
fi

pid() { pgrep -f "$EXE" | head -1; }
log() { echo "$(date +%H:%M:%S) $*" | tee -a "$OUT/run.log"; }
# The log since this case's launch (MARK lines in), so a line from an
# earlier case or launch never passes a check.
MARK=0
applog() { cat "$LOGS"/*.log 2>/dev/null | tail -n +"$((MARK + 1))"; }
pill() { WB_PID="$(pid)" "$PILL" -e "$1"; }
fpid() { lsappinfo info -only pid "$(lsappinfo front)" | sed -E 's/[^0-9]//g'; }
te_open() { printf '' >"$OUT/$1.txt"; TE=$(wb_textedit_open "$OUT/$1.txt") || { log "FAIL no TextEdit"; exit 1; }; sleep 2; }
te_close() { wb_textedit_close "${TE:-}"; }
# One take: fn (default) or SIGUSR2 (TAKE=signal); the WAV plays once.
take() {
  wb_mic_load "$WAV" || return 1
  if [ "${TAKE:-}" = signal ]; then
    kill -USR2 "$(pid)"; sleep 0.8; wb_mic_play "$WAV"; sleep 0.4; kill -USR2 "$(pid)"
  else
    "$KEYS" -e "fn down; wait 600" && wb_mic_play "$WAV" && "$KEYS" -e "wait 400; fn up"
  fi
}
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
  wb_unbind_keys "$DATA" "$(keys_needed)" ${S30_SET[@]+"${S30_SET[@]}"} | tee -a "$OUT/run.log"
  MARK=$(cat "$LOGS"/*.log 2>/dev/null | wc -l | tr -d ' ')
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
    te_open through
    # The document's window (scripts/wb-window-id.swift: id x y w h, points
    # from the top left); click inside it.
    set -- $("${WBWIN:?set WBWIN to the compiled scripts/wb-window-id.swift}" "$TE" | head -1)
    WB_FRONT_PID=$TE pill "click $(( $2 + $4 / 2 )) $(( $3 + $5 / 2 )); wait 300; key a; key s; key a; wait 300; front" | tee "$OUT/through.json"
    sleep 1
    [ "$(fpid)" = "$TE" ] && log "PASS our TextEdit stayed frontmost" || log "FAIL front changed ($(fpid))"
    v=$(osascript -e "tell application \"System Events\" to get value of text area 1 of scroll area 1 of window 1 of (first process whose unix id is $TE)" 2>/dev/null)
    [ "$v" = asa ] && log "PASS the keys reached TextEdit under the dim ('$v')" || log "FAIL TextEdit has '$v'"
    screencapture -x "$OUT/through.png"
    te_close
    ;;
  fullscreen)
    launch_spotlight note
    te_open fullscreen
    [ "$(fpid)" = "$TE" ] || { log "SKIP fullscreen: our TextEdit is not in front"; te_close; exit 1; }
    "$KEYS" -e "mods down ctrl,command; key f; mods up ctrl,command; wait 2500"
    pill "windows" | tee "$OUT/fullscreen-windows.json"
    grep -q '"layer" *: *24' "$OUT/fullscreen-windows.json" && log "PASS dim over the full-screen app" || log "FAIL no dim in full screen"
    screencapture -x "$OUT/fullscreen.png"
    [ "$(fpid)" = "$TE" ] && "$KEYS" -e "mods down ctrl,command; key f; mods up ctrl,command; wait 1500"
    te_close
    ;;
  esc)
    launch_spotlight email
    te_open esc
    WB_FRONT_PID=$TE pill "key escape; wait 800" >/dev/null
    te_close
    expect_log "spotlight: off (Esc)"
    expect_log "spotlight: panel destroyed"
    ;;
  own)
    : "${WAV:?}"
    S30_SET=(scratchpad.open=option+s)
    launch_spotlight message
    # The Scratchpad opens by its shortcut (the profile binds
    # scratchpad.open to option+s), pressed only while our TextEdit is in
    # front; the take then starts with the note in front.
    te_open own
    [ "$(fpid)" = "$TE" ] && WB_FRONT=TextEdit "$KEYS" -e "mods down option; key s; mods up option" >/dev/null
    sleep 1.5
    if [ "$(fpid)" = "$(pid)" ]; then take; sleep 6; else log "SKIP own: the Scratchpad is not in front ($(fpid))"; fi
    te_close
    applog | grep -q "spotlight: off (Dictated)" && log "FAIL the own-app take ended it" || log "PASS still on after the own-app take"
    ;;
  mail)
    : "${KEYS:?}" "${WAV:?}"
    launch_spotlight email
    open -a Mail; sleep 3
    WB_FRONT=Mail pill "cmd n; wait 1500" >/dev/null
    wb_mic_load "$WAV" && "$KEYS" -e "fn down; wait 600" && wb_mic_play "$WAV" && "$KEYS" -e "wait 400; fn up"
    expect_log "spotlight: off (Dictated)" 60
    expect_log "checklist: try_in_app done"
    log "Close the unsent Mail message without saving."
    ;;
  textedit)
    : "${WAV:?}"
    launch_spotlight email
    te_open textedit
    [ "$(fpid)" = "$TE" ] || { log "SKIP: our TextEdit is not in front"; te_close; exit 1; }
    take
    expect_log "spotlight: off (Dictated)" 60
    expect_log "checklist: try_in_app done"
    v=$(osascript -e "tell application \"System Events\" to get value of text area 1 of scroll area 1 of window 1 of (first process whose unix id is $TE)" 2>/dev/null)
    log "TextEdit text: '$v'"
    te_close
    ;;
  *) echo "usage: $0 panel|through|fullscreen|esc|own|mail|textedit"; exit 2 ;;
esac
