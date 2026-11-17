#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Wispr Bro (iOS): run the end-to-end checks in a simulator.
#   run.sh <simulator udid> <shot dir> <light|dark> [-only-testing:WBE2ETests/Class/test …]
# Needs: the Wispr Bro app installed in that simulator (bun run wb:ios:sim +
# simctl install), its keyboard enabled (see WBE2ESupport.swift), en.wav and
# nl.wav in $WB_WAV_DIR (16 kHz mono 16-bit, e.g. `say -o` + `afconvert`).
set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
sim=$1; shots=$2; look=$3; shift 3
mkdir -p "$shots"
xcrun simctl ui "$sim" appearance "$look"
cd "$here"
[ -d WBE2E.xcodeproj ] || xcodegen generate >/dev/null
export TEST_RUNNER_WB_SHOT_DIR="$shots" TEST_RUNNER_WB_SHOT_SUFFIX="$look"
export TEST_RUNNER_WB_WAV_DIR="${WB_WAV_DIR:?set WB_WAV_DIR}"
export TEST_RUNNER_WB_FAKE_WAV="${WB_FAKE_WAV:-$shots/fake-mic.wav}"
[ -n "${WB_SCRIPT:-}" ] && export TEST_RUNNER_WB_SCRIPT="$WB_SCRIPT"
exec xcodebuild test -project WBE2E.xcodeproj -scheme WBE2EHost \
  -destination "id=$sim" -derivedDataPath "$here/../build/e2e" \
  -quiet "$@"
