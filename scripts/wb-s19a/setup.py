# SPDX-License-Identifier: GPL-3.0-or-later
"""Mac test S-19a: prepares the portable test profile (never the owner's).

usage: setup.py <src-tauri/target/debug/Data> <speech model id> <cleanup gguf>
                <level> <styles json> <rules json>

  styles json  e.g. '{"personal":"very_relaxed","work":"upbeat"}' (merged
               into the fork setting `styles`)
  rules json   e.g. '{"com.apple.TextEdit":["personal",false]}': the
               `app_rules` rows to have (bundle id -> [group or null,
               auto_send]); every other row is deleted

Everything else as S-12a's setup.py: Parakeet v3, VAD off, spoken
languages en/nl/fr with automatic language, the pinned Muesli GGUF in the
profile's Hugging Face cache, the cleanup level. Run it with the app
closed (the app reads its rules at start), after one launch created the
files.
"""
import json
import os
import shutil
import sqlite3
import sys
import time

data, model, gguf, level, styles, rules = sys.argv[1:7]
if "/target/debug/Data" not in data:
    sys.exit("refusing: not a portable debug profile")
if level not in ("none", "light", "medium"):
    sys.exit("level must be none, light or medium")
styles = json.loads(styles)
rules = json.loads(rules)

path = f"{data}/settings_store.json"
store = json.load(open(path))
store["settings"].update(
    onboarding_completed=True,  # a returning user: the Hub, not onboarding
    selected_model=model,
    vad_enabled=False,  # speaker-to-mic audio is below the VAD threshold
    selected_language="auto",
    spoken_languages=["en", "nl", "fr"],
    recording_retention_period="never",
    paste_method="ctrl_v",
    append_trailing_space=False,
    clipboard_handling="dont_modify",
    auto_submit=False,  # Handy's global auto-submit stays off: only ours
)
json.dump(store, open(path, "w"), indent=2)

snap = os.path.join(
    data,
    "huggingface/hub/models--Muesli-HQ--qwen35-2b-muesli-cleanup-v12-gguf/snapshots",
    "e35af097be4f17d3673515fb321ce19aa300faf6",
)
os.makedirs(snap, exist_ok=True)
dest = os.path.join(snap, "qwen35-2b-muesli-v12-Q4_K_M.gguf")
if not os.path.exists(dest):
    shutil.copyfile(gguf, dest)

fork_settings = f"{data}/fork-settings.json"
fs = json.load(open(fork_settings)) if os.path.exists(fork_settings) else {}
s = fs.setdefault("settings", {})
s["cleanup_level"] = level
merged = {"personal": "relaxed", "work": "formal", "email": "formal", "other": "formal"}
merged.update(s.get("styles") or {})
merged.update(styles)
s["styles"] = merged
json.dump(fs, open(fork_settings, "w"), indent=2)

db = sqlite3.connect(f"{data}/fork.db")
now = int(time.time() * 1000)
db.execute("DELETE FROM app_rules")
for bundle, (group, auto) in rules.items():
    db.execute(
        'INSERT INTO app_rules ("match", kind, style_group, auto_send, source, app_name, updated_at)'
        " VALUES (?, 'app', ?, ?, 'user', NULL, ?)",
        (bundle, group, 1 if auto else 0, now),
    )
db.commit()
print(
    "level:", level,
    "| styles:", merged,
    "| rules:", db.execute('SELECT "match", style_group, auto_send FROM app_rules').fetchall(),
)
