// SPDX-License-Identifier: GPL-3.0-or-later
//! The apps RS-05 pastes into on this Mac, how to open each with a fresh
//! place to type, and how the tool can tell by itself that a paste landed.
//!
//! R5 §5.4's ten apps are Mail, Notes, TextEdit, Slack, VS Code, Chrome
//! Gmail, Safari, Notion, Messages and Terminal. Slack, VS Code and Notion are
//! not installed here; Cursor (VS Code's Electron base), Obsidian (Electron
//! notes, like Notion's desktop app) and the Claude and ChatGPT apps (chat
//! boxes, like Slack) stand in. Chrome gets a local test page instead of
//! Gmail (no network).

use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::clip::restore::TargetClass;
use crate::clip::target;
use crate::macos;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Open {
    /// `open -a <app> <new empty file>`.
    NewFile,
    /// `open -a <app> <local test page>`.
    TestPage,
    /// A new message (`open mailto:`; no address, nothing is sent).
    NewMail,
    /// A new Terminal window running `cat` into a file.
    TerminalCat,
    /// Just bring the app to the front; the owner picks the field.
    Activate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verify {
    /// Read the focused field's text through Accessibility.
    FieldText,
    /// The test page copies its text box into the window title.
    WindowTitle,
    /// The Terminal window's `cat` writes what it receives to a file.
    File,
    /// Only the owner can see it.
    Owner,
}

#[derive(Clone, Debug, Serialize)]
pub struct AppSpec {
    pub key: &'static str,
    pub name: &'static str,
    pub path: &'static str,
    pub open: Open,
    pub verify: Verify,
    /// What the owner clicks into.
    pub click: &'static str,
    /// A note for afterwards ("delete the test text").
    pub tidy: &'static str,
}

pub const APPS: &[AppSpec] = &[
    AppSpec {
        key: "textedit",
        name: "TextEdit",
        path: "/System/Applications/TextEdit.app",
        open: Open::NewFile,
        verify: Verify::FieldText,
        click: "the empty TextEdit document that opens",
        tidy: "close the TextEdit test window (no need to save)",
    },
    AppSpec {
        key: "notes",
        name: "Notes",
        path: "/System/Applications/Notes.app",
        open: Open::Activate,
        verify: Verify::FieldText,
        click: "the text of a note. A new note is best: press ⌘N in Notes, then click into it",
        tidy: "delete the test note in Notes",
    },
    AppSpec {
        key: "mail",
        name: "Mail",
        path: "/System/Applications/Mail.app",
        open: Open::NewMail,
        verify: Verify::FieldText,
        click: "the body of the new message (the big empty area, not the To: line)",
        tidy: "close the Mail test message and choose Delete Draft",
    },
    AppSpec {
        key: "messages",
        name: "Messages",
        path: "/System/Applications/Messages.app",
        open: Open::Activate,
        verify: Verify::FieldText,
        click: "the message box of any conversation. Nothing is sent: just never press Return",
        tidy: "clear the Messages box (⌘A, then Delete) without sending",
    },
    AppSpec {
        key: "terminal",
        name: "Terminal",
        path: "/System/Applications/Utilities/Terminal.app",
        open: Open::TerminalCat,
        verify: Verify::File,
        click: "the NEW Terminal window titled \"wb-paste-target\" (not this one)",
        tidy: "close the \"wb-paste-target\" Terminal window",
    },
    AppSpec {
        key: "safari",
        name: "Safari",
        path: "/Applications/Safari.app",
        open: Open::TestPage,
        verify: Verify::WindowTitle,
        click: "the text box on the \"Wispr Bro paste test\" page",
        tidy: "close the Safari test tabs",
    },
    AppSpec {
        key: "chrome",
        name: "Google Chrome",
        path: "/Applications/Google Chrome.app",
        open: Open::TestPage,
        verify: Verify::WindowTitle,
        click: "the text box on the \"Wispr Bro paste test\" page",
        tidy: "close the Chrome test tab",
    },
    AppSpec {
        key: "cursor",
        name: "Cursor",
        path: "/Applications/Cursor.app",
        open: Open::NewFile,
        verify: Verify::Owner,
        click: "the editor area of the \"wb-paste-target\" file that opens",
        tidy: "close the Cursor test file without saving",
    },
    AppSpec {
        key: "obsidian",
        name: "Obsidian",
        path: "/Applications/Obsidian.app",
        open: Open::Activate,
        verify: Verify::Owner,
        click: "the text of a note. A new scratch note is best (⌘N in Obsidian)",
        tidy: "delete the Obsidian test note",
    },
    AppSpec {
        key: "claude",
        name: "Claude",
        path: "/Applications/Claude.app",
        open: Open::Activate,
        verify: Verify::Owner,
        click: "the message box (where you type to Claude). Never press Return",
        tidy: "clear the Claude message box without sending",
    },
    AppSpec {
        key: "chatgpt",
        name: "ChatGPT",
        path: "/Applications/ChatGPT.app",
        open: Open::Activate,
        verify: Verify::FieldText,
        click: "the message box (where you type to ChatGPT). Never press Return",
        tidy: "clear the ChatGPT message box without sending",
    },
];

#[derive(Clone, Debug, Serialize)]
pub struct Installed {
    pub key: &'static str,
    pub name: &'static str,
    pub path: PathBuf,
    pub bundle_id: String,
    pub version: Option<String>,
    pub class: TargetClass,
}

pub fn installed(spec: &AppSpec) -> Option<Installed> {
    let path = PathBuf::from(spec.path);
    if !path.exists() {
        return None;
    }
    let plist = path.join("Contents/Info.plist");
    Some(Installed {
        key: spec.key,
        name: spec.name,
        bundle_id: macos::plist_value(&plist, "CFBundleIdentifier").unwrap_or_default(),
        version: macos::plist_value(&plist, "CFBundleShortVersionString"),
        class: target::classify(&path),
        path,
    })
}

/// The local test page: a text box whose content the page mirrors into the
/// window title, so Accessibility can read it from any browser.
pub const TEST_PAGE: &str = r#"<!doctype html>
<html><head><meta charset="utf-8"><title>Wispr Bro paste test</title>
<style>body{font:16px -apple-system,sans-serif;margin:2em}textarea{width:90%;height:12em;font-size:16px}</style>
</head><body>
<h1>Wispr Bro paste test</h1>
<p>Click into the box below, then keep your hands off the keyboard and mouse.</p>
<textarea id="t" autofocus placeholder="Click here"></textarea>
<script>
const t = document.getElementById('t');
const show = () => { document.title = 'WBTEST ' + t.value.replace(/\s+/g, ' ').slice(-400); };
t.addEventListener('input', show);
</script>
</body></html>
"#;

/// The Terminal target: titles its window, records the pid, and writes
/// every byte it receives (no line buffering) to `out`.
pub fn terminal_script(out: &Path, pidfile: &Path) -> String {
    format!(
        "#!/bin/sh\n# SPDX-License-Identifier: GPL-3.0-or-later\n\
         printf '\\033]0;wb-paste-target\\007'\n\
         echo $$ > '{pid}'\n\
         echo 'Wispr Bro paste test: click into this window, then hands off.'\n\
         echo '(This window only collects what is pasted into it.)'\n\
         stty -icanon min 1\n\
         exec cat > '{out}'\n",
        pid = pidfile.display(),
        out = out.display()
    )
}
