// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2025-present Voquill, Inc.
// Modified by Wispr Bro contributors, 2026-10-29.
//
// Ported from Voquill `apps/desktop/src-tauri/src/utils/log_sanitizer.rs`
// (https://github.com/voquill/voquill, commit a5dfbe0e3a35807271293a0ef02c91902abc0104),
// licensed AGPL-3.0. Changes: the original five transcript rules and their
// tests are kept verbatim; added rules for Wispr Bro / Handy log shapes
// (e-mail addresses, the user's home directory, bearer tokens, API keys,
// prompt and dictionary dumps) with their own tests; a home-directory
// parameter for testability. See docs/fork/PORTS.md.

//! Log redaction used for crash files (F-08) and the diagnostics bundle
//! (S-27). Masks transcripts, LLM output, prompts, dictionary words, e-mail
//! addresses, secrets and `$HOME` (R5 §5.7).

use once_cell::sync::Lazy;
use regex::Regex;

struct SanitizeRule {
    pattern: Regex,
    replacement: &'static str,
}

static RULES: Lazy<Vec<SanitizeRule>> = Lazy::new(|| {
    vec![
        // [Provider] Received transcript: {"length":142,"preview":"some text..."}
        // Redact the preview value but keep the rest
        SanitizeRule {
            pattern: Regex::new(
                r#"(?m)(Received transcript:\s*.*?"preview"\s*:\s*)"(?:[^"\\]|\\.)*""#,
            )
            .unwrap(),
            replacement: r#"${1}"[REDACTED]""#,
        },
        // Processed transcript: <full text until end of line>
        SanitizeRule {
            pattern: Regex::new(r"(?m)(Processed transcript:)\s*.+$").unwrap(),
            replacement: "$1 [REDACTED]",
        },
        // LLM raw output: <full text until end of line>
        SanitizeRule {
            pattern: Regex::new(r"(?m)(LLM raw output:)\s*.+$").unwrap(),
            replacement: "$1 [REDACTED]",
        },
        // [Azure Streaming] Timeout reached, finalizing with transcript: <text>
        SanitizeRule {
            pattern: Regex::new(r"(?m)(finalizing with transcript:)\s*.+$").unwrap(),
            replacement: "$1 [REDACTED]",
        },
        // [Azure Streaming] Recognition stopped, final transcript: <text>
        SanitizeRule {
            pattern: Regex::new(r"(?m)(final transcript:)\s*.+$").unwrap(),
            replacement: "$1 [REDACTED]",
        },
        // Wispr Bro additions below.
        // Transcription result: <text> / Transcript: <text> / Raw text: <text>
        // (a JSON object or list after the colon is left to the field rules)
        SanitizeRule {
            pattern: Regex::new(
                r"(?mi)((?:transcription(?: result)?|transcript|raw text|cleaned text|pasted text|prompt|custom words|dictionary(?: words)?)\s*[:=])[ \t]*[^\s{\[].*$",
            )
            .unwrap(),
            replacement: "$1 [REDACTED]",
        },
        // JSON fields that carry user text: "prompt":"…", "custom_words":[…]
        SanitizeRule {
            pattern: Regex::new(
                r#"("(?:text|transcript|transcription|prompt|post_process_prompts|custom_words|custom_filler_words|selected_microphone|clamshell_microphone|selected_output_device)"\s*:\s*)(?:"(?:[^"\\]|\\.)*"|\[[^\]]*\])"#,
            )
            .unwrap(),
            replacement: r#"${1}"[REDACTED]""#,
        },
        // Authorization: Bearer <token>
        SanitizeRule {
            pattern: Regex::new(r"(?i)(bearer\s+)[A-Za-z0-9._~+/=-]{8,}").unwrap(),
            replacement: "${1}[REDACTED]",
        },
        // API keys (sk-…, sk-ant-…, hf_…, gsk_…)
        SanitizeRule {
            pattern: Regex::new(r"\b(?:sk-[A-Za-z0-9_-]{8,}|hf_[A-Za-z0-9]{8,}|gsk_[A-Za-z0-9]{8,})")
                .unwrap(),
            replacement: "[REDACTED_KEY]",
        },
        // E-mail addresses
        SanitizeRule {
            pattern: Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap(),
            replacement: "[EMAIL]",
        },
    ]
});

/// Masks user content in `input`, and the current user's home directory
/// (from `$HOME`) as `~`.
pub fn sanitize_log_content(input: &str) -> String {
    let home = std::env::var("HOME").ok();
    sanitize_with_home(input, home.as_deref())
}

/// [`sanitize_log_content`] with an explicit home directory.
pub fn sanitize_with_home(input: &str, home: Option<&str>) -> String {
    let mut result = input.to_string();
    if let Some(home) = home
        .map(|h| h.trim_end_matches('/'))
        .filter(|h| h.len() > 1)
    {
        result = result.replace(home, "~");
    }
    for rule in RULES.iter() {
        result = rule
            .pattern
            .replace_all(&result, rule.replacement)
            .into_owned();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // Voquill's original test vectors (unchanged).

    #[test]
    fn test_redacts_assemblyai_transcript_preview() {
        let input = r#"[2024-01-15][14:30:45.123][DEBUG][webview] [AssemblyAI] Received transcript: {"length":142,"preview":"Hello this is a private dictation about my medical..."}"#;
        let result = sanitize_log_content(input);
        assert!(result.contains(r#""[REDACTED]""#));
        assert!(!result.contains("Hello this is a private"));
        assert!(result.contains("length"));
    }

    #[test]
    fn test_redacts_deepgram_transcript_preview() {
        let input = r#"[2024-01-15][14:30:45.123][DEBUG][webview] [Deepgram] Received transcript: {"length":85,"preview":"Meeting notes for the quarterly review with clien..."}"#;
        let result = sanitize_log_content(input);
        assert!(result.contains(r#""[REDACTED]""#));
        assert!(!result.contains("Meeting notes"));
    }

    #[test]
    fn test_redacts_elevenlabs_transcript_preview() {
        let input = r#"[2024-01-15][14:30:45.123][DEBUG][webview] [ElevenLabs] Received transcript: {"length":200,"preview":"Dear doctor, I am writing to inform you about my ..."}"#;
        let result = sanitize_log_content(input);
        assert!(result.contains(r#""[REDACTED]""#));
        assert!(!result.contains("Dear doctor"));
    }

    #[test]
    fn test_redacts_azure_transcript_preview() {
        let input = r#"[2024-01-15][14:30:45.123][DEBUG][webview] [Azure] Received transcript: {"length":50,"preview":"Some sensitive text here"}"#;
        let result = sanitize_log_content(input);
        assert!(result.contains(r#""[REDACTED]""#));
        assert!(!result.contains("Some sensitive text here"));
    }

    #[test]
    fn test_redacts_new_server_transcript_preview() {
        let input = r#"[2024-01-15][14:30:45.123][DEBUG][webview] [NewServer] Received transcript: {"rawLength":100,"processedLength":95,"preview":"Private conversation content that should not appea..."}"#;
        let result = sanitize_log_content(input);
        assert!(result.contains(r#""[REDACTED]""#));
        assert!(!result.contains("Private conversation"));
    }

    #[test]
    fn test_redacts_processed_transcript() {
        let input = "[2024-01-15][14:30:45.123][DEBUG][webview] Processed transcript: This is the full processed transcript that the user dictated and it contains sensitive information.";
        let result = sanitize_log_content(input);
        assert!(result.contains("Processed transcript: [REDACTED]"));
        assert!(!result.contains("sensitive information"));
    }

    #[test]
    fn test_redacts_azure_streaming_timeout_transcript() {
        let input = "[2024-01-15][14:30:45.123][DEBUG][webview] [Azure Streaming] Timeout reached, finalizing with transcript: Hello this is my private medical information";
        let result = sanitize_log_content(input);
        assert!(result.contains("finalizing with transcript: [REDACTED]"));
        assert!(!result.contains("private medical"));
    }

    #[test]
    fn test_redacts_azure_streaming_final_transcript() {
        let input = "[2024-01-15][14:30:45.123][DEBUG][webview] [Azure Streaming] Recognition stopped, final transcript: This is highly confidential dictation content";
        let result = sanitize_log_content(input);
        assert!(result.contains("final transcript: [REDACTED]"));
        assert!(!result.contains("highly confidential"));
    }

    #[test]
    fn test_preserves_non_sensitive_log_lines() {
        let input = "[2024-01-15][14:30:45.123][INFO][webview] Transcript pasted successfully";
        let result = sanitize_log_content(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_preserves_timing_logs() {
        let input = r#"[2024-01-15][14:30:45.123][DEBUG][webview] [AssemblyAI] Transcript timing: {"durationMs":1234}"#;
        let result = sanitize_log_content(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_handles_multiline_log_content() {
        let input = r#"[2024-01-15][14:30:45.123][DEBUG][webview] [Deepgram] Received transcript: {"length":85,"preview":"Secret meeting notes..."}
[2024-01-15][14:30:46.000][INFO][webview] Transcript pasted successfully
[2024-01-15][14:30:47.000][DEBUG][webview] Processed transcript: Another secret transcript here"#;
        let result = sanitize_log_content(input);
        assert!(!result.contains("Secret meeting"));
        assert!(!result.contains("Another secret"));
        assert!(result.contains("Transcript pasted successfully"));
    }

    #[test]
    fn test_redacts_llm_raw_output() {
        let input = r#"[2026-03-07][10:25:32.892][DEBUG][webview:debug@http://localhost:1420/node_modules/.vite/deps/@tauri-apps_plugin-log.js:71:12] LLM raw output: {"processedTranscription":"Testing, one, two, three."}"#;
        let result = sanitize_log_content(input);
        assert!(result.contains("LLM raw output: [REDACTED]"));
        assert!(!result.contains("Testing, one, two, three"));
    }

    #[test]
    fn test_empty_input() {
        assert_eq!(sanitize_log_content(""), "");
    }

    #[test]
    fn test_preserves_transcript_metadata_without_content() {
        let input = "[2024-01-15][14:30:45.123][INFO][webview] Storing transcription record";
        let result = sanitize_log_content(input);
        assert_eq!(result, input);
    }

    // Wispr Bro additions.

    #[test]
    fn masks_home_directory() {
        let input =
            "panicked at /Users/alice/Library/Application Support/com.wisprbro.desktop/fork.db";
        let result = sanitize_with_home(input, Some("/Users/alice"));
        assert_eq!(
            result,
            "panicked at ~/Library/Application Support/com.wisprbro.desktop/fork.db"
        );
        assert_eq!(sanitize_with_home("/ stays", Some("/")), "/ stays");
    }

    #[test]
    fn masks_emails_tokens_and_keys() {
        let input = "user bob.smith+x@example.co.uk sent Authorization: Bearer abcdEFGH1234.xyz key sk-proj-secret-key-12345 and hf_abcdefgh12345678";
        let result = sanitize_with_home(input, None);
        assert!(!result.contains("bob.smith"), "{result}");
        assert!(result.contains("[EMAIL]"));
        assert!(!result.contains("abcdEFGH1234"), "{result}");
        assert!(!result.contains("sk-proj"), "{result}");
        assert!(!result.contains("hf_abcdefgh"), "{result}");
    }

    #[test]
    fn masks_handy_shaped_transcripts_prompts_and_words() {
        for (input, secret) in [
            ("Transcription result: my bank pin is 1234", "bank pin"),
            ("transcript = call mom about the biopsy", "biopsy"),
            ("Custom words: Zyxquor, Blorbington", "Zyxquor"),
            ("Prompt: Rewrite like a pirate", "pirate"),
        ] {
            let result = sanitize_with_home(input, None);
            assert!(!result.contains(secret), "{input} -> {result}");
            assert!(result.contains("[REDACTED]"), "{result}");
        }
        let json = r#"{"custom_words":["Zyxquor","Blorb"],"prompt":"be a pirate","selected_microphone":"Alice's AirPods","log_level":"info"}"#;
        let result = sanitize_with_home(json, None);
        for secret in ["Zyxquor", "pirate", "AirPods"] {
            assert!(!result.contains(secret), "{result}");
        }
        assert!(result.contains(r#""log_level":"info""#));
    }

    #[test]
    fn keeps_ordinary_lines() {
        for line in [
            "[INFO][handy_app_lib::fork] fork ready (Wispr Bro v1.0.0-alpha.1, locale en)",
            "Model loaded in 412 ms",
            "Processed prompt length: 120 chars",
        ] {
            assert_eq!(sanitize_with_home(line, Some("/Users/nobody")), line);
        }
    }
}
