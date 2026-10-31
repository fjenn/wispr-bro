#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-11.1 / S-11.2 (docs/fork/MAC-TEST-SCRIPT.md), unattended:
# N rounds of six dictations through the running test app, each checked by
# check.py. The speech is macOS TTS (`say`, Samantha) with pauses where a
# speaker pauses around a command, played through the speakers.
#
#   round: TextEdit e-mail (3 paragraphs), Safari e-mail (WebKit editor),
#          TextEdit chat (2 lines), Chrome chat (typing path, Shift+Return),
#          TextEdit "a new line of shoes" + "literally new line",
#          TextEdit "question mark"
#
# usage: scripts/wb-s11/run.sh [rounds=3]
set -u
ROUNDS="${1:-3}"
HERE="$(cd "$(dirname "$0")" && pwd)"
WAVS="${TMPDIR:-/tmp}/wb-s11-wavs"
OUTS="${TMPDIR:-/tmp}/wb-s11-out"
mkdir -p "$WAVS" "$OUTS"

say_wav() { say -v Samantha --data-format=LEI16@16000 -o "$WAVS/$1.wav" "$2"; }
say_wav email "Hi Anna, [[slnc 700]] new paragraph. [[slnc 700]] Thanks for the notes on wispr bro. I will send the draft on Monday, [[slnc 500]] scratch that, [[slnc 500]] I will send it on Tuesday. [[slnc 700]] New paragraph. [[slnc 700]] Best, [[slnc 600]] new line. [[slnc 600]] Finn."
say_wav chat "Running late. [[slnc 700]] New line. [[slnc 700]] Start without me."
say_wav literal "We launched a new line of shoes. [[slnc 500]] Type literally new line, please."
say_wav question "Are you coming question mark"

(cd "$HERE" && exec python3 -I -m http.server 47811 --bind 127.0.0.1 >/dev/null 2>&1) &
server=$!
trap 'kill $server 2>/dev/null' EXIT
sleep 1

pass=0
fail=0
for ((r = 1; r <= ROUNDS; r++)); do
  for spec in email:textedit email:safari chat:textedit chat:chrome literal:textedit question:textedit; do
    case_="${spec%%:*}"
    target="${spec##*:}"
    out="$OUTS/$r-$case_-$target.txt"
    status=$("$HERE/take.sh" "$target" "$WAVS/$case_.wav" "$out")
    echo "round $r: $status"
    if python3 -I "$HERE/check.py" "$case_" "$target" "$out"; then
      pass=$((pass + 1))
    else
      fail=$((fail + 1))
    fi
  done
done
echo "S-11 RESULT: $pass pass, $fail fail"
