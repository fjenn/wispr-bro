// SPDX-License-Identifier: GPL-3.0-or-later
//! Fork commands for the Scratchpad Hub page and floating note window.

use tauri::{AppHandle, Manager};

use super::ForkError;
use crate::fork::db::{DbError, ForkDb};
use crate::fork::scratchpad::{store, window};
use crate::fork::ForkRuntime;

fn db<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<tauri::State<'_, ForkDb>, ForkError> {
    app.try_state::<ForkDb>().ok_or(ForkError::NotReady)
}

fn internal(error: DbError) -> ForkError {
    ForkError::Internal(error.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn fork_scratchpad_list<R: ForkRuntime>(
    app: AppHandle<R>,
    query: Option<String>,
) -> Result<Vec<store::Note>, ForkError> {
    db(&app)?
        .with_conn(|conn| store::list(conn, query.as_deref()))
        .map_err(internal)
}

#[tauri::command]
#[specta::specta]
pub fn fork_scratchpad_get<R: ForkRuntime>(
    app: AppHandle<R>,
    id: i64,
) -> Result<Option<store::Note>, ForkError> {
    db(&app)?
        .with_conn(|conn| store::get(conn, id))
        .map_err(internal)
}

#[tauri::command]
#[specta::specta]
pub fn fork_scratchpad_save<R: ForkRuntime>(
    app: AppHandle<R>,
    id: i64,
    body: String,
) -> Result<store::Note, ForkError> {
    if !store::valid_body(&body) {
        return Err(ForkError::InvalidSetting(format!(
            "notes are limited to {} characters",
            store::MAX_BODY_CHARS
        )));
    }
    let note = db(&app)?
        .with_conn(|conn| store::save(conn, id, &body, crate::fork::context::now_ms()))
        .map_err(internal)?
        .ok_or_else(|| ForkError::InvalidSetting("note not found or deleted".into()))?;
    crate::fork::scratchpad::changed(&app);
    Ok(note)
}

#[tauri::command]
#[specta::specta]
pub fn fork_scratchpad_set_pinned<R: ForkRuntime>(
    app: AppHandle<R>,
    id: i64,
    pinned: bool,
) -> Result<store::Note, ForkError> {
    let note = db(&app)?
        .with_conn(|conn| store::set_pinned(conn, id, pinned))
        .map_err(internal)?
        .ok_or_else(|| ForkError::InvalidSetting("note not found or deleted".into()))?;
    crate::fork::scratchpad::changed(&app);
    Ok(note)
}

#[tauri::command]
#[specta::specta]
pub fn fork_scratchpad_delete<R: ForkRuntime>(app: AppHandle<R>, id: i64) -> Result<(), ForkError> {
    db(&app)?
        .with_conn(|conn| store::delete(conn, id, crate::fork::context::now_ms()))
        .map_err(internal)?;
    crate::fork::scratchpad::changed(&app);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn fork_scratchpad_open<R: ForkRuntime>(app: AppHandle<R>) -> Result<store::Note, ForkError> {
    window::open_shortcut(&app).map_err(ForkError::Internal)
}

#[tauri::command]
#[specta::specta]
pub fn fork_scratchpad_new<R: ForkRuntime>(app: AppHandle<R>) -> Result<store::Note, ForkError> {
    window::open(&app, window::Open::New).map_err(ForkError::Internal)
}

#[tauri::command]
#[specta::specta]
pub fn fork_scratchpad_show_note<R: ForkRuntime>(
    app: AppHandle<R>,
    id: i64,
) -> Result<store::Note, ForkError> {
    window::open(&app, window::Open::Note(id)).map_err(ForkError::Internal)
}

#[tauri::command]
#[specta::specta]
pub fn fork_scratchpad_close<R: ForkRuntime>(app: AppHandle<R>) -> Result<(), ForkError> {
    window::close(&app).map_err(ForkError::Internal)
}

#[tauri::command]
#[specta::specta]
pub fn fork_scratchpad_expand<R: ForkRuntime>(app: AppHandle<R>) -> Result<bool, ForkError> {
    window::expand(&app).map_err(ForkError::Internal)
}

#[tauri::command]
#[specta::specta]
pub fn fork_scratchpad_copy<R: ForkRuntime>(
    _app: AppHandle<R>,
    text: String,
) -> Result<(), ForkError> {
    if !store::valid_body(&text) {
        return Err(ForkError::InvalidSetting(
            "note is too large to copy".into(),
        ));
    }
    #[cfg(target_os = "macos")]
    crate::fork::clipboard::nspb::write_text(&text, false)
        .map(|_| ())
        .map_err(|error| ForkError::Internal(error.to_string()))?;
    #[cfg(not(target_os = "macos"))]
    return Err(ForkError::InvalidSetting(
        "native note copy is available on macOS".into(),
    ));
    #[cfg(target_os = "macos")]
    Ok(())
}

/// What Polish did to a note (rc9: every outcome is shown in the note).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum NotePolishOutcome {
    /// `text` is the polished note.
    Changed,
    /// The note needs no polish.
    NoChange,
    /// The model's answer was refused or it failed.
    Refused,
    /// The cleanup model is still on its way (downloading or verifying).
    NotReady,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct NotePolish {
    pub outcome: NotePolishOutcome,
    pub text: Option<String>,
}

/// How long Polish waits for a cleanup model that is still being verified
/// or downloaded before it says "not ready" (rc9).
const MODEL_WAIT: std::time::Duration = std::time::Duration::from_secs(120);

/// The local cleanup model is coming but not usable yet: Polish waits for
/// it rather than running on Apple's stand-in, whose rewrites of a whole
/// note the guards refuse (rc9 log: `Guard([NewTokens, ContentWords])`
/// three times while Muesli downloaded).
fn model_coming(state: crate::fork::llm::runtime::LlmState) -> bool {
    use crate::fork::llm::runtime::LlmState;
    matches!(
        state,
        LlmState::WaitingForSpeechModel | LlmState::Downloading | LlmState::Verifying
    )
}

/// Waits up to [`MODEL_WAIT`] for the cleanup model; `false` when it is
/// still coming after that.
async fn wait_for_cleanup_model<R: tauri::Runtime>(app: &AppHandle<R>) -> bool {
    let started = std::time::Instant::now();
    loop {
        let coming = crate::fork::llm::runtime::runtime(app)
            .is_some_and(|rt| model_coming(rt.status().state));
        if !coming {
            return true;
        }
        if started.elapsed() >= MODEL_WAIT {
            return false;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

/// Polish is the merged S-21 engine and built-in definition, with no AX
/// selection, paste, clipboard write, or transform-run side effects.
#[tauri::command]
#[specta::specta]
pub async fn fork_scratchpad_polish<R: ForkRuntime>(
    app: AppHandle<R>,
    source: String,
) -> Result<NotePolish, ForkError> {
    if source.trim().is_empty() {
        return Err(ForkError::InvalidSetting(
            "there is no note to polish".into(),
        ));
    }
    if !store::valid_body(&source) {
        return Err(ForkError::InvalidSetting(
            "note is too large to polish".into(),
        ));
    }
    let definition = db(&app)?
        .with_conn(|conn| {
            crate::fork::transforms::record::definition(
                conn,
                crate::fork::transforms::TransformKind::Polish,
            )
        })
        .map_err(internal)?
        .ok_or_else(|| ForkError::Internal("the Polish transform is missing".into()))?;
    if !wait_for_cleanup_model(&app).await {
        log::info!("scratchpad: Polish waited for the cleanup model; still not ready");
        return Ok(NotePolish {
            outcome: NotePolishOutcome::NotReady,
            text: None,
        });
    }
    use crate::fork::transforms::run;
    let preview = run::preview(&app, definition, &source).await;
    Ok(note_polish(preview.outcome, preview.text))
}

fn note_polish(
    outcome: crate::fork::transforms::run::PreviewOutcome,
    text: Option<String>,
) -> NotePolish {
    use crate::fork::transforms::run::PreviewOutcome;
    let outcome = match (outcome, &text) {
        (PreviewOutcome::Changed, Some(_)) => NotePolishOutcome::Changed,
        (PreviewOutcome::NoChange, _) => NotePolishOutcome::NoChange,
        (PreviewOutcome::NoEngine, _) => NotePolishOutcome::NotReady,
        _ => NotePolishOutcome::Refused,
    };
    NotePolish {
        text: text.filter(|_| outcome == NotePolishOutcome::Changed),
        outcome,
    }
}

/// The note's Record button (rc9): the same as a click on the Flow Bar,
/// so it starts a hands-free take, or stops the running one. The take
/// pastes at the note's caret like any other.
#[tauri::command]
#[specta::specta]
pub fn fork_scratchpad_record<R: ForkRuntime>(_app: AppHandle<R>) {
    crate::fork::pill::click();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::llm::runtime::LlmState;
    use crate::fork::transforms::run::PreviewOutcome;

    #[test]
    fn polish_waits_only_for_a_model_on_its_way() {
        for state in [
            LlmState::WaitingForSpeechModel,
            LlmState::Downloading,
            LlmState::Verifying,
        ] {
            assert!(model_coming(state), "{state:?}");
        }
        for state in [
            LlmState::Ready,
            LlmState::Unsupported,
            LlmState::MissingRuntime,
            LlmState::DownloadFailed,
        ] {
            assert!(!model_coming(state), "{state:?}");
        }
    }

    #[test]
    fn every_preview_outcome_becomes_a_note_outcome() {
        let text = || Some("Polished.".to_owned());
        assert_eq!(
            note_polish(PreviewOutcome::Changed, text()),
            NotePolish {
                outcome: NotePolishOutcome::Changed,
                text: text()
            }
        );
        assert_eq!(
            note_polish(PreviewOutcome::NoChange, None).outcome,
            NotePolishOutcome::NoChange
        );
        assert_eq!(
            note_polish(PreviewOutcome::NoEngine, None).outcome,
            NotePolishOutcome::NotReady
        );
        for outcome in [
            PreviewOutcome::Failed,
            PreviewOutcome::Timeout,
            PreviewOutcome::NotApplied,
        ] {
            assert_eq!(
                note_polish(outcome, text()),
                NotePolish {
                    outcome: NotePolishOutcome::Refused,
                    text: None
                }
            );
        }
    }
}
