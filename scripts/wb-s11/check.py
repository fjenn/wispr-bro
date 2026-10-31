# SPDX-License-Identifier: GPL-3.0-or-later
"""Checks one Mac test S-11 take (scripts/wb-s11/run.sh).

usage: check.py <case> <target> <out file>
Prints PASS or FAIL with the text that arrived. The checks are on the shape
the pipeline must produce, not on exact words, because the speech model's
wording of the TTS audio varies a little ("Finn" / "Fin").
"""
import sys
import urllib.parse

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


for spoken in ("new paragraph", "new line.", "scratch that", "question mark"):
    need(spoken not in low, f"spoken command left as text: {spoken!r}")
need(sent == 0, f"{sent} message(s) sent by a Return")

if case == "email":
    need(text.count("\n\n") == 2, "two paragraph breaks")
    need(text.count("\n") == 5, "two paragraph breaks and one line break")
    need("Wispr Bro" in text, "rule whisper bro → Wispr Bro")
    need("Anna-Lena" in text, "rule anna → Anna-Lena")
    need("monday" not in low, "scratch that removed the Monday clause")
    need("tuesday" in low, "the clause after scratch that kept")
elif case == "chat":
    need(len(text.split("\n")) == 2, "two lines")
elif case == "literal":
    need("\n" not in text, "no line break")
    need("new line of shoes" in low, "'a new line of shoes' stays text")
    need("type new line" in low, "'literally new line' keeps the words")
elif case == "question":
    need(text.rstrip().endswith("?"), "ends with ?")
else:
    problems.append(f"unknown case {case}")

print(("PASS" if not problems else "FAIL"), case, target, repr(text), "; ".join(problems))
sys.exit(0 if not problems else 1)
