#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-29a (docs/fork/MAC-TEST-SCRIPT.md, S-29a), for the orchestrator's
# consolidated UI wave. Drives the S-29a test bundle ("Wispr Bro S29a Check",
# com.wisprbro.s29acheck, portable) through what can run without a person;
# the rest (TCC prompts on a clean account, clicks on the screens) is listed
# in MAC-TEST-SCRIPT. Every GUI case holds ~/wb-bench/gui.lock. No audio.
#
# usage: run.sh <case>   cases:
#   resume            relaunch at each step; the log must name no other step
#   keytest           at the shortcut test: synthetic fn press lights the key
#                     (log "key test down/up"), no take starts (no new WAV)
#   timing            prints the time from "step permissions" to "step
#                     shortcut_test" and to "completed" in the log (S-29a.1)
#   globe-state       prints AppleFnUsageType and whether the restore marker
#                     exists (before/after the DEC-44 consent and after
#                     switching to Ctrl+Option, which must restore it)
#   move              copies the bundle to ~/Downloads/wb-s29a/, launches it
#                     there; after "Move to Applications" is pressed, checks
#                     that it runs from /Applications or ~/Applications
#   APP=<bundle> OUT=<dir> KEYS=<compiled wb-s01-keys> must be set.
set -u
: "${APP:?}" "${OUT:?}"
LOCK="$HOME/wb-bench/gui.lock"
HERE="$(cd "$(dirname "$0")" && pwd)"
EXE="$APP/Contents/MacOS/$(/usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "$APP/Contents/Info.plist")"
DATA="$APP/Contents/MacOS/Data"
LOGS="$DATA/logs"
mkdir -p "$OUT"

if [ "${WB_S29A_LOCKED:-}" != 1 ]; then
  WB_S29A_LOCKED=1 exec /usr/bin/lockf -k -t 1800 "$LOCK" /bin/bash "$0" "$@"
fi

pid() { pgrep -f "$EXE" | head -1; }
log() { echo "$(date +%H:%M:%S) $*" | tee -a "$OUT/run.log"; }
applog() { cat "$LOGS"/*.log 2>/dev/null; }
launch() {
  "$EXE" >/dev/null 2>&1 &
  for _ in $(seq 1 60); do [ -n "$(pid)" ] && break; sleep 0.5; done
  sleep "${1:-6}"
  log "launched pid $(pid)"
}
quit_app() {
  local p; p=$(pid)
  [ -n "$p" ] && kill "$p"
  for _ in $(seq 1 40); do [ -z "$(pid)" ] && break; sleep 0.5; done
  [ -n "$(pid)" ] && kill -9 "$(pid)"
  log "quit"
}
wavs() { find "$DATA/recordings" -name '*.wav' 2>/dev/null | wc -l | tr -d ' '; }

case "${1:-}" in
resume)
  for step in welcome languages permissions mic_test shortcut_test; do
    quit_app
    python3 -I "$HERE/profile.py" "$DATA" "$step"
    mark=$(applog | wc -l)
    launch 6
    later=$(applog | tail -n +"$((mark + 1))")
    if echo "$later" | grep -q "onboarding: step " ; then
      log "FAIL resume $step: a step was written: $(echo "$later" | grep 'onboarding: step ' | tail -1)"
    else
      log "PASS resume $step (screenshot: $OUT/resume-$step.png)"
    fi
    # The app's window only (never the rest of the screen) when WBWIN names
    # the compiled scripts/wb-window-id.swift.
    if [ -n "${WBWIN:-}" ] && w=$("$WBWIN" "$(pid)" | cut -d' ' -f1) && [ -n "$w" ]; then
      screencapture -x -o -l "$w" "$OUT/resume-$step.png"
    else
      screencapture -x "$OUT/resume-$step.png"
    fi
  done
  quit_app
  ;;
keytest)
  : "${KEYS:?}"
  quit_app
  python3 -I "$HERE/profile.py" "$DATA" shortcut_test
  launch 8
  before=$(wavs); mark=$(applog | wc -l)
  "$KEYS" -e 'fn down; wait 600; fn up; wait 400' >"$OUT/keys.jsonl"
  sleep 2
  later=$(applog | tail -n +"$((mark + 1))")
  echo "$later" | grep -q "key test down" && echo "$later" | grep -q "key test up" \
    && log "PASS key lit (down/up)" || log "FAIL no key test events in the log"
  [ "$(wavs)" = "$before" ] && log "PASS no take recorded" || log "FAIL a take was recorded"
  echo "$later" | grep -q "take_started\|recording started" && log "FAIL a take started" || true
  quit_app
  ;;
timing)
  applog | grep -E "onboarding: (step|completed)" | tee "$OUT/timing.txt"
  python3 -I - "$OUT/timing.txt" <<'PY'
import re, sys, datetime
rows = [l for l in open(sys.argv[1])]
def at(pat):
    for l in rows:
        if pat in l:
            m = re.search(r"(\d{2}):(\d{2}):(\d{2})", l)
            if m:
                return datetime.timedelta(hours=int(m[1]), minutes=int(m[2]), seconds=int(m[3]))
start, test, done = at("step permissions"), at("step shortcut_test"), at("onboarding: completed")
if start and test:
    s = (test - start).total_seconds()
    print(f"permissions → shortcut test: {s:.0f} s ({'PASS' if s <= 120 else 'FAIL'} ≤ 120 s)")
if start and done:
    print(f"permissions → completed: {(done - start).total_seconds():.0f} s")
PY
  ;;
globe-state)
  v=$(defaults read com.apple.HIToolbox AppleFnUsageType 2>/dev/null || echo unset)
  m="$DATA/globe-preference.json"
  log "AppleFnUsageType=$v marker=$([ -f "$m" ] && cat "$m" || echo none)"
  ;;
move)
  quit_app
  dir="$HOME/Downloads/wb-s29a"; mkdir -p "$dir"
  name=$(basename "$APP")
  [ -e "/Applications/$name" ] || [ -e "$HOME/Applications/$name" ] \
    && { log "SKIP: $name already in an Applications folder; remove it first"; exit 1; }
  ditto "$APP" "$dir/$name"
  "$dir/$name/Contents/MacOS/$(basename "$EXE")" >/dev/null 2>&1 &
  log "launched from $dir; press Get started, then Move to Applications"
  for _ in $(seq 1 120); do
    p=$(pgrep -f "Applications/$name/Contents/MacOS/$(basename "$EXE")" | head -1)
    [ -n "$p" ] && break; sleep 1
  done
  [ -n "${p:-}" ] && log "PASS relaunched from $(ps -o command= -p "$p")" || log "FAIL no relaunch from Applications in 120 s"
  # Same volume: renamed (nothing left behind); another volume: copied.
  [ -e "$dir/$name" ] && log "original still in $dir (copied)" || log "original moved (renamed)"
  ;;
*)
  sed -n '2,25p' "$0"; exit 2 ;;
esac
