#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-05: start / stop the portable test build (never the owner's
# app). The app runs as `./handy --start-hidden` from src-tauri/target/debug
# (portable profile in ./Data, test identifier com.wisprbro.s05check).
#
# usage: scripts/wb-s05/app.sh start|stop|pid|init
#   init   first launch (creates Data/settings_store.json and fork.db), stop
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
APP_DIR="$(cd "$HERE/../../src-tauri/target/debug" && pwd)"

# The app's main process: a `handy --start-hidden` in our build folder
# whose parent is not one too (the LLM watchdog is a fork of the app with
# the same command line).
app_pid() {
  local p pp
  local all
  all=" $(pgrep -f "handy --start-hidden" | tr '\n' ' ') "
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
    cd "$APP_DIR" || exit 1
    nohup ./handy --start-hidden </dev/null >"$APP_DIR/Data/s05-stdout.log" 2>&1 &
    sleep 12
    app_pid || { echo "did not start"; exit 1; }
    ;;
  stop)
    pid=$(app_pid) || { echo "not running"; exit 0; }
    kill "$pid"
    sleep 3
    # The LLM watchdog and sidecar are children of the app; make sure
    # nothing of ours is left.
    pkill -f "$APP_DIR/handy" 2>/dev/null
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
    echo "usage: $0 start|stop|pid|init" >&2
    exit 2
    ;;
esac
