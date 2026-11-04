// SPDX-License-Identifier: GPL-3.0-or-later
//! Transforms page, editor, writing examples and diff commands (S-22).

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};

use super::ForkError;
use crate::fork::db::{DbError, ForkDb};
use crate::fork::text::diff::{self, WordDiff};
use crate::fork::transforms::record::{self, ForkTransformViewChanges};
use crate::fork::transforms::store::{self, TransformEntry, TransformInput};
use crate::fork::transforms::{literal, run, TransformKind};
use crate::fork::ForkRuntime;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkTransformsChanged {
    pub revision: u64,
}

fn db<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<tauri::State<'_, ForkDb>, ForkError> {
    app.try_state::<ForkDb>().ok_or(ForkError::NotReady)
}

fn internal(error: DbError) -> ForkError {
    ForkError::Internal(error.to_string())
}

fn changed<R: ForkRuntime>(app: &AppHandle<R>) {
    crate::fork::transforms::sync_registration(
        app,
        app.try_state::<crate::fork::settings::ForkSettingsHandle>()
            .and_then(|handle| handle.get(app).ok())
            .is_some_and(|settings| settings.transforms_enabled),
    );
    crate::fork::shortcut::reload(app);
    let _ = tauri_specta::Event::emit(
        &ForkTransformsChanged {
            revision: crate::fork::context::now_ms().max(0) as u64,
        },
        app,
    );
}

#[tauri::command]
#[specta::specta]
pub fn fork_transforms_list<R: ForkRuntime>(
    app: AppHandle<R>,
) -> Result<Vec<TransformEntry>, ForkError> {
    db(&app)?
        .with_conn(|conn| store::list(conn))
        .map_err(internal)
}

#[tauri::command]
#[specta::specta]
pub fn fork_transform_create<R: ForkRuntime>(
    app: AppHandle<R>,
    input: TransformInput,
) -> Result<TransformEntry, ForkError> {
    let input = store::validate(&input).map_err(ForkError::InvalidSetting)?;
    let entry = db(&app)?
        .with_conn(|conn| store::create(conn, &input, crate::fork::context::now_ms()))
        .map_err(internal)?;
    changed(&app);
    // OT2-H: custom transforms run on the instruction model.
    crate::fork::llm::instruct::ensure_downloaded(&app);
    Ok(entry)
}

#[tauri::command]
#[specta::specta]
pub fn fork_transform_update<R: ForkRuntime>(
    app: AppHandle<R>,
    id: i64,
    input: TransformInput,
) -> Result<TransformEntry, ForkError> {
    let input = store::validate(&input).map_err(ForkError::InvalidSetting)?;
    let entry = db(&app)?
        .with_conn(|conn| store::update(conn, id, &input, crate::fork::context::now_ms()))
        .map_err(internal)?
        .ok_or_else(|| ForkError::Internal("transform not found".into()))?;
    changed(&app);
    Ok(entry)
}

#[tauri::command]
#[specta::specta]
pub fn fork_transform_delete<R: ForkRuntime>(app: AppHandle<R>, id: i64) -> Result<(), ForkError> {
    let key = db(&app)?
        .with_conn(|conn| store::get(conn, id))
        .map_err(internal)?
        .filter(|entry| entry.builtin_key.is_none())
        .map(|entry| entry.key);
    let removed = db(&app)?
        .with_conn(|conn| store::delete(conn, id))
        .map_err(internal)?;
    if !removed {
        return Err(ForkError::InvalidSetting(
            "only a custom transform can be deleted".into(),
        ));
    }
    if let Some(key) = key {
        if let Some(settings) = app.try_state::<crate::fork::settings::ForkSettingsHandle>() {
            if let Ok((next, ())) = settings.update(&app, |settings| {
                if settings.auto_apply_transform.as_deref() == Some(key.as_str()) {
                    settings.auto_apply_transform = None;
                }
                settings.hotkeys.remove(&format!("transform.{key}"));
                Ok(())
            }) {
                let event = crate::fork::events::ForkSettingsChanged {
                    key: "hotkey".into(),
                    settings: next,
                };
                let _ = tauri_specta::Event::emit(&event, &app);
            }
        }
    }
    changed(&app);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn fork_transforms_reset<R: ForkRuntime>(
    app: AppHandle<R>,
) -> Result<Vec<TransformEntry>, ForkError> {
    let entries = db(&app)?.with_conn(store::reset).map_err(internal)?;
    changed(&app);
    Ok(entries)
}

fn draft_entry(id: i64, builtin_key: Option<String>, input: &TransformInput) -> TransformEntry {
    let key = builtin_key
        .clone()
        .unwrap_or_else(|| format!("custom-{id}"));
    TransformEntry {
        id,
        action_id: format!("transform.{key}"),
        key,
        builtin_key,
        name: input.name.clone(),
        description: input.description.clone(),
        rules: input.rules.clone(),
        prompt: input.prompt.clone(),
        examples: input.examples.clone(),
        replacements: input.replacements.clone(),
        updated_at: 0,
    }
}

#[tauri::command]
#[specta::specta]
pub async fn fork_transform_preview<R: ForkRuntime>(
    app: AppHandle<R>,
    id: i64,
    input: TransformInput,
    source: String,
) -> Result<run::TransformPreview, ForkError> {
    let input = store::validate(&input).map_err(ForkError::InvalidSetting)?;
    let builtin_key = db(&app)?
        .with_conn(|conn| store::get(conn, id))
        .map_err(internal)?
        .and_then(|entry| entry.builtin_key);
    let definition = store::definition_for(&draft_entry(id, builtin_key, &input));
    Ok(run::preview(&app, definition, &source).await)
}

/// What the editor says about a draft: exact edits it found and whether the
/// local AI is used at all (OT2-D).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct TransformPlan {
    pub edits: Vec<literal::Edit>,
    pub uses_model: bool,
    /// The custom prompt is wholly exact edits, so the editing rules and
    /// writing examples are not used.
    pub prompt_exact: bool,
}

#[tauri::command]
#[specta::specta]
pub fn fork_transform_plan(input: TransformInput) -> TransformPlan {
    let entry = draft_entry(0, None, &input);
    let compiled = store::compile(&entry);
    let plan = literal::plan(&input.prompt);
    TransformPlan {
        uses_model: !compiled.instruction.is_empty(),
        prompt_exact: !plan.edits.is_empty() && plan.rest.trim().is_empty(),
        edits: compiled.edits,
    }
}

#[tauri::command]
#[specta::specta]
pub fn fork_transform_latest<R: ForkRuntime>(
    app: AppHandle<R>,
) -> Result<Option<ForkTransformViewChanges>, ForkError> {
    db(&app)?
        .with_conn(|conn| record::latest(conn))
        .map_err(internal)
}

#[tauri::command]
#[specta::specta]
pub fn fork_transform_diff(source: String, result: String) -> WordDiff {
    diff::diff(&source, &result)
}

#[tauri::command]
#[specta::specta]
pub fn fork_transform_rate<R: ForkRuntime>(
    app: AppHandle<R>,
    run_id: i64,
    rating: Option<i64>,
) -> Result<ForkTransformViewChanges, ForkError> {
    let changed = db(&app)?
        .with_conn(|conn| record::rate(conn, run_id, rating))
        .map_err(internal)?;
    if !changed {
        return Err(ForkError::InvalidSetting("unknown run or rating".into()));
    }
    db(&app)?
        .with_conn(|conn| record::get(conn, run_id))
        .map_err(internal)?
        .ok_or_else(|| ForkError::Internal("transform run not found".into()))
}

#[tauri::command]
#[specta::specta]
pub async fn fork_transform_retry<R: ForkRuntime>(
    app: AppHandle<R>,
    run_id: i64,
) -> Result<ForkTransformViewChanges, ForkError> {
    let run = run::retry(&app, run_id)
        .await
        .map_err(ForkError::Internal)?;
    record::present(&app, run.clone());
    Ok(run)
}

#[tauri::command]
#[specta::specta]
pub fn fork_transform_read_example(path: String) -> Result<String, ForkError> {
    let path = std::path::PathBuf::from(path);
    let supported = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension.to_ascii_lowercase().as_str(), "txt" | "md"));
    if !supported {
        return Err(ForkError::InvalidSetting(
            "writing examples must be .txt or .md files".into(),
        ));
    }
    let text =
        std::fs::read_to_string(&path).map_err(|error| ForkError::Internal(error.to_string()))?;
    if text.chars().count() > store::MAX_EXAMPLE_CHARS {
        return Err(ForkError::InvalidSetting(format!(
            "writing examples are limited to {} characters",
            store::MAX_EXAMPLE_CHARS
        )));
    }
    Ok(text)
}

/// Source-level check that the two original actions keep their S-21 mapping.
pub fn builtin_id(kind: TransformKind) -> String {
    kind.action_id()
}
