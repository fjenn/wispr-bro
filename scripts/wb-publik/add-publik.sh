#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Lists Wispr Bro on publik and adds the publik edition (docs/fork/EDITIONS.md)
# to a release that is already published with the normal app, for Apple
# Silicon and Intel. Runs on the owner's Apple Silicon Mac, from a checkout of
# the publik-edition branch. Nothing of the normal release changes: its DMGs,
# updater archives, .sig files, latest.json and SHA256SUMS stay as they are
# (checked before and after); the publik files only add new names next to
# them (Wispr-Bro-publik_*, latest-publik.json, SHA256SUMS-publik).
#
#   1. publik sign-in (device flow): opens publik's approval page in the
#      browser; the owner checks the code and clicks Approve;
#   2. the listing: publik derives it from the repository, this script sets
#      our name, summary and the publik downloads for both chips, and submits
#      it (publik answers live or held for review, with the listing's slug);
#   3. the app token for that slug: kept in the login keychain
#      (wispr-bro-publik-app-token) and set as the GitHub Actions secret
#      PUBLIK_APP_TOKEN, never written to the repository or printed;
#   4. if publik chose another slug than fork::publik::APP_SLUG, commits and
#      pushes that one line to publik-edition (the build must match public
#      source);
#   5. builds, signs, notarizes and staples the publik app and DMG for both
#      chips (macos.sh --edition publik), then runs publik's release self test
#      on each (Intel under Rosetta);
#   6. one latest-publik.json for both chips, SHA256SUMS-publik and the
#      publik edition's source archive (its commit is on publik-edition, not
#      the release tag);
#   7. uploads the new files only (never --clobber), after checking that
#      opena.tech's download links match the normal file names only;
#   8. reads the release back: the normal files unchanged, the publik files
#      downloadable.
#
# Re-running resumes: a listing and token already made are reused (the slug
# in ~/.wispr-bro/publik/slug, the token in the keychain), and a chip whose
# signed build for this commit is done is not built again.
#
# Usage: scripts/wb-publik/add-publik.sh v1.0.0-alpha.7
# Env: as macos.sh (WB_NOTARY_PROFILE=wispr-bro-notary on this Mac).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
REPO="fjenn/wispr-bro"
BRANCH="publik-edition"
API="https://publikhq.com"
TAG="${1:-}"
[[ "$TAG" == v* ]] || { echo "usage: add-publik.sh v<version>" >&2; exit 2; }
VERSION="${TAG#v}"
BASE="https://github.com/${REPO}/releases/download/${TAG}"
OUT="${HOME}/wb-release/${VERSION}-publik"
STATE="${HOME}/.wispr-bro/publik"
KEYCHAIN_ITEM="wispr-bro-publik-app-token"
MOD_RS="src-tauri/src/fork/publik/mod.rs"
P="Wispr-Bro-publik_${VERSION}"
die() { echo "add-publik.sh: $*" >&2; exit 1; }
say() { echo "== $*"; }

cd "$ROOT"
[[ "$(uname -s)" == Darwin && "$(uname -m)" == arm64 ]] || die "run this on the Apple Silicon Mac"
command -v gh >/dev/null || die "needs the GitHub CLI: brew install gh, then gh auth login"
gh auth status >/dev/null 2>&1 || die "gh is not signed in: gh auth login"
[[ "$(git rev-parse --abbrev-ref HEAD)" == "$BRANCH" ]] || die "check out ${BRANCH} first"
HERE="$(bun -e "console.log(require('./src-tauri/tauri.fork.conf.json').version)")"
[[ "$HERE" == "$VERSION" ]] || die "this checkout is ${HERE}, the release is ${VERSION}"
[[ -z "$(git status --porcelain)" ]] || die "uncommitted changes; the build must match a pushed commit"
gh release view "$TAG" -R "$REPO" --json isDraft --jq '.isDraft' | grep -qx false \
  || die "${TAG} is not a published release"
if gh release view "$TAG" -R "$REPO" --json assets --jq '.assets[].name' | grep -q '^Wispr-Bro-publik_\|-publik'; then
  die "${TAG} already has publik files; nothing to add"
fi
rustup target list --installed | grep -qx x86_64-apple-darwin || rustup target add x86_64-apple-darwin
mkdir -p "$STATE" "$OUT"
chmod 700 "$STATE"

# The normal release as it is now; checked unchanged at the end.
PUB="${OUT}/published"
rm -rf "$PUB"; mkdir -p "$PUB"
curl -fsSL -o "${PUB}/latest.json" "${BASE}/latest.json" || die "no latest.json on ${TAG}"
curl -fsSL -o "${PUB}/SHA256SUMS" "${BASE}/SHA256SUMS" || die "no SHA256SUMS on ${TAG}"
gh release view "$TAG" -R "$REPO" --json assets --jq '.assets[] | .name + " " + (.size|tostring)' \
  | sort > "${PUB}/assets.txt"

# ---------------------------------------------------------------------------
# 1-4. Listing and app token (skipped when both exist already)

token_from_keychain() { security find-generic-password -s "$KEYCHAIN_ITEM" -a wispr-bro -w 2>/dev/null || true; }

TMP="$(mktemp -d)"
chmod 700 "$TMP"
trap 'rm -rf "$TMP"' EXIT
# Reads a JSON file and prints one field ('' when missing).
field() { bun -e "let j; try { j=JSON.parse(require('fs').readFileSync(process.argv[1],'utf8')) } catch { j={} } const v=process.argv[2].split('.').reduce((o,k)=>o==null?undefined:o[k], j); console.log(v==null?'':typeof v==='object'?JSON.stringify(v):String(v))" "$1" "$2"; }
# POST JSON; writes the body to $2, prints the HTTP status. Auth header file in $4 (optional).
post() {
  local url="$1" out="$2" body="$3" hdr="${4:-}"
  local args=(-sS -o "$out" -w '%{http_code}' -X POST -H 'Content-Type: application/json' --data-binary "$body")
  [[ -n "$hdr" ]] && args+=(-H "@${hdr}")
  curl "${args[@]}" "$url" || echo 000
}

SLUG="$(cat "${STATE}/slug" 2>/dev/null || true)"
TOKEN="$(token_from_keychain)"
if [[ -n "$SLUG" && "$TOKEN" == pat_* ]]; then
  say "listing ${SLUG} and its app token are already made (re-run)"
else
  say "publik: derive the listing from ${REPO}"
  code="$(post "${API}/api/submit/derive" "${TMP}/derive.json" "{\"repo\":\"${REPO}\"}")"
  LISTED=""
  if [[ "$(field "${TMP}/derive.json" error)" == repo_already_listed ]]; then
    LISTED="$(field "${TMP}/derive.json" slug)"
    echo "already listed on publik as ${LISTED}"
  elif [[ "$code" != 200 || -z "$(field "${TMP}/derive.json" manifest)" ]]; then
    cat "${TMP}/derive.json" >&2 || true
    die "publik could not derive the listing (HTTP ${code})"
  fi

  say "publik: sign in (approve in your browser)"
  code="$(post "${API}/api/v1/device/code" "${TMP}/code.json" '{"client_name":"Wispr Bro release script"}')"
  [[ "$code" == 200 ]] || { cat "${TMP}/code.json" >&2; die "publik sign-in did not start (HTTP ${code})"; }
  DEVICE_CODE="$(field "${TMP}/code.json" device_code)"
  USER_CODE="$(field "${TMP}/code.json" user_code)"
  VERIFY_URL="$(field "${TMP}/code.json" verification_uri_complete)"
  [[ -n "$DEVICE_CODE" && "$VERIFY_URL" == https://publikhq.com/* ]] || die "publik's sign-in answer is not what this script expects"
  echo
  echo "   Your browser opens publik. Check that it shows the code ${USER_CODE},"
  echo "   then click Approve (sign in with GitHub if it asks)."
  echo "   If no browser opens, open: ${VERIFY_URL}"
  echo
  open "$VERIFY_URL" || true
  ACCESS=""
  interval=5
  for _ in $(seq 1 180); do
    sleep "$interval"
    post "${API}/api/v1/device/token" "${TMP}/token.json" "{\"device_code\":\"${DEVICE_CODE}\"}" >/dev/null
    ACCESS="$(field "${TMP}/token.json" access_token)"
    [[ -n "$ACCESS" ]] && break
    case "$(field "${TMP}/token.json" error)" in
      authorization_pending|temporarily_unavailable|'') ;;
      slow_down) interval=$((interval + 5)) ;;
      expired_token) die "the publik approval ran out; run the line again" ;;
      access_denied) die "publik sign-in was not approved" ;;
      *) die "publik sign-in failed: $(field "${TMP}/token.json" error)" ;;
    esac
  done
  [[ -n "$ACCESS" ]] || die "no approval after 15 minutes; run the line again"
  # The device token stays in this header file (mode 600, deleted on exit), never printed.
  ( umask 077; printf 'Authorization: Bearer %s\n' "$ACCESS" > "${TMP}/auth.hdr" )
  unset ACCESS
  echo "approved"

  if [[ -n "$LISTED" ]]; then
    SLUG="$LISTED"
  else
    say "publik: the listing"
    # publik's own derived listing, with our wording and the publik downloads
    # for both chips in the shape publik derived (its file and platform
    # fields kept, normal file names swapped for the publik ones).
    bun -e '
const [derived, out, version] = process.argv.slice(1);
const m = require(derived).manifest;
m.name = "Wispr Bro";
m.summary = "Hold a key, speak, and clean text lands in any app. Runs on your Mac.";
m.alternativeTo = "Wispr Flow";
m.license = "GPL-3.0-or-later";
if (m.publikApi && typeof m.publikApi === "object") m.publikApi.enabled = false;
const pub = (x) => JSON.parse(JSON.stringify(x)
  .replace(/Wispr-Bro_/g, "Wispr-Bro-publik_")
  .replace(/Wispr-Bro-publik_[0-9][^"_]*_/g, `Wispr-Bro-publik_${version}_`));
const toIntel = (x) => JSON.parse(JSON.stringify(x)
  .replace(/aarch64/g, "x86_64").replace(/arm64/g, "x64")
  .replace(/Apple Silicon/gi, "Intel"));
const isMac = (a) => /dmg|mac|darwin|aarch64|x86_64|arm64/i.test(JSON.stringify(a));
let arts = Array.isArray(m.artifacts) ? m.artifacts.filter(isMac).map(pub) : [];
arts = arts.filter((a) => /Wispr-Bro-publik_/.test(JSON.stringify(a)) && /\.dmg/.test(JSON.stringify(a)));
const has = (re) => arts.find((a) => re.test(JSON.stringify(a)));
if (!has(/aarch64/) && has(/x86_64/)) arts.push(JSON.parse(JSON.stringify(has(/x86_64/))
  .replace(/x86_64/g, "aarch64").replace(/\bx64\b/g, "arm64").replace(/Intel/g, "Apple Silicon")));
if (!has(/x86_64/) && has(/aarch64/)) arts.push(toIntel(has(/aarch64/)));
if (arts.length === 0) arts = [
  { file: `Wispr-Bro-publik_${version}_aarch64.dmg`, platform: "macos-arm64" },
  { file: `Wispr-Bro-publik_${version}_x86_64.dmg`, platform: "macos-x64" },
];
m.artifacts = arts;
require("fs").writeFileSync(out, JSON.stringify(m, null, 2));
console.log("   name:      " + m.name);
console.log("   summary:   " + m.summary);
console.log("   category:  " + JSON.stringify(m.category));
console.log("   platforms: " + JSON.stringify(m.platforms));
for (const a of arts) console.log("   download:  " + JSON.stringify(a));
' "${TMP}/derive.json" "${STATE}/manifest.json" "$VERSION"
    bun -e 'const m=require(process.argv[1]); console.log(JSON.stringify({confirm:true,manifest:m}))' \
      "${STATE}/manifest.json" > "${TMP}/submit.json"

    say "publik: submit the listing"
    code="$(post "${API}/api/submit" "${TMP}/submitted.json" "@${TMP}/submit.json" "${TMP}/auth.hdr")"
    case "$code" in
      200) SLUG="$(field "${TMP}/submitted.json" slug)"
           echo "listed: ${SLUG}, $(field "${TMP}/submitted.json" status), $(field "${TMP}/submitted.json" url)"
           cp "${TMP}/submitted.json" "${STATE}/listing.json" ;;
      409) SLUG="$(field "${TMP}/submitted.json" slug)"; echo "already listed on publik as ${SLUG}" ;;
      403) url="$(field "${TMP}/submitted.json" claim_url)"
           echo "publik asks you to claim the listing first: ${url}"
           [[ "$url" == https://publikhq.com/* ]] && open "$url"
           die "claim it on that page, then run the line again" ;;
      401) die "the publik approval ran out; run the line again" ;;
      *)   cat "${TMP}/submitted.json" >&2 || true; echo >&2
           die "publik did not accept the listing (HTTP ${code}); the listing sent is in ${STATE}/manifest.json" ;;
    esac
  fi
  [[ "$SLUG" =~ ^[a-z0-9][a-z0-9-]*$ ]] || die "publik gave an unexpected slug"
  echo "$SLUG" > "${STATE}/slug"

  say "publik: the app token for ${SLUG}"
  code="$(post "${API}/api/v1/apps/${SLUG}/token" "${TMP}/apptoken.json" '{}' "${TMP}/auth.hdr")"
  TOKEN="$(field "${TMP}/apptoken.json" token)"
  [[ "$code" == 200 && "$TOKEN" == pat_* ]] || die "publik did not give an app token (HTTP ${code}, $(field "${TMP}/apptoken.json" error))"
  security add-generic-password -U -s "$KEYCHAIN_ITEM" -a wispr-bro -l "Wispr Bro publik app token" -w "$TOKEN"
  printf '%s' "$TOKEN" | gh secret set PUBLIK_APP_TOKEN -R "$REPO"
  echo "app token saved in your keychain and as the GitHub secret PUBLIK_APP_TOKEN"
fi

# The app must say the listing's slug, and its source must be public.
if ! grep -q "pub const APP_SLUG: &str = \"${SLUG}\";" "$MOD_RS"; then
  say "set the app's publik slug to ${SLUG}"
  sed -i '' -E "s/^pub const APP_SLUG: &str = \"[^\"]*\";/pub const APP_SLUG: \&str = \"${SLUG}\";/" "$MOD_RS"
  grep -q "pub const APP_SLUG: &str = \"${SLUG}\";" "$MOD_RS" || die "could not set APP_SLUG"
  git add "$MOD_RS"
  ident=()
  git config user.email >/dev/null || ident=(-c user.name=fjenn -c user.email=fjenn@users.noreply.github.com)
  git ${ident[@]+"${ident[@]}"} commit -q -m "publik: the listing's slug is ${SLUG}"
fi
git fetch -q origin "$BRANCH"
if [[ "$(git rev-parse HEAD)" != "$(git rev-parse "origin/${BRANCH}")" ]]; then
  git merge-base --is-ancestor "origin/${BRANCH}" HEAD || die "${BRANCH} moved on GitHub; get the newest and run again"
  git push -q origin "HEAD:${BRANCH}"
fi
COMMIT="$(git rev-parse HEAD)"
git branch -r --contains "$COMMIT" | grep -q . || die "commit ${COMMIT} is not pushed (its source must be public)"

# ---------------------------------------------------------------------------
# 5. Signed builds and the self test

TOKEN="$(token_from_keychain)"
[[ "$TOKEN" == pat_* ]] || die "no publik app token in the keychain"
for arch in aarch64 x86_64; do
  dir="${OUT}/${arch}"
  if [[ "$(cat "${dir}/.commit" 2>/dev/null || true)" == "$COMMIT" && -f "${dir}/${P}_${arch}.dmg" ]]; then
    echo "publik ${arch} build of ${COMMIT} already done"
    continue
  fi
  say "publik ${arch} build of ${VERSION} from ${COMMIT}"
  rm -rf "$dir"
  PUBLIK_APP_TOKEN="$TOKEN" scripts/wb-release/macos.sh --edition publik --arch "$arch" --tag "$TAG" --out "$dir"
  echo "$COMMIT" > "${dir}/.commit"
done
unset TOKEN

for arch in aarch64 x86_64; do
  say "publik self test (${arch})"
  t="${TMP}/selftest-${arch}"
  mkdir -p "$t"
  tar -xzf "${OUT}/${arch}/${P}_${arch}.app.tar.gz" -C "$t"
  app="$(find "$t" -maxdepth 1 -name '*.app' | head -1)"
  exe="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "${app}/Contents/Info.plist")"
  run=("${app}/Contents/MacOS/${exe}")
  [[ "$arch" == x86_64 ]] && run=(arch -x86_64 "${run[@]}")
  line="$(env -u PUBLIK_APP_TOKEN -u PUBLIK_ORIGIN PUBLIK_SELF_TEST=1 "${run[@]}" 2>/dev/null | grep '^publik self-test:' || true)"
  echo "   ${line:-no self-test line}"
  [[ "$line" == "publik self-test: ok ${SLUG} ${VERSION}" ]] || die "the ${arch} publik build failed its self test"
done

# ---------------------------------------------------------------------------
# 6. Feed, sums, source

UP="${OUT}/upload"
rm -rf "$UP"; mkdir -p "$UP"
for arch in aarch64 x86_64; do
  for f in "${P}_${arch}.dmg" "${P}_${arch}.app.tar.gz" "${P}_${arch}.app.tar.gz.sig"; do
    [[ -f "${OUT}/${arch}/${f}" ]] || die "missing ${OUT}/${arch}/${f}"
    cp "${OUT}/${arch}/${f}" "${UP}/${f}"
  done
done
bun scripts/wb-release/manifest.ts --version "$VERSION" --release-tag "$TAG" \
  --asset "darwin-aarch64=${UP}/${P}_aarch64.app.tar.gz" \
  --asset "darwin-x86_64=${UP}/${P}_x86_64.app.tar.gz" \
  --out "${UP}/latest-publik.json"
git archive --format=tar.gz --prefix="wispr-bro-publik-${VERSION}/" -o "${UP}/${P}_source.tar.gz" "$COMMIT"
( cd "$UP" && shasum -a 256 \
    "${P}_aarch64.dmg" "${P}_aarch64.app.tar.gz" "${P}_aarch64.app.tar.gz.sig" \
    "${P}_x86_64.dmg" "${P}_x86_64.app.tar.gz" "${P}_x86_64.app.tar.gz.sig" \
    "${P}_source.tar.gz" latest-publik.json > SHA256SUMS-publik )

# ---------------------------------------------------------------------------
# 7. Upload (new names only)

say "check that opena.tech offers only the normal downloads"
for f in src/lib/server/wispr-bro-release.ts src/lib/server/wispr-bro-intel.ts; do
  gh api "repos/fjenn/opena-store/contents/${f}" --jq '.content' | base64 --decode \
    | grep -q '\^Wispr-Bro_' || die "opena.tech (${f}) could still pick a publik download; nothing was uploaded"
done

say "upload to ${TAG}"
# Downloads first, the feed and sums last, so the feed never names a missing file.
gh release upload "$TAG" -R "$REPO" \
  "${UP}/${P}_aarch64.dmg" "${UP}/${P}_aarch64.app.tar.gz" "${UP}/${P}_aarch64.app.tar.gz.sig" \
  "${UP}/${P}_x86_64.dmg" "${UP}/${P}_x86_64.app.tar.gz" "${UP}/${P}_x86_64.app.tar.gz.sig" \
  "${UP}/${P}_source.tar.gz"
gh release upload "$TAG" -R "$REPO" "${UP}/latest-publik.json" "${UP}/SHA256SUMS-publik"

# ---------------------------------------------------------------------------
# 8. Check

say "check the release"
CHECK="${OUT}/check"
rm -rf "$CHECK"; mkdir -p "$CHECK"
curl -fsSL -o "${CHECK}/latest.json" "${BASE}/latest.json?t=$(date +%s)"
curl -fsSL -o "${CHECK}/SHA256SUMS" "${BASE}/SHA256SUMS?t=$(date +%s)"
cmp -s "${CHECK}/latest.json" "${PUB}/latest.json" || die "latest.json changed"
cmp -s "${CHECK}/SHA256SUMS" "${PUB}/SHA256SUMS" || die "SHA256SUMS changed"
gh release view "$TAG" -R "$REPO" --json assets --jq '.assets[] | .name + " " + (.size|tostring)' \
  | sort | grep -v -e '-publik' -e '^Wispr-Bro-publik_' > "${CHECK}/assets.txt"
cmp -s "${CHECK}/assets.txt" "${PUB}/assets.txt" || die "the normal files changed"
echo "normal release unchanged"
for f in latest-publik.json SHA256SUMS-publik "${P}_aarch64.dmg" "${P}_x86_64.dmg" \
         "${P}_aarch64.app.tar.gz" "${P}_x86_64.app.tar.gz" "${P}_source.tar.gz"; do
  curl -fsSIL -o /dev/null "${BASE}/${f}" || die "${f} is not downloadable"
done
bun -e "
const m = require('${CHECK}/../upload/latest-publik.json');
for (const k of ['darwin-aarch64', 'darwin-x86_64'])
  if (!m.platforms[k] || !m.platforms[k].url.includes('Wispr-Bro-publik_')) throw new Error('no publik ' + k);
console.log('latest-publik.json: ${VERSION} for Apple Silicon and Intel');
"
echo
echo "done: Wispr Bro is listed on publik as ${SLUG}, and ${TAG} has the publik downloads for Apple Silicon and Intel (${COMMIT})"
[[ -f "${STATE}/listing.json" ]] && echo "listing: $(field "${STATE}/listing.json" url) ($(field "${STATE}/listing.json" status))"
echo "publik picks up the downloads within a day, or at once with 'Check GitHub for a new release now' on your publik dashboard."
