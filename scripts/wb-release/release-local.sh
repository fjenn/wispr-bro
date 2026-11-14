#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Cuts a full Wispr Bro release on the owner's Apple Silicon Mac, for both
# chips, and publishes it as the latest release (the same assets a release
# from wb-release.yml has for macOS):
#
#   1. checks: gh signed in, the checkout's version is the tag's, no local
#      changes, the commit is pushed (its source must be public), no release
#      or tag with that name yet;
#   2. builds, signs, notarizes and staples the Apple Silicon and the Intel
#      app and DMG (macos.sh --arch aarch64, then --arch x86_64);
#   3. one latest.json with both chips (manifest.ts checks every .sig against
#      the overlay pubkey) and one SHA256SUMS for every asset;
#   4. creates the release as a draft on that commit with every file, and
#      only then publishes it as the latest release, so the update feed and
#      the download links switch in one step and never point at a missing
#      file;
#   5. reads the release back: latest.json from the stable feed URL with
#      both chips, every asset downloadable.
#
# Release notes come from src/fork/whats-new/<version>.md.
#
# Usage: scripts/wb-release/release-local.sh v1.0.0-alpha.7
# Env: as macos.sh (WB_NOTARY_PROFILE=wispr-bro-notary on the owner's Mac).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
REPO="fjenn/wispr-bro"
TAG="${1:-}"
[[ "$TAG" == v* ]] || { echo "usage: release-local.sh v<version>" >&2; exit 2; }
VERSION="${TAG#v}"
OUT="${HOME}/wb-release/${VERSION}"
FEED="https://github.com/${REPO}/releases/latest/download/latest.json"
BASE="https://github.com/${REPO}/releases/download/${TAG}"
die() { echo "release-local.sh: $*" >&2; exit 1; }

cd "$ROOT"
[[ "$(uname -s)" == Darwin && "$(uname -m)" == arm64 ]] || die "run this on the Apple Silicon Mac"
command -v gh >/dev/null || die "needs the GitHub CLI: brew install gh, then gh auth login"
gh auth status >/dev/null 2>&1 || die "gh is not signed in: gh auth login"
HERE="$(bun -e "console.log(require('./src-tauri/tauri.fork.conf.json').version)")"
[[ "$HERE" == "$VERSION" ]] || die "this checkout is ${HERE}, not ${VERSION}"
[[ -z "$(git status --porcelain)" ]] || die "uncommitted changes; the build must match a pushed commit"
COMMIT="$(git rev-parse HEAD)"
git fetch -q origin
git branch -r --contains "$COMMIT" | grep -q . || die "commit ${COMMIT} is not pushed (its source must be public)"
NOTES_MD="src/fork/whats-new/${VERSION}.md"
[[ -f "$NOTES_MD" ]] || die "no release notes at ${NOTES_MD}"
if gh release view "$TAG" -R "$REPO" >/dev/null 2>&1; then
  die "${TAG} already exists on GitHub"
fi
if git ls-remote --exit-code --tags origin "refs/tags/${TAG}" >/dev/null 2>&1; then
  die "tag ${TAG} already exists on GitHub"
fi
rustup target list --installed | grep -qx x86_64-apple-darwin || rustup target add x86_64-apple-darwin

for arch in aarch64 x86_64; do
  echo "== ${arch} build of ${VERSION} from ${COMMIT}"
  scripts/wb-release/macos.sh --arch "$arch" --tag "$TAG" --out "${OUT}/${arch}"
done

UP="${OUT}/upload"
rm -rf "$UP"; mkdir -p "$UP"
for arch in aarch64 x86_64; do
  for f in "Wispr-Bro_${VERSION}_${arch}.dmg" "Wispr-Bro_${VERSION}_${arch}.app.tar.gz" \
           "Wispr-Bro_${VERSION}_${arch}.app.tar.gz.sig"; do
    [[ -f "${OUT}/${arch}/${f}" ]] || die "missing ${OUT}/${arch}/${f}"
    cp "${OUT}/${arch}/${f}" "${UP}/${f}"
  done
done
bun scripts/wb-release/manifest.ts --version "$VERSION" --release-tag "$TAG" \
  --asset "darwin-aarch64=${UP}/Wispr-Bro_${VERSION}_aarch64.app.tar.gz" \
  --asset "darwin-x86_64=${UP}/Wispr-Bro_${VERSION}_x86_64.app.tar.gz" \
  --out "${UP}/latest.json"
( cd "$UP" && shasum -a 256 \
    "Wispr-Bro_${VERSION}_aarch64.dmg" "Wispr-Bro_${VERSION}_aarch64.app.tar.gz" \
    "Wispr-Bro_${VERSION}_aarch64.app.tar.gz.sig" \
    "Wispr-Bro_${VERSION}_x86_64.dmg" "Wispr-Bro_${VERSION}_x86_64.app.tar.gz" \
    "Wispr-Bro_${VERSION}_x86_64.app.tar.gz.sig" latest.json > SHA256SUMS )

NOTES="${OUT}/notes.md"
{
  # The note's own heading is the release title, so the body starts below it.
  grep -v '^## ' "$NOTES_MD" | sed '/./,$!d'
  echo
  echo "Requirements: macOS 13 or newer, on Apple Silicon or Intel. Pick the DMG for your chip: aarch64 is Apple Silicon (M1 or later), x86_64 is Intel."
  echo
  echo "Install: open the DMG and drag Wispr Bro to Applications. Already on an alpha? It updates itself in the background. Signed with a Developer ID and notarized by Apple."
} > "$NOTES"

echo "== release ${TAG} (draft first, published once every file is up)"
gh release create "$TAG" -R "$REPO" --target "$COMMIT" --draft \
  --title "Wispr Bro ${VERSION}" --notes-file "$NOTES" \
  "${UP}/Wispr-Bro_${VERSION}_aarch64.dmg" "${UP}/Wispr-Bro_${VERSION}_aarch64.app.tar.gz" \
  "${UP}/Wispr-Bro_${VERSION}_aarch64.app.tar.gz.sig" \
  "${UP}/Wispr-Bro_${VERSION}_x86_64.dmg" "${UP}/Wispr-Bro_${VERSION}_x86_64.app.tar.gz" \
  "${UP}/Wispr-Bro_${VERSION}_x86_64.app.tar.gz.sig" \
  "${UP}/latest.json" "${UP}/SHA256SUMS"
gh release edit "$TAG" -R "$REPO" --draft=false --latest

echo "== check the release"
CHECK="${OUT}/check"
rm -rf "$CHECK"; mkdir -p "$CHECK"
# GitHub's download CDN can serve the previous feed for a little while.
for i in $(seq 1 30); do
  curl -fsSL -o "${CHECK}/latest.json" "${FEED}?t=$(date +%s)" || true
  cmp -s "${CHECK}/latest.json" "${UP}/latest.json" && break
  [[ $i == 30 ]] && die "the stable feed still serves the old latest.json after 5 min"
  sleep 10
done
bun -e "
const m = require('${CHECK}/latest.json');
if (m.version !== '${VERSION}') throw new Error('feed version ' + m.version);
for (const k of ['darwin-aarch64', 'darwin-aarch64-app', 'darwin-x86_64', 'darwin-x86_64-app'])
  if (!m.platforms[k]) throw new Error('no ' + k + ' in the feed');
console.log('latest.json: ${VERSION} for Apple Silicon and Intel');
"
for f in latest.json SHA256SUMS \
         "Wispr-Bro_${VERSION}_aarch64.dmg" "Wispr-Bro_${VERSION}_aarch64.app.tar.gz" \
         "Wispr-Bro_${VERSION}_x86_64.dmg" "Wispr-Bro_${VERSION}_x86_64.app.tar.gz"; do
  curl -fsSIL -o /dev/null "${BASE}/${f}" || die "${f} is not downloadable"
done
echo "done: ${TAG} is the latest release, for Apple Silicon and Intel (${COMMIT})"
