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
# Real permission survival and the footer/tray paths stay RS-12 person checks.
#
# Usage: scripts/wb-f02/update-e2e.sh <out dir>
set -euo pipefail

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
    nohup "$APP/Contents/MacOS/$EXE" --start-hidden > "$OUT/app-stdout.log" 2>&1 &
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

launch
check "F-02.3a stable install ignores the newer beta ($C)" run_until "fork update selftest: UP-TO-DATE" 120
stop_app
: > "$LOG"

manifest "$B" "v$B" "$REPO/releases/download/v$B/Wispr-Bro_${B}_aarch64.app.tar.gz" \
  "$REPO/releases/latest/download/latest.json"
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
echo "passed $pass, failed $fail (logs in $OUT)"
[[ $fail == 0 ]]
