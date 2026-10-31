// SPDX-License-Identifier: GPL-3.0-or-later
//! [`clean`] with fake engines: levels, the gate, chunks, deadlines and
//! fail-open; the plan's AI-chat skip; the hooks in Handy's files.

use std::sync::atomic::AtomicUsize;
use std::sync::Mutex as StdMutex;

use super::*;

/// Replies from a function of the user turn's transcript.
struct Fake<F: Fn(&str) -> Result<String, EngineError> + Send + Sync> {
    reply: F,
    delay: Duration,
    calls: AtomicUsize,
    seen: StdMutex<Vec<(String, Duration)>>,
}

fn fake<F: Fn(&str) -> Result<String, EngineError> + Send + Sync>(reply: F) -> Fake<F> {
    Fake {
        reply,
        delay: Duration::ZERO,
        calls: AtomicUsize::new(0),
        seen: StdMutex::new(Vec::new()),
    }
}

fn transcript_of(user: &str) -> String {
    user.strip_prefix("<transcript>\n")
        .and_then(|s| s.split_once("\n</transcript>"))
        .map(|(t, _)| t.to_owned())
        .expect("OpenWhispr framing")
}

impl<F: Fn(&str) -> Result<String, EngineError> + Send + Sync> Engine for Fake<F> {
    async fn complete(
        &self,
        request: ChatRequest,
        timeout: Duration,
    ) -> Result<String, EngineError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let t = transcript_of(&request.user);
        self.seen.lock().unwrap().push((t.clone(), timeout));
        if !self.delay.is_zero() {
            if self.delay > timeout {
                tokio::time::sleep(timeout).await;
                return Err(EngineError::Timeout);
            }
            tokio::time::sleep(self.delay).await;
        }
        (self.reply)(&t)
    }
}

fn reply(text: &str) -> Result<String, EngineError> {
    Ok(serde_json::json!({ "text": text }).to_string())
}

fn req(text: &str, level: CleanupLevel) -> Request<'_> {
    Request {
        text,
        level,
        language: None,
        hint: None,
        terms: &[],
        keep: &[],
        style: None,
        context: &[],
    }
}

#[tokio::test]
async fn light_applies_a_guarded_cleanup() {
    let e = fake(|_| reply("Coffee at 3."));
    let c = clean(&e, &req("um coffee at 2 actually 3", CleanupLevel::Light)).await;
    assert_eq!(c.text, "Coffee at 3.");
    assert_eq!(c.outcome, CleanupOutcome::Applied);
    assert_eq!(c.why, None);
    assert_eq!(c.chunks.len(), 1);
    assert_eq!(c.chunks_applied(), 1);
}

#[tokio::test]
async fn level_none_and_empty_text_never_call_the_llm() {
    let e = fake(|_| reply("x"));
    let c = clean(&e, &req("um hello", CleanupLevel::None)).await;
    assert_eq!(
        (c.outcome, c.why),
        (CleanupOutcome::Skipped, Some(CleanupWhy::LevelNone))
    );
    assert_eq!(c.text, "um hello");
    let c = clean(&e, &req(" ... ", CleanupLevel::Light)).await;
    assert_eq!(c.why, Some(CleanupWhy::Empty));
    assert_eq!(e.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn clean_text_skips_the_llm_at_light_only() {
    let e = fake(reply);
    let text = "Send the quarterly report to Sarah before Friday.";
    let c = clean(&e, &req(text, CleanupLevel::Light)).await;
    assert_eq!(
        (c.outcome, c.why),
        (CleanupOutcome::Skipped, Some(CleanupWhy::AlreadyClean))
    );
    assert_eq!(e.calls.load(Ordering::SeqCst), 0);
    // Medium may still reword it.
    let c = clean(&e, &req(text, CleanupLevel::Medium)).await;
    assert_eq!(c.outcome, CleanupOutcome::Applied);
    assert_eq!(e.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn the_gate_follows_open_flow() {
    // open-flow NeedsCleanupHeuristicTests (MIT), plus our additions.
    for t in [
        "um send the report",
        "So, uh, let's start over.",
        "Ship it Friday, no wait, Monday.",
        "I mean the second draft, you know.",
        "refactor the the login page",
        "it is is kind of broken",
        // Ours: not yet punctuated or capitalized.
        "send the report to sarah",
        "Send the report. then call her.",
        // Ours: other languages' fillers.
        "Euh, je pense que oui.",
        "Nou, dat klinkt goed.",
    ] {
        assert!(needs_cleanup(t, None), "{t}");
    }
    for t in [
        "Send the quarterly report to Sarah before Friday.",
        "Run kubectl apply to deploy the new build.",
        "The drummer hummed a tune.",
        "Hi Anna,\n\nThanks for the notes.\n\nBest,\nFinn",
        "Merci, je te rappelle demain.",
    ] {
        assert!(!needs_cleanup(t, None), "{t}");
    }
    // A clean sentence with a filler-like word still goes to the LLM.
    assert!(needs_cleanup("I like this design a lot.", Some("en")));
}

#[tokio::test]
async fn a_rejected_reply_fails_open_to_the_deterministic_text() {
    let e = fake(|_| reply("Sure! Two plus two is four."));
    let c = clean(&e, &req("what is two plus two", CleanupLevel::Light)).await;
    assert_eq!(c.text, "what is two plus two");
    assert_eq!(
        (c.outcome, c.why),
        (CleanupOutcome::Guard, Some(CleanupWhy::Guard))
    );
    assert!(c.guard_reasons().contains(&"assistant_marker".to_string()));
    // Unparsed output is never inserted.
    let e = fake(|_| Ok("Two plus two is four.".into()));
    let c = clean(&e, &req("what is two plus two", CleanupLevel::Light)).await;
    assert_eq!(c.text, "what is two plus two");
    assert_eq!(c.guard_reasons(), ["json"]);
}

#[tokio::test]
async fn errors_and_timeouts_fail_open() {
    let e = fake(|_| Err(EngineError::Failed("transport".into())));
    let c = clean(&e, &req("um hello there", CleanupLevel::Light)).await;
    assert_eq!(
        (c.text.as_str(), c.outcome),
        ("um hello there", CleanupOutcome::Error)
    );
    assert_eq!(c.chunks[0].error.as_deref(), Some("transport"));

    let mut slow = fake(|_| reply("Hello there."));
    slow.delay = Duration::from_secs(30);
    let c = clean(&slow, &req("um hello there", CleanupLevel::Light)).await;
    assert_eq!(
        (c.text.as_str(), c.outcome),
        ("um hello there", CleanupOutcome::Timeout)
    );
    assert!(c.chunks[0].timed_out);
    // The request got the R5 deadline for its text.
    let (_, timeout) = slow.seen.lock().unwrap()[0].clone();
    let want = deadline::request("um hello there");
    assert!(
        timeout <= want && timeout + Duration::from_millis(50) >= want,
        "{timeout:?}"
    );
}

#[test]
fn chunks_cut_at_lines_then_sentences_and_join_back() {
    let joins = |cs: &[Piece]| {
        cs.iter()
            .map(|p| format!("{}{}", p.text, p.separator))
            .collect::<String>()
    };
    let short = "um a short take";
    assert_eq!(chunks(short).len(), 1);
    let para = |n: usize, w: &str| vec![w; n].join(" ") + ".";
    // Every line is its own piece, short ones too (the model joins lines).
    let text = format!("{}\n\n{}\n{}", para(40, "a"), para(40, "b"), para(70, "c"));
    let cs = chunks(&text);
    assert_eq!(cs.len(), 3, "{cs:?}");
    assert_eq!(cs[0].separator, "\n\n");
    assert_eq!(cs[1].separator, "\n");
    assert_eq!(joins(&cs), text);
    let letter = "dear landlord\n\num the heating is broken\n\nthanks\nalex";
    let cs = chunks(letter);
    assert_eq!(
        cs.iter().map(|p| p.text.as_str()).collect::<Vec<_>>(),
        [
            "dear landlord",
            "um the heating is broken",
            "thanks",
            "alex"
        ]
    );
    assert_eq!(joins(&cs), letter);
    // One long line is cut at sentence ends and joined back while it fits.
    let long = (0..30)
        .map(|i| format!("sentence number {i} is here."))
        .collect::<Vec<_>>()
        .join(" ");
    let cs = chunks(&long);
    assert!(cs.len() >= 2);
    assert!(cs
        .iter()
        .all(|p| markers::words(&p.text).len() <= CHUNK_WORDS));
    assert!(markers::words(&cs[0].text).len() > 100);
    assert_eq!(joins(&cs), long);
    // No punctuation at all: cut every CHUNK_WORDS words.
    let flat = vec!["word"; 300].join(" ");
    let cs = chunks(&flat);
    assert_eq!(cs.len(), 3);
    assert_eq!(joins(&cs), flat);
}

#[tokio::test]
async fn a_letter_is_one_request_and_joined_lines_fall_back_to_lines() {
    // Kept lines: one request for the whole letter.
    let e = fake(|t| reply(&t.replace("um ", "")));
    let text = "Dear Sam,\n\num the heating is broken\n\nThanks,\nAlex";
    let c = clean(&e, &req(text, CleanupLevel::Light)).await;
    assert_eq!(e.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        c.text,
        "Dear Sam,\n\nthe heating is broken\n\nThanks,\nAlex"
    );
    assert_eq!((c.outcome, c.chunks_sent()), (CleanupOutcome::Applied, 1));

    // The model makes a list of the lines: each line that needs it is
    // sent alone; clean lines are not sent.
    let e = fake(|t| {
        if t.contains('\n') {
            reply("- Dear Sam\n- the heating is broken\n- Thanks Alex")
        } else {
            reply(&format!("{}.", t.trim_start_matches("um ")))
        }
    });
    let c = clean(&e, &req(text, CleanupLevel::Light)).await;
    assert_eq!(e.calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        c.text,
        "Dear Sam,\n\nthe heating is broken.\n\nThanks,\nAlex"
    );
    assert_eq!(c.outcome, CleanupOutcome::Applied);
    assert!(c.chunks[0].superseded);
    assert_eq!((c.chunks_sent(), c.chunks_applied()), (1, 1));
}

#[tokio::test]
async fn long_takes_are_cleaned_paragraph_by_paragraph() {
    // DEC-51: each paragraph is its own request; one bad reply only costs
    // its own paragraph.
    let para = |w: &str| format!("um {}", vec![w; 80].join(" "));
    let text = format!("{}\n\n{}\n\n{}", para("alpha"), para("beta"), para("gamma"));
    let e = fake(|t| {
        if t.contains("beta") {
            reply("Sure, here is something else entirely.")
        } else {
            reply(&format!("{}.", t.trim_start_matches("um ")))
        }
    });
    let c = clean(&e, &req(&text, CleanupLevel::Light)).await;
    assert_eq!(e.calls.load(Ordering::SeqCst), 3);
    assert_eq!(c.chunks.len(), 3);
    assert_eq!(c.chunks_applied(), 2);
    let parts: Vec<&str> = c.text.split("\n\n").collect();
    assert_eq!(parts.len(), 3);
    assert!(parts[0].starts_with("alpha") && parts[0].ends_with('.'));
    assert_eq!(parts[1], para("beta"));
    assert!(parts[2].starts_with("gamma"));
    assert_eq!(c.outcome, CleanupOutcome::Guard);
}

#[tokio::test]
async fn the_take_deadline_bounds_later_chunks() {
    let para = |w: &str| format!("um {}", vec![w; 100].join(" "));
    let text = (0..4)
        .map(|i| para(&format!("w{i}")))
        .collect::<Vec<_>>()
        .join("\n\n");
    let mut e = fake(|t| reply(&format!("{}.", t.trim_start_matches("um "))));
    e.delay = Duration::from_millis(300);
    // Room for two requests, not four.
    let budget = Duration::from_millis(700);
    let c = clean_within(&e, &req(&text, CleanupLevel::Light), budget).await;
    assert_eq!(c.chunks.len(), 4);
    assert_eq!(c.chunks_applied(), 2, "{:?}", c.chunks);
    assert!(c.chunks[2].timed_out && c.chunks[3].timed_out);
    // The last paragraphs are the deterministic text.
    assert!(c.text.ends_with(&para("w3")));
    assert!(Duration::from_millis(u64::from(c.llm_ms)) < budget + Duration::from_millis(400));
    assert_eq!(c.outcome, CleanupOutcome::Timeout);
}

#[tokio::test]
async fn medium_and_terms_reach_the_prompt() {
    struct Spy(StdMutex<String>);
    impl Engine for Spy {
        async fn complete(&self, r: ChatRequest, _: Duration) -> Result<String, EngineError> {
            *self.0.lock().unwrap() = r.system.clone();
            reply("Hello Wispr Bro.")
        }
    }
    let spy = Spy(StdMutex::new(String::new()));
    let terms = vec!["Wispr Bro".to_string()];
    let c = clean(
        &spy,
        &Request {
            text: "um hello wispr bro",
            level: CleanupLevel::Medium,
            language: Some("nl"),
            hint: None,
            terms: &terms,
            keep: &terms,
            style: None,
            context: &[],
        },
    )
    .await;
    assert_eq!(c.outcome, CleanupOutcome::Applied);
    let system = spy.0.lock().unwrap().clone();
    assert!(system.contains("LEVEL MEDIUM"));
    assert!(system.contains("spell them exactly as written here: Wispr Bro."));
    assert!(system.contains("Dutch (nl)"));
}

#[test]
fn notices_and_names() {
    for w in [
        CleanupWhy::NotReady,
        CleanupWhy::LowMemory,
        CleanupWhy::Guard,
        CleanupWhy::Timeout,
        CleanupWhy::Error,
    ] {
        assert!(w.is_notice(), "{w:?}");
    }
    for w in [
        CleanupWhy::LevelNone,
        CleanupWhy::Empty,
        CleanupWhy::AlreadyClean,
        CleanupWhy::AiChat,
    ] {
        assert!(!w.is_notice(), "{w:?}");
    }
    // DB CHECK values (0001_init.sql).
    let sql = include_str!("../../db/migrations/0001_init.sql");
    for o in [
        CleanupOutcome::Applied,
        CleanupOutcome::Skipped,
        CleanupOutcome::Guard,
        CleanupOutcome::Timeout,
        CleanupOutcome::Error,
    ] {
        assert!(sql.contains(&format!("'{}'", o.as_str())), "{o:?}");
        assert_eq!(
            serde_json::to_value(o).unwrap(),
            serde_json::json!(o.as_str())
        );
    }
    for e in [
        CleanupEngine::None,
        CleanupEngine::Local,
        CleanupEngine::Apple,
    ] {
        assert!(sql.contains(&format!("'{}'", e.as_str())), "{e:?}");
    }
    for w in [
        CleanupWhy::AiChat,
        CleanupWhy::AlreadyClean,
        CleanupWhy::NotReady,
    ] {
        assert_eq!(
            serde_json::to_value(w).unwrap(),
            serde_json::json!(w.as_str())
        );
    }
}

#[test]
fn ai_chat_apps_are_classified() {
    use crate::fork::context::categories::classify;
    assert!(classify(Some("com.openai.chat"), Some("ChatGPT")).is_ai_chat);
    assert!(!classify(Some("com.apple.TextEdit"), Some("TextEdit")).is_ai_chat);
}

#[test]
fn fixes_count_runs_of_word_edits() {
    assert_eq!(fix_count("um so coffee at 2 actually 3", "Coffee at 3."), 2);
    assert_eq!(fix_count("Hello there.", "Hello there."), 0);
}

#[test]
fn the_handy_hooks_call_the_fork() {
    let actions = include_str!("../../../actions.rs");
    assert!(actions.contains("crate::fork::llm::post_process(&ah, &transcription, post_process)"));
    assert!(!actions.contains("                                process_transcription_output(&ah"));
    let jobs = include_str!("../../../managers/transcription_jobs.rs");
    assert!(jobs.contains("crate::fork::llm::post_process_job("));
    // Handy's own function stays, for headless runs and plain Handy builds.
    assert!(actions.contains("pub(crate) async fn process_transcription_output("));
}

#[test]
fn joined_lines_are_put_back_where_they_were() {
    assert_eq!(
        realign_lines(
            "dear landlord\n\num the heating is broken again\n\nthanks\nalex",
            "Dear landlord, the heating is broken again. Thanks, Alex."
        )
        .as_deref(),
        Some("Dear landlord,\n\nthe heating is broken again.\n\nThanks,\nAlex.")
    );
    assert_eq!(
        realign_lines(
            "hallo zusammen\näh das treffen fällt aus\nbis morgen",
            "Hallo zusammen, das Treffen fällt aus, bis morgen."
        )
        .as_deref(),
        Some("Hallo zusammen,\ndas Treffen fällt aus,\nbis morgen.")
    );
    // A line with no word left cannot be anchored.
    assert_eq!(
        realign_lines("todo\num\nwrite tests", "Todo: write tests."),
        None
    );
    // Nothing to do when the lines are there.
    assert_eq!(realign_lines("a b\nc d", "A b.\nC d."), None);
    assert_eq!(realign_lines("one line", "One line."), None);
}

#[tokio::test]
async fn a_joined_reply_is_realigned_instead_of_resent() {
    let e = fake(|t| {
        if t.contains('\n') {
            reply("Dear landlord, the heating is broken again. Thanks, Alex.")
        } else {
            reply("unexpected")
        }
    });
    let text = "dear landlord\n\num the heating is broken again\n\nthanks\nalex";
    let c = clean(&e, &req(text, CleanupLevel::Light)).await;
    assert_eq!(e.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        c.text,
        "Dear landlord,\n\nthe heating is broken again.\n\nThanks,\nAlex."
    );
    assert!(c.chunks[0].realigned && !c.chunks[0].superseded);
    assert_eq!(c.outcome, CleanupOutcome::Applied);
}

#[test]
fn ai_chat_targets_skip_the_llm_first() {
    assert_eq!(skip_reason(true, true, true), Some(CleanupWhy::AiChat));
    assert_eq!(skip_reason(true, false, false), Some(CleanupWhy::AiChat));
    assert_eq!(skip_reason(false, false, true), Some(CleanupWhy::NotReady));
    assert_eq!(skip_reason(false, true, false), Some(CleanupWhy::LowMemory));
    assert_eq!(skip_reason(false, true, true), None);
}

#[test]
fn dictionary_spellings_survive_the_llm() {
    use crate::fork::context::take::ledger;
    use crate::fork::text::counts::{RuleHit, TextRecord};
    use crate::fork::text::rules::Provenance;
    // The pipeline kept "Wispr Bro" from the rule "whisper bro".
    let kept = "um I use Wispr Bro daily s12a-stage9";
    let mut record = TextRecord::default();
    record.rule_hits.push(RuleHit {
        rule_id: 1,
        from_text: "whisper bro".into(),
        to_text: "Wispr Bro".into(),
    });
    record.provenance.push(Provenance {
        source_form: "whisper bro".into(),
        target_text: "Wispr Bro".into(),
        target_start: 9,
    });
    ledger().note_processed(
        "um i use whisper bro daily s12a-stage9".into(),
        kept,
        record,
    );
    // The LLM wrote the recognizer's spelling back.
    let out = restore_spellings(kept, "I use Whisper Bro daily s12a-stage9.".into());
    assert_eq!(out, "I use Wispr Bro daily s12a-stage9.");
    // Without a registered rule, nothing changes.
    assert_eq!(
        restore_spellings("plain text here", "Plain text here.".into()),
        "Plain text here."
    );
}

// S-12b: the engine choice and Apple's replies through the same guards.

#[test]
fn the_engine_follows_rs08s_role() {
    use super::super::apple::AppleRole::{Default, Fallback};
    let (l, a, n) = (
        CleanupEngine::Local,
        CleanupEngine::Apple,
        CleanupEngine::None,
    );
    // (role, ai_chat, ready, memory_ok, apple_ok) -> (skip, engine)
    for (role, ai, ready, mem, apple, want) in [
        // An AI chat never gets an engine.
        (
            Fallback,
            true,
            true,
            true,
            true,
            (Some(CleanupWhy::AiChat), n),
        ),
        (
            Default,
            true,
            true,
            true,
            true,
            (Some(CleanupWhy::AiChat), n),
        ),
        // Fallback: the sidecar first, Apple only when it cannot run.
        (Fallback, false, true, true, true, (None, l)),
        (Fallback, false, false, true, true, (None, a)),
        (Fallback, false, true, false, true, (None, a)),
        (
            Fallback,
            false,
            false,
            true,
            false,
            (Some(CleanupWhy::NotReady), n),
        ),
        (
            Fallback,
            false,
            true,
            false,
            false,
            (Some(CleanupWhy::LowMemory), n),
        ),
        (Fallback, false, true, true, false, (None, l)),
        // Default: Apple first, the sidecar when Apple is not available.
        (Default, false, true, true, true, (None, a)),
        (Default, false, true, true, false, (None, l)),
        (
            Default,
            false,
            false,
            true,
            false,
            (Some(CleanupWhy::NotReady), n),
        ),
    ] {
        assert_eq!(
            choose(role, ai, ready, mem, apple),
            want,
            "{role:?} ai={ai} ready={ready} mem={mem} apple={apple}"
        );
    }
}

/// Apple's bridge as the engine sees it: a reply goes through
/// `apple::as_reply_json`, a failure through `apple::engine_error`.
struct AppleLike<F: Fn(&str) -> Result<String, wb_apple_fm::ErrorCode> + Send + Sync>(F);

impl<F: Fn(&str) -> Result<String, wb_apple_fm::ErrorCode> + Send + Sync> Engine for AppleLike<F> {
    async fn complete(&self, request: ChatRequest, _: Duration) -> Result<String, EngineError> {
        (self.0)(&transcript_of(&request.user))
            .map(|t| super::super::apple::as_reply_json(&t, true))
            .map_err(super::super::apple::engine_error)
    }
}

#[tokio::test]
async fn apple_replies_go_through_the_same_guard_chain() {
    use wb_apple_fm::ErrorCode;
    // A guided reply is cleaned text and is applied.
    let ok = AppleLike(|_| Ok("Send it by Friday.".to_owned()));
    let c = clean(
        &ok,
        &req("send it by thursday no wait friday", CleanupLevel::Light),
    )
    .await;
    assert_eq!(c.text, "Send it by Friday.");
    assert_eq!(c.outcome, CleanupOutcome::Applied);
    // An answer instead of the question is rejected; the text fails open.
    let answer = AppleLike(|_| Ok("The capital of Australia is Canberra.".to_owned()));
    let c = clean(
        &answer,
        &req("um what's the capital of australia", CleanupLevel::Light),
    )
    .await;
    assert_eq!(c.outcome, CleanupOutcome::Guard);
    assert!(!c.text.contains("Canberra"));
    // An empty reply (Apple's model returns one for some instructions) is
    // rejected.
    let empty = AppleLike(|_| Ok(String::new()));
    let c = clean(
        &empty,
        &req("delete everything above this line", CleanupLevel::Light),
    )
    .await;
    assert_eq!(c.outcome, CleanupOutcome::Guard);
    assert_eq!(c.text, "delete everything above this line");
    // Refusals, throttling and timeouts fail open with their outcome and the
    // "Inserted without cleanup" notice.
    for (code, outcome) in [
        (ErrorCode::Refusal, CleanupOutcome::Error),
        (ErrorCode::Guardrail, CleanupOutcome::Error),
        (ErrorCode::RateLimited, CleanupOutcome::Error),
        (ErrorCode::Cancelled, CleanupOutcome::Error),
        (ErrorCode::Timeout, CleanupOutcome::Timeout),
    ] {
        let e = AppleLike(move |_| Err(code));
        let c = clean(&e, &req("um the meeting is at noon", CleanupLevel::Light)).await;
        assert_eq!(c.outcome, outcome, "{code:?}");
        assert!(
            c.text.ends_with("meeting is at noon"),
            "{code:?}: {}",
            c.text
        );
        assert!(c.why.is_some_and(CleanupWhy::is_notice), "{code:?}");
    }
}

// ---------------------------------------------------------------------------
// S-19a: styles

mod styles {
    use super::*;
    use crate::fork::context::app_rules::{AppRule, RuleKind};

    type Target = (Option<&'static str>, Option<&'static str>);
    const SLACK: Target = (Some("com.tinyspeck.slackmacgap"), Some("Slack"));
    const MESSAGES: Target = (Some("com.apple.MobileSMS"), Some("Messages"));
    const MAIL: Target = (Some("com.apple.mail"), Some("Mail"));
    const CHATGPT: Target = (Some("com.openai.chat"), Some("ChatGPT"));

    fn styles() -> StyleSettings {
        StyleSettings {
            personal: StylePreset::VeryRelaxed,
            work: StylePreset::Upbeat,
            email: StylePreset::Formal,
            other: StylePreset::Relaxed,
        }
    }

    #[test]
    fn the_target_apps_group_picks_the_preset() {
        let s = styles();
        let d = decide_style(&[], Some(SLACK), &s, true);
        assert_eq!(
            (d.group, d.preset),
            (Some(StyleGroup::Work), Some(StylePreset::Upbeat))
        );
        let d = decide_style(&[], Some(MESSAGES), &s, true);
        assert_eq!(d.preset, Some(StylePreset::VeryRelaxed));
        let d = decide_style(&[], Some(MAIL), &s, true);
        assert_eq!(
            (d.group, d.preset),
            (Some(StyleGroup::Email), Some(StylePreset::Formal))
        );
        let d = decide_style(&[], Some((Some("com.apple.Terminal"), None)), &s, true);
        assert_eq!(d.preset, Some(StylePreset::Relaxed));
        // The user's rule wins over the built-in table.
        let rules = vec![AppRule {
            key: "com.tinyspeck.slackmacgap".into(),
            kind: RuleKind::App,
            style_group: Some(StyleGroup::Personal),
            auto_send: true,
            app_name: None,
            updated_at: 0,
        }];
        let d = decide_style(&rules, Some(SLACK), &s, true);
        assert_eq!(d.preset, Some(StylePreset::VeryRelaxed));
        assert_eq!(d.auto_send.as_deref(), Some("com.tinyspeck.slackmacgap"));
        assert_eq!(decide_style(&rules, Some(MAIL), &s, true).auto_send, None);
    }

    #[test]
    fn english_only_and_never_in_ai_chats_or_without_a_target() {
        let s = styles();
        let d = decide_style(&[], Some(SLACK), &s, false);
        assert_eq!((d.group, d.preset), (Some(StyleGroup::Work), None));
        assert_eq!(decide_style(&[], Some(CHATGPT), &s, true).preset, None);
        assert_eq!(decide_style(&[], None, &s, true), StyleDecision::default());
    }

    #[test]
    fn stage_10_runs_on_the_cleaned_text() {
        let d = StyleDecision {
            group: Some(StyleGroup::Personal),
            preset: Some(StylePreset::VeryRelaxed),
            auto_send: None,
        };
        let keep = vec!["Wispr Bro".to_string()];
        assert_eq!(
            apply_style(&d, "Hey, Wispr Bro is out. See you later.".into(), &keep),
            "hey Wispr Bro is out. see you later"
        );
        assert_eq!(
            apply_style(&StyleDecision::default(), "Hey, there.".into(), &keep),
            "Hey, there."
        );
    }

    /// RV-48: auto-send is armed only when the cleanup did not fall back.
    #[test]
    fn auto_send_never_after_a_fallback() {
        let d = StyleDecision {
            group: Some(StyleGroup::Work),
            preset: Some(StylePreset::Formal),
            auto_send: Some("com.tinyspeck.slackmacgap".into()),
        };
        assert_eq!(
            auto_send_target(&d, None),
            Some("com.tinyspeck.slackmacgap")
        );
        for ok in [
            CleanupWhy::LevelNone,
            CleanupWhy::AlreadyClean,
            CleanupWhy::Empty,
        ] {
            assert!(auto_send_target(&d, Some(ok)).is_some(), "{ok:?}");
        }
        for fallback in [
            CleanupWhy::Guard,
            CleanupWhy::Timeout,
            CleanupWhy::Error,
            CleanupWhy::NotReady,
            CleanupWhy::LowMemory,
        ] {
            assert_eq!(auto_send_target(&d, Some(fallback)), None, "{fallback:?}");
        }
        assert_eq!(auto_send_target(&StyleDecision::default(), None), None);
    }

    /// Every preset sends the level's own prompt (RS-07's for Light), and
    /// a cleaned reply with the preset's punctuation still passes the
    /// guards (an "!" is not new content).
    #[tokio::test]
    async fn styles_send_the_levels_prompt_and_pass_the_guards() {
        struct Spy(StdMutex<String>);
        impl Engine for Spy {
            async fn complete(
                &self,
                request: ChatRequest,
                _timeout: Duration,
            ) -> Result<String, EngineError> {
                *self.0.lock().unwrap() = request.system.clone();
                reply("Great work on the launch today! Thanks for staying late!")
            }
        }
        for style in [
            StylePreset::Formal,
            StylePreset::Relaxed,
            StylePreset::VeryRelaxed,
            StylePreset::Upbeat,
        ] {
            let spy = Spy(StdMutex::new(String::new()));
            let mut r = req(
                "um great work on the launch today thanks for staying late",
                CleanupLevel::Light,
            );
            r.style = Some(style);
            let c = clean(&spy, &r).await;
            assert_eq!(
                c.outcome,
                CleanupOutcome::Applied,
                "{style:?} {:?}",
                c.guard_reasons()
            );
            assert_eq!(*spy.0.lock().unwrap(), prompt::cleanup_system());
        }
    }
}
