// SPDX-License-Identifier: GPL-3.0-or-later
//! The singleton floating note window. No Handy window or hook is involved.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::fork::ForkRuntime;
use tauri_specta::Event;

use super::store::{self, Note};
use super::{ForkScratchpadOpen, WINDOW_LABEL};
use crate::fork::settings::ScratchpadOpenBehavior;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Open {
    Resume,
    New,
    Note(i64),
}

fn behavior_choice(behavior: ScratchpadOpenBehavior) -> Open {
    match behavior {
        ScratchpadOpenBehavior::ResumeLast => Open::Resume,
        ScratchpadOpenBehavior::NewNote => Open::New,
    }
}

fn choose<R: ForkRuntime>(app: &AppHandle<R>, open: Open) -> Result<Note, String> {
    let db = app
        .try_state::<crate::fork::db::ForkDb>()
        .ok_or_else(|| "fork.db is not managed".to_string())?;
    db.with_conn(|conn| match open {
        Open::Resume => match store::latest(conn)? {
            Some(note) => Ok(note),
            None => store::create(conn, crate::fork::context::now_ms()),
        },
        Open::New => store::create(conn, crate::fork::context::now_ms()),
        Open::Note(id) => store::get(conn, id)?
            .ok_or_else(|| crate::fork::db::DbError::Corrupt("note not found or deleted".into())),
    })
    .map_err(|error| error.to_string())
}

pub fn open<R: ForkRuntime>(app: &AppHandle<R>, choice: Open) -> Result<Note, String> {
    let note = choose(app, choice)?;
    super::changed(app);
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        ForkScratchpadOpen { note_id: note.id }
            .emit_to(app, WINDOW_LABEL)
            .map_err(|error| error.to_string())?;
        window.show().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())?;
        return Ok(note);
    }
    let locale = crate::fork::i18n::current_locale(R::handy_app_language(app).as_deref());
    let title = crate::fork::i18n::t(locale, "rust.scratchpad.windowTitle", &[]);
    WebviewWindowBuilder::new(
        app,
        WINDOW_LABEL,
        WebviewUrl::App(format!("fork-scratchpad.html?note={}", note.id).into()),
    )
    .title(title)
    .inner_size(560.0, 440.0)
    .min_inner_size(400.0, 300.0)
    .resizable(true)
    .always_on_top(true)
    .build()
    .map_err(|error| error.to_string())?;
    Ok(note)
}

pub fn open_shortcut<R: ForkRuntime>(app: &AppHandle<R>) -> Result<Note, String> {
    let behavior = app
        .try_state::<crate::fork::settings::ForkSettingsHandle>()
        .and_then(|handle| handle.get(app).ok())
        .map(|settings| settings.scratchpad_open_behavior)
        .unwrap_or_default();
    open(app, behavior_choice(behavior))
}

pub fn close<R: ForkRuntime>(app: &AppHandle<R>) -> Result<(), String> {
    app.get_webview_window(WINDOW_LABEL)
        .ok_or_else(|| "Scratchpad is not open".to_string())?
        .close()
        .map_err(|error| error.to_string())
}

pub fn expand<R: ForkRuntime>(app: &AppHandle<R>) -> Result<bool, String> {
    let window = app
        .get_webview_window(WINDOW_LABEL)
        .ok_or_else(|| "Scratchpad is not open".to_string())?;
    if window.is_maximized().map_err(|error| error.to_string())? {
        window.unmaximize().map_err(|error| error.to_string())?;
        Ok(false)
    } else {
        window.maximize().map_err(|error| error.to_string())?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn open_behavior_selects_resume_or_new_note() {
        assert_eq!(
            super::behavior_choice(crate::fork::settings::ScratchpadOpenBehavior::ResumeLast),
            super::Open::Resume
        );
        assert_eq!(
            super::behavior_choice(crate::fork::settings::ScratchpadOpenBehavior::NewNote),
            super::Open::New
        );
    }

    #[test]
    fn capability_covers_the_native_window_and_vite_builds_its_page() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let capability: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("capabilities/fork.json")).unwrap(),
        )
        .unwrap();
        assert!(capability["windows"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!(super::WINDOW_LABEL)));
        let inputs = std::fs::read_to_string(root.join("../src/fork/vite-inputs.ts")).unwrap();
        assert!(inputs.contains("fork-scratchpad"));
        assert!(root.join("../src/fork/scratchpad/index.html").is_file());
    }
}
