// SPDX-License-Identifier: GPL-3.0-or-later
//! The dictionary (S-16): the user's words and "from → to" replacements.
//!
//! One store, two consumers, never mixed (R2 C-67):
//!
//! - **Words** (`kind = 'word'`) are mirrored into Handy's `custom_words`
//!   setting ([`migrate`]), which Handy gives the recognizer as vocabulary
//!   when the model supports it and otherwise uses for its fuzzy spelling
//!   fix (`audio_toolkit/text.rs`).
//! - **Replacements** (`kind = 'rule'`) are applied by the text pipeline's
//!   rule stage (S-11, `fork::text::rules`), which reads the active ones
//!   from `fork.db` once per take. They are never given to the recognizer.
//!
//! [`store`] is the table, [`import`] and [`export`] the CSV paths of the
//! Dictionary page, [`learn`] the opt-in auto-learn (S-34) that fills
//! Suggested. Every change emits [`ForkDictionaryChanged`].

pub mod export;
pub mod import;
pub mod learn;
pub mod migrate;
pub mod store;
#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::fork::text::fold::fold_str;

/// Longest word or "from", in characters (the S-11 rule limit, Wispr's
/// spec).
pub const MAX_PHRASE_CHARS: usize = crate::fork::text::rules::MAX_FROM_CHARS;

/// Longest replacement ("to"), in characters.
pub const MAX_REPLACEMENT_CHARS: usize = 200;

/// Most entries the dictionary holds. Handy's fuzzy word fix compares every
/// n-gram of a take with every word, so a much longer list would cost
/// latency on every take (R5 §5.1).
pub const MAX_ENTRIES: usize = 5_000;

/// What an entry is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// A word or name to spell right (vocabulary).
    Word,
    /// "from → to": a literal replacement.
    Rule,
}

impl EntryKind {
    pub fn as_sql(self) -> &'static str {
        match self {
            EntryKind::Word => "word",
            EntryKind::Rule => "rule",
        }
    }

    pub fn from_sql(s: &str) -> Option<EntryKind> {
        match s {
            "word" => Some(EntryKind::Word),
            "rule" => Some(EntryKind::Rule),
            _ => None,
        }
    }
}

/// Where an entry came from (`dictionary_entries.source`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum EntrySource {
    /// Added on the Dictionary page.
    User,
    /// From a CSV import.
    Import,
    /// From a flagged history entry (S-15b).
    Flag,
    /// Learned from a correction (S-34).
    Learn,
    /// Taken over from Handy's custom words.
    Migrated,
}

impl EntrySource {
    pub fn as_sql(self) -> &'static str {
        match self {
            EntrySource::User => "user",
            EntrySource::Import => "import",
            EntrySource::Flag => "flag",
            EntrySource::Learn => "learn",
            EntrySource::Migrated => "migrated",
        }
    }

    pub fn from_sql(s: &str) -> EntrySource {
        match s {
            "import" => EntrySource::Import,
            "flag" => EntrySource::Flag,
            "learn" => EntrySource::Learn,
            "migrated" => EntrySource::Migrated,
            _ => EntrySource::User,
        }
    }
}

/// `active` entries apply; `suggested` ones wait for approval (S-34).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum EntryStatus {
    Active,
    Suggested,
}

/// One entry as the Dictionary page shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct DictionaryEntry {
    pub id: i64,
    pub kind: EntryKind,
    /// The word, or the "from" of a replacement.
    pub phrase: String,
    /// The "to" of a replacement; `None` for a word.
    pub replacement: Option<String>,
    pub source: EntrySource,
    pub status: EntryStatus,
    /// Unix ms.
    pub created_at: i64,
    /// Unix ms of the last edit (the creation time if never edited).
    pub updated_at: i64,
    /// How often the replacement applied (S-11); 0 for words.
    pub hit_count: i64,
    pub last_hit_at: Option<i64>,
}

/// What the user typed (or a CSV row): a word, or a replacement when
/// `replacement` is given.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct EntryInput {
    pub phrase: String,
    pub replacement: Option<String>,
}

/// Why an input is not a valid entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum EntryProblem {
    /// The word (or "from") is blank.
    Empty,
    /// The word (or "from") is longer than [`MAX_PHRASE_CHARS`].
    TooLong,
    /// The replacement is longer than [`MAX_REPLACEMENT_CHARS`].
    ReplacementTooLong,
    /// The replacement is the same text as the word.
    SameAsReplacement,
    /// A line break, tab or other control character.
    ControlCharacter,
    /// The dictionary already holds [`MAX_ENTRIES`] entries.
    Full,
}

/// A validated input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanEntry {
    pub kind: EntryKind,
    pub phrase: String,
    pub replacement: Option<String>,
    /// Duplicate key: `phrase` case-folded.
    pub norm: String,
}

/// Trims and collapses runs of whitespace (any Unicode space) to one space.
pub fn tidy(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The duplicate key of a word or "from".
pub fn norm_of(phrase: &str) -> String {
    fold_str(&tidy(phrase))
}

/// Validates and tidies an input. A blank replacement makes it a word.
pub fn clean(input: &EntryInput) -> Result<CleanEntry, EntryProblem> {
    let has_control = |s: &str| s.chars().any(|c| c.is_control());
    if has_control(&input.phrase) || input.replacement.as_deref().is_some_and(has_control) {
        return Err(EntryProblem::ControlCharacter);
    }
    let phrase = tidy(&input.phrase);
    if phrase.is_empty() {
        return Err(EntryProblem::Empty);
    }
    if phrase.chars().count() > MAX_PHRASE_CHARS {
        return Err(EntryProblem::TooLong);
    }
    let replacement = input
        .replacement
        .as_deref()
        .map(tidy)
        .filter(|r| !r.is_empty());
    if let Some(r) = &replacement {
        if r.chars().count() > MAX_REPLACEMENT_CHARS {
            return Err(EntryProblem::ReplacementTooLong);
        }
        if *r == phrase {
            return Err(EntryProblem::SameAsReplacement);
        }
    }
    let norm = fold_str(&phrase);
    Ok(CleanEntry {
        kind: if replacement.is_some() {
            EntryKind::Rule
        } else {
            EntryKind::Word
        },
        phrase,
        replacement,
        norm,
    })
}

/// Entry counts the page shows on its tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct DictionaryCounts {
    /// Active words.
    pub words: u32,
    /// Active replacements.
    pub rules: u32,
    /// Waiting for approval (S-34).
    pub suggested: u32,
}

/// Emitted after the dictionary changed (page edits, imports, Handy's own
/// custom-words list taken over), so every window's Dictionary page and
/// counts refresh.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkDictionaryChanged {
    pub counts: DictionaryCounts,
}

/// Emits [`ForkDictionaryChanged`] with the current counts.
pub fn notify_changed<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    use tauri::Manager;
    use tauri_specta::Event;
    let Some(db) = app.try_state::<crate::fork::db::ForkDb>() else {
        return;
    };
    match db.with_conn(|c| store::counts(c)) {
        Ok(counts) => {
            if let Err(e) = (ForkDictionaryChanged { counts }).emit(app) {
                log::debug!("dictionary: could not emit the change event: {e}");
            }
        }
        Err(e) => log::warn!("dictionary: could not count the entries: {e}"),
    }
}
