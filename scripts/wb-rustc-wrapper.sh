#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Cargo rustc wrapper (src-tauri/.cargo/config.toml). Uses sccache when it is
# installed so every worktree shares one compile cache for dependencies;
# falls back to plain rustc otherwise (CI, fresh machines). Set
# WB_NO_SCCACHE=1 to bypass. See CONTEXT.md "Execution protocol".
if [ -z "$WB_NO_SCCACHE" ] && command -v sccache >/dev/null 2>&1; then
  export SCCACHE_CACHE_SIZE="${SCCACHE_CACHE_SIZE:-60G}"
  exec sccache "$@"
fi
exec "$@"
