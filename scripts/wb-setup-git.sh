#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# One-time per clone: register the fork's git merge drivers (CONTEXT.md,
# "Execution protocol"). Worktrees share the clone's config.
set -e
cd "$(git rev-parse --show-toplevel)"
git config merge.wbjson.name "Wispr Bro locale JSON merge"
git config merge.wbjson.driver 'python3 "$(git rev-parse --show-toplevel)/scripts/wb-merge-json.py" %O %A %B'
echo "wb merge driver registered"
