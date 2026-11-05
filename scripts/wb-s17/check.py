# SPDX-License-Identifier: GPL-3.0-or-later
"""Checks one Mac test S-17 take (scripts/wb-s17/run.sh).

usage: check.py <case> <target> <out file>
Prints PASS or FAIL with the text that arrived. The saved text must arrive
exactly (three lines, the made-up address); the dictated words around an
inline trigger are checked loosely because the speech model's wording of
the TTS audio varies a little.
"""
import sys
import urllib.parse

ADDRESS = "Alex Morgan\n12 Harbour Lane\nKiel 24103"

case, target, path = sys.argv[1:4]
raw = open(path, encoding="utf-8").read().rstrip("\n")
sent = 0
if target in ("safari", "chrome"):
    # Window title: "S11|<text>" or "S11|sent=N|<text>" (+ " - Google Chrome").
    title = raw.removesuffix(" - Google Chrome")
    parts = title.split("|")
    if len(parts) >= 3 and parts[1].startswith("sent="):
        sent = int(parts[1][5:])
    text = urllib.parse.unquote(parts[-1]) if len(parts) > 1 else ""
else:
    text = raw
low = text.lower()
problems = []


def need(ok, what):
    if not ok:
        problems.append(what)


need(sent == 0, f"{sent} message(s) sent by a Return")
need("my address" not in low, "the trigger is left as text")
if case == "alone":
    need(text == ADDRESS, "exactly the saved text, nothing else")
elif case == "inline":
    need(ADDRESS in text, "the saved text inside the sentence")
    need(low.startswith("please send"), "the words before the trigger kept")
    need("friday" in low.split(ADDRESS.lower())[-1], "the words after the trigger kept")
    need(text.count("\n") == 2, "only the saved text's two line breaks")
else:
    problems.append(f"unknown case {case}")

print(("PASS" if not problems else "FAIL"), case, target, repr(text), "; ".join(problems))
sys.exit(0 if not problems else 1)
