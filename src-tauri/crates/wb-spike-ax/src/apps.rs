// SPDX-License-Identifier: GPL-3.0-or-later
//! The apps RS-04 covers on the owner's Mac and, per app, the short list of
//! places the owner is asked to click (the ground truth: is that place a
//! text field?). Prompts are plain language; the owner is not expected to
//! know macOS internals.

use serde::{Deserialize, Serialize};

/// What the place the owner was asked to click really is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Expect {
    /// A text field with the cursor in it.
    Text,
    /// A text field with some text selected.
    Select,
    /// A password field.
    Password,
    /// Not a text field (list, sidebar, page body, button, tab).
    NotText,
    /// Recorded but not scored (for example a read-only field).
    Info,
}

impl Expect {
    pub fn label(self) -> &'static str {
        match self {
            Expect::Text => "text",
            Expect::Select => "select",
            Expect::Password => "password",
            Expect::NotText => "nottext",
            Expect::Info => "info",
        }
    }

    #[allow(dead_code)] // used by the catalog tests
    pub fn is_text_field(self) -> bool {
        matches!(self, Expect::Text | Expect::Select)
    }
}

pub struct Target {
    pub id: &'static str,
    pub prompt: &'static str,
    pub expect: Expect,
    /// A field the tool may write its `[wb]` test into (and remove it).
    pub scratch: bool,
}

const fn t(id: &'static str, expect: Expect, scratch: bool, prompt: &'static str) -> Target {
    Target {
        id,
        prompt,
        expect,
        scratch,
    }
}

pub struct AppSpec {
    /// Key in the results (`--apps` filter).
    pub key: &'static str,
    /// What the owner reads.
    pub title: &'static str,
    /// `open -a` name.
    pub app: &'static str,
    /// Where it is installed (first match wins).
    pub paths: &'static [&'static str],
    /// Asked first; "n" skips this part (signed-in-only parts).
    pub ask: Option<&'static str>,
    /// The tool's own test page runs first in this browser.
    pub test_page: bool,
    /// Open a scratch TextEdit document instead of just activating.
    pub scratch_doc: bool,
    pub intro: &'static str,
    pub targets: &'static [Target],
    pub outro: &'static str,
}

use Expect::*;

pub const APPS: &[AppSpec] = &[
    AppSpec {
        key: "textedit",
        title: "TextEdit",
        app: "TextEdit",
        paths: &["/System/Applications/TextEdit.app", "/Applications/TextEdit.app"],
        ask: None,
        test_page: false,
        scratch_doc: true,
        intro: "A TextEdit window opens with a short practice text (a file in the results folder).",
        targets: &[
            t("body", Text, true, "Click anywhere in the practice text."),
            t("select", Select, true, "Double-click a word in the practice text to select it."),
            t("end", Text, true, "Click at the very end of the practice text."),
            t("fonts", NotText, false, "Press ⌘T (the Fonts window opens), then click a font name in its list."),
            t("back", Text, true, "Press ⌘T again to close the Fonts window, then click into the practice text."),
            t("find", Text, false, "Press ⌘F: a search box appears at the top of the window, with the cursor in it."),
        ],
        outro: "Press Esc to close the search box. You can leave TextEdit open.",
    },
    AppSpec {
        key: "notes",
        title: "Notes",
        app: "Notes",
        paths: &["/System/Applications/Notes.app"],
        ask: None,
        test_page: false,
        scratch_doc: false,
        intro: "We'll make one test note. You can delete it at the end.",
        targets: &[
            t("new", Text, true, "Press ⌘N for a new note and type: hello test"),
            t("select", Select, true, "Double-click the word \"test\" in your new note to select it."),
            t("list", NotText, false, "Click any note in the list of notes (the middle column)."),
            t("folders", NotText, false, "Click a folder in the left sidebar (for example \"Notes\" or \"All iCloud\"). If there is no sidebar, press s to skip."),
            t("search", Text, false, "Click into the search box (top right, the magnifying glass)."),
            t("back", Text, false, "Click into the text of your test note again."),
        ],
        outro: "You can delete the test note now (or later).",
    },
    AppSpec {
        key: "mail",
        title: "Mail",
        app: "Mail",
        paths: &["/System/Applications/Mail.app"],
        ask: None,
        test_page: false,
        scratch_doc: false,
        intro: "We'll open one new, empty e-mail. Nothing is sent; you'll throw it away at the end.",
        targets: &[
            t("to", Text, false, "Press ⌘N: a new message opens with the cursor in the \"To:\" field."),
            t("subject", Text, false, "Click into the \"Subject:\" field."),
            t("body", Text, true, "Click into the big message area below."),
            t("select", Select, true, "Type: hello test   in the message area, then double-click \"test\" to select it."),
            t("list", NotText, false, "Go to the main Mail window (behind the new message) and click a message in the message list."),
            t("search", Text, false, "In the main Mail window, click the search box (top right)."),
        ],
        outro: "Close the new message and click \"Delete Draft\" (or \"Don't Save\").",
    },
    AppSpec {
        key: "messages",
        title: "Messages",
        app: "Messages",
        paths: &["/System/Applications/Messages.app"],
        ask: None,
        test_page: false,
        scratch_doc: false,
        intro: "Nothing is sent. Please don't press Enter in the message box.",
        targets: &[
            t("box", Text, false, "Click any conversation, then click into the message box at the bottom."),
            t("select", Select, false, "Type: hello test   in the message box (don't press Enter), then double-click \"test\"."),
            t("list", NotText, false, "Click a conversation in the list on the left."),
            t("search", Text, false, "Click the search box above the conversation list."),
        ],
        outro: "Delete the \"hello test\" you typed in the message box (select it and press Delete).",
    },
    AppSpec {
        key: "terminal",
        title: "Terminal",
        app: "Terminal",
        paths: &["/System/Applications/Utilities/Terminal.app"],
        ask: None,
        test_page: false,
        scratch_doc: false,
        intro: "This uses a second Terminal window, not the one with these instructions.",
        targets: &[
            t("new", Text, false, "Press ⌘N: a new Terminal window opens, with the cursor in it."),
            t("select", Select, false, "In that new window, double-click a word to select it."),
        ],
        outro: "Close that second window (⌘W). Keep this one.",
    },
    AppSpec {
        key: "safari",
        title: "Safari",
        app: "Safari",
        paths: &["/Applications/Safari.app", "/System/Volumes/Preboot/Cryptexes/App/System/Applications/Safari.app"],
        ask: None,
        test_page: true,
        scratch_doc: false,
        intro: "Safari opens the tool's own test page (a file on this Mac). It moves through its fields by itself for about a minute and a half: please don't click while it runs.",
        targets: &[
            t("address", Text, false, "Click into Safari's address bar (where the web address is)."),
            t("site", NotText, false, "Switch to a tab with any website you already had open (only its name, like \"example.com\", is recorded) and click on some plain text of the page. No tab? Press s."),
            t("site-field", Text, false, "On that website, click into any search box or text field. None? Press s."),
        ],
        outro: "You can close the test page tab.",
    },
    AppSpec {
        key: "chrome",
        title: "Google Chrome",
        app: "Google Chrome",
        paths: &["/Applications/Google Chrome.app"],
        ask: None,
        test_page: true,
        scratch_doc: false,
        intro: "Chrome opens the tool's own test page (a file on this Mac). It moves through its fields by itself for about a minute and a half: please don't click while it runs.",
        targets: &[
            t("address", Text, false, "Click into Chrome's address bar."),
            t("page", NotText, false, "Click on some plain text of the test page (not a field)."),
        ],
        outro: "Keep Chrome open for the next part.",
    },
    AppSpec {
        key: "gmail",
        title: "Gmail in Chrome",
        app: "Google Chrome",
        paths: &["/Applications/Google Chrome.app"],
        ask: Some("Are you signed in to Gmail in Chrome, and OK to open a new (unsent) e-mail there?"),
        test_page: false,
        scratch_doc: false,
        intro: "Open Gmail in Chrome yourself (the tool doesn't open web pages). Nothing is sent.",
        targets: &[
            t("search", Text, false, "Click into Gmail's search box at the top."),
            t("inbox", NotText, false, "Click on an empty spot of the left sidebar (below the labels)."),
            t("to", Text, false, "Click \"Compose\": a new e-mail opens with the cursor in its \"To\" field."),
            t("compose", Text, true, "Click into the big message area of the new e-mail."),
            t("select", Select, true, "Type: hello test   there, then double-click \"test\"."),
        ],
        outro: "Close the new e-mail (the trash icon at its bottom right discards it).",
    },
    AppSpec {
        key: "gdocs",
        title: "Google Docs in Chrome",
        app: "Google Chrome",
        paths: &["/Applications/Google Chrome.app"],
        ask: Some("Are you signed in to Google Docs in Chrome, and OK to open a blank document there?"),
        test_page: false,
        scratch_doc: false,
        intro: "Open a new blank Google Doc in Chrome yourself (the tool doesn't open web pages).",
        targets: &[
            t("body", Text, true, "Click into the blank document and type: hello test"),
            t("select", Select, true, "Double-click \"test\" to select it."),
            t("title", Text, false, "Click the document's name at the top left (\"Untitled document\")."),
            t("menu", NotText, false, "Press Esc, then click on an empty grey area beside the page."),
        ],
        outro: "You can delete the blank document (File → Move to trash).",
    },
    AppSpec {
        key: "cursor",
        title: "Cursor",
        app: "Cursor",
        paths: &["/Applications/Cursor.app"],
        ask: None,
        test_page: false,
        scratch_doc: false,
        intro: "We'll open one new, unsaved file. You'll close it without saving at the end.",
        targets: &[
            t("new", Text, true, "Press ⌘N (a new empty file opens), click into it and type: hello test"),
            t("select", Select, true, "Double-click \"test\" to select it."),
            t("tab", NotText, false, "Click the file's tab at the top of the editor (it says \"Untitled-1\" or similar)."),
            t("quickopen", Text, false, "Press ⌘P: a search box appears at the top with the cursor in it."),
            t("chat", Text, false, "Press Esc, then press ⌘L: the AI chat panel opens. Click into its message box."),
        ],
        outro: "Close the new file with ⌘W and click \"Don't Save\".",
    },
    AppSpec {
        key: "obsidian",
        title: "Obsidian",
        app: "Obsidian",
        paths: &["/Applications/Obsidian.app"],
        ask: None,
        test_page: false,
        scratch_doc: false,
        intro: "We'll make one test note. You can delete it at the end.",
        targets: &[
            t("new", Text, true, "Press ⌘N for a new note, click into its text area and type: hello test"),
            t("select", Select, true, "Double-click \"test\" to select it."),
            t("files", NotText, false, "Click a note's name in the left file list. (No file list? Press s.)"),
            t("quick", Text, false, "Press ⌘O: a \"find a file\" box appears with the cursor in it."),
            t("title", Text, false, "Press Esc, open your test note again and click into its title (\"Untitled\")."),
        ],
        outro: "You can delete the test note (right-click it in the file list → Delete).",
    },
    AppSpec {
        key: "claude",
        title: "Claude (desktop app)",
        app: "Claude",
        paths: &["/Applications/Claude.app"],
        ask: None,
        test_page: false,
        scratch_doc: false,
        intro: "Nothing is sent. Please don't press Enter in the message box.",
        targets: &[
            t("box", Text, true, "Click into the message box (where you type to Claude)."),
            t("select", Select, true, "Type: hello test   (don't press Enter), then double-click \"test\"."),
            t("sidebar", NotText, false, "Click a past chat in the left sidebar. (No sidebar? Press s.)"),
            t("reply", NotText, false, "Click on the text of one of Claude's replies (not the message box)."),
            t("box2", Text, false, "Click into the message box again."),
        ],
        outro: "Delete the \"hello test\" you typed (select it and press Delete).",
    },
    AppSpec {
        key: "chatgpt",
        title: "ChatGPT (desktop app)",
        app: "ChatGPT",
        paths: &["/Applications/ChatGPT.app"],
        ask: None,
        test_page: false,
        scratch_doc: false,
        intro: "Nothing is sent. Please don't press Enter in the message box.",
        targets: &[
            t("box", Text, true, "Click into the message box (where you type to ChatGPT)."),
            t("select", Select, true, "Type: hello test   (don't press Enter), then double-click \"test\"."),
            t("sidebar", NotText, false, "Click a past chat in the left sidebar. (No sidebar? Press s.)"),
            t("reply", NotText, false, "Click on the text of one of ChatGPT's replies (not the message box)."),
            t("box2", Text, false, "Click into the message box again."),
        ],
        outro: "Delete the \"hello test\" you typed (select it and press Delete).",
    },
];

/// RS-04 names these; they are not installed on the owner's Mac.
pub const NOT_TESTED: &[&str] = &[
    "Slack", "VS Code", "Discord", "Notion", "Arc", "Brave", "Edge", "Firefox", "Word", "iTerm2",
];

pub fn find(key: &str) -> Option<&'static AppSpec> {
    APPS.iter().find(|a| a.key == key)
}

pub fn installed_path(spec: &AppSpec) -> Option<&'static str> {
    spec.paths
        .iter()
        .copied()
        .find(|p| std::path::Path::new(p).exists())
}

/// The practice text in the TextEdit scratch document.
pub const PRACTICE_TEXT: &str = "Wispr Bro accessibility test. This file is in the results folder; you can close it without saving.\n\nThe quick brown fox jumps over the lazy dog, then naps in the sun.\nA second line, so there is something to click on.\n";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_consistent() {
        let mut keys = std::collections::HashSet::new();
        for a in APPS {
            assert!(keys.insert(a.key), "duplicate key {}", a.key);
            let mut ids = std::collections::HashSet::new();
            for t in a.targets {
                assert!(ids.insert(t.id), "{}: duplicate target {}", a.key, t.id);
                assert!(!t.prompt.is_empty());
                assert!(
                    !(t.scratch && !t.expect.is_text_field()),
                    "{}/{}: only text fields are scratch",
                    a.key,
                    t.id
                );
            }
            assert!(
                a.targets.iter().any(|t| t.expect.is_text_field()),
                "{} has a text field",
                a.key
            );
        }
        // Messages and Terminal are never written into.
        for k in ["messages", "terminal"] {
            assert!(find(k).unwrap().targets.iter().all(|t| !t.scratch), "{k}");
        }
    }

    #[test]
    fn session_stays_short() {
        let guided: usize = APPS.iter().map(|a| a.targets.len()).sum();
        // About 10 s per guided target plus two 90 s test pages.
        assert!(guided * 10 + 2 * 90 <= 15 * 60, "{guided} targets");
    }

    #[test]
    fn expect_labels() {
        for e in [Text, Select, Password, NotText, Info] {
            let json = serde_json::to_string(&e).unwrap();
            assert_eq!(json.trim_matches('"'), e.label());
        }
    }
}
