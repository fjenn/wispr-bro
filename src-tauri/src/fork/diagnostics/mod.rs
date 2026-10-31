// SPDX-License-Identifier: GPL-3.0-or-later
//! Diagnostics (F-08, R5 §5.7, W-24).
//!
//! - [`default_log_level`]: Handy's default file log level (hook in
//!   `settings::default_log_level`). Info; Debug only through the debug panel
//!   or `--debug`.
//! - [`settings_summary`]: replaces Handy's startup dump of the whole
//!   `AppSettings` (`settings::load_or_create_app_settings`). Logs an
//!   allow-listed summary only: no custom words, prompts, device names, API
//!   keys or endpoint URLs.
//! - [`sanitizer`]: redaction (Voquill port, AGPL-3.0-only).
//! - [`crash`]: the panic hook installed by `fork::early_init`.
//!
//! S-27 adds `bundle.rs` (diagnostics export).

pub mod crash;
pub mod sanitizer;

use serde_json::{json, Value};

use crate::settings::{AppSettings, LogLevel};

/// Hook in `managers/transcription.rs` (Handy's Info line "Transcription
/// result: …"; S-12a): the transcript never reaches the log, only its
/// length. Debug builds show the text when `WB_DEBUG_TEXT_TRACE` is set
/// (the same switch as the S-11 pipeline trace); release builds never do,
/// whatever the environment.
pub fn redact(text: &str) -> String {
    #[cfg(debug_assertions)]
    if std::env::var_os(crate::fork::text::TRACE_ENV).is_some() {
        return text.to_owned();
    }
    format!("[{} chars]", text.chars().count())
}

/// Default file log level (Handy's default is Debug).
pub fn default_log_level() -> LogLevel {
    LogLevel::Info
}

/// The allow-listed settings summary. Values are enums, booleans, counts and
/// ids the app ships (model id, provider id, language code); everything the
/// user typed or that names their hardware is reduced to a count or a flag.
pub fn summary(settings: &AppSettings) -> Value {
    json!({
        "schema": settings.settings_schema_version,
        "log_level": settings.log_level,
        "debug_mode": settings.debug_mode,
        "selected_model": settings.selected_model,
        "selected_language": settings.selected_language,
        "spoken_languages": settings.spoken_languages.len(),
        "app_language": settings.app_language,
        "onboarding_completed": settings.onboarding_completed,
        "shortcut_activation": settings.shortcut_activation,
        "bindings": settings.bindings.len(),
        "keyboard_implementation": settings.keyboard_implementation,
        "paste_method": settings.paste_method,
        "reliable_paste": settings.reliable_paste,
        "clipboard_handling": settings.clipboard_handling,
        "audio_feedback": settings.audio_feedback,
        "microphone_selected": settings.selected_microphone.is_some(),
        "always_on_microphone": settings.always_on_microphone,
        "overlay_style": settings.overlay_style,
        "vad_enabled": settings.vad_enabled,
        "custom_words": settings.custom_words.len(),
        "post_process_enabled": settings.post_process_enabled,
        "post_process_provider_id": settings.post_process_provider_id,
        "post_process_providers": settings.post_process_providers.len(),
        "post_process_prompts": settings.post_process_prompts.len(),
        "recording_retention_period": settings.recording_retention_period,
        "experimental_enabled": settings.experimental_enabled,
        "update_checks_enabled": settings.update_checks_enabled,
        "autostart_enabled": settings.autostart_enabled,
        "theme": settings.theme,
        "external_script": settings.external_script_path.is_some(),
    })
}

/// The log line [`settings_summary`] writes.
pub fn summary_line(settings: &AppSettings) -> String {
    format!("Loaded settings (summary): {}", summary(settings))
}

/// Hook in `settings::load_or_create_app_settings` (Handy dumped the whole
/// struct at Debug there, including device names, custom words and prompts).
pub fn settings_summary(settings: &AppSettings) {
    log::info!("{}", summary_line(settings));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_file_log_level_is_info() {
        assert_eq!(default_log_level(), LogLevel::Info);
        assert_eq!(
            crate::settings::get_default_settings().log_level,
            LogLevel::Info
        );
        // A store without the key deserializes with the hooked default too.
        let mut value = serde_json::to_value(crate::settings::get_default_settings()).unwrap();
        value.as_object_mut().unwrap().remove("log_level");
        let parsed: AppSettings = serde_json::from_value(value).unwrap();
        assert_eq!(parsed.log_level, LogLevel::Info);
    }

    #[test]
    fn startup_summary_has_no_user_content() {
        let mut s = crate::settings::get_default_settings();
        s.custom_words = vec!["Zyxquorian".into(), "Blorbington".into()];
        s.post_process_prompts[0].prompt = "PROMPT-SECRET pirate voice".into();
        s.post_process_prompts[0].name = "My Secret Prompt".into();
        s.selected_microphone = Some("Alice's AirPods Pro".into());
        s.clamshell_microphone = Some("Studio Mic X".into());
        s.selected_output_device = Some("Office Speakers".into());
        s.external_script_path = Some("/Users/alice/bin/paste.sh".into());
        s.custom_filler_words = Some(vec!["Fillerwordo".into()]);
        s.post_process_api_keys
            .insert("custom".into(), "sk-proj-SECRETKEY123456".into());
        if let Some(p) = s
            .post_process_providers
            .iter_mut()
            .find(|p| p.id == "custom")
        {
            p.base_url = "http://localhost:9999/secret-path".into();
        }
        let line = summary_line(&s);
        for secret in [
            "Zyxquorian",
            "Blorbington",
            "PROMPT-SECRET",
            "My Secret Prompt",
            "AirPods",
            "Studio Mic",
            "Office Speakers",
            "/Users/alice",
            "Fillerwordo",
            "SECRETKEY",
            "secret-path",
        ] {
            assert!(!line.contains(secret), "{secret} leaked: {line}");
        }
        assert!(line.contains("\"custom_words\":2"), "{line}");
        assert!(line.contains("\"microphone_selected\":true"), "{line}");
    }

    #[test]
    fn transcripts_are_redacted_to_their_length() {
        // The env switch exists only in debug builds and is off in tests.
        if std::env::var_os(crate::fork::text::TRACE_ENV).is_none() {
            assert_eq!(redact("my bank pin is 1234"), "[19 chars]");
            assert_eq!(redact(""), "[0 chars]");
        }
        let line = format!("Transcription result: {}", redact("secret words"));
        assert!(!line.contains("secret"));
        let src = include_str!("../../managers/transcription.rs");
        assert!(src.contains("crate::fork::diagnostics::redact(&final_result)"));
        assert!(!src.contains("crate::utils::redact_text(&final_result)"));
    }

    #[test]
    fn handy_hooks_are_in_place() {
        let src = include_str!("../../settings.rs");
        assert!(src.contains("crate::fork::diagnostics::default_log_level()"));
        assert!(src.contains("crate::fork::diagnostics::settings_summary(&settings);"));
        assert!(!src.contains("debug!(\"Loaded settings: {:?}\", settings);"));
    }
}
