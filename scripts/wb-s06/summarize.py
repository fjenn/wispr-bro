# SPDX-License-Identifier: GPL-3.0-or-later
"""Mac test S-06: PASS/FAIL for one case directory of scripts/wb-s06/case.sh.

usage: summarize.py <case> <case dir> <last dictation text>
Reads the files the case wrote and its slice of the app log (`recovery:`,
`insert: take N (…)` and `pill:` lines; the log never holds transcript
text). WB_S06_SAID: the words the case's WAV says (undo cases).
Prints one JSON line.
"""
import json
import os
import re
import sys

case, d, last = sys.argv[1:4]
said = os.environ.get("WB_S06_SAID", "")


def read(name, default=""):
    try:
        return open(os.path.join(d, name), encoding="utf-8").read()
    except OSError:
        return default


log = read("app.log")
field = read("field.txt").strip()
clip = read("clip-restored").strip()
r = {"case": case, "checks": {}}
c = r["checks"]


def words(s):
    return re.findall(r"[a-z']+", s.lower())


def heard(text):
    want = words(said)
    got = set(words(text))
    return bool(want) and sum(w in got for w in want) >= max(1, int(len(want) * 0.6))


if read("skip"):
    r["result"] = "SKIP"
    r["why"] = read("skip").strip()
    print(json.dumps(r))
    sys.exit(0)

if case == "paste:textedit":
    c["pasted"] = field == last
    c["again_kind"] = "(Again)" in log
    c["clipboard"] = clip == "identical"
elif case.startswith("paste:held:"):
    hold = int(case.rsplit(":", 1)[1])
    m = re.search(r'"modifier_wait_ms":(\d+)', log)
    wait = int(m.group(1)) if m else None
    r["modifier_wait_ms"] = wait
    if hold < 3000:
        c["pasted_after_release"] = field == last and wait is not None and wait >= hold - 150
        c["no_card"] = not read("card-shown")
    else:
        c["not_pasted"] = field == ""
        c["card"] = bool(read("card-shown")) and "paste_failed" in log
    c["clipboard"] = clip == "identical"
elif case == "paste:chrome":
    c["pasted"] = last in field
    c["clipboard"] = clip == "identical"
elif case == "paste:terminal":
    c["pasted"] = field == last
    c["clipboard"] = clip == "identical"
elif case == "copy:textedit":
    ours = json.loads(read("clip-ours.json") or "{}")
    r["clip"] = ours
    c["copied"] = ours.get("equal") is True and not ours.get("transient")
    c["clipboard_restored_after"] = clip == "identical"
elif case == "enter:mouse":
    # osascript ends its output with one newline of its own.
    raw = read("field.txt")
    raw = raw[:-1] if raw.endswith("\n") else raw
    c["return_typed"] = raw in ("line one\n", "\nline one")
    c["posted"] = "press-enter posted Return" in log
elif case == "undo:bar":
    c["offer"] = bool(read("offer-shown"))
    c["undo_ran"] = "Undo from the bar" in log
    c["text_in_field"] = heard(field)
    m = re.search(r"Undo delivered entry (\d+): (\w+)", log)
    r["entry"] = m and m.group(1)
    c["delivered"] = bool(m) and m.group(2) in ("confirmed", "unverified")
    c["clipboard"] = clip == "identical"
elif case == "undo:moved":
    c["offer"] = bool(read("offer-shown"))
    c["undo_ran"] = "Undo from the hotkey" in log
    c["card"] = bool(read("card-shown")) and "card (focus_moved)" in log
    c["nothing_pasted"] = field == "" and read("field2.txt").strip() == ""
    c["clipboard"] = clip == "identical"
elif case == "undo:expire":
    c["offer"] = bool(read("offer-shown"))
    c["offer_gone"] = read("after-h").strip() == "0"
    c["dropped"] = "cancelled take dropped (expired)" in log
    c["no_undo"] = "Undo from" not in log and field == ""
    c["clipboard"] = clip == "identical"
else:
    r["result"] = "UNKNOWN"
    print(json.dumps(r))
    sys.exit(0)

r["result"] = "PASS" if c and all(c.values()) else "FAIL"
print(json.dumps(r, ensure_ascii=False))
