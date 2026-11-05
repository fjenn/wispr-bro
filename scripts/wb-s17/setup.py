# SPDX-License-Identifier: GPL-3.0-or-later
"""Mac test S-17: prepares the portable test profile (never the owner's).

usage: setup.py <src-tauri/target/debug/Data> <model id>
Sets Handy's settings for the run (returning user, English, ⌘V paste, no
VAD, no LLM post-processing) and empties the snippets and the dictionary
rules in fork.db, so the snippet is added through the page. Run it with
the app closed, after one launch created the files.
"""
import json
import sqlite3
import sys

data, model = sys.argv[1:3]
if "/target/debug/Data" not in data:
    sys.exit("refusing: not a portable debug profile")

path = f"{data}/settings_store.json"
store = json.load(open(path))
store["settings"].update(
    onboarding_completed=True,
    selected_model=model,
    vad_enabled=False,  # speaker-to-mic audio is below the VAD threshold
    selected_language="en",
    recording_retention_period="never",
    paste_method="ctrl_v",
    append_trailing_space=False,
    clipboard_handling="dont_modify",
    post_process_enabled=False,
    theme="light",
)
json.dump(store, open(path, "w"), indent=2, ensure_ascii=False)

db = sqlite3.connect(f"{data}/fork.db")
db.execute("DELETE FROM snippets")
db.execute("DELETE FROM dictionary_entries WHERE kind = 'rule'")
db.commit()
print("snippets:", db.execute("SELECT COUNT(*) FROM snippets").fetchone()[0])
