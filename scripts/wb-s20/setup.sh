#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Mac test S-20: prepares the portable test profile (never the owner's),
# with the app stopped, after one launch created the files. As S-12a's
# setup.py, without Python: Parakeet v3, VAD off (speaker-to-mic audio is
# quiet), English, recordings not kept; the pinned Muesli GGUF in the profile's
# Hugging Face cache (sha256-checked by the app, nothing downloaded);
# cleanup Light; context awareness on or off.
#
# usage: scripts/wb-s20/setup.sh <src-tauri/target/debug/Data> <model id> <muesli gguf> on|off
set -eu
DATA="$1"; MODEL="$2"; GGUF="$3"; CTX="$4"
case "$DATA" in */target/debug/Data) ;; *) echo "refusing: not a portable debug profile" >&2; exit 1 ;; esac
S="$DATA/settings_store.json"
plutil -replace settings.selected_model -string "$MODEL" "$S"
plutil -replace settings.vad_enabled -bool NO "$S"
plutil -replace settings.selected_language -string en "$S"
plutil -replace settings.spoken_languages -json '["en"]' "$S"
plutil -replace settings.recording_retention_period -string never "$S"
plutil -replace settings.append_trailing_space -bool NO "$S"
SNAP="$DATA/huggingface/hub/models--Muesli-HQ--qwen35-2b-muesli-cleanup-v12-gguf/snapshots/e35af097be4f17d3673515fb321ce19aa300faf6"
mkdir -p "$SNAP"
[ -f "$SNAP/qwen35-2b-muesli-v12-Q4_K_M.gguf" ] || cp -c "$GGUF" "$SNAP/qwen35-2b-muesli-v12-Q4_K_M.gguf"
F="$DATA/fork-settings.json"
[ -f "$F" ] || echo '{"settings":{}}' >"$F"
plutil -replace settings.cleanup_level -string light "$F"
if [ "$CTX" = on ]; then v=YES; else v=NO; fi
plutil -replace settings.context_awareness -bool "$v" "$F"
echo "model $MODEL | cleanup light | context $CTX"
