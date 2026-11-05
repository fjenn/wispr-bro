// SPDX-License-Identifier: GPL-3.0-or-later
//! "Scratch that" (R5 S-11): deletes the clause spoken right before it,
//! within the same take and the same line.
//!
//! The clause is the text back to the previous clause punctuation or line
//! break, not counting the punctuation the recognizer put right before the
//! command. When the command directly follows a sentence end ("Meet Monday.
//! Scratch that."), the whole sentence goes. Examples:
//!
//! - "Hi John, I'll be late, scratch that, I'll be on time." →
//!   "Hi John, I'll be on time."
//! - "I'll bring chips, salsa, scratch that." → "I'll bring chips."
//! - "Let's meet Monday. Scratch that. Let's meet Tuesday." →
//!   "Let's meet Tuesday."
//!
//! Undoing an earlier take (cross-take "scratch that") needs the field
//! itself and is out of scope (R4c §1.7).

use super::commands::{capitalize_first, is_engine_punct};

fn is_blank(c: char) -> bool {
    c == ' ' || c == '\t'
}

fn is_terminator(c: char) -> bool {
    matches!(c, '.' | '!' | '?' | '…' | '。' | '！' | '？')
}

fn is_soft(c: char) -> bool {
    matches!(c, ',' | ';' | ':' | '—' | '–' | '，' | '；' | '：' | '、')
}

fn is_boundary(c: char) -> bool {
    c == '\n' || is_terminator(c) || is_soft(c)
}

/// Removes the command at `start..end` and the clause before it. Returns the
/// new text and where scanning goes on.
pub fn apply(text: &str, start: usize, end: usize) -> (String, usize) {
    let before = text[..start]
        .trim_end_matches(is_blank)
        .trim_end_matches(is_soft)
        .trim_end_matches(is_blank);
    let before = before.trim_end_matches(is_terminator);
    let clause_start = before
        .char_indices()
        .rev()
        .find(|(_, c)| is_boundary(*c))
        .map_or(0, |(i, c)| i + c.len_utf8());
    let mut kept = before[..clause_start]
        .trim_end_matches(is_blank)
        .to_string();

    let tail = text[end..].trim_start_matches(is_blank);
    let consumed = tail.len() - tail.trim_start_matches(is_engine_punct).len();
    let terminator = tail[..consumed].chars().find(|c| is_terminator(*c));
    let after = tail[consumed..].trim_start_matches(is_blank);

    if after.is_empty() || after.starts_with('\n') {
        // Nothing follows on this line: close what is left properly.
        kept = kept
            .trim_end_matches(is_soft)
            .trim_end_matches(is_blank)
            .to_string();
        if let Some(t) = terminator {
            if !kept.is_empty() && !kept.ends_with(is_terminator) && !kept.ends_with('\n') {
                kept.push(t);
            }
        }
    }
    let opens_sentence = kept.is_empty() || kept.ends_with(is_terminator) || kept.ends_with('\n');
    let gap =
        if kept.is_empty() || kept.ends_with('\n') || after.is_empty() || after.starts_with('\n') {
            ""
        } else {
            " "
        };
    let after = if opens_sentence {
        capitalize_first(after)
    } else {
        after.to_string()
    };
    let at = kept.len() + gap.len();
    (format!("{kept}{gap}{after}"), at)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(text: &str) -> String {
        let start = text.to_lowercase().find("scratch that").unwrap();
        apply(text, start, start + "scratch that".len()).0
    }

    #[test]
    fn deletes_the_previous_clause() {
        assert_eq!(
            scratch("Hi John, I'll be late, scratch that, I'll be on time."),
            "Hi John, I'll be on time."
        );
        assert_eq!(
            scratch("I'll bring chips, salsa, scratch that."),
            "I'll bring chips."
        );
        assert_eq!(
            scratch("Let's meet Monday. Scratch that. Let's meet Tuesday."),
            "Let's meet Tuesday."
        );
        assert_eq!(
            scratch("First one. Second one. Scratch that. third."),
            "First one. Third."
        );
    }

    #[test]
    fn nothing_before_and_line_boundaries() {
        assert_eq!(scratch("Scratch that. Hello"), "Hello");
        assert_eq!(
            scratch("Dear Ann,\nsee you Monday, scratch that, Tuesday"),
            "Dear Ann,\nTuesday"
        );
        assert_eq!(scratch("Top\nwrong words. Scratch that."), "Top\n");
    }
}
