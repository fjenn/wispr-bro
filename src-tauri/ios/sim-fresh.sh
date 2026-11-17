#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Wispr Bro (iOS): a first launch in a simulator without a model download.
#   src-tauri/ios/sim-fresh.sh <simulator udid> <folder holding huggingface/>
# Removes the app and its data, installs the last simulator build, and
# clones (cp -c: no disk space) the given Hugging Face cache into the new
# container, so the first launch finds its speech model.
set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
sim=$1; seed=$2
app="$here/../gen/apple/build/arm64-sim/Wispr Bro.app"
xcrun simctl terminate "$sim" com.wisprbro.ios 2>/dev/null || true
xcrun simctl uninstall "$sim" com.wisprbro.ios 2>/dev/null || true
xcrun simctl install "$sim" "$app"
data=$(xcrun simctl get_app_container "$sim" com.wisprbro.ios data)
support="$data/Library/Application Support/com.wisprbro.ios"
mkdir -p "$support"
cp -Rc "$seed/huggingface" "$support/"
# The App Group container outlives the app: forget the session settings too.
group=$(xcrun simctl get_app_container "$sim" com.wisprbro.ios groups 2>/dev/null | awk '{print $2}' | head -1)
[ -n "$group" ] && rm -f "$group/Library/Preferences/group.com.wisprbro.ios.plist"
echo "$support"
