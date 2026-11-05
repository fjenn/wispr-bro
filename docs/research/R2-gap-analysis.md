# R2: Gap analysis, Wispr Flow vs Handy (merged)

## 1. Purpose and how to read

This is the final gap analysis between Wispr Flow as observed in R1 (`docs/research/R1-wispr-flow-teardown.md`) and the Handy clone on branch `wispr-bro` of this repo. It merges five area passes (pass 1) with five reviews and a coverage audit (pass 2): every pass-2 correction, class change and new row is applied, each R1 feature has one owning row, conflicts were settled by reading the code, and 59 rows were spot-checked against the code for this merge (§10). It does **not** decide scope; R5 does. A row's class says how far Handy is from the R1 behavior, not whether we should build it. The working files are in `docs/research/r2-work/` (`pass1-{A..E}-*.md`, `pass2-{A..E}-review.md`, `pass2-coverage-audit.md`); go there for longer reasoning behind a row.

**Classes.** HAVE = Handy already does it (maybe with a caveat in Notes). TUNE = exists; needs copy, visual, default or placement changes, no new logic. EXTEND = exists in part; needs new logic on existing code. BUILD = nothing usable exists. OUT = cloud, account, team or sales feature with no local equivalent to build (a local substitute, if any, is named in Notes). XREF = this R1 item is owned by another row; the row only points there and is not counted. `–` = not classifiable (an unknown or a note).

**Effort** (one developer): S ≤ 2 days, M ≤ ~2 weeks, L > 2 weeks. "S-M" means the reviewers could not narrow it.

**Tags.** Wispr column uses R1's tags: (obs) observed, (inf) inferred, (user) user-reported, (unk) unknown. "Seen on in a used account" marks R1 §5/§11 states seen in one existing account, not fresh-install defaults. Handy column: V = verified in code (the cited lines were read by a pass), I = inferred from code read, U = unknown. "opened" next to a citation means it was re-read during this merge.

**Paths.** Rust paths are relative to `src-tauri/src/` (e.g. `actions.rs:464`, `managers/history.rs:455`). Frontend files are cited by file name when the name is unique under `src/` (find them with `rg --files src | rg Name`). `src-tauri/...` paths are config. `~cargo/` = `~/.cargo/registry/src/index.crates.io-*/`; crate names like `handy-keys/...` or `nspanel/...` refer to the crate sources used by `Cargo.lock`.

**Cross-references.** AF-xx = architecture finding (§3), DEC-xx = decision (§4), RE-xx = rough edge (§7), RC-xx = runtime check (§8), M-xx = merge-log entry (§10).

## 2. Executive summary

### Final counts (after merge)

| Area | Rows | HAVE | TUNE | EXTEND | BUILD | OUT | XREF (not counted) | Unclassified |
|---|---|---|---|---|---|---|---|---|
| A Capture, activation, insertion | 82 | 28 | 8 | 22 | 19 | 0 | 5 | 0 |
| B Onboarding, models, languages | 82 | 21 | 18 | 17 | 11 | 4 | 11 | 0 |
| C Hub, data, settings | 127 | 10 | 19 | 22 | 37 | 12 | 27 | 0 |
| D Text intelligence | 85 | 4 | 7 | 22 | 37 | 1 | 14 | 0 |
| E1 Notetaker | 62 | 0 | 0 | 16 | 37 | 5 | 4 | 0 |
| E2 Platform and distribution | 54 | 13 | 18 | 4 | 9 | 1 | 7 | 2 |
| **Total** | **492** | **76** | **70** | **103** | **150** | **23** | **68** | **2** |

422 rows carry a class. Pass 1 had 409 rows; pass 2 added 78 (A 13, B 14, C 18, D 14, E 19) and this merge added 5 for the coverage audit's missing and partial items (C-219, C-220, C-221, E-220, E-221). 68 rows became cross-references to a single owner.

### The 15 most important gaps

1. **D-67 / D-212**: no local LLM at all. Every cleanup level, style, Transform, Command Mode and Notetaker summary depends on it; it must run as a separate sidecar (AF-01) behind a new structured-output provider (AF-02). BUILD L.
2. **B-50**: first run blocks on the speech-model download, and the backend refuses to open the mic without a model (AF-03). EXTEND L.
3. **A-01**: no resting Flow Bar; the pill only exists while recording. Panel click-through, display policy and idle webview cost. BUILD L.
4. **A-22 / A-35**: the default hotkey is ⌥Space, not fn. fn works in handy-keys, but needs an accidental-start guard, a Globe-key answer and a fallback (AF-08). EXTEND M.
5. **A-51 / A-15**: no "no text field" detection and no fallback card: Handy pastes blindly and the text silently lands only in History. Needs an Accessibility client layer that does not exist yet (AF-05). BUILD M.
6. **C-46**: history keeps 5 dictations by default and deletes text and audio together; stats, search, undo and "original never lost" all depend on fixing this (AF-07). EXTEND M.
7. **D-01 / D-05**: cleanup runs only on a hidden second hotkey; no None/Light/Medium control. EXTEND S + BUILD S, blocked by item 1.
8. **D-48 / D-210**: no "from → to" dictionary rules, and the text pipeline strips every newline (AF-06). EXTEND M + EXTEND S.
9. **D-53 / C-69 / C-70**: no snippets (page, trigger expansion, editor). BUILD M.
10. **C-02 / C-34 / C-39**: no Dictation home; History shows and copies the pre-cleanup text while the tray copies the cleaned text. EXTEND M + TUNE S.
11. **A-12 / A-13 / A-208**: errors, empty takes and a dead hotkey tap go to a hidden window or only to the log; the pill shows nothing. BUILD S + EXTEND S.
12. **E-54 / E-61 / E-211 / E-212**: identity, updater key and endpoint still point at upstream Handy; must be fixed once, before the first public build (AF-11). TUNE/BUILD S.
13. **D-29 / D-30**: Transforms need reading and replacing the selected text in any app: new AX layer plus a full clipboard snapshot. BUILD L + EXTEND M.
14. **C-30 / C-50 / C-52 / C-54**: no stats, Insights, fix counts or app capture; the history schema stores no duration, word count or app (AF-07). Insights in total is L.
15. **D-18 / D-21**: no per-app Styles; frontmost-app capture is cheap (objc2-app-kit already has NSWorkspace) but web-app detection is not (D-22, L).

### Handy's biggest strengths to keep

- Process-isolated speech engine with crash/hang recovery and GPU→CPU fallback (`engine_supervisor/mod.rs:1-20`).
- Exactly-once, cancel-safe insertion with layout-aware ⌘V and a receipt-based "reliable paste" (A-46, A-47, A-49).
- Non-activating overlay panel that never steals focus (A-52), live transcript panel (A-18), arming state (A-09).
- Offline, honest model catalog with per-language grades, resumable hashed downloads and a mirror (B-48, B-52, B-53).
- A permissions engine and onboarding that resume after relaunch and explain failed first takes (B-13, B-26, B-46).
- A fail-open deterministic text stage and a strong custom-word engine (D-15, D-47, C-67).
- Raw and cleaned text and per-dictation audio stored side by side; retry as a background job (C-42, C-43, C-211).
- Complete signing, notarization and updater pipeline, 26 locales with a CI parity check, no telemetry (E-55–E-57, E-80, E-73).

## 3. Architecture findings that constrain the build

**AF-01. A local LLM must run as a separate sidecar process.** transcribe-cpp 0.4.0 statically links a vendored ggml (0.26.0) on macOS with unprefixed `ggml_*` symbols (553 `T _ggml_*` in `libggml-base.a`; `GGML_API` is plain `extern` without `GGML_SHARED`, `ggml/include/ggml.h:176-188`). Linking llama.cpp (e.g. `llama-cpp-sys-2`) into the same executable gives duplicate or silently mixed `ggml_*` symbols (I). The engine "worker" is the same Handy executable relaunched with a flag (`engine_supervisor/mod.rs:1-6`, opened), so putting llama.cpp in the worker does not isolate it. Options in risk order: a separate `llama-server` (or own small binary) as a Tauri sidecar; building llama.cpp against transcribe's ggml (needs an exactly compatible ggml version; the vendored commit is unknown); asking transcribe.cpp to expose text generation (it has a causal decoder but no public entry point). This overrides pass-1 E-49's `llama-cpp-2` in-worker suggestion (M-29). Rows: D-67, D-212, D-213, E-18, E-23.

**AF-02. The bundled runtime needs its own built-in provider with structured output.** The `custom` provider has `supports_structured_output: false` (`settings.rs:798-812`, opened), and `ensure_post_process_defaults` force-syncs that flag for every built-in on **every** `get_settings()` (`settings.rs:873-912`, call at `:1153`, opened). So a sidecar reached through `custom` takes the legacy path: one user message with the transcript pasted into the prompt, no system message, no JSON schema (`actions.rs:367-376`). The client has no timeout, no temperature or max_tokens, and sends `Referer`/`User-Agent`/`X-Title: Handy` headers (`llm_client.rs:184-226`, opened). A structured call that fails falls back to a second legacy request, each retried once without reasoning fields: up to 4 HTTP calls per dictation (`actions.rs:357-401`; `llm_client.rs:453-487`). Add a built-in `local` provider (127.0.0.1, random port, structured output, temperature 0, bounded tokens, short timeout, neutral headers); because built-ins are re-added on read, existing users get it automatically. A latency budget must wrap the whole chain. Rows: D-212, D-11, D-70.

**AF-03. The mic does not open without a usable model, so first-run use during the download is impossible.** `TranscribeAction::start` returns early with "Not starting recording: no model can transcribe it" when no model is loaded and the selected model has no path (`actions.rs:460-476`, opened); the load error becomes a toast in the main window. Onboarding's "Try it yourself" is disabled until the model is ready (`Demo.tsx:515-526`, opened), and the shortcut-test keycaps light only on recording events (`useTryEngine.ts:117-125`). The default English download is 731 MB, ja+en 936 MB, worst common case 2.46 GB (pass-2 B, computed from the catalog). Removing the UI gate is therefore not enough: either queue audio until the model arrives, or fetch a small bootstrap model first (B-202), or keep waiting. Decision DEC-01. Rows: B-50, B-21, B-201, B-202, B-203, B-210.

**AF-04. One engine worker, one model, an exclusive compute lease, and a global cancel.** At most one worker holds a model (`engine_supervisor/mod.rs:12-15`, opened). A live stream holds the model's exclusive compute lease and other runs get `Busy` (`~cargo/transcribe-cpp-0.4.0/src/model.rs:87-101`, `session.rs:274-293`). There is one global stream route (`transcription.rs:898-917`, opened). Esc / ✕ call `cancel_transcription` → `engine.cancel()`, which bumps a global cancel epoch and kills the in-flight worker (`utils.rs:116-118`; `supervisor.rs:475-485`, opened). Consequences: dictation during a meeting recording cannot simply share the engine (E-47); diarization cannot run while a stream is live (E-22); any second session type is a core edit in upstream's hotspot (`engine_supervisor/protocol.rs`). Designs: chunked meeting decode on the shared queue with per-owner cancel, or a second supervisor with its own model copy (double RAM/VRAM). DEC-06.

**AF-05. There is no Accessibility (AXUIElement) client in the tree.** `Cargo.lock` has no `accessibility-sys`, `objc2-application-services` or `get-selected-text`; `macos-accessibility-client` arrives only through `tauri-plugin-macos-permissions` for the trust check. `secure_input.rs` handles hotkeys under Secure Event Input, not secure fields. Reading the focused element, its role, selected text or nearby text needs a new crate or FFI layer, shared by A-50, A-51, D-22, D-29, D-52, D-58. The existing Accessibility permission covers it (no new prompt). Electron/Chromium apps often expose no AX tree until `AXManualAccessibility`/`AXEnhancedUserInterface` is set (I), so every consumer must fail open (RC-04). By contrast, the frontmost app needs nothing new: `objc2-app-kit 0.3.2` (`src-tauri/Cargo.toml:166`, opened) has `NSWorkspace` and `NSRunningApplication` in its default features (D-21).

**AF-06. The deterministic text stage removes every newline.** `collapse_stutters` splits on whitespace and joins with one space (`audio_toolkit/text.rs:330-365`, opened); `normalize_transcription_output` then replaces `\s{2,}` with a space and trims (`text.rs:468-480`, opened); `apply_custom_words` also joins with spaces (`text.rs:214-220`, opened). Multi-line snippets, voice "new line" and any formatting produced before this stage are flattened. Snippet expansion must run after normalization (and after the LLM) or be protected by placeholders. Direct typing (Linux default) turns `\n` into Return, which sends chat messages. Rows: D-210, D-53, D-56.

**AF-07. History retention and data model.** Defaults: `history_limit = 5`, retention `PreserveLimit` (`settings.rs:676-682`, opened); cleanup deletes the DB row and the WAV together (`managers/history.rs:455-470`, opened) and runs only on save and on setting change. The table stores `transcription_text` (already script-converted, dictionary-corrected, filler-stripped: `transcription.rs:1600-1662`, opened), `post_processed_text`, prompt, flags, language; not duration, word count, target app, paste outcome, pre-fix text, fix counts or previous text. `audio_seconds` is computed and emitted but not stored (`actions.rs:45-64`). `ADDED_COLUMNS` only adds TEXT columns (`history.rs:170-186`, opened), each call opens a fresh connection with no WAL or busy timeout (`history.rs:254-256`, opened), and a higher `user_version` makes released upstream builds refuse the DB (`history.rs:174-175`). FTS5 and the trigram tokenizer are compiled into the bundled SQLite 3.50.2. Implications: stats need a `daily_stats` table that survives retention; integer columns need typed `ADDED_COLUMNS` (C-209); fork tables (stats, dictionary, snippets, notes, notifications, meetings, FTS) are best in a separate DB file or `CREATE TABLE IF NOT EXISTS` outside `MIGRATIONS` (DEC-08). WAV is 16 kHz mono 16-bit = 1.92 MB per speech-minute.

**AF-08. Hotkey hook failure and fn/Globe behavior.** The macOS default backend is handy-keys (`settings.rs:238-245`); it accepts `fn` and modifier-only hotkeys (`shortcut/handy_keys.rs:400-424`, opened), Tauri's backend rejects fn (`tauri_impl.rs:50-60`, opened). `HandyKeysState::new` always returns Ok and a failed `HotkeyManager::new_with_blocking()` is only logged inside its thread (`handy_keys.rs:86-120`, opened), so the persisted Tauri fallback (`shortcut/mod.rs:40-56`, opened) does not run and **every hotkey silently dies** (A-208), e.g. after Accessibility is revoked or the app is re-signed. A modifier-only fn hotkey fires on the fn FlagsChanged, stays pressed through fn+arrow/F-keys, and with the default HoldOrToggle a short chord locks recording (A-22). handy-keys matches FN exactly (`handy-keys/types/modifiers.rs:173-174`, opened), so plain Esc does not match while fn is held (A-201). The tap blocks the fn press but always passes the release; whether macOS still runs the Globe action is unknown (RC-02). Activation mode is global, not per binding (`shortcut/handler.rs:37-46`). Hotkeys match US physical keycodes (A-203).

**AF-09. Overlay panel: clicks, hover, click-through.** The macOS overlay is a non-activating, non-focusable NSPanel (`overlay.rs:450-480`, opened) and does not set `accept_first_mouse` (only the non-macOS window does, `overlay.rs:416`, opened; wry's default is false). Whether the first click on ✕ reaches the WKWebView is unknown (RC-01); every new clickable state depends on it. tauri-nspanel supports tracking areas and mouse-moved handlers (`nspanel/builder.rs:215-260`), so hover can be driven natively if CSS `:hover` does not fire (RC-03). The panel is a fixed 256×50 pt window with no `ignores_mouse_events` toggling; Rust and CSS sizes are kept in sync by hand (`overlay.rs:36-50`; `RecordingOverlay.css:21-34`). Mic levels are emitted only to this window, only when it is enabled, and only during a recording (`overlay.rs:736-767`, opened). WebKit memory growth from overlay events is a known upstream issue (#1279). Rows: A-01, A-02, A-04, A-06, A-07, A-15, A-40.

**AF-10. System-audio capture limits.** handy-recorder can only open input devices. handy-cpal's process tap is created for one output device UID (stream 0) with an empty, exclusive process list, i.e. all processes on that device including our own sounds (`~cargo/handy-cpal-0.18.2/src/host/coreaudio/macos/loopback.rs:92-110`, opened). It does not follow a device switch (AirPods mid-call). Loopback needs macOS 14.6+; the app's minimum is 10.15 (`tauri.conf.json:42`, opened), so it must be gated at runtime. The permissions plugin only checks Screen Recording (`CGPreflightScreenCaptureAccess`, `tauri-plugin-macos-permissions-2.3.0/src/commands.rs:134-140`, opened); there is no `NSAudioCaptureUsageDescription` in `Info.plist` (opened). "Mute while dictating" mutes system output via `osascript` (`managers/audio.rs:100-111`, opened) and would deafen a meeting. Recording is held in RAM until stop (`sink.rs:70-79`). Rows: E-01, E-02, E-10, E-48, E-205, E-206.

**AF-11. Release identity and updater key.** `productName: "Handy"`, `identifier: "com.pais.handy"`, version `0.9.8` shared with upstream (`tauri.conf.json:1-8`, opened); updater pubkey and endpoint `github.com/cjpais/Handy/releases/latest/download/latest.json` (`tauri.conf.json:84-90`, opened); Windows `signCommand` with upstream's Azure account (`:77`). The pubkey is compiled in: losing our private key strands every install; an upstream merge that "takes theirs" makes our builds trust upstream's feed. Changing the identifier or Apple team after release resets macOS permissions (Accessibility, Microphone) for every user (I). The workflows grep `version` from the base `tauri.conf.json`, so identity can live in a fork overlay but version cannot. Rows: E-54, E-61, E-211, E-212, E-213. DEC-17.

**AF-12. Network inventory (verified).** (1) Update check to upstream's GitHub `latest.json`: the only call made without user action, at each launch after onboarding when `update_checks_enabled` (default true, `settings.rs:596-598`, opened); the toggle is only on the Debug page; errors only go to the console (`UpdateChecker.tsx:104-110`, opened). (2) `huggingface.co` (or `HF_ENDPOINT`) for model and detector downloads, user-started. (3) `blob.handy.computer` mirror on HF failure and (4) for legacy model URLs. (5) Cloud LLM providers and `localhost:11434`, only if post-processing is configured (off by default). (6) Links opened in the browser on click. (7) OS-level traffic (Gatekeeper/notarization check). Pass-1's `api.deepseek.com` entry was a string match in tests, not a call. No telemetry. The webview CSP is `null` (`tauri.conf.json:16`). Rows: E-68, E-74, E-75, D-69.

**AF-13. Upstream-merge hotspots.** `lib.rs` (`collect_commands!`, plugin chain, tray handler); generated `bindings.ts` (regenerate, never hand-merge); `settings.rs` (81 `AppSettings` fields, defaults, migrations: put fork settings in a separate store); 26 `translation.json` files (fork keys under one namespace, brand via interpolation); `actions.rs` `ACTION_MAP` and `transcription_coordinator.rs`; `managers/history.rs` `MIGRATIONS`; `tauri.conf.json`/`Info.plist`/`Entitlements.plist`/`Cargo.toml`; `Sidebar.tsx` and `App.tsx`; `release.yml`; `engine_supervisor/protocol.rs` (highest-risk for Notetaker). The second dictation binding id is hard-coded in about 9 call sites (D-01).

**AF-14. Apple Intelligence blocks an async worker and cannot be cancelled.** The Swift bridge waits on a `DispatchSemaphore`, called synchronously inside an async fn (`actions.rs:256-272`, opened; `apple_intelligence.swift:80-118`), so neither the cancel poll nor a `tokio::time::timeout` can pre-empt it. It needs `spawn_blocking`. Availability is collapsed to 0/1, no temperature or prewarm, macOS 26 + arm64 only, and the real bridge is compiled only with full Xcode and a macOS 26 SDK (else a stub). Row: D-68.

**AF-15. A custom app menu replaces Tauri's default menu.** Tauri 2.11.5 installs a default macOS menu (File/Edit/View/Window/Help) that Handy never overrides. Building Dictation/My Voice/Notetaker menus with `set_menu` drops Edit unless rebuilt, which would break ⌘C/⌘V/⌘Z in Hub text fields (I). Row: A-63.

**AF-16. New windows need capability entries.** Capabilities are scoped to the `main` and `recording_overlay` labels (`src-tauri/capabilities/default.json:5`). A Scratchpad window or Notetaker panel gets no plugin permissions until added. Rows: C-75, E-12.

**AF-17. Clipboard restore keeps only text, or an image when there is no text.** (`clipboard.rs:78-92`, opened.) RTF, HTML, file URLs and multi-item content are lost on every paste today, and a ⌘C-based selection reader would destroy them too. Both paste paths write plain text only (`clipboard.rs:48-62`, opened; `paste_tx/macos.rs:22`, opened), although the clipboard plugin can write HTML. The concealed-type "reliable paste" is Debug-gated off (`settings.rs:512-520`, opened). Rows: A-47, A-48, C-70, D-29.

**AF-18. Logs contain settings at the default level.** Default file log level is Debug (`settings.rs:656-658`, opened) and startup writes a full settings dump including custom words, prompts and device names (`settings.rs:1103-1110`, opened; API keys redacted). Release builds redact transcripts (`utils.rs:17-23`). Row: E-78.

**AF-19. Models live in the shared Hugging Face cache.** HF downloads go to the shared HF cache, outside app data unless portable mode sets `HF_HOME` (`managers/model.rs:33-38`, opened; `portable.rs:43-45`). Disk checks, "delete my data", disk-usage display and uninstall must account for it. Rows: B-54, B-205, E-64, E-67.

**AF-20. Machine signal for silent model choice.** The backend reports GPU name and VRAM; the frontend reduces it to a boolean. Intel Macs build CPU-only (`src-tauri/Cargo.toml:154-158`, opened). No RAM, free-disk or chip probe; benchmarks come from a Ryzen 4750U, not Apple Silicon. `recommend()` ignores its wait budget when nothing in budget is usable, so six languages get a 15 s/30 s-audio default (B-208). Rows: B-49, B-208, B-214. DEC-02.

## 4. Decisions needed (for R5)

1. **DEC-01 First run during the model download.** (a) Wait (today; minutes on slow links). (b) Bootstrap model (135-253 MB) first, then swap silently: fast first dictation, two downloads, weaker first impression. (c) Record and queue audio until the model arrives: one download, but the first result comes late and the backend gate (AF-03) must change. Rows B-50, B-202.
2. **DEC-02 Silent model policy.** Which signals pick the model (OS languages + English, GPU/Intel, RAM, free disk, measured speed), whether to start downloading at first launch before languages are confirmed, minimum supported Mac, Intel support. Trade-off: earlier start and fewer questions vs a possible re-download and unmeasured Apple Silicon speed. Rows B-47, B-49, B-208, B-214.
3. **DEC-03 Local LLM.** Sidecar `llama-server` + small model download (works on every Mac, 0.4-2.5 GB, memory next to ASR) vs Apple Intelligence only (zero download, macOS 26 + Apple Silicon only) vs both with Apple as an optimization. Also which cleanup level is the default. Rows D-67, D-68, D-212, D-213.
4. **DEC-04 Cloud LLM providers: remove or hide.** Remove in `default_post_process_providers` (clean privacy story, small upstream-file diff) vs filter in a fork layer plus a backend refusal (smaller diff, code stays). UI-only hiding is not enough (AF-02, M-04). Row D-69.
5. **DEC-05 Notetaker milestone.** In v1 or later. It drives E1 (62 rows), B-06, A-03 slot, AF-04 and AF-10 work, and the macOS 14.6 gate.
6. **DEC-06 Engine design for dictation during a meeting** (only if DEC-05 = yes). Shared queue with per-owner cancel (cheaper, live text stalls) vs second supervisor (double model memory). Row E-47.
7. **DEC-07 Retention defaults.** Text forever vs N days; audio N days (1.92 MB per speech-minute, ~1.7 GB/month at 30 min/day). Stats must survive either. Row C-46.
8. **DEC-08 Where fork data lives.** Separate SQLite file for fork tables (no upstream migration conflicts, two DBs to back up and delete) vs extra tables in `history.db` outside `MIGRATIONS`. Also a separate fork settings store vs new `AppSettings` fields. AF-07, AF-13.
9. **DEC-09 Default hotkey.** fn (Wispr parity; needs guard, Globe handling, fallback) vs keep ⌥Space; fallback for keyboards without fn (F5 dictation key, A-202, or ⌥Space); whether a single fn tap does anything; double-tap vs Handy's tap-to-lock. Rows A-22, A-23, A-35.
10. **DEC-10 Settings as a modal (Wispr) or pages (Handy today).** Modal matches R1 but costs more and changes deep links (C-204). Row C-12.
11. **DEC-11 Stats rules.** WPM on speech time (`audio_ms`, VAD-trimmed, looks fast) or wall time (needs record start); word counting for CJK/Thai; whether file transcriptions count. Rows C-30, C-31, C-212, B-29.
12. **DEC-12 Snippets: plain text v1 or rich text.** Rich needs an editor, sanitizing and HTML on both paste paths (AF-17). Row C-70.
13. **DEC-13 Medium cleanup vs "Auto Apply Polish".** One feature or two. Rows D-04, D-42.
14. **DEC-14 First-run defaults.** Sounds (off today; seen on in a used Wispr account), Show Flow Bar at all times, Show in Dock, log level, update checks. Rows A-42, A-66, A-65, E-78, E-75.
15. **DEC-15 Tutorials.** Animated in-app scenes (light, translatable) vs bundled videos (closer to Wispr, heavy, hard to localize); whether the mic quiz (B-43) and typing test (B-29) are worth it.
16. **DEC-16 Network policy.** Keep the update check on by default (disclosed) or ask; keep, replace or drop the `blob.handy.computer` mirror. Rows E-74, E-75.
17. **DEC-17 Release identity.** Product name, bundle identifier, Apple team, version line (own scheme from e.g. 1.0.0), all fixed before the first public build. AF-11.
18. **DEC-18 Meaning of Flag.** Keep forever (reuse Star) vs "wrong transcript, fix a word" (feeds the dictionary). Row C-40.
19. **DEC-19 Undo AI edit scope.** History/copy only, or also re-paste in the target app. Row C-42.
20. **DEC-20 Auto-learned dictionary words.** None in v1, frequency in history, or AX read-back of user edits (privacy-sensitive). Row D-52.
21. **DEC-21 Notifications.** In-app inbox only vs also macOS notifications (plugin + permission). Rows C-17, E-91.
22. **DEC-22 Minimum macOS.** Keep 10.15 with runtime gates (14.6 system audio, 26 Apple Intelligence) or raise it. Row E-92.
23. **DEC-23 Web-app detection for Styles.** AX URL reading vs per-browser AppleScript (Automation prompt) vs window title only vs none. Row D-22.
24. **DEC-24 Context awareness.** Read text near the caret via AX to help spelling (privacy UX, Electron gaps) or not. Row D-58.
25. **DEC-25 Bar display policy.** Cursor display vs focused window's display vs main display. Row A-212.

## 5. Gap tables per area

### Area A: capture, activation and insertion

Owner of: the pill (Flow Bar), hotkey grammar and editor, audio capture, insertion, tray and app menus, launch at login, Dock.

#### A1. Flow Bar / pill

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| A-01 | Resting bar always visible | §2, §11 pattern 5 | Thin bar centered at the bottom edge while idle (obs) | No idle state. Panel created hidden (`overlay.rs:478-480`), shown only at record start (`actions.rs:522-526`), hidden after paste/cancel/error (`actions.rs:846-848`, `utils.rs:121`). Fixed 256×50 pt panel, no `ignores_mouse_events` (`overlay.rs:49-50`, `:456-476`). V | BUILD | L | Needs: click-through for the transparent margin (exact-size window per state, or `ignores_mouse_events` toggling, `nspanel/builder.rs:635-636`); display policy and re-placement (A-212); an always-visible idle webview despite the WebKit memory-growth history (#1279, `overlay.rs:730-746`); Rust/CSS sizes kept in sync by hand (`overlay.rs:36-50`, `RecordingOverlay.css:21-34`). Pass 2 raised M→L. | tauri-nspanel panel options |
| A-02 | Hover on resting bar (tooltips "Dictate fn") | §2, §10 | Tooltips "Dictate fn", "Scratchpad" in three-button mode (obs); idle hover look (unk) | Absent: overlay renders `null` when not visible (`RecordingOverlay.tsx:214`). V. tauri-nspanel has tracking areas and `mouseEntered/mouseMoved` handlers (`nspanel/builder.rs:215-260`, `panel.rs:138-171`). V | BUILD | S | Risk lowered in pass 2. Unknown: whether CSS `:hover` fires in the non-key panel's WKWebView; native tracking events can drive state instead. Runtime check RC-03. | tauri-nspanel tracking areas |
| A-03 | Three-button bar (Dictate / Notetaker / Scratchpad), Notetaker teal badge | §10 | With Scratchpad "Added to Flow Bar", resting bar = three round buttons; Notetaker button has a small teal badge (user + obs) | Single overlay component, states recording/streaming/transcribing/processing (`RecordingOverlay.tsx:16`). V | BUILD | M | Needs a slot API on the bar. Notetaker slot and badge: see E-29. Scope note: Scratchpad is Beta in R1. | Same as A-01 |
| A-04 | Click the bar to start dictating | §2 | Click expands the bar and starts recording; focus moved to Wispr after a synthetic click (obs) | No start command from the frontend; only `cancel_operation` (`commands/mod.rs:15`, opened). Coordinator has a toggle entry used by CLI/signals (`transcription_coordinator.rs:666`, `signal_handle.rs:18-20`). V | BUILD | S | Wire a `toggle_transcription` command. First click may not reach the webview (see A-07). | internal wiring |
| A-05 | Active capsule layout: ✕ left, waveform, ✓ right | §2 | Black capsule; ✕ left, thin-bar waveform center, ✓ right (obs) | Status dot left, waveform center, timer + ✕ right (`RecordingOverlay.tsx:261-274`); themed surface with pink accent (`RecordingOverlay.css:7-9`, `:100-119`). V | TUNE | S | Visual only. ✓ is A-06. Held vs hands-free look from the keyboard is unknown in R1 (§2). | – |
| A-06 | Finish ✓ on the capsule | §2 | ✓ ends dictation and inserts (obs) | Absent; only ✕ (`RecordingOverlay.tsx:244-259`). V | BUILD | S | Same command as A-04. Could appear only when locked (A-210). | – |
| A-07 | Cancel ✕ on the pill | §2 | ✕ cancels (obs) | ✕ in listening and working rows calls `cancelOperation` (`RecordingOverlay.tsx:244-259`, `:278-286`) → `utils.rs:102-134`. V. The macOS panel does **not** set `accept_first_mouse` (only the non-mac window does, `overlay.rs:416`; mac `PanelBuilder` `overlay.rs:452-476`, opened); wry default is false. V | HAVE | – | First-click delivery to a never-key panel is unknown: runtime check RC-01. If it fails, set `accept_first_mouse` via the panel's `with_window`. Applies to every clickable pill state (A-04, A-06, A-15). | – |
| A-08 | Live waveform | §2 | Thin vertical bars follow audio (obs) | 9 bars from 16 FFT buckets, smoothed (`RecordingOverlay.tsx:18-20`, `:121-131`); ~30 fps throttle (`overlay.rs:747-755`). V | HAVE | – | Styling goes with A-05. | – |
| A-09 | "Arming" state before first mic sample | – (Handy only) | (unk) | Traveling pulse until first sample (`RecordingOverlay.tsx:32-35`; `actions.rs:549-588`). V | HAVE | – | Strength: honest feedback when Bluetooth mics wake slowly. | – |
| A-10 | Mic-name label at start ("Using Built-in mic (recommended)") | §2, §8.2 | Label above capsule for ~1-2 s (obs); purpose: confirms mic (inf) | Absent. Device resolved in `managers/audio.rs:579-643`; "system default" resolves to `None` (`:582-588`); default device identifiable via `is_default` (`commands/audio.rs:357-363`). V | BUILD | S | Window height must grow (A-01 sizing). "Recommended" tag from A-38. | – |
| A-11 | Processing state (owner; D-10 folded) | §2 | (unk) | Spinner + "Transcribing…/Processing/Loading model" (`RecordingOverlay.tsx:226-228`, `:351-365`); states recording/streaming/transcribing/processing (`overlay.rs:619-631`); LLM phase "Polishing" (`transcription.rs:87-89`). V | HAVE | – | Copy tune only ("Polishing…"). D-10 merged here. | – |
| A-12 | Error state on the pill | §2, §1.4 | Error state (unk); principle "failure is soft" (obs) | Errors are toasts in the main window (`App.tsx:155-207`); pill just hides (`actions.rs:838-848`, `:881-899`). No-model press: recording refused (`actions.rs:464-475`, opened), model load emits `loading_failed` → toast in the hidden window (`managers/transcription.rs:686-713`, `App.tsx:209-222`). V | BUILD | S | Pass 2 corrected "silent, log only" to "toast in a hidden window; nothing on the pill". Also route lost presses under Secure Input (A-205) and tap failure (A-208). | – |
| A-13 | Feedback for empty or silent dictation | §1.4 | (unk) | `Empty`/`Silent` outcomes computed (`actions.rs:31-43`, `:70-77`) and emitted for onboarding (`:45-68`); pill shows nothing. V | EXTEND | S | "Didn't catch that" / "Your mic seems muted". | – |
| A-14 | Pill message state with action ("Done. See changes"; owner, D-39 folded) | §10 | After a Transform the pill shows "Done. See changes" (obs) | No generic message state (`overlay.rs:619-631`). V | BUILD | S | Generic "message + optional action". Diff itself: D-40; record: D-201. | – |
| A-15 | Result / fallback card ("Select a textbox first, then dictate" + transcript + Copy + ✕) | §2, §1.4 | Black card above the bar when no field is focused (obs) | Absent. Cmd+V sent blindly (`clipboard.rs:799-898`); clipboard restored after; no `paste-error` fires because injection "succeeds". Text survives only in History. V | BUILD | M | Depends on A-51 detection; error routing alone will not catch the no-textbox case. Needs a larger, temporarily clickable panel and Esc (A-25). | – |
| A-16 | Spotlight coaching on the pill (owner; B-32, B-41 folded) | §10, §13.13 | Screen dims, pill highlighted, hint "Next step: Open Cursor or any IDE" / "Open your email inbox" (obs) | Absent; onboarding only draws a tray icon inside its own window (`TryIt.tsx:697-713`, `TrayPanel.tsx:93-201`). V | BUILD | M | Full-screen click-through dim panel + hint anchored to the pill. Hint content per first-use choice comes from B-31. Linux overlay hidden by default (`settings.rs:618-625`). | tauri-nspanel |
| A-17 | Position, size, animation | §2 | Bottom center; sizes, colors, timings (unk) | Bottom or top (`shortcut/mod.rs:713-734`); display under cursor, above Dock via work area (`overlay.rs:178-278`); animations (`RecordingOverlay.css:115-159`). V | TUNE | S | Restyle; keep Top as an option. | – |
| A-18 | Live transcript panel while speaking | – (Handy only) | Wispr shows only a waveform (obs) | "Live" style is the macOS default (`settings.rs:618-625`, opened) with committed/tentative text (`RecordingOverlay.tsx:288-346`). V | HAVE | – | Strength. | – |
| A-19 | Bar on all Spaces and over full-screen apps | §2 (implied) | (unk) | `can_join_all_spaces().full_screen_auxiliary()` (`overlay.rs:471-475`, opened). V | HAVE | – | – | – |
| A-20 | Hide bar from screen capture | §11, §14 | – | – | XREF | – | See E-28 (owner). The bar must honor it. | – |
| A-21 | Notetaker recording state on the bar | §14 | – | – | XREF | – | See E-29. | – |

#### A2. Hotkey grammar and editor

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| A-22 | Push-to-talk: hold fn (owner of the default) | §3, §13.7 | Default `fn`, "Hold to say something short" (obs) | Modes Toggle / PushToTalk / HoldOrToggle, default HoldOrToggle (`settings.rs:173-186`). macOS default binding `option+space` (`settings.rs:945-946`, opened). handy-keys (macOS default backend, `settings.rs:238-245`) accepts `fn` (`shortcut/handy_keys.rs:404-406`, `:413-423`, opened). V | EXTEND | M | Pass 2 TUNE S→EXTEND M. A modifier-only fn hotkey fires on the fn FlagsChanged and stays "pressed" through fn+arrow/F-keys (`handy-keys/manager.rs:56-75`); with HoldOrToggle a short fn chord becomes a tap that locks recording (`transcription_coordinator.rs:485-491`). Needs: accidental-start guard, fallback for keyboards without fn (A-202), Globe action handling (A-35), fn+Esc (A-201). Decision DEC-09. | – |
| A-23 | Hands-free: double-tap fn | §3 | Double-tap fn (or fn+Space) starts, press again to stop (obs) | HoldOrToggle locks on a **single** tap under 300 ms (`transcription_coordinator.rs:471-492`, opened; `settings.rs:408-411`). 30 ms debounce (`:14`, `:320-333`). V | EXTEND | M | Add a double-tap detector to the coordinator (window ~300-400 ms; Wispr thresholds unk). Whether a single tap should do nothing is a product choice (R1 says nothing about single taps). Extend coordinator tests. | – |
| A-24 | Hands-free: fn + Space (turn a hold into a lock) | §3 | Second hands-free binding (obs) | With fn as PTT, fn fires first; a separate `fn+space` binding is ignored while another records (`transcription_coordinator.rs:401-405`); FN matched exactly (`handy-keys/types/modifiers.rs:173-174`, opened). One global activation mode for all transcribe bindings (`shortcut/handler.rs:37-46`). V (paths), I (runtime) | BUILD | M | Needs a chord layer ("while holding X, Y locks") and per-binding activation modes. Same layer serves A-29. | skhd / Karabiner semantics for reference |
| A-25 | Cancel: Esc, "Dismiss dictation and notifications" | §3 | Esc dismisses dictation and notifications (obs) | Esc registered only while recording (`shortcut/mod.rs:60-131`), acts only when `is_recording()` (`handler.rs:61-68`), unregistered at stop (`actions.rs:628-629`), so it cannot abort transcription/post-processing (✕ and tray can). V | EXTEND | S | Widened in pass 2 and by the coverage audit: one Esc policy for processing, fallback card (A-15), pill messages (A-13/A-14), toasts (E-26, E-27) and spotlights (A-16). Do not swallow Esc when idle. fn-held case: A-201. | – |
| A-26 | Paste last transcript (⌃⌘V) | §3, §11 | Hotkey + tray item, disabled until one exists (obs) | Absent: bindings are `transcribe`, `transcribe_with_post_process`, `cancel`, `test` (`actions.rs:965-986`, opened). V | BUILD | S-M | Hotkey fires while ⌃⌘ are held; injected ⌘V may become ⌃⌘V: wait for modifier release (RC-05). Keyed bindings need the Carbon shadow under Secure Input (`secure_input.rs:4-7`). Reuse `tray.rs:611-616` lookup and `clipboard::paste`. | – |
| A-27 | Copy last transcript (⌃⌘C) | §3, §6 | Hotkey and menu item with preview (obs) | Tray item only (`tray.rs:518-524`, `:652-681`); prefers post-processed text (`tray.rs:611-616`, opened). No hotkey. V | EXTEND | S | Add binding id + ACTION_MAP entry. Text choice must match history copy (C-39). | – |
| A-28 | "Press Enter" shortcut (key or mouse button; owner) | §3 | Not set by default; "Trigger enter with a mouse shortcut (or another key) to send messages faster" (obs) | No hotkey. `send_return_key` exists (`clipboard.rs:750`); handy-keys parses mouse buttons (`handy-keys/types/key.rs:187-194`); tap reports middle/side buttons (`listener.rs:346-419`). Handy has auto-submit instead (D-61). V | BUILD | S | Needs "not set" (A-32). Whether the recorder captures mouse buttons: RC-07. | – |
| A-29 | Command Mode chord (fn + Ctrl) | §3 | "Select text and ask Flow or Perplexity" (obs label; behavior unk) | `fn+ctrl` storable (`handy_keys.rs:413-423`) but fn fires first (A-24). V/I | EXTEND | M | Expressibility only; behavior is D-44. Same chord layer as A-24. | – |
| A-30 | Feature shortcuts plumbing (Scratchpad, Opt+M, Transforms, ⌥O) | §3, §10, §11 | Per-feature shortcuts, some "not set" (obs) | Init registers ids present in defaults (`tauri_impl.rs:15-37`, `handy_keys.rs:424-458`); ACTION_MAP is static (`actions.rs:965-986`). V | EXTEND | S | Plumbing only, incl. runtime-generated ids for user transforms. Owners of the actions: C-77 (Scratchpad), E-05 (Notetaker), D-32 (transforms), D-40 (⌥O). | – |
| A-31 | Several bindings per action ("+") | §3 | Hands-free lists two bindings; "+" adds more (obs) | One `current_binding: String` (`settings.rs:81-88`); one HotkeyId per binding (`handy_keys.rs:122-124`). V | BUILD | M | Data-model change + settings migration (`settings.rs:1198` `apply_settings_migrations`); touches Secure Input shadowing. | – |
| A-32 | "Not set" bindings | §3 | Press Enter, Open Scratchpad "not set" (obs) | Empty rejected by `change_binding` (`shortcut/mod.rs:169-172`) and by the handy-keys validator (`handy_keys.rs:414-416`, opened). V | EXTEND | S | Allow empty/None and skip at registration. | – |
| A-33 | "Change hotkeys" panel (pencil, trash, +, Reset to default, Done) | §3, §13.7 | One modal from Help > Shortcuts and onboarding (obs) | Per-binding rows in General (`GeneralSettings.tsx:29-33`), per-row reset (`ShortcutInput.tsx:126-127`), safe capture mode (`shortcut/mod.rs:282-333`); refuses capture under Secure Input (`handy_keys.rs:485-493`). No single panel, delete, add or global reset. V | EXTEND | M | UI on solid plumbing. Settings row summary: C-80; Help > Shortcuts: C-100. | – |
| A-34 | Tap vs hold threshold | §3, open item 2 | (unk) | `hold_threshold_ms` 300 (`settings.rs:408-411`), Debug-only UI (`debug/HoldThreshold.tsx:15-27`). V | HAVE | – | Tune after testing. | – |
| A-35 | fn / Globe key support (owner; B-23 folded) | §3, §13.7 | fn default; onboarding shows a globe-icon fn key (obs) | handy-keys supports FN (`handy-keys/types/modifiers.rs:23`, `:107`), fn alone arrives as FlagsChanged 0x3F (`listener.rs:320-340`); the tap blocks the press of a registered modifier-only hotkey but always passes the release (`listener.rs:331-333`). Tauri backend rejects fn (`tauri_impl.rs:52-58`, opened). Fallback (`shortcut/mod.rs:44-55`, opened) runs only if `handy_keys::init_shortcuts` errs; `HandyKeysState::new` always returns Ok and a tap failure is only logged in the thread (`handy_keys.rs:88-119`, opened). V | EXTEND | M | Conflict resolved: B-23's "unknown" is replaced by A's verified finding (fn alone is supported). Pass-1 "fn reset to default" was wrong: the reset happens only on a user backend switch (`shortcut/mod.rs:486-547` via `:353`); a fallback would leave **no** dictation hotkey; tap failure → A-208. Open: does macOS still run the Globe action (emoji picker, input source, Apple Dictation)? RC-02. Event tap needs Accessibility, not Input Monitoring. | Karabiner/Hammerspoon notes |
| A-36 | Keyboard backend choice | – (Handy only) | – | Behind the Experimental toggle (`AdvancedSettings.tsx:68-75`, opened). V | HAVE | – | Pass 2 TUNE→HAVE: consumers never see it. | – |
| A-201 | Esc cancels while fn is held | §3 | Esc dismisses (obs); behavior while fn held (unk) | `escape` binding has no modifiers (`settings.rs:983-992`); handy-keys adds FN from flags to every key event (`listener.rs:209-214`) and matches FN exactly (`modifiers.rs:173-174`), so held fn blocks the Esc match. V (matching), I (outcome) | EXTEND | S | Register `fn+escape` too (needs A-31) or ignore FN for cancel. Add a handy-keys-level test. | – |
| A-202 | Dictation/mic key (F5) or Globe as hotkey | §3, §13.7 | fn/Globe default; keyboards without fn (unk) | handy-keys maps 0xB0 to `Key::Dictation`, parses "dictation"/"voice"/"globe" (`handy-keys/platform/macos/keycode.rs:133-136`, `types/key.rs:180-185`); hardware presses carry SecondaryFn, so bind `fn+dictation`. V | TUNE | S | Fallback for keyboards without fn; blocking tap should also stop Apple Dictation (I). | – |
| A-203 | Hotkey labels on non-US layouts | §3 | (unk) | handy-keys matches ANSI physical keycodes, no layout translation (`keycode.rs:7-30`); paste path is layout-aware (`input.rs:70-80`). V | EXTEND | S | "⌃⌘V"/"⌥S" labels name the US position. Translate labels via TIS/UCKeyTranslate as `input.rs` does. | `input.rs` TIS lookup |
| A-205 | Secure Input explanation (tray warning, banner, diagnostic) | – (Handy only) | (unk) | Tray warning + badge (`tray.rs:194-202`, `:474-493`), culprit app and covered bindings (`secure_input.rs:24-45`). V | HAVE | – | Strength; should also reach the pill (A-12). | – |
| A-206 | Press during processing queues the next dictation | – (Handy only) | (unk) | Coordinator remembers one press while busy (`transcription_coordinator.rs:335-376`, `:509-520`). V | HAVE | – | No pill state shows it was queued (I). | – |
| A-207 | Hotkey tap self-heals after macOS disables it | – | (unk) | Re-enables on `TapDisabledByTimeout/UserInput` (`handy-keys/listener.rs:172-193`). V | HAVE | – | Strength. | – |
| A-208 | Silent loss of every hotkey when the event tap cannot be created | §13.4 | (unk) | Tap failure logged inside the manager thread (`handy_keys.rs:114-119`, opened); `init_shortcuts` still Ok, so no fallback; each register fails and is logged (`:447-452`). No UI event. V | EXTEND | S | Trigger: Accessibility revoked/reset, app re-signed (TCC entry invalid). Needs a health check, tray warning, "Fix permission" deep link. | `tauri-plugin-macos-permissions` |
| A-209 | Second dictation hotkey "Transcribe with Post-Processing" (⌥⇧Space) | §3 (no equivalent) | One dictation hotkey; cleanup applies to all (obs) | Default `option+shift+space` (`settings.rs:963-982`), registered only when `post_process_enabled` (`handy_keys.rs:436-439`); SIGUSR1 also drives it (`signal_handle.rs:48-51`). V | TUNE | S | Remove from consumer surface or repurpose for Command Mode. Making the main binding run cleanup is D-01 (binding id appears in ~9 call sites). | – |
| A-210 | Hold vs hands-free (locked) state on the pill | §2, §3 | Held-key and hands-free pill states (unk) | Coordinator emits `RecordingPhase::Locked` (`transcription_coordinator.rs:17-30`); only onboarding listens (`useTryEngine.ts:131`). V | EXTEND | S | Lock glyph or ✓ only when locked. | Existing phase event |
| A-212 | Bar placement on multiple displays and display changes | §2 | Which display hosts the bar (unk) | Placement computed only at show time on the cursor's display (`overlay.rs:178-210`, `:510-594`); no screen-change listener (I, no `NSApplicationDidChangeScreenParameters`). V/I | BUILD | S | Needed by A-01. Decide cursor display vs focused window vs main display; re-place on connect, resolution, Dock autohide. | – |

#### A3. Audio capture

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| A-37 | Microphone list and selection (owner; C-81 folded) | §5, §6, §13.6 | Settings Microphone (Change), Microphone ▸ menus; list: Built-in (recommended), Auto-detect (system setting), others (obs) | "Default" pseudo-entry + devices (`commands/audio.rs:202-232`); selector + `ChannelSelector` in General (`GeneralSettings.tsx:42-44`); clamshell mic in Debug. V | TUNE | S | Rename "Default" → "Auto-detect (system setting)"; hide channel selector unless multi-channel. Menus: A-56, A-63. | – |
| A-38 | "Recommended" microphone (built-in); Bluetooth caution (owner; B-20, A-213 folded) | §2, §13.6 | "Built-in mic (recommended)" first; "Bluetooth devices are less reliable" (obs) | No device-type knowledge; unset = system default (`managers/audio.rs:569-571`, `:582-588`). handy-recorder already has CoreAudio transport-type code for Bluetooth/BLE (`handy-recorder/backend/headset_macos.rs:10-35`), `pub(crate)`; `InputDevice` has no transport field (`device.rs:14-30`). No component labels a mic "recommended" (grep). V | EXTEND | S | Default to built-in when the system default is a Bluetooth headset (avoids HFP drop). Copy ~40 lines or add `transport` upstream. Onboarding and Settings consume the label (B-20, C-81). | `objc2-core-audio` (already via handy-recorder) |
| A-39 | Auto-detect (follow system default) | §13.6 | "Auto-detect (system setting)" (obs) | `None` = system default (`managers/audio.rs:582-588`); missing selected mic falls back and persists Default (`:645-663`). V | HAVE | – | Strength. | – |
| A-40 | Live level meter outside the pill / mic test (owner; B-19 folded) | §13.6 | "Do you see purple bars while you speak?" with Change microphone / Yes (obs) | Levels emitted in the capture sink per recording (`audio_toolkit/audio/sink.rs:172-185`), only to the overlay and only if enabled (`overlay.rs:739-741`, `:766`, opened). Try it only detects a silent take afterwards (`useTryEngine.ts:171-174`). V | BUILD | M | Conflict resolved: A said BUILD S, B said EXTEND M for the same plumbing. One row, BUILD M: a mic-monitor mode that opens the stream without recording, emits to the requesting window, and closes cleanly so the orange dot does not linger; plus the onboarding/settings UI. | `audio_toolkit` level callback |
| A-41 | Mute all audio while dictating (owner; C-90 folded) | §5 | Toggle, off in the observed account (obs) | `mute_while_recording` default false (`settings.rs:494-495`); snapshot/restore (`managers/audio.rs:743-781`); macOS uses `osascript` (`managers/audio.rs:100-111`, opened); applied after first sample (`actions.rs:585-587`). V | HAVE | – | `osascript` spawn per dictation (I latency). Must be skipped during a meeting (E-205). String key under `settings.debug.*` (cosmetic). | coreaudio-rs |
| A-42 | Dictation sounds (owner; C-89 folded) | §5 | "Dictation and notification sounds": seen on in a used account (obs); fresh-install default (unk) | `audio_feedback: false` (`settings.rs:999`, opened); start chime after first sample (`actions.rs:578-584`); theme/volume/output (`settings.rs:281-306`, `:414-417`; UI `GeneralSettings.tsx:42-51`); theme picker in Debug. V | TUNE | S | Default is a product decision (DEC-14). Keep themes. | – |
| A-43 | Clamshell mic, input channel | – (Handy only) | – | `managers/audio.rs:554-571`; `settings.rs:440-445`; in Debug (`DebugSettings.tsx:74-75`). V | HAVE | – | Already out of the main path. | – |
| A-44 | Mic permission and no-device errors | §13.4 | Permission step with a picture of the prompt (obs) | Typed errors (`actions.rs:600-613`) → main-window toasts (`App.tsx:155-179`). V | HAVE | – | Should also reach the pill (A-12). | – |
| A-45 | Orange mic indicator only while recording | §2 | Orange indicator during recording (obs) | On-demand mic by default (`settings.rs:580-582`); stream closes after idle (`managers/audio.rs:665-687`). V | HAVE | – | Always-on mic is Debug-only. | – |
| A-211 | Mic lost mid-dictation | §13.6 | (unk) | Recovery only at next start (`managers/audio.rs:783-800`, `:1028-1040`, `:645-663`); nothing shown during the recording (I, no event found). | EXTEND | S | "Mic disconnected, kept what we heard". RC-06. | `handy-recorder/error.rs:125` |
| A-213 | Bluetooth detection reuse | §2, §13.6 | – | – | XREF | – | Merged into A-38. | – |

#### A4. Insertion

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| A-46 | Paste mechanics | §1.1 | Text appears in focused field; method (unk) | ⌘V default on macOS (`settings.rs:247-255`), layout-aware keycode (`input.rs:77`, `:133`), main-thread injection (`clipboard.rs:22-48`); alternatives Direct/None/Shift+Insert/Ctrl+Shift+V/script (`settings.rs:162-171`). V | HAVE | – | Strength (Dvorak, AZERTY). | – |
| A-47 | Clipboard preserve and restore | – | (unk) | Legacy path saves text, or an image only when there is no text (`clipboard.rs:78-92`, opened), restores after 60+60 ms (`settings.rs:664-670`); RTF/HTML/files/multi-item lost. Receipt-based "reliable paste" is Debug-gated, default false (`settings.rs:514-518`, opened; `paste_tx/mod.rs:1-32`). V | EXTEND | S | Known race (#502, `paste_tx/mod.rs:3-7`). Promote reliable paste after testing; snapshot all pasteboard types. Prerequisite for any ⌘C selection fallback (D-29). | Maccy-style multi-type snapshot |
| A-48 | Keep transcripts out of clipboard managers | – | (unk) | Reliable path declares Concealed/Transient/AutoGenerated (`paste_tx/macos.rs:32-40`, `:283-288`); default path writes plain text via the plugin (`clipboard.rs:50-61`, opened). V | EXTEND | S | Privacy promise. A-204 (copy to clipboard) also bypasses concealment. | nspasteboard.org |
| A-49 | Exactly-once insertion | – | (unk) | `PASTE_LOCK` (`clipboard.rs:18-20`); cancel-generation checks (`actions.rs:777-782`); session counter (`actions.rs:81-83`, `:844-849`). V | HAVE | – | Strength. | – |
| A-50 | Secure / password fields | – | (unk) | No secure-field detection for insertion; Secure Event Input polled only for hotkeys (`secure_input.rs:9-17`). No AX client in the tree (see AF-05). V | EXTEND | S | Policy: refuse paste into secure fields and show the card. `IsSecureEventInputEnabled` is cheap; `AXSecureTextField` role check needs the AX layer (A-51). | – |
| A-51 | No focused text field: detection | §2 | Card "Select a textbox first, then dictate" (obs); how Wispr decides (unk) | No AX focus check; blind ⌘V (`clipboard.rs:799-898`). No AXUIElement client crate (AF-05). V | BUILD | M | Shared AX layer with D-29/D-58. Electron/Chromium often expose no AX tree without `AXManualAccessibility`; a false "no textbox" is worse than blind paste, so fall back to paste + card. Same Accessibility permission. RC-04. | `accessibility-sys`, `get-selected-text` crate |
| A-52 | No focus stealing | §2 | Clicking the bar moved focus to Wispr (obs, synthetic click) | Non-activating NSPanel, `no_activate`, `focusable(false)` (`overlay.rs:466-472`, opened). V | HAVE | – | Strength; keep for every clickable state. | – |
| A-53 | Paste failure feedback | §1.4 | Soft failure, text kept (obs) | `paste-error` → hidden main-window toast (`actions.rs:838-842`; `App.tsx:185-194`). V | EXTEND | S | Route to the card (A-15). Rarely fires in the no-textbox case. | – |
| A-54 | Auto-submit and trailing space | – (Handy only) | Wispr has "Press Enter" instead (obs) | `auto_submit` Enter/Ctrl+Enter/Cmd+Enter, `append_trailing_space` (`settings.rs:476-479`, `:496-497`; `clipboard.rs:882-890`). V | HAVE | – | Keep in Advanced. Per-app variant: D-61. | – |
| A-204 | Keep transcript on clipboard after paste | §1.4 | (unk) | `ClipboardHandling::{DontModify (default), CopyToClipboard}` (`settings.rs:188-194`; `clipboard.rs:892-895`), UI in Advanced. V | HAVE | – | Safety net until A-15; defeats A-48 concealment. | – |

#### A5. System integration (tray, app menus, login, Dock)

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| A-55 | Menu-bar status icon with state | §6, §11 | Status icon exists (obs) | Template icon idle/recording/transcribing + warning (`tray.rs:194-221`). V | HAVE | – | – | – |
| A-56 | Tray menu content parity | §11 | Start Notetaker, Open Wispr Flow, Check for updates, Paste last (⌃⌘V, disabled until one exists), Shortcuts, Microphone ▸, Languages ▸, Help center, Talk to support, Share feedback, Hide next meeting, Quit (obs) | Version, Copy last, Model ▸, Unload model, Settings (⌘,), Check for updates, Quit, Cancel while busy (`tray.rs:501-587`); model submenu lists downloaded models (`tray.rs:65-66`, `:327-337`). V | EXTEND | S | Add Paste last, Shortcuts, Microphone ▸, Languages ▸, Help and Feedback. Remove Model ▸ and Unload (B-207). Meeting items: E-05, E-07. Strings i18n-generated (`tray_i18n.rs`). | – |
| A-57 | Copy/paste-last disabled when none exists | §11 | "disabled until one exists" (obs) | Copy last always enabled (`tray.rs:518-524`), silent no-op when empty (`:656-673`). V | TUNE | S | – | – |
| A-58 | Tray meeting items | §11 | – | – | XREF | – | See E-05 (Start Notetaker), E-07 (upcoming / hide next meeting). | – |
| A-59 | "Open app" item | §11 | "Open Wispr Flow" (obs) | "Settings" shows the main window (`tray.rs:504-510`; `lib.rs:287-289`). V | TUNE | S | Copy change. | – |
| A-60 | Check for updates in tray (owner; E-52 folded) | §11 | Tray item (obs) | Tray item emits `check-for-updates` only if checks are effectively enabled (`lib.rs:294-299`, opened); hidden when forced off by env (`tray.rs:589-597`). V | HAVE | – | Small TUNE: the click is a silent no-op when the user turned checks off. Hide it or open settings. | – |
| A-61 | Help center / Talk to support / Share feedback (tray) | §6, §11 | – | – | XREF | – | Hosted help: C-102 (OUT). Local Help and Feedback: C-13, C-103. Tray entry is part of A-56. | – |
| A-62 | Quit | §11 | "Quit Wispr Flow (⌘Q)" (obs) | Quit ⌘Q (`tray.rs:497`, `:525`; `lib.rs:323-325`). V | HAVE | – | – | – |
| A-63 | App menus (owner of all: Dictation, My Voice, Help, Notetaker, Edit/Window) | §6, §11 | Menus File, Edit, Dictation, Notetaker, My Voice, View, Help, Window. Dictation: words dictated, Copy last with preview, Microphone ▸, Languages ▸. My Voice: counts, Add a word, Create a snippet. Help: search box, Help center, Talk to support (⌘/), Share feedback, Shortcuts. File/View contents (unk) (obs) | No custom app menu: only the tray calls `set_menu` (`tray.rs:409`). Tauri 2.11.5 default macOS menu is active (`tauri-2.11.5/src/app.rs:1620`) with File/Edit/View/Window/Help (`menu/menu.rs:142-241`). V | BUILD | M | A custom menu **replaces** the default: rebuild Edit (undo, copy, paste, select all) or ⌘C/⌘V/⌘Z break in Hub text fields (C-203). Counts need C-30, C-58, C-69 and live refresh (C-213). Notetaker menu (E-209) is Notetaker scope. Build one fork-owned menu module. | Tauri `Menu`, `PredefinedMenuItem` |
| A-64 | Launch at login (owner; B-35, C-85, E-87 folded) | §5 | Seen on in a used account (obs); fresh-install default (unk) | Backend default false (`settings.rs:592-594`, opened) **but** onboarding pre-checks "Start at login" (`onboardingStore.ts:72` `atLogin: true`, opened) and saves it at Done (`OnboardingFlow.tsx:154`; `TrayPanel.tsx:185-198`). SMAppService login item on macOS 13+ (`autostart.rs:1-27`). V | HAVE | – | Conflict resolved (A/C/E said TUNE, B said HAVE): effective first-run default is on via onboarding, so HAVE. Settings toggle sits in Advanced (`AdvancedSettings.tsx:38`); moving it is part of C-79. | – |
| A-65 | Show app in Dock toggle (owner; C-87 folded) | §5 | Separate toggle, seen on in a used account (obs) | Dock follows window visibility: Accessory when hidden with tray, Regular when shown. Decided in three places: launch `lib.rs:147-163`, show `lib.rs:113-117`, close `lib.rs:1108-1123`; no-tray guard `lib.rs:1093-1097`. V | EXTEND | S | `show_in_dock = true` = never demote to Accessory in launch and close paths. Activation-policy flips are fragile on macOS 26 (`lib.rs:129-146`). | – |
| A-66 | "Show Flow Bar at all times" setting (owner; C-86 folded) | §5 | Toggle, seen on in a used account (obs) | Overlay style None/Minimal/Live, Top/Bottom (`settings.rs:136-146`; `ShowOverlay.tsx:21-40`); no idle bar. V | BUILD | S | Setting is trivial; the work is A-01. | – |
| A-67 | Start hidden, no-tray, CLI and signals | – (Handy only) | – | `start_hidden` (`settings.rs:418-419`); `--toggle-transcription`/SIGUSR2 (`signal_handle.rs:18-51`). V | HAVE | – | Strength for scripting. | – |
| A-68 | Overlay follows theme | §4 | Dark mode exists (obs) | `theme-changed` re-applies the overlay palette (`shortcut/mod.rs:650-655`). V | HAVE | – | – | – |
| A-69 | Tray icon recovery | – (Handy only) | – | Re-creates NSStatusItem on relaunch (`tray.rs:627-650`). V | HAVE | – | Strength. | – |

### Area B: onboarding, guided learning, permissions, models, languages

Handy's onboarding today: `STEP_ORDER` has 6 steps, welcome → permissions → model → demo → try → done (`onboardingStore.ts:17-24`, opened); Try it and All set share one mounted screen; Permissions is skipped when already granted. Progress persists in webview `localStorage`.

#### B1. First-run journey

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| B-01 | Named-stage stepper | §13 frame | GET STARTED > PERMISSIONS > SET UP > LEARN > PERSONALIZE, thin purple progress line (obs) | Unlabeled dashes (`OnboardingShell.tsx:40-61`); count from `STEP_ORDER` minus skipped Permissions (`OnboardingFlow.tsx:128-132`). V | TUNE | S | Labels need 26 locales. | – |
| B-02 | Back link top-left, Help chip bottom-left | §13 frame | Back top-left, Help chip bottom-left (obs) | Back in the footer, bottom-left (`OnboardingShell.tsx:69-80`); no Help chip. V | TUNE | S | Help chip needs a local destination (C-13). | – |
| B-03 | Two-column layout; dark "Learn" screens | §13 frame | Left controls, right illustration/demo/macOS screenshot; Learn screens dark (obs) | Fixed 720×600 centered frame shared by all screens (`OnboardingShell.tsx:38`); no dark variant. V | TUNE | L | Pass 2 M→L: touches every screen incl. `Model.tsx` (634 lines) and `TryIt.tsx` (737 lines) and their CSS. Visual only. | – |
| B-04 | Primary button disabled until a choice | §13 frame | Black pill, grey until a choice (obs) | Same pattern (`Welcome.tsx:85-92`, `Permissions.tsx:133-143`). V | HAVE | – | Restyle only. | – |
| B-05 | Welcome screen | §13 | First welcome/account screens not recorded (unk) | Welcome: mascot, "Speak instead of typing, in any app", language picker, no account (`Welcome.tsx:78-123`). V | HAVE | – | Sign-up itself is OUT (no account). Mascot is Handy's brand (E-63). | – |
| B-06 | "How can Flow help you?" (Dictation / Notetaker) with live demo per card | §13.1 | Multi-select cards; right side animates a demo of the selected card (obs) | Absent from `STEP_ORDER`. V | BUILD | S | Only meaningful with a second product mode. Scope note: depends on the Notetaker decision (DEC-05). | `Demo.tsx` engine |
| B-07 | "How do you currently record meetings?" | §13.2 | Meeting-tool chips (obs) | Absent | OUT | – | Marketing quiz for the cloud meeting product. E-50 folded. | – |
| B-08 | "What's frustrating about meetings?" | §13.3 | Multi-select list (obs) | Absent | OUT | – | Same as B-07. | – |
| B-09 | Permission rows that tick when granted | §13.4 | Three rows tick as granted (obs) | Mic + Accessibility rows, Allowed/Waiting (`Permissions.tsx:27-80`, `:170-200`). V | HAVE | – | Copy already privacy-first. | – |
| B-10 | Picture of the macOS prompt / Settings pane before it appears | §13.4, §11 patterns | Right half shows the system prompt and Settings pane (obs) | Icons only (`Permissions.tsx:172`, `:190`) and a text hint (`:104-111`). V | TUNE | M | Pass 2 EXTEND→TUNE: illustration work only (per macOS version, light/dark, locales). Prefer SVG mock-ups. | `DesktopBar.tsx`/`WindowBar.tsx` drawing style |
| B-11 | Mic permission: prompt, then System Settings | §13.4 | Real macOS prompt follows (obs) | `usePermissions.ts:231-252`, focus check after 1 s (`:239-243`). V | HAVE | – | `NSMicrophoneUsageDescription` copy is terse (`src-tauri/Info.plist:5-6`, opened). | – |
| B-12 | Accessibility permission with polling and relaunch link | §13.4 | Wispr asks for "recognize meetings", mic, meeting audio; Accessibility not seen (obs) | Request + 1 s poll + init shortcuts on grant (`usePermissions.ts:77-83`, `:143-176`); relaunch link (`Permissions.tsx:113-117`, `:201-212`). V | HAVE | – | – | – |
| B-13 | Resume after relaunch | §11, §13 | After a permission grant the app restarts and the gate is gone (obs) | Flow state in `localStorage` `handy.onboarding` (`onboardingStore.ts:26-46`, `:121-140`); "Welcome back" (`Permissions.tsx:148-156`). V | HAVE | – | Two sources of truth (rough edge RE-B2). | – |
| B-14 | System-audio permission row | §11, §13.4 | – | – | XREF | – | See E-02 (owner). The permissions plugin covers Screen Recording, not system-audio capture. | – |
| B-15 | "Allow Wispr to recognize meetings" | §13.4 | – | – | XREF | – | See E-06 (call detection). | – |
| B-16 | Connect your calendar (Google/Outlook OAuth) | §13.5 | OAuth in the browser, "You're now logged in", reopen prompt; "different calendar" options (obs) | Absent | OUT | – | Account/cloud OAuth. Local equivalent (EventKit) is E-42. | – |
| B-17 | Returning-user permission check | §11 | Gate reappears when a requirement is missing (inf) | Every launch checks mic + Accessibility (`App.tsx:263-303`); mid-session revocation surfaces only as a toast (`App.tsx:156-165`). V | HAVE | – | A tap that dies after revocation is A-208. | – |
| B-18 | Windows/Linux permission handling | n/a | – | Windows mic status (`usePermissions.ts:72-75`); Linux guide link (`TryIt.tsx:493-504`). V | HAVE | – | Platform later. | – |
| B-19 | Mic test with live level meter | §13.6 | – | – | XREF | – | See A-40 (owner, backend + UI). | – |
| B-20 | Change microphone list with "recommended" and Bluetooth caution | §13.6 | – | – | XREF | – | See A-38 (detection) and A-37 (list). Onboarding only offers a picker after a silent take (`TryIt.tsx:438-446`). | – |
| B-21 | Shortcut test: on-screen key lights when pressed | §13.7 | "Does the button turn purple while pressing it?" fn/globe key graphic, Edit shortcut / No / Yes (obs) | Keycaps light from `recordingStateEvent` "started" (`useTryEngine.ts:117-125`), which never fires without a model (`actions.rs:464-475`). Demo keycaps are animated, not live (`Demo.tsx:270-277`). V | EXTEND | S | A model-free key test should use the raw `handy-keys-event` stream (`useShortcutCapture.ts:92`, `:141`), which suspends bindings (`:111`). Inherits A-35 Globe risks if fn is default. "No" path: B-201. | `useShortcutCapture` |
| B-22 | Edit shortcut in place; Reset to default chip | §13.7 | Opens the hotkey panel; "Reset to default" chip when changed (obs) | In-place capture on Demo/Try it/All set (`Demo.tsx:492-502`, `TryIt.tsx:360-367`, `TrayPanel.tsx:146-182`); no reset chip. V | TUNE | S | Panel itself: A-33. | – |
| B-23 | fn (Globe) key as default | §3, §13.7 | – | – | XREF | – | See A-22 / A-35. Resolved: handy-keys binds fn alone (verified); B's "unknown" is superseded. | – |
| B-24 | Animated product demo | §13.1, §13.12 | Simulated composer with the pill underneath (obs) | Scripted laptop demo, hold/tap chapters, reduced-motion stills (`Demo.tsx:37-67`, `:152-303`). V | HAVE | – | Strength; base for B-31, B-40, B-42. | – |
| B-25 | Practice in a simulated app ("Write an email hands free"), Skip | §13.8 | Simulated Gmail composer (To, Subject), Skip top-right (obs) | Real dictation into a drawn Notes app, hold then tap lessons (`TryIt.tsx:588-607`); Skip is a footer button (`:491-517`). V | TUNE | S | Swap/add an email scene. Practice takes enter history (B-211). | `WindowBar`, `useTryEngine` |
| B-26 | Explain failed first takes | – (Handy only) | – | Failed / silent mic / empty / landed outside (`useTryEngine.ts:155-224`; `TryIt.tsx:424-459`). V | HAVE | – | Strength. | – |
| B-27 | "Nice job! You just spoke." celebration | §13.9 | Dark, dramatic screen (obs) | Mascot high five, glove flies to tray (`TryIt.tsx:221-305`, `:633-657`). V | HAVE | – | Tone and mascot are brand calls. | – |
| B-28 | Time-saved pitch with typing-time slider | §13.10 | "You could save N hours a week", slider default 6 h/day (obs) | Absent. V | BUILD | S | Frontend math from B-29. | – |
| B-29 | 30-second typing test | §13.10 | Monospace paragraph, live words/min, seconds left, language picker, Recalculate, Try Again, "Don't believe us?" link (obs) | Absent; speaking rate computable (`transcriptionCompleteEvent` `audio_seconds`, `useTryEngine.ts:195`). V | BUILD | M | Sample paragraphs per language. WPM basis: decision DEC-11. | – |
| B-30 | Referral "Give the magic of Flow. Get a month of Pro." | §13.11 | Invite link + Copy (obs) | Absent | OUT | – | Referral/Pro. Local "share with a friend" (user, §10): C-11. | – |
| B-31 | "How would you like to use Flow first?" chooser with captioned demos, "Try it now" | §13.12 | 7 chips (Chat with AI, Take a note, Write a message, Draft an email, Write a post, Write a document, Prompt Cursor or Windsurf), looping captioned videos (obs) | Absent. V | BUILD | M | Animated scenes are lighter than videos (decision DEC-15). Choice feeds the A-16 hint. | `Demo.tsx` scene engine |
| B-32 | "Next step" spotlight on the real pill | §13.13 | – | – | XREF | – | See A-16 (owner). | – |
| B-33 | "Try Flow in another app / Get started" banner | §13.13 | – | – | XREF | – | See C-29. | – |
| B-34 | End state: hub with stats, "New!" tags | §13.14 | – | – | XREF | – | Landing page: C-02. "New!" tags: C-15. Today onboarding ends on General settings (`OnboardingFlow.tsx:151-160`, `App.tsx:87-88`). | – |
| B-35 | Launch at login choice | §5 | – | – | XREF | – | See A-64 (HAVE via onboarding toggle). | – |
| B-36 | Summary of choices at the end | – (Handy only) | – | All set lists Languages, Speech model, Shortcut (`TrayPanel.tsx:111-182`). V | TUNE | S | Remove the "Speech model" row (B-207). | – |
| B-37 | Pace: ~5 min, never blocked | §13 | ~5 min, 14 screens, mostly single-tap (obs/inf) | "Try it yourself" disabled until the model is on disk (`Demo.tsx:515-524`, opened: `disabled={!ready}`). V | TUNE | S | Real unblocking is B-50. | – |
| B-201 | Shortcut test "No" path | §13.7 | Yes/No buttons (obs); what "No" does (unk) | No troubleshooting when nothing happens; only Permissions has a relaunch hint (`Permissions.tsx:201-212`); with no model the press is ignored (`actions.rs:464-475`). V | BUILD | S | Cover: tap not live (relaunch), Secure Input, no fn key, Globe bound to emoji picker, no model yet. Covers the audit's PARTIAL R1-§13-27. | relaunch link, `handy-keys-event` |
| B-202 | Bootstrap model: first dictation within ~1 min on slow links | §13 | Cloud product, instant (inf) | One download then wait (`Demo.tsx:515-524`). Small catalog models: parakeet-tdt_ctc-110m 135 MB (en), canary-180m-flash 218 MB, SenseVoiceSmall 253 MB; rate measured (`model.rs:310-312`). V | EXTEND | M | Two downloads, weaker first impression; skip when main model ETA < ~60 s. Alternative in decision DEC-01. | `downloadPick` |
| B-203 | Returning user whose model is missing | §11 gate pattern | Gates reappear when a requirement is missing (inf) | Check covers permissions and retired models only (`App.tsx:263-305`); auto-select picks a rank-first on-disk model (`model.rs:1486-1504`); none → recording refused (`actions.rs:464-475`). V | EXTEND | S | Re-pick and download in background; pill says "getting ready". | B-47 pick |
| B-210 | Onboarding window closed mid-flow | §13 | (unk) | Close hides the window (`lib.rs:1103-1118`); download continues while the app runs; quit stops it, partials kept. V/I | EXTEND | S | Notification when ready; shortcut does nothing until then. | tray |
| B-211 | Practice dictations appear in history | §12, §13.13 | Whispered practice email appears in Dictation history (obs) | Every completed transcription is saved (`actions.rs:786`, `:887`); Try-it takes take the same path (I). | HAVE | – | Decide whether to tag onboarding takes. Covers audit PARTIAL R1-§12-08. | – |
| B-212 | Onboarding forces hold-or-toggle activation | §13.7, §13.8 | Hold fn; hands-free via double-tap (obs) | Sets `shortcut_activation = hold_or_toggle` (`OnboardingFlow.tsx:96-105`). V | HAVE | – | Gesture differs from Wispr's double-tap (A-23). | – |

#### B2. Guided learning everywhere

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| B-38 | Dismissible example banners per feature | §4, §10, §11 | – | – | XREF | – | Component and dismissed state: C-23 (owner). Content per feature: C-29, C-61, C-73, D-206. | – |
| B-39 | "How it works" modal | §10 | Modal with a hosted ~1 min 22 s video, speed, fullscreen, PiP (obs) | `Dialog.tsx`, used by `WhatsNewModal.tsx:20-30`; no videos. V | EXTEND | M | Hosted video is a network call; bundled video grows the app. Prefer animated scenes (DEC-15). | Dialog + `Demo.tsx` |
| B-40 | "Try it out" multi-step practice engine, then "Turn on X" | §10 | Two steps, simulated composer, result, spotlight, final CTA (obs) | Engine exists only in onboarding (`useTryEngine.ts`), coupled to the onboarding store (`useTryEngine.ts:44`, `:191-193`). V | EXTEND | M | Lesson-agnostic refactor. Transforms content: D-43. "Show me how" entry: C-29. | refactor `useTryEngine` |
| B-41 | Spotlight on pill after a practice | §10 | – | – | XREF | – | See A-16. | – |
| B-42 | Whisper tutorial ("Show me how", 3 steps) | §11, §12 | Video, how to whisper with built-in mic, whisper an email in a simulated Gmail composer, Continue disabled until it lands (obs) | Absent. V | BUILD | M | Local whisper accuracy depends on model and mic gain; a tutorial that fails is a risk. C-49 folded. | B-40 engine |
| B-43 | Personalization quiz with mic recommendation table | §12 | Back arrow, progress dashes; quiet/shared space, same desk, mics owned (lilac selection), table with "Best match", Done / Email results (obs) | Absent | BUILD | M | "Email results" is cloud: replace with Copy. Worth-it question: decision DEC-15. | – |
| B-44 | Setup checklist with auto-ticking and next-step chaining (owner; C-10 folded) | §4, §10, §11, §12 | Sidebar "Set up Notetaker" card (tour, connect calendar, import, MCP); after the tour the item ticks and Connectors settings opens automatically as the next step (obs) | Absent; fixed sidebar (`Sidebar.tsx:34-77`, opened). V | BUILD | M | Conflict resolved: C-10 said S, B said M; auto-ticking needs hooks on many events, so M. Local items: try in another app, add a word, whisper tutorial, launch at login. Includes the audit's MISSING R1-§11-44 (open the next suggested step after finishing one). | – |
| B-45 | Re-entry: "Start tutorial", Help > Essentials | §10, §11 | Per-feature tutorial buttons and Help popover (obs) | Replay onboarding only from Debug (`App.tsx:316-320`; `Sidebar.tsx:66-69`). V | TUNE | S | Pass 2 EXTEND→TUNE. Per-feature relaunch is B-40/B-42/E-43. | – |
| B-46 | Permission waiting-state recovery | §13.4 | Rows tick when granted (obs) | Re-check on focus and every 15 s; toast after 3 errors (`usePermissions.ts:33-35`, `:180-217`). V | HAVE | – | Strength. | – |

#### B3. Speech models

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| B-47 | No model choice for the user | §13 "for a local product" note | R1 never observed Wispr's model setup (unk). "Silent model choice replaces any model picker" is R1's proposal for our product (inf), not Wispr behavior | Explicit "Choose a speech model" step with 3 cards and "Show all N" (`Model.tsx:77-194`, `:518-633`); recommended preselected (`:99-100`). V | EXTEND | M | Our proposal: drop `model` from `STEP_ORDER`, call `picksFor(langs)` (`catalog.ts:97-109`) after Welcome and start the download. Keep `Model.tsx` for Advanced. Policy in §3 AF-03. | `picksFor`, `downloadPick` |
| B-48 | Curated catalog with benchmarks and method | – | – | `@handy-computer/models` 0.2.3: 76 models, 29 curated ASR, per-language FLEURS error, RTF accelerated/cpu, method thresholds; `recommend()` → best/fast/accurate (`recommendations.js:158-226`); compiled in, no network for the list (`catalog/mod.rs:1-18`). Catalog carries `license` per model. V | HAVE | – | Strength (MIT). Benchmarks measured on a Ryzen 4750U, not Apple Silicon. | – |
| B-49 | Hardware signal for the choice (GPU, RAM, disk, chip) | none (product goal) | – | Backend returns GPU name + `total_vram_mb` (`transcription.rs:1874-1878`, `:1911-1927`); frontend reduces it to a boolean (`useMachineTier.ts:14`) that starts as `true` (`:10`). Intel Macs are CPU-only builds (`src-tauri/Cargo.toml:154-158`, opened). No RAM, free-disk or chip probe. V | EXTEND | M | Pass 2 corrected "one bit". R1 ref corrected: hardware-based choice is a product goal, not R1. `recommend()` is machine-independent by design (`recommendations.js:150-157`). | `sysctlbyname("hw.memsize")` via `libc`; statvfs |
| B-50 | Background download that never blocks setup | §13 note | (unk); see B-47 | Download starts on the Model step (`OnboardingFlow.tsx:162-173`), continues during Demo, Try it waits (`Demo.tsx:515-524`). Backend refuses to open the mic with no loaded/available model (`actions.rs:464-475`, opened). V | EXTEND | L | Pass 2 M→L: removing the UI gate is not enough. Needs queued audio until the model arrives, or a bootstrap model (B-202). Decision DEC-01. See AF-03. | – |
| B-51 | Download progress UI | – | – | `DownloadStatus.tsx:13-124`; `Model.tsx:220-257`. V | TUNE | S | Copy names the model ("Downloading Parakeet…"); say "Getting speech recognition ready". | – |
| B-52 | Resume an interrupted download | – | – | HTTP Range resume, mirror fallback, sha256 (`download.rs:216-311`; `model.rs:2336-2341`); onboarding after relaunch shows "stopped" and waits for Retry (`OnboardingFlow.tsx:58-67`). V | EXTEND | S | Auto-restart on relaunch. | – |
| B-53 | Failure handling: offline, retry | – | – | Backend: 4 HF attempts with 2/4/8 s backoff, 60 s stall watchdog, then mirror with Range resume (`model.rs:2008-2160`; `download.rs:20-27`); after total failure: error toast, no later retry. V | TUNE | S | Missing: offline detection, retry when the network returns. | – |
| B-54 | Disk-space check before download | – | – | None. Models go to the shared HF cache (B-205). V | BUILD | S | Check free space at the HF cache path, not app data. Worst default ~2.4 GB. | statvfs |
| B-55 | Model management page | – | – | Top-level "Models" sidebar section (`Sidebar.tsx:47-53`; `ModelsSettings.tsx:48-181`); footer switcher (`Footer.tsx:33`). V | TUNE | S | Move under Advanced. Other model surfaces: B-207. | – |
| B-56 | Advanced override | – | – | Override everywhere (B-55, B-207); `Model.tsx` is standalone-capable (`StandaloneScreens.tsx:21-76`). V | TUNE | S | Keep one override in Advanced. | – |
| B-57 | Retired-model migration | – | – | 11 retired models with replacements (`model.rs:763-856`); blocking update screen (`App.tsx:237-253`; `ModelMigration.tsx`). V | TUNE | S | Hide = TUNE S; silent background swap = EXTEND S. New installs of the fork never hit it (I). | – |
| B-58 | Fallback auto-select when the selected model disappears | – | – | Picks first downloaded model by editorial rank, ignoring spoken languages (`model.rs:1448-1507`; rank `catalog/mod.rs:38-53`). V | EXTEND | S | Rank 1 is English-only. Use the language-aware pick. | – |
| B-59 | Measure real speed and adapt | – | – | First takes timed; "Try a faster model" link (`useTryEngine.ts:194-196`, `catalog.ts:201-205`, `TryIt.tsx:716-733`). V | EXTEND | S | Switch silently to the `fast` pick in background. | – |
| B-60 | Better default appears later | – | – | Catalog compiled in (`catalog/mod.rs:307-315`); package supports a runtime catalog (`create.js:4-9`), unused. V | BUILD | M | Ship catalog with app updates; optional background upgrade. | – |
| B-204 | Model licence and attribution | – | – | Catalog `license` per model; defaults `cc-by-4.0` (attribution required); nemotron, Fun-ASR, SenseVoice are `other`. No UI shows licences. V | BUILD | S | `other` licences need legal review before becoming silent defaults. Shares a page with E-218. | About page |
| B-205 | Where models live (shared HF cache) | §5 "Store data locally" | "Store data locally" dropdown (obs) | HF downloads go to the shared HF cache (`model.rs:33-38`, opened; `:315-331`); mirror/URL to the models dir; `HF_HOME` set only in portable mode (`portable.rs:43-45`). V | TUNE | S | Privacy page and "delete my data" must cover it (E-64, E-67). Uninstall leaves GBs behind (I). | – |
| B-206 | Clean up superseded models after silent switches | – | – | `delete_model` exists (`commands/models.rs:114`); no caller after a language change or speed fallback. V/I | EXTEND | S | Keep the old model until the new one has loaded once. | `modelStore.deleteModel` |
| B-207 | Model concepts outside the Models page | §13 note | No model concept seen (obs/inf) | General card "<model> settings" with per-model language selector and Translate to English (`ModelSettingsCard.tsx:21-41`, `GeneralSettings.tsx:41`); tray model submenu (`tray.rs:65-66`, `:327-337`); footer selector (`Footer.tsx:33`). V | TUNE | S | Keep Translate to English as a plain feature if wanted (D-214). | – |
| B-208 | Slow-language guardrail | §13.10 | "Wispr works in 100+ languages" (obs) | For az/cy/he/sr/ta/ur the Best pick is whisper-large-v3 (15 s accel / 31 s CPU per 30 s audio); fa/mk get Qwen3-ASR-1.7B (7.3 s / 12.1 s), because `recommend()` ignores the budget when nothing in budget is usable (`recommendations.js:177-180`). V (computed) | EXTEND | S | Say "slower on this computer" up front or offer a lighter, less accurate model. | `isWeakLanguage` hint pattern |
| B-209 | Lower quantization for low disk/RAM | – | – | Only the default quant is downloadable; others recognized if on disk (`model.rs:179-183`); mirror holds defaults only (`catalog/mod.rs:384-388`); no non-Q8 benchmarks. V | EXTEND | M | Accuracy unmeasured. Low priority. | – |
| B-214 | Intel Mac and no-GPU policy | §8.1 | "Almost no decisions at the moment of use" (inf) | Intel builds CPU-only (`Cargo.toml:157-158`); picks ignore the tier (`catalog.ts:101`). V | EXTEND | S | Use the `fast` role on CPU; decide Intel support (min macOS 10.15, `tauri.conf.json:42`, opened). | `picksFor` + `useMachineTier` |

#### B4. Languages

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| B-61 | "My languages" (owner; C-82 folded) | §5, §6 | "My languages: Change"; Languages ▸ in Dictation and status menus (obs) | Picker on Welcome (`Welcome.tsx:35-47`); General "Languages" (`PreferredLanguages.tsx:20-139`); `spoken_languages` (`settings.rs:452-455`). V | HAVE | – | Menu entries: A-56, A-63. Remove the model card next to it (B-207). | – |
| B-62 | Languages preselected from the system | §8.1 | Language preselected (inf) | OS languages, else browser locale (`languageSuggestions.ts:20-28`); English always appended (`system_languages.rs:84-85`). V | HAVE | – | – | – |
| B-63 | Warn when a language is poorly supported | – (Handy only) | – | "no model hears it well yet" (`Welcome.tsx:60-69`, `catalog.ts:69-75`). V | HAVE | – | Honest; keep. | – |
| B-64 | Changing languages re-picks the model | – | – | Settings only write `spoken_languages` (`PreferredLanguages.tsx:35-40`). V | EXTEND | S | Re-pick and download in background. | B-47 pick |
| B-65 | Automatic detection among my languages | §13.10 | Implicit (inf) | Detector for models without their own detection (`language_detection/mod.rs:1-4`; `policy.rs:106-110`); 45 MB detector fetched with the model (`model.rs:1860-1890`). V | HAVE | – | Strength. | – |
| B-66 | Chinese script choice | – | – | `Welcome.tsx:126-165`; migrations `settings.rs:1246-1287`. V | HAVE | – | – | – |
| B-67 | App (UI) language (owner; C-83 folded) | §5 | "App Language (dropdown)" in General (obs) | Control in About (`AboutSettings.tsx:43`); default from OS locale (`settings.rs:700-704`); 26 locales (`i18n/languages.ts:18-49`). V | TUNE | S | Move to General. | – |
| B-68 | i18n infrastructure and RTL | – | – | i18next, RTL sync (`i18n/index.ts:90-128`). V | HAVE | – | Starts in English then switches (possible flash, I). | – |
| B-213 | Language-coverage claim | §13.10 | "Wispr works in 100+ languages" (obs) | 51 languages recommendable (`recommendations.js:233-247`). V | TUNE | S | Use an honest number in copy. | – |

### Area C: Hub window, data features, settings structure

Handy today (V): the main window is a settings app: 160 px sidebar with General, History, Models, Advanced, Post Process (only when enabled), Debug (debug mode), About (`Sidebar.tsx:34-77`, opened); opens on General (`App.tsx:87-88`, opened); 720×600 window (`lib.rs:970-971`, `utils.rs:26`). History keeps 5 rows by default (`settings.rs:676-682`, opened).

#### C1. Window chrome and navigation

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| C-01 | Task-grouped Hub sidebar | §4, §8.8 | Sidebar: Dictation, Notetaker, Insights, Dictionary, Snippets, Style, Transforms, Scratchpad; then Invite, Free month, Settings, Help (obs) | Grouped by technology (`Sidebar.tsx:34-77`). `SECTIONS_CONFIG` is a clean registry. V | EXTEND | M | Models/Advanced/Debug move under Settings. Each sidebar item's page is owned by its feature row (C-02, C-50, C-58, C-69, D-18, D-28, C-74, E-30). Nav items are `div`s (C-210). | – |
| C-02 | Dictation home as the landing page (owner; B-34 folded) | §4, §13.14 | App opens on Dictation home: greeting, banner, stats rail, history (obs) | History page exists; app opens on General (`App.tsx:87-88`). History page is a single `max-w-3xl` column (`HistorySettings.tsx:307-334`). V | EXTEND | M | Pass 2 S→M: the home is a new two-column layout, not the History settings page. Onboarding hand-off lands here. | – |
| C-03 | Notetaker sidebar item | §4 | – | – | XREF | – | See E-30. | – |
| C-04 | Insights sidebar item | §4 | – | – | XREF | – | See C-50. | – |
| C-05 | Dictionary sidebar item | §4 | – | – | XREF | – | See C-58 (today custom words sit in Advanced, `AdvancedSettings.tsx:52-58`). | – |
| C-06 | Snippets sidebar item | §4 | – | – | XREF | – | See C-69. | – |
| C-07 | Style sidebar item | §4 | – | – | XREF | – | See D-18. Post Process page needs two hidden switches today (Experimental and post-processing, `AdvancedSettings.tsx:32`, `:68-70`, opened). | – |
| C-08 | Transforms sidebar item | §4 | – | – | XREF | – | See D-28. | – |
| C-09 | Scratchpad sidebar item | §4 | – | – | XREF | – | See C-74. | – |
| C-10 | Setup checklist card | §4 | – | – | XREF | – | See B-44 (owner; C supplies the sidebar slot). | – |
| C-11 | Invite your team / Get a free month | §4, §10, §11 | Referral and team invites (obs); our product: "share with a friend" tab (user) | Absent. V | OUT | S (local) | Team/referral are account features. Local equivalent (user, R1 §10): copy a download link or share sheet, no tracking. | – |
| C-12 | Settings as a two-column modal | §4, §5 | Settings modal, nav left, content right (obs) | Settings are full pages (`App.tsx:50-75`); focus-trapped `Dialog` exists (`Dialog.tsx:14-60`). V | EXTEND | M | Decision DEC-10 (modal vs pages). Models page fills the window; needs a size rule. | `Dialog` |
| C-13 | Help item and popover (owner of Help and Feedback structure) | §4, §10 | Popover: Updates (What's New), Essentials (Shortcuts, Microphone, Languages), Get in touch (Help Center, Talk to support, Contact sales) (obs); our product: "Help and Feedback", contact and feedback only (user) | Absent. About has donate + source buttons (`AboutSettings.tsx:56-78`). V | BUILD | S | Entries: C-99, C-100, C-101, C-102, C-103; Contact sales C-215. | popover primitive |
| C-14 | Sidebar collapse button | §4 | Collapse next to window controls (obs) | Fixed `w-40` (`Sidebar.tsx:96`). V | BUILD | S | Icon-only mode; tooltips (`Tooltip.tsx`). | – |
| C-15 | "Beta" / "New!" tags | §4, §13.14 | Tags beside nav labels (obs) | `Tag.tsx`, `Badge.tsx` exist, unused in sidebar. V | TUNE | S | "New!" needs a seen flag. | `Tag` |
| C-16 | Top-right dark-mode toggle | §4 | One-click toggle in header (obs) | Theme dropdown in About (`AboutSettings.tsx:44`; `theme.ts:21-40`). V | TUNE | S | Needs a header bar (C-25). | `applyTheme` |
| C-17 | Notifications bell and panel | §4 | Bell opens panel with filter, "…", empty state "You're all caught up!" (obs) | Only sonner toasts (`App.tsx:350-365`); no notification plugin. V | BUILD | M | Local inbox (milestones, tips, What's New). Controls and empty state: C-201. System notifications: E-91, DEC-21. | SQLite table |
| C-18 | Profile menu | §4 | Name, email, referral, Manage account (obs) | Absent | OUT | – | Account. Greeting name: C-217. | – |
| C-19 | Warm light theme | §4, §11 | Warm off-white, calm (obs) | Neutral `#fbfbfb`, pink brand (`theme.css:12-15`, `:127`). V | TUNE | M | All tokens in one file; brand decision. | Radix Colors |
| C-20 | Dark theme | §4 | Dark theme exists; full theme list (unk) (obs) | Full dark palette, media query + `data-theme` override, also applies to the overlay (`theme.css:168-244`; `theme.ts:3-17`). V | HAVE | – | Retune with C-19. | – |
| C-21 | Serif display headings | §4, §11 | Serif on banners and headings (obs) | No serif; bundled woff2 `@font-face` pipeline exists (`App.css:32-37`, `:41-50`). V | TUNE | S | Bundle an OFL serif locally; never fetch fonts. | – |
| C-22 | Rounded cards, calm spacing | §4, §11 | Rounded cards (obs) | `SettingsGroup` card with uppercase label (`SettingsGroup.tsx:15-28`); onboarding type scale (`App.css:52-69`). V | TUNE | S | Promote onboarding tokens. | – |
| C-23 | Dismissible example banner component (owner; B-38 folded) | §4, §10, §11 | Each feature page has a dismissible banner with example chips (obs) | `Alert` has no dismiss/actions (`Alert.tsx:46-64`). V | BUILD | S | Conflict resolved: one component (S) with persisted dismissed ids; content per feature lives in C-29, C-61, C-73, D-206. | – |
| C-24 | Empty states with one clear action | §11 | Illustration + one action (obs) | History empty state is one line (`HistorySettings.tsx:275-279`). Mascot is the Handy glove (`Mascot.tsx:1-22`). V | TUNE | S | Art must be rebranded. All pages: C-206. | – |
| C-25 | Window chrome: integrated title bar, header, size | §4 | Sidebar under traffic lights; header with toggle, bell, profile (obs) | Standard title bar, 720×600 min = default (`lib.rs:970-971`; `utils.rs:26`), title "Handy". V | TUNE | M | Pass 2 S→M: overlay title bar, traffic-light inset, title-bar measurement (`lib.rs:988-998`), drag-region permission (unknown whether `core:default` grants it). | – |
| C-26 | Footer content | §5 | Version + Update in the Settings footer (obs) | Global footer: model selector, job status, update checker, version (`Footer.tsx:30-45`). V | TUNE | S | Remove model selector (B-207); keep job status (C-47). | – |
| C-27 | Version + Update button | §5 | – | – | XREF | – | See E-51. | – |
| C-201 | Notifications panel controls and empty state | §4 | Filter, "…" menu, "You're all caught up!" illustration (obs) | Absent. V | BUILD | S | On top of C-17. | menu primitive (C-41) |
| C-202 | Feature-announcement popup ("Dark mode is finally here") | §4 | Launch popup announcing a feature (obs) | `WhatsNewModal` once per version from bundled Markdown (`WhatsNewGate.tsx:14-71`). V | TUNE | S | Covers audit PARTIAL R1-§4-79. | `WhatsNewModal` |
| C-203 | Keep standard Edit/Window/Help menus | §6 | – | – | XREF | – | Folded into A-63 (custom menu replaces the Tauri default). | – |
| C-204 | Deep links inside the Hub | §10, §11, §12 | Help > Microphone/Languages, Invite → Settings > Team, checklist → Settings > MCP, tour → Connectors (obs) | Navigation is a `useState` section (`App.tsx:87-89`); no router; tray ⌘, only shows the window. V | EXTEND | S | `navigate(page, section?)` store + scroll-to-anchor. | zustand |
| C-205 | Page drop zones vs window-wide file drop | §11 | Import dialogs accept dropped .csv (obs) | `FileDropOverlay` sends every drop to transcription (`FileDropOverlay.tsx:19-29`; mounted `App.tsx:454`, opened). V | EXTEND | S | A .csv dropped on the import step would be transcribed. Route by active page/dialog. | – |
| C-206 | Empty states for every page | §4, §11, §14 | One action + example banner (obs) | Only History has one. V | BUILD | S | One `EmptyState` component. | – |
| C-210 | Sidebar keyboard and screen-reader access | §8.8 | Calm sidebar (inf) | Nav items are `div`s with `onClick` (`Sidebar.tsx:104-111`). V | TUNE | S | Use buttons / `aria-current`. Related: E-81. | – |
| C-215 | Help "Contact sales" | §10 | Under "Get in touch" (obs) | n/a | OUT | – | Sales channel; nothing local. | – |
| C-217 | Greeting name from macOS | §4 | "Welcome back, <name>" (obs) | No name in settings (`settings.rs:392-564`); `NSFullUserName` in `objc2-foundation` 0.3.2 default features. V | BUILD | S | First word of full name + override field. Feeds C-28. | objc2-foundation |
| C-218 | Selected-card lilac state | §12 | Lilac fill and border on picked cards (obs) | `--color-plum-pick` tokens exist (`theme.css:163-165`). V | HAVE | – | Reuse for tabs and cards. | – |

#### C2. Dictation home and history

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| C-28 | "Welcome back, <name>" header | §4 | Greeting with serif heading (obs) | Absent. V | BUILD | S | Name source: C-217. | – |
| C-29 | Dismissible promo banner on home (owner; B-33 folded) | §4, §11, §13.13 | "Try Flow in another app / Get started", "Working around other people? Show me how" (obs) | Absent. V | BUILD | S | Uses C-23. "Show me how" launches B-42. | C-23 |
| C-30 | Stats rail: total words | §4 | Lifetime word count (obs) | Absent; not derivable from 5-row history (`settings.rs:676-682`). V | BUILD | M | Needs `daily_stats` that survives retention (AF-07). CJK word rule: DEC-11. | `unicode-segmentation` (transitive) |
| C-31 | Stats rail: words per minute | §4 | WPM (obs) | `TranscriptionCompleteEvent.audio_seconds` computed and emitted (`actions.rs:45-64`, `:850-855`) but not stored and not tied to an entry id; with VAD it is speech time. V | BUILD | S | Store `audio_ms` at `save_entry` (`actions.rs:786-793`). Basis: DEC-11. | – |
| C-32 | Stats rail: day streak | §4 | Consecutive-day streak (obs) | Absent. V | BUILD | S | From `daily_stats`; local day at record time. | – |
| C-33 | "Your Voice Profile" progress bar | §4 | Unlocks after a word count (obs) | Absent. `ProgressLine.tsx:13-31` fits (not `ProgressBar.tsx`, a download widget). V | BUILD | S | Depends on C-30. | `ProgressLine` |
| C-34 | History rows with full transcript | §4 | One row per dictation, time, full text (obs) | Infinite scroll, live insert, fold to 4 lines (`HistorySettings.tsx:61-64`, `:105-187`). Shows `transcription_text` (pre-LLM) even when LLM text was pasted (`HistorySettings.tsx:524-534`, opened). V | TUNE | S | Show the final text. Scale: C-207. | – |
| C-35 | History grouped by date | §4 | Date headers, time per row (obs) | Full date+time per row; `formatDate`, `formatRelativeTime` exist (`dateFormat.ts:37-127`). V | EXTEND | S | Pass 2 BUILD→EXTEND. | `dateFormat.ts` |
| C-36 | History search | §4 | Search icon (obs) | Absent; cursor paging only (`history.rs:557-612`). Bundled SQLite 3.50.2 has FTS5 and `trigram` (`libsqlite3-sys-0.35.0/build.rs:132`). V | BUILD | S | Pass 2 M→S (FTS5 verified). Trigram needs ≥3 chars; `LIKE` fallback. | FTS5 |
| C-37 | Row actions on hover | §4 | Hover shows play, copy, flag, ⋮ (obs) | Icons always visible (`HistorySettings.tsx:440-492`). V | TUNE | S | Keep keyboard focus-within. | – |
| C-38 | Play audio per dictation | §4 | Play icon (obs) | `AudioPlayer` under every row, lazy URL, one at a time (`HistorySettings.tsx:211-230`; `AudioPlayer.tsx:12-40`). V | TUNE | S | Collapse to a play icon. | – |
| C-39 | Copy transcript | §4 | Copy icon (obs) | Copies `transcription_text` (`HistorySettings.tsx:288-294`, opened); tray copies post-processed (`tray.rs:611-616`, opened). V | TUNE | S | Copy the displayed final text; align A-27. | – |
| C-40 | Flag a dictation | §4 | Flag icon; purpose (unk) | Star "save" exempts from retention (`HistorySettings.tsx:452-466`; `history.rs:491`). V | EXTEND | S | Local meaning: DEC-18. | – |
| C-41 | ⋮ overflow menu per row | §4 | Undo AI edit, Retry transcript, Delete transcript, Extract audio (obs) | Flat icons; `Dropdown.tsx` is a value select (`Dropdown.tsx:11-24`); no menu library. V | BUILD | S | Menu primitive with keyboard support. | Radix DropdownMenu |
| C-42 | Undo AI edit (owner; D-07 folded) | §4 | Revert to the unedited transcript; "original dictation is never lost" (obs) | Both texts stored (`history.rs:33-35`, `:73-76`); no UI. "Raw" is already filler-stripped and dictionary-corrected (`transcription.rs:1643-1662`, opened). V | EXTEND | S | Add `shown_variant`; "exact transcript" also needs `asr_text` (D-06). Re-paste or history-only: DEC-19. | – |
| C-43 | Retry transcript (owner; D-08 folded) | §4 | Re-run transcription (obs) | Background job with current model, re-runs LLM if requested (`commands/history.rs:64-79`; `transcription_jobs.rs:282-326`). V | HAVE | – | Overwrites old text (`history.rs:384-410`): add `previous_text`. Re-applying a different cleanup level is a small EXTEND. | – |
| C-44 | Delete transcript | §4 | Delete from ⋮ menu (obs) | Removes row and WAV, cancels job (`commands/history.rs:49-62`; `history.rs:691-720`); no confirm or undo. V | EXTEND | S | Pass 2 TUNE→EXTEND: undo needs soft delete or delayed purge. | sonner action |
| C-45 | Extract audio | §4 | Export the recording (obs) | Only "Open Recordings Folder"; WAV `handy-<ts>.wav` (`actions.rs:701`). V | EXTEND | S | "Save audio as…" via `tauri-plugin-dialog`. | dialog plugin |
| C-46 | History and audio retention (owner; E-69 folded) | §4 | Raw audio kept per dictation (inf, from "Extract audio"); default duration (unk) | `history_limit` 5, `PreserveLimit` (`settings.rs:676-682`, opened); cleanup deletes row and WAV together (`history.rs:455-483`, opened); runs only on save and setting change. Frontend fallback disagrees (`RecordingRetentionPeriod.tsx:19` "never"). V | EXTEND | M | Conflict resolved: E said TUNE S (default only); splitting text and audio retention needs new cleanup logic and UI for rows without audio. WAV = 1.92 MB per speech-minute (`audio/utils.rs:31-45`). Defaults: DEC-07. | – |
| C-47 | Background job status | – (Handy only) | – | Footer "Transcribing, N%" (`TranscriptionStatus.tsx:10-34`). V | HAVE | – | Keep on home. | – |
| C-48 | Transcribe a file (picker, drag-and-drop) | – (Handy only) | – | `HistorySettings.tsx:247-254`; `FileDropOverlay.tsx`; `commands/history.rs:81-94`. Opus/ogg/webm not decodable (`transcribeFiles.ts:6-8`). V | HAVE | – | Exclude from stats (C-212). | – |
| C-49 | "Show me how" guided flows from home | §10-§12 | – | – | XREF | – | Entry banner C-29; flow B-42; engine B-40. | – |
| C-207 | History scale with long retention | §4 | Long, smooth history (obs) | Every loaded row mounts an `AudioPlayer` (`HistorySettings.tsx:284-298`, `:548`). V/I | EXTEND | S | Virtualize, or mount the player on demand. | `@tanstack/react-virtual` |
| C-208 | SQLite connection settings for more writers | (enabler) | – | New `Connection::open` per call, no WAL, no busy timeout (`history.rs:254-256`, opened). V | EXTEND | S | Needed before stats, notes autosave, FTS triggers. | rusqlite pragmas |
| C-209 | Typed added columns | (enabler) | – | `ADDED_COLUMNS` adds only TEXT columns (`history.rs:40`, `:176-184`, opened). V | EXTEND | S | Integer columns would get TEXT affinity. Keep "newer DB opens in older build". | – |
| C-211 | Failed dictation kept with audio and retry | §1.4, §8.3 | Text kept, retry available (obs) | Empty-text row saved with WAV (`actions.rs:885-897`), "Transcription failed" label, retry works. V | HAVE | – | Exclude from stats and search. | – |
| C-212 | Stats counting rules | §4, §5 | Totals are consistent (inf) | Precedent: files excluded from last-transcript and retention (`history.rs:491`, `:638`). V | EXTEND | S | Count non-empty dictations once; retries and deletes rules. | – |
| C-213 | Live refresh of stats rail and menu counts | §4, §6 | Counters update after each dictation (inf) | `HistoryUpdatePayload` events exist (`history.rs:53-64`); no stats event; tray rebuilds only on change (`tray.rs:406-409`). V | EXTEND | S | Emit `StatsUpdated`. | tauri-specta event |

#### C3. Insights

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| C-50 | Insights page, tabs Your usage / Your voice | §4 | Two tabs (obs) | Absent. No chart library (`package.json`). V | BUILD | M | Insights in total (C-30–C-33, C-50–C-57) is L. | Recharts/visx |
| C-51 | WPM with percentile | §4 | WPM and percentile (obs) | Absent. V | BUILD | S | Percentile needs population data: OUT. | – |
| C-52 | Fixes made by Flow (owner; D-57 folded) | §4, §8.4 | Words corrected, dictionary fixes (obs) | `apply_custom_words` and filler removal return only `String` (`text.rs:151`; `transcription.rs:1653-1658`). V | EXTEND | M | Conflict resolved (C BUILD M, D EXTEND S): stages return counts (S part), plus LLM word diff, per-entry columns and a `corrections` log for "most corrected word" (M). | `similar` crate |
| C-53 | Total words desktop vs mobile | §4 | Split by device (obs) | n/a | OUT | – | Single device. Platform note: E-220. | – |
| C-54 | Usage by app category (owner of storage, map, chart; D-62 folded) | §4 | Documents, other tasks, AI prompts, emails, work/personal messages (obs) | No frontmost-app capture. Capture is D-21. V | BUILD | M | `app_bundle_id`, `app_name` per row + `daily_app_stats`; bundled bundle-id→category map. Web apps in browsers: D-22. | – |
| C-55 | Streak calendar | §4 | Calendar heatmap (obs) | Absent. V | BUILD | S | From `daily_stats`. | react-calendar-heatmap |
| C-56 | Share insights | §4 | Share button (obs) | Absent. V | BUILD | S | Social share is OUT; local "save as image" only. | html-to-image |
| C-57 | Voice Profile cards (owner; D-63 folded) | §4 | Locked until ~1.7K words: Catchphrase, Peak time and place, Most used word, Most corrected word (obs) | Absent. V | BUILD | M | "Place" needs location: replace with app. Needs C-52 log. | stopwords-iso |

#### C4. Dictionary

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| C-58 | Dictionary page and store (owner) | §4 | Entries are single words or "from → to" (obs) | `custom_words: Vec<String>` (`settings.rs:463`) as chips with inline add, 50-char cap, strips `<>"'` (`CustomWords.tsx:14-41`, `:57-122`). V | EXTEND | M | Move to its own page and a richer store (C-68). Apostrophe stripping breaks "O'Brien". | – |
| C-59 | Tabs All / Personal / Shared with team (D-51 folded) | §4 | Three tabs (obs) | n/a | OUT | S (local) | "Shared with team" is a team feature. Local tabs could be All / Added by you / Suggested. | – |
| C-60 | Search, sort, refresh | §4 | Toolbar controls (obs) | Absent. V | BUILD | S | Client-side. | – |
| C-61 | "Flow spells the way you do" banner | §4 | Dismissible banner with example chips (obs) | Absent. V | BUILD | S | Uses C-23. | – |
| C-62 | "Add to vocabulary" dialog | §11 | Dialog: "Add a new word", toggles "Correct a misspelling" and "Share with team", Add disabled until filled (obs) | Inline input + Add (`CustomWords.tsx:65-88`). V | TUNE | S | Misspelling pair: D-48. Share toggle OUT. | `Dialog` |
| C-63 | Correct a misspelling (from → to) | §4, §11 | – | – | XREF | – | See D-48 (owner, pipeline + dialog fields). | – |
| C-64 | Import CSV wizard (owner; D-50 folded) | §11 | 4-step wizard, .csv up to 3 MB, browse or drag and drop (obs; later steps unk) | Absent. V | BUILD | S | Preview + dedupe. Drop routing: C-205. | papaparse |
| C-65 | Export dictionary | (implied) | – | Absent. V | BUILD | S | Handy-side proposal; pairs with E-66. | papaparse |
| C-66 | Auto-added words | §5 Plans | – | – | XREF | – | See D-52. Pass-1 "marked with a sparkle" removed: not in R1. | – |
| C-67 | Vocabulary used by the recognizer | §5 Plans ("learns names and jargon") | Learns names and jargon (obs plan text) | Words passed as decoder vocabulary when supported, else fuzzy correction (`transcription.rs:1556-1562`, `:1643-1651`). V | HAVE | – | Replacement rules must not be fed as vocabulary. | – |
| C-68 | Dictionary entry metadata | §4 | R1 shows only words and pairs, plus tabs (obs) | `Vec<String>` only; setter writes whole list (`shortcut/mod.rs:893-900`). V | EXTEND | S | Handy-side proposal (not Wispr): id, kind, from, to, source, created_at, hit_count; migrate the old list. | – |

#### C5. Snippets

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| C-69 | Snippets page (list, tabs, search/sort/refresh, Add new) | §4 | Same layout as Dictionary: tabs, search, sort, refresh, Import, Add new, banner (obs) | Absent. V | BUILD | M | Covers audit PARTIAL R1-§4-50. New `snippets` table. Expansion is D-53. | – |
| C-70 | Add snippet dialog (owner; D-54 folded) | §11 | Share toggle, trigger field, rich-text body (bold, italic, lists, link), 4,000-char counter (obs) | Absent. Plugin has `write_html` (`tauri-plugin-clipboard-manager-2.3.2/src/desktop.rs:72-87`) but both paste paths write plain text (`clipboard.rs:50-61`; `paste_tx/macos.rs:22`, opened). V | BUILD | S plain / L rich | Rich needs editor, sanitizing, HTML+plain on both paste paths, and the restore keeps only text/image. DEC-12. | Tiptap/Lexical |
| C-71 | Snippet import (owner; D-55 folded) | §11 | Import button (obs) | Absent. V | BUILD | S | Same wizard as C-64. Team share OUT. | – |
| C-72 | Snippet expansion | §4 | – | – | XREF | – | See D-53. | – |
| C-73 | Snippets example banner | §4 | Three examples (obs) | Absent. V | BUILD | S | Uses C-23. | – |

#### C6. Scratchpad (R1: Beta)

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| C-74 | Scratchpad page | §4, §10 | "Add to Flow Bar" toggle, shortcut button, "Start new note", Recents with title/preview/date and hover edit/pin/delete (obs) | Absent. V | BUILD | M | Scope note: Beta in R1. `notes` table. Bar slot: A-03. | – |
| C-75 | Floating note window | §10 | Small window: tabs "Untitled ×", "+", expand and close buttons, left rail (sidebar toggle, new note, search, AI sparkle, text size), text area, Copy (obs) | Absent. Overlay precedent is an NSPanel via `tauri-nspanel` (`overlay.rs:18`, `:454`); capabilities are scoped to `main` and `recording_overlay` (`capabilities/default.json:5`). V | BUILD | L | Scope note: Beta. New window label must be added to capabilities (AF-16). Covers audit PARTIALs R1-§10-33/34. AI sparkle: D-209. Notes search: C-216. | tauri-nspanel |
| C-76 | Dictation into the focused note | §10 | Dictating types into the note (inf) | Paste into a Handy webview textarea already works (onboarding `TryIt.tsx:600`; `useTryEngine.ts:77-83`). V | HAVE | – | Pass 2 EXTEND→HAVE. Direct insertion without clipboard is optional. | – |
| C-77 | Open Scratchpad shortcut (owner of the action) | §3, §11 | "Not set" in the hotkey panel; Scratchpad dialog proposes ⌥S (obs) | Actions: transcribe, transcribe_with_post_process, cancel, test (`actions.rs:965-986`). V | BUILD | S | Conflict resolved: default stays unset (needs A-32); dialog proposes ⌥S. Plumbing A-30. | `ShortcutInput` |
| C-78 | Scratchpad open behavior | §5 | Resume last note / Customize (obs) | Absent. V | BUILD | S | One enum setting. | – |
| C-216 | Notes search | §10 | Search in the note rail (obs) | No notes store. V | BUILD | S | FTS5 on `notes`. | FTS5 |

#### C7. Settings structure

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| C-79 | Settings grouped by task (General, System, Data and Privacy) | §5, §8.8 | Few pages in plain words (obs) | General / Advanced / About / Debug (`GeneralSettings.tsx:28-54`; `AdvancedSettings.tsx:35-80`; `AboutSettings.tsx:41-96`). V | EXTEND | M | Regroup existing controls; hide technical ones in a collapsed Advanced. Many user strings live under `settings.debug.*` keys. | – |
| C-80 | Shortcuts row ("Hold fn and speak", Change) | §5 | Summary + Change opens the hotkey panel (obs) | `ShortcutInput` rows + activation mode (`GeneralSettings.tsx:30-33`). V | EXTEND | S | Row summary with `Keycaps`; panel is A-33. | `Keycaps` |
| C-81 | Microphone row | §5 | – | – | XREF | – | See A-37, A-38. | – |
| C-82 | My languages | §5 | – | – | XREF | – | See B-61. | – |
| C-83 | App language | §5 | – | – | XREF | – | See B-67. | – |
| C-84 | Theme | §5 | Dropdown (Light seen) (obs) | System/Light/Dark in About (`AboutSettings.tsx:44`); default System. V | TUNE | S | Move to General. | – |
| C-85 | Launch at login | §5 | – | – | XREF | – | See A-64. | – |
| C-86 | Show Flow Bar at all times | §5 | – | – | XREF | – | See A-66. | – |
| C-87 | Show app in Dock | §5 | – | – | XREF | – | See A-65. | – |
| C-88 | Show menu-bar (tray) icon | §6 | Status icon exists (obs) | `ShowTrayIcon`, default on (`settings.rs:706-708`). V | HAVE | – | – | – |
| C-89 | Dictation and notification sounds | §5 | – | – | XREF | – | See A-42. | – |
| C-90 | Mute all audio while dictating | §5 | – | – | XREF | – | See A-41. | – |
| C-91 | Notifications toggles: Suggestions, Announcements, Milestones | §5 | Three toggles, seen on in a used account (obs) | Absent. V | BUILD | S | Only with C-17. Announcements = local notes only. | – |
| C-92 | Start hidden | – (Handy only) | – | `StartHidden` (`AdvancedSettings.tsx:37`). V | HAVE | – | – | – |
| C-93 | Data and Privacy page | §5, §10 | – | – | XREF | – | See E-68 (owner). Existing pieces: data/log folders (`AboutSettings.tsx:80-81`), retention, recordings folder. | – |
| C-94 | Download / delete your data | §5 | – | – | XREF | – | See E-66, E-67. | – |
| C-95 | Context awareness toggle | §5 | – | – | XREF | – | See D-58. | – |
| C-96 | Account, Team, Plans and Billing | §5 | Account page, team invites, plans Free/Pro/Growth (obs) | n/a | OUT | – | Account-only. About keeps version, licence, source. | – |
| C-97 | Connectors (Calendar, Slack) and MCP settings | §5 | Connect Google/Outlook Calendar, Slack; MCP "Add to Claude/ChatGPT/Gemini/Cursor"; MCP has no access to dictations (obs) | n/a | OUT | – | Account/cloud connectors. Local calendar: E-40/E-42; local notes for AI tools: E-41. Notetaker settings: E-201. Vibe coding: D-59, D-60. | – |
| C-98 | Technical and debug settings | – (Handy only) | Not exposed (obs) | Advanced exposes paste method, typing tool, VAD, acceleration, experimental (`AdvancedSettings.tsx:36-80`); Debug via ⌘⇧D (`App.tsx:129-152`). V | TUNE | S | Keep out of the main path. | – |
| C-219 | Data toggle "Show your public information" | §5 | Toggle, on in the observed account (obs) | n/a | OUT | – | Audit MISSING item. Account/profile feature; nothing to show locally. The privacy page states there is no public profile. | – |
| C-220 | "Sync notes" and cross-device sync | §5, §7 | Sync notes toggle; Free plan lists Mac/Windows/iOS/Android (obs) | n/a | OUT | – | Audit MISSING item. Cloud sync. Privacy page should say nothing syncs. | – |
| C-221 | "Dictation cloud storage" toggle | §5 | Off in the observed account (obs) | n/a | OUT | – | Audit PARTIAL item. Audio and text stay local (C-46). | – |
| C-214 | Other cloud data toggles ("Improve the model for everyone", "Store data locally" dropdown) | §5 | Toggles and a dropdown (obs) | n/a | OUT | – | No telemetry (E-73); D-66 folded. The local page states facts (E-68). | – |

#### C8. Help and menus

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| C-99 | What's New (owner; E-89 folded) | §10 | Opens a web page (obs) | Local modal, shown once for the newest unseen note (`WhatsNewGate.tsx:28-43`, `:61-67`); only `0.9.0.md`, `0.9.7.md`; no reopen path. V | TUNE | S | Pass 2 HAVE→TUNE. Add a Help entry; replace content with ours. | `WhatsNewModal` |
| C-100 | Help > Shortcuts | §6, §10 | Opens the hotkey panel (obs) | Absent. V | BUILD | S | Opens A-33. | – |
| C-101 | Help > Microphone, Languages | §10 | Quick access (obs) | Absent. V | EXTEND | S | Needs C-204 deep links. | – |
| C-102 | Help Center (hosted) | §6, §10 | Web help center (obs) | Only an upstream "Source code" link (`AboutSettings.tsx:66-78`). V | OUT | S (local) | Hosted help is OUT; local bundled help pages are an S option. | `MarkdownContent` |
| C-103 | Talk to support / Share feedback (owner of local feedback; E-90 folded) | §6, §10, §11 | Support chat (⌘/), Share feedback (obs); our product keeps contact + feedback (user) | Absent; log dir shown (`AboutSettings.tsx:81`). V | BUILD | S | Prefilled GitHub issue or mailto + "copy diagnostics" (no transcripts). | – |
| C-104 | About / support development | – | – | Donate `handy.computer/donate`, upstream repo (`AboutSettings.tsx:32-38`, `:74`). V | TUNE | S | Rebrand; credit Handy (MIT). | – |
| C-105 | "My Voice" app menu | §6 | – | – | XREF | – | See A-63 (counts from C-58, C-69). | – |
| C-106 | "Dictation" app menu | §6 | – | – | XREF | – | See A-63. | – |

### Area D: text intelligence

Handy's text path today (V): ASR → `post_process_transcription_text` (`transcription.rs:1600-1662`, opened): Chinese script conversion → fuzzy custom-word correction (skipped when words were given to the model as vocabulary) → filler removal → `normalize_transcription_output` (stutter collapse, whitespace collapse, trim). Then an optional LLM stage only on the second binding `transcribe_with_post_process` (`actions.rs:965-986`, opened). Then paste.

#### D1. Auto cleanup (R1: "Auto cleanup (Beta)")

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| D-01 | Cleanup on every dictation, same hotkey | §4 Style | Chosen level applies to every dictation in every app (obs) | LLM only on the second binding (`actions.rs:965-976`); main `transcribe` has `post_process: false` (opened). Second-binding id is hard-coded in ~9 places (`shortcut/mod.rs:309-311`, `:501`; `handy_keys.rs:437`; `transcription_coordinator.rs:572`; `signal_handle.rs:50`; `lib.rs:880`…). V | EXTEND | S | Scope note: Beta in R1. Wiring is S but blocked by D-67/D-212 (no usable default provider: `openai`, empty model, no prompt, `settings.rs:710-712`, `:1035`). Second binding: A-209. | – |
| D-02 | Level "None" = exact transcript | §4 | Exact transcript including mistakes (obs) | Filler removal on by default (`settings.rs:639-641`); stutter collapse and whitespace normalize always run (`text.rs:468-480`, opened). V | TUNE | S | None = filler removal off; keep trim. "Exact" also needs D-06. | – |
| D-03 | Level "Light" = fillers + grammar | §4 | Filler words and grammar (obs) | Deterministic fillers (`text.rs:315-324`, `:429-464`); grammar only via the LLM prompt "Improve Transcriptions" (`settings.rs:840-846`). V | EXTEND | M | Fixed prompt tuned for a small model + output guards (D-12). | – |
| D-04 | Level "Medium" = clarity and conciseness | §4 | Clarity and conciseness (obs) | No built-in prompt; user-written only (`PostProcessingSettings.tsx:220-236`). V | EXTEND | M | Meaning-drift risk with 1-3B models; needs an eval set. Overlap with D-42: DEC-13. | – |
| D-05 | Three-level control instead of prompts | §4, §8.6 | Segmented None/Light/Medium (obs) | Provider/URL/key/model/prompt CRUD UI (`PostProcessingSettings.tsx:25-453`); no level setting. V | BUILD | S | `cleanup_level` selects a built-in prompt; prompt editor moves to Advanced. | – |
| D-06 | Original never lost (verbatim ASR text) | §4 | "Original dictation is never lost" (obs) | Both texts stored (`history.rs:33-35`), but "raw" is post-filler and post-dictionary (`transcription.rs:1643-1662`, opened); default retention deletes rows (`settings.rs:676-682`). V | EXTEND | S | Pass 2 HAVE→TUNE; set to EXTEND here because it needs a new `asr_text` column and returning the pre-fix text from `transcription.rs:1600` (merge note M-07). Retention: C-46. UI: C-42. | – |
| D-07 | Undo AI edit | §4 | – | – | XREF | – | See C-42. | – |
| D-08 | Retry transcript | §4 | – | – | XREF | – | See C-43. | – |
| D-09 | Soft failure: cleanup failure keeps the text | §1.4, §8.3 | Text kept and copyable (obs) | LLM failure/empty → raw text pasted (`actions.rs:353-356`, `:389-400`); no notice. V | TUNE | S | Subtle pill hint "Pasted without cleanup" (A-14). | – |
| D-10 | Pill state while cleaning | §2 | – | – | XREF | – | See A-11 (copy "Polishing…"). | – |
| D-11 | Cleanup latency budget and cancel | §8.1 | Not measured (unk) | Cancel honored while awaiting HTTP (`actions.rs:154-172`); no HTTP timeout (`llm_client.rs:219-225`, opened: client built without timeout). Structured failure falls back to a second legacy request, each retried once without reasoning fields: up to 4 calls (`actions.rs:357-401`; `llm_client.rs:453-487`). Apple call is synchronous FFI inside an async fn (`actions.rs:268`), so `tokio::time::timeout` cannot pre-empt it. V/I | EXTEND | S | Budget must wrap the whole chain; Apple path needs `spawn_blocking` (AF-14). | – |
| D-12 | Cleanup must not answer or follow dictated instructions | §4 (implied) | (unk) | Prompt-level guards only (`settings.rs:844`); `<think>` stripping (`actions.rs:130-137`); structured parse failure pastes raw model content (`actions.rs:324-351`). V | EXTEND | M | Guards: length ratio, token overlap, assistant-phrase detection, fallback to deterministic text. | GBNF / JSON schema |
| D-13 | Cleanup keeps the spoken language | §4 (English-only style note) | Style is English-only; cleanup languages (unk) | Prompt rule "keep the language" (`settings.rs:844`); language known at call site but not passed (`actions.rs:747-751` vs signature `:410-414`). V | TUNE | S | Pass language explicitly. Text language ID exists (`audio_toolkit/lang_id.rs:57`). | – |
| D-14 | Number words → digits, spoken punctuation (ITN) | §13.8 | "Flow Dictation formats for you and fixes mistakes" (obs copy); specific ITN (inf) | Only via LLM prompt rules (`settings.rs:844`). V | EXTEND | M (en) / L (multi) | Deterministic ITN gives format quality without an LLM. | NeMo ITN grammars (reference) |
| D-15 | Stutter removal | §13.8 | "fixes mistakes" (inf) | Collapses 3+ repeats (`text.rs:330-365`, opened). V | HAVE | – | Two-word repeats survive. | – |
| D-16 | Self-corrections ("no wait, I mean Tuesday") | §13.8 | Specific behavior (unk) | Not handled. V | EXTEND | M | Light/Medium prompt; meaning-change risk. | – |
| D-17 | Custom filler list | – (Handy only) | – | `custom_filler_words` (`settings.rs:526`, `text.rs:440-444`); only the toggle is in UI. V | HAVE | – | Keep hidden. | – |
| D-208 | Cleanup on from the first dictation | §13.8, §8.1 | Practice dictation comes out formatted (obs copy) | Off by default (`settings.rs:696-698`, opened); no prompt selected (`:1035`). V | BUILD | S | Deterministic-only until the LLM is ready, then switch. Depends on D-67. | B-50 |
| D-210 | Keep newlines / paragraphs through the deterministic stage | §13.8, §11 | Formatted multi-line output (inf); multi-line snippet bodies (obs) | `collapse_stutters` splits on whitespace and joins with one space (`text.rs:330-365`, opened); `\s{2,}` → " " (`text.rs:326`, `:474-476`); `apply_custom_words` joins with " " (`text.rs:219`, opened). Every newline is lost. V | EXTEND | S | Blocks voice "new line" (D-56) and snippets applied before normalization. Direct typing turns "\n" into Return (sends chat messages). AF-06. | – |

#### D2. Per-app Styles

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| D-18 | Style page with app groups (owner; C-07 folded) | §4 | Tabs Personal messages / Work messages / Email / Other / Auto cleanup (Beta) (obs) | No app grouping or style concept (`settings.rs:440-560`). V | BUILD | M | UI + settings + prompt composition. | – |
| D-19 | Default app membership | §4 | WhatsApp, Telegram, Discord, Instagram… / Slack, Teams, LinkedIn… / Gmail, Outlook, Mail… / all other apps (obs) | Nothing. V | BUILD | S | Bundle-id table (+ domain table for web apps). | NSWorkspace icons |
| D-20 | "+" to add apps | §4, §11 | "+" shown; did not respond to a click (unk) | Nothing. V | BUILD | S | App picker. | – |
| D-21 | Frontmost app detection at dictation time (owner of capture) | §4 (required by styles), Insights | Implied by per-app styles and usage categories (inf) | No frontmost-app code. `objc2-app-kit 0.3.2` (`src-tauri/Cargo.toml:166`, opened) default features include `NSWorkspace`, `NSRunningApplication` (`frontmostApplication`, `bundleIdentifier`). V | BUILD | S | Capture at record start. No new crate on macOS. Storage: C-54. | objc2 NSWorkspace |
| D-22 | Web-app / site detection (Gmail in a browser) | §4 | Whether Wispr reads browser URLs (unk) | Nothing; no AX client (AF-05). V | BUILD | L | Pass 2 M→L: AX `AXURL`/`AXDocument` varies by browser; AppleScript needs an Automation prompt per browser and an entitlement; Firefox has no URL scripting. DEC-23. | ActivityWatch watchers |
| D-23 | Personal presets Formal / Casual / Very casual | §4 | Before/after cards: caps+punct / caps, less punct / no caps, less punct (obs) | Nothing. V | BUILD | S | Casual/Very casual are mostly deterministic. | – |
| D-24 | Work/Email/Other presets Formal / Casual / Excited! | §4 | "Excited!" adds exclamations (obs) | Nothing. V | BUILD | M | Need the LLM. | – |
| D-25 | Before/after example cards | §4, §8.5 | Visual examples per preset (obs) | Nothing. V | BUILD | S | Static, translatable. | – |
| D-26 | "Style formatting applies only in English" gate | §4 | Note on the page (obs) | Output-language evidence exists (`text.rs:268-301`). V | BUILD | S | Gate styles on English; cleanup for all. | – |
| D-27 | Compose style with cleanup level | §4 | Order (unk) | One prompt per call (`actions.rs:206-227`). V | BUILD | S | One composed prompt per dictation (level + style + dictionary terms). | – |
| D-207 | Edit app-group membership (remove, move, override) | §4 | Add visible; remove/move (unk) | Nothing. | BUILD | S | Rule for an app in two groups. | – |

#### D3. Transforms (R1: Beta)

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| D-28 | Transforms opt-in toggle (owner page; C-08 folded) | §4 | Off until enabled (obs) | Nothing comparable. V | BUILD | S | Scope note: Beta. | – |
| D-29 | Read the selected text in any app | §10 | Highlight text, press hotkey (obs) | No AX client in `Cargo.lock` (`macos-accessibility-client` only via the permissions plugin for the trust check); clipboard restore is text-or-image only (`clipboard.rs:78-92`, opened). V | BUILD | L | Pass 2 M→L: new AX layer (AF-05); Electron/Chromium need `AXManualAccessibility`; ⌘C fallback needs a changeCount wait (`paste_tx/macos.rs:158`) and a full multi-type snapshot (A-47). RC-04. | `get-selected-text` crate |
| D-30 | Replace the selection in place | §10 | Transform replaces the selected text (obs) | Paste over a selection replaces it in most apps (I). | EXTEND | M | Pass 2 S→M: selection must survive LLM latency; focus/app guard (D-21); Linux default Direct typing gives many undo steps. | – |
| D-31 | ⌘Z undoes the transform | §10 | "You can always ⌘Z to undo Transform" (obs hint) | ⌘V is one undo step in most apps (I); Direct typing may create many (`input.rs:268`). | TUNE | S | Force clipboard paste for transforms. | – |
| D-32 | Per-transform hotkeys (⌥1, ⌥2) (owner) | §3, §10 | Polish ⌥1, Prompt Engineer ⌥2, configured in the Transform tab (obs) | Static `ACTION_MAP` (`actions.rs:965-986`). V | EXTEND | M | Dynamic action lookup. ⌥1 types "¡" on US layouts; labels on other layouts (A-203). Plumbing A-30. | handy-keys |
| D-33 | Polish rule toggles | §10 | Make more concise, Reword for clarity, Reorder for readability, Add structure for readability, Maintain your tone (obs) | Free-text prompts `LLMPrompt {id,name,prompt}` (`settings.rs:91-95`). V | BUILD | S | Prompt builder from rule fragments. | – |
| D-34 | Customize the transform prompt | §10 | "Customize your Polish prompt" (obs) | Prompt CRUD commands and editor (`PostProcessingSettings.tsx:184-453`). V | EXTEND | S | Reuse the editor. | – |
| D-35 | Writing examples (upload) to learn the style (D-65 folded) | §10 | "Add writing examples", Upload Example, "help Flow learn your unique style" (obs) | Nothing. V | BUILD | M | Few-shot examples cost context; could derive from own history. | – |
| D-36 | Autosave and Reset per transform | §10 | Autosave On, Reset (obs) | Explicit Update; no reset; last prompt cannot be deleted (`PostProcessingSettings.tsx:364-380`). V | TUNE | S | – | – |
| D-37 | Prompt Engineer transform | §4, §10 | "Constructs optimal prompts" (obs) | Nothing. V | BUILD | S | Prompt only, once D-29/D-30 exist. | – |
| D-38 | Create your own / Create New / Reset to defaults | §4 | List management (obs) | Prompt CRUD exists. V | EXTEND | S | – | – |
| D-39 | Pill "Done. See changes" | §10 | – | – | XREF | – | See A-14. | – |
| D-40 | Diff view (⌥O), "N Changes" | §3, §10 | Diff card in the video: "8 Changes", strike/highlight (obs in video); diff window itself (unk) | Nothing. V | BUILD | M | Word-level diff in React. Needs D-201 record. | jsdiff, `similar` |
| D-41 | Diff actions: Configure, copy, thumbs, retry | §10 | Shown in the video (obs) | Nothing. V | BUILD | S | Thumbs stay local. Retry at temperature 0 returns the same text. | – |
| D-42 | Auto Apply After Dictation | §4 | Toggle with a chosen transform (Polish) (obs) | Equivalent to post-processing with a selected prompt (`actions.rs:410-442`). V | EXTEND | S | Same as Medium cleanup? DEC-13. | – |
| D-43 | Transform tutorial content | §10 | How-it-works video; two-step practice in a sample composer; "Transformed!" with ⌥O/⌘Z hints; final "Turn on Transforms" (obs) | Practice engine exists in onboarding (`TryIt.tsx`). V | EXTEND | M | Content on the B-40 engine; local animation, no hosted video. | – |
| D-201 | Transform record (last transform: source, result, app, time) | §3 (⌥O), §10 | ⌥O shows the diff of the last Transform (obs) | History stores only dictations keyed by WAV (`history.rs:22-40`). V | BUILD | M | Separate table or `kind` column; selection text is sensitive; keep bounded. | – |
| D-202 | Transform hotkey with nothing selected | §10 | (unk) | Nothing. | BUILD | S | "Select text first" pill message (A-14) or apply to last dictation. | – |
| D-203 | Cancel and focus guard during a Transform | §3 | Esc dismisses (obs); transform-specific (unk) | Cancel exists for dictation (`actions.rs:154-172`); Apple path not cancellable. V | EXTEND | S | Abort paste if frontmost app changed (D-21). | – |
| D-204 | Long selections vs model context | §10 | (unk) | No input-size check (`actions.rs:178-402`). V | BUILD | S | Chunk or refuse with a pill message. | – |
| D-205 | Polish detail modal left side | §10 | Name, "⌥Opt 1 to use", description, transformed example with "See Updates" (obs) | Prompt editor only. V | BUILD | S | Covers audit PARTIAL R1-§10-22. | – |
| D-206 | Transforms page banner and practice cards | §4, §10 | "Transform works anywhere you write" (Try it out / How it works); cards Original / Polish / Prompt Engineer with a benefits ticker (obs) | Nothing. | BUILD | S | Uses C-23 and B-40. Covers audit PARTIAL R1-§10-17. | – |

#### D4. Command Mode

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| D-44 | Command Mode: select text, speak an instruction | §3 | "Select text and ask Flow or Perplexity" (obs label); behavior not opened (unk) | Nothing; needs D-29, LLM, D-30. V | BUILD | L | Where answers go with no selection; small-model instruction quality. Chord: A-29. | – |
| D-45 | Command Mode without a selection | §3 | (unk) | Nothing. | BUILD | M | Limit to writing tasks; factual answers hallucinate. | – |
| D-46 | Perplexity backend | §3 | Label "or Perplexity" (obs) | n/a | OUT | – | Cloud web search. | – |

#### D5. Dictionary, snippets, voice commands

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| D-47 | Dictionary words improve spelling | §4, §11 | Single-word entries (obs) | Decoder vocabulary when supported, else fuzzy Levenshtein + Soundex + n-grams, threshold 0.18 (`text.rs:151-220`; `settings.rs:660-662`); threshold UI in Debug (`WordCorrectionThreshold.tsx:18-25`); fuzzy fallback is ASCII-only (`text.rs:58-60`). V | TUNE | S | Strong engine; page is C-58. | – |
| D-48 | Misspelling → correct replacements (owner; C-63 folded) | §4, §11 | "Correct a misspelling" toggle reveals Misspelling → Correct spelling fields (obs) | No pairs; `custom_words` is `Vec<String>` (`settings.rs:463`). Pipeline order today: script → fuzzy custom words → fillers → normalize (`transcription.rs:1633-1662`, opened). V | EXTEND | M | Conflict resolved (C BUILD M before fuzzy; D EXTEND S after fuzzy): EXTEND M, and run exact rules **after script conversion and before fuzzy matching**, with rule outputs protected from fuzzy rewriting (merge log M-03). Type change + settings migration + dialog fields (C-62) + case/word-boundary/CJK rules. | aho-corasick |
| D-49 | Dictionary steers the LLM cleanup | §5 ("spell names correctly", context awareness) | (inf) | `custom_words` never reach the prompt (`actions.rs:178-402`). V | EXTEND | S | Inject terms, or the LLM may undo spellings. | – |
| D-50 | Dictionary Import | §11 | – | – | XREF | – | See C-64. | – |
| D-51 | Dictionary Personal / Shared with team | §4, §11 | – | – | XREF | – | See C-59 (OUT team). | – |
| D-52 | Learns names and jargon automatically (owner; C-66 folded) | §5 Plans ("learns names and jargon"), §4 Insights ("Most corrected word") | Mechanism (unk); auto-learning inferred from plan text (inf) | Nothing. V | BUILD | L | Local options: frequent capitalized tokens from history, or AX read-back of edits (privacy-sensitive, opt-in). DEC-20. | – |
| D-53 | Snippet expansion (owner; C-72 folded) | §4 | Spoken trigger → expansion (links, email, long prompts) (obs) | Nothing. V | BUILD | M | Run after LLM cleanup or protect with placeholders; newlines need D-210. Trigger matching rules (unk). | aho-corasick |
| D-54 | Snippet rich-text body | §11 | – | – | XREF | – | See C-70. | – |
| D-55 | Snippet Import; Share with team | §11 | – | – | XREF | – | See C-71 (team share OUT). | – |
| D-56 | Voice commands (new line, new paragraph) | not in R1 | (unk) | Nothing. V | BUILD | M | Handy-side idea, not Wispr parity. Needs D-210. | – |
| D-57 | Fixes-made counters | §4 | – | – | XREF | – | See C-52. | – |

#### D6. Context, vibe coding, sending

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| D-58 | Context awareness (owner; C-95 folded) | §5 | Toggle on in the observed account: "uses limited relevant text from the app you dictate in to spell names correctly" (obs) | No AX reads. V | BUILD | L | Conflict resolved (C L later, D M): L (needs the AX layer AF-05). Vocabulary only reaches models that support it. Never store; clear toggle. DEC-24. | AX helpers |
| D-59 | Vibe coding: variable recognition in VS Code/Cursor/Windsurf | §5 | "Set up" step; mechanism (unk) | Nothing. V | BUILD | L | Possibly an editor extension. C-97 conflict resolved: classed BUILD (local-feasible), not OUT. | VS Code extension API |
| D-60 | Vibe coding: file tagging in chat | §5 | Toggle (obs); behavior (inf) | Nothing. V | BUILD | L | Needs workspace file list + D-21. | – |
| D-61 | Auto-send per app group (Handy-side idea) | not Wispr parity | Wispr's "Press Enter" is a key remap (A-28), not post-dictation (obs) | `auto_submit` global (`settings.rs:476-479`; `clipboard.rs:882-886`). V | EXTEND | S | Pass-1 framing as Wispr parity removed (audit). Per-app variant needs D-21. | – |
| D-62 | Usage categories | §4 | – | – | XREF | – | See C-54. | – |

#### D7. Personalization

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| D-63 | Voice Profile | §4 | – | – | XREF | – | See C-57. | – |
| D-64 | "My Voice" menu | §6 | – | – | XREF | – | See A-63. | – |
| D-65 | Writing examples beyond Polish | §10 | – | – | XREF | – | Merged into D-35. | – |
| D-66 | "Improve the model for everyone" | §5 | – | – | XREF | – | See C-214 (OUT); no telemetry (E-73). | – |

#### D8. Local LLM runtime and providers

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| D-67 | AI cleanup with no setup and no model choice (owner; E-49 folded) | §8.1, §13 note | Wispr's model setup never observed (unk); cleanup just works for the user (inf) | User must enable Experimental, post-processing, provider, key, model, prompt (`AdvancedSettings.tsx:68-70`, opened; `PostProcessingSettings.tsx:25-182`); prompt `None` skips cleanup (`settings.rs:1035`); default provider `openai` (`settings.rs:710-712`, opened). No LLM runtime in the dependency tree. V | BUILD | L | Core gap. Runtime must be a separate sidecar process (AF-01); needs D-212 provider; Apple Intelligence is an optimization (D-68). Also powers Notetaker summary/ask/titles (E-18, E-23, E-13). R1's "silent model choice" is our proposal (Notes only). DEC-03. | `llama-server` sidecar |
| D-68 | Apple Intelligence as on-device engine | – (local alternative) | – | FoundationModels bridge, macOS 26+, arm64 (`apple_intelligence.swift:5`, `:38-51`; `actions.rs:256-272`, opened); availability collapsed to 0/1 (`:44-50`); no `GenerationOptions`, no `prewarm`; new session per call. Build uses the real bridge only with full Xcode + macOS 26 SDK, else a stub (`build.rs:403-446`). Error copy says "macOS 15" (`shortcut/mod.rs:1324`). V | EXTEND | S | Pass 2 TUNE→EXTEND: `spawn_blocking`, availability reason, temperature, prewarm, auto-select. Release arm64 builds run on `macos-26` (`release.yml:49`). | – |
| D-69 | Cloud LLM providers (owner; E-76 folded) | §7 | – | OpenAI, Z.AI, OpenRouter, Anthropic, Groq, Cerebras, Bedrock built in (`settings.rs:714-812`); `ensure_post_process_defaults` re-adds any missing built-in and force-syncs flags on **every** `get_settings()` (`settings.rs:873-912`, call at `:1153`, opened). Default provider id `openai` (`:710-712`). V | TUNE | S | Conflict resolved on the technical claim: hiding only in the UI is not enough, because the backend re-adds providers and the stored default is `openai`. Either remove them in `default_post_process_providers` (upstream file edit) or filter after `ensure_post_process_defaults` and refuse non-local providers in `post_process_transcription`. Remove vs hide is DEC-04. Cloud LLM use itself is OUT for this product. | – |
| D-70 | Custom OpenAI-compatible endpoint (Ollama) | – | – | `custom`, base URL `http://localhost:11434/v1`, `supports_structured_output: false` (force-synced) (`settings.rs:798-812`, opened). No key needed (`llm_client.rs:198-213`). V | HAVE | – | Escape hatch only: non-structured (legacy single user message, `actions.rs:367-376`). Not the integration path for a bundled runtime (D-212). | – |
| D-71 | Silent LLM model download | §13 | – | ASR/detector downloads via hf-hub with cancel (`model.rs:1939`, `:2264`, `:2580`); LLM GGUFs ignored by discovery (`model.rs:3357-3384`). V | EXTEND | M | Reuse the detector download pattern; disk check (B-54). | hf-hub |
| D-209 | Scratchpad "AI sparkle" | §10 | AI action in the note window (obs); behavior (unk) | Nothing. | BUILD | S | Reuse transform pipeline on note text (no AX). Scope note: Beta. | – |
| D-211 | Warm the LLM at key-down | §8.1 | (unk) | ASR preloads at key-down (`actions.rs:453-461`); no LLM warm-up. V | BUILD | S | Hide load latency in recording time. | `prewarm()` |
| D-212 | Built-in local provider entry for the bundled runtime | §13 note | – | `custom` is non-structured and force-synced; no temperature/max_tokens (`llm_client.rs:112-121`); no timeout; Handy headers `Referer`/`User-Agent`/`X-Title: Handy` (`llm_client.rs:184-196`, opened). V | EXTEND | S | Add a built-in `local` provider (127.0.0.1, random port, structured output on, temperature 0, max_tokens, short timeout, neutral headers). Because built-ins are re-added on read, existing users get it automatically. AF-02. | `send_chat_completion_with_schema` |
| D-213 | LLM memory/unload policy coordinated with ASR | §8.1 | – | ASR unload timeout only (`settings.rs:465`). V | BUILD | M | 8 GB Macs. Sidecar lifetime = idle timeout. | supervisor ideas |
| D-214 | Handy extras: Translate to English, scriptable cleanup trigger | – (Handy only) | – | `translate_to_english` (`settings.rs:448-449`); CLI `--toggle-post-process` (`lib.rs:880`), SIGUSR1 (`signal_handle.rs:50`). V | HAVE | – | Translate works only on capable models. | – |

### Area E: Notetaker (meetings), platform, distribution and quality

#### E1. Notetaker

Scope note for this whole subsection: Notetaker is a second product (meetings). R1 §14 proposes treating it as a later milestone (inf); the decision is DEC-05. Rows are classed by technical gap, not by scope. Rows marked OUT are cloud/account-only. `~cargo/` = `~/.cargo/registry/src/index.crates.io-*/`.

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| E-01 | System audio capture (macOS) | §11, §13.4, §14 | Captures "what others say" after the Screen & System Audio Recording permission (obs) | handy-recorder resolves **input** devices only (`~cargo/handy-recorder-0.1.0/src/backend/cpal/mod.rs:135-143`, `:197-212`). handy-cpal 0.18.2 implements a Core Audio process tap when an input stream is opened on an output device: tap bound to **one output device UID, stream 0**, empty process list marked exclusive = all processes on that device (`~cargo/handy-cpal-0.18.2/src/host/coreaudio/macos/loopback.rs:92-110`, opened). Loopback needs macOS 14.6+ (`README.md:60`). V | EXTEND | M | Own small capture module calling handy-cpal directly + `rubato` (already a dependency), gated at runtime to 14.6+ (calling 14.6 symbols on older OS crashes, I). ScreenCaptureKit buys nothing above 14.6. Device switching E-206; own sounds E-205. AF-10. | handy-cpal loopback |
| E-02 | System-audio permission check and request (owner; B-14 folded) | §10, §11, §13.4 | Gate "Complete setup to start Notetaker", Turn on → System Settings > Screen & System Audio Recording, app restarts, picture of the prompt first (obs) | Permissions plugin wraps `CGPreflightScreenCaptureAccess`/`CGRequestScreenCaptureAccess` = Screen Recording (`~cargo/tauri-plugin-macos-permissions-2.3.0/src/commands.rs:134-140`, opened). No `NSAudioCaptureUsageDescription` (`src-tauri/Info.plist`, opened: only the mic key). V | BUILD | S-M | Pass 2 EXTEND→BUILD. No public preflight for the audio-capture consent (I): use a trial tap. Restart-after-grant flow must be designed. RC-08. | onboarding permission rows |
| E-03 | Dual-source capture (mic + system), time-aligned | §14 | Mic and system recorded together (inf from You/Them) | One recorder; sink keeps only VAD-passed samples, wall-clock alignment lost (`audio_toolkit/audio/sink.rs:70-79`). V | BUILD | M | Separate tracks with shared timestamps; keep dictation's sink untouched. | – |
| E-04 | Echo / bleed handling | §11 (You/Them) | "You" and "Them" stay clean (inf) | No AEC. V | BUILD | M | Fallback: suppress mic segments correlated with the system track. | `webrtc-audio-processing` |
| E-05 | Start Notetaker: button, ⌥M, tray item (owner; A-58 partly folded) | §3, §11 | Start button, "Join meeting / start Notetaker" ⌥M, tray "Start Notetaker" (obs) | Binding system (`settings.rs:953-990`), ACTION_MAP (`actions.rs:965`), tray handler (`lib.rs:285-299`). No meeting action. V | EXTEND | S | ⌥M types "µ" on US layouts (unk whether swallowed). Plumbing A-30. | – |
| E-06 | Detect any call automatically (B-15 folded) | §5, §11, §13.4 | "Automatically detect any call"; onboarding row "Allow Wispr to recognize meetings" (obs) | Absent. V | BUILD | M | Fully local possible (Core Audio "device running somewhere", process list, I); conflict with B-15/C-97 "OUT" resolved: not cloud, so BUILD. | – |
| E-07 | Scheduled-meeting notify, next meeting in menu bar, upcoming meetings, hide next meeting | §5, §11 | Notify 15 s before, show next meeting in menu bar, tray "See all upcoming meetings", "Hide next meeting from menu bar" (obs) | No calendar access, no notification plugin. V | BUILD | M | EventKit + calendars usage string and entitlement (I). | `objc2-event-kit` |
| E-08 | Auto-start Notetaker | §5, §11 | Setting (obs) | Absent. V | BUILD | S | After E-06; consent and false positives. | – |
| E-09 | Stop when call ends | §11 | Setting, on in the observed account (obs) | Absent. V | BUILD | S | After E-06 + silence timeout. | – |
| E-10 | Max recording length; memory for long recordings (E-84 folded) | §5, §11 | "Max recording length (default 2 hours)" (obs) | No cap; samples accumulate in RAM as `Vec<f32>` (`sink.rs:73`). V | BUILD | S | 2 h at 16 kHz f32 ≈ 460 MB per source (I). Needs E-48. | – |
| E-11 | "Join meeting" prompt | §11 tour | Notification-style prompt at the bottom (obs) | Only in-app toasts. V | BUILD | S | Overlay panel or E-91. | – |
| E-12 | Docked right-edge panel window | §11, §14 | Docked panel on the right edge; traffic lights, back, "…", side-by-side, Connect calendar, Share, copy link (obs) | Main window and the overlay NSPanel only (`overlay.rs:18`, `:452-454`). V | BUILD | M | New window label must be added to capabilities (AF-16). | tauri-nspanel |
| E-13 | Title (editable, auto-title on stop), "Today" chip, "Add folder" | §14 | "New note" until named; auto-titled on stop (obs) | History has an unused `title` column. V | BUILD | S-M | Auto-title needs the LLM (D-67) or a heuristic. | – |
| E-14 | "My thoughts" notes tab | §11, §14 | Default tab; placeholder "Write your notes here. We'll make a separate summary at the end." (obs) | Absent. V | BUILD | S-M | Notes feed the summary. | – |
| E-15 | Live transcript tab | §11, §14 | "Listening…", timer, search/translate/copy icons; tab shows a live-waveform icon while recording (obs) | `StreamTextEvent` for dictation only (`transcription.rs:1372-1377`); one global stream route (`transcription.rs:898-916`, opened); long-form non-streaming models produce no live text (`managers/transcription/policy.rs:51-59`). V | EXTEND | M | Force chunking for every non-streaming model + own stream route. Tab icons: audit PARTIAL R1-§14-07. | `LiveSession`/`ChunkedDecode` |
| E-16 | "You" / "Them" bubbles | §11 tour | Bubbles labelled You and Them (obs) | Absent. V | BUILD | S | By source: mic = You, system = Them (I). | – |
| E-17 | Stop / Resume | §14 | Stop becomes Resume (obs) | Dictation stop/cancel only (`utils.rs:102-126`). V | BUILD | S | Append segments with offsets. | – |
| E-18 | "Ask anything" + "What did I miss?" | §11, §14 | Q&A over the transcript; recap chip (obs) | No Q&A; LLM client exists (`llm_client.rs`). V | BUILD | L | Needs D-67; windowing for small contexts. | – |
| E-19 | Consent line | §10, §11, §14 | "Always get consent when transcribing others. Learn more." (obs) | Absent. V | BUILD | S | Copy + local help page. | – |
| E-20 | "We'll improve the transcript…" banner and tip cards | §11, §14 | Lilac banner; tip card "Speaker names are added later" with Next and dots (obs) | Absent. V | BUILD | S | Pure UI. | – |
| E-21 | Post-meeting transcript re-pass | §11, §14 | Transcript improved after the meeting (obs claim in UI) | Retry job exists (`transcription_jobs.rs`). V | EXTEND | M | Re-run full audio with a better batch model. | job manager |
| E-22 | Speaker diarization and naming | §5 Plans, §11, §14 | Speaker identification; names from calendar (obs) | Sortformer 4-speaker model in catalog (`catalog.json:1520-1541`) filtered out (`catalog/mod.rs:317-320`, opened: `role == "asr"`); `DiarizeSession::run(&[f32])` is batch-only and refuses while a stream holds the model (`~cargo/transcribe-cpp-0.4.0/src/diarize.rs:114-127`); no diarize request in the worker protocol (`engine_supervisor/protocol.rs:23-53`). English-only training, max 4 speakers, licence `other`. V | EXTEND | L | Pass 2 M→L. Map speaker ids onto the "Them" track; rename UI. | transcribe-cpp diarize |
| E-23 | Structured summary | §11, §14 | Summary tab: "1 MIN READ", headline, bullets, Next steps with owners; sparkle tab icon (obs) | No summarizer. V | BUILD | L | Needs D-67; map-reduce for long transcripts. | JSON-schema output |
| E-24 | Progress states | §14 | "Transcript ready, step 2 of 3", skeleton, "Turning your 1-minute meeting into a two-minute read" (obs) | Job status Queued/Running/Finished/Failed (`transcription_jobs.rs:28-40`). V | EXTEND | S | Multi-step pipeline state. | – |
| E-25 | "We hit a snag processing your meeting" + retry | §14 | Orange banner with retry (obs) | Job panics caught (`transcription_jobs.rs:250-253`, `:339-341`); worker crash recovery in the supervisor (`engine_supervisor/mod.rs:8-11`). V | EXTEND | S | Per-step retry. | – |
| E-26 | "Started by mistake? Discard / Keep" | §14 | Toast when few words captured (obs) | Empty-result detection for dictation (`transcription.rs:992`). V | EXTEND | S | Esc should dismiss (A-25). | – |
| E-27 | Screen-share visibility toast | §14 | "Your Flow Bar and Notepad show up in screen shares and screenshots. [Update settings]" (obs) | Absent. V | BUILD | S | Needs E-28. | – |
| E-28 | Hide panels and bar from screen capture (owner; A-20 folded) | §5, §11 | "Don't show Notepad and Flow Bar in screen capture", off in the observed account (obs) | No content protection on any window. V | BUILD | S | `set_content_protected` / `sharingType = .none`; ScreenCaptureKit may ignore it for some paths (I). The bar must honor it. | Tauri window API |
| E-29 | Pill recording state for Notetaker (owner; A-21 folded) | §10, §11, §14 | Bar shows waveform and record dot; Notetaker button has a teal badge (obs) | Overlay states recording/streaming/transcribing/processing (`overlay.rs:619-631`). V | EXTEND | S | Shares A-01/A-03 work. | – |
| E-30 | Notetaker page (owner; C-03 folded) | §4, §10 | Start Notetaker button, Today (calendar-driven, "No meetings today"), tabs Past notes / Shared with me / Folders; "No meetings found: Add calendar…" (obs) | No meetings page (`Sidebar.tsx:34-77`). V | BUILD | M | Adds a sidebar entry (hotspot). | – |
| E-31 | Past notes list | §4, §14 | Meetings appear under Today / Past notes (obs) | Dictation history only. V | BUILD | M | Store: E-203. | – |
| E-32 | Shared with me | §4 | Cloud sharing (obs) | n/a | OUT | – | Account/cloud sharing. | – |
| E-33 | Folders | §4, §14 | "Add folder", Folders tab (obs) | Absent. V | BUILD | S-M | Local tags/folders table. | – |
| E-34 | Search across notes and transcripts | §4 | Search icon (obs) | No search; FTS5 available (`libsqlite3-sys-0.35.0/build.rs:132`). V | BUILD | S-M | Shares FTS setup with C-36. | FTS5 |
| E-35 | Notepad default view / side-by-side | §11 | "Open Notepad" dropdown; side-by-side toggle (obs) | Absent. V | BUILD | S | UI only. | – |
| E-36 | "Show live transcript" toggle | §11 | On in the observed account (obs) | Absent. V | BUILD | S | Off = batch after (cheaper). | – |
| E-37 | Consent calendar notice | §11 | Writes a notice into calendar event descriptions (obs) | n/a | OUT | – | Cloud calendar write. Local: "Copy consent message". | – |
| E-38 | Sharing defaults, Share, copy link, auto-share | §11, §14 | Cloud links (obs) | n/a | OUT | – | Local: export as Markdown/PDF. | – |
| E-39 | Import meeting notes / recordings | §11, §12 | Import button; "Where would you like to upload from?" Granola / Otter (obs) | File import into History exists (`transcribeFiles.ts:9-20`; `FileDropOverlay.tsx`); Opus/ogg/webm not decodable (`transcribeFiles.ts:6-8`). V | EXTEND | S-M | Import from Granola/Otter accounts is cloud; audio import is local. | – |
| E-40 | Calendar connectors (Google, Outlook OAuth) | §5, §13.5 | OAuth connectors (obs) | n/a | OUT | – | Local equivalent: macOS Calendar via EventKit (E-42). | – |
| E-41 | MCP for meeting notes | §5, §12 | Hosted MCP for Claude/ChatGPT/Gemini/Cursor (obs) | n/a | OUT | – | Local later: notes as Markdown files, optional local stdio MCP. | – |
| E-42 | Calendar for speaker names and context (local) | §10, §13.5 | "Add calendar for accurate speaker names and richer summaries" (obs) | Absent. V | BUILD | M | EventKit attendees; mapping to speakers manual/heuristic. | EventKit |
| E-43 | Notetaker setup gate and 2-min guided tour | §10, §11 | Presenter video overlay, simulated meeting in the docked panel, Join prompt, You/Them, Ask anything, Stop, Summary; tour ticks the checklist and opens Connectors next (obs) | Onboarding practice framework exists (`OnboardingFlow.tsx`, `TryIt.tsx`). V/I | EXTEND | M | Scripted fake-meeting replay; chaining is B-44. | B-40 engine |
| E-44 | Meeting audio kept, playback, extract | §4, §14 | Whether meeting audio is kept (unk); dictation audio is kept (inf) | Dictation WAVs + player (`actions.rs:702-706`; `HistorySettings.tsx:548`). No audio encoder in the tree (symphonia/rodio decode only). V/I | EXTEND | M | Pass 2 S→M. Two ~2 h tracks; FLAC needs a new encoder (Opus would not re-import). | FLAC encoder crate |
| E-45 | Translate transcript | §11 | Translate icon (obs) | Whisper translate-to-English only. V | EXTEND | M | Other targets need the LLM. | – |
| E-46 | Copy transcript / export | §11 | Copy icon (obs) | History clipboard helper. V | EXTEND | S | – | – |
| E-47 | Dictation while a meeting records | §10 (three-button bar) | Flow Bar still offers dictation next to Notetaker (inf) | One worker, one model (`engine_supervisor/mod.rs:12-13`, opened); a live stream holds the model's exclusive compute lease and other runs get `Busy` (`~cargo/transcribe-cpp-0.4.0/src/model.rs:87-101`, `session.rs:274-293`); Esc → `cancel_transcription` → `engine.cancel()` bumps a global epoch and kills the in-flight worker (`utils.rs:116-118`; `transcription.rs:912-917`; `supervisor.rs:475-485`, opened). V | EXTEND | L | Pass 2 M→L. Two designs: chunked meeting decode on the shared queue with per-owner cancel (live text stalls while dictating), or a second supervisor with its own model copy (double RAM/VRAM). Core edit in a hotspot. AF-04, DEC-06. | – |
| E-48 | Long-recording robustness (stream to disk, crash recovery, sleep) | §11 (2 h) | Up to 2 h meetings (obs setting) | Recording held in memory until stop (`sink.rs:70-79`), WAV written after. V | BUILD | M | Incremental writes, recovery on next launch. Sleep: E-207. | hound streaming writer |
| E-49 | Local LLM runtime | §14 | – | – | XREF | – | See D-67 (owner). | – |
| E-50 | Notetaker onboarding questions | §13.2-3 | – | – | XREF | – | See B-07, B-08 (OUT). | – |
| E-84 | Memory for long recordings | – | – | – | XREF | – | Merged into E-10 / E-48. | – |
| E-201 | Notetaker settings page (container) | §5, §10, §11 | Sections Meeting detection, While Notetaker is active, Notepad, Keyboard shortcut, Transcript, Consent, Sharing, Import, plus Start tutorial (obs; lower sections user-reported in §10, observed in §11) | Absent. V | BUILD | S | Individual options are E-05–E-10, E-28, E-35–E-38. | `SettingsGroup` |
| E-202 | Docked-panel header controls and "…" menu | §11, §14 | Back, "…", side-by-side (obs); "…" contents (unk) | Absent. V | BUILD | S | Ships with E-12. | – |
| E-203 | Meeting store, lifecycle and retention | §4, §5, §14 | Meetings persist under Today / Past notes; Pro "more meetings kept longer" (obs) | Dictation history would purge them (`settings.rs:676-682`). V | BUILD | S-M | Separate store (DEC-08). | FTS5 |
| E-204 | Search inside one transcript | §11, §14 | Search icon in the Transcript header (obs) | Absent. V | BUILD | S | – | – |
| E-205 | Own-sound and mute contamination of the system track | §14 (You/Them) | Clean "Them" track (inf) | Tap includes all processes, so Handy's own sounds are captured (`loopback.rs:93`, `:109`, opened); `mute_while_recording` mutes system output via `osascript` (`managers/audio.rs:100-111`, opened). V; tap behavior after mute (unk) | BUILD | S | During a meeting skip mute-while-dictating and feedback sounds, or exclude our PID (handy-cpal patch). Otherwise the user goes deaf mid-call. | handy-cpal PR |
| E-206 | Output-device changes during a meeting | – | Capture follows AirPods (inf) | Tap is created for a fixed device UID (`loopback.rs:94-101`). V | BUILD | S-M | Re-create the tap on default-output change; stitch the timeline. | Core Audio listeners |
| E-207 | Prevent sleep and App Nap while recording | §11 (2 h) | Recording survives idle display (inf) | No power assertion anywhere. V | BUILD | S | Only while a meeting records. | `beginActivity` |
| E-208 | "Join meeting": open the event's meeting link and start | §3, §11 | ⌥M "Join meeting / start Notetaker" (obs) | Absent. V | BUILD | S | After E-07. | `tauri-plugin-opener` |
| E-209 | Notetaker app menu | §6 | – | – | XREF | – | See A-63. | – |
| E-210 | System audio on Windows and Linux | – | – | WASAPI loopback in handy-cpal (`host/wasapi/mod.rs:24`); Linux monitor sources listed by handy-recorder but filtered out by Handy (`commands/audio.rs:220`). V | EXTEND | M | Linux is the cheapest port. | – |
| E-221 | "Try Notetaker" card on the Notetaker page | §4, §10 | Today section card "Try Notetaker, 2 minute guided tour" with Skip / Start and a video-thumbnail icon (obs) | Absent. V | BUILD | S | Audit PARTIAL R1-§4-17. Entry to E-43; Skip = dismiss (C-23 state). | – |

#### E2. Platform, distribution and quality

| ID | Feature | R1 ref | Wispr behavior (R1 tag) | Handy today (evidence path:line, tag) | Class | Effort | Notes / risks / cross-refs | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| E-51 | Version + Update button (owner; C-27 folded) | §5, §11 | Footer "Flow v1.6.xxxx" + Update; cloud icon after updating (obs) | Footer `UpdateChecker`: check, install with progress, `relaunch()` (`UpdateChecker.tsx:64`, `:85`, `:143-162`); version `v{version}` with hard-coded fallback "0.1.2" (`Footer.tsx:13-26`). V | TUNE | S | Move to Settings footer; honest fallback. | – |
| E-52 | Tray "Check for updates…" | §11 | – | – | XREF | – | See A-60. | – |
| E-53 | Periodic update checks | §11 | Version changed 1.6.1264 → 1.6.1297 after a restart (obs); background checking or download (inf) | Checks once when the footer mounts, only after onboarding (`App.tsx:453`); errors only to console (`UpdateChecker.tsx:108-109`, opened). V | EXTEND | S | Periodic check in Rust; visible error state. | updater Rust API |
| E-54 | Our own update endpoint and signing key | §5 | Vendor-controlled (inf) | Pubkey and endpoint point at upstream (`tauri.conf.json:86`, `:88`, opened). V | TUNE | S | Must change before the first public build. AF-11. | `tauri signer generate` |
| E-55 | macOS Developer ID signing | – | – | CI imports p12, greps "Developer ID Application" (`build.yml:221-243`); local default ad-hoc `signingIdentity: "-"`, hardened runtime (`tauri.conf.json:41-43`, opened). V | HAVE | S (secrets) | Needs our secrets. Worker re-launches the same executable, so it inherits the signature (I). | – |
| E-56 | Notarization | – | – | `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` passed to tauri-action (`build.yml:453-456`). V (env), I (behavior) | HAVE | S (secrets) | `APPLE_ID_PASSWORD` is a legacy name (I). DMG stapling (unk). | – |
| E-57 | `latest.json` and signature | – | – | `createUpdaterArtifacts: true` (`tauri.conf.json:28`); signing key secrets (`build.yml:463-464`). V | HAVE | S (keys) | Release is a draft until published (`release.yml:35`). | – |
| E-58 | Release workflow | – | – | `workflow_dispatch`, version from `tauri.conf.json`, draft, 7-target matrix (`release.yml:3`, `:17-23`, `:47-69`). V | TUNE | S | Trim to the two macOS entries. Correction: Linux jobs would not fail for lack of Azure secrets; only Windows does. `asset-prefix` has no effect on release asset names. | – |
| E-59 | Windows signing coupling | – | – | `signCommand` hard-codes upstream Azure account (`tauri.conf.json:77`, opened). V | TUNE | S | Breaks any local Windows build. | – |
| E-60 | Linux packaging paths | – | – | `/usr/lib/Handy` (`tauri.conf.json:53-55`, `:65-67`), rpath `build.rs:19`, CI smoke tests (`build.yml:577-595`, `:620-642`, `:750-763`). V | TUNE | S | Change together when Linux returns. | – |
| E-61 | Bundle identity rebrand | – | – | `productName: "Handy"`, `identifier: "com.pais.handy"` (`tauri.conf.json:3`, `:5`, opened); window title "Handy". V | TUNE | S | Choose once with the Apple team; changing later resets TCC (Accessibility, Mic) and moves data (I). AF-11, DEC-17. | – |
| E-62 | Brand strings and URLs (E-77 folded) | – | – | 32 "Handy" strings in `en/translation.json`; `tray_i18n.rs`; LLM headers (`llm_client.rs:184-196`, opened); donate/repo/docs links (`AboutSettings.tsx:34`, `:74`; `SecureInputWarning.tsx:10`; `LinuxGuide.tsx:7`; `portableInstaller.ts:12`); log file name `handy` (`lib.rs:841`, `:845`); WAV `handy-{ts}.wav` (`actions.rs:701`). `settings.rs:1526` is a test fixture, not a default. V | TUNE | S-M | Use `{{appName}}` interpolation in a fork overlay to limit locale conflicts. Help links → C-103. | i18next interpolation |
| E-63 | App icons | – | – | `src-tauri/icons/*`, tray PNGs. V | TUNE | S | Mascot art too (C-24, B-27). | – |
| E-64 | Data storage locations | §5 | "Store data locally" dropdown (obs) | App data: `history.db`, `recordings/`, models dir, `settings_store.json`; logs in OS log dir; HF downloads in the shared HF cache, `HF_ENDPOINT`/`HF_HOME` honored (`~hf-hub/src/api/tokio.rs:254`, `:294`); `HF_HOME` under app data only in portable mode (`portable.rs:43-45`). V | TUNE | S | Consider setting `HF_HOME` under app data. Related: B-205. | – |
| E-65 | Open data / logs / recordings folders | – | – | `commands/mod.rs:81`, `:97`, `:111`. V | HAVE | – | – | – |
| E-66 | Download / export your data (owner; C-94 folded) | §5 | Request a copy by email (obs) | No export. V | BUILD | S | Local zip: history JSON/CSV + audio + notes + dictionary. | `zip` crate |
| E-67 | Delete your data (owner; C-94 folded) | §5 | Delete your data (obs) | Per-entry delete, retention, model delete only. V | BUILD | M | Must cover new stores (stats, dictionary, snippets, notes, meetings), settings, logs and HF cache copies. | – |
| E-68 | Data and Privacy page (owner; C-93 folded) | §5, §10 | Cloud toggles (obs); our product: "all local, no data leaves the computer", short simple page (user) | No privacy page. Pieces exist: folders (`AboutSettings.tsx:80-81`), retention. V | BUILD | S | Must be truthful: list the network calls (AF-12), the HF cache, update checks, logs. Cloud toggles: C-214, C-219–C-221. | – |
| E-69 | History retention defaults | §4 | – | – | XREF | – | See C-46. | – |
| E-70 | Settings persistence and migrations | – | – | One JSON store, salvage, schema version + migrations (`settings.rs:1112-1135`, `:1198`). V | HAVE | – | Adding `AppSettings` fields is a hotspot; prefer a fork store. | – |
| E-71 | History DB migrations | – | – | `rusqlite_migration` + `ADDED_COLUMNS` outside migrations (`history.rs:20-42`, `:170-186`, opened). V | HAVE | – | Do not append to `MIGRATIONS` in the fork. DEC-08. | – |
| E-72 | API key storage | – | – | `SecretMap` in settings, redacted in Debug output; stored in plain JSON (I). V/I | TUNE | S | Moot if cloud providers go (DEC-04); else Keychain. | `keyring` |
| E-73 | No telemetry | §5 | Opt-out toggles (obs) | No analytics crates or endpoints (search). V | HAVE | – | Add a CI grep guard. | – |
| E-74 | Model downloads (HF + mirror) | §13 | – | hf-hub with pinned revision + sha256; mirror `https://blob.handy.computer` (`catalog/mod.rs:36`); legacy URLs (`model.rs:916-1038`). V | TUNE | S | Mirror is upstream's host: keep, replace or drop (DEC-16). | – |
| E-75 | Update check on by default | §5 | – | `update_checks_enabled` default true (`settings.rs:596-598`, opened); toggle only on the Debug page (`DebugSettings.tsx:56`); env `HANDY_DISABLE_UPDATER`. V | TUNE | S | Only call made without user action. Move the toggle out of Debug; disclose. | – |
| E-76 | Cloud LLM providers | §7 | – | – | XREF | – | See D-69. | – |
| E-77 | External links | §10 | – | – | XREF | – | See E-62 (brand links) and C-103 (Help and Feedback). | – |
| E-78 | Log handling | §10 | Support flow (obs) | 500 KB, KeepOne rotation (`lib.rs:817-821`); default level Debug (`settings.rs:656-658`, opened); release builds redact transcripts (`utils.rs:17-23`), but a full settings dump (custom words, prompts, device names; API keys redacted) is logged at startup (`settings.rs:1103-1110`, opened). V | TUNE | S | Default Info or drop the dump; say it on the privacy page. "Copy diagnostics" (C-103). | – |
| E-79 | Crash handling | – | – | Engine isolated in a worker with GPU→CPU fallback (`engine_supervisor/mod.rs:1-20`, opened); `ErrorBoundary.tsx`; no panic hook for the main process. V | EXTEND | S | Local crash log only. | panic hook |
| E-80 | i18n coverage | §5 | App Language dropdown (obs) | 26 locales × 650 keys; runtime English fallback (`i18n/index.ts:93`); CI fails on missing keys (`scripts/check-translations.ts`). V | HAVE | – | Every new string needs 26 translations or a policy. | – |
| E-81 | Accessibility (VoiceOver, contrast) | – | – | Many `aria-` attributes and reduced-motion handling; VoiceOver behavior (unk). V/U | TUNE | M | Audit overlay and new windows. C-210. | axe-core via Playwright |
| E-82 | Model load / unload | – | – | Unload after 5 min (`settings.rs:150-160`); worker exits with the model. V | HAVE | – | Meetings must pin the model. | – |
| E-83 | Startup time and idle memory | – | – | Not measured. U | – | – | Runtime check RC-10. Not classified. | – |
| E-85 | Single instance | – | – | `tauri_plugin_single_instance` (`lib.rs:876-897`). V | HAVE | – | – | – |
| E-86 | CLI | – | – | `cli.rs:6-63`. V | HAVE | – | – | – |
| E-87 | Launch at login | §5 | – | – | XREF | – | See A-64. | – |
| E-88 | Portable mode | – | – | `portable.rs:1-50`. V | HAVE | – | – | – |
| E-89 | What's New after update | §10 | – | – | XREF | – | See C-99. | – |
| E-90 | Help and Feedback / support | §6, §10 | – | – | XREF | – | See C-103. | – |
| E-91 | Native notifications | §5, §11 | Notifications for meetings, milestones (obs) | No notification plugin. V | BUILD | S | Needed by E-07, E-11; DEC-21. | tauri-plugin-notification |
| E-92 | Minimum macOS vs feature gating | – | – | `minimumSystemVersion: 10.15` (`tauri.conf.json:42`, opened); CoreAudio weak-linked (`build.rs:22-27`). V | TUNE | S | System audio 14.6+, Apple Intelligence 26+: runtime gates and copy. DEC-22. | – |
| E-93 | Entitlements / usage strings | §13.4 | Each permission explained (obs) | Only microphone entitlements and `NSMicrophoneUsageDescription` (`Entitlements.plist`, `Info.plist`, opened). V | EXTEND | S | Add system-audio and calendars strings when needed. | – |
| E-94 | Nix / immutable installs | – | – | `HANDY_DISABLE_UPDATER`; `flake.nix:29-31`, `:172`. V | HAVE | – | Brand-coupled env var. | – |
| E-95 | Frontend-backend contract (bindings) | – | – | `bindings.ts` regenerated by tauri-specta (`lib.rs:799-804`); one `collect_commands!` list. V | – | – | Hotspot note, not a gap (AF-13). Not classified. | – |
| E-211 | Fork-only Tauri config overlay | – | – | Platform overlay precedent `tauri.windows.conf.json`. V | TUNE | S | Holds productName, identifier, updater pubkey and endpoint; `version` must stay in the base file (`release.yml:21`, `build.yml:85` grep it). | Tauri config merge |
| E-212 | Updater key custody + CI identity guard | – | – | Pubkey compiled in (`tauri.conf.json:86`); no guard. V | BUILD | S | Fail CI if upstream pubkey/endpoint/identifier reappear after a merge; document key backup and rotation. | shell grep step |
| E-213 | Own version scheme and tag hygiene | – | – | Version 0.9.8 shared with upstream (`tauri.conf.json:4`, opened; `Cargo.toml:3`); tags `v<version>` (`release.yml:33`). V | TUNE | S | Avoid tag collisions and non-monotonic versions. | – |
| E-214 | macOS release verification job | – | – | `build.yml` audits Linux and Windows only. V | BUILD | S | codesign, spctl, stapler, `--list-devices` smoke test. | – |
| E-215 | Defer update install while recording | §11 | Restart is user-initiated (obs) | `downloadAndInstall` then immediate `relaunch()` (`UpdateChecker.tsx:143-162`). V | EXTEND | S | Block or queue during dictation/meeting. | updater Rust API |
| E-216 | App location, translocation | – | – | No install-location check. V | BUILD | S | Offer "Move to Applications". | – |
| E-217 | Distribution channels | – | – | GitHub Releases only; Nix disables updater. V | BUILD | S | Homebrew cask in own tap; Mac App Store not viable (sandbox vs Accessibility paste and key hooks, I). | – |
| E-218 | Licences and attributions | – | – | MIT declared (`tauri.conf.json:31`); no acknowledgements UI; model licences in catalog. V | BUILD | S | Keep Handy's MIT notice; model licences shared with B-204. | `cargo-about` |
| E-219 | CI triggers for our branch | – | – | Quality/tests/nix on `main` and PRs; `main-build.yml` signs on push to `main` (`:8-9`, `:51`). V | TUNE | S | Retarget to `wispr-bro`; avoid spending signing secrets on upstream syncs. | – |
| E-220 | Platform coverage (macOS now, Windows later, mobile) | §1, §5 Plans, §9 | Product on Mac/Windows/iOS/Android (obs) | macOS-first fork; Windows/Linux code paths exist upstream. V | OUT | – | Audit PARTIAL R1-§1-06. Scope note: mobile apps are OUT (R1 §9 "Probably out of scope or later"); Windows/Linux are later per the brief. Neutral flag only. | – |

## 6. Handy strengths to keep (merged)

Capture and insertion
- Non-activating, non-focusable overlay NSPanel: dictation never steals focus (`overlay.rs:450-480`). Every new clickable state must keep this.
- Exactly-once, cancel-safe pipeline: paste lock, cancel generations, session counter, remembered press while busy, well unit-tested (`clipboard.rs:18-20`; `actions.rs:81-83`, `:777-849`; `transcription_coordinator.rs:335-376`).
- Receipt-based "reliable paste" with concealed pasteboard types (`paste_tx/`); layout-aware ⌘V (`input.rs:70-133`); alternative insertion methods, auto-submit, trailing space.
- Honest capture readiness: arming animation; chime and mute only after the first real sample (`actions.rs:549-588`).
- Live transcript panel (Live overlay style), which Wispr does not show.
- Secure Input awareness with a Carbon shadow for keyed hotkeys and a tray explanation (`secure_input.rs`); hotkey tap self-heals after macOS disables it (handy-keys).
- Mic robustness: fallback and persist on device loss, clamshell mic, channel selection, on-demand mic so the orange dot shows only while recording.
- Overlay placement on the cursor's display, above the Dock, on all Spaces and over full-screen apps; tray-icon recovery; SMAppService login item; careful activation-policy handling.
- CLI and signal control, start hidden, single instance (`cli.rs`, `signal_handle.rs`, `lib.rs:876-897`).

Onboarding and models
- Onboarding resumes after relaunch, including lessons and a "Welcome back" note (`onboardingStore.ts`); preview mode for every screen.
- Permissions engine: ordered rows, polling, focus re-check, no relaunch needed after Accessibility, Windows support (`usePermissions.ts`).
- A real first dictation inside onboarding with precise failure explanations and inline mic switching (`useTryEngine.ts:155-224`).
- Animated demo engine with reduced-motion stills (`Demo.tsx`): base for every tutorial.
- Offline, honest model catalog with per-language grades and a published method; pinned revisions, sha256, resumable downloads, 4-attempt retry, stall watchdog, mirror (`catalog/`, `managers/model.rs:2008-2160`).
- Spoken-languages model with a language detector fetched only when needed; weak-language honesty; Chinese script handling.

Text and data
- Deterministic, language-aware, fail-open text stage (fillers, stutters, whitespace) (`audio_toolkit/text.rs`; `transcription.rs:1600-1662`).
- Custom words as decoder vocabulary on capable models, careful fuzzy matcher otherwise.
- Raw and cleaned text stored side by side; retry as a cancellable background job; failed dictations kept with audio (`managers/history.rs`; `transcription_jobs.rs`; `actions.rs:885-897`).
- Forward-compatible SQLite schema (newer DBs open in older builds); FTS5 + trigram compiled in.
- LLM client hardening: reasoning-disable with per-endpoint memory, `<think>` stripping, sanitized error logging; Apple Intelligence bridge with structured output.
- File transcription by picker or drag-and-drop; offline What's New; one-file color tokens with full dark mode; accessible `Dialog`; 26 locales with a CI parity check.

Platform
- Process-isolated speech engine with crash/hang recovery and GPU→CPU fallback (`engine_supervisor/`).
- Streaming and VAD-chunked long-form decoding with one policy point; background jobs that yield to dictation.
- Complete CI signing, notarization and updater pipeline (`build.yml`); settings salvage and migrations; portable mode; Nix updater kill switch.
- No telemetry; transcripts redacted from release logs.
- Low-level Notetaker building blocks already present upstream: loopback in handy-cpal, diarization in transcribe-cpp.

## 7. Rough edges and bugs found in Handy (merged, deduplicated)

| ID | Issue | Evidence | Rows |
|---|---|---|---|
| RE-01 | Errors, empty takes, paste failures and the no-model refusal go to toasts in the usually hidden main window; the pill just hides. | `App.tsx:155-222`; `actions.rs:838-848`, `:460-476` | A-12, A-13, A-53 |
| RE-02 | Every hotkey silently dies if the event tap cannot be created; the Tauri fallback never runs. If the fallback did run, an fn binding would fail to register (no hotkey), not reset. | `shortcut/handy_keys.rs:86-120`; `shortcut/mod.rs:40-56` | A-35, A-208 |
| RE-03 | History keeps 5 dictations by default and deletes text with audio; retention never runs while idle; `history_limit` accepts 0 (then every unsaved dictation is deleted right after saving); retention UI falls back to "never" while the backend default is `PreserveLimit`. | `settings.rs:676-682`; `managers/history.rs:455-470`, `:309`, `:503-505`; `HistoryLimit.tsx:23`; `RecordingRetentionPeriod.tsx:19` | C-46 |
| RE-04 | History shows and copies the pre-LLM text; tray "Copy last" uses the cleaned text. | `HistorySettings.tsx:288-294`, `:524-534`; `tray.rs:611-616` | C-34, C-39, A-27 |
| RE-05 | "Raw" history text is not verbatim: already dictionary-corrected and filler-stripped. | `transcription.rs:1643-1662` | D-06, C-42 |
| RE-06 | The deterministic stage removes all newlines. | `audio_toolkit/text.rs:214-220`, `:330-365`, `:468-480` | D-210 |
| RE-07 | Clipboard restore keeps only text or an image, with fixed 60 ms delays and a documented race; the concealed reliable path is Debug-gated off; transcripts reach clipboard managers on the default path. | `clipboard.rs:48-92`; `settings.rs:512-520`, `:664-670`; `paste_tx/mod.rs:3-7` | A-47, A-48 |
| RE-08 | Esc cannot abort transcription or post-processing, and does not match while fn is held. | `actions.rs:628-629`; `handler.rs:61-68`; `handy-keys/types/modifiers.rs:173-174` | A-25, A-201 |
| RE-09 | Empty ("not set") bindings are rejected by both `change_binding` and the handy-keys validator. | `shortcut/mod.rs:169-172`; `shortcut/handy_keys.rs:414-416` | A-32 |
| RE-10 | Tap-to-lock with fn would start hands-free dictation on fn+arrow/F-key chords. | `transcription_coordinator.rs:471-492`; `handy-keys/manager.rs:56-75` | A-22 |
| RE-11 | Tray "Copy last transcript" is always enabled and silently does nothing when empty; tray "Check for updates" silently does nothing when the user disabled checks. | `tray.rs:518-524`, `:656-673`; `lib.rs:292-300` | A-57, A-60 |
| RE-12 | Mute-while-recording shells out to `osascript` on every dictation. | `managers/audio.rs:100-111` | A-41, E-205 |
| RE-13 | Overlay geometry is duplicated by hand in Rust and CSS. | `overlay.rs:36-54`; `RecordingOverlay.css:21-34` | A-01, A-10 |
| RE-14 | Model concepts leak everywhere: onboarding model step, General "<model> settings" card, footer selector, tray Model ▸ and Unload; download copy names the model. | `Model.tsx`; `ModelSettingsCard.tsx:21-41`; `Footer.tsx:33`; `tray.rs:65-66`, `:327-337` | B-47, B-207, A-56 |
| RE-15 | Stale backend ranking: `HANDY_RANKING` still decides backend auto-select, ignoring spoken languages; frontend never reads it. | `catalog/mod.rs:38-53`; `managers/model.rs:1096-1114`, `:1486-1491` | B-58 |
| RE-16 | Two sources of truth for onboarding: webview `localStorage` vs `onboarding_completed` in settings; upgrade migration marks onboarding done when a model was selected. | `onboardingStore.ts:60`, `:136`; `settings.rs:434-435`, `:1204-1210` | B-13 |
| RE-17 | `useMachineTier` reports GPU = true before the async check resolves. | `useMachineTier.ts:10` | B-49 |
| RE-18 | Onboarding frame is a fixed 720×600; main window min size equals default 720×600. | `OnboardingShell.tsx:38`; `lib.rs:970-971` | B-03, C-25 |
| RE-19 | Replay of onboarding and the update-check toggle are reachable only from Debug. | `App.tsx:316-320`; `DebugSettings.tsx:56` | B-45, E-75 |
| RE-20 | Custom-word input strips apostrophes and quotes ("O'Brien" → "OBrien"); fuzzy fallback ignores non-ASCII words. | `CustomWords.tsx:14-18`; `audio_toolkit/text.rs:58-60` | C-58, D-47 |
| RE-21 | Retry overwrites the previous text; Delete has no confirm or undo. | `managers/history.rs:384-410`, `:691-720`; `HistorySettings.tsx:405-412` | C-43, C-44 |
| RE-22 | LLM cleanup needs Experimental + post-processing + provider + model + prompt + a second hotkey; with defaults it still does nothing (no prompt selected). Turning Experimental off does not turn post-processing off. | `AdvancedSettings.tsx:66-76`; `settings.rs:1035`; `actions.rs:206-212`; `shortcut/mod.rs:1112-1117` | D-01, D-67 |
| RE-23 | LLM client: no timeout, no temperature/max_tokens, up to 4 HTTP calls per dictation, upstream Handy headers; structured-parse failure pastes raw model content. | `llm_client.rs:112-121`, `:184-226`, `:453-487`; `actions.rs:324-401` | D-11, D-12, D-212 |
| RE-24 | Apple Intelligence call blocks a Tokio worker and ignores cancel; error copy says "macOS 15" while the bridge requires 26; word limit reuses the model field. | `actions.rs:256-272`; `apple_intelligence.swift:80-118`; `shortcut/mod.rs:1324` | D-68 |
| RE-25 | Cloud providers are re-added and their flags force-synced on every settings read; default provider is `openai`. | `settings.rs:873-912`, `:1153`, `:710-712` | D-69 |
| RE-26 | Prompt editor: no reset to default, explicit save, cannot delete the last prompt. | `PostProcessingSettings.tsx:364-380` | D-36 |
| RE-27 | Default file log level Debug plus a startup settings dump (custom words, prompts, devices). | `settings.rs:656-658`, `:1103-1110` | E-78 |
| RE-28 | Update check runs once on footer mount (only after onboarding), errors only in console; install relaunches immediately even mid-recording. | `UpdateChecker.tsx:48-110`, `:143-162`; `App.tsx:453` | E-53, E-215 |
| RE-29 | Upstream identity hard-coded: bundle id, updater key/endpoint, Windows signing account, LLM headers, log and WAV file names. | `tauri.conf.json:1-8`, `:77`, `:84-90`; `llm_client.rs:184-196`; `lib.rs:841`; `actions.rs:701` | E-54, E-61, E-62 |
| RE-30 | Recording kept fully in RAM until stop (fine for dictation, not meetings). | `audio_toolkit/audio/sink.rs:70-79` | E-10, E-48 |
| RE-31 | Version fallback hard-coded to "0.1.2" in two places. | `Footer.tsx:22`; `AboutSettings.tsx:25` | E-51 |
| RE-32 | Sidebar nav items are `div`s (no keyboard focus or role); many user-facing strings live under `settings.debug.*` keys. | `Sidebar.tsx:104-111`; `MuteWhileRecording.tsx:23`, `HistoryLimit.tsx:30` | C-210, C-79 |
| RE-33 | Window-wide file drop sends any dropped file (e.g. a .csv) to transcription. | `FileDropOverlay.tsx:19-29`; `App.tsx:454` | C-205 |
| RE-34 | `@tauri-apps/plugin-sql` is still a frontend dependency though history moved to rusqlite; CSP is `null`. | `package.json:40`; `tauri.conf.json:16` | – |
| RE-35 | Two dictations stopped in the same second would get the same WAV name `handy-<unix seconds>.wav` (I, low risk). | `actions.rs:701` | C-45 |

## 8. Runtime checks needed (a quick test on a real Mac)

| ID | Check | Why it matters |
|---|---|---|
| RC-01 | Does the first click on the pill ✕ reach the WKWebView in the never-key, non-activating panel without `accept_first_mouse`? | Every clickable pill state (A-04, A-06, A-07, A-15). |
| RC-02 | With fn bound in handy-keys (press blocked, release passed), does macOS still run the Globe action (emoji picker, input source, Apple Dictation)? Test macOS 14, 15, 26 and each "Press 🌐 key to" setting; also the F5 dictation key (`fn+dictation`). | fn default (A-22, A-35, A-202, DEC-09). |
| RC-03 | Does CSS `:hover` fire inside the non-key panel's webview, or must tracking-area events drive hover? | A-02. |
| RC-04 | Does AX return the focused element, its role, `AXValue` and `AXSelectedText` in Slack, VS Code, Discord, Chrome, Arc, Safari, Mail, with and without `AXManualAccessibility`? | A-50, A-51, D-29, D-58 (AF-05). |
| RC-05 | What does the target app receive when ⌘V is injected while the user still holds ⌃⌘ from the paste-last hotkey? | A-26. |
| RC-06 | What happens on the pill and in history when the mic is unplugged or a Bluetooth mic drops mid-dictation? | A-211. |
| RC-07 | Does the shortcut recorder receive mouse buttons (middle, side)? | A-28. |
| RC-08 | Process tap: which TCC prompt and Settings pane appears, how to detect "granted", whether a restart is needed, whether the tap still sees audio after an output mute, and what happens when the output device switches (AirPods connect mid-call). | E-01, E-02, E-205, E-206. |
| RC-09 | Speed and resident memory of the default ASR picks on an M1 8 GB and an Intel Mac; reload time of the first dictation after the 5-minute unload. | B-49, B-214, DEC-02. |
| RC-10 | Startup time; idle CPU and memory of an always-visible overlay webview over hours (WebKit growth, #1279). | A-01, E-83. |
| RC-11 | Latency and quality of a 0.5-3B instruct model via `llama-server` on an 8 GB M1, and of Apple Intelligence, for Light/Medium cleanup; memory with ASR loaded. | D-67, D-68, D-213. |
| RC-12 | Release: is the DMG itself notarized and stapled; N-1 → N update from the footer and from the tray. | E-56, E-214. |
| RC-13 | Do global ⌥M / ⌥1 bindings swallow "µ" / "¡" for users who type them? | E-05, D-32. |

## 9. Open questions (still unknown after both passes)

Wispr side (R1 `unknown`; would need another observation session):
1. Tap vs hold thresholds, double-tap window, and whether a single fn tap does anything (R1 §3, open item 2).
2. Held-key and hands-free pill states when started from the keyboard; processing, error and idle-hover states; pixel sizes, colors, timings (R1 §2).
3. Whether a real (not synthetic) click on the bar steals focus, and how Wispr then inserts.
4. Insertion method (paste vs AX value set), clipboard behavior, how "no textbox" is decided, behavior in password fields.
5. Fresh-install defaults of Launch at login, Show Flow Bar, Show in Dock, sounds, notifications (R1 saw a used account).
6. Command Mode behavior, especially with no selection; Transform with no selection; the diff window itself.
7. Snippet trigger matching (whole utterance vs inline); Style and Auto cleanup in one model call or two, and their order; whether web apps count by site.
8. What "variable recognition" (vibe coding) installs; whether ⌘Z right after a dictation also reverts cleanup; whether Undo AI edit re-pastes.
9. File and View menu contents; status-menu items not listed in R1 §11.
10. Whether meeting audio is kept; the Notetaker "…" menu; sharing and folder dialogs.

Our side:
11. The exact ggml commit vendored by transcribe-cpp-sys 0.4.0 (`ggml/UPSTREAM` not shipped), which decides whether llama.cpp could share it (AF-01).
12. Apple's on-device model limits: context size, languages, refusal behavior.
13. Legal status of `other`-licensed models as silent defaults (nemotron, Fun-ASR, SenseVoice) and of the Sortformer diarizer; Sortformer quality on non-English audio and >4 speakers.
14. Word segmentation for CJK/Thai in stats (`unicode-segmentation` UAX #29 vs WebKit `Intl.Segmenter`).
15. Whether Tauri's `core:default` grants window start-dragging for an overlay title bar (C-25).
16. What the Metal "VRAM" value means on Apple Silicon and whether it can replace a RAM probe.
17. Whether Try-it practice takes are saved to history exactly like normal dictations (B-211, inferred yes).
18. Binary name inside Linux/Windows bundles after a `productName` change (CI smoke tests, E-60).

## 10. Merge log

### Duplicates resolved (owner ← folded rows)

| Owner | Folded / cross-referenced rows |
|---|---|
| A-11 processing state | D-10 |
| A-14 pill message state | D-39 |
| A-16 spotlight coaching | B-32, B-41 |
| A-22 fn default; A-35 fn/Globe support | B-23 |
| A-28 Press Enter | D-61 reframed as a Handy-side idea (per-app auto-send) |
| A-37 microphone list | C-81 (channel-selector note moved here) |
| A-38 recommended mic | B-20, A-213 |
| A-40 mic level meter / mic test | B-19 |
| A-41 mute while dictating | C-90 |
| A-42 sounds | C-89 |
| A-60 tray check for updates | E-52 |
| A-63 all app menus | C-105, C-106, C-203, D-64, E-209 |
| A-64 launch at login | B-35, C-85, E-87 |
| A-65 show in Dock | C-87 |
| A-66 show Flow Bar at all times | C-86 |
| B-07 / B-08 meeting quiz | E-50 |
| B-42 whisper tutorial; B-40 practice engine | C-49 |
| B-44 setup checklist | C-10 (+ audit MISSING R1-§11-44 chaining) |
| B-61 my languages | C-82 |
| B-67 app language | C-83 |
| C-02 landing page | B-34 |
| C-23 banner component | B-38 |
| C-29 home banner | B-33 |
| C-42 Undo AI edit | D-07 |
| C-43 Retry | D-08 |
| C-46 retention | E-69 |
| C-50 Insights | C-04 |
| C-52 fixes made | D-57 |
| C-54 usage by category | D-62 (capture stays in D-21) |
| C-57 Voice Profile | D-63 |
| C-58 dictionary page | C-05 |
| C-59 dictionary tabs | D-51 |
| C-64 import wizard | D-50 |
| C-69 snippets page | C-06 |
| C-70 snippet editor | D-54 |
| C-71 snippet import | D-55 |
| C-74 Scratchpad page | C-09 |
| C-99 What's New | E-89 |
| C-103 Help and Feedback | E-90, E-77 (links part to E-62), A-61 (tray entry in A-56) |
| C-214 cloud data toggles | D-66 |
| D-18 Style page | C-07 |
| D-28 Transforms | C-08 |
| D-35 writing examples | D-65 |
| D-48 from → to rules | C-63 |
| D-52 auto-learned words | C-66 |
| D-53 snippet expansion | C-72 |
| D-58 context awareness | C-95 |
| D-67 local LLM | E-49 |
| D-69 cloud providers | E-76 |
| E-02 system-audio permission | B-14 |
| E-05 start Notetaker; E-07 calendar items | A-58 |
| E-06 call detection | B-15 |
| E-10 / E-48 long recordings | E-84 |
| E-28 hide from capture | A-20 |
| E-29 Notetaker pill state | A-21 |
| E-30 Notetaker page | C-03 |
| E-51 version + update | C-27 |
| E-66 / E-67 export / delete data | C-94 |
| E-68 privacy page | C-93 |

### Conflicts resolved

| ID | Conflict | Resolution and basis |
|---|---|---|
| M-01 | fn support: B-23 "unknown" vs A-35 "supported". | Supported. handy-keys validator accepts parseable strings incl. fn and formats FN (`shortcut/handy_keys.rs:400-424`, opened); fn alone arrives as FlagsChanged 0x3F. Only the Tauri backend rejects fn (`tauri_impl.rs:50-60`, opened). Pass-1 A's "fallback resets fn to default" corrected: tap failure never triggers the fallback (`handy_keys.rs:86-120`, opened). |
| M-02 | Launch at login: A/C/E TUNE vs B HAVE; Wispr "on by default" vs unknown. | HAVE (A-64): backend default false (`settings.rs:590-600`, opened) but onboarding sets `atLogin: true` (`onboardingStore.ts:66-74`, opened) and saves it. Wispr: "seen on in a used account", fresh default unknown. |
| M-03 | Replacement rules before (C-63) or after (D-48) fuzzy matching. | Before fuzzy, after Chinese script conversion; fuzzy must not rewrite rule outputs. Basis: today's order is script → fuzzy → fillers → normalize (`transcription.rs:1598-1665`, opened); a rule's "from" is what ASR wrote, so it must see text before fuzzy rewrites it, and user-explicit rules should win over a heuristic. Class EXTEND M (owner D-48). This is a design call, flagged so R4 can revisit. |
| M-04 | Cloud providers removed (D-69 OUT) vs hidden (E-76 TUNE). | Technical claim settled: UI-only hiding is insufficient, because `ensure_post_process_defaults` re-adds built-ins on every `get_settings()` (`settings.rs:870-912`, `:1150-1156`, opened) and the default provider is `openai` (`settings.rs:694-714`, opened). Either remove them in `default_post_process_providers` or filter after the sync and refuse non-local providers. D-69 owns it as TUNE S (removing/hiding existing code); remove vs hide is DEC-04. |
| M-05 | Mic level meter: A-40 BUILD S vs B-19 EXTEND M. | One row, A-40 BUILD M: levels exist only per recording and only to the overlay (`overlay.rs:736-767`, opened); a monitor mode that opens and closes the stream cleanly plus UI is more than S. |
| M-06 | Retention: C-46 EXTEND M vs E-69 TUNE S. | C-46 EXTEND M: changing the default is trivial, but split text/audio retention needs new cleanup logic (`history.rs:455-470` deletes both, opened). |
| M-07 | D-06 "Original never lost": pass 1 HAVE, pass 2 TUNE S. | EXTEND S: besides the retention default (C-46), "exact transcript" needs a new `asr_text` column and returning the pre-fix text from `post_process_transcription_text` (`transcription.rs:1598-1665`, opened). |
| M-08 | Fixes made: C-52 BUILD M vs D-57 EXTEND S. | C-52 EXTEND M owns it (stages exist and must return counts; plus LLM word diff and a correction log). |
| M-09 | Checklist: C-10 BUILD S vs B-44 BUILD M. | B-44 BUILD M (auto-ticking hooks on many events). |
| M-10 | Banners: B-38 BUILD M vs C-23 BUILD S. | C-23 BUILD S for the component; content is counted in each feature row. |
| M-11 | Press Enter meaning: A-28 key remap vs D-61 "dictate then Enter". | A-28 matches R1 §3 ("Trigger enter with a mouse shortcut (or another key)"). D-61 kept as a Handy-side idea. |
| M-12 | Scratchpad shortcut default: C-77 "default binding" vs A-30 "not set". | Not set by default (R1 §3), ⌥S proposed in the Scratchpad dialog (R1 §11). C-77 owns the action. |
| M-13 | Transform hotkeys effort: A-30 S vs D-32 M. | D-32 EXTEND M owns them (dynamic actions, layout issues); A-30 keeps plumbing S. |
| M-14 | Context awareness: C-95 L "later" vs D-58 M. | D-58 BUILD L (needs the AX layer, AF-05). |
| M-15 | Vibe coding: C-97 OUT vs D-59/D-60 BUILD L. | BUILD L: it is local-feasible, so not OUT by the class definition. C-97 keeps only Connectors/MCP as OUT. |
| M-16 | Version + Update: C-27 HAVE vs E-51 TUNE. | E-51 TUNE S (works; moving it out of the global footer is a UX change). |
| M-17 | What's New: C-99 HAVE vs E-89 TUNE. | C-99 TUNE S (no reopen path, `WhatsNewGate.tsx:28-43`). |
| M-18 | Privacy page: C-93 EXTEND vs E-68 BUILD. | E-68 BUILD S owns the page (it holds the network inventory). |
| M-19 | Notetaker rows OUT (A-21, A-58, B-14, B-15, C-03, C-97) vs EXTEND/BUILD in E. | Notetaker rows are classed by technical gap in E1 with a scope note; OUT is reserved for cloud/account-only items. Scope is DEC-05. |
| M-20 | App menus: A-63 M vs C-105 S / C-106 S / D-64 S. | A-63 BUILD M owns all menus, including rebuilding Edit/Window (AF-15). |
| M-21 | Help/support across A-61, C-102, C-103, E-77, E-90. | C-13 owns the popover, C-103 the local feedback action, C-102 hosted help is OUT. |
| M-22 | Show Flow Bar: A-66 BUILD vs C-86 EXTEND. | A-66 BUILD S (the work is A-01). |
| M-23 | Recommended mic: C-81 TUNE vs A-38 EXTEND. | A-38 EXTEND S: detection does not exist (no "recommended" label in any component). |
| M-24 | End state: B-34 EXTEND M vs C-02 EXTEND S. | C-02 owns it, raised to M (new home layout). |
| M-25 | Processing state: A-11 HAVE vs D-10 TUNE. | A-11 HAVE; D-10 is a copy note. |
| M-26 | Call detection: E-06 BUILD vs B-15 OUT. | E-06 BUILD M (local mechanism). |
| M-27 | Dock owner: audit suggested C for Dock, C review suggested A. | A-65 owns (it carries the activation-policy risk); C-87's three-place note moved into A-65. |
| M-28 | D-69 class OUT vs E-76 TUNE. | TUNE S (see M-04): the work is changing existing code; the cloud feature itself is noted as OUT. |
| M-29 | E-49 suggested `llama-cpp-2` in the engine worker. | Rejected on the evidence in AF-01 (static unprefixed ggml; worker is the same executable). Sidecar process. |
| M-30 | Export/delete scope: C-94 S vs E-66/E-67 S with larger scope. | E-66 BUILD S; E-67 BUILD M (must cover new stores, settings, logs, HF cache). |
| M-31 | Hide from capture: A-20 vs E-28. | E-28 owns (setting lives in Notetaker settings); the bar must honor it. |
| M-32 | A-01 resting bar M vs pass-2 L; B-50 M vs L; D-29 M vs L; E-22, E-47 M vs L; B-03 M vs L. | Pass-2 values taken; evidence re-checked for A-01 (`overlay.rs:450-480`), B-50 (`actions.rs:460-476`), E-47 (`supervisor.rs:468-486`, `utils.rs:100-128`). |

All other pass-2 class/effort changes were applied as written: A-22 TUNE S→EXTEND M, A-36 TUNE→HAVE, A-26 S→S-M, A-02 risk lowered; B-03 TUNE M→L, B-10 EXTEND→TUNE, B-45 EXTEND→TUNE, B-57 TUNE (or EXTEND if silent), B-21 risk/reuse; C-76 →HAVE, C-99 →TUNE, C-35 →EXTEND, C-63 →EXTEND (then folded into D-48), C-52 →EXTEND, C-44 →EXTEND, C-70 →S plain / L rich, C-101 →EXTEND, C-10 →M (folded), C-105/106 →M (folded), C-36 →S, C-25 →M, C-02 →M, C-27 →TUNE (folded), C-94 →M (folded into E-67); D-14 →M/L, D-22 →L, D-29 →L, D-30 →M, D-48 →M, D-57 →M (folded), D-58 →L, D-68 →EXTEND, D-65 merged; E-02 →BUILD S-M, E-22 →L, E-44 →M, E-47 →L, E-84 merged, E-52 HAVE + TUNE note.

### Spot-checked rows (code opened during this merge)

59 rows across all areas; every cited range was opened and matched the claim unless noted.
- **A (18):** A-02 (`RecordingOverlay.tsx:212-216`), A-04 (`commands/mod.rs:10-20`), A-07 and A-52 (`overlay.rs:405-420`, `:450-480`), A-12 and B-50 (`actions.rs:460-476`), A-18 (`settings.rs:616-626`), A-22 (`settings.rs:940-950`), A-23 (`transcription_coordinator.rs:471-492`), A-24/A-201 (`handy-keys modifiers.rs:170-176`), A-32 (`handy_keys.rs:400-424`), A-35 (`shortcut/mod.rs:40-56`, `handy_keys.rs:86-120`, `tauri_impl.rs:50-60`), A-40 (`overlay.rs:736-767`), A-42 (`settings.rs:995-1002`), A-47/A-48 (`clipboard.rs:48-62`, `:78-92`), A-60 (`lib.rs:292-300`), A-64 (`settings.rs:590-600`, `onboardingStore.ts:66-74`).
- **B (7, incl. B-50 above):** B-37 (`Demo.tsx:515-526`), B-47 (`onboardingStore.ts:15-25`), B-49/B-214 (`src-tauri/Cargo.toml:150-168`), B-205 (`managers/model.rs:33-38`), B-11 (`Info.plist`).
- **C (9):** C-01 (`Sidebar.tsx:34-77`), C-02 (`App.tsx:85-90`), C-34/C-39 (`HistorySettings.tsx:288-294`, `:524-534`; `tray.rs:609-618`), C-46 (`settings.rs:654-684`; `managers/history.rs:455-470`), C-205 (`App.tsx:450-455`), C-208 (`history.rs:250-258`), C-209 (`history.rs:20-42`, `:170-186`), C-07 (`AdvancedSettings.tsx:30-34`, `:66-76`).
- **D (10):** D-01 (`actions.rs:965-986`), D-06/D-48 (`transcription.rs:1598-1665`), D-210 (`text.rs:214-222`, `:324-366`, `:468-482`), D-21 (`Cargo.toml:166`), D-67 (`AdvancedSettings.tsx:66-76`), D-68 (`actions.rs:256-272`), D-69 (`settings.rs:694-714`, `:870-912`, `:1150-1156`), D-70 (`settings.rs:798-812`), D-212 (`llm_client.rs:184-226`).
- **E (15):** E-01/E-205/E-206 (`handy-cpal loopback.rs:92-110`), E-02 (`tauri-plugin-macos-permissions commands.rs:134-140`), E-22 (`catalog/mod.rs:312-322`), E-47 (`engine_supervisor/mod.rs:1-24`, `utils.rs:100-128`, `transcription.rs:898-916`, `supervisor.rs:468-486`), E-53 (`UpdateChecker.tsx:48-52`, `:104-110`), E-54/E-59/E-61/E-92/E-213 (`tauri.conf.json:1-8`, `:38-44`, `:74-92`), E-75 (`settings.rs:590-600`), E-78 (`settings.rs:654-658`, `:1100-1110`), E-93 (`Entitlements.plist`, `Info.plist`).

**Fixes from the spot-check:** A-32 also needed the handy-keys validator (`handy_keys.rs:414-416` rejects empty strings), not only `change_binding`; A-52 citation narrowed to `overlay.rs:466-472`; D-210 whitespace collapse is at `text.rs:474-476`; E-47 supervisor `cancel()` is at `supervisor.rs:475-485`; E-78 settings dump at `settings.rs:1106-1110`. No claim was found wrong beyond line drift.

### Unsupported Wispr claims corrected

| Rows | Was | Now |
|---|---|---|
| C-66, C-68 | Dictionary words "marked with a sparkle", per-entry source/sparkle/counts as Wispr behavior | Sparkle dropped; metadata moved to Notes as a Handy-side proposal; auto-learning tagged (inf) from R1 §5 plan text (D-52). |
| B-49 | R1 ref "auto-chosen from the hardware" | Re-cited as a product goal, not R1. |
| D-67, B-47 | Wispr "Cloud, invisible (obs)" for silent model choice | Wispr model setup (unk); "silent model choice" moved to Notes as our proposal. |
| A-64 (B-35, C-85, E-87), A-42 (C-89), A-66, A-65, C-91, E-09, E-28, E-36, C-219, C-221 | "On/off by default" | "Seen on/off in a used account" (obs); fresh default (unk). Only E-10's "default 2 hours" is a stated default in R1. |
| D-61 | Press Enter as "after dictation" | A-28 wording (key remap); D-61 kept as a Handy-side idea. |
| A-23 | "as Wispr implies" a single tap is a no-op | Removed; R1 says nothing about single taps. |
| E-53 | Wispr does background checks and silent download | Version changed after a restart (obs); background behavior (inf). |
| E-44 | Meeting audio kept (from dictation rows) | Meeting audio (unk); dictation audio kept (inf). |
