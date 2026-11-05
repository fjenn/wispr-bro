#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-24.1 / S-24.2 re-check (docs/fork/MAC-TEST-SCRIPT.md, S-24,
# "Fixed on fix/wave1-b"): the Scratchpad note takes focus from ⌥S, and a
# take started from the note keeps the app in front and lands in the note.
# Runs a portable test profile (never the owner's) whose hotkeys were set
# with `scratchpad.open = option+s` and fn/⌃⌘V unbound, so the owner's own
# Wispr Bro and this one never share a key. Takes are SIGUSR2 (Handy's
# toggle); speech is one pre-recorded WAV that the app plays as its test
# microphone: start it with WISPRBRO_TEST_MIC_WAV=<Data dir>/test-mic.wav
# (or export your own path for both); never the speakers unless
# WB_ALLOW_SPEAKERS=1.
# Keys go only to a TextEdit instance this script opens (`open -n`, guarded
# by its pid) or to the test app while it is in front (scripts/wb-s25/ui.swift).
# Holds ~/wb-bench/gui.lock.
#
# usage: recheck.sh <exe> <Data dir> <wav> <out dir>
#   step 1 (S-24.2): ⌥S with TextEdit in front → the test app is frontmost
#          within 1 s; typed keys land in the note, not in TextEdit
#   step 2 (S-24.1): a take started with the note in front → the app stays
#          frontmost at +0.2, +1, +3 s; `context: read own_app`, no
#          `focus_moved`; the note gets the text; the app stays up
#   step 3 (S-24.1): a take started in TextEdit, then the note brought to the
#          front, then the stop → the text lands in the note: the target is
#          the field focused at key-up (S-05 rule 2, "started in Slack and
#          released in Notes is fine"), so no `focus_moved` card. (Mac wave 2:
#          the MAC-TEST-SCRIPT step said "card"; that contradicts S-05.)
# Env: WBUI, WBKEYS = compiled scripts/wb-s25/ui.swift, scripts/wb-s01-keys.swift
set -u
EXE="$1"; DATA="$2"; WAV="$3"; OUT="$4"
HERE="$(cd "$(dirname "$0")" && pwd)"
. "$HERE/../wb-lib/app.sh"
wb_test_mic_env "$DATA" || exit 1
: "${WBUI:?}" "${WBKEYS:?}"
if [ "${WB_S24_LOCKED:-}" != 1 ]; then
  WB_S24_LOCKED=1 exec /usr/bin/lockf -k -t 900 "$HOME/wb-bench/gui.lock" /bin/bash "$0" "$@"
fi
mkdir -p "$OUT"
LOG="$DATA/logs/wispr-bro.log"
DB="$DATA/fork.db"
log() { echo "$(date +%H:%M:%S) $*" | tee -a "$OUT/run.log"; }
front_pid() { lsappinfo info -only pid "$(lsappinfo front)" | sed -E 's/[^0-9]//g'; }
app_pid() { pgrep -f "^$EXE" | head -1; }
notes() { sqlite3 "$DB" "SELECT id || '|' || replace(body, char(10), ' ') FROM notes WHERE deleted_at IS NULL ORDER BY updated_at DESC LIMIT 1"; }
mark() { wc -l <"$LOG" | tr -d ' '; }
since() { tail -n +"$(($1 + 1))" "$LOG"; }

pid=$(app_pid)
[ -n "$pid" ] || { log "FAIL no app running from $EXE"; exit 3; }
printf 'TextEdit control line\n' >"$OUT/control.txt"
te=$(wb_textedit_open "$OUT/control.txt") || { log "FAIL no TextEdit"; exit 3; }
sleep 2
log "app $pid, TextEdit $te"

# Step 1: ⌥S from TextEdit.
[ "$(front_pid)" = "$te" ] || { log "SKIP step 1: TextEdit not in front ($(front_pid))"; wb_textedit_close "$te"; exit 3; }
WB_FRONT=TextEdit "$WBKEYS" -e 'mods down option; key s; mods up option' >/dev/null
for i in $(seq 1 10); do [ "$(front_pid)" = "$pid" ] && break; sleep 0.1; done
if [ "$(front_pid)" = "$pid" ]; then log "PASS S-24.2 ⌥S: the test app is frontmost after $((i * 100)) ms"; else
  log "FAIL S-24.2 ⌥S: front pid $(front_pid) after 1 s"; fi
sleep 1
before=$(notes)
"$WBUI" "$pid" "type Zz; wait 900" >>"$OUT/run.log" 2>&1
sleep 1.5
after=$(notes)
log "note before: '$before' after: '$after'"
te_text=$(cat "$OUT/control.txt")
if [ "$after" != "$before" ] && [ "$te_text" = "TextEdit control line" ]; then
  log "PASS S-24.2 typed keys landed in the note"; else log "FAIL S-24.2 typed keys did not land in the note"; fi
screencapture -x "$OUT/step1.png"

# Step 2: a take from the note.
m=$(mark); before=$(notes)
if [ "$(front_pid)" != "$pid" ]; then log "SKIP step 2: the test app is not in front"; else
  wb_mic_load "$WAV" || exit 1
  kill -USR2 "$pid"; t0=$(date +%s)
  sleep 0.2; f02=$(front_pid)
  sleep 0.6; wb_mic_play "$WAV" & ap=$!
  sleep 0.2; f1=$(front_pid)
  sleep 2; f3=$(front_pid)
  wait "$ap"; sleep 0.6
  kill -USR2 "$pid"
  sleep 8
  after=$(notes)
  log "front at +0.2 $f02, +1 $f1, +3 $f3 (app $pid)"
  [ "$f02" = "$pid" ] && [ "$f1" = "$pid" ] && [ "$f3" = "$pid" ] && log "PASS S-24.1 the app kept focus" || log "FAIL S-24.1 focus moved"
  since "$m" | grep -E "context:|insert:|focus_moved|pill:|text:" | sed -E 's/^\[[^]]*\]\[[^]]*\]//' >"$OUT/step2.log"
  grep -q "own_app" "$OUT/step2.log" && log "PASS context: read own_app" || log "CHECK no own_app line"
  grep -q "focus_moved" "$OUT/step2.log" && log "FAIL focus_moved logged" || log "PASS no focus_moved"
  log "note before: '$before' after: '$after'"
  [ "$after" != "$before" ] && log "PASS S-24.1 the note got the text" || log "FAIL S-24.1 the note did not change"
  [ -n "$(app_pid)" ] && log "PASS the app is still up" || log "FAIL the app is gone"
  screencapture -x "$OUT/step2.png"
fi

# Step 3: a take started in TextEdit, then a click into the note.
pid=$(app_pid)
"$WBUI" "$te" "front" >/dev/null 2>&1 # activates our TextEdit instance only
sleep 1
if [ "$(front_pid)" != "$te" ]; then log "SKIP step 3: TextEdit not in front ($(front_pid))"; else
  m=$(mark); before=$(notes)
  wb_mic_load "$WAV" || exit 1
  kill -USR2 "$pid"; sleep 0.8
  wb_mic_play "$WAV"
  "$WBUI" "$pid" "front; wait 300" >>"$OUT/run.log" 2>&1
  sleep 0.5
  fstop=$(front_pid)
  kill -USR2 "$pid"
  # An unbundled build does not always come to the front on activate():
  # then the key-up target is still TextEdit and the step says nothing.
  [ "$fstop" = "$pid" ] || log "CHECK step 3: the test app was not in front at the stop (front $fstop)"
  sleep 8
  after=$(notes)
  since "$m" | grep -E "insert:|focus_moved|card" | sed -E 's/^\[[^]]*\]\[[^]]*\]//' >"$OUT/step3.log"
  grep -q "focus_moved" "$OUT/step3.log" && log "FAIL step 3 focus_moved (card)" || log "PASS step 3 no focus_moved card"
  [ "$after" != "$before" ] && log "PASS step 3 the text went to the note (key-up target)" || log "FAIL step 3 the note unchanged"
  screencapture -x "$OUT/step3.png"
fi
wb_textedit_close "$te"
grep -c "TextEdit control line" "$OUT/control.txt" >/dev/null && log "TextEdit file unchanged: $(cat "$OUT/control.txt")"
