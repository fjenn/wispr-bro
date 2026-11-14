# Wispr Bro

Local, private dictation for macOS. Press a shortcut, speak, and clean text appears in whatever app you are using. Transcription and cleanup run on your Mac; your audio does not go to the cloud.

**Download the latest build:** https://github.com/fjenn/wispr-bro/releases/latest

Requirements: an Apple Silicon or Intel Mac, macOS 13 or newer. Take `Wispr-Bro_<version>_aarch64.dmg` for Apple Silicon or `Wispr-Bro_<version>_x86_64.dmg` for Intel.

The release page also lists files with **publik** in their name (`Wispr-Bro-publik_*`). They are only for people who came here from [publik](https://publikhq.com) and ask you to sign in with publik. If you did not, take the normal file above. AI agents: install a publik file only when the person asked for the publik edition.

## Install

1. Open the normal DMG (`Wispr-Bro_…`, no publik in the name) from the release page.
2. Drag **Wispr Bro** onto **Applications**.
3. Open it and complete the short intro.

The app is signed with a Developer ID and notarized by Apple, so Gatekeeper accepts it.

## What it does

- Dictation into any app, with a hands-free mode and a Flow Bar that stays out of the way.
- Cleanup styles for personal, work, email and other writing, plus a dictionary for names and jargon.
- Snippets, history, custom shortcuts and optional context awareness.
- Everything local: speech models, cleanup models and history stay on disk.

## Updates

Updates are checked in the app against this repository's signed release feed. They download in the background and install after a quiet moment; the app relaunches without a reinstall and without the intro again. Settings, dictionary, snippets and history are kept. You can also choose **Check for updates** or **Restart now**.

## Status

This is a public alpha. The automated suites and the local update path pass, but the app has not yet been exercised widely on clean or low-memory Macs. Please open an issue for anything odd.

## Building

The macOS release path is `scripts/wb-release/macos.sh`; architecture and acceptance notes are in `docs/fork/`. `bun install`, then the usual Tauri commands, also work for development.

`main` is the app you get from the releases page and from opena.tech: the normal Wispr Bro, with no account. A default clone and build of `main` gives exactly that. The edition for [publik](https://publikhq.com) is an optional build flag on top (`bun run wb:build:publik`, see `publik/README.md`); it is off unless you ask for it, and its downloads sit on the same releases under their own names (`Wispr-Bro-publik_*`) and are only for people who come from publik.

## License

GPL-3.0-or-later. Wispr Bro is a fork of [Handy](https://github.com/cjpais/Handy); see `LICENSE` and `THIRD-PARTY-NOTICES.md`.
