# R3 verification log

Verifier pass over `R3a-web-stack-peers.md`, `R3b-native-mac-peers.md` and `R3c-commercial.md`, date **2026-10-28**. Methods: `gh api repos/<o>/<r>` (health and license), LICENSE files read in the clones under `…/scratchpad/clones/r3a-*`, `r3b-*`, or via `gh api …/contents/<license>`; `rg` for GPL markers and provenance notes; HF API `/api/models/<id>`; WebFetch of vendor pages. Nothing was built, installed or run. GPL/AGPL projects: only LICENSE, README and docs were touched. Result: **P** = pass, **F** = fail (corrected), **P*** = pass with a minor correction.

## 1. Licenses

| # | Project | Check | Result | Correction |
|---|---|---|---|---|
| L1 | SpeakoFlow | `LICENSE` in clone: MIT, © 2026 Abhishek Barali / © 2025 CJ Pais; GPL-marker scan of the tree: none | P | – |
| L2 | OpenWhispr | `LICENSE`: MIT © 2024 OpenWhispr Team; `resources/mediaremote-adapter/LICENSE`: BSD-3-Clause (R3a: "not read") | P | Added the BSD-3 label |
| L3 | OpenFlow-T (avijeett007) | MIT © 2025 CJ Pais | P | – |
| L4 | Tolkovo | MIT © 2025 CJ Pais / © 2026 kozliq | P | – |
| L5 | klava-nevinovata | MIT © 2025 CJ Pais | P | – |
| L6 | AivoRelay | MIT, but `THIRD_PARTY_LICENSES.md` + `src-tauri/resources/licenses/` show LAME (LGPL-2), `mp3lame-sys`/`mp3lame-encoder` (LGPL) and GPL-3.0 text built into the app | F (mixed-license trap not reported) | Added a trap; nothing to port from the MP3 export path |
| L7 | Parler | MIT © 2025 CJ Pais | P | – |
| L8 | Whispering / epicenter | API NOASSERTION; root LICENSE is an index: every package under `packages/` and `apps/` is AGPL-3.0-or-later | P | – |
| L9 | Tambourine | API AGPL-3.0 | P | – |
| L10 | Voquill | `LICENCE`: enterprise directory under the enterprise license, the rest AGPLv3 | P | – |
| L11 | dictation-cleanup-rules | API MIT | P | – |
| L12 | BetterWispr | Apache-2.0 confirmed. `docs/oss-reuse.md` read in full: VoiceInk GPL-3.0 adaptations in 11 files. R3b listed 9 and missed `Sources/BetterWispr/App/AppModel.swift` (speech-model warm-up adapted) and `Speech/WarmUpClip.swift`. SystemAudioTap = Muesli MIT + AudioCap BSD-2 (confirmed); VoiceCommands = Scribe MIT (confirmed); CorrectionLearner = OpenWhispr MIT (confirmed). Two files carry license markers in the tree (`WordAgreementEngine.swift`, `CorrectionReviewer.swift`). | F | GPL list extended to 11; R3b citations into `AppModel.swift` (cleanup levels, selection replace) downgraded to idea only |
| L13 | Hex legacy | MIT © 2025 Kit Langton | P | – |
| L14 | hex (Rust) | MIT © 2026 Christopher Langton; provenance scan: `dot_scope.rs` ported from OpenCode (MIT), Rune capsule from Scrawl (not in our scope) | P | Note: uses a git-patched `transcribe-cpp-sys` fork |
| L15 | OpenSuperWhisper | MIT © 2024 OpenSuperWhisper | P | – |
| L16 | Muesli | MIT © 2026 Pranav Hari; NOTICE lists Apache-2.0 FluidAudio parts and AIElementsSwift (Apache-2.0) | P | – |
| L17 | Yap | MIT © 2026 Frigade, Inc. | P | – |
| L18 | open-flow (Robj1925) | MIT © 2026 Rob J | P | – |
| L19 | Talkify | MIT © 2026 Tornike Gomareli; Siri wave view ported from an MIT project | P | – |
| L20 | AudioWhisper | MIT © 2024 AudioWhisper Contributors | P | – |
| L21 | VoiceInk | API NOASSERTION; LICENSE file = GNU GPL v3 | P | – |
| L22 | MacParakeet | API NOASSERTION; LICENSE = GPL v3 with a MacParakeet header | P | – |
| L23 | FluidVoice | API GPL-3.0; LICENSE at tag `v1.5.8` = Apache-2.0; release v1.5.8 published 2026-02-18 | P | Upgraded from (inf) to [verified] |
| L24 | TypeWhisper | API GPL-3.0 | P | – |
| L25 | Model: SpeakoFlow Mini | HF `license:apache-2.0`, base Qwen3.5-0.8B; dataset `SpeakoFlow/dictation-cleanup-examples` `license:cc-by-4.0` | P | – |
| L26 | Model: Muesli Qwen 0.8B / 2B GGUF | HF `license:apache-2.0` (both) | P | – |
| L27 | Model: Muesli LFM 1.2B | HF `license:other` | P | – |
| L28 | Model: Muesli Gemma 4 E2B LiteRT | HF `license:apache-2.0`; not mentioned by R3b | P* | Noted as a fourth Muesli cleanup model (LiteRT format, not GGUF) |
| L29 | Model: VoiceInk-Refine-Mini | HF `license:apache-2.0`, base Qwen3.5-0.8B, lastModified 2026-10-28 | P | – |
| L30 | Model: FluidIntelligence | HF `license:other` | P | – |
| L31 | Model: unsloth Qwen3.5-2B GGUF | HF `license:apache-2.0` | P | – |
| L32 | Model: pyannote segmentation-3.0 | HF `license:mit`, gated=auto | P | R3a's "MIT (inf)" and "gated (inf)" upgraded to verified |
| L33 | Model: WeSpeaker ResNet34-LM | HF `license:cc-by-4.0` | P | – |

## 2. Health (GitHub API, 2026-10-28)

All matched the working files (stars within ±2, same push dates). Archived status re-checked: only kitlangton/Hex is archived.

| Repo | Stars | Forks | Last push | Archived | API license |
|---|---|---|---|---|---|
| cjpais/Handy | 35,010 | 3,328 | 2026-10-27 | no | MIT |
| AbhishekBarali/SpeakoFlow | 373 | 40 | 2026-10-25 | no | MIT |
| OpenWhispr/openwhispr | 10,251 | 1,315 | 2026-10-28 | no | MIT |
| avijeett007/openflow | 15 | 4 | 2026-10-12 | no | MIT |
| kozliq/Tolkovo | 12 | 2 | 2026-10-25 | no | MIT |
| egsok/klava-nevinovata | 37 | 4 | 2026-10-11 | no | MIT |
| MaxITService/AIVORelay | 59 | 3 | 2026-10-26 | no | MIT |
| Melvynx/Parler | 236 | 88 | 2026-06-18 | no | MIT |
| EpicenterHQ/epicenter | 4,827 | 385 | 2026-10-28 | no | NOASSERTION (AGPL index) |
| kstonekuan/tambourine-voice | 391 | 72 | 2026-07-17 | no | AGPL-3.0 |
| voquill/voquill | 1,022 | 130 | 2026-09-26 | no | NOASSERTION (AGPL + enterprise) |
| AbhishekBarali/dictation-cleanup-rules | 1 | 0 | 2026-08-29 | no | MIT |
| opennookorg/betterwispr | 469 | 38 | 2026-10-24 | no | Apache-2.0 |
| kitlangton/Hex | 2,898 | 230 | 2026-10-14 | **yes** | MIT |
| anomalyco/hex | 636 | 81 | 2026-10-28 | no | MIT |
| Starmel/OpenSuperWhisper | 3,096 | 282 | 2026-10-20 | no | MIT |
| Muesli-HQ/muesli | 1,490 | 152 | 2026-10-28 | no | MIT |
| FrigadeHQ/yap | 462 | 33 | 2026-10-16 | no | MIT |
| Robj1925/open-flow | 6 | 2 | 2026-10-19 | no | MIT |
| tornikegomareli/Talkify | 585 | 41 | 2026-10-17 | no | MIT |
| mazdak/AudioWhisper | 262 | 30 | 2026-02-21 | no | MIT |
| Beingpax/VoiceInk | 6,960 | 979 | 2026-10-28 | no | NOASSERTION (GPL-3.0 file) |
| altic-dev/FluidVoice | 12,582 | 916 | 2026-10-27 | no | GPL-3.0 |
| TypeWhisper/typewhisper-mac | 1,887 | 151 | 2026-10-28 | no | GPL-3.0 |
| moona3k/macparakeet | 741 | 90 | 2026-10-23 | no | NOASSERTION (GPL-3.0 file) |

## 3. Code claims (opened in the clones)

| # | Claim (file) | Result | Note |
|---|---|---|---|
| C1 | SF `managers/local_llm.rs`: llama-server child process; resolution env → bundled → app-data auto-download → PATH | P | – |
| C2 | SF cleanup port 11436, context 4,096 | P | `:41`, `:48` |
| C3 | SF pinned llama.cpp `b11429` from ggml-org releases | P | `:92`, `:488` |
| C4 | SF no checksum or signature for the downloaded engine | P | No hash/digest/signature near the download; only `HashSet` uses |
| C5 | SF validators: channel names `final/analysis/commentary`, prompt echo, punctuation-only, empty, length RATIO 3 / FLOOR 80 | P | `actions.rs:1361-1437`, `:1507` |
| C6 | SF JSON schema field `cleaned_text` | P | `actions.rs:244` |
| C7 | SF prewarm of the cleanup LLM during recording | P | `actions.rs:590`, call `:2840` |
| C8 | SF temperature pinned to 0 | P | comments `:309`, `:1135` |
| C9 | SF AX layer: one thread, 0.2 s timeout, AXManualAccessibility lease 60 s, never AXEnhancedUserInterface, secure and terminal exclusions | P | `smart_spacing/macos.rs:4-46` |
| C10 | SF selection: sentinel on macOS, 250 ms budget, bail on non-text | P | `selection.rs:33-74` |
| C11 | SF hook failure only logged (A-208 still present) | P | `shortcut/handy_keys.rs:148-155` |
| C12 | SF hotkey refused + progress on pill during download | P | `speech_readiness.rs:1-21` |
| C13 | OW Globe: `TISUpdateFnUsageType` via dlsym, `AppleFnUsageType`, crash marker, tap re-enable on timeout, `FN_INTERRUPTED` | P | `macos-globe-listener.swift:60-99,263,331` |
| C14 | OW Globe suppression enabled automatically when a Globe hotkey is bound (no consent) | P (new finding) | `hotkeyManager.js:676-694` |
| C15 | OW defaults `cleanupProvider "openai"`, `transcriptionMode "openwhispr"`, `floatingIconAutoHide false` | P | `settingsStore.ts:1699,1783,1859-1863` |
| C16 | OW retention audio 30 d, transcripts forever | P | `retentionSettings.js:4-6` |
| C17 | OW editable-target roles and secure-subrole exclusion; AXURL host walk | P | `macos-text-monitor.swift:72-77,211` |
| C18 | OW browser judged by AX host, else site name ending the window title; tone suffix "Restyle only…" and "never changes meaning" | P | `toneAppCategories.js:12-14`; `prompts/toneStyle.ts:27,37` |
| C19 | OW llama-server ports 8221-8240, 5-min idle unload, ctx 4,096 | P | `llamaServer.js:15-24` |
| C20 | TK pill: Status level, `hides_on_deactivate(false)`, nonactivating, `focusable(false)`, `accept_first_mouse(true)`, `_setPreventsActivation:`, Space-change notification, click command | P | `pill.rs:189-255,479-527,736` |
| C21 | TK product settings: post-process off, update checks off, no history | P | `mod.rs:20-42` |
| C22 | OpenFlow-T window title via `osascript` System Events; frontmost via NSWorkspace | P | `active_app.rs:47-59` |
| C23 | OpenFlow-T per-process tap, 14.4+, mic-only fallback | P | `meeting/capture.rs:8-52` |
| C24 | OpenFlow-T README: ad-hoc signed, re-prompts after auto-update | P | README `:79-84` |
| C25 | klava three-thread heartbeat watchdog | P | `watchdog.rs:1-20` |
| C26 | Hex legacy 100 ms watchdog revives disabled tap; wake/session rebuild | P | `KeyEventMonitorClient.swift:117,393-395,551-552` |
| C27 | Hex legacy 48 HotKeyProcessor tests | P | `HotKeyProcessorTests.swift` 48 matches |
| C28 | hex Chromium 155/156 detection via framework version; Transient/AutoGenerated markers | P | `paste.rs:92-95,362-382,523-524` |
| C29 | hex chord breaker keycode 0x44 | P | `keyboard.rs:34` |
| C30 | hex leaves Globe's own action to the system | P* | Doc row `dictation.md:136` and test comment `suppression.rs:4468`; R3b's citation `dictation.md:136` is a table row, not a policy statement, but the meaning holds |
| C31 | hex Apple FM bridge: temp 0.2, max tokens 60/220, context-overflow retry, semaphore timeout with detached task, `@_cdecl`, weak-linked | P | `apple_intelligence.swift:78-194`; `build.rs:51-59` |
| C32 | hex per-language table on `handy-computer/transcribe.cpp` Q8_0 GGUFs; transcribe-cpp =0.4.1 | P* | `language-recommendations.md:7`; Cargo also patches `transcribe-cpp-sys` from a fork (added) |
| C33 | Muesli TelemetryDeck initialized; enabled when the build injects an app ID; no user toggle found | P | `AppDelegate.swift:18-24`; `build_native_app.sh:19-20`; settings only send signals |
| C34 | Muesli pill `hitTest`, `acceptsFirstMouse`, tracking area | P | `FloatingIndicatorController.swift:32-67` |
| C35 | Muesli URL via `kAXDocumentAttribute` | P | `ScreenContextCapture.swift:248,285` |
| C36 | Muesli cleanup guard rejects assistant markers | P | `Qwen3PostProcessor.swift:119-129` |
| C37 | Muesli cleanup model list (R3b: 0.8B, 2B, LFM) | P* | Also a Gemma 4 E2B LiteRT model (added) |
| C38 | Yap `isFaithful` ≥ 0.6, greedy, 6,000-char cap, "never instructions" framing | P | `TranscriptCleanupService.swift:24,41,116,144-155` |
| C39 | Yap F5 / keycode 176 consumed by an active tap; Secure Input PID from IORegistry | P | `FunctionKeyMonitor.swift:28-32,101,163`; `SecureInput.swift:28` |
| C40 | open-flow `needsCleanup` gate; `max(96, chars/2)` tokens; deadline `min(…, 2+0.03×words)` | P | `LLMCleanerStage.swift:19-28`; `AppleFoundationCleaner.swift:97,108` |
| C41 | open-flow listen-only tap, wake rebuild, `tapIsEnabled` re-enable | P | `EventTap.swift:62,78,107-111` |
| C42 | Talkify 10 s default timeout with cancel; Sparkle `isBusy` deferral | P | `PromptShapingService.swift:24,61`; `SparkleUpdaterService.swift:32,83` |
| C43 | BetterWispr capsule `canBecomeKey=false`, nonactivating, `acceptsFirstMouse`, tracking area | P | `CapsuleController.swift:6,66,74,100` (file not on the GPL list) |
| C44 | OpenSuperWhisper bundled `ggml-tiny.en.bin` copied on first launch | P* | `WhisperModelManager.swift:91-112`; file is 77.7 MB in the repo root (R3b: 75 MB) |

44 code claims checked: 44 pass (5 with minor notes). None failed outright.

## 4. Wispr and competitor documentation (re-fetched 2026-10-28)

| # | Page | R3c claim | Result | Correction |
|---|---|---|---|---|
| D1 | 6409258247 starting-your-first-dictation | <0.5 s tap does not paste; double press within 0.5 s; third press cancels; ~7 s restore in Chrome; 14-day audio removal | P | – |
| D2 | same | Click keeps "the target field selected" | P* | Exact wording is "keeping the intended text field selected" |
| D3 | same | Mac does not restore files, PDFs, RTFD, sound (cited W1, W2, W19) | F (citation/wording) | Found on W1 (6391241694): "file contents, PDF, RTFD and audio" |
| D4 | same | Free plan 2,000 words/week (cited W26, W2, W1) | P* | Figure only on the pricing page; W2 gives mobile caps only |
| D5 | 1790396454 Flow Bar | Cursor display; hidden until setup ends; menu bar/Dock clearance; flush in full screen; Hide for 1 hour; capture behavior | P | – |
| D6 | same | Drag to edges | P* | Not on this page; the source is the changelog (R3c also cited W39) |
| D7 | 7971211038 paste failures | Clipboard paste; "Select a textbox first, then dictate"; ⌃⌘V/⌃⌘C; clipboard managers skip | P | – |
| D8 | 1621472516 non-QWERTY | Copy replaces the clipboard, not restored; empty transcript message; layout-following paste, restart | P | – |
| D9 | 4283510616 auto cleanup | None/Light (default)/Medium; High retired; AI apps skip; "never lost"; 2,000 words for Voice Profile | P | – |
| D10 | 5373093536 smart formatting | Voice commands regardless of Smart Formatting; backtrack; trailing-period rule; Undo AI edit copies and does not paste | P | – |
| D11 | 4678293671 context awareness | On by default; no screenshots; exclusions; cleared at end; app name + domain sent when off | P | – |
| D12 | 9967754953 auto-update | Auto default; ≥20 min after the last dictation; not during dictation/recording; quiet relaunch; 24 h pause after 3 failures | P | – |
| D13 | 5784437944 snippets | Alone/inline rules; case; longest wins; personal beats team; 60/4,000 limits; no dynamic variables; possessives vs contractions | P | – |
| D14 | 8841649969 secure input | fn hold works; Fn+Space and Esc stop; no alert; causes; workaround | P | – |
| D15 | wisprflow.ai/data-controls | Cloud always; ZDR; auto-add monitors the text box; usage stats regardless | P | – |
| D16 | 4497184932 Notetaker privacy | Cloud processing; audio deleted 7 d after upload; local 7/30 d; no bot | P* | "No opt-out" is not stated here; the security FAQ says transcripts are stored in Wispr's cloud and describes no opt-out. Reworded to "no storage opt-out described" |
| D17 | 1036674442 requirements | macOS 12+, 8 GB, separate Intel/AS builds, 14.4 system-audio-only, no Linux/iPad/VMs | P | – |
| D18 | 6391241694 hands-free | Session cap 19/20 min; 0.5 s cancel; Esc + Undo; Input Monitoring named; offline does not block | P | – |
| D19 | wisprflow.ai/pricing | Free 2,000/week desktop; Pro $15 / $12 annual; 50 % education | P* | "Resets Sunday" not found on the page: marked unverified |
| D20 | 3467817258 security FAQ | US-hosted; encrypted in transit/at rest, not E2E; training on for Free/Pro, off for Enterprise | P* | FAQ does not state the Growth default; the changelog row stays as R3c's (obs) |
| D21 | superwhisper.com/docs/models/language.md | S1-mini only local LLM, 462 MB, English only, Message/Mail, Pro; default cloud S1-Language | P | – |
| D22 | spokenly.app/docs/local-only-mode | Blocks all but localhost and model downloads; turns off otherwise-sent anonymous analytics | P* | The page does not say it is off by default; R3c's implication softened |

17 Wispr pages and 2 competitor pages re-fetched; 22 claims checked: 13 pass, 8 pass with wording or citation fixes, 1 citation fail corrected.

## 5. Contradictions between the files, and their resolution

| # | Contradiction | Check | Resolution |
|---|---|---|---|
| X1 | R3a: OpenWhispr suppresses Globe (MIT, adoptable). R3b: "No permissive project suppresses it." | Read `macos-globe-listener.swift:56-120`, `hotkeyManager.js:676-694` | R3a is right. OpenWhispr ships it and turns it on automatically when Globe is bound. R3b's statement holds only for native-Swift projects. |
| X2 | R3a: SpeakoFlow Mini is "the only purpose-trained local cleanup model found." R3b: Muesli Apache-2.0 cleanup GGUFs and VoiceInk-Refine-Mini. | HF API for all | At least four Apache-2.0 dictation-cleanup fine-tunes exist (SF Mini, Muesli 0.8B, Muesli 2B, Refine Mini) plus a Muesli Gemma 4 E2B LiteRT build. |
| X3 | R3a "nobody is silent and nobody uses a bootstrap model" vs R3b OpenSuperWhisper bundled tiny.en and MacParakeet silent locale rule | OSW code opened; MacParakeet README (R3b) | Scopes differed. Merged: no web-stack project is silent; a native permissive app (OSW) bundles a bootstrap model; only GPL MacParakeet picks silently. |
| X4 | R3c: VoiceInk "macOS 14.4+". R3b: "macOS 15+". | VoiceInk README | README says macOS 15.0 or later. R3c corrected. |
| X5 | R3c differentiator "every local competitor shows a model library" vs R3b MacParakeet silent choice | – | Restated: true for commercial apps; among open source only MacParakeet (GPL) is silent. |
| X6 | "OpenFlow" refers to two projects (R3a avijeett007 Tauri; R3b Robj1925 Swift) | Clone remotes | Renamed OpenFlow-T and open-flow in R3. |
| X7 | BetterWispr GPL file list (R3b) vs `docs/oss-reuse.md` | Read the doc | List extended from 9 to 11 files (AppModel.swift, WarmUpClip.swift). |
| X8 | R3a: pyannote seg-3.0 MIT (inf), gated (inf); R3b: pyannote CC-BY-4.0 (via SpeakerKit) | HF API; BetterWispr doc | Both true for different artifacts: segmentation-3.0 is MIT and gated; Argmax's SpeakerKit conversion of community-1 is CC-BY-4.0. |
| X9 | R3a SpeakoFlow uses transcribe-cpp 0.4.0; R3b says hex uses "the same engine family" at 0.4.1 | hex Cargo.toml | True, but hex also patches `transcribe-cpp-sys` from its own git fork; noted. |

## 6. Corrections applied in R3-landscape.md (summary)

1. OpenWhispr's Globe suppression: shipped and adoptable (with consent), not "nobody".
2. Cleanup models: four or more Apache-2.0 candidates, not one.
3. BetterWispr GPL-derived files: 11, including `AppModel.swift`; R3b's AppModel citations are idea only.
4. AivoRelay: LGPL/GPL components bundled (new trap).
5. VoiceInk minimum macOS 15.0.
6. Wispr clipboard formats: "file contents, PDF, RTFD and audio", source W1.
7. Wispr bar click wording, drag source (changelog), Notetaker "no opt-out" softened, free-plan reset day unverified.
8. Spokenly Local Only Mode: default state not documented.
9. pyannote segmentation-3.0 license and gating verified (MIT, gated).
10. FluidVoice Apache-2.0 at v1.5.8 verified from the LICENSE at the tag.
11. Muesli: added the Gemma 4 E2B LiteRT model; telemetry claim narrowed to "enabled in release builds that inject an app ID; no user toggle found".
12. OpenSuperWhisper bootstrap size 77.7 MB.
