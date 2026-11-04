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

/// The note page as Vite emits it (`src/fork/vite-inputs.ts`): the input's
/// source path, like the Flow Bar's. A path that does not exist falls back to
/// the Hub's `index.html`, so the window would show the Hub.
pub const PAGE: &str = "src/fork/scratchpad/index.html";

pub fn open<R: ForkRuntime>(app: &AppHandle<R>, choice: Open) -> Result<Note, String> {
    let note = choose(app, choice)?;
    super::changed(app);
    // S-38: a park scheduled when it was last closed no longer applies.
    crate::fork::hub::park::shown(app, WINDOW_LABEL);
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        ForkScratchpadOpen { note_id: note.id }
            .emit_to(app, WINDOW_LABEL)
            .map_err(|error| error.to_string())?;
        present(&window)?;
        return Ok(note);
    }
    let locale = crate::fork::i18n::current_locale(R::handy_app_language(app).as_deref());
    let title = crate::fork::i18n::t(locale, "rust.scratchpad.windowTitle", &[]);
    let builder = WebviewWindowBuilder::new(
        app,
        WINDOW_LABEL,
        WebviewUrl::App(format!("{PAGE}?note={}", note.id).into()),
    )
    .title(title)
    .inner_size(560.0, 440.0)
    .min_inner_size(400.0, 300.0)
    .resizable(true)
    .always_on_top(true);
    let window = title_strip(builder)
        .build()
        .map_err(|error| error.to_string())?;
    R::test_keep_painting(&window);
    // Closing hides it (Handy's `CloseRequested`); hidden, it is parked.
    crate::fork::hub::park::watch(app, &window);
    present(&window)?;
    Ok(note)
}

/// Height of the note window's title strip (tabs, expand, close); keep in
/// step with `h-11` in `src/fork/scratchpad/NoteWindow.tsx`.
pub const TITLE_STRIP_HEIGHT: f64 = 44.0;

/// Where macOS draws the traffic lights: at the start of the title strip,
/// on its centre line (tao's `y` lands the buttons' centre
/// [`crate::fork::hub::TRAFFIC_LIGHTS_CENTRE_OFFSET`] above it, measured in
/// Mac wave 6). The web page keeps 78 pt free for them (`ps-[78px]`).
pub const TRAFFIC_LIGHTS: (f64, f64) = (
    16.0,
    TITLE_STRIP_HEIGHT / 2.0 + crate::fork::hub::TRAFFIC_LIGHTS_CENTRE_OFFSET,
);

/// Design W6: the note window draws its own title strip (tabs, expand,
/// close) under an overlay title bar with hidden title text, like the Hub.
/// A no-op elsewhere.
fn title_strip<'a, R: tauri::Runtime, M: Manager<R>>(
    builder: WebviewWindowBuilder<'a, R, M>,
) -> WebviewWindowBuilder<'a, R, M> {
    #[cfg(target_os = "macos")]
    {
        builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true)
            .traffic_light_position(tauri::LogicalPosition::new(
                TRAFFIC_LIGHTS.0,
                TRAFFIC_LIGHTS.1,
            ))
    }
    #[cfg(not(target_os = "macos"))]
    {
        builder
    }
}

/// Shows the note and gives it keyboard focus, on the first open too
/// (Mac wave 1, Finding 3: a freshly built window was made key but the
/// app stayed in the background, so typing went to the previous app).
/// `set_focus` makes the window key and activates the app (tao:
/// `makeKeyAndOrderFront` + `activateIgnoringOtherApps`), which an
/// accessory app (no Dock icon) needs when a shortcut opens the note.
fn present<R: ForkRuntime>(window: &tauri::WebviewWindow<R>) -> Result<(), String> {
    window.unminimize().map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
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

    /// Finding 3 regression: both ways of opening the note (an existing
    /// window, a new one) end in `present`, which focuses it.
    #[test]
    fn every_open_path_focuses_the_note() {
        let source = include_str!("window.rs");
        let open =
            &source[source.find("pub fn open<").unwrap()..source.find("fn present<").unwrap()];
        assert_eq!(open.matches("present(&window)?").count(), 2, "{open}");
        let present = &source[source.find("fn present<").unwrap()..];
        assert!(present[..present.find("\n}").unwrap()].contains("set_focus()"));
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
        assert!(root.join("..").join(super::PAGE).is_file());
    }

    #[test]
    fn traffic_lights_sit_in_the_note_title_strip() {
        let (x, y) = super::TRAFFIC_LIGHTS;
        // The three 14 pt buttons with their gaps end before the 78 pt the
        // page keeps free.
        assert!(x > 0.0 && x + 60.0 < 78.0);
        let centre = y - crate::fork::hub::TRAFFIC_LIGHTS_CENTRE_OFFSET;
        assert!((centre - super::TITLE_STRIP_HEIGHT / 2.0).abs() < 0.5);
        let page = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../src/fork/scratchpad/NoteWindow.tsx"),
        )
        .unwrap();
        assert!(page.contains("h-11") && page.contains("ps-[78px]"));
    }
}
