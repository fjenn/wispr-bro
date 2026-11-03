#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test F-02.3 (docs/fork/MAC-TEST-SCRIPT.md, queued for the Mac wave): the
# 0.x → 0.x+1 update end to end, served from a loopback stand-in for GitHub
# releases, without any click.
#
#   1. Builds two debug app bundles with the test identity
#      `com.wisprbro.f02check` ("Wispr Bro F02 Check"), both signed with our
#      Developer ID and their updater archives signed with our real updater
#      key: A at the overlay version, B one prerelease step higher.
#   2. Serves a GitHub-shaped tree on 127.0.0.1:${F02_PORT:-18092}
#      (`python3 -m http.server`): stable `latest.json` → A's own version and
#      the beta `latest.json` → a higher beta, then stable → B.
#   3. Runs A from a scratch folder with a throwaway HOME (nothing touches the
#      real ~/Library) and `WISPRBRO_UPDATE_ENDPOINT` +
#      `WISPRBRO_UPDATE_SELFTEST=1` (debug-only hooks, `fork/update`):
#        phase 1 (stable channel, only a beta is newer): "UP-TO-DATE";
#        phase 2 (stable → B): "INSTALLED B", relaunch, then B logs
#        "UP-TO-DATE".
#   4. Checks the bundle on disk is B, valid (`codesign --verify --deep
#      --strict`) and has the same designated requirement as A, which is what
#      macOS privacy grants (Microphone, Accessibility) are tied to.
#
# S-28 phases (the same two builds, A restored from a pristine copy each
# time; the background scheduler, no selftest; shorter timings through the
# debug-only WISPRBRO_UPDATE_TIMING):
#   S-28.1 B downloads in the background; a hands-free take (SIGUSR2; the
#          test microphone plays a silent WAV, WISPRBRO_TEST_MIC_WAV, so no
#          real mic, no speakers and nothing to paste) runs past the quiet window
#          and nothing installs; after the take and the quiet window B
#          installs and the app relaunches hidden (the Hub had been hidden),
#          with no layer-0 window afterwards.
#   S-28.2 B ready, the app is quit: B installs at quit, no relaunch.
#   S-28.3 B's archive is tampered with: three failed installs, then the
#          24-hour pause (update-state.json).
#   S-28.4 A's folder is read-only: no background install, no password
#          prompt, the "waiting for Restart now" line instead.
# S-28.1 needs Automation for the terminal → System Events (hides the app).
# The throwaway profile starts with every global shortcut unbound (Mac wave
# 2 finding 7: fn and ⌃⌘V were live next to the owner's app) and a cloned
# model cache (no 1.6 GB download; `WB_MODEL_CACHE` or the newest test
# identity's), both through scripts/wb-lib/profile.py.
#
# Real permission survival and the footer/tray paths stay RS-12 person checks.
#
# Usage: scripts/wb-f02/update-e2e.sh <out dir>   (F02_ONLY_S28=1 skips F-02.3)
set -euo pipefail
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../wb-lib/app.sh"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="$1"
PORT="${F02_PORT:-18092}"
IDENTITY="${WB_SIGNING_IDENTITY:-Developer ID Application: Finn Jennen (446XT9839S)}"
NAME="Wispr Bro F02 Check"
ID="com.wisprbro.f02check"
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
cd "$ROOT"

A="$(bun -e "console.log(require('./src-tauri/tauri.fork.conf.json').version)")"
B="$(bun -e "const v='$A';const m=v.match(/^(.*\\.)(\\d+)$/);console.log(m?m[1]+(+m[2]+1):v+'-alpha.1')")"
C="$(bun -e "const [maj,min,pat]='$A'.split('-')[0].split('.').map(Number);console.log(maj+'.'+(min+1)+'.0-beta.1')")"
echo "A=$A B=$B (stable) C=$C (beta)"

export TAURI_SIGNING_PRIVATE_KEY="${TAURI_SIGNING_PRIVATE_KEY:-$(cat "$HOME/.wispr-bro/updater.key")}"
if [[ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ]]; then
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(security find-generic-password -s wispr-bro-updater -a updater -w)"
  export TAURI_SIGNING_PRIVATE_KEY_PASSWORD
fi
export APPLE_SIGNING_IDENTITY="$IDENTITY" CMAKE_POLICY_VERSION_MINIMUM=3.5
unset APPLE_ID APPLE_PASSWORD APPLE_API_KEY APPLE_API_ISSUER APPLE_API_KEY_PATH

ci/llama-server/build.sh --if-missing
BUNDLE="src-tauri/target/debug/bundle/macos"
build() { # <version> <dest dir>
  bun tauri build --debug --bundles app --config src-tauri/tauri.fork.conf.json \
    --config "{\"identifier\":\"$ID\",\"productName\":\"$NAME\",\"version\":\"$1\"}" \
    > "$OUT/build-$1.log" 2>&1
  mkdir -p "$2"
  rm -rf "$2/$NAME.app"
  ditto "$BUNDLE/$NAME.app" "$2/$NAME.app"
  cp "$BUNDLE/$NAME.app.tar.gz" "$2/Wispr-Bro_$1_aarch64.app.tar.gz"
  cp "$BUNDLE/$NAME.app.tar.gz.sig" "$2/Wispr-Bro_$1_aarch64.app.tar.gz.sig"
}
build "$B" "$OUT/b"
build "$A" "$OUT/a"
rm -rf "$OUT/a-pristine"
mkdir -p "$OUT/a-pristine"
ditto "$OUT/a/$NAME.app" "$OUT/a-pristine/$NAME.app"
bun scripts/wb-release/minisign.ts "$OUT/b/Wispr-Bro_${B}_aarch64.app.tar.gz"

FEED="$OUT/feed"
REPO="$FEED/fjenn/wispr-bro"
BASE="http://127.0.0.1:$PORT/fjenn/wispr-bro"
rm -rf "$FEED"
mkdir -p "$REPO/releases/latest/download" "$REPO/releases/download/v$B" "$REPO/releases/download/beta"
cp "$OUT/b/Wispr-Bro_${B}_aarch64.app.tar.gz"* "$REPO/releases/download/v$B/"
cp "$OUT/b/Wispr-Bro_${B}_aarch64.app.tar.gz"* "$REPO/releases/download/beta/"
mv "$REPO/releases/download/beta/Wispr-Bro_${B}_aarch64.app.tar.gz" "$REPO/releases/download/beta/Wispr-Bro_${C}_B_aarch64.app.tar.gz"
mv "$REPO/releases/download/beta/Wispr-Bro_${B}_aarch64.app.tar.gz.sig" "$REPO/releases/download/beta/Wispr-Bro_${C}_B_aarch64.app.tar.gz.sig"
manifest() { # <version> <release tag> <asset> <out>
  bun scripts/wb-release/manifest.ts --version "$1" --release-tag "$2" \
    --asset "darwin-aarch64=$3" --base "$BASE" --out "$4"
}
manifest "$C" beta "$REPO/releases/download/beta/Wispr-Bro_${C}_B_aarch64.app.tar.gz" \
  "$REPO/releases/download/beta/latest.json"
# Phase 1: the stable feed offers A's own version (nothing newer).
bun -e "const j=require('$REPO/releases/download/beta/latest.json');j.version='$A';require('fs').writeFileSync('$REPO/releases/latest/download/latest.json',JSON.stringify(j))"

python3 -I -m http.server "$PORT" --bind 127.0.0.1 --directory "$FEED" > "$OUT/feed.log" 2>&1 &
SERVER=$!
APP_HOME="$OUT/home"
APP="$OUT/a/$NAME.app"
LOG="$APP_HOME/Library/Logs/$ID/wispr-bro.log"
cleanup() {
  pkill -f "$OUT/a/$NAME.app/Contents/MacOS/" 2>/dev/null || true
  kill "$SERVER" 2>/dev/null || true
}
trap cleanup EXIT
rm -rf "$APP_HOME"
mkdir -p "$APP_HOME"
DATA="$APP_HOME/Library/Application Support/$ID"
wb_unbind_keys "$DATA"
wb_clone_models "$DATA" "${WB_MODEL_CACHE:-}"
SILENCE="$OUT/silence.wav"
python3 -I -c 'import sys,wave; w=wave.open(sys.argv[1],"wb"); w.setnchannels(1); w.setsampwidth(2); w.setframerate(16000); w.writeframes(b"\0\0"*16000); w.close()' "$SILENCE"
DR_A="$(codesign -d -r- "$APP" 2>&1 | sed -n 's/^designated => //p')"
EXE="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$APP/Contents/Info.plist")"

run_until() { # <pattern> <timeout s>
  local deadline=$(( $(date +%s) + $2 ))
  while [[ $(date +%s) -lt $deadline ]]; do
    if [[ -f "$LOG" ]] && grep -q "$1" "$LOG"; then return 0; fi
    sleep 1
  done
  return 1
}
launch() {
  HOME="$APP_HOME" WISPRBRO_UPDATE_ENDPOINT="$BASE" WISPRBRO_UPDATE_SELFTEST=1 \
    WISPRBRO_TEST_MIC_WAV="$SILENCE" nohup "$APP/Contents/MacOS/$EXE" --start-hidden > "$OUT/app-stdout.log" 2>&1 &
}
stop_app() {
  pkill -f "$APP/Contents/MacOS/" 2>/dev/null || true
  sleep 3
}

pass=0; fail=0
# check <label> <command…>: PASS/FAIL without stopping the run.
check() {
  local label="$1"; shift
  if "$@"; then echo "PASS $label"; pass=$((pass + 1)); else echo "FAIL $label"; fail=$((fail + 1)); fi
}

STABLE_B() {
  manifest "$B" "v$B" "$REPO/releases/download/v$B/Wispr-Bro_${B}_aarch64.app.tar.gz" \
    "$REPO/releases/latest/download/latest.json"
}

if [[ "${F02_ONLY_S28:-}" != 1 ]]; then
launch
check "F-02.3a stable install ignores the newer beta ($C)" run_until "fork update selftest: UP-TO-DATE" 120
stop_app
: > "$LOG"

STABLE_B
launch
check "F-02.3b $A downloads, verifies and installs $B" run_until "fork update selftest: INSTALLED $B" 300
check "F-02.3c relaunched as $B" run_until "fork update selftest: v$B" 120
check "F-02.3d $B is up to date" run_until "fork update selftest: UP-TO-DATE" 120
stop_app

V="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP/Contents/Info.plist")"
check "F-02.3e bundle on disk is $B (got $V)" test "$V" = "$B"
check "F-02.3f updated bundle passes codesign --verify --deep --strict" codesign --verify --deep --strict "$APP"
DR_B="$(codesign -d -r- "$APP" 2>&1 | sed -n 's/^designated => //p')"
check "F-02.3g designated requirement unchanged (privacy grants carry over)" test -n "$DR_A" -a "$DR_A" = "$DR_B"
grep "fork update selftest\|update:" "$LOG" > "$OUT/update-lines.log" || true
fi

# ------------------------------------------------------------ S-28 -------
STABLE_B
WINDOW_ID="$OUT/wb-window-id"
swiftc -O -o "$WINDOW_ID" "$ROOT/scripts/wb-window-id.swift" >/dev/null
restore_a() {
  stop_app
  chmod -R u+w "$OUT/a" 2>/dev/null || true
  rm -rf "$OUT/a/$NAME.app"
  ditto "$OUT/a-pristine/$NAME.app" "$OUT/a/$NAME.app"
  rm -f "$DATA/update-state.json"
  : > "$LOG" 2>/dev/null || true
}
launch_s28() { # <WISPRBRO_UPDATE_TIMING>
  HOME="$APP_HOME" WISPRBRO_UPDATE_ENDPOINT="$BASE" WISPRBRO_UPDATE_TIMING="$1" \
    WISPRBRO_TEST_MIC_WAV="$SILENCE" nohup "$APP/Contents/MacOS/$EXE" > "$OUT/app-stdout.log" 2>&1 &
}
app_pid() { pgrep -f "$APP/Contents/MacOS/$EXE" | head -1; }
bundle_version() { /usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP/Contents/Info.plist"; }
not_in_log() { ! grep -q "$1" "$LOG"; }
no_hub_window() { ! "$WINDOW_ID" "$(app_pid)" >/dev/null 2>&1; }

# S-28.1: a take outlasting the quiet window defers the install; then a
# quiet relaunch.
restore_a
launch_s28 "first=3,quiet=40,tick=1,retry=5"
check "S-28.1a $B downloads and verifies in the background" run_until "update: $B ready" 120
# The throwaway HOME starts without a speech model: the first one downloads
# from Hugging Face, and a take that ends before it is ready keeps the
# transcription coordinator busy until it loads (Mac wave 2: 1c timed out
# that way). Wait for the model before the take.
run_until "models: selected\|speech setup at launch: Ready" 900 || echo "NOTE no speech model ready after 900 s"
osascript -e "tell application \"System Events\" to set visible of (first process whose unix id is $(app_pid)) to false" >/dev/null 2>&1 || true
kill -USR2 "$(app_pid)" # hands-free take starts
sleep 55
check "S-28.1b nothing installs during a 55 s take (past the 40 s quiet window)" not_in_log "update: installing"
kill -USR2 "$(app_pid)" # take ends
check "S-28.1c $B installs after the take and the quiet window" run_until "update: installed $B" 120
check "S-28.1d relaunches hidden (the Hub was hidden)" run_until "update: relaunching (hidden)" 30
check "S-28.1e the new process says it was relaunched hidden" run_until "update: relaunched after an update (hidden)" 120
sleep 3
check "S-28.1f no Hub window after the relaunch" no_hub_window
check "S-28.1g bundle on disk is $B" test "$(bundle_version)" = "$B"
cp "$LOG" "$OUT/s28-1.log" 2>/dev/null || true

# S-28.2: install at quit, no relaunch.
restore_a
launch_s28 "first=3,quiet=3600,tick=1"
check "S-28.2a $B ready" run_until "update: $B ready" 120
osascript -e "tell application id \"$ID\" to quit" >/dev/null 2>&1 || kill -TERM "$(app_pid)"
for _ in $(seq 1 30); do [[ -z "$(app_pid)" ]] && break; sleep 1; done
check "S-28.2b installed at quit" run_until "update: installing at quit" 5
check "S-28.2c bundle on disk is $B" test "$(bundle_version)" = "$B"
sleep 5
check "S-28.2d no relaunch after quit" test -z "$(app_pid)"
cp "$LOG" "$OUT/s28-2.log" 2>/dev/null || true

# S-28.3: three failed installs (tampered archive), then the pause.
restore_a
cp "$REPO/releases/download/v$B/Wispr-Bro_${B}_aarch64.app.tar.gz" "$OUT/b-archive.bak"
printf 'tampered' >> "$REPO/releases/download/v$B/Wispr-Bro_${B}_aarch64.app.tar.gz"
launch_s28 "first=2,quiet=1,tick=1,retry=3"
check "S-28.3a three failures pause background updates" run_until "failed installs, pausing background updates for 24 h" 180
check "S-28.3b update-state.json records 3 failures and a pause" \
  python3 -I -c 'import json,sys,time; d=json.load(open(sys.argv[1])); sys.exit(0 if d["count"]==3 and d["paused_until_ms"]>time.time()*1000 else 1)' "$DATA/update-state.json"
check "S-28.3c bundle on disk is still $A" test "$(bundle_version)" = "$A"
cp "$OUT/b-archive.bak" "$REPO/releases/download/v$B/Wispr-Bro_${B}_aarch64.app.tar.gz"
cp "$LOG" "$OUT/s28-3.log" 2>/dev/null || true

# S-28.4: a folder that needs a password never prompts in the background.
restore_a
chmod a-w "$OUT/a"
launch_s28 "first=2,quiet=1,tick=1"
check "S-28.4a $B ready" run_until "update: $B ready" 120
check "S-28.4b waits for Restart now instead of asking for a password" run_until "waiting for Restart now" 30
sleep 10
# "update: installing needs an administrator password" is the expected
# line above; an install logs "update: installing <version>".
check "S-28.4c nothing installed" not_in_log "update: installing $B"
stop_app
chmod u+w "$OUT/a"
cp "$LOG" "$OUT/s28-4.log" 2>/dev/null || true

grep -h "update:" "$OUT"/s28-*.log >> "$OUT/update-lines.log" 2>/dev/null || true
echo "passed $pass, failed $fail (logs in $OUT)"
[[ $fail == 0 ]]
