// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Links the Hub opens in the user's browser (S-26b). All on GitHub, one of
 * the two hosts the app may name (R5 §1); the app itself never fetches
 * them. Rust keeps the same repository in `fork/share.rs` and
 * `fork/diagnostics/issue_url.rs`.
 */
export const REPO_URL = "https://github.com/fjenn/wispr-bro";
/** Release notes: Help › What's new. */
export const RELEASES_URL = "https://github.com/fjenn/wispr-bro/releases";
/** User docs: Help › Documentation (as the native Help menu, S-26a). */
export const DOCS_URL =
  "https://github.com/fjenn/wispr-bro/tree/wispr-bro/docs/fork/user";
/** The whisper lesson's "More about microphones" (S-32, P-1). */
export const MICROPHONES_DOC_URL =
  "https://github.com/fjenn/wispr-bro/blob/wispr-bro/docs/fork/user/microphones.md";
export const LICENSE_URL =
  "https://github.com/fjenn/wispr-bro/blob/wispr-bro/LICENSE";
/** Handy, the MIT-licensed app Wispr Bro is built on. */
export const HANDY_URL = "https://github.com/cjpais/Handy";
/** Share with a friend: the same link for everyone (no referral code). */
export const DOWNLOAD_URL =
  "https://github.com/fjenn/wispr-bro/releases/latest";
/**
 * Share with a friend: Homebrew. Hidden until S-39 publishes the
 * `fjenn/tap` cask: turn `BREW_CASK_PUBLISHED` on in the same change.
 */
export const BREW_CASK_PUBLISHED = false;
export const BREW_COMMAND = "brew install --cask fjenn/tap/wispr-bro";
/** Support › Contribute: how to help build Wispr Bro. */
export const CONTRIBUTING_URL =
  "https://github.com/fjenn/wispr-bro/blob/wispr-bro/CONTRIBUTING-WB.md";
/**
 * Support › Sponsor on GitHub (OT2-E). Hidden until the owner opens a GitHub
 * Sponsors profile (none on 2026-11-04): turn `SPONSOR_PUBLISHED` on in the
 * same change. GitHub is the only host the app may name (R5 §1), so no
 * other donation service.
 */
export const SPONSOR_PUBLISHED = false;
export const SPONSOR_URL = "https://github.com/sponsors/fjenn";
