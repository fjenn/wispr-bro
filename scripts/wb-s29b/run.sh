#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-29b (docs/fork/MAC-TEST-SCRIPT.md, S-29b), for the orchestrator's
# consolidated UI wave. Drives the S-29b test bundle ("Wispr Bro S29b Check",
# com.wisprbro.s29bcheck, portable) through the practice take with real keys
# (scripts/wb-s01-keys.swift) and one pre-recorded `say` WAV that the app
# plays as its test microphone (WISPRBRO_TEST_MIC_WAV; never the speakers).
# Every GUI case holds
# ~/wb-bench/gui.lock. Screens that need a person (the typing test by hand,
# the celebration by eye) are listed in MAC-TEST-SCRIPT.
#
# usage: run.sh <case>   cases:
#   practice   at Practice with a ready speech model: fn+Space, the WAV, fn.
#              PASS = log "practice take Text", the newest entry_meta row has
#              source 'practice', the clipboard is unchanged, no paste
#              (log "went to the practice screen"); screenshot of the composer
#   queued     at Practice with an empty models folder and HF_ENDPOINT on the
#              local stand-in (scripts/wb-s07a/hf-mirror.py, slow rate): the
#              take shows "Setting up… N%" (log "practice take Queued"), then
#              either fills (log "practice take Text (late)") or the step
#              moves on after 90 s (log "step first_dictation") and the take
#              is in History
#   outside    practice mode on, but TextEdit in front: the take pastes into
#              TextEdit as usual (source 'dictation'), no practice event
#   timing     from the log: "step welcome" → "step first_dictation" and
#              → "onboarding: completed" (§5.3: first dictation ≤ 2 min)
#   login      after Start dictating with the box checked: the launch agent
#              exists (Handy's autostart), and is gone after unchecking it in
#              Settings › System
#   APP=<bundle> OUT=<dir> KEYS=<compiled wb-s01-keys> WAV=<say -o file>
#   [TAKE=signal: SIGUSR2 instead of fn, see hands_free_take]
set -u
: "${APP:?}" "${OUT:?}"
LOCK="$HOME/wb-bench/gui.lock"
HERE="$(cd "$(dirname "$0")" && pwd)"
. "$HERE/../wb-lib/app.sh"
EXE="$APP/Contents/MacOS/$(/usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "$APP/Contents/Info.plist")"
DATA="$APP/Contents/MacOS/Data"
LOGS="$DATA/logs"
mkdir -p "$OUT"
# Pre-recorded audio through the app's test microphone (never the speakers).
[ -n "${WAV:-}" ] && wb_test_mic "$OUT" "$WAV"
# Shortcuts unbound next to the owner's app; fn takes keep fn.
keys_needed() { [ "${TAKE:-}" = signal ] || echo dictate,hands_free; }

if [ "${WB_S29B_LOCKED:-}" != 1 ]; then
  WB_S29B_LOCKED=1 exec /usr/bin/lockf -k -t 1800 "$LOCK" /bin/bash "$0" "$@"
fi

pid() { pgrep -f "$EXE" | head -1; }
log() { echo "$(date +%H:%M:%S) $*" | tee -a "$OUT/run.log"; }
applog() { cat "$LOGS"/*.log 2>/dev/null; }
launch() {
  wb_unbind_keys "$DATA" "$(keys_needed)" >>"$OUT/run.log"
  "$EXE" >/dev/null 2>&1 &
  for _ in $(seq 1 60); do [ -n "$(pid)" ] && break; sleep 0.5; done
  sleep "${1:-6}"
  open -a "$APP"  # bring the onboarding window to the front
  sleep 1
  # `open -a` does not always activate an app started from its executable;
  # WBUI (compiled scripts/wb-s25/ui.swift) activates it by pid.
  [ -n "${WBUI:-}" ] && "$WBUI" "$(pid)" front >/dev/null 2>&1
  sleep 0.5
  log "launched pid $(pid)"
}
quit_app() {
  local p; p=$(pid)
  [ -n "$p" ] && kill "$p"
  for _ in $(seq 1 40); do [ -z "$(pid)" ] && break; sleep 0.5; done
  [ -n "$(pid)" ] && kill -9 "$(pid)"
  log "quit"
}
clip() { pbpaste | shasum -a 256 | cut -c1-16; }
newest_source() {
  sqlite3 "$DATA/fork.db" "SELECT source FROM entry_meta ORDER BY entry_id DESC LIMIT 1" 2>/dev/null
}
# fn+Space locks hands-free (S-01 rule 6); the WAV plays once; fn stops.
# TAKE=signal starts and stops the take with SIGUSR2 (Handy's toggle, the
# same record path) instead: a synthetic fn also reaches any other Wispr Bro
# that holds fn (the owner's own instance), and the practice routing depends
# only on which app is in front when the take starts.
front_pid() { lsappinfo info -only pid "$(lsappinfo front)" | sed -E 's/[^0-9]//g'; }
# $1 = the pid that must be in front when the take starts (the take pastes
# wherever focus is; never start one with someone else's window in front).
hands_free_take() {
  : "${WAV:?}"
  if [ -n "${1:-}" ] && [ "$(front_pid)" != "$1" ]; then
    log "SKIP take: pid $(front_pid) is in front, not $1"; return 1
  fi
  wb_mic_load "$WAV" || return 1
  if [ "${TAKE:-}" = signal ]; then
    kill -USR2 "$(pid)"; sleep 0.8
    wb_mic_play "$WAV"; sleep 0.6
    kill -USR2 "$(pid)"
    return
  fi
  : "${KEYS:?}"
  "$KEYS" -e 'fn down; wait 250; space down; wait 60; space up; wait 120; fn up; wait 300' >>"$OUT/keys.jsonl"
  wb_mic_play "$WAV"; sleep 0.6
  "$KEYS" -e 'fn down; wait 120; fn up' >>"$OUT/keys.jsonl"
}

case "${1:-}" in
practice)
  quit_app
  python3 -I "$HERE/profile.py" "$DATA" practice
  launch 10
  cb0=$(clip); mark=$(applog | wc -l)
  hands_free_take "$(pid)" || { quit_app; exit 1; }
  sleep 8
  later=$(applog | tail -n +"$((mark + 1))")
  echo "$later" | grep -q "practice take Text" && log "PASS composer filled" || log "FAIL no practice text"
  echo "$later" | grep -q "went to the practice screen" && log "PASS not pasted" || log "FAIL no practice delivery"
  [ "$(newest_source)" = practice ] && log "PASS source practice" || log "FAIL source $(newest_source)"
  [ "$(clip)" = "$cb0" ] && log "PASS clipboard unchanged" || log "FAIL clipboard changed"
  screencapture -x "$OUT/practice.png"
  quit_app
  ;;
queued)
  : "${HF_ENDPOINT:?run hf-mirror.py at a slow --rate and export HF_ENDPOINT}"
  quit_app
  python3 -I "$HERE/profile.py" "$DATA" practice
  rm -rf "$DATA/models"
  launch 6
  mark=$(applog | wc -l)
  hands_free_take "$(pid)" || { quit_app; exit 1; }
  sleep 3
  screencapture -x "$OUT/queued.png"
  for _ in $(seq 1 120); do
    later=$(applog | tail -n +"$((mark + 1))")
    echo "$later" | grep -q "practice take Text (late)\|step first_dictation" && break
    sleep 1
  done
  echo "$later" | grep -q "practice take Queued" && log "PASS queued shown" || log "FAIL not queued"
  if echo "$later" | grep -q "practice take Text (late)"; then
    log "PASS filled late"
  elif echo "$later" | grep -q "step first_dictation"; then
    log "PASS moved on after 90 s"
  else
    log "FAIL neither filled nor moved on in 120 s"
  fi
  screencapture -x "$OUT/queued-after.png"
  quit_app
  ;;
outside)
  quit_app
  python3 -I "$HERE/profile.py" "$DATA" practice
  launch 10
  # A TextEdit instance of our own: the take must paste into a document
  # this script made, never into a TextEdit window the owner has open.
  printf '' >"$OUT/outside.txt"
  te=$(wb_textedit_open "$OUT/outside.txt") || { log "FAIL no TextEdit"; exit 1; }
  sleep 2
  fp=$(lsappinfo info -only pid "$(lsappinfo front)" | sed -E 's/[^0-9]//g')
  [ "$fp" = "$te" ] || { log "SKIP outside: our TextEdit is not in front ($fp)"; wb_textedit_close "$te"; quit_app; exit 1; }
  mark=$(applog | wc -l)
  hands_free_take "$te"
  sleep 8
  wb_textedit_close "$te"
  later=$(applog | tail -n +"$((mark + 1))")
  echo "$later" | grep -q "practice take" && log "FAIL a practice event" || log "PASS no practice event"
  [ "$(newest_source)" = dictation ] && log "PASS source dictation" || log "FAIL source $(newest_source)"
  screencapture -x "$OUT/outside.png"
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
start, first, done = at("step welcome"), at("step first_dictation"), at("onboarding: completed")
if start and first:
    s = (first - start).total_seconds()
    print(f"welcome → first dictation: {s:.0f} s ({'PASS' if s <= 120 else 'FAIL'} ≤ 120 s)")
if start and done:
    s = (done - start).total_seconds()
    print(f"welcome → completed: {s:.0f} s ({'PASS' if s <= 300 else 'FAIL'} ≤ 5 min)")
PY
  ;;
login)
  # Handy's autostart registers an SMAppService login item on macOS 13+
  # (not a ~/Library/LaunchAgents plist; Mac wave 2): read the app's log.
  applog | grep -E "(Registered|Unregistered) login item" | tail -2 | tee -a "$OUT/run.log"
  applog | grep -E "login item" | tail -1 | grep -q "Registered login item" \
    && log "login item registered (uncheck Settings › System › Launch on startup to remove it)" \
    || log "no login item registered (or it was removed)"
  ;;
*)
  sed -n '2,37p' "$0"; exit 2 ;;
esac
