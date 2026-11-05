#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-08 (docs/fork/MAC-TEST-SCRIPT.md), unattended. Drives the S-08
# test bundle ("Wispr Bro S08 Check", com.wisprbro.s08check, portable) through
# its real record path: SIGUSR2 (Handy's toggle) → microphone → model →
# History (paste is off). Every case holds ~/wb-bench/gui.lock (it may
# change the default input) and puts the default input back.
#
# Pre-recorded audio, never the speakers: the app is launched with
# WISPRBRO_TEST_MIC_WAV=$OUT/test-mic.wav (`open --env`; the test identity
# honours it) and `take` loads a `say -o` WAV into it (wb_mic_load /
# wb_mic_play, scripts/wb-lib/app.sh). The cases about the real device
# need the real microphone, so they run on an app launched without it
# (`relaunch <mute> real`, written to $OUT/mic) and play nothing: revoke and
# denied. `loss` must cut real mic audio mid-take, which the test
# microphone (no device) cannot give: it refuses unless WB_ALLOW_SPEAKERS=1
# (one short WAV, WB_SPEAKER_VOLUME, default 0.6). Every launch starts from a
# profile with all global shortcuts unbound (takes are SIGUSR2) and the
# model cache cloned (scripts/wb-lib/profile.py).
#
# usage: run.sh <case> [args]   cases: take, loss, mute, premuted,
#   relaunch <mute 0|1> [real], revoke, denied
#   HAL=<wbhal binary> APP=<bundle> OUT=<dir> must be set.
set -u
: "${HAL:?}" "${APP:?}" "${OUT:?}"
. "$(cd "$(dirname "$0")" && pwd)/../wb-lib/app.sh"
LOCK="$HOME/wb-bench/gui.lock"
EXE="$APP/Contents/MacOS/$(/usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "$APP/Contents/Info.plist")"
DATA="$APP/Contents/MacOS/Data"
mkdir -p "$OUT"
MIC_WAV="$(cd "$OUT" && pwd)/test-mic.wav"

if [ "${WB_S08_LOCKED:-}" != 1 ]; then
  WB_S08_LOCKED=1 exec /usr/bin/lockf -k -t 1800 "$LOCK" /bin/bash "$0" "$@"
fi

pid() { pgrep -fx "$EXE --start-hidden" | head -1; }
log() { echo "$(date +%H:%M:%S.%N | cut -c1-12) $*" | tee -a "$OUT/run.log"; }
state() { "$HAL" state >"$OUT/$1.json"; log "state $1: $(cat "$OUT/$1.json")"; }
last_row() {
  sqlite3 "$DATA/history.db" \
    "SELECT id, file_name, transcription_text FROM transcription_history ORDER BY id DESC LIMIT 1"
}
# launch [wait s] [real]: `real` opens the real microphone (no test mic).
launch() {
  local env=()
  wb_unbind_keys "$DATA" >>"$OUT/run.log"
  [ -d "$DATA/huggingface" ] || wb_clone_models "$DATA" "${WB_MODEL_CACHE:-}" >>"$OUT/run.log"
  if [ "${2:-}" = real ]; then
    echo real >"$OUT/mic"
  else
    echo test >"$OUT/mic"
    env=(--env "WISPRBRO_TEST_MIC_WAV=$MIC_WAV")
  fi
  open ${env[@]+"${env[@]}"} -a "$APP" --args --start-hidden
  for _ in $(seq 1 60); do [ -n "$(pid)" ] && break; sleep 0.5; done
  sleep "${1:-12}" # model load + prepared mic
  log "launched pid $(pid) (microphone: $(cat "$OUT/mic"))"
}
mic_mode() { cat "$OUT/mic" 2>/dev/null || echo unknown; }
# The running app's microphone: the test one for `take`, the real one for
# the device cases.
need_mic() {
  [ "$(mic_mode)" = "$1" ] && return 0
  log "refused: this case needs the app on the $1 microphone (now $(mic_mode)); run.sh relaunch <mute>$([ "$1" = real ] && echo ' real')"
  exit 1
}
quit_app() {
  local p; p=$(pid)
  [ -n "$p" ] && osascript -e "tell application id \"com.wisprbro.s08check\" to quit" >/dev/null 2>&1
  for _ in $(seq 1 40); do [ -z "$(pid)" ] && break; sleep 0.5; done
  [ -n "$(pid)" ] && kill "$(pid)"
  log "quit"
}
toggle() { kill -USR2 "$(pid)"; log "toggle"; }
restore_input() {
  "$HAL" set-default-input BuiltInMicrophoneDevice && log "default input back on the built-in mic"
}

case "${1:-}" in
take) # one spoken take into History (a `say -o` WAV as the test microphone)
  wav="$OUT/take.wav"
  say -v Samantha --data-format=LEI16@16000 -o "$wav" "${2:-The quick brown fox jumps over the lazy dog.}"
  if [ "${WB_ALLOW_SPEAKERS:-}" != 1 ]; then need_mic test; export WISPRBRO_TEST_MIC_WAV="$MIC_WAV"; fi
  wb_mic_load "$wav" || exit 1
  toggle; sleep 0.8
  wb_mic_play "$wav"
  sleep 0.6; toggle; sleep 8
  log "row: $(last_row)"
  ;;
loss) # the take's mic (a public aggregate over the built-in mic) vanishes
  if [ "${WB_ALLOW_SPEAKERS:-}" != 1 ]; then
    log "refused: loss cuts real mic audio, which only the speakers give (the test microphone opens no device); WB_ALLOW_SPEAKERS=1 to run it"
    exit 1
  fi
  stop="$OUT/agg.stop"; rm -f "$stop"
  "$HAL" agg-hold "WB S08 Mic" wb-s08-agg BuiltInMicrophoneDevice "$stop" 300 >"$OUT/agg.log" 2>&1 &
  sleep 2
  "$HAL" set-default-input wb-s08-agg && log "default input = WB S08 Mic"
  quit_app; launch 14 real
  # The only speaker playback of the run (WB_ALLOW_SPEAKERS=1): one short
  # WAV (made once with `say -o`), because the loss must cut real mic audio.
  [ -f "$OUT/before.wav" ] || say -v Samantha -o "$OUT/before.wav" --data-format=LEI16@22050 "Before the cable came out I said this."
  toggle; sleep 0.8
  WB_SPEAKER_VOLUME="${WB_SPEAKER_VOLUME:-0.6}" wb_mic_play "$OUT/before.wav"
  sleep 0.3; touch "$stop"; log "aggregate destroyed"
  sleep 3; toggle; sleep 10
  log "row: $(last_row)"
  cat "$OUT/agg.log" | tee -a "$OUT/run.log"
  restore_input
  sleep 4
  # The next take (silent) opens the built-in mic again.
  toggle; sleep 3; toggle; sleep 6
  log "next take row: $(last_row)"
  ;;
mute) # mute while dictating restores the output exactly
  state mute-before
  toggle; sleep 2.5
  state mute-during
  pgrep -fl osascript | tee "$OUT/osascript-during.txt"
  sleep 1; toggle; sleep 6
  state mute-after
  ;;
premuted) # the user had muted the output: it stays muted, nothing else changes
  osascript -e 'set volume output muted true'
  state premuted-before
  toggle; sleep 2.5; state premuted-during
  sleep 1; toggle; sleep 6
  state premuted-after
  osascript -e 'set volume output muted false'
  state premuted-restored
  ;;
relaunch) # quit, apply setup.py with mute=$2, launch (test mic, or `real`)
  quit_app
  /usr/bin/python3 -I "$(dirname "$0")/setup.py" "$DATA" s08-parakeet-v3 "${2:-0}" </dev/null | tee -a "$OUT/run.log"
  launch 15 "${3:-}"
  ;;
revoke) # Microphone revoked mid-take, then Quit & Reopen
  # No speech: the saved take is checked by its length, not its words.
  need_mic real
  toggle; sleep 3
  tccutil reset Microphone com.wisprbro.s08check | tee -a "$OUT/run.log"
  log "revoked"
  sleep 3
  old=$(pid)
  if "$HAL" press UserNotificationCenter "Quit & Reopen" 15 | tee -a "$OUT/run.log" | grep -q '"pressed":true'; then
    log "pressed Quit & Reopen"
  else
    # macOS 26.6.2 showed no dialog after `tccutil reset` in this run (RS-11
    # saw one): send the same quit request the button sends (a quit Apple
    # event), then reopen as it would.
    log "no dialog; sending the quit Apple event"
    osascript -e 'tell application id "com.wisprbro.s08check" to quit' >/dev/null 2>&1
    for _ in $(seq 1 60); do [ -z "$(pid)" ] && break; sleep 0.5; done
    open -a "$APP" --args --start-hidden # the real mic, as before the revoke
  fi
  for _ in $(seq 1 60); do [ "$(pid)" != "$old" ] && break; sleep 0.5; done
  log "quit seen (old pid $old)"
  for _ in $(seq 1 120); do [ -n "$(pid)" ] && break; sleep 0.5; done
  log "relaunched pid $(pid)"
  sleep 40
  log "recovered dir: $(ls "$DATA/recovered" 2>&1)"
  log "row: $(last_row)"
  ;;
denied) # after a revoke: a take answered Don't Allow, then a relaunch and a take
  # In the same process the open still succeeds and delivers silence (the
  # in-process status is stale, RS-11): "Only silence came in".
  need_mic real
  toggle
  "$HAL" press UserNotificationCenter "Don’t Allow" 20 | tee -a "$OUT/run.log"
  sleep 4; toggle; sleep 8
  # After a relaunch the open fails with PermissionDenied: "Microphone access
  # is off" with Fix.
  quit_app; launch 20 real
  toggle; sleep 1.5
  # Only the bar's strip (bottom centre), never the whole screen.
  b=$(osascript -e 'tell application "Finder" to get bounds of window of desktop')
  w=$(echo "$b" | cut -d, -f3 | tr -d ' '); h=$(echo "$b" | cut -d, -f4 | tr -d ' ')
  screencapture -x -R "$((w / 2 - 230)),$((h - 62)),460,50" "$OUT/denied-relaunch.png"
  log "screenshot denied-relaunch.png"
  sleep 6
  ;;
*)
  echo "usage: run.sh take|loss|mute|premuted|relaunch <mute> [real]|revoke|denied" >&2
  exit 2
  ;;
esac
