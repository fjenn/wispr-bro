# SPDX-License-Identifier: GPL-3.0-or-later
"""Mac test S-12a: prepares the portable test profile (never the owner's).

usage: setup.py <src-tauri/target/debug/Data> <speech model id> <cleanup gguf> <level>
Sets Handy's settings for the run (Parakeet v3, VAD off, the owner's spoken
languages en/nl/fr with automatic language), puts the pinned Muesli GGUF in
the profile's Hugging Face cache (the app checks its sha256, so nothing is
downloaded), sets the fork cleanup level and replaces the dictionary
replacements with one the stage-9 check needs. Run it with the app closed,
after one launch created the files.
"""
import json
import os
import shutil
import sqlite3
import sys
import time

data, model, gguf, level = sys.argv[1:5]
if "/target/debug/Data" not in data:
    sys.exit("refusing: not a portable debug profile")
if level not in ("none", "light", "medium"):
    sys.exit("level must be none, light or medium")

path = f"{data}/settings_store.json"
store = json.load(open(path))
store["settings"].update(
    selected_model=model,
    vad_enabled=False,  # speaker-to-mic audio is below the VAD threshold
    selected_language="auto",
    spoken_languages=["en", "nl", "fr"],
    recording_retention_period="never",
    paste_method="ctrl_v",
    append_trailing_space=False,
    clipboard_handling="dont_modify",
)
json.dump(store, open(path, "w"), indent=2)

# The pinned cleanup model (catalog.rs MUESLI_2B_V12), as hf-hub stores it.
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
fs.setdefault("settings", {})["cleanup_level"] = level  # store key STORE_KEY
json.dump(fs, open(fork_settings, "w"), indent=2)

db = sqlite3.connect(f"{data}/fork.db")
now = int(time.time() * 1000)
db.execute("DELETE FROM dictionary_entries WHERE kind = 'rule'")
for frm in ("whisper bro", "whisperbro"):
    db.execute(
        "INSERT INTO dictionary_entries (kind, from_text, to_text, created_at) VALUES ('rule', ?, ?, ?)",
        (frm, "Wispr Bro", now),
    )
db.commit()
print("level:", level, "| rules:", db.execute("SELECT from_text, to_text FROM dictionary_entries WHERE kind = 'rule'").fetchall())
