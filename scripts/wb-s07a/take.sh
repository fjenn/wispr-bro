#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-07a (docs/fork/MAC-TEST-SCRIPT.md): one take through the running
# test app's real record path (SIGUSR2 = Handy's toggle) with a `say`
# sentence (synthetic, no personal recordings) played through the speakers.
# Posts no key events. Holds ~/wb-bench/gui.lock (speakers and mic are
# shared with other runs) for the take only.
#
# Usage: scripts/wb-s07a/take.sh <app pid> <aiff file>
set -u
PID="$1"; AUDIO="$2"
LOCK="$HOME/wb-bench/gui.lock"
if [ "${S07A_LOCKED:-}" != 1 ]; then
  S07A_LOCKED=1 exec /usr/bin/lockf -k -t 600 "$LOCK" "$0" "$@"
fi
kill -USR2 "$PID"; sleep 0.8
afplay "$AUDIO"; sleep 0.5
kill -USR2 "$PID"
echo "take done $(date +%s)"
