// SPDX-License-Identifier: GPL-3.0-or-later
//! Dictation home commands (S-15a). The stats themselves come from
//! `fork_stats_summary` (S-09, `commands/history.rs`) and the
//! `ForkStatsUpdated` event; this file adds the greeting's name source.
//!
//! The greeting reads the fork setting `greeting_name` first (the user's
//! override, F-07) and falls back to [`fork_account_name`], the first word of
//! the macOS account's full name (`NSFullUserName`, C-217).

use crate::fork::settings::MAX_GREETING_NAME_LEN;

/// The first name of the macOS account (the first word of its full name),
/// or `None` when the account has no usable full name or the platform has
/// none. Never fails: the greeting then says hello without a name.
#[tauri::command]
#[specta::specta]
pub fn fork_account_name() -> Option<String> {
    first_name(&account_full_name()?)
}

#[cfg(target_os = "macos")]
fn account_full_name() -> Option<String> {
    // Thread-safe Foundation call; returns "" when the account has no full
    // name (it never returns nil).
    Some(objc2_foundation::NSFullUserName().to_string())
}

#[cfg(not(target_os = "macos"))]
fn account_full_name() -> Option<String> {
    None
}

/// The greeting name in `full`: its first word without surrounding
/// punctuation ("Sam Lee" → "Sam", "Lee, Sam" → "Lee"), capped at
/// [`MAX_GREETING_NAME_LEN`] characters, `None` if nothing printable is left.
/// A name written without spaces (e.g. Japanese or Chinese) is kept whole.
pub(crate) fn first_name(full: &str) -> Option<String> {
    let word = full.split_whitespace().next()?;
    let word = word.trim_matches(|c: char| c.is_ascii_punctuation() && c != '\'' && c != '-');
    let word = word.trim_matches(|c: char| c == '\'' || c == '-');
    if word.is_empty() || word.chars().any(char::is_control) {
        return None;
    }
    Some(word.chars().take(MAX_GREETING_NAME_LEN).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_word_of_a_full_name() {
        assert_eq!(first_name("Sam Lee").as_deref(), Some("Sam"));
        assert_eq!(first_name("  Ana   María López ").as_deref(), Some("Ana"));
        assert_eq!(first_name("Siobhán").as_deref(), Some("Siobhán"));
        assert_eq!(first_name("Jean-Luc Picard").as_deref(), Some("Jean-Luc"));
        assert_eq!(first_name("O'Neil Smith").as_deref(), Some("O'Neil"));
    }

    #[test]
    fn surrounding_punctuation_is_dropped() {
        assert_eq!(first_name("Lee, Sam").as_deref(), Some("Lee"));
        assert_eq!(first_name("\"Sam\" Lee").as_deref(), Some("Sam"));
        assert_eq!(first_name("(Sam)").as_deref(), Some("Sam"));
    }

    #[test]
    fn names_without_spaces_are_kept_whole() {
        assert_eq!(first_name("山田太郎").as_deref(), Some("山田太郎"));
        assert_eq!(first_name("김민준").as_deref(), Some("김민준"));
    }

    #[test]
    fn empty_or_unprintable_names_give_none() {
        assert_eq!(first_name(""), None);
        assert_eq!(first_name("   "), None);
        assert_eq!(first_name("..."), None);
        assert_eq!(first_name("a\u{7}b"), None);
    }

    #[test]
    fn long_names_are_capped_like_the_setting() {
        let long = "x".repeat(MAX_GREETING_NAME_LEN + 20);
        let name = first_name(&long).unwrap();
        assert_eq!(name.chars().count(), MAX_GREETING_NAME_LEN);
    }

    #[test]
    fn the_account_name_call_never_panics() {
        // On a Mac this reads the real account; the value is not asserted
        // (CI accounts often have no full name), only its shape.
        if let Some(name) = fork_account_name() {
            assert!(!name.is_empty());
            assert!(!name.chars().any(char::is_whitespace));
            assert!(name.chars().count() <= MAX_GREETING_NAME_LEN);
        }
    }
}
