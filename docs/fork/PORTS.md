# Ported code

Every file in this repo that contains code ported from another project, with
the exact source. Each row is also in `NOTICE` §3 (F-04; AGPL files in §4).
Ported files keep the
original copyright line plus a "Modified by Wispr Bro contributors, <date>"
notice. AGPL ports live in files marked `SPDX-License-Identifier:
AGPL-3.0-only` and never in a Handy file.

| Our file                                                            | Source repo                                                               | Commit                                                       | Source file(s)                                                                                                               | Licence                                            | Slice                        |
| ------------------------------------------------------------------- | ------------------------------------------------------------------------- | ------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------- | ---------------------------- |
| `src-tauri/src/fork/diagnostics/sanitizer.rs`                       | [voquill/voquill](https://github.com/voquill/voquill)                     | `a5dfbe0e3a35807271293a0ef02c91902abc0104`                   | `apps/desktop/src-tauri/src/utils/log_sanitizer.rs`                                                                          | AGPL-3.0 (© 2025-present Voquill, Inc.)           | F-08                         |
| `src-tauri/crates/wb-spike-ax/src/ax/element.rs`                    | [AbhishekBarali/SpeakoFlow](https://github.com/AbhishekBarali/SpeakoFlow) | `9818a17fcd124e5f1ea6d34780ddea9b154322d8`                   | `src-tauri/src/smart_spacing/macos.rs`                                                                                       | MIT (© 2026 Abhishek Barali)                      | RS-04 (spike; S-05 lifts it) |
| `src-tauri/crates/wb-spike-ax/src/ax/editable.rs`                   | [OpenWhispr/openwhispr](https://github.com/OpenWhispr/openwhispr)         | `b9e9ebefac4b961456c269164a93ddfdd53438b9`                   | `resources/macos-text-monitor.swift` (`isEditableTextElement`)                                                               | MIT (© 2024 OpenWhispr Team)                      | RS-04 (spike; S-05 lifts it) |
| `src-tauri/crates/wb-spike-ax/src/ax/lease.rs`, `.../ax/markers.rs` | [Beingpax/VoiceInk](https://github.com/Beingpax/VoiceInk)                 | `19546d2a7f85578f49efb538eb888b37ff2f8f8c`                   | `VoiceInk/Infrastructure/SystemIntegration/Accessibility/AccessibilityTextReader.swift`                                      | GPL-3.0 (© the VoiceInk authors)                  | RS-04 (spike; S-05 lifts it) |
| `src-tauri/crates/wb-spike-clipboard/src/clip/restore.rs`           | [anomalyco/hex](https://github.com/anomalyco/hex) + Handy                 | `3607595d92345af4be9f0eaa86ae3750c8851e39` (hex, branch dev) | hex `src/paste.rs` (`ClipboardRestore`); Handy `src-tauri/src/paste_tx/mod.rs` (`evaluate`)                                  | MIT (© 2026 Christopher Langton; © 2025 CJ Pais) | RS-05 (spike, not shipped)   |
| `src-tauri/crates/wb-spike-clipboard/src/clip/carbon.rs`            | [anomalyco/hex](https://github.com/anomalyco/hex)                         | `3607595d92345af4be9f0eaa86ae3750c8851e39`                   | `src/paste.rs` (`capture_clipboard`, `restore_clipboard`)                                                                    | MIT (© 2026 Christopher Langton)                  | RS-05 (spike, not shipped)   |
| `src-tauri/crates/wb-spike-clipboard/src/clip/target.rs`            | [anomalyco/hex](https://github.com/anomalyco/hex)                         | `3607595d92345af4be9f0eaa86ae3750c8851e39`                   | `src/paste.rs` (`chromium_major_versions`)                                                                                   | MIT (© 2026 Christopher Langton)                  | RS-05 (spike, not shipped)   |
| `src-tauri/crates/wb-spike-clipboard/src/clip/keys.rs`              | [anomalyco/hex](https://github.com/anomalyco/hex) + Handy                 | `3607595d92345af4be9f0eaa86ae3750c8851e39`                   | hex `src/keyboard.rs`, `crates/capture/src/capture/macos.rs` (`wait_for_modifiers_released`); Handy `src-tauri/src/input.rs` | MIT (© 2026 Christopher Langton; © 2025 CJ Pais) | RS-05 (spike, not shipped)   |

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

### `wb-spike-ax/src/ax/*` (RS-04 spike)

- The spike crate is a test tool and is never built into or shipped with the
  app, so these rows are not in `NOTICE` yet. When S-05 lifts the files into
  `src-tauri/src/fork/ax/`, the rows move to the new paths and into `NOTICE` §3.
- `element.rs`: SpeakoFlow's AX helpers and app helpers kept; timeout made a
  parameter; added raw-error reads, URL, rect/bounds, settable, setters,
  children, attribute-name lists. SpeakoFlow's caret-window read is
  re-expressed in `probe.rs`.
- `editable.rs`: OpenWhispr's text-role rule (disabled, secure subrole,
  TextField/TextArea/ComboBox + settable value or selected text); its
  "selection means not editable" check dropped; VoiceInk's `AXEditable` /
  `AXProtectedContent` checks re-expressed; fail-open rule as R5 §4.6 states.
- `lease.rs` / `markers.rs`: VoiceInk's per-PID refcount with restore of the
  previous value and retry back-off; the selected-text read through text
  markers. Swift → Rust; the lease is held 60 s after the last release
  (SpeakoFlow) instead of restored at refcount 0.

### `crates/wb-spike-clipboard/src/clip/` (RS-05 spike)

- A test tool, never built into the app, so these rows are not in `NOTICE`
  yet. When S-04 lifts the files into `src-tauri/src/fork/clipboard/`, the
  rows move to the new paths and the hex and Handy notices go into `NOTICE`
  §3.
- Read from the GitHub API (`gh api repos/anomalyco/hex/contents/...`, branch
  `dev`, head `3607595d92345af4be9f0eaa86ae3750c8851e39`; `src/paste.rs` last
  changed in `a8a4645d45c294aca85eb58f99f1d40a1569ddc8`). hex's `LICENSE`:
  "MIT License, Copyright (c) 2026 Christopher Langton", copied unchanged to
  `LICENSES/MIT-hex.txt` (Handy's is `LICENSES/MIT-Handy.txt`). Nothing was built
  or run from hex.
- Kept: the restore guard (`register`/generation/`changeCount`), the
  synthesised-flavor rule and Carbon capture/restore, the Chromium 155/156
  detection, the key-event specs, the 20-unit typing split and the
  modifier-release poll. Changed: decisions returned instead of acting
  inline, string errors, injectable clock and flag source, fn added to the
  modifier set, deadlines per target (R5 W-07).
