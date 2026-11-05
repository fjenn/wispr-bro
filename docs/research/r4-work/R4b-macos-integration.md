# R4b: Parts bin, macOS system integration

Deliverable R4, part b. This file lists verified open-source parts for the macOS system-integration gaps in `R2-gap-analysis.md` (AF-05, AF-08, AF-09, AF-10, AF-15, AF-16, AF-17 and their rows). It picks one choice per capability and lists what still needs a test on a real Mac.

## Method

- **Read first:** R2 §1-4 and the Area A table, plus rows D-21, D-22, D-29, D-30, D-58, C-17, E-01, E-02, E-10, E-205, E-206 (and the linked rows E-07, E-28, E-91, A-63, A-65).
- **Handy code opened for this pass** (paths relative to `src-tauri/src/` unless noted):
  - `Cargo.toml`, `Cargo.lock`
  - `clipboard.rs:50-120`, `paste_tx/macos.rs:1-336`
  - `overlay.rs:1-60` and `:440-480`
  - `shortcut/handy_keys.rs:80-125`, `secure_input.rs:1-60`
  - `lib.rs:113-165` (activation policy), `managers/audio.rs:95-111`
  - `src-tauri/Entitlements.plist`, `src-tauri/Info.plist`, `src-tauri/capabilities/default.json`, `src-tauri/build.rs` (Swift and weak-link lines), `src-tauri/tauri.conf.json` (macOS block)
- **Dependency sources read:**
  - Under `~/.cargo/registry/src/index.crates.io-*/`: `handy-keys-0.3.4` (`platform/macos/listener.rs`, `permissions.rs`, `manager.rs`), `handy-cpal-0.18.2` (`host/coreaudio/macos/loopback.rs`, `README.md`), `objc2-core-audio-0.3.2`, `objc2-app-kit-0.3.2`, `arboard-3.6.1`, `tauri-2.11.5` (`menu/`, `app.rs`, `webview/`, `window/`), `tauri-utils-2.9.3` (`acl/capability.rs`), `wry-0.55.1`.
  - In the git checkouts: `tauri-nspanel` at commit `da9c9a8` (`builder.rs`, `panel.rs`, examples) and the cjpais `tao` checkout (`dock.rs`).
- **Clones:** every outside candidate was cloned or downloaded into its own empty directory under `<scratchpad>/clones/r4b-<name>/`. Repositories were shallow-cloned. Crates were downloaded as tarballs from crates.io. Nothing was built, installed or run.
- **License evidence:** the LICENSE file was read wherever one exists. Where there is no file, that is stated and the evidence is the Cargo metadata or the README.
- **Stars and activity:** from the GitHub API (`gh api repos/...`, 2026-10-28). Downloads and release dates: from the crates.io API on the same day.
- **GPL code:** read as docs only, never as code to copy.
- **Tags:**
  - **(obs)** = observed in code, license text or API data, with the path or URL given.
  - **(inf)** = inferred from code or docs.
  - **(unk)** = unknown; needs a test.
- **Effort:** S ≤ 2 days, M ≤ 2 weeks, L > 2 weeks (same scale as R2).

**Already in Handy's lock (useful baseline):**

- `objc2 0.6.4`, `objc2-foundation`, `objc2-app-kit`, `objc2-core-audio`, `objc2-core-foundation`, `objc2-core-graphics`, `objc2-user-notifications` and `block2 0.6.2` (obs, `Cargo.lock`).
- `arboard 3.6.1`, via `tauri-plugin-clipboard-manager` (obs).
- `tauri-nspanel` from git, branch `v2.1`, commit `da9c9a8` (obs).
- `handy-keys 0.3.4` and `handy-cpal 0.18.2` (obs).

**Not in the lock:**

- `objc2-application-services`, `objc2-event-kit` and `accessibility-sys` are absent (obs).
- `rdev` is declared in `Cargo.toml` but nothing under `src-tauri/src` uses it (obs, `rg rdev src` is empty). `AGENTS.md` is stale on this.

---

## 1. Accessibility client (AF-05: A-50, A-51, D-29, D-30, D-58, D-52)

**What we need:**
- Read the focused element: system-wide `AXFocusedUIElement`.
- Read its role and subrole, to answer "is it a text field?" and "is it `AXSecureTextField`?".
- Read `AXSelectedText`, `AXSelectedTextRange`, `AXValue` and `AXNumberOfCharacters`.
- Read text near the caret: `AXStringForRange` (parameterized) or a slice of `AXValue`.
- Read the caret's screen position: `AXBoundsForRange`.
- Replace the selection: set `AXSelectedText`, or paste over it.
- Enable the AX tree in Electron and Chromium apps, set a messaging timeout, and fall back to ⌘C or blind paste when AX fails.

### Candidates

| Candidate | License (evidence) | Activity / adoption | macOS | What it gives | Fit with Handy | Cost | Risks | Verdict |
|---|---|---|---|---|---|---|---|---|
| **objc2-application-services 0.3.2** ([crates.io](https://crates.io/crates/objc2-application-services), [repo](https://github.com/madsmtm/objc2)) | Zlib OR Apache-2.0 OR MIT (obs: crate `Cargo.toml:37`; repo has `LICENSE-ZLIB.txt`, `LICENSE-APACHE.txt`, `LICENSE-MIT.txt`) | objc2 repo 1,083★, pushed 2026-10-17; crate 312k downloads, released 2025-10-04 (obs) | 10.13+ (inf, AX C API) | Raw FFI for `AXUIElement` (copy, set, parameterized, actions, `copy_element_at_position`, `set_messaging_timeout`), `AXObserver`, `AXValue` (`AXValueCreate`) and `AXIsProcessTrustedWithOptions` (obs: `src/generated/HIServices/AXUIElement.rs`, 2,631 lines). **No attribute-name constants**: `AXAttributeConstants.rs` is 44 lines of `AXMenuItemModifiers` only, so names are string literals (obs). | Same objc2 0.6 / objc2-core-foundation 0.3 family Handy already locks; adds one small crate. New fork module `ax.rs` (~250 lines of safe wrapper). Consumers: `actions.rs` (pre-paste check), `clipboard.rs` / `paste_tx/macos.rs` (secure-field refusal), new selection reader for Transforms. | M | All calls are `unsafe`; we write the safe layer. | **Adopt** (base layer) |
| accessibility 0.2.0 / accessibility-sys 0.2.0 ([repo](https://github.com/eiz/accessibility)) | MIT / Apache-2.0 (obs: `LICENSE-MIT`, `LICENSE-APACHE`) | 92★, last push 2026-01-01. crates.io max is 0.2.0 (2025-03-22); repo master has unpublished 0.3.0 (obs). 340k / 130k downloads. | 10.x (inf) | Safe-ish `AXUIElement::attribute`, `set_attribute`, `perform_action`, `set_messaging_timeout` (obs: `accessibility/src/ui_element.rs:34-168`). `accessibility-sys` has the string constants (`kAXSelectedTextRangeAttribute`, `kAXBoundsForRangeParameterizedAttribute`, `kAXSecureTextFieldSubrole`, `kAXURLAttribute`; obs). No parameterized-attribute wrapper and no observer wrapper (obs grep). | Built on `cocoa 0.26` + `objc 0.2` + `core-foundation 0.10` (obs: `accessibility/Cargo.toml`), the legacy stack. Handy has moved its own code to objc2, though the legacy crates are still in the lock through other dependencies. | S-M | Slow cadence; legacy objc stack. | **Study only.** Copy constant names from `accessibility-sys` (MIT/Apache) if wanted. |
| axuielement 0.10.2 ([crates.io](https://crates.io/crates/axuielement), [repo](https://github.com/doom-fish/axuielement-rs)) | MIT OR Apache-2.0 (obs: crate `LICENSE-MIT`, `LICENSE-APACHE`) | 2★, created 2026-05-15, 15 versions, 6.4k downloads, last release 2026-10-26 (obs) | 10.13+ (obs README) | Safe API: system-wide focused element, string attributes, parameterized attributes, `AXValue` round-trips including `CFRange`, an `AXObserver` that is safe to drop, text markers (obs README). | `build.rs` runs `swift build` on a bundled Swift package (obs: `build.rs:24-30`). Handy already compiles Swift with `swiftc` in `build.rs:376-470` (obs), but this adds SwiftPM plus the `apple-cf` / `doom-fish-utils` stack. | S to integrate | Single maintainer, very young; Swift build step (Nix packaging, CI); parallel CF type system. | **Study / fallback** if the own wrapper grows too big |
| get-selected-text 0.1.6 ([repo](https://github.com/yetone/get-selected-text)) | Cargo metadata says "MIT / Apache-2.0"; **the repo has no LICENSE file** (obs: repo root lists only `.github .gitignore Cargo.toml README.md src`) | 210★, last push 2024-10-09, released 2024-05-29 (obs) | – | 139-line macOS file (obs `src/macos.rs`): AX `AXSelectedText` on the focused element, otherwise an AppleScript that sends ⌘C via System Events and restores **text only**; caches per app which method worked. | Old deps (`cocoa 0.24`, `accessibility-ng`, `active-win-pos-rs 0.8`); the osascript fallback needs Automation consent for System Events (inf) and loses rich clipboard (same flaw as AF-17). | – | Unmaintained; no license file; lossy fallback. | **Study only** (per-app "AX worked / use ⌘C" cache is a good idea) |
| selection 1.2.0 ([crate](https://crates.io/crates/selection), repo pot-app/Selection) | **GPL-3.0-only** (obs crates.io; repo archived) | archived | – | – | – | – | GPL | **Reject** |
| macos-accessibility-client 0.0.2 ([codeberg](https://codeberg.org/fresskoma/macos-accessibility-client)) | Apache-2.0 (obs crates.io metadata) | 591k downloads; already in the lock as 0.0.1 via `tauri-plugin-macos-permissions` (obs) | – | Trust check only (`AXIsProcessTrustedWithOptions`) | Already present | – | Not an AX client | **Keep as is** (not the answer to AF-05) |
| xa11y 0.15.4 / xa11y-macos ([repo](https://github.com/xa11y/xa11y)) | MIT (obs: GitHub license API; the crate tarballs ship no LICENSE file) | 85★, 31 versions since 2026-03-21, pushed 2026-10-26 (obs) | – | Cross-platform client (macOS AX, Windows UIA, Linux AT-SPI) with a selector engine; sets `AXSelectedText` / `AXSelectedTextRange` (obs `xa11y-macos/src/ax.rs:4745-4782`). Maps `AXSecureTextField` → plain `TextField` (obs `ax.rs:2380`), which loses the secure signal. README claims macOS 26+ needs Screen Recording "to read window content" (unk, not verified). | Heavy (5k+ line `ax.rs`, `rayon`, an ObjC shim built with `cc`); a test/automation tool, not a hot-path dictation helper. | L | Secure-field signal lost; heavy. | **Study only** (possible Windows/Linux reference later) |
| uiautomation 0.25.1 ([repo](https://github.com/leexgone/uiautomation-rs)) | Apache-2.0 (obs: repo `LICENSE`) | 204★, 980k downloads, released 2026-09-04 (obs) | Windows | UIA client (focused element, TextPattern) | Windows port only | – | – | **Adopt later (Windows)** |
| Hex `PasteboardClient.insertTextAtCursor` ([repo](https://github.com/kitlangton/Hex), Swift) | MIT (obs `LICENSE`, © 2025 Kit Langton). **Repo is archived** (obs GitHub API) | 2,898★ | – | Focused element → check `AXValue` / `AXSelectedText` → set `AXSelectedText` (obs `Hex/Clients/PasteboardClient.swift:350-380`) | Pattern for D-30 "replace selection" without the clipboard | – | Archived. AX set can report success but do nothing in Chromium/Electron (inf) → RC-04b. | **Study** |
| meeting-record `Accessibility.swift` ([crate](https://crates.io/crates/meeting-record), [repo](https://github.com/hoangvvo/meeting-record)) | MIT (obs crate `LICENSE.md`, © 2026 Hoang Vo) | 0★, 5 versions since 2026-09-12 (obs) | – | Messaging timeout per element; sets `AXEnhancedUserInterface` on Chromium/Electron apps, "Without this, `AXWebArea` does not exist in Chromium or Electron trees" (obs `native/macos/Accessibility.swift:63-67`) | Reference for the Electron/Chromium enable step | – | – | **Study** |

**Electron and Chromium enabling:**
- **Electron:** the documented way for third-party assistive tech is to set `AXManualAccessibility = true` on the app's `AXUIElement` ([Electron accessibility docs](https://www.electronjs.org/docs/latest/tutorial/accessibility), obs via search summary).
- **Chromium browsers:** `AXEnhancedUserInterface` also works (inf, meeting-record does it), but it has side effects. Rectangle (MIT, 30k★) switches it **off** before resizing windows (obs: [`Rectangle/AccessibilityElement.swift:219-225`](https://github.com/rxhanson/Rectangle/blob/main/Rectangle/AccessibilityElement.swift)), which points to animation and positioning problems for window managers (inf).
- **Plan:** set `AXManualAccessibility` once per Electron pid, lazily, only after the first AX read fails. Use `AXEnhancedUserInterface` only as an opt-in last resort.

**Fallback order (inf):**
1. AX read of the focused element (with a timeout of about 0.1-0.25 s).
2. Enable `AXManualAccessibility` and retry once.
3. ⌘C with a `changeCount` wait plus a full multi-type snapshot (§4).
4. Blind paste plus the fallback card (A-15).

A false "no text field" must never block a paste (R2 A-51). The secure-field check is `AXSubrole == "AXSecureTextField"` (constant name, obs `accessibility-sys`). It complements the existing `IsSecureEventInputEnabled` poll (`secure_input.rs:9-17`, obs).

---

## 2. Frontmost app, window title, browser URL (D-21, D-22, DEC-23)

| Candidate | License (evidence) | Activity | What it gives | Fit | Cost | Risks | Verdict |
|---|---|---|---|---|---|---|---|
| **objc2-app-kit 0.3.2 `NSWorkspace`** (already in the lock) | Zlib/Apache-2.0/MIT (obs) | 78M downloads (obs) | `frontmostApplication`, `menuBarOwningApplication` (obs `src/generated/NSWorkspace.rs:298-303`), `NSWorkspaceDidActivateApplicationNotification` (obs `:664`); bundle ID and pid from `NSRunningApplication`. Default features include it (R2 AF-05, obs). | New `frontmost.rs`; call at record start in `actions.rs`. Storage via the history schema (C-54). | S | `frontmostApplication` is updated through the main run loop (inf); read it on the main thread, or use AX `AXFocusedApplication` for a fresh pid. | **Adopt** |
| AX window title (`AXFocusedWindow` → `AXTitle`) via the §1 layer | – | – | Window title with Accessibility only. `CGWindowListCopyWindowInfo` `kCGWindowName` needs Screen Recording on 10.15+ (inf). | Part of `ax.rs` | S | – | **Adopt** |
| mado 0.0.21 ([crate](https://crates.io/crates/mado), [repo](https://github.com/builder-group/abstand)) | MIT (obs crate `LICENSE`, © 2024 @bennobuilder) | 43★ repo; 21 versions since 2026-04-03; 585 downloads (obs) | Pure-AX browser URL: document URL first, otherwise the address-bar `AXValue` **only when the bar is not focused** (to avoid uncommitted edits) (obs `src-swift/BrowserURLExtractor.swift:4-30`). Browser table covers Chromium (Chrome, Brave, Edge, Opera, Arc, Helium), Vivaldi, Safari and Gecko (Firefox, Zen) (obs `SupportedBrowsers.swift`). No AppleScript (obs grep). | Swift via `swift-rs` in `build.rs`; `specta 2.0.0-rc.21` caret (compatible with Handy's `=rc.22`, inf). | M to port | 0.0.x, small user base; Swift build step | **Adapt** (port the ~200 lines of AX URL logic into `ax.rs`) |
| active-win-pos-rs 0.11.0 ([repo](https://github.com/dimusic/active-win-pos-rs)) | MIT / Apache-2.0 (obs `LICENSE-MIT`, `LICENSE-APACHE`) | 155★, released 2026-05-26 (obs) | App name and window title via bindgen'd NSWorkspace plus `CGWindowListCopyWindowInfo` `kCGWindowName` (obs `src/mac/platform_api.rs:6,50,90`) | Duplicates objc2-app-kit; title needs Screen Recording (inf) | – | Extra permission | **Reject** |
| x-win 5.8.0, frontmost 1.1.0, browser-url 0.1.1 | MIT (crates.io metadata, obs) | small (6k / 88 / 344 recent downloads) | Window info / URL | Nothing beyond the above | – | Immature | **Reject** |
| ActivityWatch aw-watcher-window ([repo](https://github.com/ActivityWatch/aw-watcher-window)) | **MPL-2.0** (obs GitHub API) | 131★ | AppleScript/JXA per browser (obs file list `macos_applescript.py`, `macos_jxa.py`) | – | – | File-level copyleft; Automation prompts | **Study only (docs)** |
| Per-browser AppleScript (`tell application "Google Chrome" to get URL of active tab ...`) | – | – | Exact URL for Safari and Chromium; **Firefox has no URL scripting** (R2 D-22) | Needs `NSAppleEventsUsageDescription` in `Info.plist` and `com.apple.security.automation.apple-events` in `Entitlements.plist` (hardened runtime; neither is present today, obs); one consent prompt per browser (inf) | M | Prompt fatigue; a privacy story we would have to explain | **Reject for v1** |

**DEC-23 input:** AX URL (mado approach), with the window title as fallback, and no AppleScript. This needs no new permission beyond Accessibility (inf). Chromium may need the §1 enable step (RC-04c).

---

## 3. Hotkeys: fn/Globe, double-tap, chords, dead-tap recovery (AF-08: A-22, A-23, A-35, A-201, A-202, A-208)

### What handy-keys 0.3.4 already does (obs)

- **Event tap:** one `CGEventTap` at **`SessionEventTap`**, head insert, default (blocking) mode. Mask: KeyDown, KeyUp, FlagsChanged and mouse buttons (`listener.rs:446-475`). There is no `NSSystemDefined`.
- **fn handling:** fn is detected as FlagsChanged keycode `0x3F`. A press of a registered modifier-only hotkey is blocked; the release is passed (`listener.rs:320-340`).
- **Tap disabled by macOS:** `TapDisabledByTimeout` / `TapDisabledByUserInput` are re-enabled from inside the callback. It deliberately does **not** poll `CGEventTapIsEnabled`, because WindowServer RPCs "leak kernel IPC vouchers and eventually panic the whole machine (cjpais/Handy#1827)" (`listener.rs:172-193`).
- **Tap creation failure:** `spawn()` returns `Err(AccessibilityNotGranted)` or `Err(EventTapCreationFailed)` correctly (`listener.rs:71-98`).
- **No health API:** the public API is `new`, `new_with_blocking`, `register`, `unregister`, `recv` and `try_recv` (`manager.rs`). There is no way to ask whether the tap is alive.
- **The A-208 bug is in Handy's wrapper**, not in handy-keys: `HandyKeysState::new` returns `Ok` before the manager thread creates the tap. The error is only logged (`shortcut/handy_keys.rs:89-119`, obs).

### Candidates

| Candidate | License (evidence) | Activity | What it gives | Fit | Cost | Risks | Verdict |
|---|---|---|---|---|---|---|---|
| **handy-keys 0.3.4** (in use) ([repo](https://github.com/handy-computer/handy-keys)) | MIT (obs crate `LICENSE`, © 2026 handy-computer) | 36★, 146k downloads, released 2026-08-07 (obs) | fn, modifier-only, mouse buttons, blocking, self-healing tap | Keep. Fix the wrapper: create `HotkeyManager` inside the thread and send the `Result` back on a oneshot channel before `HandyKeysState::new` returns, so the persisted Tauri fallback (`shortcut/mod.rs:40-56`) can run. Touches `shortcut/handy_keys.rs` only. | S | Upstream-owned crate; small upstream PRs (health getter, fn+Esc) are better than a fork | **Keep + fix wrapper** |
| **shortcut-macos** (Rust crate inside anarlog, formerly Hyprnote) ([repo](https://github.com/fastrepl/anarlog/tree/main/crates/shortcut-macos)) | MIT for everything outside `enterprise/` (obs: root `LICENSE` MIT © Fastrepl; `LICENSE.enterprise` covers only the `enterprise/` directory) | anarlog 9,621★, pushed 2026-10-28 (obs) | `HotKeyProcessor` state machine: press-and-hold, **double-tap lock**, a "different key within 1 s cancels" guard, a **300 ms minimum for modifier-only** hotkeys, a "dirty until full release" rule, Esc cancels; 311 lines plus 605 lines of tests; dependencies only `serde`, `specta`, `thiserror` (obs `src/processor.rs`, `src/decision.rs`, `tests/processor.rs`). Also a tap at **`kCGHIDEventTap`** (obs `src/tap.rs:118,237`). | Port `processor.rs` + `decision.rs` + tests into `transcription_coordinator.rs`, driven by handy-keys events. Covers A-22 (accidental-start guard), A-23 (double-tap), A-25/A-201 (Esc while fn held is handled in the state machine, not by exact match). | M | The design mirrors Hex's `HotKeyProcessor` (same constants and names; Hex is MIT). anarlog does not credit Hex (obs grep), so keep both notices to be safe (inf). | **Adapt (port)** |
| Hex `HotKeyProcessor` + `KeyEventMonitorClient` ([repo](https://github.com/kitlangton/Hex), Swift) | MIT (obs `LICENSE`); **archived** | 2,898★ | The original of the above: `doubleTapWindow` 0.3 s, `modifierOnlyMinimumDuration` 0.3 s, `pressAndHoldCancelWindow` 1.0 s (obs `HexCore/Sources/HexCore/Constants.swift:22,40,53`). The tap at `.cghidEventTap` has a 100 ms watchdog that re-enables a tap "macOS disabled without sending a tapDisabled event (observed after sleep, #250)" by polling `CGEvent.tapIsEnabled` (obs `Hex/Clients/KeyEventMonitorClient.swift:390-440`). | Reference for thresholds and the sleep case | – | Hex's polling conflicts with handy-keys' anti-polling rule (#1827). Test both before choosing (RC-14). | **Study** |
| **axwatch 0.1.0** ([crate](https://crates.io/crates/axwatch), [repo](https://github.com/AprilNEA/axwatch)) | MIT OR Apache-2.0 (obs crate `LICENSE-MIT` © 2026 AprilNEA, `LICENSE-APACHE`) | 0★, 1 release (2026-09-30), 8.6k downloads (obs) | Watches the Accessibility grant through two undocumented notifications (`com.apple.accessibility.api` distributed and `com.apple.tcc.access.changed` Darwin), plus an optional heartbeat. Its documented caveats: `AXIsProcessTrusted` stays `true` after the app's row is **removed**, so confirm with a capability probe (create a filtering tap and check for NULL); distributed notifications are withheld from unsigned binaries on macOS 15+ (obs README). ~500 lines; depends on `objc2-application-services` and `objc2-core-foundation` 0.3.2 (obs). | Health monitor for A-208: on wake, call `is_trusted()` plus a probe tap; on failure, emit a tray warning and a pill error (A-12) and re-create the handy-keys manager. New `shortcut/health.rs`; touches `tray.rs` (warning badge already exists for Secure Input). | S | Undocumented notification names; a brand-new crate | **Adopt or vendor** (small; vendoring avoids a 0★ dependency) |
| global-hotkey 0.9.0 ([repo](https://github.com/tauri-apps/global-hotkey)) | Apache-2.0 OR MIT (obs `LICENSE-APACHE`, `LICENSE-MIT`) | 266★, 7M downloads (obs) | Carbon `RegisterEventHotKey` (obs `src/platform_impl/macos/mod.rs:27,117`): no fn, no modifier-only | Already behind `tauri-plugin-global-shortcut` (the Secure Input shadow path) | – | – | **Keep as fallback only** |
| rdev (Narsil 0.5.3 / rustdesk-org fork) | MIT (obs `LICENSE`, © 2020 Nicolas Patry) | Narsil 747★ (crates.io 0.5.3 from 2023); fork pushed 2026-10-06 (obs) | CGEventTap listen and grab | **Declared but unused** in Handy (obs) | – | Git dependency, dead weight | **Reject; remove from `Cargo.toml`** |
| KeyboardShortcuts (sindresorhus, Swift) ([repo](https://github.com/sindresorhus/KeyboardShortcuts)) | MIT (obs `license`) | 2,724★, pushed 2026-09-11 (obs) | Carbon `RegisterEventHotKey` (obs `Sources/KeyboardShortcuts/HotKey.swift`), recorder UI | No fn, Swift UI | – | – | **Study only** (recorder UX) |

**fn / Globe action suppression (RC-02):**
- Handy blocks the fn press at the **session** tap. Hex and anarlog tap at the **HID** level (obs above).
- The Globe action is a system feature configured by "Press 🌐 key to" (the `AppleFnUsageType` key in `com.apple.HIToolbox`; values: Do Nothing, Change Input Source, Show Emoji & Symbols, Start Dictation). The key is documented by [nix-darwin](https://mynixos.com/nix-darwin/option/system.defaults.hitoolbox.AppleFnUsageType), which notes that a restart is needed (inf, third-party doc).
- Whether a blocking tap at either level stops the Globe action is **unknown**.
- Plan:
  1. Test the session tap against a HID tap (RC-02/RC-15).
  2. If neither suppresses the action, onboarding detects a non-"Do Nothing" setting (read-only `defaults read`; inf) and offers a deep link to Keyboard settings. Writing the preference ourselves is not recommended, because it changes a user setting and may need a logout (inf).

---

## 4. Clipboard: full snapshot and restore, concealment, change count (AF-17: A-47, A-48, D-29, C-70)

| Candidate | License (evidence) | Activity | Multi-type / multi-item? | Concealment | Fit | Cost | Verdict |
|---|---|---|---|---|---|---|---|
| **Own snapshot with objc2-app-kit `NSPasteboard`** (already in the lock; Handy already uses it in `paste_tx/macos.rs`) | Zlib/Apache-2.0/MIT (obs) | – | Yes: iterate `pasteboardItems`, then each item's `types`, then `dataForType`; restore with one `NSPasteboardItem` per saved item via `writeObjects` | Declare concealment types on our writes | ~60 lines in `paste_tx/macos.rs` (replace `saved_text` / `saved_image` at `:270-278` and the restore at `:157-175`) and in `clipboard.rs:78-92`, sharing one `snapshot()` / `restore()` | S | **Adopt (write; pattern below)** |
| Hex `PasteboardSnapshot` ([repo](https://github.com/kitlangton/Hex)) | MIT (obs) | archived | Yes, per item and per type (obs `Hex/Clients/PasteboardClient.swift:51-82`). Also waits for `changeCount` to advance before ⌘V (obs `:237-265`; changelog "panel apps always receive the latest transcript") | – | The 30-line reference for the Rust version | – | **Study (port pattern)** |
| arboard 3.6.1 (in the lock; latest 3.7.0) ([repo](https://github.com/1Password/arboard)) | MIT OR Apache-2.0 (obs `LICENSE-MIT.txt`, `LICENSE-APACHE.txt`) | 978★, 60M downloads, pushed 2026-10-28 (obs) | Text, HTML (`get().html()`, `set().html(html, alt)`) and images; not arbitrary multi-item (obs `src/lib.rs:205,240`, `platform/osx.rs`) | **`SetExtApple::exclude_from_history()`** adds `org.nspasteboard.ConcealedType` (obs `platform/osx.rs:276-292,410-435`, present in 3.6.1) | Direct dependency `arboard = "3.6"` (no new code in the lock). Use it for the default (legacy) paste path so transcripts stay out of Maccy and Paste (A-48), and for HTML snippets (C-70, DEC-12). Touches `clipboard.rs:50-62`. | S | **Adopt (default-path writes)** |
| tauri-plugin-clipboard-manager 2.3.2 (in use) | Apache-2.0 OR MIT (obs crates.io) | plugins-workspace 1,838★ | Text / image / HTML via arboard | No concealment flag exposed (inf; wraps arboard without the Apple extension) | Keep for frontend copy buttons | – | **Keep** |
| clipboard-rs 0.3.5 ([repo](https://github.com/ChurchTao/clipboard-rs)) | MIT (obs `LICENSE`, © 2024 桃树夭) | 182★, 995k downloads, released 2026-06-30 (obs) | Reads each format from the first item that has it, and **writes all formats into one `NSPasteboardItem`** (obs `src/platform/macos.rs:167-235`, `:379-440`), so a multi-item clipboard is not restored faithfully | `Other(format, bytes)` could carry markers | Would replace a working objc2 path | – | **Reject** |
| tauri-plugin-clipboard (CrossCopy) 2.1.11 ([repo](https://github.com/CrossCopy/tauri-plugin-clipboard)) | MIT (obs `LICENSE`, © 2024 Huakun Shen) | 315★; last release and push 2024-10 (obs) | Via clipboard-rs (same flattening) | – | – | – | **Reject** (stale) |
| Maccy ([repo](https://github.com/p0deje/Maccy)) | MIT (obs `LICENSE`, © 2025 Alex Rodionov) | 22,108★, pushed 2026-10-26 (obs) | Consumer side: polls `changeCount` (obs `Maccy/Clipboard.swift:43,162-166`); ignores items carrying `org.nspasteboard.AutoGeneratedType`, `ConcealedType` or `TransientType` (obs `:27-31`, `Extensions/NSPasteboard.PasteboardType+Types.swift:10-16`), plus a default user list (`de.petermaurer.TransientPasteboardType`, `com.agilebits.onepassword`, ...) (obs `Extensions/Defaults.Keys+Names.swift:40-48`) | Confirms that the markers Handy declares (`paste_tx/macos.rs:32-40`) are honored | Reference and test target | – | **Study** |

**Notes:**
- **Restore duplicates history entries (inf).** A restore is itself a new pasteboard write, so polling clipboard managers record the user's old content again as a "new copy". If the restore carries a `TransientType` marker on each restored item, Maccy skips it (inf from the ignore list). The cost is one extra type on the user's data. Decide in R5.
- **Concealment is only half done.** The reliable path already uses lazy promises plus concealment and a `changeCount` guard (`paste_tx/macos.rs:157-175`, obs). Promoting it (A-47) and switching both paths to the full snapshot closes AF-17.
- **⌘C selection fallback (D-29):** snapshot, post ⌘C, wait until `changeCount` changes (timeout 150-300 ms, inf), read `public.utf8-plain-text` (and RTF/HTML for Transforms), then restore the snapshot.

---

## 5. Overlay / pill (AF-09: A-01, A-02, A-04, A-06, A-07, A-15, A-16, A-212; E-28)

| Candidate | License (evidence) | Activity | Capability (evidence) | Fit | Cost | Verdict |
|---|---|---|---|---|---|---|
| **tauri-nspanel** (in use, git `v2.1` at `da9c9a8`) → **crates.io 2.1.1** ([repo](https://github.com/ahkohd/tauri-nspanel)) | MIT OR Apache-2.0 (obs `LICENSE_MIT`, `LICENSE_APACHE-2.0` in the checkout; crates.io metadata) | 427★, release v2.1.1 on 2026-10-08, pushed 2026-10-25 (obs). Handy's pin is **25 commits behind** `v2.1`, missing "fix: keep mouse tracking area in sync with panel", "fix: keep input methods above elevated panels" and "fix: synchronize nonactivating panel activation state" (obs `gh api compare da9c9a8...v2.1`). | See the capability list below this table. | Switch `Cargo.toml` from the git branch to `tauri-nspanel = "2.1.1"`; touches `overlay.rs:24-33,452-476`. | S | **Adopt (bump to 2.1.1)** |
| anarlog `plugins/overlay` ([repo](https://github.com/fastrepl/anarlog/tree/main/plugins/overlay)) | MIT (obs, see §3) | anarlog 9.6k★ | **Click-through for a transparent window:** a 20 Hz task compares `window.cursor_position()` with the rectangles the frontend reports ("fake window bounds") and calls `set_ignore_cursor_events(true/false)` only when the state changes (obs `plugins/overlay/src/lib.rs:164-250`) | Port about 120 lines into `overlay.rs` plus a small frontend hook. Lets the always-visible bar (A-01) sit in a larger transparent panel without blocking clicks underneath. | S-M | **Adapt** |
| anarlog `dictation-ui-macos` (Swift native pill) | MIT (obs) | – | A native `NSPanel` pill (`ignoresMouseEvents = true`, `.canJoinAllSpaces`, `.fullScreenAuxiliary`, `.ignoresCycle`, `.stationary`; placed on `NSScreen.screens` containing `NSEvent.mouseLocation`, using `visibleFrame`) (obs `swift-lib/src/OverlayManager+Creation.swift:15-40`) | Evidence that a webview-free native pill is viable if RC-10 (WebKit memory over hours) fails | L | **Study** (plan B for RC-10) |
| tauri-plugin-positioner 2.x ([plugins-workspace](https://github.com/tauri-apps/plugins-workspace)) | Apache-2.0 OR MIT (obs crates.io) | 733k downloads | Tray-relative and monitor-relative positions | Handy already computes placement (`overlay.rs:178-278`) | – | **Reject** (nothing new) |

**tauri-nspanel capabilities (obs in the checkout):**
- `ignores_mouse_events` and `accepts_mouse_moved_events` on the builder (`builder.rs:629-636`).
- Runtime `set_ignores_mouse_events` (`panel.rs:487`).
- Tracking areas with `active_always` / `mouse_entered_and_exited` / `mouse_moved` (`builder.rs:248-301`) and mouse handlers (`panel.rs:139-187`).
- Collection behavior `can_join_all_spaces` / `full_screen_auxiliary` / `stationary` / `ignores_cycle` (`builder.rs:108-180`).
- `with_window(|w| ...)` receives a Tauri `WebviewWindowBuilder` (`builder.rs:800-808`).
- The panel macro has **no** `accepts_first_mouse` (grep empty).

**How the AF-09 items are covered (inf unless tagged):**
- **First click (RC-01):** pass `.with_window(|w| w.decorations(false).transparent(true).focusable(false).accept_first_mouse(true))`. The Tauri builder has `accept_first_mouse` (obs `tauri-2.11.5/src/webview/webview_window.rs:899`), and wry wires it into its WKWebView subclass (obs `wry-0.55.1/src/wkwebview/mod.rs:309`). One line in `overlay.rs:470`.
- **Hover (RC-03):** a tracking area with `active_always()` fires in a never-key panel of an inactive app. WKWebView's own tracking probably follows key/active state, so CSS `:hover` may not fire. Drive hover from the native `mouseEntered` / `mouseExited` events and emit them to the overlay.
- **Click-through:** use the anarlog polling pattern. While `ignoresMouseEvents` is true the window gets no mouse events, so a tracking area cannot switch it back; that is why anarlog polls the cursor.
- **Multi-display (A-212):** observe `NSApplicationDidChangeScreenParametersNotification` (obs `objc2-app-kit-0.3.2/src/generated/NSApplication.rs:1883`) through `NSNotificationCenter` and `block2`, both already in the lock. Re-place the pill on connect, resolution change and Dock auto-hide. Pick the cursor display, as Handy does today (DEC-25 input).
- **Hide from screen capture (E-28):** `WebviewWindow::set_content_protected` (Tauri API, inf). Test with ScreenCaptureKit-based recorders (RC-17).

---

## 6. System audio and meetings capture (AF-10: E-01, E-02, E-10, E-205, E-206; E-07)

### Today (obs)

- **Tap is per device:** handy-cpal creates a Core Audio process tap with `initWithProcesses_andDeviceUID_withStream(empty, device_uid, 0)` and `exclusive = true`, which means every process on **one** output device, our own sounds included (`handy-cpal-0.18.2/src/host/coreaudio/macos/loopback.rs:84-110`).
- **Version gate:** handy-cpal's README says loopback needs macOS 14.6+ (`README.md:60`).
- **APIs already in the lock:** `objc2-core-audio 0.3.2` has `CATapDescription::initStereoGlobalTapButExcludeProcesses` / `initMonoGlobalTapButExcludeProcesses` / `initExcludingProcesses_andDeviceUID_withStream`, plus `kAudioHardwarePropertyProcessObjectList`, `kAudioHardwarePropertyTranslatePIDToProcessObject`, `kAudioProcessPropertyIsRunningInput`, `kAudioProcessPropertyBundleID` and `kAudioDevicePropertyDeviceIsRunningSomewhere` (obs `src/generated/AudioHardware.rs:1082-1084,1302,1757-1766,1874-1946`).

So the whole fix is reachable with crates Handy already has.

### Candidates

| Candidate | License (evidence) | Activity | macOS | What it gives | Fit | Cost | Risks | Verdict |
|---|---|---|---|---|---|---|---|---|
| **flexaudio-os-macos 0.8.1** ([crate](https://crates.io/crates/flexaudio-os-macos), [repo](https://github.com/Studio-Sadola/flexaudio)) | MIT (obs crate `LICENSE`, © 2026 tubome / Studio Sadola) | 10★, 15 versions since 2026-07-04, released 2026-10-20 (obs) | 14.4+, enforced at runtime (obs `src/version.rs`) | `MacSystemBackend::new(exclude_self, device_id)`: `None` → **global tap of the default output excluding our process** via `initStereoGlobalTapButExcludeProcesses`; `with_exclude_pids` for helper trees; a private aggregate device plus IOProc on its own thread; maps the illegal-operation status to `PermissionDenied`; notes that "macOS can deliver zero samples while permission is missing"; a silence probe (obs `src/system.rs:1-60,129-210`, README). Written on **objc2-core-audio 0.3.2**, Handy's version. | New fork module `system_audio.rs` feeding the existing `rubato` resampler and VAD. Replaces the handy-cpal loopback use. Closes E-205 (exclude self) and E-206 (follows the default output; a global tap is not bound to one device UID, inf → RC-08b). | M | **Exact pins** `objc2 =0.6.4`, `block2 =0.6.2`, `libc =0.2.186` (obs crate `Cargo.toml:55-67`), which force Handy's `libc` from 0.2.182 to 0.2.186 and block future objc2 bumps; 5.3k lines; young. | **Adapt** (vendor `system.rs` + `tap/` + `common.rs` under MIT with notice, drop the pins). Adopting it as a dependency is acceptable only if the pins are tolerated. |
| Own module on objc2-core-audio (port AudioCap / meeting-record) | – | – | 14.4+ (inf, Apple's process-tap API; AudioCap README says 14.4; audiotee says 14.2; handy-cpal says 14.6 for loopback) | Same as above, written by us | Same file | M-L | More code | Fallback if vendoring is rejected |
| meeting-record 0.3.0 ([crate](https://crates.io/crates/meeting-record), [repo](https://github.com/hoangvvo/meeting-record)) | MIT (obs `LICENSE.md`, © 2026 Hoang Vo) | 0★, 5 versions since 2026-09-12 (obs) | – | Swift (2.6k lines) built in `build.rs`. **Global tap excluding processes** (`TapCapture.swift:267-272`). **Public-API permission probe**: "Probe audio-capture permission with a temporary tap; no preflight API exists" (`Permissions.swift:5-32`). Default-output listener (`AudioProcesses.swift:197-203`). **Meeting detection** from processes running input, bundle-ID catalog and browser AX content (`MeetingDetector.swift`, `MeetingCatalog.swift`, `BrowserContent.swift`). (obs) | Swift build step; a whole capture stack beside ours | L | Very young | **Study** (permission probe and call detection) |
| AudioCap ([repo](https://github.com/insidegui/AudioCap), Swift) | **BSD-2-Clause** (obs `LICENSE`, © 2024 Guilherme Rambo) | 512★, last push 2025-08-07 (obs) | 14.4 (obs README) | Step-by-step tap setup (README). Permission check and request via the **private** `TCCAccessPreflight` / `TCCAccessRequest("kTCCServiceAudioCapture")` through `dlopen` (obs `AudioCap/ProcessTap/AudioRecordingPermission.swift:40-113`); `NSAudioCaptureUsageDescription` in `Info.plist` (obs) | Documentation reference | – | Private API (fine outside the Mac App Store, inf; may break) | **Study** |
| audiotee ([repo](https://github.com/makeusabrew/audiotee), Swift CLI) | README says "The MIT License, © 2025 Nick Payne"; **no LICENSE file in the repo** (obs) | 92★, pushed 2026-03-31 (obs) | 14.2 (obs README) | Tap with `deviceUID = nil` (system default) and process exclusion (obs `Sources/AudioTeeCore/Core/AudioTapManager.swift:47-60`). README: "trying to include or exclude a PID which isn't currently playing audio will probably fail to convert to an Audio Object" (obs README:98). | – | – | No license file | **Study only** |
| anarlog `crates/detect`, `crates/tcc`, `crates/device-monitor`, `crates/apple-calendar` ([repo](https://github.com/fastrepl/anarlog)) | MIT outside `enterprise/` (obs) | 9.6k★ | – | Mic-in-use detection: `DEVICE_IS_RUNNING_SOMEWHERE` listener plus a 1 s poll of apps (obs `crates/detect/src/mic/macos/mod.rs:15-21`). TCC private preflight in Swift (obs `crates/tcc/swift-lib/src/lib.swift:10-38`). Calendar on **objc2-event-kit** (obs `crates/apple-calendar/Cargo.toml:34`). | detect and device-monitor are written on **cidre**, a different binding stack, so port to objc2-core-audio; apple-calendar is objc2, so it ports easily | M | – | **Study / port** (calendar: adapt) |
| screencapturekit 11.1.1 ([repo](https://github.com/doom-fish/screencapturekit-rs)) | MIT OR Apache-2.0 (obs `LICENSE-MIT`, `LICENSE-APACHE`) | 244★, 3M downloads, released 2026-10-26 (obs) | 12.3+ (inf; SCK audio 13+); version features `macos_13_0` to `macos_26_0` (obs `Cargo.toml:55-65`) | Audio capture with `set_excludes_current_process_audio` (obs `src/stream/configuration/audio.rs:378`), microphone capture on 15+ (`:339`) | Swift bridge built with `xcrun` / SwiftPM in `build.rs` (obs `build.rs:42-140`); needs **Screen Recording** permission and shows the screen-recording indicator (inf) | M | Heavier permission; monthly re-consent prompts on 15+ (inf) | **Reject for 14.4+**. Fallback only if DEC-22 wants system audio on 13.x. |
| cidre 0.29.0 ([repo](https://github.com/yury/cidre)) | MIT (obs `LICENSE.txt`, © Yury Korolev) | 225★, 311k downloads, pushed 2026-10-28 (obs) | – | Full Apple bindings including `core_audio::TapDesc` / `Tap` (obs `cidre/src/core_audio/tap_description.rs`, `hardware_tapping.rs`) | A second binding ecosystem next to objc2 | – | Duplication | **Reject** (objc2-core-audio already covers it) |
| ruhear 0.1.1 | Apache-2.0 (crates.io, obs) | 2.6k downloads, 2025-12 (obs) | – | SCK wrapper | – | – | – | **Reject** |
| **objc2-event-kit 0.3.2** (E-07 calendar) | Zlib/Apache-2.0/MIT (obs crates.io) | 288k downloads (obs) | `requestFullAccessToEventsWithCompletion` is 14+ (inf); `authorizationStatusForEntityType`, `predicateForEventsWithStartDate_endDate_calendars`, `eventsMatchingPredicate` (obs `src/generated/EKEventStore.rs:76,120,421,476`) | Local calendar read | New `calendar.rs`. Needs `NSCalendarsFullAccessUsageDescription` (14+) and `NSCalendarsUsageDescription` (older) in `Info.plist`, and the hardened-runtime calendars entitlement `com.apple.security.personal-information.calendars` (inf) | M | – | **Adopt** (port anarlog's apple-calendar for shape) |

### Call-in-progress detection (inf; uses only objc2-core-audio)

1. Register a listener on the default input device's `kAudioDevicePropertyDeviceIsRunningSomewhere`. This is cheap, but it is also true while **we** record.
2. When it fires, list `kAudioHardwarePropertyProcessObjectList`. For each process read `kAudioProcessPropertyIsRunningInput`, `kAudioProcessPropertyPID` and `kAudioProcessPropertyBundleID`, excluding our own PID.
3. Match bundle IDs against a small catalog (zoom.us, Teams, FaceTime, Slack, browsers). meeting-record's `MeetingCatalog.swift` and anarlog's detect crate are references.

Process-object properties need macOS 14+ (unk exact version, RC-18).

### Permission (E-02)

- Add `NSAudioCaptureUsageDescription` to `src-tauri/Info.plist`.
- Detect consent with meeting-record's **public** temporary-tap probe rather than the private TCC preflight. The private route is acceptable outside the Mac App Store but fragile (inf).
- `tauri-plugin-macos-permissions` checks only Screen Recording (R2 AF-10), which is a different TCC service.

### Not covered by these parts

E-10 and E-48: RAM-held recordings and streaming to disk are a separate problem, not a system-integration one. Use the `hound` writer.

---

## 7. Other system surfaces

| Need | Candidate | License (evidence) | Facts | Fit / files | Cost | Verdict |
|---|---|---|---|---|---|---|
| Native notifications (E-91, DEC-21, C-17 system part) | **tauri-plugin-notification** ([plugins-workspace](https://github.com/tauri-apps/plugins-workspace)) | Apache-2.0 OR MIT (obs crate `Cargo.toml:27`) | Latest 2.8.0 (released 2026-10-28). 2.7.0 and 2.8.0 require **tauri ^2.12** and Rust 1.90 (obs `Cargo.toml`); 2.3.3 requires tauri ^2.8.2 (obs). Desktop backend is `notify-rust` 4.18 (obs `src/desktop.rs:487-520`), which on macOS uses `mac-notification-sys` (NSUserNotification, deprecated since macOS 11, inf). The UNUserNotificationCenter backend is behind notify-rust's `preview-macos-un` feature (obs crates.io features). `close` is a no-op on macOS (obs `desktop.rs:711-713`). | Add the plugin and permission entries in `capabilities/default.json`. With tauri 2.11.5, pin `~2.3` or bump Tauri first. | S | **Adopt** (simplest). Alternative: own ~100 lines on `objc2-user-notifications` 0.3.2 (already in the lock) for UN APIs and actions. |
| App menu with a working Edit menu (AF-15, A-63, C-203) | Tauri 2 menu API (in use) | – | `Menu::default(app)` builds File/Edit/View/Window/Help (obs `tauri-2.11.5/src/menu/menu.rs:142`). `PredefinedMenuItem::{undo, redo, cut, copy, paste, select_all, services, hide, quit}` (obs `menu/predefined.rs:28-305`). `Menu::append` / `insert` (obs `menu.rs:264-335`). `set_as_app_menu` (obs `:390`). | Fork-owned `app_menu.rs`: start from `Menu::default`, insert the Dictation / My Voice / Help submenus, keep Edit. Wire it in `lib.rs` setup. | S | **Adopt** (no crate) |
| New windows need capability entries (AF-16, C-75, E-12) | Tauri ACL | – | Capability `windows` accepts glob patterns ("Windows can be added ... by exact name ... or glob patterns like `*` or `admin-*`", obs `tauri-utils-2.9.3/src/acl/capability.rs:85,150`) | Put fork windows under one prefix (e.g. `fork-*`) in a separate fork capability file. Avoid `*`, which would hand every plugin permission to any future window. | S | **Adopt** |
| Dock visibility toggle (A-65) | Tauri `set_activation_policy` (in use) / `AppHandle::set_dock_visibility` | – | `set_dock_visibility` exists (obs `tauri-2.11.5/src/app.rs:660`). In the cjpais tao checkout it is a show/hide with a 1 s guard against the "multiple dock icons" macOS bug (obs `tao .../macos/dock.rs:33-58`). Handy's launch-time Accessory logic exists to avoid the macOS 26 demotion bug (obs `lib.rs:129-163`). | Keep Handy's policy code and add the `show_in_dock` setting to its three decision points | S | **Keep** |
| Launch at login (A-64) | Existing `autostart.rs` (SMAppService, `objc2-service-management` 0.3.2) | – | Present (R2 A-64) | – | – | **Keep** |
| Permissions UI | tauri-plugin-macos-permissions 2.3.0 (in use) ([repo](https://github.com/ayangweb/tauri-plugin-macos-permissions)) | MIT (crates.io and GitHub, obs) | 172★, last push 2025-12-14 (obs) | Covers Accessibility and Screen Recording. Add the audio-capture probe (§6) and the Accessibility health watch (§3) beside it. | – | **Keep** |
| Multi-display | objc2-app-kit notifications (§5) | – | – | – | S | **Adopt** |

---

## Recommended set (one choice per capability)

| # | Capability | Choice | Closes (R2 IDs) | Cost |
|---|---|---|---|---|
| 1 | AX client | **objc2-application-services 0.3.2** + a fork `ax.rs` safe wrapper (focused element, role/subrole, selected text and range, value, `AXStringForRange`, `AXBoundsForRange`, set `AXSelectedText`, messaging timeout, lazy `AXManualAccessibility`). Fallbacks: ⌘C with full snapshot (#4), then blind paste plus card. | AF-05, A-50, A-51, D-29, D-30, D-58 (enabler), D-52 (AX read-back option), RC-04 | M |
| 2 | Frontmost app / title / URL | **objc2-app-kit NSWorkspace** (in tree) + AX window title + **port of mado's AX URL logic** (MIT); no AppleScript | D-21, D-22 (DEC-23: AX URL + title fallback) | S + M |
| 3 | Hotkeys | **Keep handy-keys**, fix the `HandyKeysState::new` error path; **port anarlog `shortcut-macos` HotKeyProcessor** (MIT; Hex lineage) into the coordinator; **vendor axwatch** (MIT/Apache) plus a probe-tap health check; remove the unused `rdev` | AF-08, A-208, A-22, A-23, A-201, A-25 (fn case), A-35 (part; Globe = RC-02), DEC-09 inputs | S + M |
| 4 | Clipboard | **Own multi-item snapshot/restore on objc2 NSPasteboard** (Hex pattern) in `paste_tx/macos.rs` + `clipboard.rs`; **arboard `exclude_from_history()` and `.html()`** for default-path writes; promote reliable paste | AF-17, A-47, A-48, D-29 (fallback prerequisite), C-70 (HTML), A-204 (keep concealed unless "copy to clipboard") | S |
| 5 | Overlay / pill | **tauri-nspanel 2.1.1** from crates.io + `accept_first_mouse(true)` via `with_window` + **anarlog overlay click-through pattern** (MIT) + nspanel tracking area for hover + screen-parameters observer + `set_content_protected` | AF-09, A-01 (infrastructure), A-02, A-04/A-06/A-07 (first click), A-15 (clickable card), A-16 (click-through dim), A-212, E-28 | S + S-M |
| 6 | System audio and meetings | **Vendor flexaudio-os-macos's global exclude-self tap** (MIT, objc2-core-audio 0.3.2) gated to 14.4+; **public probe-tap permission check** (meeting-record pattern) + `NSAudioCaptureUsageDescription`; **call detection** on objc2-core-audio process objects; **objc2-event-kit** for local calendar | AF-10, E-01, E-02, E-205, E-206, E-07 (calendar part), DEC-22 input (14.4 gate) | M + M |
| 7 | Notifications / menus / capabilities / Dock | **tauri-plugin-notification** (`~2.3` on Tauri 2.11, or bump Tauri to 2.12 for 2.8); **Tauri `Menu::default` + inserted submenus**; **prefixed capability globs**; keep the activation-policy code | E-91, DEC-21, C-17 (system half), AF-15, A-63, C-203, AF-16, C-75, E-12, A-65 | S each |

**Not recommended:**
- Rejected: selection (GPL-3.0), tauri-plugin-clipboard (stale), clipboard-rs (flattens items), active-win-pos-rs (needs Screen Recording for titles), cidre and screencapturekit-rs for 14.4+ (second ecosystem / heavier permission), rdev (unused).
- Docs only: aw-watcher-window (MPL-2.0), FluidVoice (GPL-3.0, not opened).
- Not checked: VoiceInk and Epicenter, whose license shows as NOASSERTION.

---

## Needs a runtime check on a real Mac

| ID | Check | Why |
|---|---|---|
| RC-01 | First click on the pill ✕ with and without `accept_first_mouse(true)` in the never-key panel | #5; A-04/A-06/A-07 |
| RC-02 / RC-15 | Does blocking the fn press stop the Globe action (emoji, input source, Apple Dictation) at the **session** tap (handy-keys) and at the **HID** tap (Hex/anarlog)? Test macOS 14, 15 and 26, each "Press 🌐 key to" value, and the F5 dictation key. | #3; A-35, DEC-09 |
| RC-03 | CSS `:hover` in the non-key panel against a tracking area with `active_always` | #5; A-02 |
| RC-04 | AX in Slack, VS Code, Discord, Notion, Chrome, Arc, Safari, Mail and Word: focused element, `AXSubrole`, `AXSelectedText`, `AXSelectedTextRange`, `AXBoundsForRange`, with and without `AXManualAccessibility`; latency at a 0.1 s / 0.25 s messaging timeout | #1; A-50, A-51, D-29, D-58 |
| RC-04b | Does setting `AXSelectedText` really replace text in Chromium/Electron editors, or report success while doing nothing? | #1; D-30 |
| RC-04c | Browser URL via AX in Chrome, Arc, Brave, Edge, Safari and Firefox without enabling flags; does `AXEnhancedUserInterface` disturb window managers (Rectangle) or animations? | #2; D-22 |
| RC-08 | Process tap: the TCC prompt and Settings pane that appear; whether the probe tap reports "granted" without a restart; zero-sample behavior when denied | #6; E-02 |
| RC-08b | Does a **global** exclude-self tap follow an AirPods connect or disconnect mid-call without being re-created? | #6; E-206 |
| RC-08c | Excluding our own PID when Handy has no audio process object yet (audiotee README warning): does translation fail? Must we exclude lazily after the first sound plays? | #6; E-205 |
| RC-08d | Does mute-while-dictating (`osascript` output mute) silence the global tap? | #6; E-205 |
| RC-14 | After sleep or wake and after Accessibility is toggled off, on, or the row is removed: does handy-keys' tap still deliver? Do the axwatch notifications arrive in a signed build? Does `AXIsProcessTrusted` lie after row removal (axwatch caveat)? | #3; A-208 |
| RC-16 | Restore with a `TransientType` marker: does Maccy (and Paste, Raycast) skip it? Are RTF, HTML, file URLs and multiple items restored byte-identical? | #4; A-47, A-48 |
| RC-17 | `set_content_protected` on the pill: hidden from ScreenCaptureKit recorders (Zoom share, QuickTime, OBS)? | #5; E-28 |
| RC-18 | `kAudioProcessPropertyIsRunningInput` / `ProcessObjectList` availability on 14.0-14.3 and reliability for Zoom, Teams, Meet in Chrome, and FaceTime | #6; call detection |
| RC-19 | tauri-plugin-notification on macOS 15/26: delivery from a signed bundle (and from `tauri dev`), click callback, permission prompt timing | #7; E-91 |
| RC-20 | `frontmostApplication` freshness when read off the main thread against AX `AXFocusedApplication` | #2; D-21 |
| RC-21 | Click-through toggling latency at 20 Hz: missed first clicks when the cursor moves fast onto the pill | #5; A-01 |
