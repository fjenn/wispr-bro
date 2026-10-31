#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-12a: one dictation into a new TextEdit document this script
# creates, through the running test app's real record path (SIGUSR2 →
# microphone → speech model → text pipeline → cleanup → delivery), timing
# key-up (the second toggle) to text in the field by polling the document.
# Based on scripts/wb-s11/take.sh: holds ~/wb-bench/gui.lock for the whole
# take, checks the frontmost app before recording, closes only what it
# opened.
#
# usage: scripts/wb-s12a/take.sh <wav> <out file>
#   <out> gets the document text; stdout: "OK <ms from key-up to text>"
set -u
WAV="$1"; OUT="$2"
HERE="$(cd "$(dirname "$0")" && pwd)"
APP_DIR="$(cd "$HERE/../../src-tauri/target/debug" && pwd)"
LOCK="$HOME/wb-bench/gui.lock"

if [ "${WB_S12A_LOCKED:-}" != 1 ]; then
  WB_S12A_LOCKED=1 exec /usr/bin/lockf -k -t 900 "$LOCK" /bin/bash "$HERE/take.sh" "$@"
fi

pid=""
for p in $(pgrep -f "handy --start-hidden"); do
  if [ -z "$pid" ] && lsof -a -p "$p" -d cwd -Fn 2>/dev/null | grep -qx "n$APP_DIR"; then
    pid=$p
  fi
done
[ -n "$pid" ] || { echo "NO APP" | tee "$OUT"; exit 3; }

front() { lsappinfo info -only bundleid "$(lsappinfo front)" | sed -E 's/.*="?([^"]*)"?/\1/'; }

doc=$(osascript -e 'tell application "TextEdit"' -e 'activate' -e 'set d to make new document' \
  -e 'return name of d' -e 'end tell')
sleep 3
f=$(front)
if [ "$f" != com.apple.TextEdit ]; then
  echo "SKIP front=$f" | tee "$OUT"
else
  kill -USR2 "$pid"
  sleep 0.8
  afplay "$WAV"
  sleep 0.6
  kill -USR2 "$pid"
  # Key-up: poll the document until text appears (≈ 60-90 ms per poll).
  ms=$(python3 -I - "$doc" <<'PY'
import subprocess, sys, time
doc = sys.argv[1]
t0 = time.monotonic()
while time.monotonic() - t0 < 15:
    r = subprocess.run(["osascript", "-e", f'tell application "TextEdit" to get text of document "{doc}"'],
                       capture_output=True, text=True)
    if r.stdout.strip():
        print(int((time.monotonic() - t0) * 1000))
        break
else:
    print(-1)
PY
)
  sleep 1.5
  osascript -e "tell application \"TextEdit\" to get text of document \"$doc\"" >"$OUT"
  echo "OK $ms"
fi
osascript -e "tell application \"TextEdit\" to close document \"$doc\" saving no"
exit 0
