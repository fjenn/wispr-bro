// SPDX-License-Identifier: GPL-3.0-or-later
//! Apple Foundation Models as a cleanup engine (R5 S-12b, RS-08).
//!
//! The bridge is `crates/wb-apple-fm` (its own Swift build, `wb_apple_fm_`
//! symbols); Handy's `apple_intelligence.rs` and `swift/apple_intelligence.swift`
//! are not touched and never called by the fork. [`AppleEngine`] answers the
//! same [`ChatRequest`] as the sidecar, so [`super::post_process::clean`] runs
//! the same gate, prompt, guard chain, deadlines and fail-open on its reply.
//!
//! - **Blocking call, cancelling timeout.** The bridge blocks; it runs in
//!   `spawn_blocking`. The request deadline is enforced in Swift (the task is
//!   cancelled when it passes), and if the caller gives up first (the take's
//!   outer bound, Esc) dropping the future cancels the Swift task by its id.
//! - **Availability** is read at most every 10 s (hex); the reasons map to
//!   [`AppleAvailability`] for the AI engine panel.
//! - **Prewarm** at key-down (TypeWhisper): the system unloads the model a few
//!   seconds after a request; a prewarmed session with the take's
//!   instructions is used by its first request.
//! - **Role** ([`ROLE`], RS-08's decision): see `docs/fork/spikes/RS-08-apple-fm.md`.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;
use wb_apple_fm::{self as fm, ErrorCode, Generation, Guardrails};

use super::chat::ChatRequest;
use super::post_process::{Engine, EngineError};

/// What Apple's model is used for (RS-08 outcome).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum AppleRole {
    /// Every cleanup runs on Apple's model when it is available.
    Default,
    /// Only while the local model is not ready (downloading, failed).
    Fallback,
}

/// RS-08: Apple's model missed the quality gates against Muesli (see the
/// spike report), so it only stands in while the local model is not ready.
pub const ROLE: AppleRole = AppleRole::Fallback;

/// Guardrails for cleanup (RS-08 compared both).
pub const GUARDRAILS: Guardrails = Guardrails::PermissiveContentTransformations;

/// Guided generation of `{text}` instead of a plain reply. RS-08: the plain
/// reply (the prompt already asks for `{"text": …}` JSON, which Apple's
/// model writes) was better on every metric and faster than guided
/// generation, so cleanup asks for a plain reply.
pub const GUIDED: bool = false;

/// `SystemLanguageModel.default.availability` for the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum AppleAvailability {
    Available,
    /// This Mac cannot run Apple Intelligence.
    DeviceNotEligible,
    /// Apple Intelligence is off in System Settings.
    NotEnabled,
    /// Apple's model is still downloading.
    ModelNotReady,
    /// macOS before 26, or a build without the bridge.
    Unsupported,
    Unknown,
}

impl From<fm::Availability> for AppleAvailability {
    fn from(a: fm::Availability) -> Self {
        match a {
            fm::Availability::Available => Self::Available,
            fm::Availability::DeviceNotEligible => Self::DeviceNotEligible,
            fm::Availability::AppleIntelligenceNotEnabled => Self::NotEnabled,
            fm::Availability::ModelNotReady => Self::ModelNotReady,
            fm::Availability::UnsupportedSystem => Self::Unsupported,
            fm::Availability::Unknown => Self::Unknown,
        }
    }
}

/// Apple's model in the AI engine panel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ForkAppleStatus {
    pub availability: AppleAvailability,
    pub role: AppleRole,
    /// Context window in tokens (0 when unavailable).
    pub context_tokens: u32,
    /// Requests since launch.
    pub requests: u32,
    /// Of those, refused by Apple's guardrails.
    pub refusals: u32,
    /// Of those, throttled by the system (`rateLimited`).
    pub rate_limited: u32,
}

static REQUESTS: AtomicU32 = AtomicU32::new(0);
static REFUSALS: AtomicU32 = AtomicU32::new(0);
static RATE_LIMITED: AtomicU32 = AtomicU32::new(0);

/// The current availability (cached 10 s).
pub fn availability() -> AppleAvailability {
    fm::availability().into()
}

/// `true` when Apple's model can clean a take in `language` now (an
/// unknown language counts as supported: the guards check the result).
pub fn usable_for(language: Option<&str>) -> bool {
    availability() == AppleAvailability::Available
        && language.is_none_or(|l| fm::supports_language(primary(l)))
}

fn primary(code: &str) -> &str {
    code.split(['-', '_']).next().unwrap_or(code)
}

/// Status for the panel.
pub fn status() -> ForkAppleStatus {
    let availability = availability();
    ForkAppleStatus {
        availability,
        role: ROLE,
        context_tokens: if availability == AppleAvailability::Available {
            u32::try_from(fm::context_size()).unwrap_or(u32::MAX)
        } else {
            0
        },
        requests: REQUESTS.load(Ordering::Relaxed),
        refusals: REFUSALS.load(Ordering::Relaxed),
        rate_limited: RATE_LIMITED.load(Ordering::Relaxed),
    }
}

/// Loads Apple's model and caches `system` for the take's first request.
/// Returns at once; does nothing when the model is not available.
pub fn prewarm(system: &str) {
    if availability() != AppleAvailability::Available {
        return;
    }
    let system = system.to_owned();
    // The bridge call itself returns quickly; keep it off async threads.
    std::thread::spawn(move || fm::prewarm(&system, GUARDRAILS));
}

/// Cancels the Swift request when the future is dropped before it finished.
struct CancelOnDrop {
    id: u64,
    armed: bool,
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if self.armed {
            fm::cancel(self.id);
        }
    }
}

/// The model's reply as the guard chain's JSON (`{"text": …}`): a guided
/// reply is the text itself; a plain reply may already be that JSON.
pub fn as_reply_json(text: &str, guided: bool) -> String {
    if !guided {
        if let Ok(serde_json::Value::Object(o)) = serde_json::from_str(text.trim()) {
            if o.get("text").is_some_and(serde_json::Value::is_string) {
                return text.trim().to_owned();
            }
        }
    }
    serde_json::json!({ "text": text }).to_string()
}

/// Maps a bridge failure for the cleanup core (names only, no text).
pub fn engine_error(e: ErrorCode) -> EngineError {
    match e {
        ErrorCode::Timeout => EngineError::Timeout,
        other => EngineError::Failed(format!("apple_{}", other.as_str())),
    }
}

/// Apple's on-device model, the same contract as the sidecar.
#[derive(Clone, Copy, Debug)]
pub struct AppleEngine {
    /// Show "Cleaning up…" on the pill when a request starts.
    pub pill: bool,
    pub guardrails: Guardrails,
    pub guided: bool,
}

impl AppleEngine {
    pub fn for_dictation() -> Self {
        Self {
            pill: true,
            guardrails: GUARDRAILS,
            guided: GUIDED,
        }
    }
}

impl Engine for AppleEngine {
    async fn complete(
        &self,
        request: ChatRequest,
        timeout: Duration,
    ) -> Result<String, EngineError> {
        if self.pill {
            crate::fork::pill::cleaning_up();
        }
        let id = fm::next_request_id();
        let mut cancel = CancelOnDrop { id, armed: true };
        let (guardrails, guided) = (self.guardrails, self.guided);
        REQUESTS.fetch_add(1, Ordering::Relaxed);
        let joined = tokio::task::spawn_blocking(move || {
            fm::complete(
                id,
                &Generation {
                    instructions: &request.system,
                    prompt: &request.user,
                    max_tokens: request.max_tokens,
                    timeout,
                    guardrails,
                    guided,
                },
            )
        })
        .await;
        cancel.armed = false;
        match joined {
            Ok(Ok(text)) => Ok(as_reply_json(&text, guided)),
            Ok(Err(e)) => {
                match e {
                    ErrorCode::Guardrail | ErrorCode::Refusal => {
                        REFUSALS.fetch_add(1, Ordering::Relaxed);
                    }
                    ErrorCode::RateLimited => {
                        RATE_LIMITED.fetch_add(1, Ordering::Relaxed);
                    }
                    _ => {}
                }
                Err(engine_error(e))
            }
            Err(_) => Err(EngineError::Failed("apple_join".into())),
        }
    }
}

/// Ten 50-word dictations (our own text, written for RS-08): fillers, a
/// self-repair, no punctuation, as a recognizer writes them. RS-08's bench
/// and the app self-check use them; never user text.
pub const SAMPLES_50: [&str; 10] = [
    "um so i wanted to give you a quick update on the garden project we finished the raised beds on saturday and the soil arrives next week uh the only open question is the watering system which we can sort out together when you are back from the trip in may",
    "okay so for the team lunch on friday i booked the italian place near the station for twelve people at one o'clock no wait half past twelve please let me know by thursday if you have any allergies or if you cannot make it so i can change the booking",
    "hi anna thanks for sending the draft so quickly i read it on the train this morning and i think the second section works really well but the introduction is a bit long um could you cut it to two short paragraphs and move the budget table to the end",
    "so the plan for tomorrow is uh we leave the house at seven drive to the coast and stop for breakfast on the way then we walk along the cliffs until noon and take the ferry back in the afternoon bring a warm jacket because it gets windy up there",
    "just a reminder that the library books are due back on monday i put them in the blue bag by the front door um there are six of them including the two cookbooks and the travel guide for portugal which we might want to borrow again in the summer",
    "i talked to the landlord this afternoon and he agreed to fix the heating before the end of the month uh he will send a technician on wednesday morning between nine and eleven so someone needs to be home then can you do that or should i take the morning off",
    "so about the conference talk i think we should open with the customer story then show the three numbers and finish with the demo um the whole thing has to fit in twenty minutes no actually eighteen because they want time for questions at the end of each session",
    "hey can you pick up a few things on your way home we need milk eggs bread and some apples and if they have the good coffee get two bags uh also the pharmacy called and your prescription is ready so maybe stop there first before it closes at six",
    "the new printer finally works i had to reinstall the driver and connect it to the office network again um it prints double sided now which should save a lot of paper the only problem is the scanner which still does not send anything to the shared folder",
    "okay notes from the call with the bakery they can deliver the cake on saturday at ten in the morning it will be chocolate with raspberry filling for about forty guests uh the price is a hundred and twenty euros and they want half of it paid in advance this week",
];

/// Env var that runs the Apple self-check (S-12b Mac test).
pub const SELFCHECK_ENV: &str = "WISPRBRO_SELFCHECK_APPLE";

/// With `WISPRBRO_SELFCHECK_APPLE=1` (or `=local`, which expects every take
/// on the local model instead of Apple's): once the app is ready, cleans
/// [`SAMPLES_50`] through the app's own plan and run (engine choice, prewarm,
/// guards, record, event) while the app stays in the background, and logs
/// `fork apple selfcheck: …` per take and a summary: `prewarmed` takes after
/// 20 s idle with the key-down prewarm and 1.5 s of "speech", then a
/// back-to-back `burst` (RS-08: `rateLimited` as a background app).
pub fn selfcheck<R: crate::fork::ForkRuntime>(app: &AppHandle<R>) {
    let expect_local = std::env::var(SELFCHECK_ENV).as_deref() == Ok("local");
    if !expect_local && !crate::utils::env_flag_enabled(SELFCHECK_ENV) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        use super::post_process::{plan, prewarm_apple, run, Source};
        let spoken = vec!["en".to_string()];
        tokio::time::sleep(Duration::from_secs(5)).await;
        let st = status();
        log::info!(
            "fork apple selfcheck: availability {:?}, role {:?}, context {} tokens",
            st.availability,
            st.role,
            st.context_tokens
        );
        let mut rows: Vec<(String, u32, bool)> = Vec::new();
        let mut engines: std::collections::BTreeMap<String, u32> = Default::default();
        let mut outcomes: std::collections::BTreeMap<String, u32> = Default::default();
        let rounds: [(&str, usize); 2] = [("prewarmed", 8), ("burst", 30)];
        for (phase, n) in rounds {
            for i in 0..n {
                let text = SAMPLES_50[i % SAMPLES_50.len()];
                if phase == "prewarmed" {
                    tokio::time::sleep(Duration::from_secs(20)).await;
                    prewarm_apple(&app);
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                }
                let t0 = Instant::now();
                let Some(p) = plan(
                    &app,
                    text,
                    Source::Dictation,
                    Vec::new(),
                    spoken.clone(),
                    None,
                )
                .await
                else {
                    log::error!("fork apple selfcheck: FAIL the fork does not own cleanup here");
                    return;
                };
                let (out, rec) = run(&app, text, &p).await;
                let ms = u32::try_from(t0.elapsed().as_millis()).unwrap_or(u32::MAX);
                *engines.entry(rec.engine.as_str().into()).or_default() += 1;
                *outcomes.entry(rec.outcome.as_str().into()).or_default() += 1;
                log::info!(
                    "fork apple selfcheck: {phase} {i}: engine {} {} {} ms{} -> {out:?}",
                    rec.engine.as_str(),
                    rec.outcome.as_str(),
                    ms,
                    rec.why
                        .map(|w| format!(" ({})", w.as_str()))
                        .unwrap_or_default()
                );
                rows.push((
                    phase.to_string(),
                    ms,
                    rec.engine == super::post_process::CleanupEngine::Apple,
                ));
            }
        }
        let pcts = |phase: &str| {
            let mut v: Vec<u32> = rows.iter().filter(|r| r.0 == phase).map(|r| r.1).collect();
            v.sort_unstable();
            let at = |p: f64| {
                v.get(((v.len().saturating_sub(1)) as f64 * p).round() as usize)
                    .copied()
                    .unwrap_or(0)
            };
            (at(0.5), at(0.95))
        };
        let st = status();
        let as_expected = rows.iter().all(|r| r.2 != expect_local);
        log::info!(
            "fork apple selfcheck: {} engines {engines:?} outcomes {outcomes:?}; prewarmed p50/p95 {:?} ms, burst p50/p95 {:?} ms; requests {} refusals {} rate_limited {}",
            if as_expected { "PASS" } else { "FAIL" },
            pcts("prewarmed"),
            pcts("burst"),
            st.requests,
            st.refusals,
            st.rate_limited
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn availability_reasons_map_for_the_ui() {
        use fm::Availability as A;
        for (a, want) in [
            (A::Available, AppleAvailability::Available),
            (A::DeviceNotEligible, AppleAvailability::DeviceNotEligible),
            (
                A::AppleIntelligenceNotEnabled,
                AppleAvailability::NotEnabled,
            ),
            (A::ModelNotReady, AppleAvailability::ModelNotReady),
            (A::UnsupportedSystem, AppleAvailability::Unsupported),
            (A::Unknown, AppleAvailability::Unknown),
        ] {
            assert_eq!(AppleAvailability::from(a), want);
        }
        assert_eq!(
            serde_json::to_string(&AppleAvailability::NotEnabled).unwrap(),
            "\"not_enabled\""
        );
    }

    #[test]
    fn replies_become_the_guard_chains_json() {
        assert_eq!(
            as_reply_json("Hi, there.", true),
            r#"{"text":"Hi, there."}"#
        );
        // A guided reply that looks like JSON is still the text.
        assert_eq!(
            as_reply_json(r#"{"text":"x"}"#, true),
            r#"{"text":"{\"text\":\"x\"}"}"#
        );
        assert_eq!(
            as_reply_json(" {\"text\": \"Hi.\"} ", false),
            "{\"text\": \"Hi.\"}"
        );
        assert_eq!(as_reply_json("Hi.", false), r#"{"text":"Hi."}"#);
        assert_eq!(
            as_reply_json("{\"answer\": 1}", false),
            r#"{"text":"{\"answer\": 1}"}"#
        );
    }

    #[test]
    fn errors_map_to_timeout_or_named_failures() {
        assert_eq!(engine_error(ErrorCode::Timeout), EngineError::Timeout);
        assert_eq!(
            engine_error(ErrorCode::RateLimited),
            EngineError::Failed("apple_rate_limited".into())
        );
        assert_eq!(
            engine_error(ErrorCode::Guardrail),
            EngineError::Failed("apple_guardrail".into())
        );
        assert_eq!(
            engine_error(ErrorCode::Cancelled),
            EngineError::Failed("apple_cancelled".into())
        );
    }

    #[test]
    fn language_codes_are_reduced_to_their_primary_tag() {
        assert_eq!(primary("nl-BE"), "nl");
        assert_eq!(primary("pt_BR"), "pt");
        assert_eq!(primary("fr"), "fr");
    }
}
