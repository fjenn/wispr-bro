<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Slice agent brief (template)

The orchestrator (Claude Code or Kimi) hands each builder agent this text with
the placeholders filled: `{SLICE}` (id and title), `{WT}` (worktree path),
`{BRANCH}`, `{BASE_NOTE}` (what the branch is based on), `{PORT}` (a dev port
unique to the agent) and `{EXTRA}` (slice-specific notes). It implements the
"Execution protocol" in `CONTEXT.md`; change both together.

---

You are building slice **{SLICE}** of Wispr Bro, a free, fully local
GPL-3.0 macOS dictation app (a fork of Handy: Tauri 2 + Rust + React/TS, bun).
Work only in the git worktree `{WT}` (branch `{BRANCH}`); never modify the main
checkout `~/Projects/wispr-bro`. {BASE_NOTE} If the worktree already holds
uncommitted work from an earlier agent, review it, keep what is right and
finish it. Build the slice to production quality, commit, and report. Nobody
answers questions mid-task; decide obvious calls yourself and record them in
the slice's R5 "As built" block.

## Read first

1. `CONTEXT.md`, especially **"Execution protocol"**. Follow it exactly.
2. `docs/research/R5-prd-and-slices.md`: §1–§5 and the slice; follow its ids
   into R1–R4 and `docs/fork/spikes/`; read the "As built" blocks of the slices
   it depends on.
3. `docs/fork/ARCHITECTURE.md`, `docs/fork/MAC-TEST-SCRIPT.md`,
   `docs/fork/PORTS.md`, `AGENTS.md`, and the fork code it builds on.

{EXTRA}

## Hard rules

- Thin layer: code in `src-tauri/src/fork/` and `src/fork/` (docs under
  `docs/fork/`); Handy files only via hooks budgeted in `ci/hooks.txt`. Never
  add to Handy's `MIGRATIONS`, `AppSettings`, `collect_commands!` or
  `translation.json`; fork strings go in the `fork` namespace.
- Reuse first (R4 / GPL addendum parts); ports keep their copyright line, a
  modification notice and PORTS/NOTICE rows. New files carry
  `SPDX-License-Identifier: GPL-3.0-or-later`.
- Never copy Wispr Flow's code, prompts, UI text, sounds, icons or images.
- Privacy: the app contacts only GitHub releases and Hugging Face. No
  telemetry. Never log user text at INFO.
- Never open or commit `docs/research/evidence/` or anything from `~/wb-bench`.
- Migrations: next free number in `src-tauri/src/fork/db/migrations/` at
  rebase time.
- Tests never write to the real `~/Library/Application Support`.
- Do not push. Do not touch `origin`.

## Efficiency (owner's instruction: thorough enough, not excessive)

- Builds share the sccache compile cache (`src-tauri/.cargo/config.toml`).
- Translations: add English strings, fill the other locales with
  `bun scripts/wb-translate-missing.ts` (copy-English, flagged); the machine
  translation batch runs once per wave. Don't start a `llama-server` for it.
- Run each check once at the end; while iterating, run only the tests you
  touched. Fix flaky tests (a load-only axe timeout gets a 180 s timeout in its
  spec) instead of re-running them.
- **No real-Mac GUI runs.** The orchestrator runs one batched Mac wave. Write
  or extend the scripted driver for the slice's real-Mac acceptance (reuse
  `scripts/wb-s*`; pre-recorded WAVs, never speakers) and list the exact checks
  in MAC-TEST-SCRIPT under the slice, marked "queued for the Mac wave".

## Checks (all must pass before you report)

`cargo test --lib`, fork clippy (0 findings), `cargo fmt --check`,
`bun run lint`, `npx tsc --noEmit`, `bun run check:translations`,
`bun run wb:check:translations`, `bun scripts/wb-guards.ts`,
`bun run wb:format:check`, and the Playwright fork suite on installed Chrome
(`WB_PW_CHANNEL=chrome`, dev port {PORT} via a scratch config; new UI gets a
spec with axe light/dark, RTL, keyboard-only). Run `bun install` first.
Regenerate bindings with
`WB_WRITE_FORK_BINDINGS=1 cargo test --lib fork_bindings_are_current` in
`src-tauri`.

## Finish

- Commits: conventional prefixes, `git commit -s`, each message ending with the
  orchestrator's Co-Authored-By line.
- Rebase onto the current `wispr-bro` before reporting (locale conflicts:
  rebuild from base plus your keys; CONTEXT/ARCHITECTURE/R5/PORTS: keep both
  sides), re-run the checks, confirm no conflict markers and a clean tree.
- Update R5 "As built", ARCHITECTURE, MAC-TEST-SCRIPT and CONTEXT status.
- Stop every process you started. Report briefly: commits, what was built,
  check results, what is queued for the Mac wave, what is not verified.
