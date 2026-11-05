#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-37 (docs/fork/MAC-TEST-SCRIPT.md, S-37.3), for the
# orchestrator's consolidated UI wave: the scripted AX-tree check. It opens
# each screen with scripts/wb-s31/lesson.swift (AX presses by accessible
# title, keys only while the app is frontmost) and reads every control's
# AXRole / AXTitle / AXDescription with scripts/wb-s37/axtree.swift, which
# fails on an unnamed control. No audio, nothing typed into other apps.
# Every case holds ~/wb-bench/gui.lock. VoiceOver by ear is the person's
# part (docs/fork/VOICEOVER-CHECKLIST.md, S-37.1/.2).
#
# usage: run.sh <case>   cases:
#   hub         every Hub page (sidebar buttons by title), the Help popover,
#               the notifications panel, every Settings section, the
#               Scratchpad window and the resting Flow Bar
#   onboarding  a bundle whose profile has not finished onboarding: the
#               welcome screen (and the next one when Continue is enabled)
#   lessons     Help › Essentials › each lesson's first screen
#   prefs       prints Increase Contrast / Reduce Motion as the Mac has them
#               (the person turns both on for S-37.2; nothing is changed here)
#   APP=<bundle> OUT=<dir> LESSON=<compiled wb-s31/lesson.swift>
#   AXTREE=<compiled wb-s37/axtree.swift>
# Results: $OUT/axtree.tsv (one row per control), $OUT/run.log with one
# PASS/FAIL line per screen; exit 1 when any screen has an unnamed control.
set -u
: "${APP:?}" "${OUT:?}" "${LESSON:?}" "${AXTREE:?}"
LOCK="$HOME/wb-bench/gui.lock"
EXE="$APP/Contents/MacOS/$(/usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "$APP/Contents/Info.plist")"
mkdir -p "$OUT"

if [ "${WB_S37_LOCKED:-}" != 1 ]; then
  WB_S37_LOCKED=1 exec /usr/bin/lockf -k -t 1800 "$LOCK" /bin/bash "$0" "$@"
fi

FAILED=0
pid() { pgrep -f "$EXE" | head -1; }
log() { echo "$(date +%H:%M:%S) $*" | tee -a "$OUT/run.log"; }
drive() { "$LESSON" "$(pid)" "$1" >>"$OUT/run.log" 2>&1; }
# A debug build writes `../src/bindings.ts` relative to its working
# directory at every start (Handy's specta export): started from the repo it
# wrote next to the checkout, and started by LaunchServices (`open -a`, cwd
# /) it panics on the read-only disk before it forwards anything (Mac wave
# 4). So the app runs from a folder of ours, and the Hub is opened by a
# second launch of the same executable (the single-instance plugin forwards
# it to the running app, which shows the Hub), retried while the first
# instance is not yet listening.
RUN_CWD="$OUT/cwd/run"
mkdir -p "$RUN_CWD"
# No take runs here, but the app prepares the microphone at launch: give it
# the test microphone (a second of silence) so the real one is never opened.
if [ -z "${WISPRBRO_TEST_MIC_WAV:-}" ]; then
  export WISPRBRO_TEST_MIC_WAV="$OUT/cwd/silence.wav"
  [ -s "$WISPRBRO_TEST_MIC_WAV" ] || python3 -I -c 'import sys,wave; w=wave.open(sys.argv[1],"wb"); w.setnchannels(1); w.setsampwidth(2); w.setframerate(16000); w.writeframes(b"\0\0"*16000); w.close()' "$WISPRBRO_TEST_MIC_WAV"
fi
launch() {
  [ -n "$(pid)" ] || { (cd "$RUN_CWD" && exec "$EXE") >/dev/null 2>&1 & }
  for _ in $(seq 1 60); do [ -n "$(pid)" ] && break; sleep 0.5; done
  sleep 8
  # The Hub's window (built on demand, S-38 parking) can take several
  # seconds to appear and load; reading it earlier finds nothing to press
  # (Mac wave 3).
  for _ in 1 2 3; do
    (cd "$RUN_CWD" && exec "$EXE") >/dev/null 2>&1 &
    for _ in $(seq 1 40); do "$LESSON" "$(pid)" windows 2>/dev/null | grep -q "window 'Wispr Bro'" && break 2; sleep 0.5; done
  done
  sleep 5
  log "launched pid $(pid)"
}
# First-run modals hide the page from AX: the first-use chooser on a
# profile's first Hub open ("Maybe later"; Mac wave 4: the wave 3 profile had
# answered it), then the one-time notifications popup; What's New no longer
# opens on a new install (S-28.9), its `Got it` stays harmless.
dismiss() { drive "front; press-if Maybe later; wait 1500; press-if Got it; wait 1500; press-if Got it"; }
check() {
  # $1 = screen label. Reads the whole app (every window and panel).
  sleep 0.8
  if "$AXTREE" "$(pid)" "$1" "$OUT/axtree.tsv" 2>>"$OUT/unnamed.txt" | tee -a "$OUT/run.log" | grep -q " 0 unnamed"; then
    log "PASS $1"
  else
    log "FAIL $1 (see unnamed.txt)"
    FAILED=1
  fi
}
[ -f "$OUT/axtree.tsv" ] || printf 'label\tverdict\trole\tsubrole\ttitle\tdescription\thelp\tvalue\ttitleUI\tenabled\twindow\n' >"$OUT/axtree.tsv"

case "${1:-}" in
  hub)
    launch
    dismiss
    for page in "Dictation" "Insights" "Dictionary" "Snippets" "Style" "Transforms" "Scratchpad" "Share with a friend"; do
      drive "press $page" || { log "FAIL cannot open $page"; FAILED=1; continue; }
      check "hub $page"
    done
    # Help and Notifications are Radix menu/popover triggers (AXPopUpButton):
    # `open` focuses them and posts Space (AXPress alone opens nothing).
    drive "front; open Notifications" && check "notifications panel" && drive "front; key escape"
    drive "front; open Help" && check "help popover" && drive "front; key escape"
    drive "press Settings" || { log "FAIL cannot open Settings"; exit 1; }
    for section in "General" "System" "Vibe coding" "Data and Privacy" "Advanced" "About"; do
      drive "press $section" || { log "FAIL cannot open $section"; FAILED=1; continue; }
      check "settings $section"
    done
    drive "front; key escape"
    drive "press Scratchpad; press New note" && check "scratchpad window"
    # The resting bar is a native panel: it is in every reading above;
    # this row set is the bar alone after the windows closed.
    check "resting bar"
    ;;
  onboarding)
    launch
    dismiss
    check "onboarding welcome"
    # Welcome's button is "Get started"; later screens say Continue.
    drive "press-if Continue; press-if Get started" && check "onboarding second screen"
    ;;
  lessons)
    launch
    dismiss
    for lesson in "Learn Transforms" "Learn to dictate quietly"; do
      # Help › Essentials items are Radix menu items: `item` focuses one
      # and posts Return (AXPress does not select it).
      drive "front; open Help; item $lesson" || { log "FAIL cannot open $lesson"; FAILED=1; continue; }
      check "lesson $lesson"
      drive "front; key escape"
    done
    ;;
  prefs)
    contrast=$(defaults read com.apple.universalaccess increaseContrast 2>/dev/null || echo 0)
    motion=$(defaults read com.apple.universalaccess reduceMotion 2>/dev/null || echo 0)
    log "Increase Contrast: $contrast, Reduce Motion: $motion (S-37.2 wants 1 and 1, set by the person in System Settings › Accessibility › Display)"
    ;;
  *)
    sed -n '3,27p' "$0"; exit 2 ;;
esac
exit "$FAILED"
