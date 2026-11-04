# SPDX-License-Identifier: GPL-3.0-or-later
"""Mac test S-05: one take's verdict from the files take.sh wrote.

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
route = re.findall(r"insert: take \d+ target (.*)", log)
outcome = re.findall(r"insert: take \d+ (confirmed|unverified|card|refused|cancelled) via", log)
r["route"] = None
if route:
    raw = route[-1]
    try:
        r["route"] = json.loads(raw)["route"]
        j = json.loads(raw)
        r["ax"] = {k: j.get(k) for k in ("app", "family", "role", "subrole", "verdict", "same_as_key_up", "ax_ms")}
    except ValueError:
        r["route"] = raw
r["outcome"] = outcome[-1] if outcome else None
card = read("card-shown") == "yes"
r["card"] = card
ok = True

if case.startswith("field:"):
    res = read("result.txt") or ""
    if case == "field:textedit":
        r["field_chars"] = len(res)
        landed = len(res) > 0
    else:
        m = re.search(r"WB-S05 (\w+) (\d+)", res)
        r["field"] = m.group(1) if m else None
        r["field_chars"] = int(m.group(2)) if m else 0
        landed = bool(m)
    r["landed"] = landed
    # S-05's criterion: no card with a text field focused. Whether the text
    # landed is S-04's; it is reported, and a take that pasted but did not
    # land is a FAIL only where the harness can paste at all (Safari did not
    # take a synthetic ⌘V from any process on 2026-10-31: see the run file).
    ok = not card and r["route"] is not None and r["route"].startswith("paste")
    if not case.startswith("field:safari"):
        ok = ok and landed
elif case in ("card:finder", "card:desktop"):
    # OT2-A: no field → the clipboard (plain text) and "Copied to clipboard".
    r["expected"] = "no_text_field"
    ours = json.loads(read("clip-ours.json") or "{}")
    r["copy"] = ours
    r["copied"] = "copied to the clipboard" in log
    r["clipboard_restored"] = read("clip-restored")
    ok = not card and r["route"] == "card (no_text_field)" and r["copied"]
    ok = ok and ours.get("equal") is True and not ours.get("concealed")
    ok = ok and r["clipboard_restored"] == "identical"
elif case.startswith("card:"):
    want = {"card:password": "secure_field"}[case]
    r["expected"] = want
    ok = card and r["route"] == f"card ({want})"
    r["clipboard_restored"] = read("clip-restored")
    ok = ok and r["clipboard_restored"] == "identical"
    if case == "card:desktop":
        r["after_close_h"] = read("after-close-h")
        ok = ok and r["after_close_h"] == "0"
    else:
        ours = json.loads(read("clip-ours.json") or "{}")
        r["copy"] = ours
        r["after_copy_h"] = read("after-copy-h")
        ok = ok and ours.get("equal") is True and r["after_copy_h"] == "0"
        ok = ok and ours.get("concealed") is (case == "card:password")
        ok = ok and "pill: card copied" in log
elif case == "switch":
    te = read("textedit.txt") or ""
    ch = read("chrome.txt") or ""
    r["textedit_chars"] = len(te)
    r["chrome_title_has_text"] = "input" in ch.replace("WB-S05 ", "", 1).split(" ")[0:1]
    ok = len(te) > 0 and not card and not r["chrome_title_has_text"] and (r["route"] or "").startswith("paste")
elif case.startswith("moved:"):
    te = read("textedit.txt") or ""
    # The window activated after the stop: our own Chrome instance since
    # Mac wave 7 (other.txt); Safari before (safari.txt).
    sa = (read("other.txt") or read("safari.txt") or "").strip()
    sw = read("switch.json") or ""
    r["textedit_chars"] = len(te)
    r["safari_empty"] = sa.endswith("empty")
    r["switch"] = [json.loads(l) for l in sw.splitlines() if l.strip()]
    ours = json.loads(read("clip-ours.json") or "{}")
    r["copied"] = "copied to the clipboard" in log
    r["clipboard_restored"] = read("clip-restored")
    # OT2-A: focus moved after the release → the clipboard, not the card.
    ok = not card and r["copied"] and r["route"] == "card (focus_moved)" and len(te) == 0
    ok = ok and r["safari_empty"] and ours.get("equal") is True
    ok = ok and r["clipboard_restored"] == "identical"

r["result"] = "PASS" if ok else "FAIL"
print(json.dumps(r))
