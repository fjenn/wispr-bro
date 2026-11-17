#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Automatic sync, end to end on one Mac: a Mac test-identity build of this
# clone (com.wisprbro.synccheck, portable, never the owner's app or data)
# and the iPhone app in a simulator (which shares the Mac's network, so
# Bonjour and the connection are the real ones).
#
#   e2e.sh mac-setup <app dir> <binary>    portable profile, sync on, onboarded
#       (<app dir> must end in target/debug: an unbundled build finds its
#       resources only there, e.g. <scratch>/synccheck/target/debug)
#   e2e.sh mac-start <app dir>             start it (pairing code 271828;
#                                          REAL_CODE=1: a random one, HIDDEN=1: no window)
#   e2e.sh mac-stop  <app dir>
#   e2e.sh sim-setup <sim udid> <hf seed>  fresh install, onboarded
#   e2e.sh word <fork.db> <word>           add a dictionary word, as the page does
#   e2e.sh words <fork.db>                 the active words
#   e2e.sh take <fork.db> <day> <words>    a day's stats
#   e2e.sh total <fork.db>                 words across devices (what Home shows)
#   e2e.sh wait-word <fork.db> <word> [seconds]   until the word is (not) there:
#   e2e.sh wait-gone <fork.db> <word> [seconds]
#   e2e.sh sim-db <sim udid>               path of the simulator app's fork.db
#
# The phone's side of the pairing is done on its screen (the e2e script
# runner, `ios-native/e2e/run.sh … WBScriptTests`, or by hand): Settings ›
# Sync with your Mac › Find my Mac › the Mac › 271828.
#
# Words and stats are written with sqlite3 the way the app writes them; the
# sync engine reads the database every two seconds, so it needs no signal.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
CODE=271828
cmd=${1:?command}; shift

sql() { sqlite3 -cmd ".timeout 5000" "$@"; }
now_ms() { python3 -I -c 'import time; print(int(time.time()*1000))'; }

case "$cmd" in
mac-setup)
  app=${1:?app dir}; bin=${2:?binary}
  case "$app" in *synccheck*) ;; *) echo "refusing: not a synccheck folder" >&2; exit 1 ;; esac
  mkdir -p "$app/Data"
  cp "$bin" "$app/wispr-bro"
  # The resources the binary looks for next to itself (a clone, no space).
  rm -rf "$app/resources" && cp -Rc "$(dirname "$bin")/resources" "$app/resources"
  printf "Handy Portable Mode\n" >"$app/portable"
  # Tauri takes a folder for Cargo's output (resources next to the binary)
  # only when it is named target/<profile> and holds Cargo's lock file.
  : >"$app/.cargo-lock"
  # A speech model without a download: the newest test identity's cache.
  cache=$(ls -dt "$HOME/Library/Application Support"/com.wisprbro.*check/huggingface 2>/dev/null | head -1)
  [ -n "$cache" ] && [ ! -d "$app/Data/huggingface" ] && cp -Rc "$cache" "$app/Data/huggingface"
  # One launch writes the settings files; then: onboarded, shortcuts
  # unbound (the owner's app keeps fn), sync on.
  (cd "$app" && ./wispr-bro --start-hidden >/dev/null 2>&1 &)
  for _ in $(seq 1 60); do [ -f "$app/Data/fork-settings.json" ] && [ -f "$app/Data/fork.db" ] && break; sleep 1; done
  sleep 3
  "$0" mac-stop "$app"
  python3 -I - "$app/Data" <<'PY'
import json, sys
data = sys.argv[1]
p = f"{data}/settings_store.json"
store = json.load(open(p))
store["settings"].update(onboarding_completed=True, update_checks_enabled=False, spoken_languages=["en"])
json.dump(store, open(p, "w"), indent=2)
p = f"{data}/fork-settings.json"
fork = json.load(open(p))
s = fork.setdefault("settings", {})
s["onboarding"] = {"status": "completed", "current_step": None, "completed_steps": [], "checklist_done": [], "completed_at_ms": 1}
s["first_use_choice"] = "email"
s["update_checks_enabled"] = False
# Next to the owner's own Wispr Bro: this app answers no global shortcut.
s["hotkeys"] = {action: [] for action in s.get("hotkeys", {})}
json.dump(fork, open(p, "w"), indent=2)
json.dump({"enabled": True, "peers": []}, open(f"{data}/sync-peers.json", "w"))
PY
  echo "mac: ready in $app"
  ;;
mac-start)
  app=${1:?app dir}
  # REAL_CODE=1: a random code as in the product (for pairing a real
  # iPhone by hand) instead of the script's fixed one.
  [ -n "${REAL_CODE:-}" ] && CODE=
  (cd "$app" && WB_SYNC_TEST_CODE=$CODE WISPRBRO_HUB_ROUTE="${ROUTE:-#/hub/settings/privacy}" \
    nohup ./wispr-bro ${HIDDEN:+--start-hidden} >"$app/stdout.log" 2>&1 &)
  sleep 4
  pgrep -f "$app/wispr-bro" >/dev/null || pgrep -f "^./wispr-bro" >/dev/null || { echo "mac: did not start" >&2; exit 1; }
  echo "mac: started"
  ;;
mac-stop)
  app=${1:?app dir}
  pids=$(lsof -a -d cwd -c wispr-bro -Fpn 2>/dev/null | awk -v d="n$app" '/^p/{p=substr($0,2)} $0==d{print p}')
  [ -n "$pids" ] && kill $pids 2>/dev/null || true
  for _ in $(seq 1 20); do
    left=$(lsof -a -d cwd -c wispr-bro -Fpn 2>/dev/null | awk -v d="n$app" '/^p/{p=substr($0,2)} $0==d{print p}')
    [ -z "$left" ] && break; sleep 0.5
  done
  [ -n "${left:-}" ] && kill -9 $left 2>/dev/null || true
  echo "mac: stopped"
  ;;
sim-setup)
  sim=${1:?simulator}; seed=${2:?folder holding huggingface/}
  xcrun simctl bootstatus "$sim" -b >/dev/null
  support=$("$HERE/../../src-tauri/ios/sim-fresh.sh" "$sim" "$seed" | tail -1)
  xcrun simctl launch "$sim" com.wisprbro.ios >/dev/null
  for _ in $(seq 1 60); do [ -f "$support/fork-settings.json" ] && [ -f "$support/fork.db" ] && break; sleep 1; done
  sleep 3
  xcrun simctl terminate "$sim" com.wisprbro.ios
  python3 -I - "$support" <<'PY'
import json, sys
data = sys.argv[1]
p = f"{data}/settings_store.json"
store = json.load(open(p))
store["settings"].update(onboarding_completed=True, spoken_languages=["en"])
json.dump(store, open(p, "w"), indent=2)
p = f"{data}/fork-settings.json"
fork = json.load(open(p))
fork.setdefault("settings", {})["onboarding"] = {"status": "completed", "current_step": None, "completed_steps": [], "checklist_done": [], "completed_at_ms": 1}
json.dump(fork, open(p, "w"), indent=2)
PY
  echo "$support"
  ;;
sim-db)
  data=$(xcrun simctl get_app_container "${1:?simulator}" com.wisprbro.ios data)
  echo "$data/Library/Application Support/com.wisprbro.ios/fork.db"
  ;;
word)
  db=${1:?db}; word=${2:?word}; t=$(now_ms)
  norm=$(printf '%s' "$word" | tr '[:upper:]' '[:lower:]')
  sql "$db" "INSERT INTO dictionary_entries (kind, term, norm, source, status, created_at, updated_at)
             VALUES ('word', '$word', '$norm', 'user', 'active', $t, $t);"
  ;;
unword)
  sql "${1:?db}" "DELETE FROM dictionary_entries WHERE kind = 'word' AND term = '${2:?word}';"
  ;;
words)
  sql "${1:?db}" "SELECT term FROM dictionary_entries WHERE kind = 'word' AND status = 'active' ORDER BY term;" | paste -sd, -
  ;;
take)
  db=${1:?db}; day=${2:?day}; n=${3:?words}
  sql "$db" "INSERT INTO daily_stats (day, words, dictations, speech_ms, timed_words) VALUES ('$day', $n, 1, 30000, $n)
             ON CONFLICT (day) DO UPDATE SET words = words + $n, dictations = dictations + 1,
               speech_ms = speech_ms + 30000, timed_words = timed_words + $n;"
  ;;
total)
  sql "${1:?db}" "SELECT COALESCE(SUM(words), 0) FROM all_daily_stats;"
  ;;
wait-word | wait-gone)
  db=${1:?db}; word=${2:?word}; limit=${3:-30}; start=$(date +%s)
  while :; do
    n=$(sql "$db" "SELECT COUNT(*) FROM dictionary_entries WHERE kind = 'word' AND status = 'active' AND term = '$word';")
    if { [ "$cmd" = wait-word ] && [ "$n" = 1 ]; } || { [ "$cmd" = wait-gone ] && [ "$n" = 0 ]; }; then
      echo "$cmd $word: after $(($(date +%s) - start)) s"; exit 0
    fi
    [ $(($(date +%s) - start)) -ge "$limit" ] && { echo "$cmd $word: NOT within $limit s" >&2; exit 1; }
    sleep 1
  done
  ;;
*) echo "unknown command $cmd" >&2; exit 2 ;;
esac
