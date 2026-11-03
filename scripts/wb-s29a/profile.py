# SPDX-License-Identifier: GPL-3.0-or-later
"""Mac test S-29a: puts the portable test profile (never the owner's) at an
onboarding step.

usage: profile.py <Contents/MacOS/Data> fresh|<step>
  fresh   first run: Handy onboarding not completed, fork onboarding not
          started, no spoken languages (macOS languages are suggested)
  <step>  in progress at that step (welcome, move_to_applications,
          languages, permissions, mic_test, shortcut_test): the resume check
Run it with the app closed, after one launch created the files.
"""
import json
import sys

STEPS = ["welcome", "move_to_applications", "languages", "permissions",
         "mic_test", "shortcut_test"]

data, what = sys.argv[1:3]
if "/Contents/MacOS/Data" not in data or "s29a" not in data.lower():
    sys.exit("refusing: not the S-29a portable test profile")
if what != "fresh" and what not in STEPS:
    sys.exit(f"unknown step {what!r}")

path = f"{data}/settings_store.json"
store = json.load(open(path))
store["settings"].update(onboarding_completed=False, update_checks_enabled=False)
if what == "fresh":
    store["settings"]["spoken_languages"] = []
json.dump(store, open(path, "w"), indent=2, ensure_ascii=False)

fpath = f"{data}/fork-settings.json"
fork = json.load(open(fpath))
settings = fork.setdefault("settings", {})
settings["onboarding"] = (
    {"status": "not_started"} if what == "fresh"
    else {"status": "in_progress", "current_step": what}
)
json.dump(fork, open(fpath, "w"), indent=2, ensure_ascii=False)
print(f"profile: {what}")
