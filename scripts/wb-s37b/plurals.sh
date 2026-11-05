#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-37b.1 (docs/fork/MAC-TEST-SCRIPT.md): the app menu's counts use
# the app language's CLDR plural form. Reads the menu bar of the running test
# app through Accessibility (scripts/wb-s26a/menus.swift, no events posted)
# and checks every "N words dictated" / dictionary / snippet count line
# against the locale's fork.json form for N (Intl.PluralRules). No audio.
#
#   scripts/wb-s37b/plurals.sh <pid> <lang> <out dir>
#
# Set the app language first (Settings › General › App language, or the
# profile's settings store) and relaunch, so the menu is rebuilt.
set -u
PID="$1"; LANG_CODE="$2"; OUT="$3"; mkdir -p "$OUT"
if [ "${S37B_LOCKED:-}" != 1 ]; then
  S37B_LOCKED=1 exec /usr/bin/lockf -k -t 600 "$HOME/wb-bench/gui.lock" "$0" "$PID" "$LANG_CODE" "$OUT"
fi
MENUS="${WBMENUS:?set WBMENUS to the compiled scripts/wb-s26a/menus.swift}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
"$MENUS" "$PID" menubar > "$OUT/menubar.txt" || exit 1
bun "$ROOT/scripts/wb-s37b/plurals-check.ts" "$LANG_CODE" "$OUT/menubar.txt"
