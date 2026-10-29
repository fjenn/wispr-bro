# R4 verification log

Independent verification of `R4a-local-llm.md`, `R4b-macos-integration.md`, `R4c-text-data-ui.md` and `R4d-notetaker-distribution.md`, done on 2026-10-28 before writing `docs/research/R4-parts-bin.md`.

**Method.** I read the four files fully, R2 §1-4 and §8-9, and the three R3 files (for overlaps). For each check I opened the primary source myself: LICENSE files in the researcher clones (`scratchpad/clones/r3*`, `r4*`), crate sources in `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/` and `~/.cargo/git/checkouts/`, Handy's `Cargo.toml`, `Cargo.lock`, `package.json`, `catalog.json`, and the source files cited. Remote metadata came from the crates.io API, the npm registry, the Hugging Face model API, the ModelScope API and read-only `gh api` calls. One file was downloaded: the sherpa-onnx `pyannote-segmentation-3-0` tarball. It went into its own empty folder (`scratchpad/verify-seg30/`), and I only listed it and read its `LICENSE`.

I did not build, install or run anything from a clone. I created nothing on GitHub and did not open `docs/research/evidence/`.

**Result key.** PASS means the claim matches the source. FAIL means it does not, and a correction follows. PARTIAL means the claim is true but leaves out something that matters. NEW means a finding no R4 file reported.

---

## 1. Licenses of recommended items

| # | Item | Evidence opened | Result | Note / correction |
|---|---|---|---|---|
| L-01 | llama.cpp | `r4a-llamacpp/src/LICENSE` ("MIT License, Copyright (c) 2023-2026 The ggml authors"); `LICENSE` inside release `llama-b11972` arm and x64 tarballs; GitHub SPDX MIT | PASS | Vendored `cpp-httplib` is in `vendor/`. The release macOS build turns on `-DLLAMA_BUILD_BORINGSSL=ON` (`release.yml:150`). Our own build must turn TLS off (R4a already proposes this), which keeps BoringSSL's licence out of the bundle. |
| L-02 | Qwen3.5-2B weights | `r4a-model-licenses/Qwen_Qwen3.5-2B.LICENSE` = Apache 2.0 text (11,544 bytes); HF API `cardData.license = apache-2.0`, link to repo LICENSE, not gated | PASS | |
| L-03 | Qwen3-4B-Instruct-2507 weights | `Qwen_Qwen3-4B-Instruct-2507.LICENSE` = Apache 2.0 text (11,343 bytes); HF API apache-2.0 | PASS | |
| L-04 | GGUF repos for both | HF API `unsloth/Qwen3.5-2B-GGUF` and `unsloth/Qwen3-4B-Instruct-2507-GGUF`: apache-2.0, license link points to Qwen's LICENSE. Q4_K_M sizes 1,280,835,840 and 2,497,281,120 bytes (match R4a). `Qwen/*-GGUF` and `ggml-org/*-GGUF` for these two models return HTTP 401 (no first-party GGUF) | PASS / NEW | No first-party GGUF exists, so the community quant is the source. Pin revision and sha256. `lmstudio-community/Qwen3-4B-Instruct-2507-GGUF` is a second source. |
| L-05 | Qwen2.5-3B rejection | `Qwen_Qwen2.5-3B-Instruct.LICENSE` starts "Qwen RESEARCH LICENSE AGREEMENT" | PASS | |
| L-06 | hex (anomalyco, Rust) | `r4a-hex/src/LICENSE` and `r3b-hex-rust/LICENSE`: "MIT License, Copyright (c) 2026 Christopher Langton"; git remote `anomalyco/hex`; Cargo `license = "MIT"` | PASS | |
| L-07 | Hex legacy (kitlangton, Swift) | `r3b-hex-legacy/LICENSE`: MIT, © 2025 Kit Langton; GitHub `archived: true` | PASS / NEW | R4a's "Hex" (anomalyco/hex, Rust) and R4b's "Hex" (kitlangton/Hex, Swift) are **different projects** by different authors. Both are MIT. The parts bin names them "hex (Rust)" and "Hex legacy (Swift)". |
| L-08 | anarlog and its `enterprise/` exclusion | Root `LICENSE` (MIT, © 2023-present Fastrepl); `LICENSING.md` path table: `enterprise/**` is commercial, everything else MIT "unless a nearer license … says otherwise"; `LICENSE.enterprise` | PASS | I checked for nearer LICENSE files in every cited path (`crates/shortcut-macos`, `plugins/overlay`, `crates/apple-calendar`, `crates/template-app`, `crates/detect`, `crates/pyannote-local`, `crates/embedding`, `crates/aec`, `crates/tcc`). None has one, so all are MIT. Other LICENSE files exist only in `enterprise/`, `docs/`, `agent-plugins/anarlog/`, a mobile assets folder, `crates/tiptap/vendor/…` and `apps/desktop/src/settings/stats/tremor/`, none of which we use. |
| L-09 | axwatch 0.1.0 | Crate `LICENSE-MIT` (© 2026 AprilNEA) + `LICENSE-APACHE`; Cargo `MIT OR Apache-2.0`; GitHub SPDX Apache-2.0 | PASS | |
| L-10 | mado 0.0.21 | Crate `LICENSE` MIT (© 2024 @bennobuilder); crates.io MIT | **PARTIAL / NEW** | The source repo `builder-group/abstand` has a root `LICENSE` that says "covered by the GNU Affero General Public License v3.0 … unless a file header or a license file in a subdirectory specifies another license". `docs/decisions/license.md` records AGPL as the repo default. Only `crates/mado/LICENSE` is MIT. **Correction:** port only from the crates.io 0.0.21 tarball (or `crates/mado/`), never from elsewhere in that repo, and record the provenance. Also do not port `WebsiteInfo.swift`: it builds a `https://www.google.com/s2/favicons` URL, which would be a network call. |
| L-11 | flexaudio-os-macos 0.8.1 | Crate `LICENSE` MIT (© 2026 tubome / Studio Sadola); flexaudio-core 0.8.1 MIT (crates.io); GitHub MIT | PASS | Now the alternative, not the choice (see R-06). |
| L-12 | SpeakoFlow files | `r3a-speakoflow/repo/LICENSE`: MIT, © 2026 Abhishek Barali + © 2025 CJ Pais; commit 967857fa 2026-10-21 | PASS | |
| L-13 | objc2-application-services 0.3.2 | Crate `Cargo.toml:37` `Zlib OR Apache-2.0 OR MIT`; the crate tarball ships **no** LICENSE file; the repo `madsmtm/objc2` has `LICENSE-ZLIB.txt`, `LICENSE-APACHE.txt`, `LICENSE-MIT.txt`, `LICENSE.md` (GitHub SPDX shows NOASSERTION because there are three) | PASS | |
| L-14 | objc2-event-kit 0.3.2 | crates.io `Zlib OR Apache-2.0 OR MIT`, MSRV 1.71 | PASS | |
| L-15 | text-processing-rs | `LICENSE` Apache 2.0; `NOTICE` credits NVIDIA NeMo (Apache-2.0, pinned commit 1f12635…); `THIRD-PARTY-LICENSES.md` lists rustfst (MIT/Apache) and flate2; GitHub `NVIDIA/NeMo-text-processing` SPDX Apache-2.0 | PASS | The Apache NOTICE must ship in our third-party notices. crates.io's newest version is 0.2.2; the repo is at 0.3.0. |
| L-16 | text2num | `r4c-text2num/LICENSE` MIT, © 2021-2024 Groupe Allo-Media; crates.io MIT | PASS | |
| L-17 | aho-corasick | crates.io `Unlicense OR MIT`; GitHub Unlicense; already in `Cargo.lock` 1.1.4 | PASS | |
| L-18 | similar | `r4c-similar/LICENSE` Apache 2.0; crates.io Apache-2.0 3.2.1, MSRV 1.85 | PASS | |
| L-19 | papaparse | npm 5.7.0 MIT; GitHub MIT | PASS | |
| L-20 | driver.js | `r4c-driverjs/license` "The MIT License, Copyright (c) Kamran Ahmed"; npm 1.10.2 MIT | PASS | |
| L-21 | react-activity-calendar | `LICENSE` MIT © 2021 Jonathan Gruber; npm 3.2.1 MIT, peer React `^18 \|\| ^19`; dependencies date-fns and @floating-ui/react are MIT (npm) | PASS | |
| L-22 | Recharts | npm 3.10.1 MIT, peer React 18 OK; dependencies immer, react-redux, es-toolkit are MIT; GitHub MIT | PASS | |
| L-23 | TanStack Virtual | npm `@tanstack/react-virtual` 3.14.23 MIT; virtual-core MIT; GitHub MIT | PASS | |
| L-24 | Radix | npm dropdown-menu 2.2.1, popover 1.3.1, tabs 1.2.1: MIT, peer React 18 OK; GitHub MIT | PASS | |
| L-25 | sonner | `node_modules/sonner/LICENSE.md` present; npm 2.0.8 MIT; GitHub MIT; already imported in `App.tsx:9` | PASS | |
| L-26 | canvas-confetti | npm 1.9.4 ISC; GitHub ISC | PASS | |
| L-27 | Newsreader font | GitHub `productiontype/Newsreader` OFL-1.1; npm `@fontsource-variable/newsreader` 5.3.0 OFL-1.1 | PASS | |
| L-28 | Source Serif 4 font | GitHub `adobe-fonts/source-serif` OFL-1.1; npm `@fontsource-variable/source-serif-4` 5.3.1 OFL-1.1 | PASS | |
| L-29 | sherpa-onnx code | `r4d-sherpa-onnx/LICENSE` Apache 2.0; `rust/sherpa-onnx/Cargo.toml` `license = "Apache-2.0"`, version 2.0.1; GitHub Apache-2.0 | PASS | |
| L-30 | pyannote segmentation-3.0 weights | HF API `license: mit`, `gated: auto`. The sherpa tarball `sherpa-onnx-pyannote-segmentation-3-0.tar.bz2` (6,958,444 bytes) contains `LICENSE` = "MIT License, Copyright (c) 2022 CNRS" (downloaded and read) | PASS | |
| L-31 | 3D-Speaker CAM++ / ERes2Net (en, VoxCeleb) | ModelScope API `License: Apache License 2.0` for `iic/speech_campplus_sv_en_voxceleb_16k` and `iic/speech_eres2net_sv_en_voxceleb_16k`; sherpa release assets 29.6 MB and 26.5 MB; GitHub `modelscope/3D-Speaker` Apache-2.0 | PASS / NEW | The weights are Apache-2.0, but they were trained on VoxCeleb (a CC BY 4.0 dataset). We should credit VoxCeleb as a courtesy, not as a licence duty. The models are hosted on **github.com** releases, which would add a new runtime network endpoint (AF-12). Mirror them to our own HF repo with the LICENSE file. |
| L-32 | whisper-base weights | HF API `openai/whisper-base` and `handy-computer/whisper-base-gguf`: apache-2.0 | PASS | |
| L-33 | sysinfo | `r4d-sysinfo/LICENSE` MIT © 2015 Guillaume Gomez; crates.io 0.39.6 / 0.40.0 MIT | PASS / NEW | **MSRV is 1.95**, which R4d does not mention. That is fine: CI uses `dtolnay/rust-toolchain@stable` and the local toolchain is 1.99.1. |
| L-34 | Moonshine catalog mislabel | `catalog.json`: all 12 of `moonshine-{tiny,base}-{ar,ja,ko,uk,vi,zh}` say `"license": "mit"`. HF API for `handy-computer/moonshine-tiny-ja-gguf` and `…-base-ar-gguf`: `license: other`, `license_name: moonshine-community-license`. The upstream `moonshine-ai/moonshine/LICENSE` says the "legacy non-streaming models for languages other than English … remain under the Moonshine Community License, a non-commercial license" and lists Arabic, Japanese, Korean, Mandarin, Spanish Base, Ukrainian and Vietnamese | **PASS (claim confirmed)** | R4d's finding is correct. English Tiny/Base and every streaming Moonshine model are MIT. |
| L-35 | parakeet-tdt_ctc-110m | HF API `handy-computer/parakeet-tdt_ctc-110m-gguf`: cc-by-4.0 | PASS | Attribution duty. |
| L-36 | Hex legacy / FreeFlow / OpenWhispr / Tolkovo / Meetily / transcribe.cpp / tauri-action / pyannote-rs / Applite / hyprwhspr-rs / jsdiff / rusqlite_migration | LICENSE files read: FreeFlow MIT © 2026 Zach Latta; OpenWhispr MIT © 2024 OpenWhispr Team; Tolkovo MIT © 2025 CJ Pais + © 2026 kozliq; Meetily MIT; transcribe.cpp MIT; tauri-action MIT; pyannote-rs MIT; Applite MIT; hyprwhspr-rs MIT-text; jsdiff BSD-3; rusqlite_migration Apache-2.0 | PASS | |
| L-37 | webrtc-audio-processing (v2 option only) | crates.io shows `license: non-standard`; repo `COPYING` is the Google BSD-3 WebRTC text | PARTIAL | R4d's "BSD-3-Clause" is right, but crates.io does not show it. If adopted, ship `COPYING`, and check whether a WebRTC `PATENTS` file applies. |
| L-38 | eslint-plugin-jsx-a11y | npm 6.10.2 MIT, but it **depends on `axe-core` (MPL-2.0)** | **FAIL (omission)** | R4c lists it as plain MIT. **Correction:** it brings MPL-2.0 `axe-core` into dev dependencies. That is the same owner decision as `@axe-core/playwright` (also MPL-2.0). Neither ships in the app. |
| L-39 | Muesli cleanup fine-tunes (new challenger) | HF API `Muesli-HQ/qwen35-2b-muesli-cleanup-v12-gguf` and `…-0.8b-…`: apache-2.0, base Qwen3.5, no dataset listed | NEW | Usable as bake-off challengers. Training data is undocumented, so they cannot be a silent default until provenance is known. |
| L-40 | SpeakoFlow Mini (R3a challenger) | HF API `SpeakoFlow/speakoflow-mini`: apache-2.0, dataset `SpeakoFlow/dictation-cleanup-examples` = cc-by-4.0 | PASS | Credit the CC-BY dataset in the notices. |

## 2. Version and compatibility claims

| # | Claim | Evidence | Result | Correction |
|---|---|---|---|---|
| V-01 | tauri-nspanel pin `da9c9a8` is 25 commits behind `v2.1`; 2.1.1 is on crates.io | `Cargo.lock:6545-6547` (git v2.1 #da9c9a8…); `gh api compare da9c9a8...v2.1` → ahead_by 25 with the three named fixes; `…v2.1.1` → 23 ahead; crates.io 2.1.1 (2026-10-08) needs `tauri ^2.8.5`, objc2 ^0.6.1, objc2-app-kit ^0.3.1, plus a new `pastey ^0.2` | PASS | Compatible with Tauri 2.11.5. Tolkovo pins the same commit, so its pill code is proven on the current pin. |
| V-02 | tauri-plugin-notification: "pin `~2.3` on Tauri 2.11, or bump Tauri" | crates.io deps: 2.3.3 → tauri ^2.8.2; **2.4.0 (2026-08-31) → tauri ^2.10**; 2.5.0-2.8.0 → tauri ^2.12 (MSRV 1.90) | **FAIL** | **Correction:** the newest version that works with Tauri 2.11.5 is **2.4.0**. Pin `tauri-plugin-notification = "~2.4"` and `@tauri-apps/plugin-notification` 2.4.x. |
| V-03 | arboard `exclude_from_history` in 3.6.1 | `arboard-3.6.1/src/platform/osx.rs:276-292,403-412` writes `org.nspasteboard.ConcealedType`; `lib.rs:200,228` `html()` | PASS | arboard is only transitive today (no line in `Cargo.toml`), so making it direct adds no new code. |
| V-04 | reqwest Unix-socket support in the locked version | `Cargo.lock` reqwest 0.12.28 (and 0.13.2); `reqwest-0.12.28/src/async_impl/client.rs:1808-1809` `#[cfg(unix)] pub fn unix_socket` | PASS | No feature flag is needed (R4a's "verify the feature flag" is now answered). |
| V-05 | objc2-application-services has no attribute-name constants | `AXAttributeConstants.rs` is 44 lines; `AXUIElement.rs` has `copy_attribute_value:312`, `set_attribute_value:675`, `copy_parameterized_attribute_value:965`, `set_messaging_timeout:1445` | PASS | |
| V-06 | rdev declared but unused | `Cargo.toml:49` git rustdesk-org/rdev; `Cargo.lock` rdev 0.5.0-2; `grep -rn rdev src-tauri/src` finds nothing | PASS | `AGENTS.md` still lists rdev (stale). |
| V-07 | `@tauri-apps/plugin-sql` unused | `package.json:40` `~2.3.1`; `grep plugin-sql src/` finds nothing; no `tauri-plugin-sql` in `Cargo.toml` | PASS | |
| V-08 | tauri-action `@v0` vs v1 | `build.yml:450` `tauri-apps/tauri-action@v0`, `:479` `assetNamePattern`; `r4d-tauri-action/CHANGELOG.md` lists the breaking renames (`releaseAssetNamePattern`, `uploadUpdaterJson`, removal of includeDebug/includeRelease) | PASS | |
| V-09 | Handy lock baseline | objc2 0.6.4, objc2-foundation/app-kit/core-audio/core-foundation/user-notifications 0.3.2, block2 0.6.2, libc 0.2.182, aho-corasick 1.1.4, unicode-segmentation 1.12.0, zip 4.6.1, strsim 0.11.1, whatlang 0.16.4, nix 0.29.0 + 0.30.1, rusqlite 0.37.0, rusqlite_migration 2.3.0, tauri 2.11.5, tauri-plugin-updater 2.10.1, tauri-plugin-log 2.8.0, specta 2.0.0-rc.22; absent: objc2-application-services, objc2-event-kit, similar, sysinfo, csv, sherpa-onnx, tauri-plugin-notification | PASS | |
| V-10 | flexaudio exact pins | `Cargo.toml` `block2 =0.6.2`, `libc =0.2.186`, `objc2 =0.6.4`, `objc2-core-audio =0.3.2`, `objc2-core-foundation =0.3.2`, `objc2-foundation =0.3.2` | PASS | The libc pin would force Handy's 0.2.182 up to 0.2.186. This is one reason to prefer hex `meeting_capture.rs` (see R-06). |
| V-11 | llama-server flags and defaults | `tools/server/README.md`: `--port` default 9931 (:194); `--cache-ram` default 8192 (:168); `-cms` default 8192 (:167); `--host` accepts `.sock` (:193); `--api-key` / `LLAMA_API_KEY` (:215); `--no-webui` (:210, default on); `--sleep-idle-seconds` (:248, :2549); `reasoning_effort: none` (:1449); `// connection is closed` (`server-context.cpp:5397`) | PASS | |
| V-12 | llama-server accepts Handy's request shape | `server-common.cpp:1378-1402` reads `response_format.type == "json_schema"` → `json_schema.schema`; Handy sends `{"type":"json_schema","json_schema":{"name":"transcription_output","strict":true,"schema":…}}` (`llm_client.rs:412-420`) | PASS / NEW | `strict` is ignored, which is harmless. Handy's schema field is `transcription`, not `text` as in R4a §4.2. Keep Handy's name. |
| V-13 | Minimum macOS of the sidecar | Release builds use `-DCMAKE_OSX_DEPLOYMENT_TARGET=13.3` (`release.yml:106,118`). R4d proposes app minimum 13.0. ggml-metal uses runtime `@available(macOS 15.0, …)` checks (`ggml-metal-device.m:1002,1023`) | **NEW (cross-file inconsistency)** | **Correction:** build our sidecar with `CMAKE_OSX_DEPLOYMENT_TARGET=13.0`, matching the app minimum, and check it on a macOS 13 machine (RT-10). Otherwise the sidecar refuses to launch on 13.0-13.2. |
| V-14 | sysinfo MSRV | crates.io 0.39.6 and 0.40.0: `rust_version 1.95` | NEW | OK on stable (see L-33). |
| V-15 | Homebrew notability | `Package-Acceptance-Policy.md:32-38`: 30 forks / 30 watchers / 75 stars; self-submit 90/90/225; repo under 30 days old is normally not eligible | PASS | |
| V-16 | "A fork cannot replace the original's cask" (R4d) | `Acceptable-Casks.md:67-71`: "A fork may replace the original cask when it meets the shared fork criteria" | FAIL (minor) | The statement is wrong in general but does not matter to us: we ship under a distinct name. |
| V-17 | Handy's homebrew cask | `Casks/h/handy.rb`: `depends_on macos: :ventura`, `auto_updates true` (line 38), zap includes `~/Library/LaunchAgents/Handy.plist` | PASS | Our template should also zap a LaunchAgents plist if one is ever written. |
| V-18 | `asset-prefix` has no effect today | `build.yml:245-256` applies the prefix only when `asset-name-pattern` is set; `release.yml` passes `asset-prefix: "handy"` but no pattern | PASS | |

## 3. Code claims (file:line opened)

| # | Citation | Result |
|---|---|---|
| C-01 | hex `src/apple_intelligence.swift:141-160` `blocking(timeout:)` with `task.cancel()` | PASS (lines 141-160) |
| C-02 | hex `apple_intelligence.swift:171-192` availability 0-5; `:194-202` contextSize; `:80-97` permissive guardrails, `temperature: 0.2` | PASS |
| C-03 | hex `src/apple_intelligence.rs:80-93` 10 s availability cache | PASS (`AVAILABILITY_TTL`) |
| C-04 | Handy `apple_intelligence.swift:80,118` semaphore wait with no timeout | PASS |
| C-05 | Handy `actions.rs:256-272` synchronous Apple call; `model` parsed as token limit | PASS |
| C-06 | Handy `actions.rs:154-172` `complete_unless_cancelled` | PASS |
| C-07 | Handy `actions.rs:340-350` pastes raw content on a parse failure | PASS |
| C-08 | Handy `llm_client.rs:79-84` `reasoning_effort: "none"`; `:184-196` Referer/User-Agent/X-Title headers | PASS |
| C-09 | Handy `settings.rs:798-812` custom provider `supports_structured_output: false`, base `localhost:11434`; `:873` `ensure_post_process_defaults` | PASS |
| C-10 | FreeFlow `PostProcessingService.swift:42`, `:117` prompts; `TranscriptTextCore.swift:128-182` heuristic | PARTIAL: the detector returns false unless the raw transcript has an English instruction word (`ask`, `write`, `email`, …), and only when `outputLanguage` is empty (`:133`, `:139-145`). It is narrower than R4a says. |
| C-11 | OpenWhispr `src/locales/en/prompts.json` keys; `src/helpers/llamaServer.js:220-233` ctx and cache-ram comments | PASS (`cleanupPrompt` is 2,227 chars) |
| C-12 | SpeakoFlow `actions.rs:1283-1460` validators (`transcription_allows_empty_output`, `validate_cleaned_output`, `is_implausibly_long`, `validate_cleaned_output_against_prompt`, `is_leaked_channel_name`, `echo_words`) | PASS |
| C-13 | SpeakoFlow `smart_spacing/macos.rs` (450 lines) on `objc2-application-services =0.3.2`: AXManualAccessibility lease (:221, :323), terminal exclusion (:314), `AXSecureTextField` (:348), `AXStringForRange` (:187), `AXSelectedTextRange` (:359), messaging timeout (:216) | PASS |
| C-14 | handy-keys `listener.rs:172-193` re-enable in callback, no polling (#1827); `:446-475` `SessionEventTap`; `manager.rs` public API | PASS (`get_hotkey` and `hotkey_count` also exist) |
| C-15 | Handy `shortcut/handy_keys.rs:86-120` returns `Ok` before the tap exists; the error is only logged | PASS |
| C-16 | anarlog `shortcut-macos`: `processor.rs` 311 lines, `tests/processor.rs` 605, deps serde/specta/thiserror only, `KCG_HID_EVENT_TAP` (`tap.rs:117,237`), constants 300 / 1000 / 300 / 150 ms (`decision.rs`); no Hex credit (grep) | PASS |
| C-17 | Hex legacy `Constants.swift:22,40,53` (0.3 / 0.3 / 1.0 s); `KeyEventMonitorClient.swift:117,393-395` 100 ms `tapIsEnabled` watchdog | PASS |
| C-18 | Hex legacy `PasteboardClient.swift:51-82` snapshot; `:350-380` `insertTextAtCursor` | PASS |
| C-19 | Handy `clipboard.rs:78-92` and `paste_tx/macos.rs:270-278` save text, or an image only when there is no text | PASS |
| C-20 | Handy `overlay.rs:450-480` PanelBuilder with `focusable(false)`, no `accept_first_mouse`; `:416` the non-macOS window sets it | PASS |
| C-21 | tauri-nspanel `builder.rs:800-808` `with_window`; `:629-636` ignores/accepts mouse; no `accept_first_mouse` in the crate | PASS |
| C-22 | tauri 2.11.5 `webview_window.rs:899` `accept_first_mouse`; wry 0.55.1 `wkwebview/mod.rs:309`; `menu/menu.rs:142` `Menu::default`, `:390` `set_as_app_menu`; `app.rs:660` `set_dock_visibility`; `webview_window.rs:2227` `set_content_protected`; tauri-utils `acl/capability.rs:85,150` globs | PASS |
| C-23 | objc2-app-kit `NSApplication.rs:1883` screen-parameters notification; `NSWorkspace.rs:298-303` frontmost / menuBarOwning | PASS |
| C-24 | handy-cpal `loopback.rs:84-110` `initWithProcesses_andDeviceUID_withStream` + `setExclusive(true)`; README: loopback needs 14.6+ | PASS |
| C-25 | objc2-core-audio `AudioHardware.rs:1302,1766,1885-1887` | PASS |
| C-26 | Handy `Info.plist`: only `NSMicrophoneUsageDescription`; `Entitlements.plist`: microphone and audio-input only | PASS |
| C-27 | mado `BrowserURLExtractor.swift:4-30` document URL first, address bar only when not focused; 116 + 47 lines | PASS |
| C-28 | Tolkovo `tolkovo/pill.rs` (806 lines): `PanelLevel::Status` (:198), `hides_on_deactivate(false)` (:204), `accept_first_mouse(true)` (:211, :255), `_setPreventsActivation:` (:493-497), Space-change observer (:510-527) | PASS |
| C-29 | anarlog `plugins/overlay/src/lib.rs:164-250` cursor polling toggles `set_ignore_cursor_events` | PASS |
| C-30 | text-processing-rs `lib.rs:1178-1183` `split_whitespace` pretokenize; `custom_rules.rs:13-16` global RwLock; `itn/en/punctuation.rs:37-50` period/dash/percent/plus; `tests/en_tests.rs:420-426` | PASS |
| C-31 | Handy `text.rs:163,326,331,474` whitespace collapse; `transcription.rs:1600-1662` stage order inside `fail_open_text_transform`; `Cargo.toml:45` `rusqlite_migration = "2.3"`; `history.rs:5,22,40` | PASS |
| C-32 | transcribe-cpp `diarize.rs:117` `run(&mut self, pcm, options)`; `catalog/mod.rs:319-320` keeps role `asr` only; `protocol.rs:23` has no diarize request | PASS |
| C-33 | sherpa-onnx `rust/sherpa-onnx-sys/build.rs:13,45,115,126,137,141` downloads prebuilt libs from GitHub releases unless `SHERPA_ONNX_LIB_DIR` is set; default feature `static`; `offline_speaker_diarization.rs:72-108` `FastClusteringConfig { num_clusters: -1, threshold: 0.5 }` | PASS |
| C-34 | sysinfo `src/unix/apple/disk.rs:8,120,285` `kCFURLVolumeAvailableCapacityForImportantUsageKey` | PASS |
| C-35 | Handy `UpdateChecker.tsx:143,162` `downloadAndInstall` then `relaunch()`; `release.yml:21` greps the version; `:35` `draft: true`; `build.yml:454` `APPLE_ID_PASSWORD` | PASS |
| C-36 | Handy `lib.rs:58` `FILE_LOG_LEVEL = Debug`; `settings.rs:1108` `debug!("Loaded settings: {:?}")`; `Cargo.toml:206-207` `strip = true`, `panic = "unwind"` | PASS |
| C-37 | `engine_supervisor/mod.rs:1-15` worker = same executable; `supervisor.rs:45` stderr tail | PASS |
| C-38 | hex `src/meeting_capture.rs:418-426` `initMonoGlobalTapButExcludeProcesses` with our process object on objc2-core-audio 0.3.2 (no exact pin), `:733-760` ScreenCaptureKit fallback; `src/microphone_activity.rs:6-14` uses `kAudioHardwarePropertyProcessObjectList`, `kAudioProcessPropertyIsRunningInput`, `…BundleID`, `…PID`; `src/meeting_echo.rs` (534 lines) signal-level bleed classifier | NEW (R3b found these; R4 did not use them) |

**Code-claim total:** 38 references opened. 37 pass, 1 partial (C-10), 0 fail.

## 4. Fit with R2 constraints

| # | Check | Result |
|---|---|---|
| F-01 | AF-01: no second ggml in Handy's binary or worker | PASS. llama-server is its own process. sherpa-onnx links onnxruntime, not ggml. A link test is still listed (RT-33). |
| F-02 | AF-12: no network calls by default | PARTIAL. Every pick is offline at runtime **if** (a) llama-server is never given `-hf`/`-mu`, is built without TLS/curl, and runs with `--no-webui`; (b) sherpa diarization models are mirrored to our HF repo rather than fetched from github.com (L-31); (c) we never port mado `WebsiteInfo.swift` (L-10); (d) `i18next-cli` locize commands stay unused (dev only); (e) the sherpa-onnx `build.rs` download happens only in CI and is replaced by a vendored `SHERPA_ONNX_LIB_DIR`. The update check and model downloads remain disclosed, user-facing calls (AF-12, DEC-16). |
| F-03 | Nothing silently needs a Tauri version newer than 2.11.5 | **FAIL in R4b, corrected:** tauri-plugin-notification 2.5+ needs ^2.12, so use 2.4.0 (V-02). Everything else fits: tauri-nspanel 2.1.1 ^2.8.5; keep tauri-plugin-updater 2.10.1 and tauri-plugin-log 2.8.0 as locked. |
| F-04 | Minimum macOS gates stated | Added to the parts table: app 13.0 (DEC-22); llama-server built for 13.0 (V-13); process tap 14.4 (flexaudio `version.rs`; Apple API), while handy-cpal's own loopback says 14.6, which no longer matters once the tap is ported; EventKit full access 14.0; Apple FM 26 on Apple Silicon only; tauri-plugin-notification uses deprecated NSUserNotification through mac-notification-sys (works, check on 26). |
| F-05 | AF-13: fork code away from upstream hotspots | PASS with care: new code goes in fork modules. The one-line hooks into `lib.rs`, `actions.rs`, `overlay.rs`, `transcription.rs` and `settings.rs` are named per row. |
| F-06 | AF-19 / AF-20: models in the HF cache, machine probe | PASS (sysinfo + catalog reuse). |
| F-07 | No GPL/AGPL/LGPL code | PASS after L-10: mado must be taken from the MIT crate, not the AGPL monorepo. BetterWispr (R3b) has GPL-derived files, so it is not used here. |

## 5. Reconciliation decisions (across R4 files and with R3)

| # | Capability | R4 position | R3 position | Decision | Why |
|---|---|---|---|---|---|
| R-01 | AX client | R4b: write our own `ax.rs` (~250 lines) on objc2-application-services | R3a: SpeakoFlow `smart_spacing/macos.rs` (MIT) is a ready AX layer | **Adapt SpeakoFlow's file** as the base, then add selected-text read/write, bounds, window title and URL | Same crate and version, already handles AXManualAccessibility leases, terminals, secure fields and timeouts (C-13). Less new code. |
| R-02 | Hotkey state machine | R4b: port anarlog `shortcut-macos` | R3b: port Hex legacy `HotKeyProcessor` (Swift, 48 tests) | **anarlog `processor.rs` + `decision.rs` + tests**, keeping both the anarlog and the Hex MIT notices | Already Rust, three tiny deps, same constants as Hex (C-16, C-17). |
| R-03 | Tap health | R4b: axwatch + probe tap; do not poll | R3b: Hex 100 ms `tapIsEnabled` watchdog | **No polling.** Use handy-keys' callback re-enable, plus a wake/session-active re-check, plus an axwatch-style permission watch and probe tap | Polling conflicts with handy-keys' #1827 kernel-voucher leak. Measure on a real Mac (RT-19). |
| R-04 | Cleanup prompts | R4a: OpenWhispr + FreeFlow | R3a: SpeakoFlow prompts; R3b: Talkify framing | **Seed with OpenWhispr `cleanupPrompt`** (shortest, 10 locales); FreeFlow `commandModeSystemPrompt` for Transforms/Command; SpeakoFlow's prompt as a bake-off challenger | Small models need short prompts; the bake-off decides (RT-04). |
| R-05 | Output guards | R4a: FreeFlow heuristic + own guards | R3a: SpeakoFlow validators (Rust) | **Port SpeakoFlow validators** (already Rust, same code base) **+ FreeFlow heuristic + R4a diff/entity, length and language guards** | Least new code. The FreeFlow part is narrow (C-10). |
| R-06 | System audio tap | R4b: vendor flexaudio-os-macos | R3b: hex `meeting_capture.rs` (Rust) | **Port hex `meeting_capture.rs` tap path** (skip its ScreenCaptureKit fallback); flexaudio is the alternative | Same objc2-core-audio 0.3.2 with no exact pins (V-10), and part of a working meeting stack. |
| R-07 | Call detection | R4b: write our own on objc2-core-audio process objects | R3b: hex | **Port hex `microphone_activity.rs`** (+ the catalog part of `meeting_detection.rs`) | It is exactly R4b's plan, already written (C-38). |
| R-08 | Echo handling | R4d: text-match bleed suppression (own code) | R3b / hex: `meeting_echo.rs` signal-level | **Port hex `meeting_echo.rs`**, keeping text-match as a cheap second pass | Signal-level handling survives double-talk and comes with replay tests. |
| R-09 | Pill | R4b: tauri-nspanel 2.1.1 + anarlog click-through | R3a: Tolkovo `pill.rs` | **Tolkovo pill pattern on tauri-nspanel 2.1.1**; anarlog polling only if the pill lives in a larger transparent window | Tolkovo is MIT, same stack and same pin (V-01), and already clickable, non-activating and draggable. |
| R-10 | Clipboard snapshot | R4b: own objc2 snapshot (Hex legacy pattern) | R3b: hex `paste.rs` (Carbon Pasteboard); R3a: OpenWhispr | **Own objc2 NSPasteboard snapshot** + hex's restore guard (generation + changeCount + Transient/AutoGenerated markers) | Avoids the deprecated Carbon API; about 60 lines. |
| R-11 | Browser URL | R4b: port mado | R3b: Muesli `kAXDocument` | **mado's extractor (from the MIT crate)**: AXDocument first, unfocused address bar second | Covers Chromium windows with no document URL. Licence caveat in L-10. |
| R-12 | LLM supervisor | R4a: write a small one (OpenWhispr flags, Jan patterns) | R3a: port SpeakoFlow `local_llm.rs` parts | **Write a small supervisor** borrowing SpeakoFlow's prewarm and resident window and OpenWhispr's flags; **do not port SpeakoFlow's runtime engine download** | SpeakoFlow downloads `llama-server` at runtime with no checksum (R3a trap). We bundle a signed sidecar instead. |
| R-13 | Models | R4a: Qwen3.5-2B / Qwen3-4B-2507 | R3a: SpeakoFlow Mini; R3b: Muesli fine-tunes | **Keep R4a defaults**; add both fine-tunes as challengers | Licences are clean, but the fine-tunes' data provenance is unclear (L-39, L-40). |

## 6. Corrections carried into R4-parts-bin.md (summary)

1. tauri-plugin-notification: use **2.4.0**, not `~2.3` (V-02).
2. mado: the source repo is AGPL by default; port only from the MIT crate, skip the favicon code (L-10).
3. eslint-plugin-jsx-a11y pulls MPL-2.0 axe-core: owner decision (L-38).
4. Build llama-server for **macOS 13.0** to match the app minimum (V-13).
5. Mirror sherpa diarization models to our HF repo to keep the network inventory HF-only (L-31).
6. "Hex" means two different projects; the names are now explicit (L-07).
7. FreeFlow detector scope narrowed (C-10); Handy's schema field is `transcription` (V-12).
8. Five R4 recommendations replaced by permissive code found in R3 (R-01, R-06, R-07, R-08, R-09); prompts and guards merged (R-04, R-05).
9. sysinfo MSRV 1.95 noted; webrtc-audio-processing crates.io licence is "non-standard" (BSD-3 text).
10. Minor: Homebrew fork replacement rule (V-16).

No licence verdict flipped from allowed to forbidden for a recommended **runtime** part. The changes are a provenance restriction (mado) and a dev-tool MPL flag (jsx-a11y).
