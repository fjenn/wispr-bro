// SPDX-License-Identifier: GPL-3.0-or-later
//! Transform definitions, recent-dictation fallback and the bounded run log.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::selection::RecentDictation;
use super::TransformKind;
use crate::fork::db::{DbError, ForkDb};

pub const KEEP_RUNS: usize = 20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Definition {
    pub id: i64,
    pub key: String,
    pub name: String,
    /// What the model is asked to do; empty when the transform is only exact
    /// edits and never reaches the model.
    pub instruction: String,
    /// Exact edits applied by code after the model (or alone) (OT2-D).
    pub edits: Vec<super::literal::Edit>,
    /// Letters the instruction asks the model to change without a clear
    /// mapping; a reply that leaves them all alone was not applied.
    pub unclear: Vec<super::literal::Target>,
    pub guard: GuardProfile,
}

impl Definition {
    pub fn uses_model(&self) -> bool {
        !self.instruction.trim().is_empty()
    }
}

/// How closely a transform's model output must resemble its input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardProfile {
    /// Polish: the cleanup guards.
    Strict,
    /// Prompt Engineer: may restructure, must keep numbers and entities.
    Flexible,
    /// Custom transforms do what the user wrote: translate, add numbering,
    /// rewrite. Only framing, assistant and output-bound checks apply.
    Instruction,
}

pub fn definition(conn: &Connection, kind: TransformKind) -> Result<Option<Definition>, DbError> {
    super::store::definition(conn, kind)
}

pub fn definition_by_id(conn: &Connection, id: i64) -> Result<Option<Definition>, DbError> {
    super::store::definition_by_id(conn, id)
}

pub fn definition_by_key(conn: &Connection, key: &str) -> Result<Option<Definition>, DbError> {
    super::store::definition_by_key(conn, key)
}

/// The newest dictation no older than 60 seconds, using the same shown text
/// as paste-last (History's raw/final variant and snippet-expanded insertion).
pub fn recent_dictation(
    conn: &Connection,
    now_ms: i64,
) -> Result<Option<RecentDictation>, DbError> {
    const QUERY: &str = "
        SELECT created_at, text FROM (
            SELECT entry_id, created_at,
                   CASE WHEN shown_variant = 'raw' AND trim(coalesce(asr_text, '')) != ''
                        THEN asr_text
                        ELSE coalesce(nullif(inserted_text, ''), final_text)
                   END AS text
              FROM entry_meta
             WHERE deleted_at IS NULL AND source = 'dictation'
        )
         WHERE trim(coalesce(text, '')) != ''
         ORDER BY created_at DESC, entry_id DESC
         LIMIT 1";
    let row = conn
        .query_row(QUERY, [], |row| {
            Ok(RecentDictation {
                created_at_ms: row.get(0)?,
                text: row.get(1)?,
            })
        })
        .optional()?;
    Ok(row.filter(|row| now_ms.saturating_sub(row.created_at_ms) <= 60_000))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewRun<'a> {
    pub transform_id: i64,
    pub app_bundle_id: Option<&'a str>,
    pub source_text: &'a str,
    pub result_text: &'a str,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkTransformViewChanges {
    pub run_id: i64,
    pub transform_id: i64,
    pub app_bundle_id: Option<String>,
    pub source_text: String,
    pub result_text: String,
    /// -1 thumbs down, 1 thumbs up, null unrated; local to `fork.db`.
    pub rating: Option<i64>,
    pub created_at_ms: i64,
}

pub fn insert(conn: &mut Connection, run: &NewRun<'_>) -> Result<i64, DbError> {
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO transform_runs
             (transform_id, app_bundle_id, source_text, result_text, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            run.transform_id,
            run.app_bundle_id,
            run.source_text,
            run.result_text,
            run.created_at_ms
        ],
    )?;
    let id = tx.last_insert_rowid();
    // 0001 already installs this bound as a trigger. Keep the explicit delete
    // as well so the store's contract is clear at its one write site.
    tx.execute(
        "DELETE FROM transform_runs
         WHERE id NOT IN (SELECT id FROM transform_runs ORDER BY id DESC LIMIT ?1)",
        [KEEP_RUNS],
    )?;
    tx.commit()?;
    Ok(id)
}

pub fn latest(conn: &Connection) -> Result<Option<ForkTransformViewChanges>, DbError> {
    conn.query_row(
        "SELECT id, transform_id, app_bundle_id, source_text, result_text, rating, created_at
         FROM transform_runs ORDER BY id DESC LIMIT 1",
        [],
        |row| {
            Ok(ForkTransformViewChanges {
                run_id: row.get(0)?,
                transform_id: row.get(1)?,
                app_bundle_id: row.get(2)?,
                source_text: row.get(3)?,
                result_text: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                rating: row.get(5)?,
                created_at_ms: row.get(6)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

pub fn get(conn: &Connection, run_id: i64) -> Result<Option<ForkTransformViewChanges>, DbError> {
    conn.query_row(
        "SELECT id, transform_id, app_bundle_id, source_text, result_text, rating, created_at
         FROM transform_runs WHERE id = ?1",
        [run_id],
        |row| {
            Ok(ForkTransformViewChanges {
                run_id: row.get(0)?,
                transform_id: row.get(1)?,
                app_bundle_id: row.get(2)?,
                source_text: row.get(3)?,
                result_text: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                rating: row.get(5)?,
                created_at_ms: row.get(6)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

pub fn rate(conn: &Connection, run_id: i64, rating: Option<i64>) -> Result<bool, DbError> {
    if rating.is_some_and(|rating| rating != -1 && rating != 1) {
        return Ok(false);
    }
    Ok(conn.execute(
        "UPDATE transform_runs SET rating = ?2 WHERE id = ?1",
        params![run_id, rating],
    )? > 0)
}

pub fn record<R: Runtime>(app: &AppHandle<R>, run: &NewRun<'_>) -> Result<i64, String> {
    let db = app
        .try_state::<ForkDb>()
        .ok_or_else(|| "fork.db is not managed".to_string())?;
    db.with_conn(|conn| insert(conn, run))
        .map_err(|e| e.to_string())
}

/// Presents a run in both S-22 views: the main window and the pill's diff card.
pub fn present<R: crate::fork::ForkRuntime>(app: &AppHandle<R>, run: ForkTransformViewChanges) {
    let id = run.run_id;
    crate::fork::pill::show_transform_diff(run.clone());
    // S-38: a parked Hub is rebuilt and gets the run once its page listens.
    let handle = app.clone();
    crate::fork::hub::park::show_main_then(app, move || {
        if let Err(e) = tauri_specta::Event::emit_to(&run, &handle, "main") {
            log::warn!("transforms: could not send run {id} to the main window: {e}");
        }
    });
}

pub fn view_changes<R: crate::fork::ForkRuntime>(app: &AppHandle<R>) {
    let Some(db) = app.try_state::<ForkDb>() else {
        return;
    };
    match db.with_conn(|conn| latest(conn)) {
        Ok(Some(run)) => present(app, run),
        Ok(None) => log::debug!("transforms: View changes with no recorded run"),
        Err(e) => log::warn!("transforms: could not read the latest run: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_seeded_and_run_history_is_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let db = ForkDb::in_dir(dir.path());
        db.with_conn(|conn| {
            for kind in [TransformKind::Polish, TransformKind::PromptEngineer] {
                let definition = definition(conn, kind)?.expect("seeded definition");
                assert!(!definition.instruction.is_empty());
                for i in 0..25 {
                    insert(
                        conn,
                        &NewRun {
                            transform_id: definition.id,
                            app_bundle_id: Some("com.example.app"),
                            source_text: "before",
                            result_text: "after",
                            created_at_ms: i,
                        },
                    )?;
                }
            }
            let (count, oldest): (i64, i64) = conn.query_row(
                "SELECT COUNT(*), MIN(created_at) FROM transform_runs",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            assert_eq!((count, oldest), (KEEP_RUNS as i64, 5));
            let latest = latest(conn)?.unwrap();
            assert_eq!(latest.created_at_ms, 24);
            assert!(rate(conn, latest.run_id, Some(-1))?);
            assert_eq!(get(conn, latest.run_id)?.unwrap().rating, Some(-1));
            assert!(!rate(conn, latest.run_id, Some(0))?);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn recent_dictation_is_limited_to_sixty_seconds() {
        let dir = tempfile::tempdir().unwrap();
        let db = ForkDb::in_dir(dir.path());
        db.with_conn(|conn| {
            crate::fork::db::upsert_entry_meta(
                conn,
                1,
                &[
                    ("created_at", &1_000_i64),
                    ("final_text", &"hello"),
                    ("source", &"dictation"),
                ],
            )?;
            assert!(recent_dictation(conn, 61_000)?.is_some());
            assert!(recent_dictation(conn, 61_001)?.is_none());
            Ok(())
        })
        .unwrap();
    }
}
