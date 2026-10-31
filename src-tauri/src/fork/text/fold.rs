// SPDX-License-Identifier: GPL-3.0-or-later
//! Case-folded copies of text with a map back to the original bytes, so the
//! dictionary rules and the voice commands can match case-insensitively
//! with `aho-corasick` (which folds ASCII only) and still cut the original
//! text at the right place.
//!
//! Folding is per character: Unicode lowercase, plus typographic apostrophes
//! (’ ‘) read as `'`, because recognizers write either. A character whose
//! lowercase is longer (`İ` → `i̇`) maps every folded byte back to its own
//! start, and a match that would cut such a group is rejected.

/// `text` folded, with the original byte offset of every folded byte.
#[derive(Clone, Debug)]
pub struct Folded {
    pub text: String,
    /// `map[i]`: byte offset in the original of the character folded byte
    /// `i` came from; `map[text.len()]` is the original's length.
    map: Vec<usize>,
}

/// One character, folded.
pub fn fold_char(c: char, out: &mut String) {
    match c {
        '\u{2019}' | '\u{2018}' | '\u{02BC}' => out.push('\''),
        c if c.is_ascii() => out.push(c.to_ascii_lowercase()),
        c => out.extend(c.to_lowercase()),
    }
}

/// The folded form of a pattern (no map needed).
pub fn fold_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        fold_char(c, &mut out);
    }
    out
}

impl Folded {
    pub fn new(original: &str) -> Folded {
        Folded::with(original, fold_char)
    }

    /// `original` folded per character by `fold` (S-17's snippets fold the
    /// Turkish dotted and dotless i as well).
    pub fn with(original: &str, fold: impl Fn(char, &mut String)) -> Folded {
        let mut text = String::with_capacity(original.len());
        let mut map = Vec::with_capacity(original.len() + 1);
        for (at, c) in original.char_indices() {
            let before = text.len();
            fold(c, &mut text);
            map.extend(std::iter::repeat_n(at, text.len() - before));
        }
        map.push(original.len());
        Folded { text, map }
    }

    /// Whether folded byte `i` starts a character group (or is the end).
    fn starts_group(&self, i: usize) -> bool {
        i == 0 || i == self.text.len() || self.map[i] != self.map[i - 1]
    }

    /// The original byte span of folded span `start..end`, if it does not
    /// cut a folded character group.
    pub fn original_span(&self, start: usize, end: usize) -> Option<(usize, usize)> {
        (self.starts_group(start) && self.starts_group(end))
            .then(|| (self.map[start], self.map[end]))
    }
}

/// Characters of scripts written without spaces between words (Han, Kana,
/// Thai, Lao, Khmer, Myanmar). Word boundaries cannot be required next to
/// them.
pub fn is_spaceless(c: char) -> bool {
    matches!(c as u32,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x323AF
        | 0x3005..=0x3007 | 0x3040..=0x309F | 0x30A0..=0x30FF | 0x31F0..=0x31FF
        | 0xFF66..=0xFF9F
        | 0x0E00..=0x0EFF | 0x1000..=0x109F | 0xA9E0..=0xA9FF | 0xAA60..=0xAA7F
        | 0x1780..=0x17FF | 0x19E0..=0x19FF)
}

/// A letter, digit or `_`: what a whole-word match must not touch.
pub fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whether `text[start..end]` stands as whole words: the characters on
/// either side are not word characters. Next to a spaceless script (on
/// either side of the edge) no boundary is required, since those scripts
/// have none.
pub fn whole_words(text: &str, start: usize, end: usize) -> bool {
    let inner_first = text[start..end].chars().next();
    let inner_last = text[start..end].chars().next_back();
    let left = text[..start].chars().next_back();
    let right = text[end..].chars().next();
    let edge_ok = |outer: Option<char>, inner: Option<char>| match (outer, inner) {
        (None, _) => true,
        (Some(o), Some(i)) if is_spaceless(o) || is_spaceless(i) => true,
        (Some(o), _) => !is_word_char(o),
    };
    edge_ok(left, inner_first) && edge_ok(right, inner_last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_case_and_apostrophes_and_maps_back() {
        let f = Folded::new("Don’t ÄRGERN");
        assert_eq!(f.text, "don't ärgern");
        let at = f.text.find("ärgern").unwrap();
        let (s, e) = f.original_span(at, at + "ärgern".len()).unwrap();
        assert_eq!(&"Don’t ÄRGERN"[s..e], "ÄRGERN");
    }

    #[test]
    fn a_match_cannot_cut_a_longer_lowercase() {
        // 'İ' lowercases to "i\u{307}" (two chars, three bytes).
        let f = Folded::new("İx");
        assert_eq!(f.text, "i\u{307}x");
        assert!(f.original_span(0, 1).is_none());
        assert_eq!(f.original_span(0, 3), Some((0, 2)));
    }

    #[test]
    fn whole_words_and_spaceless_scripts() {
        assert!(whole_words("a foo b", 2, 5));
        assert!(!whole_words("afoo b", 1, 4));
        assert!(!whole_words("foo_bar", 0, 3));
        assert!(whole_words("(foo),", 1, 4));
        // Han next to Han: no boundary exists, so none is required.
        let t = "我用东京站";
        let s = t.find("东京").unwrap();
        assert!(whole_words(t, s, s + "东京".len()));
    }
}
