# Upstream sync (F-09)

How upstream Handy (`cjpais/Handy`, branch `main`) reaches `wispr-bro`. Plan:
R5 §4.13 and slice F-09. The fork stays mergeable because upstream files get
only the hooks in R5 §4.2, and CI measures them (`ci/hooks.txt`, hook-size
guard in `docs/fork/ARCHITECTURE.md`, "CI guards").

## How it works

`.github/workflows/wb-upstream-sync.yml` runs every Monday (06:17 UTC) and on
demand (Actions, "wb-upstream-sync", "Run workflow"; input `base`, default
`wispr-bro`, lets you test it against a throwaway branch). The job:

1. Checks out `base` with full history (read-only `GITHUB_TOKEN`, credentials
   not persisted), adds the `upstream` remote and fetches `upstream/main`.
2. Creates `sync/upstream-<YYYY-MM-DD>` from `base` and runs
   `bun scripts/wb-upstream-sync.ts --in-place`, which merges
   `upstream/main` with `--no-ff` and:
   - **clean merge:** runs the cheap guards (`bun run wb:guards`) on the
     merge and writes the PR body: upstream changes since the merge base,
     the hotspots among them, new English strings containing "Handy", the
     guard summary and the checklist below;
   - **conflict:** aborts the merge (nothing is committed or pushed) and
     writes an issue body listing the conflicting files and the exact
     commands to resolve them locally;
   - **up to date:** does nothing.
3. Clean: force-pushes the branch with `WB_SYNC_TOKEN` and opens a PR into
   `base` (or updates the body of the open PR for that branch), then closes
   an open conflict issue if there is one. Conflict: opens the issue
   "Upstream sync: merge conflicts", or replaces its body if it is open.

Guard failures do not stop the PR: they are in its body, and `wb-ci.yml`
runs on the PR anyway (that is why the job pushes with a PAT, see below).
The job installs no dependencies and runs no upstream build scripts: the
guards are pure Bun scripts. It never sees signing or notarization secrets.

### Dry run on your machine

```sh
bun run wb:sync:dryrun -- --fetch          # merge upstream/main into a temp worktree of HEAD
bun run wb:sync:dryrun -- --into wispr-bro # … of another branch
bun run wb:sync:dryrun -- --out /tmp/body.md --keep  # keep the worktree, save the report
```

It runs the same code as the workflow in a temporary detached worktree,
never pushes, prints the report and removes the worktree (unless `--keep`).
The last line is `status=clean|conflict|uptodate`.

## Required secret: `WB_SYNC_TOKEN`

Pushes and PRs made with the default `GITHUB_TOKEN` do **not** trigger other
workflows, so `wb-ci.yml` would never run on the sync PR. The job therefore
uses a token from the repository secret `WB_SYNC_TOKEN`; without it the job
fails at its first step.

Create it once (owner):

1. GitHub, Settings, Developer settings, Fine-grained personal access tokens,
   "Generate new token". Resource owner: the account that owns `wispr-bro`.
   Repository access: only that repository. Expiry: at most one year (put a
   reminder in the calendar).
2. Repository permissions, read and write: **Contents**, **Pull requests**,
   **Issues**, **Workflows** (a merge that touches `.github/workflows/` cannot
   be pushed without it). Metadata read is added automatically. Nothing else.
3. Repository, Settings, Secrets and variables, Actions, "New repository
   secret": name `WB_SYNC_TOKEN`, value the token.

A GitHub App installation token with the same four permissions works too
(mint it in a step with the App's id and key). Do not reuse a token that can
reach other repositories, and never add signing, notarization or updater
secrets to this workflow or its environment.

Test once: create a throwaway branch from `wispr-bro` (e.g.
`sync-test/base`), run the workflow with `base: sync-test/base`, check that a
PR into it opens and that `wb-ci` runs on it, then close the PR and delete
both branches.

## Upstream workflows to disable (owner, Actions UI)

Upstream workflow files are never edited (R5 §4.1, CI row), so they arrive
with every sync. In the repository's Actions tab, open each of these and
choose "..." then "Disable workflow":

- `release.yml`
- `build.yml`
- `main-build.yml`
- `pr-test-build.yml`
- `build-test.yml`
- `nix-check.yml`

Keep enabled (no secrets; they catch upstream breakage): `test.yml`,
`code-quality.yml`, `playwright.yml`. A disabled workflow stays disabled
across syncs; a **new** upstream workflow file is enabled by default, so the
checklist asks you to decide on every new one.

## Cadence

- The workflow proposes a sync every week; merging is the owner's call.
- M1: merge a sync PR every two weeks (close the superseded weekly PR).
- Security fixes upstream: sync and merge immediately.
- Never merge a sync in the 3 days before a release. This is a rule for the
  person merging, not enforced by the workflow.

## Checklist

The workflow copies this block into every sync PR body (between the markers).

<!-- checklist:begin -->

- [ ] **Bindings:** regenerate `src/bindings.ts` (debug build, `bun run tauri dev` once) and commit it; never hand-merge it. Fork bindings (`src/fork/bindings.ts`) regenerate the same way.
- [ ] **Version lines:** re-apply our version in `src-tauri/Cargo.toml` and `package.json` (DEC-48) if upstream bumped theirs; `Cargo.lock`'s `handy` entry follows.
- [ ] **Hooks:** re-check every hook in `ci/hooks.txt` (the R5 §4.2 table) still sits in the right function, and update the line numbers cited in R5 §4.2, `ci/hooks.txt` comments and `docs/fork/ARCHITECTURE.md` if upstream moved them. Budgets must still equal the measured diff (`git diff --no-renames --numstat $(git merge-base HEAD upstream/main) -- <file>`).
- [ ] **Workflows:** review upstream changes to `release.yml` and `build.yml` and port fixes (runner images, action versions, signing steps) into `wb-*` workflows; decide enabled/disabled for any new upstream workflow (`docs/fork/UPSTREAM-SYNC.md`).
- [ ] **Settings:** compare upstream `settings.rs` default changes with `fork::settings::apply_product_settings()` (R5 §4.4) and adjust the fork side; do not edit Handy defaults.
- [ ] **Strings:** new or changed strings containing "Handy": run `bun run wb:brand`, commit the overrides in `src/i18n/locales/*/fork.json`, then `bun run wb:check:translations`.
- [ ] **Hosts:** new upstream hosts fail the baseline guard (`bun run wb:guard:hosts`); review each, then add it to `ci/upstream-hosts-baseline.txt` or prove it unreachable in `ci/inert-hosts.txt` (owner-approved).
- [ ] **Protocol:** read the `src-tauri/src/engine_supervisor/protocol.rs` diff (untouched in M1; M2 decides in N-01).
- [ ] **Catalog:** catalog changes (`src-tauri/src/catalog/`) checked against the licence register (`about.toml`, `cargo-about`), and against `fork::models` policy.
- [ ] **Guards and tests:** all guards green: `bun run wb:guards`, `bun run wb:test:scripts`, `bun run lint`, `bun run wb:format:check`, `cargo test`, `bun run wb:clippy`, `bun run wb:test:ui` (all of `wb-ci.yml`).
- [ ] **Exit check:** `git diff upstream/main --stat` lists only files in `ci/hooks.txt` plus fork paths.

<!-- checklist:end -->

## Hooks

The source of truth is `ci/hooks.txt`: every upstream file the fork may
change, with its line budget and the slice and hook that justify it. R5 §4.2
explains each hook. The hook-size guard (`bun run wb:guard:hooks`) fails on
any changed upstream file that is not listed or over budget. Do not copy the
list here.

## Resolving conflicts in hotspot files

Conflicts land almost only in the files listed in `ci/hooks.txt`, because
nothing else upstream is edited. General rule: **take upstream's version,
then re-apply our hook**, never the other way round. The hook is one line
(or a few after rustfmt), and its comment in `ci/hooks.txt` says exactly
what it is.

```sh
git checkout --theirs -- <file>     # upstream's side of the conflicting file
# re-apply the hook described in ci/hooks.txt for <file>
git add <file>
```

- **`src-tauri/src/lib.rs`:** keep upstream's plugin chain and
  `invoke_handler(...)` list as they are, then re-add `pub mod fork;`,
  `.plugin(fork::init())`, and wrap the handler as
  `.invoke_handler(fork::compose_invoke(invoke_handler))`. The plumbing guard
  checks the last one. Log file prefix: `fork::FILE_PREFIX`.
- **`src-tauri/src/settings.rs`:** keep upstream's new fields and defaults;
  re-apply only the hooks listed (log level, post-process providers,
  settings summary, the `#[ignore]` test). Put any default we want changed in
  `fork::settings::apply_product_settings()`.
- **`src/bindings.ts`:** do not merge by hand. Take upstream's version, then
  regenerate it from a debug build.
- **`package.json`, `src-tauri/Cargo.toml`:** take upstream's dependency and
  script changes, keep our version line and the `wb:*` block between the
  `//fork` keys. Then regenerate the lockfiles: `bun install` (rewrites
  `bun.lock` and, via Handy's postinstall, `.nix/bun.nix` and
  `.nix/bun-lock-hash`) and `cargo update -w` in `src-tauri` (for
  `Cargo.lock`, never a bare `cargo update`). Lockfile conflicts: take
  upstream's lockfile and regenerate, never hand-merge.
- **`src/i18n/index.ts`, `tray.rs`, `actions.rs`, `main.rs`, `managers/*`,
  `llm_client.rs`, `catalog/mod.rs`:** take upstream and re-add the single
  `fork::…` call at the new location of the same statement.
- **A hook's target disappeared upstream** (function renamed or removed):
  stop, find the new equivalent, and note the change in the PR; if the hook
  no longer fits in its budget, change the fork side, and only raise the
  budget in `ci/hooks.txt` with a slice comment as a last resort.

After resolving: `bun run wb:guards` (the hook guard proves the hooks are
back within budget), then the checklist above.
