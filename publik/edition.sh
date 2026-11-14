# SPDX-License-Identifier: GPL-3.0-or-later
# Sourced by `scripts/wb-release/macos.sh --edition publik` (publik/README.md):
# what the publik edition changes in a release build. Everything else is the
# normal build.

ASSET_PREFIX="Wispr-Bro-publik"
FEED="latest-publik.json"
SUMS="SHA256SUMS-publik"
# Its own releases (publik-v<version>, rolling publik-beta), never the normal
# one, so releases/latest only ever serves the normal build.
RELEASE_PREFIX="publik-"
EDITION_CONFIG=(--config publik/tauri.conf.json)
export VITE_WB_EDITION=publik

# publik's app token is baked in at build time (never in the repository).
if [[ "${PUBLIK_APP_TOKEN:-}" != pat_* ]]; then
  if [[ $UNSIGNED == 0 ]]; then
    echo "macos.sh: the publik edition needs PUBLIK_APP_TOKEN (pat_…; publik/README.md)" >&2; exit 2
  fi
  echo "macos.sh: no PUBLIK_APP_TOKEN: this dry-run publik build will say it has no publik app token" >&2
fi

# After the build: the app carries that token.
edition_check() {
  [[ "${PUBLIK_APP_TOKEN:-}" == pat_* ]] || return 0
  LC_ALL=C grep -qF "$PUBLIK_APP_TOKEN" \
    "$1/Contents/MacOS/$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$1/Contents/Info.plist")"
}
