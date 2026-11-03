// SPDX-License-Identifier: GPL-3.0-or-later
//! Fork app commands.
//!
//! Every command here is named `fork_*`: `fork::compose_invoke` routes by that
//! prefix. Commands are plain app commands (not plugin commands), so they need
//! no ACL permission from local pages (`docs/fork/ARCHITECTURE.md`, "Routing").
//!
//! Rule: a fork command reads Handy state with `try_state` and returns
//! [`ForkError::NotReady`] when that state does not exist yet. It never panics.

pub mod audio;
pub mod csp;
pub mod dictionary;
pub mod history;
pub mod hotkeys;
pub mod hub;
pub mod insights;
pub mod llm;
pub mod models;
pub mod notifications;
pub mod onboarding;
pub mod pill;
pub mod privacy;
pub mod scratchpad;
pub mod selfcheck;
pub mod settings;
pub mod snippets;
pub mod stats;
pub mod style;
pub mod transforms;
pub mod update;

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};
use tauri_specta::Event;

use super::events::ForkPingEvent;
use super::{ForkState, HandyModelManager};

/// Typed error returned by fork commands.
///
/// Serialized as `{ "kind": "NotReady" }`,
/// `{ "kind": "InvalidSetting", "message": "…" }` or
/// `{ "kind": "Internal", "message": "…" }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", content = "message")]
pub enum ForkError {
    /// Handy has not finished its setup (`RunEvent::Ready` has not run yet),
    /// or the app runs in a headless CLI mode where the fork stays inactive.
    NotReady,
    /// A `fork_set_setting` value failed validation; nothing was changed.
    InvalidSetting(String),
    /// Unexpected failure; the message is for logs and the debug panel only.
    Internal(String),
}

impl std::fmt::Display for ForkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForkError::NotReady => write!(f, "fork not ready"),
            ForkError::InvalidSetting(message) => write!(f, "invalid setting: {message}"),
            ForkError::Internal(message) => write!(f, "fork internal error: {message}"),
        }
    }
}

impl std::error::Error for ForkError {}

/// Reply of [`fork_ping`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ForkPingReply {
    /// Always `"pong"`.
    pub message: String,
    /// Sequence number, also carried by the `ForkPingEvent` emitted afterwards.
    pub seq: u32,
    /// `true` once `fork::on_ready` has run.
    pub ready: bool,
    /// `true` in debug builds (`cfg(debug_assertions)`).
    pub debug_build: bool,
    /// `true` when the app was launched with `--debug`.
    pub cli_debug: bool,
}

/// Snapshot of Handy's model registry, as seen by the fork.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ForkModelStatus {
    /// Number of models Handy knows about (catalog, on disk and custom).
    pub known_models: u32,
    /// Ids of the models that are downloaded, in Handy's order.
    pub downloaded_model_ids: Vec<String>,
}

/// Read-only view of Handy's model manager. Implemented for Handy's
/// `ModelManager`; tests implement it for a fake (`fork::testutil`).
pub(crate) trait HandyModels: Send + Sync + 'static {
    fn model_status(&self) -> ForkModelStatus;
}

impl HandyModels for crate::managers::model::ModelManager {
    fn model_status(&self) -> ForkModelStatus {
        let models = self.get_available_models();
        ForkModelStatus {
            known_models: u32::try_from(models.len()).unwrap_or(u32::MAX),
            downloaded_model_ids: models
                .into_iter()
                .filter(|m| m.is_downloaded)
                .map(|m| m.id)
                .collect(),
        }
    }
}

/// Health check for the fork command path. Emits `ForkPingEvent` after
/// building the reply.
#[tauri::command]
#[specta::specta]
pub fn fork_ping<R: Runtime>(app: AppHandle<R>) -> Result<ForkPingReply, ForkError> {
    let state = app
        .try_state::<ForkState>()
        .ok_or_else(|| ForkError::Internal("ForkState is not managed".into()))?;
    let seq = state.next_ping_seq();
    let cli_debug = app
        .try_state::<crate::CliArgs>()
        .map(|args| args.debug)
        .unwrap_or(false);
    let reply = ForkPingReply {
        message: "pong".into(),
        seq,
        ready: state.is_ready(),
        debug_build: cfg!(debug_assertions),
        cli_debug,
    };
    if let Err(e) = (ForkPingEvent { seq }).emit(&app) {
        log::warn!("fork: failed to emit ForkPingEvent: {e}");
    }
    Ok(reply)
}

/// Handy's model state. Returns [`ForkError::NotReady`] before
/// `fork::on_ready` has run, or while Handy's `ModelManager` is not managed.
#[tauri::command]
#[specta::specta]
pub fn fork_model_status<R: Runtime>(app: AppHandle<R>) -> Result<ForkModelStatus, ForkError> {
    let ready = app
        .try_state::<ForkState>()
        .map(|s| s.is_ready())
        .unwrap_or(false);
    if !ready {
        return Err(ForkError::NotReady);
    }
    let models = app
        .try_state::<Arc<HandyModelManager>>()
        .ok_or(ForkError::NotReady)?;
    Ok(models.model_status())
}
