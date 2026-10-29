# Pass 2 review, area D: Text intelligence

**Reviewer stance.** Independent, skeptical re-check of `pass1-D-text-intelligence.md` against the code at `<repo>/` (branch `wispr-bro`, Handy 0.9.8) and against `R1-wispr-flow-teardown.md`. Read-only; nothing built or run. One read-only inspection of existing build output was made (`nm` on already-built static archives under `src-tauri/target/debug/build/transcribe-cpp-sys-*/out/lib/`) to answer the ggml symbol question.

**Tags.** `verified-in-code` = opened and read at the cited `path:line` in this pass. `inferred` = reasoned from code that was read, not executed. `unknown` = not determined.

## Summary

- **Rows checked:** 71 of 71 (D-01 to D-71), plus every `path:line` cited in pass-1 sections 1, 3, 4 and 5.
- **Evidence errors found:** 16. Most are small (wrong line, a claim that is too narrow). Five matter for planning:
  1. **The `custom` provider cannot use structured output.** Its flag is `false` and is reset to `false` on every settings read. So a `llama-server` sidecar reached through `custom` takes the legacy path: one user message with the transcript pasted into the prompt, no system message, no JSON schema (E-04).
  2. **The ggml clash is real and the worker process does not avoid it.** transcribe-cpp links ggml statically on macOS, with unprefixed `ggml_*` symbols. The worker is the same executable started again, so a llama.cpp linked into it would clash too (section "Local LLM").
  3. **"Original never lost" (D-06) is false by default.** History keeps 5 entries (`history_limit` = 5, `PreserveLimit`) and deletes older rows and WAVs (E-10).
  4. **The plain-text paste evidence cites the wrong path.** It points at the debug-only reliable-paste code. The default path writes text through `tauri-plugin-clipboard-manager`, which also has `write_html`. Clipboard restore only saves text or an image, which matters for a Cmd+C selection fallback (E-06).
  5. **The deterministic stage removes all newlines.** No pass-1 row covers this. It blocks voice "new line" commands and multi-line snippets (new row D-210).
- **Class/effort changes:** 11 (D-06, D-14, D-22, D-29, D-30, D-48, D-57, D-58, D-68, D-65 merged into D-35, plus D-70 kept HAVE with a new dependent row).
- **New rows:** 14 (D-201 to D-214).
- **Duplicates with other areas:** 8. They need one owner each: D-39 = A-14, D-61 ≈ A-28, D-62 = C-54, D-63 = C-57, D-64 = C-105, D-48 ≈ C-63 (classes differ), D-57 ≈ C-52 (classes differ), D-69 ≈ E-76 (classes differ: OUT vs TUNE).
- **Cross-area conflict to escalate:** E-49 proposes `llama-cpp-2` in-process inside the engine-supervisor worker. That is the configuration that clashes on ggml (see "Local LLM").

---

## Evidence corrections

| Row | Problem | Correct evidence (path:line) | Fix |
|---|---|---|---|
| §1 item 3 / D-01 | "Registered only while `post_process_enabled`" cites `shortcut/mod.rs:1088-1105`. That is the toggle command. The guard that skips registration at startup and resume is elsewhere. | `shortcut/mod.rs:309-311`, `:501`; `shortcut/handy_keys.rs:437`; `shortcut/tauri_impl.rs:25`; `secure_input.rs:483`. The binding ID is also hard-coded in `transcription_coordinator.rs:572, 819, 1167`, `signal_handle.rs:50`, `lib.rs:880`. verified-in-code | Cite the guards. Note that removing or renaming the second binding touches about 9 call sites. |
| §3.1 table (apple_intelligence) | Format given as "(native)". The struct says OpenAI. The Apple branch then bypasses HTTP. | `settings.rs:784-785` (`supports_structured_output: true`, `api_format: OpenAi`); branch at `actions.rs:256-298`. verified-in-code | Change the format to "OpenAi (unused; native branch)". |
| D-69, §5 item 8 | "Re-added by migration on every load." The call is actually in `get_settings`, which runs on every settings read. It also overwrites `supports_structured_output` and `api_format` of built-in providers each time. | `settings.rs:1112-1157` (call at `:1153`), sync at `:883-904`, add at `:906-910`. verified-in-code | Reword: "on every `get_settings()` call; built-in flags are force-synced". So no stored edit of a built-in provider survives. |
| D-70, §3.4 item 1 | Says a sidecar reached through the existing `custom` path "supports JSON-schema/GBNF constrained output". In code, `custom` has `supports_structured_output: false`, which is force-synced. So it uses the legacy path: the transcript goes into the prompt text as a single user message, with no system role and no `response_format`. | `settings.rs:801-808`, `:884-892`; legacy path `actions.rs:367-376`; `send_chat_completion` passes `None` system and schema at `llm_client.rs:354-362`. verified-in-code | A local runtime needs its own built-in provider entry with structured output on (new row D-212). Treat the `custom` path as non-structured. |
| §3.4 item 6 | "Transcript as the user turn (already the structured path, `actions.rs:253-254`)": true only for structured providers. On any structured-call `Err`, the code sends a second, legacy request. Each request can also be retried once without reasoning fields. So one dictation can make up to 4 HTTP calls. | `actions.rs:357-363` (fall-through), `:367-401`; retry `llm_client.rs:453-487`. verified-in-code | Note the hidden latency multiplier. A latency budget must wrap the whole chain, not one request. |
| D-54, §5 item 12 | "Paste publishes plain text only" cites `paste_tx/macos.rs:22`, which is the debug-gated reliable path (`reliable_paste` defaults to false). The default path writes text through the Tauri clipboard plugin, and that plugin already has `write_html`. | Default write `clipboard.rs:50-61`; `reliable_paste` default `settings.rs:514-518`, `:1046`; plugin `tauri-plugin-clipboard-manager` 2.3.2 (`Cargo.toml:42`), `write_html` at `~/.cargo/registry/src/.../tauri-plugin-clipboard-manager-2.3.2/src/desktop.rs:72`. verified-in-code | Cite `clipboard.rs:57-60`. Rich paste is closer than implied for the HTML part. RTF and multi-item still need NSPasteboard code. |
| D-29, D-30 | Clipboard restore is presented as reusable. It saves only text, or an image when there is no text. RTF, HTML, file URLs and multiple items are lost. A Cmd+C fallback for reading the selection would therefore destroy rich clipboard content. | `clipboard.rs:81-90`, `:123-137`. verified-in-code (A-47 says the same) | Add a dependency on a multi-type pasteboard snapshot (A-47) before any Cmd+C fallback. |
| D-29 | "Has the pieces" overstates it: no Accessibility *client* API is in the tree. `macos-accessibility-client` is pulled in only by `tauri-plugin-macos-permissions` 2.3.0 for the trust check. `secure_input.rs` concerns hotkeys under Secure Event Input, not detecting secure fields. | `Cargo.lock` (no `accessibility-sys`, no `objc2-application-services`, no `get-selected-text`); `Cargo.lock` entry `tauri-plugin-macos-permissions` depends on `macos-accessibility-client`; `secure_input.rs:1-19`. verified-in-code | AXUIElement access needs a new crate or raw FFI. Effort raised (see class table). |
| D-21, §6 Q10 | "Feature flags of `objc2-app-kit` not checked". Now checked: `objc2-app-kit = "0.3"` with default features resolves to 0.3.2. Its default feature set includes `NSWorkspace`, `NSRunningApplication`, `NSImage` and `NSAccessibility`. `frontmostApplication` is gated on `NSRunningApplication`. | `src-tauri/Cargo.toml:166`; `Cargo.lock:3825-3826`; registry `objc2-app-kit-0.3.2/Cargo.toml:2017+` (default list), `src/generated/NSWorkspace.rs:296-298`, `src/generated/NSRunningApplication.rs:141` (`bundleIdentifier`). verified-in-code | Mark verified. No new crate is needed for frontmost app and icon. AppKit's `NSAccessibility` is the provider-side API, not the AXUIElement client. |
| D-06 | HAVE "Original never lost". With defaults, only 5 history entries are kept, and cleanup deletes the row and the WAV. | `settings.rs:676-682` (`history_limit` = 5, `PreserveLimit`); deletion per C-46 `history.rs:455-483` (cited by area C; not re-read here). verified-in-code for defaults | Change class to TUNE (retention default); cross-reference C-46 and E-69. |
| D-11 | Says the Apple call blocks and so cannot be cancelled (`inferred`). Stronger than that: `complete_unless_cancelled` uses `tokio::time::timeout` around the future, and a future that blocks synchronously inside `poll` cannot be pre-empted. So a "latency budget" wrapper built the same way would not work for Apple either. | `actions.rs:154-172` (timeout loop), `:268-272` (synchronous FFI inside async fn); `apple_intelligence.swift:80-118` (semaphore); `apple_intelligence.rs:41-43`. inferred (from code structure) | Fix: `spawn_blocking`. A real Swift-side cancel needs `Task` cancellation, which is not wired. |
| D-13, D-26 | "Pass language evidence explicitly into the prompt": the language is available at the call site but not passed. `process_transcription_output` takes only text. | `actions.rs:747-751` (`language` in hand), `:410-414` (signature); history stores it `history.rs:40, 80-83`. verified-in-code | Small plumbing change. Also note Handy has text-based language ID (`audio_toolkit/lang_id.rs:57`) usable for the English-only style gate. |
| D-47 | Omits that the correction threshold has a UI, in Debug. | `src/components/settings/debug/WordCorrectionThreshold.tsx:18-25`. verified-in-code | Add. |
| D-68 | The copy is inconsistent: the model-list error says "macOS 15 or later", while Swift requires 26.0. | `shortcut/mod.rs:1324`; `apple_intelligence.swift:5, 40, 64`. verified-in-code | Fix the copy when rebranding. |
| D-43 | `TryIt.tsx:488` is an unrelated comment line (file has 737 lines). The claim is "existence only", which holds. | `src/components/onboarding/screens/TryIt.tsx` (file exists). verified-in-code | Drop the line number. |
| D-10 | Lists the overlay states as recording/transcribing/processing. There are also `streaming` and "loading model" states. | `overlay.rs:619-631`; `translation.json:959-961`. verified-in-code | Minor wording. |

All other cited lines were checked and support their claims, including D-02, D-03, D-05, D-07, D-08, D-09, D-12, D-15, D-17, D-32, D-34, D-36, D-61, D-67, D-71 and §3.3 (`build.rs:371-445`, `apple_intelligence.swift:25-36, 44-50, 92-107`, `actions.rs:267`). verified-in-code.

---

## Class/effort changes

| Row | Pass-1 | New | Reason with evidence |
|---|---|---|---|
| D-06 Original never lost | HAVE | TUNE S | Both texts are stored, but the default retention of 5 entries deletes them (`settings.rs:676-682`). Also the "raw" text is post-filler and post-dictionary (`transcription.rs:1643-1660`). Keeping the verbatim ASR text needs one more column (inferred). verified-in-code |
| D-14 Number words / spoken punctuation (ITN) | EXTEND M | EXTEND M (English only) / L (multilingual) | No ITN code exists. Rule sets are per language. Handy outputs many languages (`text.rs:315-324` language-gated lists show the per-language pattern). inferred |
| D-22 Web app / site detection | BUILD M | BUILD L | No AX client crate (Cargo.lock). Per-browser behaviour differs: AppleScript needs an Automation prompt per browser plus the `NSAppleEventsUsageDescription` and apple-events entitlement. Firefox has no URL scripting. inferred |
| D-29 Read selected text in any app | BUILD M | BUILD L | Needs a new AX layer (none in Cargo.lock). Electron and Chromium apps often expose `AXSelectedText` only after `AXManualAccessibility`/`AXEnhancedUserInterface` is set (inferred). The Cmd+C fallback needs a changeCount wait (pattern at `paste_tx/macos.rs:158`) plus a full multi-type clipboard snapshot, because the current restore is text-or-image only (`clipboard.rs:81-90`). verified-in-code / inferred |
| D-30 Replace selection in place | EXTEND S | EXTEND M | Paste-over-selection works only if the selection survives LLM latency. It needs a focus/app guard (D-21), a re-select or abort policy, and an exception for Linux, whose default paste method is Direct typing (`settings.rs:247-254`), where many undo steps result. inferred |
| D-48 Misspelling → correction pairs | EXTEND S | EXTEND M | `custom_words` is `Vec<String>` (`settings.rs:463`), so the type changes and needs a settings migration, a dialog, and case/word-boundary/CJK rules. Area C rates the same feature BUILD M (C-63). Pick one owner. verified-in-code |
| D-57 Fixes-made counters | EXTEND S | EXTEND M | The deterministic passes return only `String` (`text.rs:151`, `:429`). Counting LLM edits needs a word diff, and the counts must be stored per entry (schema `history.rs:22-40`). Matches C-52 M. verified-in-code |
| D-58 Context awareness | BUILD M | BUILD L | Same missing AX layer as D-29. Vocabulary only reaches ASR models that support it (`transcription.rs:1556-1562`). Electron apps expose little. Privacy UX is also needed. inferred |
| D-68 Apple Intelligence as engine | TUNE S | EXTEND S | The needed changes are logic, not UX: `spawn_blocking` (`actions.rs:268`), an availability reason instead of 0/1 (`apple_intelligence.swift:45-50`), `GenerationOptions` for temperature (none set, `:92-107`), session prewarm, and auto-select. verified-in-code |
| D-65 Writing examples (beyond Polish) | BUILD M | merged into D-35 | Same R1 source (§10 Polish modal) and same mechanism. |
| D-70 Custom OpenAI-compatible endpoint | HAVE | HAVE (escape hatch only) | Kept, but it is not the integration path for a bundled runtime because it is non-structured (E-04). See new row D-212. verified-in-code |

No change to the rest. D-01 stays EXTEND S for the wiring alone, but it is blocked by D-67 and D-212. With defaults, the provider is `openai` with an empty model and no prompt, so cleanup is skipped (`actions.rs:198-212`; `settings.rs:710-712, 1035`). verified-in-code

---

## Local LLM: verified facts

### Providers and defaults
- Built-ins, in order: openai, zai, openrouter, anthropic, groq, cerebras, then `apple_intelligence` (only when Handy is compiled for macOS aarch64), then bedrock_mantle, then custom (`settings.rs:714-812`). The structured-output flags are as pass 1 says. The Apple entry's `api_format` is `OpenAi` (`:785`). verified-in-code
- Defaults: `post_process_enabled` false (`:696-698`), provider `openai` (`:710-712`), API keys empty for every provider (`:814-820`), models empty except Apple = `"Apple Intelligence"` (`:12`, `:822-838`), one prompt "Improve Transcriptions" (`:840-846`), selected prompt `None` (`:1035`), `experimental_enabled` false (`:1040`). verified-in-code
- How cloud providers come back: `ensure_post_process_defaults` (`:873-938`) runs inside `get_settings` (`:1153`). On each read it adds any missing built-in and force-syncs `supports_structured_output` and `api_format` (custom keeps its format). Removing cloud providers therefore has to happen in `default_post_process_providers` or in a UI filter, never in stored settings. verified-in-code
- Turning Experimental off does not turn post-processing off. `change_experimental_enabled_setting` writes only its own flag (`shortcut/mod.rs:1112-1117`), and the toggle is merely hidden (`AdvancedSettings.tsx:68-70`). So hidden-but-active post-processing is possible. inferred
- Other triggers for the cleanup path besides the hotkey: the `--toggle-post-process` CLI flag (`lib.rs:880`) and SIGUSR1 on macOS (`signal_handle.rs:50`). verified-in-code

### HTTP client (OpenAI and Anthropic formats)
- A new `reqwest::Client` is built per request, with no timeout and no connect timeout (`llm_client.rs:219-225`); reqwest 0.12 (`Cargo.toml:65`). verified-in-code
- Headers advertise upstream Handy: `Referer` and `User-Agent` point at github.com/cjpais/Handy, plus `X-Title: Handy` (`llm_client.rs:186-195`). verified-in-code
- The OpenAI request body has only `model`, `messages`, `stream: false`, an optional `response_format` (`json_schema`, `strict: true`, name `transcription_output`) and the reasoning fields. There is no `temperature`, `max_tokens` or `seed` (`llm_client.rs:112-121`, `:413-435`). verified-in-code
- Anthropic format: `max_tokens` 4096; a response truncated at `max_tokens` is an error, which pastes the raw text. No system message: the whole prompt goes as the user turn (`llm_client.rs:139-148`, `:524-587`). verified-in-code
- Reasoning-disable fields are sent only for `custom` and `openrouter` (`actions.rs:248`). On a 400/422 the request is retried once without them, and the rejection is remembered per (base_url, model) (`llm_client.rs:87-110`, `:451-487`). verified-in-code
- Structured parse: if the JSON fails to parse or lacks the `transcription` field, the raw model content is pasted (`actions.rs:324-351`). verified-in-code

### Apple Intelligence
- **Bridge:** a Swift static library built by `build.rs` with `swiftc -parse-as-library -target arm64-apple-macosx11.0`, then `libtool -static`, linked as `static=apple_intelligence` with `-weak_framework FoundationModels` (`build.rs:476-539`). verified-in-code
- **Real or stub:** the real file is used only if the SDK has `FoundationModels.framework`, the toolchain is not Command-Line-Tools-only (checked via `xcode-select -p`), and `HANDY_FORCE_AI_STUB` is not `1`. Otherwise the stub, whose availability always returns 0 (`build.rs:403-446`, `:554-566`; `swift/apple_intelligence_stub.swift:9-12`). verified-in-code
- **Build-script quirk:** `build.rs` gates the bridge on `#[cfg(all(target_os="macos", target_arch="aarch64"))]` (`build.rs:2-3`). A build script compiles for the host, so an x86_64 build on an arm64 runner would still compile the arm64 bridge. Rust code never references it on x86_64 (`actions.rs:1-2`). Likely only a linker warning. inferred
- **Release:** the arm64 build runs on `macos-26` runners "for Apple Intelligence SDK"; Intel builds on `macos-latest` (`.github/workflows/release.yml:49-54`). verified-in-code
- **Runtime:** macOS 26.0+ (`apple_intelligence.swift:5, 40, 64`), Apple silicon builds only, and `SystemLanguageModel.default.availability == .available`, collapsed to 0/1 (`:44-50`, `:71-77`). Availability is checked on every dictation (`actions.rs:260`), not at startup, to avoid a SIGABRT on betas (`settings.rs:772-775`). App minimum is macOS 10.15 (`tauri.conf.json:42`). verified-in-code
- **Call:**
  - A new `LanguageModelSession(model:, instructions:)` per dictation. It calls `respond(to:generating: CleanedTranscript)`, a `@Generable` struct with one field `cleanedText`, and falls back to plain `respond(to:)` on any error (`apple_intelligence.swift:5-9`, `:89-107`). verified-in-code
  - No `GenerationOptions`, no `prewarm()`, no `@Guide`. verified-in-code
  - Model-side context limits and language coverage are not visible in code. unknown
- **Threading:** `Task.detached` plus `DispatchSemaphore.wait()` on the calling thread, which is a Tokio worker inside an async fn (`apple_intelligence.swift:80-118`; `actions.rs:268`). verified-in-code
  - It is not cancellable and not covered by any timeout wrapper. inferred

### transcribe-cpp vendors ggml: clash question
- **Version:** `transcribe-cpp` 0.4.0 depends on `transcribe-cpp-sys` 0.4.0 (`Cargo.lock:7320-7339`). The -sys crate ships a full vendored ggml (`ggml/` in the crate root; `Cargo.toml` include list), `GGML_VERSION` 0.26.0 (`transcribe-cpp-sys-0.4.0/ggml/CMakeLists.txt:6-8`). The upstream commit file `ggml/UPSTREAM` is not shipped in the crate, so the exact ggml commit is unknown. verified-in-code
- **Link mode:** macOS links it statically, with the `metal` feature on arm64 and CPU-only on x86_64 (`src-tauri/Cargo.toml:154-158`; `build.rs:16-17` comment). The link manifest of the existing debug build lists `transcribe, ggml, ggml-cpu, ggml-metal, ggml-base`, `"shared": false` (`target/debug/build/transcribe-cpp-sys-*/out/lib/transcribe-link.json`). verified-in-code
- **Symbols:**
  - Without `GGML_SHARED`, `GGML_API` is a plain `extern`, so symbols carry no prefix (`ggml/include/ggml.h:176-188`). CMake sets hidden visibility (`CMakeLists.txt:65-68`). verified-in-code
  - In a static archive, hidden symbols are still global to the static linker: `nm` shows 553 `T _ggml_*` symbols in `libggml-base.a`, and `_ggml_init` is marked "private external". verified-in-code
  - Consequence: linking a second static ggml, such as llama.cpp through `llama-cpp-sys-2`, into the same executable gives duplicate-symbol link errors, or silently resolves `ggml_*` to one copy, depending on which archive members get pulled. With different ggml versions that is undefined behaviour. inferred
- **The worker does not isolate this.** The engine "worker" is the same Handy executable started again with `--transcribe-worker` (`engine_supervisor/mod.rs:3-5`, `:36-48`). An in-process or worker-mode llama.cpp would be linked into the same binary. verified-in-code / inferred
- **Options, in order of risk:**
  1. A separate `llama-server` (or own small LLM) binary as a Tauri sidecar. No shared link, so no clash.
  2. Build llama.cpp against transcribe's own ggml archives (llama.cpp supports an external ggml). This needs an exactly compatible ggml API version, which is brittle. inferred
  3. Ask the transcribe.cpp maintainers to expose text generation. The crate already has a Llama/Qwen3-style causal decoder (`src/causal_lm/causal_lm.h:1-13`), but `include/transcribe.h` has no text-generation entry point (grep). inferred
- Pass-1 §3.4.1 is confirmed. E-49 (area E) should not recommend `llama-cpp-2` in the engine worker without solving this.

### Pass-1 "Local LLM: current state and gap": confirm or correct
- **Confirmed:** no LLM runtime in the dependency tree; Apple path facts; no timeout; no temperature; Handy headers; ASR preload at key-down (`actions.rs:453-461`); discovery ignores llama GGUFs (`model.rs:3357-3384`).
- **Corrected:**
  - The `custom` path is non-structured, so a sidecar needs a new provider entry (E-04).
  - The structured-failure fallback doubles the requests (E-05).
  - The "worker-process pattern" does not avoid the ggml clash.
  - `objc2-app-kit` features suffice for the frontmost app.
  - Clipboard restore loses rich types.

---

## New rows

| ID (D-2xx) | Feature | R1 ref | Wispr behavior | Handy today (evidence) | Class | Effort | Notes / risks | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| D-201 | Transform record ("last transform" state: source text, result, app, time) powering "Done. See changes", ⌥O, retry, thumbs, undo | §3 (Opt O), §10 Try-it step 2, How-it-works video | ⌥O shows the diff of the last Transform; the pill offers "See changes". `observed` | History stores only dictations, keyed by WAV file (`history.rs:22-40`, `actions.rs:785-796`). No place for a transform (no audio, source = selection). verified-in-code | BUILD | M | Decide: separate table or `kind` column. Keep it local and bounded; selection text is sensitive. | rusqlite migrations already used (`history.rs:22-36`) |
| D-202 | Transform hotkey with nothing selected | §10 ("Highlight text, then press ⌥1") | `unknown` | Nothing. | BUILD | S | Options: error pill "Select text first" (pattern of the Wispr fallback card, A-15), or apply to the last dictation. Detecting "no selection" reliably is itself the D-29 problem. inferred | A-15 card |
| D-203 | Cancel (Esc) and focus guard during a Transform | §3 Cancel "Dismiss dictation and notifications" | Esc dismisses. Transform-specific behaviour `unknown`. | Cancel generation and polling exist for dictation (`actions.rs:154-172`, `:765-775`); the Apple path is not cancellable (`actions.rs:268`). verified-in-code | EXTEND | S | Abort the paste if the frontmost app changed (needs D-21). | `complete_unless_cancelled` |
| D-204 | Long selections vs model context | §10 (Transform works anywhere) | `unknown` | Apple word-limit truncation of the output only (`apple_intelligence.swift:25-36`); no input-size check anywhere in `post_process_transcription` (`actions.rs:178-402`). verified-in-code | BUILD | S | Chunk or refuse with a pill message. Small models (and Apple's on-device model, limit `unknown` here) have short contexts. | n/a |
| D-205 | Polish detail modal: description, "⌥1 to use", example output with "See Updates" | §10 Polish detail modal | Left column of the modal. `observed` | Prompt editor only (`PostProcessingSettings.tsx:184-453`). verified-in-code | BUILD | S | Static, translatable content. | i18n (`translation.json`) |
| D-206 | Transforms page banner "Transform works anywhere you write" + "Opt o to view changes" hint + practice cards (Original / Polish / Prompt Engineer, benefits ticker) | §4 Transforms, §10 Try-it step 1 | Banner with Try it out / How it works. `observed` | Nothing. | BUILD | S | Owner probably area B (B-38/B-40); listed so it is not lost. | onboarding TryIt components |
| D-207 | Edit app-group membership: remove/move an app, one group per app, per-app override | §4 Style ("+" to add; behavior `unknown`) | Add is visible; remove/move `unknown`. | Nothing. | BUILD | S | Needs a rule for conflicts (an app in two groups). inferred | NSWorkspace icons (`NSImage` feature present, see D-21) |
| D-208 | Cleanup on from the first dictation (onboarding promises "formats for you and fixes mistakes") | §13 step 8, §8.1 | Practice dictation already comes out formatted. `observed` (copy) | Off by default (`settings.rs:696-698`); no prompt selected (`:1035`). verified-in-code | BUILD | S | The LLM may still be downloading during onboarding: run deterministic-only until ready, then switch without asking. Depends on D-67. | B-50 background download |
| D-209 | Scratchpad "AI sparkle" action | §10 Scratchpad window (left rail) | AI action in the note window. Behaviour `unknown`. | Nothing (no scratchpad, C-74/C-75). | BUILD | S | After Transforms; reuse the transform pipeline on the note text (no AX needed, in-app). inferred | D-212 provider |
| D-210 | Keep newlines / paragraphs through the deterministic stage | §13 step 8 (formatting), §11 snippets (multi-line bodies) | Formatted multi-line output. `inferred` | `collapse_stutters` splits on whitespace and joins with one space (`text.rs:330-364`); `\s{2,}` → `" "` (`text.rs:326`, `:418-420`); `apply_custom_words` joins with `" "` (`text.rs:219`). Every newline is lost before the LLM. verified-in-code | EXTEND | S | Blocks voice "new line" (D-56) and any snippet expansion applied before normalization. Apply snippets after the LLM or protect them with placeholders. Direct typing (Linux default `settings.rs:250-251`) turns "\n" into Return, which sends messages in chat apps. inferred | n/a |
| D-211 | Warm the LLM at key-down / prewarm Apple session | §8.1 (no friction) | Not measured. | ASR preloads at key-down (`actions.rs:453-461`); no LLM warm-up; Apple builds a new session per call without `prewarm()` (`apple_intelligence.swift:92-95`). verified-in-code | BUILD | S | Hides model load latency inside recording time. | `LanguageModelSession.prewarm()`; llama-server `/completion` with `n_predict:0` or keep-alive (inferred) |
| D-212 | Dedicated local provider entry for the bundled runtime | none (local equivalent of §13 "silent model choice") | n/a | `custom` is non-structured and force-synced (`settings.rs:801-808`, `:884-892`); no temperature/max_tokens (`llm_client.rs:112-121`); no timeout (`:219-225`); Handy headers (`:186-195`). verified-in-code | EXTEND | S | Add a built-in `local` provider (127.0.0.1, random port, `supports_structured_output: true`, temperature 0, max_tokens scaled to the input, short timeout, neutral headers). Because `ensure_post_process_defaults` adds built-ins on every read (`settings.rs:1153`), it appears for existing users automatically. | existing `send_chat_completion_with_schema` |
| D-213 | LLM memory and unload policy coordinated with ASR | §8.1 | n/a | ASR unload timeout only (`settings.rs:465`; `ModelUnloadTimeout`); `maybe_unload_immediately` for ASR (`actions.rs:96`). No LLM lifecycle. verified-in-code | BUILD | M | 8 GB Macs: ASR model plus LLM resident may swap. Sidecar lifetime = idle timeout. inferred | engine supervisor lifecycle ideas (`engine_supervisor/mod.rs:8-15`) |
| D-214 | Handy extras worth keeping: Translate to English (ASR task) and scriptable cleanup trigger (CLI `--toggle-post-process`, SIGUSR1) | none | No equivalent observed. | `translate_to_english` (`settings.rs:448-449`), task planning (`transcription.rs:1587-1591`); CLI `lib.rs:880`; signal `signal_handle.rs:50`. verified-in-code | HAVE | - | Translate only works on models that support it. The CLI trigger is a cheap hook for "dictate with style X" from Raycast/Shortcuts. | n/a |

**Rows considered and not added**, because pass 1 already covers them: Polish rule toggles (D-33), Autosave/Reset (D-36), writing-example upload (D-35), "N Changes" card actions incl. thumbs/retry (D-40, D-41), ⌥O (D-40, now backed by D-201), ⌘Z (D-31), auto-apply transform selection (D-42), "Done. See changes" pill (D-39 = A-14), English-only note (D-26), add apps to a group (D-20).

One more note on D-41: "Retry" of a transform at temperature 0 would return the same text, so retry needs non-zero temperature or a varied prompt. Apple sampling is left at default today (`apple_intelligence.swift:92-107`). inferred

---

## Open questions resolved

1. **Q10, `objc2-app-kit` exposes `NSWorkspace`?** Yes. 0.3.2 with default features includes `NSWorkspace` and `NSRunningApplication`; `frontmostApplication` and `bundleIdentifier` are generated (registry paths above). verified-in-code
2. **Q11, llama.cpp next to transcribe-cpp's ggml?**
   - Static, unprefixed `ggml_*` globals: verified-in-code.
   - Duplicate or mixed symbols if a second static ggml is linked into the same executable: inferred.
   - The worker shares the executable (`engine_supervisor/mod.rs:3-5`): verified-in-code.
   - Recommendation: a separate sidecar binary. inferred
3. **Q8, minimum macOS in code:** 10.15 (`tauri.conf.json:42`). Apple Intelligence is runtime-gated at 26.0 and weak-linked (`build.rs:535-539`). The product decision remains open, but the code supports "Apple Intelligence as an optimization". verified-in-code
4. **Pass-1 §3.3 "release builds must use full Xcode":** upstream CI builds arm64 on `macos-26` runners for this reason (`.github/workflows/release.yml:49`). Whether those runners default to full Xcode is not visible in the workflow lines read. inferred/unknown
5. **Does a missing API key block the `custom` chat request?** No. Headers simply omit auth when the key is empty (`llm_client.rs:198-213`), and `post_process_transcription` does not check the key (`actions.rs:239-243`). verified-in-code (pass 1 had `inferred`)

## Still open

1. Wispr behaviour not in R1:
   - web-app detection (Q1);
   - order and number of model calls for style plus cleanup (Q2);
   - Command Mode without a selection (Q3);
   - snippet trigger matching (Q4);
   - what "variable recognition" installs (Q5);
   - whether ⌘Z right after a dictation also reverts cleanup (Q6);
   - Transform with no selection (D-202).
2. Product decisions:
   - Medium cleanup vs Auto-apply Polish (Q7);
   - one owner for each duplicate pair: A-14/D-39, A-28/D-61, C-63/D-48, C-52/D-57, E-76/D-69 (OUT vs TUNE: remove from defaults or hide in UI).
3. Small-model choice and measured latency on an 8 GB M1 (Q9). Not measurable read-only.
4. Exact ggml commit vendored by transcribe-cpp-sys 0.4.0 (`ggml/UPSTREAM` not shipped), so whether llama.cpp could share it is unknown.
5. Apple on-device model limits (context size, supported languages, refusal behaviour): unknown from code.
6. Whether Electron/Chromium targets the product cares about (Slack, Discord, VS Code, Chrome) return `AXSelectedText` without enabling enhanced accessibility. Needs a hands-on test.
