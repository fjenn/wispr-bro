#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Git merge driver for the fork's locale files (fork.json, fork.flags.json).

Parallel slices add keys to the same 26 locale files, so text merges conflict
on almost every merge. This merges them as JSON instead, key by key, three
ways: a key changed on one side takes that side; a key deleted on one side and
unchanged on the other is deleted; a key changed differently on both sides
keeps ours and fails the merge (exit 1) so a person decides. Output keeps
"ours" key order, new keys from "theirs" appended, 2-space indent.

Registered by `scripts/wb-setup-git.sh`:
  git config merge.wbjson.driver "python3 scripts/wb-merge-json.py %O %A %B"
and `.gitattributes`: src/i18n/locales/*/fork*.json merge=wbjson
"""
import json
import sys

MISSING = object()


def load(path):
    try:
        with open(path, encoding="utf-8") as f:
            text = f.read()
        return json.loads(text) if text.strip() else {}
    except (OSError, ValueError):
        return None


def merge(base, ours, theirs, path, conflicts):
    if isinstance(ours, dict) and isinstance(theirs, dict):
        b = base if isinstance(base, dict) else {}
        out = {}
        keys = list(ours) + [k for k in theirs if k not in ours]
        for k in keys:
            v = merge(b.get(k, MISSING), ours.get(k, MISSING), theirs.get(k, MISSING), path + [k], conflicts)
            if v is not MISSING:
                out[k] = v
        return out
    if ours == theirs:
        return ours
    if ours == base:
        return theirs
    if theirs == base:
        return ours
    conflicts.append(".".join(path))
    return ours


def main():
    base_p, ours_p, theirs_p = sys.argv[1:4]
    base, ours, theirs = load(base_p), load(ours_p), load(theirs_p)
    if ours is None or theirs is None:
        return 1  # not valid JSON on a side: leave it to git's text merge
    conflicts = []
    out = merge(base if base is not None else {}, ours, theirs, [], conflicts)
    with open(ours_p, "w", encoding="utf-8") as f:
        f.write(json.dumps(out, ensure_ascii=False, indent=2) + "\n")
    for c in conflicts:
        print(f"wb-merge-json: both sides changed {c}; kept ours", file=sys.stderr)
    return 1 if conflicts else 0


if __name__ == "__main__":
    sys.exit(main())
