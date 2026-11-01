// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Abhishek Barali (SpeakoFlow sentinel capture; MIT)
// Copyright (c) 2024 tisfeng (SelectedTextKit Edit menu fallback; MIT)
// Modified by Wispr Bro contributors, 2026-11-01.
//
// The freshness rule is ported from SpeakoFlow `src-tauri/src/selection.rs`
// at 9818a17fcd124e5f1ea6d34780ddea9b154322d8: never accept clipboard text
// until a sentinel planted for this capture has been replaced. The macOS Edit
// menu fallback follows SelectedTextKit `AXManager+Menu.swift` at
// 93dc960a5ef8d52de98af41d4abd7a89576d091c (tisfeng, MIT): find Copy by its
// command shortcut, not a localized title, require it enabled, then AXPress.
// The sentinel uses our own value, clipboard snapshot and bounded polling.

//! Selected-text capture and the no-selection fallback order.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capture {
    pub target: u64,
    pub app_bundle_id: Option<String>,
    pub selected: Option<String>,
    pub field: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentDictation {
    pub text: String,
    pub created_at_ms: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Replacement {
    Selection,
    WholeField,
    AtCaret,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Input {
    pub text: String,
    pub replacement: Replacement,
    pub target: u64,
    pub app_bundle_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionIssue {
    SelectTextFirst,
    TooLong,
}

pub const MAX_CHARS: usize = 6_000;

fn present(text: &str) -> bool {
    !text.trim().is_empty()
}

fn bounded(text: String) -> Result<String, SelectionIssue> {
    if text.chars().count() > MAX_CHARS {
        Err(SelectionIssue::TooLong)
    } else {
        Ok(text)
    }
}

/// Selection → whole field → dictation from the past 60 s → Select text first.
pub fn choose_input(
    capture: &Capture,
    recent: Option<RecentDictation>,
    now_ms: i64,
) -> Result<Input, SelectionIssue> {
    let make = |text: String, replacement| {
        Ok(Input {
            text: bounded(text)?,
            replacement,
            target: capture.target,
            app_bundle_id: capture.app_bundle_id.clone(),
        })
    };
    if let Some(text) = capture.selected.as_ref().filter(|text| present(text)) {
        return make(text.clone(), Replacement::Selection);
    }
    if let Some(text) = capture.field.as_ref().filter(|text| present(text)) {
        return make(text.clone(), Replacement::WholeField);
    }
    if let Some(recent) = recent.filter(|recent| {
        present(&recent.text) && now_ms.saturating_sub(recent.created_at_ms) <= 60_000
    }) {
        return make(recent.text, Replacement::AtCaret);
    }
    Err(SelectionIssue::SelectTextFirst)
}

/// The four capture strategies, each returning `None` when it cannot answer
/// and `Some("")` when it answered that there is no selection.
pub trait SelectionSteps {
    fn ax_selected(&mut self) -> Option<String>;
    fn marker_selected(&mut self) -> Option<String>;
    fn menu_copy(&mut self) -> Option<String>;
    fn shortcut_copy(&mut self) -> Option<String>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureMethod {
    Ax,
    Markers,
    MenuCopy,
    ShortcutCopy,
    None,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainResult {
    pub text: Option<String>,
    pub method: CaptureMethod,
}

/// AX → VoiceInk text markers → SelectedTextKit Edit Copy → sentinel ⌘C.
pub fn through_chain(steps: &mut dyn SelectionSteps) -> ChainResult {
    macro_rules! attempt {
        ($method:expr, $read:ident) => {
            if let Some(text) = steps.$read() {
                return ChainResult {
                    text: present(&text).then_some(text),
                    method: $method,
                };
            }
        };
    }
    attempt!(CaptureMethod::Ax, ax_selected);
    attempt!(CaptureMethod::Markers, marker_selected);
    attempt!(CaptureMethod::MenuCopy, menu_copy);
    attempt!(CaptureMethod::ShortcutCopy, shortcut_copy);
    ChainResult {
        text: None,
        method: CaptureMethod::None,
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use std::time::{Duration, Instant};

    use tauri::AppHandle;

    use super::{Capture, CaptureMethod};
    use crate::fork::ax::{editable, element as el, focus, markers, thread};
    use crate::fork::clipboard::{keys, nspb, snapshot::Snapshot};
    use crate::fork::insert::target;

    const READ_BUDGET: Duration = Duration::from_millis(350);
    const COPY_BUDGET: Duration = Duration::from_millis(250);
    const COPY_POLL: Duration = Duration::from_millis(15);

    #[derive(Default)]
    struct Direct {
        selected: Option<String>,
        field: Option<String>,
        readable: bool,
    }

    fn direct(ctx: &mut thread::Ctx, target_id: u64) -> Direct {
        focus::capture(ctx, target_id);
        let focus = focus::read(ctx, Some(target_id));
        if !matches!(
            editable::delivery(focus.verdict),
            editable::Delivery::Paste | editable::Delivery::PasteFailOpen
        ) {
            return Direct::default();
        }
        let Some(element) = ctx
            .target(target_id)
            .and_then(|target| target.element.clone())
        else {
            return Direct {
                readable: true,
                ..Direct::default()
            };
        };
        let mut selected = el::attr_string(&element, "AXSelectedText");
        if selected.is_none() && markers::supported(&element) {
            selected = markers::selected_text(&element);
        }
        Direct {
            selected,
            field: el::attr_string(&element, "AXValue"),
            readable: true,
        }
    }

    fn press_edit_copy(ctx: &mut thread::Ctx, target_id: u64) -> bool {
        let Some(pid) = ctx.target(target_id).and_then(|target| target.pid) else {
            return false;
        };
        let app = ctx.app(pid);
        let Some(bar) = el::attr_element(&app, "AXMenuBar") else {
            return false;
        };
        for top in el::children(&bar).into_iter().take(8) {
            for menu in el::children(&top) {
                for item in el::children(&menu) {
                    let command = el::attr_string(&item, "AXMenuItemCmdChar")
                        .is_some_and(|key| key.eq_ignore_ascii_case("c"));
                    let command_only = el::attr_isize(&item, "AXMenuItemCmdModifiers") == Some(0);
                    if command && command_only && el::attr_bool(&item, "AXEnabled") == Some(true) {
                        return el::press(&item, "AXPress")
                            == objc2_application_services::AXError::Success;
                    }
                }
            }
        }
        false
    }

    fn on_main<R: tauri::Runtime, T: Send + 'static>(
        app: &AppHandle<R>,
        f: impl FnOnce() -> Result<T, String> + Send + 'static,
    ) -> Result<T, String> {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        app.run_on_main_thread(move || {
            let _ = tx.send(f());
        })
        .map_err(|e| format!("could not reach the main thread: {e}"))?;
        rx.recv()
            .map_err(|_| "the main thread dropped the request".to_string())?
    }

    fn clipboard_text() -> Option<String> {
        let (snapshot, _) = nspb::read();
        snapshot.items.iter().find_map(|item| {
            item.get(crate::fork::clipboard::markers::TEXT)
                .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
        })
    }

    fn wait_for_copy(sentinel: &str, sentinel_count: i64) -> Option<(String, i64)> {
        let deadline = Instant::now() + COPY_BUDGET;
        while Instant::now() < deadline {
            std::thread::sleep(COPY_POLL);
            let count = nspb::change_count();
            if count == sentinel_count {
                continue;
            }
            let text = clipboard_text()?;
            if text != sentinel {
                return Some((text, count));
            }
        }
        None
    }

    /// Tries the enabled menu item first, then ⌘C. The original pasteboard is
    /// restored only while our capture still owns it; a later user copy wins.
    fn copied_selection<R: tauri::Runtime>(
        app: &AppHandle<R>,
        target_id: u64,
    ) -> (Option<String>, CaptureMethod) {
        let modifiers = keys::wait_for_modifiers_released(
            Duration::from_secs(3),
            keys::flags_now,
            Instant::now,
            std::thread::sleep,
        );
        if modifiers.timed_out {
            return (None, CaptureMethod::None);
        }
        let snapshot = nspb::read().0;
        let sentinel = format!("\u{200b}wispr-bro-selection-{target_id}\u{200b}");
        let sentinel_for_write = sentinel.clone();
        let sentinel_count = match on_main(app, move || nspb::write_text(&sentinel_for_write, true))
        {
            Ok(count) => count,
            Err(error) => {
                log::warn!("transforms: could not plant the selection sentinel: {error}");
                let restore = Snapshot {
                    items: snapshot.items,
                };
                if let Err(error) = on_main(app, move || nspb::write(&restore)) {
                    log::warn!("transforms: could not restore the clipboard: {error}");
                }
                return (None, CaptureMethod::None);
            }
        };

        let menu_pressed =
            thread::run(READ_BUDGET, move |ctx| press_edit_copy(ctx, target_id)).unwrap_or(false);
        let mut method = CaptureMethod::MenuCopy;
        let mut copied = menu_pressed
            .then(|| wait_for_copy(&sentinel, sentinel_count))
            .flatten();
        if copied.is_none() {
            method = CaptureMethod::ShortcutCopy;
            if on_main(app, || keys::post_command_letter(b'c')).is_ok() {
                copied = wait_for_copy(&sentinel, sentinel_count);
            }
        }

        if let Some((_, captured_count)) = copied.as_ref() {
            if nspb::change_count() != *captured_count {
                copied = None;
            }
        }
        if nspb::change_count() == sentinel_count
            || copied
                .as_ref()
                .is_some_and(|(_, count)| *count == nspb::change_count())
        {
            let restore = Snapshot {
                items: snapshot.items,
            };
            if let Err(e) = on_main(app, move || nspb::write(&restore)) {
                log::warn!(
                    "transforms: could not restore the clipboard after selection capture: {e}"
                );
            }
        }
        (copied.map(|(text, _)| text), method)
    }

    pub fn capture<R: tauri::Runtime>(app: &AppHandle<R>) -> Capture {
        let target_id = target::capture_now();
        let direct =
            thread::run(READ_BUDGET, move |ctx| direct(ctx, target_id)).unwrap_or_default();
        let selected = match direct.selected {
            Some(text) if !text.trim().is_empty() => Some(text),
            Some(_) => None,
            None if direct.readable => copied_selection(app, target_id).0,
            None => None,
        };
        Capture {
            target: target_id,
            app_bundle_id: crate::fork::context::frontmost::frontmost()
                .and_then(|app| app.bundle_id),
            selected,
            field: direct.field,
        }
    }
}

#[cfg(target_os = "macos")]
pub use mac::capture;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    struct FakeSteps {
        answers: VecDeque<Option<String>>,
        calls: usize,
    }

    impl SelectionSteps for FakeSteps {
        fn ax_selected(&mut self) -> Option<String> {
            self.calls += 1;
            self.answers.pop_front().flatten()
        }
        fn marker_selected(&mut self) -> Option<String> {
            self.calls += 1;
            self.answers.pop_front().flatten()
        }
        fn menu_copy(&mut self) -> Option<String> {
            self.calls += 1;
            self.answers.pop_front().flatten()
        }
        fn shortcut_copy(&mut self) -> Option<String> {
            self.calls += 1;
            self.answers.pop_front().flatten()
        }
    }

    #[test]
    fn every_selection_fallback_is_used_in_order() {
        for (index, method) in [
            (0, CaptureMethod::Ax),
            (1, CaptureMethod::Markers),
            (2, CaptureMethod::MenuCopy),
            (3, CaptureMethod::ShortcutCopy),
        ] {
            let mut answers = vec![None; 4];
            answers[index] = Some("chosen".into());
            let mut fake = FakeSteps {
                answers: answers.into(),
                calls: 0,
            };
            let got = through_chain(&mut fake);
            assert_eq!(got.text.as_deref(), Some("chosen"));
            assert_eq!(got.method, method);
            assert_eq!(fake.calls, index + 1);
        }
    }

    #[test]
    fn an_answered_empty_selection_does_not_copy_or_use_stale_clipboard() {
        let mut fake = FakeSteps {
            answers: vec![Some(String::new()), Some("stale".into())].into(),
            calls: 0,
        };
        let got = through_chain(&mut fake);
        assert_eq!(got.text, None);
        assert_eq!(got.method, CaptureMethod::Ax);
        assert_eq!(fake.calls, 1);
    }

    fn capture(selected: Option<&str>, field: Option<&str>) -> Capture {
        Capture {
            target: 9,
            app_bundle_id: Some("com.example.editor".into()),
            selected: selected.map(str::to_owned),
            field: field.map(str::to_owned),
        }
    }

    #[test]
    fn no_selection_falls_back_to_field_then_recent_dictation_then_message() {
        let input = choose_input(&capture(Some("selection"), Some("field")), None, 0).unwrap();
        assert_eq!(
            (input.text.as_str(), input.replacement),
            ("selection", Replacement::Selection)
        );
        let input = choose_input(&capture(None, Some("field")), None, 0).unwrap();
        assert_eq!(
            (input.text.as_str(), input.replacement),
            ("field", Replacement::WholeField)
        );
        let recent = RecentDictation {
            text: "dictated".into(),
            created_at_ms: 1_000,
        };
        let input = choose_input(&capture(None, None), Some(recent.clone()), 61_000).unwrap();
        assert_eq!(
            (input.text.as_str(), input.replacement),
            ("dictated", Replacement::AtCaret)
        );
        assert_eq!(
            choose_input(&capture(None, None), Some(recent.clone()), 61_001),
            Err(SelectionIssue::SelectTextFirst)
        );
        assert_eq!(
            choose_input(&capture(None, None), None, 0),
            Err(SelectionIssue::SelectTextFirst)
        );
    }

    #[test]
    fn whitespace_is_not_input_and_long_input_is_refused_not_truncated() {
        assert_eq!(
            choose_input(&capture(Some(" \n "), Some("field")), None, 0)
                .unwrap()
                .text,
            "field"
        );
        assert_eq!(
            choose_input(&capture(Some(&"x".repeat(6_001)), None), None, 0),
            Err(SelectionIssue::TooLong)
        );
        assert_eq!(MAX_CHARS, 6_000);
    }
}
