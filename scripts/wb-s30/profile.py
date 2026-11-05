# SPDX-License-Identifier: GPL-3.0-or-later
"""Mac test S-30: puts the portable test profile (never the owner's) right
after setup: onboarding completed, no first-use choice, empty checklist.

usage: profile.py <Contents/MacOS/Data> [choice]
  choice  also store this first_use_choice (skips the chooser)
Run it with the app closed, after one launch created the files.
"""
import json
import sys

data = sys.argv[1]
choice = sys.argv[2] if len(sys.argv) > 2 else None
if "/Contents/MacOS/Data" not in data or "s30" not in data.lower():
    sys.exit("refusing: not the S-30 portable test profile")

path = f"{data}/settings_store.json"
store = json.load(open(path))
store["settings"].update(onboarding_completed=True, update_checks_enabled=False)
json.dump(store, open(path, "w"), indent=2, ensure_ascii=False)

fpath = f"{data}/fork-settings.json"
fork = json.load(open(fpath))
settings = fork.setdefault("settings", {})
settings["onboarding"] = {
    "status": "completed",
    "current_step": None,
    "completed_steps": [],
    "checklist_done": [],
    "completed_at_ms": 1,
}
settings["first_use_choice"] = choice
settings["dismissed_banners"] = [
    b for b in settings.get("dismissed_banners", [])
    if b not in ("first-use.chooser", "first-use.own-apps", "hub.checklist")
]
settings["bar_always_visible"] = True
settings["bar_hidden_until_ms"] = None
json.dump(fork, open(fpath, "w"), indent=2, ensure_ascii=False)
print(f"profile: completed, choice {choice}")
