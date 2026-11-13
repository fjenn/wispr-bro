# Editions

Wispr Bro is built in two editions from this one repository.

**Wispr Bro** (the normal edition) is the app as it has always been: fully
local, no account, no telemetry. It is what `bun run wb:dev`, `bun run
wb:build` and every release without the publik switch produce, with the same
asset names (`Wispr-Bro_<v>_aarch64.dmg`, `….app.tar.gz`, `latest.json`,
`SHA256SUMS`) and the same update feed as before. Anyone who downloads Wispr
Bro from its releases or from another site gets this edition.

**Wispr Bro for publik** is the same app plus what publik
(publikhq.com) needs to list it: signing in with publik, usage counting for
publik's weekly-active-user count, publik's release self test, and later an
optional cloud mode. It only exists for people who install Wispr Bro through
publik.

## How they stay apart

Everything publik-only lives in its own folders, and the normal build never
compiles it:

| Part         | Where                                                                                | How it is switched on                                                                                       |
| ------------ | ------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------- |
| Rust         | `src-tauri/src/fork/publik/` (and `src-tauri/crates/wb-publik/` if it grows a crate) | the `publik` Cargo feature, off by default                                                                  |
| Frontend     | `src/fork/publik/`                                                                   | `VITE_WB_EDITION=publik`, reached only through `src/fork/edition.ts` with a dynamic import                  |
| Tauri config | `src-tauri/tauri.publik.conf.json`                                                   | passed as an extra `--config`; it turns the Cargo feature on and points the updater at `latest-publik.json` |

Code outside those folders reaches the Rust module only behind
`#[cfg(feature = "publik")]`, so without the feature the module and every hook
into it are not compiled at all.

The publik edition has its own update feed, `latest-publik.json`, next to the
normal `latest.json` on the same GitHub release (`fork::update::channel` takes
its paths from `fork::publik`). An installed copy therefore only ever updates
to its own edition: the normal app never turns into the publik one, and the
other way round.

## Building

```
bun run wb:dev            # normal edition, as before
bun run wb:build          # normal edition, as before
bun run wb:dev:publik     # publik edition
bun run wb:build:publik   # publik edition
scripts/wb-release/macos.sh --out out --edition publik   # signed publik release
```

`macos.sh --edition publik` names its assets `Wispr-Bro-publik_<v>_aarch64.*`
and writes `latest-publik.json` and `SHA256SUMS-publik`. Without `--edition`
(or with `--edition standard`) every name is unchanged.

In CI, `wb-build.yml` builds the publik edition only with `publik: true`, as
its own job and artifact (`wb-release-macos-aarch64-publik`). `wb-release.yml`
passes that only while the repository variable `WB_PUBLIK_EDITION` is `on`;
then the publik assets are attached to the same release, next to the normal
ones, with a line in the release notes saying which download is which. While
the variable is unset, releases are exactly as before.

## The edition guard

`scripts/wb-guard-edition.ts` (`bun run wb:guard:edition`, also part of `bun
run wb:guards`) fails CI when publik code could reach the normal build:

- a publik marker (publik's domain, its install and telemetry API paths, the
  publik feed, checksum and asset names, its self-test switch) anywhere in
  `src/` or `src-tauri/` outside the folders above;
- a Rust `mod publik` or `publik::` path without a
  `#[cfg(feature = "publik")]` on it or within the three lines above it;
- the word publik in anything a normal user could see: any frontend file
  outside `src/fork/publik/` and the gateway (screens, settings, the about
  page, translations), or a Rust string literal outside the publik folder;
- a frontend import of a `publik/` path from anywhere but
  `src/fork/edition.ts`;
- the `publik` feature in Cargo's `default` features, or in the normal Tauri
  configs.

`macos.sh` also runs it on what it built: `--edition standard` fails if the
built frontend (`dist/`) or the app contains any publik marker or the word
publik in any spelling, and
`--edition publik` fails if the app does not read `latest-publik.json` (the
feature was not really on).

## What people who download Wispr Bro elsewhere see

Nothing from publik. A copy downloaded from anywhere but publik is the normal
edition: its screens, settings, about page, menus and translations never
mention publik (the guard above enforces it), and its updates come from
`latest.json`, which only ever points at the normal edition.

A site that links straight to a normal asset (for example
`releases/latest/download/Wispr-Bro_<v>_aarch64.dmg`) is unaffected, since
those names never change. A site that links to the GitHub releases page will
show the `Wispr-Bro-publik_*` downloads next to the normal ones once the
publik edition is switched on, with a line in the release notes saying which
is which.

## The publik app token

publik's setup guide has apps commit their publik app token
(`publik-app-token.txt`, a `pat_…` value it calls public by design). Wispr Bro
does not commit it: the publik build reads it from the build environment (a
GitHub Actions secret in CI), so the repository and the normal edition never
contain it.
