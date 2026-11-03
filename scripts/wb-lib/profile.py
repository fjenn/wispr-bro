# SPDX-License-Identifier: GPL-3.0-or-later
"""Seeds a test identity's data folder for a Mac-wave driver (never the
owner's: the folder must belong to a `com.wisprbro.<slice>check` identity or
a portable `Contents/MacOS/Data`).

  profile.py unbind <data dir> [--keep id,id...] [--set id=chord[/activation]...]
      Every global shortcut unbound (fork-settings.json `hotkeys` = [] for
      each action), so a test app running next to the owner's Wispr Bro
      never answers fn, fn+Space, fn+Ctrl, ⌃⌘V, ⌃⌘C, ⌥1, ⌥2, ⌥O or ⌥S
      (Mac wave 2 finding 7). `--keep` leaves the listed actions on their
      current chord (or its default); `--set` binds an action to a chord
      (activation `press` unless given, e.g. scratchpad.open=option+s).
      Use either only for a check that needs that key. Esc (`cancel`) stays: it
      is only live during a take. Works before the first launch (writes a
      minimal file at the current schema version) and on an existing file
      (other settings kept). Takes then run by SIGUSR2 (Handy's toggle).

  profile.py models <data dir> [--from <huggingface dir>]
      Clones a model cache into <data dir>/huggingface (APFS `cp -c`, no
      extra disk space; plain copy elsewhere) so a fresh test identity does
      not download ~1.6 GB from Hugging Face again. Without --from: the
      newest non-empty `~/Library/Application Support/com.wisprbro.*check/
      huggingface`, else WB_MODEL_CACHE. Nothing to clone is not an error
      (the app then downloads as usual).

Run with the app closed.
"""
import glob
import json
import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))

# Every action with a default chord (`default_hotkeys()` in
# src-tauri/src/fork/settings.rs; app.test.ts checks this list against it)
# plus the Scratchpad's runtime id, which has none by default but is often
# set to ⌥S.
DEFAULTS = {
    "dictate": [{"chord": "fn", "activation": "hold"}],
    "hands_free": [
        {"chord": "fn", "activation": "double_tap"},
        {"chord": "fn+space", "activation": "press"},
    ],
    "command_mode": [{"chord": "fn+ctrl", "activation": "hold"}],
    "paste_last": [{"chord": "ctrl+command+v", "activation": "press"}],
    "copy_last": [{"chord": "ctrl+command+c", "activation": "press"}],
    "transform.polish": [{"chord": "option+1", "activation": "press"}],
    "transform.prompt_engineer": [{"chord": "option+2", "activation": "press"}],
    "transform.view_changes": [{"chord": "option+o", "activation": "press"}],
    "scratchpad.open": [],
    "press_enter": [],
    "undo_cancel": [],
}
# Live only while a take runs; never unbound.
KEEP_ALWAYS = {"cancel"}


def refuse_unless_test_profile(data):
    real = os.path.realpath(data)
    if "/Contents/MacOS/Data" in real:
        return
    m = re.search(r"/Application Support/(com\.wisprbro\.[A-Za-z0-9]+)(/|$)", real)
    if m and re.fullmatch(r"com\.wisprbro\.[A-Za-z0-9]+check", m.group(1)):
        return
    sys.exit(f"refusing: {data} is not a test identity's data folder")


def schema_version():
    src = open(os.path.join(ROOT, "src-tauri/src/fork/settings.rs")).read()
    return int(re.search(r"CURRENT_SCHEMA_VERSION: u32 = (\d+);", src).group(1))


def unbind(data, keep, chords=None):
    chords = chords or {}
    refuse_unless_test_profile(data)
    unknown = [k for k in keep if k not in DEFAULTS]
    if unknown:
        sys.exit(f"unknown action(s) to keep: {', '.join(unknown)}")
    os.makedirs(data, exist_ok=True)
    path = os.path.join(data, "fork-settings.json")
    if os.path.exists(path):
        store = json.load(open(path))
    else:
        store = {"settings": {"schema_version": schema_version()}}
    settings = store.setdefault("settings", {})
    hotkeys = settings.get("hotkeys")
    if not isinstance(hotkeys, dict):
        # A file without `hotkeys` would take every default: start from
        # the defaults so the ones kept keep their chord.
        hotkeys = {k: v for k, v in DEFAULTS.items()}
        hotkeys["cancel"] = [{"chord": "escape", "activation": "press"}]
    for action in set(DEFAULTS) | set(hotkeys):
        if action in KEEP_ALWAYS:
            continue
        if action in keep:
            hotkeys[action] = hotkeys.get(action) or DEFAULTS.get(action, [])
        else:
            hotkeys[action] = []
    for action, (chord, activation) in chords.items():
        hotkeys[action] = [{"chord": chord, "activation": activation}]
    settings["hotkeys"] = hotkeys
    json.dump(store, open(path, "w"), indent=2, ensure_ascii=False)
    live = sorted(k for k, v in hotkeys.items() if v)
    print(f"profile: shortcuts unbound except {', '.join(live) or 'none'}")


def default_cache():
    home = os.path.expanduser("~")
    found = []
    for d in glob.glob(f"{home}/Library/Application Support/com.wisprbro.*check/huggingface"):
        if os.path.isdir(d) and os.listdir(d):
            found.append((os.path.getmtime(d), d))
    if found:
        return max(found)[1]
    env = os.environ.get("WB_MODEL_CACHE")
    return env if env and os.path.isdir(env) else None


def models(data, source):
    refuse_unless_test_profile(data)
    source = source or default_cache()
    dest = os.path.join(data, "huggingface")
    if not source:
        print("profile: no model cache to clone; the app downloads its models")
        return
    if os.path.realpath(source) == os.path.realpath(dest):
        print("profile: model cache already in place")
        return
    if os.path.exists(dest):
        shutil.rmtree(dest)
    os.makedirs(data, exist_ok=True)
    # `cp -c` clones on APFS (copy-on-write); -R keeps the hub's symlinks.
    if subprocess.run(["cp", "-cR", source, dest]).returncode != 0:
        shutil.copytree(source, dest, symlinks=True)
    print(f"profile: model cache cloned from {source}")


def main(argv):
    if len(argv) < 3 or argv[1] not in ("unbind", "models"):
        sys.exit(__doc__)
    what, data, rest = argv[1], argv[2], argv[3:]
    if what == "unbind":
        keep, chords = [], {}
        while len(rest) >= 2:
            flag, value, rest = rest[0], rest[1], rest[2:]
            if flag == "--keep":
                keep += [k for k in value.split(",") if k]
            elif flag == "--set" and "=" in value:
                action, binding = value.split("=", 1)
                chord, _, activation = binding.partition("/")
                chords[action] = (chord, activation or "press")
            else:
                sys.exit(__doc__)
        if rest:
            sys.exit(__doc__)
        unbind(data, keep, chords)
    else:
        src = rest[1] if rest[:1] == ["--from"] and len(rest) > 1 else None
        models(data, src)


if __name__ == "__main__":
    main(sys.argv)
