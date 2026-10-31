// SPDX-License-Identifier: GPL-3.0-or-later
//! Controller-level tests of the Flow Bar (S-03): event mappings, the
//! settings it reads, VoiceOver announcements, the Handy hooks and the
//! strings it needs.

use super::model::*;
use super::*;
use crate::fork::settings::{ForkSettings, OnboardingStatus};

#[test]
fn handy_outcomes_and_errors_map_to_messages() {
    assert_eq!(message_for_outcome("empty"), Some(PillMessage::NoSpeech));
    assert_eq!(message_for_outcome("silent"), Some(PillMessage::MicMuted));
    assert_eq!(
        message_for_outcome("failed"),
        Some(PillMessage::TranscriptionFailed)
    );
    assert_eq!(message_for_outcome("typed"), None);
    assert_eq!(
        message_for_recording_error("microphone_permission_denied"),
        PillMessage::MicDenied
    );
    assert_eq!(
        message_for_recording_error("no_input_device"),
        PillMessage::NoMicrophone
    );
    assert_eq!(
        message_for_recording_error("unknown"),
        PillMessage::MicFailed
    );
    // The names Handy actually emits.
    let actions = include_str!("../../actions.rs");
    for name in ["Empty", "Silent", "Failed", "Typed"] {
        assert!(actions.contains(&format!("    {name},")), "{name}");
    }
    let audio = include_str!("../../managers/audio.rs");
    for kind in ["microphone_permission_denied", "no_input_device"] {
        assert!(audio.contains(kind), "{kind}");
    }
}

#[test]
fn setup_and_visibility_come_from_the_settings() {
    let mut s = ForkSettings::default();
    // DEC-38: no fork onboarding yet and Handy's not done → hidden.
    assert!(!prefs_for(&s, false, 0).setup_done);
    // Until S-29a, Handy's own onboarding counts.
    assert!(prefs_for(&s, true, 0).setup_done);
    s.onboarding.status = OnboardingStatus::Completed;
    assert!(prefs_for(&s, false, 0).setup_done);
    s.onboarding.status = OnboardingStatus::InProgress;
    assert!(!prefs_for(&s, false, 0).setup_done);

    s.bar_hidden_until_ms = Some(10_000);
    assert!(prefs_for(&s, true, 9_999).hidden_for_now);
    assert!(!prefs_for(&s, true, 10_000).hidden_for_now);
    s.bar_always_visible = false;
    assert!(!prefs_for(&s, true, 0).always_visible);
}

fn view(phase: PillPhase, message: Option<PillMessage>) -> ActiveView {
    ActiveView {
        phase,
        shape: geometry::Shape::Held,
        mic: None,
        message,
        warning: None,
        progress: None,
        text: None,
        more: 0,
        copied: false,
    }
}

#[test]
fn voiceover_announces_each_change_once() {
    let held = view(PillPhase::Held, None);
    let locked = view(PillPhase::Locked, None);
    let tr = view(PillPhase::Transcribing, None);
    let msg = view(PillPhase::Message, Some(PillMessage::MicDenied));
    let key = |b: Option<&ActiveView>, a: Option<&ActiveView>| announcement(b, a).map(|(k, _)| k);
    assert_eq!(
        key(None, Some(&held)).as_deref(),
        Some("rust.pill.announce.listening")
    );
    // Held → locked is the same take: no second "listening".
    assert_eq!(key(Some(&held), Some(&locked)), None);
    assert_eq!(key(Some(&held), Some(&held)), None);
    assert_eq!(
        key(Some(&locked), Some(&tr)).as_deref(),
        Some("rust.pill.announce.transcribing")
    );
    let (k, high) = announcement(Some(&tr), Some(&msg)).unwrap();
    assert_eq!((k.as_str(), high), ("rust.pill.message.mic_denied", true));
    assert_eq!(
        key(Some(&held), None).as_deref(),
        Some("rust.pill.announce.cancelled")
    );
    // A processed take ending is not a cancel.
    assert_eq!(key(Some(&tr), None), None);
    let mut warned = locked.clone();
    warned.warning = Some(PillWarning::Silence);
    assert_eq!(
        key(Some(&locked), Some(&warned)).as_deref(),
        Some("rust.pill.announce.silenceWarning")
    );
}

#[test]
fn every_pill_string_exists_in_english() {
    let keys = crate::fork::i18n::english_keys();
    let mut needed = vec![
        "rust.pill.name".to_string(),
        "rust.pill.hint".into(),
        "rust.pill.hideForAnHour".into(),
        "rust.pill.announce.listening".into(),
        "rust.pill.announce.transcribing".into(),
        "rust.pill.announce.cleaningUp".into(),
        "rust.pill.announce.cancelled".into(),
        "rust.pill.announce.silenceWarning".into(),
        "rust.pill.announce.lengthWarning".into(),
    ];
    use PillMessage::*;
    for m in [
        NoSpeech,
        MicMuted,
        InsertedWithoutCleanup,
        NotPasted,
        TranscriptionFailed,
        MicDenied,
        NoMicrophone,
        MicFailed,
    ] {
        needed.push(format!("rust.pill.message.{}", m.key()));
    }
    for k in needed {
        assert!(keys.contains(&k), "missing {k} in en/fork.json");
    }
}

#[test]
fn state_events_carry_tone_and_fix() {
    let a = view(PillPhase::Message, Some(PillMessage::MicDenied));
    let e = state_event(7, &a);
    assert_eq!(e.seq, 7);
    assert_eq!(e.tone, Some(PillTone::Error));
    assert_eq!(e.fix, Some(PillFix::MicrophonePrivacy));
    let e = state_event(8, &view(PillPhase::Held, None));
    assert_eq!((e.tone, e.fix, e.message), (None, None, None));
    // Wire names the web view relies on.
    let v = serde_json::to_value(&e).unwrap();
    assert_eq!(v["phase"], "held");
    assert_eq!(v["shape"], "held");
}

#[test]
fn handy_hooks_are_in_place_and_the_overlay_is_never_built() {
    let overlay = include_str!("../../overlay.rs");
    assert!(overlay.contains(
        "pub fn create_recording_overlay(app_handle: &AppHandle) {\n    if crate::fork::pill::replaces_handy_overlay() {\n        return;\n    }"
    ));
    assert!(overlay.contains(
        "fn show_overlay_state(app_handle: &AppHandle, state: &str) {\n    crate::fork::pill::on_overlay_state(app_handle, state);"
    ));
    assert!(overlay.contains(
        "pub fn hide_recording_overlay(app_handle: &AppHandle) {\n    crate::fork::pill::on_overlay_hide(app_handle);"
    ));
    // One levels hook (S-01's), which feeds the bar too.
    assert_eq!(overlay.matches("crate::fork::").count(), 4);
    assert!(include_str!("../audio/mod.rs").contains("crate::fork::pill::on_levels(app, levels);"));
    // `cargo test` builds without the identity overlay: Handy's own overlay
    // stays; product builds replace it.
    assert_eq!(
        replaces_handy_overlay(),
        cfg!(target_os = "macos") && crate::fork::is_product_build()
    );
}

#[test]
fn hooks_before_start_are_harmless() {
    // No controller running in unit tests: every entry point is a no-op.
    assert!(!is_running());
    on_hotkey_effect(&crate::fork::shortcut::processor::Effect::TakeStarted);
    on_take_warning(crate::fork::shortcut::processor::WarnKind::Silence);
    prewarm();
    ready(3);
    ui_action(PillUiAction::Stop);
    show_message(PillMessage::NoSpeech);
    assert_eq!(snapshot(), None);
}
