#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Mac test S-08: launch the test bundle and answer its first Microphone
# prompt with Allow (a silent take on the real microphone: nothing is
# played). The profile starts with every global shortcut unbound and the
# model cache cloned. HAL=<wbhal> APP=<bundle> OUT=<dir>.
set -u
: "${HAL:?}" "${APP:?}" "${OUT:?}"
. "$(cd "$(dirname "$0")" && pwd)/../wb-lib/app.sh"
if [ "${WB_S08_LOCKED:-}" != 1 ]; then
  WB_S08_LOCKED=1 exec /usr/bin/lockf -k -t 1800 "$HOME/wb-bench/gui.lock" /bin/bash "$0" "$@"
fi
EXE="$APP/Contents/MacOS/$(/usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "$APP/Contents/Info.plist")"
pid() { pgrep -fx "$EXE --start-hidden" | head -1; }
if [ -z "$(pid)" ]; then
  DATA="$APP/Contents/MacOS/Data"
  wb_unbind_keys "$DATA"
  [ -d "$DATA/huggingface" ] || wb_clone_models "$DATA" "${WB_MODEL_CACHE:-}"
  mkdir -p "$OUT"; echo real >"$OUT/mic"
  open -a "$APP" --args --start-hidden
fi
for _ in $(seq 1 600); do [ -n "$(pid)" ] && break; sleep 1; done
sleep 15
kill -USR2 "$(pid)"
"$HAL" buttons UserNotificationCenter
"$HAL" press UserNotificationCenter Allow 30
sleep 3
kill -USR2 "$(pid)"
sleep 4
echo grant-finished
