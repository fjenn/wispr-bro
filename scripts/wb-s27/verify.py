# SPDX-License-Identifier: GPL-3.0-or-later
"""Mac test S-27: check the files the app produced. Prints PASS/FAIL lines;
exits 1 on any failure. Reads only the paths it is given.

  verify.py export <export.zip> <seed.json>       S-27.1 contents
  verify.py diagnostics <diagnostics.zip> <seed.json>
  verify.py erased <Data dir> <seed.json> kept|deleted   S-27.2 (after relaunch)
  verify.py onboarding <Data dir>                 S-27.2 (after relaunch)
"""
import csv
import io
import json
import os
import sys
import zipfile

failed = False


def check(ok, what):
    global failed
    print(("PASS " if ok else "FAIL ") + what)
    failed |= not ok


def files_under(root):
    out = []
    for base, _dirs, names in os.walk(root):
        for n in names:
            out.append(os.path.relpath(os.path.join(base, n), root))
    return sorted(out)


cmd = sys.argv[1]
if cmd == "export":
    z = zipfile.ZipFile(sys.argv[2])
    seed = json.load(open(sys.argv[3]))
    names = set(z.namelist())
    manifest = json.loads(z.read("manifest.json"))
    check(manifest.get("schema_version") == 1, "manifest schema_version 1")
    for n in ("history.csv", "history.json", "databases/fork.db",
              "settings/settings_store.json", "settings/fork-settings.json"):
        check(n in names, f"export has {n}")
    rows = list(csv.DictReader(io.StringIO(z.read("history.csv").decode())))
    texts = {r["transcription_text"] for r in rows}
    for r in seed["rows"]:
        check(r["text"] in texts, f"history.csv has row {r['id']}")
        check(f"recordings/{r['file_name']}" in names, f"export has {r['file_name']}")
    check(not any(n.startswith(("models/", "huggingface/", "logs/")) for n in names),
          "export has no models or logs")
elif cmd == "diagnostics":
    z = zipfile.ZipFile(sys.argv[2])
    seed = json.load(open(sys.argv[3]))
    home = os.path.expanduser("~")
    check("diagnostics.txt" in z.namelist(), "diagnostics.txt present")
    check(any(n.startswith("logs/crash-") for n in z.namelist()), "crash file included")
    for n in z.namelist():
        text = z.read(n).decode("utf-8", "replace")
        check(seed["marker"] not in text, f"{n}: marker redacted")
        check(f"{home}/" not in text, f"{n}: home path redacted")
    check(not any(n.endswith((".db", ".wav", ".json")) for n in z.namelist()),
          "no databases, audio or settings in diagnostics")
elif cmd == "erased":
    # After the relaunch (the fresh app recreates empty stores at once, so
    # "only models left" is a unit test; this checks the seeded data is gone).
    data, seed, mode = sys.argv[2], json.load(open(sys.argv[3])), sys.argv[4]
    for m in seed["kept_models"]:
        present = os.path.exists(os.path.join(data, m))
        check(present == (mode == "kept"), f"{m} {'kept' if mode == 'kept' else 'deleted'}")
    for r in seed["rows"]:
        check(not os.path.exists(os.path.join(data, "recordings", r["file_name"])),
              f"{r['file_name']} deleted")
    history = os.path.join(data, "history.db")
    if os.path.exists(history):
        import sqlite3
        con = sqlite3.connect(f"file:{history}?mode=ro", uri=True)
        texts = {t for (t,) in con.execute("SELECT transcription_text FROM transcription_history")}
        check(not texts & {r["text"] for r in seed["rows"]}, "seeded History rows deleted")
    hits = []
    for rel in files_under(data):
        if rel.startswith(("models/", "huggingface/")):
            continue
        try:
            with open(os.path.join(data, rel), "rb") as f:
                if seed["marker"].encode() in f.read(4 << 20):
                    hits.append(rel)
        except OSError:
            pass
    check(not hits, f"marker phrase gone from Data ({hits[:5]})")
elif cmd == "onboarding":
    data = sys.argv[2]
    def find(node):
        if isinstance(node, dict):
            if isinstance(node.get("onboarding"), dict):
                return node["onboarding"].get("status")
            return next((s for v in node.values() if (s := find(v))), None)
        return None

    path = os.path.join(data, "fork-settings.json")
    status = find(json.load(open(path))) if os.path.exists(path) else "not_started"
    check(status in ("not_started", "in_progress"), f"onboarding restarted ({status})")
    check(not os.path.exists(os.path.join(data, "recordings")) or
          not os.listdir(os.path.join(data, "recordings")), "no recordings after relaunch")
else:
    sys.exit(__doc__)
sys.exit(1 if failed else 0)
