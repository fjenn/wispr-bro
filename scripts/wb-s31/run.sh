#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-31 (docs/fork/MAC-TEST-SCRIPT.md, S-31), for the orchestrator's
# consolidated UI wave. Drives the S-31 test bundle ("Wispr Bro S31 Check",
# com.wisprbro.s31check, portable, onboarding completed, a ready AI engine)
# through the Transforms lesson with scripts/wb-s31/lesson.swift (AX presses
# and the lesson's keys, posted only while the app is frontmost). No audio:
# the lesson has no take. Every GUI case holds ~/wb-bench/gui.lock.
#
# usage: run.sh <case>   cases:
#   off      Transforms off: Transforms › Start tutorial › Try it out ›
#            select all › ⌥1 (the web view's own keydown) › "Text
#            rewritten" › ⌥O shows the changes › ⌘Z restores the draft ›
#            ⌥1 again › Turn on Transforms. PASS = all steps, the whole run
#            under 120 s (exit criterion), transforms_enabled true in the
#            settings store, the clipboard unchanged, no "transforms: run"
#            line (nothing pasted)
#   on       Transforms on (after `off`): the same practice; ⌥1 is caught by
#            the fork tap and handed over (log "tutorials: transform.polish
#            went to the practice step"), no "transforms: run" line, Done
#   outside  practice step open, then TextEdit in front with a selected
#            sentence: ⌥1 runs a real Polish in TextEdit (log "transforms:
#            run … pasted"), no "went to the practice step" line
#   APP=<bundle> OUT=<dir> LESSON=<compiled lesson.swift> [WBUI=<compiled wb-s25/ui.swift>]
set -u
: "${APP:?}" "${OUT:?}" "${LESSON:?}"
LOCK="$HOME/wb-bench/gui.lock"
. "$(cd "$(dirname "$0")" && pwd)/../wb-lib/app.sh"
EXE="$APP/Contents/MacOS/$(/usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "$APP/Contents/Info.plist")"
DATA="$APP/Contents/MacOS/Data"
LOGS="$DATA/logs"
mkdir -p "$OUT"

if [ "${WB_S31_LOCKED:-}" != 1 ]; then
  WB_S31_LOCKED=1 exec /usr/bin/lockf -k -t 1800 "$LOCK" /bin/bash "$0" "$@"
fi

pid() { pgrep -f "$EXE" | head -1; }
log() { echo "$(date +%H:%M:%S) $*" | tee -a "$OUT/run.log"; }
applog() { cat "$LOGS"/*.log 2>/dev/null; }
drive() { "$LESSON" "$(pid)" "$1" | tee -a "$OUT/run.log"; return "${PIPESTATUS[0]}"; }
launch() {
  [ -n "$(pid)" ] || { "$EXE" >/dev/null 2>&1 & }
  for _ in $(seq 1 60); do [ -n "$(pid)" ] && break; sleep 0.5; done
  sleep 6
  open -a "$APP"; sleep 1
  log "launched pid $(pid)"
}
clip_hash() { pbpaste | shasum | cut -c1-12; }
practice() {
  drive "front; press-if Got it; press Transforms; press Start tutorial; wait-text Make your writing; press Try it out; wait-text Select the text" || return 1
  drive "select-all; key ${KEY:-opt-1}; wait-text Text rewritten; field" || return 1
}

case "${1:-}" in
  off)
    launch
    before=$(clip_hash); lines=$(applog | grep -c "transforms: run")
    start=$(date +%s)
    practice || { log "FAIL practice"; exit 1; }
    drive "key opt-o; wait-text changes; key cmd-z; wait 500; field" || log "FAIL View changes / undo"
    grep -q "field: hey so i think" "$OUT/run.log" && log "PASS ⌘Z restored the draft" || log "FAIL ⌘Z did not restore the draft"
    drive "select-all; key opt-1; wait-text Text rewritten; press Turn on Transforms; wait 1000" || log "FAIL Turn on"
    took=$(( $(date +%s) - start ))
    [ "$took" -lt 120 ] && log "PASS lesson end-to-end in ${took} s" || log "FAIL lesson took ${took} s"
    grep -rq '"transforms_enabled": *true' "$DATA" && log "PASS transforms_enabled" || log "CHECK transforms_enabled in the settings store"
    [ "$(clip_hash)" = "$before" ] && log "PASS clipboard unchanged" || log "FAIL clipboard changed"
    [ "$(applog | grep -c "transforms: run")" = "$lines" ] && log "PASS no real run" || log "FAIL a real run started"
    screencapture -x "$OUT/off.png"
    log "CHECK by eye in off.png: the bar spotlight, then the Transforms page with the switch on"
    ;;
  on)
    launch
    lines=$(applog | grep -c "transforms: run")
    practice || { log "FAIL practice"; exit 1; }
    applog | grep -q "tutorials: transform.polish went to the practice step" && log "PASS handed over by the tap" || log "FAIL no hand-over line"
    [ "$(applog | grep -c "transforms: run")" = "$lines" ] && log "PASS no real run" || log "FAIL a real run started"
    drive "press Done" || log "FAIL Done"
    ;;
  outside)
    launch
    drive "front; press-if Got it; press Transforms; press Start tutorial; press Try it out; wait-text Select the text" || exit 1
    handed=$(applog | grep -c "went to the practice step")
    # No trailing newline: Polish's line-count guard refuses a selection
    # that ends in an empty line.
    printf 'so basically the review moves to thursday i think' > "$OUT/outside.txt"
    # A TextEdit instance of our own (the owner's TextEdit, if it runs, has
    # the same name; keys are guarded by this instance's pid).
    te=$(wb_textedit_open "$OUT/outside.txt") || { log "FAIL no TextEdit"; exit 1; }
    sleep 2
    "${WBUI:-swift $(dirname "$0")/../wb-s25/ui.swift}" "$te" "front; key cmd-a" | tee -a "$OUT/run.log"
    "$LESSON" "$te" "key opt-1; wait 4000" | tee -a "$OUT/run.log"
    wb_textedit_close "$te"
    applog | grep -q "transforms: run .* pasted" && log "PASS a real Polish in TextEdit" || log "FAIL no real run in TextEdit"
    [ "$(applog | grep -c "went to the practice step")" = "$handed" ] && log "PASS not handed to the lesson" || log "FAIL handed to the lesson"
    ;;
  *)
    sed -n '3,30p' "$0"; exit 2 ;;
esac
