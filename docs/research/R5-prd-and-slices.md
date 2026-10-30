# R5: Product requirements and build plan (Wispr Bro)

**Status:** revised after independent review (`r5-review.md`, 48 issues; every issue is resolved or escalated in §14). Final plan for M0, M0.5 and M1; coarse plan for M2. Inputs: R1 (Wispr Flow teardown), R2 (gap analysis, 492 rows), R3 (landscape), R4 (parts bin) and the R4 GPL addendum, all in `docs/research/`.

**How to use this file.** A coding agent picks the next slice whose dependencies are done, reads that slice block, the architecture rules in §4 and the non-functional budgets in §5, and builds only that slice. Row ids (A-xx … E-xx), AF-xx, DEC-xx and RC-xx are R2 ids. RT-xx are R4 runtime checks. W-xx are R4 "write ourselves" items. Slice ids: **F-xx** = M0 foundations, **RS-xx** = M0.5 real-Mac spikes, **S-xx** = M1 slices, **N-xx** = M2 Notetaker slices. A letter suffix (S-07a, S-07b, S-07c) marks a slice split from an oversized one. ⚠ marks an upstream-merge hotspot from AF-13. All owner decisions are made (DEC-26, DEC-29 and DEC-44 were settled after review).

**Path conventions.** Rust paths are relative to `src-tauri/src/`. Frontend paths start with `src/`. Config paths start with `src-tauri/` or `.github/`.

**Line numbers** in this plan refer to Handy as forked (version 0.9.8) and Tauri 2.11.5 / tauri-specta 2.0.0-rc.21 / handy-keys 0.3.4 as locked in `Cargo.lock`. The upstream-sync checklist (§4.13) re-checks them.

---

## 1. Product summary and principles

**What it is.** Wispr Bro is a free, open-source (GPL-3.0) macOS dictation app. Hold fn, speak, release: clean text appears in the focused field of any app. It matches Wispr Flow's consumer experience, but everything runs on the Mac. There is no account, no cloud, no telemetry and no usage cap. It updates itself. The user never picks a model. Windows and Linux come later.

**Who it is for.** People who write a lot on a Mac (email, chat, documents, AI prompts, code comments) and want Wispr Flow's speed and polish without sending their voice to a server or paying a subscription. Secondary: privacy-bound users (health, legal, finance) and people on slow or no internet.

**The experience bar: "feels like Wispr Flow".**
1. One key. Hold fn to talk, double-tap fn for hands-free, Esc to cancel. Nothing to choose at the moment of use.
2. A calm resting bar at the bottom of the screen confirms state (listening, working, done) and the active mic.
3. Text lands where the cursor is, formatted, in about a second. If it cannot land, it is never lost: a card offers Copy, and History keeps it.
4. Personalization is visual and example-led (before/after cards, banners with example chips), never prompt-driven.
5. Every feature teaches itself with a short guided scene and a practice step.
6. The first dictation works within a minute of granting permissions (≤ 2 min from install, §5.3). No model picker, no settings checklist.

**Privacy promise (exact).** The app makes network requests to exactly two places, and only for these purposes:

| # | Host | Purpose | When |
|---|---|---|---|
| 1 | `github.com` and its release-asset redirect hosts (`objects.githubusercontent.com`, `release-assets.githubusercontent.com`) | Update check (`https://github.com/<owner>/wispr-bro/releases/latest/download/latest.json`, beta: `…/releases/download/beta/latest.json`) and update download | At launch and every 6 h while update checks are on (default on, disclosed, can be turned off) |
| 2 | `huggingface.co` and its download CDNs (`*.hf.co`, `*.huggingface.co`) | Model downloads from pinned revisions: `handy-computer/*` (speech, VAD, language detector) and our own HF org `<hf-org>/*` (LLM GGUFs and any other asset we add, mirrored with LICENSE files) | First run, language changes, silent model upgrades |

`<owner>` and `<hf-org>` are placeholders filled from the F-02 owner inputs; the release workflow fails while any placeholder remains.

Nothing else: no telemetry, no crash upload, no fonts or scripts from CDNs, no AppleScript, no cloud LLM. Links the user clicks open in the browser and are not app traffic. Apple's own OS traffic (Gatekeeper, notarization) is outside the app. After setup the app works fully offline. CI enforces this list with source and bundle scans plus a runtime egress test on the release build (§4.12).

**Reuse-first rule.** Before writing code, use the part named in R4 or the GPL addendum for that capability (adopt a crate, vendor files, or port code with its notice). Write our own code only for W-xx items. Never copy Wispr Flow code, prompts, sounds, icons or images; copy behaviour only.

**Own-words rule (UI copy).** Behaviour, flow and screen structure may follow Wispr Flow. All UI copy is written by us in `en/fork.json`; no Wispr Flow sentence is reused. Quoted UI strings in this plan are our working copy, not Wispr's text. Generic functional labels (Copy, Undo, Done, Cancel, Retry, Settings, key names, page names such as Dictionary or Snippets) are fine. The plan says "Flow Bar" or "pill" for the feature; the UI calls it "the bar". S-37 runs a copy check: any English fork string that matches an R1 quote over four words long is rewritten.

**Thin-layer rule.** Our code lives in one namespace (§4.1). Upstream Handy files get at most one-line hooks into that namespace, each listed in §4.2 and enforced by the hook-size guard (§4.12). Never append to Handy's `MIGRATIONS`, `AppSettings`, `collect_commands!` list or `translation.json` files. Ported GPL or AGPL code never goes into a Handy file; it lives in fork files that the hooks call. Upstream-bound fixes (code we would send to Handy) live in separate, MIT-clean commits with no GPL-derived code.

**Build-once rule.** Each slice ships at production quality: tests, translations (machine translation marked for review is allowed until S-37), accessibility labels, dark mode. No throwaway UI. A panel that lands before its final home is mounted on the fork debug panel (F-05) and moves unchanged to its home page later.

**GPL obligations in brief.** The combined app is GPL-3.0. Our own files carry `SPDX-License-Identifier: GPL-3.0-or-later`. Ported GPL files keep their original copyright line plus a "Modified by Wispr Bro contributors, <date>" notice. Ported AGPL code (Voquill, Tambourine, VoiceTypr, OpenLess, abstand) lives in files marked `AGPL-3.0-only` and is listed in NOTICE. Keep Handy's MIT notice in `LICENSES/MIT-Handy.txt`. Every release publishes the matching tagged source and build scripts (including the `llama-server` CI build). Strip any licence-key or telemetry code we port. Contributions use DCO sign-off. Model weights and fonts are separate works tracked in the licence register (R4 §5). The Mac App Store is out.

---

## 2. Scope

### 2.1 M1: the complete dictation product (in scope)

Every dictation-side feature of R1 that has a local equivalent:

| Area | In M1 |
|---|---|
| Flow Bar (pill) | Resting bar on the cursor's display (bottom by default; top kept as an Advanced option); hover tooltip; click to start/stop; held, hands-free, processing, message, error and "no text field" card states; mic label; right-click "Hide for an hour"; drag along the bottom edge; Scratchpad slot with tooltips |
| Hotkeys | fn hold (push to talk), double-tap fn or fn+Space (hands-free), Esc cancel, Ctrl+Opt fallback, paste last ⌃⌘V, copy last ⌃⌘C, Press Enter (unset), Command Mode fn+Ctrl, Transforms ⌥1/⌥2, view changes ⌥O, Open Scratchpad (unset); one hotkeys panel; self-healing |
| Insertion | Clipboard paste with full restore, clipboard-manager concealment, paste verification, secure-field refusal, fail-open card when no text field is focused |
| Models and first run | Silent model choice; tiny starter model first; full model in background; takes recorded before any model is ready are kept and delivered to History with one notification (the clipboard is written only on the user's Copy); silent upgrades |
| Text intelligence | Newline-safe pipeline; dictionary words and from→to rules; voice commands (new line, new paragraph, scratch that); cleanup levels None/Light/Medium (default Light, skipped in AI-chat apps); per-app Styles; context awareness; Transforms (Polish, Prompt Engineer, custom) with diff view; Command Mode; snippets (plain text, DEC-12); optional ITN |
| Hub | Sidebar (Dictation, Insights, Dictionary, Snippets, Style, Transforms, Scratchpad, Share with a friend, Settings, Help) with a setup checklist card; Dictation home with stats rail and history; Insights; notifications inbox |
| Settings | Two-column modal (General, System, Data and Privacy, Advanced, About) |
| Onboarding and tutorials | Permissions with pictures, mic test, shortcut test, hands-free practice, first-dictation celebration, typing test and time saved, first-use chooser, pill spotlight, setup checklist, Transforms tutorial, whisper tutorial with mic quiz |
| Platform | App menus and tray, auto-updates that never interrupt, Help and Feedback (GitHub), Share with a friend, Data and Privacy page, export, delete all, local crash reports, 26 locales (web and Rust-side strings), VoiceOver basics |
| Late M1 | Dictionary auto-learn (opt-in), Wispr Flow / Handy vocabulary importer (opt-in), vibe coding |

### 2.2 M2: Notetaker (starts after the M1 exit criteria pass)

Local meeting notes: system-audio capture (macOS 14.4+), live transcript with You/Them, post-meeting re-pass and diarization, summary and question answering through the local LLM, Notetaker page and settings, call detection, local calendar via EventKit, guided tour, Notetaker slot on the Flow Bar. See §10.

### 2.3 Out of scope (with the local replacement)

| Wispr feature | Why out | Local replacement |
|---|---|---|
| Account, sign-in, profile menu, Manage account | No account | Greeting name from macOS (S-15a); no sign-out data loss |
| Teams, shared dictionary/snippets, Invite your team | Team feature | Local tabs All / Added by you / Suggested (S-16) |
| Plans, billing, Pro, referral "free month" | Free product | "Share with a friend" page: copy link, share sheet, no tracking (S-26b) |
| Cloud sync, cloud dictation storage, "Improve the model for everyone", public profile | Cloud | Data and Privacy page states the facts (S-27) |
| MCP for meeting notes | Cloud/account; network API would trigger AGPL §13 | Later: notes as local Markdown files (not in M1/M2) |
| Slack, Google/Outlook calendar connectors | Cloud OAuth | macOS Calendar via EventKit (N-11) |
| Perplexity in Command Mode | Cloud web search | Command Mode writes and edits text only (S-23) |
| Hosted help center, Contact sales, "Email results" | Hosted / sales | Help links to the repo docs and GitHub; Copy results (S-26a, S-26b, S-32) |
| Notetaker sharing links, calendar consent notice, Granola/Otter account import | Cloud | Export Markdown; copy a consent message; local audio import (N-08) |
| Mobile apps, desktop vs mobile word totals | Single device | Desktop totals only; Windows/Linux in a later milestone |
| Onboarding PERSONALIZE stage (referral screen) | Free product | None; our stepper has four stages (S-29a) |

---

## 3. Decisions

"Owner" = decided by the owner, not to be reopened. "Default" = set for this plan; the owner may override. No decision is pending (DEC-53 was settled by RS-01 round 2 on 2026-10-30). New decisions not listed in R2 §4 are numbered DEC-26 onward.

| ID | Decision | Status | Reason | R2 rows |
|---|---|---|---|---|
| DEC-01 | First run: download the whisper-base starter (~85 MB, Apache-2.0, multilingual) first; mic test and first dictation use it; the full model downloads in the background and replaces it silently after it has loaded once. A press before any model is usable still records: audio is saved and transcribed as soon as a model is ready. A result that arrives in the same session shows on the pill card with Copy. Results that arrive later (offline first launch, relaunch) go to History, and one notification says "N dictations ready" with **Copy latest** and **Open**. The app never writes the clipboard for a queued take on its own; only a click on Copy or ⌃⌘V (paste latest) does. | Owner (+ queue default) | First dictation in seconds; a press is never refused; the user's clipboard is never overwritten behind their back (§5.4) | B-50, B-202, B-203, AF-03 |
| DEC-02 | Silent model policy: OS languages + English; probe chip, RAM and free disk (sysinfo); choose the most accurate model that meets the latency budget (§5.1) on this machine class; never the slow whisper-large default for a language where it misses budget; Apple Silicon first; Intel per DEC-29. Start the full download at Welcome from OS languages; re-pick if the user changes languages. The English pick depends on DEC-26. | Default | Fewest questions, earliest start; re-download is rare | B-47, B-49, B-208, B-214, AF-20 |
| DEC-03 | Local LLM: bundled, signed `llama-server` sidecar + catalog-downloaded GGUF by RAM tier (≤8 GB Qwen3.5-2B Q4_K_M; ≥16 GB Qwen3-4B-Instruct-2507 Q4_K_M; Intel per DEC-29) + Apple Foundation Models as an optional fast path on macOS 26. Default cleanup level Light; skip LLM formatting in AI-chat apps. Tiers may change after RS-06/RS-07. **Update 2026-10-29 (RS-07 rescore, owner decision):** Muesli 2B v12 (`Muesli-HQ/qwen35-2b-muesli-cleanup-v12-gguf`, Apache-2.0) with the OpenWhispr prompt and JSON schema is the cleanup model on every Apple Silicon tier, pending RS-06 latency and memory on M1 8 GB. The owner confirmed its training data comes from valid sources, which closes R4 checklist item 6 for Muesli. Qwen3.5-2B / Qwen3-4B-2507 remain the fallback. | Default | Works on every Mac; Apple FM is not a baseline; Light is Wispr's documented default | D-67, D-68, D-212, D-213 |
| DEC-04 | Cloud LLM providers removed from defaults and refused by the backend (non-loopback endpoints rejected). The custom OpenAI-compatible endpoint stays in Advanced for loopback only (Ollama). | Default | Clean privacy story; UI-only hiding is not enough | D-69, D-70, E-72 |
| DEC-05 | Notetaker is M2, started only after M1 exit criteria pass. | Owner | Sequential milestones | E1 rows, B-06 |
| DEC-06 | Engine design for dictation during a meeting: decided in N-01 after a benchmark; both options planned. | Default | Needs measured memory and stall data | E-47, AF-04 |
| DEC-07 | Retention: text forever (user can change), audio 14 days; stats live in tables that survive retention and deletion. | Default | Matches Wispr's 14-day audio; disk ≈ 1.92 MB per speech-minute | C-46, AF-07 |
| DEC-08 | Fork data in a separate `fork.db` (WAL, FTS5 trigram, daily stats) and a separate fork settings store. | Default | No upstream migration conflicts | AF-07, AF-13, E-70, E-71 |
| DEC-09 | Default hotkey fn: hold = push to talk; double-tap fn or fn+Space = hands-free; Esc = cancel; Wispr's 0.5 s tap rules; accidental-start guard; fallback Ctrl+Opt / Ctrl+Opt+Space for keyboards without fn. Globe action handling per RS-01 and DEC-44. | Owner | Wispr parity | A-22, A-23, A-24, A-35, A-202 |
| DEC-10 | Settings as a two-column modal like Wispr. | Default | R1 parity; deep links handled by a nav store | C-12, C-204 |
| DEC-11 | WPM on speech time (VAD-trimmed `audio_ms`); CJK/Kana one word per character, Thai/Lao/Khmer/Myanmar characters ÷ 4; file transcriptions, practice takes and failed takes excluded from stats. | Default | Wispr's documented rule | C-30, C-31, C-212, B-29 |
| DEC-12 | Snippets are plain text with newlines in M1. This is a declared parity gap (R1 shows a rich editor; §9 lists it). R4 found no licence-audited rich editor (TipTap/Lexical "only if rich snippets"). Build-once safeguard: the `snippets` table has a nullable `body_html` column and both paste paths already write every pasteboard flavour (S-04), so rich text later adds only an editor, with no migration or paste rework. | Default | No vetted editor part; no rework later | C-70 |
| DEC-13 | One prompt engine with two entry points: cleanup levels and the Polish transform. "Apply after every dictation" runs the chosen transform through the same engine after cleanup. | Default | No duplicate prompts or guards | D-04, D-42 |
| DEC-14 | First-run defaults: sounds on, Flow Bar always visible (after setup), Dock icon shown, launch at login on, theme follows System (options Light, Dark, System), file log level Info with no settings dump, update checks on and disclosed. | Default | Matches the used Wispr account; honest logs; R1 does not show the full theme list | A-42, A-66, A-65, E-78, E-75, C-84 |
| DEC-15 | Tutorials are animated in-app scenes on Handy's demo engine, not bundled videos; include the typing test and the mic recommendation quiz. | Default | Light, translatable, offline | B-39, B-42, B-43, B-29 |
| DEC-16 | Network: only the two hosts in §1; drop the upstream `blob.handy.computer` mirror; CI guards reject new hosts and telemetry SDKs (§4.12). | Default | Verifiable promise | E-74, E-75, AF-12 |
| DEC-17 | Name Wispr Bro; bundle id `com.wisprbro.desktop`; own version line, 1.0.0 = first public release (internal builds `1.0.0-alpha.N`/`-beta.N`); Apple team = the owner's Developer ID team; identity fixed before the first build that leaves the team. | Owner | Changing identity later resets TCC | AF-11, E-61 |
| DEC-18 | Flag = "wrong transcript": opens a dialog that offers to add the corrected word or a from→to rule to the dictionary. | Default | Feeds the visible correction loop | C-40 |
| DEC-19 | Undo AI edit restores the raw text in History and copy; in the target app the user's ⌘Z applies; no re-paste. | Default | Wispr's documented behaviour | C-42 |
| DEC-20 | Dictionary auto-learn: opt-in, suggestions need user approval, rejected words never return. | Default | Privacy-sensitive AX read-back | D-52 |
| DEC-21 | Notifications: in-app inbox + macOS notifications for model ready, update ready, queued takes ready, errors needing action. | Default | Few, useful system alerts | C-17, E-91 |
| DEC-22 | Minimum macOS 13.0 with runtime gates: system audio 14.4+, EventKit full access 14.0+, Apple FM 26+. | Default | Matches Homebrew `:ventura`; sidecar built for 13.0 | E-92 |
| DEC-23 | Web-app detection by AX URL (no AppleScript), window-title fallback; store the host only. | Default | No Automation prompt | D-22 |
| DEC-24 | Context awareness on by default, local only, never stored, fail-open, visible indicator and an off switch. | Default | Wispr default; local makes it safe | D-58 |
| DEC-25 | Flow Bar follows the display under the cursor. | Default | Wispr's documented behaviour | A-212 |
| DEC-26 | CC-BY-4.0 speech weights (Parakeet TDT, parakeet-110m, Canary) are allowed as silent defaults, with an Acknowledgements entry in About and NOTICE attribution. RS-09 still benchmarks them against the Apache/MIT alternatives to confirm the pick. | Owner (default accepted; owner may override) | Best English accuracy and speed in Handy's catalog; attribution is the only obligation | B-204, E-218 |
| DEC-27 | Never default: NVIDIA Open Model License and `other`-licensed models; the 12 non-English Moonshine models (non-commercial) are hidden from our catalog view. | Default | Licence register R4 §5 | B-204 |
| DEC-28 | Private API `_setPreventsActivation:` is allowed behind a feature check, only if RS-03 shows it is needed. Any private API that changes a user's system preference falls under DEC-44. | Default | Outside the App Store; reversible; no user preference touched | A-01 |
| DEC-29 | Apple Silicon only for M1 and M2: no x86_64 build; the Homebrew cask uses `depends_on arch: :arm64`; Intel columns in §5 and Intel runs in RS-06/RS-09/RS-10 and §9 are skipped. Intel support (CPU-only ASR, LLM cleanup off by default) is a later milestone; items marked "(DEC-29)" describe that later work and are not built now. | Owner | Start on Apple Silicon, add Intel later | B-214 |
| DEC-30 | Pill shows a waveform capsule by default (Wispr); Handy's live transcript panel stays as an Advanced option. | Default | Parity; keeps a Handy strength | A-18, A-05 |
| DEC-31 | Transforms and Command Mode are opt-in (Transforms page toggle); their hotkeys register only when on. With Command Mode off, Ctrl pressed while fn is held passes through and the take stays plain dictation. | Default | Wispr parity; avoids swallowing ⌥ characters; small models need expectation-setting | D-28, D-44, A-29 |
| DEC-32 | No word or time cap. Hands-free auto-stops after 120 s of continuous silence (pill warns at 100 s); hard stop at 30 min with a warning at 28 min; audio is still transcribed. An auto-stopped take pastes only if the field captured at start still has focus; otherwise it goes to the card. | Default | Wispr has no silence stop (R3: warning at 19 min, stop at 20 min); 2 min does not cut off a user who pauses to think; bounded RAM | A-23 |
| DEC-33 | Update channel stable by default; beta channel opt-in in Advanced. The channel endpoint is chosen in Rust (`fork/update/channel.rs`, F-02); the rolling `beta` release is a GitHub prerelease so `releases/latest` never points at it. | Default | Safe testers path | E-53 |
| DEC-34 | Every model or asset we add is mirrored to our HF org with its LICENSE and pinned by revision + sha256; Handy's ASR repos are used as is, pinned. | Default | One host, our own pins | E-74, D-71 |
| DEC-35 | Fork strings live in a separate i18next namespace `fork` (`src/i18n/locales/<lng>/fork.json`); Handy brand strings are overridden at load, never edited. Rust-side strings (tray, app menu, macOS notifications, VoiceOver announcements) live in the same files under `rust.*` and are embedded by `fork/i18n.rs`. | Default | Zero conflicts in 26 upstream files | E-62, E-80, AF-13 |
| DEC-36 | MPL-2.0 dev-only tools (axe-core via jsx-a11y and @axe-core/playwright) are allowed; they never ship. | Default | MPL is GPL-compatible anyway | E-81 |
| DEC-37 | Wispr Flow and Handy vocabulary importer: late M1, opt-in, reads a copy of the user's own local data. | Owner | Migration for target users | C-64, D-55 |
| DEC-38 | The resting bar stays hidden until setup ends, then appears. Active states (held, locked, processing, card) do show during onboarding practice. No Dictation/Notetaker choice in M1 onboarding. | Default | Wispr's documented rule; practice needs the active pill | A-01, B-06 |
| DEC-39 | Models live under our app data (`HF_HOME` set before managers start), so disk checks, delete-all and uninstall cover them. | Default | AF-19 | B-205, E-64 |
| DEC-40 | ITN (spoken numbers to digits) ships only if RS-13 shows the default ASR models write spoken-form numbers. | Default | Avoid a stage with no gain | D-14 |
| DEC-41 | Vibe coding ships in late M1, off until the user runs its setup. | Default | Needs IDE screen-reader mode | D-59, D-60 |
| DEC-42 | The free-text prompt editor moves to Advanced; consumers see levels, styles and transform rules only. | Default | R1 pattern 6 | D-05, D-34 |
| DEC-43 | Webview CSP exactly as in §4.11: IPC, the asset protocol for audio playback, `data:`/`blob:` images for "Save as image", no remote sources. | Default | Second wall behind the network guard, without breaking features | AF-12 |
| DEC-44 | Where the Globe key's system action cannot be suppressed: keep fn as the default; during onboarding ask for consent, then set the user's Globe setting to "Do Nothing" through the private `TISUpdateFnUsageType` API (OpenWhispr port) with a crash-safe restore marker; restore the original setting on quit-with-fn-disabled, on turning the feature off, and on uninstall (Settings > Data and Privacy > "Restore system keyboard settings"). If the user declines, show the guidance sheet with a deep link to set it manually. RS-01 records which macOS versions need it. **Update 2026-10-30 (RS-01 round 1, macOS 26.6.2, M5 Max, built-in keyboard):** confirmed needed. Blocking the fn-down event in the fork's tap does not stop the Globe action: with "Show Emoji & Symbols" (the macOS default, key unset) the emoji picker opened on quick taps and on every short fn press, and with "Change Input Source" the input source switched on quick taps and double-taps. A 1 s hold never triggered anything, and "Do Nothing" and "Start Dictation" showed nothing. So onboarding applies the DEC-44 path when `AppleFnUsageType` is 1 (Change Input Source) or 2 (Show Emoji & Symbols), or is unset (default Emoji); it leaves 0 (Do Nothing) and 3 (Start Dictation) alone. macOS 14 and 15 are still untested. One read of the value disagreed with System Settings (value 2 while the owner saw Do Nothing), so the check re-reads before acting. Details: `docs/fork/spikes/RS-01-fn-globe.md`. **Update 2026-10-30 (RS-01 round 2):** with "Do Nothing" and the bare fn press *not* blocked, nothing popped up on quick taps, double-taps, holds or chords, and fn+F11 (show desktop) and fn+←/→ kept their function. Since blocking the fn press did not stop the Globe action in round 1 either, the fork tap leaves `{FN}` out of the blocking set when the value is 0 (Do Nothing), which is the state DEC-44 sets; it keeps blocking it only for 3 (Start Dictation), the one value tested blocked only. fn+Space and Esc stay blocked as in §4.8. | Owner | Keeps the owner's fn default without a manual step for most users | A-35, A-01 |
| DEC-45 | UI copy is ours (own-words rule, §1). | Owner | Copying Wispr's text plus the "Wispr" name raises passing-off risk | all UI rows |
| DEC-46 | Fork command routing: fork commands are app commands named `fork_*` with their own tauri-specta builder, merged into Handy's handler by one `lib.rs` line (§4.1). No `fork` plugin commands, no `build.rs` ACL manifest. | Default | Passes Tauri 2.11.5's ACL without permissions; no `build.rs` edit; bindings stay generated | E-95 |
| DEC-47 | The mic opens only after `T_show` (150 ms) of uninterrupted fn hold, or at once on a second press (double-tap) or Space (fn+Space). If Handy's mic stream is still warm (it stays open 30 s after a take, `managers/audio.rs:20`), capture starts at fn-down from that stream and is discarded on interrupt. | Default | No orange mic indicator on fn chords or quick taps (A-45) | A-22, A-45 |
| DEC-48 | Version: `src-tauri/Cargo.toml` `version` (and `package.json` `version`) carry our version line as a listed one-line hotspot, re-applied at each sync, so `CARGO_PKG_VERSION`, the tray label and What's New report our version. The identity guard checks that Cargo, the overlay and the git tag agree. | Default | Handy reads `CARGO_PKG_VERSION` in `tray.rs:461` and `settings.rs:605`; one line is cheaper than hooking every consumer | E-213 |
| DEC-49 | LLM transport: Unix socket in the app data dir if RS-06 shows the pinned `llama-server` serves `/health` on `--host <path>.sock`; otherwise `127.0.0.1` on a random free port (retry on bind race) with a random API key. Loopback only, so AGPL §13 does not apply and nothing is reachable from the network. | Default | reqwest 0.12.28 supports `unix_socket` (client side verified); the server side is unverified | D-212 |
| DEC-50 | Defaults that differ from R1 on purpose: typing-time slider default 2 h a day (R1: 6 h), because the time-saved number must be a plausible estimate, not a sales figure. Voice Profile unlocks at 1,700 words (same as R1's ~1.7K). | Default | Honest numbers; parity elsewhere | B-28, C-33, C-57 |
| DEC-51 | Long takes feel instant. While the user is still speaking, the app transcribes finished stretches at natural pauses (VAD boundaries) and cleans them up paragraph by paragraph, so key-up waits only for the last stretch. Target: a 5-minute take inserts within the Light budget of §5.1 (≤ 50 words) plus 1 s. Seams must not lose or double words; one final pass may re-join them. RS-09 measures chunked vs whole-take WER and latency on 1-, 3- and 5-minute takes in en, nl and fr. | Owner (2026-10-29) | Owner observation that long Wispr Flow dictations still appear immediately (mechanism inferred, not observed) | RS-09, S-07a, S-12a |
| DEC-52 | Starter per language (RS-09 provisional, M5 Max). **en:** parakeet-110m as starter (6.5 % WER, 0.55 GB), then parakeet-unified-en (streams). **fr:** whisper-base as a stopgap starter, then whisper-large-v3-turbo. **nl:** no starter, because nothing ≤ 150 MB is usable (whisper-base 64 %, whisper-small 30 % WER on short takes). Download Parakeet TDT 0.6B v3 (~740 MB, 17 % WER, fast) first, during onboarding. Until it is ready, takes are queued (card), never transcribed with a weak model. Then upgrade to turbo on Macs that meet §5.1 with it. §5.3 gets a Dutch exception: "ready" within the time the 740 MB download takes, with onboarding steps running meanwhile. whisper-small is used only as the without-CC-BY mid option. Chunking per DEC-51 applies to Whisper; re-join seams in a final pass (self-corrections split across a seam caused the worst errors). | Default (owner delegated) | A first Dutch take with 30-64 % errors is worse than waiting a minute | RS-09, S-07a, S-07b, S-29a |
| DEC-53 | First-word capture: a **prepared** mic, with `T_show` kept at 150 ms (DEC-47). While idle, the CoreAudio input unit is built and initialised (`AudioUnitInitialize`) but not started; each take only starts and stops it. This needs an open/start split in handy-recorder (its `Recorder::open` starts the stream) and a re-prepare on device change. **RS-01 round 2 (2026-10-30, macOS 26.6.2, M5 Max, 10 cued takes per variant at `T_show` 100 ms):** prepared fn → first audio p50 129 ms / p95 140 ms; cold open p50 211 ms / p95 216 ms; 0 takes clipped by us in either; all 20 transcribed with "testing". The owner saw no orange indicator while the unit was only prepared, and saw it during each take and gone after. So `T_show` does not need to drop to 100 ms: prepared at 150 ms gives ≈ 180 ms, inside §5.1, and opens the mic on fewer taps (the owner's natural quick taps lasted 72-164 ms; at 100 ms, 5 of 9 opened the mic). Fallback if preparing fails on a device: cold open (≈ 260 ms at 150 ms). The pre-roll from before fn-down stays dropped (it keeps the indicator on). Still unmeasured: the 50-take clipping gate, macOS 14/15, Bluetooth and USB mics. | Default (owner delegated) | Fastest first audio without lighting the mic indicator while idle | RS-01, S-01, A-45 |
| DEC-54 | A non-modifier key pressed while fn is held in a push-to-talk press makes the press a chord **at any time**, not only within `T`: the take is dropped, nothing is inserted, the key passes through. The transcript is kept in History (not pasted) when the take had run ≥ `T`, so nothing spoken is lost. Keys while locked still pass through. Why: in both RS-01 rounds the owner held fn 1.1-1.7 s before the first chord key (fn+arrows, fn+F11, fn+click), and the old rule (keys after `T` pass through, take continues) would have pasted a take at the cursor the user was just moving. Cost: a chord pressed after `T_show` lights the orange indicator briefly (the mic had opened); chords within `T_show` never do. | Default (owner delegated) | Navigating with fn never pastes text | RS-01, S-01, A-45 |

---

## 4. Architecture of the fork

### 4.1 One namespace

| Layer | Namespace / location | Rules |
|---|---|---|
| Rust | `src-tauri/src/fork/` (module `crate::fork`), with an in-app Tauri plugin named `fork` (`fork::init()`) that owns fork state, the init hooks and `on_event`; the plugin registers no commands | Every fork Rust file starts with an SPDX header. Full submodule list (no slice may add a top-level module that is not here): `shortcut/`, `pill/`, `audio/`, `clipboard/`, `ax/`, `context/`, `insert/`, `models/`, `llm/`, `text/`, `db/`, `dictionary/`, `commands/`, `onboarding/`, `recovery/`, `stats/`, `transforms/`, `scratchpad/`, `import/`, `diagnostics/` (`mod.rs`, `sanitizer.rs` marked AGPL-3.0-only, `crash.rs`, `bundle.rs`), `update/`, `vendor/` (axwatch), `hub.rs` (S-13: main-window size and title bar), `settings.rs`, `i18n.rs`, `events.rs`, `notify.rs`, `app_menu.rs`, `tray.rs`, `export.rs`, `erase.rs`, `net.rs`, `testutil.rs` (`cfg(test)`), later `meeting/`, `calendar.rs`. Swift for Apple FM lives in its own crate `src-tauri/crates/wb-apple-fm/` (own `build.rs`, `fork_apple_fm.swift`) |
| Init order | Verified in tauri 2.11.5: plugins initialize inside `Builder::build()` (`app.rs:2440`); the app's `.setup()` closure runs later, on the first `RunEvent::Ready` (`app.rs:1423-1428`, `2521-2533`); plugin `on_event(RunEvent::Ready)` is dispatched right after it. Handy creates its managers synchronously in that closure (`lib.rs:1049-1051`, `initialize_core_logic`) | Plugin `setup` only registers fork state: `ForkState`, the fork settings store, the `fork.db` handle, the fork event registry. Everything that touches Handy managers, windows, menus or the tray runs in `fork::on_ready`, called from the plugin's `on_event(RunEvent::Ready)`. Fork commands read Handy state with `try_state` and return a typed `NotReady` error, never panic. In headless mode (`CliArgs` has `--transcribe-file`, `--list-devices` or `--list-models`) `on_ready` does nothing |
| Commands | App commands named `fork_*`, collected by a second tauri-specta `Builder` (no `plugin_name`) that exports `src/fork/bindings.ts` in debug builds. One `lib.rs` line merges the handlers: `.invoke_handler(fork::compose_invoke(invoke_handler))`. `compose_invoke` routes a command whose name starts with `fork_` to the fork handler and every other command to Handy's (`Invoke::message.command()` is public; a handler consumes the `Invoke`, so routing is by name, before the call) | Tauri checks the ACL only for `plugin:` commands, for remote origins, or when the app defines an app ACL manifest (`webview/mod.rs:1819-1827`). Handy's `build.rs` ends in plain `tauri_build::build()`, so app commands from local pages need no permission. CI asserts both conditions (§4.12). Rejected: a `fork` plugin with commands (an in-app plugin has no permission manifest, so `fork:default` does not exist; it would need an `InlinedPlugin` manifest in `build.rs` and tauri-specta's `plugin:fork|` prefix) and a `fork::commands!()` entry inside `collect_commands!` (a macro call cannot be spliced into another macro's list) |
| Events | tauri-specta rc.21 derives an event's name from its type name in kebab-case and has no per-event override (`tauri-specta-macros` `lib.rs:23`) | Every fork event type is named `Fork…` (`ForkPillState` → `fork-pill-state`). The fork builder's `mount_events` merges into the same `EventRegistry` Handy uses |
| React | `src/fork/` (`hub/`, `home/`, `history/`, `insights/`, `dictionary/`, `snippets/`, `style/`, `transforms/`, `scratchpad/`, `settings/`, `onboarding/`, `tutorials/`, `pill/`, `share/`, `whats-new/`, `debug/`, `ui/`, `styles/`, `bindings.ts`, `vite-inputs.ts`) | Reuse Handy components by import; never edit them for fork features |
| i18n | Namespace `fork` in `src/i18n/locales/<lng>/fork.json` (26 locales); brand overrides for Handy keys under `fork.json > overrides` applied with `addResourceBundle(lng, "translation", overrides, true, true)`; Rust strings under `rust.*`, embedded by `fork/i18n.rs` with `include_str!` for all 26 files, locale taken from Handy's app-language setting (then the OS locale, then English), tray and menus rebuilt on language change | English is the source; parity checked in CI |
| Windows and capabilities | Window labels `fork-*` (`fork-pill`, `fork-scratchpad`, `fork-spotlight`, later `fork-meeting`); `src-tauri/capabilities/fork.json` grants these windows and `main` only the plugin permissions fork pages use (`core:default`, `store:default`, `dialog:default`, `opener:default`, `notification:default` from S-18); fork commands need none (app commands) | Explicit labels or the `fork-*` glob, never `*` (AF-16) |
| Data | `fork.db` and `fork-settings.json` in the app data dir | §4.3, §4.4 |
| Config | `src-tauri/tauri.fork.conf.json` (identity overlay and CSP, passed with `--config`) | §4.11 |
| CI | Fork-owned workflow files prefixed `wb-` (`wb-release.yml`, `wb-build.yml`, `wb-ci.yml`, `wb-egress.yml`, `wb-dco.yml`, `wb-upstream-sync.yml`, `wb-tap-bump.yml`). Upstream workflows are not edited. Kept enabled (no secrets, catch upstream breakage): `test.yml`, `code-quality.yml`, `playwright.yml`. Disabled in the Actions UI: `release.yml`, `build.yml`, `main-build.yml`, `pr-test-build.yml`, `build-test.yml`, `nix-check.yml` | Upstream workflow changes never conflict |

### 4.2 Upstream hotspot hooks (AF-13)

Each hook is one call into `crate::fork` (Rust) or one import + one JSX swap (React). A hook is added by the slice that needs it; the hook-size guard (§4.12) checks every listed file's diff against upstream, and the upstream-sync checklist (§4.13) re-checks all of them. F-09 re-checks this table against `git diff upstream/main --stat`; any changed upstream file not listed here fails.

| Hotspot | Hook | Slice |
|---|---|---|
| `main.rs` | `fork::early_init()` before `run()`: sets `HF_HOME` under app data, installs the panic hook | F-05, F-08 |
| ⚠ `lib.rs` plugin chain | `.plugin(fork::init())` (state only, §4.1 init order) | F-05 |
| ⚠ `lib.rs:1132` | `.invoke_handler(invoke_handler)` becomes `.invoke_handler(fork::compose_invoke(invoke_handler))` | F-05 |
| ⚠ `lib.rs:129-163` activation policy | `if fork::settings::show_in_dock() { return; }` before demoting to Accessory | S-14 |
| `build.rs` | Not edited. DEC-46 needs no ACL manifest; the Apple FM bridge builds in `crates/wb-apple-fm`; Rust strings are embedded by `fork/i18n.rs`. CI asserts `build.rs` still ends in plain `tauri_build::build()` | F-05 |
| `vite.config.ts` | One import and one spread: `...forkInputs(__dirname)` inside `build.rollupOptions.input` (`vite.config.ts:20-25`). `src/fork/vite-inputs.ts` lists `fork-pill` (S-03), `fork-scratchpad` (S-24), `fork-spotlight` (S-30) | S-03 |
| `playwright.config.ts` | Not edited. Fork UI tests use `playwright.fork.config.ts` (`testDir: tests/fork`), run by `bun run wb:test:ui` | F-10 |
| ⚠ `bindings.ts` | Regenerated, never hand-merged. Fork types live in `src/fork/bindings.ts` | F-05 |
| ⚠ `Cargo.toml` `version`, `package.json` `version` | Our version line (DEC-48); re-applied at each sync | F-01 |
| ⚠ `Cargo.toml`, `package.json` dependencies | Fork dependencies in one block marked `# fork` / a `"//fork"` comment key; `crates/wb-apple-fm` as a path dependency | all |
| ⚠ `settings.rs` `default_post_process_providers` | Returns `fork::llm::default_providers()` (adds `local`, drops cloud providers) | F-08, S-10 |
| ⚠ `settings.rs:656-658, 1103-1110` | Default log level from `fork::diagnostics::default_log_level()`; settings dump replaced by `fork::diagnostics::settings_summary()` | F-08 |
| ⚠ `settings.rs` other defaults | Not edited. `fork::settings::apply_product_settings()` sets Handy-side values (§4.4) | F-07 |
| ⚠ `translation.json` × 26 | Not edited (DEC-35) | F-05 |
| ⚠ `actions.rs` record start | `fork::on_record_start(&ctx)`: frontmost app, AX target, LLM prewarm, pill state | S-03, S-05, S-09, S-10 |
| ⚠ `actions.rs:460-476` mic gate | `if fork::models::can_record_without_model() {…}` (starter / queued take) | S-07a |
| ⚠ `actions.rs` post-processing (`post_process_transcription`, incl. the Apple call at `actions.rs:256-272`) | `fork::llm::post_process(...)` handles `local` and Apple; Handy's legacy chain runs only for the custom endpoint | S-12a |
| ⚠ `actions.rs` before paste | `fork::insert::deliver(...)`: post-LLM stages, snippets, AX check, card, clipboard policy, paste verification. **As built (S-04):** `use crate::fork::insert;` plus `insert::deliver(final_text, app)` in place of `utils::paste(final_text, app)` (same signature, 3 changed lines; later slices add their stages inside `deliver`, not in `actions.rs`) | S-04, S-05, S-11, S-17 |
| ⚠ `ACTION_MAP`, `transcription_coordinator.rs` | Fork hotkeys do not use `ACTION_MAP`. One public fn `TranscriptionCoordinator::external_command(cmd)` (start, stop, lock, cancel, with an origin tag) added in a separate MIT-clean commit | S-01 |
| `shortcut/mod.rs` `register_shortcut`, `unregister_shortcut` | First line of each: `if fork::shortcut::owns(&binding.id) { return Ok(()); }`. Handy then never registers `transcribe`, `transcribe_with_post_process` or `cancel` in either keyboard backend, on any path (startup, resume after capture, cancel reconcile) | S-01 |
| `shortcut/handy_keys.rs` | `HandyKeysState::new` builds the manager inside its thread and returns its `Result` via a oneshot, so the persisted fallback runs (upstream-bound fix). No raw-event hook: the fork runs its own tap (§4.8) | S-02 |
| `secure_input.rs` (binding collection for the Carbon shadow) | The collected list passes through `fork::shortcut::secure_input_bindings(list)`, which drops fork-owned ids and adds the fork's keyed bindings that need the Carbon shadow (Esc while active, Ctrl+Opt+Space, ⌃⌘V, ⌃⌘C, ⌥1/⌥2/⌥O when on). The covered/uncovered lists Handy shows then describe the fork's bindings (A-205) | S-02 |
| `overlay.rs` `emit_levels` (`:730`) | First line, before the `OVERLAY_ENABLED` gate: `fork::pill::on_levels(app_handle, levels);` (the fork throttles to 30 fps and targets only `fork-pill`) | S-03 |
| `overlay.rs` `show_overlay_state` (`:488`), `emit_recording_ready` (`:599`), `hide_recording_overlay` (`:689`) | First line of each: `fork::pill::on_state(app_handle, …)` / `on_recording_ready` / `on_hide`, before any gate. Typed events (`RecordingStateEvent`, `StreamPhaseEvent`, `StreamTextEvent`) already reach every webview and need no hook | S-03 |
| `overlay.rs` macOS `create_recording_overlay` (`:452`) | First line: `if fork::pill::replaces_handy_overlay(app_handle) { return; }`, so Handy's hidden `recording_overlay` webview is never built (saves its WebKit process against §5.2). An S-03 test runs a full take with no `recording_overlay` window | S-03 |
| `managers/transcription.rs:1600-1662` | `fork::text::process(raw, ctx)` wraps Handy's deterministic stages per line and returns the verbatim ASR text too | S-11, S-09 |
| ⚠ `managers/history.rs` | `fork::db::on_saved(id, &entry)`, `on_deleted(id)`, `on_retried(id, &old)`. No change to `MIGRATIONS` | S-09 |
| `managers/model.rs` | `fork::models::catalog_overlay()` merged at catalog load; `fork::models::is_fork_asset(path)` in the GGUF-ignore check; mirror URL builder returns `None` through `fork::net` | F-08, S-07a, S-10 |
| `clipboard.rs:50-92`, `paste_tx/macos.rs` | **Not edited (S-04 decision).** With the ⌘V paste method on macOS the fork owns the paste end to end through the `actions.rs` hook; every other paste method (and other platforms) still runs Handy's `utils::paste` unchanged, so there is nothing to delegate | S-04 |
| `llm_client.rs` | `RequestOptions` struct: transport (Unix socket or loopback port + key, DEC-49), `temperature`, `max_tokens`, timeout, neutral headers (upstream-bound) | S-10 |
| `apple_intelligence.rs`, `swift/apple_intelligence.swift` | Not edited. The fork's Apple FM bridge is `crates/wb-apple-fm` + `fork/llm/apple.rs`; Handy's call site moves behind `fork::llm::post_process` (row above) | S-12b |
| `tray.rs` | `fork::tray::extend(&builder)` in the menu builder: replaces Handy's title/version item (`"Handy v{}"`, `tray.rs:461-463`) and tooltip with localized fork strings (F-01), then adds/removes items (S-26a) | F-01, S-26a |
| `managers/audio.rs:100-111` (`set_mute`) and `:204` (`get_mute`) | Both `osascript` calls delegate to `fork::audio::{set_mute, get_mute}` (CoreAudio) | S-08 |
| `utils.rs:102` `cancel_current_operation` | `audio_manager.cancel_recording()` becomes `fork::recovery::cancel_keeping_audio(app, &audio_manager)`: samples are kept 4 s for Undo, then dropped | S-06 |
| ⚠ `src/App.tsx` | Import `ForkApp` and render it in place of Handy's sidebar + section layout and onboarding switch | S-13, S-29a |
| ⚠ `Sidebar.tsx` | Not edited (the fork sidebar replaces it) | S-13 |
| `src/i18n/index.ts` | One call `registerForkI18n(i18n)` after init (loads `fork.json`, applies overrides) | F-05 |
| ⚠ `tauri.conf.json`, `Info.plist`, `Entitlements.plist` | Not edited for identity (overlay). Usage strings use the overlay's `bundle.macOS.infoPlist` if Tauri 2.11 supports it (check in F-01); otherwise one edited line per string, listed in the sync checklist | F-01 |
| ⚠ `release.yml`, `build.yml` and other upstream workflows | Not edited; enabled/disabled per §4.1 CI row | F-02 |
| ⚠ `engine_supervisor/protocol.rs` | Untouched in M1. M2 decides in N-01 | N-01 |

### 4.3 `fork.db` schema

One SQLite file `fork.db` in the app data dir, opened with `journal_mode=WAL`, `busy_timeout=5000`, `synchronous=NORMAL`, one connection per process behind a mutex, migrations via `rusqlite_migration 2.3` with its own `user_version`. `history.db` stays Handy's; links use Handy's history row id. FTS is maintained by the app (no cross-DB triggers).

**Consistency between the two databases.** Fork writes happen in hooks after Handy has saved to `history.db`, so a crash can leave a Handy row without `entry_meta`. Rules: every fork write is an idempotent upsert keyed by `entry_id`; a reconcile job at launch backfills missing `entry_meta` from Handy rows (marked `backfilled`, stats derived from text) and drops `entry_meta` rows whose Handy row is gone; fork retention deletes Handy rows only through Handy's history manager API (never by SQL on `history.db`); delete-all removes both databases (S-27).

| Table | Key columns | Written by |
|---|---|---|
| `entry_meta` | `entry_id` PK (Handy history id), `created_at`, `asr_text` (verbatim), `final_text`, `shown_variant` (`final`/`raw`), `previous_text`, `audio_ms`, `word_count`, `char_count`, `language`, `app_bundle_id`, `app_name`, `style_group` (`personal`/`work`/`email`/`other`), `usage_category` (`documents`/`ai_prompts`/`email`/`work`/`personal`/`other`), `url_host`, `cleanup_level`, `cleanup_engine` (`none`/`local`/`apple`), `cleanup_outcome` (`applied`/`skipped`/`guard`/`timeout`/`error`), `fix_count_dictionary`, `fix_count_llm`, `paste_outcome` (`confirmed`/`unverified`/`card`/`refused`/`queued`), `context_used`, `source` (`dictation`/`file`/`practice`/`command`/`transform`), `flagged`, `backfilled`, `deleted_at`, `audio_deleted_at` | S-09 |
| `daily_stats` | `day` PK (local date at record time), `words`, `dictations`, `speech_ms`, `fixes_dictionary`, `fixes_llm` | S-09 |
| `daily_app_stats` | (`day`, `usage_category`) PK, `words`, `dictations` | S-09 |
| `corrections` | `id`, `entry_id`, `from_text`, `to_text`, `source` (`rule`/`llm`/`flag`/`learn`), `created_at` | S-11, S-25 |
| `history_fts` | FTS5 `trigram` over `final_text` with `entry_id` (contentless, app-maintained) | S-09 |
| `dictionary_entries` | `id`, `kind` (`word`/`rule`), `term`, `from_text`, `to_text`, `source` (`user`/`import`/`flag`/`learn`/`migrated`), `status` (`active`/`suggested`/`rejected`), `created_at`, `hit_count`, `last_hit_at` | S-16 |
| `snippets` | `id`, `trigger`, `trigger_norm`, `body`, `body_html` (nullable, unused in M1, DEC-12), `created_at`, `updated_at`, `hit_count` | S-17 |
| `notes`, `notes_fts` | `id`, `title`, `body`, `pinned`, `created_at`, `updated_at`, `deleted_at`; FTS5 trigram | S-24 |
| `notifications` | `id`, `kind`, `title_key`, `body_key`, `args_json`, `action`, `created_at`, `read_at` | S-18 |
| `app_rules` | `match` PK (bundle id or host pattern), `style_group`, `auto_send`, `source` (`builtin`/`user`) | S-19a |
| `transforms` | `id`, `builtin_key`, `name`, `hotkey`, `rules_json`, `prompt`, `examples_json`, `updated_at` | S-21, S-22 |
| `transform_runs` | `id`, `transform_id`, `app_bundle_id`, `source_text`, `result_text`, `rating`, `created_at` (bounded to the last 20 rows; sensitive) | S-21 |
| `queued_takes` | `id`, `wav_path`, `created_at`, `status` (`waiting`/`transcribing`/`ready`/`failed`), `entry_id` | S-07a (session), S-07c (durable) |
| M2: `meetings`, `meeting_segments`, `meeting_notes`, `folders` | see §10 | N-04 |

**App taxonomies (defined once in S-09, `fork/context/categories.json`, with tests):** `style_group` drives Styles (S-19a); `usage_category` drives Insights (S-25); a separate `is_ai_chat` flag drives the cleanup skip (S-12a). An app has one value of each; a host rule overrides the app rule for browsers (S-19b).

Delete-all removes `fork.db`, `-wal` and `-shm`.

### 4.4 Fork settings store

`fork-settings.json` through `tauri-plugin-store`, a typed `ForkSettings` struct (serde defaults, specta types, its own `schema_version` and migrations), exposed by `fork_get_settings` / `fork_set_setting` and a React hook `useForkSettings()` with a Zustand store. Contents: hotkey bindings (≤4 per action, activation per binding, unset allowed), cleanup level, style per group, context awareness, Transforms and Command Mode toggles, auto-apply transform, bar visibility / position (bottom or top) / hidden-until, live-text option, retention (text, audio), notification toggles, update checks and update channel, onboarding state (the single source of truth, fixing RE-16; `fork/onboarding/state.rs` is only the logic over it), dismissed banners, seen "New!" tags, first-use choice, typing WPM, greeting-name override, auto-learn toggle, vibe-coding setup.

`apply_product_settings()` runs on every start and has two lists:

| List | Values | Rule |
|---|---|---|
| `ENFORCED` (every start) | Handy overlay off; Handy `history_limit`/retention set to keep everything (the fork job owns retention); second post-process hotkey disabled; Experimental off; post-process provider not a cloud provider (a cloud value becomes `local`; `local` and a loopback `custom` stay); `update_checks_enabled` mirrored from the fork setting | Values the product depends on; the user cannot change them through Handy's UI, which the fork does not mount |
| `FIRST_RUN_DEFAULTS` (once, recorded in fork settings) | concealed paste on, sounds on, file log level Info, paste method, keyboard backend | User choices in Advanced persist across launches; a test changes each one, relaunches and checks it |

### 4.5 LLM sidecar, supervisor, `local` provider and guards

- **Binary.** `llama-server` built in CI from a pinned llama.cpp tag (MIT): static, `GGML_METAL_EMBED_LIBRARY=ON`, `CMAKE_OSX_DEPLOYMENT_TARGET=13.0`, TLS off, server target only, one per arch, shipped as Tauri `externalBin` and signed before the app. Never downloaded at runtime. (R4 row "Local LLM runtime"; W-04.)
- **Supervisor** (`fork/llm/supervisor.rs`, W-01): Rust port of TypeWhisper `LlamaCppServer.swift` + `LlamaCppRuntime.swift` (GPL-3.0-or-later, port; `LlamaCppPluginTests.swift` as the acceptance list). Spawn `-m <gguf> --host <app-data>/llm.sock -c 4096 -np 1 --cache-ram 256 --reasoning off --jinja --no-webui --offline` (or `--host 127.0.0.1 --port <random>` per DEC-49; `--offline` only if the pinned tag accepts it, checked in RS-06), random `LLAMA_API_KEY` in env, scrubbed `LLAMA_*`/`HF_*` env, never `-hf`. Health poll `/health`; prewarm at key-down (ordered, below); restart once, then back off; CPU fallback `-ngl 0`; SIGTERM then SIGKILL; stale-PID kill at launch; the child dies with the parent (kqueue `NOTE_EXIT` watchdog). Esc closes the connection to abort generation (RT-12).
- **Memory policy** (D-213). Load the LLM only if available memory ≥ model RSS + 1.5 GB; otherwise skip cleanup for that take ("Inserted without cleanup"). **Ordered loads:** loads never overlap. At key-down: if the ASR model is loaded, wake the LLM at once; if the ASR model is cold, load ASR first and start the LLM load only after ASR reports loaded (while the user is still speaking) or at key-up, whichever is first. ASR has priority; if the LLM is not ready by the deadline, the take fails open. **One idle owner:** the supervisor stops the sidecar after 120 s without a request (no `--sleep-idle-seconds` flag). ASR unload stays Handy's 5 min default. RS-06 measures the cold-ASR + cold-LLM case on M1 8 GB (§5.1).
- **`local` provider** (W-02): built-in provider id `local`, transport per DEC-49, structured output on, temperature 0, `max_tokens` from transcript length, reasoning off, one call per take, neutral headers (no `Referer`/`X-Title: Handy`), via reqwest `unix_socket` (0.12.28, `#[cfg(unix)]`) or loopback. Because built-ins are re-added on read, every install gets it.
- **Prompt builder** (`fork/llm/prompt.rs`, W-03): one builder for level + style fragment + transform rules + dictionary terms + context hints + language. Seeds: OpenWhispr `cleanupPrompt` (MIT, port), FreeFlow `commandModeSystemPrompt` (MIT, port). Transcript passed as a JSON field with BEGIN/END boundary (FluidVoice framing, TypeWhisper boundary; GPL, port).
- **Guards** (`fork/llm/guards.rs`), in order: JSON parse; SpeakoFlow validators (empty, punctuation-only, channel name, prompt echo, > max(80, 3×) chars; MIT, port); FreeFlow `appearsToHaveExecutedInstruction` (MIT, port; English); BetterWispr `accepted()` word band 0.4×–1.2×+2 and ≤25 % new tokens (GPL-3.0 VoiceInk-derived, port with attribution); Muesli assistant-marker check (MIT, port); entity guard (digits, URLs, e-mails, code spans, dictionary terms, line count must survive); language check (Handy `lang_id.rs`). Any hit, deadline miss or error pastes the deterministic text and shows "Inserted without cleanup". Never paste unparsed output.
- **Deadline:** `1.2 s + 25 ms × expected output tokens`, capped at 6 s, wrapping the whole chain.
- **Apple FM** (`fork/llm/apple.rs` + `crates/wb-apple-fm`, S-12b): our own bridge, ported from hex `apple_intelligence.swift`/`.rs` (MIT) with MacParakeet error mapping and TypeWhisper prewarm call sites (GPL, port), all in fork files; called from `spawn_blocking` with a cancelling timeout. Handy's own bridge files are not edited.

### 4.6 AX layer

`fork/ax/` (W-05): port of SpeakoFlow `smart_spacing/macos.rs` (MIT) on `objc2-application-services 0.3.2` (Zlib/Apache/MIT, adopt). One dedicated AX thread; 0.2 s messaging timeout; `AXManualAccessibility` lease for Chromium/Electron (on at first read, off 60 s after the last read, per-PID refcount from VoiceInk `AccessibilityTextReader.swift`, GPL port); never `AXEnhancedUserInterface`; skip secure fields, Secure Input and terminals. API: `focused_target()`, `is_editable(target)` (role in TextField/TextArea/ComboBox/web text area, enabled, not secure, value settable; OpenWhispr rule, MIT port), `caret_window(budget)`, `selected_text()` (AX → WebKit/Chromium text markers → AX Edit › Copy menu press via SelectedTextKit pattern, MIT → ⌘C with sentinel from SpeakoFlow `selection.rs`, MIT), `set_selected_text` (used only after RS-04 says it works; else paste), `window_title()`, `document_url()`. Fail-open rule (TypeWhisper, GPL port): an empty AX answer with a focused window means "paste anyway". Gecko/Thunderbird excluded from AX writes. Every caller tolerates `Unknown`. **Target rule:** the insertion target is the element focused at key-up; if focus moves to another element between key-up and delivery, the text goes to the card. **RS-04 (2026-10-30, macOS 26.6.2):** the frontmost app comes from NSWorkspace (system-wide `AXFocusedApplication` returned `CannotComplete` on every call, even from Terminal); only controls (button, table, list, link) and "no window" get the card, while containers and non-editable web areas fail open, because in Safari, Obsidian, Cursor and ChatGPT they hosted fields AX did not report; no text role ever got the card. Electron apps (Cursor, Obsidian, Claude) usually report no focused element with or without the lease, and the lease changed no answer (Chrome rejects it), so selected text there, and in Mail and Messages, comes from the copy fallback. `AXSelectedText` writes are no-ops in web and Electron fields: replace selection by paste. A whole read can take ~0.3 s in Electron, so read only what the verdict needs before pasting. Details: `docs/fork/spikes/RS-04-ax.md`.

### 4.7 Clipboard layer

`fork/clipboard/` (W-07): port of hex `src/paste.rs` (MIT): snapshot every `NSPasteboardItem` and type (skip file contents, PDF, RTFD and audio, as Wispr does), write our text with `org.nspasteboard.TransientType`, `AutoGeneratedType` and Handy's concealed type, paste with the layout-aware ⌘V (Handy `input.rs`), wait for hotkey modifiers to be physically released, restore only if `changeCount` is unchanged, after a paste receipt or 1.5 s (7 s for Chromium/Electron targets). Back-to-back takes carry the last real snapshot forward. `arboard 3.6.1` (MIT/Apache, adopt as direct dependency) for `exclude_from_history()` writes. Paste verification (TypeWhisper `insertTextAtAndVerifyChange`, FluidVoice `PasteVerifier.swift`; GPL, port): poll `AXValue` change ≤ 300 ms → `confirmed`/`unverified`. Chromium 155/156 path types ≤ 20 UTF-16 units (hex). The card's Copy replaces the clipboard and does not restore it. **Nothing else writes the clipboard:** queued takes, late results and auto-stopped takes reach the user through the card, History and notifications, and touch the clipboard only on an explicit Copy or ⌃⌘V. **RS-05 (2026-10-30):** use the objc2 `NSPasteboardItem` path (hex's Carbon path adds two text types on restore); 68/68 restores byte-identical across 11 apps and six source types, the transcript written as a lazy item was read 4-53 ms after ⌘V and restored ~210 ms after it, 56/56 pastes landed with 0 old-clipboard pastes, and Chrome 156 dropped 0/12 ⌘V (keep the typing path for 155/156 anyway, as hex does). Clipboard managers and held ⌃⌘ remain unmeasured (S-04/S-06 acceptance). Details: `docs/fork/spikes/RS-05-clipboard.md`. **S-04 as built (2026-10-30):** one more rule from the S-04 property test: a paste waits until the previous take's target has read its text (or that take's deadline passed) before it replaces the transcript, otherwise a late reader inserts the next take's text a second time; the restore and the snapshot of the next take run under one lock. The restore is unmarked (byte-identical), so a manager may record it as a copy of the user's own content (RT-24 unchanged). `arboard` is not adopted: the objc2 item already carries the concealed marker `exclude_from_history()` writes.

### 4.8 Hotkey layer

`fork/shortcut/` owns the fork's hotkeys with **its own tap**: `handy_keys::KeyboardListener::new_with_blocking(blocking)` (public MIT API of handy-keys 0.3.4, `listener.rs:31-70`; no handy-keys fork). Handy's `HotkeyManager` only emits events for registered hotkeys (`manager.rs:92-275`), so it cannot feed the fn grammar, which needs every key event while fn is held. Handy's manager keeps running for nothing the fork owns (the `owns()` hook in `shortcut/mod.rs` stops it registering `transcribe`, `transcribe_with_post_process` and `cancel`), so two taps coexist; RS-01 tests exactly this setup and S-02 watches both.

**Blocking set (`BlockingHotkeys`, changed at runtime).** handy-keys blocks an event when a hotkey in the set matches its modifiers and key (`state.rs:40-48`); for fn it blocks the fn-down FlagsChanged event (`platform/macos/listener.rs:320-335`), which is what suppresses the Globe action. Rules: `{FN}` (modifier-only) is always in the set while fn is the active binding, so Globe suppression is kept; `{FN, Space}` is added while fn is down and removed on release; `{Esc}` is added only while recording or processing (and, under Secure Input, shadowed through the Carbon path, S-02); the Ctrl+Opt fallback profile adds its own entries the same way. Cards, toasts and the spotlight observe Esc from the event stream; they never block it (Esc registry in `fork/shortcut/actions.rs`).

Raw events flow to `processor.rs`, a port of anarlog `crates/shortcut-macos` `processor.rs` + `decision.rs` + tests (MIT, port; keep anarlog and Hex legacy notices) extended with Wispr's 0.5 s grammar (§S-01). Test vectors from MacParakeet `FnKeyStateMachineTests`/`HotkeyGestureControllerTests` and FluidVoice `ModifierOnlyShortcutFlagsDecision` tests (GPL, port as tests). `health.rs`: axwatch 0.1.0 (MIT/Apache, vendor) + a probe tap on wake and permission change for both taps; no polling of `CGEventTapIsEnabled` (handy-keys #1827). The processor drives the coordinator through `external_command`. Fork bindings live in fork settings.

**Secure Event Input (what still works).** While any process holds Secure Input (focused password field, Terminal/iTerm2 Secure Keyboard Entry), event taps stop receiving key-down/key-up events but FlagsChanged still flows (Handy `secure_input.rs:1-20`; R3 §3). So: fn hold (push to talk) and double-tap fn (hands-free) keep working; Ctrl for Command Mode is seen (flags); fn+Space lock does not work; Esc through the tap does not work, so the fork shadows Esc through Handy's Carbon-backed global-shortcut path while recording or processing (the same mechanism Handy uses for its cancel binding); if RS-14 shows the shadow fails, ✕ on the pill is the cancel. Keyed fork bindings (Ctrl+Opt+Space, ⌃⌘V, ⌥1/⌥2/⌥O) are shadowed the same way. The pill says why fn+Space is ignored (S-02).

### 4.9 Pill window

`fork/pill/` (W-08) + `src/fork/pill/`: a fork-owned `fork-pill` NSPanel via `tauri-nspanel 2.1.1` (MIT/Apache, adopt; bump from the git pin), following Tolkovo `tolkovo/pill.rs` (MIT, port): `PanelLevel::Status`, non-activating, `hides_on_deactivate(false)`, `accept_first_mouse(true)` via `with_window`, `_setPreventsActivation:` only if RS-03 shows it is still needed, re-show on Space change, all Spaces + full-screen auxiliary. The window is sized exactly per state (no transparent margin, so no click-through toggling). Sizes are defined once in Rust (`pill/geometry.rs`) and exported to CSS variables through a generated JSON file (fixes RE-13). Hover comes from a native tracking area. Placement: bottom centre of the display under the cursor (top centre if the user chose top), above the Dock and below the menu bar, flush in full-screen Spaces, re-placed on `NSApplicationDidChangeScreenParametersNotification` and on a 1 Hz cursor-display check while resting. Mic levels and overlay phases arrive through the `overlay.rs` hooks (§4.2) before Handy's overlay gate; Handy's own overlay window is never created. Plan B for RT-17 memory: Voquill `rust_macos_pill` (AGPL, reference) or anarlog's native Swift pill.

### 4.10 Text pipeline (stage order)

Newline-safe throughout: stages that tokenise on whitespace run per line through `fork/text/lines.rs` (W-12); the final whitespace pass collapses spaces inside lines only. Each stage returns `(text, change_count)`.

| # | Stage | Code |
|---|---|---|
| 1 | ASR text → store verbatim `asr_text` | Handy |
| 2 | Chinese script conversion | Handy |
| 3 | Exact dictionary rules from→to (whole word, case-insensitive, literal, longest wins), outputs protected with Private Use Area placeholders | port of VoiceTypr `writing/library_rules.rs` (AGPL-3.0, file marked AGPL) on aho-corasick |
| 4 | Handy custom-word vocabulary / fuzzy match (placeholders protected) | Handy |
| 5 | Filler removal, stutter collapse (per line); off at level None | Handy `text.rs` via the wrapper |
| 6 | ITN numbers/money/dates (only if S-33 ships) | text-processing-rs |
| 7 | Voice commands at clause edges: new line, new paragraph, scratch that; "literally" escape | own + MacParakeet `SpokenPunctuation.swift` tables (GPL, port data) |
| 8 | LLM cleanup / style / auto-apply transform with guards; fail-open to stage 7 output | `fork/llm` |
| 9 | Dictionary rules re-applied (the LLM must not undo spellings) | stage 3 code |
| 10 | Deterministic style rules (trailing periods, casing for the most relaxed preset, exclamation for the upbeat preset) | port of BetterWispr `StyleFormatter.swift` (Apache) |
| 11 | Snippet expansion: longest first, word/Unicode boundaries, single pass, never rescan | own aho-corasick matcher (W-13), OpenWhispr `snippets.ts` (MIT) reference |
| 12 | Line-safe whitespace normalization, placeholder restore, trailing space option | own |
| 13 | Insertion: paste keeps `\n`; typing mode uses Shift+Return in chat apps | `fork/insert` |

### 4.11 Identity overlay and CSP

`src-tauri/tauri.fork.conf.json`: `productName: "Wispr Bro"`, `identifier: "com.wisprbro.desktop"`, `version` (our line, equal to `Cargo.toml`, DEC-48), `bundle.macOS.minimumSystemVersion: "13.0"`, `bundle.externalBin` (llama-server), `plugins.updater.pubkey` (ours) and `endpoints` (ours, stable channel; the beta endpoint is set at runtime by `fork/update/channel.rs` through `updater_builder().endpoints(...)`), icons, and `app.security`. Every `tauri dev/build` call passes `--config src-tauri/tauri.fork.conf.json` (package.json scripts `wb:dev`, `wb:build`). The `wb-*` workflows read the version from the overlay. Brand strings come from the i18n overrides and `{{appName}}`. The Nix env var `HANDY_DISABLE_UPDATER` keeps working; we also honour `WISPRBRO_DISABLE_UPDATER`.

**CSP (DEC-43).** Handy ships `csp: null` with the asset protocol on (`tauri.conf.json:16-17`); history playback uses `convertFileSrc(…, "asset")` (`HistorySettings.tsx:221`). The overlay sets:

```
"security": {
  "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: asset: http://asset.localhost; media-src 'self' blob: asset: http://asset.localhost; font-src 'self' data:; connect-src 'self' ipc: http://ipc.localhost; object-src 'none'; base-uri 'self'; frame-ancestors 'none'",
  "devCsp": "<same, plus http://localhost:1420 in script-src, style-src, img-src and connect-src, and ws://localhost:1420 in connect-src>",
  "assetProtocol": { "enable": true, "scope": { "allow": ["$APPDATA/**"] } }
}
```

Tauri adds hashes for its own injected scripts. Portable mode (E-88) adds its data dir to the asset scope at runtime. F-08 verifies the CSP in a real `wb:build` smoke run (play a recording, render an image to `data:`/`blob:`, call a fork command); S-25 re-runs it with "Save as image".

### 4.12 CI guards (`wb-ci.yml` on every PR, `wb-egress.yml` nightly and on release; all blocking)

| Guard | Rule |
|---|---|
| Identity | Fail if the effective config (base + overlay) contains `com.pais.handy`, `cjpais/Handy` or the upstream updater pubkey prefix; if `productName` ≠ Wispr Bro; if our pubkey or our endpoint is missing; if `Cargo.toml` version, overlay version and the git tag (on tags) disagree; if `<owner>`/`<hf-org>` placeholders remain (release only) |
| Fork plumbing | `src-tauri/build.rs` still ends in plain `tauri_build::build()` and defines no `AppManifest`; no Handy command name starts with `fork_`; `compose_invoke` is the only `invoke_handler` call |
| Hook size | For every file in `ci/hooks.txt` (the §4.2 table), `git diff upstream/main -- <file>` stays within the listed number of changed lines; any other changed upstream file fails |
| Network: fork code | Scan `src/fork/`, `src-tauri/src/fork/`, `src-tauri/crates/wb-*`, `scripts/wb-*` and `src-tauri/tauri.fork.conf.json` for URL literals; every host must be in `ci/allowed-hosts.txt` (the §1 hosts plus `link:` hosts that may appear only as `openUrl` arguments). New hosts need an owner-approved PR |
| Network: upstream code | Scan the rest of `src/`, `src-tauri/src/`, `src-tauri/*.json` and `src/i18n/locales/**/translation.json`; every host must be in `ci/upstream-hosts-baseline.txt`, a reviewed list (today: `api.openai.com`, `api.anthropic.com`, `api.groq.com`, `api.cerebras.ai`, `api.deepseek.com`, `openrouter.ai`, `api.z.ai`, `bedrock-mantle.us-east-1.api.aws`, `blob.handy.computer`, `handy.computer`, `github.com` and doc/example hosts). A new upstream host fails the sync PR until it is reviewed and either proven unreachable (added to `ci/inert-hosts.txt` with the test that proves it, e.g. DEC-04's backend refusal or F-08's mirror hook) or listed in §1 |
| Network: built bundle | After `wb:build`, scan `dist/**/*.js` and the strings of the Mach-O binary; every host must be in `allowed-hosts.txt` or `inert-hosts.txt` |
| Network: runtime egress | `wb-egress.yml` on a macOS runner: launch the release build with a fresh profile behind a pf anchor that blocks all outbound traffic except loopback and DNS, log blocked packets with `pflog`, and log DNS queries; run a scripted session (launch, skip onboarding, 5 takes from WAV fixtures, open every Hub page, update check, model download from a local HF mock on loopback); fail on any DNS query or connection attempt from our processes to a host outside §1 |
| Telemetry | Match exact package names, never substrings: `ci/telemetry-packages.txt` lists `sentry`, `sentry-*`, `@sentry/*`, `tauri-plugin-sentry`, `posthog-*`, `posthog-js`, `posthog-rs`, `mixpanel*`, `@aptabase/*`, `tauri-plugin-aptabase`, `telemetrydeck*`, `firebase`, `@firebase/*`, `@amplitude/*`, `amplitude-js`, `@segment/*`, `analytics-node`, `@datadog/*`, `dd-trace`, `@bugsnag/*`, `bugsnag*`, `react-ga*`, `@vercel/analytics`. Checked against package names in `Cargo.lock`, `bun.lock` and import specifiers (so `unicode-segmentation` passes) |
| Licence headers | Every file under `src-tauri/src/fork/`, `src-tauri/crates/wb-*`, `src/fork/` and `scripts/wb-*` has an SPDX header, except generated files listed in `ci/generated.txt` (`*.gen.json`, `src/fork/bindings.ts`); AGPL-marked files are listed in NOTICE; no file outside fork paths gains a GPL or AGPL header |
| Licence register | `cargo-about` + npm licence check fail on GPL-2.0-only, unlicensed or non-free licences |
| i18n | Handy's `check-translations.ts` wrapped for the `fork` namespace: key parity across 26 locales, placeholders identical, no hardcoded JSX strings (ESLint), `rust.*` keys used by `fork/i18n.rs` present. Before S-37, entries flagged `machine` or `untranslated` pass; from S-37 on (file `ci/i18n-strict` exists) they fail |
| Upstream-clean | Files listed in `ci/upstream-bound.txt` (e.g. the `HandyKeysState::new` fix) must not import `crate::fork` and carry no GPL header |
| Tests | `cargo test`, `cargo clippy -D warnings`, `bun run lint`, `bun run format:check`, `bunx playwright test -c playwright.fork.config.ts` |

### 4.13 Upstream-sync workflow

`wb-upstream-sync.yml` (W-23, ~50 lines): weekly (Monday) and on demand, fetch `upstream/main`, merge into `sync/upstream-<date>` off `wispr-bro`, open a PR with a checklist: regenerate `bindings.ts`; re-apply the `Cargo.toml`/`package.json` version lines; re-check every hook in §4.2 and the line numbers it cites; review upstream changes to `release.yml`/`build.yml` and port fixes into `wb-*`; `settings.rs` defaults vs `apply_product_settings`; new strings containing "Handy" (override them); new upstream hosts (baseline guard); `protocol.rs` diff (M2); catalog changes (licence register); run all guards. Uses a fine-grained PAT or App token with workflow scope; never has signing secrets. Merge cadence: every two weeks in M1, immediately for security fixes, never in the 3 days before a release.

---

## 5. Non-functional requirements

All numbers are measured on the reference machines: **M1 8 GB** (macOS 14), **M-series 16 GB+** (macOS 15 and 26), and **Intel i5/i7 16 GB** (macOS 13) if DEC-29 keeps Intel. "Take" = one dictation.

### 5.1 Latency (key-up to text in the field)

| Case | Apple Silicon p50 | Apple Silicon p95 | Intel p50 (DEC-29) | Intel p95 (DEC-29) |
|---|---|---|---|---|
| ≤ 20 s take, cleanup None or deterministic only | ≤ 500 ms | ≤ 1,000 ms | ≤ 1,500 ms | ≤ 3,000 ms |
| ≤ 50 words, Light cleanup (local LLM) | ≤ 1,300 ms | ≤ 2,200 ms | opt-in only | ≤ 3,000 ms if offered |
| ≤ 50 words, Medium or a style | ≤ 1,600 ms | ≤ 2,600 ms | – | – |
| 51-200 words, any cleanup | ≤ 2,500 ms | deadline (≤ 6 s), then fail-open | – | – |
| Transform on ≤ 150 words | ≤ 2,000 ms | ≤ 3,500 ms | – | – |
| fn-down → capture running, cold mic stream (DEC-47: mic opens at `T_show` = 150 ms) | ≤ 250 ms | ≤ 350 ms | ≤ 300 ms | ≤ 450 ms |
| fn-down → capture running, warm stream (≤ 30 s after a take) | ≤ 20 ms | ≤ 50 ms | ≤ 30 ms | ≤ 80 ms |
| First take after ASR unload (extra over the rows above) | ≤ +800 ms | ≤ +1,500 ms | ≤ +2,000 ms | ≤ +4,000 ms |
| First take with ASR and LLM both cold, M1 8 GB, ordered loads (extra over the Light row) | ≤ +1,500 ms | ≤ +2,500 ms, else that take fails open to deterministic text | – | – |

First-word clipping from the later mic open: ≤ 1 clipped first word in 50 natural takes (RS-01). If a RAM tier misses its LLM budget in RS-06/S-38, use the next smaller model for that tier.

### 5.2 Memory and CPU

| Item | Ceiling |
|---|---|
| App idle (pill visible, models unloaded) | ≤ 250 MB RSS total for the app process plus every WebKit process (Hub, `fork-pill`, any hidden fork window; Handy's `recording_overlay` is never created); idle CPU ≤ 0.5 % averaged over 10 min |
| Pill webview growth | ≤ 50 MB over 8 h resting with 100 takes (RT-17) |
| 8 GB Mac, ASR + LLM loaded during a take | ≤ 3.2 GB combined RSS; no swap growth > 200 MB over a 30-take session |
| LLM resident time | stopped by the supervisor after 120 s idle (single idle owner); ASR unload stays Handy's 5 min default |
| Memory pressure | if available memory < model RSS + 1.5 GB, skip the LLM for that take; never crash or stall |

### 5.3 Startup and first run

| Item | Budget |
|---|---|
| Launch to hotkey ready and pill visible | ≤ 2 s (p95 ≤ 3 s) on Apple Silicon |
| Starter model ready on a clean install | ≤ 25 s at 50 Mbit/s, ≤ 90 s at 10 Mbit/s |
| Permissions granted to first successful dictation | ≤ 60 s median at 50 Mbit/s |
| Install to first successful dictation (incl. permissions) | ≤ 2 min median at 50 Mbit/s |
| Onboarding completion | median ≤ 5 min, 90th percentile ≤ 8 min (10 testers) |

### 5.4 Reliability

- **Offline.** After setup, every M1 feature works with networking off. The update check fails silently and retries.
- **Exactly-once insertion.** 0 duplicated, 0 lost and 0 wrong-target insertions in a 500-take soak across 10 apps (Mail, Notes, TextEdit, Slack, VS Code, Chrome Gmail, Safari, Notion, Messages, Terminal), run with the S-38 harness. A lost insertion means the text is not in the field and no card appeared.
- **Never lose words.** Every take with speech ends in the field, on the card, or in History (and queued takes deliver later). Audio is saved before transcription starts.
- **Clipboard.** Restored byte-identical for text, RTF, HTML, URLs, images and multi-item content in RS-05's matrix; clipboard managers (Maccy, Raycast, Paste) do not record dictated text. Only an explicit Copy or ⌃⌘V writes the clipboard for queued or late takes; a test checks the clipboard is unchanged after queue delivery.
- **Hotkey self-healing.** After sleep/wake, the hotkey works within 2 s of wake. After Accessibility is revoked, the user is told within 2 s (pill + tray, notification if the app is hidden); after it is re-granted, hotkeys return within 5 s without relaunch.
- **Negative cases (each owned by a slice, with its expected result):** mic permission revoked while running → pill error with Fix, audio so far kept (S-08); model download hash mismatch, HF 429/5xx or a captive portal → retry with backoff (honouring `Retry-After`), then a "Couldn't download · Retry" state, never a corrupt model (S-07a); sleep, lid close or fast user switching during a take → recording stops, audio kept, result goes to the card after wake, never pasted blind (S-04); sidecar or download in flight at sleep → restarted or resumed on wake (S-10, S-07a); CJK IME active during paste → paste verified, else card (S-04); disk full while recording → pill message, transcript kept in memory and delivered to the card plus an inbox entry (S-09); take finished in another app or focus moved during processing → paste only into the element focused at key-up, else card (S-05).
- **Crashes.** No crash in the 500-take soak or the 8 h idle run. The LLM sidecar or engine worker crashing never takes the app down; the take fails open.
- **Updates.** Never install while recording, processing or within 20 min of the last take; TCC permissions survive updates (RT-36).

### 5.5 Accessibility

- Every Hub, settings, onboarding and dialog control is reachable by keyboard with a visible focus ring; Esc closes dialogs; sidebar items are buttons with `aria-current`.
- VoiceOver reads every control's name and state; the pill announces recording, processing, inserted and card text through `NSAccessibility` announcements (localized through `fork/i18n.rs`); the card is reachable with the keyboard.
- Text contrast WCAG AA (4.5:1; 3:1 for large text and icons) in light and dark themes.
- `prefers-reduced-motion` replaces animations with stills (Handy's demo engine already supports it); confetti respects it.
- Automated: `eslint-plugin-jsx-a11y` and `@axe-core/playwright` on every page (no serious or critical findings). Manual: the scripted VoiceOver checklist per screen in `docs/fork/VOICEOVER-CHECKLIST.md` (S-37).

### 5.6 Localization

- 26 locales kept in sync with Handy's set; English is the source. New strings land in `en/fork.json` first. `scripts/wb-translate-missing.ts` (W-25, created in F-05) fills the other 25 with a pluggable backend: the local `llama-server` (4B tier when available) or "copy English and flag `untranslated`"; it enforces identical `{{placeholders}}` and flags every machine entry `machine`. Flags pass CI until S-37 and fail from S-37 on.
- Before 1.0 a person reviews every machine-translated string for the 10 sample-text languages (en, es, fr, de, pt, it, ja, zh, ko, ru); the other 15 locales ship machine-translated with the `machine` flag cleared only after S-37's pass, listed as such in the release notes.
- Rust-side strings (tray, app menu, notifications, VoiceOver announcements) follow the same rules through `rust.*` keys.
- No hardcoded user-facing strings (ESLint). Brand via `{{appName}}`.
- RTL (Arabic, Hebrew) layouts checked in Playwright. Sample texts (typing test, tutorials) exist for the 10 languages above; other locales fall back to English sample text with translated UI.

### 5.7 Crash and diagnostics

- Panic hook (`fork/diagnostics/crash.rs`) writes `crash-<timestamp>.txt` to the logs folder; next launch puts "Wispr Bro quit unexpectedly · Copy report" in the inbox. `.ips` reports from `~/Library/Logs/DiagnosticReports/` for our bundle are picked up the same way.
- Nothing is uploaded. "Report a problem" opens a prefilled GitHub issue (version, macOS, chip, RAM, model ids; no transcripts) and offers Copy diagnostics / Save diagnostics… (zip of sanitized logs, `diagnostics/bundle.rs`).
- File log level Info by default; transcripts, dictionary words, prompts, e-mails and `$HOME` are masked by the sanitizer (`fork/diagnostics/sanitizer.rs`, port of Voquill `log_sanitizer.rs`, marked `AGPL-3.0-only`, listed in NOTICE). Debug level only through the Debug panel or `--debug`.

---

## 6. M0 Foundations

M0 makes the fork safe to build on: our identity, our signing and update key, our CI, the fork namespace, our data stores, and the privacy cleanups. It ships an internal signed build `1.0.0-alpha.1` that behaves like Handy with our identity and privacy defaults. Slice template: goal, outcome, rows, parts, Handy files, new files, tests, Mac checks, exit, effort, dependencies.

### F-01 Identity overlay and rebrand basics
- **Goal:** the app is Wispr Bro everywhere a user or macOS sees a name or version, without editing upstream identity files.
- **User-visible outcome:** the app, Dock, menu bar, tray, permission prompts, About and window titles say Wispr Bro with our icon; bundle id `com.wisprbro.desktop`; version `1.0.0-alpha.N` everywhere (tray label, What's New, diagnostics); minimum macOS 13.0 with runtime gates (DEC-22).
- **R2 rows closed:** E-61, E-62, E-63, E-92, E-211, E-213
- **Regression protected (HAVE):** E-94
- **R4 parts:** fork overlay config (R4 "Release pipeline", own); `{{appName}}` interpolation and i18n overrides (own, DEC-35).
- **Handy files touched:** none for identity (overlay). ⚠ `Cargo.toml` and `package.json` `version` lines (DEC-48). `tray.rs` (`fork::tray::extend` hook, first use: title/version item and tooltip). `Info.plist` usage strings only if the overlay cannot carry them ⚠. `package.json` scripts `wb:dev`/`wb:build` ⚠. The main window title "Handy" is replaced in `fork::on_ready`.
- **New fork files:** `src-tauri/tauri.fork.conf.json`; `src-tauri/icons-fork/*`; `src/i18n/locales/<lng>/fork.json` with `overrides` for every Handy key whose English text contains "Handy" (generated by `scripts/wb-brand-overrides.ts`) and the first `rust.*` keys (tray title, version label); `fork/i18n.rs`; `fork/tray.rs` (title/version only); WAV and log file name prefixes set in `fork::on_ready`.
- **Tests:** script test that every Handy key containing "Handy" has an override in all 26 locales; Rust test that the effective config has our identity; Rust test that `CARGO_PKG_VERSION` equals the overlay version; `fork/i18n.rs` test that every `rust.*` key resolves in all 26 locales and that the tray and app-menu strings contain no "Handy"; E-94 env-var test (`HANDY_DISABLE_UPDATER` and `WISPRBRO_DISABLE_UPDATER` both disable the updater).
- **Real-Mac acceptance:** clean Mac: the Microphone and Accessibility prompts show "Wispr Bro"; the tray tooltip shows our version; the app installs side by side with Handy without sharing settings or TCC entries.
- **Exit criteria:** no "Handy" string visible in the UI, tray or menus in English and two other locales (manual sweep with screenshots); identity guard (F-03) green.
- **Effort / depends on:** S / F-05.

### F-02 Signing, notarization, updater key, update channels and macOS release workflows
- **Goal:** we can publish signed, notarized, auto-updating macOS builds from our repo with our key, on a stable and a beta channel.
- **User-visible outcome:** a DMG that opens without warnings and updates itself from our GitHub releases.
- **Owner inputs (needed before the first run; CI fails while any is a placeholder):** GitHub owner (`<owner>`), HF org name (`<hf-org>`, created by the owner, DEC-34 mirrors uploaded by the owner or a token they issue), Homebrew tap repo name, Apple team id, Developer ID certificate, App Store Connect API key, updater key pair, repo secrets (R4 §7 A). The owner also disables the upstream workflows listed in §4.1 in the Actions UI.
- **R2 rows closed:** E-54, E-58, E-59, E-214, E-219
- **Regression protected (HAVE):** E-55, E-56, E-57
- **R4 parts:** tauri-action `@v0` (MIT, adopt); tauri-plugin-updater 2.10.1 (Apache/MIT, keep; `updater_builder().endpoints(...)` for the channel); R4 §7 ship checklist A-C.
- **Handy files touched:** none.
- **New fork files:** `.github/workflows/wb-release.yml`, `wb-build.yml` (macOS matrix: `macos-26` aarch64, plus `macos-latest` x86_64 cross-build if DEC-29 keeps Intel; `--config src-tauri/tauri.fork.conf.json`, version from the overlay, `tagName` + `releaseId`), `wb-verify` job (`codesign --verify --deep --strict`, `spctl -a -vvv -t exec` on the app, `spctl … -t open --context context:primary-signature` on the DMG, `xcrun stapler validate`, `--list-devices` smoke test natively on arm64; the x86_64 build's smoke test runs under Rosetta 2 on the arm runner, or is skipped with a recorded reason; from S-10 on also the `llama-server` binary). `wb-*` workflows trigger on `wispr-bro` only and never run signing on sync branches. The beta release is a rolling GitHub **prerelease** tagged `beta`, so `releases/latest` never points at it. The overlay sets no Windows `signCommand`, so the upstream Azure coupling never applies. `fork/update/{mod,channel}.rs`: Rust update check that picks the stable or beta endpoint from the fork setting and checks the signature against our pubkey; exposed to the fork debug panel. `docs/fork/RELEASING.md` (key custody: minisign key with two offline backups, rotation steps, secrets list).
- **Tests:** workflow dry run on a fork branch; tag-format check (`v1.0.0-alpha.N`, monotonic); channel endpoint selection unit test; the identity guard sees our pubkey and endpoint.
- **Real-Mac acceptance:** RS-12 (DMG stapled, `1.0.0-alpha.1 → alpha.2` update keeps permissions).
- **Exit criteria:** two consecutive internal releases published as drafts with the DMG(s), `.app.tar.gz` + `.sig`, one `latest.json` pointing at our repo; the alpha.1 install updates to alpha.2; a beta prerelease is ignored by a stable-channel install.
- **Effort / depends on:** M / F-01, F-07.

### F-03 CI guards
- **Goal:** the privacy, identity, licence and thin-layer promises are enforced by CI, not memory, and pass on untouched upstream files.
- **User-visible outcome:** none directly; PRs that break a promise fail.
- **R2 rows closed:** E-212
- **Regression protected (HAVE):** E-73
- **R4 parts:** `cargo-about 0.9.3` (MIT/Apache, adopt, dev); `eslint-plugin-i18next` + `scripts/check-translations.ts` (Handy, extend); addendum §4 analytics SDK list.
- **Handy files touched:** none (`scripts/check-translations.ts` is wrapped by `scripts/wb-check-translations.ts`).
- **New fork files:** `.github/workflows/wb-ci.yml`, `.github/workflows/wb-egress.yml`; `ci/allowed-hosts.txt`, `ci/upstream-hosts-baseline.txt`, `ci/inert-hosts.txt`, `ci/telemetry-packages.txt`, `ci/generated.txt`, `ci/hooks.txt`, `ci/upstream-bound.txt`; `scripts/wb-guard-{identity,plumbing,hooks,hosts,bundle-hosts,telemetry,spdx}.ts`; `scripts/wb-egress/` (pf anchor, scripted session, local HF mock); `about.toml`.
- **Tests:** each guard has a fixture that must fail (a planted `posthog-js` import, a planted URL in a fork file, a new host in an upstream file, a missing SPDX header, the upstream bundle id, an extra line in a hotspot, an `AppManifest` in `build.rs`) and pass after removal; a fixture with `unicode-segmentation` passes the telemetry guard; the current upstream tree passes the baseline guard.
- **Real-Mac acceptance:** `wb-egress.yml` passes on the macOS runner with the alpha build.
- **Exit criteria:** all `wb-ci` guards run on every PR in under 5 min and are required checks on `wispr-bro`; `wb-egress` green nightly.
- **Effort / depends on:** M / F-01, F-05.

### F-04 GPL compliance and notices
- **Goal:** the repo meets the GPL-3.0 obligations from day one.
- **User-visible outcome:** LICENSE and an Acknowledgements file in the repo; About shows "GPL-3.0" and a source link (page polish in S-39).
- **R2 rows closed:** E-218
- **R4 parts:** addendum §4 checklist (W-30); `cargo-about` + npm licence list + manual C/C++ list (W-26).
- **Handy files touched:** `LICENSE` replaced by GPL-3.0 text; Handy's MIT text moved to `LICENSES/MIT-Handy.txt` (keep the notice); README licence paragraph ⚠ (small).
- **New fork files:** `NOTICE` (ported sources with repo, commit, files, licence; AGPL-marked files incl. `fork/diagnostics/sanitizer.rs`; Apache NOTICE texts; OFL fonts; model attributions, incl. CC-BY models only if DEC-26 allows them as defaults or they are downloaded); `CONTRIBUTING-WB.md` with DCO sign-off; `.github/workflows/wb-dco.yml`; `docs/fork/LICENSE-REGISTER.md` (R4 §5 register kept current; models never default per DEC-27); `scripts/wb-notices.ts` generating `THIRD-PARTY-NOTICES.md`.
- **Tests:** DCO check on PRs; notices generator runs in CI and fails on unknown licences.
- **Exit criteria:** release workflow attaches `THIRD-PARTY-NOTICES.md` and the source tarball to every release.
- **Effort / depends on:** S / F-03.

### F-05 Fork scaffolding
- **Goal:** one namespace with working plumbing (init hooks, commands, events, React root, i18n namespace, translate script, capabilities, debug panel) so later slices only add files.
- **User-visible outcome:** none (an internal "fork ready" log line and a hidden fork debug panel, route `/fork/debug`, opened with ⌘⇧D in debug builds or with `--debug`).
- **R2 rows closed:** E-95
- **R4 parts:** Tolkovo `tolkovo/mod.rs` product-layer pattern (MIT, port); tauri-specta rc.21 (in tree, adopt; second builder).
- **Handy files touched:** ⚠ `lib.rs` (two lines: `.plugin(fork::init())` in the plugin chain; `.invoke_handler(fork::compose_invoke(invoke_handler))` at `lib.rs:1132`), `main.rs` (one `fork::early_init()`), `src/i18n/index.ts` (one `registerForkI18n`), ⚠ `Cargo.toml` / `package.json` (fork blocks).
- **New fork files:** `src-tauri/src/fork/mod.rs` (`init()` plugin: state-only `setup`, `on_event` that calls `on_ready` on `RunEvent::Ready`; `compose_invoke`, which builds the fork specta builder, exports `src/fork/bindings.ts` under `cfg(debug_assertions)` exactly as Handy does at `lib.rs:798-804`, and routes `fork_*` commands); `fork/events.rs` (`Fork*` event types); `fork/commands/mod.rs` (sample `fork_ping` and `fork_model_status`, which reads Handy's `ModelManager`); `src-tauri/capabilities/fork.json`; `src/fork/index.ts`, `src/fork/i18n.ts`, `src/fork/bindings.ts`, `src/fork/debug/DebugPanel.tsx` (mount point for panels that land before their home page), `src/fork/ui/` (Radix-based primitives start here); `scripts/wb-translate-missing.ts` (W-25, pluggable backend per §5.6; first backend "copy English and flag `untranslated`", the `llama-server` backend added when S-10 lands); `docs/fork/ARCHITECTURE.md` (init order, routing, event naming, with the verified line references from §4.1); SPDX template.
- **Tests:** Rust: `fork_model_status` called through the composed handler after `Ready` returns Handy's model state (proves on_ready ordering), and returns `NotReady` when called before; a Handy command still resolves through the composed handler; no Handy command name starts with `fork_`; a fork event round-trips through `mount_events`. Playwright smoke test (fork config) calling one fork command through mocked IPC. Build check that Handy's `bindings.ts` is untouched by fork codegen. Headless `--list-models` run starts no fork windows.
- **Exit criteria:** `bun run wb:dev` runs with the plugin; `fork_ping`, `fork_model_status` and one `Fork*` event round-trip in dev **and** in a release build (proving the ACL behaviour in release); routing and init rules written in `docs/fork/ARCHITECTURE.md`.
- **Effort / depends on:** M / none.

### F-06 `fork.db` and migrations
- **Goal:** a fork database with migrations, WAL and FTS ready for every later slice.
- **User-visible outcome:** none.
- **R2 rows closed:** C-208, C-209
- **Regression protected (HAVE):** E-71
- **R4 parts:** rusqlite 0.37 `bundled` (MIT, in tree) + rusqlite_migration 2.3 (Apache-2.0, in tree).
- **Handy files touched:** none (hooks into `managers/history.rs` arrive in S-09).
- **New fork files:** `fork/db/mod.rs` (open, pragmas, single connection, migration runner, backup-before-migrate, upsert helpers), `fork/db/migrations/0001_init.sql` (empty skeleton tables from §4.3 that S-09 fills), `fork/db/fts.rs` (trigram helpers with `LIKE` fallback for < 3 chars).
- **Tests:** fresh-create, upgrade from every prior version, downgrade refusal message, WAL concurrency (writer + reader threads), Handy `history.db` `user_version` unchanged (E-71), delete removes `-wal`/`-shm`.
- **Exit criteria:** tests green; a corrupted `fork.db` is moved aside and recreated with an inbox entry (or a log line before S-18), never crashing the app.
- **Effort / depends on:** S / F-05.

### F-07 Fork settings store and product defaults
- **Goal:** fork settings exist outside `AppSettings`, and product values are applied correctly on every start.
- **User-visible outcome:** first-run defaults per DEC-14 (sounds on, Dock icon, launch at login on, theme System, log level Info, update checks on).
- **R2 rows closed:** none (enabler slice)
- **Regression protected (HAVE):** E-70
- **R4 parts:** tauri-plugin-store (in tree); Tolkovo `apply_product_settings` (MIT, port).
- **Handy files touched:** none (uses Handy's existing settings commands).
- **New fork files:** `fork/settings.rs` (struct incl. onboarding state, defaults, migrations, `apply_product_settings` with the `ENFORCED` and `FIRST_RUN_DEFAULTS` lists of §4.4), `src/fork/settings/useForkSettings.ts`, `src/fork/settings/store.ts`.
- **Tests:** defaults, migrations, salvage of a corrupt file, `apply_product_settings` idempotence; `ENFORCED` values re-applied after a manual change; each `FIRST_RUN_DEFAULTS` value changed by the user persists across relaunch; a stored cloud provider becomes `local` while a loopback `custom` provider stays.
- **Exit criteria:** changing a fork setting persists across relaunch; Handy's own settings file is changed only through `apply_product_settings`.
- **Effort / depends on:** S / F-05.

### F-08 Privacy cleanups
- **Goal:** the app keeps the §1 promise before any feature work.
- **User-visible outcome:** no cloud providers anywhere; logs at Info with no settings dump; nothing contacts `blob.handy.computer`; a strict CSP on every page.
- **R2 rows closed:** D-69, E-72, E-74, E-78
- **Regression protected (HAVE):** D-70
- **R4 parts:** Voquill `utils/log_sanitizer.rs` (AGPL-3.0, port; file marked AGPL) for redaction; R4 "Crash / diagnostics" row (own).
- **Handy files touched:** ⚠ `settings.rs` (`default_post_process_providers` hook; log-level and settings-dump hooks); `managers/model.rs` / downloader (mirror fallback removed through a hook returning `None` for mirror URLs); `llm_client.rs` (neutral headers, loopback-only check); ⚠ `Cargo.toml` (remove `rdev`); ⚠ `package.json` (remove `@tauri-apps/plugin-sql`); `main.rs` (panic hook installed by `fork::early_init`). CSP only through the overlay (§4.11).
- **New fork files:** `fork/llm/providers.rs` (default list: `local` placeholder until S-10, `custom` loopback-only), `fork/net.rs` (`is_allowed_host`, loopback check), `fork/diagnostics/mod.rs` (level, allow-listed summary), `fork/diagnostics/sanitizer.rs` (`SPDX-License-Identifier: AGPL-3.0-only`, Voquill port, listed in NOTICE), `fork/models/license_filter.rs` (hide non-commercial Moonshine, never-default list), CSP block in `tauri.fork.conf.json`, `src/fork/debug/CspSmoke.tsx`.
- **Tests:** settings read returns no cloud provider; a stored `openai` provider from an old file is migrated to `local`; a non-loopback custom URL is refused by the backend; log output at startup contains no custom words or prompts; downloader never builds a `blob.handy.computer` URL (this test is the `inert-hosts.txt` proof for that host); sanitizer test vectors.
- **Real-Mac acceptance:** in a real `wb:build`: the CSP smoke panel plays a recording through `convertFileSrc`, renders an element to a `data:` and a `blob:` image, and calls a fork command, with no CSP violation in the console; Handy's own history page still plays audio. 10-minute session with `nettop`/Little Snitch: only `github.com` (update check) contacted.
- **Exit criteria:** network guards green; egress check passes.
- **Effort / depends on:** S / F-05, F-07.

### F-09 Upstream-sync workflow
- **Goal:** upstream Handy changes arrive weekly as a reviewable PR.
- **User-visible outcome:** none.
- **R2 rows closed:** none (enabler slice)
- **R4 parts:** own `gh` workflow (R4 "Upstream sync").
- **New fork files:** `.github/workflows/wb-upstream-sync.yml`, `docs/fork/UPSTREAM-SYNC.md` (the §4.13 checklist and hook list).
- **Tests:** manual dispatch creates a PR against a test branch; the hook-size guard runs on it.
- **Exit criteria:** first real sync PR merged with all guards green; `git diff upstream/main --stat` lists only files in `ci/hooks.txt` plus fork paths.
- **Effort / depends on:** S / F-03.

### F-10 Test and eval harness
- **Goal:** every later slice can add unit, Playwright and real-Mac tests, and the cleanup eval runs locally with reproducible metrics.
- **User-visible outcome:** none.
- **R2 rows closed:** none (enabler).
- **R4 parts:** Playwright (in tree); Voquill `test/evals/post-processing-eval.test.ts` cases (AGPL, port as data); Disfl-QA and AMI-disfluent (CC-BY-4.0 / Apache, eval only, never shipped).
- **New fork files:** `playwright.fork.config.ts` and `tests/fork/` (fixtures with a mocked Tauri IPC for fork commands and events); `src-tauri/src/fork/testutil.rs` (fake clock, fake AX, fake pasteboard, fake coordinator, fake network traits); `docs/fork/MAC-TEST-SCRIPT.md` (skeleton of §11.3); `eval/cleanup/` with `README.md` (metric definitions, §11.1), `items.jsonl` (≥ 200 adversarial items: questions, commands, lists, code, names, numbers, URLs, non-English, plus Disfl-QA/AMI samples; each item labelled per §11.1 by two people), runner `scripts/wb-eval-cleanup.ts` that calls a running `llama-server` at temperature 0 and computes every metric deterministically (no LLM judge), reporting pre-guard and post-guard numbers and latency.
- **Tests:** harness self-tests; metric functions tested on hand-made items with known answers.
- **Exit criteria:** the eval runner produces a report for one model in under 15 min on an M1; inter-labeller agreement recorded.
- **Effort / depends on:** M / F-05.

---

## 7. M0.5 Real-Mac spikes

Run on the reference machines (§5) with signed builds from F-02 where noted. Each spike writes `docs/fork/spikes/RS-xx.md` with results and the decision. A spike that fails switches its gated slice to the listed fallback; no slice starts before its gating spike has a written result. Intel runs apply only if DEC-29 keeps Intel.

| ID | Check (R2 RC / R4 RT) | Procedure | Pass criteria | Gates | Fallback if it fails |
|---|---|---|---|---|---|
| RS-01 | fn / Globe suppression with two taps, chords, mic indicator (RC-02, RT-18) | Test binary with Handy's `HotkeyManager` running as shipped **plus** the fork's own `KeyboardListener::new_with_blocking` with the §4.8 dynamic blocking set. On macOS 14, 15 and 26, for each "Press 🌐 key to" value (Do nothing, Change input source, Show emoji & symbols, Start dictation): hold fn 1 s ×10, quick tap (< 150 ms) ×10, double-tap ×10. 100 fn chords (fn+←/→/↑/↓, fn+Delete, fn+F1..F12) in TextEdit and Finder, checking each chord's native function (Home/End/PgUp/PgDn, forward delete, F-key). The F5 dictation key bound as `fn+dictation`. Ctrl+Opt on an external non-Apple keyboard. Mic indicator: watch the orange indicator during the 100 chords and the quick taps with DEC-47 timing. First-word clipping: 50 natural takes starting to speak at once. Both taps alive after 10 sleep/wake cycles | No Globe action (emoji picker, input switch, dictation) on tap, hold or double-tap for all four values; 100/100 chords keep their native function with 0 starts; F5 suppressed; Ctrl+Opt works on an external keyboard; orange indicator never shown during chords or quick taps; ≤ 1 clipped first word in 50 takes; both taps deliver after every wake | S-01, S-29a shortcut step | Globe action fires on some macOS versions: **DEC-44** (ask consent in onboarding, set the Globe setting to "Do Nothing" via the private API with crash-safe restore; guidance sheet if declined). Chords lose their native function: on that macOS version stop blocking the fn press (keep blocking fn+Space and Esc) and use the DEC-44 path there for the Globe action. Clipping > 1/50: lower `T_show` to 100 ms and re-run the indicator count; if both cannot pass, keep the indicator rule (A-45) and accept the clip, recorded in the spike report |
| RS-02 | Tap health after sleep, revoke, re-sign (RC-14 via RT-19, A-208) | Signed build. (a) Sleep 10 min, wake, press fn within 2 s; ×10. (b) Remove the app from Accessibility while running; press fn; re-add; press fn. (c) Install a re-signed build over it. (d) Lid close with external display. Log tap state of **both** taps, axwatch notifications, `AXIsProcessTrusted` | Both taps deliver within 2 s of wake in 10/10; loss is detected within 2 s in (b)/(c); recovery within 5 s of re-grant without relaunch; axwatch notifications arrive in the signed build | S-02 | If axwatch notifications are withheld: re-check on wake, app activation and every pill show (no tap polling); if re-grant needs relaunch, show "Relaunch to finish" with a one-click relaunch |
| RS-03 | Pill panel: first click, hover, Space, capture, displays, idle memory (RC-01, RC-03, RT-13..RT-17, RC-10) | Prototype `fork-pill` with nspanel 2.1.1 + Tolkovo setup, with Handy's `recording_overlay` not created. With TextEdit focused: click ✕ and ✓ ×20 each, then type: does TextEdit keep focus? With and without `_setPreventsActivation:`. Hover via a native tracking area ×20. Space switch and full-screen app. QuickTime/Zoom/OBS capture with `set_content_protected`. Sleep/wake, external display unplug/replug, mixed-scale displays (Retina + 1× external). 8 h idle with a 100-take script; sample RSS of the app and of every WebKit process, and CPU, every minute | First click acts in 20/20 without focus loss; tooltip appears within 200 ms in 20/20 hovers with TextEdit frontmost; re-shows on every Space and after wake and display changes, placed correctly on both scales; capture hiding works in ≥ 2 of 3 recorders; pill RSS growth ≤ 50 MB; combined idle RSS of all processes ≤ 250 MB (§5.2); idle CPU ≤ 0.5 % | S-03 | Focus loss: add `_setPreventsActivation:` (DEC-28); hover: tracking area plus CSS fallback; memory growth: native pill (anarlog Swift pill or Voquill `rust_macos_pill` as reference) for the resting state with the webview only while active |
| RS-04 | AX matrix incl. replace-selection and URL (RC-04, RT-20..RT-23) | Prototype `fork/ax` CLI. Apps: Slack, VS Code, Cursor, Discord, Notion, Chrome (Gmail, Docs), Arc, Brave, Edge, Safari, Firefox, Mail, Messages, Word, TextEdit, Terminal, iTerm2. For each app, ≥ 20 focus events across different fields: focused element role/subrole, editable verdict, caret window, selected text, `AXSelectedText` set, bounds, window title, document URL; with and without the `AXManualAccessibility` lease; latency at 0.1 and 0.25 s timeouts; `frontmostApplication` vs `AXFocusedApplication` freshness | Editable verdict correct in ≥ 95 % of focused text fields and 0 false "no text field" when a text field is focused (fail-open counts as pass); selected text readable in ≥ 90 % via the chain; URL host readable in Chrome, Arc, Brave, Edge, Safari; p95 AX call ≤ 100 ms | S-05, S-19b, S-20, S-21, S-34, S-36 | Editable detection weak in an app family: treat that family as "always paste" (fail-open). Replace-selection unreliable: always replace by clipboard paste (default anyway). URL missing: window-title site name. Selection unreadable: menu-Copy then ⌘C sentinel |
| RS-05 | Clipboard fidelity and paste hygiene (RC-05, RT-24, Chromium) | Prototype `fork/clipboard`. Before each paste, put on the clipboard: plain text, RTF from TextEdit, HTML from Safari, a file URL from Finder, an image, multi-item (two files). Paste into 10 apps. Compare bytes after restore. Run Maccy, Raycast clipboard history, Paste: do they record dictated text? Paste-last with ⌃⌘ still held: what arrives? Chrome version check for 155/156. Paste with a CJK IME (Japanese Kana, Pinyin) active | Byte-identical restore for all listed types; no clipboard manager records the dictated text; held ⌃⌘ never turns ⌘V into ⌃⌘V after the modifier wait; paste lands with the IME active (or verification reports `unverified`) | S-04, S-06 | Manager records it: add the manager-specific ignore markers the manager documents; if still recorded, document it on the privacy page. Restore loss for a type: exclude that type from snapshot and say so. IME swallows paste: card |
| RS-06 | `llama-server` build, signing, transport, memory, latency, contention, abort, Intel (RC-11, RT-01, 02, 03, 06, 09, 10, 11, 12) | CI-style static build for 13.0, signed as `externalBin`, notarized, launched on a clean Mac (macOS 13 and 26). Transport: `/health` on `--host <path>.sock`; `--offline` accepted. Per candidate model (Qwen3.5-0.8B/2B, Qwen3-1.7B/4B-2507, Muesli 2B, Gemma-4-E2B challenger only) at Q4_K_M: cold start to `/health`, prefill and generation tok/s, peak RSS with `-c 4096 --cache-ram 256`, with the ASR model loaded, on 8 GB and 16 GB; `-cms` prefix reuse; cleanup while ASR Metal is busy; Esc frees the slot; cold ASR + cold LLM with ordered loads on M1 8 GB (§4.5); Intel CPU speed (DEC-29) | Signed binary passes Gatekeeper on 13 and 26; Unix socket serves `/health` (else DEC-49 loopback fallback, also tested); 2B model Light p95 ≤ 2.2 s end-to-end on M1 8 GB with combined RSS ≤ 3.2 GB; cold-cold extra ≤ +2.5 s p95; abort ≤ 100 ms; no crash under contention | S-10, S-12a, DEC-03, DEC-29, DEC-49 | 8 GB misses budget: Qwen3.5-0.8B on 8 GB (if quality passes RS-07) or Apple FM where available; else Light becomes deterministic-only on that tier with Medium/Transforms still offered. Signing fails: build our own small binary on llama.cpp's server code (same licence). Socket fails: loopback TCP with key |
| RS-07 | Cleanup quality bake-off (RT-04, RT-05, RT-08) | F-10 eval set × models from RS-06 × prompts (OpenWhispr seed, FreeFlow, SpeakoFlow, trimmed) × JSON schema vs plain; guards on; metrics as defined in §11.1 | Light (post-guard): content damage ≤ 2 %, answer rate ≤ 0.5 %, filler removal ≥ 90 %, language kept ≥ 99 %; Medium: content damage ≤ 4 %; guard catch rate ≥ 90 % of the damaging pre-guard outputs | S-12a, S-19a, S-21, DEC-03 | No model passes Light: ship Light as deterministic + LLM only for punctuation/casing with a stricter guard; Medium and styles marked Beta |
| RS-08 | Apple Foundation Models (RT-07) | macOS 26 Apple Silicon: cold and prewarmed latency for Light on 50 words; refusal rate default vs permissive guardrails; `rateLimited` as a background app; real context size; eval set quality (§11.1 metrics) | p95 ≤ 1.2 s prewarmed; refusals ≤ 1 %; every RS-07 Light metric within 1 percentage point of the RS-07 winner | S-12b (Apple path default or fallback-only) | Use Apple FM only while the GGUF downloads |
| RS-09 | ASR models: starter quality, time to first text, speed and RSS (RC-09, RT-31, RT-37) | Clean Mac with Network Link Conditioner at 10 and 50 Mbit/s: Welcome → starter ready → first take. 50 sample takes (en, es, de, ja, zh) per candidate on M1 8 GB, M 16 GB and Intel (DEC-29). Candidates without CC-BY: whisper-base, whisper-turbo, Moonshine en base. CC-BY candidates (for DEC-26 option 1): Parakeet TDT 0.6B v3, Parakeet unified en, parakeet-110m. Measure WER, RTF, latency for 10 s takes, RSS, reload after unload; Metal and CPU | Starter ready within §5.3; starter WER on clean English ≤ 15 %; the per-language policy table meets §5.1 latency on each machine class, produced twice (with and without CC-BY models) | S-07a, S-07b policy table | Starter too weak in a language: prefer a bigger starter for that language only if it stays ≤ 150 MB (Moonshine en base; parakeet-110m only under DEC-26 option 1); else keep whisper-base and show "improving accuracy…" until the swap |
| RS-10 | macOS 13 baseline and notifications (RT-30, Metal on 13) | Signed build on macOS 13.x (Apple Silicon; Intel if DEC-29): launch, permissions, transcribe-cpp Metal, nspanel pill, `llama-server` Metal, tauri-plugin-notification 2.4.0 delivery + click callback + action button ("Copy latest") + permission prompt timing (also on 15 and 26) | Everything runs on 13; notifications deliver, the click opens the right page and the action button runs its action | S-18, DEC-22 | Metal failure on 13: CPU fallback on 13 only; notification plugin fails: ~100 lines on `objc2-user-notifications` (in tree) |
| RS-11 | Small checks: mic loss, mouse buttons, ⌥ characters (RC-06, RC-07, RC-13) | Unplug USB mic and drop AirPods mid-take; revoke Microphone permission mid-take; shortcut recorder with middle/side mouse buttons; global ⌥1, ⌥2, ⌥O, ⌥S, ⌥M bound: does typing "¡", "™", "ø", "ß", "µ" still work when Transforms are off/on? | Mid-take loss keeps audio so far; permission revoke is detected and audio so far kept; mouse buttons captured; with Transforms off nothing is swallowed | S-06, S-08, S-21, S-24 | Mouse buttons not captured: keyboard-only Press Enter; ⌥ keys swallowed when on: warn in the Transforms toggle and suggest ⌃⌥ alternatives |
| RS-12 | Updater and DMG (RC-12, RT-35, RT-36) | After F-02: install alpha.N from DMG on a clean Mac, grant permissions, publish alpha.N+1, update from the footer and from the tray; publish a beta prerelease and check a stable install ignores it | DMG stapled; update applies; Mic and Accessibility survive; stable ignores beta | F-02 exit, S-28, S-39 | Staple the DMG in `wb-release.yml` explicitly |
| RS-13 | ITN need (RT-39) | 20 dictations with numbers, money, dates, times, measures through the chosen default models (en + 2 other languages) | If ≥ 20 % of number expressions come out in spoken form, ITN is worth shipping | S-33 (DEC-40) | Close D-14 as not needed |
| RS-14 | Secure Event Input (R3 §3, A-205) | Signed build with both taps. Enable Secure Input three ways: password field focused in Safari, Terminal "Secure Keyboard Entry", iTerm2 secure input. For each: fn hold → take; double-tap fn → lock; fn+Space; Esc while recording (tap) and with the Carbon shadow registered (§4.8); Ctrl while fn held (Command Mode on and off); ⌃⌘V; whether the Globe action is still suppressed; pill message shown | fn hold and double-tap work; Esc cancels through the Carbon shadow in all three cases; the pill explains that fn+Space is unavailable; Globe stays suppressed; Esc pressed in the password field while idle is never swallowed | S-01, S-02 | Carbon shadow fails: ✕ on the pill is the cancel under Secure Input and the pill says so; §9's Esc criterion reads "except under Secure Input" |

Spikes run in parallel with M0 where they need no signed build (RS-01, RS-03, RS-04, RS-05, RS-07, RS-09, RS-11, RS-13 can start on dev builds; RS-02, RS-06, RS-10, RS-12, RS-14 need F-02).

---

## 8. M1 slices

Order: core loop (S-01..S-08, with S-07a and S-07b), data and text intelligence foundations (S-09..S-12b), the Hub and its features (S-13..S-28; S-07c is built right after S-18), onboarding (S-29a, S-29b, S-30), tutorials (S-31, S-32), late features and polish (S-33..S-38), release (S-39). Each slice ends in something a user can try. Every slice also: adds its strings (our own wording, §1) to `en/fork.json` and runs `scripts/wb-translate-missing.ts` (available from F-05); passes the CI guards; updates `docs/fork/MAC-TEST-SCRIPT.md` with its real-Mac checks. Build order follows the dependency lists, not the numbering (for example S-03 is built before S-02, and S-16 before S-15b). No outcome, test or exit criterion relies on a slice that is not among its listed dependencies; when a panel lands before its home page exists, it mounts on the fork debug panel (F-05) and moves unchanged later.

### Core loop

### S-01 Hotkey engine: fn grammar on the fork's own tap
- **Goal:** Wispr's hotkey grammar on fn, with no accidental starts and no mic flash, as a tested state machine on our own blocking listener.
- **User-visible outcome:** hold fn to dictate; double-tap fn or fn+Space for hands-free; Esc cancels even with fn held; fn+arrow, fn+Delete and fn+F-keys keep their normal function and never insert text (DEC-54), and never light the mic indicator when the key follows fn within `T_show`; Ctrl+Opt fallback for keyboards without fn.
- **Tap:** `fork/shortcut/listener.rs` runs `KeyboardListener::new_with_blocking` with the dynamic blocking set of §4.8 on its own thread; Handy's manager no longer registers fork-owned bindings (`owns()` hook in `shortcut/mod.rs`).
- **Grammar (timings are constants in one file):** `T = 500 ms`, `T_show = 150 ms`.
  1. fn down alone → pending. If the mic stream is warm (≤ 30 s after a take), capture starts now into a pre-roll buffer. After `T_show` of uninterrupted hold, the take starts: the prepared input unit starts (DEC-53; cold open if it could not be prepared), the pill and start sound appear.
  2. Another non-modifier key (except Space) while fn is held in a push-to-talk press, at any time (DEC-54) → interrupted: discard silently (pre-roll dropped, mic never started before `T_show`; after `T` the transcript goes to History, not pasted), pass the key through. Esc cancels; Ctrl turns the take into a Command Mode take only if Command Mode is on (DEC-31; behaviour in S-23), otherwise Ctrl passes through and the take stays plain dictation.
  3. fn up before `T` → tap: no paste; wait `T` for a second press. Second press within `T` → hands-free lock; the mic opens at once on the second press (audio from a first press held ≥ `T_show` is kept). No second press → cancel, nothing saved.
  4. fn up at or after `T` → stop and insert (push to talk).
  5. Locked: a fn press stops and inserts; a press within `T` of locking (third quick press) cancels.
  6. fn+Space at any time while fn is held → lock; the mic opens at once if not open; Space is swallowed (`{FN, Space}` is in the blocking set only while fn is down).
  7. Esc while recording or processing → cancel (matches with or without fn). `{Esc}` is in the blocking set only in those states, so idle Esc is never swallowed. Cards, toasts and the spotlight observe Esc and never block it (Esc registry, `actions.rs`).
  8. Hands-free silence auto-stop after 120 s of continuous silence, warning at 100 s; hard stop at 30 min with a warning at 28 min (DEC-32).
  9. Stuck fn recovery: if the tap thinks fn is down but `CGEventSourceFlagsState` says up for 1 s, synthesize the release.
  Fallback profile (Ctrl+Opt hold, Ctrl+Opt+Space lock) uses the same machine; it is activated from onboarding or the hotkeys panel, not by default (DEC-44 may change this on some macOS versions).
- **R2 rows closed:** A-22, A-23, A-24, A-25, A-29, A-30, A-31, A-32, A-35, A-201, A-202, A-203
- **Regression protected (HAVE):** A-34, A-45, A-206, A-67, E-86
- **R4 parts:** handy-keys 0.3.4 `KeyboardListener::new_with_blocking` and `BlockingHotkeys` (MIT, adopt, in tree, no fork); anarlog `crates/shortcut-macos` `processor.rs`, `decision.rs`, `tests/processor.rs` (MIT, port); Hex legacy `HotKeyProcessor` (MIT, credit); MacParakeet `FnKeyStateMachineTests`, `HotkeyGestureControllerTests` and FluidVoice `ModifierOnlyShortcutFlagsDecision` tests (GPL-3.0, port as test vectors); OpenWhispr `FN_INTERRUPTED` rule (MIT, port); hex `keyboard.rs` chord breaker (MIT, port); Yap F5 Dictation-key suppression (MIT, port). OpenWhispr `TISUpdateFnUsageType` (MIT, port) only under DEC-44 option 2.
- **Handy files touched:** `shortcut/mod.rs` (`owns()` first lines of `register_shortcut`/`unregister_shortcut`), ⚠ `transcription_coordinator.rs` (`external_command`, separate MIT-clean commit), `input.rs` reused for layout-aware labels (no edit).
- **New fork files:** `fork/shortcut/{mod,listener,blocking,processor,decision,bindings,labels,actions}.rs` (`actions.rs` is the action registry: fixed ids plus runtime ids for user transforms, Scratchpad and later Notetaker, and the Esc registry); `fork/shortcut/tests/*.rs`.
- **Tests:** table-driven tests with a fake clock for every rule above and the ported vectors (target ≥ 150 cases), including: Ctrl with Command Mode off passes through; a chord within `T_show` never opens a cold mic; blocking-set contents per state; integration test with a fake coordinator (start/stop/lock/cancel order, one press queued while busy — A-206); bindings: ≤ 4 per action, unset allowed, per-binding activation, conflict detection; labels on AZERTY/QWERTZ/Dvorak; CLI `--toggle-transcription` and `--cancel` still work (A-67, E-86).
- **Real-Mac acceptance:** RS-01 and RS-14 outcomes applied; 20 PTT takes, 20 double-tap locks, 20 fn+Space locks; 100 fn+arrow/fn+F-key/fn+Delete chords with 0 insertions and native function kept, half typed fluently (key within `T_show`: indicator never shown, A-45) and half with fn held first (DEC-54: take dropped); 50 quick taps with no indicator; Esc with fn held cancels; external keyboard with Ctrl+Opt; under Secure Input: fn hold and double-tap work.
- **Exit criteria:** all tests green; 0 insertions in the chord run and 0 indicator flashes for fluent chords; fn-down → capture running within the §5.1 rows.
- **Effort / depends on:** L / F-05, F-07, RS-01, RS-14.

### S-02 Hotkey self-healing, permission health and Secure Input
- **Goal:** the hotkey never dies silently, and Secure Input is explained where the user looks.
- **User-visible outcome:** if the shortcut stops working (Accessibility revoked, re-signed app, failed tap), the pill and tray say so within 2 s with a "Fix" action; it comes back on its own after re-grant and after wake. Under Secure Input the pill names the app holding it (when known), says fn+Space is unavailable and that Esc or ✕ still cancels.
- **R2 rows closed:** A-208
- **Regression protected (HAVE):** A-205, A-207, B-17
- **R4 parts:** own fix to `HandyKeysState::new` (W-06, upstream-bound, MIT-clean); axwatch 0.1.0 (MIT/Apache, vendor); VoiceInk `ShortcutMonitor.swift` recovery triggers (wake, display wake, session active, missed release) (GPL-3.0, port triggers); Yap "reset and re-grant" guide and `SecureInput.swift` holder lookup (MIT, port); tauri-plugin-macos-permissions (in tree); Handy `secure_input.rs` Carbon shadow (MIT, keep).
- **Handy files touched:** `shortcut/handy_keys.rs` (manager built inside its thread; `Result` returned by oneshot so the persisted fallback runs); `secure_input.rs` (one hook: `fork::shortcut::secure_input_bindings`, so the Carbon shadow covers the fork's keyed bindings and its covered/uncovered lists describe them); `tray.rs` (warning badge via the existing Secure Input path).
- **New fork files:** `fork/shortcut/health.rs` (watches both taps), `fork/vendor/axwatch/`, `src/fork/pill/states/ShortcutLost.tsx`, `src/fork/pill/states/SecureInput.tsx`, `src/fork/settings/FixShortcutSheet.tsx` (picture of the Accessibility pane, Open Settings, Reset and re-grant steps).
- **Tests:** unit: tap-creation failure propagates; health state machine (Healthy → Lost → Recovering → Healthy) for each tap with fake notifications; probe tap stub; `secure_input_bindings` drops fork-owned ids and adds fork keyed bindings; the Secure Input diagnostic lists fork bindings.
- **Real-Mac acceptance:** RS-02 and RS-14 procedures on the signed build pass.
- **Exit criteria:** RS-02 criteria met; no polling of `CGEventTapIsEnabled` in the code; A-205 diagnostic shows the fork's bindings.
- **Effort / depends on:** M / S-01, S-03, RS-02, RS-14.

### S-03 Flow Bar (resting pill and states)
- **Goal:** Wispr's Flow Bar as the single, always-visible entry point.
- **User-visible outcome:** after setup, a thin bar rests at the bottom centre of the display under the cursor (top centre as an Advanced option, A-17). Hover shows "Hold fn to dictate". Click starts hands-free dictation without stealing focus; click again or ✓ stops; ✕ cancels. While held: waveform capsule driven by live mic levels. While locked: ✕, waveform, ✓. Mic label "Mic: MacBook Pro Microphone" for 1.5 s at start. Processing: "Transcribing…" / "Cleaning up…". Messages ("No speech heard", "Mic looks muted", "Inserted without cleanup") and errors with a Fix action. Right-click → "Hide for an hour". Drag along the bottom edge (position remembered per display). Setting "Always show the bar" (default on).
- **R2 rows closed:** A-01, A-02, A-04, A-05, A-06, A-10, A-12, A-13, A-14, A-17, A-210, A-212, A-66
- **Regression protected (HAVE):** A-07, A-08, A-09, A-11, A-18, A-19, A-52, A-68
- **R4 parts:** tauri-nspanel 2.1.1 (MIT/Apache, adopt; bump); Tolkovo `tolkovo/pill.rs:183-260,476-565,736-790` (MIT, port); anarlog `plugins/overlay` click-through (MIT, port only if RS-03 needs it); MacParakeet Idle Pill spec (GPL, reference for hover sizes); Wispr docs 1790396454 (behaviour).
- **Handy files touched:** `overlay.rs` (five first-line hooks, §4.2: `emit_levels`, `show_overlay_state`, `emit_recording_ready`, `hide_recording_overlay`, and the skip in macOS `create_recording_overlay`); ⚠ `actions.rs` (`on_record_start` hook updates pill state); `vite.config.ts` (`forkInputs` spread); `RecordingOverlay.tsx` untouched (Handy overlay never created).
- **New fork files:** `fork/pill/{mod,panel,geometry,placement,states,levels}.rs`; `src/fork/pill/{index.html,main.tsx,Pill.tsx,states/*.tsx,pill.css}`; `src/fork/pill/geometry.gen.json` (generated from Rust); `src/fork/vite-inputs.ts`; capability entry for `fork-pill`.
- **Tests:** Rust placement tests (one/two displays, mixed scale, Dock bottom/left/right/autohide, notch, full-screen Space, display unplugged, top position); geometry JSON in sync (build fails otherwise); a full take with no `recording_overlay` window (Handy's show/hide/emit paths tolerate the missing window); levels reach `fork-pill` with Handy's overlay disabled; Playwright snapshot of every state with mocked events in light/dark and reduced motion; Esc/✕/✓ wiring; "Hide for an hour" timer.
- **Real-Mac acceptance:** RS-03 criteria on the real slice; click-to-start in TextEdit, Slack, Chrome inserts into that app (keeps focus); the waveform moves with speech; bar follows the cursor's display within 1 s; idle 8 h memory run.
- **Exit criteria:** RS-03 criteria hold; resting bar hidden until onboarding ends while active states show during practice (DEC-38).
- **Effort / depends on:** L / F-05, F-07, RS-03, S-01.

### S-04 Insertion and clipboard
- **Goal:** fast, exactly-once insertion that never damages the user's clipboard.
- **User-visible outcome:** dictated text pastes into any app; whatever was on the clipboard (text, rich text, images, files) is back afterwards; clipboard managers do not log dictated text; paste failures are detected. If the Mac sleeps, the lid closes or the user switches accounts mid-take, nothing is pasted blind: the audio is kept and the text is offered on the card afterwards.
- **R2 rows closed:** A-47, A-48, A-53
- **Regression protected (HAVE):** A-46, A-49, A-54, A-204
- **R4 parts:** hex `src/paste.rs:92-542` (MIT, port: snapshot, transient markers, restore guard, modifier wait, Chromium 155/156 typing); Hex legacy `PasteboardSnapshot` (MIT, reference); arboard 3.6.1 `exclude_from_history()` (MIT/Apache, adopt direct); TypeWhisper `insertTextAtAndVerifyChange`, FluidVoice `PasteVerifier.swift` and `PasteDeliveryCoordinator.swift` FIFO slot (GPL-3.0, port); Handy `input.rs` layout-aware ⌘V and receipt-based reliable paste (MIT, keep).
- **Handy files touched:** `clipboard.rs:50-92`, `paste_tx/macos.rs:157-175,270-278` (delegate to `fork::clipboard`); ⚠ `actions.rs` (`fork::insert::deliver` hook); concealed paste set as a first-run default via `apply_product_settings`.
- **New fork files:** `fork/clipboard/{mod,snapshot,restore,markers}.rs`, `fork/insert/{mod,deliver,verify,session}.rs` (`session.rs`: sleep, screen lock and session-inactive notifications stop the take and route its result to the card).
- **Tests:** fake-pasteboard tests (multi-item round trip, changeCount guard, user copies during paste → no restore, back-to-back takes carry the snapshot forward); verification outcomes returned by `deliver()`; one insertion path per take (property test over cancel/retry races); sleep and session-switch during recording and during processing → card, never paste; IME active → paste then verify.
- **Real-Mac acceptance:** RS-05 matrix on the real slice; 100-take insertion run across the 10 apps of §5.4; lid close mid-take; fast user switch mid-take; Japanese IME active.
- **Exit criteria:** RS-05 pass criteria; 0 lost/duplicated insertions in the 100-take run; `deliver()` returns a `paste_outcome` per take and logs it (stored per take from S-09).
- **Effort / depends on:** M / F-05, RS-05.
- **As built (2026-10-30), decisions that change the plan above:**
  - **Hooks:** only `actions.rs` (`use crate::fork::insert;` + `insert::deliver(final_text, app)` for `utils::paste`). `clipboard.rs` and `paste_tx/` are not edited (§4.2 updated): on macOS with the ⌘V method `fork::insert` does the whole paste; other paste methods and platforms fall through to Handy unchanged. Handy's `append_trailing_space`, `auto_submit` (Return only after a confirmed paste or a read receipt) and "copy to clipboard" (plain, unmarked text instead of the restore) are honoured. The concealed-paste first-run default (`reliable_paste`) now only matters for the non-⌘V methods: the fork's transcript always carries the three markers.
  - **Files:** `fork/clipboard/{mod,snapshot,restore,markers,target,keys,nspb}.rs` (the RS-05 files, moved; the spike compiles them through `#[path]`, only hex's `carbon.rs` stays there), `fork/insert/{mod,deliver,verify,session,mac,tests}.rs`. `deliver.rs` is pure over a `Platform` trait (tests: `FakePasteboard`; the soak: the real pasteboard); `mac.rs` is the AppKit platform (pasteboard writes and ⌘V on the main thread, reads and waits on Handy's paste thread, restore on its own thread). The `Pasteboard` trait moved from `testutil` to `fork::clipboard` unchanged.
  - **Verification:** a fingerprint of the focused field (`AXNumberOfCharacters`, `AXSelectedTextRange`, else the value's length and hash; never the text) before ⌘V and every 25 ms for 300 ms after. Mail's body (a web area) and Chromium/Electron fields without an AX tree give `unverified`; an unverified paste is never retried. TypeWhisper/FluidVoice were followed in behaviour only (no code ported).
  - **Held modifiers:** if a hotkey modifier is still down 3 s after the take (hex's wait), the take goes to the card instead of sending ⌘V with it held (new; the soak lost a paste this way when another tool held a key).
  - **Next take waits for the previous read** (new; §4.7). **One delivery at a time**: a FIFO slot around `paste()`; neither `paste()` nor `settle()` ever retries, a ⌘V that cannot be posted is a card (`paste_failed`).
  - **Session:** `session.rs` follows `NSWorkspaceWillSleep/DidWake`, `SessionDidResignActive/DidBecomeActive` and the `com.apple.screenIsLocked/Unlocked` distributed notifications, plus Handy's `recording-state-event` for each take's span; a take whose span saw an interruption (or that is delivered while one lasts) goes to the card. Display sleep alone does not count (long hands-free takes). A recording in progress is stopped through Handy's external toggle (`send_external_input("transcribe")`, what SIGUSR2 sends); switch to S-01's `external_command(Stop)` once it lands.
  - **Real-Mac result (2026-10-30, M5 Max, macOS 26.6.2; `docs/fork/test-runs/1.0.0-alpha.1-M-26.md`):** 100 takes, 10 in each of TextEdit, Notes, Mail, Messages, Terminal, Safari, Chrome 156 (local page for Gmail), Cursor (for VS Code), Claude (for Slack), ChatGPT 156 (for Notion), through the product engine (`wb-spike-clipboard --s04-soak`): 100 in the field exactly once, 0 lost, 0 duplicated, 0 old-clipboard pastes; 100/100 clipboards byte-identical (six RS-05 source types); 70 confirmed, 30 unverified (Mail's web area, ChatGPT, Cursor, one Safari); 20 typed (Chromium 156); read receipt p50 14 ms, ⌘V→restore p50 221 ms / p95 360 ms. Japanese IME: both pastes landed and were confirmed. Lid close and user switching: only the wiring was checked (observer + gate with in-process NSWorkspace notifications, 4/4); a person runs S-04.3/.4. Clipboard managers: still untested (none installed). The app's own glue (`mac.rs`) was compiled and unit-tested but not run in the app.
  - **The card itself is S-05.** S-04 emits `ForkInsertOutcome { take, outcome, card_reason, text }` (text only for a card) and keeps `fork::insert::last_card()`; the reasons have strings under `fork.json > insert.cardReason`. Until S-05 the user sees Handy's paste-error path and the text in History.

### S-05 Accessibility layer, "no text field" card and secure fields
- **Goal:** know where the text will land, and never lose it when it cannot.
- **User-visible outcome:** with no text field focused, the pill shows a card "Click into a text field first" with the transcript, Copy and ✕ (Esc dismisses). An empty transcript shows "No speech heard" and copies nothing. Password fields are refused with a card. Electron and Chromium apps work. If the user moves to another field or app while the text is being processed, the text goes to the card instead of the wrong place.
- **R2 rows closed:** A-15, A-50, A-51
- **R4 parts:** SpeakoFlow `smart_spacing/macos.rs` + `mod.rs` (MIT, port) on objc2-application-services 0.3.2 (Zlib/Apache/MIT, adopt); OpenWhispr `macos-text-monitor.swift:45-100` editable rule (MIT, port); TypeWhisper `TextInsertionService.swift` fail-open rule, `missedTextField`, Gecko exclusion and test names (GPL-3.0, port); VoiceInk `AccessibilityTextReader.swift` text markers and per-PID lease refcount (GPL-3.0, port); Talkify / BetterWispr capture-target pattern (MIT / Apache original, port); Wispr docs 1621472516, 7971211038 (behaviour).
- **Handy files touched:** ⚠ `actions.rs` (record-start capture via `on_record_start`; delivery via `deliver`), `paste_tx/macos.rs` (secure-field refusal through `fork::insert`).
- **New fork files:** `fork/ax/{mod,thread,element,lease,editable,markers}.rs`, `fork/insert/target.rs` (target = element focused at key-up, §4.6), `src/fork/pill/states/Card.tsx`.
- **Tests:** decision table with a fake AX provider (editable, secure, no window, desktop, terminal, Electron empty tree → fail-open, focus moved to another app's field before key-up → paste there, focus moved after key-up → card); lease on/off timing; card copy semantics (replaces clipboard, no restore); Esc observed, not blocked, while the card shows.
- **Real-Mac acceptance:** RS-04 matrix on the real slice; 0 false cards with a text field focused; card appears on the desktop and in Finder; start a take in Slack, release in Notes → text in Notes; switch apps during processing → card.
- **Exit criteria:** RS-04 thresholds met in the real build; every take ends in field, card or History.
- **Effort / depends on:** M / S-03, S-04, RS-04.

### S-06 Recovery shortcuts
- **Goal:** Wispr's recovery tools.
- **User-visible outcome:** ⌃⌘V pastes the last transcript, ⌃⌘C copies it; "Press Enter" can be bound to a key or mouse button (unset by default); after a cancel, the pill shows "Cancelled · Undo" for 4 s, and Undo transcribes the cancelled audio and pastes it into the original field if that field still has focus, otherwise shows it on the card.
- **R2 rows closed:** A-26, A-27, A-28
- **R4 parts:** SpeakoFlow `dictation_recovery.rs` (MIT, port); hex modifier-release wait (in S-04); Wispr docs 7971211038, 6391241694 (behaviour).
- **Handy files touched:** `utils.rs:102` `cancel_current_operation` (one hook: `fork::recovery::cancel_keeping_audio`). The hook takes the samples from `AudioRecordingManager` instead of discarding them; if the recorder has no public way to hand them over, a small `take_samples()` method is added in a separate MIT-clean, upstream-bound commit. A cancel during processing reuses the WAV saved before transcription (§5.4).
- **New fork files:** `fork/recovery/{mod,undo}.rs`, `fork/shortcut/actions/{paste_last,copy_last,press_enter,undo_cancel}.rs`.
- **Tests:** last-transcript selection (shown variant, skips practice and failed takes); disabled when none; samples kept exactly 4 s, then dropped; cancel → Undo → text in History and in the field; Undo after focus moved → card.
- **Real-Mac acceptance:** RS-05 held-modifier case; ⌃⌘V into Slack, Chrome, Terminal; mouse-button Press Enter per RS-11; cancel → Undo in Mail.
- **Exit criteria:** all three shortcuts and Undo work in the 10 apps.
- **Effort / depends on:** S / S-01, S-03, S-04.

### S-07a Starter model, mic gate and "Setting up" first run
- **Goal:** the first dictation works within a minute of granting permissions, on any link speed.
- **User-visible outcome:** at Welcome the app starts downloading a small starter model. A press before it is ready still records; the pill shows "Setting up… 40%", and the text appears on the pill card with Copy as soon as the starter is ready (nothing is pasted late, the clipboard is untouched until Copy). A returning user whose model is missing gets a background re-download with the same pill state. Downloads resume after interruption, check free disk first, retry with backoff, and show "Couldn't download · Retry" after repeated failures.
- **R2 rows closed:** B-202, B-203, B-50, B-51, B-52, B-53, B-54
- **R4 parts:** whisper-base Q8_0 `handy-computer/whisper-base-gguf` (Apache-2.0, adopt); SpeakoFlow `speech_readiness.rs` (MIT, port); Handy catalog and hf-hub downloader (MIT, keep).
- **Handy files touched:** ⚠ `actions.rs:460-476` (mic-gate hook); `managers/model.rs` (catalog overlay, starter handling); `main.rs` (`HF_HOME` under app data, DEC-39); onboarding model step skipped through a fork flag until S-29a replaces the flow.
- **New fork files:** `fork/models/{starter,download,catalog_overlay,readiness}.rs`, `fork/models/catalog.fork.json`, `fork/models/queue_session.rs` (in-session queue: WAV saved, `queued_takes` row `waiting`, transcribed on ready, result to the card), `src/fork/pill/states/SettingUp.tsx`.
- **Tests:** download state machine (resume, offline, retry with backoff honouring `Retry-After` on 429/5xx, sha256 mismatch → file deleted and retried once, captive-portal HTML response → "check your connection" state, disk full); in-session queued take round trip (clipboard unchanged until Copy); swap never during a take.
- **Real-Mac acceptance:** RS-09 first-run part: clean installs at 10 and 50 Mbit/s meet §5.3; a press during the download records and shows the card when ready; captive-portal Wi-Fi simulation.
- **Exit criteria:** §5.3 first-run budgets; no model name visible outside Advanced.
- **Effort / depends on:** M / F-06, F-07, F-08, S-03, RS-09.

### S-07b Silent model policy, upgrades and cleanup
- **Goal:** the user never picks a model, and the best model for this Mac arrives and swaps in silently.
- **User-visible outcome:** right after the starter, the app downloads the best model for this Mac and the user's languages and swaps it in after it has loaded once. Changing languages re-picks. A better catalog pick later downloads in the background and swaps at an idle moment; superseded models are deleted. Retired catalog models are hidden and an existing selection migrates silently. Models appear only in Settings › Advanced (mounted on the fork debug panel until S-14).
- **Policy (DEC-02):** inputs: OS preferred languages + English; chip; RAM; free space on the model volume (purgeable-aware); measured speed after 5 takes. For each language, candidates come from the catalog (grades) and RS-09's latency table; the English table used depends on DEC-26 (both are kept). Pick the highest-grade model whose expected key-up latency for a 10 s take meets §5.1 on this machine class and whose size fits free disk minus 5 GB and the RAM tier. If nothing fits, pick the fastest usable model and say "slower on this Mac" once. Intel (DEC-29): CPU, the `fast` role. Re-pick on language change, low measured speed, or a catalog update that brings a better pick. Keep the old model until the new one has loaded once, then delete it.
- **R2 rows closed:** B-47, B-49, B-57, B-58, B-59, B-60, B-206, B-207, B-208, B-214, B-64
- **Regression protected (HAVE):** B-48, B-62, B-63, B-65
- **R4 parts:** sysinfo 0.39.x `system`,`disk` (MIT, adopt); hex `docs/plans/language-recommendations.md` table (MIT, data); TypeWhisper `SetupWizardEngineRecommendation` tests (GPL-3.0, port as spec); MacParakeet silent locale rule (GPL, port idea).
- **Handy files touched:** `catalog/mod.rs` (stale `HANDY_RANKING` bypassed via the overlay; RE-15); `tray.rs` (Model ▸ and Unload removed through `fork::tray::extend`).
- **New fork files:** `fork/models/{policy,machine,upgrade,cleanup}.rs`, `src/fork/settings/advanced/Models.tsx` (wraps Handy's model page).
- **Tests:** policy unit tests (≥ 60 vectors across languages, RAM, disk, chip, measured speed, both DEC-26 tables); swap never during a take; old model deleted only after the new one loaded; retired-model migration.
- **Real-Mac acceptance:** RS-09 policy part on the real slice: language change to ja+en re-picks; Intel picks CPU (DEC-29).
- **Exit criteria:** policy table matches RS-09 on each machine class; no model name visible outside Advanced.
- **Effort / depends on:** M / S-07a, RS-09.

### S-08 Microphone and sound polish
- **Goal:** the right mic by default and honest audio feedback.
- **User-visible outcome:** mic list in this order: the built-in mic marked "(recommended)", "Auto-detect (system setting)", then other devices, with a caution on Bluetooth devices ("Wired or built-in mics work best; Bluetooth can drop out"). If the system default is a Bluetooth headset and the user never chose, the built-in mic is used. Unplugging a mic mid-take keeps what was heard. If Microphone permission is revoked while running, the pill shows an error with Fix and keeps the audio so far. Sounds are on by default with Handy's themes. "Mute all audio while dictating" uses native CoreAudio (off by default). A level-meter API powers the mic test.
- **R2 rows closed:** A-37, A-38, A-40, A-42, A-211
- **Regression protected (HAVE):** A-39, A-41, A-43, A-44
- **R4 parts:** handy-recorder `backend/headset_macos.rs` transport type (MIT, port ~40 lines); Tambourine `audio_mute/macos.rs` + `tests/audio_mute_tests.rs` (AGPL-3.0, port; file marked AGPL; W-29).
- **Handy files touched:** `managers/audio.rs:100-111` and `:204` (both `osascript` calls delegate to `fork::audio`), `commands/audio.rs` (device list labels through a fork wrapper command, no edit).
- **New fork files:** `fork/audio/{mod,transport,mute,monitor,permission}.rs`, `src/fork/settings/MicrophonePicker.tsx`.
- **Tests:** transport classification; default-mic rule; list order; mute save/restore per device and `get_mute` without `osascript`; monitor mode does not create history rows; permission-revoke handling with a fake.
- **Real-Mac acceptance:** RS-11 mic-loss and permission-revoke cases; AirPods connected as default → built-in used; mute restores the previous volume exactly.
- **Exit criteria:** no `osascript` spawn anywhere in a take or at mute; mic loss keeps partial text.
- **Effort / depends on:** M / S-03, RS-11.

### Data and text intelligence foundations

### S-09 History data model, retention, stats store and app capture
- **Goal:** the data every Hub feature needs, stored once and correctly.
- **User-visible outcome:** History keeps everything (text forever, audio 14 days by default); stats survive retention and deletes; each entry knows its app; a full disk during recording keeps the text.
- **R2 rows closed:** C-46, C-212, D-06, D-21
- **R4 parts:** rusqlite + rusqlite_migration (in tree); unicode-segmentation 1.12 (MIT/Apache, adopt direct) + CJK/Thai rule (W-14); objc2-app-kit `NSWorkspace` (in tree) for the frontmost app; SpeakoFlow `insights.rs` and Muesli `DictationStore.swift` stats patterns (MIT, port); AudioWhisper / OpenWhispr bundle-id → category map (MIT, data).
- **Handy files touched:** ⚠ `managers/history.rs` (save/delete/retry hooks); `managers/transcription.rs:1600` (returns verbatim ASR text through the S-11 hook); ⚠ `actions.rs` (`on_record_start` captures frontmost app; `audio_ms` passed at save, `actions.rs:786-793`).
- **New fork files:** `fork/db/migrations/0002_history.sql`, `fork/db/{entry_meta,stats,fts,retention,reconcile}.rs`, `fork/stats/{wordcount,streak,wpm}.rs`, `fork/context/frontmost.rs`, `fork/context/categories.json` (`style_group`, `usage_category`, `is_ai_chat` per app, §4.3).
- **Rules:** count each non-empty dictation once; a retry updates counts by the delta; user deletes never subtract; file, practice and failed takes are excluded (C-48, C-211); local day at record time; `paste_outcome` from `deliver()` stored per take; retention job at launch and every 6 h deletes WAVs past the audio limit and rows past the text limit (Handy rows through Handy's history manager API), never stats; soft delete with a 10 s purge delay; WAV names include milliseconds (RE-35); launch reconcile per §4.3; disk full while recording → recording stops with a pill message, transcript delivered from memory to the card and an inbox entry (from S-18).
- **Tests:** migration tests; word counts (en, de, ja, zh, th, mixed); WPM on speech ms; retention with a fake clock; stats invariants across retry/delete; FTS search incl. < 3 chars fallback; Handy history rows deleted only through the manager API; kill between the Handy write and the fork write, relaunch → `entry_meta` backfilled; orphan `entry_meta` dropped; taxonomy table tests; disk-full fake.
- **Real-Mac acceptance:** 50 takes in 5 apps produce correct per-app rows; 14-day audio expiry simulated by clock override.
- **Exit criteria:** all invariants tested; `paste_outcome` recorded per take; a 10,000-row history searches in ≤ 100 ms.
- **Effort / depends on:** M / F-06, S-04.

### S-10 Local LLM runtime
- **Goal:** a bundled, supervised, private LLM ready for every text feature.
- **User-visible outcome:** after the speech model, the app silently downloads the right cleanup model; Settings › Advanced › "AI engine" (on the fork debug panel until S-14) shows Ready and a "Try it" button that cleans a sample sentence.
- **R2 rows closed:** D-67, D-71, D-211, D-212, D-213
- **Regression protected (HAVE):** E-82
- **R4 parts:** llama.cpp `llama-server` pinned tag (MIT, vendor-build, W-04); TypeWhisper `LlamaCppServer.swift`, `LlamaCppRuntime.swift`, `LlamaCppPluginTests.swift` (GPL-3.0-or-later, port; tests as spec; never its runtime download); SpeakoFlow `managers/local_llm.rs` prewarm/resident ideas (MIT, port parts); reqwest 0.12.28 `unix_socket` (in tree); Qwen3.5-2B Q4_K_M and Qwen3-4B-Instruct-2507 Q4_K_M (Apache-2.0, mirrored to our HF org, pinned revision + sha256; DEC-34); Handy hf-hub downloader (keep).
- **Handy files touched:** ⚠ `settings.rs` (`local` provider via the providers hook); `llm_client.rs` (`RequestOptions`: transport, temperature, max_tokens, timeout, neutral headers; upstream-bound); `managers/model.rs` (`is_fork_asset` so our GGUFs are not treated as ASR); `wb-build.yml` builds the binary.
- **New fork files:** `fork/llm/{supervisor,process,transport,health,memory,catalog,download}.rs`, `ci/llama-server/build.sh`, `src/fork/settings/advanced/AiEngine.tsx`; the `llama-server` backend for `scripts/wb-translate-missing.ts`.
- **Behaviour:** the supervisor starts or wakes the sidecar at key-down with the ordered loads of §4.5 (prewarm, D-211) so model load hides inside recording time; it alone stops the sidecar after 120 s idle; on wake from sleep it health-checks and restarts the sidecar if needed.
- **Tests:** ported supervisor tests (spawn, health timeout, crash restart once, backoff, CPU fallback, stale PID, env scrub, parent death); transport tests for socket and loopback-with-key; memory-policy and load-order unit tests (cold ASR never overlaps an LLM load); abort on cancel; provider list contains `local` and no cloud entries.
- **Real-Mac acceptance:** RS-06 on the real slice: signed binary under Gatekeeper on macOS 13 and 26; RSS and latency per tier incl. the cold-cold row; Esc abort ≤ 100 ms; kill the sidecar mid-take → take fails open, sidecar restarts; sleep mid-take → recovered on wake.
- **Exit criteria:** RS-06 thresholds in the real build; no network access by the sidecar (verified with nettop and the `wb-egress` run; `--offline` passed when the tag supports it).
- **Effort / depends on:** L / F-02, F-08, RS-06.

### S-11 Text pipeline v2
- **Goal:** the newline-safe, ordered pipeline of §4.10 with from→to rules and voice commands.
- **User-visible outcome:** "new line" and "new paragraph" work; multi-line text survives; "scratch that" deletes the previous clause; from→to dictionary rules apply (managed in S-16).
- **R2 rows closed:** D-210, D-48, D-56
- **Regression protected (HAVE):** C-67, D-15, D-17
- **R4 parts:** aho-corasick 1.1 (Unlicense/MIT, adopt direct); unicode-segmentation (adopt); VoiceTypr `src-tauri/src/writing/library_rules.rs` + 17 tests (AGPL-3.0, port; file marked AGPL) for the rule stage with protected URLs/e-mails and provenance; MacParakeet `SpokenPunctuation.swift` tables + literal escape (GPL-3.0, port data); SpeakoFlow `audio_toolkit/replacements.rs`, hyprwhspr-rs `normalize.rs` (MIT, reference); MacParakeet `spec/07-text-processing.md` (GPL, reference).
- **Handy files touched:** `managers/transcription.rs:1600-1662` (one `fork::text::process` call); ⚠ `actions.rs` (post-LLM stages run in `deliver`). `audio_toolkit/text.rs` not edited (wrapped per line).
- **New fork files:** `fork/text/{mod,lines,rules,commands,scratch,whitespace,placeholders,counts}.rs`, `fork/text/tables/commands.<lang>.json`.
- **Tests:** golden tests per stage; property test "no stage removes `\n`"; Wispr dictionary spec as acceptance (whole word, case-insensitive, literal, longest wins, ≤ 60 chars); clause-edge rule ("the period of time" stays text); Handy's existing `text.rs` tests unchanged.
- **Real-Mac acceptance:** dictate a three-paragraph e-mail into Mail and a two-line Slack message (Shift+Return path in typing mode).
- **Exit criteria:** stage order and change counts recorded per take (`fix_count_dictionary`).
- **Effort / depends on:** M / S-09.

### S-12a Cleanup levels on the local LLM
- **Goal:** Wispr's None / Light / Medium on every dictation, private and safe.
- **User-visible outcome:** every dictation is cleaned at Light by default (fillers, grammar, punctuation, self-corrections like "coffee at 2, actually 3" → "coffee at 3"). AI-chat apps (`is_ai_chat`) get no LLM formatting. If cleanup is slow, wrong or the model is not ready, the deterministic text is inserted and the pill says "Inserted without cleanup". The level picker component is built here and mounted on the fork debug panel; S-19a mounts the same component in Style › Cleanup. The second "post-process" hotkey and the Experimental gate are gone.
- **R2 rows closed:** A-209, D-01, D-02, D-03, D-04, D-05, D-09, D-11, D-12, D-13, D-16, D-208, D-49
- **R4 parts:** OpenWhispr `src/locales/*/prompts.json` `cleanupPrompt` (MIT, port seed); FreeFlow `PostProcessingService.swift:117` and `TranscriptTextCore.swift:128-182` (MIT, port); SpeakoFlow `actions.rs:1283-1460` validators (MIT, port); BetterWispr `TranscriptPolisher.swift:218-233` `accepted()` + `PolishContextTests` (GPL-3.0 VoiceInk-derived, port with VoiceInk attribution); TypeWhisper boundary wrapper and language hint (GPL, port); FluidVoice JSON-field framing (GPL, port); open-flow `needsCleanup` gate (MIT, port); Muesli `Qwen3PostProcessor.swift:110-140` assistant markers (MIT, port); similar 3.2.1 (Apache-2.0, adopt) for the LLM fix count.
- **Handy files touched:** ⚠ `actions.rs` (`post_process_transcription` → `fork::llm::post_process`, which also owns the Apple call site from `actions.rs:256-272`; Handy's bridge files untouched).
- **New fork files:** `fork/llm/{prompt,guards,post_process,deadline}.rs`, `fork/llm/prompts/*.txt` (our own text, seeded from the MIT sources), `src/fork/style/CleanupLevelPicker.tsx`.
- **Tests:** prompt builder snapshots; each guard with positive and negative cases; deadline and fail-open; AI-chat skip by `is_ai_chat`; language-kept check; dictionary terms injected and re-applied; nightly eval gate against the F-10 set with the RS-07 thresholds computed per §11.1.
- **Real-Mac acceptance:** RS-07 thresholds on the real build; §5.1 Light latency on M1 8 GB and 16 GB; cleanup works offline.
- **Exit criteria:** eval thresholds met; `cleanup_outcome` recorded per take; no unparsed output ever inserted.
- **Effort / depends on:** L / S-10, S-11, RS-07.

### S-12b Apple Foundation Models path
- **Goal:** use Apple's on-device model where it is faster, without touching Handy's bridge.
- **User-visible outcome:** on macOS 26 with Apple Intelligence on, cleanup can run on Apple's model (default or fallback per RS-08), with the same guards and fail-open behaviour; elsewhere nothing changes.
- **R2 rows closed:** D-68
- **R4 parts:** hex `apple_intelligence.swift`/`.rs` (MIT, port) + MacParakeet `FoundationModelsAppleIntelligence*` error mapping and TypeWhisper prewarm call sites (GPL, port), all into fork files.
- **Handy files touched:** none (the call site already goes through `fork::llm::post_process` from S-12a; `build.rs`, `apple_intelligence.rs` and `swift/apple_intelligence.swift` untouched).
- **New fork files:** `src-tauri/crates/wb-apple-fm/{Cargo.toml,build.rs,src/lib.rs,swift/fork_apple_fm.swift}` (own Swift build, unique symbol prefix `wb_apple_fm_`), `fork/llm/apple.rs` (availability reasons, prewarm, `spawn_blocking` with a cancelling timeout).
- **Tests:** availability reasons mapping; cancel and timeout; guard chain applied to Apple output; build on a machine without the macOS 26 SDK uses the stub.
- **Real-Mac acceptance:** RS-08 thresholds on the real build.
- **Exit criteria:** RS-08 decision applied (default or fallback-only); no symbol clash with Handy's bridge.
- **Effort / depends on:** S / S-12a, RS-08.

### Hub and features

### S-13 Hub shell
- **Goal:** Wispr's calm Hub chrome as the home for every feature.
- **User-visible outcome:** a warm light (and dark) window with a left sidebar: Dictation, Insights, Dictionary, Snippets, Style, Transforms, Scratchpad; at the bottom Share with a friend, Settings, Help; a slot for the setup checklist card (filled in S-30). Collapse button, header with theme toggle and notifications bell, serif display headings, rounded cards, example banners and empty states with one clear action, "New!"/"Beta" tags. The footer shows background-job status; S-28 adds the version and update status to the same footer.
- **R2 rows closed:** C-01, C-14, C-15, C-16, C-19, C-21, C-22, C-23, C-24, C-25, C-26, C-204, C-205, C-206, C-210
- **Regression protected (HAVE):** C-20, C-218
- **R4 parts:** Radix Primitives (dropdown-menu 2.2.1, popover 1.3.1, tabs 1.2.1, context-menu, collapsible; MIT, adopt); sonner 2.0.x and lucide-react (in tree); Newsreader 5.3.0 + Source Serif 4 5.3.1 (OFL-1.1, vendor woff2 subsets via the existing subset pipeline); Handy color tokens and `Dialog`/`Tooltip` (keep).
- **Handy files touched:** ⚠ `src/App.tsx` (render `ForkApp`); `Sidebar.tsx` untouched; main window size/min size set in `fork::on_ready` (no `lib.rs` edit).
- **New fork files:** `src/fork/hub/{ForkApp,Sidebar,Header,Footer,nav,dropRouter}.tsx`, `src/fork/ui/{Banner,EmptyState,Tag,Menu,Tabs}.tsx`, `src/fork/styles/{tokens.css,fonts.css}`, `src/assets/fonts/fork/*`.
- **Tests:** Playwright: every nav item, keyboard-only traversal, `aria-current`, banner dismissal persists, theme switch, deep link `navigate(page, section)`, file drop routing by page (a test page registers a drop zone; a drop outside any zone is ignored and never transcribed, RE-33; the Dictionary import case is tested in S-16), axe checks.
- **Real-Mac acceptance:** overlay title bar drags the window (C-25 drag permission check); VoiceOver reads the sidebar.
- **Exit criteria:** every page reachable by keyboard; light/dark contrast AA.
- **Effort / depends on:** L / F-05, F-07.
- **As built (2026-10-30):** decisions that refine this block (details in `docs/fork/ARCHITECTURE.md` "Hub (S-13)"; real-Mac and Playwright evidence in the S-13 report):
  - **Settings until S-14** is a page slot that hosts Handy's own sections unchanged as tabs (`src/fork/hub/HandySettingsPage.tsx`; section = tab, so `navigate("settings", "history")` deep-links), keeping every Handy setting reachable. S-14 replaces it with the modal. History is a drop zone there, so Handy's "drop audio to transcribe" survives, scoped to that page (RE-33). Placeholder pages for slices not built yet show an empty state with one action (a deep link where an interim home exists: Dictionary → Settings › Advanced custom words, Dictation → History).
  - **Help** is a Radix menu (non-modal) with an Essentials group of deep links (shortcut and microphone, models and languages, history); S-26b adds Updates and Get in touch. **Notifications** bell opens a Radix popover with an empty state; S-18 fills it through `setNotificationsPanel`. Checklist card slot: `setSidebarCard` (S-30).
  - **Theme toggle** in the header flips light ⇄ dark in one click and saves Handy's `theme`; System stays in Settings › About (S-14 General later). Tags: Transforms and Scratchpad "Beta", Insights "New!" (cleared on first visit through `seen_new_tags`). Sidebar collapse state is a per-window UI convenience in `localStorage` (no fork setting).
  - **Window:** default page area 1040 × 720 (shrunk to the screen's work area less 48 pt), minimum Handy's 720 × 600 page; frame adds the measured title bar (32 pt on macOS 26). Set in `fork/hub.rs` (a new top-level fork module, added to §4.1). The overlay title bar is set at Ready for onboarded users and by `ForkApp` on mount, restored on unmount (onboarding screens keep the standard bar); title text hidden. Capabilities: `core:window:allow-start-dragging` and `core:window:allow-set-title-bar-style` in `capabilities/fork.json` (`core:default` grants neither, C-25).
  - **Footer:** background jobs (Handy's `TranscriptionStatus`) plus Handy's update checker and version until S-28 replaces them; Handy's model selector is gone (B-207).
  - **Fonts:** Newsreader (Latin, Vietnamese) and Source Serif 4 (Cyrillic, Greek) are Fontsource's own per-script woff2 subsets, copied and sha256-pinned by `scripts/wb-fonts.ts` (no re-subsetting needed); `src/assets/fonts/fork/**` added to the guards' fork paths.
  - **Tokens:** `src/fork/styles/tokens.css` also retunes Handy's background, text, `--color-ink-2` and `--color-ink-3` (Handy's greys measured 3.9-4.2:1, below AA).
  - **Debug host** stays its own root (F-05 planned to move it into `ForkApp`): it must open over onboarding, and the self-check needs it in any state.
  - **package.json:** npm/bun have no comment keys inside dependency maps, so the §4.2 "`//fork` comment key" cannot mark fork dependencies; they are listed in `ci/hooks.txt` and ARCHITECTURE.md instead. `src/App.tsx` hook budget: 42 changed lines (import + JSX swap; Handy's unused helpers left in place for clean merges).
  - Axe runs on every Hub page and popup in light and dark, but excludes Handy's hosted settings sections (`[data-handy-content]`), which have serious findings of their own (unnamed icon buttons, a disallowed ARIA attribute); S-14 rebuilds them and S-37 audits them.

### S-14 Settings modal and hotkeys panel
- **Goal:** settings grouped by task in Wispr's two-column modal, with one hotkeys panel.
- **User-visible outcome:** Settings opens as a modal: **General** (Shortcuts "Hold fn to talk · Change", Microphone, My languages, App language, Theme Light/Dark/System), **System** (Launch at login, Always show the bar, Show app in Dock, Show menu-bar icon, Start hidden; Sounds; Mute while dictating; Notifications; Scratchpad open behaviour), **Data and Privacy** (filled in S-27), **Advanced** (collapsed: models from S-07b, AI engine from S-10, paste method, typing tool, auto-submit, trailing space, VAD, translate to English, live text in pill, bar position top/bottom, keyboard backend, prompt editor, beta updates, debug), **About**. The panels first mounted on the fork debug panel move here unchanged. The hotkeys panel lists every action with edit, delete, add (≤ 4), Reset to default, Done; it opens from Settings, Help › Shortcuts and onboarding.
- **R2 rows closed:** A-33, A-65, B-55, B-56, B-67, C-12, C-79, C-80, C-84, C-98
- **Regression protected (HAVE):** A-36, A-64, B-61, B-66, C-88, C-92, D-214
- **R4 parts:** Radix (adopt); Handy setting components (reuse by import); Tauri activation-policy code (keep).
- **Handy files touched:** ⚠ `lib.rs:129-163` (show-in-dock hook).
- **New fork files:** `src/fork/settings/{SettingsModal,General,System,DataPrivacy,Advanced,About}.tsx`, `src/fork/settings/hotkeys/{HotkeysPanel,BindingRow,Capture}.tsx`.
- **Tests:** Playwright for each section, deep links into sections, hotkey capture (refused under Secure Input with an explanation), conflicts, reset; Dock toggle survives window close; Advanced choices persist across relaunch (F-07 lists).
- **Real-Mac acceptance:** Dock toggle on macOS 26 (activation-policy flips); launch-at-login after reboot.
- **Exit criteria:** no technical control outside Advanced; every existing Handy setting still reachable.
- **Effort / depends on:** L / S-13, S-01, S-07b, S-08, S-10.

### S-15a Dictation home and stats rail
- **Goal:** Wispr's Dictation home: greeting, stats rail and a promo banner.
- **User-visible outcome:** "Hello, <first name>" (from macOS, overridable); a right rail with total words, words per minute and day streak, plus a Voice Profile progress bar (unlocks at 1,700 words, DEC-50); one dismissible promo banner; the rail refreshes live after each take.
- **R2 rows closed:** C-02, C-217, C-28, C-29, C-30, C-31, C-32, C-33, C-213
- **R4 parts:** objc2 `NSFullUserName` (in tree); Radix (adopt).
- **Handy files touched:** none (reads through fork commands).
- **New fork files:** `src/fork/home/{Home,Greeting,StatsRail,VoiceProfileBar,PromoBanner}.tsx`, `fork/commands/stats.rs`.
- **Tests:** Playwright: greeting fallback when no name; stats live refresh (`fork-stats-updated` event); banner dismissal; numbers match a fixture.
- **Real-Mac acceptance:** 10 takes update the rail live.
- **Exit criteria:** home is the landing page after onboarding; the rail matches `daily_stats`.
- **Effort / depends on:** M / S-09, S-13.

### S-15b History and row actions
- **Goal:** a history the user can act on.
- **User-visible outcome:** history grouped by date with search; each row shows the final text and on hover: play, copy, flag, and ⋮ (Undo AI edit / Redo AI edit, Retry transcript, Delete transcript, Save audio as…). Delete shows "Deleted · Undo". Background jobs and file transcriptions appear here (files tagged, not counted).
- **R2 rows closed:** C-34, C-35, C-36, C-37, C-38, C-39, C-40, C-41, C-42, C-44, C-45, C-207
- **Regression protected (HAVE):** B-211, C-43, C-47, C-48, C-211
- **R4 parts:** @tanstack/react-virtual 3.14 (MIT, adopt); Radix dropdown (adopt); sonner Undo toast (in tree); tauri-plugin-dialog (in tree) for Save audio.
- **Handy files touched:** none (reads through fork commands; Handy history commands reused for audio and retry).
- **New fork files:** `src/fork/history/{HistoryList,HistoryRow,RowMenu,FlagDialog,Search}.tsx`, `fork/commands/history.rs`.
- **Rules:** copy uses the shown variant (C-39, RE-04); Undo AI edit toggles `shown_variant` and never re-pastes (DEC-19); Flag opens "Wrong word?" → pick a word, type the correction → adds a word or rule through the S-16 store (DEC-18); Retry keeps `previous_text` and offers a different cleanup level; Save audio is disabled with "Audio removed after 14 days" when expired.
- **Tests:** Playwright for every row action, keyboard focus-within, search, grouping across midnight and time zones; flag writes a dictionary entry; 10,000-row scroll performance.
- **Real-Mac acceptance:** play audio (CSP check), retry, undo AI edit, delete/undo, flag → word appears in Dictionary.
- **Exit criteria:** all R1 §4 history behaviours present.
- **Effort / depends on:** M / S-09, S-12a, S-13, S-15a, S-16.

### S-16 Dictionary
- **Goal:** Wispr's Dictionary page on a richer local store.
- **User-visible outcome:** Dictionary page with tabs All / Added by you / Suggested (Suggested fills in S-34), search, sort, refresh, "Add word" dialog (word, or "Replace a word" from → to; the Add button stays disabled until the fields are filled), a dismissible banner "Teach Wispr Bro your words" with example chips, Import and Export. Import is a four-step wizard: (1) choose or drop a `.csv` (≤ 3 MB, with a link to our accepted-format help page), (2) map columns and preview, (3) review duplicates and conflicts, (4) done with counts. Apostrophes and non-ASCII words work. Existing Handy custom words are migrated once.
- **R2 rows closed:** C-58, C-60, C-61, C-62, C-64, C-65, C-68, D-47
- **Regression protected (HAVE):** none
- **R4 parts:** papaparse 5.7.0 (MIT, adopt); csv 1.4 (Unlicense/MIT, adopt); MacParakeet `VocabularyImportExportService` versioned format (GPL, reference).
- **Handy files touched:** none (words are mirrored into Handy's `custom_words` setting through its existing command; rules are never fed to the recognizer).
- **New fork files:** `fork/db/migrations/0003_dictionary.sql`, `fork/dictionary/{store,migrate,import,export}.rs`, `src/fork/dictionary/{DictionaryPage,AddWordDialog,ImportWizard}.tsx`, `docs/user/import-format.md`.
- **Tests:** one-time migration of existing custom words (idempotent); ≤ 60 chars rule; CSV formula-injection guard (`= + - @`), 3 MB cap; dedupe; words vs rules routing; wizard steps; a `.csv` dropped on the import step is imported, never transcribed (C-205).
- **Real-Mac acceptance:** add "Siobhán" and "gonna → going to"; dictate both in Mail.
- **Exit criteria:** the dictionary drives stage 3/4 of the pipeline; Dictionary page counts update live.
- **Effort / depends on:** M / S-11, S-13.

### S-17 Snippets
- **Goal:** Wispr's snippets: say a trigger, get the saved text.
- **User-visible outcome:** Snippets page (list, search, sort, Add new, Import/Export, banner with three examples). Saying "my calendar link" alone or inside a sentence expands to the saved text, newlines included. Bodies are plain text (DEC-12).
- **Matching rules (Wispr spec):** alone or inline; internal punctuation must match inline; spoken case ignored; expansion keeps its saved casing; longest match wins; possessives expand ("my email's"), "n't" contractions do not; trigger ≤ 60 chars; body ≤ 4,000 chars; single pass, never rescan; runs last (§4.10 stage 11).
- **R2 rows closed:** C-69, C-70, C-71, C-73, D-53
- **R4 parts:** aho-corasick (adopt); OpenWhispr `src/utils/snippets.ts` (MIT, port); espanso-render (GPL, reference only for future variables).
- **Handy files touched:** none beyond the `deliver` hook.
- **New fork files:** `fork/db/migrations/0004_snippets.sql` (incl. nullable `body_html`), `fork/text/snippets.rs`, `src/fork/snippets/{SnippetsPage,SnippetDialog}.tsx`.
- **Tests:** the Wispr spec as an acceptance table (≥ 40 cases); Unicode boundaries; snippet bodies never pass through the LLM.
- **Real-Mac acceptance:** a three-line address snippet in Mail, Slack, Notes.
- **Exit criteria:** acceptance table green; Snippets page counts update live.
- **Effort / depends on:** M / S-11, S-13, S-16 (shared import wizard).

### S-18 Notifications
- **Goal:** a local inbox plus a few macOS notifications.
- **User-visible outcome:** the bell opens a panel (filter, mark all read, empty state "Nothing new"); entries for model ready, update ready, shortcut problems, crash reports, milestones (streaks, word counts) and tips; a one-time popup for a new feature per version; macOS notifications only for model ready, update ready, queued takes ready and errors needing action, with action buttons where useful. Toggles: Suggestions, Announcements, Milestones. Esc dismisses toasts (observed, not blocked).
- **R2 rows closed:** C-17, C-201, C-202, C-91, E-91
- **R4 parts:** tauri-plugin-notification 2.4.0 + `@tauri-apps/plugin-notification` 2.4.x (Apache/MIT, adopt; 2.5+ needs Tauri 2.12); fallback `objc2-user-notifications` (in tree).
- **Handy files touched:** ⚠ `Cargo.toml`, `package.json` (fork block); the plugin is registered at runtime from `fork::on_ready` (`AppHandle::plugin`); `notification:default` added to `capabilities/fork.json`.
- **New fork files:** `fork/notify.rs`, `fork/db/migrations/0005_notifications.sql`, `src/fork/hub/NotificationsPanel.tsx`, `src/fork/hub/FeaturePopup.tsx`.
- **Tests:** inbox CRUD; toggles filter creation; macOS notification only for the four kinds; click and action buttons route to the right page or action.
- **Real-Mac acceptance:** RS-10 notification items on the real slice.
- **Exit criteria:** no notification permission prompt before the first real need.
- **Effort / depends on:** M / S-13, RS-10.

### S-07c Offline queue and late delivery (built after S-18)
- **Goal:** takes recorded with no usable model are never lost and never overwrite the clipboard.
- **User-visible outcome:** on an offline first launch (or when the model is missing and cannot download), presses keep recording; each take appears in History as "Waiting for speech model". When a model arrives, the takes are transcribed in order and one notification says "3 dictations ready" with **Copy latest** and **Open**. ⌃⌘V pastes the latest. If the onboarding window was closed mid-download, the inbox and a macOS notification say when dictation is ready.
- **R2 rows closed:** B-210
- **R4 parts:** Handy `managers/transcription_jobs.rs` (MIT, reuse) for the transcription queue; R4 "Bootstrap model" row queued-audio pattern (W-22).
- **Handy files touched:** none beyond the S-07a mic-gate hook.
- **New fork files:** `fork/models/queue.rs` (durable `queued_takes`, survives relaunch, ordered processing), `src/fork/history/WaitingRow.tsx`.
- **Tests:** offline first launch → 3 takes → relaunch → model arrives → 3 History rows, one notification, **clipboard unchanged** until Copy; Copy latest writes only the latest; failed transcription stays in History with Retry.
- **Real-Mac acceptance:** offline first launch with networking off, 3 takes, quit, relaunch online; notification action copies the latest.
- **Exit criteria:** 0 lost queued takes across relaunch; clipboard untouched by delivery.
- **Effort / depends on:** S / S-07a, S-09, S-15b, S-18.

### S-19a Styles: app groups and presets
- **Goal:** Wispr's per-app styles.
- **User-visible outcome:** Style page with tabs Personal / Work / Email / Other / Cleanup (the S-12a level picker). Each group shows its apps (icons, "+" to add, remove/move), and preset cards with before/after text: Formal / Relaxed / Very relaxed (personal), Formal / Relaxed / Upbeat (work, email, other). Styles apply in English only (cleanup level applies to all languages). Optional per-app "Press Enter after dictating" (off by default; the pill shows "sends automatically" when it is on; never after a guard fallback, an unverified paste or a card).
- **R2 rows closed:** D-18, D-19, D-20, D-23, D-24, D-25, D-26, D-27, D-207, D-61
- **R4 parts:** OpenWhispr `src/helpers/toneAppCategories.js` (~80 signatures) and `src/config/prompts/toneStyle.ts` (MIT, data/port); BetterWispr `Domain/WritingMode.swift` precedence + `WritingModeTests` (GPL-3.0 VoiceInk-derived, port); BetterWispr `StyleFormatter.swift`/`WritingStyle.swift` deterministic rules (Apache-2.0 original, port); Wispr trailing-period rules (behaviour).
- **Handy files touched:** none beyond hooks.
- **New fork files:** `fork/context/app_rules.rs`, `fork/db/migrations/0006_app_rules.sql`, `fork/text/style_rules.rs`, `src/fork/style/{StylePage,GroupTab,PresetCard,AppPicker,CleanupTab}.tsx`.
- **Tests:** app matching precedence; English gate; deterministic preset rules; prompt composition snapshot (level + style); auto-send only after a confirmed paste and never after a fallback.
- **Real-Mac acceptance:** Slack → Work, Messages → Personal, Mail → Email.
- **Exit criteria:** R1 §4 Style behaviours present for native apps; RS-07 Medium/style thresholds hold.
- **Effort / depends on:** M / S-05, S-09, S-12a, S-13.

### S-19b Styles: web-app detection
- **Goal:** Gmail in a browser counts as Email, detected locally.
- **User-visible outcome:** web apps in Chrome, Arc, Brave, Edge and Safari get the style of their group (Gmail → Email, Slack web → Work); the Style page lists sites alongside apps.
- **R2 rows closed:** D-22
- **R4 parts:** mado 0.0.21 `BrowserURLExtractor.swift` + `SupportedBrowsers.swift` from the crates.io tarball (MIT, port; never `WebsiteInfo.swift`); BetterWispr `WebsiteReader.swift` (GPL, port fallback); BetterWispr host patterns (GPL, port); Tambourine origin-only reduction (AGPL, port idea).
- **Handy files touched:** none.
- **New fork files:** `fork/context/url.rs`, host rules in `fork/context/categories.json`.
- **Tests:** host matching precedence over the browser app; host-only storage; window-title fallback.
- **Real-Mac acceptance:** RS-04 URL rows on the real slice; Gmail in Chrome/Arc/Safari → Email.
- **Exit criteria:** host detected in the five browsers or the title fallback used; only the host is stored.
- **Effort / depends on:** M / S-19a, RS-04.

### S-20 Context awareness
- **Goal:** spell names and terms right using text near the cursor, locally.
- **User-visible outcome:** on by default; a small "context" glyph appears on the pill while processing when nearby text was used; the History row says "Context used" (never the text). Off switch in Data and Privacy.
- **Rules (Wispr exclusions):** never read password, secure, numeric or URL-bar fields, terminals, password managers or banking/finance apps (bundle list); budget ≤ 1,500 characters before/selected/after the caret; ≤ 100 ms or skip; never stored, cleared when the take ends; used only as spelling hints for the ASR vocabulary (where the model supports it) and the LLM prompt.
- **R2 rows closed:** D-58
- **R4 parts:** SpeakoFlow caret window (MIT, in S-05 layer); FluidVoice `DictationContextCapture.swift` budgeted walk + password-manager skip list (GPL-3.0, port); Wispr docs 4678293671 (behaviour).
- **New fork files:** `fork/context/caret.rs`, `fork/context/exclusions.json`, `src/fork/pill/states/ContextGlyph.tsx`, `eval/context/names.jsonl` (30 items: an unusual name visible near the caret, spoken in the take).
- **Tests:** exclusion list; budget and timeout; nothing persisted (DB and logs checked); prompt includes hints only when allowed.
- **Real-Mac acceptance:** dictate a colleague's unusual surname visible in a Slack thread; verify the spelling and that the text never hits disk.
- **Exit criteria:** on the 30-item name set, correct-spelling rate improves by ≥ 20 percentage points with context on vs off; added p95 latency ≤ 100 ms.
- **Effort / depends on:** M / S-05, S-12a.

### S-21 Transforms core
- **Goal:** rewrite selected text in any app with a hotkey.
- **User-visible outcome:** with Transforms on (the fork setting, toggled on the fork debug panel until S-22 adds the page toggle), select text anywhere and press ⌥1 (Polish) or ⌥2 (Prompt Engineer): the text is replaced in place, ⌘Z undoes it, and the pill shows "Done · View changes". With nothing selected, the transform uses the field contents, else the last dictation from the past 60 s, else the pill says "Select text first".
- **R2 rows closed:** D-29, D-30, D-31, D-32, D-37, D-201, D-202, D-203, D-204
- **R4 parts:** selection chain: AX (S-05), VoiceInk text markers (GPL, port), SelectedTextKit menu Edit › Copy via AX (MIT, port), SpeakoFlow `selection.rs` ⌘C sentinel (MIT, port); MacParakeet `SelectionCaptureService`/`SelectionReplacementService` tests (GPL, port as spec); OpenLess `selection.rs` (AGPL, reference); FreeFlow `commandModeSystemPrompt` (MIT, port); `fork/llm` engine (S-12a).
- **Handy files touched:** none beyond hooks.
- **New fork files:** `fork/transforms/{mod,selection,replace,run,record}.rs`, `fork/db/migrations/0007_transforms.sql`.
- **Rules:** replace by clipboard paste (one ⌘Z step); abort if the frontmost app or element changed (D-203); refuse selections > 6,000 characters with a pill message (D-204); Esc cancels; keep the last 20 runs only.
- **Tests:** selection chain fallbacks with fakes; focus guard; no-selection rules; record bounding; hotkeys registered only when Transforms are on (DEC-31).
- **Real-Mac acceptance:** RS-04 replace rows + RS-11 ⌥ characters on the real slice; Polish in Gmail, Slack, VS Code, Notes.
- **Exit criteria:** §5.1 Transform latency; ⌘Z restores the original in the 10 apps.
- **Effort / depends on:** L / S-04, S-05, S-12a, RS-04, RS-11.

### S-22 Transforms page, editor, diff and auto-apply
- **Goal:** Wispr's Transforms page and diff view.
- **User-visible outcome:** Transforms page with an opt-in toggle, banner "Rewrite selected text in any app" (its "Try it" and "How it works" buttons appear when S-31 registers the lesson), "Apply after every dictation" with a chosen transform, My Transforms (Polish, Prompt Engineer, Create your own), Reset to defaults, Create new. The Polish modal: hotkey, description, example with a preview button, Autosave, Reset, rule toggles (Shorter, Clearer wording, Better order, Add headings and lists, Keep my tone), custom prompt, writing examples (paste or load .txt/.md; up to 3 × 1,500 chars). ⌥O or the pill action opens a diff card: "N changes", removed words struck, additions highlighted, Configure, Copy, 👍/👎 (stored locally), Retry.
- **R2 rows closed:** D-28, D-33, D-34, D-35, D-36, D-38, D-40, D-41, D-42, D-205, D-206
- **R4 parts:** similar 3.2.1 (Apache-2.0, adopt) word diff with `_deadline`; Radix (adopt); prompt builder (S-12a).
- **New fork files:** `fork/text/diff.rs`, `src/fork/transforms/{TransformsPage,TransformCard,TransformModal,DiffCard,RulesList,ExamplesEditor}.tsx`, `src/fork/pill/states/DiffCard.tsx` (W-16).
- **Rules:** auto-apply runs after cleanup through the same engine (DEC-13); on failure the original is pasted; runs closer than 5 s apart skip auto-apply.
- **Tests:** diff ops snapshot; rules → prompt fragments; custom transform CRUD; auto-apply fail-open.
- **Real-Mac acceptance:** ⌥O after a transform in Slack; auto-apply on a 3-sentence dictation.
- **Exit criteria:** R1 §4/§10 Transforms behaviours present (lesson entry points from S-31).
- **Effort / depends on:** L / S-21, S-13.

### S-23 Command Mode
- **Goal:** speak an instruction to edit or write text.
- **User-visible outcome:** with Command Mode on (Transforms page card), hold fn+Ctrl and say "make this friendlier": the selection is rewritten. With no selection, the instruction uses the field contents as context and inserts the result at the caret (writing tasks such as "reply saying I'll be late"). With no text and no selection, the pill says "Command Mode edits and writes text. Select text or click into a field." It never looks things up. With Command Mode off, fn+Ctrl is plain dictation (DEC-31).
- **R2 rows closed:** D-44, D-45
- **R4 parts:** FreeFlow `commandModeSystemPrompt` (MIT, port); `fork/transforms` (S-21); S-12a guards with an instruction-mode profile.
- **New fork files:** `fork/transforms/command.rs`, `src/fork/transforms/CommandModeCard.tsx`, `eval/command/{items.jsonl,RUBRIC.md}` (50 commands; 3-point rubric: 2 = did exactly what was asked with no damage, 1 = right intent with a minor flaw, 0 = wrong, damaging or answered instead of edited).
- **Tests:** chord routing from S-01 (on and off); three context cases; output bounds; guard profile.
- **Real-Mac acceptance:** 20 commands across Mail, Slack, Docs; ⌘Z restores.
- **Exit criteria:** on the 50-command set, two reviewers score independently with the rubric; ≥ 90 % of items score ≥ 1 from both and ≥ 70 % score 2 from both.
- **Effort / depends on:** M / S-21.

### S-24 Scratchpad
- **Goal:** Wispr's Scratchpad: quick notes you can dictate into.
- **User-visible outcome:** Scratchpad page with "Add to the bar", a shortcut button (dialog proposes ⌥S; unset by default), "New note", Recents (edit, pin, delete) and search. A small floating note window with tabs, +, expand, close, a left rail (sidebar, new note, search, ✨ Polish, text size), the text area and Copy. With "Add to the bar" on, the resting bar becomes a group: Dictate and Scratchpad (Notetaker joins in M2); hovering each shows a tooltip ("Hold fn to dictate", "Scratchpad"). Open behaviour: resume last note or new note.
- **R2 rows closed:** A-03, C-74, C-75, C-77, C-78, C-216, D-209
- **Regression protected (HAVE):** C-76
- **R4 parts:** tauri-nspanel / Tauri window (adopt); FTS5 trigram (in tree); `fork/transforms` for ✨ (S-21).
- **Handy files touched:** none (the `fork-scratchpad` page is added to `src/fork/vite-inputs.ts`).
- **New fork files:** `fork/scratchpad/{mod,window,store}.rs`, `fork/db/migrations/0008_notes.sql`, `src/fork/scratchpad/{ScratchpadPage,NoteWindow,NoteTabs}.tsx`, capability entry `fork-scratchpad`, `fork/pill/slots.rs`.
- **Tests:** autosave (debounced 500 ms, survives crash), tabs, search, pin, slot API with tooltips (A-03), window capability.
- **Real-Mac acceptance:** dictate into the note window; open via ⌥S after binding; note survives force-quit.
- **Exit criteria:** R1 §10 Scratchpad behaviours present.
- **Effort / depends on:** L / S-03, S-13, S-21, F-06.

### S-25 Insights
- **Goal:** Wispr's Insights, computed locally.
- **User-visible outcome:** Insights with tabs Usage / Voice. Usage: words per minute (speech time) compared with the user's typing speed (from the typing test, else "average typist: 40 wpm"), fixes made (dictionary fixes, AI words corrected), total words, usage by `usage_category` (documents, AI prompts, email, work messages, personal messages, other), a streak heatmap (bands 0 / 1-249 / 250-499 / 500-749 / 750+), and "Save as image". Voice: locked until 1,700 words with blurred cards, then cards for a frequent phrase, peak time and app, most used word, most corrected word.
- **R2 rows closed:** C-50, C-51, C-52, C-54, C-55, C-56, C-57
- **R4 parts:** Recharts 3.10.1 (MIT, adopt, lazy-loaded); react-activity-calendar 3.2.1 (MIT, adopt); html-to-image (MIT, adopt); stopwords-iso (MIT, data); similar (fix counts, S-12a).
- **New fork files:** `fork/stats/{insights,voice}.rs`, `src/fork/insights/{InsightsPage,UsageTab,VoiceTab,Heatmap,CategoryChart}.tsx`.
- **Tests:** aggregates from `daily_stats`/`corrections`; locked/unlocked states; image export.
- **Real-Mac acceptance:** "Save as image" works in the release build under the CSP (§4.11).
- **Exit criteria:** numbers match a hand-computed fixture; no population percentile anywhere.
- **Effort / depends on:** M / S-09, S-11, S-12a, S-13.

### S-26a App menus and tray
- **Goal:** Wispr's menu structure, local and honest.
- **User-visible outcome:** app menus Dictation ("N words dictated", Copy last transcript with preview, Microphone ▸, Languages ▸), Vocabulary ("N dictionary words", "N snippets", Add word…, New snippet…), Help (registered with `set_as_help_menu_for_nsapp()`, so macOS adds its search box; Documentation, Shortcuts, Microphone, Languages) while Edit/Window keep ⌘C/⌘V/⌘Z working. Counts update live. Tray: Open Wispr Bro, Check for updates… (Handy's existing check until S-28 routes it to `fork/update`), Paste last transcript (disabled until one exists), Shortcuts, Microphone ▸, Languages ▸, Documentation, Quit (no Model ▸ or Unload). All menu strings come from `fork/i18n.rs`.
- **R2 rows closed:** A-56, A-57, A-59, A-63, C-100, C-101
- **Regression protected (HAVE):** A-55, A-62, A-69
- **R4 parts:** Tauri 2.11.5 `Menu::default` + inserted submenus and `Submenu::set_as_help_menu_for_nsapp` (MIT/Apache, W-11).
- **Handy files touched:** `tray.rs` (`fork::tray::extend`, already present from F-01).
- **New fork files:** `fork/app_menu.rs` (extensible builder: later slices add items by id), `fork/tray.rs` (items).
- **Tests:** menu structure snapshot (Edit present, Help registered); disabled states; counts live (C-213 menu part, C-105); localized strings in 3 locales.
- **Real-Mac acceptance:** ⌘C/⌘V/⌘Z in Hub text fields; the Help search box finds menu items; the tray icon comes back after the menu bar process (ControlCenter) restarts (A-69).
- **Exit criteria:** R1 §6 and §11 menu items present (minus out-of-scope ones and the S-26b items).
- **Effort / depends on:** M / S-13, S-14, S-06, S-15a, S-16, S-17.

### S-26b Help popover, support, Share with a friend, About
- **Goal:** Wispr's help surfaces, local and honest.
- **User-visible outcome:** sidebar Help popover: Updates (What's New), Essentials (Shortcuts, Microphone, Languages, lessons from S-31), Get in touch (Report a problem, Send feedback). "Report a problem… ⌘/" and "Send feedback" are added to the Help menu and tray. Report a problem opens a prefilled GitHub issue (no transcripts) and offers Copy diagnostics. Share with a friend: copy the download link or `brew install --cask` line, macOS share sheet; no tracking. About: version, GPL-3.0, source link, credit to Handy.
- **R2 rows closed:** C-13, C-103, C-104
- **R4 parts:** `NSSharingServicePicker` via objc2-app-kit (in tree); diagnostics bundle (F-08, S-27).
- **Handy files touched:** none.
- **New fork files:** `src/fork/hub/HelpPopover.tsx`, `src/fork/share/SharePage.tsx`, `src/fork/settings/About.tsx`, `fork/diagnostics/issue_url.rs`.
- **Tests:** issue URL builder (no transcript fields); popover keyboard navigation; share sheet invocation.
- **Real-Mac acceptance:** ⌘/ opens a prefilled issue in the browser; share sheet shows.
- **Exit criteria:** R1 §6 Help and §11 items present (minus out-of-scope ones).
- **Effort / depends on:** S / S-26a, F-08.

### S-27 Data and Privacy, export, delete all, diagnostics
- **Goal:** the privacy promise made visible and controllable.
- **User-visible outcome:** Data and Privacy page in plain words: everything stays on this Mac; the two network uses with hosts; no account, telemetry or cloud; where data lives (data, models, recordings, logs, each with "Show in Finder"); retention (text: forever / 1 year / 90 / 30 days; audio: 14 days default, never keep / 1 / 7 / 30 / 90 days / forever); context awareness toggle; update checks toggle (bound to the fork setting from F-07). S-34 adds its auto-learn row here. "Export my data" (zip) and "Delete all my data" (typed confirmation; optional "also delete downloaded models"). Crash reports and Copy/Save diagnostics.
- **R2 rows closed:** B-205, E-64, E-66, E-67, E-68, E-79
- **Regression protected (HAVE):** E-65, E-88
- **R4 parts:** csv 1.4, zip 4.x (in tree, adopt direct); Voquill log sanitizer (from F-08); TypeWhisper `UserDataEraser` / MacParakeet eraser checklist (GPL, reference); `std::panic::set_hook` + tauri-plugin-log (in tree, W-24).
- **Handy files touched:** none beyond F-08 hooks.
- **New fork files:** `fork/export.rs`, `fork/erase.rs`, `fork/diagnostics/{crash,bundle}.rs` (crash file, `.ips` pickup, sanitized zip), `src/fork/settings/DataPrivacy.tsx`.
- **Erase order:** stop recording and jobs → unload models and stop the sidecar → delete `history.db`, `fork.db` (+ `-wal`, `-shm`), recordings, settings stores, logs, (models if chosen) → relaunch into onboarding.
- **Tests:** export contents and schema version; erase leaves no files in the app data dir except models when kept; portable mode paths (E-88); crash file creation and inbox entry.
- **Real-Mac acceptance:** export opens in Finder; erase then relaunch shows onboarding; `brew uninstall --zap` paths match (S-39).
- **Exit criteria:** every statement on the page is true and checked by a test or the egress check.
- **Effort / depends on:** M / S-09, S-14, F-08.

### S-28 Updates that never interrupt
- **Goal:** Wispr's update policy with our signed feed.
- **User-visible outcome:** updates download in the background and install only when idle (no dictation or processing, ≥ 20 min since the last take) or at quit; the app relaunches quietly to the bar; after 3 failed installs it pauses 24 h. The Hub footer (S-13) shows "Up to date", "Update ready · Restart now" or "Couldn't check · Retry" next to the version. Tray "Check for updates…" now uses `fork/update` and is honest when checks are off. What's New appears once per version, never while recording. Beta channel opt-in in Advanced (endpoint selection from F-02).
- **R2 rows closed:** C-99, E-51, E-53, E-75, E-215
- **Regression protected (HAVE):** A-60
- **R4 parts:** tauri-plugin-updater 2.10.1 (keep; `download()` then `install()`); MacParakeet `SparkleUpdateGuard.swift` cases (GPL, port as tests); Talkify deferral (MIT, idea); Handy offline What's New (keep content mechanism).
- **Handy files touched:** none (Handy's `UpdateChecker.tsx` is not mounted by the fork footer).
- **New fork files:** `fork/update/{scheduler,idle_gate,failures}.rs`, `src/fork/hub/UpdateStatus.tsx`, `src/fork/whats-new/*.md` (our content).
- **Tests:** guard cases (blocks during recording, vetoes relaunch mid-take, skips dev builds, 3-failure pause); channel URL selection; tray item routed to the fork check.
- **Real-Mac acceptance:** RS-12 on the real slice; update during a long hands-free take waits.
- **Exit criteria:** never installs mid-take in a 2-day beta test.
- **Effort / depends on:** M / F-02, S-14, S-18, S-26a.

### Onboarding

### S-29a Onboarding: welcome, permissions and the two tests
- **Goal:** Wispr's onboarding start without an account: tiny tests with immediate yes/no, never blocked by downloads.
- **User-visible outcome:** a stepper Welcome › Permissions › Setup › Practice (R1's PERSONALIZE stage is a referral screen and is out, §2.3) with a Back link and a Help chip. Welcome (privacy promise; the starter download starts here) → Move to Applications if needed → Permissions (Microphone, Accessibility) with a picture of each macOS prompt and pane, rows that tick when granted → Test your microphone (copy: wired or built-in mics work best, Bluetooth can drop out; live bars; "Do the bars move when you talk?"; Change microphone opens the list in the S-08 order) → Test your shortcut (an on-screen fn/🌐 key lights while pressed; Edit shortcut opens the hotkeys panel; "No" leads to a diagnosis: tap not live, Secure Input, no fn key → switch to Ctrl+Opt, Globe bound to emoji/input source → the DEC-44 path).
- **R2 rows closed:** B-01, B-02, B-03, B-10, B-21, B-22, B-201, E-216
- **Regression protected (HAVE):** B-04, B-05, B-09, B-11, B-12, B-13, B-46, B-212
- **R4 parts:** Handy onboarding hooks `usePermissions.ts`, `useTryEngine.ts`, `Demo.tsx` (MIT, reuse by import); MacParakeet `OnboardingViewModelTests` (GPL-3.0, port as behavioural spec); hex `native/permission_guide.m` and OpenWhispr `PermissionGuideOverlay.tsx` (MIT, port); Yap "reset and re-grant" (MIT, idea).
- **Handy files touched:** ⚠ `src/App.tsx` (fork onboarding replaces the switch; Handy onboarding files untouched).
- **New fork files:** `src/fork/onboarding/{OnboardingFlow,Stepper,Welcome,MoveToApplications,Permissions,MicTest,ShortcutTest}.tsx`, `src/fork/onboarding/art/*.svg` (macOS prompt mock-ups per version, light/dark), `fork/onboarding/state.rs` (logic over the onboarding state stored in `fork/settings.rs`, RE-16).
- **Tests:** Playwright for every step and branch incl. each "No" diagnosis; resume after relaunch at each step; preview mode; never waits on downloads.
- **Real-Mac acceptance:** clean Mac: permissions to shortcut test in ≤ 2 min; each diagnosis branch reachable.
- **Exit criteria:** every step and branch covered; no step waits on a download.
- **Effort / depends on:** M / S-01, S-03, S-07a, S-08, S-14.

### S-29b Onboarding: languages, practice, celebration, typing test and summary
- **Goal:** the first dictation inside onboarding, celebrated, and a number for the time it can save.
- **User-visible outcome:** Languages (preselected from macOS; copy states an honest language count) → "Practice: dictate an email without holding the key" in a simulated composer (Skip always offered). If the starter is not ready, the practice still records and shows "Setting up… N%"; the composer fills when the starter arrives, and after 90 s the step moves on by itself with the take kept in History → Practice stage, dark theme: "That's it. You dictated your first text." (confetti, reduced-motion aware) → time saved: "Dictating could save you about 0 hours a week" (the counter starts at 0) → "How fast do you type?" 30-second typing test (language picker, sample paragraph per language, live wpm, seconds left, Try again, "Recalculate"), after which the counter updates, with a typing-time-per-day slider (default 2 h, DEC-50) and a "Take the typing test" link if it was skipped → launch-at-login choice and a summary of choices. The resting bar appears when setup ends (DEC-38).
- **R2 rows closed:** B-25, B-28, B-29, B-36, B-37, B-213
- **Regression protected (HAVE):** B-26, B-27
- **R4 parts:** canvas-confetti 1.9.4 (ISC, adopt); Handy `Demo.tsx` (MIT, extend); typing test and slider (W-18).
- **New fork files:** `src/fork/onboarding/{Languages,HandsFreePractice,FirstDictation,TimeSaved,TypingTest,Summary}.tsx`, `src/fork/onboarding/samples/<lang>.json`.
- **Tests:** Playwright: practice with a mocked slow starter download (records, shows progress, fills later, auto-advances at 90 s); wpm math; counter starts at 0 and updates after the test; slider; skip paths.
- **Real-Mac acceptance:** 10 people on clean Macs (8 GB and 16 GB, one Intel if DEC-29): median ≤ 5 min, first dictation within 2 min of install (§5.3).
- **Exit criteria:** §5.3 met; 0 testers stuck, where stuck = needed outside help or spent > 15 min in onboarding.
- **Effort / depends on:** M / S-29a, S-12a, S-15a.

### S-30 First-use chooser, pill spotlight, setup checklist
- **Goal:** turn setup into a habit: a first real use and a next step.
- **User-visible outcome:** the main window opens with "What do you want to try first?" (Ask an AI assistant, Jot a note, Reply to a message, Write an email, Post a comment, Draft a document, Prompt a coding tool) with a captioned animated scene each → "Try it now" dims the screen and spotlights the bar with a hint ("Now open your email and hold fn") until the first real dictation in another app or Esc → home shows a "Use it in your own apps" banner. A setup checklist card sits in the sidebar (as in R1) and ticks itself, chaining to the next step. Items register through a checklist registry as their features exist: Try it in another app, Add a word, Create a snippet, Launch at login (here); Try a Transform (added by S-31); Try whispering (added by S-32).
- **R2 rows closed:** A-16, B-31, B-44
- **R4 parts:** driver.js 1.10.2 (MIT, adopt; `t()` strings only, never user text); Handy `Demo.tsx` scene engine (MIT, extend); own click-through dim NSPanel `fork-spotlight` (W-08).
- **New fork files:** `src/fork/onboarding/FirstUseChooser.tsx`, `src/fork/onboarding/scenes/*.tsx`, `fork/pill/spotlight.rs`, `src/fork/hub/{SetupChecklist,checklistRegistry}.tsx` (page `fork-spotlight` added to `src/fork/vite-inputs.ts`).
- **Tests:** checklist auto-ticks from events; registry adds items later without changes here; spotlight dismissal rules; Esc observed, not blocked.
- **Real-Mac acceptance:** spotlight panel is click-through and sits above full-screen apps; dismisses on the first dictation in Mail.
- **Exit criteria:** every chooser option has a scene and a matching hint.
- **Effort / depends on:** M / S-29b, S-03, S-15a, S-16, S-17.

### Tutorials

### S-31 Lesson engine and Transforms tutorial
- **Goal:** every feature teaches itself with a short scene and a practice step.
- **User-visible outcome:** a generic lesson modal (progress dashes, close, back): intro scene → practice in a simulated app with success detection → "Turn on X". "How it works" opens animated scenes (no video). Transforms lesson: three cards (Original / Polish / Prompt Engineer) and a benefits ticker; "Select the text, then press ⌥1"; on success "Text rewritten. Press ⌥O to view the changes; ⌘Z undoes it." with a spotlight on the bar's "Done · View changes"; "Turn on Transforms". Lessons re-open from the Help popover's Essentials, page banners (the S-22 banner buttons appear now) and "Start tutorial" buttons. Adds "Try a Transform" to the setup checklist.
- **R2 rows closed:** B-39, B-40, B-45, D-43
- **Regression protected (HAVE):** B-24
- **R4 parts:** Handy `Demo.tsx` (MIT, extend); driver.js (adopt); SpeakoFlow `tourScenes.tsx` real-component tours (MIT, port idea).
- **New fork files:** `src/fork/tutorials/{LessonEngine,LessonModal,PracticeComposer,SuccessDetector,lessonRegistry}.tsx`, `src/fork/tutorials/lessons/transforms.ts`.
- **Tests:** lesson engine state machine; success detection with mocked events; re-entry points; checklist item registration.
- **Exit criteria:** the Transforms lesson completes end-to-end on a real Mac in under 2 min.
- **Effort / depends on:** M / S-22, S-30.

### S-32 Whisper tutorial and microphone quiz
- **Goal:** Wispr's quiet-dictation flow with a concrete recommendation.
- **User-visible outcome:** home banner "Dictate quietly · Learn how" → (1) scene: speaking softly still works, (2) how to whisper with the built-in mic, (3) practice: whisper a short email in a simulated composer (Continue enables when ≥ 70 % of the words match; otherwise tips: get closer, raise input volume) → quiz: (1) "Pick a comfortable microphone" with our own illustrations of four mic types and a "More about mics" link to our docs page, (2) "Where do you usually work?" (private / mixed / shared spaces), (3) "Do you move between places?" (one desk / different places), (4) "Which microphones do you have?" → recommendation table (mic quality, whispering around others, whispering at home, price) with a "Best match" column; Done / Copy results. Adds "Try whispering" to the setup checklist.
- **R2 rows closed:** B-42, B-43
- **R4 parts:** lesson engine (S-31); own recommendation rules table.
- **New fork files:** `src/fork/tutorials/lessons/whisper.ts`, `src/fork/tutorials/MicQuiz.tsx`, `src/fork/tutorials/micTable.ts`, illustrations (own SVG), `docs/user/microphones.md`.
- **Tests:** quiz → recommendation rules (all combinations); success threshold.
- **Real-Mac acceptance:** whispered take on the starter and the full model at 30 cm.
- **Exit criteria:** the lesson never dead-ends (Skip always available).
- **Effort / depends on:** M / S-31, S-08.

### Late features and polish

### S-33 ITN: spoken numbers to digits (gated by RS-13)
- **Goal:** "twenty five dollars" → "$25" where the ASR does not already do it.
- **User-visible outcome:** setting "Write numbers as digits" (on for English when shipped).
- **R2 rows closed:** D-14
- **R4 parts:** text-processing-rs 0.2.2 (Apache-2.0 + NOTICE, adopt; number/money/date/time/measure taggers only, never its punctuation tagger or `custom_rules`); text2num 2.8.0 (MIT, adopt) for it/pt/nl/da; TypeWhisper number parsers (GPL, reference).
- **Handy files touched:** none (stage 6 in `fork::text`).
- **New fork files:** `fork/text/itn.rs`.
- **Tests:** per-tagger goldens; "the period of time" untouched; per line.
- **Exit criteria:** if RS-13 fails, this slice closes D-14 as "not needed" with the RS-13 report attached.
- **Effort / depends on:** M / S-11, RS-13.

### S-34 Dictionary auto-learn (opt-in)
- **Goal:** learn names and jargon from the user's own corrections, with approval.
- **User-visible outcome:** toggle "Learn from my corrections" (off by default) in Dictionary and Data and Privacy (this slice adds the row to the S-27 page). After a paste, if the user fixes a word in that field (committed by Return, Tab or focus change within 30 s), the fixed word appears in Dictionary › Suggested with Accept / Reject. Rejected words never return.
- **R2 rows closed:** D-52
- **R4 parts:** BetterWispr `Integrations/CorrectionWatcher.swift`, `CorrectionLearner`, `Transcription/CorrectionReviewer.swift` + `CorrectionLearningTests` (GPL-3.0 VoiceInk-derived, port with attribution, W-27); VoiceInk `Features/Dictionary/AutoLearn/` `CorrectionDiffEngine` (GPL-3.0, port); TypeWhisper commit triggers (GPL, port idea).
- **New fork files:** `fork/dictionary/learn/{watcher,diff,review}.rs`.
- **Rules:** uses the S-05 lease; skips secure fields, terminals, password managers; stores only the corrected word pair, never field text.
- **Tests:** ported learning tests; multi-hunk diffs; reject list.
- **Real-Mac acceptance:** correct "Kubernetees" → "Kubernetes" in Slack; suggestion appears.
- **Exit criteria:** no field text persisted (checked in DB and logs).
- **Effort / depends on:** M / S-16, S-05, S-20, S-27, RS-04.

### S-35 Vocabulary importers: Wispr Flow and Handy (opt-in)
- **Goal:** one-click migration of the user's own words and snippets.
- **User-visible outcome:** in Dictionary and Snippets › Import: "From Wispr Flow" and "From Handy". The app copies the user's local Wispr Flow database (`~/Library/Application Support/Wispr Flow/flow.sqlite` + WAL) to a temp folder, checks the copy, opens it read-only, and previews dictionary words, replacements and snippets; Handy's `com.pais.handy/settings_store.json` custom words likewise. The user picks what to import.
- **R2 rows closed:** none directly (owner request, DEC-37); extends the import paths of C-64 (S-16) and C-71 (S-17)
- **R4 parts:** TypeWhisper `Services/AppVocabularyImport.swift` and `WisprFlowImportReader.swift` + tests (GPL-3.0-or-later, port, W-28).
- **New fork files:** `fork/import/{wispr,handy}.rs`, `src/fork/dictionary/ImportFromApp.tsx`.
- **Rules:** never writes to the other app's files; schema unknown or changed → "Couldn't read Wispr Flow data" and nothing imported; no Wispr code or assets.
- **Tests:** fixtures of both formats; two-identical-reads rule for Handy; WAL copy consistency.
- **Exit criteria:** import works on the owner's Mac with a real Wispr Flow install.
- **Effort / depends on:** S / S-16, S-17.

### S-36 Vibe coding
- **Goal:** Wispr's vibe-coding helpers for VS Code, Cursor and Windsurf, locally.
- **User-visible outcome:** Settings › Vibe coding: "Recognize code names · Set up" walks the user through turning on the IDE's screen-reader-optimized mode, after which identifiers visible in the editor are used as spelling hints (camelCase, snake_case kept); "Tag files in chat" (toggle): in Cursor/Windsurf chat inputs, "index dot ts" becomes `@index.ts` when a matching file name is visible (open tabs or explorer via AX). Off until set up.
- **R2 rows closed:** D-59, D-60
- **R4 parts:** S-20 context layer; Wispr docs 6434410694 (behaviour).
- **New fork files:** `fork/context/ide.rs`, `fork/text/file_tags.rs`, `src/fork/settings/VibeCoding.tsx`.
- **Tests:** identifier extraction; file-tag rewrite only in listed apps and only for visible names.
- **Real-Mac acceptance:** dictate "rename get user by id" in Cursor with the identifier visible.
- **Exit criteria:** no filesystem scanning; works with AX only.
- **Effort / depends on:** L / S-20, S-19a.

### S-37 Accessibility, localization and copy pass
- **Goal:** meet §5.5 and §5.6 everywhere, and confirm all copy is our own.
- **User-visible outcome:** VoiceOver reads the Hub, settings, onboarding and the pill; keyboard-only use works; all 26 languages complete (web and Rust strings).
- **R2 rows closed:** E-81
- **Regression protected (HAVE):** B-68, E-80
- **R4 parts:** eslint-plugin-jsx-a11y 6.10.2 and @axe-core/playwright (MIT / MPL-2.0 dev only, DEC-36); i18next-cli 1.80.5 (MIT, dev; never locize commands); `scripts/wb-translate-missing.ts` (W-25, from F-05).
- **New fork files:** `fork/pill/accessibility.rs` (NSAccessibility announcements), `docs/fork/VOICEOVER-CHECKLIST.md` (scripted steps per screen: focus order, every control's spoken name and state, announcements), `scripts/wb-copy-check.ts` (compares English fork strings with R1 quotes, §1), `ci/i18n-strict`.
- **Tests:** axe on every page (0 serious/critical); RTL snapshots (ar, he); key parity and placeholder checks; strict i18n mode on (no `machine`/`untranslated` flags left); copy check green.
- **Real-Mac acceptance:** the VoiceOver checklist passes for onboarding, the Hub, settings and one dictation; Increase Contrast and Reduce Motion on.
- **Exit criteria:** §5.5 checklist passes; no flagged fork key in any locale; human review done for the 10 sample-text languages.
- **Effort / depends on:** M / all UI slices.

### S-38 Performance and reliability hardening
- **Goal:** meet every §5 budget on the reference machines.
- **User-visible outcome:** fast, quiet, stable app.
- **R2 rows closed:** E-83
- **Regression protected (HAVE):** E-85
- **R4 parts:** none new; uses RS-03/RS-06/RS-09 harnesses.
- **Soak harness (`scripts/wb-soak/`):** a test-only virtual audio input (BlackHole, GPL-3.0, test machines only, never shipped) set as the app's microphone plays prerecorded WAV takes (incl. multi-line and snippet takes); each take is started and stopped with `--toggle-transcription` (the CLI goes through the same coordinator and `deliver` path as the hotkey); the driver focuses the target app and field by script before each take, then reads the field back through AX and compares it with the expected text; per take it logs outcome (`confirmed`/`card`/`lost`/`duplicate`/`wrong target`), latency and clipboard restore. A separate **hardware run** does 50 takes with a real fn key on a real keyboard, the tester following a printed script, results logged the same way.
- **New fork files:** `scripts/wb-soak/` (driver, AX read-back, per-app field scripts), `docs/fork/PERF.md` (measurements).
- **Tests:** latency benches (p50/p95 per §5.1), 8 h idle run, 500-take soak, 50-take hardware run, memory-pressure test on 8 GB, startup timing, single instance (E-85).
- **Exit criteria:** all §5 numbers met or the model tier changed to meet them (recorded).
- **Effort / depends on:** M / S-01..S-32.

### Release

### S-39 1.0 release
- **Goal:** ship Wispr Bro 1.0.0 publicly.
- **User-visible outcome:** `brew install --cask <owner>/tap/wispr-bro` and a DMG on GitHub; About › Acknowledgements lists every ported source, crate, font and model with licences.
- **R2 rows closed:** B-204, E-217
- **R4 parts:** R4 §7 ship checklist C-D; Homebrew cask (`auto_updates true`, `arch arm:/intel:` or `depends_on arch: :arm64` per DEC-29, `depends_on macos: :ventura`, `zap` under `com.wisprbro.desktop` app data incl. our models dir); `wb-tap-bump.yml` (~30 lines, PAT scoped to the tap); open-source-mac-os-apps and awesome-mac PRs (CC0 lists).
- **New fork files:** `src/fork/settings/Acknowledgements.tsx` (from `THIRD-PARTY-NOTICES.md`), `.github/workflows/wb-tap-bump.yml`, README (product, privacy promise, install, licence, credits).
- **Tests:** `brew audit --cask --strict --online` (RT-38); install/uninstall `--zap`; RS-12 re-run 1.0.0-rc → 1.0.0.
- **Exit criteria:** §9 M1 exit checklist passes; release published (not prerelease); `latest.json` resolves.
- **Effort / depends on:** M / S-37, S-38 and the §9 checklist. Homebrew/cask self-submission later, when the repo is ≥ 30 days old with ≥ 225 stars (or 90 forks/watchers).

---

## 9. M1 exit criteria ("dictation is perfect")

M2 starts only when every box is ticked. Evidence goes in `docs/fork/M1-EXIT.md` with links to test runs. Intel items apply only if DEC-29 keeps Intel.

**Latency and resources**
- [ ] Every §5.1 latency row met on M1 8 GB, M-series 16 GB+ and the Intel Mac (DEC-29) (S-38 report), or the model tier was changed to meet it.
- [ ] §5.2 memory and CPU ceilings met, including the 8 h idle run with the bar visible (≤ 50 MB growth) and the combined idle RSS of all processes.
- [ ] §5.3 startup and first-run budgets met at 10 and 50 Mbit/s on clean Macs.

**Reliability**
- [ ] 500-take soak (S-38 harness) across the 10 apps of §5.4: 0 lost, 0 duplicated, 0 wrong-target insertions; every take ends in the field, on the card or in History; plus the 50-take hardware run with the same result.
- [ ] Clipboard restored byte-identical for the RS-05 type matrix; Maccy, Raycast and Paste do not record dictated text; queued delivery leaves the clipboard unchanged.
- [ ] Hotkey self-healing: RS-02 procedure passes on the release build for both taps (wake ≤ 2 s; revoke detected ≤ 2 s; re-grant recovers ≤ 5 s).
- [ ] 0 insertions in 100 fn chords, 0 orange-indicator flashes in fluent chords (key within `T_show`) and in 50 quick taps; chords keep their native function; Esc cancels in every state, and under Secure Input through the Carbon shadow (RS-14) or, if that failed, through ✕ on the pill.
- [ ] Every §5.4 negative case passes in its owning slice's test.
- [ ] No crash in the soak, the idle run or a 2-week beta with ≥ 20 users (crash files reviewed).
- [ ] Updates: 1.0.0-rc.N → rc.N+1 installs only when idle and keeps Microphone and Accessibility permissions (RS-12).

**Quality of text**
- [ ] RS-07 thresholds met by the shipped default per tier, computed per §11.1 (Light content damage ≤ 2 %, answer rate ≤ 0.5 %, filler removal ≥ 90 %, language kept ≥ 99 %; Medium damage ≤ 4 %).
- [ ] Newlines, voice commands, dictionary rules and snippets pass their acceptance tables (S-11, S-16, S-17).

**Onboarding**
- [ ] 10 testers on clean Macs (8 GB, 16 GB, one Intel if DEC-29): median completion ≤ 5 min, 90th percentile ≤ 8 min, first dictation ≤ 2 min after install and ≤ 60 s after permissions, 0 stuck (S-29b definition).

**Parity with R1** (each item ticked with a screenshot in `docs/fork/PARITY.md`; behaviour and structure match, copy is our own)
- [ ] R1 §2 Flow Bar: resting bar, active capsule with ✕/waveform/✓, mic label, fallback card, processing and error states, hover tooltip, click to start, "Hide for an hour"; three-button bar tooltips (two buttons until M2).
- [ ] R1 §3 hotkeys: every action except "Join meeting" (M2), with the panel (edit, delete, add, Reset to default, Done).
- [ ] R1 §4 Hub: Dictation home (greeting, stats rail, Voice Profile at 1,700 words, history with all row actions), Insights (both tabs), Dictionary, Snippets, Style (all tabs incl. Cleanup), Transforms, Scratchpad, notifications panel, theme Light/Dark/System. Not Notetaker, profile menu, team, referral.
- [ ] R1 §5 Settings: General, System and a local Data and Privacy page; Vibe coding. Not Notetaker, Connectors, MCP, Account, Team, Plans and Billing.
- [ ] R1 §6 and §11 menus and tray, minus meeting items; Help menu with the macOS search box.
- [ ] R1 §10 guided flows: Help popover, Transforms guide (How it works, two-step practice with the view-changes and ⌘Z hint, Polish modal), Scratchpad window, three-button bar (two buttons until M2).
- [ ] R1 §11 dialogs: Add to vocabulary (Add disabled until filled), four-step Import dictionary with the format link, Add snippet, Scratchpad shortcut, quiet-dictation guide; setup checklist card in the sidebar.
- [ ] R1 §12 quiz including the mic-choice first step and the "more about mics" link, with the recommendation table (Copy instead of Email).
- [ ] R1 §13 onboarding local variant: permissions with pictures, mic test (wired/built-in advice; list order built-in, Auto-detect, others), shortcut test, hands-free practice, first-dictation celebration, time saved starting at 0 with the typing test inside the Practice stage, first-use chooser, pill spotlight, home banner.
- [ ] Declared parity gaps (accepted, not failures): snippets are plain text (DEC-12); typing-time slider default 2 h instead of 6 h (DEC-50); no PERSONALIZE/referral stage (§2.3).

**Privacy, accessibility, localization, licence**
- [ ] 24 h egress capture (Little Snitch or `nettop`) on a used install shows only §1 hosts; `wb-egress` green on the release build; the full Mac test script passes with networking off after setup.
- [ ] §5.5 accessibility checklist passes (keyboard-only, VoiceOver checklist, contrast AA, reduced motion; axe: 0 serious/critical).
- [ ] All 26 locales complete for the `fork` namespace and `rust.*` keys; RTL checked; 10 sample-text languages human-reviewed.
- [ ] All CI guards green; THIRD-PARTY-NOTICES and Acknowledgements complete; source tarball attached to the release.
- [ ] The three owner decisions (DEC-26, DEC-29, DEC-44) are answered and applied.

**Bugs and test runs**
- [ ] No open P1 bug. P1 = data loss, wrong-target or lost insertion, hotkey dead without notice, crash, privacy breach (unlisted network call, text persisted against the rules, or the clipboard overwritten without a user action), onboarding blocker. At most 10 open P2 bugs, each with a workaround.
- [ ] The Mac test script (§11.3) passes on M1 8 GB, an M-series 16 GB+ and one Intel Mac (DEC-29), together covering macOS 13, 14, 15 and 26.

---

## 10. M2 Notetaker plan

Coarse slices; each gets the full slice template when M2 starts. M2 adds the real-Mac checks RT-25..RT-29 and RT-32..RT-34 to its first slices. M2 needs macOS 14.4+ for system audio (gated at runtime; on 13.0-14.3 Notetaker records the mic only and says so). UI copy follows the own-words rule (§1).

| ID | Slice | Goal and outcome | R2 rows | R4 parts (licence, verb) | Depends | Effort |
|---|---|---|---|---|---|---|
| N-01 | Engine design spike (DEC-06) | Benchmark both designs on 8 GB and 16 GB: (a) chunked meeting decode on the shared engine with per-owner cancel, dictation borrowing the mic and a "Dictated" marker in the transcript (Wispr's documented behaviour); (b) a second supervisor with its own model copy. Measure RAM, live-text stall while dictating, cancel isolation, `protocol.rs` diff size. Write the decision | E-47 (AF-04) | transcribe-cpp (in tree); hex "meeting chunks only while dictation is idle" (MIT, idea) | M1 exit | M |
| N-02 | System-audio capture | Global process tap excluding our PID on a private aggregate device, probe-tap permission check, 14.4 gate, usage string, stream to disk with crash recovery, sleep/App Nap prevention, own-sound exclusion and no mute-while-dictating during meetings, rebuild on output-device change | E-01, E-02, E-10, E-48, E-93, E-205, E-206, E-207 | hex `src/meeting_capture.rs:422-665` tap path (MIT, port); Muesli `MeetingSystemAudioWatchdog.swift` (MIT, port idea); RT-25..RT-28 | N-01 | M |
| N-03 | Dual-source capture and echo | Mic and system as separate time-aligned tracks; bleed suppression | E-03, E-04 | hex `src/meeting_echo.rs` (MIT, port) + own text-match pass; FluidVoice AEC3 C bridge (GPL-3.0, port) only if RT-34 requires real-time AEC | N-02 | M |
| N-04 | Meeting store and docked panel | `fork-meeting` right-edge panel: header controls and "…" menu, editable title, Today chip, Add folder, tabs Notes / Transcript / Summary, Stop / Resume, consent line, improvement banner and tip cards, screen-share toast and "hide from screen capture" setting, bar recording state, notes view / side-by-side; meeting tables | E-12, E-14, E-17, E-19, E-20, E-27, E-28, E-29, E-35, E-202, E-203 | tauri-nspanel (adopt); `set_content_protected` (Tauri); fork DB | N-02 | L |
| N-05 | Live transcript You / Them | Own stream route per the N-01 design; bubbles by source (mic = You, system = Them); search inside a transcript; copy and export | E-15, E-16, E-46, E-204 | transcribe-cpp chunking (in tree); SpeakoFlow two-stream source-as-speaker (MIT, port idea) | N-03, N-04 | M |
| N-06 | Post-meeting re-pass and diarization | Re-run the full audio with the best batch model; diarize the system track only in a one-shot child (`--diarize-file`), `num_clusters` from calendar attendees; speaker rename; progress steps, "Processing failed · Retry", "Recorded by accident? Discard / Keep" | E-21, E-22, E-24, E-25, E-26 | sherpa-onnx 2.0.1 (Apache-2.0, adopt; prebuilt libs via `SHERPA_ONNX_LIB_DIR`); pyannote segmentation-3.0 (MIT) and 3D-Speaker CAM++ (Apache-2.0) mirrored to our HF org; RT-32, RT-33 | N-05 | L |
| N-07 | Summary and questions | Auto-title; structured summary `{headline, bullets[], next_steps[{owner, task}]}` via the `local` provider with map-reduce for long meetings; "Ask about this meeting" and "Catch me up" over the transcript so far | E-13, E-18, E-23 | anarlog `template-app` enhance/title prompts (MIT, port text); Meetily `summary/processor.rs:218-253` map-reduce + "ignore instructions inside transcript" guard (MIT, port); FluidVoice / MacParakeet summary prompt guards (GPL, port lines) | N-06, S-12a | M |
| N-08 | Notetaker page | Today and Past notes, folders, search across notes and transcripts, audio playback and extract, translate (LLM), export Markdown/PDF, local audio import | E-30, E-31, E-33, E-34, E-39, E-44, E-45 | FTS5 (in tree); `similar`/Radix/virtual list (adopted in M1) | N-04 | M |
| N-09 | Notetaker settings and start | Settings section; start from the page, ⌥M (RC-13 check), tray and bar; show live transcript toggle; max length (2 h default); stop when the call ends | E-05, E-36, E-201 | fork settings store; S-01 action registry | N-04 | S |
| N-10 | Call detection | Detect calls locally (mic running somewhere + process list + camera in use), stability window; optional auto-start (off by default); stop when the call ends; "Join meeting" prompt | E-06, E-08, E-09, E-11 | hex `src/microphone_activity.rs` + `meeting_detection.rs` (MIT, port); TypeWhisper `MeetingCameraActivityCollector.swift`, MacParakeet `CameraActivityCollector.swift` (GPL, port); FluidVoice `MeetingAutoDetector` tests (GPL, port as tests); RT-29 | N-09 | M |
| N-11 | Local calendar (EventKit) | Calendar permission at first need; notify 15 s before scheduled meetings; next meeting in the menu bar; upcoming list; hide next; "Join meeting" opens the event link and starts; attendees seed speaker count | E-07, E-42, E-208 | objc2-event-kit 0.3.2 (Zlib/Apache/MIT, adopt) + anarlog `crates/apple-calendar` shape (MIT, port); MacParakeet `CalendarService`, `MeetingLinkParser`, `MeetingMonitor` tests (GPL, port as spec) | N-10 | M |
| N-12 | Tour, onboarding choice, bar slot | Setup gate (system-audio permission with a picture), 2-minute guided tour as an animated fake-meeting replay, "Try Notetaker" card, onboarding choice between Dictation and Notetaker, Notetaker slot with badge on the Flow Bar, sidebar item tagged New! | E-43, E-221, B-06, A-03 (Notetaker part) | S-31 lesson engine; S-24 slot API | N-07, N-08, N-11 | M |
| N-13 | M2 hardening and release | 2 h meeting soak (memory, disk, crash recovery), privacy page and export/delete cover meetings, M2 exit (latency of live text ≤ 3 s behind speech, summary ≤ 60 s for a 1 h meeting on 16 GB), release 1.1.0 | – | – | N-01..N-12 | M |

Excluded from M2: sharing links, calendar consent notice, cloud calendar connectors, MCP, "Shared with me", Granola/Otter account import (see §2.3). Windows and Linux system audio (E-210) belong to the later platform milestone.

---

## 11. Test and release strategy

### 11.1 Automated tests and eval metrics
- **Rust unit tests** next to each fork module, using the fakes in `fork/testutil.rs` (clock, AX, pasteboard, coordinator, network). Hot logic (hotkey processor, blocking set, text pipeline, LLM guards, model policy, stats rules, retention, reconcile) targets ≥ 90 % line coverage.
- **Rust integration tests** run the fork plugin against mocked managers (no audio hardware), including the composed invoke handler and the Ready-ordering test (F-05).
- **Ported test suites** are kept as test vectors with attribution (anarlog, MacParakeet, FluidVoice, TypeWhisper, VoiceTypr, BetterWispr).
- **Cleanup eval metrics** (`eval/cleanup/README.md`, computed by `scripts/wb-eval-cleanup.ts`, no LLM judge). Each item has: the input transcript; the expected output per level; labelled **protected spans** (names, numbers, URLs, e-mails, code spans, dictionary terms); labelled **filler spans**; a **content-word list** (input words minus stopwords from stopwords-iso and minus filler spans); an `is_instruction` flag (the transcript is a question or a command); the language. Two people label every item independently; disagreements are resolved by discussion and the agreement rate is recorded.
  - **Content damage** (per item, yes/no): a protected span is missing or changed in the output (exact match after Unicode NFC; case-insensitive for words), or content-word recall against the expected output is below 0.95. Rate = damaged items / all items.
  - **Answer rate:** among `is_instruction` items, the share where the output answers or executes instead of transcribing: output word count > 1.5 × input word count, or ≥ 3 content words that appear in neither the input nor the expected output, or the output starts with an assistant marker from the Muesli list.
  - **Filler removal:** labelled filler spans absent from the output / all labelled filler spans (Light and Medium only).
  - **Language kept:** Handy `lang_id` of the output equals the labelled language, over items with ≥ 5 words.
  - **Guard catch rate:** damaging pre-guard outputs (content damage or answer) that the guard chain rejected / all damaging pre-guard outputs.
  - Gates use **post-guard** output (what the user gets); model selection in RS-07 also reports pre-guard numbers. Runs use temperature 0 and a fixed seed.
- **Nightly cleanup eval** (F-10 set) on a self-hosted Apple Silicon runner; a drop beyond RS-07 thresholds fails the nightly.
- **CI guards** (§4.12) on every PR; `wb-egress` nightly and on release.

### 11.2 Playwright (UI)
- `playwright.fork.config.ts`, mocked Tauri IPC for every fork command and event (`tests/fork/fixtures`).
- One spec per page and per pill state; light and dark; one RTL locale; reduced motion; axe checks on every spec.
- Onboarding: every step and every "No" branch, resume after relaunch, slow-download practice.
- Features that Playwright cannot prove (CSP with the real asset protocol, notifications, menus) are covered by real-build smoke runs (F-08, S-25, RS-10).

### 11.3 Manual Mac test script (outline of `docs/fork/MAC-TEST-SCRIPT.md`)
Run per release candidate on M1 8 GB, M-series 16 GB+ and Intel (DEC-29) (≈ 3 h each); results in `docs/fork/test-runs/<version>-<machine>.md`.
1. Clean install from DMG and Homebrew; Gatekeeper; permissions; onboarding timing.
2. Hotkeys: grammar table, chords (native function and no mic indicator), Esc, fallback profile, Secure Input (RS-14 cases), sleep/wake, revoke/re-grant.
3. Flow Bar: states, click, hover, drag, hide, displays (mixed scale), full screen, Spaces, top position.
4. Insertion matrix: 10 apps × (short, multi-line, snippet, no text field, password field), clipboard restore types, clipboard managers, IME, app switch during processing.
5. First run at 10 and 50 Mbit/s, offline first launch with queued takes, captive portal, language change.
6. Cleanup levels, styles per app group, AI-chat skip, context awareness, voice commands.
7. Transforms, diff, auto-apply, Command Mode on and off, ⌘Z.
8. Hub: history actions, search, dictionary, snippets, insights, scratchpad, notifications, settings, menus, tray.
9. Updates: idle install, beta channel, permissions kept.
10. Privacy: egress capture, offline run, export, delete all.
11. Accessibility: VoiceOver checklist, keyboard-only, contrast, reduced motion.
12. Soak: 100 takes and 2 h idle (full 500-take, 50-take hardware and 8 h runs before 1.0).

### 11.4 Channels
- **Internal alpha** (`1.0.0-alpha.N`) from F-02: team only, draft releases; testers update through the fork debug panel's "Check now".
- **Beta channel** from S-28: the rolling `beta` prerelease whose `latest.json` the app reads when "Receive beta updates" is on (Advanced, S-14; endpoint chosen in Rust, F-02). ≥ 20 external testers for ≥ 2 weeks before 1.0.
- **Stable**: `releases/latest/download/latest.json`.

### 11.5 Release cadence
- Before 1.0: alpha builds as slices land; betas weekly from S-28.
- After 1.0: beta weekly, stable minor every 4 weeks, patch releases for P1 bugs within 48 h. Every release runs the guards, `wb-verify`, `wb-egress`, the Mac test script (abridged for patches) and RS-12.

### 11.6 Upstream merge cadence
Weekly automated sync PR (§4.13); merged every two weeks during M1 after the hook checklist and the full test suite pass; security fixes immediately; never in the 3 days before a release. Upstream-bound fixes (e.g. the `HandyKeysState::new` error path, `llm_client` request options, coordinator `external_command`, a recorder `take_samples()` if S-06 needs it, the Moonshine licence label) are offered upstream as MIT-only PRs following Handy's PR template.

---

## 12. Risks and mitigations (top 15)

| # | Risk | Mitigation | Gate |
|---|---|---|---|
| 1 | macOS still runs the Globe action when fn is our hotkey, or blocking fn breaks fn chords | Own blocking tap with `{FN}` always blocked; RS-01 checks every "Press 🌐" value and 100 chords; DEC-44 options (pending owner) where suppression fails | RS-01 → S-01, S-29a |
| 2 | The event taps die after sleep, revoke or re-sign and the user does not know | Error path fix, axwatch + probe for both taps, pill/tray/notification, auto-recovery | RS-02 → S-02 |
| 3 | The always-visible pill leaks webview memory or loses first clicks | Exact-size panel, no idle animation, `accept_first_mouse`, Handy's overlay never built; native resting pill as plan B | RS-03 → S-03, S-38 |
| 4 | AX gaps in Electron/Chromium cause false "no text field" cards or failed transforms | Fail-open rule, lease, menu-Copy and ⌘C fallbacks, clipboard replace | RS-04 → S-05, S-21 |
| 5 | Clipboard restore loses user data, clipboard managers log dictated text, or late results overwrite the clipboard | Full snapshot, transient + concealed markers, changeCount guard, documented exclusions; queued and late results never write the clipboard | RS-05 → S-04, S-07c |
| 6 | LLM + ASR exceed memory on 8 GB Macs | Memory policy, ordered loads, one idle owner, smaller tier model, skip cleanup under pressure | RS-06 → S-10, S-38 |
| 7 | Small models change meaning or answer dictated questions | Light default, guard chain, AI-chat skip, Undo AI edit, eval gate with defined metrics | RS-07 → S-12a |
| 8 | Cleanup latency misses the budget | Ordered prewarm at key-down, deadline with fail-open, tier downgrade, Apple FM path | RS-06, RS-08 → S-12a, S-12b, S-38 |
| 9 | The starter model gives a weak first impression | Per-language starter choice, quick swap, "improving accuracy" copy | RS-09 → S-07a |
| 10 | `llama-server` fails signing, notarization, macOS 13 or the Unix socket | CI static build with 13.0 target; verify job; loopback-with-key fallback; own small server binary as last fallback | RS-06, RS-10 → S-10 |
| 11 | Fork plumbing breaks: ACL, init order, upstream merges | Composed handler with CI assertions, `on_ready` rule with an ordering test, listed one-line hooks with a hook-size guard, weekly sync, upstream-bound fixes sent upstream | F-05, F-03, F-09 |
| 12 | Updater key loss or identity change strands users or resets permissions | Key with two offline backups, identity guard, fixed identity before external builds, RS-12 every release | F-02, F-03, RS-12 |
| 13 | Licence contamination (GPL-2.0-only, non-free weights, missing attribution, AGPL file marking, GPL code in Handy files) | Licence register, cargo-about guard, SPDX guard (incl. upstream files), NOTICE, never-default model list, DEC-26 to the owner | F-03, F-04 |
| 14 | Private APIs or Secure Input behaviour change on a macOS update | Feature checks, public fallback paths, re-run RS-01, RS-03 and RS-14 on each macOS beta | RS-01, RS-03, RS-14 |
| 15 | M1 scope (46 slices) slips and quality drops | Strict dependency order with no forward dependencies; split slices with their own exits; only S-33 (gated by RS-13) and S-35 (optional importer) may move to 1.1 without failing §9 | §9 |

Also tracked: trademark risk of the name (owner accepted; identity lives only in the overlay and i18n overrides, so a rename is one slice; our UI copy is our own, DEC-45); the CC-BY default (DEC-26) can be applied either way by switching the English pick table in the catalog overlay.

---

## 13. Traceability appendix

Generated from the R2 tables and this plan's row-to-slice map; every row id appears exactly once in 13.1-13.4, and every slice's "R2 rows closed" and "Regression protected" lists match this appendix exactly (re-checked by script after the review; §14). The Feature column repeats R2's row titles, which quote Wispr's UI for identification; they are not our UI copy (§1). Counts: 325 TUNE/EXTEND/BUILD/unclassified rows, 76 HAVE, 23 OUT, 68 XREF = 492.

### 13.1 TUNE, EXTEND and BUILD rows (and the 2 unclassified rows) → slice

"deferred" = not in M1 or M2, with the reason in the note.


**A1. Flow Bar / pill**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| A-01 | Resting bar always visible | BUILD L | S-03 |  |
| A-02 | Hover on resting bar (tooltips "Dictate fn") | BUILD S | S-03 |  |
| A-03 | Three-button bar (Dictate / Notetaker / Scratchpad), Notetaker teal badge | BUILD M | S-24 | Dictate + Scratchpad slots in M1; Notetaker slot and teal badge in N-12 |
| A-04 | Click the bar to start dictating | BUILD S | S-03 |  |
| A-05 | Active capsule layout: ✕ left, waveform, ✓ right | TUNE S | S-03 |  |
| A-06 | Finish ✓ on the capsule | BUILD S | S-03 |  |
| A-10 | Mic-name label at start ("Using Built-in mic (recommended)") | BUILD S | S-03 |  |
| A-12 | Error state on the pill | BUILD S | S-03 | pill error state; Secure Input and tap-loss messages wired in S-02 |
| A-13 | Feedback for empty or silent dictation | EXTEND S | S-03 |  |
| A-14 | Pill message state with action ("Done. See changes"; owner, D-39 folded) | BUILD S | S-03 | generic message + action; first consumer S-21 |
| A-15 | Result / fallback card ("Select a textbox first, then dictate" + transcript + Copy + ✕) | BUILD M | S-05 |  |
| A-16 | Spotlight coaching on the pill (owner; B-32, B-41 folded) | BUILD M | S-30 |  |
| A-17 | Position, size, animation | TUNE S | S-03 | Top-of-screen position kept as an Advanced option (R2 A-17) |

**A2. Hotkey grammar and editor**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| A-22 | Push-to-talk: hold fn (owner of the default) | EXTEND M | S-01 |  |
| A-23 | Hands-free: double-tap fn | EXTEND M | S-01 |  |
| A-24 | Hands-free: fn + Space (turn a hold into a lock) | BUILD M | S-01 |  |
| A-25 | Cancel: Esc, "Dismiss dictation and notifications" | EXTEND S | S-01 | one Esc registry (`fork/shortcut/actions.rs`): Esc blocked only while recording or processing; card (S-05), toasts (S-18), spotlight (S-30) observe it |
| A-26 | Paste last transcript (⌃⌘V) | BUILD S-M | S-06 |  |
| A-27 | Copy last transcript (⌃⌘C) | EXTEND S | S-06 |  |
| A-28 | "Press Enter" shortcut (key or mouse button; owner) | BUILD S | S-06 | mouse buttons gated by RS-11 |
| A-29 | Command Mode chord (fn + Ctrl) | EXTEND M | S-01 | chord only; behavior in S-23 |
| A-30 | Feature shortcuts plumbing (Scratchpad, Opt+M, Transforms, ⌥O) | EXTEND S | S-01 |  |
| A-31 | Several bindings per action ("+") | BUILD M | S-01 |  |
| A-32 | "Not set" bindings | EXTEND S | S-01 |  |
| A-33 | "Change hotkeys" panel (pencil, trash, +, Reset to default, Done) | EXTEND M | S-14 |  |
| A-35 | fn / Globe key support (owner; B-23 folded) | EXTEND M | S-01 | Globe handling gated by RS-01; fallback per DEC-44 (owner decision pending) |
| A-201 | Esc cancels while fn is held | EXTEND S | S-01 |  |
| A-202 | Dictation/mic key (F5) or Globe as hotkey | TUNE S | S-01 |  |
| A-203 | Hotkey labels on non-US layouts | EXTEND S | S-01 |  |
| A-208 | Silent loss of every hotkey when the event tap cannot be created | EXTEND S | S-02 |  |
| A-209 | Second dictation hotkey "Transcribe with Post-Processing" (⌥⇧Space) | TUNE S | S-12a |  |
| A-210 | Hold vs hands-free (locked) state on the pill | EXTEND S | S-03 |  |
| A-212 | Bar placement on multiple displays and display changes | BUILD S | S-03 |  |

**A3. Audio capture**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| A-37 | Microphone list and selection (owner; C-81 folded) | TUNE S | S-08 |  |
| A-38 | "Recommended" microphone (built-in); Bluetooth caution (owner; B-20, A-213 folded) | EXTEND S | S-08 |  |
| A-40 | Live level meter outside the pill / mic test (owner; B-19 folded) | BUILD M | S-08 |  |
| A-42 | Dictation sounds (owner; C-89 folded) | TUNE S | S-08 |  |
| A-211 | Mic lost mid-dictation | EXTEND S | S-08 |  |

**A4. Insertion**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| A-47 | Clipboard preserve and restore | EXTEND S | S-04 |  |
| A-48 | Keep transcripts out of clipboard managers | EXTEND S | S-04 |  |
| A-50 | Secure / password fields | EXTEND S | S-05 |  |
| A-51 | No focused text field: detection | BUILD M | S-05 |  |
| A-53 | Paste failure feedback | EXTEND S | S-04 | paste verification; card routing in S-05 |

**A5. System integration (tray, app menus, login, Dock)**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| A-56 | Tray menu content parity | EXTEND S | S-26a |  |
| A-57 | Copy/paste-last disabled when none exists | TUNE S | S-26a |  |
| A-59 | "Open app" item | TUNE S | S-26a |  |
| A-63 | App menus (owner of all: Dictation, My Voice, Help, Notetaker, Edit/Window) | BUILD M | S-26a | Help menu registered as the macOS help menu (search box) |
| A-65 | Show app in Dock toggle (owner; C-87 folded) | EXTEND S | S-14 |  |
| A-66 | "Show Flow Bar at all times" setting (owner; C-86 folded) | BUILD S | S-03 |  |

**B1. First-run journey**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| B-01 | Named-stage stepper | TUNE S | S-29a |  |
| B-02 | Back link top-left, Help chip bottom-left | TUNE S | S-29a |  |
| B-03 | Two-column layout; dark "Learn" screens | TUNE L | S-29a |  |
| B-06 | "How can Flow help you?" (Dictation / Notetaker) with live demo per card | BUILD S | N-12 | Dictation/Notetaker choice only once Notetaker exists |
| B-10 | Picture of the macOS prompt / Settings pane before it appears | TUNE M | S-29a |  |
| B-21 | Shortcut test: on-screen key lights when pressed | EXTEND S | S-29a |  |
| B-22 | Edit shortcut in place; Reset to default chip | TUNE S | S-29a |  |
| B-25 | Practice in a simulated app ("Write an email hands free"), Skip | TUNE S | S-29b |  |
| B-28 | Time-saved pitch with typing-time slider | BUILD S | S-29b | inside the Practice stage, as in R1 §13; slider default 2 h (DEC-50) |
| B-29 | 30-second typing test | BUILD M | S-29b | inside the Practice stage, with language picker and Recalculate |
| B-31 | "How would you like to use Flow first?" chooser with captioned demos, "Try it now" | BUILD M | S-30 |  |
| B-36 | Summary of choices at the end | TUNE S | S-29b |  |
| B-37 | Pace: ~5 min, never blocked | TUNE S | S-29b |  |
| B-201 | Shortcut test "No" path | BUILD S | S-29a |  |
| B-202 | Bootstrap model: first dictation within ~1 min on slow links | EXTEND M | S-07a |  |
| B-203 | Returning user whose model is missing | EXTEND S | S-07a |  |
| B-210 | Onboarding window closed mid-flow | EXTEND S | S-07c | inbox + macOS notification; S-07c is built after S-18 |

**B2. Guided learning everywhere**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| B-39 | "How it works" modal | EXTEND M | S-31 |  |
| B-40 | "Try it out" multi-step practice engine, then "Turn on X" | EXTEND M | S-31 |  |
| B-42 | Whisper tutorial ("Show me how", 3 steps) | BUILD M | S-32 |  |
| B-43 | Personalization quiz with mic recommendation table | BUILD M | S-32 |  |
| B-44 | Setup checklist with auto-ticking and next-step chaining (owner; C-10 folded) | BUILD M | S-30 | sidebar card as in R1; launch-at-login item; Transform and whisper items register in S-31 and S-32 |
| B-45 | Re-entry: "Start tutorial", Help > Essentials | TUNE S | S-31 |  |

**B3. Speech models**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| B-47 | No model choice for the user | EXTEND M | S-07b |  |
| B-49 | Hardware signal for the choice (GPU, RAM, disk, chip) | EXTEND M | S-07b |  |
| B-50 | Background download that never blocks setup | EXTEND L | S-07a |  |
| B-51 | Download progress UI | TUNE S | S-07a |  |
| B-52 | Resume an interrupted download | EXTEND S | S-07a |  |
| B-53 | Failure handling: offline, retry | TUNE S | S-07a |  |
| B-54 | Disk-space check before download | BUILD S | S-07a |  |
| B-55 | Model management page | TUNE S | S-14 |  |
| B-56 | Advanced override | TUNE S | S-14 |  |
| B-57 | Retired-model migration | TUNE S | S-07b |  |
| B-58 | Fallback auto-select when the selected model disappears | EXTEND S | S-07b |  |
| B-59 | Measure real speed and adapt | EXTEND S | S-07b |  |
| B-60 | Better default appears later | BUILD M | S-07b |  |
| B-204 | Model licence and attribution | BUILD S | S-39 | licence register in F-04; Acknowledgements page in S-39; CC-BY defaults per DEC-26 (pending) |
| B-205 | Where models live (shared HF cache) | TUNE S | S-27 |  |
| B-206 | Clean up superseded models after silent switches | EXTEND S | S-07b |  |
| B-207 | Model concepts outside the Models page | TUNE S | S-07b |  |
| B-208 | Slow-language guardrail | EXTEND S | S-07b |  |
| B-209 | Lower quantization for low disk/RAM | EXTEND M | deferred | later: lower quants for low disk/RAM; accuracy unmeasured, low priority (R2) |
| B-214 | Intel Mac and no-GPU policy | EXTEND S | S-07b | Intel per DEC-29 (owner decision pending) |

**B4. Languages**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| B-64 | Changing languages re-picks the model | EXTEND S | S-07b |  |
| B-67 | App (UI) language (owner; C-83 folded) | TUNE S | S-14 |  |
| B-213 | Language-coverage claim | TUNE S | S-29b |  |

**C1. Window chrome and navigation**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| C-01 | Task-grouped Hub sidebar | EXTEND M | S-13 |  |
| C-02 | Dictation home as the landing page (owner; B-34 folded) | EXTEND M | S-15a |  |
| C-12 | Settings as a two-column modal | EXTEND M | S-14 |  |
| C-13 | Help item and popover (owner of Help and Feedback structure) | BUILD S | S-26b |  |
| C-14 | Sidebar collapse button | BUILD S | S-13 |  |
| C-15 | "Beta" / "New!" tags | TUNE S | S-13 |  |
| C-16 | Top-right dark-mode toggle | TUNE S | S-13 |  |
| C-17 | Notifications bell and panel | BUILD M | S-18 |  |
| C-19 | Warm light theme | TUNE M | S-13 |  |
| C-21 | Serif display headings | TUNE S | S-13 |  |
| C-22 | Rounded cards, calm spacing | TUNE S | S-13 |  |
| C-23 | Dismissible example banner component (owner; B-38 folded) | BUILD S | S-13 |  |
| C-24 | Empty states with one clear action | TUNE S | S-13 |  |
| C-25 | Window chrome: integrated title bar, header, size | TUNE M | S-13 |  |
| C-26 | Footer content | TUNE S | S-13 | job status in S-13; version + update status join the footer in S-28 (E-51) |
| C-201 | Notifications panel controls and empty state | BUILD S | S-18 |  |
| C-202 | Feature-announcement popup ("Dark mode is finally here") | TUNE S | S-18 |  |
| C-204 | Deep links inside the Hub | EXTEND S | S-13 |  |
| C-205 | Page drop zones vs window-wide file drop | EXTEND S | S-13 | routing tested in S-13; the Dictionary `.csv` drop is tested in S-16 |
| C-206 | Empty states for every page | BUILD S | S-13 |  |
| C-210 | Sidebar keyboard and screen-reader access | TUNE S | S-13 |  |
| C-217 | Greeting name from macOS | BUILD S | S-15a |  |

**C2. Dictation home and history**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| C-28 | "Welcome back, <name>" header | BUILD S | S-15a |  |
| C-29 | Dismissible promo banner on home (owner; B-33 folded) | BUILD S | S-15a | banner; "Show me how" target built in S-32 |
| C-30 | Stats rail: total words | BUILD M | S-15a | storage in S-09 |
| C-31 | Stats rail: words per minute | BUILD S | S-15a | storage in S-09 |
| C-32 | Stats rail: day streak | BUILD S | S-15a | storage in S-09 |
| C-33 | "Your Voice Profile" progress bar | BUILD S | S-15a | unlocks at 1,700 words (DEC-50) |
| C-34 | History rows with full transcript | TUNE S | S-15b |  |
| C-35 | History grouped by date | EXTEND S | S-15b |  |
| C-36 | History search | BUILD S | S-15b | FTS index in S-09 |
| C-37 | Row actions on hover | TUNE S | S-15b |  |
| C-38 | Play audio per dictation | TUNE S | S-15b |  |
| C-39 | Copy transcript | TUNE S | S-15b |  |
| C-40 | Flag a dictation | EXTEND S | S-15b | writes through the S-16 store; S-15b depends on S-16 |
| C-41 | ⋮ overflow menu per row | BUILD S | S-15b |  |
| C-42 | Undo AI edit (owner; D-07 folded) | EXTEND S | S-15b | stored variants in S-09 |
| C-44 | Delete transcript | EXTEND S | S-15b | soft delete backend in S-09 |
| C-45 | Extract audio | EXTEND S | S-15b |  |
| C-46 | History and audio retention (owner; E-69 folded) | EXTEND M | S-09 |  |
| C-207 | History scale with long retention | EXTEND S | S-15b |  |
| C-208 | SQLite connection settings for more writers | EXTEND S | F-06 |  |
| C-209 | Typed added columns | EXTEND S | F-06 |  |
| C-212 | Stats counting rules | EXTEND S | S-09 |  |
| C-213 | Live refresh of stats rail and menu counts | EXTEND S | S-15a | stats rail live refresh; menu counts in S-26a |

**C3. Insights**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| C-50 | Insights page, tabs Your usage / Your voice | BUILD M | S-25 |  |
| C-51 | WPM with percentile | BUILD S | S-25 | no population percentile (needs cloud data); compare with the user's own typing speed |
| C-52 | Fixes made by Flow (owner; D-57 folded) | EXTEND M | S-25 | per-stage counts in S-11, LLM diff count in S-12a |
| C-54 | Usage by app category (owner of storage, map, chart; D-62 folded) | BUILD M | S-25 | app capture and daily_app_stats in S-09 |
| C-55 | Streak calendar | BUILD S | S-25 |  |
| C-56 | Share insights | BUILD S | S-25 |  |
| C-57 | Voice Profile cards (owner; D-63 folded) | BUILD M | S-25 |  |

**C4. Dictionary**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| C-58 | Dictionary page and store (owner) | EXTEND M | S-16 |  |
| C-60 | Search, sort, refresh | BUILD S | S-16 |  |
| C-61 | "Flow spells the way you do" banner | BUILD S | S-16 |  |
| C-62 | "Add to vocabulary" dialog | TUNE S | S-16 |  |
| C-64 | Import CSV wizard (owner; D-50 folded) | BUILD S | S-16 |  |
| C-65 | Export dictionary | BUILD S | S-16 |  |
| C-68 | Dictionary entry metadata | EXTEND S | S-16 |  |

**C5. Snippets**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| C-69 | Snippets page (list, tabs, search/sort/refresh, Add new) | BUILD M | S-17 |  |
| C-70 | Add snippet dialog (owner; D-54 folded) | BUILD S plain / L rich | S-17 | plain text with newlines (DEC-12, declared parity gap); `body_html` column reserved |
| C-71 | Snippet import (owner; D-55 folded) | BUILD S | S-17 |  |
| C-73 | Snippets example banner | BUILD S | S-17 |  |

**C6. Scratchpad (R1: Beta)**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| C-74 | Scratchpad page | BUILD M | S-24 |  |
| C-75 | Floating note window | BUILD L | S-24 |  |
| C-77 | Open Scratchpad shortcut (owner of the action) | BUILD S | S-24 |  |
| C-78 | Scratchpad open behavior | BUILD S | S-24 |  |
| C-216 | Notes search | BUILD S | S-24 |  |

**C7. Settings structure**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| C-79 | Settings grouped by task (General, System, Data and Privacy) | EXTEND M | S-14 |  |
| C-80 | Shortcuts row ("Hold fn and speak", Change) | EXTEND S | S-14 |  |
| C-84 | Theme | TUNE S | S-14 |  |
| C-91 | Notifications toggles: Suggestions, Announcements, Milestones | BUILD S | S-18 |  |
| C-98 | Technical and debug settings | TUNE S | S-14 |  |

**C8. Help and menus**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| C-99 | What's New (owner; E-89 folded) | TUNE S | S-28 |  |
| C-100 | Help > Shortcuts | BUILD S | S-26a |  |
| C-101 | Help > Microphone, Languages | EXTEND S | S-26a |  |
| C-103 | Talk to support / Share feedback (owner of local feedback; E-90 folded) | BUILD S | S-26b |  |
| C-104 | About / support development | TUNE S | S-26b |  |

**D1. Auto cleanup (R1: "Auto cleanup (Beta)")**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| D-01 | Cleanup on every dictation, same hotkey | EXTEND S | S-12a |  |
| D-02 | Level "None" = exact transcript | TUNE S | S-12a |  |
| D-03 | Level "Light" = fillers + grammar | EXTEND M | S-12a |  |
| D-04 | Level "Medium" = clarity and conciseness | EXTEND M | S-12a |  |
| D-05 | Three-level control instead of prompts | BUILD S | S-12a |  |
| D-06 | Original never lost (verbatim ASR text) | EXTEND S | S-09 |  |
| D-09 | Soft failure: cleanup failure keeps the text | TUNE S | S-12a |  |
| D-11 | Cleanup latency budget and cancel | EXTEND S | S-12a |  |
| D-12 | Cleanup must not answer or follow dictated instructions | EXTEND M | S-12a | metric defined in §11.1 (answer rate) |
| D-13 | Cleanup keeps the spoken language | TUNE S | S-12a |  |
| D-14 | Number words → digits, spoken punctuation (ITN) | EXTEND M (en) / L (multi) | S-33 | ships only if RS-13 passes |
| D-16 | Self-corrections ("no wait, I mean Tuesday") | EXTEND M | S-12a |  |
| D-208 | Cleanup on from the first dictation | BUILD S | S-12a |  |
| D-210 | Keep newlines / paragraphs through the deterministic stage | EXTEND S | S-11 |  |

**D2. Per-app Styles**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| D-18 | Style page with app groups (owner; C-07 folded) | BUILD M | S-19a |  |
| D-19 | Default app membership | BUILD S | S-19a |  |
| D-20 | "+" to add apps | BUILD S | S-19a |  |
| D-21 | Frontmost app detection at dictation time (owner of capture) | BUILD S | S-09 |  |
| D-22 | Web-app / site detection (Gmail in a browser) | BUILD L | S-19b |  |
| D-23 | Personal presets Formal / Casual / Very casual | BUILD S | S-19a |  |
| D-24 | Work/Email/Other presets Formal / Casual / Excited! | BUILD M | S-19a |  |
| D-25 | Before/after example cards | BUILD S | S-19a |  |
| D-26 | "Style formatting applies only in English" gate | BUILD S | S-19a |  |
| D-27 | Compose style with cleanup level | BUILD S | S-19a |  |
| D-207 | Edit app-group membership (remove, move, override) | BUILD S | S-19a |  |

**D3. Transforms (R1: Beta)**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| D-28 | Transforms opt-in toggle (owner page; C-08 folded) | BUILD S | S-22 |  |
| D-29 | Read the selected text in any app | BUILD L | S-21 |  |
| D-30 | Replace the selection in place | EXTEND M | S-21 |  |
| D-31 | ⌘Z undoes the transform | TUNE S | S-21 |  |
| D-32 | Per-transform hotkeys (⌥1, ⌥2) (owner) | EXTEND M | S-21 |  |
| D-33 | Polish rule toggles | BUILD S | S-22 |  |
| D-34 | Customize the transform prompt | EXTEND S | S-22 |  |
| D-35 | Writing examples (upload) to learn the style (D-65 folded) | BUILD M | S-22 |  |
| D-36 | Autosave and Reset per transform | TUNE S | S-22 |  |
| D-37 | Prompt Engineer transform | BUILD S | S-21 |  |
| D-38 | Create your own / Create New / Reset to defaults | EXTEND S | S-22 |  |
| D-40 | Diff view (⌥O), "N Changes" | BUILD M | S-22 |  |
| D-41 | Diff actions: Configure, copy, thumbs, retry | BUILD S | S-22 |  |
| D-42 | Auto Apply After Dictation | EXTEND S | S-22 |  |
| D-43 | Transform tutorial content | EXTEND M | S-31 |  |
| D-201 | Transform record (last transform: source, result, app, time) | BUILD M | S-21 |  |
| D-202 | Transform hotkey with nothing selected | BUILD S | S-21 |  |
| D-203 | Cancel and focus guard during a Transform | EXTEND S | S-21 |  |
| D-204 | Long selections vs model context | BUILD S | S-21 |  |
| D-205 | Polish detail modal left side | BUILD S | S-22 |  |
| D-206 | Transforms page banner and practice cards | BUILD S | S-22 | page banner; practice cards in S-31 |

**D4. Command Mode**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| D-44 | Command Mode: select text, speak an instruction | BUILD L | S-23 |  |
| D-45 | Command Mode without a selection | BUILD M | S-23 |  |

**D5. Dictionary, snippets, voice commands**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| D-47 | Dictionary words improve spelling | TUNE S | S-16 |  |
| D-48 | Misspelling → correct replacements (owner; C-63 folded) | EXTEND M | S-11 | engine; dialog fields in S-16 |
| D-49 | Dictionary steers the LLM cleanup | EXTEND S | S-12a |  |
| D-52 | Learns names and jargon automatically (owner; C-66 folded) | BUILD L | S-34 |  |
| D-53 | Snippet expansion (owner; C-72 folded) | BUILD M | S-17 |  |
| D-56 | Voice commands (new line, new paragraph) | BUILD M | S-11 |  |

**D6. Context, vibe coding, sending**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| D-58 | Context awareness (owner; C-95 folded) | BUILD L | S-20 |  |
| D-59 | Vibe coding: variable recognition in VS Code/Cursor/Windsurf | BUILD L | S-36 |  |
| D-60 | Vibe coding: file tagging in chat | BUILD L | S-36 |  |
| D-61 | Auto-send per app group (Handy-side idea) | EXTEND S | S-19a | off by default; never after a guard fallback, an unverified paste or a card |

**D8. Local LLM runtime and providers**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| D-67 | AI cleanup with no setup and no model choice (owner; E-49 folded) | BUILD L | S-10 | runtime; cleanup use in S-12a |
| D-68 | Apple Intelligence as on-device engine | EXTEND S | S-12b | own bridge in `crates/wb-apple-fm`; Handy's bridge untouched |
| D-69 | Cloud LLM providers (owner; E-76 folded) | TUNE S | F-08 |  |
| D-71 | Silent LLM model download | EXTEND M | S-10 |  |
| D-209 | Scratchpad "AI sparkle" | BUILD S | S-24 |  |
| D-211 | Warm the LLM at key-down | BUILD S | S-10 | ordered loads: LLM load waits for a cold ASR load (§4.5) |
| D-212 | Built-in local provider entry for the bundled runtime | EXTEND S | S-10 |  |
| D-213 | LLM memory/unload policy coordinated with ASR | BUILD M | S-10 | one idle owner (supervisor); ordered loads; cold-cold case in RS-06 |

**E1. Notetaker**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| E-01 | System audio capture (macOS) | EXTEND M | N-02 |  |
| E-02 | System-audio permission check and request (owner; B-14 folded) | BUILD S-M | N-02 |  |
| E-03 | Dual-source capture (mic + system), time-aligned | BUILD M | N-03 |  |
| E-04 | Echo / bleed handling | BUILD M | N-03 |  |
| E-05 | Start Notetaker: button, ⌥M, tray item (owner; A-58 partly folded) | EXTEND S | N-09 |  |
| E-06 | Detect any call automatically (B-15 folded) | BUILD M | N-10 |  |
| E-07 | Scheduled-meeting notify, next meeting in menu bar, upcoming meetings, hide next meeting | BUILD M | N-11 |  |
| E-08 | Auto-start Notetaker | BUILD S | N-10 |  |
| E-09 | Stop when call ends | BUILD S | N-10 |  |
| E-10 | Max recording length; memory for long recordings (E-84 folded) | BUILD S | N-02 |  |
| E-11 | "Join meeting" prompt | BUILD S | N-10 |  |
| E-12 | Docked right-edge panel window | BUILD M | N-04 |  |
| E-13 | Title (editable, auto-title on stop), "Today" chip, "Add folder" | BUILD S-M | N-07 | title field in N-04; auto-title in N-07 |
| E-14 | "My thoughts" notes tab | BUILD S-M | N-04 |  |
| E-15 | Live transcript tab | EXTEND M | N-05 |  |
| E-16 | "You" / "Them" bubbles | BUILD S | N-05 |  |
| E-17 | Stop / Resume | BUILD S | N-04 |  |
| E-18 | "Ask anything" + "What did I miss?" | BUILD L | N-07 |  |
| E-19 | Consent line | BUILD S | N-04 |  |
| E-20 | "We'll improve the transcript…" banner and tip cards | BUILD S | N-04 |  |
| E-21 | Post-meeting transcript re-pass | EXTEND M | N-06 |  |
| E-22 | Speaker diarization and naming | EXTEND L | N-06 |  |
| E-23 | Structured summary | BUILD L | N-07 |  |
| E-24 | Progress states | EXTEND S | N-06 |  |
| E-25 | "We hit a snag processing your meeting" + retry | EXTEND S | N-06 |  |
| E-26 | "Started by mistake? Discard / Keep" | EXTEND S | N-06 |  |
| E-27 | Screen-share visibility toast | BUILD S | N-04 |  |
| E-28 | Hide panels and bar from screen capture (owner; A-20 folded) | BUILD S | N-04 |  |
| E-29 | Pill recording state for Notetaker (owner; A-21 folded) | EXTEND S | N-04 |  |
| E-30 | Notetaker page (owner; C-03 folded) | BUILD M | N-08 |  |
| E-31 | Past notes list | BUILD M | N-08 |  |
| E-33 | Folders | BUILD S-M | N-08 |  |
| E-34 | Search across notes and transcripts | BUILD S-M | N-08 |  |
| E-35 | Notepad default view / side-by-side | BUILD S | N-04 |  |
| E-36 | "Show live transcript" toggle | BUILD S | N-09 |  |
| E-39 | Import meeting notes / recordings | EXTEND S-M | N-08 | local audio import only; Granola/Otter account import is cloud |
| E-42 | Calendar for speaker names and context (local) | BUILD M | N-11 |  |
| E-43 | Notetaker setup gate and 2-min guided tour | EXTEND M | N-12 |  |
| E-44 | Meeting audio kept, playback, extract | EXTEND M | N-08 |  |
| E-45 | Translate transcript | EXTEND M | N-08 |  |
| E-46 | Copy transcript / export | EXTEND S | N-05 |  |
| E-47 | Dictation while a meeting records | EXTEND L | N-01 |  |
| E-48 | Long-recording robustness (stream to disk, crash recovery, sleep) | BUILD M | N-02 |  |
| E-201 | Notetaker settings page (container) | BUILD S | N-09 |  |
| E-202 | Docked-panel header controls and "…" menu | BUILD S | N-04 |  |
| E-203 | Meeting store, lifecycle and retention | BUILD S-M | N-04 |  |
| E-204 | Search inside one transcript | BUILD S | N-05 |  |
| E-205 | Own-sound and mute contamination of the system track | BUILD S | N-02 |  |
| E-206 | Output-device changes during a meeting | BUILD S-M | N-02 |  |
| E-207 | Prevent sleep and App Nap while recording | BUILD S | N-02 |  |
| E-208 | "Join meeting": open the event's meeting link and start | BUILD S | N-11 |  |
| E-210 | System audio on Windows and Linux | EXTEND M | deferred | Windows/Linux system audio: later platform milestone |
| E-221 | "Try Notetaker" card on the Notetaker page | BUILD S | N-12 |  |

**E2. Platform, distribution and quality**

| Row | Feature | Class | Slice | Note |
|---|---|---|---|---|
| E-51 | Version + Update button (owner; C-27 folded) | TUNE S | S-28 |  |
| E-53 | Periodic update checks | EXTEND S | S-28 |  |
| E-54 | Our own update endpoint and signing key | TUNE S | F-02 |  |
| E-58 | Release workflow | TUNE S | F-02 |  |
| E-59 | Windows signing coupling | TUNE S | F-02 |  |
| E-60 | Linux packaging paths | TUNE S | deferred | Linux packaging paths: change when Linux returns |
| E-61 | Bundle identity rebrand | TUNE S | F-01 |  |
| E-62 | Brand strings and URLs (E-77 folded) | TUNE S-M | F-01 | web strings via i18n overrides; Rust strings via `fork/i18n.rs`; tray version label via the tray hook |
| E-63 | App icons | TUNE S | F-01 |  |
| E-64 | Data storage locations | TUNE S | S-27 |  |
| E-66 | Download / export your data (owner; C-94 folded) | BUILD S | S-27 |  |
| E-67 | Delete your data (owner; C-94 folded) | BUILD M | S-27 |  |
| E-68 | Data and Privacy page (owner; C-93 folded) | BUILD S | S-27 |  |
| E-72 | API key storage | TUNE S | F-08 | moot after DEC-04; key fields removed with cloud providers |
| E-74 | Model downloads (HF + mirror) | TUNE S | F-08 |  |
| E-75 | Update check on by default | TUNE S | S-28 |  |
| E-78 | Log handling | TUNE S | F-08 |  |
| E-79 | Crash handling | EXTEND S | S-27 |  |
| E-81 | Accessibility (VoiceOver, contrast) | TUNE M | S-37 |  |
| E-83 | Startup time and idle memory | unclassified  | S-38 | unclassified in R2; RC-10 budget |
| E-91 | Native notifications | BUILD S | S-18 |  |
| E-92 | Minimum macOS vs feature gating | TUNE S | F-01 |  |
| E-93 | Entitlements / usage strings | EXTEND S | N-02 | system-audio and calendar usage strings added only in M2 |
| E-95 | Frontend-backend contract (bindings) | unclassified  | F-05 | unclassified in R2; fork bindings file |
| E-211 | Fork-only Tauri config overlay | TUNE S | F-01 |  |
| E-212 | Updater key custody + CI identity guard | BUILD S | F-03 | key custody documented in F-02 |
| E-213 | Own version scheme and tag hygiene | TUNE S | F-01 | `Cargo.toml` version hotspot (DEC-48) |
| E-214 | macOS release verification job | BUILD S | F-02 |  |
| E-215 | Defer update install while recording | EXTEND S | S-28 |  |
| E-216 | App location, translocation | BUILD S | S-29a |  |
| E-217 | Distribution channels | BUILD S | S-39 |  |
| E-218 | Licences and attributions | BUILD S | F-04 | page shown in S-39 |
| E-219 | CI triggers for our branch | TUNE S | F-02 |  |
### 13.2 HAVE rows and the slice whose tests protect them

A-204 (transcript left on the clipboard) and B-212 (onboarding forces hold-or-toggle) are superseded by design: S-04 restores the clipboard and S-29a uses the fork grammar; those slices test the replacement behaviour.

| Slice | HAVE rows protected by its regression tests |
|---|---|
| F-01 | E-94 (Nix / immutable installs) |
| F-02 | E-55 (macOS Developer ID signing), E-56 (Notarization), E-57 (`latest.json` and signature) |
| F-03 | E-73 (No telemetry) |
| F-06 | E-71 (History DB migrations) |
| F-07 | E-70 (Settings persistence and migrations) |
| F-08 | D-70 (Custom OpenAI-compatible endpoint (Ollama)) |
| S-01 | A-34 (Tap vs hold threshold), A-45 (Orange mic indicator only while recording; DEC-47, chord and quick-tap check), A-206 (Press during processing queues the next dictation), A-67 (Start hidden, no-tray, CLI and signals), E-86 (CLI) |
| S-02 | A-205 (Secure Input explanation (tray warning, banner, diagnostic); now lists the fork's bindings and reaches the pill), A-207 (Hotkey tap self-heals after macOS disables it), B-17 (Returning-user permission check) |
| S-03 | A-07 (Cancel ✕ on the pill), A-08 (Live waveform), A-09 ("Arming" state before first mic sample), A-11 (Processing state (owner; D-10 folded)), A-18 (Live transcript panel while speaking), A-19 (Bar on all Spaces and over full-screen apps), A-52 (No focus stealing), A-68 (Overlay follows theme) |
| S-04 | A-46 (Paste mechanics), A-49 (Exactly-once insertion), A-54 (Auto-submit and trailing space), A-204 (Keep transcript on clipboard after paste) |
| S-07b | B-48 (Curated catalog with benchmarks and method), B-62 (Languages preselected from the system), B-63 (Warn when a language is poorly supported), B-65 (Automatic detection among my languages) |
| S-08 | A-39 (Auto-detect (follow system default)), A-41 (Mute all audio while dictating (owner; C-90 folded)), A-43 (Clamshell mic, input channel), A-44 (Mic permission and no-device errors) |
| S-10 | E-82 (Model load / unload) |
| S-11 | C-67 (Vocabulary used by the recognizer), D-15 (Stutter removal), D-17 (Custom filler list) |
| S-13 | C-20 (Dark theme), C-218 (Selected-card lilac state) |
| S-14 | A-36 (Keyboard backend choice), A-64 (Launch at login (owner; B-35, C-85, E-87 folded)), B-61 ("My languages" (owner; C-82 folded)), B-66 (Chinese script choice), C-88 (Show menu-bar (tray) icon), C-92 (Start hidden), D-214 (Handy extras: Translate to English, scriptable cleanup trigger) |
| S-15b | B-211 (Practice dictations appear in history), C-43 (Retry transcript (owner; D-08 folded)), C-47 (Background job status), C-48 (Transcribe a file (picker, drag-and-drop)), C-211 (Failed dictation kept with audio and retry) |
| S-24 | C-76 (Dictation into the focused note) |
| S-26a | A-55 (Menu-bar status icon with state), A-62 (Quit), A-69 (Tray icon recovery) |
| S-27 | E-65 (Open data / logs / recordings folders), E-88 (Portable mode) |
| S-28 | A-60 (Check for updates in tray (owner; E-52 folded)) |
| S-29a | B-04 (Primary button disabled until a choice), B-05 (Welcome screen), B-09 (Permission rows that tick when granted), B-11 (Mic permission: prompt, then System Settings), B-12 (Accessibility permission with polling and relaunch link), B-13 (Resume after relaunch), B-212 (Onboarding forces hold-or-toggle activation), B-46 (Permission waiting-state recovery) |
| S-29b | B-26 (Explain failed first takes), B-27 (first-dictation celebration) |
| S-31 | B-24 (Animated product demo) |
| S-37 | B-68 (i18n infrastructure and RTL), E-80 (i18n coverage) |
| S-38 | E-85 (Single instance) |
| not protected in M1 | B-18 (Windows/Linux permission handling) — Windows/Linux permission handling; covered when those platforms return |

### 13.3 OUT rows (excluded) and their local replacement

| Row | Feature | Local replacement (slice) |
|---|---|---|
| B-07 | "How do you currently record meetings?" | none (marketing quiz for the cloud meeting product) |
| B-08 | "What's frustrating about meetings?" | none |
| B-16 | Connect your calendar (Google/Outlook OAuth) | macOS Calendar via EventKit (N-11) |
| B-30 | Referral "Give the magic of Flow. Get a month of Pro." | Share with a friend (S-26b) |
| C-11 | Invite your team / Get a free month | Share with a friend (S-26b) |
| C-18 | Profile menu | greeting name from macOS (S-15a) |
| C-215 | Help "Contact sales" | none |
| C-53 | Total words desktop vs mobile | desktop totals only (S-25) |
| C-59 | Tabs All / Personal / Shared with team (D-51 folded) | local tabs All / Added by you / Suggested (S-16) |
| C-96 | Account, Team, Plans and Billing | About page (S-26b) |
| C-97 | Connectors (Calendar, Slack) and MCP settings | local calendar (N-11); vibe coding (S-36); Notetaker settings (N-09) |
| C-219 | Data toggle "Show your public information" | privacy page says there is no public profile (S-27) |
| C-220 | "Sync notes" and cross-device sync | privacy page says nothing syncs (S-27) |
| C-221 | "Dictation cloud storage" toggle | audio and text stay local; retention controls (S-27) |
| C-214 | Other cloud data toggles ("Improve the model for everyone", "Store data locally" dropdown) | privacy page states the facts; no telemetry (S-27, F-03) |
| C-102 | Help Center (hosted) | Help › Documentation links to repo docs (S-26a) |
| D-46 | Perplexity backend | Command Mode writes and edits only (S-23) |
| E-32 | Shared with me | none (local only) |
| E-37 | Consent calendar notice | copy a consent message (N-04) |
| E-38 | Sharing defaults, Share, copy link, auto-share | export Markdown/PDF (N-08) |
| E-40 | Calendar connectors (Google, Outlook OAuth) | EventKit (N-11) |
| E-41 | MCP for meeting notes | later: notes as Markdown files; no MCP in M1/M2 |
| E-220 | Platform coverage (macOS now, Windows later, mobile) | mobile out; Windows/Linux in a later milestone |

### 13.4 XREF rows (owned by another row)

Each XREF row is closed by its owner row's slice.

| Row | Owner row(s) | Slice of the owner |
|---|---|---|
| A-20 | E-28 | N-04 |
| A-21 | E-29 | N-04 |
| A-213 | A-38 | S-08 |
| A-58 | E-05, E-07 | N-09, N-11 |
| A-61 | C-102, C-13, C-103, A-56 | OUT, S-26b, S-26a |
| B-14 | E-02 | N-02 |
| B-15 | E-06 | N-10 |
| B-19 | A-40 | S-08 |
| B-20 | A-38, A-37 | S-08 |
| B-23 | A-22, A-35 | S-01 |
| B-32 | A-16 | S-30 |
| B-33 | C-29 | S-15a |
| B-34 | C-02, C-15 | S-15a, S-13 |
| B-35 | A-64 | S-14 (HAVE) |
| B-38 | C-23, C-29, C-61, C-73, D-206 | S-13, S-15a, S-16, S-17, S-22 |
| B-41 | A-16 | S-30 |
| C-03 | E-30 | N-08 |
| C-04 | C-50 | S-25 |
| C-05 | C-58 | S-16 |
| C-06 | C-69 | S-17 |
| C-07 | D-18 | S-19a |
| C-08 | D-28 | S-22 |
| C-09 | C-74 | S-24 |
| C-10 | B-44 | S-30 |
| C-27 | E-51 | S-28 |
| C-203 | A-63 | S-26a |
| C-49 | C-29, B-42, B-40 | S-15a, S-32, S-31 |
| C-63 | D-48 | S-11 |
| C-66 | D-52 | S-34 |
| C-72 | D-53 | S-17 |
| C-81 | A-37, A-38 | S-08 |
| C-82 | B-61 | S-14 (HAVE) |
| C-83 | B-67 | S-14 |
| C-85 | A-64 | S-14 (HAVE) |
| C-86 | A-66 | S-03 |
| C-87 | A-65 | S-14 |
| C-89 | A-42 | S-08 |
| C-90 | A-41 | S-08 (HAVE) |
| C-93 | E-68 | S-27 |
| C-94 | E-66, E-67 | S-27 |
| C-95 | D-58 | S-20 |
| C-105 | A-63, C-58, C-69 | S-26a, S-16, S-17 |
| C-106 | A-63 | S-26a |
| D-07 | C-42 | S-15b |
| D-08 | C-43 | S-15b (HAVE) |
| D-10 | A-11 | S-03 (HAVE) |
| D-39 | A-14 | S-03 |
| D-50 | C-64 | S-16 |
| D-51 | C-59 | OUT |
| D-54 | C-70 | S-17 |
| D-55 | C-71 | S-17 |
| D-57 | C-52 | S-25 |
| D-62 | C-54 | S-25 |
| D-63 | C-57 | S-25 |
| D-64 | A-63 | S-26a |
| D-65 | D-35 | S-22 |
| D-66 | C-214, E-73 | OUT, F-03 (HAVE) |
| E-49 | D-67 | S-10 |
| E-50 | B-07, B-08 | OUT |
| E-84 | E-10, E-48 | N-02 |
| E-209 | A-63 | S-26a |
| E-52 | A-60 | S-28 (HAVE) |
| E-69 | C-46 | S-09 |
| E-76 | D-69 | F-08 |
| E-77 | E-62, C-103 | F-01, S-26b |
| E-87 | A-64 | S-14 (HAVE) |
| E-89 | C-99 | S-28 |
| E-90 | C-103 | S-26b |

### 13.5 Write-ourselves items (R4 §3 + addendum §3)

| W | Item | Slice(s) |
|---|---|---|
| W-01 | LLM supervisor (port of TypeWhisper server code) | S-10 |
| W-02 | Built-in `local` provider and request options | S-10 (provider list hook first in F-08) |
| W-03 | Prompt builder + guard chain | S-12a (reused by S-12b, S-19a, S-21, S-23, N-07) |
| W-04 | CI build of static `llama-server` + `externalBin` | S-10 (local build for RS-06) |
| W-05 | AX extensions (selection, markers, bounds, title, URL) | S-05 (core), S-19b (URL), S-20 (caret), S-21 (selection) |
| W-06 | `HandyKeysState::new` fix + wake re-check + health UI (+ the fork's own blocking listener that replaces the earlier raw-event-forward idea) | S-01 (own `KeyboardListener::new_with_blocking` tap), S-02 (fix, health of both taps, Secure Input shadow) |
| W-07 | Clipboard snapshot/restore + paste verification + ⌘C fallback | S-04 (⌘C fallback used in S-21) |
| W-08 | Pill module, hover, screen observer, spotlight panel | S-03 (level and state hooks before Handy's overlay gate; spotlight in S-30) |
| W-09 | Meeting glue (tap → resampler/VAD → disk, probe, bleed pass, camera signal) | N-02, N-03, N-10 |
| W-10 | Calendar module | N-11 |
| W-11 | App menu + fork capability file | S-26a (capability file in F-05) |
| W-12 | Newline-safe stage wrapper | S-11 |
| W-13 | Dictionary rules (VoiceTypr port), snippet matcher, voice commands, scratch that | S-11 (rules, commands), S-17 (snippets) |
| W-14 | Word counts, `daily_stats`, streak, WPM | S-09 |
| W-15 | Fork DB schema, FTS, export, delete-all | F-06 (schema), S-09 (FTS), S-27 (export, delete-all) |
| W-16 | Diff card UI and Undo AI edit toggle | S-22 (diff), S-15b (undo toggle) |
| W-17 | Hub pieces (home, stats rail, banners, empty states, checklist, sidebar) | S-13, S-15a, S-30 |
| W-18 | Typing test and time-saved slider | S-29b |
| W-19 | Diarization one-shot mode + speaker merge | N-06 |
| W-20 | Summary module (map-reduce + schema) | N-07 |
| W-21 | Machine probe + silent model policy | S-07b |
| W-22 | Queued-audio first run | S-07a (in-session), S-07c (durable queue, notification) |
| W-23 | Release overlay, identity guard, verify job, tap-bump, upstream sync | F-01, F-02, F-03, F-09, S-39 |
| W-24 | Panic hook, log redaction, diagnostics export | F-08 (`fork/diagnostics/mod.rs`, `sanitizer.rs` marked AGPL-3.0-only and listed in NOTICE), S-27 (`crash.rs`, `bundle.rs`) |
| W-25 | `translate-missing` script | F-05 (script with pluggable backend: copy-English-and-flag; `llama-server` backend added in S-10), S-37 (strict mode: flags rejected) |
| W-26 | Third-party notices generator | F-04 (page in S-39) |
| W-27 | Dictionary auto-learn with approval queue | S-34 |
| W-28 | Vocabulary import from Handy and Wispr Flow | S-35 |
| W-29 | Native mute while dictating | S-08 |
| W-30 | GPL compliance (LICENSE, SPDX, NOTICE, source per release) | F-04 |

### 13.6 R4 recommended parts → slices

| R4 capability (part) | Slice(s) |
|---|---|
| Local LLM runtime (`llama-server`, pinned tag) | RS-06, S-10 |
| LLM models per RAM tier (Qwen3.5-2B, Qwen3-4B-2507; challengers) | RS-06, RS-07, S-10 |
| Apple Foundation Models bridge (hex) | RS-08, S-12b (in `crates/wb-apple-fm`) |
| Cleanup prompts and guards (OpenWhispr, FreeFlow, SpeakoFlow; + BW, TypeWhisper, FluidVoice, Voquill evals) | F-10, RS-07, S-12a |
| Accessibility client (SpeakoFlow on objc2-application-services; + VoiceInk, TypeWhisper, FluidVoice) | RS-04, S-05, S-20 |
| Selected-text read (SelectedTextKit, SpeakoFlow `selection.rs`; OpenLess reference) | S-21 |
| Frontmost app / window title / browser URL (NSWorkspace, mado port; + BW `WebsiteReader`, Tambourine origin-only) | S-09, S-19b |
| Hotkeys and tap health (handy-keys `KeyboardListener::new_with_blocking`, anarlog processor, axwatch; + MacParakeet/FluidVoice vectors, VoiceInk triggers) | RS-01, RS-02, RS-14, S-01, S-02 |
| Secure Input holder (Yap) | S-02 |
| Clipboard (NSPasteboard snapshot, hex `paste.rs`, arboard; + paste verification) | RS-05, S-04 |
| Clipboard-free typing (MacParakeet, OpenLess; reference) | S-04 (only if hex's typing path proves weak) |
| Pill / overlay (tauri-nspanel 2.1.1, Tolkovo pattern, anarlog click-through; Voquill pill reference) | RS-03, S-03, S-24, S-30 |
| Mute while dictating (Tambourine) | S-08 |
| System audio (hex tap) | N-02 |
| Call detection (hex; + camera signal) | N-10 |
| Calendar (objc2-event-kit, anarlog shape; + MacParakeet tests) | N-11 |
| Notifications (tauri-plugin-notification 2.4.0) | RS-10, S-18 |
| Menus, capabilities, Dock (Tauri `Menu::default`, fork capability file) | F-05, S-14, S-26a |
| Text pipeline (aho-corasick, unicode-segmentation; VoiceTypr rules, MacParakeet tables) | S-11, S-17 |
| ITN (text-processing-rs, text2num) | RS-13, S-33 |
| Word counts (unicode-segmentation + CJK rule) | S-09 |
| Diff (`similar`) | S-12a, S-22, S-25 |
| Fork database, FTS, stats (rusqlite, rusqlite_migration) | F-06, S-09 |
| Import / export (papaparse, csv, zip) | S-16, S-17, S-27 |
| UI primitives (Radix, sonner, lucide) | S-13 onward |
| Charts (Recharts, react-activity-calendar) | S-25 |
| Virtual list (@tanstack/react-virtual) | S-15b |
| Tours / spotlight (driver.js; own pill spotlight) | S-30, S-31 |
| Celebrations (canvas-confetti) | S-29b |
| Fonts (Newsreader, Source Serif 4) | S-13 |
| i18n / a11y tooling (i18next-cli, jsx-a11y, translate script) | F-03, F-05, S-37 |
| Diarization (sherpa-onnx + seg-3.0 + CAM++) | N-06 |
| Echo handling (hex `meeting_echo.rs`; FluidVoice AEC3 option) | N-03 |
| Meeting summaries (anarlog prompts, Meetily map-reduce) | N-07 |
| Hardware probe (sysinfo) | S-07b |
| Bootstrap model (whisper-base) + queued audio | RS-09, S-07a, S-07c |
| Release pipeline (tauri-action, overlay) | F-01, F-02 |
| Updater (tauri-plugin-updater 2.10.1) | F-02, S-28 |
| Distribution channels (Homebrew tap, listings) | S-39 |
| Crash / diagnostics (panic hook, log, Voquill sanitizer) | F-08, S-27 |
| Upstream sync (own workflow) | F-09 |
| Windows / Linux ports (Voquill reference) | later platform milestone (not M1/M2) |

### 13.7 Runtime checks → spikes

| R2 RC | R4 RT | Spike |
|---|---|---|
| RC-01 | RT-13 | RS-03 |
| RC-02 | RT-18 | RS-01 |
| RC-03 | RT-14 | RS-03 |
| RC-04 | RT-20, RT-21, RT-22, RT-23 | RS-04 |
| RC-05 | – | RS-05 |
| RC-06 | – | RS-11 |
| RC-07 | – | RS-11 |
| RC-08 | RT-25..RT-28 | N-02 (M2) |
| RC-09 | RT-31 | RS-09 |
| RC-10 | RT-17 | RS-03, S-38 |
| RC-11 | RT-01..RT-06, RT-09..RT-12 | RS-06, RS-07 |
| RC-12 | RT-35, RT-36 | RS-12 |
| RC-13 | – | RS-11 (⌥M in N-09) |
| – | RT-07 | RS-08 |
| – | RT-08 | RS-07, S-12a |
| – | RT-15, RT-16 | RS-03 |
| – | RT-19 | RS-02 |
| – | RT-24 | RS-05 |
| – | RT-29 | N-10 (M2) |
| – | RT-30 | RS-10 |
| – | RT-32, RT-33 | N-06 (M2) |
| – | RT-34 | N-03 (M2) |
| – | RT-37 | RS-09 |
| – | RT-38 | S-39 |
| – | RT-39 | RS-13 |

---
| – | Secure Event Input (R3 §3) | RS-14 |

---

## 14. Review resolution log

Source: `docs/research/r5-review.md`. Status: **fixed** = applied as the review asked or with an equivalent verified mechanism; **partly fixed** = valid part applied, the rest kept with a reason; **rejected** = kept as is, with a reason. Paths in the Evidence column were opened during this revision (Handy at fork state; dependency sources from `~/.cargo/registry`).

| RV | Status | What changed (section/slice) | Evidence checked |
|---|---|---|---|
| RV-01 | fixed | Route (a) chosen and specified: fork commands are app commands `fork_*` with a second tauri-specta builder; one `lib.rs:1132` line `.invoke_handler(fork::compose_invoke(invoke_handler))` routes by name before the call. Plugin registers no commands. `fork::commands!()` fallback deleted. CI asserts plain `tauri_build::build()` and no `fork_` Handy names (§4.1, §4.2, §4.12, DEC-46, F-05) | `tauri-2.11.5/src/webview/mod.rs:1795-1857` (ACL only for `plugin:`, app manifest or remote); `src-tauri/build.rs` ends in `tauri_build::build()`; `lib.rs:806, 1132`; `ipc/mod.rs:211-219, 543` (`Invoke` fields and `command()` public); `tauri-specta-2.0.0-rc.21/src/builder.rs:85-125, 252-255` (`plugin_name` optional, `invoke_handler`) |
| RV-02 | fixed | Init-order rule in §4.1: plugin `setup` registers state only; manager-dependent work in `fork::on_ready` from `on_event(RunEvent::Ready)`; `NotReady` errors; headless skip. F-05 test calls `fork_model_status` before and after Ready | `tauri-2.11.5/src/app.rs:2440` (plugins in `build`), `:1423-1428` and `:2521-2533` (app setup on Ready, then `on_event_loop_event`), `:2600-2650` (plugins' `on_event` dispatch); `lib.rs:1049-1051` (`initialize_core_logic` synchronous in setup) |
| RV-03 | fixed | §4.8 rewritten: own `KeyboardListener::new_with_blocking` tap with a runtime `BlockingHotkeys` set (`{FN}` always, `{FN,Space}` while fn down, `{Esc}` only while active); raw-event hook dropped; `owns()` moved to `shortcut/mod.rs` `register_shortcut`/`unregister_shortcut`; RS-01 tests both taps; S-02 health watches both | `handy-keys-0.3.4/src/listener.rs:31-70`, `platform/state.rs:14, 40-48`, `platform/macos/listener.rs:295-335` (fn-down FlagsChanged blocked when `{FN}` matches); `shortcut/handy_keys.rs:114` (`HotkeyManager::new_with_blocking`), `:269` (listener only in capture); `shortcut/mod.rs:134-150, 287-312` |
| RV-04 | fixed | Five first-line hooks in `overlay.rs`: `emit_levels` before the `OVERLAY_ENABLED` gate, `show_overlay_state`, `emit_recording_ready`, `hide_recording_overlay`, and a skip at the top of macOS `create_recording_overlay`. §5.2 and RS-03 measure all WebKit processes together | `overlay.rs:452, 488, 599, 689, 730-741` (gate), `:766` (`emit_to("recording_overlay", "mic-level")`); `transcription_coordinator.rs:38` (typed events broadcast) |
| RV-05 | fixed | §4.12 network guard split: fork-path scan vs `ci/allowed-hosts.txt`; upstream scan vs reviewed `ci/upstream-hosts-baseline.txt`; built-bundle scan with `ci/inert-hosts.txt` proofs; runtime egress test `wb-egress.yml` with a pf deny-all anchor. Telemetry guard uses exact package names (`ci/telemetry-packages.txt`) | `grep` over `src`, `src-tauri/src`: `api.openai.com` 28, `api.anthropic.com` 27, `blob.handy.computer` 6, `openrouter.ai`, `api.z.ai`, `api.groq.com`, `api.cerebras.ai`, `api.deepseek.com`, `bedrock-mantle…aws`; 26 `translation.json` files contain `api.anthropic.com`; `Cargo.lock:7461` `unicode-segmentation` |
| RV-06 | fixed | DEC-01, §2.1, §4.7, §5.4: queued and late takes go to the card (same session) or History + one notification with Copy latest / Open; clipboard written only on Copy or ⌃⌘V. S-07c test "clipboard unchanged after delivery" | R5 DEC-01 (old text); R4 "Bootstrap model" row ("deliver by copy + notification") overridden |
| RV-07 | fixed | Translate script created in F-05 with pluggable backend (copy-and-flag, then `llama-server` from S-10); flags pass CI until `ci/i18n-strict` (S-37); human review of the 10 sample-text languages before 1.0 (§5.6, §4.12, W-25) | old §8 preamble, S-37, §13.5 W-25 |
| RV-08 | fixed | §4.8 states what works under Secure Input (fn hold, double-tap, Ctrl flags; not fn+Space; Esc via the Carbon shadow, else ✕). New spike RS-14. §9 Esc criterion qualified. A-205 message on the pill (S-02) | `secure_input.rs:1-20` (taps lose KeyDown, FlagsChanged flows; Carbon-backed shadow for keyed bindings and cancel); R3 §3 "Secure Event Input" row |
| RV-09 | fixed | DEC-47: mic opens after `T_show` or at once on a second press/Space; warm-stream pre-roll only. §5.1 rows changed; RS-01 counts indicator flashes and first-word clipping; A-45 now protected by S-01 with a chord + quick-tap test | R2 A-45 (`settings.rs:580-582`, `managers/audio.rs:665-687`); `managers/audio.rs:20` (`STREAM_IDLE_TIMEOUT` 30 s) |
| RV-10 | fixed | RS-01 pass criteria: no Globe action on tap, hold or double-tap for all four values; 100/100 chords keep native function; fallback for "chords broken" added | RS-01 row |
| RV-11 | fixed (different mechanism) | Apple FM bridge in its own crate `src-tauri/crates/wb-apple-fm` (own `build.rs`, `fork_apple_fm.swift`, symbol prefix) + `fork/llm/apple.rs`; Handy's bridge and `build.rs` untouched; new slice S-12b. A separate crate needs no `build.rs` hook at all, which is thinner than the suggested hook | `build.rs:371-541` (`REAL_SWIFT_FILE = "swift/apple_intelligence.swift"`, links via swiftc) |
| RV-12 | partly fixed | Added to §4.2: `vite.config.ts` spread hook, `build.rs` (listed as not edited, CI-asserted), `managers/audio.rs:204` `get_mute`, `Cargo.toml`/`package.json` version, `utils.rs:102`, `secure_input.rs`, `shortcut/mod.rs`. Playwright: separate `playwright.fork.config.ts` instead of editing `playwright.config.ts` (no hotspot). F-09 re-checks against `git diff upstream/main --stat`; hook-size guard added | `vite.config.ts:20-25`; `managers/audio.rs:100-111, 195-215`; `playwright.config.ts` exists |
| RV-13 | fixed | `fork/i18n.rs` embeds the 26 `fork.json` files (`rust.*` keys), locale from Handy's app-language setting; tray title/version item replaced through the tray hook in F-01; tests check tray and menu strings (§4.1, DEC-35, F-01, S-26a) | `build.rs:270` `generate_tray_translations`; `tray.rs:455-466` (`"Handy v{}"`) |
| RV-14 | fixed | Route (a): `Cargo.toml` and `package.json` `version` lines are a listed hotspot (DEC-48); identity guard checks Cargo = overlay = tag | `src-tauri/Cargo.toml:3` (`0.9.8`); `tauri.conf.json:4`; `settings.rs:604-606`; `tray.rs:461-463` |
| RV-15 | fixed | Forward dependencies removed: `paste_outcome` stored in S-09; offline queue split to S-07c after S-18; S-06 depends on S-03; S-15b depends on S-16; menu counts moved to S-26a; checklist items register in S-31/S-32; C-205 import test moved to S-16; tray update item routed in S-28; panels before S-14/S-22 mount on the F-05 debug panel; beta channel endpoint in F-02, external beta from S-28; S-27 auto-learn row added by S-34; S-12a picker no longer placed in a Settings page that does not exist yet | slice blocks S-04, S-06, S-07, S-12, S-13, S-15, S-16, S-17, S-21, S-26, S-27, S-30, §11.4 (old) |
| RV-16 | partly fixed | Split: S-07 → S-07a/b/c; S-12 → S-12a/b; S-15 → S-15a/b; S-19 → S-19a/b; S-26 → S-26a/b; S-29 → S-29a/b (typing test and time saved moved from S-30 into S-29b). S-13 kept whole: its 15 rows are chrome pieces on one set of primitives with one test surface; splitting would create a half-built shell | old row counts; §13.1 re-mapped |
| RV-17 | fixed | Cancel hook in `utils.rs:102` (`fork::recovery::cancel_keeping_audio`), optional upstream-bound `take_samples()`; Undo pastes into the original field if still focused, else card; test cancel → Undo (S-06) | `utils.rs:100-133` (`cancel_recording` then coordinator notify); `managers/audio.rs:1195, 1278` |
| RV-18 | fixed | §4.5 ordered loads (LLM waits for a cold ASR load), one idle owner (supervisor; `--sleep-idle-seconds` removed); §5.1 cold-cold row; RS-06 case | old §4.5 spawn line and memory policy |
| RV-19 | fixed | DEC-49: Unix socket only if RS-06 shows `/health` on `--host <path>.sock`, else loopback random port + API key; `--offline` added to the spawn line when the tag accepts it | `reqwest-0.12.28/src/async_impl/client.rs:1808-1812` (`unix_socket`, `#[cfg(unix)]`); locked `reqwest 0.12.28`, `hyper 1.8.1`, `hyper-util 0.1.20` |
| RV-20 | fixed | §11.1 defines labels, content damage, answer rate, filler removal, language kept, guard catch rate, pre/post-guard use; two labellers; no LLM judge; F-10 README and metric tests | RS-07 row; F-10 |
| RV-21 | fixed (owner decided: CC-BY defaults allowed with attribution) | DEC-26 escalated, then settled; RS-09 benchmarks both sets; S-07b keeps both pick tables; F-04 NOTICE conditional | R4 §7 "Owner decisions needed first", §8 Q1 |
| RV-22 | fixed (owner decided: consent + private API, guidance sheet if declined) | New DEC-44 escalated, then settled; DEC-28 narrowed to `_setPreventsActivation:`; RS-01 fallback points at DEC-44 | R4 §8 Q8; owner decision "default hotkey fn" (DEC-09) |
| RV-23 | fixed (owner decided: Apple Silicon only now, Intel later) | DEC-29 escalated, then settled; every Intel item tagged "(DEC-29)" (§5, RS-06/09/10, F-02 matrix, S-39 cask, §9, §11.3) | R4 §8 Q10 |
| RV-24 | fixed | Own-words rule in §1 and DEC-45; every quoted Wispr sentence in slices replaced by our wording (e.g. "Click into a text field first", "Teach Wispr Bro your words", "Nothing new", quiz questions rewritten); S-37 copy check script | R1 §4, §10-§13 quotes vs old S-03, S-05, S-16, S-18, S-22, S-29, S-30, S-32 |
| RV-25 | fixed | Exact CSP and `devCsp` in §4.11 incl. `ipc:`, `http://ipc.localhost`, asset protocol in `img-src`/`media-src`, `data:`/`blob:` images, narrowed asset scope; real-build smoke in F-08 and S-25 | `tauri.conf.json:16-24` (`csp: null`, asset protocol scope `**`); `HistorySettings.tsx:8, 221` (`convertFileSrc`); `tauri-utils-2.9.3/src/config.rs:2897-2907` (`devCsp`) |
| RV-26 | fixed | S-29b practice: records with "Setting up… N%", fills when the starter arrives, Skip always, auto-advance at 90 s with the take kept; Playwright slow-download case | R2 AF-03; old S-29 |
| RV-27 | fixed | §5.4 negative-case list with owners: S-08 mic revoke, S-07a hash/429/portal, S-04 sleep/lid/user switch and IME, S-09 disk full, S-05 app switch, S-10 sidecar on wake. Target rule in §4.6 (element focused at key-up; focus change during processing → card) | checklist item 4; R3 §3 |
| RV-28 | fixed | S-38 soak harness specified: test-only virtual input (BlackHole), `--toggle-transcription`, scripted focus, AX read-back, per-take outcome log; plus a 50-take hardware fn run (§9) | `AGENTS.md` CLI table (`--transcribe-file` is headless) |
| RV-29 | fixed | §4.3 consistency rules: idempotent upserts by `entry_id`, launch reconcile (backfill, orphan drop), retention deletes Handy rows only via Handy's history manager; S-09 kill-between-writes test | old §4.3 and §4.4 |
| RV-30 | fixed | F-02: `fork/update/channel.rs` uses `updater_builder().endpoints(...)`; beta is a GitHub prerelease; x86_64 smoke under Rosetta or skipped with a reason (and only if DEC-29 keeps Intel); identity guard checks pubkey and endpoint | `.github/workflows/release.yml:52-54` (`macos-latest` x86_64 cross-build); `tauri-plugin-updater-2.10.1/src/updater.rs:197` (`endpoints`) |
| RV-31 | fixed | DEC-31 and S-01 rule 2: with Command Mode off, Ctrl passes through and the take stays dictation; test vector added; S-23 tests on and off | old S-01 rule 2 |
| RV-32 | fixed | `fork/diagnostics/{mod,sanitizer}.rs` in F-08 (sanitizer marked AGPL-3.0-only, in NOTICE); S-27 adds `crash.rs`, `bundle.rs` (§4.1, §5.7, W-24) | old F-08 and S-27 new-file lists; addendum §4 |
| RV-33 | fixed | §4.4 `ENFORCED` vs `FIRST_RUN_DEFAULTS` lists; provider rule keeps a loopback `custom`; F-07 tests that Advanced choices persist | old §4.4 |
| RV-34 | fixed | §4.1 CI row lists kept-enabled (`test`, `code-quality`, `playwright`) and disabled (`release`, `build`, `main-build`, `pr-test-build`, `build-test`, `nix-check`) workflows | `.github/workflows/` listing; triggers in `test.yml:2-8`, `playwright.yml:2-4`, `build-test.yml:3` |
| RV-35 | fixed | F-02 "Owner inputs" box; release fails while `<owner>`/`<hf-org>` placeholders remain (§1, §4.12) | §1 table; DEC-34 |
| RV-36 | fixed | §4.1 holds the full submodule list; onboarding state lives in `fork/settings.rs` with `onboarding/state.rs` as logic; custom-word migration only in S-16 (removed from S-11) | old slice new-file lists |
| RV-37 | fixed | §4.3 defines `style_group`, `usage_category` and `is_ai_chat` (one value each per app, in `categories.json`, tested in S-09); `entry_meta.app_category` split into two columns | old §4.3, S-12, S-19, S-25 |
| RV-38 | fixed | S-20: ≥ +20 points on a 30-item name set and ≤ 100 ms added p95; RS-03 hover criterion; RS-04 ≥ 20 focus events per app; S-23 rubric in `eval/command/RUBRIC.md`; "stuck" defined in S-29b; S-37 VoiceOver checklist file | old S-20, S-03/RS-03, RS-04, S-23, S-29, S-37 |
| RV-39 | fixed | Esc registry in `fork/shortcut/actions.rs`; Esc blocked only while recording or processing; cards, toasts and spotlight observe it (§4.8, S-01 rule 7, A-25 note) | old S-01 rule 7 |
| RV-40 | fixed | DEC-32: silence auto-stop at 120 s (warning at 100 s); auto-stopped take pastes only into the field captured at start, else card; 30 min cap kept | R3 §3 "Session cap" (warning 19 min, stop 20 min) |
| RV-41 | fixed | DEC-38: resting bar hidden until setup ends; active states show during onboarding practice | old DEC-38 |
| RV-42 | partly fixed | Plain text kept for M1 because R4 has no licence-audited rich editor (R4 line 154: TipTap/Lexical "only if rich snippets", plain in v1). Build-once safeguard added (nullable `body_html`, both paste paths already multi-flavour) and the gap declared in §9 | R4-parts-bin.md:154; R2 DEC-12 and C-70 |
| RV-43 | fixed | §1 principle 6 now "within a minute of granting permissions (≤ 2 min from install)"; §5.3 adds the 60 s row | old §1 and §5.3 |
| RV-44 | fixed | `secure_input.rs` hook `fork::shortcut::secure_input_bindings` makes the covered/uncovered lists describe the fork's bindings; pill message (S-02) | `secure_input.rs:20-45` (`covered_bindings`, `uncovered_bindings`) |
| RV-45 | fixed | RS-03 adds sleep/wake, display unplug/replug, mixed-scale displays and combined RSS of all WebKit processes against §5.2 | old RS-03 |
| RV-46 | fixed | SPDX guard exempts files listed in `ci/generated.txt` (`*.gen.json`, `src/fork/bindings.ts`) | old §4.12 |
| RV-47 | fixed (verified) | tauri-specta rc.21 has no per-event name override: names are kebab-case of the type name. Rule: fork event types are named `Fork…` (e.g. `fork-pill-state`); `fork://` prefix dropped (§4.1) | `tauri-specta-macros-2.0.0-rc.16/src/lib.rs:15-31` (`to_kebab_case`, no attribute parsing); `tauri-specta-2.0.0-rc.21/src/event.rs:20-47` (shared `EventRegistry`, `get_or_manage`) |
| RV-48 | fixed | S-19a: per-app auto-send off by default, "sends automatically" hint on the pill, never after a guard fallback, unverified paste or card; D-61 note | R2 D-61 note |

**R1 parity gaps (review section "Missing from R1 parity").**

| # | Gap | Resolution |
|---|---|---|
| P-1 | Quiz step "right microphone" and the "more info" link | S-32 quiz step (1) with our own illustrations and a "More about mics" link to `docs/user/microphones.md` |
| P-2 | Tooltips on the three-button bar | S-24 slot API with per-button tooltips |
| P-3 | Checklist location; launch-at-login item | S-30: sidebar card (R1); Launch at login item; registry so later items add themselves |
| P-4 | Typing test and time saved inside the LEARN stage; language picker, Recalculate, typing-test link | Moved into S-29b (Practice stage) with all three controls |
| P-5 | Slider 6 h vs 2 h; Voice Profile 1.7K vs 2,000 | DEC-50: slider 2 h kept as a declared deviation (honest estimate); Voice Profile unlock set to 1,700 (parity) |
| P-6 | Four-step Import wizard with format link; Add disabled until filled | S-16 spells out the four steps, the format link and the disabled Add button |
| P-7 | Help menu search box | S-26a registers the Help menu with `set_as_help_menu_for_nsapp()` (`tauri-2.11.5/src/menu/submenu.rs:390`) |
| P-8 | "Transformed!" practice information | S-31 success step gives the same information (⌥O to view changes, ⌘Z to undo) in our own words |
| P-9 | Mic test advice and Change list order | S-08 list order and caution; S-29a mic test copy (our words) |
| P-10 | Time-saved counter starts at 0 and updates after the test | S-29b |
| P-11 | Theme options | DEC-14: Light, Dark, System; default System |
| P-12 | Handy's Top-of-screen position | Kept as an Advanced option (S-03, S-14, A-17 note) |

**Traceability fixes.** A-45: now protected by S-01 (the slice that could break it) with a chord and quick-tap indicator test. B-210: moved to S-07c, built after S-18. C-40: S-15b depends on S-16. A-17, A-205, C-26, C-205, C-213, D-12, D-213, E-62: notes and owning slices updated as listed above. W-25 → F-05 (script) and S-37 (strict); W-24 → F-08 `diagnostics/sanitizer.rs` (AGPL-marked) and S-27; W-06 → S-01 (own blocking tap) and S-02 (fix and health).

**Coverage re-check (after this revision).** A script (`coverage.py`, kept outside the repo) parsed R2 §5 and this file: 492 R2 row ids, each placed exactly once (325 in §13.1, 76 in §13.2, 23 in §13.3, 68 in §13.4); every F/S slice's "R2 rows closed" and "Regression protected" lists equal the appendix entries for that slice (split slices included); every slice id named in the appendix exists. S-35 lists no rows by design.

---

## 15. Summary

1. Slices: M0 has 10 (F-01..F-10), M0.5 has 14 real-Mac spikes (RS-01..RS-14), M1 has 46 (S-01..S-39 with S-07, S-12, S-15, S-19, S-26 and S-29 split into lettered slices), M2 has 13 coarse slices (N-01..N-13).
2. First five to build: F-05 fork scaffolding (composed invoke handler, `on_ready`, translate script), F-07 fork settings store, F-01 identity overlay (incl. version line and Rust strings), F-06 `fork.db`, F-02 signing/notarization/updater/channels; spikes RS-01, RS-03, RS-04, RS-05, RS-07, RS-09 start in parallel on dev builds.
3. First five M1 slices: S-01 hotkey engine on the fork's own blocking tap, S-03 Flow Bar, S-02 hotkey self-healing and Secure Input, S-04 insertion and clipboard, S-05 AX layer and "no text field" card.
4. Riskiest assumption 1: macOS Globe actions can be suppressed by blocking the fn press without breaking fn chords (RS-01); where not, the owner picks a DEC-44 option.
5. Riskiest assumption 2: a 2B local model plus the ASR model fit an 8 GB Mac and clean 50 words within 2.2 s p95, also from cold with ordered loads (RS-06).
6. Riskiest assumption 3: small local models reach ≤ 2 % content damage at Light with our guards, as defined in §11.1 (RS-07).
7. Riskiest assumption 4: an always-visible webview pill stays clickable without focus loss and the whole app stays within 250 MB idle with Handy's overlay never built (RS-03).
8. Riskiest assumption 5: fail-open AX decisions avoid false "no text field" cards in Electron/Chromium apps, and a statically built `llama-server` signs and runs on macOS 13 (RS-04, RS-06, RS-10).
9. Every R2 row is placed: 325 TUNE/EXTEND/BUILD/unclassified rows map to a slice; only B-209 (lower quants), E-60 (Linux packaging) and E-210 (Windows/Linux system audio) are deferred, with reasons. Not protected in M1: HAVE row B-18 (Windows/Linux permission handling).
10. Three owner decisions are pending: DEC-26 (CC-BY-4.0 speech weights as silent defaults), DEC-44 (Globe key when suppression fails), DEC-29 (Intel support).
