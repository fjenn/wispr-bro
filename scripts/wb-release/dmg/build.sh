#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Makes the Wispr Bro DMG with its install window (OT2-E): our background
# (src-tauri/dmg-fork/background.png + @2x), the app on the left, the
# Applications link on the right, icons at 128 pt, no toolbar, sidebar or
# status bar, the window sized to the picture. Layout comes from Tauri's
# `bundle.macOS.dmg` in src-tauri/tauri.fork.conf.json, the one source for
# this script, the background generator (background.ts) and the checks
# (inspect.py via verify.sh).
#
# Why dmgbuild and not Tauri's own DMG bundler: Tauri's bundle_dmg.sh lays
# the window out by driving Finder with AppleScript, which opens the window
# on screen and needs Automation permission; dmgbuild (MIT, pinned by hash
# in requirements.txt, build-time only) writes the .DS_Store directly and
# attaches the image -nobrowse. macos.sh signs, notarizes and staples the
# result exactly as before.
#
# Usage: scripts/wb-release/dmg/build.sh <app> <out.dmg>
#   source it and call `wb_dmg_python` to get the venv's python (verify.sh).
# Env: WB_DMG_VENV  venv folder (default ~/.cache/wispr-bro/dmgbuild-<hash>)
set -euo pipefail

WB_DMG_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WB_ROOT="$(cd "${WB_DMG_DIR}/../../.." && pwd)"

wb_dmg_python() { # prints the python of a venv with requirements.txt installed
  local req="${WB_DMG_DIR}/requirements.txt" hash venv
  hash="$(shasum -a 256 "$req" | cut -c1-12)"
  venv="${WB_DMG_VENV:-${HOME}/.cache/wispr-bro/dmgbuild-${hash}}"
  if [[ ! -x "${venv}/bin/python3" ]] || ! "${venv}/bin/python3" -c 'import dmgbuild, ds_store, mac_alias' 2>/dev/null; then
    rm -rf "$venv"
    /usr/bin/env python3 -m venv "$venv" >&2
    "${venv}/bin/python3" -m pip install --quiet --disable-pip-version-check \
      --require-hashes --only-binary :all: -r "$req" >&2
  fi
  echo "${venv}/bin/python3"
}

wb_dmg_build() { # <app> <out.dmg>
  local app="$1" out="$2" py conf
  [[ -d "$app" ]] || { echo "dmg/build.sh: no app at $app" >&2; return 1; }
  py="$(wb_dmg_python)"
  conf="${WB_ROOT}/src-tauri/tauri.fork.conf.json"
  # bundle.macOS.dmg → -D defines (one line each: key=value).
  local defines=()
  while IFS= read -r line; do defines+=(-D "$line"); done < <(bun -e "
    const d = require('${conf}').bundle.macOS.dmg;
    const p = require('path');
    const bg = p.join('${WB_ROOT}/src-tauri', d.background);
    console.log([
      'background=' + bg,
      'window_x=' + d.windowPosition.x, 'window_y=' + d.windowPosition.y,
      'window_w=' + d.windowSize.width, 'window_h=' + d.windowSize.height,
      'app_x=' + d.appPosition.x, 'app_y=' + d.appPosition.y,
      'apps_x=' + d.applicationFolderPosition.x, 'apps_y=' + d.applicationFolderPosition.y,
      'icon_size=128',
      'volume_icon=${WB_ROOT}/src-tauri/icons-fork/icon.icns',
    ].join('\n'));")
  local bg
  bg="$(printf '%s\n' "${defines[@]}" | sed -n 's/^background=//p')"
  [[ -f "$bg" && -f "${bg%.png}@2x.png" ]] || {
    echo "dmg/build.sh: background $bg or its @2x is missing (bun scripts/wb-release/dmg/background.ts)" >&2
    return 1
  }
  rm -f "$out"
  "$py" -m dmgbuild -s "${WB_DMG_DIR}/settings.py" -D "app=${app}" "${defines[@]}" \
    "Wispr Bro" "$out" >&2
  [[ -s "$out" ]] || { echo "dmg/build.sh: dmgbuild wrote nothing" >&2; return 1; }
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  [[ $# -eq 2 ]] || { echo "usage: build.sh <app> <out.dmg>" >&2; exit 2; }
  wb_dmg_build "$1" "$2"
fi
