# Ported code

Every file in this repo that contains code ported from another project, with
the exact source. Each row is also in `NOTICE` §3 (F-04; AGPL files in §4).
Ported files keep the
original copyright line plus a "Modified by Wispr Bro contributors, <date>"
notice. AGPL ports live in files marked `SPDX-License-Identifier:
AGPL-3.0-only` and never in a Handy file.

| Our file                                                                                  | Source repo                                                                                                                                                                                                                                                                                                                                                        | Commit                                                                                                             | Source file(s)                                                                                                                       | Licence                                                                | Slice                        |
| ----------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------- | ---------------------------- |
| `src-tauri/src/fork/diagnostics/sanitizer.rs`                                             | [voquill/voquill](https://github.com/voquill/voquill)                                                                                                                                                                                                                                                                                                              | `a5dfbe0e3a35807271293a0ef02c91902abc0104`                                                                         | `apps/desktop/src-tauri/src/utils/log_sanitizer.rs`                                                                                  | AGPL-3.0 (© 2025-present Voquill, Inc.)                               | F-08                         |
| `src-tauri/crates/wb-spike-ax/src/ax/element.rs`                                          | [AbhishekBarali/SpeakoFlow](https://github.com/AbhishekBarali/SpeakoFlow)                                                                                                                                                                                                                                                                                          | `9818a17fcd124e5f1ea6d34780ddea9b154322d8`                                                                         | `src-tauri/src/smart_spacing/macos.rs`                                                                                               | MIT (© 2026 Abhishek Barali)                                          | RS-04 (spike; S-05 lifts it) |
| `src-tauri/crates/wb-spike-ax/src/ax/editable.rs`                                         | [OpenWhispr/openwhispr](https://github.com/OpenWhispr/openwhispr)                                                                                                                                                                                                                                                                                                  | `b9e9ebefac4b961456c269164a93ddfdd53438b9`                                                                         | `resources/macos-text-monitor.swift` (`isEditableTextElement`)                                                                       | MIT (© 2024 OpenWhispr Team)                                          | RS-04 (spike; S-05 lifts it) |
| `src-tauri/crates/wb-spike-ax/src/ax/lease.rs`, `.../ax/markers.rs`                       | [Beingpax/VoiceInk](https://github.com/Beingpax/VoiceInk)                                                                                                                                                                                                                                                                                                          | `19546d2a7f85578f49efb538eb888b37ff2f8f8c`                                                                         | `VoiceInk/Infrastructure/SystemIntegration/Accessibility/AccessibilityTextReader.swift`                                              | GPL-3.0 (© the VoiceInk authors)                                      | RS-04 (spike; S-05 lifts it) |
| `src-tauri/src/fork/clipboard/restore.rs`                                                 | [anomalyco/hex](https://github.com/anomalyco/hex) + Handy                                                                                                                                                                                                                                                                                                          | `3607595d92345af4be9f0eaa86ae3750c8851e39` (hex, branch dev)                                                       | hex `src/paste.rs` (`ClipboardRestore`); Handy `src-tauri/src/paste_tx/mod.rs` (`evaluate`)                                          | MIT (© 2026 Christopher Langton; © 2025 CJ Pais)                     | RS-05, shipped by S-04       |
| `src-tauri/crates/wb-spike-clipboard/src/clip/carbon.rs`                                  | [anomalyco/hex](https://github.com/anomalyco/hex)                                                                                                                                                                                                                                                                                                                  | `3607595d92345af4be9f0eaa86ae3750c8851e39`                                                                         | `src/paste.rs` (`capture_clipboard`, `restore_clipboard`)                                                                            | MIT (© 2026 Christopher Langton)                                      | RS-05 (spike, not shipped)   |
| `src-tauri/src/fork/clipboard/target.rs`                                                  | [anomalyco/hex](https://github.com/anomalyco/hex)                                                                                                                                                                                                                                                                                                                  | `3607595d92345af4be9f0eaa86ae3750c8851e39`                                                                         | `src/paste.rs` (`chromium_major_versions`)                                                                                           | MIT (© 2026 Christopher Langton)                                      | RS-05, shipped by S-04       |
| `src-tauri/src/fork/clipboard/keys.rs`                                                    | [anomalyco/hex](https://github.com/anomalyco/hex) + Handy                                                                                                                                                                                                                                                                                                          | `3607595d92345af4be9f0eaa86ae3750c8851e39`                                                                         | hex `src/keyboard.rs`, `crates/capture/src/capture/macos.rs` (`wait_for_modifiers_released`); Handy `src-tauri/src/input.rs`         | MIT (© 2026 Christopher Langton; © 2025 CJ Pais)                     | RS-05, shipped by S-04       |
| `src-tauri/crates/wb-spike-pill/src/pill.rs`                                              | [kozliq/Tolkovo](https://github.com/kozliq/Tolkovo)                                                                                                                                                                                                                                                                                                                | `ffc5b6231eacc0c5ba8ab59634ad0f2dd4b44b35`                                                                         | `src-tauri/src/tolkovo/pill.rs` (panel set-up, `prevent_activation`, Space observer)                                                 | MIT (© 2025 CJ Pais; © 2026 kozliq)                                  | RS-03 (spike; S-03 lifts it) |
| `src-tauri/crates/wb-spike-pill/tools/vdisplay.m`                                         | [Stengo/DeskPad](https://github.com/Stengo/DeskPad)                                                                                                                                                                                                                                                                                                                | `c3349f0e237e000cb4826fb3ea1cdd1c44949461`                                                                         | `DeskPad/CGVirtualDisplayPrivate.h` (declarations only)                                                                              | MIT (© 2022 Bastian Andelefski)                                       | RS-03 (spike, not shipped)   |
| `src-tauri/crates/wb-handy-recorder/` (vendored crate, `[patch.crates-io]`)               | [handy-computer/handy-recorder](https://github.com/handy-computer/handy-recorder)                                                                                                                                                                                                                                                                                  | `1d975fe8c10f9a69d778cf16955c7e0699b4b8e1`                                                                         | `crates/handy-recorder` (0.1.0 from crates.io)                                                                                       | MIT (© 2026 handy-computer)                                           | S-01                         |
| `src-tauri/src/fork/shortcut/tests/ported.rs` (test vectors only)                         | [fastrepl/anarlog](https://github.com/fastrepl/anarlog), [moona3k/macparakeet](https://github.com/moona3k/macparakeet), [altic-dev/FluidVoice](https://github.com/altic-dev/FluidVoice), [OpenWhispr/openwhispr](https://github.com/OpenWhispr/openwhispr), [anomalyco/hex](https://github.com/anomalyco/hex), [kitlangton/Hex](https://github.com/kitlangton/Hex) | `a82755f4`, `a388b7d4`, `ea94c8a6`, `b9e9ebef`, `ed8c18e9`, `3e300721`                                             | see notes                                                                                                                            | MIT (anarlog, OpenWhispr, hex, Hex); GPL-3.0 (MacParakeet, FluidVoice) | S-01                         |
| `src-tauri/src/fork/stats/streak.rs`                                                      | [AbhishekBarali/SpeakoFlow](https://github.com/AbhishekBarali/SpeakoFlow)                                                                                                                                                                                                                                                                                          | `9818a17fcd124e5f1ea6d34780ddea9b154322d8`                                                                         | `src-tauri/src/managers/history.rs` (`compute_streaks`)                                                                              | MIT (© 2026 Abhishek Barali)                                          | S-09                         |
| `src-tauri/src/fork/context/categories.json`, `.../context/categories.rs` (matching rule) | [OpenWhispr/openwhispr](https://github.com/OpenWhispr/openwhispr) + [mazdak/AudioWhisper](https://github.com/mazdak/AudioWhisper)                                                                                                                                                                                                                                  | `b9e9ebefac4b961456c269164a93ddfdd53438b9` (OpenWhispr), `71fed3683d6e62f631c606003e57430622be0b2b` (AudioWhisper) | OpenWhispr `src/helpers/toneAppCategories.js`; AudioWhisper `Sources/Managers/AppCategoryManager.swift`                              | MIT (© 2024 OpenWhispr Team; © 2024 AudioWhisper Contributors)       | S-09                         |
| Our file                                                                                  | Source repo                                                                                                                                                                                                                                                                                                                                                        | Commit                                                                                                             | Source file(s)                                                                                                                       | Licence                                                                | Slice                        |
| -------------------------------------------------------------------------------           | -----------------------------------------------------------------------------                                                                                                                                                                                                                                                                                      | ------------------------------------------------------------                                                       | ------------------------------------------------------------------------------------------------------------------------------       | --------------------------------------------------                     | ---------------------------- |
| `src-tauri/src/fork/diagnostics/sanitizer.rs`                                             | [voquill/voquill](https://github.com/voquill/voquill)                                                                                                                                                                                                                                                                                                              | `a5dfbe0e3a35807271293a0ef02c91902abc0104`                                                                         | `apps/desktop/src-tauri/src/utils/log_sanitizer.rs`                                                                                  | AGPL-3.0 (© 2025-present Voquill, Inc.)                               | F-08                         |
| `src-tauri/crates/wb-spike-ax/src/ax/element.rs`                                          | [AbhishekBarali/SpeakoFlow](https://github.com/AbhishekBarali/SpeakoFlow)                                                                                                                                                                                                                                                                                          | `9818a17fcd124e5f1ea6d34780ddea9b154322d8`                                                                         | `src-tauri/src/smart_spacing/macos.rs`                                                                                               | MIT (© 2026 Abhishek Barali)                                          | RS-04 (spike; S-05 lifts it) |
| `src-tauri/crates/wb-spike-ax/src/ax/editable.rs`                                         | [OpenWhispr/openwhispr](https://github.com/OpenWhispr/openwhispr)                                                                                                                                                                                                                                                                                                  | `b9e9ebefac4b961456c269164a93ddfdd53438b9`                                                                         | `resources/macos-text-monitor.swift` (`isEditableTextElement`)                                                                       | MIT (© 2024 OpenWhispr Team)                                          | RS-04 (spike; S-05 lifts it) |
| `src-tauri/crates/wb-spike-ax/src/ax/lease.rs`, `.../ax/markers.rs`                       | [Beingpax/VoiceInk](https://github.com/Beingpax/VoiceInk)                                                                                                                                                                                                                                                                                                          | `19546d2a7f85578f49efb538eb888b37ff2f8f8c`                                                                         | `VoiceInk/Infrastructure/SystemIntegration/Accessibility/AccessibilityTextReader.swift`                                              | GPL-3.0 (© the VoiceInk authors)                                      | RS-04 (spike; S-05 lifts it) |
| `src-tauri/src/fork/clipboard/restore.rs`                                                 | [anomalyco/hex](https://github.com/anomalyco/hex) + Handy                                                                                                                                                                                                                                                                                                          | `3607595d92345af4be9f0eaa86ae3750c8851e39` (hex, branch dev)                                                       | hex `src/paste.rs` (`ClipboardRestore`); Handy `src-tauri/src/paste_tx/mod.rs` (`evaluate`)                                          | MIT (© 2026 Christopher Langton; © 2025 CJ Pais)                     | RS-05, shipped by S-04       |
| `src-tauri/crates/wb-spike-clipboard/src/clip/carbon.rs`                                  | [anomalyco/hex](https://github.com/anomalyco/hex)                                                                                                                                                                                                                                                                                                                  | `3607595d92345af4be9f0eaa86ae3750c8851e39`                                                                         | `src/paste.rs` (`capture_clipboard`, `restore_clipboard`)                                                                            | MIT (© 2026 Christopher Langton)                                      | RS-05 (spike, not shipped)   |
| `src-tauri/src/fork/clipboard/target.rs`                                                  | [anomalyco/hex](https://github.com/anomalyco/hex)                                                                                                                                                                                                                                                                                                                  | `3607595d92345af4be9f0eaa86ae3750c8851e39`                                                                         | `src/paste.rs` (`chromium_major_versions`)                                                                                           | MIT (© 2026 Christopher Langton)                                      | RS-05, shipped by S-04       |
| `src-tauri/src/fork/clipboard/keys.rs`                                                    | [anomalyco/hex](https://github.com/anomalyco/hex) + Handy                                                                                                                                                                                                                                                                                                          | `3607595d92345af4be9f0eaa86ae3750c8851e39`                                                                         | hex `src/keyboard.rs`, `crates/capture/src/capture/macos.rs` (`wait_for_modifiers_released`); Handy `src-tauri/src/input.rs`         | MIT (© 2026 Christopher Langton; © 2025 CJ Pais)                     | RS-05, shipped by S-04       |
| `src-tauri/src/fork/llm/process.rs`, `.../llm/supervisor.rs`, `.../llm/chat.rs`           | [TypeWhisper/typewhisper-mac](https://github.com/TypeWhisper/typewhisper-mac)                                                                                                                                                                                                                                                                                      | `4620bb59a804a2aeeaf5607cf10aa437c7589504`                                                                         | `TypeWhisperPluginSDK/Plugins/LlamaCppPlugin/LlamaCppServer.swift`, `LlamaCppRuntime.swift`, `Tests/LlamaCppPluginTests.swift`       | GPL-3.0-or-later (© 2026 TypeWhisper)                                 | S-10                         |
| `src-tauri/src/fork/llm/prompt.rs` (system prompt text)                                   | [OpenWhispr/openwhispr](https://github.com/OpenWhispr/openwhispr)                                                                                                                                                                                                                                                                                                  | `b9e9ebefac4b961456c269164a93ddfdd53438b9`                                                                         | `src/locales/en/prompts.json` (`cleanupPrompt`)                                                                                      | MIT (© 2024 OpenWhispr Team)                                          | S-10                         |
| `src-tauri/src/fork/text/rules.rs`                                                        | [ideaplexa/voicetypr](https://github.com/ideaplexa/voicetypr)                                                                                                                                                                                                                                                                                                      | `f86e571df5bd5c0a4d8950095167b45185fa59d4`                                                                         | `src-tauri/src/writing/library_rules.rs`                                                                                             | AGPL-3.0 (© the Voicetypr authors); file marked AGPL-3.0-only         | S-11                         |
| `src-tauri/src/fork/text/tables/commands.*.json` (data)                                   | [moona3k/macparakeet](https://github.com/moona3k/macparakeet)                                                                                                                                                                                                                                                                                                      | `3ec699152f78df8adccba0cbff5cb968c9790947`                                                                         | `Sources/MacParakeetCore/TextProcessing/SpokenPunctuation.swift` (phrases, literal prefixes); `SpokenPunctuationTests.swift` (cases) | GPL-3.0 (© 2026 Daniel Moon)                                          | S-11                         |
| `src-tauri/src/fork/pill/panel.rs`                                                        | [kozliq/Tolkovo](https://github.com/kozliq/Tolkovo)                                                                                                                                                                                                                                                                                                                | `ffc5b6231eacc0c5ba8ab59634ad0f2dd4b44b35`                                                                         | `src-tauri/src/tolkovo/pill.rs` (panel set-up, Space observer; via `crates/wb-spike-pill/src/pill.rs`)                               | MIT (© 2025 CJ Pais; © 2026 kozliq)                                  | S-03                         |

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

### `fork/clipboard/` (RS-05 spike, lifted by S-04)

- Written in the RS-05 spike (`crates/wb-spike-clipboard/src/clip/`) and
  moved unchanged in behaviour to `src-tauri/src/fork/clipboard/` by S-04
  (2026-10-30); the spike now compiles the product files through `#[path]`.
  The hex and Handy notices are in `NOTICE` §3. Only `carbon.rs` (hex's
  Carbon capture/restore, kept for the RS-05 comparison) stays in the
  spike crate, which is never shipped.
- S-04 additions (ours, GPL-3.0-or-later): `RestoreGuard::new`/`is_current`,
  `keys::post_return`, `nspb::write_plain_text`, `Item::text`/`with`.
- Not ported: TypeWhisper `insertTextAtAndVerifyChange` and FluidVoice
  `PasteVerifier` / `PasteDeliveryCoordinator` (R4). `fork/insert/verify.rs`
  and the delivery slot in `fork/insert/deliver.rs` follow their behaviour
  (compare the field before and after; one delivery at a time) and were
  written from the RS-05 measurements; no code was taken.
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

### `crates/wb-spike-pill/` (RS-03 spike)

- `pill.rs` follows Tolkovo's panel set-up and copies its
  `_setPreventsActivation:` call (behind `respondsToSelector:`); RS-03 found
  the call is not needed, so S-03 lifts the set-up without it.
- `tools/vdisplay.m` re-declares DeskPad's interface for Apple's private
  `CGVirtualDisplay` classes to add a test display; it is a test helper and
  is never built into the app, so neither row is in `NOTICE` yet.

### `crates/wb-handy-recorder` (S-01, DEC-53)

- Vendored unchanged first (commit "vendor handy-recorder 0.1.0
  unchanged"), with an SPDX/copyright header added to each file; the next
  commit adds `RecorderConfig::pause_when_idle`: the input stream is built
  and initialised at `open` but only runs between `start` and `stop`
  (`InputStream::set_running`, a `SetRunning` device message, a watchdog
  that does not count a stopped stream as silent, queued audio dropped at a
  prepared start), six tests over the crate's fake backend, and a
  first-audio wall-clock stamp in the debug log for the §5.1 latency check.
- Upstream-bound (MIT). The app uses it through `[patch.crates-io]` in
  `src-tauri/Cargo.toml`; CI runs the crate's own suite.

### `fork/shortcut/tests/ported.rs` (S-01)

- Only input sequences and expected outcomes are taken; no upstream code.
  66 vectors: anarlog 19 (`crates/shortcut-macos/tests/processor.rs`; the
  semantics from `src/processor.rs`, `src/decision.rs`; `enterprise/` not
  read), MacParakeet 20 (`FnKeyStateMachineTests.swift`,
  `HotkeyGestureControllerTests.swift`), FluidVoice 11
  (`HotkeyShortcutTests.swift`, the `ModifierOnlyShortcutFlagsDecision`
  cases), OpenWhispr 8 (from the `FN_INTERRUPTED` code, no upstream tests),
  hex 7 (`src/keyboard.rs` chord breaker, `src/suppression.rs` tests), Hex
  legacy 1 (`HotKeyProcessorTests.swift`).
- Each vector is re-timed to T = 500 ms / T_show = 150 ms; where upstream
  expects something else (300 ms double-tap window, keys after a threshold
  that keep recording, a 5 s undo window), the expectation follows R5 S-01
  and a `DIVERGES:` comment says how.
- `fork/shortcut/processor.rs` itself is Wispr Bro's own grammar from the
  RS-01 spike, written to the anarlog / Hex `HotKeyProcessor` behaviour
  (credited, nothing copied). `fork/shortcut/labels.rs` repeats the
  `UCKeyTranslate` technique of Handy's `input.rs` (MIT, private there).

### `fork/stats/streak.rs`, `fork/context/categories.*` (S-09)

- `streak.rs`: SpeakoFlow's `compute_streaks` kept as is (current streak
  ends today or yesterday, longest run, unsorted and duplicate days); it
  returns a `Streaks` struct, and `parse_days` reads `daily_stats.day`. The
  stats-store pattern (lifetime counters in their own table that survive
  pruning, deltas clamped at zero, `timed_words` so untimed rows cannot
  inflate words per minute) follows SpeakoFlow `usage_daily` /
  `apply_usage_delta` in behaviour; that SQL was written anew for
  `fork.db`'s schema.
- `categories.json`: OpenWhispr's personal / work / email app signatures
  and its browser list (macOS-relevant entries; the Windows exe and Linux
  WM_CLASS names were left out), plus bundle ids from AudioWhisper's
  built-in map. AudioWhisper's own categories (terminal, coding, chat,
  writing) were not used; its ids were mapped to Wispr Bro's groups.
  Documents and AI-prompt apps are our additions. OpenWhispr's site hosts
  and window-title rules are for S-19b and were not ported yet.
- `categories.rs`: OpenWhispr's `tokenPattern` (whole-token match, no
  letter or digit on either side) and category order (email, work,
  personal) re-implemented in Rust without regexes.

### `fork/llm/{process,supervisor,chat}.rs` (S-10)

- Read at the pinned commit; ported from Swift to Rust, not copied: the
  spawn line (`--parallel 1 --jinja --reasoning off --no-ui --no-slots
--offline`, `--alias`, thread count), the environment rule (no inherited
  `LLAMA_*`; we clear the whole environment), the random key, port-bind
  retries, the stderr tail with "last error line", restart only when a
  ready server exits unexpectedly, `finish_reason == "length"` and context
  overflow as errors, and the size + hash verification marker.
- Not ported: the runtime download of llama.cpp (R5: bundled sidecar, never
  downloaded), the Hugging Face browser, the model import UI, the shell
  watchdog script (replaced by a forked kqueue `NOTE_EXIT` watchdog).
- Added: Unix-socket transport (DEC-49), CPU fallback, backoff, idle stop,
  wake health check, streamed replies (abort frees the slot at the next
  token), memory policy and ordered loads.
- `LlamaCppPluginTests.swift` served as the acceptance list for
  `fork/llm/supervisor/tests.rs`, `process.rs` and `chat.rs` tests.

### `fork/llm/prompt.rs` (S-10)

- The system prompt is OpenWhispr's `cleanupPrompt` byte for byte (a test
  compares it with `eval/cleanup/prompts/openwhispr.txt`, the RS-07 input),
  plus our JSON-reply line. S-12a grows this file into the prompt builder.

### `fork/text/rules.rs` (S-11)

- Kept: the word-boundary rule (`candidate_has_boundaries`), protected URL
  and e-mail tokens (`span_in_protected_token`), candidate order (leftmost,
  then longest) with greedy non-overlapping selection, per-application
  provenance, the final restoration guard (`apply_final_restoration_guard`,
  exact offset then nearest shifted occurrence), `sanitize_transcript`, and
  the 9 original tests that cover those parts (re-expressed for our types).
- Changed: one `aho-corasick` automaton over Unicode case-folded "from"
  texts (with a byte map back to the original) instead of one regex per
  rule; the caller chooses the inserted text, so the pipeline can protect
  outputs with placeholders; rules over 60 characters, blank or identity
  rules are dropped (Wispr's dictionary limit); no boundary is required next
  to scripts without spaces; of two rules with the same "from" the older
  wins.
- Not ported: whole-utterance snippets (`match_snippet`; S-17 builds inline
  snippets), custom-word spoken forms and per-rule language scopes (Wispr
  Bro's dictionary has neither), and the 8 tests for them.
- Only this file was read, through the GitHub API at the pinned commit;
  nothing was built or run. Voicetypr's licence gating
  (`commands/license.rs`) is not part of this file.

### `fork/text/tables/commands.*.json` (S-11, data)

- From MacParakeet's `SpokenPunctuation.swift`: the "?" and "!" phrases in
  English, German, Spanish, French, Portuguese, Polish, Chinese and
  Japanese, and the literal prefixes (`literal`, `wörtlich`, `littéral`,
  `dosłownie`, with their unaccented spellings). Its tests became cases in
  `fork/text/commands.rs` (including 请问号码 staying text).
- Re-expressed, not copied: the matching (MacParakeet uses one regex per
  phrase; we use one automaton per language with clause-edge rules), the
  comma-before and terminators-after consumption.
- Our own: the line, paragraph and "scratch that" phrases, Italian, the
  blocker words, and the clause-edge rule.

### `fork/pill/` (S-03)

- `panel.rs` lifts the RS-03 spike's Tolkovo panel set-up (status level,
  non-activating, never key, `hides_on_deactivate(false)`, all Spaces +
  full-screen auxiliary, the Space-change re-show). It sets
  `_setPreventsActivation:` once at creation (DEC-28 fallback, as Tolkovo
  does); everything else in `fork/pill/` is ours.
- `native.rs` (the native resting bar) follows the approach of anarlog's Swift
  pill and Voquill's `rust_macos_pill` (a native panel at rest, a web view
  only while active, the RS-03 fallback); neither project's code was read for
  it or copied, so neither is a port.
