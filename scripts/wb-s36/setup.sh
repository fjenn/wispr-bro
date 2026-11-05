#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-36: after scripts/wb-s20/setup.sh (portable test profile,
# never the owner's), with the app stopped: vibe coding set up for Cursor
# (DEC-41: nothing is on before this), "Tag files in chat" on or off.
#
# usage: scripts/wb-s36/setup.sh <src-tauri/target/debug/Data> on|off|unset
#   on     set up for Cursor, code names on, file tags on
#   off    set up for Cursor, code names on, file tags off
#   unset  the default: not set up (S-36.4)
set -eu
DATA="$1"; MODE="$2"
case "$DATA" in */target/debug/Data) ;; *) echo "refusing: not a portable debug profile" >&2; exit 1 ;; esac
F="$DATA/fork-settings.json"
[ -f "$F" ] || echo '{"settings":{}}' >"$F"
case "$MODE" in
  on) v='{"enabled":true,"setup_completed":true,"editors":["cursor"],"file_tags_enabled":true}' ;;
  off) v='{"enabled":true,"setup_completed":true,"editors":["cursor"],"file_tags_enabled":false}' ;;
  unset) v='{"enabled":false,"setup_completed":false,"editors":[],"file_tags_enabled":false}' ;;
  *) echo "usage: setup.sh <Data> on|off|unset" >&2; exit 2 ;;
esac
plutil -replace settings.vibe_coding -json "$v" "$F"
plutil -replace settings.context_awareness -bool YES "$F"
echo "vibe coding $MODE"
