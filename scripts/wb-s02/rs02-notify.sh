#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# RS-02 (non-GUI part): does a privacy-record edit post the Darwin
# notification `com.apple.tcc.access.changed` that S-02's health check
# (vendored axwatch) listens for, and how fast?
#
# It resets the Accessibility record of an app that never asks for
# Accessibility (default: Chess, `com.apple.Chess`, so no real grant
# changes) N times while `notifyutil` waits for the notification. A
# throwaway bundle in a temp dir does not work: LaunchServices does not
# resolve it for tccutil (OSStatus -10814). Nothing needs a person or the
# GUI.
#
# Usage: scripts/wb-s02/rs02-notify.sh [trials] [bundle-id]   (default 10)
set -uo pipefail

TRIALS="${1:-10}"
BUNDLE="${2:-com.apple.Chess}"
NAME=com.apple.tcc.access.changed
WORK="$(mktemp -d -t wb-rs02)"
trap 'pkill -f "notifyutil -1 $NAME" 2>/dev/null; rm -rf "$WORK"' EXIT

ms() { python3 -c 'import time; print(int(time.time()*1000))'; }

hits=0
lat=()
for i in $(seq 1 "$TRIALS"); do
  # -1: exit after the first notification.
  notifyutil -1 "$NAME" > "$WORK/n$i.log" 2>&1 &
  waiter=$!
  sleep 0.3
  t0=$(ms)
  err=$(/usr/bin/tccutil reset Accessibility "$BUNDLE" 2>&1) || echo "trial $i: tccutil failed: $err"
  got=0
  for _ in $(seq 1 40); do
    if ! kill -0 "$waiter" 2>/dev/null; then got=1; break; fi
    sleep 0.05
  done
  t1=$(ms)
  if [ "$got" = 1 ]; then
    hits=$((hits + 1))
    lat+=("$((t1 - t0))")
    echo "trial $i: notification after <= $((t1 - t0)) ms"
  else
    kill "$waiter" 2>/dev/null
    wait "$waiter" 2>/dev/null
    echo "trial $i: no notification within 2 s"
  fi
done
echo "RESULT: $hits/$TRIALS resets of $BUNDLE posted $NAME; latencies (ms, 50 ms resolution): ${lat[*]:-none}"
