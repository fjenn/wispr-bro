# R4 addendum: re-scoring GPL-3.0 and AGPL-3.0 sources

This addendum applies the owner's licence change to `R4-parts-bin.md`. The app will be released under **GPL-3.0**. Handy's MIT code may be combined into a GPL-3.0 work, so GPL-3.0 and AGPL-3.0 code is now a real code source and no longer idea-only.

Everything in R4 that was permissive stays valid. This file only says where a GPL or AGPL source is now **better** than the R4 pick, where it is worth **adding as a reference or partial port**, and where we should **keep the current pick**. Wispr Flow's proprietary code and assets stay off-limits; we copy behaviour only.

## 0. How to read

**Tags.**
- **(obs):** I read it in the clone, in the LICENSE, or through the GitHub API.
- **(inf):** inferred.
- **(unk):** not determined.

**How the claims were checked.** Five clones were surveyed by parallel read-only passes. I re-checked every load-bearing claim in the code myself: licence files, line counts, function names and the flags quoted.

**Paths.**
- Clone root: `<scratchpad>/clones/`. Paths below are relative to `gpl-<name>/`.
- Handy paths are relative to `src-tauri/src/`, as in R4.

**Clone commits (all 2026, unless noted; shallow, never built or run).**

| Clone | Commit | Date |
|---|---|---|
| gpl-voiceink | 1027ea3 | 10-27 |
| gpl-fluidvoice | 6150956 | 10-27 |
| gpl-typewhisper | 851cda8 | 10-28 |
| gpl-macparakeet | 590b6c2 | 10-23 |
| gpl-betterwispr | 304335a | 10-24 |
| gpl-epicenter (main) | 7cc869b | 10-17 |
| gpl-whispering-v7 (tag v7.11.0) | a6b15c2 | 2025-12-27 |
| gpl-tambourine | c54b527 | 06-30 |
| gpl-voquill | a5dfbe0 | 09-26 |
| gpl-espanso | 80d9e35 | 10-24 |
| gpl-shepherd | 7f25ce5 | 10-22 |
| gpl-introjs | 5b12889 | 09-21 |
| gpl-selection | 622e8f3 | 2024-05-05, archived |
| gpl-mado (builder-group/abstand) | 17c7268 | 10-24 |
| gpl-easydict | be3b7a1 | 10-28 |
| gpl-extra-openless | ac0b159 | 10-28 |
| gpl-extra-voicetypr | f86e571 | 10-15 |
| gpl-extra-alttab | c500294 | 10-23 |

**Verdict words.**
- **swap:** replace the R4 pick or its primary source.
- **add:** keep the R4 pick and port specific pieces or tests from the GPL source.
- **reference:** read for design or test vectors only.
- **keep current:** nothing better found.

**Headline.** No GPL or AGPL project replaces a whole R4 pick:
- Most GPL dictation apps are large SwiftUI/SwiftData apps. Their best parts are entangled with app state.
- The Rust/Tauri ones (Voquill, Tambourine, Whispering v7, OpenLess, VoiceTypr) are thinner than SpeakoFlow and hex where it matters.

The gain is a set of **well-tested pieces and test suites**. They shrink several "write ourselves" items and lower the risk on AX, paste, hotkeys and the LLM supervisor. Three gaps that R4 left to us now have code: dictionary auto-learn, vocabulary import, and native mute.

---

## 1. Licence compatibility per project

Rules applied:
- GPL-3.0-only, GPL-3.0-or-later, GPL-2.0-or-later, LGPL and AGPL-3.0 are usable. AGPL is usable through GPLv3 §13, and those parts keep AGPL terms.
- GPL-2.0-only, unlicensed code and custom non-free terms are not usable.

When a project ships the plain GPL text with no "or (at your option) any later version" statement, I treat it as **GPL-3.0-only** (inf). That is compatible either way, but it pins the combined work to version 3 (see §4).

| Project | Exact licence | Evidence | Compatible | Notes |
|---|---|---|---|---|
| **VoiceInk** (Beingpax) | GPL-3.0, no "or later" statement, so treated as -only (inf) | `LICENSE`; README "GNU General Public License v3.0"; LICENSE has one commit, 76a15470 (2025-02-22); no per-file headers (obs) | **Yes** | No CLA. Paid builds via Polar (`Infrastructure/Licensing/LicenseManager.swift`): strip it. Remote announcements fetch (`Infrastructure/RemoteConfig/AnnouncementsService.swift`): strip it. 11 `.scpt` AppleScripts and `NSAppleScript` paste: do not port. **VoiceInk Refine weights** (`beingpax/VoiceInk-Refine-V1`) carry their own LICENSE and stay excluded (obs, `VoiceInkRefineModelDownloader.swift:58-63`). |
| **FluidVoice** (altic-dev) | GPL-3.0 from **2026-02-23**; Apache-2.0 before | `LICENSE`; README "From 2026-02-23 onward … GPLv3. Versions published before this date were licensed under Apache License 2.0"; relicensing commit e727ddf6 (2026-02-23) (obs) | **Yes** | No CLA (obs). Vendored WebRTC AEC3 (`Vendor/WebRTCAudioProcessing/Notices`) is BSD-3 + PATENTS, plus Abseil/Perfetto Apache-2.0, RNNoise BSD, and PFFFT/Ooura permissive (obs). It ships a **prebuilt** `libFluidAEC3-system-exact.a`; rebuild it from the pinned sources if used (inf). SPM dep `zeppelin-embed` is GPL-3.0. **Exclusions:** Fluid-1 weights (`license:other`), the closed "Fluid Intelligence" runtime (`#if PRIVATE_AI_PROVIDER`, `Sources/Fluid/Services/PrivateAIProvider.swift`), and PostHog analytics (`Sources/Fluid/Analytics/AnalyticsConfig.swift:8`, `eu.i.posthog.com`, weekly active ping with no off switch). |
| **TypeWhisper** (typewhisper-mac) | **GPL-3.0-or-later** + commercial dual licence | `LICENSE`; `LICENSE-COMMERCIAL.md` "GPL-3.0-or-later" (obs) | **Yes** | **CLA** for contributors (`CONTRIBUTING.md:81-94`). That only matters for contributing upstream; it does not limit our GPL rights. Premium features are plain booleans in the repo (`Services/LicenseService.swift`, 1,771 lines; Polar API). Removing them is GPL-permitted (inf). **`TRADEMARK.md`:** no TypeWhisper name or icon. No analytics SDK. Plugin model files: whisper-tiny is Apache-2.0, GigaAM is MIT; Supertonic is OpenRAIL-M and stays excluded. |
| **MacParakeet** | GPL-3.0, "Copyright (C) 2026 Daniel Moon", no "or later" statement, so treated as -only (inf) | `LICENSE` header lines 1-2; README "GPL-3.0" (obs) | **Yes** | No CLA. `THIRD_PARTY_LICENSES.md` lists FFmpeg as **GPL-2.0-or-later** (static, with x264/x265). That is compatible, but we do not need it. **Exclusions:** opt-out telemetry (`Services/Telemetry/*`), the launch-time feed (`Services/Discover/DiscoverService.swift`), LemonSqueezy licensing, and MediaRemote private API. Models: Parakeet is CC-BY-4.0, Orukeet CC-BY-SA-4.0, Nemotron OpenMDW. |
| **BetterWispr**: the 11 VoiceInk-adapted files | Repo Apache-2.0. The 11 files listed in `docs/oss-reuse.md` derive from VoiceInk GPL-3.0 (at VoiceInk c09cc1f) | `LICENSE`, `docs/oss-reuse.md` (obs) | **Yes, now.** Treat these files as GPL-3.0; the rest of the repo stays Apache-2.0. | The files: `Speech/WordAgreementEngine.swift`, `Speech/LiveTranscriber.swift`, `Integrations/CorrectionWatcher.swift` (117 lines), `Transcription/CorrectionReviewer.swift`, `Speech/WarmUpClip.swift`, the warm-up parts of `App/AppModel.swift`, `Notes/APINotesModel.swift`, `Transcription/TranscriptPolisher.swift`, `Integrations/ContextCapture.swift`, `Domain/WritingMode.swift`, `Integrations/WebsiteReader.swift` (65 lines). All are under `Sources/BetterWisprCore/` except AppModel. The trap from R3 §7 is now harmless, but **keep the VoiceInk copyright attribution** on anything ported. |
| **Whispering / Epicenter** | Main: **AGPL-3.0-or-later**. Tag v7.11.0: AGPLv3 text, package.json `"AGPL-3.0"` | Root `LICENSE` index; `apps/whispering/LICENSE`; `CONTRIBUTING.md:288` "no CLA" (obs) | **Yes** (AGPL parts) | Main dropped system-wide dictation; its README says "Dictating into other apps is not part of it" (obs, survey). v7 uses Aptabase analytics: do not port. v7's Zapsplat sound files are not AGPL: excluded (inf). Some old npm versions stay MIT (root LICENSE §2). |
| **Tambourine** | AGPL-3.0. The app says `"AGPL-3.0"`; the Python server says `AGPL-3.0-only` | `LICENSE`; `app/package.json:27`; `app/src-tauri/Cargo.toml:5`; `server/pyproject.toml:6` (obs) | **Yes** (AGPL parts) | No CLA. The formatting server is Python/Pipecat; we only consider the Rust app code. |
| **Voquill** | AGPLv3 (no "or later"), **except `enterprise/`**, which is under the proprietary "Voquill Enterprise License" | Root `LICENCE` lines 1-7; `enterprise/LICENCE` (obs) | **Yes**, outside `enterprise/` | `packages/rust_transcription/Cargo.toml:5` says `license = "MIT"`, but its LICENCE file is the AGPL text. **Treat it as AGPL** (inf). No app package imports `enterprise/` (survey; inf). The desktop app uses Firebase, Mixpanel and Stripe in TS: do not port. No CLA. |
| **espanso** | **GPL-3.0-or-later** | File headers, e.g. `espanso-match/src/lib.rs:6-9` "either version 3 … or (at your option) any later version" (obs); `espanso/Cargo.toml` `license = "GPL-3.0"` | **Yes** | The vendored `espanso-clipboard/src/x11/native/clip` is MIT. `xdo.c` has no header (xdotool upstream is BSD, inf). |
| **shepherd.js** | AGPL-3.0 + commercial | `LICENSE.md` (obs) | Legally yes (inf), **but avoid** | Its LICENSE.md says you "must purchase a commercial license" if your company makes revenue, even if the project using it does not. That conflicts with AGPL's no-further-restrictions clause; it invites a dispute and adds nothing over driver.js. |
| **intro.js** | AGPL-3.0 + commercial | `license.md` (obs) | Legally yes (inf), **avoid** | Same pattern as shepherd.js ("commercial projects need a commercial license"). |
| **`selection` crate** (pot-app) | **GPL-3.0-only** | `Cargo.toml` `license = "GPL-3.0-only"` (obs) | Licence yes, **provenance no** | Its macOS AX function says "Copy from https://github.com/yetone/get-selected-text" (`src/macos.rs`). That repo has no LICENSE (R4 §4). The fallback drives System Events through `osascript`, which triggers an Automation prompt. Archived. **Reject.** |
| **mado / abstand** | Repo **AGPL-3.0** by default (workspace `AGPL-3.0-only`); `crates/mado` is **MIT** | Root `LICENSE` line 3; `crates/mado/LICENSE`, `crates/mado/Cargo.toml:6` (obs) | **Yes** (both) | `apps/desktop` adds no extra URL or AX code. `apps/desktop/crates/macos/src-swift/WindowLiquidGlass.swift` and `OverlayAttachment.swift` are now usable as AGPL (survey). |
| **Easydict** | GPL-3.0, derived from Bob (GPL-3.0) | `LICENSE`; README line 114 (obs) | **Yes** | The selection logic lives in **SelectedTextKit**, which is **MIT** (GitHub API, obs). Port from that, not from Easydict. Its AXSwift fork is MIT. |
| **OpenLess** (new) | **AGPL-3.0-only** (2.x); README says 1.x stayed MIT | `LICENSE`; README line 449; `app/src-tauri/Cargo.toml:4` (obs) | **Yes** | `crates/micyou-dsp` is GPLv3 with the "MicYou Plugin Exception" (an additional permission, so compatible) (obs, `crates/micyou-dsp/LICENSE`). Uses objc2 0.5, older than Handy's 0.6. Telemetry not checked (unk). |
| **VoiceTypr** (ideaplexa/voicetypr, new) | AGPL-3.0 | `LICENSE`; GitHub API (obs) | **Yes** | Licence gating is in `src-tauri/src/commands/license.rs`: strip it. `crates/keytrigger` says MIT in Cargo.toml but ships no LICENSE file, so treat it as AGPL (survey; inf). |
| **alt-tab-macos** (new) | GPL-3.0 | `LICENCE.md` (obs) | **Yes** | Reference for tap health and permissions only. |
| **InputSourcePro** (new) | GPL-3.0 | GitHub API (obs) | **Yes** | `PreferencesVM+BrowserAddress.swift` detects when the browser address bar has focus (survey, not cloned). |
| KoboldCpp | AGPL-3.0 | GitHub API (obs) | Yes | Usable now, but no benefit over the llama.cpp `llama-server` sidecar. Stays out. |
| AivoRelay MP3 path (LAME) | LGPL | R3 §7 | Yes | Usable, but we do not need MP3 export. |
| aw-watcher-window | MPL-2.0 | R4 §4 | Yes (MPL-2.0 §3.3, unless a file is marked "Incompatible With Secondary Licenses") (inf) | Still not wanted: it relies on per-browser AppleScript. |

**GPL-2.0-only:** none found among the candidates (obs, survey of all clones plus the GitHub search). Re-check before taking any Linux or X11 tool later.

---

## 2. Per-capability verdicts

"Effort change" compares against R4's estimate:
- "=" means the same size, lower risk.
- "−" means less code for us.
- "new" means R4 had no part for it.

| Capability | Current R4 choice | GPL/AGPL candidate (repo, files) | Verdict | Why | Effort change | R2 ids |
|---|---|---|---|---|---|---|
| **Local LLM runtime / supervisor (W-01)** | `llama-server` sidecar + own ~300-line supervisor using SpeakoFlow and OpenWhispr ideas | **TypeWhisper** `TypeWhisperPluginSDK/Plugins/LlamaCppPlugin/LlamaCppServer.swift` (464 lines), `LlamaCppRuntime.swift` (253), `LlamaCppModels.swift` (348), plus `Tests/LlamaCppPluginTests.swift` (814 lines, about 35 tests) (obs) | **swap the primary reference** (port to Rust). Runtime stays a bundled, signed sidecar. | Same architecture as ours, already tested. It launches with `--parallel 1 --jinja --reasoning off --cache-ram 0 --no-ui --offline` (`LlamaCppServer.swift:30-41`, obs), a random loopback port with bind-race retries, a random `LLAMA_API_KEY`, scrubbed `LLAMA_*` env, a `/health` poll, an output-tail capture for errors, restart on exit, and a parent-death watchdog script (survey). Its test list doubles as our acceptance list. **Do not copy its runtime download of llama.cpp** (R4: bundle, never download executables). FluidVoice's local runtime is closed; MacParakeet's is gated MLX; VoiceInk's is an MLX XPC service for 16 GB machines. None fits. | = (M), risk ↓; tests come free as a spec | AF-01, D-67, D-211, D-213, DEC-03 |
| LLM models per RAM tier | Qwen3.5-2B / Qwen3-4B-Instruct-2507 | TypeWhisper `LlamaCppModels.swift` (RAM-based picks); FluidVoice `Services/PrivateAIHardwareRecommendation.swift` (110 lines, IOKit GPU cores, bandwidth table) | **reference** | They confirm our tiering. Fluid-1 and VoiceInk Refine weights stay excluded (§5). | = | D-71, AF-19, AF-20 |
| Apple Foundation Models bridge | hex `apple_intelligence.swift` + `.rs` | TypeWhisper `Services/LLM/FoundationModelsProvider.swift:181-236` (prewarm keeps a session alive, permissive guardrails, context overflow detection). MacParakeet `Services/LLM/FoundationModelsAppleIntelligence*.swift` (availability and `GenerationError` mapping incl. `localeLimited`; `AppleIntelligenceLLMClientTests` 439 lines) | **keep hex; add** the MacParakeet error mapping and TypeWhisper's prewarm-at-record-start call sites (`DictationViewModel.swift:2790`) | hex is already a Rust↔Swift bridge. The additions are about 30 lines. | = | AF-14, D-68, D-208 |
| **Cleanup prompts and guards (W-03)** | OpenWhispr prompt + SpeakoFlow validators + FreeFlow check | **BetterWispr** `Transcription/TranscriptPolisher.swift:218-233` `accepted()` (word band 0.4×-1.2×+2, question marks kept, ≤25 % new meaningful tokens; `PolishContextTests` 13 tests). **TypeWhisper** `FoundationModelsProvider.swift:24-178` `TypeWhisperDictatedTextBoundary` (BEGIN/END markers, echo stripping) and `Models/Workflow.swift:867-1010` language hint "German (de)". **FluidVoice** `Persistence/SettingsStore.swift:1044-1215` (transcript passed as a JSON field: "Do not answer the speaker"). **VoiceInk** `Core/Enhancement/AIPrompts.swift` (56 lines). **Voquill** `apps/desktop/test/evals/post-processing-eval.test.ts` (726 lines of eval cases) | **add** BW `accepted()` as one more guard; add the boundary wrapper; **mine the Voquill evals** for the RT-04 adversarial set | No GPL project has guards as complete as SpeakoFlow's (obs: VoiceInk `AIEnhancementOutputFilter.swift` only strips think tags; MacParakeet and FluidVoice have no length or answer guard). The additions are small and tested. | ≈ −50 lines of guard code; eval set comes free | AF-02, D-11, D-12, D-212, DEC-13 |
| **Accessibility client (W-05)** | SpeakoFlow `smart_spacing/macos.rs` on objc2-application-services | **TypeWhisper** `Services/TextInsertionService.swift` (2,893 lines): AXManualAccessibility lease with restore (`:39`), fail-open rule `focusedApplicationHasNoFocusedWindow` (`:684`), `missedTextField` result, Gecko/Thunderbird excluded from AX writes; about 170 insertion/paste tests in `TypeWhisperTests/TypeWhisperIntegrationTests.swift`. **VoiceInk** `Infrastructure/SystemIntegration/Accessibility/AccessibilityTextReader.swift` (453): WebKit/Chromium **text-marker** reads (7 hits; SpeakoFlow has 0, obs), per-PID refcount of AXManualAccessibility. **Voquill** `apps/desktop/src-tauri/src/platform/macos/accessibility.rs` (2,569, Rust, no tests). **FluidVoice** `Services/DictationContextCapture.swift` (508; budgeted tree walk, password-manager skip list; tests 460) | **keep SpeakoFlow as the Rust base; add** text markers (VoiceInk), the fail-open rule and Gecko exclusion (TypeWhisper), and FluidVoice's budgeted context walk for D-58. Use TypeWhisper's test names as our RT-20/RT-21 matrix. | No candidate is a drop-in Rust layer that beats SpeakoFlow: Voquill's is raw CF FFI with manual `CFRelease` and no tests (survey). The value is in proven heuristics for Electron and contenteditable, which were R4's main runtime risks. | = (M), risk ↓ | AF-05, A-50, A-51, A-15, D-29, D-52, D-58, DEC-24, RC-04 |
| **Selected-text read (D-29)** | SpeakoFlow `selection.rs` (⌘C with sentinel only) | **SelectedTextKit** (MIT, used by Easydict): AX `kAXSelectedText` → text markers → **menu-bar Edit › Copy via AX** → ⌘C. **OpenLess** `openless-all/app/src-tauri/src/selection.rs` (1,667 lines, Rust; AX first, then copy-restore, insertion-target validation, Windows path; 2 in-file tests). **MacParakeet** `SelectionCaptureService.swift` (526) + `SelectionReplacementService.swift` (427), tests 461 + 412 | **add** an AX-first step and the menu-Copy step before SpeakoFlow's ⌘C fallback, following SelectedTextKit (MIT; no GPL needed). Use MacParakeet's tests as the spec. OpenLess is a reference. | The menu route avoids the held-modifier case where SpeakoFlow gives up (survey). OpenLess is Rust, but it is tangled with its workspace and on objc2 0.5 (obs). | ≈ +60 lines, fewer failures | D-29, D-30, D-44 |
| Frontmost app / window title / browser URL | objc2 NSWorkspace + port of mado 0.0.21 (MIT) | **BetterWispr** `Integrations/WebsiteReader.swift` (65 lines, AX only: `AXDocument` → outermost `AXWebArea` `AXURL` → bounded search; obs `:21-41`). **FluidVoice** `Services/Meeting/MeetingAutoDetectSignals.swift` `AXBrowserTabReader` (PWAs). **Tambourine** `app/src-tauri/src/active_app_context/macos.rs` (378, Rust, origin-only URLs, tests in `tests/active_app_context_*`). **InputSourcePro** address-bar focus check. TypeWhisper and VoiceInk use AppleScript: reject. | **keep the mado port; add** BW's 65-line reader as the fallback and cross-check, and Tambourine's **origin-only** reduction (privacy) | mado's code is MIT and already chosen. These are short, AX-only, and widen coverage. | = | D-21, D-22, DEC-23 |
| **Hotkeys and tap health (W-06)** | handy-keys 0.3.4 + port of anarlog `shortcut-macos` + axwatch | **MacParakeet** `MacParakeetCore/STT/FnKeyStateMachine.swift` (234), `HotkeyGestureController.swift`, `Services/System/BackgroundEventTap.swift` (169, all taps on one dedicated thread), with the largest test suite seen (`HotkeyManagerTests` 3,266 lines, `FnKeyStateMachineTests` 335, `HotkeyGestureControllerTests` 434). **TypeWhisper** `Services/HotkeyService.swift` (3,608): background watchdog with generation tokens and backoff (`testWatchdogReenablesRealEventTapWhileMainThreadIsBlocked`). **VoiceInk** `Features/Shortcuts/Coordination/ShortcutMonitor.swift` (1,053): rebuild on wake, display wake and session-active; missed-release detection; no tests. **FluidVoice** `GlobalHotkeyManager.swift` `ModifierOnlyShortcutFlagsDecision` (tests 4,874). OpenLess `macos_dictation_key.rs` (F5 Dictation key). | **keep current; add** MacParakeet's and FluidVoice's tests as **test vectors** for the ported anarlog processor, and VoiceInk's recovery triggers for the health check | handy-keys is already in tree. A Swift tap manager would duplicate it. No GPL project solves Globe suppression: FluidVoice PR #193 closed unmerged, MacParakeet only ignores keycode 179 (survey). | = (S+M), more test coverage | AF-08, A-22, A-23, A-25, A-35, A-201, A-208, DEC-09, RC-02 |
| Secure Input holder (A-205) | Yap IORegistry `kCGSSessionSecureInputPID` (MIT, R3 #21) | espanso `espanso-mac-utils/src/native.mm:42` (obs); TypeWhisper `Services/SecureInputDiagnostics.swift` (181) | **keep current** (Yap, MIT) | Same technique; no gain. | = | A-205 |
| **Clipboard and paste (W-07)** | hex `paste.rs` + own snapshot code + arboard | **TypeWhisper** `TextInsertionService.swift:734` `insertTextAtAndVerifyChange` (ignores the AX setter result, polls `AXValue` for a change, falls back to paste; fixes the #1732 double insert; tests `:7732-7760`). **FluidVoice** `Services/PasteVerifier.swift` (196: confirmed / notLanded / unknown), `PasteDeliveryCoordinator.swift` (868: FIFO delivery slot, 500 ms lease), `PasteKeyCodeResolver.swift` (UCKeyTranslate). **MacParakeet** `ClipboardService.swift` (380, restore generation counter; tests 431), `PasteShortcutKeyResolver.swift` (tests 108). **Voquill** `CGEventSourceStateID::Private` for synthetic keys (`platform/macos/input.rs:126`). | **keep hex; add** verify-after-insert (TypeWhisper/FluidVoice) and the layout-aware V keycode resolver | hex is Rust and keeps every pasteboard flavour plus the Chromium 155/156 workaround. Every GPL candidate restores fewer flavours or is Swift. Paste verification is the missing piece behind A-53 ("paste failed" feedback). | ≈ +80 lines, closes A-53 better | AF-17, A-46, A-47, A-48, A-49, A-53, C-70, D-29, D-30 |
| Clipboard-free typing (fallback) | hex Chromium path (Unicode typing ≤20 units) | MacParakeet `Services/System/StreamingCursorInserter.swift` (317) + scheduler (172): batched `CGEventKeyboardSetUnicodeString`, own-event marker, user-interrupt detection; tests 218 + 76. OpenLess `unicode_keystroke.rs` (switches to an ASCII input source so an IME cannot swallow text). espanso `espanso-inject/src/mac/native.mm`. | **reference** | Only needed if hex's typing path proves weak. The IME detail from OpenLess is worth noting for CJK users. | = | A-46, RC-05 |
| Pill / overlay | tauri-nspanel 2.1.1 + Tolkovo pill pattern | **Voquill** `packages/rust_macos_pill` (3,613 lines, `cocoa 0.26` + `objc 0.2`, CoreGraphics drawing, CVDisplayLink parked when idle, `hitTest` click-through, no drag, no tests). MacParakeet `Views/Dictation/IdlePillController.swift` (173). TypeWhisper `NotchIndicatorLayout.swift`. abstand `WindowLiquidGlass.swift`. | **keep current** on macOS. **Reference** for Voquill's Windows (`rust_windows_pill`, Direct2D) and Linux (`rust_gtk_pill`, layer-shell) pills later. | The native macOS pill gives up React styling and uses deprecated `cocoa`/`objc` crates. It is also the plan-B for RT-17 (webview memory), so note it there. | = | AF-09, A-01, A-02, A-04, A-212, DEC-25, RT-17 |
| System audio | hex `meeting_capture.rs` process tap | TypeWhisper, FluidVoice and MacParakeet all use **ScreenCaptureKit** (Screen Recording permission) (survey; obs for TypeWhisper `AudioRecorderService.swift:1391`, MacParakeet `Audio/SystemAudioStream.swift`) | **keep current** | The process tap avoids the Screen Recording prompt. | = | AF-10, E-01, E-02 |
| **Mute while dictating** (new row) | Handy `managers/audio.rs:100-111` via `osascript` (AF-10); no R4 part | **Tambourine** `app/src-tauri/src/audio_mute/macos.rs` (858 lines, Rust, CoreAudio `AudioObject` mute/volume save and restore) + `windows.rs` (WASAPI) + `tests/audio_mute_tests.rs` (273) (survey). OpenLess `audio_mute.rs`. | **swap** (port Tambourine) | Replaces an AppleScript call with tested Rust and a per-device restore. It also gives us Windows. It must not mute during a meeting recording (E-205). | new, S (≈ −100 lines vs writing it) | A-41, E-205 |
| Call detection | hex `microphone_activity.rs` + `meeting_detection.rs` | TypeWhisper `MeetingAudioActivityCollector.swift` (572, 15 tests), **`MeetingCameraActivityCollector.swift`** (347, CoreMediaIO), `MeetingLinkParser.swift` (315, 12 tests). MacParakeet `MeetingDetection/CameraActivityCollector.swift` (225). FluidVoice `Meeting/MeetingAutoDetector.swift` (tests 2,491 lines, adds Google Meet in-call AX evidence). | **keep hex; add** the camera-in-use signal and FluidVoice's test cases | hex is Rust and covers the audio signal. The camera signal and Meet evidence cut false positives (RT-29). | ≈ +150 lines | E-06, E-07 |
| Calendar | objc2-event-kit + anarlog `apple-calendar` shape | **MacParakeet** `MacParakeetCore/Calendar/CalendarService.swift` (297), `MeetingLinkParser.swift` (108), `MeetingMonitor.swift` (196), tests 560 + 731 + 189. TypeWhisper `CalendarMeetingAutomationPolicy.swift` (1,015, 51 tests). | **keep current; add** MacParakeet's link parser and monitor tests as the spec | The anarlog shape is already Rust; MacParakeet brings tested link regexes and auto-start policy. | = | E-07, E-42 |
| Notifications, menus, capabilities, Dock | tauri-plugin-notification 2.4.0; own `app_menu.rs` | none better | **keep current** | — | = | E-91, AF-15, AF-16 |
| **Text pipeline: dictionary rules (W-13 part)** | aho-corasick + own code; SpeakoFlow `replacements.rs` reference | **VoiceTypr** `src-tauri/src/writing/library_rules.rs` (1,031 lines, Rust, 17 tests): applies rules to **finished transcripts** with word-boundary checks, `span_in_protected_token` (URLs and e-mails are never rewritten), per-language scope and per-operation provenance (obs). VoiceInk `PhoneticVocabularyMatcher.swift` (394). MacParakeet `TextProcessing/TextProcessingPipeline.swift` (368) + tests 651. | **swap** the replacement stage to a port of VoiceTypr's rules; **keep own snippet matcher** | VoiceTypr is the only Rust code that does exactly our job on finished text, with tests and provenance (which feeds "N fixes"). Its snippets match only a **whole utterance** (`match_snippet`, obs), so Wispr-style inline snippets still need our aho-corasick matcher. | ≈ −200 lines of W-13 | D-48, C-63, D-210, AF-06 |
| Text pipeline: snippets | own aho-corasick matcher | espanso `espanso-match` (rolling per-keystroke matcher, wrong shape); `espanso-render` (date/choice variables) | **keep current**; espanso-render is a **reference** if snippet variables are ever wanted (drop its shell/script extensions) | Our snippets expand inside a finished string, not as keystrokes (obs, survey). | = | D-53, C-72, DEC-12 |
| Text pipeline: voice commands, spoken punctuation, newlines | own tables at clause edges | MacParakeet `TextProcessing/SpokenPunctuation.swift` (136, 8 languages, a "literal" escape; tests 81) and trailing action extraction ("press return"); FluidVoice `ASRService+SpokenPunctuationFormatting.swift` (1,086, app-aware) + `SpokenSendParser.swift` (tests 641); TypeWhisper `SpeechPunctuationService.swift` (366, CJK) | **add** MacParakeet's spoken-punctuation tables and the literal escape as seed data | The tables are data, not structure, so they are cheap to port. | ≈ −80 lines of tables | D-56, D-61, A-54 |
| **Dictionary auto-learn** (new; was write/idea in R2 DEC-20) | none (DEC-20 open) | **BetterWispr** `Integrations/CorrectionWatcher.swift` (117) + `CorrectionLearner` + `Transcription/CorrectionReviewer.swift` (`CorrectionLearningTests`, 17 tests). **VoiceInk** `Features/Dictionary/AutoLearn/` (2,715 lines; `CorrectionDiffEngine`, anchor diff, focus observer, user approval queue). **TypeWhisper** `TargetAppCorrectionLearningService.swift` (779, 30 tests; learns on Return/Tab/focus-change commit). **FluidVoice** `AutomaticDictionaryCorrection{Tracker,Detector}.swift` (2 hits in 7 days). | **swap** "write ourselves" to **port** BW's three files, using VoiceInk's diff engine for multi-hunk cases, behind a user-approval queue | Previously idea-only (GPL-derived). Now it is tested code. It needs the AX layer (read back the field after paste). BW never restores `AXManualAccessibility` (survey), so use our lease. | new, M (≈ −300 lines vs from scratch) | D-52, C-66, DEC-20 |
| **Vocabulary import from Handy and Wispr Flow** (new) | papaparse CSV wizard only | **TypeWhisper** `Services/AppVocabularyImport.swift` (322): reads Handy `com.pais.handy/settings_store.json` (obs `:27`) and requires two identical reads; `WisprFlowImportReader.swift` (182) copies the Wispr sqlite + WAL with hash checks and opens the copy read-only; `AppVocabularyImportTests.swift`; `docs/app-vocabulary-import.md` | **add** (port) | It is a one-click migration for exactly our target users. Reading the user's own local Wispr database is interoperability with user data, not Wispr code (inf). Confirm with the owner. | new, S | C-64, C-65, D-48, D-55 |
| ITN | text-processing-rs 0.2.2 + text2num | TypeWhisper `NumberWordNormalizer.swift` (284) + EN/DE/FR/ES/NL/PL parsers (about 1,600 lines; zh/ja stubs; 44 tests) | **keep current**; TypeWhisper is a **reference** if text-processing-rs's taggers fall short in DE/FR/ES | text-processing-rs is a crate we adopt (no code to write). The Swift parsers would be a port. | = | D-14 |
| Word counts / stats / streaks | unicode-segmentation + own `daily_stats` | VoiceInk `DashboardStatsLoader.swift` (SwiftData); MacParakeet `DailyDictationStat` + tests; TypeWhisper `UsageStatisticsService.swift` (485) | **keep current** | All are tied to their stores. | = | AF-07, C-30, C-31, DEC-11 |
| Diff (fix counts) | similar 3.2.1 | TypeWhisper `TextDiffService.swift` (249); FluidVoice `UI/HistoryTextDiff.swift` (49) | **keep current** | `similar` is an adopted crate. | = | D-40, D-57, C-52 |
| Fork DB, FTS, history, retention | rusqlite + own `fork.db` | MacParakeet failed-dictation audio kept + `RetranscribeCommand.swift`; TypeWhisper `HistoryRetranscriptionService.swift` (44 tests), `DictationRecoveryAudioStore.swift`; FluidVoice audio pruning to a byte budget (`TranscriptionHistoryStore.swift:529`) | **keep current; reference** for failed-take retention and the audio byte budget | SwiftData and GRDB stores don't port; the policies do. | = | AF-07, C-46, C-211, DEC-07 |
| Import / export | papaparse, csv, zip | MacParakeet `VocabularyImportExportService.swift` (versioned bundle with a forward-version check; tests 671); TypeWhisper `SettingsBackupExporter.swift` (45 tests), `UserDataEraser`; Tambourine `commands/export_import.rs` (890, Rust) | **keep current; reference** the versioned bundle format and the eraser checklist | Small, and ours is already planned. | = | C-64, C-65, C-71, E-66, E-67 |
| UI primitives, charts, virtual list, fonts, celebrations | Radix, Recharts, etc. | none | **keep current** | — | = | — |
| Tours / spotlight | driver.js 1.10.2 (MIT) | shepherd.js, intro.js (AGPL + commercial) | **keep current** | No feature gain; licence-wording risk (§1). | = | B-40, B-44, B-45, DEC-15 |
| Onboarding flow and permission guides | own on Handy's Demo engine; hex/OpenWhispr drag tile | **MacParakeet** `MacParakeetViewModels/OnboardingViewModel.swift` (1,452) + `OnboardingViewModelTests` (**2,090 lines**): welcome → permissions → a real dictation with a paste grace period → done. VoiceInk `OnboardingPermissionController.swift`. alt-tab-macos `src/macos/SystemPermissions.swift`. | **add** MacParakeet's view-model tests as the behavioural spec for our React/Rust flow | The best-specified first run among all projects. It also covers R2's "flow ending in a real dictation". | = (M), fewer design rounds | B-10, B-12, B-21, B-25, B-46, DEC-15 |
| Per-app styles | OpenWhispr signature table + BW `StyleFormatter`/`WritingStyle` (original Apache) | **BetterWispr** `Domain/WritingMode.swift` (152; trigger word > website > app; host/subdomain/path and `*.github.com` patterns; `WritingModeTests` 12 tests). Voquill `db/app_target_queries.rs` (app → tone, paste key, insertion method). TypeWhisper `Models/Profile.swift`. VoiceInk `ModeConfig.swift:417` substring URL match (buggy, reject). | **add** BW `WritingMode` precedence and matching (port) | Tested, small, and now usable. Keep OpenWhispr's data table. | ≈ −60 lines | D-18, D-19, D-21, D-22, D-23, D-24, DEC-23 |
| Hardware probe / silent model choice | sysinfo + own policy | TypeWhisper `ViewModels/SetupWizardEngineRecommendation.swift` (456, **60 tests**); FluidVoice IOKit GPU-core probe; Voquill `system/gpu.rs` (wgpu in a child process, for Windows/Linux) | **add** TypeWhisper's recommendation tests as the policy spec | Policy logic is the risky part, and it is tested there. | = | AF-20, B-49, B-208, DEC-02 |
| Bootstrap model / queued audio | whisper-base + own queue | MacParakeet failed-audio store; TypeWhisper `MicrophonePrerollRingBuffer.swift` (88, tested); BW `Speech/WarmUpClip.swift` (20, warm-up cancelled when dictation starts) | **reference**; port BW's warm-up (about 20 lines) if the first-take latency test (RT-31) needs it | — | = | AF-03, B-50, B-202, A-45 |
| Diarization, echo, summaries | sherpa-onnx; hex `meeting_echo.rs`; anarlog + Meetily prompts | FluidVoice `Vendor/WebRTCAudioProcessing/BridgeSources/fluid_aec3_bridge.{h,cc}` (48 + 190 lines, pure C ABI; `MeetingAEC3Tests` 1,575). MacParakeet `MeetingEchoDelayEstimator.swift` (122, tests 230). FluidVoice `Meeting/MeetingSummaryProvider.swift:151-171` and MacParakeet `Models/Prompt.swift:286+` prompts. OpenLess `crates/micyou-dsp` (GPLv3 + exception). | **keep current; add** FluidVoice's AEC3 C bridge as the v2 path if RT-34 says real-time AEC is needed (rebuild the `.a` from source), plus the summary-prompt guard lines | The AEC3 bridge is shorter than wrapping `webrtc-audio-processing` ourselves (inf). | = now; v2 AEC ≈ −1 week | E-04, E-13, E-14, E-20, E-22, E-205 |
| Release pipeline, distribution | tauri-action, Homebrew tap | Voquill per-flavour `tauri.{dev,prod,enterprise}.conf.json` + `release/README.md` channel promotion | **reference** for a later beta channel | — | = | AF-11, E-54, DEC-17 |
| Updater install-when-idle | tauri-plugin-updater; `download()` then `install()` when idle | MacParakeet `App/SparkleUpdateGuard.swift` (115, tests 61); FluidVoice `UpdateBundleValidator.swift` / `UpdateRollbackBackup.swift` | **keep current; reference** the guard cases (blocks during a recording, vetoes relaunch, skips dev builds) | Trivial to mirror in TS. | = | E-53, E-215, AF-11 |
| Crash / diagnostics | panic hook + log + `.ips` pickup | Voquill `apps/desktop/src-tauri/src/utils/log_sanitizer.rs` (167, **Rust**, 14 tests, regex redaction). MacParakeet `MacParakeetObjCShims/MPKCrashSignalHandler.c` (510, async-signal-safe; not its uploader). FluidVoice `Services/FileLogger.swift`. TypeWhisper `ErrorLogService.swift` report fields. | **add** (port) Voquill's log sanitizer into W-24 | Tested Rust for the redaction step of AF-18. | ≈ −80 lines | AF-18, E-73, E-78, E-79 |
| Upstream sync, i18n tooling | own workflows | none | **keep current** | — | = | AF-13, E-219 |
| **Windows / Linux ports** (later; not in R4 v1) | none chosen | **Voquill** (AGPL): `platform/windows/input.rs` (528: SendInput, modifier release, Win-key Start-menu quirk), `platform/windows/accessibility.rs` (2,149, UIA) + `jab.rs` (993, Java Access Bridge), `platform/linux/wl/compositor.rs` (840: registers hotkeys in GNOME/KDE/Sway/Hyprland/Cosmic), `linux/wl/input.rs` (wtype/ydotool), plus native pills. OpenLess `selection.rs` Windows path. Tambourine `audio_mute/windows.rs`. | **reference now; likely port later** | The biggest future saving found. No tests, and it uses `enigo 0.1` (survey). | later: −several weeks | E-92 (platform), DEC-22 |

---

## 3. Updated "write ourselves" list

Changes against R4 §3. Items not listed are unchanged.

| # | Item | R4 size | Now covered by | New size |
|---|---|---|---|---|
| W-01 | LLM supervisor | ~300 lines, M | Port from TypeWhisper `LlamaCppServer.swift` + `LlamaCppRuntime.swift` (checksum and archive install logic only for a bundled binary), with `LlamaCppPluginTests.swift` as acceptance tests | ~300 lines, M, lower risk |
| W-03 | Prompt builder + guards | ~400 lines, M | + BW `TranscriptPolisher.accepted()`, TypeWhisper boundary wrapper, FluidVoice JSON-field framing; Voquill eval cases for RT-04 | ~350 lines, M |
| W-05 | AX extensions | ~300 lines, M | + VoiceInk text-marker reads, TypeWhisper fail-open and Gecko rules, SelectedTextKit (MIT) menu-Copy step, BW `WebsiteReader` (65 lines) | ~350 lines, M, but fewer unknown failures in RT-20/21/22 |
| W-06 | Hotkey fix + health | ~120 lines, S | Test vectors from MacParakeet and FluidVoice; recovery triggers from VoiceInk `ShortcutMonitor.swift` | ~120 lines, S |
| W-07 | Clipboard snapshot + selection fallback | ~120 lines, S | + paste verification (TypeWhisper `insertTextAtAndVerifyChange`, FluidVoice `PasteVerifier.swift`), layout-aware V key (MacParakeet / FluidVoice resolvers) | ~200 lines, S, closes A-53 |
| W-09 | Meeting glue | ~400 lines, M | + camera-in-use signal (TypeWhisper/MacParakeet CMIO collectors) | ~450 lines, M |
| W-13 | Dictionary rules, snippets, voice commands | ~600 lines + tables, M | Replacement stage ported from VoiceTypr `writing/library_rules.rs` (protected tokens, boundaries, provenance); spoken-punctuation tables from MacParakeet `SpokenPunctuation.swift` | ~350 lines own + port, M |
| W-24 | Panic hook, log redaction, diagnostics | ~250 lines, S | Redaction ported from Voquill `utils/log_sanitizer.rs` | ~170 lines, S |
| **W-27 (new)** | Dictionary auto-learn (D-52) behind an approval queue | was out of scope / DEC-20 open | Port BW `CorrectionWatcher` / `CorrectionLearner` / `CorrectionReviewer` (+ tests); VoiceInk `CorrectionDiffEngine` for multi-hunk diffs | ~400 lines, M (instead of ~700 from scratch) |
| **W-28 (new)** | Vocabulary import from Handy and Wispr Flow | — | Port TypeWhisper `AppVocabularyImport.swift` + `WisprFlowImportReader.swift` | ~250 lines, S |
| **W-29 (new)** | Native mute while dictating (replaces `osascript`) | — | Port Tambourine `audio_mute/macos.rs` (+ tests) | ~port, S |
| **W-30 (new)** | GPL compliance: LICENSE, SPDX headers, notices including GPL/AGPL sources, a source link per release | — | §4 checklist; extends W-26 | ~50 lines config + docs, S |

Net effect (inf):
- v1 dictation stays at roughly the R4 size (about 4,500 lines).
- Three features R4 had no part for (W-27..W-29) now cost about 650 lines instead of about 1,100.
- Risk drops most on AX, paste and the supervisor.

---

## 4. Obligations of going GPL-3.0 (checklist)

- [ ] **Licence file.** Add `LICENSE` with the full GPL-3.0 text. **Keep Handy's MIT notice** (MIT requires it), e.g. as `LICENSES/MIT-Handy.txt`, and say in the README that the combined work is GPL-3.0.
- [ ] **Version choice.** Several inputs are effectively GPL-3.0-only or AGPL-3.0-only: VoiceInk, MacParakeet, FluidVoice, Voquill, Tambourine, OpenLess, and the `selection` crate if ever used (§1). So:
  - license our own files as **GPL-3.0-or-later**;
  - state that the distributed app as a whole is **GPL-3.0**;
  - do not promise "or later" for the combined work (inf).
- [ ] **SPDX headers** on every new fork file (`// SPDX-License-Identifier: GPL-3.0-or-later`).
- [ ] **Ported files.** Ported or vendored files keep the original copyright and licence line, e.g. "Portions © VoiceInk contributors, GPL-3.0" or "© Federico Terzi". Each also gets a "Modified by <project>, <date>" notice (GPLv3 §5a).
- [ ] **AGPL parts.** Put code from Voquill, Tambourine, VoiceTypr, OpenLess and abstand in files marked `SPDX-License-Identifier: AGPL-3.0-only` (or `-or-later` for Epicenter), listed in NOTICE.
  - Combining them with GPL-3.0 is allowed (GPLv3 §13), but those files keep AGPL terms.
  - The network clause (AGPL §13) is not triggered by a local app. **It would be triggered if we ever add a network-reachable API.** Keep any local API bound to loopback, and offer source in the UI if that changes (inf).
- [ ] **Source offer (§6).**
  - Every GitHub release publishes the matching tagged source and build scripts, including the CI `llama-server` build and the overlay config.
  - Homebrew and any mirror point to the same release.
  - Anything we ship prebuilt that contains GPL code must be buildable from the repo: for example a rebuilt FluidVoice AEC3 `.a`, or the Swift bridge objects.
- [ ] **No additional restrictions.**
  - We cannot add terms that limit GPL rights.
  - Model weights and fonts are separate works with their own terms ("aggregate"). Keep them in the §5 register of R4, and do not bundle any that forbid redistribution in another app.
  - Strip the licence-key gates we port (Polar, LemonSqueezy, VoiceTypr `commands/license.rs`).
- [ ] **Attribution list.**
  - Extend W-26: run `cargo-about` for crates and an npm licence list, and add a **manual list of ported GPL/AGPL/MIT sources** with repo, commit and files. Use the per-row sources in §2 above.
  - Also list the NOTICE files: text-processing-rs, the WebRTC PATENTS file if AEC3 is used, and OFL fonts.
  - Show the list on the About / Acknowledgements page.
- [ ] **Trademarks.** Do not use the TypeWhisper (`TRADEMARK.md`), VoiceInk, FluidVoice, MacParakeet or Voquill names, icons or UI strings. Wispr Flow assets stay off-limits.
- [ ] **Upstream contributions to Handy (MIT).** Any PR we send upstream (R4 §8 Q9) must contain only code we wrote and agree to license MIT. **Never GPL-derived code.** Keep upstream-bound fixes, such as the `HandyKeysState::new` error path, in separate clean files.
- [ ] **Contributors.** Adopt a DCO sign-off, not a CLA (inf, owner decision), so contributions arrive as GPL-3.0-or-later.
- [ ] **CI guard.** Extend the analytics grep guard to the SDKs found in GPL sources: PostHog, Mixpanel, Aptabase, TelemetryDeck, Firebase. Add a check that every file under `fork/` has an SPDX header.
- [ ] **Mac App Store** remains out: GPL conflicts with App Store terms (inf). It was already rejected for sandbox reasons.

---

## 5. Still not usable, and why

| Item | Why |
|---|---|
| Wispr Flow code, assets, prompts, sounds, icons | Proprietary; behaviour only (owner rule). |
| VoiceInk Refine weights (`beingpax/VoiceInk-Refine-V1`, "VoiceInk only"); FluidVoice Fluid-1 (`license:other`, non-commercial) and the "Fluid Intelligence" runtime (closed); FluidVoice `models.fluidvoice.app` CoreML tars | Model and runtime terms are separate from the code licence (R3 §7; `PrivateAIProvider.swift`). |
| TypeWhisper Supertonic (OpenRAIL-M); MacParakeet Orukeet (CC-BY-SA-4.0) and Nemotron (OpenMDW) | Separate model terms; not wanted as defaults (owner check if ever). |
| Voquill `enterprise/` | Proprietary "Voquill Enterprise License" (`enterprise/LICENCE`). |
| `selection` crate (pot-app) macOS code | Copied from `yetone/get-selected-text`, which has no licence. Also uses AppleScript. |
| shepherd.js, intro.js | AGPL, but their licence files claim commercial users must buy a licence. That adds dispute risk with no feature gain over driver.js (MIT). |
| Whispering v7 Zapsplat sound files | Zapsplat licence, not AGPL (inf). |
| Product names, icons, branding of TypeWhisper / VoiceInk / FluidVoice / MacParakeet / Voquill | Trademarks (`TRADEMARK.md` in TypeWhisper); GPL does not grant marks. |
| Telemetry, feeds and licensing modules: FluidVoice `Sources/Fluid/Analytics/*`, MacParakeet `Services/Telemetry/*` and `DiscoverService.swift`, VoiceInk `AnnouncementsService.swift` and `LicenseManager.swift`, Voquill Firebase/Mixpanel/Stripe, Whispering v7 Aptabase | Licence allows them, product rules forbid them (no telemetry, no account, zero network by default: AF-12, E-73). |
| AppleScript / `osascript` paths: VoiceInk `BrowserURLService.swift` and `.scpt` files, TypeWhisper `BrowserURLResolver.swift`, FluidVoice `CommandModeService.swift` | Licence allows them; they trigger Automation prompts (R3 §7). |
| MediaRemote private-framework adapters (VoiceInk, TypeWhisper, FluidVoice, MacParakeet) | Private API; not needed for v1 (inf). |
| Code whose licence field and file disagree: Voquill `rust_transcription` (MIT field, AGPL file), VoiceTypr `keytrigger` (MIT field, no file) | Usable only **as AGPL**. Never relabel them MIT. |
| GPL-2.0-only code | Incompatible with GPL-3.0. None found among the candidates; check any future Linux/X11 tool. |
| anarlog `enterprise/`, OpenWhispr cloud defaults, other R4 §4 rejections made for non-licence reasons (stack, quality, permissions) | The licence change does not affect them. |

---

## Summary of recommended swaps

1. **LLM supervisor (W-01):** port TypeWhisper's `LlamaCppServer.swift` (flags, env scrub, parent-death watchdog) and use its 814-line test file as the spec. Keep the bundled, signed sidecar.
2. **Dictionary replacement stage (W-13):** port VoiceTypr's Rust `library_rules.rs` (17 tests, protected URLs and e-mails, provenance). Keep our own matcher for inline snippets.
3. **Dictionary auto-learn (new W-27):** port BetterWispr's tested `CorrectionWatcher` / `CorrectionLearner` / `CorrectionReviewer`, with VoiceInk's diff engine, behind user approval.
4. **Vocabulary import (new W-28):** port TypeWhisper's Handy and Wispr Flow importers.
5. **Mute while dictating (new W-29):** port Tambourine's tested Rust CoreAudio mute and restore in place of `osascript`.
6. **Log redaction (W-24):** port Voquill's Rust `log_sanitizer.rs` (14 tests).
7. **Adds, no swaps:** VoiceInk text markers and TypeWhisper's fail-open AX rules; TypeWhisper/FluidVoice paste verification; MacParakeet hotkey and onboarding tests as specs; BW `WebsiteReader` and `WritingMode`.
8. **Keep:** every other R4 pick. GPL obligations (§4) add one small item, W-30: licence, SPDX headers, AGPL file marking, source per release, attribution list.
