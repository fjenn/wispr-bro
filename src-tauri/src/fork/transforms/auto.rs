// SPDX-License-Identifier: GPL-3.0-or-later
//! Optional transform after dictation cleanup (S-22), fail-open and throttled.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Runtime};

use super::record::{self, Definition, NewRun};
use super::run::{self, GenerateError};
use crate::fork::llm::chat::CancellationToken;
use crate::fork::llm::post_process::Engine;

pub const MIN_INTERVAL: Duration = Duration::from_secs(5);
static LAST_RUN: Mutex<Option<Instant>> = Mutex::new(None);

/// Runs closer than five seconds apart skip auto-apply. The first run and one
/// exactly five seconds later are allowed.
pub fn admits(last: Option<Instant>, now: Instant) -> bool {
    last.is_none_or(|last| now.saturating_duration_since(last) >= MIN_INTERVAL)
}

fn claim(now: Instant) -> bool {
    let mut last = LAST_RUN.lock().unwrap_or_else(|e| e.into_inner());
    if !admits(*last, now) {
        return false;
    }
    *last = Some(now);
    true
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutoApplied {
    pub text: String,
    pub applied: bool,
    pub skipped: bool,
}

pub async fn with_engine<E: Engine>(
    engine: Option<&E>,
    definition: &Definition,
    source: &str,
) -> AutoApplied {
    match run::generate_definition(engine, definition, source, &CancellationToken::new()).await {
        Ok(generated) => AutoApplied {
            text: generated.text,
            applied: true,
            skipped: false,
        },
        Err(error) => {
            if !matches!(error, GenerateError::Cancelled | GenerateError::Unchanged) {
                log::info!("transforms: auto-apply failed open: {error:?}");
            }
            AutoApplied {
                text: source.to_owned(),
                applied: false,
                skipped: false,
            }
        }
    }
}

/// Applies the chosen definition after cleanup. Any engine/guard/deadline
/// failure returns the cleanup output unchanged, which the normal paste path
/// then inserts. Successful changed runs join the same 20-run diff history.
pub async fn apply<R: Runtime>(
    app: &AppHandle<R>,
    definition: &Definition,
    source: &str,
) -> AutoApplied {
    if !claim(Instant::now()) {
        log::info!(
            "transforms: auto-apply {} skipped (throttled)",
            definition.id
        );
        return AutoApplied {
            text: source.to_owned(),
            applied: false,
            skipped: true,
        };
    }
    // Exact edits need no engine.
    let engine = if definition.uses_model() {
        let Some(engine) = run::app_engine(app).await else {
            log::info!(
                "transforms: auto-apply {} skipped (no engine)",
                definition.id
            );
            return AutoApplied {
                text: source.to_owned(),
                applied: false,
                skipped: false,
            };
        };
        Some(engine)
    } else {
        None
    };
    let started = Instant::now();
    let result = with_engine(engine.as_ref(), definition, source).await;
    // Audit OT2: outcome and timing only (never the text), so a Mac run can
    // tell an unchanged rewrite from one that never ran.
    log::info!(
        "transforms: auto-apply {} {} in {} ms",
        definition.id,
        match (result.applied, result.text != source) {
            (true, true) => "changed the text",
            (true, false) => "left the text unchanged",
            (false, _) => "failed open",
        },
        started.elapsed().as_millis()
    );
    if result.applied && result.text != source {
        let app_bundle_id =
            crate::fork::context::frontmost::frontmost().and_then(|app| app.bundle_id);
        match record::record(
            app,
            &NewRun {
                transform_id: definition.id,
                app_bundle_id: app_bundle_id.as_deref(),
                source_text: source,
                result_text: &result.text,
                created_at_ms: crate::fork::context::now_ms(),
            },
        ) {
            Ok(_) => crate::fork::pill::show_message(
                crate::fork::pill::model::PillMessage::TransformDone,
            ),
            Err(error) => log::warn!("transforms: could not record auto-apply: {error}"),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::llm::chat::ChatRequest;
    use crate::fork::llm::post_process::EngineError;

    struct FakeEngine {
        fail: bool,
    }

    impl Engine for FakeEngine {
        async fn complete(
            &self,
            _request: ChatRequest,
            _timeout: Duration,
        ) -> Result<String, EngineError> {
            if self.fail {
                return Err(EngineError::Failed("fake".into()));
            }
            Ok(serde_json::json!({"text": "Rewritten text."}).to_string())
        }
    }

    fn definition() -> Definition {
        Definition {
            id: 1,
            key: "polish".into(),
            name: "Polish".into(),
            instruction: "Polish".into(),
            edits: Vec::new(),
            unclear: Vec::new(),
            guard: super::super::record::GuardProfile::Flexible,
        }
    }

    #[test]
    fn runs_under_five_seconds_apart_are_skipped() {
        let start = Instant::now();
        assert!(admits(None, start));
        assert!(!admits(Some(start), start + Duration::from_millis(4_999)));
        assert!(admits(Some(start), start + Duration::from_secs(5)));
    }

    #[tokio::test]
    async fn auto_apply_is_fail_open() {
        let source = "Original cleanup output.";
        let failed = with_engine(Some(&FakeEngine { fail: true }), &definition(), source).await;
        assert_eq!(failed.text, source);
        assert!(!failed.applied);
        let applied = with_engine(Some(&FakeEngine { fail: false }), &definition(), source).await;
        assert_eq!(applied.text, "Rewritten text.");
        assert!(applied.applied);
    }
}
