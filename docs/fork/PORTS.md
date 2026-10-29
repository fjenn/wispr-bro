# Ported code

Every file in this repo that contains code ported from another project, with
the exact source. Each row is also in `NOTICE` §3 (F-04; AGPL files in §4).
Ported files keep the
original copyright line plus a "Modified by Wispr Bro contributors, <date>"
notice. AGPL ports live in files marked `SPDX-License-Identifier:
AGPL-3.0-only` and never in a Handy file.

| Our file                                      | Source repo                                           | Commit                                     | Source file(s)                                      | Licence                                  | Slice |
| --------------------------------------------- | ----------------------------------------------------- | ------------------------------------------ | --------------------------------------------------- | ---------------------------------------- | ----- |
| `src-tauri/src/fork/diagnostics/sanitizer.rs` | [voquill/voquill](https://github.com/voquill/voquill) | `a5dfbe0e3a35807271293a0ef02c91902abc0104` | `apps/desktop/src-tauri/src/utils/log_sanitizer.rs` | AGPL-3.0 (© 2025-present Voquill, Inc.) | F-08  |

## Notes per port

### `fork/diagnostics/sanitizer.rs` (F-08)

- Kept: the five transcript rules (`Received transcript` preview,
  `Processed transcript`, `LLM raw output`, `finalizing with transcript`,
  `final transcript`), `sanitize_log_content`, and all 14 original tests.
- Added: home-directory masking (`$HOME` → `~`), e-mail addresses, bearer
  tokens, API-key shapes (`sk-…`, `hf_…`, `gsk_…`), `transcript:` / `prompt:` /
  `custom words:` style lines, and JSON fields that carry user text or device
  names; `sanitize_with_home` for tests; tests for each addition.
- Voquill's `enterprise/` directory (proprietary) was not read or used. Only
  this one file was read, from a read-only shallow clone; nothing was built
  or run.
