// SPDX-License-Identifier: GPL-3.0-or-later
//! Deadline-bounded word diff for Transform changes (S-22).
//!
//! Uses the same longest-common-subsequence approach as `text::counts`, but
//! preserves every source byte in the emitted operations. Oversized or slow
//! comparisons degrade to one replacement instead of delaying the pill.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;

pub const MAX_TOKENS: usize = 2_000;
pub const DIFF_BUDGET: Duration = Duration::from_millis(25);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum DiffOpKind {
    Equal,
    Delete,
    Insert,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct DiffOp {
    pub kind: DiffOpKind,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct WordDiff {
    pub ops: Vec<DiffOp>,
    /// Maximal removed/added regions between unchanged words.
    pub changes: u32,
    /// The full comparison exceeded its token or time budget.
    pub degraded: bool,
}

fn push(ops: &mut Vec<DiffOp>, kind: DiffOpKind, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = ops.last_mut().filter(|last| last.kind == kind) {
        last.text.push_str(text);
    } else {
        ops.push(DiffOp {
            kind,
            text: text.to_owned(),
        });
    }
}

fn replacement(before: &str, after: &str) -> WordDiff {
    let mut ops = Vec::new();
    push(&mut ops, DiffOpKind::Delete, before);
    push(&mut ops, DiffOpKind::Insert, after);
    WordDiff {
        ops,
        changes: u32::from(before != after),
        degraded: true,
    }
}

/// Word/separator tokens with exact byte spans. Letters, marks and digits stay
/// together; whitespace and punctuation remain in the output operations.
fn tokens(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut word: Option<bool> = None;
    for (at, c) in text.char_indices() {
        let is_word = c.is_alphanumeric()
            || matches!(c, '_' | '\u{200c}' | '\u{200d}')
            || !c.is_ascii() && !c.is_whitespace() && !c.is_ascii_punctuation() && !c.is_control();
        if word.is_some_and(|was| was != is_word) {
            out.push(&text[start..at]);
            start = at;
        }
        word = Some(is_word);
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// A word-level diff completed within `budget`. Equal source text always
/// produces one Equal operation; changed text always reconstructs both inputs.
pub fn within(before: &str, after: &str, budget: Duration) -> WordDiff {
    if before == after {
        return WordDiff {
            ops: vec![DiffOp {
                kind: DiffOpKind::Equal,
                text: before.to_owned(),
            }],
            changes: 0,
            degraded: false,
        };
    }
    if before.is_empty() || after.is_empty() {
        let mut result = replacement(before, after);
        result.degraded = false;
        return result;
    }
    let started = Instant::now();
    let a = tokens(before);
    let b = tokens(after);
    if a.len() > MAX_TOKENS || b.len() > MAX_TOKENS {
        return replacement(before, after);
    }

    let (n, m) = (a.len(), b.len());
    let mut table = vec![vec![0u16; m + 1]; n + 1];
    for i in (0..n).rev() {
        if i % 16 == 0 && started.elapsed() >= budget {
            return replacement(before, after);
        }
        for j in (0..m).rev() {
            table[i][j] = if a[i] == b[j] {
                table[i + 1][j + 1].saturating_add(1)
            } else {
                table[i + 1][j].max(table[i][j + 1])
            };
        }
    }

    let mut ops = Vec::new();
    let mut changes = 0u32;
    let mut changed = false;
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            push(&mut ops, DiffOpKind::Equal, a[i]);
            i += 1;
            j += 1;
            changed = false;
            continue;
        }
        if !changed {
            changes = changes.saturating_add(1);
            changed = true;
        }
        if j >= m || (i < n && table[i + 1][j] >= table[i][j + 1]) {
            push(&mut ops, DiffOpKind::Delete, a[i]);
            i += 1;
        } else {
            push(&mut ops, DiffOpKind::Insert, b[j]);
            j += 1;
        }
    }
    WordDiff {
        ops,
        changes,
        degraded: false,
    }
}

pub fn diff(before: &str, after: &str) -> WordDiff {
    within(before, after, DIFF_BUDGET)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(ops: &[DiffOp], kind: DiffOpKind) -> String {
        ops.iter()
            .filter(|op| op.kind == kind)
            .map(|op| op.text.as_str())
            .collect()
    }

    #[test]
    fn diff_ops_snapshot() {
        let result = diff(
            "um we could perhaps meet on Tuesday",
            "We could meet on Tuesday.",
        );
        let ops = result
            .ops
            .iter()
            .map(|op| (op.kind, op.text.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            ops,
            [
                (DiffOpKind::Delete, "um we"),
                (DiffOpKind::Insert, "We"),
                (DiffOpKind::Equal, " could "),
                (DiffOpKind::Delete, "perhaps "),
                (DiffOpKind::Equal, "meet on Tuesday"),
                (DiffOpKind::Insert, "."),
            ]
        );
        assert_eq!(result.changes, 3);
        assert!(!result.degraded);
    }

    #[test]
    fn operations_preserve_both_texts_and_count_changed_regions() {
        for (before, after, changes) in [
            ("same text", "same text", 0),
            ("a b c", "a new b c", 1),
            ("one two three", "one three", 1),
            ("hello\nworld", "hello\n\nworld", 1),
            ("", "added", 1),
        ] {
            let result = diff(before, after);
            let before_ops = result
                .ops
                .iter()
                .filter(|op| op.kind != DiffOpKind::Insert)
                .map(|op| op.text.as_str())
                .collect::<String>();
            let after_ops = result
                .ops
                .iter()
                .filter(|op| op.kind != DiffOpKind::Delete)
                .map(|op| op.text.as_str())
                .collect::<String>();
            assert_eq!(before_ops, before);
            assert_eq!(after_ops, after);
            assert_eq!(result.changes, changes);
        }
    }

    #[test]
    fn deadline_and_size_degrade_to_one_complete_replacement() {
        let before = "word ".repeat(2_001);
        let result = diff(&before, "different");
        assert!(result.degraded);
        assert_eq!(result.changes, 1);
        assert_eq!(render(&result.ops, DiffOpKind::Delete), before);
        assert_eq!(render(&result.ops, DiffOpKind::Insert), "different");
        let result = within("a b c", "a x c", Duration::ZERO);
        assert!(result.degraded);
    }
}
