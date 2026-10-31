# SPDX-License-Identifier: GPL-3.0-or-later
"""Mac test S-11: prepares the portable test profile (never the owner's).

usage: setup.py <src-tauri/target/debug/Data> <model id>
Sets Handy's settings for the run and replaces the dictionary rules in
fork.db with the four the checks expect (S-16 has no UI yet). Run it with
the app closed, after one launch created the files.
"""
import json
import sqlite3
import sys
import time

data, model = sys.argv[1:3]
if "/target/debug/Data" not in data:
    sys.exit("refusing: not a portable debug profile")

path = f"{data}/settings_store.json"
store = json.load(open(path))
store["settings"].update(
    selected_model=model,
    vad_enabled=False,  # speaker-to-mic audio is below the VAD threshold
    selected_language="en",
    recording_retention_period="never",
    paste_method="ctrl_v",
    append_trailing_space=False,
    clipboard_handling="dont_modify",
)
json.dump(store, open(path, "w"), indent=2)

db = sqlite3.connect(f"{data}/fork.db")
now = int(time.time() * 1000)
db.execute("DELETE FROM dictionary_entries WHERE kind = 'rule'")
for frm, to in [
    ("wispr bro", "Wispr Bro"),
    ("whisper bro", "Wispr Bro"),
    ("whisperbro", "Wispr Bro"),
    ("anna", "Anna-Lena"),
]:
    db.execute(
        "INSERT INTO dictionary_entries (kind, from_text, to_text, created_at) VALUES ('rule', ?, ?, ?)",
        (frm, to, now),
    )
db.commit()
print("rules:", db.execute("SELECT from_text, to_text FROM dictionary_entries WHERE kind = 'rule'").fetchall())
