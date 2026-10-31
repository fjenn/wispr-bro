// SPDX-License-Identifier: GPL-3.0-or-later
//! The newline-safe stage wrapper (W-12, closes AF-06 / D-210).
//!
//! Handy's deterministic stages tokenize on whitespace and join with one
//! space (`collapse_stutters`, `apply_custom_words`) and collapse every
//! `\s{2,}` (`normalize_transcription_output`), so run on a whole text they
//! flatten it to one line. Run on each line separately they cannot:
//! `audio_toolkit/text.rs` stays untouched.

/// Runs `stage` on every line of `text` and joins the results with `\n`.
/// Blank lines stay empty (the stage is not called for them), so paragraph
/// breaks survive. The stage also gets the byte offset its result will have
/// in the output.
pub fn per_line_at(text: &str, mut stage: impl FnMut(&str, usize) -> String) -> String {
    let mut out = String::with_capacity(text.len());
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if !line.trim().is_empty() {
            let at = out.len();
            out.push_str(&stage(line, at));
        }
    }
    out
}

/// [`per_line_at`] for a stage that takes and returns owned text, like
/// Handy's.
pub fn per_line(text: &str, mut stage: impl FnMut(String) -> String) -> String {
    per_line_at(text, |line, _| stage(line.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Handy's whitespace behaviour in miniature: split and join with spaces.
    fn flatten(s: String) -> String {
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn a_flattening_stage_keeps_every_line() {
        assert_eq!(flatten("a\nb".into()), "a b");
        assert_eq!(per_line("a  x\nb\n\n c ", flatten), "a x\nb\n\nc");
        assert_eq!(per_line("", flatten), "");
        assert_eq!(per_line("\n", flatten), "\n");
    }

    #[test]
    fn the_stage_sees_each_line_once() {
        let mut seen = Vec::new();
        let out = per_line_at("one\n\ntwo", |l, at| {
            seen.push((l.to_string(), at));
            l.to_uppercase()
        });
        assert_eq!(out, "ONE\n\nTWO");
        assert_eq!(seen, [("one".to_string(), 0), ("two".to_string(), 5)]);
    }
}
