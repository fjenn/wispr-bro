#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test OT2-D (docs/fork/MAC-TEST-SCRIPT.md, "Owner test 2: transforms do
# what they say"), for the orchestrator's transform Mac wave. Drives a
# portable test bundle (onboarding completed, Transforms on, ⌥1/⌥2 bound, a
# ready AI engine) in its own TextEdit instance. The transforms are set in
# the bundle's own Data/fork.db (never the owner's): Polish's prompt becomes
# the case's instruction, so the default ⌥1/⌥2 run it. No audio. Holds
# ~/wb-bench/gui.lock.
#
# usage: run.sh <case>   cases:
#   exact     Polish = "Replace all A and E with 0 and 1"; ⌥1 on "Please
#             send the agenda before the meeting." PASS = the saved file is
#             "Pl10s1 s1nd th1 0g1nd0 b1for1 th1 m11ting.", the log line has
#             "llm 0" (no model), clipboard unchanged, one ⌘Z restores
#   nomatch   same Polish on "Buy hot rum now." PASS = no "pasted" line, log
#             "Unchanged", the file unchanged; CHECK the bar said "Nothing
#             to change" (nomatch.png)
#   notapplied Prompt Engineer = "Make it about half as long" (instruction
#             guards, the model echoes); ⌥2 on a short sentence. PASS = no
#             paste, log "Unchanged" or "Guard"; CHECK the bar said
#             "Couldn't apply this transform" (notapplied.png) or, when the
#             model did shorten it, a paste (then PASS as a real rewrite)
#   restore   Transforms › Reset to defaults equivalent: the built-ins'
#             prompts back to their seeds
#   APP=<bundle> OUT=<dir> LESSON=<compiled wb-s31/lesson.swift> [WBUI=<compiled wb-s25/ui.swift>]
#   WB_OT2D_MOD=ctrl-opt: the transforms are bound to ⌃⌥1/⌃⌥2 in the test
#   profile and the driver presses those (a Wispr Bro of the owner's with
#   Transforms on also answers ⌥1/⌥2, in any app; Mac wave 7)
set -u
: "${APP:?}" "${OUT:?}" "${LESSON:?}"
LOCK="$HOME/wb-bench/gui.lock"
. "$(cd "$(dirname "$0")" && pwd)/../wb-lib/app.sh"
EXE="$APP/Contents/MacOS/$(/usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "$APP/Contents/Info.plist")"
DATA="$APP/Contents/MacOS/Data"
DB="$DATA/fork.db"
LOGS="$DATA/logs"
mkdir -p "$OUT"
case "${WB_OT2D_MOD:-opt}" in ctrl-opt) MOD=ctrl-opt- ;; *) MOD=opt- ;; esac

if [ "${WB_OT2D_LOCKED:-}" != 1 ]; then
  WB_OT2D_LOCKED=1 exec /usr/bin/lockf -k -t 1800 "$LOCK" /bin/bash "$0" "$@"
fi
case "$DB" in "$HOME/Library/"*) echo "refusing the real app data dir"; exit 2 ;; esac

pid() { pgrep -f "$EXE" | head -1; }
log() { echo "$(date +%H:%M:%S) $*" | tee -a "$OUT/run.log"; }
applog() { cat "$LOGS"/*.log 2>/dev/null; }
clip_hash() { pbpaste | shasum | cut -c1-12; }
quit_app() { [ -n "$(pid)" ] && kill "$(pid)"; for _ in $(seq 1 20); do [ -z "$(pid)" ] && break; sleep 0.5; done; }
launch() {
  [ -n "$(pid)" ] || { "$EXE" >/dev/null 2>&1 & }
  for _ in $(seq 1 60); do [ -n "$(pid)" ] && break; sleep 0.5; done
  sleep 8
  log "launched pid $(pid)"
}
set_prompt() { # <builtin_key> <prompt>
  quit_app
  sqlite3 "$DB" "UPDATE transforms SET prompt = '$(printf %s "$2" | sed "s/'/''/g")', rules_json = NULL WHERE builtin_key = '$1';"
}
ui() { "${WBUI:-swift $(dirname "$0")/../wb-s25/ui.swift}" "$@" | tee -a "$OUT/run.log"; }
# Runs <chord> on <text> in our own TextEdit; leaves the saved text in $OUT/<case>.txt.
take() { # <case> <text> <chord>
  printf '%s' "$2" > "$OUT/$1.txt"
  te=$(wb_textedit_open "$OUT/$1.txt") || { log "FAIL no TextEdit"; exit 1; }
  sleep 2
  ui "$te" "front; key cmd-a"
  "$LESSON" "$te" "key $3; wait 5000" | tee -a "$OUT/run.log"
  screencapture -x "$OUT/$1.png"
  ui "$te" "key cmd-s; wait 800"
}

case "${1:-}" in
  exact)
    set_prompt polish "Replace all A and E with 0 and 1"; launch
    before=$(clip_hash); lines=$(applog | grep -c "pasted in")
    take exact "Please send the agenda before the meeting." "${MOD}1"
    [ "$(cat "$OUT/exact.txt")" = "Pl10s1 s1nd th1 0g1nd0 b1for1 th1 m11ting." ] \
      && log "PASS exact text" || log "FAIL text: $(cat "$OUT/exact.txt")"
    applog | grep "pasted in" | tail -1 | grep -q "llm 0" && log "PASS no model (llm 0)" || log "FAIL the model ran or nothing pasted"
    [ "$(applog | grep -c "pasted in")" -gt "$lines" ] || log "FAIL no paste"
    [ "$(clip_hash)" = "$before" ] && log "PASS clipboard unchanged" || log "FAIL clipboard changed"
    ui "$te" "key cmd-z; wait 500; key cmd-s; wait 800"
    wb_textedit_close "$te"
    [ "$(cat "$OUT/exact.txt")" = "Please send the agenda before the meeting." ] \
      && log "PASS one ⌘Z restores" || log "FAIL after ⌘Z: $(cat "$OUT/exact.txt")"
    ;;
  nomatch)
    set_prompt polish "Replace all A and E with 0 and 1"; launch
    lines=$(applog | grep -c "pasted in")
    take nomatch "Buy hot rum now." "${MOD}1"
    wb_textedit_close "$te"
    [ "$(applog | grep -c "pasted in")" = "$lines" ] && log "PASS nothing pasted" || log "FAIL pasted"
    applog | grep "without a replacement" | tail -1 | grep -q Unchanged && log "PASS Unchanged" || log "FAIL no Unchanged line"
    [ "$(cat "$OUT/nomatch.txt")" = "Buy hot rum now." ] && log "PASS text kept" || log "FAIL text: $(cat "$OUT/nomatch.txt")"
    log "CHECK by eye in nomatch.png: the bar says \"Nothing to change\""
    ;;
  notapplied)
    set_prompt prompt_engineer "Make it about half as long"; launch
    lines=$(applog | grep -c "pasted in")
    take notapplied "Thank you so much for taking the time to meet with us yesterday, we really enjoyed it." "${MOD}2"
    wb_textedit_close "$te"
    if [ "$(applog | grep -c "pasted in")" = "$lines" ]; then
      applog | grep "without a replacement" | tail -1 | grep -Eq "Unchanged|Guard" && log "PASS refused, nothing pasted" || log "FAIL no reason line"
      log "CHECK by eye in notapplied.png: \"Couldn't apply this transform\" (Unchanged) or \"Couldn't transform that text\" (Guard)"
    else
      log "PASS the model shortened it: $(cat "$OUT/notapplied.txt")"
    fi
    ;;
  restore)
    quit_app
    sqlite3 "$DB" "UPDATE transforms SET prompt = 'Polish this text. Fix grammar, spelling, punctuation and awkward wording while preserving its meaning, tone, language, formatting and approximate length. Return the same text when no change is needed.' WHERE builtin_key = 'polish';
UPDATE transforms SET prompt = 'Turn this text into a clear, effective prompt for an AI assistant. Preserve every requirement, constraint, fact and requested output format. Remove ambiguity and unnecessary wording. Do not answer or carry out the prompt.' WHERE builtin_key = 'prompt_engineer';"
    log "restored the built-in prompts"
    ;;
  *)
    sed -n '3,32p' "$0"; exit 2 ;;
esac
