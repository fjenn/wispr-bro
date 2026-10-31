# Wispr Bro: start here

Wispr Bro is a free, fully local, open-source (GPL-3.0) macOS dictation app. Hold fn, speak, release, and clean text lands in whatever field has focus. It matches Wispr Flow's consumer experience, but everything runs on the Mac: no account, no cloud, no telemetry, no usage caps, auto-updating, and the user never picks a model. Apple Silicon first; Intel, Windows and Linux later.

This repo is a fork of [Handy](https://github.com/cjpais/Handy) (MIT; Tauri 2 + Rust + React 18/TypeScript). Branch `wispr-bro` is ours; `main` tracks the `upstream` remote.

## Read in this order

1. **This file.**
2. **`docs/research/R5-prd-and-slices.md`**: the plan. Product rules (§1), scope (§2), decisions (§3), fork architecture (§4), performance budgets (§5), slices F-xx, RS-xx, S-xx, N-xx (§6-§10), exit criteria (§9). It is the source of truth for what to build next.
3. When a slice needs detail, follow its ids into:
   - `docs/research/R1-wispr-flow-teardown.md`: how Wispr Flow looks and behaves (the target experience).
   - `docs/research/R2-gap-analysis.md`: every feature compared with Handy's code (row ids A-xx..E-xx, architecture findings AF-xx, decisions DEC-xx, runtime checks RC-xx).
   - `docs/research/R3-landscape.md`: how other apps solved the same problems, plus Wispr Flow's documented behavior.
   - `docs/research/R4-parts-bin.md` and `docs/research/R4-addendum-gpl.md`: the exact open-source part to use for each capability, licenses, things we write ourselves (W-xx), runtime checks (RT-xx), ship checklist.
4. `AGENTS.md`: Handy's own developer notes (build commands, structure, conventions). Still valid for the upstream code.

Working files behind each deliverable are in `docs/research/r2-work/`, `r3-work/`, `r4-work/` and `r5-review.md`. Read them only when a deliverable points there.

## Current status (update this when it changes)

As of 2026-10-30:

- **M0 foundations:** F-01 and F-03..F-10 are done on `wispr-bro`. F-02 (signing, notarization, updater, release workflows) is still open. Apple Team ID: `446XT9839S`; the owner keeps all certificates and keys. No Hugging Face org is needed: models download from their upstream HF repos, pinned by revision and sha256.
- **GitHub:** the public repo `fjenn/wispr-bro` exists but is **empty on purpose**. Remote `origin` has its push URL disabled. The owner said: push nothing until we have tested. Ask before any push.
- **M0.5 spikes done (provisional, one M5 Max):**
  - RS-07 cleanup bake-off: `docs/fork/spikes/RS-07-cleanup-bakeoff.md`. Muesli 2B v12 is the default (DEC-03 update).
  - RS-09 speech models: `docs/fork/spikes/RS-09-asr-bench.md`. Starters per language are in DEC-52; long-take chunking is DEC-51.
- **RS-01 (fn/Globe):** rounds 1 and 2 done 2026-10-30 on macOS 26.6.2; report in `docs/fork/spikes/RS-01-fn-globe.md`. DEC-44 confirmed (onboarding sets the Globe setting to Do Nothing with consent); with Do Nothing the bare fn press is not blocked. DEC-53: prepared mic, `T_show` 150 ms (round 2: prepared p50 129 ms at 100 ms, no idle orange dot, 0 clipped). DEC-54: any key with fn during a push-to-talk press drops the take (owner holds fn ~1 s before chord keys). S-01 builds on it (below).
- **RS-04 (AX matrix):** done 2026-10-30 (owner session + unattended); report `docs/fork/spikes/RS-04-ax.md`, outcomes in R5 §4 `fork/ax/`. Prototype in `src-tauri/crates/wb-spike-ax/src/ax/` (containers now fail open).
- **RS-05 (clipboard):** done 2026-10-30; report `docs/fork/spikes/RS-05-clipboard.md`, outcomes in R5 §4 `fork/clipboard/`. Prototype in `src-tauri/crates/wb-spike-clipboard/src/clip/`. Untested: clipboard managers (none installed), held ⌃⌘ (moved to S-06).
- **S-13 (Hub shell):** built and merged 2026-10-30: `src/fork/hub/`, `src/fork/ui/`, `src/fork/styles/`, `fork/hub.rs`; the `App.tsx` hook renders `ForkApp`. As-built decisions are in R5 under S-13. Not yet run: Mac-test-script S-13.1-S-13.5 by a person (VoiceOver itself, file drops from Finder, onboarding title-bar switch).
- **S-04 (insertion and clipboard):** built and merged 2026-10-30: `fork/clipboard/` (the RS-05 files, moved), `fork/insert/`; one `actions.rs` hook. 100-take soak in 10 apps: 0 lost, 0 duplicated, 100/100 clipboards byte-identical (`docs/fork/test-runs/1.0.0-alpha.1-M-26.md`). Not yet run: lid close / user switch by a person (S-04.3/.4), clipboard managers, the delivery inside the running app. The card UI is S-05.
- **S-01 (hotkey engine):** built 2026-10-30 on branch `worktree-agent-aa96506e13fbe63bc` (to merge into `wispr-bro`): `fork/shortcut/`, `fork/audio/`, vendored `crates/wb-handy-recorder` (prepared mic), coordinator `external_command`. Real-Mac results and what was not verified are in R5 under S-01 "As built"; RS-14 partial report `docs/fork/spikes/RS-14-secure-input.md`. Open: Ctrl+Opt on an external keyboard, real-key chord/indicator run by a person, Safari/iTerm2 Secure Input, CLI flags on a running instance.
- **S-09 (history data model):** built 2026-10-30 on branch `worktree-agent-a146c45d550dee19a`: `fork/db/{entry_meta,stats,retention,reconcile,disk,handy,jobs}.rs`, migration 0002, `fork/stats/`, `fork/context/` (frontmost app, `categories.json`, take ledger), history/stats commands; hooks in `managers/history.rs`, `actions.rs`, `managers/transcription.rs`. Real app (portable test profile): 52 takes in 5 apps with correct per-app rows and exact stats, 14-day audio expiry by clock override, purge through Handy (`docs/fork/test-runs/1.0.0-alpha.1-M-26-S-09.md`). Not run: disk full on a real disk; the delete/undo UI is S-15b.
- **S-10 (local LLM runtime):** built 2026-10-30: `ci/llama-server/build.sh` (static b11624 sidecar, `externalBin`), `fork/llm/` (supervisor, Unix-socket transport, pinned Muesli download, memory policy, streamed client), AI engine panel on the debug panel, `llama-server` translation backend. RS-06 partial report `docs/fork/spikes/RS-06-llm-runtime.md` (M5 Max, ad hoc). Open: M1 8 GB rows and the memory rule there, Developer ID + notarization (F-02), macOS 13, real sleep, `wb-egress`.
- **S-15a (Dictation home and stats rail):** built 2026-10-30: `src/fork/home/` (greeting with the macOS first name and an in-place override, stats rail, voice profile bar, promo banner, live refresh on `fork-stats-updated`), `fork/commands/stats.rs` (`fork_account_name`). Real app (portable test profile): seeded rail equals `daily_stats`, 10 takes updated it live (`docs/fork/test-runs/1.0.0-alpha.1-M-26-S-15a.md`). Not run: VoiceOver by a person; the history list is S-15b.
- **S-11 (text pipeline v2):** built 2026-10-31: `fork/text/` (VoiceTypr rule port, AGPL file; voice-command tables with MacParakeet's punctuation data), `fork/db/rules.rs`, migration 0003 (`entry_meta.text_stages`); the `finish_text` hook now wraps Handy's stages per line (`fork::text::process`), post-LLM stages inside `fork::insert::deliver`. Real app (portable test profile, TTS into the mic): three-paragraph e-mail into TextEdit and a Safari WebKit editor, two-line chat into TextEdit and a Chrome 156 chat page via typing/Shift+Return (`docs/fork/test-runs/1.0.0-alpha.1-M-26-S-11.md`). Not run: Mail and Slack themselves, a person's voice, other languages on real speech, stage 9 on a real LLM rewrite (S-12a).
- **S-16 (Dictionary):** built 2026-10-31: `fork/dictionary/` (store, Handy custom-words migration and mirror, CSV import/export), migration 0004, `fork/commands/dictionary.rs`, `src/fork/dictionary/` (page, Add word dialog, import wizard), the shared four-step import wizard `src/fork/ui/ImportWizard.tsx` (S-17 reuses it), `docs/user/import-format.md`. Real app (portable test profile, TTS): Handy words migrated, "gonna → going to" added in the dialog changed a real take, "Used 1 time" live (`docs/fork/test-runs/1.0.0-alpha.1-M-26-S-16.md`). Found: accented words (Siobhán) do not fix on Parakeet (Handy's fuzzy fix is ASCII-only); a replacement works. Not run: Mail, a person's voice, VoiceOver, Finder drag and Export panel by a person.
- **Bench inputs** (outside the repo, never committed): `~/wb-bench/` holds models, llama.cpp b11624, the owner's voice recording (`audio/`, personal data) and results.

## How to work

- **One slice at a time.** Pick the next slice in R5 whose dependencies are done. Build only that slice, to production quality (tests, translations, accessibility labels, dark mode). Order: M0 foundations (F-01..F-10), M0.5 real-Mac spikes (RS-01..RS-14, can run in parallel), then M1 slices S-01 onward. Do not start M2 (Notetaker, N-xx) until every M1 exit criterion in R5 §9 passes.
- **Reuse before writing.** For every capability, use the part R4 or the GPL addendum names (adopt a crate, vendor files, or port code with its license notice). Write our own code only for the W-xx items.
- **Thin layer on Handy.** Our code lives in the fork namespace (R5 §4.1). Upstream files get only the one-line hooks listed in R5 §4.2. Never add to Handy's `MIGRATIONS`, `AppSettings`, `collect_commands!` or `translation.json` files; fork strings go in the `fork` i18n namespace.
- **Licensing.** The app is GPL-3.0. New files carry `SPDX-License-Identifier: GPL-3.0-or-later`; ported files keep their original copyright line plus a modification notice; AGPL ports are marked `AGPL-3.0-only` and listed in NOTICE. Never copy Wispr Flow's code, prompts, UI text, sounds, icons or images; follow its behavior and structure only.
- **Privacy is a hard rule.** The app may contact only GitHub releases (updates) and Hugging Face (models). No other host, no telemetry SDK, no AppleScript. CI guards enforce this (R5 §4.12).
- **Verify, don't assume.** Run the tests and the slice's real-Mac acceptance checks. Never report a slice done without running them; say plainly what was not verified.
- **Commits.** Conventional commit prefixes; one slice per branch or PR.

## Do not

- Do not open or commit `docs/research/evidence/`. It holds local-only reference screenshots that include personal data and Wispr Flow's UI. It is gitignored.
- Do not change the bundle identifier (`com.wisprbro.desktop`), the Apple signing team or the updater public key after the first public build. Doing so resets macOS permissions for every user or breaks auto-updates.
- Do not re-add cloud LLM providers, telemetry, accounts or sign-in.
