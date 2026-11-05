#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# OT2-B (quiet speech and whispers): the speech sources of the quiet test
# set. macOS `say` writes each sentence to a 16 kHz mono 16-bit WAV (to a
# file, never the speakers); `refs.tsv` holds `<file>\t<text>` for scoring.
# Nothing personal and nothing from ~/wb-bench: the sentences are ours.
#
# The levels, the whispered variants and the café noise are made from these
# at test time by `src-tauri/src/fork/audio/quiet_set.rs` (the generated
# set would be ~25 MB, so it is not committed):
#
#   scripts/wb-quiet/make-speech.sh <dir>
#   WB_QUIET_SPEECH=<dir> cargo test --lib fork::audio::quiet_bench -- --ignored --nocapture
#
# `t*.wav` are the talker's sentences; `b*.wav` are other voices, mixed
# into the café noise as distant babble.
set -eu
DIR="${1:?usage: make-speech.sh <dir>}"
mkdir -p "$DIR"
REFS="$DIR/refs.tsv"
: > "$REFS"

say_to() { # <voice> <file> <text>
  [ -s "$DIR/$2" ] || say -v "$1" -o "$DIR/$2" --file-format=WAVE --data-format=LEI16@16000 "$3"
}

i=0
while IFS='|' read -r voice text; do
  i=$((i + 1))
  f=$(printf 't%02d.wav' "$i")
  say_to "$voice" "$f" "$text"
  printf '%s\t%s\n' "$f" "$text" >> "$REFS"
done <<'EOF'
Samantha|Please move the meeting with the design team to Thursday afternoon.
Daniel|I left the spare keys under the blue flower pot by the back door.
Karen|Can you send me the report before lunch so I can read it on the train?
Moira|The coffee here is good but the music is far too loud today.
Samantha|Remind me to call the dentist tomorrow morning before work.
Daniel|We need more chairs and a long table for the party on Saturday.
Karen|Thanks for the update, I will look at the numbers tonight and reply.
Moira|Turn left at the bakery and the library is the second building on the right.
EOF

j=0
while IFS='|' read -r voice text; do
  j=$((j + 1))
  say_to "$voice" "$(printf 'b%02d.wav' "$j")" "$text"
done <<'EOF'
Fred|So I told him we could probably finish the whole thing by next week if nobody gets sick.
Ralph|Did you see the game last night, it went to overtime and nobody left their seats.
Kathy|Honestly the best part of the trip was the little market near the harbour on Sunday.
Rishi|I think the new phone is nice but the battery does not last a full day for me.
Tessa|We should try that noodle place on the corner, my sister says it is really good.
Junior|Can I get a large latte with oat milk and one of those chocolate croissants please.
EOF
echo "make-speech: $i sentences and $j babble voices in $DIR"
