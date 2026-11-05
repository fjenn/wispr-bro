# SPDX-License-Identifier: GPL-3.0-or-later
"""Mac test OT2-A: one take's verdict from the files take.sh wrote.

usage: summarize.py <case> <take dir>   prints one JSON line, PASS or FAIL
Never prints transcript text: only lengths and whether texts match.
"""
import json
import os
import re
import sys

case, d = sys.argv[1:3]


def read(name):
    p = os.path.join(d, name)
    return open(p, encoding="utf-8", errors="replace").read().strip() if os.path.exists(p) else None


r = {"case": case, "dir": os.path.basename(d)}
if read("skip") is not None:
    r.update(result="SKIP", why=read("skip"))
    print(json.dumps(r))
    sys.exit(0)

log = read("app.log") or ""
r["cancelled"] = "Cancelled" in log or "ChordIgnored" in log
r["stuck_released"] = "StuckReleased" in log
r["copied"] = "copied to the clipboard" in log
route = re.findall(r"insert: take \d+ target (.*)", log)
r["route"] = None
if route:
    try:
        r["route"] = json.loads(route[-1])["route"]
    except ValueError:
        r["route"] = route[-1]
r["front_at_release"] = read("front-at-release")
a = read("docA.txt") or ""
b = read("docB.txt") or ""
r["docA_chars"], r["docB_chars"] = len(a), len(b)
ours = json.loads(read("clip-ours.json") or "{}")
r["clipboard"] = {k: ours.get(k) for k in ("equal", "concealed", "transient")}
r["clipboard_restored"] = read("clip-restored")
hist = read("history-text.txt") or ""
r["history_chars"] = len(hist)

ok = not r["cancelled"] and not r["stuck_released"] and r["history_chars"] > 0
ok = ok and r["clipboard_restored"] == "identical"
if case in ("hold:cmdtab", "free:cmdtab"):
    ok = ok and r["front_at_release"] == "com.apple.TextEdit" and len(a) > 0
    ok = ok and (r["route"] or "").startswith("paste") and not r["copied"]
elif case == "hold:click":
    ok = ok and len(b) > 0 and len(a) == 0 and (r["route"] or "").startswith("paste")
elif case in ("hold:finder", "hold:desktop"):
    ok = ok and r["front_at_release"] == "com.apple.finder" and len(a) == 0
    ok = ok and r["route"] == "card (no_text_field)" and r["copied"]
    # Plain text the user pastes themselves: unmarked, equal to History.
    ok = ok and ours.get("equal") is True and not ours.get("concealed") and not ours.get("transient")
r["result"] = "PASS" if ok else "FAIL"
print(json.dumps(r))
