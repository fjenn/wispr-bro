#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-12b.2: shows the running test app's window, runs S-16's UI
# driver on it (events only while that app is in front) and screenshots the
# window. Holds ~/wb-bench/gui.lock for the run.
#
# usage: scripts/wb-s12b/panel.sh <pid> '<ui.swift commands>' <png>
set -u
PID="$1"; CMDS="$2"; PNG="$3"
HERE="$(cd "$(dirname "$0")" && pwd)"
. "$HERE/../wb-lib/app.sh"
if [ "${WB_S12B_LOCKED:-}" != 1 ]; then
  WB_S12B_LOCKED=1 exec /usr/bin/lockf -k -t 900 "$HOME/wb-bench/gui.lock" /bin/bash "$HERE/panel.sh" "$@"
fi
# A second launch of the same portable build brings the running app's
# window forward (single instance) and exits.
(D="$HERE/../../src-tauri/target/debug"; cd "$D" && "./$(wb_bin_name "$D")" >/dev/null 2>&1 &)
sleep 3
swift "$HERE/../wb-s16/ui.swift" "$PID" "front; $CMDS" || exit $?
read -r wid _ < <(swift "$HERE/../wb-window-id.swift" "$PID")
screencapture -x -o -l"$wid" "$PNG" && echo "shot $PNG"
