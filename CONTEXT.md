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
