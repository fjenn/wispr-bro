#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-34: after scripts/wb-s20/setup.sh (portable test profile,
# never the owner's), with the app stopped: auto-learn on or off, and a
# test-only dictionary rule "zebra → Kubernetees" so the take's text holds
# the misspelling the driver then corrects (Parakeet spells Kubernetes
# right, so the misspelling cannot come from speech). Earlier S-34 rows
# (learned words, rejected words, learn corrections) are cleared.
#
# usage: scripts/wb-s34/setup.sh <src-tauri/target/debug/Data> on|off
set -eu
DATA="$1"; MODE="$2"
case "$DATA" in */target/debug/Data) ;; *) echo "refusing: not a portable debug profile" >&2; exit 1 ;; esac
F="$DATA/fork-settings.json"
[ -f "$F" ] || echo '{"settings":{}}' >"$F"
case "$MODE" in
  on) plutil -replace settings.auto_learn_enabled -bool YES "$F" ;;
  off) plutil -replace settings.auto_learn_enabled -bool NO "$F" ;;
  *) echo "usage: setup.sh <Data> on|off" >&2; exit 2 ;;
esac
DB="$DATA/fork.db"
sqlite3 "$DB" <<'SQL'
DELETE FROM dictionary_entries WHERE source = 'learn';
DELETE FROM corrections WHERE source = 'learn';
INSERT OR IGNORE INTO dictionary_entries (kind, from_text, to_text, norm, source, status, created_at, updated_at)
  VALUES ('rule', 'zebra', 'Kubernetees', 'zebra', 'user', 'active', 1, 1);
SQL
echo "auto-learn $MODE | rule zebra → Kubernetees"
