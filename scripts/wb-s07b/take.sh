#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-07b.2 (docs/fork/MAC-TEST-SCRIPT.md): one take through the
# running test app's record path (SIGUSR2 = Handy's toggle, no key events)
# with a `say` file played through the speakers, pasted into a TextEdit
# document this script opens (so the text can only land in our window).
# Saves and restores the clipboard (text); holds ~/wb-bench/gui.lock for
# the take only; refuses to start a take unless TextEdit is in front.
#
# Usage: scripts/wb-s07b/take.sh <app pid> <aiff> <scratch dir>
set -u
PID="$1"; AUDIO="$2"; DIR="$3"
if [ "${S07B_LOCKED:-}" != 1 ]; then
  S07B_LOCKED=1 exec /usr/bin/lockf -k -t 600 "$HOME/wb-bench/gui.lock" "$0" "$@"
fi
SAVED="$DIR/clipboard-saved.txt"
pbpaste > "$SAVED"
NAME="s07b-take-$(date +%s).txt"
DOC="$DIR/$NAME"
: > "$DOC"
open -a TextEdit "$DOC"; sleep 2
FRONT=$(osascript -e 'tell application "System Events" to get name of first process whose frontmost is true')
if [ "$FRONT" != "TextEdit" ]; then
  echo "front is $FRONT, not TextEdit: no take"
  osascript -e "tell application \"TextEdit\" to close (every document whose name is \"$NAME\") saving no" > /dev/null 2>&1 || true
  exit 1
fi
T0=$(date +%s)
kill -USR2 "$PID"; sleep 0.8
afplay "$AUDIO"; sleep 0.5
kill -USR2 "$PID"
echo "take ended $(date +%s) (started $T0)"
sleep 6
TEXT=$(osascript -e "tell application \"TextEdit\" to get text of (first document whose name is \"$NAME\")" 2>/dev/null)
echo "pasted: $TEXT"
osascript -e "tell application \"TextEdit\" to close (every document whose name is \"$NAME\") saving no" > /dev/null 2>&1 || true
pbcopy < "$SAVED"
echo "clipboard restored"
