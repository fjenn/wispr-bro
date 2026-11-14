# Editions

Wispr Bro is built in two editions from this one repository. The default
branch, its releases (`releases/latest`) and opena.tech are one and the same
normal app; the publik edition is a thin layer on top that only a build flag
turns on, published only on its own releases.

**Wispr Bro** (the normal edition) is the app as it has always been: fully
local, no account, no telemetry. It is what `bun run wb:dev`, `bun run
wb:build` and every release without the publik switch produce, with the same
asset names (`Wispr-Bro_<v>_<arch>.dmg` for `aarch64` and `x86_64`,
`….app.tar.gz`, `latest.json`, `SHA256SUMS`) and the same update feed as before. Anyone who downloads Wispr
Bro from its releases or from another site gets this edition.

**Wispr Bro for publik** is the same app plus what publik
(publikhq.com) needs to list it: signing in with publik, usage counting for
publik's weekly-active-user count, publik's release self test, and later an
optional cloud mode. It only exists for people who install Wispr Bro through
publik.

## How they stay apart

Everything publik-only is in this folder, `publik/`, and the normal build
never compiles or bundles any of it:

| Part         | Where                                       | How it is switched on                                                                                       |
| ------------ | ------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| Rust         | `publik/rust/`                              | the crate `wb-publik`, an optional dependency of the app that only the `publik` Cargo feature turns on      |
| Frontend     | `publik/app/`                               | `VITE_WB_EDITION=publik`: `src/fork/vite-inputs.ts` then adds its page as an extra Vite entry               |
| Tauri config | `publik/tauri.conf.json`                    | passed as an extra `--config`; it turns the Cargo feature on and points the updater at `latest-publik.json` |
| Release      | `publik/edition.sh`, `publik/add-publik.sh` | `macos.sh --edition publik` sources `edition.sh`; `add-publik.sh` publishes the publik release              |
| Guard        | `publik/guard.ts`                           | always on, inside `bun run wb:guards`                                                                       |

Outside this folder there are only small hooks, each behind
`#[cfg(feature = "publik")]` or the `VITE_WB_EDITION` check: three lines in
`src-tauri/src/fork/mod.rs` (start, the sign-in gate, its commands), the feed
paths in `src-tauri/src/fork/update/channel.rs`, the optional dependency and
feature in `src-tauri/Cargo.toml`, the extra Vite entry, two `package.json`
scripts, the `--edition` switch in `scripts/wb-release/macos.sh`, the
`[publik]` hosts section and the guard's entry in `scripts/wb-guards.ts`.

The publik edition has its own update feed, `latest-publik.json`, on the
rolling prerelease `publik` (`releases/download/publik/latest-publik.json`;
`publik-beta` for the beta channel), never on a normal release
(`fork::update::channel` takes its paths from `wb_publik`). An installed
copy therefore only ever updates to its own edition: the normal app never
turns into the publik one, and the other way round.

## What the publik edition adds

All of it follows publik's setup guide (publikhq.com/skills/setup.md) and does
what publik's Rust module does, with the same requests, files and words:

- **Signing in.** At launch, before the Hub, a small "Sign in with publik"
  window (`publik/rust/src/gate.rs`, page `publik/app/`) checks the computer's
  publik key (setting one up with `POST /api/v1/installs` the first time) and
  the account behind it (`GET /api/v1/wallet`). Until the person has signed
  in, it offers "Sign in with publik", which opens publik in the browser, and
  looks every 3 seconds for 10 minutes. The Hub, the dictation pipeline and
  everything else in `fork::on_ready` wait for it; closing the window quits
  the app. Like publik's module, there is no offline way past it. The key is
  kept in publik's folder (`~/Library/Application Support/publik/apps/wispr-bro.json`, mode 0600).
- **Usage counting.** After sign-in, `app_opened` once and `active_session`
  every 30 minutes while the app runs (`POST /api/v1/telemetry`), which is
  what publik counts as weekly active use. `DO_NOT_TRACK`,
  `PUBLIK_TELEMETRY=0` or publik's own `consent.json` turn it off.
- **The release self test.** `PUBLIK_SELF_TEST=1` on a built publik app prints
  `publik self-test: ok wispr-bro <version>` (or why not) and exits, before
  any window: what publik's listing asks to see.
- **Choosing how to pay for AI** is in the gate too, for the optional cloud
  mode (step 4); it stays off (`USES_AI = false`) until then.

All requests go to publik through the system `curl`, with the key passed on
standard input, not the command line. `publikhq.com` is listed in
`ci/allowed-hosts.txt` under `[publik]`, which the hosts guard accepts only in
`publik/`. The tests (`cargo test --manifest-path publik/rust/Cargo.toml`)
run the real requests against a small local stand-in for publik
(`publik/rust/src/fake.rs`); `bun test publik/` runs the guard's tests.

## Building

```
bun run wb:dev            # normal edition, as before
bun run wb:build          # normal edition, as before
bun run wb:dev:publik     # publik edition
bun run wb:build:publik   # publik edition
scripts/wb-release/macos.sh --out out --edition publik   # signed publik release
```

`macos.sh --edition publik` (with `--arch aarch64` or `--arch x86_64`, like
the normal build) names its assets `Wispr-Bro-publik_<v>_<arch>.*` and writes
`latest-publik.json` and `SHA256SUMS-publik`, whose download URLs point at the
release `publik-v<v>` (`publik-beta` for a beta). Without `--edition` (or with
`--edition standard`) every name is unchanged. Everything the publik switch
sets is in `publik/edition.sh`.

## Releases

Normal releases (`wb-release.yml`, `scripts/wb-release/release-local.sh`)
never carry anything of the publik edition. Pull requests that touch publik
are checked by `wb-publik-preview.yml`, which tests this folder and builds
unsigned publik apps for both chips as artifacts; no release workflow builds
it.

The publik edition of a version is published after that version's normal
release, on the owner's Mac, with `publik/add-publik.sh v<version>`
from the `publik-edition` branch. In one run it does publik's sign-in in the
browser, the listing (publik's own derived listing with our wording and the
publik downloads for both chips), the app token (login keychain item
`wispr-bro-publik-app-token` and the `PUBLIK_APP_TOKEN` secret, never the
repository), the listing's slug in `APP_SLUG` (`publik/rust/src/lib.rs`) if publik chose
another, and signed builds for both chips with publik's self test. It then
publishes them on their own release `publik-v<version>` (tag on the publik
commit, never marked latest, with the source archive and `SHA256SUMS-publik`)
and puts `latest-publik.json` on the rolling prerelease `publik`. It checks
before and after that `releases/latest` still is the same normal release and
that its files, `latest.json` and `SHA256SUMS` are unchanged. A re-run reuses
the listing, the token and finished builds.

## The edition guard

`publik/guard.ts` (part of `bun run wb:guards`; `bun publik/guard.ts` alone) fails CI when publik code could reach the normal build:

- a publik marker (publik's domain, its install and telemetry API paths, the
  publik feed, checksum and asset names, its self-test switch) anywhere in
  `src/` or `src-tauri/`;
- a Rust use of `wb_publik` (or a `mod publik`) without a
  `#[cfg(feature = "publik")]` on it or within the three lines above it;
- the word publik in anything a normal user could see: any frontend file
  but the gateway (screens, settings, the about page, translations), or a
  Rust string literal in the app;
- a frontend import of a `publik/` path from anywhere but
  `src/fork/vite-inputs.ts`;
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

`releases/latest`, its files and every site that reads it (opena.tech) only
ever see the normal edition, since the publik files are on releases that are
never marked latest. The full release list on GitHub shows the `publik-v*`
releases too; their notes say they are for people who come from publik and
link to the normal latest release.

## The publik app token

publik's setup guide has apps commit their publik app token
(`publik-app-token.txt`, a `pat_…` value it calls public by design). Wispr Bro
does not commit it: the publik build reads it at compile time from
`PUBLIK_APP_TOKEN` (the GitHub Actions secret of that name in CI, passed only
to the publik job), so the repository and the normal edition never contain
it. `macos.sh --edition publik` refuses a signed build without it and checks
that the built app carries it; the normal edition unsets it. At run time
`PUBLIK_APP_TOKEN` overrides the built-in one, as in publik's module.
