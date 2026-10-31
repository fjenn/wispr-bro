# SPDX-License-Identifier: GPL-3.0-or-later
"""Mac test S-05: prepares the portable test profile (never the owner's).

usage: setup.py <src-tauri/target/debug/Data> <model id>
Sets Handy's settings for the run: setup done (the resting bar shows),
the copied speech model, VAD off (speaker-to-mic audio is quiet), ⌘V
delivery (the fork's paste path), no trailing space, the clipboard left
alone. Run it with the app closed, after one launch created the files.
"""
import json
import sys

data, model = sys.argv[1:3]
if "/target/debug/Data" not in data:
    sys.exit("refusing: not a portable debug profile")

path = f"{data}/settings_store.json"
store = json.load(open(path))
store["settings"].update(
    onboarding_completed=True,
    selected_model=model,
    vad_enabled=False,
    selected_language="en",
    recording_retention_period="never",
    paste_method="ctrl_v",
    append_trailing_space=False,
    clipboard_handling="dont_modify",
    theme="light",
)
json.dump(store, open(path, "w"), indent=2, ensure_ascii=False)
print("settings:", {k: store["settings"][k] for k in ("selected_model", "paste_method", "vad_enabled")})
