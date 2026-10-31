// SPDX-License-Identifier: GPL-3.0-or-later
//! The text pipeline's side of `fork.db` (S-11): the active dictionary rules
//! (`dictionary_entries`, kind `rule`, status `active`; S-16 manages them)
//! and the record of the ones that applied to a take (`corrections` rows
//! with source `rule`, the rules' `hit_count` / `last_hit_at`).

use rusqlite::{params, Connection};

use super::DbError;
use crate::fork::text::counts::RuleHit;
use crate::fork::text::rules::Rule;

/// Every active rule, oldest first.
pub fn active_rules(conn: &Connection) -> Result<Vec<Rule>, DbError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, from_text, to_text FROM dictionary_entries
         WHERE kind = 'rule' AND status = 'active'
         ORDER BY id",
    )?;
    let rules = stmt
        .query_map([], |r| {
            Ok(Rule {
                id: r.get(0)?,
                from: r.get(1)?,
                to: r.get(2)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rules)
}

/// Replaces the `corrections` rows of source `rule` for `entry_id` with
/// `hits` and counts each hit on its rule. A retry calls it again with the
/// new hits.
pub fn record_hits(
    conn: &Connection,
    entry_id: i64,
    hits: &[RuleHit],
    now_ms: i64,
) -> Result<(), DbError> {
    conn.execute(
        "DELETE FROM corrections WHERE entry_id = ?1 AND source = 'rule'",
        [entry_id],
    )?;
    let mut insert = conn.prepare_cached(
        "INSERT INTO corrections (entry_id, from_text, to_text, source, created_at)
         VALUES (?1, ?2, ?3, 'rule', ?4)",
    )?;
    let mut bump = conn.prepare_cached(
        "UPDATE dictionary_entries SET hit_count = hit_count + 1, last_hit_at = ?2
         WHERE id = ?1",
    )?;
    for hit in hits {
        insert.execute(params![entry_id, hit.from_text, hit.to_text, now_ms])?;
        bump.execute(params![hit.rule_id, now_ms])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::db::ForkDb;

    fn db() -> (tempfile::TempDir, ForkDb) {
        let dir = tempfile::tempdir().unwrap();
        let db = ForkDb::in_dir(dir.path());
        (dir, db)
    }

    #[test]
    fn active_rules_are_rules_that_are_active() {
        let (_d, db) = db();
        db.with_conn(|c| {
            c.execute_batch(
                "INSERT INTO dictionary_entries (kind, term, created_at) VALUES ('word', 'Kubernetes', 1);
                 INSERT INTO dictionary_entries (kind, from_text, to_text, created_at) VALUES ('rule', 'jira', 'Jira', 2);
                 INSERT INTO dictionary_entries (kind, from_text, to_text, status, created_at) VALUES ('rule', 'a', 'b', 'suggested', 3);
                 INSERT INTO dictionary_entries (kind, from_text, to_text, created_at) VALUES ('rule', 'gh', 'GitHub', 4);",
            )?;
            let rules = active_rules(c)?;
            assert_eq!(
                rules.iter().map(|r| (r.from.as_str(), r.to.as_str())).collect::<Vec<_>>(),
                [("jira", "Jira"), ("gh", "GitHub")]
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn hits_are_recorded_once_per_take_and_counted_per_rule() {
        let (_d, db) = db();
        db.with_conn(|c| {
            c.execute(
                "INSERT INTO dictionary_entries (kind, from_text, to_text, created_at) VALUES ('rule', 'jira', 'Jira', 1)",
                [],
            )?;
            let id = c.last_insert_rowid();
            let hit = RuleHit {
                rule_id: id,
                from_text: "JIRA".into(),
                to_text: "Jira".into(),
            };
            record_hits(c, 7, &[hit.clone(), hit.clone()], 100)?;
            // A retry replaces the take's rows.
            record_hits(c, 7, &[hit], 200)?;
            let rows: i64 = c.query_row(
                "SELECT COUNT(*) FROM corrections WHERE entry_id = 7 AND source = 'rule'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(rows, 1);
            let (hits, last): (i64, i64) = c.query_row(
                "SELECT hit_count, last_hit_at FROM dictionary_entries WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            assert_eq!((hits, last), (3, 200));
            Ok(())
        })
        .unwrap();
    }
}
