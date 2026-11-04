#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# wb-verify (F-02, E-214): checks release artifacts the way a user's Mac will.
# Run by scripts/wb-release/macos.sh and, on a fresh runner, by wb-build.yml's
# `verify` job.
#
#   DMG:     signed by our Team ID, notarized and stapled
#            (`spctl -t open --context context:primary-signature`,
#            `stapler validate`); the install window's background (1x and
#            2x), hidden toolbar/sidebar, window and icon size and icon
#            positions (`dmg/check_layout.py`, against bundle.macOS.dmg)
#   app:     (inside the DMG) `codesign --verify --deep --strict`, identifier,
#            version, minimum macOS, hardened runtime, Team ID on every Mach-O
#            incl. the `llama-server` sidecar, `spctl -a -t exec` (Notarized
#            Developer ID), `stapler validate`
#   updater: `.app.tar.gz` signature against the overlay pubkey; the archive
#            holds `Wispr Bro.app`, itself stapled and valid
#   smoke:   `<app> --list-devices` (headless, own throwaway HOME, so nothing
#            is written to the real ~/Library)
#
# Usage: scripts/wb-release/verify.sh --dmg <dmg> [--updater <tar.gz>]
#                                     [--version V] [--unsigned]
# --unsigned: dry-run artifacts (ad-hoc): only structure, identity and smoke.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEAM_ID="446XT9839S"
IDENTIFIER="com.wisprbro.desktop"
PRODUCT="Wispr Bro"
DMG=""; UPDATER=""; VERSION=""; UNSIGNED=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dmg) DMG="$2"; shift 2 ;;
    --updater) UPDATER="$2"; shift 2 ;;
    --version) VERSION="$2"; shift 2 ;;
    --unsigned) UNSIGNED=1; shift ;;
    *) echo "verify.sh: unknown argument $1" >&2; exit 2 ;;
  esac
done
[[ -f "$DMG" ]] || { echo "usage: verify.sh --dmg <dmg> [--updater <tar.gz>] [--version V] [--unsigned]" >&2; exit 2; }
VERSION="${VERSION:-$(bun -e "console.log(require('${ROOT}/src-tauri/tauri.fork.conf.json').version)")}"

fails=0
ok() { echo "  ✓ $*"; }
ko() { echo "  ✗ $*"; fails=$((fails + 1)); }
check() { # <label> <command…>
  local label="$1"; shift
  local out
  if out="$("$@" 2>&1)"; then ok "$label"; else ko "$label: $(tail -3 <<<"$out" | tr '\n' ' ')"; fi
}

WORK="$(mktemp -d)"
MNT="${WORK}/mnt"
cleanup() {
  hdiutil detach "$MNT" -quiet >/dev/null 2>&1 || true
  rm -rf "$WORK"
}
trap cleanup EXIT

echo "== DMG $(basename "$DMG")"
if [[ $UNSIGNED == 0 ]]; then
  check "DMG signature valid" codesign --verify --strict "$DMG"
  dmg_team="$(codesign -dv "$DMG" 2>&1 | sed -n 's/^TeamIdentifier=//p')"
  [[ "$dmg_team" == "$TEAM_ID" ]] && ok "DMG Team ID ${TEAM_ID}" || ko "DMG Team ID ${dmg_team:-none}"
  check "DMG Gatekeeper (spctl -t open, primary signature)" \
    spctl -a -vvv -t open --context context:primary-signature "$DMG"
  check "DMG stapled" xcrun stapler validate "$DMG"
fi
mkdir -p "$MNT"
hdiutil attach "$DMG" -nobrowse -readonly -noautoopen -mountpoint "$MNT" -quiet
APP="${MNT}/${PRODUCT}.app"
[[ -d "$APP" ]] && ok "DMG holds ${PRODUCT}.app" || { ko "no ${PRODUCT}.app in the DMG"; exit 1; }
[[ -L "${MNT}/Applications" ]] && ok "DMG has the Applications link" || ko "no Applications link"
# The install window (OT2-E): background picture (1x + 2x), toolbar and
# sidebar hidden, window size, icon size and both icon positions, read from
# the volume's .DS_Store against the overlay's bundle.macOS.dmg.
echo "== DMG install window"
# shellcheck source=dmg/build.sh
DMG_PY="$(source "${ROOT}/scripts/wb-release/dmg/build.sh"; wb_dmg_python)"
if layout="$("$DMG_PY" -I "${ROOT}/scripts/wb-release/dmg/check_layout.py" "$MNT" "${ROOT}/src-tauri/tauri.fork.conf.json" 2>&1)"; then
  echo "$layout"
else
  echo "$layout"
  n="$(grep -c '✗' <<<"$layout" || true)"
  fails=$((fails + (n > 0 ? n : 1)))
fi

verify_app() { # <app> <label>
  local app="$1" label="$2" plist="$1/Contents/Info.plist"
  echo "== app (${label})"
  local id ver minos exe
  id="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$plist")"
  ver="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$plist")"
  minos="$(/usr/libexec/PlistBuddy -c 'Print :LSMinimumSystemVersion' "$plist")"
  exe="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$plist")"
  [[ "$id" == "$IDENTIFIER" ]] && ok "identifier $id" || ko "identifier $id"
  [[ "$ver" == "$VERSION" ]] && ok "version $ver" || ko "version $ver ≠ $VERSION"
  [[ "$minos" == "13.0" ]] && ok "minimum macOS $minos" || ko "minimum macOS $minos"
  [[ -x "${app}/Contents/MacOS/llama-server" ]] && ok "llama-server sidecar present" || ko "llama-server missing"
  APP_EXE="${app}/Contents/MacOS/${exe}"
  if [[ $UNSIGNED == 1 ]]; then
    check "ad-hoc signature valid (deep, strict)" codesign --verify --deep --strict "$app"
    return
  fi
  check "codesign --verify --deep --strict" codesign --verify --deep --strict --verbose=2 "$app"
  local f info team nested=0 nested_bad=0
  while IFS= read -r -d '' f; do
    grep -q 'Mach-O' <<<"$(file -b "$f")" || continue
    nested=$((nested + 1))
    info="$(codesign -dv --verbose=2 "$f" 2>&1 || true)"
    team="$(sed -n 's/^TeamIdentifier=//p' <<<"$info")"
    if [[ "$team" != "$TEAM_ID" ]] || ! grep -q 'flags=.*runtime' <<<"$info"; then
      ko "${f#"$app"/}: team=${team:-none} $(grep -o 'flags=[^ ]*' <<<"$info" || echo unsigned)"
      nested_bad=1
    fi
  done < <(find "${app}/Contents" -type f -print0)
  [[ $nested_bad == 0 ]] && ok "${nested} Mach-O file(s) signed by ${TEAM_ID} with the hardened runtime"
  local stamped
  for stamped in "${app}" "${app}/Contents/MacOS/${exe}" "${app}/Contents/MacOS/llama-server"; do
    # Capture first: with pipefail, `grep -q` closing the pipe early can fail
    # codesign with SIGPIPE and report a stamped binary as unstamped.
    if grep -q '^Timestamp=' <<<"$(codesign -dv --verbose=2 "$stamped" 2>&1 || true)"; then
      ok "secure timestamp: ${stamped#"$app"}"
    else
      ko "no secure timestamp: ${stamped#"$app"}"
    fi
  done
  if grep -q 'com.apple.security.device.audio-input' <<<"$(codesign -d --entitlements - --xml "$app" 2>/dev/null || true)"; then
    ok "app entitlements include the microphone"
  else
    ko "app entitlements lack com.apple.security.device.audio-input"
  fi
  local gk
  gk="$(spctl -a -vvv -t exec "$app" 2>&1 || true)"
  if grep -q 'accepted' <<<"$gk" && grep -q 'source=Notarized Developer ID' <<<"$gk"; then
    ok "Gatekeeper: accepted, Notarized Developer ID"
  else
    ko "Gatekeeper: $(tr '\n' ' ' <<<"$gk")"
  fi
  check "app stapled" xcrun stapler validate "$app"
  APP_EXE="${app}/Contents/MacOS/${exe}"
}

verify_app "$APP" "in the DMG"

echo "== smoke"
SMOKE_HOME="${WORK}/home"
mkdir -p "$SMOKE_HOME"
if out="$(HOME="$SMOKE_HOME" "$APP_EXE" --list-devices 2>"${WORK}/smoke.err")"; then
  ok "--list-devices: $(head -3 <<<"$out" | tr '\n' ' ')"
else
  ko "--list-devices failed: $(tail -3 "${WORK}/smoke.err" | tr '\n' ' ')"
fi

if [[ -n "$UPDATER" ]]; then
  echo "== updater $(basename "$UPDATER")"
  if out="$(bun "${ROOT}/scripts/wb-release/minisign.ts" "$UPDATER" 2>&1)"; then
    ok "${out#✓ }"
  else
    ko "$out"
  fi
  top="$(tar -tzf "$UPDATER" | cut -d/ -f1 | sort -u)"
  [[ "$top" == "${PRODUCT}.app" ]] && ok "archive holds only ${PRODUCT}.app" || ko "archive top level: $top"
  mkdir -p "${WORK}/x"
  tar -xzf "$UPDATER" -C "${WORK}/x"
  verify_app "${WORK}/x/${PRODUCT}.app" "from the updater archive"
fi

echo
if [[ $fails -gt 0 ]]; then
  echo "verify: ${fails} check(s) FAILED"
  exit 1
fi
echo "verify: all checks passed"
