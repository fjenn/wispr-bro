#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Mac test S-08: launch the test bundle and answer its first Microphone
# prompt with Allow (a silent take). HAL=<wbhal> APP=<bundle> OUT=<dir>.
set -u
: "${HAL:?}" "${APP:?}" "${OUT:?}"
if [ "${WB_S08_LOCKED:-}" != 1 ]; then
  WB_S08_LOCKED=1 exec /usr/bin/lockf -k -t 1800 "$HOME/wb-bench/gui.lock" /bin/bash "$0" "$@"
fi
EXE="$APP/Contents/MacOS/$(/usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "$APP/Contents/Info.plist")"
pid() { pgrep -fx "$EXE --start-hidden" | head -1; }
[ -n "$(pid)" ] || open -a "$APP" --args --start-hidden
for _ in $(seq 1 600); do [ -n "$(pid)" ] && break; sleep 1; done
sleep 15
kill -USR2 "$(pid)"
"$HAL" buttons UserNotificationCenter
"$HAL" press UserNotificationCenter Allow 30
sleep 3
kill -USR2 "$(pid)"
sleep 4
echo grant-finished
