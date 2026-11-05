// SPDX-License-Identifier: GPL-3.0-or-later
//! "Tag files in chat" (S-36, D-60): in Cursor's or Windsurf's chat input,
//! a spoken file name ("index dot ts") becomes a tag (`@index.ts`) when the
//! project has that exact name: a file in its list
//! (`context::workspace`, names only) or one visible in the editor's tabs or
//! file explorer ([`FileNames`], fed by `context::caret`), which win.
//!
//! This module never lists a directory or opens a file itself.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::fork::context::file_names::NameSet;
use crate::fork::context::hints::Corrected;

/// Visible file names one take keeps at most.
pub const MAX_FILE_NAMES: usize = 40;
const MAX_FILE_NAME_CHARS: usize = 120;
const MAX_EXTENSION_CHARS: usize = 12;

/// A spoken (`index dot ts`, `user-profile dot test dot ts`) or written
/// (`index.ts`) file name: word characters and `-`, joined by "dot" or `.`.
static FILE_MENTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)[\p{L}\p{N}_-]+(?:(?:[ \t]+dot[ \t]+|\.)[\p{L}\p{N}_-]+)+")
        .expect("static file-mention regex")
});

fn file_name_like(token: &str) -> bool {
    if token.chars().count() > MAX_FILE_NAME_CHARS
        || token.starts_with(['.', '-'])
        || token.ends_with(['.', '-'])
        || token.contains("..")
        || !token.chars().any(char::is_alphabetic)
    {
        return false;
    }
    let Some((stem, extension)) = token.rsplit_once('.') else {
        return false;
    };
    !stem.is_empty()
        && (1..=MAX_EXTENSION_CHARS).contains(&extension.chars().count())
        && extension.chars().all(char::is_alphanumeric)
        && extension.chars().any(char::is_alphabetic)
}

/// Collects file names from the text of AX tab and explorer rows, each
/// once, at most [`MAX_FILE_NAMES`]. Paths keep only their last part; URLs
/// and bare words are not names. Only the names are kept.
#[derive(Debug, Default)]
pub struct FileNames {
    found: Vec<String>,
    seen: HashSet<String>,
}

impl FileNames {
    pub fn is_full(&self) -> bool {
        self.found.len() >= MAX_FILE_NAMES
    }

    pub fn push(&mut self, text: &str) {
        for line in text.lines() {
            if line.contains("://") {
                continue;
            }
            let mut start = None;
            for (at, c) in line
                .char_indices()
                .chain(std::iter::once((line.len(), ' ')))
            {
                if c.is_alphanumeric() || matches!(c, '_' | '-' | '.') {
                    start.get_or_insert(at);
                    continue;
                }
                let Some(from) = start.take() else {
                    continue;
                };
                if self.is_full() {
                    return;
                }
                let name = &line[from..at];
                if file_name_like(name) && self.seen.insert(name.to_owned()) {
                    self.found.push(name.to_owned());
                }
            }
        }
    }

    pub fn into_vec(self) -> Vec<String> {
        self.found
    }
}

/// The file names in `lines` (see [`FileNames`]).
pub fn visible_names(lines: &[String]) -> Vec<String> {
    let mut names = FileNames::default();
    for line in lines {
        names.push(line);
    }
    names.into_vec()
}

/// The written form of a mention: "dot" words become `.`.
fn written(mention: &str) -> String {
    static DOT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)[ \t]+dot[ \t]+").expect("static dot regex"));
    DOT.replace_all(mention, ".").into_owned()
}

/// Turns each mention of a known file name into `@name`, spelled as the
/// project or the editor shows it. Spoken ("index dot ts") and written
/// (`index.ts`, as a cleanup may write it) mentions both count. Nothing
/// changes for an unknown name, a name two spellings share (`index.ts` and
/// `Index.ts`), a mention already tagged or inside a path, URL or e-mail
/// address, or when no names are known.
pub fn rewrite(text: &str, names: &NameSet) -> Corrected {
    rewrite_outside(text, names, &[])
}

/// [`rewrite`], never touching the byte spans in `protected` (dictionary
/// rule outputs).
pub fn rewrite_outside(text: &str, names: &NameSet, protected: &[(usize, usize)]) -> Corrected {
    let unchanged = || Corrected {
        text: text.to_owned(),
        ..Corrected::default()
    };
    if text.is_empty() || names.is_empty() {
        return unchanged();
    }
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    let mut edits = Vec::new();
    for found in FILE_MENTION.find_iter(text) {
        let before = text[..found.start()].chars().next_back();
        let after = text[found.end()..].chars().next();
        // Already a tag, or part of a path, URL, address or longer word.
        if before.is_some_and(|c| matches!(c, '@' | '/' | '\\' | ':' | '.' | '`'))
            || after.is_some_and(|c| matches!(c, '/' | '\\' | '@' | '`'))
        {
            continue;
        }
        if protected
            .iter()
            .any(|&(start, end)| found.start() < end && start < found.end())
        {
            continue;
        }
        let Some(name) = names
            .exact(&written(found.as_str()))
            .filter(|name| file_name_like(name))
        else {
            continue;
        };
        let replacement = format!("@{name}");
        out.push_str(&text[copied..found.start()]);
        out.push_str(&replacement);
        edits.push((found.start(), found.end(), replacement.len()));
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

    fn rewrite(text: &str, visible: &[String]) -> Corrected {
        super::rewrite(text, &NameSet::from_names(visible))
    }

    #[test]
    fn extraction_keeps_only_visible_file_names() {
        let lines = strings(&[
            "index.ts",
            "src/user-profile.test.ts",
            "README.md, modified",
            "https://example.com/index.html",
            "no extension here",
            "Version 1.2 and e.g. this",
        ]);
        assert_eq!(
            visible_names(&lines),
            strings(&["index.ts", "user-profile.test.ts", "README.md"])
        );
        let many = (0..60).map(|i| format!("file{i}.ts")).collect::<Vec<_>>();
        assert_eq!(visible_names(&many).len(), MAX_FILE_NAMES);
    }

    #[test]
    fn spoken_names_become_tags_only_when_visible() {
        let visible = strings(&["index.ts", "user-profile.test.ts"]);
        let changed = rewrite(
            "Open index dot ts and user-profile dot test dot ts, not other dot js.",
            &visible,
        );
        assert_eq!(
            changed.text,
            "Open @index.ts and @user-profile.test.ts, not other dot js."
        );
        assert_eq!(changed.fixes, 2);
        assert_eq!(rewrite("Index Dot TS", &visible).text, "@index.ts");
        assert_eq!(rewrite("index dot tsx", &visible).text, "index dot tsx");
        assert_eq!(rewrite("index dot ts", &[]).text, "index dot ts");
    }

    #[test]
    fn written_names_tag_once_and_paths_stay() {
        let visible = strings(&["index.ts"]);
        assert_eq!(
            rewrite("Fix index.ts please.", &visible).text,
            "Fix @index.ts please."
        );
        assert_eq!(rewrite("@index.ts", &visible).text, "@index.ts");
        let twice = rewrite("Fix index.ts please.", &visible).text;
        assert_eq!(rewrite(&twice, &visible).text, twice);
        for kept in [
            "src/index.ts",
            "see https://example.com/index.ts",
            "me@index.ts",
            "`index.ts`",
            "app.index.ts",
        ] {
            assert_eq!(rewrite(kept, &visible).text, kept, "{kept}");
        }
    }

    #[test]
    fn any_project_file_is_tagged_and_visible_names_win() {
        let names = NameSet::build(
            ["user-service.ts"],
            [
                "user_service.ts",
                "user-service.ts",
                "deep_module.rs",
                ".env",
            ],
        );
        assert_eq!(
            super::rewrite("see deep_module.rs and user-service dot ts", &names).text,
            "see @deep_module.rs and @user-service.ts"
        );
        // A dotfile or folder without an extension is never a tag here.
        assert_eq!(super::rewrite("check env", &names).text, "check env");
    }

    #[test]
    fn case_ambiguity_and_punctuation_are_safe() {
        assert_eq!(
            rewrite("index dot ts", &strings(&["index.ts", "Index.ts"])).text,
            "index dot ts"
        );
        assert_eq!(
            rewrite(
                "Use index dot ts, then index dot ts.",
                &strings(&["index.ts"])
            )
            .text,
            "Use @index.ts, then @index.ts."
        );
    }
}
