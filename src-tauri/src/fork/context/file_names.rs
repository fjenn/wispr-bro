// SPDX-License-Identifier: GPL-3.0-or-later
//! Vibe coding (S-36): the file and folder names one editor take knows
//! (the project's list from [`super::workspace`], and the names visible in
//! the editor's tabs and explorer), and the spoken form of each.
//!
//! [`respell`] turns a file name as a recognizer writes it ("check dot git
//! attributes", "check.git attributes", "user service dot ts") into the
//! project's exact name (`.gitattributes`, `user_service.ts`). The spoken
//! form of a name is its letters and digits, lower case, with its dots:
//! `_`, `-`, spaces and case do not count, so snake, kebab and camel case
//! all match their spoken words. A dotfile also matches with its inner dots
//! left out ("dot env local" → `.env.local`). Only names with a dot are
//! respelled, and only when a dot is said or written: "git attributes" in a
//! sentence stays.

use std::collections::HashMap;
use std::sync::Arc;

use super::hints::Corrected;

/// Words one spoken file name spans at most (`user profile dot test dot ts`
/// is 7 with its dots).
const MAX_SPAN_TOKENS: usize = 16;

/// Which list a name came from: a visible one wins over the project's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Tier {
    Visible,
    Project,
}

/// Names by key; `None` when two different names share the key within the
/// best tier that has it (ambiguous: left alone).
#[derive(Debug, Default)]
struct Keyed(HashMap<String, (Tier, Option<Arc<str>>)>);

impl Keyed {
    fn insert(&mut self, key: String, tier: Tier, name: &Arc<str>) {
        match self.0.get_mut(&key) {
            None => {
                self.0.insert(key, (tier, Some(name.clone())));
            }
            Some((have, current)) => {
                if tier < *have {
                    *have = tier;
                    *current = Some(name.clone());
                } else if tier == *have && current.as_deref() != Some(&**name) {
                    *current = None;
                }
            }
        }
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key)?.1.as_deref()
    }
}

/// The names one take knows, looked up by their written form (any case)
/// or their spoken key.
#[derive(Debug, Default)]
pub struct NameSet {
    exact: Keyed,
    spoken: Keyed,
    len: usize,
}

/// The spoken key of a name with a dot (`user_service.ts` →
/// `userservice.ts`), or `None` for a name nobody says with a dot or that
/// has characters a recognizer never writes (`+`, `@`, spaces aside).
pub fn spoken_key(name: &str) -> Option<String> {
    let mut key = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_alphanumeric() {
            key.extend(c.to_lowercase());
        } else if c == '.' {
            if key.ends_with('.') {
                return None;
            }
            key.push('.');
        } else if !matches!(c, '_' | '-' | ' ') {
            return None;
        }
    }
    let has_inner_dot = key
        .char_indices()
        .any(|(i, c)| c == '.' && key[i + 1..].starts_with(|n: char| n.is_alphanumeric()));
    (has_inner_dot && !key.ends_with('.') && key.chars().filter(|c| *c != '.').count() >= 2)
        .then_some(key)
}

/// A dotfile's key without its inner dots (`.env.local` → `.envlocal`).
fn loose_key(key: &str) -> Option<String> {
    let rest = key.strip_prefix('.')?;
    rest.contains('.')
        .then(|| format!(".{}", rest.replace('.', "")))
}

impl NameSet {
    /// `visible`: names in the editor's tabs and explorer (they win);
    /// `project`: the project's list.
    pub fn build<'a>(
        visible: impl IntoIterator<Item = &'a str>,
        project: impl IntoIterator<Item = &'a str>,
    ) -> NameSet {
        let mut set = NameSet::default();
        let tiers = visible
            .into_iter()
            .map(|n| (Tier::Visible, n))
            .chain(project.into_iter().map(|n| (Tier::Project, n)));
        for (tier, name) in tiers {
            if name.is_empty() {
                continue;
            }
            let shared: Arc<str> = Arc::from(name);
            set.exact.insert(name.to_lowercase(), tier, &shared);
            if let Some(key) = spoken_key(name) {
                if let Some(loose) = loose_key(&key) {
                    set.spoken.insert(loose, tier, &shared);
                }
                set.spoken.insert(key, tier, &shared);
            }
            set.len += 1;
        }
        set
    }

    /// Visible names only (tests, and takes without a project).
    pub fn from_names(names: &[String]) -> NameSet {
        NameSet::build(names.iter().map(String::as_str), std::iter::empty())
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Names added (with repeats).
    pub fn len(&self) -> usize {
        self.len
    }

    /// The one name written `written` in any case.
    pub fn exact(&self, written: &str) -> Option<&str> {
        self.exact.get(&written.to_lowercase())
    }

    /// The one name with the spoken key `key`.
    pub fn spoken(&self, key: &str) -> Option<&str> {
        self.spoken.get(key)
    }

    /// The names written as `@name` in `text` (each once, as the set spells
    /// them; an `@` after a letter, as in an e-mail address, is no tag).
    pub fn mentioned_in(&self, text: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for (at, _) in text.match_indices('@') {
            if text[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric())
            {
                continue;
            }
            let rest = &text[at + 1..];
            let end = rest
                .char_indices()
                .find(|(_, c)| !(c.is_alphanumeric() || matches!(c, '_' | '-' | '.')))
                .map_or(rest.len(), |(i, _)| i);
            // The longest name the token starts with ("@index.ts." ends a
            // sentence).
            let token = &rest[..end];
            let mut cut = token.len();
            loop {
                let candidate = &token[..cut];
                if let Some(name) = self.exact(candidate).filter(|n| *n == candidate) {
                    if !out.iter().any(|o| o == name) {
                        out.push(name.to_owned());
                    }
                    break;
                }
                match candidate.rfind('.') {
                    Some(dot) if dot > 0 => cut = dot,
                    _ => break,
                }
            }
        }
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Word,
    /// The word "dot".
    SaidDot,
    /// A `.` with a letter or digit right after it.
    WrittenDot,
}

#[derive(Clone, Debug)]
struct Token {
    start: usize,
    end: usize,
    kind: Kind,
    /// The token's part of a key: lower case, without `_` and `-`.
    key: String,
    /// Only spaces or tabs (or nothing) between this token and the next.
    joins_next: bool,
}

fn tokens(text: &str) -> Vec<Token> {
    let mut out: Vec<Token> = Vec::new();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let word_char = |c: char| c.is_alphanumeric() || c == '_';
    let mut i = 0;
    // Whether the text since the last token is only blanks.
    let mut blank_gap = true;
    while i < chars.len() {
        let (at, c) = chars[i];
        if word_char(c) {
            let start = at;
            let mut j = i;
            while j < chars.len()
                && (word_char(chars[j].1)
                    || (chars[j].1 == '-'
                        && j > i
                        && chars.get(j + 1).is_some_and(|n| word_char(n.1))))
            {
                j += 1;
            }
            let end = chars.get(j).map_or(text.len(), |c| c.0);
            let word = &text[start..end];
            let kind = if word.eq_ignore_ascii_case("dot") {
                Kind::SaidDot
            } else {
                Kind::Word
            };
            let key = if kind == Kind::SaidDot {
                ".".to_owned()
            } else {
                word.chars()
                    .filter(|c| c.is_alphanumeric())
                    .flat_map(char::to_lowercase)
                    .collect()
            };
            if let Some(last) = out.last_mut() {
                last.joins_next &= blank_gap;
            }
            out.push(Token {
                start,
                end,
                kind,
                key,
                joins_next: true,
            });
            blank_gap = true;
            i = j;
            continue;
        }
        if c == '.' && chars.get(i + 1).is_some_and(|n| n.1.is_alphanumeric()) {
            if let Some(last) = out.last_mut() {
                last.joins_next &= blank_gap;
            }
            out.push(Token {
                start: at,
                end: at + 1,
                kind: Kind::WrittenDot,
                key: ".".to_owned(),
                joins_next: true,
            });
            blank_gap = true;
            i += 1;
            continue;
        }
        if !matches!(c, ' ' | '\t') {
            blank_gap = false;
            if let Some(last) = out.last_mut() {
                last.joins_next = false;
            }
        }
        i += 1;
    }
    out
}

/// Stage "identifiers", first pass: file names as a recognizer writes them
/// become the names in `names` (see the module docs). The longest name
/// wins; an ambiguous key, a span in `protected` (a dictionary rule's
/// output), a name inside a longer token, path or address, and text that
/// already reads as the name are left alone. A dot the cleanup glued to the
/// word before ("check.git attributes") is split off again, but only for a
/// name said as two or more words, so `this.cursor` stays.
pub fn respell(text: &str, names: &NameSet, protected: &[(usize, usize)]) -> Corrected {
    let unchanged = || Corrected {
        text: text.to_owned(),
        ..Corrected::default()
    };
    if names.is_empty() || !(text.contains('.') || text.to_lowercase().contains("dot")) {
        return unchanged();
    }
    let toks = tokens(text);
    let overlaps = |s: usize, e: usize| protected.iter().any(|&(ps, pe)| s < pe && ps < e);
    let mut out = String::with_capacity(text.len() + 8);
    let mut copied = 0;
    let mut edits = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let mut key = String::new();
        let mut best: Option<(usize, &str)> = None;
        for (j, tok) in toks.iter().enumerate().skip(i).take(MAX_SPAN_TOKENS) {
            key.push_str(&tok.key);
            if tok.kind == Kind::Word {
                if let Some(name) = names.spoken(&key) {
                    best = Some((j, name));
                }
            }
            if !tok.joins_next {
                break;
            }
        }
        let Some((j, name)) = best else {
            i += 1;
            continue;
        };
        let (start, end) = (toks[i].start, toks[j].end);
        let before = text[..start].chars().next_back();
        let mut after = text[end..].chars();
        let (next, beyond) = (after.next(), after.next());
        // Part of a longer name (`.env.local.bak`, `.env_x`, `a-b`) or of a
        // path or address.
        let longer = next.is_some_and(|c| matches!(c, '_' | '@' | '/' | '\\'))
            || (next.is_some_and(|c| matches!(c, '.' | '-'))
                && beyond.is_some_and(char::is_alphanumeric));
        let words = toks[i..=j].iter().filter(|t| t.kind == Kind::Word).count();
        let spaced = match (toks[i].kind, before) {
            (_, None) => Some(false),
            (Kind::WrittenDot, Some(c)) if c.is_whitespace() || "([{\"'`".contains(c) => {
                Some(false)
            }
            (Kind::WrittenDot, Some(c)) if c.is_alphanumeric() && words >= 2 => Some(true),
            (Kind::WrittenDot, Some(_)) => None,
            (_, Some('.' | '_' | '@' | '/' | '\\' | ':' | '-')) => None,
            _ => Some(false),
        };
        let Some(spaced) = spaced.filter(|_| !longer && !overlaps(start, end)) else {
            i += 1;
            continue;
        };
        let replacement = if spaced {
            format!(" {name}")
        } else {
            name.to_owned()
        };
        if text[start..end] != replacement {
            out.push_str(&text[copied..start]);
            out.push_str(&replacement);
            edits.push((start, end, replacement.len()));
            copied = end;
        }
        i = j + 1;
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

    fn project() -> NameSet {
        NameSet::build(
            std::iter::empty(),
            [
                ".gitattributes",
                ".gitignore",
                ".env",
                ".env.local",
                ".vscode",
                ".DS_Store",
                ".github",
                ".cursor",
                "user_service.ts",
                "UserCard.tsx",
                "user-profile.test.ts",
                "README.md",
                "Cargo.toml",
                "index.ts",
                "src",
                "docs",
                "prod.env",
            ],
        )
    }

    fn fix(text: &str) -> String {
        respell(text, &project(), &[]).text
    }

    /// The matching table: what a recognizer writes → what the take keeps.
    #[test]
    fn spoken_and_mangled_names_become_project_names() {
        for (said, kept) in [
            // The owner's rc4 take in Cursor.
            ("check.git attributes", "check .gitattributes"),
            ("check .git attributes", "check .gitattributes"),
            ("check dot git attributes", "check .gitattributes"),
            ("Check dot gitattributes.", "Check .gitattributes."),
            ("add it to the dot git ignore", "add it to the .gitignore"),
            ("Dot env local has the key", ".env.local has the key"),
            ("put it in dot env", "put it in .env"),
            ("open .vs code and .ds store", "open .vscode and .DS_Store"),
            ("see dot git hub", "see .github"),
            // snake, camel and kebab case, extensions.
            ("open user service dot ts", "open user_service.ts"),
            ("open user service.ts", "open user_service.ts"),
            ("open User Service dot TS", "open user_service.ts"),
            ("edit user card dot tsx now", "edit UserCard.tsx now"),
            (
                "run user profile dot test dot ts",
                "run user-profile.test.ts",
            ),
            ("read me dot md", "README.md"),
            ("bump cargo dot toml.", "bump Cargo.toml."),
            ("fix index dot ts, then commit", "fix index.ts, then commit"),
            ("in usercard.tsx", "in UserCard.tsx"),
        ] {
            assert_eq!(fix(said), kept, "{said:?}");
        }
        let c = respell("check.git attributes and dot env", &project(), &[]);
        assert_eq!(c.fixes, 2);
        assert_eq!(c.text, "check .gitattributes and .env");
    }

    #[test]
    fn sentences_code_paths_and_unknown_names_stay() {
        for text in [
            "I checked it. Git attributes are fine.",
            "Done. Cursor rules are next.",
            "git attributes and env vars",
            "user service ts",
            "Read this.cursor and prod.env",
            "keep .gitattributes and .env.local as they are",
            "see src/.env.local.bak",
            "src/user_service.ts",
            "old_user service dot ts",
            "e.g. env vars",
            "other dot js",
            "a.user service dot ts",
            "me@index.ts",
            "connect the dot and the line",
            "",
        ] {
            assert_eq!(fix(text), text, "{text:?}");
        }
        let text = "dot env";
        assert_eq!(respell(text, &project(), &[(0, 3)]).text, text);
        assert_eq!(respell("dot env", &NameSet::default(), &[]).text, "dot env");
    }

    #[test]
    fn ambiguous_names_stay_and_visible_names_win() {
        let both = NameSet::build(std::iter::empty(), ["user_service.ts", "user-service.ts"]);
        assert_eq!(
            respell("user service dot ts", &both, &[]).text,
            "user service dot ts"
        );
        // The open tab decides.
        let tab = NameSet::build(["user-service.ts"], ["user_service.ts", "user-service.ts"]);
        assert_eq!(
            respell("user service dot ts", &tab, &[]).text,
            "user-service.ts"
        );
        // Case alone also differs: ambiguous in the project.
        let cased = NameSet::build(std::iter::empty(), ["index.ts", "Index.ts"]);
        assert_eq!(respell("index dot ts", &cased, &[]).text, "index dot ts");
        assert_eq!(cased.exact("INDEX.TS"), None);
        assert_eq!(project().exact("readme.MD"), Some("README.md"));
    }

    #[test]
    fn spoken_keys_ignore_case_and_separators() {
        assert_eq!(
            spoken_key("user_service.ts").as_deref(),
            Some("userservice.ts")
        );
        assert_eq!(spoken_key("UserCard.tsx").as_deref(), Some("usercard.tsx"));
        assert_eq!(spoken_key(".DS_Store").as_deref(), Some(".dsstore"));
        assert_eq!(spoken_key(".env.local").as_deref(), Some(".env.local"));
        for none in ["src", "Makefile", "a.", "..x", "c++.md", ".x", "v1.2+3"] {
            assert_eq!(spoken_key(none), None, "{none}");
        }
    }

    #[test]
    fn tags_in_text_are_found_by_name() {
        let set = project();
        assert_eq!(
            set.mentioned_in("look at @user_service.ts and @README.md. Also me@index.ts"),
            vec!["user_service.ts", "README.md"]
        );
        assert!(set.mentioned_in("@nothing.ts").is_empty());
    }

    #[test]
    fn fifty_thousand_names_build_and_match_quickly() {
        let names: Vec<String> = (0..50_000).map(|i| format!("module_{i}.rs")).collect();
        let timer = crate::fork::testutil::CpuTimer::start();
        let set = NameSet::build(std::iter::empty(), names.iter().map(String::as_str));
        let c = respell("open module 49999 dot rs and module 7 dot rs", &set, &[]);
        assert_eq!(c.text, "open module_49999.rs and module_7.rs");
        assert!(timer.elapsed() < std::time::Duration::from_millis(1_500));
    }
}
