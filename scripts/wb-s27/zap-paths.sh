#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Mac test S-27.3: list every ~/Library path a test identity left behind, for
# comparison with the cask's `zap` stanza (S-39). Lists only; deletes nothing.
#   sh scripts/wb-s27/zap-paths.sh com.wisprbro.s27check
set -eu
id="${1:?usage: zap-paths.sh <bundle identifier>}"
case "$id" in com.wisprbro.*check) ;; *) echo "refusing: not a test identity" >&2; exit 2 ;; esac
find "$HOME/Library" -maxdepth 3 -name "*$id*" 2>/dev/null | sed "s|^$HOME|~|" | sort
