# R4: Parts bin (verified)

The open-source building blocks for a free, fully local, consumer-grade macOS dictation app built on Handy (MIT; Tauri 2.11.5, Rust, React 18). Windows and Linux come later. This file merges and corrects four research passes (`r4-work/R4a`-`R4d`) and reconciles them with R3 (`r3-work/`). Every check behind it is listed in `r4-work/R4-verification-log.md`.

## 1. Purpose and how to read

**Purpose.** Give R5 (scope and plan) one recommended part per capability, so we write as little code as possible. Each part comes with its licence, how we use it, the Handy files it touches, the R2 gaps it closes, effort, and what must still be tested on a real Mac.

**Owner's rules applied.**
- Code licences allowed: MIT, Apache-2.0, BSD, ISC, Zlib and Unlicense. Fonts may be OFL-1.1.
- Never GPL, AGPL, LGPL, or code without a licence.
- Model weights get their own check. CC-BY attribution, non-commercial terms, use restrictions and custom terms are flagged in §5.

**Tags.**
- **v** means verified in this pass: I opened the LICENSE file, the source line or the registry metadata myself. The ID in brackets points into the verification log (`L-`, `V-`, `C-`, `F-`, `R-`).
- **(inf)** means inferred, not run.
- **(unk)** means unknown until a run on a real Mac; those runs are listed in §6 as RT-xx.

**Effort.**
- S: up to 2 days.
- M: up to 2 weeks.
- L: more than 2 weeks.

**Adoption verbs.**
- **adopt:** use as a dependency, unchanged.
- **vendor:** copy the files in, keep the notice.
- **port:** translate or copy the logic into our code, keep the notice.
- **idea:** design only, no code taken.

**Naming.** Two different projects are called "Hex":
- **hex (Rust)** is `anomalyco/hex`, MIT, © 2026 Christopher Langton.
- **Hex legacy (Swift)** is `kitlangton/Hex`, MIT, © 2025 Kit Langton, now archived.

**Paths.** Handy paths are relative to `src-tauri/src/` unless they start with `src/` (frontend) or `.github/`.

---

## 2. The recommended parts list

One row per capability. The licence column shows what was verified.

| Capability | Choice (name, version, URL) | License (verified) | How we use it | Handy files touched | R2 ids closed | Effort | Risks / runtime checks |
|---|---|---|---|---|---|---|---|
| **Local LLM runtime** | llama.cpp `llama-server`, pinned release tag (b11972 or newer at build time), https://github.com/ggml-org/llama.cpp | MIT **v** [L-01] | **Vendor-build.** CI builds one static binary per arch from the pinned tag: `-DBUILD_SHARED_LIBS=OFF -DGGML_METAL_EMBED_LIBRARY=ON -DCMAKE_OSX_DEPLOYMENT_TARGET=13.0`, TLS/BoringSSL off, `llama-server` target only. Ship it as a Tauri `externalBin` (signed by Tauri before the app). Spawn: `-m <gguf> --host <unix .sock> -c 4096 -np 1 --cache-ram 256 --reasoning off --no-webui --sleep-idle-seconds 120`, with a random `LLAMA_API_KEY` passed in env. A small supervisor of our own (~300 lines) handles health polling, prewarm at key-down, restart once, CPU fallback (`-ngl 0`), and SIGTERM then SIGKILL. It borrows SpeakoFlow's prewarm and resident-window ideas and OpenWhispr's flags, and does **not** download engines at runtime [R-12]. Talk to it through `reqwest::ClientBuilder::unix_socket` (in reqwest 0.12.28, no feature flag) **v** [V-04]. | `llm_client.rs` (socket transport, neutral headers, `temperature`, `max_tokens`, timeout); new `fork/llm/supervisor.rs`; fork overlay `tauri.fork.conf.json` (`bundle.externalBin`); `.github/workflows/build.yml` (build step) | AF-01, D-67, D-211, D-213, DEC-03 | M | Separate process, so no second ggml in Handy or the worker **v** [F-01]. Defaults that must be overridden: ctx = full model context, `--cache-ram` 8 GiB, web UI on, port 9931 **v** [V-11]. Never pass `-hf`. RT-01, RT-03, RT-06, RT-10, RT-11, RT-12. |
| **LLM models per RAM tier** | Apple Silicon ≤ 8 GB: **Qwen3.5-2B Q4_K_M** (`unsloth/Qwen3.5-2B-GGUF`, 1,280,835,840 B). Apple Silicon ≥ 16 GB: **Qwen3-4B-Instruct-2507 Q4_K_M** (`unsloth/Qwen3-4B-Instruct-2507-GGUF`, 2,497,281,120 B). Intel: LLM off by default (deterministic text), opt-in Qwen3.5-0.8B/2B on CPU. macOS 26 with Apple Intelligence: Apple FM while the GGUF downloads (row below). | Apache-2.0 **v** [L-02, L-03, L-04] | **Adopt** (download). Add them to a fork catalog section with HF revision + sha256 pinned (`catalog/mod.rs` pattern), downloaded silently after the ASR model through Handy's hf-hub downloader. Bake-off challengers: Qwen3-1.7B, Qwen3.5-4B (reasoning off), `Muesli-HQ/qwen35-2b-muesli-cleanup-v12-gguf` (Apache-2.0, training data undocumented) [L-39], SpeakoFlow Mini (Apache-2.0, CC-BY-4.0 data, English only) [L-40], Gemma-4-E2B, Ministral-3-3B, Granite-4.2-3B, Phi-4-mini. | `catalog/` (fork section), `managers/model.rs` (stop ignoring our LLM GGUFs, test at `:3357-3384`), RAM probe (hardware row) | D-71, AF-19, AF-20, DEC-03 | S | No first-party GGUF exists for either pick (HF 401) [L-04], so the source is a community quant: pin it. Qwen3.5 is hybrid attention, and prefix reuse needs `-cms` tuning (RT-06). The 2B repo also holds vision `mmproj` files; do not download them. RT-02, RT-04. |
| **Apple Foundation Models bridge** | hex (Rust) `src/apple_intelligence.swift` + `src/apple_intelligence.rs`, https://github.com/anomalyco/hex | MIT **v** [L-06] | **Port** about 40 changed lines: `blocking(timeout:)` with `task.cancel()` **v** [C-01], availability reasons 0-5, `contextSize`, permissive guardrails, `GenerationOptions`, 10 s availability cache **v** [C-02, C-03]. Rust calls it in `spawn_blocking` inside `complete_unless_cancelled`. One generation only; add `supportsLocale` and `prewarm(promptPrefix:)`. | `swift/apple_intelligence.swift`, `apple_intelligence.rs`, `actions.rs:256-272` | AF-14, D-68, D-208, D-211 | S | macOS 26 + Apple Silicon + Apple Intelligence on + model ready. The bridge compiles only with full Xcode and the macOS 26 SDK (release already uses `macos-26`). Guardrail refusals. RT-07. |
| **Cleanup prompts and guards** | Prompts: **OpenWhispr** `src/locales/*/prompts.json` `cleanupPrompt` (2,227 chars, 10 locales) as the seed; **FreeFlow** `Sources/PostProcessingService.swift:117` `commandModeSystemPrompt` for Transforms and Command Mode; SpeakoFlow `settings.rs:3120-3320` prompt as challenger. Guards: **SpeakoFlow** `actions.rs:1283-1460` validators + **FreeFlow** `TranscriptTextCore.swift:128-182` `appearsToHaveExecutedInstruction`. | MIT (OpenWhispr, FreeFlow, SpeakoFlow) **v** [L-12, L-36] | **Port.** One prompt builder: level + style fragment + rules + dictionary terms + language. Built-in `local` provider: structured output on, temperature 0, `max_tokens`, reasoning off, one call, one deadline (`1.2 s + 25 ms × expected tokens`, max 6 s). Guards run in this order: JSON parse (Handy's `transcription` field **v** [V-12]); SpeakoFlow empty / punctuation-only / channel-name / prompt-echo / over-length checks; FreeFlow preamble check; our length-ratio band; our diff guard (digits, URLs, e-mail, code, dictionary words must survive); `whatlang` language check. Any hit pastes the deterministic text and shows "Pasted without cleanup". Never paste unparsed output (today `actions.rs:340-350` does) **v** [C-07]. | `settings.rs` (built-in `local` provider via `default_post_process_providers`), `actions.rs` (`post_process_transcription`: skip the legacy chain for `local` and Apple), `llm_client.rs`, new `fork/llm/{prompt,guards}.rs` | AF-02, D-01, D-05, D-09, D-11, D-12, D-27, D-29, D-44, D-212, DEC-13 | M | The FreeFlow check is English-only and fires only when the raw text has instruction words **v** [C-10]. Small models drift (SpeakoFlow Mini's own card reports 10.7 % content damage). RT-04, RT-05, RT-08. |
| **Accessibility (AX) client** | **SpeakoFlow** `src-tauri/src/smart_spacing/macos.rs` (450 lines) on **objc2-application-services 0.3.2**, https://crates.io/crates/objc2-application-services | SpeakoFlow MIT **v** [L-12]; objc2-application-services Zlib OR Apache-2.0 OR MIT **v** [L-13] | **Port** SpeakoFlow's layer (focused element, `AXStringForRange` caret window, AXManualAccessibility lease, messaging timeout, secure-field and terminal exclusion **v** [C-13]) into fork `ax.rs`. Add selected-text read, `AXSelectedTextRange`, `AXBoundsForRange`, set `AXSelectedText`, and `AXFocusedWindow`→`AXTitle`. Attribute names are string literals (the crate has no constants **v** [V-05]). Fallback order: AX, then enable AXManualAccessibility and retry once, then ⌘C with full snapshot (clipboard row), then blind paste + card. A false "no text field" never blocks a paste. Alternative: R4b's own wrapper from scratch [R-01]. | new `fork/ax.rs`; `Cargo.toml` (one crate); hooks in `actions.rs` (pre-paste check), `paste_tx/macos.rs` (secure-field refusal) | AF-05, A-50, A-51, D-29, D-30, D-52, D-58, DEC-24 | M | All calls are `unsafe`. Electron/Chromium trees appear only after the lease. Setting `AXSelectedText` may report success and do nothing (RT-21). RT-20, RT-21. |
| **Frontmost app / window title / browser URL** | objc2-app-kit 0.3.2 `NSWorkspace` (already in tree) + AX title (row above) + **port of mado 0.0.21** `src-swift/BrowserURLExtractor.swift` + `SupportedBrowsers.swift` (~160 Swift lines → Rust), https://crates.io/crates/mado | objc2 triple **v**; mado crate MIT **v**, **but its source repo is AGPL-3.0 by default** [L-10] | **Port** from the crates.io tarball (or `crates/mado/`) only. Order: AX document URL first, then the address-bar value only when the bar is not focused **v** [C-27]. No AppleScript, no Automation prompt. **Do not port `WebsiteInfo.swift`** (Google favicon URL = network). | new `fork/frontmost.rs`, `fork/ax.rs`; call at record start in `actions.rs`; store via fork DB | D-21, D-22, DEC-23 | S + M | Record provenance (crate 0.0.21, MIT). `frontmostApplication` freshness off the main thread (RT-23). Chromium may need the lease (RT-22). |
| **Hotkeys and tap health** | Keep **handy-keys 0.3.4** (in tree; MIT). Port **anarlog `crates/shortcut-macos`** `processor.rs` + `decision.rs` + `tests/processor.rs`, https://github.com/fastrepl/anarlog. Vendor **axwatch 0.1.0**, https://crates.io/crates/axwatch | handy-keys MIT; anarlog MIT outside `enterprise/` **v** [L-08]; axwatch MIT OR Apache-2.0 **v** [L-09]; Hex legacy MIT (credit) **v** [L-07] | 1) **Fix** `HandyKeysState::new`: build the manager inside the thread, return its `Result` on a oneshot, so the Tauri fallback runs (`shortcut/handy_keys.rs:86-120` **v** [C-15]). 2) **Port** the state machine (double-tap lock 300 ms, modifier-only minimum 300 ms, cancel window 1 s, dirty-until-release, Esc cancels; 311 + 605 test lines, deps serde/specta/thiserror **v** [C-16]) into the coordinator, driven by handy-keys events. Keep both the anarlog and Hex MIT notices. 3) **Vendor** axwatch (~500 lines) plus a probe tap as a health check on wake and on permission change. **No polling** of `CGEventTapIsEnabled` (handy-keys #1827) [R-03]. 4) Remove the unused `rdev` **v** [V-06]. | `shortcut/handy_keys.rs`, `transcription_coordinator.rs` (hook), new `fork/shortcut/{processor,health}.rs`, `tray.rs` (warning), `Cargo.toml` (drop rdev) | AF-08, A-22, A-23, A-25, A-201, A-208, A-35 (part), DEC-09 | S + M | Globe/fn system action at the session vs the HID tap (RT-18). axwatch relies on undocumented notifications that are withheld from unsigned builds on macOS 15+ (RT-19). |
| **Clipboard** | Own multi-item snapshot/restore on **objc2-app-kit `NSPasteboard`** (in tree), following Hex legacy `PasteboardSnapshot` (`Hex/Clients/PasteboardClient.swift:51-82`) plus hex (Rust) `src/paste.rs` restore guard (generation + `changeCount`, `org.nspasteboard.TransientType`/`AutoGeneratedType`). Plus **arboard 3.6.1** `SetExtApple::exclude_from_history()` and `.html()` | objc2 triple; Hex legacy and hex MIT **v**; arboard MIT OR Apache-2.0 **v** [V-03] | **Write** ~60 lines: iterate `pasteboardItems` → `types` → `dataForType`; restore one `NSPasteboardItem` per saved item; mark the restore Transient. Use arboard (already compiled, make it direct) for concealed default-path writes and HTML. ⌘C selection fallback: snapshot, ⌘C, wait for `changeCount`, read, restore. Promote Handy's concealed "reliable paste". | `paste_tx/macos.rs:157-175,270-278`, `clipboard.rs:50-92` **v** [C-19], `Cargo.toml` | AF-17, A-47, A-48, A-204, C-70, D-29 | S | Clipboard managers may log the restore as a new copy (RT-24). |
| **Pill / overlay** | **tauri-nspanel 2.1.1** from crates.io (bump from git `da9c9a8`, 23 commits behind 2.1.1) **v** [V-01], https://crates.io/crates/tauri-nspanel + **Tolkovo** `src-tauri/src/tolkovo/pill.rs` pattern, https://github.com/kozliq/Tolkovo + **anarlog** `plugins/overlay/src/lib.rs:164-250` click-through only if needed | tauri-nspanel MIT OR Apache-2.0 **v**; Tolkovo MIT **v** [C-28]; anarlog MIT **v** [C-29] | **Adopt** nspanel 2.1.1 (needs `tauri ^2.8.5`, so OK). **Port** Tolkovo's resting-pill setup: `PanelLevel::Status`, `hides_on_deactivate(false)`, `accept_first_mouse(true)` via `with_window`, Space-change re-show, manual drag, click to toggle. Hover comes from a native tracking area, not CSS. Re-place on `NSApplicationDidChangeScreenParametersNotification`. Hide from capture with `set_content_protected` **v** [C-22, C-23]. anarlog's 20 Hz cursor polling only if the pill sits in a larger transparent window. | `overlay.rs:24-33,450-480` **v** [C-20], `Cargo.toml`, new `fork/pill.rs`, `src/overlay/*` | AF-09, A-01, A-02, A-04, A-06, A-07, A-15, A-16, A-212, E-28, DEC-25 | S-M | Tolkovo uses the private `_setPreventsActivation:`. Check whether nspanel 2.1.1's "synchronize nonactivating panel activation state" fix makes it unnecessary (RT-13). WebKit memory over hours (RT-17). RT-13..RT-17. |
| **System audio** | **hex (Rust)** `src/meeting_capture.rs` tap path (`initMonoGlobalTapButExcludeProcesses` with our own process object, private aggregate device; objc2-core-audio 0.3.2, no exact pins) **v** [C-38]. Alternative: flexaudio-os-macos 0.8.1 (MIT, but it pins `=objc2 0.6.4`, `=libc 0.2.186` …) **v** [V-10, L-11] | hex MIT **v**; objc2-core-audio triple | **Port** the tap path only (skip hex's ScreenCaptureKit fallback, which needs Screen Recording). Feed the existing resampler and VAD. Add `NSAudioCaptureUsageDescription`. Detect permission with a temporary probe tap (public API; meeting-record pattern, idea). Replaces handy-cpal's per-device, own-sounds-included loopback **v** [C-24]. | new `fork/meeting/capture.rs`, `Info.plist`, `managers/audio.rs` (mute-while-dictating off during meetings) | AF-10, E-01, E-02, E-205, E-206 | M | Gate at **macOS 14.4** (Apple process tap; flexaudio `version.rs`). A global tap should follow the default output (RT-26). Self-exclusion before we have an audio object (RT-27). RT-25..RT-28. |
| **Call detection** | **hex (Rust)** `src/microphone_activity.rs` (664 lines) + the provider catalog and stability rules of `src/meeting_detection.rs` | MIT **v** [C-38] | **Port.** Listen to `kAudioDevicePropertyDeviceIsRunningSomewhere`, then list process objects with `IsRunningInput`, excluding our PID, and match bundle IDs (zoom.us, Teams, FaceTime, Slack, browsers) with a ~2 s stability window. | new `fork/meeting/detect.rs` | E-06, E-07 (detection part) | S-M | Process-object properties on 14.0-14.3 and reliability for Meet in Chrome (RT-29). |
| **Calendar** | **objc2-event-kit 0.3.2**, https://crates.io/crates/objc2-event-kit, shaped after anarlog `crates/apple-calendar` | Zlib OR Apache-2.0 OR MIT **v** [L-14]; anarlog MIT **v** | **Adopt** the crate, **port** the shape: authorization, `requestFullAccessToEvents` (14+), predicate query, attendee count (which seeds diarization `num_clusters`). | new `fork/calendar.rs`, `Info.plist` (`NSCalendarsFullAccessUsageDescription`, `NSCalendarsUsageDescription`), `Entitlements.plist` (`com.apple.security.personal-information.calendars`) | E-07, E-42 | M | Calendar consent prompt timing. Full-access API is 14+ (older systems use the legacy request). |
| **Notifications** | **tauri-plugin-notification 2.4.0** (Rust + `@tauri-apps/plugin-notification` 2.4.x), https://github.com/tauri-apps/plugins-workspace | Apache-2.0 OR MIT **v** | **Adopt.** **Correction:** 2.5.0+ require `tauri ^2.12`; 2.4.0 is the newest that fits Tauri 2.11.5 **v** [V-02]. Alternative: ~100 lines on `objc2-user-notifications` 0.3.2 (already locked) if actions or UN APIs are needed. | `Cargo.toml`, `package.json`, `capabilities/` (fork file), `lib.rs` plugin chain | E-91, C-17 (system half), DEC-21 | S | The desktop backend uses notify-rust → mac-notification-sys (NSUserNotification, deprecated) (inf); `close` is a no-op on macOS. RT-30. |
| **Menus, capabilities, Dock** | Tauri 2.11.5 `Menu::default` + inserted submenus; capability `windows` globs; existing activation-policy code | Tauri MIT/Apache | **Write** a fork `app_menu.rs`: start from `Menu::default` (keeps Edit, so ⌘C/⌘V/⌘Z work), insert Dictation / My Voice / Help, `set_as_app_menu` **v** [C-22]. Fork windows under a `fork-*` prefix in a separate capability file (never `*`). Keep Handy's Dock policy and add `show_in_dock`. | new `fork/app_menu.rs`, `lib.rs` (one hook), `capabilities/fork.json`, `lib.rs:129-163` | AF-15, AF-16, A-63, A-65, C-75, C-203, E-12 | S | none |
| **Text pipeline** (dictionary, snippets, voice commands) | **aho-corasick 1.1.x** (1.1.4 already compiled via regex) + **unicode-segmentation 1.12** (already compiled) + own code; MIT references: SpeakoFlow `audio_toolkit/replacements.rs`, OpenWhispr `snippets.ts`, hyprwhspr-rs `src/text/normalize.rs` | aho-corasick Unlicense OR MIT **v** [L-17]; unicode-segmentation MIT/Apache; references MIT **v** | **Adopt** both crates as direct deps (no new compiled code). **Write** (§3): a newline-safe wrapper that runs Handy's existing stages per line (closes AF-06 without editing `text.rs`); exact "from → to" rules before fuzzy matching (Private Use Area placeholders); voice commands after ITN at clause edges; **snippet expansion last, after the LLM**. Each stage returns `(String, change_count)` for "fixes made". | one call site in `managers/transcription.rs:1600-1662` **v** [C-31]; `actions.rs` (snippets after post-processing); new `fork/text/{lines,rules,snippets,commands}.rs` | AF-06, D-48, D-53, D-56, D-210, C-52, C-63, C-72 | M | "the period of time" must stay text: clause-edge rule. Direct typing turns `\n` into Return (use Shift+Return or clipboard paste for chat apps). |
| **ITN** (spoken → written numbers, money, dates) | **text-processing-rs 0.2.2** (crates.io; repo 0.3.0), https://github.com/FluidInference/text-processing-rs + **text2num 2.8.0**, https://github.com/allo-media/text2num-rs | Apache-2.0 + NOTICE (NeMo-derived, NeMo Apache-2.0) **v** [L-15]; text2num MIT **v** [L-16] | **Adopt, later milestone, behind a setting, English first.** Run per line. Use only the number / money / date / time / measure taggers (its punctuation tagger turns a lone "period" into "." **v** [C-30]). Never use its global `custom_rules`. text2num covers it/pt/nl/da numbers. | `managers/transcription.rs` (one stage), `Cargo.toml` | D-14 | M | Gain is small if the default ASR already writes digits (RT-39). Needs an upstream feature flag or a thin fork to switch the punctuation tagger off. Ship the NOTICE. |
| **Word counts** | unicode-segmentation `unicode_words()` + CJK rule (Han/Kana: one per character; Thai/Lao/Khmer/Myanmar: characters ÷ 4 estimate) | MIT/Apache **v** | **Write** (S), computed in Rust at save time. Store `word_count` and `char_count` per entry so the rule can change later. | fork DB save path | DEC-11, C-30, C-31, C-212, B-29 | S | ICU4X segmenter (Unicode-3.0) only if Thai counts matter (open question). |
| **Diff** ("N Changes", fix counts) | **similar 3.2.1** (features `unicode`, `inline`), https://github.com/mitsuhiko/similar | Apache-2.0 **v** [L-18] | **Adopt.** Word diff in Rust at save time: store `fix_count`; return `[{tag, text}]` ops to the UI card. Use the `_deadline` variants for long texts. UI: `<del>`/`<ins>` with screen-reader prefixes (own code). | new `fork/text/diff.rs`, fork command module, `bindings.ts` (regenerate) | D-40, D-57, C-52, C-42 (with stored texts) | S | MSRV 1.85 (OK). |
| **Fork database, FTS, stats** | rusqlite 0.37 `bundled` (SQLite 3.50.2 with FTS5 + trigram) + **rusqlite_migration 2.3** (both already direct deps), separate `fork.db` | rusqlite MIT; rusqlite_migration Apache-2.0 **v** | **Reuse.** Separate `fork.db` with its own `user_version` and migrations: `daily_stats`, `entry_meta`, `dictionary_rules`, `snippets`, `notes`, `notifications`, `history_fts(trigram)` maintained by the app (no cross-DB triggers). One connection per DB: WAL, `busy_timeout=5000`, `synchronous=NORMAL`. | new `fork/db.rs`; hooks on save/delete in `managers/history.rs` | AF-07, AF-13, DEC-07, DEC-08, C-30..C-32, C-36, C-46, C-208, C-209, C-216 | S-M | Adding WAL to `history.db` changes a file upstream builds also open (backward-compatible, inf). Delete-all must remove the `-wal`/`-shm` files. |
| **Import / export** | **papaparse 5.7.0** (import preview), **csv 1.4** + **zip 4.x** (zip 4.6.1 already compiled) for export | papaparse MIT **v** [L-19]; csv Unlicense/MIT **v**; zip MIT **v** | **Adopt.** Parse CSV in the webview, preview and dedupe, send rows to Rust. Export zip of CSV/JSON + recordings + dictionary + snippets via `tauri-plugin-dialog`. Delete-all is our own command. | `package.json`, new import dialog, new `fork/export.rs` | C-64, C-65, C-71, D-50, D-55, E-66, E-67 | S (+M delete-all) | Prefix cells starting with `= + - @` (formula injection). Enforce the 3 MB cap ourselves. |
| **UI primitives** | **Radix Primitives** (`@radix-ui/react-dropdown-menu` 2.2.1, `-popover` 1.3.1, `-tabs` 1.2.1, `-context-menu`, `-collapsible`); keep **sonner** 2.0.x and **lucide-react** (present) | Radix MIT, peer React 18 OK **v** [L-24]; sonner MIT **v** [L-25]; lucide ISC | **Adopt** one package per need, styled with Handy tokens in `src/fork/ui/`. Keep Handy's own `Dialog` and `Tooltip`. sonner's action button gives "Deleted · Undo". shadcn only as a styling reference (React 19 drift). | `package.json`, `src/fork/ui/*` | C-13, C-14, C-17, C-41, C-44, C-50, C-59, C-201, D-18 | S each | none |
| **Charts** | **Recharts 3.10.1** (lazy-loaded Insights) + **react-activity-calendar 3.2.1** (streak heatmap) | both MIT **v** [L-21, L-22] | **Adopt.** Data straight from `daily_stats`. `html-to-image` (MIT) for "save as image" only when C-56 is built. | `package.json`, Insights page | C-31, C-32, C-50, C-54, C-55 | S | Bundle size: lazy-load. |
| **Virtual list** | **@tanstack/react-virtual 3.14.23** | MIT **v** [L-23] | **Adopt** on top of the existing cursor paging; variable heights via `measureElement`. | `src/components/settings/history/*` or the new home list | C-207 | S | none |
| **Tours / spotlight** | **driver.js 1.10.2** (in-window); native pill spotlight = own code | MIT **v** [L-20] | **Adopt** for in-window coach marks, passing `t()` strings for the button texts. Never pass user text (it sets `innerHTML`). Extend Handy's own `Demo.tsx` scene engine for animations (no Lottie or Rive: both fetch WASM from CDNs). Pill spotlight = a click-through dim NSPanel (write ourselves). | `package.json`, onboarding/Hub components, `fork/pill.rs` | B-40, B-44, B-45, D-43, C-29, A-16, DEC-15 | S (+M per scene) | Cannot spotlight native windows. |
| **Celebrations** | **canvas-confetti 1.9.4** | ISC **v** [L-26] | **Adopt** with `disableForReducedMotion`. | onboarding, streak milestones | B-27 | S | none |
| **Fonts** | **Newsreader** (display serif, `@fontsource-variable/newsreader` 5.3.0) + **Source Serif 4** (Cyrillic/Greek fallback, 5.3.1) + `system-ui` for the UI | OFL-1.1 **v** [L-27, L-28] | **Vendor** woff2 subsets through the existing `@font-face` pipeline and the `subset-font` devDependency. Never fetch fonts from the network. | `src/App.css`, `src/assets/fonts/` | C-19, C-21 | S | Ship the OFL texts in notices. |
| **i18n / a11y tooling** | Keep `eslint-plugin-i18next` + `scripts/check-translations.ts`; add **i18next-cli 1.80.5** (status/lint only, dev); **eslint-plugin-jsx-a11y 6.10.2** (dev); own `scripts/translate-missing.ts` | i18next-cli MIT **v**; jsx-a11y MIT **but pulls `axe-core` MPL-2.0** [L-38] | **Adopt (dev only).** Never use i18next-cli's locize commands (cloud). The translate script calls the local `llama-server`, enforces exact `{{placeholders}}`, and marks machine output for review. Fork keys stay under one namespace. | `package.json` (dev), `eslint.config.js`, `i18next.config.ts`, `scripts/` | E-80, E-81, C-210, AF-13 | S | MPL-2.0 dev dependency (owner decision, §8). |
| **Diarization** (Notetaker) | **sherpa-onnx 2.0.1** offline speaker diarization (Rust crate), https://github.com/k2-fsa/sherpa-onnx + **pyannote segmentation-3.0** (sherpa ONNX tarball) + **3D-Speaker CAM++ en VoxCeleb** (`3dspeaker_speech_campplus_sv_en_voxceleb_16k.onnx`, 29.6 MB) | sherpa-onnx Apache-2.0 **v** [L-29]; seg-3.0 MIT (tarball LICENSE, © 2022 CNRS) **v** [L-30]; CAM++ Apache-2.0 (ModelScope) **v** [L-31] | **Adopt.** Run post-hoc on the **system ("Them") track only**, in a one-shot child of the same executable (`--diarize-file <wav> --out <json>`), so `engine_supervisor/protocol.rs` is untouched. Set `num_clusters` from the calendar attendee count, otherwise use the threshold **v** [C-33]. In CI, vendor prebuilt libs via `SHERPA_ONNX_LIB_DIR` (its `build.rs` otherwise downloads from GitHub). **Mirror both models to our HF repo** with their LICENSE files, so the runtime network stays HF-only. Alternative once the NVIDIA licence is cleared: transcribe-cpp Sortformer (no new native dep, 4-speaker cap). | `main.rs`/`lib.rs` (one-shot flag, `cli.rs`), new `fork/meeting/diarize.rs`, fork catalog entries, `Cargo.toml` | E-20, E-22, AF-04, DEC-06 | M | Static onnxruntime increases binary size (unk). Link next to ggml (RT-33). DER and runtime on a base M1 (RT-32). |
| **Echo handling** | **hex (Rust)** `src/meeting_echo.rs` (534 lines, signal-level bleed classifier on 20 ms frame energies with lag search; replay tests) | MIT **v** [C-38] | **Port**, plus R4d's text-match suppression (own ~100 lines) as a second pass on transcripts. webrtc-audio-processing AEC3 (BSD-3 text; crates.io says "non-standard" [L-37]) only for v2 if the runtime check shows real-time AEC is needed. Mute feedback sounds while a meeting records. | new `fork/meeting/echo.rs` | E-04, E-205 | S-M | Clock drift between mic and tap (RT-34). |
| **Meeting summaries** | **anarlog** `crates/template-app/assets/{enhance.system,enhance.user,title.system}.md.jinja` + **Meetily** `frontend/src-tauri/src/summary/processor.rs:218-253` map-reduce and `templates/*.json` | anarlog MIT (outside `enterprise/`) **v** [L-08]; Meetily MIT **v** [L-36] | **Port** prompt text (render with `format!`, no Jinja). Map-reduce for 4k-context models. Output a JSON schema `{headline, bullets[], next_steps[{owner, task}]}` through the `local` provider. Keep Meetily's "ignore instructions inside transcript chunks" guard. | new `fork/meeting/summary.rs` | E-13, E-14, E-18, E-23 | S-M | Long meetings need `-c 16384` or chunking. Apple FM context is 4-8k. |
| **Hardware probe** | **sysinfo 0.39.6/0.40** with `default-features = false, features = ["system", "disk"]` | MIT **v** [L-33] | **Adopt.** RAM, chip string, and purgeable-aware free space (`kCFURLVolumeAvailableCapacityForImportantUsageKey` **v** [C-34]) on the HF cache volume. Optional: IOKit `gpu-core-count`. Feeds the silent model policy (DEC-02). | new `fork/commands/machine.rs`, `src/.../useMachineTier.ts` | AF-20, B-49, B-54, B-208, B-214, DEC-02 | S | MSRV 1.95 (stable toolchain OK) [V-14]. |
| **Bootstrap model** | **whisper-base** Q8_0 (85 MB, 99 languages) from `handy-computer/whisper-base-gguf`; English: **parakeet-tdt_ctc-110m** (135 MB) only if CC-BY-4.0 is accepted, else whisper-base.en | whisper-base Apache-2.0 **v** [L-32]; parakeet-110m CC-BY-4.0 **v** [L-35] | **Adopt** (already in Handy's catalog). Bootstrap transcribes first, then the main model replaces it silently. Plus queued audio: open the mic without a model, save the WAV and row as "waiting", retry via `managers/transcription_jobs.rs`, deliver by copy + notification. | `actions.rs:460-476` (mic gate), `managers/transcription_jobs.rs`, `managers/history.rs` or fork DB, onboarding gates | AF-03, B-50, B-202, B-206, DEC-01 | M | Time to first text on slow links (RT-37). |
| **Release pipeline** | **tauri-action `@v0`** (keep now, `@v1` later in a separate change), https://github.com/tauri-apps/tauri-action; fork overlay config | MIT **v** [L-36, V-08] | **Adopt.** Overlay `tauri.fork.conf.json` (productName, identifier, `version: "1.0.0"`, `minimumSystemVersion: "13.0"`, updater pubkey and endpoint, `externalBin`) passed via `--config`. Workflows read the version from the overlay. macOS-only matrix. Identity guard step. macOS verification job (codesign, spctl app + DMG, stapler, `--list-devices` smoke). | `.github/workflows/{release,build,main-build}.yml`, new `src-tauri/tauri.fork.conf.json` | AF-11, E-54, E-56, E-57, E-58, E-211..E-214, DEC-17 | S-M | v1 renames: `releaseAssetNamePattern`, `uploadUpdaterJson`. Keep `tagName` + `releaseId` both set. RT-35. |
| **Updater** | **tauri-plugin-updater 2.10.1** (already locked; do not bump: newer versions are not needed) | Apache-2.0 / MIT | **Keep.** Own minisign key with two offline backups. Change `UpdateChecker.tsx` from `downloadAndInstall` + immediate `relaunch()` **v** [C-35] to `download()`, then `install()` when no dictation or meeting is active. Move the toggle out of Debug and disclose the call (DEC-16). | `src/components/update-checker/UpdateChecker.tsx`, `settings.rs` default | E-53, E-57, E-75, E-215, AF-11, AF-12 | S | Losing the key strands every install. Same identifier and team keep permissions (RT-36). |
| **Distribution channels** | Own Homebrew tap at launch → **homebrew/cask** (self-submit at ≥ 225 stars / 90 forks / 90 watchers, or a third party at ≥ 75 / 30 / 30; repo ≥ 30 days) **v** [V-15]; **open-source-mac-os-apps** and **awesome-mac** PRs; optional MacUpdate | lists are CC0 (R4d); Homebrew BSD-2 | Cask with `auto_updates true` (so `brew upgrade` leaves it to our updater), `arch arm:/intel:`, `depends_on macos: :ventura`, `zap`. ~30-line tap-bump job on `release: published`. | new tap repo (owner action), `.github/workflows/tap-bump.yml` | E-217, DEC-22 | S | Gatekeeper checks mandatory in homebrew/cask (signed + notarized). Distinct product name. RT-38. |
| **Crash / diagnostics** | `std::panic::set_hook` + tauri-plugin-log (present) + macOS `~/Library/Logs/DiagnosticReports/*.ips` pickup; no crash service | Rust std; plugin Apache/MIT | **Write** (S each): panic hook writes `crash-<ts>.txt`; next launch offers "Copy report"; default file level Info; replace the settings dump (`settings.rs:1108` **v** [C-36]) with an allow-listed summary; mask `$HOME` and e-mails; "Copy diagnostics" / "Save diagnostics…" (zip) / prefilled GitHub issue link (nothing sent by the app). CI grep guard against analytics crates. | `lib.rs` (hook, log target), `settings.rs:656-658,1108`, new `fork/diagnostics.rs` | AF-18, E-73, E-78, E-79, C-103, DEC-14 | S | Unsymbolized frames (`strip = true`), which is acceptable. |
| **Upstream sync** | DIY `gh` workflow (weekly + manual merge PR with conflict list); optional `peter-evans/create-pull-request` (MIT) | own; MIT | **Write** ~50 lines: fetch upstream, merge into `sync/upstream-<date>`, open a PR with a checklist (regenerate `bindings.ts`, identity guard, `protocol.rs`, `settings.rs`, locales). Use a fine-grained PAT or App token so CI runs. Never give it signing secrets. | new `.github/workflows/upstream-sync.yml` | AF-13, E-219 | S | Workflow-file changes need a token with workflow scope. |

**Minimum macOS gates (stated once).**
- **App:** 13.0 (DEC-22; matches Homebrew's `:ventura`).
- **llama-server sidecar:** built for 13.0. Upstream release builds use 13.3, but ggml-metal gates newer APIs at runtime [V-13].
- **Process tap:** 14.4.
- **EventKit full access:** 14.0.
- **Apple FM:** 26.0, Apple Silicon only.
- **Metal LLM:** Apple Silicon only. Intel release builds are CPU-only.

---

## 3. Write ourselves (consolidated, with size estimates)

| # | Item | Size | Notes |
|---|---|---|---|
| W-01 | LLM supervisor: spawn, health, prewarm, idle, restart, CPU fallback, stale-PID kill, socket and key | ~300 lines Rust, M | SpeakoFlow and OpenWhispr as references |
| W-02 | Built-in `local` provider and request changes (temperature, max_tokens, timeout, neutral headers, socket) | ~150 lines, S | `settings.rs`, `llm_client.rs` |
| W-03 | Prompt builder + guard chain (diff/entity, length band, language) around the ported SpeakoFlow and FreeFlow checks | ~400 lines, M | plus tests on the adversarial set |
| W-04 | CI build of the static `llama-server` per arch + `externalBin` wiring | ~60 lines YAML/CMake, S | |
| W-05 | AX extensions on top of SpeakoFlow's layer (selection read/write, bounds, title, URL port) | ~300 lines, M | |
| W-06 | `HandyKeysState::new` error-path fix + wake re-check + health UI | ~120 lines, S | |
| W-07 | Clipboard multi-item snapshot/restore + ⌘C selection fallback | ~120 lines, S | |
| W-08 | Pill module (Tolkovo pattern), hover events, screen observer, pill spotlight panel | ~400 lines Rust + UI, M | |
| W-09 | Meeting glue: ported tap → resampler/VAD → WAV on disk; probe-tap permission check; text-match bleed pass | ~400 lines, M | |
| W-10 | Calendar module on objc2-event-kit | ~200 lines, S-M | |
| W-11 | App menu (`Menu::default` + submenus), fork capability file | ~120 lines, S | |
| W-12 | Newline-safe stage wrapper and line-safe whitespace collapse | ~80 lines, S | closes AF-06 |
| W-13 | Dictionary rules, snippet matcher (offset map, boundaries, case propagation), voice-command tables per language, in-utterance "scratch that" | ~600 lines + tables, M | |
| W-14 | Word-count rule, `daily_stats` upserts, streak and WPM math | ~150 lines, S | |
| W-15 | Fork DB schema and migrations, FTS maintenance, export zip, delete-all sequence | ~500 lines, M | |
| W-16 | Diff card UI and Undo AI edit toggle | ~200 lines TSX, S | |
| W-17 | Hub pieces: home, stats rail, banners, empty states, checklist card, sidebar groups | L (UI), from R2 | |
| W-18 | Typing test (B-29), time-saved slider (B-28) | ~250 lines, S | |
| W-19 | Diarization one-shot mode + speaker merge with transcripts | ~250 lines, M | |
| W-20 | Summary module (map-reduce + schema) | ~200 lines, S-M | |
| W-21 | Machine probe command + silent model policy | ~200 lines, S-M | |
| W-22 | Queued-audio first run | ~250 lines, M | |
| W-23 | Release overlay, identity guard, verification job, tap-bump job, upstream-sync job | ~250 lines YAML/shell, S-M | |
| W-24 | Panic hook, log redaction, diagnostics export | ~250 lines, S | |
| W-25 | `translate-missing.ts` | ~120 lines, S | |
| W-26 | Third-party notices generator: `cargo-about` (MIT OR Apache-2.0, 0.9.3) + a manual list for C/C++ (llama.cpp, onnxruntime, sherpa deps) + model and font attributions | ~100 lines config, S | |

Rough total for v1 dictation (W-01..W-08, W-11..W-16, W-21..W-26) is about 4,500 lines of Rust/TS plus UI. Notetaker (W-09, W-10, W-19, W-20) adds about 1,000.

---

## 4. Rejected (with reason)

| Item | Reason |
|---|---|
| Linking llama.cpp in-process (`llama-cpp-2`, own worker linking llama.cpp) | AF-01 duplicate ggml symbols. An own worker binary is the fallback only if packaging `llama-server` fails. |
| mistral.rs, MLX (Python or Swift), candle, kalosm, llamafile | No Intel build / Python runtime / own server code / APE not signable as a sidecar. MLX Swift: study for later. |
| Ollama as a dependency | Needs a separate install. It remains reachable through Handy's `custom` provider. |
| KoboldCpp, VoiceInk, Epicenter/Whispering, Tambourine, VoiceTypr, FluidVoice (current), TypeWhisper, MacParakeet | GPL/AGPL (licence only checked). |
| Qwen2.5-3B, Llama 3.2, Gemma 3, LFM2.5/LFM3, Qwen3.8 community | Research licence, custom terms or acceptable-use policy, revenue threshold, custom licence. |
| `selection` crate | GPL-3.0. |
| get-selected-text, audiotee | No LICENSE file in the repo. |
| xa11y, axuielement, accessibility-sys | Heavy, loses the secure-field signal / young Swift build step / legacy objc stack (constants may be copied). |
| active-win-pos-rs, x-win, frontmost, browser-url | Window titles need Screen Recording; immature. |
| Per-browser AppleScript, aw-watcher-window (MPL-2.0) | Automation prompts; copyleft (docs only). |
| mado as a dependency, or code from `builder-group/abstand` outside `crates/mado` | The repo is AGPL-3.0 by default; the crate carries Swift build steps; `WebsiteInfo.swift` makes network calls. |
| flexaudio-os-macos as a dependency | Exact pins on objc2/libc/block2. Alternative to the hex tap if needed (vendor, drop the pins). |
| screencapturekit-rs, cidre, ruhear | Screen Recording permission / second binding ecosystem. |
| clipboard-rs, tauri-plugin-clipboard (CrossCopy) | Flattens multi-item clipboards; stale. |
| rdev | Declared, unused (remove). |
| Hex legacy tap polling watchdog | Conflicts with handy-keys #1827 (kernel IPC voucher leak). |
| tauri-plugin-notification ≥ 2.5 | Needs Tauri ≥ 2.12 (use 2.4.0, or bump Tauri deliberately). |
| tauri-plugin-positioner | Nothing new. |
| sherpa-onnx ITN, NeMo Python, WeTextProcessing / wetext-rs | Second runtime / Python / extra FST downloads. |
| rapidfuzz-rs, chrono-english, dissimilar, imara-diff, diff-match-patch | Licence mismatch or stale / wrong tool or granularity. |
| refinery, tauri-plugin-sql (and remove the unused JS `@tauri-apps/plugin-sql`) | Second migration system; Handy left it. |
| shadcn CLI output, Base UI, React Aria, Ark, Headless UI, kbar | React 19 drift / heavier / narrower. |
| react-calendar-heatmap, @uiw/react-heat-map, nivo | Unmaintained or heavier. |
| Plate, Milkdown; TipTap / Lexical (only if rich snippets) | Licence audit per package / smaller ecosystems. Plain text in v1 (DEC-12). |
| shepherd.js, intro.js | AGPL-3.0 + commercial. |
| onborda, @stepperize/react, react-typing-game-hook | No LICENSE file / licence mismatch. |
| Lottie (lottie-web), dotLottie, Rive | Heavy; fetch WASM from CDNs by default; proprietary editors. |
| i18n-ai-translate, @inlang/cli, i18next-parser, i18next-scanner | GPL / no licence file + hosted service / archived. |
| pyannote-rs, sherpa-rs | Weak clustering, stale / archived. |
| pyannote.audio, diart, FluidAudio, MOSS-Transcribe-Diarize, granite-speech diarize, multitalker Parakeet | Python / Swift + scoped-CC-BY models / two languages / chunk-local speakers / NVIDIA licence. |
| anarlog `pyannote-local` + `embedding` + `aec` | WeSpeaker CC-BY weights without provenance; `ort` pre-release; internal crates (study only). |
| DTLN-aec, macOS voice-processing I/O | Weight provenance unrecorded / ducks audio, unknown with taps. |
| Meetily as code base, Meetily `hardware_detector.rs` | Ships `posthog-rs` analytics; RAM from an env var. |
| sentry-rust, crash-handler/minidumper, human-panic | Telemetry / overkill / CLI-only. |
| wei/pull, tgymnich/fork-sync, aormsby fork-sync | Third-party app with write access / stale / no PR. |
| Mac App Store, "Latest", MacUpdater | Sandbox breaks AX paste and key hooks / Sparkle-only / service ended. |
| SpeakoFlow runtime `llama-server` download | Downloads an executable at runtime with no checksum. Bundle a signed sidecar instead. |

---

## 5. Model and asset license register

"Default?" means the item may ship or be fetched silently without the user choosing it.

| Asset | Where from | License (verified) | Attribution / duty | Default? |
|---|---|---|---|---|
| Qwen3.5-2B (Q4_K_M GGUF) | `unsloth/Qwen3.5-2B-GGUF` (pinned rev + sha256) | Apache-2.0 **v** | Keep LICENSE; NOTICE list | **Yes** (≤ 8 GB tier) |
| Qwen3-4B-Instruct-2507 (Q4_K_M GGUF) | `unsloth/Qwen3-4B-Instruct-2507-GGUF` | Apache-2.0 **v** | as above | **Yes** (≥ 16 GB tier) |
| Qwen3.5-0.8B, Qwen3-1.7B, Qwen3.5-4B | Qwen / unsloth | Apache-2.0 (R4a; 0.8B/1.7B by card) | as above | Challenger / Intel opt-in |
| Muesli `qwen35-{0.8b,2b}-muesli-cleanup-v12-gguf` | Muesli-HQ on HF | Apache-2.0 (card) **v** | Training data undocumented | No (bake-off only) |
| SpeakoFlow Mini | `SpeakoFlow/speakoflow-mini` | Apache-2.0 **v**; dataset CC-BY-4.0 **v** | Credit the dataset | No (English only; bake-off) |
| Gemma 4 E2B/E4B | google | Apache-2.0 per card; prohibited-use page status (unk) | — | No until the prohibited-use question is closed |
| Phi-4-mini, SmolLM3, Granite 4.x, Ministral-3-3B | vendors | MIT / Apache-2.0 (R4a) | — | Challengers only |
| Qwen2.5-3B, Llama 3.2, Gemma 3, LFM2.5/LFM3, Qwen3.8 | — | Research / custom / AUP / revenue threshold | — | **Never** |
| Apple Foundation Models | macOS 26 system | Apple SDK terms | none (system API) | Yes, as an optional path |
| parakeet-unified-en-0.6b, parakeet-tdt-0.6b-v3 (Handy's defaults), parakeet-tdt_ctc-110m, canary-180m-flash, canary-1b-v2 | `handy-computer/*-gguf` | **CC-BY-4.0** (catalog; 110m **v**) | Attribution on an About/Acknowledgements page + NOTICE | Only if the owner accepts CC-BY |
| whisper-base / -base.en / small / medium / large-v3(-turbo) | `handy-computer/*-gguf` | Apache-2.0 **v** (base) | Keep LICENSE | **Yes** (whisper-base bootstrap) |
| Moonshine English tiny/base and all streaming | `handy-computer/*-gguf` | MIT **v** (upstream LICENSE) | Keep notice | Yes (fallback) |
| Moonshine `{tiny,base}-{ar,ja,ko,uk,vi,zh}` (12 models) | `handy-computer/*-gguf` | **Moonshine Community License, non-commercial** **v** (HF API + upstream LICENSE). Handy's `catalog.json` wrongly says `mit` **v** [L-34] | — | **Never as default**; hide or relabel; report upstream (owner) |
| nemotron-3.5, Fun-ASR, SenseVoiceSmall, medasr | catalog | `other` | Legal review | No |
| Sortformer 4spk v2.1, multitalker-parakeet | catalog / NVIDIA | **NVIDIA Open Model License** (R4d) | NOTICE "Licensed by NVIDIA Corporation…"; guardrail-termination clause; terms may change | No (until legal OK) |
| pyannote segmentation-3.0 (ONNX) | sherpa-onnx release tarball → mirror on our HF | MIT, © 2022 CNRS **v** | Keep LICENSE | **Yes** (diarization) |
| 3D-Speaker CAM++ en VoxCeleb (ONNX) | sherpa-onnx release → mirror on our HF | Apache-2.0 **v** (ModelScope) | Keep licence; courtesy credit VoxCeleb (CC BY 4.0 data) | **Yes** (diarization) |
| 3D-Speaker ERes2Net en VoxCeleb | as above | Apache-2.0 **v** | as above | Challenger |
| WeSpeaker ResNet34, NeMo TitaNet | HF | CC-BY-4.0 | Attribution | No (CC-BY decision) |
| reverb-diarization-v1 | sherpa | Non-commercial (R4d) | — | **Never** |
| Silero VAD v6.2 | Handy resources | MIT (catalog) | Keep notice | Yes (existing) |
| NeMo-derived ITN data inside text-processing-rs | crate | Apache-2.0 + NOTICE **v** | Ship the NOTICE | Yes (when ITN ships) |
| stopwords-iso | npm | MIT (R4c) | — | Yes (C-57) |
| Newsreader | Fontsource / productiontype | OFL-1.1 **v** | Ship OFL.txt | Yes |
| Source Serif 4 | Fontsource / adobe-fonts | OFL-1.1 **v** | Ship OFL.txt | Yes |
| Noto Sans CJK subset (`src/assets/fonts/cjk-subset.woff2`) | Handy | OFL-1.1 (inf, Noto) | Ship OFL.txt | Yes (existing) |
| Instrument Serif, Inter | Google Fonts | OFL-1.1 (R4c, not re-checked) | Ship OFL.txt | Optional |
| Disfl-QA, AMI-disfluent, DisfluencySpeech | eval only | CC BY 4.0 / CC BY 4.0, Apache-2.0 / Apache-2.0 | Credit in the eval repo | Never shipped |
| PodcastFillers, Bhasik, CNRS Paroles | eval only | "cc" unknown variant / CC BY-ND / CC BY-NC-SA | — | Do not use (except Bhasik as unmodified eval) |

---

## 6. Runtime checks and benchmarks on a real Mac

Merged from R4a (B-1..B-12), R4b (RC-xx), R4c and R4d, with duplicates removed. Test machines: base M1/M2 with 8 GB, an M-series Mac with 16 GB or more, one Intel Mac, and macOS 13, 14, 15 and 26.

| ID | Check | Gates (parts) | Source ids |
|---|---|---|---|
| RT-01 | `llama-server` cold start to `/health` 200 (spawn + Metal compile + mmap); second start | LLM runtime | B-1 |
| RT-02 | Prefill and generation tokens/s per candidate model at Q4_K_M / Q4_0, ctx 4096 | LLM models | B-2 |
| RT-03 | Peak RSS of `llama-server` per model with `-c 4096 --cache-ram 256`; headroom on 8 GB with the ASR model loaded | LLM runtime, models, hardware policy | B-3, RC-11 |
| RT-04 | Cleanup bake-off on Disfl-QA + AMI + our ~200-item adversarial set: answer rate, meaning drift, fillers, language kept, per model × prompt (OpenWhispr / FreeFlow / SpeakoFlow / trimmed), including the Muesli and SpeakoFlow fine-tunes | models, prompts, guards | B-4 |
| RT-05 | JSON-schema vs plain output: quality, speed, cost of large `maxLength` grammars | guards | B-5 |
| RT-06 | Prefix-cache reuse for hybrid Qwen3.5 with default vs `-cms 0` | LLM runtime | B-6 |
| RT-07 | Apple FM latency (cold, prewarmed), refusal rate (default vs permissive), `rateLimited` for a background app, real `contextSize` on 26.x | Apple FM bridge | B-7 |
| RT-08 | End-to-end key-up → paste with cleanup on, p50/p95 vs the deadline | runtime, prompts, guards | B-8 |
| RT-09 | Intel CPU-only speed with Qwen3.5-0.8B/2B: does Intel get cleanup at all? | models | B-9 |
| RT-10 | Static `llama-server` built for 13.0: signs and notarizes as `externalBin` with Handy's entitlements, launches under Gatekeeper on a clean Mac, runs on macOS 13 | LLM runtime, release | B-10, V-13 |
| RT-11 | GPU contention: cleanup while transcribe-cpp Metal is busy (next dictation or a Notetaker stream) | LLM runtime | B-11 |
| RT-12 | Esc during generation frees the slot within 100 ms (connection-close abort) | supervisor | B-12 |
| RT-13 | Pill: first click on ✕ with `accept_first_mouse(true)` in the never-key panel. With nspanel 2.1.1, is Tolkovo's `_setPreventsActivation:` still needed? | pill | RC-01 |
| RT-14 | Hover: CSS `:hover` vs a tracking area with `active_always` | pill | RC-03 |
| RT-15 | Click-through toggling latency (only if anarlog polling is used) | pill | RC-21 |
| RT-16 | `set_content_protected` hides the pill from ScreenCaptureKit recorders (Zoom share, QuickTime, OBS) | pill | RC-17 |
| RT-17 | Idle CPU and memory of an always-visible overlay webview over hours (WebKit growth #1279); plan B is anarlog's native Swift pill | pill | RC-10 |
| RT-18 | fn/Globe: does blocking the fn press stop the Globe action at the session tap vs the HID tap, for each "Press 🌐 key to" value and the F5 dictation key, on macOS 14/15/26? | hotkeys | RC-02, RC-15 |
| RT-19 | After sleep/wake and after Accessibility is toggled or removed: does the handy-keys tap still deliver? Do axwatch notifications arrive in a signed build? Does `AXIsProcessTrusted` lie after the row is removed? | hotkeys / tap health | RC-14 |
| RT-20 | AX matrix in Slack, VS Code, Discord, Notion, Chrome, Arc, Safari, Mail, Word: focused element, subrole, selection, range, bounds, with and without the lease; latency at a 0.1 / 0.25 s timeout | AX client | RC-04 |
| RT-21 | Does setting `AXSelectedText` really replace text in Chromium/Electron editors? | AX client (Transforms) | RC-04b |
| RT-22 | Browser URL via AX in Chrome, Arc, Brave, Edge, Safari, Firefox; side effects of the lease on window managers | frontmost / URL | RC-04c |
| RT-23 | `frontmostApplication` freshness off the main thread vs AX `AXFocusedApplication` | frontmost | RC-20 |
| RT-24 | Clipboard restore with a Transient marker: do Maccy, Paste and Raycast skip it? Are RTF, HTML, file URLs and multi-item content byte-identical? | clipboard | RC-16 |
| RT-25 | Process tap: TCC prompt and pane; whether the probe tap reports "granted" without a restart; zero samples when denied | system audio | RC-08 |
| RT-26 | Global exclude-self tap follows an AirPods connect or disconnect mid-call | system audio | RC-08b |
| RT-27 | Excluding our PID before we have an audio process object | system audio | RC-08c |
| RT-28 | Mute-while-dictating vs the global tap | system audio | RC-08d |
| RT-29 | Process-object properties on 14.0-14.3; reliability for Zoom, Teams, Meet in Chrome, FaceTime | call detection | RC-18 |
| RT-30 | tauri-plugin-notification 2.4.0 on macOS 15/26: delivery from a signed bundle, click callback, permission timing | notifications | RC-19 |
| RT-31 | ASR speed and RSS of the default candidates on a base M1/M2 8 GB, Metal and CPU, with the LLM sidecar loaded; reload after the 5-minute unload | hardware policy, bootstrap | R4d-B1, RC-09 |
| RT-32 | Diarization DER (seg-3.0 + CAM++ vs ERes2Net vs Sortformer) on 3-5 real system-track recordings; runtime and RSS for 60 minutes on a base M1 | diarization | R4d-B2 |
| RT-33 | sherpa-onnx static link next to transcribe-cpp/ggml: duplicate symbols, binary-size delta | diarization | R4d-B3 |
| RT-34 | How often the mic picks up the call on laptop speakers; is hex `meeting_echo` + text match enough, or is AEC3 needed? | echo | R4d-B4 |
| RT-35 | Does Tauri notarize and staple the DMG, or only the `.app`? | release | R4d-B5, RC-12 |
| RT-36 | Updater 1.0.0 → 1.0.1 keeps TCC permissions (same identifier and team); install-when-idle works | updater | R4d-B6, RC-12 |
| RT-37 | Bootstrap time to first text at 10 and 50 Mbit/s (whisper-base vs parakeet-110m) | bootstrap | R4d-B7 |
| RT-38 | `brew audit --cask --strict --online` / `--new` passes (Gatekeeper check) | distribution | R4d-B8 |
| RT-39 | Do the default ASR models emit spoken-form numbers in 20 sample dictations? (Decides whether ITN is worth shipping.) | ITN | R4c open point |

---

## 7. Ship checklist: first signed, notarized, auto-updating build

**Owner decisions needed first:**
- DEC-17: product name, bundle identifier, Apple team.
- DEC-22: minimum macOS (recommended 13.0) and whether to support Intel.
- DEC-16: update-check default.
- CC-BY acceptance, which decides the English default model.

**A. Apple and keys (once)**
1. Create a **Developer ID Application** certificate. Export it as `.p12` with a password and base64 it.
2. Create an App Store Connect API key (or an app-specific password for the Apple ID).
3. Generate the updater key: `bun run tauri signer generate -- -w ~/.tauri/<app>.key`, with a password. Back it up in two offline places. Never commit it.
4. Add the repo secrets:
   - `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `KEYCHAIN_PASSWORD`, `APPLE_TEAM_ID`;
   - either `APPLE_API_ISSUER` + `APPLE_API_KEY` + a step that writes the `.p8` to `APPLE_API_KEY_PATH`, or `APPLE_ID` + `APPLE_PASSWORD`;
   - `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

   Leave all Azure secrets unset.

**B. Repo changes (fork-owned files where possible)**

5. Add `src-tauri/tauri.fork.conf.json` with: `productName`, `identifier`, `version: "1.0.0"`, `bundle.macOS.minimumSystemVersion: "13.0"`, and `plugins.updater` with our `pubkey` and our `releases/latest/download/latest.json` endpoint.
6. Edit `release.yml` and `build.yml`:
   - read the version from the overlay;
   - pass `--config src-tauri/tauri.fork.conf.json` in every macOS `args`;
   - trim the matrix to `macos-26`/aarch64 (+ `macos-latest`/x86_64 if Intel is supported);
   - keep `tagName` + `releaseId`;
   - stay on `tauri-action@v0`.
7. Add the **identity guard** step: fail if `cjpais/Handy`, the upstream pubkey prefix or `com.pais.handy` appear in the effective config.
8. Add the **macOS verification job**: `codesign --verify --deep --strict`, `spctl -a -vvv -t exec` on the app, `spctl … -t open --context context:primary-signature` on the DMG, `xcrun stapler validate`, and a `--list-devices` smoke test.
9. If the LLM ships in this build:
   - add the `llama-server` CI build (13.0 target, Metal embedded, TLS off) and `externalBin`;
   - confirm the job in step 8 also verifies `Contents/MacOS/llama-server-*` (RT-10).
10. Rebrand strings and icons minimally. Set `NSMicrophoneUsageDescription` copy with the new name. Keep the entitlements. Add `NSAudioCaptureUsageDescription`, the calendars usage strings and the entitlement **only** when Notetaker ships.
11. Move the update toggle out of Debug, set its default per DEC-16, and disclose the GitHub call on the privacy page. Switch to `download()` then `install()` when idle.
12. Generate **third-party notices**: `cargo-about` for crates, an npm licence list, a manual list for llama.cpp / onnxruntime / sherpa C++ deps, the Apache NOTICE files (text-processing-rs, any others), the OFL texts, and the model attributions (CC-BY Parakeet if used). Show them on an About/Acknowledgements page.
13. Hide or relabel the 12 non-English Moonshine models in our catalog view (non-commercial). Exclude `other`/NVIDIA models from silent defaults.
14. Add the CI grep guard against analytics crates. Remove `rdev` and the unused `@tauri-apps/plugin-sql`.
15. Retarget `main-build.yml` to the fork branch. Add the upstream-sync workflow (PAT or App token, no signing secrets).

**C. First release**

16. Run **Release**. Check the draft for: two DMGs, two `.app.tar.gz` + `.sig`, and one `latest.json` with `darwin-aarch64` (+ `darwin-x86_64`) pointing at our repo.
17. Clean-Mac test (or a second user account): the DMG opens without warnings; `spctl -a -vvv` says "Notarized Developer ID"; grant Mic + Accessibility; dictate; run cleanup if it is included.
18. Publish the draft (not as a prerelease). `…/releases/latest/download/latest.json` must resolve.
19. **Updater dry run:** release 1.0.1. The 1.0.0 install must offer and apply it and keep its permissions (RT-36).

**D. Homebrew and listings**

20. Create the `<owner>/homebrew-tap` repo with `Casks/<app>.rb`: `arch arm:/intel:`, `auto_updates true`, `depends_on macos: :ventura`, and `zap` paths under our identifier (never `~/.cache/huggingface`). Test `brew install --cask`, `brew audit --cask --strict --online`, `brew uninstall --zap`.
21. Add a tap-bump job on `release: published` (sha256 of both DMGs; PAT scoped to the tap).
22. Open PRs to **open-source-mac-os-apps** (`applications.json`; categories `audio`/`productivity`; English README; licence present) and **awesome-mac** ("Voice-to-Text", OSS + Freeware badges).
23. When the repo is at least 30 days old and has ≥ 225 stars (or 90 forks/watchers), self-submit to **homebrew/cask**. After merge, BrewTestBot autobumps and Applite lists it.
24. Optional: a MacUpdate listing with the direct `releases/download/v<ver>/…dmg` URL.

---

## 8. Open questions

1. **CC-BY-4.0 weights:** are they acceptable? Handy's default Parakeets, parakeet-110m and canary are CC-BY. This decides the English default and the bootstrap model. If not, use whisper / Moonshine-English / Qwen3-ASR (Apache/MIT).
2. **NVIDIA Open Model License** (Sortformer, multitalker): acceptable at all? If yes, Sortformer becomes the zero-dependency diarization alternative.
3. **MPL-2.0 dev tools** (`axe-core` through eslint-plugin-jsx-a11y, `@axe-core/playwright`): allowed in dev dependencies? They never ship.
4. **Minimum macOS:** 13.0 with the sidecar built for 13.0 (RT-10), or raise to 13.3 / 14.4 to simplify gates (system audio needs 14.4 anyway).
5. **Model hosting:** do we mirror pinned GGUFs and the sherpa ONNX files in our own HF org (one network host, our own sha pins) or fetch from unsloth/GitHub? The recommendation is to mirror (AF-12).
6. **Fine-tuned cleanup models** (Muesli, SpeakoFlow Mini): ask the authors for training-data provenance before either could become a default.
7. **Gemma 4:** is Google's prohibited-use page incorporated into the Gemma 4 licence? Until answered, it stays a challenger only.
8. **Private APIs** (`_setPreventsActivation:` from Tolkovo; any private TCC preflight): accept them behind feature checks, or require the public route?
9. **Upstream contributions:** report the Moonshine licence mislabel to `@handy-computer/models`; propose a health getter for handy-keys; propose a feature flag in text-processing-rs to disable the punctuation tagger. All are owner actions.
10. **Intel:** is cleanup offered at all (RT-09), and do we build Intel at all, given Homebrew moves Intel to Tier 3 from Sept 2026?
11. **mado provenance:** porting from the MIT 0.0.21 crate is clean. Re-check the crate licence if we ever update the port, because the upstream repo is AGPL by default.
