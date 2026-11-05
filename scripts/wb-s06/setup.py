# SPDX-License-Identifier: GPL-3.0-or-later
"""Mac test S-06: prepares the portable test profile (never the owner's).

usage: setup.py <src-tauri/target/debug/Data> <model id>

Run with the app closed, after one launch created the files
(`scripts/wb-s05/app.sh init`).

- Handy's settings as S-05 sets them (setup done, the copied speech model,
  VAD off for speaker-to-mic audio, the fork's ⌘V delivery, no trailing
  space, the clipboard left alone), but recordings are kept (Undo of a take
  cancelled mid-transcription and History retries need the WAV).
- Fork settings: cleanup off (no LLM download in a test profile), Press
  Enter on the mouse's back button, Undo on ⌃⌥Z; paste-last and copy-last
  keep their defaults (⌃⌘V, ⌃⌘C).
- One History dictation to paste and copy again: a row in Handy's
  history.db and its entry_meta row in fork.db, with a fixed text that
  contains no personal data.
"""
import json
import sqlite3
import sys
import time

data, model = sys.argv[1:3]
if "/target/debug/Data" not in data:
    sys.exit("refusing: not a portable debug profile")

LAST_TEXT = "Recovery check: the quick brown fox jumps over the lazy dog."

path = f"{data}/settings_store.json"
store = json.load(open(path))
store["settings"].update(
    onboarding_completed=True,
    selected_model=model,
    vad_enabled=False,
    selected_language="en",
    recording_retention_period="preserve_limit",
    paste_method="ctrl_v",
    append_trailing_space=False,
    clipboard_handling="dont_modify",
    auto_submit=False,
    theme="light",
)
json.dump(store, open(path, "w"), indent=2, ensure_ascii=False)
print("handy settings:", {k: store["settings"][k] for k in ("selected_model", "paste_method", "vad_enabled")})

# Fork settings: their own store file next to Handy's (F-07).
fpath = f"{data}/fork-settings.json"
fstore = json.load(open(fpath))
fs = fstore["settings"]
fs["cleanup_level"] = "none"
fs["hotkeys"]["press_enter"] = [{"chord": "mousex1", "activation": "press"}]
fs["hotkeys"]["undo_cancel"] = [{"chord": "ctrl+option+z", "activation": "press"}]
fs["onboarding"]["status"] = "completed"
json.dump(fstore, open(fpath, "w"), indent=2, ensure_ascii=False)
print("fork settings:", {k: fs["hotkeys"][k] for k in ("paste_last", "copy_last", "press_enter", "undo_cancel")})

# The last dictation.
now_ms = int(time.time() * 1000)
h = sqlite3.connect(f"{data}/history.db")
cols = [r[1] for r in h.execute("PRAGMA table_info(transcription_history)")]
row = {
    "file_name": f"wispr-bro-{now_ms}.wav",
    "timestamp": now_ms // 1000,
    "saved": 0,
    "title": "S-06 seed",
    "transcription_text": LAST_TEXT,
}
row = {k: v for k, v in row.items() if k in cols}
cur = h.execute(
    f"INSERT INTO transcription_history ({', '.join(row)}) VALUES ({', '.join('?' for _ in row)})",
    list(row.values()),
)
entry_id = cur.lastrowid
h.commit()
h.close()
f = sqlite3.connect(f"{data}/fork.db")
f.execute(
    "INSERT OR REPLACE INTO entry_meta (entry_id, created_at, asr_text, final_text, source, word_count, char_count) "
    "VALUES (?, ?, ?, ?, 'dictation', ?, ?)",
    (entry_id, now_ms, LAST_TEXT, LAST_TEXT, len(LAST_TEXT.split()), len(LAST_TEXT)),
)
f.commit()
f.close()
print("seeded last dictation: entry", entry_id)
