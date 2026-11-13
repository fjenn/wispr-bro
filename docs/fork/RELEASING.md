# Releasing Wispr Bro

F-02 built the release path: one script for this Mac and for CI
(`scripts/wb-release/macos.sh`), two workflows (`wb-build.yml`,
`wb-release.yml`), the update channels in `src-tauri/src/fork/update/`, and
the custody rules below. Nothing has been pushed yet (CONTEXT.md: the owner
pushes after testing).

The publik edition is a second, opt-in build from the same tag with its own
assets and feed (`Wispr-Bro-publik_*`, `latest-publik.json`); the normal
release is unchanged. See `docs/fork/EDITIONS.md`.

## Identity (never change after the first public build)

| What                | Value                                                                                    | Where it is pinned                                                                                     |
| ------------------- | ---------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| Bundle identifier   | `com.wisprbro.desktop`                                                                   | overlay; `wb-guard-identity.ts`                                                                        |
| Build number        | `CFBundleVersion` = `1` (numeric; `CFBundleShortVersionString` is the marketing version) | overlay `bundle.macOS.bundleVersion`; `verify.sh` checks it is numeric                                 |
| CFBundleDisplayName | `Wispr Bro`                                                                              | `src-tauri/Info.plist` merge; `verify.sh` (with `LSRequiresCarbon` = false)                            |
| DMG volume          | `Wispr Bro Installer` (the app inside stays `Wispr Bro.app`)                             | `scripts/wb-release/dmg/build.sh`; `dmg/check_layout.py` checks name and icon                          |
| Signing identity    | `Developer ID Application: Finn Jennen (446XT9839S)`                                     | `macos.sh`, `verify.sh`, `wb-build.yml`                                                                |
| Updater public key  | minisign key id `89E085217ACC46CF`                                                       | overlay `plugins.updater.pubkey`; `wb-guard-identity.ts` `UPDATER_PUBKEY_ID`; `fork/update/channel.rs` |
| Stable feed         | `https://github.com/fjenn/wispr-bro/releases/latest/download/latest.json`                | overlay endpoint; guard `STABLE_ENDPOINT`; `channel.rs`                                                |
| Beta feed           | `https://github.com/fjenn/wispr-bro/releases/download/beta/latest.json`                  | `channel.rs` (chosen at runtime, DEC-33)                                                               |

Changing the key strands every install (they reject updates signed with
another key). Changing the identifier or team resets every user's macOS
permissions. The guards fail CI on either.

## Updater key custody

- **Private key:** `~/.wispr-bro/updater.key` on the owner's Mac (mode 600,
  outside the repo; minisign format, encrypted with a password).
- **Password:** login keychain, generic password, service `wispr-bro-updater`,
  account `updater` (`security find-generic-password -s wispr-bro-updater -a updater -w`).
- **Public key:** `~/.wispr-bro/updater.key.pub`; its base64 is the overlay's
  `plugins.updater.pubkey`.
- **Backups (owner, now):** two offline copies of `updater.key` **and** its
  password, kept apart from each other (e.g. an encrypted USB stick in a safe
  and a password manager entry with the key file attached). Without the key
  no existing install can be updated again.
- **CI copy:** the GitHub secret `TAURI_SIGNING_PRIVATE_KEY` (+ password),
  readable only by `wb-build.yml` on `wispr-bro` / `v*` tags.

### If the key leaks

An attacker with the key still needs to serve the update from our GitHub
release (the app only downloads from `github.com/fjenn/wispr-bro/releases`),
so the response is: rotate the GitHub credentials first, then ship one last
release signed with the **old** key whose overlay carries the **new** pubkey,
and only then start signing with the new key. Update `UPDATER_PUBKEY_ID` in
`wb-guard-identity.ts` and `PUBKEY_KEY_ID` in `channel.rs` in that release.

### If the key is lost

Installs cannot update. Ship the new key in a release users download by
hand (DMG), announce it in the README and the release notes, and keep the old
feed alive with a last `latest.json` pointing at nothing newer.

## Secrets (owner adds them at publish time)

Repository → Settings → Secrets and variables → Actions:

| Secret                               | Content                                                                                                            |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------ |
| `APPLE_CERTIFICATE`                  | base64 of the Developer ID Application certificate + private key exported as `.p12` (Keychain Access → Export)     |
| `APPLE_CERTIFICATE_PASSWORD`         | the `.p12` export password                                                                                         |
| `APPLE_API_KEY_ID`                   | App Store Connect API key id (the key behind the `wispr-bro-notary` profile, or a new one with the Developer role) |
| `APPLE_API_ISSUER`                   | its issuer id                                                                                                      |
| `APPLE_API_KEY_P8`                   | the text of `AuthKey_<id>.p8`                                                                                      |
| `TAURI_SIGNING_PRIVATE_KEY`          | the text of `~/.wispr-bro/updater.key`                                                                             |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | the password from the keychain item `wispr-bro-updater`                                                            |
| `PUBLIK_APP_TOKEN`                   | publik edition only (docs/fork/EDITIONS.md): publik's app token for Wispr Bro (`pat_…`)                            |

`GITHUB_TOKEN` is automatic. Also, once: disable the upstream workflows
`release.yml`, `build.yml`, `main-build.yml`, `pr-test-build.yml`,
`build-test.yml` and `nix-check.yml` in the Actions UI (R5 §4.1), and protect
`v*` tags (only the owner creates them).

## Channels

- **Stable** (default): `releases/latest/download/latest.json`. GitHub's
  `releases/latest` is the newest **published, non-prerelease** release, so
  drafts and the beta prerelease never reach a stable install.
- **Beta** (Settings › Advanced › Update channel): reads the rolling `beta`
  prerelease's `latest.json` **and** the stable one and takes the newer, so a
  beta tester moves to a stable release once it overtakes the last beta.
- **Internal alphas** (`1.0.0-alpha.N`) are stable-channel drafts: testers
  update from the fork debug panel (Updates › Check now / Install) once the
  owner publishes the draft.

## Cutting a release

1. Bump the version in `src-tauri/tauri.fork.conf.json`, `src-tauri/Cargo.toml`
   and `package.json` (all three equal, DEC-48), merge to `wispr-bro`.
2. Tag the merge commit: `git tag -s v1.0.0-alpha.3 && git push origin v1.0.0-alpha.3`.
   Tags are `vX.Y.Z` or `vX.Y.Z-(alpha|beta|rc).N`, strictly newer than every
   existing `v*` tag (`scripts/wb-release/tag.ts`).
3. `wb-release.yml` runs: preflight (tag, branch, guards in release mode),
   notices, egress, build + wb-verify, publish.
   - `-beta.N` → assets replace the rolling `beta` prerelease; the `beta` tag
     moves to the commit.
   - anything else → a **draft** release `vX…`. Review it (DMG opens without
     a warning on a clean Mac, RS-12 below), then **Publish** with "Set as the
     latest release". Only then does the stable feed serve it.
4. Run RS-12 (`docs/fork/MAC-TEST-SCRIPT.md` F-02.3/RS-12) on every release.

### Locally (this Mac)

```sh
WB_NOTARY_PROFILE=wispr-bro-notary scripts/wb-release/macos.sh --out ~/wb-release/1.0.0-alpha.1
```

**Both chips, published in one go** (what 1.0.0-alpha.7 used): from a
clean, pushed checkout whose version is the release's, with
`src/fork/whats-new/<version>.md` written,

```sh
WB_NOTARY_PROFILE=wispr-bro-notary scripts/wb-release/release-local.sh v1.0.0-alpha.7
```

builds, signs, notarizes and staples both the Apple Silicon and the Intel
app and DMG, writes one `latest.json` with both chips and one `SHA256SUMS`,
creates the release as a draft with every file, publishes it as the latest
release only then, and reads the stable feed and every download back. It
needs `gh` signed in to an account that can edit `fjenn/wispr-bro`.

**Once per Mac, first:** let `codesign` use the Developer ID private key
without asking. Otherwise every `codesign` call (the script makes about ten)
opens a "codesign wants to access key" dialog and the run waits until someone
answers it (F-02's unattended run stopped exactly there). Either answer the
first dialog with the login password and **Always Allow**, or run once:
`security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "<login password>" ~/Library/Keychains/login.keychain-db`.

Needs the Developer ID in the login keychain, the `wispr-bro-notary` notarytool
profile, `~/.wispr-bro/updater.key` and its keychain password. Output: the
DMG, `Wispr-Bro_<v>_aarch64.app.tar.gz` + `.sig`, `latest.json`,
`SHA256SUMS`, notarization logs and `release.log`. It ends with
`scripts/wb-release/verify.sh`. Notarization usually takes minutes, but
Apple can hold the first submissions of a new team for hours (F-02's first
one was still "In Progress" after 30 min); `macos.sh` waits up to 2 h per
submission, and `--reuse-app` resumes without rebuilding. `--unsigned` is the secret-free dry run CI uses
on other branches (`wb-build.yml`, `workflow_dispatch`).

## What a release contains

| Asset                                         | Made by                                                     |
| --------------------------------------------- | ----------------------------------------------------------- |
| `Wispr-Bro_<v>_aarch64.dmg`                   | signed, notarized, **stapled** (RS-12)                      |
| `Wispr-Bro_<v>_aarch64.app.tar.gz` + `.sig`   | the stapled app; minisign signature by the updater key      |
| `latest.json`                                 | `scripts/wb-release/manifest.ts` (signature checked first)  |
| `wispr-bro-<v>-source.tar.gz`                 | `git archive` of the tag (GPL-3.0 §6, linked in the notes)  |
| `LICENSE`, `NOTICE`, `THIRD-PARTY-NOTICES.md` | the tag's files; notices checked by `wb-notices.ts --check` |
| `SHA256SUMS`                                  | all of the above                                            |

## Release notes

`wb-release.yml` drafts the GitHub release notes (step "Release notes"):
the platform line, how to install, the translation status and the GPL
source link. Translation status: all 25 non-English locales were reviewed
by AI, not by native speakers (S-37, `docs/fork/TRANSLATION-REVIEW.md`);
the notes say so until a person has reviewed at least the 10 sample-text
languages (R5 §5.6), then list the locales that are still AI-reviewed
only. In-app What's New notes live in `src/fork/whats-new/`.

## GPL release assets (F-04)

Every release (stable, beta and drafts) carries `THIRD-PARTY-NOTICES.md`,
`LICENSE`, `NOTICE` and `wispr-bro-<version>-source.tar.gz` from the tagged
commit, and the release notes link the source tarball. `wb-release.yml`'s
`notices` job runs `bun scripts/wb-notices.ts --check` (cargo-about 0.9.3)
before publishing, so stale notices fail the release. GitHub's automatic
"Source code" archives are not relied on. If a release ever ships a prebuilt
binary that contains GPL code and is not built from this repo by the
workflow, its sources and build script go into the repo first. Homebrew
(S-39) and any mirror point at the same GitHub release.

## Why THIRD-PARTY-NOTICES.md is committed

It is generated by `scripts/wb-notices.ts` but committed, so every
dependency change shows its licence diff in review. `wb-ci.yml`'s
`licences` job runs `--check` on every PR (macOS runner, same target set
as the release build), so the committed file never drifts. Regenerate with
`bun scripts/wb-notices.ts` after any change to `Cargo.lock`, `bun.lock`,
`NOTICE` or the catalog's CC-BY models.
