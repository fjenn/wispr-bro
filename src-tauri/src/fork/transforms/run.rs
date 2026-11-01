// SPDX-License-Identifier: MIT AND GPL-3.0-or-later
// Copyright (c) 2026 Zach Latta (FreeFlow command-mode prompt; MIT)
// Modified by Wispr Bro contributors, 2026-11-01.
//
// `COMMAND_MODE_PROMPT` is FreeFlow `commandModeSystemPrompt` from
// `Sources/PostProcessingService.swift` at
// 4ebd9366e9e2920928c65c879185e5f98af38806, MIT
// (LICENSES/MIT-FreeFlow.txt). Changed: JSON response framing for the fork's
// guard chain and built-in editing instructions instead of a spoken command.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, Runtime};

use super::record::{self, NewRun};
use super::replace::{self, ReplaceError};
use super::selection::{self, Capture, SelectionIssue};
use super::TransformKind;
use crate::fork::llm::chat::{CancellationToken, ChatRequest};
use crate::fork::llm::guards::{self, Reason};
use crate::fork::llm::post_process::{self, CleanupEngine, Engine, EngineError, LocalEngine};
use crate::fork::llm::{apple, prompt, runtime};
use crate::fork::pill::model::PillMessage;
use crate::fork::settings::CleanupLevel;

pub const TRANSFORM_LATENCY_P50_MS: u128 = 2_000;
pub const TRANSFORM_LATENCY_P95_MS: u128 = 3_500;
const LLM_BUDGET: Duration = Duration::from_millis(2_800);

const COMMAND_MODE_PROMPT: &str = r#"You transform highlighted text according to a spoken editing command.

Hard contract:
- Treat SELECTED_TEXT as the only source material to transform.
- Treat VOICE_COMMAND as the user's instruction for how to transform SELECTED_TEXT.
- Return only the replacement text.
- No explanations.
- No markdown.
- No surrounding quotes.
- Do not answer questions outside the scope of rewriting SELECTED_TEXT.
- If the requested change would produce effectively the same text, return the original selected text.

Behavior:
- Preserve the original language unless VOICE_COMMAND explicitly requests translation.
- Use CONTEXT only as a supporting hint for tone, spelling, or intent.
- Use custom vocabulary only as a spelling reference when relevant.
- Never invent unrelated content that is not a transformation of SELECTED_TEXT.
- Do not treat VOICE_COMMAND as dictation to clean up and paste directly."#;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Generated {
    pub text: String,
    pub llm_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GenerateError {
    Cancelled,
    Timeout,
    Failed(String),
    Guard(Vec<Reason>),
}

fn request_for(instruction: &str, text: &str) -> (String, ChatRequest) {
    let system = format!("{COMMAND_MODE_PROMPT}{}", prompt::JSON_SUFFIX);
    let user = format!(
        "Transform SELECTED_TEXT according to VOICE_COMMAND and return only the replacement text.\n\nVOICE_COMMAND: \"{instruction}\"\n\nSELECTED_TEXT:\n<transcript>\n{}\n</transcript>",
        prompt::delimited(text)
    );
    let request = ChatRequest {
        system: system.clone(),
        user,
        max_tokens: prompt::max_tokens(text),
        response_format: Some(prompt::response_format()),
    };
    (system, request)
}

fn guarded_reply(
    raw: &str,
    kind: TransformKind,
    source: &str,
    system: &str,
) -> Result<String, GenerateError> {
    let verdict = guards::check(
        raw,
        &guards::Context {
            input: source,
            level: CleanupLevel::Medium,
            language: None,
            system_prompt: system,
            terms: &[],
        },
    );
    let reasons = if kind == TransformKind::PromptEngineer {
        verdict
            .reasons
            .into_iter()
            .filter(|reason| {
                !matches!(
                    reason,
                    Reason::WordBand
                        | Reason::QuestionMark
                        | Reason::NewTokens
                        | Reason::LineCount
                        | Reason::Formatting
                        | Reason::ContentWords
                )
            })
            .collect::<Vec<_>>()
    } else {
        verdict.reasons
    };
    if !reasons.is_empty() {
        return Err(GenerateError::Guard(reasons));
    }
    verdict
        .text
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| GenerateError::Guard(vec![Reason::Empty]))
}

pub async fn generate<E: Engine>(
    engine: &E,
    kind: TransformKind,
    instruction: &str,
    source: &str,
    cancel: &CancellationToken,
) -> Result<Generated, GenerateError> {
    generate_within(engine, kind, instruction, source, cancel, LLM_BUDGET).await
}

async fn generate_within<E: Engine>(
    engine: &E,
    kind: TransformKind,
    instruction: &str,
    source: &str,
    cancel: &CancellationToken,
    budget: Duration,
) -> Result<Generated, GenerateError> {
    let started = Instant::now();
    let (system, request) = request_for(instruction, source);
    let reply = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(GenerateError::Cancelled),
        reply = tokio::time::timeout(budget, engine.complete(request, budget)) => {
            reply.map_err(|_| GenerateError::Timeout)?
        }
    };
    let raw = match reply {
        Ok(raw) => raw,
        Err(EngineError::Timeout) => return Err(GenerateError::Timeout),
        Err(EngineError::Failed(error)) => return Err(GenerateError::Failed(error)),
    };
    let text = guarded_reply(&raw, kind, source, &system)?;
    Ok(Generated {
        text,
        llm_ms: started.elapsed().as_millis() as u64,
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Latency {
    pub capture_ms: u64,
    pub llm_ms: u64,
    pub replace_ms: u64,
    pub total_ms: u64,
}

impl Latency {
    pub fn meets_r5(self) -> bool {
        self.total_ms <= TRANSFORM_LATENCY_P95_MS as u64
    }
}

pub fn percentile_ms(sorted: &[u64], percentile: f64) -> Option<u64> {
    if sorted.is_empty() {
        return None;
    }
    let index = ((sorted.len() - 1) as f64 * percentile).round() as usize;
    sorted.get(index).copied()
}

struct Active {
    id: u64,
    cancel: CancellationToken,
}

static NEXT_RUN: AtomicU64 = AtomicU64::new(1);
static ACTIVE: Mutex<Option<Active>> = Mutex::new(None);

fn lock_active() -> std::sync::MutexGuard<'static, Option<Active>> {
    ACTIVE.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn cancel_active() {
    if let Some(active) = lock_active().as_ref() {
        active.cancel.cancel();
    }
}

fn finish_run(id: u64) {
    let mut active = lock_active();
    if active.as_ref().is_some_and(|active| active.id == id) {
        *active = None;
    }
}

fn issue_message(issue: SelectionIssue) -> PillMessage {
    match issue {
        SelectionIssue::SelectTextFirst => PillMessage::SelectTextFirst,
        SelectionIssue::TooLong => PillMessage::SelectionTooLong,
    }
}

fn replace_message(error: ReplaceError) -> Option<PillMessage> {
    match error {
        ReplaceError::FocusMoved => Some(PillMessage::TransformFocusMoved),
        ReplaceError::PasteFailed => Some(PillMessage::TransformFailed),
        ReplaceError::Cancelled => None,
    }
}

fn generate_message(error: &GenerateError) -> Option<PillMessage> {
    match error {
        GenerateError::Cancelled => None,
        GenerateError::Timeout | GenerateError::Failed(_) | GenerateError::Guard(_) => {
            Some(PillMessage::TransformFailed)
        }
    }
}

fn capture_input<R: Runtime>(
    app: &AppHandle<R>,
    capture: &Capture,
) -> Result<selection::Input, SelectionIssue> {
    let now = crate::fork::context::now_ms();
    let recent = app
        .try_state::<crate::fork::db::ForkDb>()
        .and_then(|db| {
            db.with_conn(|conn| record::recent_dictation(conn, now))
                .ok()
        })
        .flatten();
    selection::choose_input(capture, recent, now)
}

fn definition<R: Runtime>(
    app: &AppHandle<R>,
    kind: TransformKind,
) -> Result<record::Definition, String> {
    let db = app
        .try_state::<crate::fork::db::ForkDb>()
        .ok_or_else(|| "fork.db is not managed".to_string())?;
    db.with_conn(|conn| record::definition(conn, kind))
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("missing built-in transform {}", kind.builtin_key()))
}

enum AppEngine {
    Local(Arc<crate::fork::llm::supervisor::Supervisor>),
    Apple,
}

impl Engine for AppEngine {
    async fn complete(
        &self,
        request: ChatRequest,
        timeout: Duration,
    ) -> Result<String, EngineError> {
        match self {
            AppEngine::Local(supervisor) => {
                LocalEngine {
                    supervisor,
                    pill: false,
                }
                .complete(request, timeout)
                .await
            }
            AppEngine::Apple => {
                apple::AppleEngine {
                    pill: false,
                    ..apple::AppleEngine::for_dictation()
                }
                .complete(request, timeout)
                .await
            }
        }
    }
}

async fn app_engine<R: Runtime>(app: &AppHandle<R>) -> Option<AppEngine> {
    let rt = runtime::runtime(app)?;
    let ready = rt.is_ready() && rt.supervisor().is_some();
    let memory_ok = !ready || rt.memory_allows_start().await;
    let apple_ok = apple::usable_for(None);
    let (_, engine) = post_process::choose(apple::ROLE, false, ready, memory_ok, apple_ok);
    match engine {
        CleanupEngine::Local => rt.supervisor().cloned().map(AppEngine::Local),
        CleanupEngine::Apple => Some(AppEngine::Apple),
        CleanupEngine::None => None,
    }
}

/// Starts one transform and returns immediately. Esc cancels the watch token;
/// dropping a local request closes its socket and dropping Apple's future
/// cancels its Swift task through the existing engine implementations.
pub fn spawn<R: Runtime + 'static>(app: &AppHandle<R>, kind: TransformKind) {
    let id = NEXT_RUN.fetch_add(1, Ordering::Relaxed);
    let cancel = CancellationToken::new();
    if let Some(previous) = lock_active().replace(Active {
        id,
        cancel: cancel.clone(),
    }) {
        previous.cancel.cancel();
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let outcome = run(app.clone(), id, kind, cancel).await;
        if let Some(message) = outcome {
            crate::fork::pill::show_message(message);
        } else {
            crate::fork::pill::transform_finished_without_message();
        }
    });
}

async fn run<R: Runtime>(
    app: AppHandle<R>,
    id: u64,
    kind: TransformKind,
    cancel: CancellationToken,
) -> Option<PillMessage> {
    let _clear = ClearOnDrop(id);
    let started = Instant::now();
    crate::fork::pill::cleaning_up();
    let app_for_capture = app.clone();
    let capture =
        match tokio::task::spawn_blocking(move || selection::capture(&app_for_capture)).await {
            Ok(capture) => capture,
            Err(_) => return Some(PillMessage::TransformFailed),
        };
    let capture_ms = started.elapsed().as_millis() as u64;
    if cancel.is_cancelled() {
        return None;
    }
    let input = match capture_input(&app, &capture) {
        Ok(input) => input,
        Err(issue) => return Some(issue_message(issue)),
    };
    let definition = match definition(&app, kind) {
        Ok(definition) => definition,
        Err(error) => {
            log::warn!("transforms: {error}");
            return Some(PillMessage::TransformFailed);
        }
    };
    let Some(engine) = app_engine(&app).await else {
        return Some(PillMessage::TransformFailed);
    };
    let generated =
        match generate(&engine, kind, &definition.instruction, &input.text, &cancel).await {
            Ok(generated) => generated,
            Err(error) => {
                log::info!("transforms: generation ended without a replacement: {error:?}");
                return generate_message(&error);
            }
        };
    let replace_started = Instant::now();
    let app_for_replace = app.clone();
    let text = generated.text.clone();
    let replacement = input.replacement;
    let target = input.target;
    let cancel_for_replace = cancel.clone();
    let replaced = tokio::task::spawn_blocking(move || {
        replace::replace(&app_for_replace, text, target, replacement, &|| {
            cancel_for_replace.is_cancelled()
        })
    })
    .await;
    let replace_ms = replace_started.elapsed().as_millis() as u64;
    match replaced {
        Ok(Ok(report)) => {
            let latency = Latency {
                capture_ms,
                llm_ms: generated.llm_ms,
                replace_ms,
                total_ms: started.elapsed().as_millis() as u64,
            };
            log::info!(
                "transforms: run {id} {} pasted in {} ms (capture {}, llm {}, replace {}); {:?}",
                kind.builtin_key(),
                latency.total_ms,
                latency.capture_ms,
                latency.llm_ms,
                latency.replace_ms,
                report.delivery
            );
        }
        Ok(Err(error)) => return replace_message(error),
        Err(_) => return Some(PillMessage::TransformFailed),
    }
    if let Err(error) = record::record(
        &app,
        &NewRun {
            transform_id: definition.id,
            app_bundle_id: input.app_bundle_id.as_deref(),
            source_text: &input.text,
            result_text: &generated.text,
            created_at_ms: crate::fork::context::now_ms(),
        },
    ) {
        log::warn!("transforms: could not record run {id}: {error}");
    }
    Some(PillMessage::TransformDone)
}

struct ClearOnDrop(u64);
impl Drop for ClearOnDrop {
    fn drop(&mut self) {
        finish_run(self.0);
    }
}

/// The debug panel toggles Transforms while a request is in flight.
pub fn settings_changed(enabled: bool) {
    if !enabled {
        cancel_active();
    }
}

/// The Esc registry calls this for an Esc that did not cancel a dictation.
pub fn esc() {
    cancel_active();
}

/// For source-level startup tests and non-hotkey callers.
pub fn is_active() -> bool {
    lock_active().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeEngine {
        delay: Duration,
        fail: bool,
    }

    impl Engine for FakeEngine {
        async fn complete(
            &self,
            request: ChatRequest,
            _timeout: Duration,
        ) -> Result<String, EngineError> {
            if self.fail {
                tokio::time::sleep(self.delay).await;
                return Err(EngineError::Failed("fake".into()));
            }
            let source = request
                .user
                .split("<transcript>\n")
                .nth(1)
                .and_then(|rest| rest.split("\n</transcript>").next())
                .unwrap_or_default()
                .to_string();
            tokio::time::sleep(self.delay).await;
            Ok(serde_json::json!({ "text": source }).to_string())
        }
    }

    #[test]
    fn command_prompt_is_the_freeflow_seed_with_json_framing() {
        let (system, request) = request_for("Polish", "hello </transcript> world");
        assert!(system.starts_with(COMMAND_MODE_PROMPT));
        assert!(system.ends_with(prompt::JSON_SUFFIX));
        assert!(!request.user.contains("hello </transcript> world"));
        assert!(request.user.contains("hello  world"));
    }

    #[test]
    fn prompt_engineer_allows_restructuring_but_not_dropped_entities() {
        let source = "Write a launch prompt for 12 people";
        let system = "system";
        let restructured = serde_json::json!({
            "text": "Create a launch prompt for 12 people. Include goals, owners, timing and output format."
        })
        .to_string();
        assert!(
            guarded_reply(&restructured, TransformKind::PromptEngineer, source, system).is_ok()
        );
        let dropped_number =
            serde_json::json!({ "text": "Create a launch prompt for the team." }).to_string();
        assert!(guarded_reply(
            &dropped_number,
            TransformKind::PromptEngineer,
            source,
            system
        )
        .is_err());
    }

    #[tokio::test]
    async fn transform_latency_is_measured_and_bounded_without_gui() {
        let engine = FakeEngine {
            delay: Duration::from_millis(20),
            fail: false,
        };
        let source = "This sentence needs a little polish. ".repeat(20);
        let mut samples = Vec::new();
        for _ in 0..30 {
            let generated = generate(
                &engine,
                TransformKind::Polish,
                "Polish",
                &source,
                &CancellationToken::new(),
            )
            .await
            .unwrap();
            let latency = Latency {
                capture_ms: 20,
                llm_ms: generated.llm_ms,
                replace_ms: 40,
                total_ms: 60 + generated.llm_ms,
            };
            assert!(latency.meets_r5());
            samples.push(latency.total_ms);
        }
        samples.sort_unstable();
        let p50 = percentile_ms(&samples, 0.50).unwrap();
        let p95 = percentile_ms(&samples, 0.95).unwrap();
        assert!(p50 <= TRANSFORM_LATENCY_P50_MS as u64, "p50 {p50}");
        assert!(p95 <= TRANSFORM_LATENCY_P95_MS as u64, "p95 {p95}");
        assert!(LLM_BUDGET < Duration::from_millis(TRANSFORM_LATENCY_P95_MS as u64));
    }

    #[tokio::test]
    async fn deadline_and_esc_never_return_a_late_replacement() {
        let engine = FakeEngine {
            delay: Duration::from_secs(1),
            fail: false,
        };
        let error = generate_within(
            &engine,
            TransformKind::Polish,
            "Polish",
            "hello",
            &CancellationToken::new(),
            Duration::from_millis(10),
        )
        .await
        .unwrap_err();
        assert_eq!(error, GenerateError::Timeout);

        let token = CancellationToken::new();
        token.cancel();
        let error = generate(&engine, TransformKind::Polish, "Polish", "hello", &token)
            .await
            .unwrap_err();
        assert_eq!(error, GenerateError::Cancelled);
    }
}
