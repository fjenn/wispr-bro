// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, Runtime};

use super::literal;
use super::record::{self, GuardProfile, NewRun};
use super::replace::{self, ReplaceError};
use super::selection::{self, Capture, SelectionIssue};
use super::TransformKind;
use crate::fork::llm::chat::{CancellationToken, ChatRequest};
use crate::fork::llm::guards::{self, Reason};
use crate::fork::llm::post_process::{Engine, EngineError, LocalEngine};
use crate::fork::llm::{apple, instruct, prompt};
use crate::fork::pill::model::PillMessage;
use crate::fork::settings::CleanupLevel;

pub const TRANSFORM_LATENCY_P50_MS: u128 = 2_000;
pub const TRANSFORM_LATENCY_P95_MS: u128 = 3_500;
/// The deadline of a short transform; longer texts get [`budget`].
#[cfg(test)]
const LLM_BUDGET: Duration = crate::fork::llm::deadline::TRANSFORM_FLOOR;

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
    /// The result equals the input: exact edits matched nothing, Polish found
    /// nothing to fix, or the model did not carry out the instruction.
    Unchanged,
    /// The transform needs the model and no engine is ready.
    NoEngine,
}

/// The transform prompt: the user's instruction, the text and "return only
/// the result", nothing else. Transforms run on a general instruct model
/// (`catalog::TRANSFORM_MODEL`) that simply follows the instruction; no
/// worked examples and no instruction-specific teaching (owner, 2026-11-04).
pub(crate) const TRANSFORM_PROMPT: &str =
    "Apply the instruction to the text. Return only the result.";

fn user_turn(instruction: &str, text: &str) -> String {
    format!(
        "Instruction: {}\n\nText:\n{}",
        instruction.trim(),
        prompt::delimited(text)
    )
}

fn request_for(instruction: &str, text: &str) -> (String, ChatRequest) {
    let system = TRANSFORM_PROMPT.to_owned();
    let request = ChatRequest {
        system: system.clone(),
        user: user_turn(instruction, text),
        max_tokens: instruction_max_tokens(text),
        response_format: None,
        examples: Vec::new(),
    };
    (system, request)
}

/// The model's reply as the guard chain's JSON: a plain-text reply is
/// wrapped; Apple's engine already answers in that shape.
fn as_reply_json(raw: &str) -> String {
    if prompt::parse_reply(raw).is_some() {
        raw.to_owned()
    } else {
        serde_json::json!({ "text": raw.trim() }).to_string()
    }
}

/// Output budget of an instruction: room to grow a short text (an e-mail, a
/// list, a reply) and the cleanup budget for long ones (OT2-D).
pub(crate) fn instruction_max_tokens(text: &str) -> u32 {
    let chars = u32::try_from(text.chars().count()).unwrap_or(u32::MAX);
    prompt::max_tokens(text)
        .max(64u32.saturating_add(chars))
        .clamp(256, 2048)
}

/// The deadline of one transform request (scaled with the text).
pub fn budget(source: &str) -> Duration {
    crate::fork::llm::deadline::transform(source)
}

fn guarded_reply(
    raw: &str,
    guard: GuardProfile,
    source: &str,
    system: &str,
) -> Result<String, GenerateError> {
    let text = match guard {
        // Polish edits the user's own words: the cleanup guards, with a
        // paragraph-tolerant line rule.
        GuardProfile::Strict => {
            let ctx = guards::Context {
                input: source,
                level: CleanupLevel::Medium,
                language: None,
                system_prompt: system,
                terms: &[],
            };
            let verdict = guards::check(raw, &ctx);
            let reasons: Vec<Reason> = verdict
                .reasons
                .into_iter()
                .filter(|reason| {
                    *reason != Reason::LineCount
                        || !verdict
                            .text
                            .as_deref()
                            .is_some_and(|text| guards::lines_survive_loosely(source, text))
                })
                .collect();
            if !reasons.is_empty() {
                return Err(GenerateError::Guard(reasons));
            }
            verdict.text.unwrap_or_default()
        }
        // An instruction does what the user asked: only a runaway reply is
        // refused (OT2-H).
        GuardProfile::Instruction | GuardProfile::Flexible => {
            let text = prompt::parse_reply(raw).ok_or(GenerateError::Guard(vec![Reason::Json]))?;
            if guards::is_runaway(source, &text) {
                return Err(GenerateError::Guard(vec![Reason::TooLong]));
            }
            text
        }
    };
    if text.trim().is_empty() {
        return Err(GenerateError::Guard(vec![Reason::Empty]));
    }
    Ok(text)
}

fn same_text(a: &str, b: &str) -> bool {
    a.split_whitespace().eq(b.split_whitespace())
}

pub async fn generate<E: Engine>(
    engine: &E,
    kind: TransformKind,
    instruction: &str,
    source: &str,
    cancel: &CancellationToken,
) -> Result<Generated, GenerateError> {
    let definition = record::Definition {
        id: 0,
        key: kind.builtin_key().to_owned(),
        name: kind.builtin_key().to_owned(),
        instruction: instruction.to_owned(),
        edits: Vec::new(),
        guard: if kind == TransformKind::PromptEngineer {
            GuardProfile::Flexible
        } else {
            GuardProfile::Strict
        },
    };
    generate_within(Some(engine), &definition, source, cancel, budget(source)).await
}

/// Runs one transform: the model when its instruction needs it (guarded and
/// within the deadline), then its exact edits. Exact-only transforms never
/// need an engine. Every caller (shortcut, preview, Retry, auto-apply and
/// the eval) goes through here.
pub async fn generate_definition<E: Engine>(
    engine: Option<&E>,
    definition: &record::Definition,
    source: &str,
    cancel: &CancellationToken,
) -> Result<Generated, GenerateError> {
    generate_within(engine, definition, source, cancel, budget(source)).await
}

async fn generate_within<E: Engine>(
    engine: Option<&E>,
    definition: &record::Definition,
    source: &str,
    cancel: &CancellationToken,
    budget: Duration,
) -> Result<Generated, GenerateError> {
    let started = Instant::now();
    let mut text = source.to_owned();
    let mut llm_ms = 0;
    if definition.uses_model() {
        let engine = engine.ok_or(GenerateError::NoEngine)?;
        let (system, request) = request_for(&definition.instruction, source);
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
        text = guarded_reply(&as_reply_json(&raw), definition.guard, source, &system)?;
        llm_ms = started.elapsed().as_millis() as u64;
    } else if cancel.is_cancelled() {
        return Err(GenerateError::Cancelled);
    }
    if !definition.edits.is_empty() {
        text = literal::apply(&definition.edits, &text).text;
    }
    if same_text(&text, source) {
        return Err(GenerateError::Unchanged);
    }
    Ok(Generated { text, llm_ms })
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

fn generate_message(error: &GenerateError, definition: &record::Definition) -> Option<PillMessage> {
    match error {
        GenerateError::Cancelled => None,
        // Exact edits that matched nothing, or Polish on text that needs
        // nothing, report so rather than pasting the same text.
        GenerateError::Unchanged
            if !definition.uses_model() || definition.guard == GuardProfile::Strict =>
        {
            Some(PillMessage::TransformNoChange)
        }
        GenerateError::Unchanged => Some(PillMessage::TransformNotApplied),
        GenerateError::NoEngine => Some(PillMessage::TransformGettingReady),
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

pub(crate) enum AppEngine {
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

/// The engine for transforms and Command Mode: the instruction model
/// (OT2-H); while it is not on disk yet (its download starts here) or memory
/// is short, Apple's model when it is usable; else none ("getting ready").
pub(crate) async fn app_engine<R: Runtime>(app: &AppHandle<R>) -> Option<AppEngine> {
    if let Some(supervisor) = instruct::ready_supervisor(app).await {
        return Some(AppEngine::Local(supervisor));
    }
    apple::usable_for(None).then_some(AppEngine::Apple)
}

#[derive(Clone, Copy)]
enum Target {
    Builtin(TransformKind),
    Custom(i64),
}

/// Starts one transform and returns immediately. Esc cancels the watch token;
/// dropping a local request closes its socket and dropping Apple's future
/// cancels its Swift task through the existing engine implementations.
pub fn spawn<R: Runtime + 'static>(app: &AppHandle<R>, kind: TransformKind) {
    spawn_target(app, Target::Builtin(kind));
}

pub fn spawn_custom<R: Runtime + 'static>(app: &AppHandle<R>, transform_id: i64) {
    spawn_target(app, Target::Custom(transform_id));
}

fn spawn_target<R: Runtime + 'static>(app: &AppHandle<R>, target: Target) {
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
        let outcome = run(app.clone(), id, target, cancel).await;
        if let Some(message) = outcome {
            crate::fork::pill::show_message(message);
        } else {
            crate::fork::pill::transform_finished_without_message();
        }
    });
}

fn target_definition<R: Runtime>(
    app: &AppHandle<R>,
    target: Target,
) -> Result<record::Definition, String> {
    match target {
        Target::Builtin(kind) => definition(app, kind),
        Target::Custom(id) => {
            let db = app
                .try_state::<crate::fork::db::ForkDb>()
                .ok_or_else(|| "fork.db is not managed".to_string())?;
            db.with_conn(|conn| record::definition_by_id(conn, id))
                .map_err(|e| e.to_string())?
                .filter(|definition| definition.key.starts_with("custom-"))
                .ok_or_else(|| format!("missing custom transform {id}"))
        }
    }
}

async fn run<R: Runtime>(
    app: AppHandle<R>,
    id: u64,
    target: Target,
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
    let definition = match target_definition(&app, target) {
        Ok(definition) => definition,
        Err(error) => {
            log::warn!("transforms: {error}");
            return Some(PillMessage::TransformFailed);
        }
    };
    let engine = if definition.uses_model() {
        app_engine(&app).await
    } else {
        None
    };
    let generated =
        match generate_definition(engine.as_ref(), &definition, &input.text, &cancel).await {
            Ok(generated) => generated,
            Err(error) => {
                log::info!("transforms: generation ended without a replacement: {error:?}");
                return generate_message(&error, &definition);
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
                definition.key,
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

/// What the editor's Preview shows.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct TransformPreview {
    /// The transformed text, when there is one.
    pub text: Option<String>,
    pub outcome: PreviewOutcome,
    /// Exact edits only: the model was not used.
    pub exact: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum PreviewOutcome {
    Changed,
    /// Nothing changed (exact edits found no match, or nothing to fix).
    NoChange,
    /// The model returned the text without doing what was asked.
    NotApplied,
    /// The model's answer was rejected or it failed.
    Failed,
    Timeout,
    /// The local AI is not ready (still downloading, or memory).
    NoEngine,
}

pub fn preview_outcome(
    result: &Result<Generated, GenerateError>,
    definition: &record::Definition,
) -> PreviewOutcome {
    match result {
        Ok(_) => PreviewOutcome::Changed,
        Err(GenerateError::Unchanged) => {
            match generate_message(&GenerateError::Unchanged, definition) {
                Some(PillMessage::TransformNoChange) => PreviewOutcome::NoChange,
                _ => PreviewOutcome::NotApplied,
            }
        }
        Err(GenerateError::Timeout) => PreviewOutcome::Timeout,
        Err(GenerateError::NoEngine) => PreviewOutcome::NoEngine,
        Err(_) => PreviewOutcome::Failed,
    }
}

pub async fn preview<R: Runtime>(
    app: &AppHandle<R>,
    definition: record::Definition,
    source: &str,
) -> TransformPreview {
    let engine = if definition.uses_model() {
        app_engine(app).await
    } else {
        None
    };
    let result = generate_definition(
        engine.as_ref(),
        &definition,
        source,
        &CancellationToken::new(),
    )
    .await;
    if let Err(error) = &result {
        log::info!("transforms: preview ended without a result: {error:?}");
    }
    TransformPreview {
        outcome: preview_outcome(&result, &definition),
        exact: !definition.uses_model(),
        text: result.ok().map(|generated| generated.text),
    }
}

/// Regenerates a recorded source without pasting into a changed focus target.
/// The new run appears in both diff views, where Copy is an explicit action.
pub async fn retry<R: Runtime>(
    app: &AppHandle<R>,
    run_id: i64,
) -> Result<record::ForkTransformViewChanges, String> {
    let previous = app
        .try_state::<crate::fork::db::ForkDb>()
        .ok_or_else(|| "fork.db is not managed".to_string())?
        .with_conn(|conn| record::get(conn, run_id))
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "transform run not found".to_string())?;
    let definition = app
        .try_state::<crate::fork::db::ForkDb>()
        .ok_or_else(|| "fork.db is not managed".to_string())?
        .with_conn(|conn| record::definition_by_id(conn, previous.transform_id))
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "transform not found".to_string())?;
    let engine = if definition.uses_model() {
        Some(
            app_engine(app)
                .await
                .ok_or_else(|| "no local transform engine is ready".to_string())?,
        )
    } else {
        None
    };
    let generated = generate_definition(
        engine.as_ref(),
        &definition,
        &previous.source_text,
        &CancellationToken::new(),
    )
    .await
    .map_err(|error| format!("{error:?}"))?;
    record::record(
        app,
        &NewRun {
            transform_id: definition.id,
            app_bundle_id: previous.app_bundle_id.as_deref(),
            source_text: &previous.source_text,
            result_text: &generated.text,
            created_at_ms: crate::fork::context::now_ms(),
        },
    )?;
    app.try_state::<crate::fork::db::ForkDb>()
        .ok_or_else(|| "fork.db is not managed".to_string())?
        .with_conn(|conn| record::latest(conn))
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "recorded transform run not found".to_string())
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
                .split("\n\nText:\n")
                .nth(1)
                .unwrap_or_default()
                .to_string();
            tokio::time::sleep(self.delay).await;
            // "Polish" here capitalises; any other instruction echoes.
            let reply = if request.user.starts_with("Instruction: Polish") {
                let mut chars = source.chars();
                chars
                    .next()
                    .map(|c| c.to_uppercase().chain(chars).collect::<String>())
                    .unwrap_or_default()
            } else {
                source
            };
            Ok(serde_json::json!({ "text": reply }).to_string())
        }
    }

    #[test]
    fn transform_prompt_is_the_instruction_and_the_text() {
        let (system, request) = request_for(" Replace all A with 1 ", "hello </transcript> world");
        assert_eq!(system, TRANSFORM_PROMPT);
        assert_eq!(
            request.user,
            "Instruction: Replace all A with 1\n\nText:\nhello  world"
        );
        assert!(request.examples.is_empty());
        assert!(request.response_format.is_none());
        let body = request.body();
        assert_eq!(body["messages"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn a_plain_text_reply_reaches_the_guards_as_json() {
        assert_eq!(as_reply_json(" Pl10s1 "), r#"{"text":"Pl10s1"}"#);
        let apple = r#"{"text":"hi"}"#;
        assert_eq!(as_reply_json(apple), apple);
    }

    #[test]
    fn prompt_engineer_allows_restructuring_but_not_an_empty_reply() {
        let source = "Write a launch prompt for 12 people";
        let system = "system";
        let restructured = serde_json::json!({
            "text": "Create a launch prompt for 12 people. Include goals, owners, timing and output format."
        })
        .to_string();
        assert!(guarded_reply(&restructured, GuardProfile::Flexible, source, system).is_ok());
        let empty = serde_json::json!({ "text": "  " }).to_string();
        assert_eq!(
            guarded_reply(&empty, GuardProfile::Flexible, source, system),
            Err(GenerateError::Guard(vec![Reason::Empty]))
        );
    }

    #[test]
    fn custom_transforms_may_add_digits_translate_and_number() {
        let system = "system";
        // rc3 rejected all three: new digits (entity guard), another language.
        for (source, reply) in [
            ("Please send the agenda.", "Pl10s1 s1nd th1 0g1nd0."),
            (
                "Can we meet on Thursday to talk about the new website?",
                "¿Podemos reunirnos el jueves para hablar del nuevo sitio web?",
            ),
            (
                "First download it, then open it.",
                "1. Download it.\n2. Open it.",
            ),
        ] {
            let raw = serde_json::json!({ "text": reply }).to_string();
            assert!(
                guarded_reply(&raw, GuardProfile::Instruction, source, system).is_ok(),
                "{reply}"
            );
            assert!(guarded_reply(&raw, GuardProfile::Strict, source, system).is_err());
        }
    }

    #[test]
    fn instructions_may_grow_a_text_but_not_run_away() {
        let source = "tell sarah the contract is signed";
        let email = serde_json::json!({
            "text": "Hi Sarah,\n\nI'm happy to let you know that the contract is signed. We can start planning the next steps whenever you are ready.\n\nBest regards"
        })
        .to_string();
        // rc3 refused this as more than 3× the input.
        assert!(guarded_reply(&email, GuardProfile::Instruction, source, "s").is_ok());
        let runaway = serde_json::json!({ "text": "word ".repeat(400) }).to_string();
        assert_eq!(
            guarded_reply(&runaway, GuardProfile::Instruction, source, "s"),
            Err(GenerateError::Guard(vec![Reason::TooLong]))
        );
        // Prompt Engineer turns "write me …" into a longer prompt.
        let request = "write me a blog post about solar panels for homeowners under 500 words";
        let prompt = serde_json::json!({
            "text": "Write a blog post of at most 500 words for homeowners who are considering solar panels. Explain costs, savings, installation and maintenance in plain language, end with a short checklist, and use a friendly, practical tone with clear headings for each part of the post so that it is easy to scan."
        })
        .to_string();
        assert!(guarded_reply(&prompt, GuardProfile::Flexible, request, "s").is_ok());
        // Room to write the longer text.
        assert!(instruction_max_tokens(source) >= 256);
        assert_eq!(instruction_max_tokens(&"x".repeat(10_000)), 2048);
    }

    #[test]
    fn polish_keeps_paragraphs_but_may_rewrap_lines() {
        let note = "First point of the note\nstill the first paragraph.\n\nSecond paragraph here.";
        let rewrapped = serde_json::json!({
            "text": "First point of the note, still the first paragraph.\n\nSecond paragraph here."
        })
        .to_string();
        // rc3 refused this (3 lines became 2).
        assert!(guarded_reply(&rewrapped, GuardProfile::Strict, note, "s").is_ok());
        let merged = serde_json::json!({
            "text": "First point of the note, still the first paragraph. Second paragraph here."
        })
        .to_string();
        assert!(matches!(
            guarded_reply(&merged, GuardProfile::Strict, note, "s"),
            Err(GenerateError::Guard(reasons)) if reasons.contains(&Reason::LineCount)
        ));
    }

    #[test]
    fn long_texts_get_a_scaled_deadline() {
        assert_eq!(budget("short"), LLM_BUDGET);
        let note = "word ".repeat(336);
        assert!(
            budget(&note) > Duration::from_secs(10),
            "{:?}",
            budget(&note)
        );
        assert_eq!(
            budget(&"word ".repeat(2_000)),
            crate::fork::llm::deadline::TAKE_CAP
        );
    }

    fn definition(instruction: &str, edits: &str) -> record::Definition {
        record::Definition {
            id: 1,
            key: "custom-1".into(),
            name: "Mine".into(),
            instruction: instruction.into(),
            edits: literal::plan(edits).edits,
            guard: GuardProfile::Instruction,
        }
    }

    #[tokio::test]
    async fn exact_transforms_never_need_the_model() {
        let definition = definition("", "Replace all A and E with 0 and 1");
        let generated = generate_definition::<FakeEngine>(
            None,
            &definition,
            "Please send the agenda.",
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(generated.text, "Pl10s1 s1nd th1 0g1nd0.");
        assert_eq!(generated.llm_ms, 0);
        let error = generate_definition::<FakeEngine>(
            None,
            &definition,
            "Nothing to do: buy rum.",
            &CancellationToken::new(),
        )
        .await
        .unwrap_err();
        assert_eq!(error, GenerateError::Unchanged);
        assert_eq!(
            generate_message(&error, &definition),
            Some(PillMessage::TransformNoChange)
        );
    }

    #[tokio::test]
    async fn exact_edits_follow_the_model_and_an_echo_is_reported() {
        // The fake engine echoes its input: the model "did nothing".
        let engine = FakeEngine {
            delay: Duration::ZERO,
            fail: false,
        };
        let echo = definition("Make it formal", "");
        let error =
            generate_definition(Some(&engine), &echo, "hey there", &CancellationToken::new())
                .await
                .unwrap_err();
        assert_eq!(error, GenerateError::Unchanged);
        assert_eq!(
            generate_message(&error, &echo),
            Some(PillMessage::TransformNotApplied)
        );
        let mixed = definition("Make it formal", "Replace Bob with Robert");
        let generated = generate_definition(
            Some(&engine),
            &mixed,
            "Bob says hi",
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(generated.text, "Robert says hi");
        let no_engine = generate_definition::<FakeEngine>(
            None,
            &mixed,
            "Bob says hi",
            &CancellationToken::new(),
        )
        .await
        .unwrap_err();
        assert_eq!(no_engine, GenerateError::NoEngine);
        assert_eq!(
            preview_outcome(&Err(no_engine), &mixed),
            PreviewOutcome::NoEngine
        );
    }

    #[tokio::test]
    async fn transform_latency_is_measured_and_bounded_without_gui() {
        let engine = FakeEngine {
            delay: Duration::from_millis(20),
            fail: false,
        };
        let source = "this sentence needs a little polish. ".repeat(20);
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
            Some(&engine),
            &definition("Polish", ""),
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
