// SPDX-License-Identifier: GPL-3.0-or-later
//! Snippets (S-17): a spoken trigger and the plain text it stands for.
//!
//! [`store`] is the `snippets` table (migration 0006), [`import`] and
//! [`export`] the CSV paths of the Snippets page. The text pipeline's stage
//! 11 (`fork::text::snippets`) reads the snippets once per delivery and
//! expands them after the LLM step; the page edits them here. Every change
//! (and every take that used a snippet) emits [`ForkSnippetsChanged`].

pub mod export;
pub mod import;
pub mod store;
#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::fork::text::snippets::{nfc, snippet_fold, MAX_BODY_CHARS, MAX_TRIGGER_CHARS};

/// Most snippets the page holds (Wispr's import limit is 1,000 items).
pub const MAX_SNIPPETS: usize = 1_000;

/// Where a snippet came from (`snippets.source`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SnippetOrigin {
    /// Added on the Snippets page.
    User,
    /// From a CSV import.
    Import,
}

impl SnippetOrigin {
    pub fn as_sql(self) -> &'static str {
        match self {
            SnippetOrigin::User => "user",
            SnippetOrigin::Import => "import",
        }
    }

    pub fn from_sql(s: &str) -> SnippetOrigin {
        match s {
            "import" => SnippetOrigin::Import,
            _ => SnippetOrigin::User,
        }
    }
}

/// One snippet as the page shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SnippetEntry {
    pub id: i64,
    /// What the user says.
    pub trigger: String,
    /// The text written instead (plain text, `\n` line breaks).
    pub body: String,
    pub source: SnippetOrigin,
    /// Unix ms.
    pub created_at: i64,
    /// Unix ms of the last edit.
    pub updated_at: i64,
    /// How many takes expanded it.
    pub hit_count: i64,
    pub last_hit_at: Option<i64>,
}

/// What the user typed (or a CSV row).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SnippetInput {
    pub trigger: String,
    pub body: String,
}

/// Why an input is not a valid snippet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SnippetProblem {
    /// The trigger is blank.
    EmptyTrigger,
    /// The trigger is longer than [`MAX_TRIGGER_CHARS`].
    TriggerTooLong,
    /// The trigger has no letter or digit (nothing that can be spoken).
    TriggerNotSpoken,
    /// A line break, tab or other control character in the trigger.
    TriggerControlCharacter,
    /// The text is blank.
    EmptyBody,
    /// The text is longer than [`MAX_BODY_CHARS`].
    BodyTooLong,
    /// A control character other than a line break or tab in the text.
    BodyControlCharacter,
    /// The page already holds [`MAX_SNIPPETS`] snippets.
    Full,
}

/// A validated input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanSnippet {
    pub trigger: String,
    pub body: String,
    /// Duplicate key ([`norm_of`]).
    pub norm: String,
}

/// Trims and collapses runs of whitespace to one space.
pub fn tidy(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The duplicate key of a trigger: NFC, tidied, folded as the matcher
/// folds (so two triggers that would match the same words are one).
pub fn norm_of(trigger: &str) -> String {
    snippet_fold(&tidy(&nfc(trigger)))
}

/// The text as saved: NFC, `\r\n` and `\r` as `\n`, trailing whitespace and
/// leading blank lines removed; the first line's indentation, inner blank
/// lines and spacing stay as typed.
pub fn tidy_body(s: &str) -> String {
    let s = nfc(s).replace("\r\n", "\n").replace('\r', "\n");
    let s = s.trim_end();
    let start = s
        .char_indices()
        .find(|(_, c)| !c.is_whitespace())
        .map_or(s.len(), |(i, _)| i);
    let line_start = s[..start].rfind('\n').map_or(0, |i| i + 1);
    s[line_start..].to_owned()
}

/// Validates and tidies an input.
pub fn clean(input: &SnippetInput) -> Result<CleanSnippet, SnippetProblem> {
    if input.trigger.chars().any(char::is_control) {
        return Err(SnippetProblem::TriggerControlCharacter);
    }
    let trigger = tidy(&nfc(&input.trigger));
    if trigger.is_empty() {
        return Err(SnippetProblem::EmptyTrigger);
    }
    if trigger.chars().count() > MAX_TRIGGER_CHARS {
        return Err(SnippetProblem::TriggerTooLong);
    }
    if !trigger.chars().any(char::is_alphanumeric) {
        return Err(SnippetProblem::TriggerNotSpoken);
    }
    let raw = input.body.replace("\r\n", "\n").replace('\r', "\n");
    if raw
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(SnippetProblem::BodyControlCharacter);
    }
    let body = tidy_body(&raw);
    if body.is_empty() {
        return Err(SnippetProblem::EmptyBody);
    }
    if body.chars().count() > MAX_BODY_CHARS {
        return Err(SnippetProblem::BodyTooLong);
    }
    let norm = snippet_fold(&trigger);
    Ok(CleanSnippet {
        trigger,
        body,
        norm,
    })
}

/// The counts the page shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SnippetCounts {
    pub snippets: u32,
    /// Takes that used a snippet, summed over every snippet.
    pub uses: u32,
}

/// Emitted after the snippets changed (page edits, imports) or a take used
/// one, so every window's Snippets page and counts refresh.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkSnippetsChanged {
    pub counts: SnippetCounts,
}

/// Emits [`ForkSnippetsChanged`] with the current counts.
pub fn notify_changed<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    use tauri::Manager;
    use tauri_specta::Event;
    let Some(db) = app.try_state::<crate::fork::db::ForkDb>() else {
        return;
    };
    match db.with_conn(|c| store::counts(c)) {
        Ok(counts) => {
            if let Err(e) = (ForkSnippetsChanged { counts }).emit(app) {
                log::debug!("snippets: could not emit the change event: {e}");
            }
        }
        Err(e) => log::warn!("snippets: could not count them: {e}"),
    }
}

/// Points the text pipeline at `fork.db` for its snippets and the record of
/// their use (`fork::on_ready`). A delivery reads them once (one indexed
/// query); a database error means no snippets for that take, never a
/// failed take. The use is written off the paste thread.
pub fn install<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    use tauri::Manager;
    let source_app = app.clone();
    let sink_app = app.clone();
    crate::fork::text::install_snippets(
        move || {
            let Some(db) = source_app.try_state::<crate::fork::db::ForkDb>() else {
                return Vec::new();
            };
            db.with_conn(|c| store::for_matching(c))
                .unwrap_or_else(|e| {
                    log::warn!("text: could not read the snippets: {e}");
                    Vec::new()
                })
        },
        move |used| {
            let app = sink_app.clone();
            let spawned = std::thread::Builder::new()
                .name("snippet-use".into())
                .spawn(move || {
                    let Some(db) = app.try_state::<crate::fork::db::ForkDb>() else {
                        return;
                    };
                    let now = crate::fork::context::now_ms();
                    match db.with_conn(|c| store::record_use(c, &used, now)) {
                        Ok(()) => {
                            log::info!(
                                "snippets: {} expanded (entry {})",
                                used.hits.len(),
                                used.entry_id
                                    .map_or_else(|| "?".to_owned(), |id| id.to_string())
                            );
                            if !used.hits.is_empty() {
                                notify_changed(&app);
                            }
                        }
                        Err(e) => log::warn!("snippets: could not record a take's snippets: {e}"),
                    }
                });
            if let Err(e) = spawned {
                log::warn!("snippets: could not start the recorder: {e}");
            }
        },
    );
}
