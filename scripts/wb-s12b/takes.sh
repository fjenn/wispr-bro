#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-12b.2: three real dictations while Apple's model stands in for
# the cleanup model (app started with WB_DEBUG_LLM_HOLD_MODEL=1). The WAVs
# are made once with `say -o`; each take goes through S-12a's take.sh (holds
# ~/wb-bench/gui.lock, checks the frontmost app, its own TextEdit document,
# key-up → text). The only check that needs the speakers: keep it to these
# three takes. Output volume is set to a moderate level and put back, and
# the clipboard is saved and restored around the run.
#
# usage: scripts/wb-s12b/takes.sh <out dir>
set -u
OUT="$1"
HERE="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$OUT/wavs"
say_wav() { [ -f "$OUT/wavs/$1.wav" ] || say -v "$2" --data-format=LEI16@16000 -o "$OUT/wavs/$1.wav" "$3"; }
say_wav en_fillers Samantha "Um, so I was thinking we could, uh, meet on Friday, [[slnc 300]] no wait, Thursday, to go over the budget."
say_wav en_question Samantha "What is the capital of Australia?"
say_wav nl_repair Xander "Eh, we spreken af om drie uur, nee wacht, om vier uur."

vol=$(osascript -e 'output volume of (get volume settings)')
muted=$(osascript -e 'output muted of (get volume settings)')
swift "$HERE/clip.swift" save "$OUT/clipboard.plist"
restore() {
  osascript -e "set volume output volume $vol" -e "set volume output muted $muted"
  swift "$HERE/clip.swift" restore "$OUT/clipboard.plist"
}
trap restore EXIT
osascript -e 'set volume output volume 40' -e 'set volume output muted false'

for c in en_fillers en_question nl_repair; do
  status=$("$HERE/../wb-s12a/take.sh" "$OUT/wavs/$c.wav" "$OUT/$c.txt")
  printf '%s\t%s\t%s\n' "$c" "$status" "$(tr '\n' '|' <"$OUT/$c.txt")"
done
