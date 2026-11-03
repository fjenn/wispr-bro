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
wb_running_in() {
  local dir="${1:?dir}" p
  for p in $(pgrep -f "$WB_BIN_RE"); do
    if lsof -a -p "$p" -d cwd -Fn 2>/dev/null | /usr/bin/grep -qx "n$dir"; then
      return 0
    fi
  done
  return 1
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
