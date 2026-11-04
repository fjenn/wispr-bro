# SPDX-License-Identifier: GPL-3.0-or-later
#
# Checks a mounted Wispr Bro DMG's install window against the layout in
# src-tauri/tauri.fork.conf.json (bundle.macOS.dmg), by reading the
# volume's .DS_Store (ds_store, MIT) and background file; no Finder.
# Prints one "✓ …" / "✗ …" line per check; exit 1 when any fails.
#
# Usage: python3 -I check_layout.py <mountpoint> <tauri.fork.conf.json> [icon_size]
import json
import os
import plistlib
import re
import subprocess
import sys

from ds_store import DSStore
from mac_alias import Alias

mnt, conf_path = sys.argv[1], sys.argv[2]
icon_size = float(sys.argv[3]) if len(sys.argv) > 3 else 128.0
dmg = json.load(open(conf_path))["bundle"]["macOS"]["dmg"]
app_name = "Wispr Bro.app"
fails = 0


def plist(value):
    """ds_store decodes bplist entries itself; older versions give bytes."""
    if isinstance(value, tuple):
        value = value[1]
    return plistlib.loads(value) if isinstance(value, (bytes, bytearray)) else value


def check(ok, label):
    global fails
    print(("  ✓ " if ok else "  ✗ ") + label, flush=True)
    if not ok:
        fails += 1


bg = os.path.join(mnt, ".background.tiff")
check(os.path.isfile(bg), "background picture .background.tiff present")
if os.path.isfile(bg):
    info = subprocess.run(
        ["/usr/bin/tiffutil", "-info", bg], capture_output=True, text=True
    ).stdout
    sizes = re.findall(r"Image Width: (\d+) Image Length: (\d+)", info)
    w, h = dmg["windowSize"]["width"], dmg["windowSize"]["height"]
    want = {(str(w), str(h)), (str(2 * w), str(2 * h))}
    check(
        want.issubset(set(sizes)),
        f"background has 1x and 2x images ({', '.join('x'.join(s) for s in sizes) or 'none'})",
    )

store = os.path.join(mnt, ".DS_Store")
check(os.path.isfile(store), ".DS_Store present")
if os.path.isfile(store):
    with DSStore.open(store, "r") as d:
        root = d["."]
        bwsp = plist(root["bwsp"])
        icvp = plist(root["icvp"])
        check(not bwsp.get("ShowToolbar", True), "toolbar hidden")
        check(not bwsp.get("ShowSidebar", True), "sidebar hidden")
        check(not bwsp.get("ShowStatusBar", True), "status bar hidden")
        m = re.findall(r"-?\d+", bwsp.get("WindowBounds", ""))
        ws = dmg["windowSize"]
        check(
            len(m) == 4 and int(m[2]) == ws["width"] and int(m[3]) == ws["height"],
            f"window {ws['width']}×{ws['height']} (bounds {bwsp.get('WindowBounds')})",
        )
        check(icvp.get("iconSize") == icon_size, f"icon size {icvp.get('iconSize')}")
        check(icvp.get("backgroundType") == 2, "background type: picture")
        alias = icvp.get("backgroundImageAlias")
        target = Alias.from_bytes(alias).target.filename if alias else None
        check(target == ".background.tiff", f"background alias → {target}")
        for name, key in ((app_name, "appPosition"), ("Applications", "applicationFolderPosition")):
            want = (dmg[key]["x"], dmg[key]["y"])
            try:
                got = tuple(d[name]["Iloc"])
            except KeyError:
                got = None
            check(got == want, f"{name} at {got} (want {want})")

if fails:
    sys.exit(1)
