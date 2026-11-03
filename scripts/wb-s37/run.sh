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
launch() {
  [ -n "$(pid)" ] || { "$EXE" >/dev/null 2>&1 & }
  for _ in $(seq 1 60); do [ -n "$(pid)" ] && break; sleep 0.5; done
  sleep 6
  open -a "$APP"; sleep 1
  log "launched pid $(pid)"
}
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
    drive "front; press-if Got it"
    for page in "Dictation" "Insights" "Dictionary" "Snippets" "Style" "Transforms" "Scratchpad" "Share with a friend"; do
      drive "press $page" || { log "FAIL cannot open $page"; FAILED=1; continue; }
      check "hub $page"
    done
    drive "press Notifications" && check "notifications panel" && drive "key escape"
    drive "press Help" && check "help popover" && drive "key escape"
    drive "press Settings" || { log "FAIL cannot open Settings"; exit 1; }
    for section in "General" "System" "Vibe coding" "Data and Privacy" "Advanced" "About"; do
      drive "press $section" || { log "FAIL cannot open $section"; FAILED=1; continue; }
      check "settings $section"
    done
    drive "key escape"
    drive "press Scratchpad; press New note" && check "scratchpad window"
    # The resting bar is a native panel: it is in every reading above;
    # this row set is the bar alone after the windows closed.
    check "resting bar"
    ;;
  onboarding)
    launch
    drive "front; press-if Got it"
    check "onboarding welcome"
    drive "press Continue" && check "onboarding second screen"
    ;;
  lessons)
    launch
    drive "front; press-if Got it; press Help"
    for lesson in "Learn Transforms" "Learn to dictate quietly"; do
      drive "press $lesson" || { log "FAIL cannot open $lesson"; FAILED=1; continue; }
      check "lesson $lesson"
      drive "key escape"
      drive "press Help"
    done
    drive "key escape"
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
