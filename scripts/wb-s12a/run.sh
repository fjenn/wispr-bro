#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-12a (docs/fork/MAC-TEST-SCRIPT.md), unattended: dictations
# through the running test app (portable profile, test identifier) into new
# TextEdit documents, with macOS TTS written to WAVs (`say -o`) that the app
# plays as its test microphone (never the speakers). Uses S-11's take.sh (holds ~/wb-bench/gui.lock, checks the frontmost
# app, closes only what it opened). The app must be running already, started
# by `scripts/wb-s05/app.sh start` (see the Mac test script for the phase).
#
# usage: scripts/wb-s12a/run.sh <light|timeout|none> [rounds=1]
#   light    en/nl/fr fillers and lead-ins, a self-repair, a question that
#            must not be answered, a dictionary spelling the LLM must keep
#   timeout  the app was started with WB_DEBUG_CLEANUP_DEADLINE_MS=1:
#            the take goes in as spoken ("Inserted without cleanup")
#   none     level None: no LLM, Handy's filler removal off
# Each take prints a TSV line: phase, case, wall time of the second toggle
# (s since epoch, ms), result text (escaped).
set -u
PHASE="$1"
ROUNDS="${2:-1}"
HERE="$(cd "$(dirname "$0")" && pwd)"
TAKE="$HERE/take.sh"
WAVS="${TMPDIR:-/tmp}/wb-s12a-wavs"
OUTS="${TMPDIR:-/tmp}/wb-s12a-out"
mkdir -p "$WAVS" "$OUTS"

say_wav() { say -v "$2" --data-format=LEI16@16000 -o "$WAVS/$1.wav" "$3"; }
say_wav en_fillers Samantha "Um, so I was thinking we could, uh, meet on Friday, [[slnc 300]] no wait, Thursday, to go over the budget."
say_wav en_question Samantha "What is the capital of Australia?"
say_wav en_spelling Samantha "So, uh, I really like using whisper bro every day."
say_wav nl_leadin Xander "Nou, dus ik heb de offerte gisteren verstuurd."
say_wav nl_repair Xander "We spreken af om drie uur, nee wacht, om vier uur."
say_wav fr_leadin Thomas "Alors, du coup, j'ai envoyé le devis hier."
say_wav fr_repair Thomas "Euh, on se voit mardi, non attends, mercredi."

case "$PHASE" in
  light) cases="en_fillers en_question en_spelling nl_leadin nl_repair fr_leadin fr_repair" ;;
  timeout | none) cases="en_fillers fr_leadin" ;;
  *) echo "unknown phase $PHASE" >&2; exit 2 ;;
esac

for ((r = 1; r <= ROUNDS; r++)); do
  for c in $cases; do
    out="$OUTS/$PHASE-$r-$c.txt"
    status=$("$TAKE" "$WAVS/$c.wav" "$out")
    # take.sh sends the second toggle ~0.6 s after the audio ends; its own
    # timestamps are not printed, so the log's stop line is the reference.
    printf '%s\t%s\t%s\t%s\n' "$PHASE" "$c" "$status" "$(tr '\n' '|' <"$out")"
  done
done
