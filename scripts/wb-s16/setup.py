# SPDX-License-Identifier: GPL-3.0-or-later
"""Mac test S-16: prepares the portable test profile (never the owner's).

usage: setup.py <src-tauri/target/debug/Data> <model id>
Sets Handy's settings for the run, as a returning user who still has two
custom words from Handy (Siobhán, Kubernetes) and no dictionary yet, so the
first S-16 launch shows the one-time migration. Empties the dictionary and
the custom-words mirror in fork.db. Run it with the app closed, after one
launch created the files.
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
    custom_words=["Siobhán", "Kubernetes"],
    theme="light",
)
json.dump(store, open(path, "w"), indent=2, ensure_ascii=False)

db = sqlite3.connect(f"{data}/fork.db")
db.execute("DELETE FROM dictionary_entries")
db.execute("DELETE FROM dictionary_handy_mirror")
db.commit()
print("custom_words:", store["settings"]["custom_words"])
print("dictionary rows:", db.execute("SELECT COUNT(*) FROM dictionary_entries").fetchone()[0])
