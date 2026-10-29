# Contributing to Wispr Bro

Wispr Bro is a GPL-3.0 fork of [Handy](https://github.com/cjpais/Handy)
(MIT). These rules apply to every change on the `wispr-bro` branch. Handy's
own `CONTRIBUTING.md` covers contributions to Handy itself. Start with
`CONTEXT.md` for the plan and the reading order.

## 1. Sign off every commit (DCO)

We use the [Developer Certificate of Origin](https://developercertificate.org/)
1.1 instead of a CLA. By signing off you certify that you wrote the change,
or have the right to submit it, under the project's licence
(GPL-3.0-or-later for new files, the original licence for ported files).

Every commit needs a trailer with your real name and an e-mail you can be
reached at:

```text
Signed-off-by: Jane Doe <jane@example.com>
```

`git commit -s` adds it. To fix the commits of a branch:
`git rebase --signoff <base>` (then force-push your branch). The
`wb-dco` workflow fails a pull request when any of its commits lacks a
`Signed-off-by:` line, or when no sign-off matches the commit's author.

## 2. Licence headers

- Every new file under a fork code path (`src-tauri/src/fork/`,
  `src-tauri/crates/wb-*`, `src/fork/`, `scripts/wb-*`) starts with an SPDX
  line in its first five lines:
  `// SPDX-License-Identifier: GPL-3.0-or-later` (use the file type's comment
  syntax). Generated files are listed in `ci/generated.txt` instead.
- **Ported code** keeps its original copyright and licence line, adds
  `Modified by Wispr Bro contributors, <YYYY-MM-DD>`, gets a row in
  `docs/fork/PORTS.md` (repo, commit, files, licence) and an entry in
  `NOTICE` §3.
- **AGPL ports** (Voquill, Tambourine, VoiceTypr, OpenLess, abstand) live in
  their own fork files marked `SPDX-License-Identifier: AGPL-3.0-only` and
  listed in `NOTICE` §4. The SPDX guard fails otherwise.
- Never put a GPL or AGPL header, or GPL-derived code, into a Handy file.
- A new dependency, model, font or other asset needs an accepted licence
  (`about.toml`, `docs/fork/LICENSE-REGISTER.md`). Rerun
  `bun scripts/wb-notices.ts` and commit `THIRD-PARTY-NOTICES.md`.

## 3. Thin layer on Handy

- Fork code lives in the fork namespace (R5 §4.1). Upstream Handy files get
  only the small hooks listed in `ci/hooks.txt`, within their line budget;
  the hooks guard fails on anything else.
- Never add to Handy's `MIGRATIONS`, `AppSettings`, `collect_commands!` or
  `translation.json`. Fork strings go in the `fork` i18n namespace.
- Fixes meant for upstream Handy go in separate, MIT-clean commits that
  contain only code you wrote (listed in `ci/upstream-bound.txt`).

## 4. Never copy Wispr Flow

Wispr Bro follows Wispr Flow's behaviour and screen structure only. Do not
copy its code, prompts, UI text, sounds, icons, images, fonts or any other
material, and do not commit screenshots or recordings of it.
`docs/research/evidence/` is local-only and never committed. All UI copy is
written in our own words in `src/i18n/locales/en/fork.json`.

## 5. Privacy

The app may contact only GitHub releases (updates) and Hugging Face
(models). No telemetry, analytics, crash upload, cloud LLM, account or
AppleScript. The host and telemetry guards enforce this; a new host needs an
owner-approved PR that also updates R5 §1.

## 6. Before you open a pull request

```sh
bun run wb:guards
bun run wb:test:scripts
bun run lint
bun run wb:format:check
cd src-tauri && CMAKE_POLICY_VERSION_MINIMUM=3.5 cargo test
```

Use conventional commit prefixes (`feat:`, `fix:`, `docs:`, `ci:`, …), one
slice per branch or PR, and follow `.github/PULL_REQUEST_TEMPLATE.md`.
