#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-17.1 (docs/fork/MAC-TEST-SCRIPT.md), unattended: N rounds of
# the three-line address snippet said alone and inside a sentence, into
# TextEdit (native text view; Mail's stand-in), a Safari WebKit
# contenteditable page (a mail editor's / Notes' stand-in) and a Chrome chat
# page (Enter sends, Shift+Enter is a new line; Slack's stand-in, S-04's
# typing path). Each take goes through the running test app's real record
# path with S-11's take.sh (SIGUSR2 → microphone → speech model → pipeline
# → delivery; it holds ~/wb-bench/gui.lock per take, checks the frontmost
# app and closes only what it opened). The clipboard is snapshotted before
# the run and put back after it. The app plays the `say -o` WAVs as its
# test microphone (started by `scripts/wb-s05/app.sh start`), never the
# speakers; repetition still belongs in the unit tests.
#
# usage: scripts/wb-s17/run.sh [rounds=1] [case:target ...]
#   (default: every case in every target)
set -u
ROUNDS="${1:-1}"
shift || true
SPECS="${*:-alone:textedit inline:textedit alone:safari inline:safari alone:chrome inline:chrome}"
HERE="$(cd "$(dirname "$0")" && pwd)"
S11="$HERE/../wb-s11"
WAVS="${TMPDIR:-/tmp}/wb-s17-wavs"
OUTS="${TMPDIR:-/tmp}/wb-s17-out"
mkdir -p "$WAVS" "$OUTS"

swift "$HERE/clip.swift" save "$OUTS/clipboard.plist"
restore() {
  swift "$HERE/clip.swift" restore "$OUTS/clipboard.plist"
  kill "$server" 2>/dev/null
}

say_wav() { say -v Samantha --data-format=LEI16@16000 -o "$WAVS/$1.wav" "$2"; }
say_wav alone "My address."
say_wav inline "Please send the parcel to my address by Friday."

(cd "$S11" && exec python3 -I -m http.server 47811 --bind 127.0.0.1 >/dev/null 2>&1) &
server=$!
trap restore EXIT
sleep 1

pass=0
fail=0
for ((r = 1; r <= ROUNDS; r++)); do
  for spec in $SPECS; do
    case_="${spec%%:*}"
    target="${spec##*:}"
    out="$OUTS/$r-$case_-$target.txt"
    status=$("$S11/take.sh" "$target" "$WAVS/$case_.wav" "$out")
    echo "round $r: $status"
    if python3 -I "$HERE/check.py" "$case_" "$target" "$out"; then
      pass=$((pass + 1))
    else
      fail=$((fail + 1))
    fi
  done
done
echo "S-17 RESULT: $pass pass, $fail fail"
