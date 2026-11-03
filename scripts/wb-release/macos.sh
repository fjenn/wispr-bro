#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Builds, signs, notarizes and staples a Wispr Bro macOS release (F-02, E-55,
# E-56, E-57). One path for this Mac and for CI (`wb-build.yml` calls it), so
# the local acceptance run exercises exactly what CI publishes.
#
#   1. identity guard in release mode (no placeholders, our pubkey and feed,
#      versions and tag agree);
#   2. `llama-server` sidecar (ci/llama-server/build.sh) signed with the
#      Developer ID and the hardened runtime before the app;
#   3. `tauri build --bundles app` with the overlay; Tauri signs the app, its
#      nested binaries and the sidecar copy with the same identity, hardened
#      runtime and src-tauri/Entitlements.plist, but without a secure
#      timestamp (Tauri 2.11 passes no `--timestamp`), which notarization
#      requires. So every Mach-O is re-signed inside-out with `--timestamp`
#      (sidecar without entitlements, the app with Entitlements.plist).
#      Tauri's own notarization is kept off (no APPLE_ID / APPLE_API_* in its
#      environment) so this script controls it;
#   4. every Mach-O in the bundle is checked for our Team ID and the hardened
#      runtime, then the app is notarized (`notarytool --wait`) and stapled;
#   5. the updater archive `Wispr-Bro_<v>_aarch64.app.tar.gz` is made from the
#      *stapled* app and signed with the updater key (`tauri signer sign`),
#      then checked against the overlay pubkey;
#   6. the DMG is made from the stapled app (hdiutil, app + /Applications
#      link), signed, notarized and stapled (RS-12: staple the DMG
#      explicitly);
#   7. `latest.json` for the release, SHA256SUMS, and scripts/wb-release/
#      verify.sh on the outputs.
#
# Usage: scripts/wb-release/macos.sh --out <dir> [--tag vX.Y.Z[-pre.N]] [--unsigned] [--reuse-app]
#   --tag       release tag (default v<overlay version>); `-beta.N` puts the
#               assets on the rolling `beta` release in latest.json
#   --unsigned  CI dry run without secrets: ad-hoc signature, no notarization,
#               no updater archive or latest.json (verify.sh --unsigned)
#   --reuse-app resume after a failed notarization: skip `tauri build` and
#               re-sign the existing bundle (never used in CI)
#
# Env (signed mode):
#   WB_SIGNING_IDENTITY  codesign identity (default: our Developer ID)
#   WB_KEYCHAIN          keychain holding it (CI: a temporary keychain)
#   notarization, one of:
#     WB_NOTARY_PROFILE  notarytool keychain profile (this Mac: wispr-bro-notary)
#     WB_NOTARY_KEY_PATH + WB_NOTARY_KEY_ID + WB_NOTARY_ISSUER  (CI: API key)
#   updater key, one of:
#     TAURI_SIGNING_PRIVATE_KEY (+ TAURI_SIGNING_PRIVATE_KEY_PASSWORD)  (CI)
#     ~/.wispr-bro/updater.key + keychain item wispr-bro-updater/updater (this Mac)
# Secrets are never printed.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEAM_ID="446XT9839S"
IDENTITY="${WB_SIGNING_IDENTITY:-Developer ID Application: Finn Jennen (${TEAM_ID})}"
PRODUCT="Wispr Bro"
ASSET_PREFIX="Wispr-Bro"
ARCH="aarch64"

OUT=""
TAG=""
UNSIGNED=0
REUSE=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --out) OUT="$2"; shift 2 ;;
    --tag) TAG="$2"; shift 2 ;;
    --unsigned) UNSIGNED=1; shift ;;
    --reuse-app) REUSE=1; shift ;;
    *) echo "macos.sh: unknown argument $1" >&2; exit 2 ;;
  esac
done
[[ -n "$OUT" ]] || { echo "usage: macos.sh --out <dir> [--tag v…] [--unsigned]" >&2; exit 2; }
[[ "$(uname -s)" == Darwin && "$(uname -m)" == arm64 ]] || {
  echo "macos.sh: Apple Silicon macOS only (DEC-29)" >&2; exit 1; }

VERSION="$(bun -e "console.log(require('${ROOT}/src-tauri/tauri.fork.conf.json').version)")"
TAG="${TAG:-v${VERSION}}"
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
LOG="${OUT}/release.log"
: > "$LOG"
step() { printf '\n== %s\n' "$*" | tee -a "$LOG"; }
die() { echo "macos.sh: $*" | tee -a "$LOG" >&2; exit 1; }

cd "$ROOT"
TAG_INFO="$(bun scripts/wb-release/tag.ts "$TAG" --tags /dev/null)" || die "bad tag $TAG"
RELEASE_TAG="$(sed -n 's/^release_tag=//p' <<<"$TAG_INFO")"
echo "version ${VERSION}, tag ${TAG}, release ${RELEASE_TAG}" | tee -a "$LOG"

step "identity guard (release mode)"
bun scripts/wb-guard-identity.ts --release --tag "$TAG" | tee -a "$LOG"

KEYCHAIN_ARGS=()
[[ -n "${WB_KEYCHAIN:-}" ]] && KEYCHAIN_ARGS=(--keychain "$WB_KEYCHAIN")

notarize() { # <file> <label>
  local file="$1" label="$2" json id status
  local auth=()
  if [[ -n "${WB_NOTARY_PROFILE:-}" ]]; then
    auth=(--keychain-profile "$WB_NOTARY_PROFILE")
  elif [[ -n "${WB_NOTARY_KEY_PATH:-}" ]]; then
    auth=(--key "$WB_NOTARY_KEY_PATH" --key-id "$WB_NOTARY_KEY_ID" --issuer "$WB_NOTARY_ISSUER")
  else
    die "no notarization credentials (WB_NOTARY_PROFILE or WB_NOTARY_KEY_PATH)"
  fi
  json="${OUT}/notary-${label}.json"
  xcrun notarytool submit "$file" "${auth[@]}" --wait --timeout 2h \
    --output-format json > "$json" || true
  id="$(bun -e "try{console.log(require('$json').id??'')}catch{console.log('')}")"
  status="$(bun -e "try{console.log(require('$json').status??'')}catch{console.log('')}")"
  echo "notarization ${label}: ${status:-no answer} (${id:-no id})" | tee -a "$LOG"
  if [[ -n "$id" ]]; then
    xcrun notarytool log "$id" "${auth[@]}" "${OUT}/notary-${label}-log.json" >/dev/null 2>&1 || true
  fi
  [[ "$status" == "Accepted" ]] || die "notarization of ${label} failed; see ${OUT}/notary-${label}-log.json"
}

step "llama-server sidecar"
ci/llama-server/build.sh --if-missing 2>&1 | tail -3 | tee -a "$LOG"
SIDECAR="$(ci/llama-server/build.sh --print-path)"
[[ -x "$SIDECAR" ]] || die "sidecar missing: $SIDECAR"
if [[ $UNSIGNED == 0 ]]; then
  codesign --force --options runtime --timestamp ${KEYCHAIN_ARGS[@]+"${KEYCHAIN_ARGS[@]}"} \
    --identifier com.wisprbro.desktop.llama-server --sign "$IDENTITY" "$SIDECAR"
  codesign --verify --strict "$SIDECAR"
  echo "sidecar signed: $(codesign -dv "$SIDECAR" 2>&1 | grep -E '^(TeamIdentifier|CodeDirectory)' | tr '\n' ' ')" | tee -a "$LOG"
fi

step "tauri build (.app)"
# Tauri notarizes on its own when these are set; this script does it instead.
unset APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID APPLE_API_KEY APPLE_API_ISSUER APPLE_API_KEY_PATH
export CMAKE_POLICY_VERSION_MINIMUM=3.5
BUILD_CONFIG=(--config src-tauri/tauri.fork.conf.json)
if [[ $UNSIGNED == 1 ]]; then
  BUILD_CONFIG+=(--config '{"bundle":{"createUpdaterArtifacts":false}}')
  unset APPLE_SIGNING_IDENTITY TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PRIVATE_KEY_PASSWORD
else
  export APPLE_SIGNING_IDENTITY="$IDENTITY"
  if [[ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]]; then
    [[ -f "$HOME/.wispr-bro/updater.key" ]] || die "no updater key (TAURI_SIGNING_PRIVATE_KEY or ~/.wispr-bro/updater.key)"
    TAURI_SIGNING_PRIVATE_KEY="$(cat "$HOME/.wispr-bro/updater.key")"
    export TAURI_SIGNING_PRIVATE_KEY
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(security find-generic-password -s wispr-bro-updater -a updater -w)" \
      || die "updater key password not in the keychain (wispr-bro-updater)"
    export TAURI_SIGNING_PRIVATE_KEY_PASSWORD
  fi
fi
APP="${ROOT}/src-tauri/target/release/bundle/macos/${PRODUCT}.app"
if [[ $REUSE == 1 && -d "$APP" ]]; then
  echo "reusing $APP" | tee -a "$LOG"
else
  bun tauri build "${BUILD_CONFIG[@]}" --bundles app 2>&1 | tee -a "${OUT}/tauri-build.log" | tail -25
fi
[[ -d "$APP" ]] || die "no app at $APP"

if [[ $UNSIGNED == 0 ]]; then
  step "re-sign inside-out with a secure timestamp"
  EXE="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$APP/Contents/Info.plist")"
  ENTITLEMENTS="${ROOT}/src-tauri/Entitlements.plist"
  while IFS= read -r -d '' f; do
    [[ "$f" == "$APP/Contents/MacOS/$EXE" ]] && continue
    grep -q 'Mach-O' <<<"$(file -b "$f")" || continue
    ident=()
    [[ "$(basename "$f")" == llama-server ]] && ident=(--identifier com.wisprbro.desktop.llama-server)
    codesign --force --options runtime --timestamp ${KEYCHAIN_ARGS[@]+"${KEYCHAIN_ARGS[@]}"} \
      ${ident[@]+"${ident[@]}"} --sign "$IDENTITY" "$f"
    echo "  signed ${f#"$APP"/}" | tee -a "$LOG"
  done < <(find "$APP/Contents" -type f -print0)
  codesign --force --options runtime --timestamp ${KEYCHAIN_ARGS[@]+"${KEYCHAIN_ARGS[@]}"} \
    --entitlements "$ENTITLEMENTS" --sign "$IDENTITY" "$APP"
  echo "  signed ${PRODUCT}.app (entitlements: $(plutil -extract com.apple.security.device.audio-input raw "$ENTITLEMENTS" >/dev/null && echo microphone))" | tee -a "$LOG"
fi

if [[ $UNSIGNED == 1 ]]; then
  step "unsigned dry run: DMG only"
  DMG="${OUT}/${ASSET_PREFIX}_${VERSION}_${ARCH}.dmg"
  STAGE="$(mktemp -d)"
  ditto "$APP" "${STAGE}/${PRODUCT}.app"
  ln -s /Applications "${STAGE}/Applications"
  hdiutil create -volname "$PRODUCT" -srcfolder "$STAGE" -fs HFS+ -format UDZO -ov "$DMG" >/dev/null
  rm -rf "$STAGE"
  scripts/wb-release/verify.sh --dmg "$DMG" --unsigned | tee -a "$LOG"
  exit 0
fi

step "nested code: Team ID, hardened runtime, secure timestamp"
bad=0
while IFS= read -r -d '' f; do
  if grep -q 'Mach-O' <<<"$(file -b "$f")"; then
    info="$(codesign -dv --verbose=2 "$f" 2>&1 || true)"
    team="$(sed -n 's/^TeamIdentifier=//p' <<<"$info")"
    if [[ "$team" != "$TEAM_ID" ]] || ! grep -q 'flags=.*runtime' <<<"$info" || ! grep -q '^Timestamp=' <<<"$info"; then
      echo "  ✗ ${f#"$APP"/}: team=${team:-none}, $(grep -o 'flags=[^ ]*' <<<"$info" || echo 'unsigned')" | tee -a "$LOG"
      bad=1
    else
      echo "  ✓ ${f#"$APP"/}" | tee -a "$LOG"
    fi
  fi
done < <(find "$APP/Contents" -type f -print0)
[[ $bad == 0 ]] || die "nested code not signed with ${TEAM_ID} + hardened runtime + timestamp"
codesign --verify --deep --strict --verbose=2 "$APP" 2>&1 | tee -a "$LOG"

step "notarize + staple the app"
ZIP="$(mktemp -d)/app.zip"
ditto -c -k --keepParent "$APP" "$ZIP"
notarize "$ZIP" app
rm -f "$ZIP"
xcrun stapler staple "$APP" | tee -a "$LOG"

step "updater archive from the stapled app"
TARBALL="${OUT}/${ASSET_PREFIX}_${VERSION}_${ARCH}.app.tar.gz"
rm -f "$TARBALL" "${TARBALL}.sig"
COPYFILE_DISABLE=1 tar -czf "$TARBALL" -C "$(dirname "$APP")" "${PRODUCT}.app"
bun tauri signer sign "$TARBALL" > /dev/null
[[ -s "${TARBALL}.sig" ]] || die "updater signature not written"
bun scripts/wb-release/minisign.ts "$TARBALL" | tee -a "$LOG"

step "DMG from the stapled app"
DMG="${OUT}/${ASSET_PREFIX}_${VERSION}_${ARCH}.dmg"
STAGE="$(mktemp -d)"
ditto "$APP" "${STAGE}/${PRODUCT}.app"
ln -s /Applications "${STAGE}/Applications"
rm -f "$DMG"
hdiutil create -volname "$PRODUCT" -srcfolder "$STAGE" -fs HFS+ -format UDZO -ov "$DMG" >/dev/null
rm -rf "$STAGE"
codesign --force --timestamp ${KEYCHAIN_ARGS[@]+"${KEYCHAIN_ARGS[@]}"} --sign "$IDENTITY" "$DMG"
notarize "$DMG" dmg
xcrun stapler staple "$DMG" | tee -a "$LOG"

step "latest.json + SHA256SUMS"
bun scripts/wb-release/manifest.ts --version "$VERSION" --release-tag "$RELEASE_TAG" \
  --asset "darwin-${ARCH}=${TARBALL}" --out "${OUT}/latest.json" | tee -a "$LOG"
( cd "$OUT" && shasum -a 256 "$(basename "$DMG")" "$(basename "$TARBALL")" \
  "$(basename "$TARBALL").sig" latest.json > SHA256SUMS )

step "verify"
scripts/wb-release/verify.sh --dmg "$DMG" --updater "$TARBALL" --version "$VERSION" | tee -a "$LOG"
echo "release artifacts in ${OUT}" | tee -a "$LOG"
