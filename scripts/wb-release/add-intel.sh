#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Adds the Intel build to a release that is already published with only the
# Apple Silicon one. Nothing of the Apple Silicon release changes: its DMG,
# updater archive and .sig stay as they are, and its entries in latest.json
# keep every value (scripts/wb-release/add-platform.ts checks this).
#
#   1. builds, signs, notarizes and staples the Intel app and DMG at the
#      release's version (macos.sh --arch x86_64; this checkout's version
#      must be the release's),
#   2. adds darwin-x86_64 to the published latest.json and the Intel files
#      to SHA256SUMS (the Apple Silicon lines stay; only latest.json's own
#      line changes, since that file changed),
#   3. uploads the Intel DMG, archive and .sig, then the new latest.json and
#      SHA256SUMS (gh, signed in to an account that can edit fjenn/wispr-bro),
#   4. reads the release back and checks it.
#
# Usage: scripts/wb-release/add-intel.sh v1.0.0-alpha.6
# Env: as macos.sh (WB_NOTARY_PROFILE=wispr-bro-notary on this Mac).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
REPO="fjenn/wispr-bro"
TAG="${1:-}"
[[ "$TAG" == v* ]] || { echo "usage: add-intel.sh v<version>" >&2; exit 2; }
VERSION="${TAG#v}"
BASE="https://github.com/${REPO}/releases/download/${TAG}"
OUT="${HOME}/wb-release/${VERSION}-x86_64"
die() { echo "add-intel.sh: $*" >&2; exit 1; }

cd "$ROOT"
command -v gh >/dev/null || die "needs the GitHub CLI: brew install gh, then gh auth login"
gh auth status >/dev/null 2>&1 || die "gh is not signed in: gh auth login"
HERE="$(bun -e "console.log(require('./src-tauri/tauri.fork.conf.json').version)")"
[[ "$HERE" == "$VERSION" ]] || die "this checkout is ${HERE}, the release is ${VERSION}"
[[ -z "$(git status --porcelain)" ]] || die "uncommitted changes; the build must match a pushed commit"
COMMIT="$(git rev-parse HEAD)"
git branch -r --contains "$COMMIT" | grep -q . || die "commit ${COMMIT} is not pushed (its source must be public)"
rustup target list --installed | grep -qx x86_64-apple-darwin || rustup target add x86_64-apple-darwin

PUB="${OUT}/published"
mkdir -p "$PUB"
curl -fsSL -o "${PUB}/latest.json" "${BASE}/latest.json" || die "no latest.json on ${TAG}"
curl -fsSL -o "${PUB}/SHA256SUMS" "${BASE}/SHA256SUMS" || die "no SHA256SUMS on ${TAG}"
grep -q '"darwin-x86_64"' "${PUB}/latest.json" && die "${TAG} already has an Intel entry"
# The published latest.json is the one SHA256SUMS describes.
( cd "$PUB" && grep ' latest.json$' SHA256SUMS | shasum -a 256 -c - ) >/dev/null \
  || die "the published latest.json does not match its SHA256SUMS"

echo "== Intel build of ${VERSION} from ${COMMIT}"
scripts/wb-release/macos.sh --arch x86_64 --tag "$TAG" --out "$OUT"

DMG="${OUT}/Wispr-Bro_${VERSION}_x86_64.dmg"
TARBALL="${OUT}/Wispr-Bro_${VERSION}_x86_64.app.tar.gz"
for f in "$DMG" "$TARBALL" "${TARBALL}.sig" "${OUT}/latest.json"; do
  [[ -f "$f" ]] || die "missing $f"
done

NEW="${OUT}/upload"
rm -rf "$NEW"; mkdir -p "$NEW"
bun scripts/wb-release/add-platform.ts --published "${PUB}/latest.json" \
  --add "${OUT}/latest.json" --asset "$TARBALL" --out "${NEW}/latest.json"
{
  grep -v ' latest.json$' "${PUB}/SHA256SUMS"
  ( cd "$OUT" && shasum -a 256 "$(basename "$DMG")" "$(basename "$TARBALL")" "$(basename "$TARBALL").sig" )
  ( cd "$NEW" && shasum -a 256 latest.json )
} > "${NEW}/SHA256SUMS"

echo "== upload to ${TAG}"
# New files first (never overwriting), the feed last, so no install ever sees
# an Intel entry whose file is not there yet.
gh release upload "$TAG" -R "$REPO" "$DMG" "$TARBALL" "${TARBALL}.sig"
gh release upload "$TAG" -R "$REPO" --clobber "${NEW}/latest.json" "${NEW}/SHA256SUMS"

echo "== check the release"
CHECK="${OUT}/check"
rm -rf "$CHECK"; mkdir -p "$CHECK"
# GitHub's download CDN can serve the old file for a little while.
for i in $(seq 1 30); do
  curl -fsSL -o "${CHECK}/latest.json" "${BASE}/latest.json?t=$(date +%s)" || true
  cmp -s "${CHECK}/latest.json" "${NEW}/latest.json" && break
  [[ $i == 30 ]] && die "the release still serves the old latest.json after 5 min"
  sleep 10
done
bun -e "
const a = require('${PUB}/latest.json'), b = require('${CHECK}/latest.json');
for (const k of Object.keys(a.platforms))
  if (JSON.stringify(a.platforms[k]) !== JSON.stringify(b.platforms[k])) throw new Error(k + ' changed');
if (!b.platforms['darwin-x86_64'] || !b.platforms['darwin-x86_64-app']) throw new Error('no Intel entry');
console.log('latest.json: Apple Silicon entries unchanged, Intel added');
"
for f in "$(basename "$DMG")" "$(basename "$TARBALL")" "$(basename "$TARBALL").sig" \
         "Wispr-Bro_${VERSION}_aarch64.dmg" "Wispr-Bro_${VERSION}_aarch64.app.tar.gz"; do
  curl -fsSIL -o /dev/null "${BASE}/${f}" || die "${f} is not downloadable"
done
echo "done: ${TAG} now has the Intel build (${COMMIT})"
