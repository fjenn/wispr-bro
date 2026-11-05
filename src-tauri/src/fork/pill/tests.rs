// SPDX-License-Identifier: GPL-3.0-or-later
//! Controller-level tests of the Flow Bar (S-03): event mappings, the
//! settings it reads, VoiceOver announcements, the Handy hooks and the
//! strings it needs.

use super::model::*;
use super::*;
use crate::fork::events::ForkCardReason;
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
        card_reason: None,
        more: 0,
        copied: false,
        sends_automatically: false,
        context: false,
        diff: None,
        holder: None,
        word: None,
    }
}

fn card_view(reason: Option<ForkCardReason>, text: Option<&str>) -> ActiveView {
    ActiveView {
        phase: PillPhase::Card,
        shape: geometry::Shape::Card,
        text: text.map(str::to_string),
        card_reason: reason,
        ..view(PillPhase::Card, None)
    }
}

#[test]
fn one_card_for_queued_and_not_pasted_takes() {
    let tr = view(PillPhase::Transcribing, None);
    // S-07a: a queued take.
    let queued = card_view(None, Some("hello"));
    assert_eq!(
        announcement(Some(&tr), Some(&queued)),
        Some(("rust.pill.announce.card".into(), true))
    );
    // S-05: the reason is the announcement (and the card's label).
    let moved = card_view(Some(ForkCardReason::FocusMoved), Some("hello"));
    assert_eq!(
        announcement(Some(&tr), Some(&moved)),
        Some(("rust.pill.card.focus_moved".into(), true))
    );
    let e = state_event(3, &moved);
    assert_eq!(e.card_reason, Some(ForkCardReason::FocusMoved));
    assert_eq!(e.text.as_deref(), Some("hello"));
    let v = serde_json::to_value(&e).unwrap();
    assert_eq!(v["phase"], "card");
    assert_eq!(v["card_reason"], "focus_moved");
    assert_eq!(serde_json::to_value(PillUiAction::Copy).unwrap(), "copy");
    // Every reason has its string.
    let keys = crate::fork::i18n::english_keys();
    for r in [
        ForkCardReason::Sleep,
        ForkCardReason::ScreenLocked,
        ForkCardReason::SessionInactive,
        ForkCardReason::PasteFailed,
        ForkCardReason::DiskFull,
        ForkCardReason::NoTextField,
        ForkCardReason::FocusMoved,
        ForkCardReason::SecureField,
    ] {
        assert!(keys.contains(&card_key(r)), "{}", card_key(r));
    }
}

#[test]
fn a_password_fields_card_never_carries_its_text() {
    let mut m = Model::new(Prefs::default());
    m.step(
        0,
        Input::Card(model::CardContent {
            text: "pw".into(),
            reason: Some(ForkCardReason::SecureField),
        }),
    );
    let a = m.view_at(1).active.unwrap();
    assert_eq!(a.text, None);
    assert_eq!(state_event(1, &a).text, None);
    // Copy reads the model's full text.
    let c = m.card().unwrap();
    assert!(c.hidden());
    assert_eq!(c.text, "pw");
    // Handy's "failed" report of the same take does not replace the card;
    // a start error does.
    m.step(2, Input::Message(PillMessage::TranscriptionFailed));
    assert_eq!(m.view_at(2).active.unwrap().phase, PillPhase::Card);
    m.step(3, Input::Message(PillMessage::MicDenied));
    assert_eq!(m.view_at(3).active.unwrap().phase, PillPhase::Message);
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

/// S-34: each "New word" note is spoken once, with its word; Add's
/// confirmation too; pausing (hover) says nothing.
#[test]
fn voiceover_reads_each_new_word_note_once() {
    use crate::fork::pill::accessibility::spoken;
    let note = |id: i64, word: &str, paused: bool, result: Option<WordResult>| {
        let mut v = view(PillPhase::Word, None);
        v.shape = geometry::Shape::Word;
        v.word = Some(ForkPillWord {
            id,
            word: word.into(),
            remaining_ms: 15_000,
            total_ms: 15_000,
            paused,
            result,
        });
        v
    };
    let a = note(1, "Kubernetes", false, None);
    let (text, high) = spoken("en", None, Some(&a)).unwrap();
    assert!(text.starts_with("New word: Kubernetes."), "{text}");
    assert!(!high);
    assert_eq!(
        spoken("en", Some(&a), Some(&note(1, "Kubernetes", true, None))),
        None
    );
    let added = note(1, "Kubernetes", false, Some(WordResult::Added));
    assert_eq!(
        spoken("en", Some(&a), Some(&added))
            .map(|(t, _)| t)
            .as_deref(),
        Some("Added to Dictionary")
    );
    let next = note(2, "Supabase", false, None);
    assert!(spoken("en", Some(&added), Some(&next))
        .unwrap()
        .0
        .contains("Supabase"));
    // Its wire names.
    let v = serde_json::to_value(state_event(3, &added)).unwrap();
    assert_eq!(v["phase"], "word");
    assert_eq!(v["shape"], "word");
    assert_eq!(v["word"]["result"], "added");
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
        "rust.pill.announce.micLostWarning".into(),
        "rust.pill.announce.inserted".into(),
        "rust.pill.announce.settingUp".into(),
        "rust.pill.announce.card".into(),
        "rust.pill.announce.undo".into(),
        "rust.pill.announce.diff".into(),
        "rust.pill.announce.secureInputWarning".into(),
        "rust.pill.announce.newWord".into(),
        "rust.pill.word.added".into(),
        "rust.pill.word.full".into(),
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

/// S-24.1 regression: tauri-nspanel's `no_activate(true)` flips the whole
/// app to the Prohibited activation policy while it builds a panel, which
/// deactivates Wispr Bro when its own window (the Scratchpad note) is in
/// front, so a take started there lost its target. No fork window may use
/// it; the bar's web view is built hidden and unfocused instead.
#[test]
fn no_fork_panel_switches_the_app_activation_policy() {
    fn visit(dir: &std::path::Path, hits: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(&path, hits);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let source = std::fs::read_to_string(&path).unwrap();
                for (n, line) in source.lines().enumerate() {
                    let code = line.trim_start();
                    if code.starts_with(".no_activate(") && !code.starts_with(".no_activate(false)")
                    {
                        hits.push(format!("{}:{}", path.display(), n + 1));
                    }
                }
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/fork");
    let mut hits = Vec::new();
    visit(&root, &mut hits);
    assert!(hits.is_empty(), "no_activate in fork code: {hits:?}");
    let panel = std::fs::read_to_string(root.join("pill/panel.rs")).unwrap();
    assert!(panel.contains(".focused(false)") && panel.contains(".visible(false)"));
}

#[test]
fn voiceover_reads_the_cards_text_but_never_a_password_fields() {
    use crate::fork::pill::accessibility::spoken;
    let tr = view(PillPhase::Transcribing, None);
    let moved = card_view(Some(ForkCardReason::FocusMoved), Some(" Meet at noon. "));
    let (text, high) = spoken("en", Some(&tr), Some(&moved)).unwrap();
    assert!(high);
    assert_eq!(text, "You moved on, so nothing was pasted. Meet at noon.");
    // S-05: a password field's card has no text in the view.
    let secret = card_view(Some(ForkCardReason::SecureField), None);
    let (text, _) = spoken("en", Some(&tr), Some(&secret)).unwrap();
    assert_eq!(text, "Not typed into a password field");
    // Other states speak only their key.
    let (text, high) = spoken("en", None, Some(&view(PillPhase::Held, None))).unwrap();
    assert_eq!((text.as_str(), high), ("Listening", false));
    // Localized through fork/i18n.rs.
    let (de, _) = spoken("de", None, Some(&view(PillPhase::Held, None))).unwrap();
    assert!(!de.is_empty());
}

/// Mac wave 3, S-28.1d-f: a take started while the app was hidden (⌘H)
/// unhid it, Hub included, because the bar's panels could hide with the
/// app and AppKit unhides a hidden app when such a window is ordered in.
/// Every panel the take path orders in turns `canHide` off before its first
/// order-in, and nothing on the bar's path unhides or activates the app.
/// Mac wave 4 (S-28.8): the panels holding a web view are built hidden and
/// inside `activation::without_app_activation`, since wry activates the
/// app after each web view build and that unhides it.
#[test]
fn bar_panels_never_unhide_a_hidden_app() {
    let native = include_str!("native.rs");
    let panel = include_str!("panel.rs");
    let spotlight = include_str!("spotlight.rs");
    assert!(native.contains("window.setCanHide(false);"));
    // The resting bar: before `Resting::new` returns (it is ordered in later).
    let new = &native[native.find("pub fn new(mtm: MainThreadMarker").unwrap()..];
    let new = &new[..new.find("Resting { panel, view }").unwrap()];
    assert!(new.contains("stay_when_app_hidden(&panel);"), "resting bar");
    // The web view and the spotlight: before their first `panel.show()`.
    for (name, src, call) in [
        ("fork-pill", panel, "native::stay_when_app_hidden(win);"),
        (
            "spotlight",
            spotlight,
            "super::super::native::stay_when_app_hidden(win);",
        ),
    ] {
        let at = src
            .find(call)
            .unwrap_or_else(|| panic!("{name}: no canHide"));
        let show = src.find("panel.show();").unwrap();
        assert!(at < show, "{name}: canHide is set after the first order-in");
        assert_eq!(src.matches("panel.show();").count(), 1, "{name}");
        // Mac wave 4, S-28.8: the build itself must not order the window in
        // (tao orders a `visible` window front during the build, before
        // canHide can be set) and must not let wry activate the app (its
        // `NSApp.activate()` after every web view build unhides a hidden
        // app; `activation.rs`).
        let builder = &src[src.find("PanelBuilder::<_,").unwrap()..];
        let build = builder.find(".build()").unwrap();
        let builder = &builder[..build];
        assert!(builder.contains(".visible(false)"), "{name}: built visible");
        let guard = src
            .find("activation::without_app_activation(mtm, || {")
            .unwrap_or_else(|| panic!("{name}: built without the activation guard"));
        let at_builder = src.find("PanelBuilder::<_,").unwrap();
        let built = at_builder + build;
        assert!(
            guard < at_builder,
            "{name}: the guard opens after the builder"
        );
        assert!(
            !src[guard..built].contains("});"),
            "{name}: the guard closes before .build()"
        );
        assert!(built < at, "{name}: canHide is set before the panel exists");
        assert_eq!(src.matches("PanelBuilder::<_,").count(), 1, "{name}");
    }
    let activation = include_str!("activation.rs");
    assert!(activation.contains("(sel!(activate), "));
    assert!(activation.contains("(sel!(activateIgnoringOtherApps:), "));
    for (name, src) in [
        ("native", native),
        ("panel", panel),
        ("spotlight", spotlight),
    ] {
        let code: String = src
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for banned in [
            "unhide",
            "activateIgnoringOtherApps",
            "makeKeyAndOrderFront",
            "set_focus(",
            "show_main_window",
        ] {
            assert!(!code.contains(banned), "{name} calls {banned}");
        }
    }
}
