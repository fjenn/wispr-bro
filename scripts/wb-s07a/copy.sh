#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-07a.2 (docs/fork/MAC-TEST-SCRIPT.md): click Copy on the bar's
# card and check the clipboard holds exactly the card text, then put the
# clipboard back. Opens its own TextEdit document so the click happens with
# a window of this run in front (the card is a non-activating panel; no key
# events are posted). Holds ~/wb-bench/gui.lock.
#
# Usage: scripts/wb-s07a/copy.sh <app pid> <wbpill binary> <expected text> <scratch dir>
set -u
PID="$1"; WBPILL="$2"; EXPECT="$3"; DIR="$4"
if [ "${S07A_LOCKED:-}" != 1 ]; then
  S07A_LOCKED=1 exec /usr/bin/lockf -k -t 600 "$HOME/wb-bench/gui.lock" "$0" "$@"
fi
SAVED="$DIR/clipboard-saved.txt"
pbpaste > "$SAVED"
DOC="$DIR/s07a-front.txt"
: > "$DOC"
open -a TextEdit "$DOC"; sleep 2
# Copy sits 70 pt from the card's right edge, mid-height (400 x 58 card).
WB_PID="$PID" WB_FRONT=TextEdit "$WBPILL" -e "front; clickat 330 29; wait 600; front"
GOT="$(pbpaste)"
if [ "$GOT" = "$EXPECT" ]; then echo "COPY OK: clipboard = card text"; else echo "COPY MISMATCH: got ${#GOT} chars"; fi
# Close our document and put the clipboard back.
osascript -e 'tell application "TextEdit" to close (every document whose name is "s07a-front.txt") saving no' > /dev/null 2>&1 || true
pbcopy < "$SAVED"
echo "clipboard restored"
