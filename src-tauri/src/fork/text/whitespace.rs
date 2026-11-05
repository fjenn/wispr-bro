// SPDX-License-Identifier: GPL-3.0-or-later
//! Stage 12 of the text pipeline (R5 §4.10): line-safe whitespace.
//!
//! Inside each line, runs of spaces and tabs become one space and the line
//! is trimmed of them; every `\n` stays, blank lines included (they are paragraph
//! breaks the user asked for). Handy's own normalization collapses `\s{2,}`
//! across lines, which is why it runs per line ([`super::lines`]).

/// Normalizes `text`; returns it with the number of lines that changed.
pub fn normalize(text: &str) -> (String, u32) {
    let mut out = String::with_capacity(text.len());
    let mut changed = 0u32;
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let start = out.len();
        let mut pending_space = false;
        for c in line.chars() {
            // Only plain spaces and tabs: no-break and ideographic spaces
            // are typography the user (or the language) wants.
            if c == ' ' || c == '\t' {
                pending_space = out.len() > start;
            } else {
                if pending_space {
                    out.push(' ');
                    pending_space = false;
                }
                out.push(c);
            }
        }
        if out[start..] != *line {
            changed += 1;
        }
    }
    (out, changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_inside_lines_and_keeps_every_newline() {
        assert_eq!(normalize("  a   b \t c  \n\n  d  ").0, "a b c\n\nd");
        assert_eq!(normalize("a\n\n\n\nb").0, "a\n\n\n\nb");
        assert_eq!(normalize("\nx\n").0, "\nx\n");
    }

    #[test]
    fn counts_changed_lines_and_keeps_no_break_spaces() {
        assert_eq!(normalize("ok\nfine").1, 0);
        assert_eq!(normalize("ok \n fine").1, 2);
        // French typography: the narrow no-break space before "?" stays.
        assert_eq!(normalize("Ça va\u{202F}?").0, "Ça va\u{202F}?");
    }
}
