#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-19a (docs/fork/MAC-TEST-SCRIPT.md), unattended: per phase, the
# test app (portable profile, test identifier, src-tauri/target/debug) is
# stopped, the profile gets the phase's styles and app rules
# (scripts/wb-s19a/setup.py), the app is started again, and dictations go
# into new TextEdit documents through the real record path (S-12a's
# take.sh: holds ~/wb-bench/gui.lock per take, checks the frontmost app,
# closes only what it opened). The audio is macOS TTS written to WAVs
# (`say -o`) that the app plays as its test microphone
# (WISPRBRO_TEST_MIC_WAV=Data/test-mic.wav, set at each start; never the
# speakers unless WB_ALLOW_SPEAKERS=1). Each start unbinds the profile's
# global shortcuts (takes are SIGUSR2) and clones the model cache when
# Data/huggingface is missing (scripts/wb-lib/profile.py).
#
# TextEdit stands in for the apps of each group: a rule puts it in the
# group under test, so the whole chain runs for real (frontmost app → the
# user's rule → the group's preset → cleanup with the preset's prompt →
# stage 10 → paste → Return after a confirmed paste).
#
# usage: scripts/wb-s19a/run.sh <phase> [<phase> ...]
#   personal  TextEdit in Personal, Very relaxed: an English and a Dutch take
#   work      TextEdit in Work, Upbeat
#   email     TextEdit in Email, Formal
#   other     no rule (TextEdit is Other), Other Relaxed
#   autosend  TextEdit in Work (Formal) with "Press Enter after dictating"
#   fallback  the same, app started with WB_DEBUG_CLEANUP_DEADLINE_MS=1:
#             inserted without cleanup, so no Return
# Each take prints: phase, case, take.sh status, the document text (\n as |).
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
APP_DIR="$ROOT/src-tauri/target/debug"
. "$HERE/../wb-lib/app.sh"
wb_test_mic_env "$APP_DIR/Data" || exit 1
TAKE="$ROOT/scripts/wb-s12a/take.sh"
SCRATCH="${WB_S19A_SCRATCH:-${TMPDIR:-/tmp}/wb-s19a}"
WAVS="$SCRATCH/wavs"
OUTS="$SCRATCH/out"
LOGS="$SCRATCH/logs"
MODEL_ID="${WB_S19A_MODEL_ID:-s19a-parakeet-v3}"
GGUF="${WB_S19A_GGUF:-$HOME/wb-bench/llm/qwen35-2b-muesli-v12-Q4_K_M.gguf}"
mkdir -p "$WAVS" "$OUTS" "$LOGS"

say_wav() { [ -f "$WAVS/$1.wav" ] || say -v "$2" --data-format=LEI16@16000 -o "$WAVS/$1.wav" "$3"; }
# A self-repair, so the model is asked (a clean sentence would skip it).
say_wav warmup_repair Samantha "This warm up starts at 2, actually 3."
say_wav en_lunch Samantha "Hey Sam, um, are you free for lunch tomorrow? Let's meet at noon, if that works."
say_wav nl_lunch Xander "Hoi Sam, heb je morgen tijd voor de lunch? We kunnen om twaalf uur afspreken."
say_wav en_launch Samantha "Great job on the launch today, team. Thanks for all the hard work."
say_wav en_contract Samantha "Hi Priya, thanks for sending the contract. I will review it and reply by Friday."
say_wav en_notes Samantha "So far, the new onboarding flow works well, and the feedback is good."
# A self-repair, so the cleanup model must run (no clean-text skip): the
# fallback phase then really falls back.
say_wav en_send Samantha "The build finished at 2, actually 3, so I am merging it now."

app_pids() {
  for p in $(pgrep -f "$WB_BIN_RE --start-hidden"); do
    if lsof -a -p "$p" -d cwd -Fn 2>/dev/null | grep -qx "n$APP_DIR"; then echo "$p"; fi
  done
}

# The main process only: the other `wispr-bro --start-hidden` with the same
# directory is S-10's watchdog (a child of the main one), which stops the
# sidecar when the app ends.
main_pids() {
  local all
  all=$(app_pids)
  for p in $all; do
    parent=$(ps -o ppid= -p "$p" | tr -d ' ')
    echo "$all" | grep -qx "$parent" || echo "$p"
  done
}

stop_app() {
  local pids
  pids=$(main_pids)
  [ -n "$pids" ] && kill $pids 2>/dev/null
  for _ in $(seq 1 40); do
    [ -z "$(app_pids)" ] && return 0
    sleep 0.5
  done
  pids=$(app_pids)
  [ -n "$pids" ] && kill -9 $pids 2>/dev/null
  sleep 1
}

log_lines() { cat "$APP_DIR"/Data/logs/*.log 2>/dev/null | wc -l | tr -d ' '; }

# Starts the app and waits until its cleanup model is ready (the pinned
# GGUF's checksum takes about a minute on each start).
start_app() {
  local phase="$1"; shift
  local from
  wb_unbind_keys "$APP_DIR/Data" >>"$LOGS/$phase.profile" || return 1
  [ -d "$APP_DIR/Data/huggingface" ] || wb_clone_models "$APP_DIR/Data" "${WB_MODEL_CACHE:-}" >>"$LOGS/$phase.profile"
  from=$(log_lines)
  (cd "$APP_DIR" && env "$@" WB_DEBUG_TEXT_TRACE=1 "./$(wb_bin_name "$APP_DIR")" --start-hidden >"$LOGS/$phase.stdout" 2>&1 &)
  for _ in $(seq 1 120); do
    sleep 2
    if cat "$APP_DIR"/Data/logs/*.log 2>/dev/null | tail -n +"$((from + 1))" | grep -q "llm: .* ready ("; then
      sleep 2
      return 0
    fi
  done
  echo "$phase: the cleanup model did not get ready" >&2
}

# The last cleanup line of the app's log (no text).
last_cleanup() { cat "$APP_DIR"/Data/logs/*.log | grep "cleanup: " | tail -1 | sed -E 's/.*cleanup: //'; }

setup() {
  python3 -I "$HERE/setup.py" "$APP_DIR/Data" "$MODEL_ID" "$GGUF" light "$1" "$2"
}

# Prints: phase, case, take.sh status, the document text (\n as |; the last
# | is osascript's own line end, so a Return in the document shows as ||),
# and the take's cleanup line.
take() {
  local phase="$1" case="$2"
  local out="$OUTS/$phase-$case.txt"
  local status
  status=$("$TAKE" "$WAVS/$case.wav" "$out")
  printf '%s\t%s\t%s\t%s\t%s\n' "$phase" "$case" "$status" "$(tr '\n' '|' <"$out")" "$(last_cleanup)"
}

TE=com.apple.TextEdit
for phase in "$@"; do
  stop_app
  envs=()
  case "$phase" in
    personal) setup '{"personal":"very_relaxed"}' "{\"$TE\":[\"personal\",false]}"; cases="en_lunch nl_lunch" ;;
    work) setup '{"work":"upbeat"}' "{\"$TE\":[\"work\",false]}"; cases="en_launch" ;;
    email) setup '{"email":"formal"}' "{\"$TE\":[\"email\",false]}"; cases="en_contract" ;;
    other) setup '{"other":"relaxed"}' '{}'; cases="en_notes" ;;
    autosend) setup '{"work":"formal"}' "{\"$TE\":[\"work\",true]}"; cases="en_send" ;;
    fallback)
      setup '{"work":"formal"}' "{\"$TE\":[\"work\",true]}"
      envs=(WB_DEBUG_CLEANUP_DEADLINE_MS=1)
      cases="en_send"
      ;;
    *) echo "unknown phase $phase" >&2; exit 2 ;;
  esac
  start_app "$phase" "${envs[@]+"${envs[@]}"}"
  # Warm-up takes (sidecar start, Metal shaders) until one is cleaned by
  # the model (at most three); not scored.
  for _ in 1 2 3; do
    take "$phase" warmup_repair
    last_cleanup | grep -qE "^(applied|skipped light \(none, already_clean)" && break
    [ "$phase" = fallback ] && break
  done
  for c in $cases; do take "$phase" "$c"; done
done
stop_app
