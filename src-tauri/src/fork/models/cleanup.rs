// SPDX-License-Identifier: GPL-3.0-or-later
//! Which speech models the fork may delete (S-07b, B-206, B-57).
//!
//! `fork.db` › `speech_models` lists the models the fork chose: the
//! starter, every policy pick and every selection it swapped away from.
//! A model is deleted only when all of this holds:
//! - a newer pick replaced it (`superseded_at`), or the policy retired it;
//! - it is not the selection;
//! - the selection has loaded at least once since it was selected
//!   (`loaded_at`): the old model stays until the new one is known to work;
//! - the fork downloaded it (`starter`/`policy`), or it is a retired
//!   selection the fork migrated away from. A model the user downloaded on
//!   the Models page is never deleted.
//!
//! [`to_delete`] is the pure rule; the rest are the table's reads and
//! writes, and `model_policy_state`'s small keys.

use rusqlite::{params, Connection, OptionalExtension};

use crate::fork::db::DbError;

/// How the fork came to manage a model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// The first model (S-07a).
    Starter,
    /// A silent upgrade or re-pick.
    Policy,
    /// A selection the fork found (Handy's, the user's, a cache hit).
    Selection,
}

impl Origin {
    pub fn as_str(self) -> &'static str {
        match self {
            Origin::Starter => "starter",
            Origin::Policy => "policy",
            Origin::Selection => "selection",
        }
    }

    fn parse(s: &str) -> Origin {
        match s {
            "starter" => Origin::Starter,
            "policy" => Origin::Policy,
            _ => Origin::Selection,
        }
    }
}

/// One `speech_models` row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub handy_id: String,
    pub origin: Origin,
    pub loaded_at: Option<i64>,
    pub superseded_at: Option<i64>,
    pub deleted_at: Option<i64>,
}

/// The models to delete now. `retired(handy_id)` says whether the policy
/// retired a model.
pub fn to_delete(rows: &[Row], selected: &str, retired: impl Fn(&str) -> bool) -> Vec<String> {
    let selected_loaded = rows
        .iter()
        .any(|r| r.handy_id == selected && r.loaded_at.is_some());
    if selected.is_empty() || !selected_loaded {
        return Vec::new();
    }
    rows.iter()
        .filter(|r| r.handy_id != selected && r.deleted_at.is_none())
        .filter(|r| {
            let gone = r.superseded_at.is_some() || retired(&r.handy_id);
            let ours = matches!(r.origin, Origin::Starter | Origin::Policy)
                || (r.origin == Origin::Selection && retired(&r.handy_id));
            gone && ours
        })
        .map(|r| r.handy_id.clone())
        .collect()
}

/// Adds a model (keeps the first origin; a re-added model is live again).
pub fn add(c: &Connection, handy_id: &str, origin: Origin, now: i64) -> Result<(), DbError> {
    c.execute(
        "INSERT INTO speech_models (handy_id, origin, added_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(handy_id) DO UPDATE SET superseded_at = NULL, deleted_at = NULL",
        params![handy_id, origin.as_str(), now],
    )?;
    Ok(())
}

/// The selection loaded once.
pub fn mark_loaded(c: &Connection, handy_id: &str, now: i64) -> Result<(), DbError> {
    c.execute(
        "UPDATE speech_models SET loaded_at = COALESCE(loaded_at, ?2) WHERE handy_id = ?1",
        params![handy_id, now],
    )?;
    Ok(())
}

/// A newer pick replaced it.
pub fn mark_superseded(c: &Connection, handy_id: &str, now: i64) -> Result<(), DbError> {
    c.execute(
        "UPDATE speech_models SET superseded_at = ?2 WHERE handy_id = ?1",
        params![handy_id, now],
    )?;
    Ok(())
}

pub fn mark_deleted(c: &Connection, handy_id: &str, now: i64) -> Result<(), DbError> {
    c.execute(
        "UPDATE speech_models SET deleted_at = ?2 WHERE handy_id = ?1",
        params![handy_id, now],
    )?;
    Ok(())
}

pub fn rows(c: &Connection) -> Result<Vec<Row>, DbError> {
    let mut st = c.prepare(
        "SELECT handy_id, origin, loaded_at, superseded_at, deleted_at
         FROM speech_models ORDER BY added_at, handy_id",
    )?;
    let rows = st
        .query_map([], |r| {
            Ok(Row {
                handy_id: r.get(0)?,
                origin: Origin::parse(&r.get::<_, String>(1)?),
                loaded_at: r.get(2)?,
                superseded_at: r.get(3)?,
                deleted_at: r.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// `model_policy_state` keys.
pub const KEY_PINNED: &str = "pinned";
pub const KEY_FORK_SELECTED: &str = "fork_selected";
pub const KEY_SLOWER_SAID: &str = "slower_said";

pub fn get_state(c: &Connection, key: &str) -> Result<Option<String>, DbError> {
    Ok(c.query_row(
        "SELECT value FROM model_policy_state WHERE key = ?1",
        params![key],
        |r| r.get(0),
    )
    .optional()?)
}

pub fn set_state(c: &Connection, key: &str, value: Option<&str>) -> Result<(), DbError> {
    match value {
        Some(v) => c.execute(
            "INSERT INTO model_policy_state (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, v],
        )?,
        None => c.execute(
            "DELETE FROM model_policy_state WHERE key = ?1",
            params![key],
        )?,
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, origin: Origin) -> Row {
        Row {
            handy_id: id.into(),
            origin,
            loaded_at: None,
            superseded_at: None,
            deleted_at: None,
        }
    }

    fn loaded(mut r: Row) -> Row {
        r.loaded_at = Some(1);
        r
    }

    fn superseded(mut r: Row) -> Row {
        r.superseded_at = Some(2);
        r
    }

    const NONE: fn(&str) -> bool = |_| false;

    #[test]
    fn the_old_model_goes_only_after_the_new_one_loaded() {
        let old = superseded(loaded(row("starter", Origin::Starter)));
        let new = row("main", Origin::Policy);
        assert!(to_delete(&[old.clone(), new.clone()], "main", NONE).is_empty());
        let new = loaded(new);
        assert_eq!(to_delete(&[old, new], "main", NONE), vec!["starter"]);
    }

    #[test]
    fn the_selection_is_never_deleted() {
        let a = superseded(loaded(row("a", Origin::Policy)));
        assert!(to_delete(std::slice::from_ref(&a), "a", NONE).is_empty());
        assert!(to_delete(&[a], "", NONE).is_empty());
    }

    #[test]
    fn a_users_model_is_never_deleted() {
        let users = superseded(loaded(row("mine", Origin::Selection)));
        let main = loaded(row("main", Origin::Policy));
        assert!(to_delete(&[users, main], "main", NONE).is_empty());
    }

    #[test]
    fn a_retired_selection_migrates_and_is_deleted() {
        let retired = loaded(row("old-large", Origin::Selection));
        let main = loaded(row("main", Origin::Policy));
        let is_retired = |id: &str| id == "old-large";
        assert_eq!(
            to_delete(&[retired.clone(), main.clone()], "main", is_retired),
            vec!["old-large"]
        );
        // Still selected: kept.
        assert!(to_delete(&[retired, main], "old-large", is_retired).is_empty());
    }

    #[test]
    fn deleted_and_live_models_stay_out() {
        let mut gone = superseded(loaded(row("gone", Origin::Starter)));
        gone.deleted_at = Some(3);
        let live = loaded(row("other", Origin::Policy));
        let main = loaded(row("main", Origin::Policy));
        assert!(to_delete(&[gone, live, main], "main", NONE).is_empty());
    }

    #[test]
    fn rows_and_state_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::fork::db::ForkDb::in_dir(dir.path());
        db.with_conn(|c| {
            add(c, "s", Origin::Starter, 1)?;
            add(c, "m", Origin::Policy, 2)?;
            mark_loaded(c, "s", 3)?;
            mark_loaded(c, "s", 9)?; // first load wins
            mark_superseded(c, "s", 4)?;
            mark_loaded(c, "m", 5)?;
            let r = rows(c)?;
            assert_eq!(r[0].loaded_at, Some(3));
            assert_eq!(to_delete(&r, "m", |_| false), vec!["s"]);
            mark_deleted(c, "s", 6)?;
            assert!(to_delete(&rows(c)?, "m", |_| false).is_empty());
            // Picked again later: live again, origin kept.
            add(c, "s", Origin::Policy, 7)?;
            let s = rows(c)?.into_iter().find(|r| r.handy_id == "s").unwrap();
            assert_eq!(s.origin, Origin::Starter);
            assert_eq!((s.superseded_at, s.deleted_at), (None, None));

            assert_eq!(get_state(c, KEY_PINNED)?, None);
            set_state(c, KEY_PINNED, Some("x"))?;
            set_state(c, KEY_PINNED, Some("y"))?;
            assert_eq!(get_state(c, KEY_PINNED)?.as_deref(), Some("y"));
            set_state(c, KEY_PINNED, None)?;
            assert_eq!(get_state(c, KEY_PINNED)?, None);
            Ok(())
        })
        .unwrap();
    }
}
