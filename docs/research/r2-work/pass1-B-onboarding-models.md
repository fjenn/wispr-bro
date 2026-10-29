# Pass 1-B: Onboarding, first run, guided learning, models and languages

**Scope.** Gap analysis of Handy (branch `wispr-bro`) against the Wispr Flow teardown (`docs/research/R1-wispr-flow-teardown.md`, read in full; sections 10 to 13 are central) for: the first-run journey, per-feature guided learning, permissions, speech models (catalog, recommendation, hardware detection, download, management, migration, override) and languages (spoken languages, detection, app language/i18n).

**Method.** Read-only. Every Handy claim carries a tag: `verified-in-code` (I read the lines cited), `inferred` (reasoned from code I read), `unknown` (not determined). Line numbers are from the files as they stand on `wispr-bro`. Sizes and picks in the model section were computed by running the package's own `recommend()` from `node_modules/@handy-computer/models` (read-only, a scratch script outside the repo).

**Files read.**
- Frontend: `src/App.tsx`; `src/components/onboarding/` : `onboardingStore.ts`, `OnboardingFlow.tsx`, `usePermissions.ts`, `OnboardingShell.tsx`, `StandaloneScreens.tsx`, `ModelMigration.tsx` (skimmed, key lines), `DownloadStatus.tsx`, `downloadPick.ts`, `useOnboardingModels.ts`, `useTryEngine.ts`, `TrayPanel.tsx`, `screens/Welcome.tsx`, `screens/Permissions.tsx`, `screens/Model.tsx`, `screens/Demo.tsx`, `screens/TryIt.tsx`; `src/hooks/useMachineTier.ts`, `src/hooks/useModelCatalog.ts`; `src/lib/models/catalog.ts`, `grading.ts`, `retired.ts`, `downloadState.ts` (head); `src/stores/modelStore.ts` (action and listener lines); `src/components/settings/models/ModelsSettings.tsx`; `src/components/settings/general/GeneralSettings.tsx`, `ModelSettingsCard.tsx`; `src/components/settings/PreferredLanguages.tsx`; `src/components/Sidebar.tsx` (section config); `src/components/model-selector/ModelSelector.tsx` (imports), `src/components/footer/Footer.tsx` (grep); `src/components/whats-new/WhatsNewModal.tsx`; `src/lib/utils/languageSuggestions.ts`; `src/i18n/index.ts`, `src/i18n/languages.ts`, `src/i18n/locales/en/translation.json` (onboarding keys); `package.json`.
- Package: `node_modules/@handy-computer/models/` : `package.json`, `dist/src/recommendations.js`, `create.js`, `index.js`; `catalog.json` (identical to the Rust copy, checked with `diff`).
- Backend: `src-tauri/src/catalog/mod.rs` (full), `catalog.json` (structure, methodology, benchmarks, sizes); `src-tauri/src/managers/model.rs` (retired table, auto-select, HF download, URL download, cancel, list ordering); `src-tauri/src/managers/model/download.rs` (resume logic, grep); `src-tauri/src/commands/models.rs` (full); `src-tauri/src/settings.rs` (fields, defaults, migrations); `src-tauri/src/language_detection/mod.rs` (head), `policy.rs` (signatures); `src-tauri/src/managers/transcription.rs:1936-1943`; `src-tauri/src/shortcut/mod.rs:1520-1530`; `src-tauri/src/overlay.rs:730-766` (grep); `src-tauri/Info.plist`.
- Not opened: `docs/research/evidence/` (per rules).

**Shape of Handy's onboarding today** (`verified-in-code`, `onboardingStore.ts:17-24`): five screens, Welcome (languages + app language) > Permissions (skipped when already granted) > Model (pick 1 of 3, or "Show all") > Demo (animated laptop, waits for the download) > Try it (real dictation into a drawn Notes app, hold then tap) which morphs into "All set" (tray panel summary, launch at login, Done). Progress persists in `localStorage` across relaunches.

## Feature table

Legend: Class HAVE / TUNE / EXTEND / BUILD / OUT. Effort S (under 2 days), M (up to about a week), L (more).

### A. First-run journey (R1 section 13)

| ID | Feature | R1 ref | Wispr behavior | Handy today (evidence path:line) | Class | Effort | Notes / risks | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| B-01 | Named-stage stepper in header | 13 frame | GET STARTED > PERMISSIONS > SET UP > LEARN > PERSONALIZE, thin purple progress line | Unlabeled dashes, current one wider: `OnboardingShell.tsx:40-61`; count derives from `STEP_ORDER` minus skipped Permissions `OnboardingFlow.tsx:128-132` (verified-in-code) | TUNE | S | Stage labels must be translated into 26 locales | Plain component; no lib needed |
| B-02 | Back link and Help chip | 13 frame | Back top-left, Help chip bottom-left | Back in footer `OnboardingShell.tsx:69-80`; no Help chip anywhere in the shell (verified-in-code) | TUNE | S | Help chip needs a local destination (FAQ page or GitHub link) | - |
| B-03 | Two-column full-window layout, calm illustration/demo on the right; dark "Learn" screens | 13 frame | Left: heading and controls; right: illustration, demo, or macOS screenshot; later screens dark | Centered single column in a fixed 720x600 box: `OnboardingShell.tsx:38`; no dark variant for learn screens (verified-in-code) | TUNE | M | Fixed size means no scaling on large windows; every screen's layout would change | Design work, no code reuse |
| B-04 | Primary button disabled until a choice is made | 13 frame | Black pill, grey when disabled | Same pattern: `Welcome.tsx:85-92`, `Permissions.tsx:133-143`; disabled styling forced neutral `OnboardingShell.tsx:38` (verified-in-code) | HAVE | - | Visual restyle only | - |
| B-05 | Welcome / sign-up screen | 13 (unknown in R1) | Sign-up before recording started (unknown details) | Welcome with mascot, title "Speak instead of typing, in any app", language picker, no account: `Welcome.tsx:78-123`; copy in `en/translation.json` `onboarding.welcome.*` (verified-in-code) | HAVE | - | Sign-up itself is OUT | - |
| B-06 | "How can Flow help you?" multi-select with live demo per card | 13.1 | Dictation / Notetaker cards; right side animates a demo of the selected card | Absent (no such screen in `STEP_ORDER`, `onboardingStore.ts:17-24`) (verified-in-code) | BUILD | S | Only meaningful once there is a second product mode (Notetaker is OUT for v1). Recommend skipping in v1 | Reuse `Demo.tsx` laptop engine for the per-card demo |
| B-07 | "How do you currently record meetings?" chips | 13.2 | Meeting-tool quiz | Absent | OUT | - | Cloud/meeting product only | - |
| B-08 | "What's frustrating about meetings?" | 13.3 | Multi-select quiz | Absent | OUT | - | Same | - |
| B-09 | Permission rows that tick as granted | 13.4 | Three rows, each ticks when granted | Microphone and (macOS) Accessibility rows, granted in order, status Allowed / Waiting spinner: `Permissions.tsx:27-80`, `:99-103`, `:170-200` (verified-in-code) | HAVE | - | Copy is already privacy-first ("never uploaded", `onboarding.permissions.microphone.why`) | - |
| B-10 | Picture of the macOS System Settings screen and the exact prompt before it appears | 13.4, 11 patterns | Right half shows the system prompt and Settings pane to expect | Icons only (`Mic`, `Keyboard`): `Permissions.tsx:172`, `:190`; "while waiting" text instruction instead: `Permissions.tsx:104-111`, `:54` (verified-in-code) | EXTEND | M | Screenshots vary by macOS version and language; vector mock-ups age better. Windows needs its own | Draw as SVG like `DesktopBar.tsx` / `WindowBar.tsx`; look for MIT permission-illustration kits |
| B-11 | Microphone permission: request, then System Settings if denied | 13.4 | Real macOS prompt follows | Prompt on first ask, then opens privacy settings: `usePermissions.ts:231-252`; detects "no prompt shown" by focus after 1 s `:239-243` (verified-in-code) | HAVE | - | Info.plist usage string is terse: "Request microphone access to transcribe audio locally" `src-tauri/Info.plist:5-6` (TUNE copy) | - |
| B-12 | Accessibility permission with polling, no-relaunch init, relaunch escape hatch | 13.4 | (Wispr does not ask for Accessibility in the observed frames; it asks for "recognize meetings" and mic) | Request + 1 s polling + `initializeEnigo/initializeShortcuts` on grant: `usePermissions.ts:77-83`, `:143-176`, `:219-229`; "Relaunch Handy" link appears after grant: `Permissions.tsx:113-117`, `:201-212` (verified-in-code) | HAVE | - | Relaunch link exists because the shortcut sometimes does not respond until relaunch (inferred from copy `onboarding.permissions.relaunch`) | - |
| B-13 | Resume where you left off after relaunch | 11 (gate restarts app), 13 | After granting System Audio the app restarts and the gate is gone | Whole flow state persisted to `localStorage` key `handy.onboarding`: `onboardingStore.ts:26-46`, `:60`, `:79-119`, `:121-140`; "Welcome back. Picking up where you left off." `Permissions.tsx:115`, `:148-156` (verified-in-code) | HAVE | - | Strength; keep. Two sources of truth (localStorage + backend `onboarding_completed`), see Rough edges | - |
| B-14 | System audio permission ("capture meeting audio") with picture | 11, 13.4 | Turn on > System Settings > Screen & System Audio Recording; app restarts | Not requested; only mic + accessibility (`Permissions.tsx:100-102`). Plugin exposes screen-recording check/request (`node_modules/tauri-plugin-macos-permissions-api/dist-js/index.d.ts:63-77`) (verified-in-code) | OUT | - | Needed only for Notetaker (later milestone). Plugin already present if needed | `tauri-plugin-macos-permissions` (already a dependency) |
| B-15 | "Allow Wispr to recognize meetings" | 13.4 | Permission row | Absent | OUT | - | Meeting detection, later | - |
| B-16 | Connect your calendar | 13.5 | Google / Outlook OAuth in browser | Absent | OUT | - | Account/cloud. No local equivalent needed | - |
| B-17 | Returning-user permission check | 11 gate, implied | Gate reappears if permission missing | On every launch: macOS checks accessibility + mic, Windows checks mic; shows standalone Permissions screen: `App.tsx:263-303`, `StandaloneScreens.tsx:13-19`. Mid-session revocation surfaces only as a toast on recording: `App.tsx:156-165` (verified-in-code) | HAVE | - | Could add a periodic/foreground re-check with a banner (TUNE later) | - |
| B-18 | Windows/Linux permission handling | n/a (macOS observed) | - | Windows mic status via `getWindowsMicrophonePermissionStatus`: `usePermissions.ts:72-75`, `:127-137`; Linux has no permission step, shows a Linux guide link: `TryIt.tsx:493-504` (verified-in-code) | HAVE | - | Platform later; keep as is | - |
| B-19 | Test your microphone: live level meter ("Do you see purple bars?") with Yes | 13.6 | Live purple bars, Change microphone, Yes | No live meter in onboarding. Levels are emitted only to the overlay window: `overlay.rs:766` (`emit_to("recording_overlay", "mic-level", …)`). Try it detects a silent mic after a take and offers a picker: `useTryEngine.ts:171-174`, `TryIt.tsx:425-450` (verified-in-code) | EXTEND | M | Needs a "monitor mic without transcribing" command and an emit to the main window; must not hold the mic open after the step (macOS orange dot) | Reuse `audio_toolkit` level callback (`managers/audio.rs:471-472`, `:516`) |
| B-20 | Change microphone list with "Built-in mic (recommended)", Auto-detect, Bluetooth caution | 13.6 | Recommends built-in or wired; Bluetooth less reliable | `MicrophoneDropdown` offered only on a silent take: `TryIt.tsx:438-446`; no "recommended" label or Bluetooth note in onboarding copy (verified-in-code for onboarding; settings picker not inspected in detail) | EXTEND | S | Overlaps with the microphone area of another pass; "recommended" needs a heuristic (built-in device name) | - |
| B-21 | Test the dictation shortcut: on-screen key lights up when pressed, Yes/No | 13.7 | fn key graphic turns purple on press | Keycaps light up from real recording events: `useTryEngine.ts:117-136` (`recordDown`), `TryIt.tsx:555-563`; Demo keycaps are animated, not live: `Demo.tsx:270-277`. Try it requires the model on disk (Demo button disabled until ready: `Demo.tsx:517-524`) (verified-in-code) | EXTEND | S | A key-press test that does not need a model would let users verify the shortcut while the model downloads | Reuse `useShortcutCapture` + recording-state events |
| B-22 | Edit shortcut in place; Reset to default chip | 13.7 | Opens the hotkey panel; Reset to default | In-place capture on Demo (`Demo.tsx:492-502`, `:555-596`), Try it (`TryIt.tsx:360-367`), All set panel (`TrayPanel.tsx:146-182`). No reset-to-default control on these screens (verified-in-code) | TUNE | S | Add Reset chip when changed | - |
| B-23 | fn (Globe) key as the default | 3, 13.7 | Hold fn | macOS default is `option+space`: `settings.rs:945-946` (verified-in-code). Whether the key backend (`handy_keys`, `shortcut/mod.rs:1534`) can capture fn alone: unknown | EXTEND | M | fn needs a low-level event tap on macOS; owned by the hotkeys pass, listed here because the onboarding test screen depends on it | Look for MIT Swift/Rust fn-key monitors (CGEventTap, IOHID) |
| B-24 | Animated product demo of how dictation works | 13.1, 13.12 | Simulated composer with the pill underneath | Full scripted laptop demo, hold and tap chapters, reduced-motion stills: `Demo.tsx:37-67`, `:152-303`, `:375-600` (verified-in-code) | HAVE | - | Strength; reusable for B-31, B-40, B-42 | - |
| B-25 | Practice in a simulated app ("Write an email hands free"), Skip link | 13.8 | Gmail composer with To/Subject; dictate; Skip top-right | Real dictation into a drawn Notes app; two lessons (hold, then tap); Skip before first take, Finish after: `TryIt.tsx:588-607`, `:491-517`; lessons persisted `onboardingStore.ts:40-41` (verified-in-code) | TUNE | S | Swap or add an email composer scene to show formatting; Skip placement | Reuse `WindowBar`, `useTryEngine` |
| B-26 | Detect and explain failed first takes | (not in Wispr) | - | Distinguishes failed, silent mic, empty, landed outside the note: `useTryEngine.ts:155-224`; messages `TryIt.tsx:424-459` (verified-in-code) | HAVE | - | Better than Wispr; keep | - |
| B-27 | "Nice job! You just spoke" celebration | 13.9 | Dark dramatic screen | Mascot high five, burst, glove flies to tray icon: `TryIt.tsx:221-305`, `:345-352`, `:633-657` (verified-in-code) | HAVE | - | Different tone (playful vs dramatic); restyle is a brand call | - |
| B-28 | Time-saved pitch: "you could save N hours a week", typing-time slider | 13.10 | Counter, slider (default 6 h/day) | Absent (verified-in-code: no such screen) | BUILD | S | Pure frontend math from WPM numbers (B-29) | - |
| B-29 | 30-second typing speed test with live WPM, language picker, sample text | 13.10 | Monospace paragraph, words/min, seconds left, Try again | Absent. Speaking rate is computable: `transcriptionCompleteEvent` carries `audio_seconds` and `wait_seconds` (`useTryEngine.ts:195`) (verified-in-code) | BUILD | M | Sample paragraphs per language need translation | Small MIT typing-test React components |
| B-30 | Referral "Give the magic of Flow. Get a month of Pro" | 13.11 | Invite link, Copy | Absent | OUT | - | Local equivalent (user-reported, R1 section 10): optional "Share with a friend" link | - |
| B-31 | "How would you like to use Flow first?" chooser with captioned looping demos, "Try it now" | 13.12 | 7 chips (Chat with AI, Note, Message, Email, Post, Document, Prompt Cursor) | Absent (verified-in-code) | BUILD | M | Videos would bloat the bundle; animated scenes are lighter | Reuse `Demo.tsx` scene engine with per-app scenes |
| B-32 | "Next step" spotlight coaching on the real pill | 13.13, 10 (Transforms) | Screen dims, pill highlighted, hint "Open your email inbox" | Absent. Only a drawn tray icon/panel inside the onboarding window: `TryIt.tsx:697-713`, `TrayPanel.tsx:93-201` (verified-in-code) | BUILD | M | Needs a transparent overlay/hint near the real pill (overlay window exists, `overlay.rs`) | Reuse `overlay.rs` window; coach-mark libs only for in-window parts |
| B-33 | "Try Flow in another app / Get started" banner on home | 13.13, 13.14 | Banner on the Dictation home | Absent; the app opens on General settings: `App.tsx:87-88`, `:396` (verified-in-code) | BUILD | S | Depends on a Home/Hub page (other pass) | - |
| B-34 | End state: hub with stats, "New!" tags | 13.14 | Normal hub, stats, New! tag | Flow ends in Settings (General) after `completeOnboarding`: `OnboardingFlow.tsx:151-160`, `App.tsx:396` (verified-in-code) | EXTEND | M | Cross-reference Hub pass | - |
| B-35 | Launch at login choice | 5 System | On by default in Settings | Toggle on All set panel, default on, saved at Done (avoids repeated macOS notifications): `onboardingStore.ts:44-45`, `:72`; `TrayPanel.tsx:185-198`; `OnboardingFlow.tsx:151-156` (verified-in-code). Backend default is off: `settings.rs:592-594` | HAVE | - | Keep | - |
| B-36 | Summary of choices at the end | (not in Wispr) | - | All set panel lists Languages, Speech model, Shortcut, each editable with return: `TrayPanel.tsx:111-182`; detour logic `OnboardingFlow.tsx:133-139`, `:246` (verified-in-code) | TUNE | S | Remove or demote the "Speech model" row for a no-model-picking product | - |
| B-37 | Pace: ~5 min, single-tap screens, never blocked | 13 "what makes it work" | Most screens single-choice | 5 screens, but "Try it yourself" is disabled until the model is on disk: `Demo.tsx:517-524`, `OnboardingFlow.tsx:174-177` (verified-in-code) | TUNE | S | On slow networks the user waits on a looping demo; see model section | - |

### B. Guided learning everywhere (R1 sections 10 to 12)

| ID | Feature | R1 ref | Wispr behavior | Handy today (evidence path:line) | Class | Effort | Notes / risks | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| B-38 | Dismissible example banners per feature (Dictionary, Snippets, Transforms) | 4, 10, 11 | Banner with concrete example chips, dismissible | None found (`rg` for banner/dismiss/tutorial in `src/components` matched only What's New, secure-input warning, dialogs) (verified-in-code) | BUILD | M | Needs persisted "dismissed" ids (settings or localStorage) | Generic banner component; no lib needed |
| B-39 | "How it works" modal with video | 10 (Transforms) | Modal with hosted ~1.5 min video | Modal infra exists: `components/ui/Dialog.tsx`, used by `WhatsNewModal.tsx:20-30` (verified-in-code); no videos | EXTEND | M | Hosted video is cloud; local videos grow the app. Prefer animated scenes | Reuse Dialog + `Demo.tsx` scenes |
| B-40 | "Try it out" multi-step practice with simulated app, progress bar, then "Turn on X" | 10 (Transforms) | Two steps, composer, result, spotlight on pill | Engine for practice exists only in onboarding: `useTryEngine.ts` (whole file), `TryIt.tsx` (verified-in-code) | EXTEND | M | `useTryEngine` is coupled to the onboarding store (`useTryEngine.ts:44`, `:191-193`); needs a lesson-agnostic version | Refactor `useTryEngine` |
| B-41 | Spotlight on pill after a practice ("Done. See changes") | 10 | Screen dims, pill highlighted | Same gap as B-32 | BUILD | (in B-32) | Shared implementation | - |
| B-42 | Whisper tutorial (3 steps: video, how to whisper with built-in mic, whisper an email; Continue disabled until it lands) | 11, 12 | Simulated Gmail composer | Absent (verified-in-code) | BUILD | M | Local whisper accuracy depends on model and mic gain; risk of a tutorial that fails | Reuse practice engine (B-40) |
| B-43 | Personalization quiz with mic recommendation table ("Best match"), Done / Email results | 12 | Office/desk/mics owned quiz; comparison table | Absent | BUILD | M | Static data; "Email results" is cloud: replace with Copy. Low priority | - |
| B-44 | Sidebar setup checklist with progress, items tick automatically | 4, 11, 12 | "Set up Notetaker" card: tour, calendar, import, MCP | Absent; sidebar sections are fixed: `Sidebar.tsx:36-75` (verified-in-code) | BUILD | M | Local items: try in another app, add a word, whisper tutorial, set launch at login | - |
| B-45 | Re-entry: Help > Essentials, "Start tutorial" | 10, 11 | Help popover; tutorial restart buttons | Re-running onboarding only from Debug: `App.tsx:316-320` (`runOnboarding`), Debug section gated `Sidebar.tsx:66-69` (verified-in-code) | EXTEND | S | Expose "Replay tutorial" outside Debug | - |

### C. Permissions handling (cross-cutting, beyond first run)

Covered by B-09 to B-18. One extra row:

| ID | Feature | R1 ref | Wispr behavior | Handy today (evidence path:line) | Class | Effort | Notes / risks | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| B-46 | Waiting-state recovery (prompt dismissed, user away in Settings) | 13.4 implied | Rows tick when granted | Re-check on window focus and after 15 s while in front; up to 3 polling errors then toast: `usePermissions.ts:33-35`, `:180-217`, `:167-173` (verified-in-code) | HAVE | - | Strength | - |

### D. Speech models

| ID | Feature | R1 ref | Wispr behavior | Handy today (evidence path:line) | Class | Effort | Notes / risks | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| B-47 | No model choice for the user (silent default) | 13 closing note | Cloud; no model concept | Explicit "Choose a speech model" step with 3 cards (Recommended / Faster / More accurate), grades per language, speed bars, sizes, "Show all N models" table: `Model.tsx:77-194`, `:518-563`, `:565-633`; recommended is preselected: `Model.tsx:99-100` (verified-in-code) | EXTEND | M | Remove the step from `STEP_ORDER` (`onboardingStore.ts:17-24`); call the same picks function after Welcome and start the download. Keep the screen for Advanced | Logic already in `picksFor` (`catalog.ts:97-109`) |
| B-48 | Curated catalog with benchmarks and recommendation method | - | - | `@handy-computer/models` 0.2.3 (`package.json:29`); 76 models, 29 curated ASR; per-language FLEURS error with CI, real-time factor "accelerated" and "cpu"; methodology thresholds; `recommend()` returns best/fast/accurate: `recommendations.js:158-226`. Compiled into the binary, zero network for the list: `catalog/mod.rs:1-18`, `:307-315` (verified-in-code) | HAVE | - | Strength; reuse as is (MIT) | - |
| B-49 | Hardware detection feeding the choice (GPU/CPU, chip, RAM, disk) | 13 closing note ("auto-chosen from the hardware" is our goal) | n/a | Only "is there a GPU and CPU not forced": `useMachineTier.ts:7-18` via `get_available_accelerators` (`transcription.rs:1936-1943`). The package states picks "do not depend on the machine": `recommendations.js:150-157`. No RAM/chip/disk code (no matches for memory or free-space APIs in `src-tauri/src`) (verified-in-code) | EXTEND | M | Benchmarks were measured on a Ryzen 4750U (Vulkan + CPU), not Apple Silicon (`catalog.json` `benchmarks.speed`) | `sysinfo` crate (RAM, CPU brand), `fs4`/`sysinfo` for free disk; `sysctl hw.memsize` |
| B-50 | Background download that never blocks setup | 13 closing note | n/a | Starts when Model's button is pressed (`OnboardingFlow.tsx:162-173`, `downloadPick.ts:10-33`), continues during Demo ("Downloads while you finish setup", `onboarding.model.hint`), but Try it waits for it: `Demo.tsx:517-524` (verified-in-code) | EXTEND | M | Start right after languages are known (or at launch from OS languages); let practice and the app open while downloading, with a "getting ready" state | - |
| B-51 | Download progress UI (size, speed, ETA, verifying, ready, stopped) | - | - | `DownloadStatus.tsx:13-124`; full-card line `Model.tsx:220-257`; states in `downloadState.ts` (verified-in-code) | TUNE | S | Copy names the model ("Downloading Parakeet…", `onboarding.download.downloading`); for a no-model product say "Getting speech recognition ready" | - |
| B-52 | Resume an interrupted download | - | - | Backend keeps partials and resumes with HTTP Range, falls back to mirror, verifies sha256: `download.rs:86-87`, `:216-311`; URL path keeps partial on cancel `model.rs:2336-2341`; HF path mirror list `model.rs:1898`, `catalog/mod.rs:32-36`, `:384-408`. Onboarding, after a relaunch, marks it failed and waits for Retry: `OnboardingFlow.tsx:58-67`, `Demo.tsx:512-515` (verified-in-code) | EXTEND | S | Auto-restart the download on relaunch instead of showing "stopped" | - |
| B-53 | Failure handling: offline, server error, retry | - | - | Event + log on failure `commands/models.rs:51-59`; toast host present during onboarding `App.tsx:345-365`; manual Retry `Demo.tsx:512-515` (verified-in-code) | TUNE | S | Add automatic retry with backoff and an offline message; whether backend retries internally: `download.rs:24-26` says a retry loop exists (verified-in-code, details not read) | - |
| B-54 | Disk-space check before download | - | - | None found (verified-in-code: no free-space API usage) | BUILD | S | Worst default is 2.4 GB (see model section) | `fs4` or `sysinfo` crate |
| B-55 | Model management page | - | - | Top-level "Models" sidebar section (`Sidebar.tsx:48-51`, `App.tsx:57-61`): Yours / Fits your languages / Missing a language / Untested, rescan, change languages: `ModelsSettings.tsx:48-181`; footer model switcher `Footer.tsx:33` (verified-in-code) | TUNE | S | Move under Advanced; hide footer switcher | - |
| B-56 | Advanced override | goal | - | Override exists everywhere (Models page, footer, onboarding) rather than in Advanced (same evidence as B-55) | TUNE | S | Keep `Model.tsx` as the Advanced "change model" screen (it is already standalone-capable: `StandaloneScreens.tsx:21-76`) | - |
| B-57 | Retired-model migration | - | - | Backend table of 11 retired ONNX models with GGUF replacements: `model.rs:763-856`; blocking update screen for returning users: `App.tsx:237-253`, `:403-417`; `ModelMigration.tsx:41-253`; Models page notice `RetiredModelsNotice.tsx` (verified-in-code) | TUNE | S | Fork users start fresh, so rarely triggered. If kept: make silent (download replacement in background, swap, delete old) | - |
| B-58 | Fallback auto-select when the selected model disappears | - | - | Clears a missing selection, then picks the first downloaded model by Handy's editorial rank, not by spoken languages: `model.rs:1448-1507`, ordering `model.rs:1096-1114`, rank table `catalog/mod.rs:38-53` (verified-in-code) | EXTEND | S | Rank 1 is English-only Parakeet; a non-English user with several models on disk could get it (inferred) | - |
| B-59 | Measure real speed on this machine and adapt | - | - | First takes are timed; "X took Ns on this computer. Try a faster model" link: `useTryEngine.ts:194-196`, `catalog.ts:201-205`, `TryIt.tsx:716-733` (verified-in-code) | EXTEND | S | Switch silently to the package's "fast" pick (download in background) instead of asking | - |
| B-60 | Better default appears later (catalog update, upgrade path) | - | (cloud, invisible) | Catalog is compiled in (`catalog/mod.rs:307-315`); the package supports a runtime catalog (`create.js:4-9`) but Handy does not fetch one (verified-in-code) | BUILD | M | Ship catalog updates with app updates; optional "a better model is available" background upgrade | - |

### E. Languages

| ID | Feature | R1 ref | Wispr behavior | Handy today (evidence path:line) | Class | Effort | Notes / risks | Reuse hint |
|---|---|---|---|---|---|---|---|---|
| B-61 | "My languages" (multi) in Settings and menus | 5 General, 6 menus | "My languages: Change"; Languages ▸ in Dictation and status menus | Picker on Welcome `Welcome.tsx:35-47`, `:105-124`; "Languages" group in General `GeneralSettings.tsx:35-40`, `PreferredLanguages.tsx:20-139`; stored as `spoken_languages` `settings.rs:452-455` (verified-in-code). Tray/menu submenu: unknown (not inspected) | HAVE | - | Menu submenu belongs to the menu-bar pass | - |
| B-62 | Languages preselected from the system | 8.1 ("language preselected") | Preselected | From OS languages injected by backend, else browser locale, with English: `languageSuggestions.ts:20-28`; seeded on fresh flow `onboardingStore.ts:64` (verified-in-code) | HAVE | - | - | - |
| B-63 | Warn when a language is poorly supported | (not in Wispr; "100+ languages") | - | "no model hears it well yet" hint: `Welcome.tsx:60-69`, `catalog.ts:69-75`; same in settings `PreferredLanguages.tsx:65`, `:130-136` (verified-in-code) | HAVE | - | Honest; keep | - |
| B-64 | Changing languages re-picks the model | goal | (cloud) | Settings only write `spoken_languages`: `PreferredLanguages.tsx:35-40`; on Welcome, a pick that no longer fits is reset but only inside onboarding: `Welcome.tsx:35-46` (verified-in-code) | EXTEND | S | Re-run the pick and download in the background when the current model misses a new language | Same pick function as B-47 |
| B-65 | Automatic detection among my languages | 13.10 ("100+ languages") | Implicit | Detector chooses among spoken languages for models without their own detection; never blocks: `language_detection/mod.rs:1-4`, `:39-50`; `policy.rs:106-110`, `:152-160`; detector (45 MB, `lang-id-voxlingua107-ecapa` F16) downloaded together with the model as one progress bar: `model.rs:1860-1890`, `:1918-1935` (verified-in-code) | HAVE | - | Strength | - |
| B-66 | Chinese script choice | - | - | On Welcome when Chinese is picked: `Welcome.tsx:126-165`; saved `OnboardingFlow.tsx:179-186`; migrations `settings.rs:1246-1287` (verified-in-code) | HAVE | - | - | - |
| B-67 | App (UI) language dropdown in General | 5 General | "App Language (dropdown)" | Menu on Welcome `Welcome.tsx:71-81`; settings control lives in About: `AboutSettings.tsx:43`; default from OS locale `settings.rs:700-704`; 26 locales `i18n/languages.ts:18-49` (verified-in-code) | TUNE | S | Move to General. Every new screen in this table adds strings for 26 locales (ESLint requires i18n keys) | - |
| B-68 | i18n infrastructure and RTL | - | - | i18next, RTL direction sync: `i18n/index.ts:90-128`, `App.tsx:109-112` (verified-in-code) | HAVE | - | Starts in English then syncs async (`i18n/index.ts:92`, `:104-128`): possible English flash on non-English systems (inferred) | - |

### Class counts

- HAVE (20): B-04, B-05, B-09, B-11, B-12, B-13, B-17, B-18, B-24, B-26, B-27, B-35, B-46, B-48, B-61, B-62, B-63, B-65, B-66, B-68
- TUNE (13): B-01, B-02, B-03, B-22, B-25, B-36, B-37, B-51, B-53, B-55, B-56, B-57, B-67
- EXTEND (16): B-10, B-19, B-20, B-21, B-23, B-34, B-39, B-40, B-45, B-47, B-49, B-50, B-52, B-58, B-59, B-64
- BUILD (13): B-06, B-28, B-29, B-31, B-32, B-33, B-38, B-41, B-42, B-43, B-44, B-54, B-60
- OUT (6): B-07, B-08, B-14, B-15, B-16, B-30

Total rows: 68 = 20 + 13 + 16 + 13 + 6.

## Automatic model choice: current state and gap

**What data Handy has today** (`verified-in-code` unless marked):
- **Per model** (`catalog.json`, mirrored in `node_modules/@handy-computer/models/catalog.json`): id, name, editorial description, HF repo + pinned revision, family, parameter count, languages, capabilities (streaming, translate, languageDetection, timestamps), default quant (Q8_0 for every offered model, asserted in `catalog/mod.rs:528-541`), every quant file with byte size and sha256, `minEngine`.
- **Benchmarks** (`results`): per language word/char error % on FLEURS test with a lower confidence bound; real-time factor on two tiers, `accelerated` and `cpu`, both measured on a **Ryzen 4750U** (Vulkan backend for accelerated) at Q8_0 (`benchmarks` block).
- **Method thresholds** (`methodology`): reference dictation 30 s, free wait 3 s, wait weight 0.5, overall max wait 5 s, max language error 20 %, experimental 30 %, fast CPU max wait 4 s, accurate max wait 10 s.
- **Recommendation**: `recommend(languages)` returns Best / Fast / Accurate (`recommendations.js:158-226`). Best minimizes `error + 0.5·max(0, wait − 3 s)` among models under the 20 % floor and within 5 s on the accelerated tier, preferring models that can auto-detect among the chosen languages. Fast is the most accurate model within 4 s **on CPU**. The function is explicitly machine-independent (`recommendations.js:150-157`); Handy calls it without engine options (`catalog.ts:101`).
- **Machine signal**: one bit, GPU present and not forced to CPU (`useMachineTier.ts:7-18`), used only to display waits and order the "others" list (`catalog.ts:111-137`, `grading.ts:53`). It starts as `true` before the command resolves (`useMachineTier.ts:10`). Apple Silicon reports a Metal GPU and lands on "accelerated" (inferred; `gpu_device_options` not read).
- **Not present**: RAM size, chip generation, free disk space, Apple Silicon or Windows-laptop benchmarks, memory footprint per model, network speed. No code anywhere picks and downloads without a click: the backend auto-select only chooses among models **already on disk** and only after onboarding (`model.rs:1476-1504`).

**What the default would be today** (computed with the package's `recommend()`; sizes are Q8_0 file sizes from the catalog):

| Spoken languages | Best pick (Handy preselects this) | Download | Bench wait for 30 s audio (accel / CPU) |
|---|---|---|---|
| en | parakeet-unified-en-0.6b | 731 MB | 1.2 s / 2.2 s |
| en + de, en + es, fr, pt, es (OS language + English is the usual seed) | parakeet-ultra | 740 MB | 1.3 s / 2.3 s |
| de | canary-1b-v2 (+ 45 MB detector, it cannot detect language itself) | 1,189 MB | 2.1 s / 3.6 s |
| ja | Fun-ASR-MLT-Nano-2512 | 891 MB | 3.4 s / 5.5 s |
| zh | Qwen3-ASR-0.6B | 850 MB | 3.1 s / 5.7 s |
| ru | gigaam-v3-e2e-rnnt | 274 MB | 2.0 s / 3.8 s |
| ar, el, vi (worst cases) | cohere-transcribe models | 2,410 MB | not computed |

Across the package's 51 recommendable languages, 11 have a best pick over 1.3 GB (ar, az, cy, el, fa, he, mk, sr, ta, ur, vi).

**First-run download time for the likely default** (731 MB, English, or 740 MB, OS language + English), transfer only:

| Connection | 731-740 MB | 1.19 GB (de) | 2.41 GB (worst) |
|---|---|---|---|
| 25 Mbit/s | ~4 min | ~6.3 min | ~13 min |
| 50 Mbit/s | ~2 min | ~3.2 min | ~6.4 min |
| 100 Mbit/s | ~1 min | ~1.6 min | ~3.2 min |
| 300 Mbit/s | ~20 s | ~32 s | ~64 s |

Plus a sha256 pass ("Checking…", `DownloadStatus.tsx:82-85`) and the model load. Today this time is spent on the Demo screen, whose continue button is disabled until the model is ready (`Demo.tsx:517-524`).

**Gap to silent choice + background download:**
1. **Trigger without UI** (EXTEND, S): after Welcome (languages known) call `picksFor(langs)` role `pick` (`catalog.ts:97-109`) and `downloadPick` (`downloadPick.ts:10-33`); drop `model` from `STEP_ORDER` (`onboardingStore.ts:17-24`). Optionally start even earlier, at launch, from the OS-language seed (`onboardingStore.ts:64`) and re-pick if the user edits languages (cancel keeps partials: `model.rs:2580-2612`).
2. **Hardware guardrails** (EXTEND, M): when no GPU, or low RAM, prefer the package's `fast` role (it is defined on the CPU budget); refuse picks that do not fit RAM or disk. Needs RAM and free-disk probes (none today) and a per-model memory estimate (not in the catalog; file size is a usable proxy, inferred).
3. **Apple Silicon calibration** (EXTEND, M): benchmarks are from an AMD laptop; validate that "Best" stays under the 5 s budget on the slowest supported Mac (M1 8 GB). Unknown today.
4. **Do not block** (EXTEND, M): let Try it, the shortcut test, and the main app run while the model downloads; the pill should say "getting ready" if used early.
5. **Self-heal** (EXTEND, S): auto-resume after relaunch (`OnboardingFlow.tsx:58-67` currently flags failed), auto-retry with backoff, disk-space check (BUILD, S).
6. **Adapt after first use** (EXTEND, S): when the timed take is too slow (`useTryEngine.ts:194-196`), download the `fast` pick in the background and switch, instead of the link.
7. **Hide model names** in progress copy (TUNE, S) and move every model control to Advanced (TUNE, S).
8. **Language-aware fallback** in backend auto-select (`model.rs:1486-1504`) (EXTEND, S).

## Handy strengths to keep

- **Resume after relaunch**: the whole flow, including partially done lessons and a "Welcome back" note after the Accessibility relaunch (`onboardingStore.ts:26-46`, `:121-140`; `Permissions.tsx:113-117`, `:148-156`).
- **Permissions engine**: ordered rows, polling, focus-based re-check, no relaunch needed after Accessibility, Windows support (`usePermissions.ts` whole file); returning-user gate (`App.tsx:263-303`).
- **Real first dictation inside onboarding** with precise failure explanations (silent mic, no words, focus elsewhere) and inline mic switching (`useTryEngine.ts:155-224`; `TryIt.tsx:424-459`).
- **Animated demo engine** with reduced-motion stills and OS-specific drawing (`Demo.tsx:37-70`, `:152-303`; `DesktopBar.tsx`, `WindowBar.tsx`): the base for every guided tutorial.
- **Offline, honest model catalog** with per-language grades and a published method (`catalog/mod.rs:1-18`; `recommendations.js`), mirror fallback and hash verification (`catalog/mod.rs:32-36`, `:374-444`), resumable downloads (`download.rs:216-311`).
- **Spoken-language model**: multi-language preferences, detector fetched only when needed (`model.rs:1860-1890`, `:1918-1935`), weak-language honesty (`catalog.ts:69-75`), Chinese script handling.
- **Preview mode** for every onboarding screen without side effects (`App.tsx:376-394`; `OnboardingFlow.tsx:107-124`): useful for design iteration.
- **Inline shortcut capture** on three screens (`Demo.tsx:492-502`; `TryIt.tsx:360-367`; `TrayPanel.tsx:146-182`).

## Rough edges found in code

1. **Stale backend ranking**: `HANDY_RANKING` "recommended" flags are kept "until onboarding moves to the package's recommendations" (`catalog/mod.rs:38-53`), but onboarding already uses the package (`catalog.ts:97-109`). The frontend never reads `recommended`/`is_recommended` (rg found no use outside bindings, verified-in-code). The rank still decides backend auto-select (`model.rs:1096-1114`, `:1486-1491`), which ignores spoken languages.
2. **Two sources of truth for onboarding**: progress in webview `localStorage` (`onboardingStore.ts:60`, `:136`) vs `onboarding_completed` in the Tauri store (`settings.rs:434-435`, set by `commands.completeOnboarding()` at `OnboardingFlow.tsx:155`). Clearing webview data mid-flow restarts at Welcome (inferred). Upgrade migration marks onboarding done whenever a model was selected (`settings.rs:1204-1210`).
3. **Download gates the flow**: `Demo.tsx:517-524` disables "Try it yourself" until the model is on disk; an interrupted download after relaunch becomes "stopped" and needs a click (`OnboardingFlow.tsx:58-67`).
4. **Machine tier flicker and coarseness**: `useMachineTier` defaults to GPU = true before the async check (`useMachineTier.ts:10`) and treats any GPU as the Vulkan benchmark tier (`:17`).
5. **No disk-space or RAM check** before multi-GB downloads (no matching code).
6. **Fixed 720x600 onboarding frame** (`OnboardingShell.tsx:38`) limits the two-column, illustration-rich layout Wispr uses.
7. **Model concepts leak into the first run**: model step, grades, sizes and model names in download copy (`Model.tsx`; `onboarding.download.downloading`), and a "Speech model" row on All set (`TrayPanel.tsx:125-140`).
8. **Settings language change does not re-pick the model** (`PreferredLanguages.tsx:35-40`).
9. **Replay of onboarding only in Debug** (`App.tsx:316-320`).
10. **Retired-model update is a blocking screen** with user choices (`App.tsx:246-253`; `ModelMigration.tsx`), heavier than needed for a "never think about models" product.
11. **Stale copy**: `onboarding.models.small/medium/turbo/large/breeze-asr` (Whisper names) remain in `en/translation.json` though the catalog drives names now (inferred unused; not grepped exhaustively).
12. **App language control sits in About** (`AboutSettings.tsx:43`) rather than General; i18n starts in English and switches asynchronously (`i18n/index.ts:92`, `:104-128`).
13. **Default macOS shortcut is Option+Space** (`settings.rs:945-946`), so the Wispr-style "on-screen fn key" test does not map directly.

## Platform concerns

- **macOS**: Accessibility is required for typing and global shortcuts; the relaunch hint exists because grants do not always take effect in-process (`Permissions.tsx:201-212`). fn/Globe as hotkey needs low-level capture (unknown support in `handy_keys`). Screen/System Audio permission (Notetaker) would force an app restart, which Handy's resume logic already handles.
- **Windows**: only microphone privacy is checked (`usePermissions.ts:72-75`); illustrations of permission screens need a Windows set. Benchmarks' Vulkan tier is closest to Windows/Linux laptops.
- **Linux**: no permission step; Try it shows a setup guide link (`TryIt.tsx:493-504`); overlay hidden by default (`settings.rs:618-625`), which affects any pill-based coaching (B-32).
- **Translations**: 26 UI locales (`i18n/languages.ts`); every new onboarding/tutorial string multiplies localization work.

## Open questions

1. Should the model download start at first launch (from OS languages) before the user confirms languages, accepting a possible re-download if they change them?
2. What minimum Mac (chip, RAM) do we support, and do we need our own Apple Silicon speed numbers before trusting the package's "Best" as a silent default?
3. Do we want any model UI outside Advanced (for example a "Speech quality: Faster / Balanced / Most accurate" control mapping to the package's three roles) instead of a raw model list?
4. Default hotkey: keep Option+Space (works today) or invest in fn/Globe to match the Wispr onboarding test?
5. Tutorials: animated scenes (light, translatable) or bundled videos (closer to Wispr, heavier, hard to localize)?
6. Is the personalization quiz with mic recommendations worth building for a free product, or is a short "use a wired or built-in mic" tip enough?
7. Do retired-model migrations matter for this fork's users at all (new installs only), or can that UI be dropped?
8. Unknown in this pass: whether the onboarding download survives closing the window (tray mode), whether `handy_keys` can bind fn alone, and the exact backend retry policy in `download.rs` beyond the stall watchdog.
