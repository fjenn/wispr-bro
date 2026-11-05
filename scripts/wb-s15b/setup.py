# SPDX-License-Identifier: GPL-3.0-or-later
"""Mac test S-15b: prepares the portable test profile (never the owner's).

usage: python3 -I scripts/wb-s15b/setup.py <…/Contents/MacOS/Data> <model id>

Sets Handy's settings as a returning user (the Hub shows, no update
checks, recordings kept, the speech model for Retry) and empties the
dictionary, so the Flag check starts from nothing. Run it with the app
closed, after one launch created the files.
"""
import json
import sqlite3
import sys

data, model = sys.argv[1:3]
if "/Data" not in data or "s15b" not in data:
    sys.exit("refusing: not the S-15b portable test profile")

path = f"{data}/settings_store.json"
store = json.load(open(path))
store["settings"].update(
    onboarding_completed=True,
    update_checks_enabled=False,
    selected_model=model,
    selected_language="en",
    recording_retention_period="never",
    theme="light",
    app_language="en",
)
json.dump(store, open(path, "w"), indent=2, ensure_ascii=False)

db = sqlite3.connect(f"{data}/fork.db")
db.execute("DELETE FROM dictionary_entries")
db.commit()
print("ok: model", model)
