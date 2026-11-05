# SPDX-License-Identifier: GPL-3.0-or-later
"""Mac test S-08: prepares the portable test profile (never the owner's).

usage: setup.py <Contents/MacOS/Data> <model id> [mute=0|1]
A returning user who never chose a microphone (Handy's
`selected_microphone` unset, fork `mic_follow_system` off), sounds on, a
local custom model, no paste (the text stays in History), and optionally
"Mute all audio while dictating". Run it with the app closed, after one
launch created the files.
"""
import json
import sys

data, model = sys.argv[1:3]
mute = len(sys.argv) > 3 and sys.argv[3] == "1"
if "/Contents/MacOS/Data" not in data or "s08" not in data.lower():
    sys.exit("refusing: not the S-08 portable test profile")

path = f"{data}/settings_store.json"
store = json.load(open(path))
store["settings"].update(
    onboarding_completed=True,
    selected_model=model,
    vad_enabled=False,  # speaker-to-mic audio is below the VAD threshold
    selected_language="en",
    recording_retention_period="never",
    paste_method="none",
    clipboard_handling="dont_modify",
    selected_microphone=None,
    audio_feedback=True,
    mute_while_recording=mute,
    update_checks_enabled=False,
    theme="light",
)
json.dump(store, open(path, "w"), indent=2, ensure_ascii=False)

fpath = f"{data}/fork-settings.json"
try:
    fork = json.load(open(fpath))
    fork.setdefault("settings", {})["mic_follow_system"] = False
    fork["settings"]["update_checks_enabled"] = False
    json.dump(fork, open(fpath, "w"), indent=2)
except FileNotFoundError:
    pass
print("ok: mute_while_recording =", mute)
