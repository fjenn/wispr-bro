// SPDX-License-Identifier: GPL-3.0-or-later
//! Lesson commands (S-31).

/// A lesson's practice step is open (`on`, renewed every 60 s; two minutes
/// unless renewed) or closed (`off`). While on, a transform shortcut pressed
/// with Wispr Bro in front comes back as `fork-lesson-shortcut` instead of
/// running (`fork::tutorials::practice`).
#[tauri::command]
#[specta::specta]
pub fn fork_lesson_practice(on: bool) {
    crate::fork::tutorials::practice::set(on, std::time::Instant::now());
}
