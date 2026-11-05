// SPDX-License-Identifier: GPL-3.0-or-later
//! Private Use Area placeholders that protect dictionary-rule outputs from
//! Handy's later stages (fuzzy custom words, filler removal, stutter
//! collapse; R5 §4.10 stage 3).
//!
//! Each protected value becomes one character U+E000 + n. Handy's stages
//! leave such a character alone: it is not alphanumeric, so the custom-word
//! matcher builds no key from it, the filler patterns' `\b` never matches
//! next to it, and stutter collapse only touches alphabetic words. One
//! case can still move it: a custom-word n-gram that ends on the placeholder
//! is rewritten with the placeholder glued to the replacement. [`restore`]
//! therefore checks that every placeholder survived once, with the same
//! kind of neighbour (space or not) on each side, and reports failure so the
//! pipeline can fall back to unprotected rules.

/// First placeholder character.
const BASE: u32 = 0xE000;
/// Placeholders available (U+E000..U+F8FF, the BMP Private Use Area).
pub const CAPACITY: usize = 0x1900;

/// Whether `c` is in the placeholder range.
pub fn is_placeholder(c: char) -> bool {
    (BASE..BASE + CAPACITY as u32).contains(&(c as u32))
}

/// Whether `text` already holds characters from the placeholder range (then
/// nothing in it can be protected safely).
pub fn collides(text: &str) -> bool {
    text.chars().any(is_placeholder)
}

/// Protected values of one line.
#[derive(Debug, Default)]
pub struct Placeholders {
    values: Vec<String>,
}

/// Where a restored value ended up in the restored text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Restored {
    pub index: usize,
    pub start: usize,
}

impl Placeholders {
    pub fn new() -> Placeholders {
        Placeholders::default()
    }

    /// A placeholder for `value`, or `None` when the range is used up.
    pub fn protect(&mut self, value: &str) -> Option<char> {
        if self.values.len() >= CAPACITY {
            return None;
        }
        let c = char::from_u32(BASE + self.values.len() as u32)?;
        self.values.push(value.to_string());
        Some(c)
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Replaces every placeholder in `processed` by its value. `protected`
    /// is the text the placeholders were put into, before Handy's stages.
    /// `None` when a placeholder went missing, appears twice, or lost or
    /// gained a space next to it.
    pub fn restore(&self, protected: &str, processed: &str) -> Option<(String, Vec<Restored>)> {
        let shape = |text: &str| -> Option<Vec<(usize, bool, bool)>> {
            let chars: Vec<char> = text.chars().collect();
            let mut seen = Vec::new();
            for (i, c) in chars.iter().enumerate() {
                if !is_placeholder(*c) {
                    continue;
                }
                let index = (*c as u32 - BASE) as usize;
                if index >= self.values.len() {
                    return None;
                }
                let spaced = |n: Option<&char>| n.is_none_or(|n| n.is_whitespace());
                let left = spaced(i.checked_sub(1).and_then(|j| chars.get(j)));
                let right = spaced(chars.get(i + 1));
                seen.push((index, left, right));
            }
            seen.sort_unstable();
            Some(seen)
        };
        let before = shape(protected)?;
        let after = shape(processed)?;
        if before != after || before.len() != self.values.len() {
            return None;
        }

        let mut out = String::with_capacity(processed.len());
        let mut restored = Vec::with_capacity(self.values.len());
        for c in processed.chars() {
            if is_placeholder(c) {
                let index = (c as u32 - BASE) as usize;
                restored.push(Restored {
                    index,
                    start: out.len(),
                });
                out.push_str(&self.values[index]);
            } else {
                out.push(c);
            }
        }
        Some((out, restored))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut p = Placeholders::new();
        let a = p.protect("Voicetypr").unwrap();
        let b = p.protect("O'Brien").unwrap();
        let protected = format!("ask {b} about {a}, now");
        let (out, restored) = p.restore(&protected, &protected).unwrap();
        assert_eq!(out, "ask O'Brien about Voicetypr, now");
        assert_eq!(restored.len(), 2);
        assert_eq!(&out[restored[0].start..], "O'Brien about Voicetypr, now");
    }

    #[test]
    fn a_lost_duplicated_or_glued_placeholder_fails() {
        let mut p = Placeholders::new();
        let a = p.protect("X").unwrap();
        let protected = format!("the {a} rules");
        assert!(p.restore(&protected, "the rules").is_none());
        assert!(p
            .restore(&protected, &format!("the {a} {a} rules"))
            .is_none());
        // A custom word swallowed "the" and glued the placeholder to itself.
        assert!(p.restore(&protected, &format!("Thee{a} rules")).is_none());
        // Spacing elsewhere may change (Handy collapses runs of spaces).
        assert!(p
            .restore(&format!("the  {a}  rules"), &format!("the {a} rules"))
            .is_some());
    }

    #[test]
    fn collisions_are_detected() {
        assert!(collides("a\u{E000}b"));
        assert!(!collides("plain"));
    }
}
