#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-15a.1 (docs/fork/MAC-TEST-SCRIPT.md): N takes through the
# running test app's real record path (SIGUSR2 = Handy's toggle), synthetic
# speech from `say -o` (no personal recordings) that the app plays as its
# test microphone, with the Hub window open. Start the app with
# `scripts/wb-s05/app.sh start` (shortcuts unbound, model cache cloned,
# WISPRBRO_TEST_MIC_WAV=<Data>/test-mic.wav), then `./wispr-bro` once more to
# show the Hub. Never the speakers unless WB_ALLOW_SPEAKERS=1. After each take: a screenshot of the app's window
# and the daily_stats totals, to compare. Each take holds ~/wb-bench/gui.lock.
#
# Usage: scripts/wb-s15a-takes.sh <app pid> <Data dir> <out dir> [N=10]
set -u
. "$(cd "$(dirname "$0")" && pwd)/wb-lib/app.sh"
PID="$1"; DATA="$2"; OUT="$3"; N="${4:-10}"
wb_test_mic_env "$DATA" || exit 1
LOCK="$HOME/wb-bench/gui.lock"
mkdir -p "$OUT/audio"
WINID="$OUT/wb-window-id"
[ -x "$WINID" ] || swiftc -O "$(cd "$(dirname "$0")" && pwd)/wb-window-id.swift" -o "$WINID" || exit 1
SENT=(
  "Please book a table for four at seven thirty on Friday evening."
  "The new printer on the second floor is finally working again."
  "Let's move the weekly sync to Tuesday morning at nine."
  "Remember to bring the charger and the blue folder to the office."
  "I finished the first draft and shared it with the whole team."
  "Could you check whether the train leaves at eight or at half past?"
  "The garden needs watering twice a week during the summer."
  "We should order more coffee filters before the end of the month."
  "Thanks for your help today, the demo went really well."
  "Send me the slides when you have a moment this afternoon."
)
for ((i = 0; i < N; i++)); do
  f="$OUT/audio/take-$i.aiff"
  [ -f "$f" ] || say -o "$f" "${SENT[$((i % 10))]}"
done

stats() {
  sqlite3 "$DATA/fork.db" "SELECT COALESCE(SUM(words),0), COALESCE(SUM(dictations),0), COALESCE(SUM(speech_ms),0), COALESCE(SUM(timed_words),0) FROM daily_stats"
}

take() {
  local i="$1"
  # No focus change: SIGUSR2 needs none, and `screencapture -l` grabs the
  # window's own content even when another app is in front.
  wb_mic_load "$OUT/audio/take-$i.aiff" || return 4
  kill -USR2 "$PID"; sleep 0.6
  wb_mic_play "$OUT/audio/take-$i.aiff"; sleep 0.4
  kill -USR2 "$PID"; sleep 4
  read -r wid x y w h < <("$WINID" "$PID")
  screencapture -x -o -l "$wid" "$OUT/take-$(printf %02d "$i").png"
  echo "take $i stats(words,dictations,speech_ms,timed)=$(stats)"
}

if [ "${S15A_TAKE:-}" = 1 ]; then take "$5"; exit $?; fi

echo "before stats=$(stats)"
for ((i = 0; i < N; i++)); do
  for try in 1 2 3; do
    S15A_TAKE=1 /usr/bin/lockf -k -t 600 "$LOCK" "$0" "$PID" "$DATA" "$OUT" "$N" "$i"
    rc=$?
    [ $rc = 0 ] && break
    [ $rc = 4 ] && exit 4
    sleep 3
  done
done
echo TAKES-END
