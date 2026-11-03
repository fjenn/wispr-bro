#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-05: start / stop the portable test build (never the owner's
# app). The app runs as `./wispr-bro --start-hidden` (or `./handy` from a
# plain `cargo build`, scripts/wb-lib/app.sh) from src-tauri/target/debug
# (portable profile in ./Data, test identifier com.wisprbro.s05check).
#
# usage: scripts/wb-s05/app.sh start [keep] [action=chord ...] | stop | pid | init
#   start  with the app closed: the profile's global shortcuts unbound except
#          the comma-separated actions in [keep] and the [action=chord]
#          bindings a check needs (scripts/wb-lib/profile.py, Mac wave 2
#          finding 7), the model cache cloned when Data/huggingface is
#          missing, then the app started with the test microphone
#          (WISPRBRO_TEST_MIC_WAV=Data/test-mic.wav; the take drivers load
#          each WAV into it, never the speakers; WB_ALLOW_SPEAKERS=1 starts
#          it on the real mic instead). Other variables pass through
#          (WB_DEBUG_TEXT_TRACE=1, WB_DEBUG_CLEANUP_DEADLINE_MS=1 …).
#   init   first launch (creates Data/settings_store.json and fork.db), stop
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
APP_DIR="$(cd "$HERE/../../src-tauri/target/debug" && pwd)"
. "$HERE/../wb-lib/app.sh"

# The app's main process: a `wispr-bro`/`handy --start-hidden` in our build folder
# whose parent is not one too (the LLM watchdog is a fork of the app with
# the same command line).
app_pid() {
  local p pp
  local all
  all=" $(pgrep -f "$WB_BIN_RE --start-hidden" | tr '\n' ' ') "
  for p in $all; do
    pp=$(ps -o ppid= -p "$p" | tr -d ' ')
    case "$all" in *" $pp "*) continue ;; esac
    if lsof -a -p "$p" -d cwd -Fn 2>/dev/null | /usr/bin/grep -qx "n$APP_DIR"; then
      echo "$p"
      return 0
    fi
  done
  return 1
}

case "${1:-}" in
  start)
    if app_pid >/dev/null; then echo "already running: $(app_pid)"; exit 0; fi
    keep="${2:-}"
    shift 2 2>/dev/null || shift $#
    wb_unbind_keys "$APP_DIR/Data" "$keep" "$@" || exit 1
    [ -d "$APP_DIR/Data/huggingface" ] || wb_clone_models "$APP_DIR/Data" "${WB_MODEL_CACHE:-}" || exit 1
    wb_test_mic_env "$APP_DIR/Data" || exit 1
    echo "test microphone: ${WISPRBRO_TEST_MIC_WAV:-off (WB_ALLOW_SPEAKERS=1, real mic)}"
    cd "$APP_DIR" || exit 1
    nohup "./$(wb_bin_name "$APP_DIR")" --start-hidden </dev/null >"$APP_DIR/Data/s05-stdout.log" 2>&1 &
    sleep 12
    app_pid || { echo "did not start"; exit 1; }
    ;;
  stop)
    pid=$(app_pid) || { echo "not running"; exit 0; }
    kill "$pid"
    sleep 3
    # The LLM watchdog and sidecar are children of the app; make sure
    # nothing of ours is left.
    pkill -f "$APP_DIR/$WB_BIN_RE" 2>/dev/null
    pkill -f "$APP_DIR/llama-server" 2>/dev/null
    echo "stopped $pid"
    ;;
  pid)
    app_pid
    ;;
  init)
    "$0" start && "$0" stop
    ls "$APP_DIR/Data"
    ;;
  *)
    echo "usage: $0 start [keep] [action=chord ...] | stop | pid | init" >&2
    exit 2
    ;;
esac
