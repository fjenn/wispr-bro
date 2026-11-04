// SPDX-License-Identifier: GPL-3.0-or-later
//! Vibe coding (S-36, D-59): the listed editors, what each may add to a
//! take, and the code names their visible AX text provides.
//!
//! Code names come only from the bounded AX read in [`super::caret`], which
//! keeps the names it finds here and drops the text; file names also come
//! from the project's list ([`super::workspace`], names only, never
//! contents) and are respelled by [`super::file_names`]. Identifiers are
//! used like S-20's hints (the recognizer's vocabulary, the spelling stage,
//! the cleanup prompt's keep list), and their exact spoken forms ("get user
//! by id") become the visible spelling (`getUserById`) in [`correct`].

use std::collections::{HashMap, HashSet};

use regex::Regex;

use super::hints::Corrected;
use crate::fork::settings::VibeCodingSettings;

/// Identifiers one take keeps at most (S-20's hint cap).
pub const MAX_IDENTIFIERS: usize = super::hints::MAX_HINTS;
const MIN_IDENTIFIER_CHARS: usize = 3;
const MAX_IDENTIFIER_CHARS: usize = 40;

/// The editors vibe coding knows, by the id stored in
/// `vibe_coding.editors`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Editor {
    VsCode,
    Cursor,
    Windsurf,
}

impl Editor {
    pub const ALL: [Editor; 3] = [Editor::VsCode, Editor::Cursor, Editor::Windsurf];

    pub fn id(self) -> &'static str {
        match self {
            Editor::VsCode => "vscode",
            Editor::Cursor => "cursor",
            Editor::Windsurf => "windsurf",
        }
    }

    pub fn from_id(id: &str) -> Option<Editor> {
        Editor::ALL.into_iter().find(|e| e.id() == id)
    }

    /// The bundle ids of this editor's macOS app (checked against Cursor
    /// 3.28.7's `Info.plist`; VS Code Insiders counts as VS Code).
    pub fn bundle_ids(self) -> &'static [&'static str] {
        match self {
            Editor::VsCode => &["com.microsoft.VSCode", "com.microsoft.VSCodeInsiders"],
            Editor::Cursor => &["com.todesktop.230313mzl4w4u92"],
            Editor::Windsurf => &["com.exafunction.windsurf"],
        }
    }

    /// The editor of a frontmost app's bundle id (exact ids, any case).
    pub fn from_bundle(bundle_id: Option<&str>) -> Option<Editor> {
        let bundle_id = bundle_id?;
        Editor::ALL.into_iter().find(|editor| {
            editor
                .bundle_ids()
                .iter()
                .any(|id| id.eq_ignore_ascii_case(bundle_id))
        })
    }

    /// Editors with an AI chat whose input takes `@file` tags.
    pub fn tags_files(self) -> bool {
        matches!(self, Editor::Cursor | Editor::Windsurf)
    }
}

/// What the frontmost editor may contribute to this take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Access {
    pub editor: Editor,
    /// "Recognize code names": set up for this editor and switched on.
    pub recognize: bool,
    /// "Tag files in chat": on, and the editor has a chat (Cursor,
    /// Windsurf). Applies only when the focused field is its chat input.
    pub file_tags: bool,
}

/// The features `settings` allow in the app with `bundle_id`, or `None`.
/// Both need the setup to list that editor (DEC-41: off until set up);
/// then each has its own switch. File tags apply only in an editor with a
/// chat (Cursor, Windsurf).
pub fn access(settings: &VibeCodingSettings, bundle_id: Option<&str>) -> Option<Access> {
    let editor = Editor::from_bundle(bundle_id)?;
    let set_up = settings.setup_completed && settings.editors.iter().any(|id| id == editor.id());
    let access = Access {
        editor,
        recognize: set_up && settings.enabled,
        file_tags: set_up && settings.file_tags_enabled && editor.tags_files(),
    };
    (access.recognize || access.file_tags).then_some(access)
}

/// Words that name an AI chat input in its label, placeholder, id or DOM
/// classes. `aichat` and `aislash` are Cursor's (3.28: the agent panel is
/// `workbench.panel.aichat.<id>`, its input `aislash-editor-input`).
const CHAT_WORDS: &[&str] = &[
    "chat", "composer", "cascade", "agent", "aichat", "aislash", "prompt",
];

/// How many ancestors of the focused field [`is_chat_input`]'s caller may
/// pass as the field's container.
pub const CHAT_ANCESTORS: usize = 12;

fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '@')
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Whether the focused field is an editor's AI chat input. `role`; its
/// `labels` (description, title, placeholder, identifiers, help, and its
/// DOM classes); `container`: the DOM ids and classes of its nearest
/// [`CHAT_ANCESTORS`] ancestors. It is when its labels or its container
/// name a chat, or its placeholder offers `@` mentions ("@ to mention",
/// "@ for context"). The editor buffer (Monaco's `inputarea`), a search
/// box, the terminal or an unlabelled field outside a chat never is (no
/// fail-open here: a wrong `@` in code is worse than a missed tag).
pub fn is_chat_input(role: &str, labels: &str, container: &str) -> bool {
    if !matches!(role, "AXTextArea" | "AXTextField" | "AXComboBox") {
        return false;
    }
    let own = words(labels);
    let around = words(container);
    if own
        .iter()
        .any(|w| matches!(w.as_str(), "search" | "terminal" | "xterm" | "inputarea"))
        || around
            .iter()
            .any(|w| matches!(w.as_str(), "terminal" | "xterm"))
    {
        return false;
    }
    let chat_word = |w: &String| CHAT_WORDS.contains(&w.as_str());
    own.iter().any(chat_word)
        || around.iter().any(chat_word)
        || own.windows(2).any(|pair| {
            pair[0] == "@" && matches!(pair[1].as_str(), "to" | "for" | "mention" | "mentions")
        })
}

/// The bundle ids whose AX tree the AX thread keeps built while they are in
/// front ([`crate::fork::ax::thread::keep_warm`]): the set-up editors, when
/// context awareness is on and either vibe-coding feature applies there.
pub fn warm_bundles(settings: &VibeCodingSettings, context_awareness: bool) -> Vec<String> {
    if !context_awareness {
        return Vec::new();
    }
    Editor::ALL
        .into_iter()
        .filter(|editor| access(settings, editor.bundle_ids().first().copied()).is_some())
        .flat_map(|editor| editor.bundle_ids().iter().map(|id| (*id).to_owned()))
        .collect()
}

/// Applies [`warm_bundles`] (fork settings loaded or changed).
pub fn sync_warm(settings: &crate::fork::settings::ForkSettings) {
    #[cfg(target_os = "macos")]
    crate::fork::ax::thread::keep_warm(warm_bundles(
        &settings.vibe_coding,
        settings.context_awareness,
    ));
    #[cfg(not(target_os = "macos"))]
    let _ = settings;
}

fn identifier_like(token: &str) -> bool {
    let chars: Vec<char> = token.chars().collect();
    if !(MIN_IDENTIFIER_CHARS..=MAX_IDENTIFIER_CHARS).contains(&chars.len())
        || !chars.iter().any(|c| c.is_ascii_alphabetic())
        || chars.first().is_some_and(char::is_ascii_digit)
    {
        return false;
    }
    let snake = token.split('_').filter(|part| !part.is_empty()).count() >= 2
        && token
            .split('_')
            .filter(|part| !part.is_empty())
            .all(|part| part.chars().any(|c| c.is_ascii_alphabetic()))
        && token.chars().any(|c| c.is_ascii_lowercase());
    let camel = chars.windows(2).any(|pair| {
        (pair[0].is_ascii_lowercase() || pair[0].is_ascii_digit()) && pair[1].is_ascii_uppercase()
    }) || chars.windows(3).any(|triple| {
        triple[0].is_ascii_uppercase()
            && triple[1].is_ascii_uppercase()
            && triple[2].is_ascii_lowercase()
    });
    snake || camel
}

/// Collects exact camelCase, PascalCase and snake_case tokens from text,
/// each once, at most [`MAX_IDENTIFIERS`]. Ordinary words, ALL_CAPS
/// constants and overlong tokens are not code names. Only the tokens are
/// kept; the text is not.
#[derive(Debug, Default)]
pub struct Identifiers {
    found: Vec<String>,
    seen: HashSet<String>,
}

impl Identifiers {
    pub fn is_full(&self) -> bool {
        self.found.len() >= MAX_IDENTIFIERS
    }

    pub fn push(&mut self, text: &str) {
        let mut start = None;
        for (at, c) in text
            .char_indices()
            .chain(std::iter::once((text.len(), ' ')))
        {
            if c.is_ascii_alphanumeric() || c == '_' {
                start.get_or_insert(at);
                continue;
            }
            let Some(from) = start.take() else {
                continue;
            };
            if self.is_full() {
                return;
            }
            // A token glued to other letters (`naïveValue`) is not ASCII
            // code; skip it whole.
            if c.is_alphanumeric()
                || text[..from]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_alphanumeric)
            {
                continue;
            }
            let token = &text[from..at];
            if identifier_like(token) && self.seen.insert(token.to_owned()) {
                self.found.push(token.to_owned());
            }
        }
    }

    /// The dotfile names (`.gitattributes`, `.env.local`) in the text of a
    /// tab or explorer row. Code like `.then(` is never a file row.
    pub fn push_dotnames(&mut self, text: &str) {
        for word in text.split_whitespace() {
            if self.is_full() {
                return;
            }
            let word = word.trim_matches(|c: char| "\"'`,;:".contains(c));
            let word = word.rsplit(['/', '\\']).next().unwrap_or(word);
            if dotname_like(word) && self.seen.insert(word.to_owned()) {
                self.found.push(word.to_owned());
            }
        }
    }

    pub fn into_vec(self) -> Vec<String> {
        self.found
    }
}

fn dotname_like(token: &str) -> bool {
    let mut chars = token.chars();
    chars.next() == Some('.')
        && chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && (3..=MAX_IDENTIFIER_CHARS).contains(&token.len())
        && token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && !token.ends_with('.')
        && !token.contains("..")
}

/// The identifiers in `sections`, nearest first (see [`Identifiers`]).
pub fn identifiers(sections: &[&str]) -> Vec<String> {
    let mut ids = Identifiers::default();
    for section in sections {
        ids.push(section);
    }
    ids.into_vec()
}

fn push_segment(chars: &[char], out: &mut Vec<String>) {
    if !chars.is_empty() {
        out.push(chars.iter().collect::<String>().to_lowercase());
    }
}

/// The words a person says for an identifier. An acronym ends before the
/// capital that starts the next word: `HTTPServer` is `http server`.
pub fn spoken_segments(identifier: &str) -> Vec<String> {
    let mut out = Vec::new();
    for snake_part in identifier.split('_').filter(|part| !part.is_empty()) {
        let chars: Vec<char> = snake_part.chars().collect();
        let mut current = Vec::new();
        for (i, &c) in chars.iter().enumerate() {
            let boundary = c.is_uppercase()
                && !current.is_empty()
                && (chars[i - 1].is_lowercase()
                    || chars[i - 1].is_ascii_digit()
                    || (chars[i - 1].is_uppercase()
                        && chars.get(i + 1).is_some_and(|next| next.is_lowercase())));
            if boundary {
                push_segment(&current, &mut out);
                current.clear();
            }
            current.push(c);
        }
        push_segment(&current, &mut out);
    }
    out
}

fn spoken_key(text: &str) -> String {
    text.split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

fn unchanged(text: &str) -> Corrected {
    Corrected {
        text: text.to_owned(),
        ..Corrected::default()
    }
}

/// Stage "identifiers" (before S-20's spelling stage): the exact spoken
/// form of a visible identifier ("get user by id", any case, words split by
/// spaces) becomes its visible spelling (`getUserById`). The longest form
/// wins; two visible identifiers with the same spoken form are ambiguous
/// and left alone; a span in `protected` (a dictionary rule's output) is
/// never touched.
pub fn correct(text: &str, visible: &[String], protected: &[(usize, usize)]) -> Corrected {
    if text.is_empty() || visible.is_empty() {
        return unchanged(text);
    }
    let mut by_spoken: HashMap<String, Option<&str>> = HashMap::new();
    for identifier in visible {
        let segments = spoken_segments(identifier);
        if segments.len() < 2 || identifier.starts_with('.') {
            continue;
        }
        by_spoken
            .entry(segments.join(" "))
            .and_modify(|current| {
                if *current != Some(identifier.as_str()) {
                    *current = None;
                }
            })
            .or_insert(Some(identifier.as_str()));
    }
    let mut keys: Vec<&String> = by_spoken
        .iter()
        .filter_map(|(key, identifier)| identifier.map(|_| key))
        .collect();
    if keys.is_empty() {
        return unchanged(text);
    }
    keys.sort_by(|a, b| {
        b.split(' ')
            .count()
            .cmp(&a.split(' ').count())
            .then_with(|| b.len().cmp(&a.len()))
            .then_with(|| a.cmp(b))
    });
    let alternatives = keys
        .iter()
        .map(|key| {
            key.split(' ')
                .map(regex::escape)
                .collect::<Vec<_>>()
                .join(r"[ \t]+")
        })
        .collect::<Vec<_>>()
        .join("|");
    let Ok(re) = Regex::new(&format!(r"(?i)\b(?:{alternatives})\b")) else {
        return unchanged(text);
    };
    let overlaps = |s: usize, e: usize| protected.iter().any(|&(ps, pe)| s < pe && ps < e);
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    let mut edits = Vec::new();
    for found in re.find_iter(text) {
        if overlaps(found.start(), found.end()) {
            continue;
        }
        // Part of a longer token (`_get user`, `get user_2`, `obj.get
        // user`, `get user.ts`): not a phrase. A sentence's own full stop
        // is not glue.
        let glued = |c: Option<char>, beyond: Option<char>| {
            c.is_some_and(|c| {
                c == '_' || c == '@' || (c == '.' && beyond.is_some_and(char::is_alphanumeric))
            })
        };
        let mut before = text[..found.start()].chars().rev();
        let mut after = text[found.end()..].chars();
        if glued(before.next(), before.next()) || glued(after.next(), after.next()) {
            continue;
        }
        let Some(Some(identifier)) = by_spoken.get(&spoken_key(found.as_str())) else {
            continue;
        };
        out.push_str(&text[copied..found.start()]);
        out.push_str(identifier);
        edits.push((found.start(), found.end(), identifier.len()));
        copied = found.end();
    }
    out.push_str(&text[copied..]);
    Corrected {
        fixes: edits.len() as u32,
        text: out,
        edits,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn set_up() -> VibeCodingSettings {
        VibeCodingSettings {
            enabled: true,
            setup_completed: true,
            editors: strings(&["vscode", "cursor", "windsurf"]),
            file_tags_enabled: true,
        }
    }

    #[test]
    fn only_listed_editors_get_their_own_features() {
        let settings = set_up();
        assert_eq!(
            access(&settings, Some("com.microsoft.VSCode")),
            Some(Access {
                editor: Editor::VsCode,
                recognize: true,
                file_tags: false,
            })
        );
        assert_eq!(
            access(&settings, Some("com.microsoft.VSCodeInsiders")).map(|a| a.editor),
            Some(Editor::VsCode)
        );
        assert!(access(&settings, Some("com.todesktop.230313mzl4w4u92"))
            .is_some_and(|a| a.file_tags && a.recognize));
        assert!(access(&settings, Some("com.exafunction.windsurf"))
            .is_some_and(|a| a.file_tags && a.recognize));
        for bundle in [
            "com.apple.TextEdit",
            "com.google.Chrome",
            "com.tinyspeck.slackmacgap",
            "com.todesktop.230313mzl4w4u92.other",
        ] {
            assert_eq!(access(&settings, Some(bundle)), None, "{bundle}");
        }
        assert_eq!(access(&settings, None), None);
    }

    #[test]
    fn code_names_need_the_setup_for_that_editor() {
        let mut fresh = VibeCodingSettings::default();
        assert_eq!(access(&fresh, Some("com.microsoft.VSCode")), None);
        assert_eq!(access(&fresh, Some("com.todesktop.230313mzl4w4u92")), None);
        // A switch alone does nothing before the setup (DEC-41).
        fresh.file_tags_enabled = true;
        fresh.enabled = true;
        assert_eq!(access(&fresh, Some("com.todesktop.230313mzl4w4u92")), None);

        // File tags alone, after the setup: Cursor yes, never VS Code.
        let mut tags_only = set_up();
        tags_only.enabled = false;
        assert_eq!(
            access(&tags_only, Some("com.todesktop.230313mzl4w4u92")),
            Some(Access {
                editor: Editor::Cursor,
                recognize: false,
                file_tags: true,
            })
        );
        assert_eq!(access(&tags_only, Some("com.microsoft.VSCode")), None);

        let mut other_editor = set_up();
        other_editor.editors = strings(&["cursor"]);
        other_editor.file_tags_enabled = false;
        assert_eq!(access(&other_editor, Some("com.microsoft.VSCode")), None);
        assert_eq!(
            access(&other_editor, Some("com.exafunction.windsurf")),
            None
        );
        let mut switched_off = set_up();
        switched_off.enabled = false;
        switched_off.file_tags_enabled = false;
        assert_eq!(access(&switched_off, Some("com.microsoft.VSCode")), None);
    }

    #[test]
    fn only_labelled_chat_inputs_qualify() {
        for (role, labels) in [
            ("AXTextArea", "Cursor chat input"),
            ("AXTextField", "composer-input"),
            ("AXTextArea", "Message Cascade"),
            ("AXTextArea", "Ask anything, @ to mention files"),
            ("AXTextArea", "Plan, @ for context, / for commands"),
            ("AXTextArea", "Agent"),
            // Cursor 3.28.7's agent input, as its AX tree shows it.
            ("AXTextArea", "Prompt aislash-editor-input"),
        ] {
            assert!(is_chat_input(role, labels, ""), "{role}: {labels}");
        }
        for (role, labels) in [
            (
                "AXTextArea",
                "Editor content;Press Alt+F1 for Accessibility Options.",
            ),
            ("AXSearchField", "Chat history search"),
            ("AXTextField", "Search chat"),
            ("AXTextArea", "Terminal 1, chat"),
            ("AXTextField", "Search files by name"),
            ("AXWebArea", "Chat"),
            ("AXTextArea", ""),
            ("AXTextArea", "user@example.com"),
            ("AXTextArea", "Message (⌘⏎ to commit on 'main')"),
            // Monaco's editor buffer in Cursor (screen-reader mode).
            (
                "AXTextArea",
                "user_service.ts inputarea monaco-mouse-cursor-text",
            ),
        ] {
            assert!(!is_chat_input(role, labels, ""), "{role}: {labels}");
        }
    }

    #[test]
    fn a_chat_container_qualifies_an_unnamed_input() {
        // Cursor 3.28.7: the field's own labels are only "Prompt"-free
        // text; its ancestors carry the agent panel's DOM id and classes.
        let cursor = "aislash-editor-grid ai-input-full-input-box composer-input-blur-wrapper \
                      composer-bar workbench.panel.aichat.5e4df9a5 composite auxiliarybar";
        assert!(is_chat_input("AXTextArea", "", cursor));
        // The editor buffer's ancestors name no chat.
        let editor = "overflow-guard monaco-editor editor-instance split-view-view \
                      workbench.parts.editor part editor monaco-grid-view";
        assert!(!is_chat_input("AXTextArea", "user_service.ts", editor));
        // The integrated terminal, even inside a panel named after a chat.
        assert!(!is_chat_input(
            "AXTextArea",
            "",
            "xterm-helper-textarea chat"
        ));
        assert!(!is_chat_input(
            "AXTextArea",
            "",
            "terminal-wrapper composer"
        ));
        assert!(!is_chat_input("AXGroup", "", cursor));
    }

    #[test]
    fn set_up_editors_are_kept_warm() {
        let mut settings = set_up();
        settings.editors = strings(&["cursor"]);
        assert_eq!(
            warm_bundles(&settings, true),
            strings(&["com.todesktop.230313mzl4w4u92"])
        );
        assert!(warm_bundles(&settings, false).is_empty());
        assert!(warm_bundles(&VibeCodingSettings::default(), true).is_empty());
        let all = warm_bundles(&set_up(), true);
        assert!(all.contains(&"com.microsoft.VSCodeInsiders".to_string()));
        assert_eq!(all.len(), 4);
        assert_eq!(
            Editor::from_bundle(Some("COM.TODESKTOP.230313MZL4W4U92")),
            Some(Editor::Cursor)
        );
    }

    #[test]
    fn identifier_extraction_keeps_code_spelling() {
        let visible = [
            "const getUserById = user_id => HTTPServer(user_id);",
            "let count = 1; let simple = true; let MAX_COUNT = 2; let __init__;",
            "getUserById and user_id appear again; 2fast4you",
        ];
        assert_eq!(
            identifiers(&visible),
            strings(&["getUserById", "user_id", "HTTPServer"])
        );
        assert!(identifiers(&["ordinary words and Capitalized Names, iPhone"]).len() == 1);
        assert!(identifiers(&["naïveValue", "über_cool"]).is_empty());
        let many = (0..40)
            .map(|i| format!("valueNumber{i} "))
            .collect::<String>();
        assert_eq!(identifiers(&[&many]).len(), MAX_IDENTIFIERS);
    }

    #[test]
    fn spoken_segments_split_like_speech() {
        assert_eq!(
            spoken_segments("getUserById"),
            strings(&["get", "user", "by", "id"])
        );
        assert_eq!(spoken_segments("HTTPServer"), strings(&["http", "server"]));
        assert_eq!(spoken_segments("user_id"), strings(&["user", "id"]));
        assert_eq!(
            spoken_segments("parseJSONBody"),
            strings(&["parse", "json", "body"])
        );
    }

    #[test]
    fn spoken_identifier_forms_become_visible_names() {
        let visible = strings(&["getUserById", "user_id", "HTTPServer"]);
        let c = correct(
            "Rename get user by id and user id, then call HTTP server.",
            &visible,
            &[],
        );
        assert_eq!(
            c.text,
            "Rename getUserById and user_id, then call HTTPServer."
        );
        assert_eq!(c.fixes, 3);
        assert_eq!(
            correct("get user by identity", &visible, &[]).text,
            "get user by identity"
        );
        assert_eq!(correct("getUserById", &visible, &[]).text, "getUserById");
        // Never across a line break, never inside a longer token.
        assert_eq!(
            correct("get user\nby id", &visible, &[]).text,
            "get user\nby id"
        );
        assert_eq!(correct("old_user id", &visible, &[]).text, "old_user id");
        assert_eq!(
            correct("a.get user by id", &visible, &[]).text,
            "a.get user by id"
        );
        assert_eq!(correct("user id.ts", &visible, &[]).text, "user id.ts");
        assert_eq!(
            correct("Call HTTP server.", &visible, &[]).text,
            "Call HTTPServer."
        );
    }

    #[test]
    fn ambiguity_and_protected_spans_are_left_alone() {
        assert_eq!(
            correct("get user", &strings(&["getUser", "get_user"]), &[]).text,
            "get user"
        );
        assert_eq!(
            correct("get user by id", &strings(&["getUserById", "getUser"]), &[]).text,
            "getUserById"
        );
        let text = "call get user by id now";
        let start = text.find("get").unwrap();
        let c = correct(text, &strings(&["getUserById"]), &[(start, start + 3)]);
        assert_eq!(c.text, text);
        assert_eq!(c.fixes, 0);
    }

    #[test]
    fn visible_dotnames_are_collected() {
        let mut ids = Identifiers::default();
        ids.push_dotnames("src/.storybook  .env.production, .then( ... .x.");
        assert_eq!(ids.into_vec(), strings(&[".storybook", ".env.production"]));
        // A dotfile name is never an identifier's spoken form.
        let visible = strings(&[".env.production", ".DS_STORE"]);
        assert_eq!(correct("ds store", &visible, &[]).text, "ds store");
    }
}
