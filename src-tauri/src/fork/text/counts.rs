// SPDX-License-Identifier: GPL-3.0-or-later
//! What the pipeline did to one take: the stages in the order they ran with
//! their change counts, and the dictionary rules that applied (S-11 exit
//! criterion; stored per take in `entry_meta.text_stages`,
//! `fix_count_dictionary` and `corrections`).

use serde::Serialize;

use super::rules::Provenance;

/// A pipeline stage (R5 §4.10 numbering in the docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// 3: exact dictionary rules.
    Rules,
    /// 2, 4, 5: Handy's script conversion, custom words, fillers and
    /// stutters, run per line.
    Handy,
    /// 7: line breaks, spoken punctuation, literal escapes.
    Commands,
    /// 7: "scratch that".
    Scratch,
    /// 9: rule outputs the LLM undid, put back.
    RulesRestored,
    /// 12: line-safe whitespace.
    Whitespace,
}

impl Stage {
    pub fn as_str(self) -> &'static str {
        match self {
            Stage::Rules => "rules",
            Stage::Handy => "handy",
            Stage::Commands => "commands",
            Stage::Scratch => "scratch",
            Stage::RulesRestored => "rules_restored",
            Stage::Whitespace => "whitespace",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct StageCount {
    pub stage: Stage,
    pub changes: u32,
}

/// One dictionary rule application (a `corrections` row, source `rule`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleHit {
    pub rule_id: i64,
    /// The words as the recognizer wrote them.
    pub from_text: String,
    pub to_text: String,
}

/// Everything the pipeline reports about one take.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextRecord {
    pub stages: Vec<StageCount>,
    pub rule_hits: Vec<RuleHit>,
    /// For stage 9 after an LLM rewrite.
    pub provenance: Vec<Provenance>,
}

impl TextRecord {
    pub fn push(&mut self, stage: Stage, changes: u32) {
        self.stages.push(StageCount { stage, changes });
    }

    /// `entry_meta.fix_count_dictionary`.
    pub fn dictionary_fixes(&self) -> i64 {
        i64::try_from(self.rule_hits.len()).unwrap_or(i64::MAX)
    }

    /// `entry_meta.text_stages`: `[{"stage":"rules","changes":1},…]`.
    pub fn stages_json(&self) -> String {
        serde_json::to_string(&self.stages).unwrap_or_else(|_| "[]".into())
    }

    /// One log line: the stage order and counts, no text.
    pub fn describe(&self) -> String {
        self.stages
            .iter()
            .map(|s| format!("{}={}", s.stage.as_str(), s.changes))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Longest texts compared word by word; longer ones count 1 if they differ.
const MAX_DIFF_TOKENS: usize = 2_000;

/// How many words a stage removed or added (a word-level edit count by the
/// longest common subsequence). Used for stages that report no count of
/// their own (Handy's).
pub fn changed_words(before: &str, after: &str) -> u32 {
    if before == after {
        return 0;
    }
    let a: Vec<&str> = before.split_whitespace().collect();
    let b: Vec<&str> = after.split_whitespace().collect();
    if a.len() > MAX_DIFF_TOKENS || b.len() > MAX_DIFF_TOKENS {
        return 1;
    }
    // One row of the LCS table at a time.
    let mut prev = vec![0usize; b.len() + 1];
    let mut row = vec![0usize; b.len() + 1];
    for x in &a {
        for (j, y) in b.iter().enumerate() {
            row[j + 1] = if x == y {
                prev[j] + 1
            } else {
                row[j].max(prev[j + 1])
            };
        }
        std::mem::swap(&mut prev, &mut row);
    }
    let lcs = prev[b.len()];
    u32::try_from(a.len() + b.len() - 2 * lcs).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_edits() {
        assert_eq!(changed_words("a b c", "a b c"), 0);
        // Whitespace alone is no word change.
        assert_eq!(changed_words("a  b", "a b"), 0);
        assert_eq!(changed_words("um so hello", "so hello"), 1);
        assert_eq!(changed_words("I I I think", "I think"), 2);
        assert_eq!(changed_words("color", "colour"), 2);
    }

    #[test]
    fn record_json_and_log_line() {
        let mut r = TextRecord::default();
        r.push(Stage::Rules, 2);
        r.push(Stage::Handy, 1);
        assert_eq!(
            r.stages_json(),
            r#"[{"stage":"rules","changes":2},{"stage":"handy","changes":1}]"#
        );
        assert_eq!(r.describe(), "rules=2 handy=1");
        assert_eq!(r.dictionary_fixes(), 0);
    }
}
