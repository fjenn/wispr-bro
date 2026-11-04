#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Shared helpers for the Mac drivers (scripts/wb-s*): find the app's
# executable and talk to the running instance without hanging.
#
# Since S-38 the bundle's executable is `wispr-bro` (`mainBinaryName`); a
# `tauri build --no-bundle` leaves `target/<profile>/wispr-bro`, while a
# plain `cargo build` still writes the crate name `handy`. A symlink from one
# to the other does not work (Tauri's resource lookup panics), so the drivers
# run whichever real file is there.
#
# usage: . "$(dirname "$0")/../wb-lib/app.sh"
#   wb_bin_name <dir|bundle.app>   executable name (CFBundleExecutable for a
#                                  bundle; else the newest of wispr-bro, handy)
#   wb_bin_path <dir|bundle.app>   its full path
#   WB_BIN_RE                      pgrep -f pattern matching either name
#   wb_cli <dir> <args...>         `./<bin> <args>` in <dir>, killed after
#                                  WB_CLI_TIMEOUT s (default 15); returns 124
#                                  on a timeout
#   wb_cancel <dir>                `--cancel` to the running instance; does
#                                  nothing when no instance runs from <dir>
#   wb_unbind_keys <data> [ids] [id=chord...]
#                                  every global shortcut of the test profile
#                                  unbound except the comma list <ids>; each
#                                  id=chord binds one for a check that needs it
#                                  (profile.py unbind; Mac wave 2 finding 7)
#   wb_clone_models <data> [src]   model cache cloned into the test profile
#                                  (profile.py models; no 1.6 GB download)
#   wb_test_mic_wav <text> <wav>   a 16 kHz WAV of <text> (`say`), for
#                                  WISPRBRO_TEST_MIC_WAV (never the speakers)
#   wb_test_mic <dir> <wav>        a driver that launches the app: exports
#                                  WISPRBRO_TEST_MIC_WAV (default <dir>/
#                                  test-mic.wav) and loads <wav> into it
#   wb_mic_load <wav>              before a take: the test microphone's file
#                                  becomes <wav> (WISPRBRO_TEST_MIC_WAV); fails
#                                  without it unless WB_ALLOW_SPEAKERS=1
#   wb_mic_play <wav>              during a take: waits while the app's test
#                                  microphone plays <wav>; without
#                                  WISPRBRO_TEST_MIC_WAV it refuses unless
#                                  WB_ALLOW_SPEAKERS=1 (then `afplay`)

WB_BIN_RE='(handy|wispr-bro)'

wb_bin_name() {
  local dir="${1:?dir or bundle}" plist exe best="" t bt=0
  plist="$dir/Contents/Info.plist"
  if [ -f "$plist" ]; then
    exe=$(/usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "$plist" 2>/dev/null)
    [ -n "$exe" ] && { echo "$exe"; return 0; }
  fi
  for exe in wispr-bro handy; do
    [ -f "$dir/$exe" ] && [ -x "$dir/$exe" ] || continue
    t=$(stat -f %m "$dir/$exe" 2>/dev/null || echo 0)
    if [ -z "$best" ] || [ "$t" -gt "$bt" ]; then best="$exe"; bt="$t"; fi
  done
  echo "${best:-handy}"
}

wb_bin_path() {
  local dir="${1:?dir or bundle}"
  if [ -f "$dir/Contents/Info.plist" ]; then
    echo "$dir/Contents/MacOS/$(wb_bin_name "$dir")"
  else
    echo "$dir/$(wb_bin_name "$dir")"
  fi
}

# Whether an instance of the app runs with <dir> as its working directory
# (how every driver starts it).
#
# Only processes whose command is the app (`./wispr-bro`, `…/MacOS/wispr-bro`,
# `handy`) count, and their working directories come from one `lsof` call:
# the old `pgrep -f "$WB_BIN_RE"` also matched every compiler, test binary,
# Vite and browser process with "wispr-bro" in a path, and one `lsof` per
# match took over 30 s on a loaded Mac (the `wb_cancel` self-test timeout).
wb_running_in() {
  local dir="${1:?dir}" pids
  pids=$(pgrep -f "(^|/)$WB_BIN_RE( |\$)" | paste -sd, -)
  [ -n "$pids" ] || return 1
  lsof -a -p "$pids" -d cwd -Fn 2>/dev/null | /usr/bin/grep -qx "n$dir"
}

wb_cli() {
  local dir="${1:?dir}" pid i limit
  shift
  limit="${WB_CLI_TIMEOUT:-15}"
  (cd "$dir" && exec "./$(wb_bin_name "$dir")" "$@") >/dev/null 2>&1 &
  pid=$!
  for ((i = 0; i < limit * 10; i++)); do
    kill -0 "$pid" 2>/dev/null || { wait "$pid"; return $?; }
    sleep 0.1
  done
  kill "$pid" 2>/dev/null
  sleep 0.5
  kill -9 "$pid" 2>/dev/null
  wait "$pid" 2>/dev/null
  echo "wb_cli: $* did not return within ${limit} s; killed" >&2
  return 124
}

# `--cancel` is forwarded by the single-instance plugin. With no instance
# running it would start a whole new app that never exits (the 9-minute
# hang in Mac wave 1), so it is sent only to a running instance, and with a
# timeout.
wb_cancel() {
  local dir="${1:?dir}"
  if ! wb_running_in "$dir"; then
    echo "wb_cancel: no app running from $dir; nothing to cancel" >&2
    return 1
  fi
  wb_cli "$dir" --cancel
}

# A TextEdit instance of the driver's own (`open -n`, no restored windows),
# so keys and pastes can be guarded by its pid: the owner's TextEdit, if it
# runs, has the same name and bundle id and may hold their documents.
#   wb_textedit_open <file>   opens <file> in a new instance; prints its pid
#   wb_textedit_close <pid>   quits that instance without saving
wb_textedit_open() {
  local file="${1:?file}" before p i
  before=" $(pgrep -x TextEdit | tr '\n' ' ') "
  open -n -a TextEdit "$file" --args -ApplePersistenceIgnoreState YES
  for ((i = 0; i < 50; i++)); do
    for p in $(pgrep -x TextEdit); do
      case "$before" in *" $p "*) ;; *) echo "$p"; return 0 ;; esac
    done
    sleep 0.2
  done
  echo "wb_textedit_open: no new TextEdit instance" >&2
  return 1
}

wb_textedit_close() {
  local p="${1:?pid}"
  ps -o comm= -p "$p" 2>/dev/null | /usr/bin/grep -q TextEdit || return 0
  kill "$p" 2>/dev/null
  sleep 1
  kill -9 "$p" 2>/dev/null
  return 0
}

WB_LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Mac wave 2 finding 7: a test app started with its default shortcuts
# answers fn, fn+Space, ⌃⌘V… next to the owner's own Wispr Bro. Every
# driver seeds its test profile with them unbound (takes go by SIGUSR2) and
# keeps only what a check needs.
wb_unbind_keys() {
  local data="${1:?data dir}" keep="${2:-}" args=() b
  shift; shift 2>/dev/null || true
  [ -n "$keep" ] && args+=(--keep "$keep")
  for b in "$@"; do args+=(--set "$b"); done
  python3 -I "$WB_LIB_DIR/profile.py" unbind "$data" ${args[@]+"${args[@]}"}
}

wb_clone_models() {
  local data="${1:?data dir}" src="${2:-}"
  if [ -n "$src" ]; then
    python3 -I "$WB_LIB_DIR/profile.py" models "$data" --from "$src"
  else
    python3 -I "$WB_LIB_DIR/profile.py" models "$data"
  fi
}

# The app's microphone plays this file (test identities and debug builds
# only, `fork/audio/test_mic.rs`): export WISPRBRO_TEST_MIC_WAV=<wav> before
# launching. 16 kHz mono 16-bit, the recorder's own rate.
wb_test_mic_wav() {
  local text="${1:?text}" wav="${2:?wav}"
  [ -s "$wav" ] && return 0
  mkdir -p "$(dirname "$wav")"
  say -o "$wav" --file-format=WAVE --data-format=LEI16@16000 "$text"
}

# Pre-recorded audio, never the speakers (the owner's rule): the app,
# started with WISPRBRO_TEST_MIC_WAV=<path> (test identities and debug
# builds only), reads <path> at the start of every take and plays it as its
# microphone. A driver exports the same variable, calls wb_mic_load before
# starting a take and wb_mic_play where it used to call afplay.
wb_mic_load() {
  local wav="${1:?wav}"
  if [ -z "${WISPRBRO_TEST_MIC_WAV:-}" ]; then
    [ "${WB_ALLOW_SPEAKERS:-}" = 1 ] && return 0
    echo "wb_mic_load: no WISPRBRO_TEST_MIC_WAV (start the app with it); no take through the speakers (set WB_ALLOW_SPEAKERS=1 to force)" >&2
    return 1
  fi
  [ "$(cd "$(dirname "$wav")" && pwd)/$(basename "$wav")" = "$WISPRBRO_TEST_MIC_WAV" ] && return 0
  # Any `say -o` / afplay format (AIFF, WAV) → 16 kHz mono WAV, swapped in
  # atomically (the app reads it when the take starts).
  mkdir -p "$(dirname "$WISPRBRO_TEST_MIC_WAV")"
  afconvert -f WAVE -d LEI16@16000 -c 1 "$wav" "$WISPRBRO_TEST_MIC_WAV.tmp.wav" \
    && mv -f "$WISPRBRO_TEST_MIC_WAV.tmp.wav" "$WISPRBRO_TEST_MIC_WAV"
}

# A driver whose app is started in one step (scripts/wb-s05/app.sh start, a
# phase restart) and whose takes run in another agrees on one file: <data
# dir>/test-mic.wav, exported as WISPRBRO_TEST_MIC_WAV before the app starts
# and before each take. An existing WISPRBRO_TEST_MIC_WAV is kept;
# WB_ALLOW_SPEAKERS=1 leaves it unset (the old speakers-into-the-mic path).
wb_test_mic_env() {
  local data="${1:?data dir}"
  [ -n "${WISPRBRO_TEST_MIC_WAV:-}" ] && return 0
  [ "${WB_ALLOW_SPEAKERS:-}" = 1 ] && return 0
  mkdir -p "$data" || return 1
  WISPRBRO_TEST_MIC_WAV="$(cd "$data" && pwd)/test-mic.wav"
  export WISPRBRO_TEST_MIC_WAV
}

wb_test_mic() {
  local dir="${1:?dir}" wav="${2:?wav}"
  mkdir -p "$dir"
  export WISPRBRO_TEST_MIC_WAV="${WISPRBRO_TEST_MIC_WAV:-$(cd "$dir" && pwd)/test-mic.wav}"
  wb_mic_load "$wav"
}

wb_wav_seconds() {
  python3 -I -c 'import sys,wave; w=wave.open(sys.argv[1]); print(w.getnframes()/w.getframerate())' "${1:?wav}" 2>/dev/null \
    || afinfo "$1" 2>/dev/null | sed -n 's/^estimated duration: \([0-9.]*\).*/\1/p'
}

wb_mic_play() {
  local wav="${1:?wav}" secs
  if [ -n "${WISPRBRO_TEST_MIC_WAV:-}" ]; then
    secs="$(wb_wav_seconds "$wav")"
    sleep "${secs:-3}"
    return 0
  fi
  if [ "${WB_ALLOW_SPEAKERS:-}" = 1 ]; then
    # WB_SPEAKER_VOLUME (afplay -v, 0-1): keep it moderate.
    afplay ${WB_SPEAKER_VOLUME:+-v "$WB_SPEAKER_VOLUME"} "$wav"
    return $?
  fi
  echo "wb_mic_play: no WISPRBRO_TEST_MIC_WAV (start the app with it); not playing through the speakers (set WB_ALLOW_SPEAKERS=1 to force)" >&2
  return 1
}
