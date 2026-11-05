// SPDX-License-Identifier: GPL-3.0-or-later
// Ported from SpeakoFlow (https://github.com/AbhishekBarali/SpeakoFlow,
// commit 9818a17f), src-tauri/src/smart_spacing/rules.rs
// (`needs_leading_space` and the character classes it uses).
// Copyright (c) 2026 Abhishek Barali. Licence: MIT.
// Modified by Wispr Bro contributors, 2026-11-04: only the leading-space
// rule is kept; the character before the caret must be a letter, a digit, a
// closing mark or a spaced dash (no space after symbols or emoji); the list
// marker check is reduced to a bare "-"/"*"/"+"/"•"; the dictation is never
// trimmed; case, trailing space and final-period rules are not ported.
//! Smart leading space (S-05 follow-up after rc4): a dictation that goes
//! right after a word gets one space in front of it, as Wispr Flow does.
//!
//! Pure: [`lead`] takes the text before the caret (a short tail read by
//! `fork::ax::before_caret`, `None` when AX could not tell) and the
//! dictation, and returns the dictation, with a space in front only when
//! the character before the caret is a letter, a digit or a closing mark
//! (`. , ; : ! ? … % ) ] } " ' ” ’ »`), and the dictation starts with a
//! word or an opening mark. Never at the start of a field, after
//! whitespace or a line break, after an opening bracket or quote, before
//! punctuation, or next to a script written without spaces (CJK, Thai,
//! Lao, Khmer, Myanmar...). Unknown field: the dictation is unchanged.
//!
//! Handy's "append trailing space" setting is applied after this, so two
//! takes in a row get exactly one space: the second sees the first's
//! trailing space before the caret and adds none.

/// How many UTF-16 units before the caret the read takes: enough for the
/// elision, list-marker and URL checks below; the rule itself looks mostly
/// at the last character.
pub const TAIL_UNITS: isize = 256;

/// The dictation as it should be pasted after `before` (`None`: the field
/// could not be read; nothing changes).
pub fn lead(text: String, before: Option<&str>) -> String {
    match before {
        Some(before) if needs_leading_space(before, &text) => format!(" {text}"),
        _ => text,
    }
}

/// An embedded object (an attachment, an image) counts as a line break.
fn object_to_newline(c: char) -> char {
    if c == '\u{FFFC}' {
        '\n'
    } else {
        c
    }
}

/// Horizontal whitespace: everything that separates words on one line.
fn is_hspace(c: char) -> bool {
    matches!(
        c,
        ' ' | '\t' | '\u{00A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    )
}

fn is_space(c: char) -> bool {
    is_hspace(c) || c.is_whitespace()
}

/// Marks that open something; no space follows them.
fn is_opener(c: char) -> bool {
    matches!(
        c,
        '(' | '['
            | '{'
            | '\u{201C}'
            | '\u{2018}'
            | '\u{00AB}'
            | '\u{2039}'
            | '\u{201E}'
            | '\u{201A}'
            | '\u{00BF}'
            | '\u{00A1}'
            | '\u{300C}'
            | '\u{300E}'
            | '\u{3010}'
            | '\u{3008}'
            | '\u{300A}'
            | '\u{3014}'
            | '\u{FF08}'
            | '\u{FF3B}'
            | '\u{FF5B}'
    )
}

/// Marks that close something.
fn is_closer(c: char) -> bool {
    matches!(
        c,
        ')' | ']'
            | '}'
            | '"'
            | '\''
            | '\u{201D}'
            | '\u{2019}'
            | '\u{00BB}'
            | '\u{203A}'
            | '\u{300D}'
            | '\u{300F}'
            | '\u{3011}'
            | '\u{FF09}'
            | '\u{FF3D}'
            | '\u{FF5D}'
    )
}

/// Characters that hang on the previous word: no space goes before them.
fn is_hanger(c: char) -> bool {
    matches!(
        c,
        '.' | ','
            | ';'
            | ':'
            | '!'
            | '?'
            | '%'
            | ')'
            | ']'
            | '}'
            | '\u{2026}'
            | '\u{2030}'
            | '\u{201D}'
            | '\u{2019}'
            | '\u{00BB}'
            | '\u{203A}'
            | '\u{0964}'
            | '\u{0965}'
            | '\u{060C}'
            | '\u{061B}'
            | '\u{061F}'
            | '\u{06D4}'
            | '\u{0589}'
            | '\u{1361}'..='\u{1368}'
            | '\u{3000}'..='\u{303F}'
            | '\u{FF01}'..='\u{FF0F}'
            | '\u{FF1A}'..='\u{FF20}'
            | '\u{FF3D}'
            | '\u{FF5D}'
            | '\u{FF61}'..='\u{FF65}'
    )
}

/// Scripts written without spaces between words. No ASCII space is ever put
/// next to them. Hangul is deliberately absent: Korean separates words with
/// spaces.
fn is_spaceless(c: char) -> bool {
    matches!(
        c,
        '\u{0E00}'..='\u{0E7F}'   // Thai
            | '\u{0E80}'..='\u{0EFF}' // Lao
            | '\u{0F00}'..='\u{0FFF}' // Tibetan
            | '\u{1000}'..='\u{109F}' // Myanmar
            | '\u{1780}'..='\u{17FF}' // Khmer
            | '\u{1950}'..='\u{197F}' // Tai Le
            | '\u{1980}'..='\u{19DF}' // New Tai Lue
            | '\u{19E0}'..='\u{19FF}' // Khmer symbols
            | '\u{1A20}'..='\u{1AAF}' // Tai Tham
            | '\u{1B00}'..='\u{1B7F}' // Balinese
            | '\u{1BC0}'..='\u{1BFF}' // Batak
            | '\u{2E80}'..='\u{2FDF}' // CJK radicals
            | '\u{3000}'..='\u{303F}' // CJK punctuation
            | '\u{3040}'..='\u{309F}' // Hiragana
            | '\u{30A0}'..='\u{30FF}' // Katakana
            | '\u{3100}'..='\u{312F}' // Bopomofo
            | '\u{31A0}'..='\u{31BF}' // Bopomofo extended
            | '\u{31F0}'..='\u{31FF}' // Katakana extensions
            | '\u{3400}'..='\u{4DBF}' // CJK extension A
            | '\u{4E00}'..='\u{9FFF}' // CJK unified ideographs
            | '\u{A500}'..='\u{A63F}' // Vai
            | '\u{A980}'..='\u{A9DF}' // Javanese
            | '\u{A9E0}'..='\u{A9FF}' // Myanmar extended B
            | '\u{AA00}'..='\u{AA5F}' // Cham
            | '\u{AA60}'..='\u{AA7F}' // Myanmar extended A
            | '\u{AA80}'..='\u{AADF}' // Tai Viet
            | '\u{F900}'..='\u{FAFF}' // CJK compatibility ideographs
            | '\u{FE10}'..='\u{FE1F}' // vertical forms
            | '\u{FE30}'..='\u{FE4F}' // CJK compatibility forms
            | '\u{FF00}'..='\u{FFEF}' // fullwidth and halfwidth forms
            | '\u{20000}'..='\u{3134F}' // CJK extensions B and later
    )
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

const ELISIONS: &[&str] = &[
    "l", "d", "j", "m", "n", "s", "t", "c", "qu", "jusqu", "lorsqu", "puisqu", "quoiqu", "presqu",
    "dell", "all", "nell", "sull", "dall", "un",
];

/// Whether a straight quote at the end of `before` opens a quotation.
fn straight_quote_opens(before: &str) -> bool {
    let mut chars = before.chars().rev();
    let _quote = chars.next();
    match chars.next() {
        None => true,
        Some(c) => {
            is_space(c) || is_opener(c) || c == '-' || c == '"' || c == '\'' || c == ':' || c == '='
        }
    }
}

/// Whether `before` ends with a French or Italian elision such as "l'".
fn ends_with_elision(before: &str) -> bool {
    let Some(quote) = before.chars().last() else {
        return false;
    };
    if quote != '\'' && quote != '\u{2019}' {
        return false;
    }
    let body = &before[..before.len() - quote.len_utf8()];
    let word: String = body
        .chars()
        .rev()
        .take_while(|c| c.is_alphabetic())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if word.is_empty() {
        return false;
    }
    let preceded_by_word = body[..body.len() - word.len()]
        .chars()
        .last()
        .is_some_and(is_word_char);
    !preceded_by_word && ELISIONS.contains(&word.to_lowercase().as_str())
}

/// A line holding only "-", "*", "+" or "•": the list item's text starts
/// after a space.
fn bare_list_marker(before: &str) -> bool {
    let line = before.rsplit('\n').next().unwrap_or(before);
    matches!(line.trim_start(), "-" | "*" | "+" | "\u{2022}")
}

fn needs_leading_space(before: &str, text: &str) -> bool {
    let Some(last) = before.chars().last().map(object_to_newline) else {
        return false;
    };
    let Some(first) = text.chars().next() else {
        return false;
    };
    if is_space(last) || is_space(first) {
        return false;
    }
    if is_spaceless(last) || is_spaceless(first) {
        return false;
    }
    if bare_list_marker(before) {
        return true;
    }

    // What sits before the caret.
    if is_opener(last) {
        return false;
    }
    if (last == '"' || last == '\'') && straight_quote_opens(before) {
        return false;
    }
    if ends_with_elision(before) {
        return false;
    }
    // A dash with a space in front is a separator; one without is a hyphen.
    let spaced_dash = matches!(last, '-' | '\u{2013}' | '\u{2014}')
        && before.chars().rev().nth(1).is_some_and(is_hspace);
    // The owner's rule: only after a letter, a digit or a closing mark
    // (SpeakoFlow also spaces after symbols and emoji).
    if !(last.is_alphanumeric() || is_closer(last) || is_hanger(last) || spaced_dash) {
        return false;
    }

    // What the dictation starts with.
    if is_hanger(first) {
        let mut rest = text.chars().skip(1);
        // ":-)" and ";-)" are smileys, not punctuation.
        let smiley = matches!(first, ':' | ';')
            && rest.next() == Some('-')
            && matches!(rest.next(), Some(')') | Some('('));
        if !smiley {
            return false;
        }
    }
    if first == '"' || first == '\'' {
        let next = text.chars().nth(1);
        let closes = match next {
            None => true,
            Some(c) => is_space(c) || is_hanger(c) || is_closer(c),
        };
        if closes {
            return false;
        }
        // Possessive: "'s" hangs on the previous word.
        if first == '\''
            && matches!(next, Some('s') | Some('S'))
            && !text.chars().nth(2).is_some_and(is_word_char)
        {
            return false;
        }
    }
    if first == '-' && text.chars().nth(1).is_some_and(|c| c.is_alphanumeric()) {
        return false;
    }

    // Numbers and addresses that continue across the seam.
    if first.is_ascii_digit()
        && matches!(last, '.' | ',' | ':')
        && before
            .chars()
            .rev()
            .nth(1)
            .is_some_and(|c| c.is_ascii_digit())
    {
        return false;
    }
    let last_token = before.rsplit(is_space).next().unwrap_or("");
    if (last_token.contains("://") || last_token.contains('@'))
        && !text.chars().any(is_space)
        && matches!(last, '.' | '/' | ':' | '?' | '=' | '&' | '#')
    {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fit(before: &str, text: &str) -> String {
        lead(text.to_string(), Some(before))
    }

    /// The rule table: (text before the caret, dictation, pasted).
    const TABLE: &[(&str, &str, &str)] = &[
        // After a letter, a digit or a closing mark: one space.
        ("Hello", "world", " world"),
        ("first", "Second.", " Second."),
        ("We ate.", "Then we left.", " Then we left."),
        ("Yes,", "but no", " but no"),
        ("Note;", "more", " more"),
        ("We need:", "bread", " bread"),
        ("Really!", "Yes", " Yes"),
        ("Really?", "Yes", " Yes"),
        ("Wait\u{2026}", "what", " what"),
        ("up 50%", "then", " then"),
        ("room 101", "is free", " is free"),
        ("(see above)", "and", " and"),
        ("[1]", "says", " says"),
        ("{x}", "and", " and"),
        ("He said \"hi\"", "and left", " and left"),
        ("The students'", "were there", " were there"),
        ("\u{201C}Hi\u{201D}", "she said", " she said"),
        ("caf\u{00E9}", "au lait", " au lait"),
        (
            "\u{041F}\u{0440}\u{0438}\u{0432}\u{0435}\u{0442}",
            "\u{043C}\u{0438}\u{0440}",
            " \u{043C}\u{0438}\u{0440}",
        ),
        ("\u{C548}\u{B155}.", "\u{BC18}\u{AC11}", " \u{BC18}\u{AC11}"),
        ("Then", "\"Hello,\" she said.", " \"Hello,\" she said."),
        ("Then", "(maybe)", " (maybe)"),
        ("paid", "$50", " $50"),
        ("This is it -", "or not", " or not"),
        ("Notes\n-", "Buy milk", " Buy milk"),
        // Start of a field, after whitespace or a line break: none.
        ("", "Hello", "Hello"),
        ("Done. ", "Hello", "Hello"),
        ("word\t", "next", "next"),
        ("Hi team\n", "Quick update.", "Quick update."),
        ("a\u{00A0}", "b", "b"),
        ("Hello\u{FFFC}", "Next", "Next"),
        // After an opening bracket or quote: none.
        ("(", "aside", "aside"),
        ("[", "x", "x"),
        ("{", "x", "x"),
        ("He said \"", "Hello", "Hello"),
        ("\u{201C}", "Hello", "Hello"),
        ("\u{00AB}", "Bonjour", "Bonjour"),
        ("'", "quoted", "quoted"),
        ("{\"key\":\"", "value", "value"),
        // After glue and symbols: none.
        ("#", "release", "release"),
        ("self-", "aware", "aware"),
        ("see https://example.com/", "docs", "docs"),
        ("mail bob@", "example.com", "example.com"),
        ("costs $", "50", "50"),
        ("path/", "to", "to"),
        ("snake_", "case", "case"),
        ("Great job \u{1F44D}", "Thanks", "Thanks"),
        ("J'ai pris l'", "avion", "avion"),
        ("l\u{2019}", "avion", "avion"),
        // The dictation starts with punctuation or whitespace: none.
        ("It works", ", right?", ", right?"),
        ("Hello", ".", "."),
        ("Hello", "!", "!"),
        ("about 50", "%", "%"),
        ("(", ")", ")"),
        ("Bob", "'s car", "'s car"),
        ("mary", "-kate", "-kate"),
        ("word", " next", " next"),
        ("Hi,", "\nThanks.", "\nThanks."),
        ("Hello", "\u{201D}", "\u{201D}"),
        ("smile", ":-)", " :-)"),
        // Numbers that continue.
        ("version 2.", "5", "5"),
        ("at 10:", "30", "30"),
        ("1,", "000", "000"),
        // Scripts without spaces: none.
        ("\u{6211}\u{8BF4}", "\u{4F60}\u{597D}", "\u{4F60}\u{597D}"),
        (
            "Say",
            "\u{3053}\u{3093}\u{306B}\u{3061}\u{306F}",
            "\u{3053}\u{3093}\u{306B}\u{3061}\u{306F}",
        ),
        ("\u{4E2D}\u{6587}", "abc", "abc"),
        (
            "\u{0E2A}\u{0E27}\u{0E31}\u{0E2A}\u{0E14}\u{0E35}",
            "\u{0E04}\u{0E23}\u{0E31}\u{0E1A}",
            "\u{0E04}\u{0E23}\u{0E31}\u{0E1A}",
        ),
        ("\u{3002}", "Next", "Next"),
        // Nothing to paste: unchanged.
        ("word", "", ""),
        ("word", "   ", "   "),
    ];

    #[test]
    fn the_rule_table() {
        for (before, text, want) in TABLE {
            assert_eq!(&fit(before, text), want, "before {before:?}, text {text:?}");
        }
    }

    #[test]
    fn an_unreadable_field_changes_nothing() {
        assert_eq!(lead("Hello".into(), None), "Hello");
        assert_eq!(lead(", ok".into(), None), ", ok");
    }

    #[test]
    fn two_takes_in_a_row_get_exactly_one_space() {
        // Handy's trailing space after the first take: the second adds none.
        assert_eq!(fit("Done. ", "Next."), "Next.");
        // Without it, the second take brings its own.
        assert_eq!(fit("Done.", "Next."), " Next.");
    }

    #[test]
    fn a_truncated_tail_is_safe() {
        // The read may cut a surrogate pair at its start.
        assert_eq!(fit("\u{FFFD}abc", "def"), " def");
        assert_eq!(fit("\u{FFFD}", "def"), "def");
    }
}
