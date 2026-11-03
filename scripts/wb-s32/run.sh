#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-32 (docs/fork/MAC-TEST-SCRIPT.md, S-32), for the orchestrator's
# Mac wave: "a whispered take on the starter and the full model at 30 cm".
# No microphone, no speakers: a synthetic whispered WAV (`say -o`, the
# Whisper voice when installed, then scaled down by quiet_wav.py) goes
# through the app's own file-input path (`--transcribe-file`, the same
# batch transcription as a take, headless) on the S-32 test bundle ("Wispr
# Bro S32 Check", com.wisprbro.s32check, portable, both models installed).
# The text is graded with the lesson's own rule (wordmatch.py mirrors
# whisperMatch.ts): PASS = at least 70 % of the sentence's words.
# Headless runs open no window, so no gui.lock is needed.
#
# usage: run.sh <case>   cases:
#   wav       make $OUT/whisper-30cm.wav (and the loud reference
#             $OUT/whisper-ref.wav) from the lesson's English sentence
#   starter   transcribe it on the starter (STARTER, default the English
#             starter parakeet-tdt_ctc-110m) and grade it
#   full      the same on the full model (FULL, default
#             parakeet-unified-en-0.6b)
#   all       wav, starter, full; also the reference WAV on both models,
#             so a FAIL can be told apart from a model that is not installed
#   models    list the bundle's models (--list-models), to pick ids
#   APP=<bundle> OUT=<dir> [GAIN_DB=-24] [STARTER=<id>] [FULL=<id>]
set -u
: "${APP:?}" "${OUT:?}"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
EXE="$APP/Contents/MacOS/$(/usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "$APP/Contents/Info.plist")"
STARTER="${STARTER:-parakeet-tdt_ctc-110m}"
FULL="${FULL:-parakeet-unified-en-0.6b}"
GAIN_DB="${GAIN_DB:--24}"
WAV="$OUT/whisper-30cm.wav"
REF="$OUT/whisper-ref.wav"
mkdir -p "$OUT"

log() { echo "$(date +%H:%M:%S) $*" | tee -a "$OUT/run.log"; }
sentence() {
  python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1]))["lessons"]["whisper"]["practice"]["sentence"])' \
    "$ROOT/src/i18n/locales/en/fork.json"
}

make_wav() {
  local voice=Samantha raw="$OUT/whisper-say.wav"
  say -v '?' | grep -q '^Whisper ' && voice=Whisper
  say -v "$voice" --data-format=LEI16@16000 -o "$raw" "$(sentence)"
  python3 -I "$HERE/quiet_wav.py" "$raw" "$REF" 0 -90 | tee -a "$OUT/run.log"
  python3 -I "$HERE/quiet_wav.py" "$raw" "$WAV" "$GAIN_DB" | tee -a "$OUT/run.log"
  log "voice $voice, gain $GAIN_DB dB"
}

# grade <label> <model id> <wav>
grade() {
  local label="$1" model="$2" wav="$3" json text share
  json=$("$EXE" --transcribe-file "$wav" --model "$model" --json 2>>"$OUT/stderr.log")
  if [ -z "$json" ]; then
    log "FAIL $label ($model): no result (model not installed? see stderr.log)"
    return 1
  fi
  echo "$json" >>"$OUT/results.jsonl"
  text=$(python3 -I -c 'import json,sys; print(json.loads(sys.argv[1])["text"])' "$json")
  if share=$(python3 -I "$HERE/wordmatch.py" "$(sentence)" "$text"); then
    log "PASS $label ($model): $share of the words"
  else
    log "FAIL $label ($model): $share of the words (< 0.70)"
  fi
}

[ -f "$WAV" ] || [ "${1:-}" = wav ] || [ "${1:-}" = models ] || make_wav

case "${1:-}" in
  wav) make_wav ;;
  starter) grade "whisper 30 cm, starter" "$STARTER" "$WAV" ;;
  full) grade "whisper 30 cm, full" "$FULL" "$WAV" ;;
  all)
    make_wav
    grade "reference, starter" "$STARTER" "$REF"
    grade "reference, full" "$FULL" "$REF"
    grade "whisper 30 cm, starter" "$STARTER" "$WAV"
    grade "whisper 30 cm, full" "$FULL" "$WAV"
    ;;
  models) "$EXE" --list-models ;;
  *) sed -n '3,30p' "$0"; exit 2 ;;
esac
