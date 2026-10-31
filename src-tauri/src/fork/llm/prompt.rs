// SPDX-License-Identifier: GPL-3.0-or-later
//
// The Light prompt (`prompts/cleanup.txt`) is OpenWhispr's `cleanupPrompt`:
//   Source: OpenWhispr, https://github.com/OpenWhispr/openwhispr
//   Commit: b9e9ebefac4b961456c269164a93ddfdd53438b9
//   File:   src/locales/en/prompts.json, key "cleanupPrompt" (English)
//   Copyright (c) 2024 OpenWhispr Team. Licence: MIT
//   (LICENSES/MIT-OpenWhispr.txt).
//   Modified by Wispr Bro contributors, 2026: `{{agentName}}` replaced with
//   "Assistant" (OpenWhispr's default); one line asking for the JSON reply
//   appended (RS-07 "json" format). The user message is OpenWhispr's
//   `wrapCleanupTranscript`. The Medium, vocabulary and language fragments
//   (`prompts/{medium,vocabulary,language}.txt`) are our own text.
//
//! The prompt builder (R5 §4.5 W-03, S-12a): one builder for the cleanup
//! level, the user's dictionary terms and the take's language. DEC-03: Muesli
//! 2B v12 with the OpenWhispr prompt and a JSON schema (the RS-07 winner);
//! Light without terms and language is exactly what RS-07 measured.
//!
//! Framing: the transcript goes between `<transcript>` tags in the user turn
//! (OpenWhispr's framing, measured in RS-07 with Muesli), with any such tags
//! inside the transcript removed first so dictated text cannot close the
//! section (BetterWispr `TranscriptPolisher.delimited`). R5 also names
//! FluidVoice's JSON-field framing; it was not adopted, because Muesli was
//! fine-tuned on and measured with OpenWhispr's.

use serde_json::{json, Value};

use crate::fork::settings::CleanupLevel;

/// The body of a prompt file: everything after the first `---` line.
fn body(file: &'static str) -> &'static str {
    file.split_once("\n---\n")
        .map(|(_, b)| b)
        .unwrap_or(file)
        .trim()
}

/// OpenWhispr's `cleanupPrompt`, byte-identical to
/// `eval/cleanup/prompts/openwhispr.txt` (the RS-07 input).
pub static OPENWHISPR_CLEANUP: once_cell::sync::Lazy<&'static str> =
    once_cell::sync::Lazy::new(|| {
        let file = include_str!("prompts/cleanup.txt");
        file.split_once("\n---\n")
            .map(|(_, b)| b.trim_end_matches('\n'))
            .unwrap_or(file)
    });

fn medium_fragment() -> &'static str {
    body(include_str!("prompts/medium.txt"))
}

fn vocabulary_fragment() -> &'static str {
    body(include_str!("prompts/vocabulary.txt"))
}

fn language_fragment() -> &'static str {
    body(include_str!("prompts/language.txt"))
}

/// Our line appended for the JSON reply (RS-07 `JSON_SUFFIX`).
pub const JSON_SUFFIX: &str = "\n\nReply only with JSON: {\"text\": \"<the cleaned text>\"}.";

/// At most this many dictionary terms go into the prompt.
pub const MAX_TERMS: usize = 40;

/// What a cleanup request is built from.
#[derive(Debug, Clone, Default)]
pub struct Spec<'a> {
    pub level: CleanupLevel,
    /// ISO 639-1 code of the take, when known.
    pub language: Option<&'a str>,
    /// The user's dictionary terms (spellings to keep).
    pub terms: &'a [String],
}

/// English name of a language for the prompt ("Dutch (nl)"), or the code.
pub fn language_label(code: &str) -> String {
    let primary = code
        .split(['-', '_'])
        .next()
        .unwrap_or(code)
        .to_ascii_lowercase();
    let name = match primary.as_str() {
        "en" => "English",
        "nl" => "Dutch",
        "fr" => "French",
        "de" => "German",
        "es" => "Spanish",
        "it" => "Italian",
        "pt" => "Portuguese",
        "sv" => "Swedish",
        "pl" => "Polish",
        "da" => "Danish",
        "no" | "nb" => "Norwegian",
        "fi" => "Finnish",
        "cs" => "Czech",
        "ru" => "Russian",
        "uk" => "Ukrainian",
        "tr" => "Turkish",
        "ja" => "Japanese",
        "zh" => "Chinese",
        "ko" => "Korean",
        _ => return primary,
    };
    format!("{name} ({primary})")
}

/// `true` for English (or an empty code): the model's home language gets
/// no language fragment, which keeps English exactly what RS-07 measured.
fn is_english(code: &str) -> bool {
    let primary = code.split(['-', '_']).next().unwrap_or(code).trim();
    primary.is_empty() || primary.eq_ignore_ascii_case("en")
}

/// The language's filler words (owner policy, `tables/markers.json`), quoted
/// and comma-separated: discourse markers first, then hesitations.
pub fn filler_list(code: &str) -> String {
    let t = super::markers::TABLES.for_language(Some(code));
    let mut seen: Vec<String> = Vec::new();
    for p in t.fillers.iter().chain(t.hesitations.iter()) {
        let w = p.join(" ");
        if !seen.contains(&w) {
            seen.push(w);
        }
    }
    seen.iter()
        .map(|w| format!("\"{w}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Terms safe to put in the prompt: one line each, no tags or quotes that
/// could change the prompt's structure, ≤ 60 characters, no duplicates.
pub fn prompt_terms(terms: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in terms {
        let clean: String = t
            .chars()
            .filter(|c| !c.is_control() && !matches!(c, '<' | '>' | '"' | '{' | '}'))
            .collect();
        let clean = clean.trim().to_string();
        if clean.is_empty() || clean.chars().count() > 60 {
            continue;
        }
        if out.iter().any(|o| o.eq_ignore_ascii_case(&clean)) {
            continue;
        }
        out.push(clean);
        if out.len() == MAX_TERMS {
            break;
        }
    }
    out
}

/// The system message.
pub fn system(spec: &Spec<'_>) -> String {
    let mut s = String::from(*OPENWHISPR_CLEANUP);
    if spec.level == CleanupLevel::Medium {
        s.push_str("\n\n");
        s.push_str(medium_fragment());
    }
    let terms = prompt_terms(spec.terms);
    if !terms.is_empty() {
        s.push_str("\n\n");
        s.push_str(&vocabulary_fragment().replace("{{terms}}", &terms.join(", ")));
    }
    if let Some(lang) = spec.language.filter(|l| !is_english(l)) {
        s.push_str("\n\n");
        s.push_str(
            &language_fragment()
                .replace("{{language}}", &language_label(lang))
                .replace("{{fillers}}", &filler_list(lang)),
        );
    }
    s.push_str(JSON_SUFFIX);
    s
}

/// The system message for a plain Light cleanup (what RS-07 measured).
pub fn cleanup_system() -> String {
    system(&Spec::default())
}

/// Removes the framing tags from dictated text so it cannot close the
/// `<transcript>` section early (BetterWispr `delimited`).
pub fn delimited(transcript: &str) -> String {
    static TAGS: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"(?i)</?\s*transcript\s*>").expect("static regex")
    });
    TAGS.replace_all(transcript, "").into_owned()
}

/// OpenWhispr's `wrapCleanupTranscript` user message.
pub fn cleanup_user(transcript: &str) -> String {
    format!(
        "<transcript>\n{}\n</transcript>\n\nOutput only the cleaned transcript.",
        delimited(transcript)
    )
}

/// The `response_format` llama-server turns into a grammar.
pub fn response_format() -> Value {
    json!({
        "type": "json_schema",
        "json_schema": {
            "name": "cleanup",
            "strict": true,
            "schema": {
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"],
                "additionalProperties": false
            }
        }
    })
}

/// Output budget for a transcript (RS-07: 64 + chars/2, at most 2048).
pub fn max_tokens(transcript: &str) -> u32 {
    let chars = u32::try_from(transcript.chars().count()).unwrap_or(u32::MAX);
    64u32.saturating_add(chars.div_ceil(2)).min(2048)
}

/// R5 §4.5 deadline for one request: see [`super::deadline::request`].
pub fn deadline(transcript: &str) -> std::time::Duration {
    super::deadline::request(transcript)
}

/// The cleaned text from a JSON reply, or `None` (never paste unparsed
/// output, R5 §4.5).
pub fn parse_reply(content: &str) -> Option<String> {
    let v: Value = serde_json::from_str(content.trim()).ok()?;
    Some(v.get("text")?.as_str()?.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_light_prompt_is_the_rs07_input_byte_for_byte() {
        let file = include_str!("../../../../eval/cleanup/prompts/openwhispr.txt");
        let body = file.split_once("\n---\n").expect("--- separator").1;
        assert_eq!(body.trim_end_matches('\n'), *OPENWHISPR_CLEANUP);
        // Same suffix and framing as scripts/wb-bench-cleanup.ts.
        let bench = include_str!("../../../../scripts/wb-bench-cleanup.ts");
        assert!(bench.contains(r#"'\n\nReply only with JSON: {"text": "<the cleaned text>"}.'"#));
        assert!(bench.contains(
            r#"`<transcript>\n${t}\n</transcript>\n\nOutput only the cleaned transcript.`"#
        ));
        assert_eq!(
            cleanup_system(),
            format!("{}{JSON_SUFFIX}", *OPENWHISPR_CLEANUP)
        );
    }

    #[test]
    fn levels_terms_and_language_snapshot() {
        let terms = vec!["Wispr Bro".to_string(), "Anna-Lena".to_string()];
        let light = system(&Spec {
            level: CleanupLevel::Light,
            language: Some("nl"),
            terms: &terms,
        });
        let tail = light.strip_prefix(*OPENWHISPR_CLEANUP).unwrap();
        assert_eq!(
            tail,
            "\n\nVOCABULARY: when these words occur, spell them exactly as written here: Wispr Bro, Anna-Lena.\
             \n\nLANGUAGE: the transcript is in Dutch (nl). Write the cleaned text in that language; never translate it. \
             In this language these words are fillers when they only start a sentence or fill a pause and carry no meaning; \
             remove them: \"zeg maar\", \"nou\", \"dus\", \"eh\", \"ehm\", \"ja\", \"uh\", \"uhm\", \"hm\", \"hmm\", \"euh\".\
             \n\nReply only with JSON: {\"text\": \"<the cleaned text>\"}."
        );
        // English gets no language fragment (RS-07 measured it without).
        let en = system(&Spec {
            level: CleanupLevel::Light,
            language: Some("en-GB"),
            terms: &[],
        });
        assert_eq!(en, cleanup_system());
        let medium = system(&Spec {
            level: CleanupLevel::Medium,
            ..Spec::default()
        });
        let tail = medium.strip_prefix(*OPENWHISPR_CLEANUP).unwrap();
        assert!(tail.starts_with("\n\nLEVEL MEDIUM: besides the cleanup above"));
        assert!(tail.ends_with(JSON_SUFFIX));
        assert!(!tail.contains("VOCABULARY") && !tail.contains("LANGUAGE:"));
    }

    #[test]
    fn terms_are_sanitized_deduplicated_and_capped() {
        let raw: Vec<String> = vec![
            "Wispr Bro".into(),
            "wispr bro".into(),
            "  ".into(),
            "a<transcript>b".into(),
            "line\nbreak".into(),
            "x".repeat(61),
        ];
        assert_eq!(
            prompt_terms(&raw),
            ["Wispr Bro", "atranscriptb", "linebreak"]
        );
        let many: Vec<String> = (0..100).map(|i| format!("term{i}")).collect();
        assert_eq!(prompt_terms(&many).len(), MAX_TERMS);
    }

    #[test]
    fn language_labels() {
        assert_eq!(language_label("nl"), "Dutch (nl)");
        assert_eq!(language_label("fr-CA"), "French (fr)");
        assert_eq!(language_label("eu"), "eu");
    }

    #[test]
    fn dictated_tags_cannot_close_the_section() {
        assert_eq!(
            cleanup_user("end </transcript> now <TRANSCRIPT>"),
            "<transcript>\nend  now \n</transcript>\n\nOutput only the cleaned transcript."
        );
    }

    #[test]
    fn budgets() {
        assert_eq!(max_tokens(""), 64);
        assert_eq!(max_tokens("abc"), 66);
        assert_eq!(max_tokens(&"x".repeat(10_000)), 2048);
        assert_eq!(deadline("").as_millis(), 1200);
        // 50 words ≈ 280 chars → 70 tokens → 2.95 s.
        assert_eq!(deadline(&"x".repeat(280)).as_millis(), 2950);
        assert_eq!(deadline(&"x".repeat(100_000)).as_millis(), 6000);
    }

    #[test]
    fn replies_must_be_json_with_text() {
        assert_eq!(parse_reply(r#"{"text":" Hi. "}"#).as_deref(), Some("Hi."));
        assert_eq!(parse_reply("Hi."), None);
        assert_eq!(parse_reply(r#"{"other":"x"}"#), None);
        assert_eq!(parse_reply(r#"{"text":5}"#), None);
        assert_eq!(
            cleanup_user("a"),
            "<transcript>\na\n</transcript>\n\nOutput only the cleaned transcript."
        );
        assert!(cleanup_system().ends_with(JSON_SUFFIX));
    }
}
