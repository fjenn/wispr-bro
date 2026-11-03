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
export const LICENSE_URL =
  "https://github.com/fjenn/wispr-bro/blob/wispr-bro/LICENSE";
/** Handy, the MIT-licensed app Wispr Bro is built on. */
export const HANDY_URL = "https://github.com/cjpais/Handy";
/** Share with a friend: the same link for everyone (no referral code). */
export const DOWNLOAD_URL =
  "https://github.com/fjenn/wispr-bro/releases/latest";
/** Share with a friend: Homebrew (S-39 publishes the `fjenn/tap` cask). */
export const BREW_COMMAND = "brew install --cask fjenn/tap/wispr-bro";
