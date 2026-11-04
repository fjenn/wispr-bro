#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-36.9-.11 (docs/fork/MAC-TEST-SCRIPT.md, "S-36 project file
# names"): the project's file list for vibe coding, against a throwaway
# Cursor (own --user-data-dir and --extensions-dir in a temp folder, never
# the owner's) on a throwaway git project:
#   read      the editor read's window hint: AXDocument (the active file)
#             and title, the resolved root and how many names it lists
#             (`caret::live_tests::live_editor_read`, AX reads only)
#   mention   Cursor's chat `@` picker offers a file that is neither open
#             nor shown in the explorer when it is typed key by key
#             (`insert::mac::live_tests::live_mention_through_the_picker`
#             with WB_S36_LIVE_FILE=deep_hidden_module.ts); types only
#             while that Cursor is in front
# Runs only when the person at this Mac has been idle for 120 s; quits the
# throwaway Cursor and removes its folders at the end.
#
# usage: scripts/wb-s36/project.sh read|mention|all
set -u
WHAT="${1:-all}"
HERE="$(cd "$(dirname "$0")" && pwd)"
TAURI="$(cd "$HERE/../../src-tauri" && pwd)"
BASE="$(mktemp -d "${TMPDIR:-/tmp}/wb-s36-project.XXXXXX")"
PROFILE="$BASE/profile"
WS="$BASE/shop-api"

idle=$(ioreg -c IOHIDSystem | awk '/HIDIdleTime/ {print int($NF/1e9); exit}')
if [ "${idle:-0}" -lt 120 ]; then
  echo "SKIP: the Mac was used ${idle}s ago (needs 120 s idle)"; rm -rf "$BASE"; exit 3
fi

mkdir -p "$WS/src/deep/nested" "$PROFILE/User"
printf '* text=auto\n' >"$WS/.gitattributes"
printf 'node_modules/\n' >"$WS/.gitignore"
printf '# shop-api\n' >"$WS/README.md"
printf 'export function getUserById(user_id: string) {}\n' >"$WS/src/user_service.ts"
printf 'export * from "./user_service";\n' >"$WS/src/index.ts"
printf 'export const hidden = 1;\n' >"$WS/src/deep/nested/deep_hidden_module.ts"
mkdir -p "$WS/node_modules/left-pad" && printf '1\n' >"$WS/node_modules/left-pad/index.js"
git -C "$WS" init -q && git -C "$WS" add -A >/dev/null 2>&1
cat >"$PROFILE/User/settings.json" <<'JSON'
{
  "editor.accessibilitySupport": "on",
  "window.restoreWindows": "none",
  "security.workspace.trust.enabled": false,
  "workbench.startupEditor": "none",
  "update.mode": "none",
  "telemetry.telemetryLevel": "off"
}
JSON

cursor_pid() {
  local p
  for p in $(pgrep -x Cursor); do
    ps -o command= -p "$p" | /usr/bin/grep -qF -- "--user-data-dir=$PROFILE" && { echo "$p"; return 0; }
  done
  return 1
}
front_pid() { lsappinfo info -only pid "$(lsappinfo front)" | sed -E 's/[^0-9]//g'; }
cleanup() {
  pkill -f "user-data-dir=$PROFILE" 2>/dev/null; sleep 1
  pkill -9 -f "user-data-dir=$PROFILE" 2>/dev/null
  rm -rf "$BASE"
}
trap cleanup EXIT

open -na Cursor --args --user-data-dir="$PROFILE" --extensions-dir="$PROFILE/ext" \
  --new-window "$WS" --goto "$WS/src/user_service.ts:1:1"
sleep 10
pid=$(cursor_pid) || { echo "FAIL: the throwaway Cursor did not start"; exit 1; }
if [ "$(front_pid)" != "$pid" ]; then
  echo "SKIP: the throwaway Cursor is not in front (front pid $(front_pid))"; exit 3
fi

cd "$TAURI" || exit 1
rc=0
if [ "$WHAT" = read ] || [ "$WHAT" = all ]; then
  WB_S36_LIVE_PID="$pid" cargo test --lib live_editor_read -- --ignored --nocapture 2>&1 \
    | /usr/bin/grep -E "workspace:|first read|after keep-warm|test result" || rc=1
fi
if [ "$WHAT" = mention ] || [ "$WHAT" = all ]; then
  if [ "$(front_pid)" = "$pid" ]; then
    osascript -e 'tell application "System Events" to keystroke "l" using command down'
    sleep 2
    WB_S36_LIVE_PID="$pid" WB_S36_LIVE_FILE=deep_hidden_module.ts \
      cargo test --lib live_mention_through_the_picker -- --ignored --nocapture 2>&1 \
      | /usr/bin/grep -E "offered|panicked|test result" || rc=1
  else
    echo "SKIP mention: the throwaway Cursor left the front"; rc=3
  fi
fi
exit $rc
