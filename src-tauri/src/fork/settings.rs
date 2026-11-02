// SPDX-License-Identifier: GPL-3.0-or-later
//! Fork settings store and product defaults (F-07, R5 §4.4).
//!
//! - [`ForkSettings`] lives in `fork-settings.json` (tauri-plugin-store, key
//!   `"settings"`), next to Handy's `settings_store.json` and never inside
//!   Handy's `AppSettings`. It has its own `schema_version` and migrations
//!   ([`MIGRATIONS`]). A file that cannot be read is backed up and salvaged
//!   field by field ([`decode_stored`]).
//! - [`apply_product_settings`] runs from `fork::on_ready` on every start. It
//!   writes the Handy-side values the product depends on ([`ENFORCED`]) and,
//!   once per key, the first-run defaults ([`FIRST_RUN_DEFAULTS`]), recorded in
//!   [`ForkSettings::applied_first_run_defaults`]. It changes Handy's settings
//!   only through Handy's own `settings::get_settings` / `write_settings` and
//!   Handy's setting commands (see `ForkRuntime` in `fork/mod.rs`).
//!
//! The pure parts (decode, migrate, validate, apply) are plain functions over
//! values, so they are tested without a Tauri runtime (`settings/tests.rs`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use specta::Type;

use crate::settings::{
    AppSettings, KeyboardImplementation, LogLevel, OverlayStyle, PasteMethod,
    RecordingRetentionPeriod, Theme,
};

#[cfg(test)]
mod tests;

/// File name of the fork settings store (in the app data dir, or the portable
/// data dir in portable mode).
pub const FORK_SETTINGS_FILE: &str = "fork-settings.json";
/// Key of the settings object inside the store (same layout as Handy's store).
pub const STORE_KEY: &str = "settings";
/// Current [`ForkSettings::schema_version`].
pub const CURRENT_SCHEMA_VERSION: u32 = 6;

/// The only app identity whose Handy settings the fork may rewrite. A build
/// without the identity overlay (`cargo build`, `bun run tauri dev`) runs as
/// `com.pais.handy` and would otherwise rewrite a real Handy install's
/// settings, so `apply_product_settings` is skipped there.
pub const PRODUCT_IDENTIFIER: &str = "com.wisprbro.desktop";

/// `true` since S-03: the fork's Flow Bar (`fork::pill`) replaces Handy's
/// recording overlay, so [`ENFORCED`] turns Handy's `overlay_style` off (the
/// bar takes its phases from the coordinator's events and the `overlay.rs`
/// hooks, which run before Handy's `overlay_style` gate). Handy's own
/// `recording_overlay` window is never built in product builds
/// (`fork::pill::replaces_handy_overlay`).
pub const HANDY_OVERLAY_REPLACED: bool = true;

/// Provider ids and the loopback default of Handy's custom provider live with
/// the provider list (F-08, `fork/llm/providers.rs`).
pub use super::llm::providers::{CUSTOM_PROVIDER_ID, LOCAL_PROVIDER_ID, LOOPBACK_CUSTOM_BASE_URL};

/// Limits checked by [`validate`].
pub const MAX_BINDINGS_PER_ACTION: usize = 4;
pub const MAX_LIST_ITEMS: usize = 500;
pub const MAX_ID_LEN: usize = 64;
pub const MAX_GREETING_NAME_LEN: usize = 64;
pub const MAX_RETENTION_DAYS: u32 = 3650;
pub const MAX_TYPING_WPM: u32 = 300;
/// Displays whose bar position is remembered (oldest keys are dropped).
pub const MAX_BAR_DISPLAYS: usize = 32;

static SHOW_IN_DOCK: AtomicBool = AtomicBool::new(true);

/// The `lib.rs` close-window hook reads this before hiding the Dock icon.
pub fn show_in_dock() -> bool {
    SHOW_IN_DOCK.load(Ordering::Acquire)
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// How a hotkey binding drives its action.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyActivation {
    /// Hold to run, release to stop (push to talk).
    Hold,
    /// Press once to start, press again to stop.
    Toggle,
    /// Two quick presses (fn double-tap).
    DoubleTap,
    /// A single press fires the action (cancel, paste last, …).
    Press,
}

/// One binding of an action. `chord` uses Handy's key-name format joined by
/// `+` (`fn`, `fn+space`, `ctrl+option`, `escape`, `ctrl+command+v`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Type)]
pub struct HotkeyBinding {
    pub chord: String,
    pub activation: HotkeyActivation,
}

impl HotkeyBinding {
    fn new(chord: &str, activation: HotkeyActivation) -> Self {
        Self {
            chord: chord.into(),
            activation,
        }
    }
}

/// Fixed action ids. S-01 adds runtime ids (user transforms, Scratchpad) to
/// the same map; an id is `[a-z0-9_.-]{1,64}`.
pub mod hotkey_actions {
    pub const DICTATE: &str = "dictate";
    pub const HANDS_FREE: &str = "hands_free";
    pub const CANCEL: &str = "cancel";
    /// S-23: hold fn+Ctrl to speak an editing or writing instruction.
    pub const COMMAND_MODE: &str = "command_mode";
    pub const PASTE_LAST: &str = "paste_last";
    /// S-06: copy the last transcript (⌃⌘C).
    pub const COPY_LAST: &str = "copy_last";
    /// S-06: press Return in the frontmost app (unset by default; a key or
    /// a mouse button).
    pub const PRESS_ENTER: &str = "press_enter";
    /// S-06: bring back the take just cancelled (unset by default; the
    /// bar's Undo button does the same).
    pub const UNDO_CANCEL: &str = "undo_cancel";
}

/// Default bindings (DEC-09): hold fn to talk; double-tap fn or fn+Space for
/// hands-free; Esc cancels; ⌃⌘V pastes the last take and ⌃⌘C copies it
/// (S-06). Press Enter and Undo are unset (no entry = unset for them).
pub fn default_hotkeys() -> BTreeMap<String, Vec<HotkeyBinding>> {
    use hotkey_actions::*;
    use HotkeyActivation::*;
    BTreeMap::from([
        (DICTATE.into(), vec![HotkeyBinding::new("fn", Hold)]),
        (
            HANDS_FREE.into(),
            vec![
                HotkeyBinding::new("fn", DoubleTap),
                HotkeyBinding::new("fn+space", Press),
            ],
        ),
        (CANCEL.into(), vec![HotkeyBinding::new("escape", Press)]),
        (
            COMMAND_MODE.into(),
            vec![HotkeyBinding::new("fn+ctrl", Hold)],
        ),
        (
            PASTE_LAST.into(),
            vec![HotkeyBinding::new("ctrl+command+v", Press)],
        ),
        (
            COPY_LAST.into(),
            vec![HotkeyBinding::new("ctrl+command+c", Press)],
        ),
        (
            "transform.polish".into(),
            vec![HotkeyBinding::new("option+1", Press)],
        ),
        (
            "transform.prompt_engineer".into(),
            vec![HotkeyBinding::new("option+2", Press)],
        ),
        (
            "transform.view_changes".into(),
            vec![HotkeyBinding::new("option+o", Press)],
        ),
    ])
}

/// LLM cleanup level (DEC-03: Light by default).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type, Default)]
#[serde(rename_all = "snake_case")]
pub enum CleanupLevel {
    None,
    #[default]
    Light,
    Medium,
}

/// Style preset of an app group (S-19a). Personal offers Formal / Relaxed /
/// Very relaxed; Work, Email and Other offer Formal / Relaxed / Upbeat.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum StylePreset {
    Formal,
    Relaxed,
    VeryRelaxed,
    Upbeat,
}

/// Style per app group.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(default)]
pub struct StyleSettings {
    pub personal: StylePreset,
    pub work: StylePreset,
    pub email: StylePreset,
    pub other: StylePreset,
}

impl Default for StyleSettings {
    fn default() -> Self {
        Self {
            personal: StylePreset::Relaxed,
            work: StylePreset::Formal,
            email: StylePreset::Formal,
            other: StylePreset::Formal,
        }
    }
}

/// Where the resting bar sits (bottom by default, top in Advanced).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type, Default)]
#[serde(rename_all = "snake_case")]
pub enum BarPosition {
    #[default]
    Bottom,
    Top,
}

/// What S-24 opens when the Scratchpad shortcut or bar slot is used.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScratchpadOpenBehavior {
    #[default]
    ResumeLast,
    NewNote,
}

/// Update channel (DEC-33).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type, Default)]
#[serde(rename_all = "snake_case")]
pub enum UpdateChannel {
    #[default]
    Stable,
    Beta,
}

/// Which notifications are shown (DEC-21, C-91). All on by default.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(default)]
pub struct NotificationSettings {
    /// Send macOS notifications (the in-app inbox always records the four
    /// kinds below).
    pub system: bool,
    pub model_ready: bool,
    pub update_ready: bool,
    pub queued_takes_ready: bool,
    pub errors: bool,
    /// S-18: tips in the inbox ("Suggestions").
    pub suggestions: bool,
    /// S-18: feature announcements, inbox and the one-time popup.
    pub announcements: bool,
    /// S-18: word-count and streak milestones in the inbox.
    pub milestones: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            system: true,
            model_ready: true,
            update_ready: true,
            queued_takes_ready: true,
            errors: true,
            suggestions: true,
            announcements: true,
            milestones: true,
        }
    }
}

/// Where the user is in onboarding. The single source of truth (RE-16);
/// `fork/onboarding/state.rs` (S-29a) is only the logic over it.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type, Default)]
#[serde(rename_all = "snake_case")]
pub enum OnboardingStatus {
    #[default]
    NotStarted,
    InProgress,
    Completed,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Type, Default)]
#[serde(default)]
pub struct OnboardingState {
    pub status: OnboardingStatus,
    /// Id of the step to resume at (S-29a/b define the ids).
    pub current_step: Option<String>,
    pub completed_steps: Vec<String>,
    /// Setup-checklist items the user has done (S-30).
    pub checklist_done: Vec<String>,
    /// Unix milliseconds.
    pub completed_at_ms: Option<i64>,
}

/// Vibe coding (DEC-41): off until the user runs its setup.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Type, Default)]
#[serde(default)]
pub struct VibeCodingSettings {
    pub enabled: bool,
    pub setup_completed: bool,
    /// Editors whose screen-reader mode the setup has confirmed.
    pub editors: Vec<String>,
}

/// All fork settings. Missing fields take their [`Default`] value
/// (container-level `serde(default)`), so a partial file never fails as a
/// whole.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Type)]
#[serde(default)]
pub struct ForkSettings {
    /// Schema of this object; see [`MIGRATIONS`]. Not settable.
    pub schema_version: u32,
    /// Bindings per action id (≤ 4 each; an empty list means "unset").
    pub hotkeys: BTreeMap<String, Vec<HotkeyBinding>>,
    pub cleanup_level: CleanupLevel,
    pub styles: StyleSettings,
    /// DEC-24: on, local only, never stored.
    pub context_awareness: bool,
    /// DEC-31: Transforms and Command Mode are opt-in.
    pub transforms_enabled: bool,
    pub command_mode_enabled: bool,
    /// Transform applied after every dictation (DEC-13), by transform id.
    pub auto_apply_transform: Option<String>,
    /// "Always show the bar" (DEC-14; default on).
    pub bar_always_visible: bool,
    pub bar_position: BarPosition,
    /// "Hide for an hour": hidden until this Unix time in ms.
    pub bar_hidden_until_ms: Option<i64>,
    /// "Hide the bar in screen recordings" (S-03; RS-03: hidden from
    /// `screencapture`, its video mode and ScreenCaptureKit streams).
    pub bar_hide_in_recordings: bool,
    /// Where the user dragged the bar on each display, as a fraction of the
    /// display's visible width (0 = left end), by display key
    /// (`fork::pill::placement::display_key`). No entry = centred.
    pub bar_x_by_display: BTreeMap<String, f64>,
    /// DEC-30: Handy's live transcript in the pill, Advanced option (off).
    pub live_text_in_pill: bool,
    /// S-14/S-24: whether the shortcut resumes the last note or creates one.
    pub scratchpad_open_behavior: ScratchpadOpenBehavior,
    /// S-24: add the Scratchpad slot to the resting Flow Bar.
    pub scratchpad_on_bar: bool,
    /// DEC-7: `None` keeps text forever.
    pub text_retention_days: Option<u32>,
    /// DEC-7: audio 14 days; `None` keeps audio forever, `0` keeps none.
    pub audio_retention_days: Option<u32>,
    pub notifications: NotificationSettings,
    /// Mirrored into Handy's `update_checks_enabled` on every start.
    pub update_checks_enabled: bool,
    pub update_channel: UpdateChannel,
    /// DEC-14 (S-14 hook): the app shows a Dock icon.
    pub show_in_dock: bool,
    pub onboarding: OnboardingState,
    pub dismissed_banners: Vec<String>,
    pub seen_new_tags: Vec<String>,
    /// Option picked in the first-use chooser (S-30), by id.
    pub first_use_choice: Option<String>,
    /// From the typing test (S-29b); `None` = not taken.
    pub typing_wpm: Option<u32>,
    /// DEC-50: typing time per day, hours (default 2).
    pub typing_hours_per_day: f32,
    /// Greeting name; `None` uses the macOS account name (S-15a).
    pub greeting_name: Option<String>,
    /// DEC-20: opt-in.
    pub auto_learn_enabled: bool,
    pub vibe_coding: VibeCodingSettings,
    /// S-08: "Auto-detect (system setting)" was chosen in the microphone
    /// list, so the system default is used even when it is a Bluetooth
    /// headset. Off (never chosen): a wireless default is skipped for the
    /// built-in mic. Written by `fork_mic_choose` with Handy's
    /// `selected_microphone`; not settable on its own.
    pub mic_follow_system: bool,
    /// Keys of [`FIRST_RUN_DEFAULTS`] already applied. Not settable.
    pub applied_first_run_defaults: Vec<String>,
}

impl Default for ForkSettings {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            hotkeys: default_hotkeys(),
            cleanup_level: CleanupLevel::Light,
            styles: StyleSettings::default(),
            context_awareness: true,
            transforms_enabled: false,
            command_mode_enabled: false,
            auto_apply_transform: None,
            bar_always_visible: true,
            bar_position: BarPosition::Bottom,
            bar_hidden_until_ms: None,
            bar_hide_in_recordings: false,
            bar_x_by_display: BTreeMap::new(),
            live_text_in_pill: false,
            scratchpad_open_behavior: ScratchpadOpenBehavior::ResumeLast,
            scratchpad_on_bar: false,
            text_retention_days: None,
            audio_retention_days: Some(14),
            notifications: NotificationSettings::default(),
            update_checks_enabled: true,
            update_channel: UpdateChannel::Stable,
            show_in_dock: true,
            onboarding: OnboardingState::default(),
            dismissed_banners: Vec::new(),
            seen_new_tags: Vec::new(),
            first_use_choice: None,
            typing_wpm: None,
            typing_hours_per_day: 2.0,
            greeting_name: None,
            auto_learn_enabled: false,
            vibe_coding: VibeCodingSettings::default(),
            mic_follow_system: false,
            applied_first_run_defaults: Vec::new(),
        }
    }
}

/// One change sent by `fork_set_setting`, typed per field:
/// `{ "key": "cleanup_level", "value": "medium" }`. The internal fields
/// (`schema_version`, `applied_first_run_defaults`) have no variant.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Type)]
#[serde(tag = "key", content = "value", rename_all = "snake_case")]
pub enum ForkSettingUpdate {
    /// Replaces the bindings of one action. An empty list unsets it.
    Hotkey {
        action: String,
        bindings: Vec<HotkeyBinding>,
    },
    CleanupLevel(CleanupLevel),
    Styles(StyleSettings),
    ContextAwareness(bool),
    TransformsEnabled(bool),
    CommandModeEnabled(bool),
    AutoApplyTransform(Option<String>),
    BarAlwaysVisible(bool),
    BarPosition(BarPosition),
    BarHiddenUntilMs(Option<i64>),
    BarHideInRecordings(bool),
    BarXByDisplay(BTreeMap<String, f64>),
    LiveTextInPill(bool),
    ScratchpadOpenBehavior(ScratchpadOpenBehavior),
    ScratchpadOnBar(bool),
    TextRetentionDays(Option<u32>),
    AudioRetentionDays(Option<u32>),
    Notifications(NotificationSettings),
    UpdateChecksEnabled(bool),
    UpdateChannel(UpdateChannel),
    ShowInDock(bool),
    Onboarding(OnboardingState),
    DismissedBanners(Vec<String>),
    SeenNewTags(Vec<String>),
    FirstUseChoice(Option<String>),
    TypingWpm(Option<u32>),
    TypingHoursPerDay(f32),
    GreetingName(Option<String>),
    AutoLearnEnabled(bool),
    VibeCoding(VibeCodingSettings),
}

impl ForkSettingUpdate {
    /// The `key` this update is serialized with.
    pub fn key(&self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.get("key").and_then(Value::as_str).map(str::to_owned))
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_.-".contains(c))
}

fn check_id(field: &str, id: &str) -> Result<(), String> {
    if valid_id(id) {
        Ok(())
    } else {
        Err(format!("{field}: invalid id {id:?}"))
    }
}

fn check_ids(field: &str, ids: &[String]) -> Result<(), String> {
    if ids.len() > MAX_LIST_ITEMS {
        return Err(format!("{field}: more than {MAX_LIST_ITEMS} items"));
    }
    ids.iter().try_for_each(|id| check_id(field, id))
}

fn valid_chord(chord: &str) -> bool {
    !chord.is_empty()
        && chord.len() <= MAX_ID_LEN
        && chord.split('+').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        })
}

/// Validates one action's bindings: ≤ 4, well-formed chords, no duplicate.
pub fn validate_bindings(action: &str, bindings: &[HotkeyBinding]) -> Result<(), String> {
    check_id("hotkey action", action)?;
    if bindings.len() > MAX_BINDINGS_PER_ACTION {
        return Err(format!(
            "hotkey {action}: at most {MAX_BINDINGS_PER_ACTION} bindings"
        ));
    }
    for (i, b) in bindings.iter().enumerate() {
        if !valid_chord(&b.chord) {
            return Err(format!("hotkey {action}: invalid chord {:?}", b.chord));
        }
        if bindings[..i].contains(b) {
            return Err(format!("hotkey {action}: duplicate binding {:?}", b.chord));
        }
    }
    Ok(())
}

fn check_styles(styles: &StyleSettings) -> Result<(), String> {
    use StylePreset::*;
    if styles.personal == Upbeat {
        return Err("styles.personal: upbeat is not a personal preset".into());
    }
    for (group, preset) in [
        ("work", styles.work),
        ("email", styles.email),
        ("other", styles.other),
    ] {
        if preset == VeryRelaxed {
            return Err(format!("styles.{group}: very_relaxed is only for personal"));
        }
    }
    Ok(())
}

fn check_days(field: &str, days: Option<u32>) -> Result<(), String> {
    match days {
        Some(d) if d > MAX_RETENTION_DAYS => {
            Err(format!("{field}: at most {MAX_RETENTION_DAYS} days"))
        }
        _ => Ok(()),
    }
}

/// Semantic checks of one update (types are already checked by serde).
pub fn validate(update: &ForkSettingUpdate) -> Result<(), String> {
    use ForkSettingUpdate as U;
    match update {
        U::Hotkey { action, bindings } => validate_bindings(action, bindings),
        U::Styles(styles) => check_styles(styles),
        U::AutoApplyTransform(Some(id)) => check_id("auto_apply_transform", id),
        U::FirstUseChoice(Some(id)) => check_id("first_use_choice", id),
        U::BarHiddenUntilMs(Some(ms)) if *ms < 0 => {
            Err("bar_hidden_until_ms: must not be negative".into())
        }
        U::BarXByDisplay(map) => {
            if map.len() > MAX_BAR_DISPLAYS {
                return Err(format!(
                    "bar_x_by_display: more than {MAX_BAR_DISPLAYS} displays"
                ));
            }
            for (key, x) in map {
                check_id("bar_x_by_display", key)?;
                if !x.is_finite() || !(0.0..=1.0).contains(x) {
                    return Err(format!("bar_x_by_display.{key}: must be between 0 and 1"));
                }
            }
            Ok(())
        }
        U::TextRetentionDays(days) => match days {
            Some(0) => Err("text_retention_days: 0 would delete every entry".into()),
            _ => check_days("text_retention_days", *days),
        },
        U::AudioRetentionDays(days) => check_days("audio_retention_days", *days),
        U::DismissedBanners(ids) => check_ids("dismissed_banners", ids),
        U::SeenNewTags(ids) => check_ids("seen_new_tags", ids),
        U::TypingWpm(Some(wpm)) if *wpm == 0 || *wpm > MAX_TYPING_WPM => Err(format!(
            "typing_wpm: must be between 1 and {MAX_TYPING_WPM}"
        )),
        U::TypingHoursPerDay(h) if !h.is_finite() || *h < 0.0 || *h > 24.0 => {
            Err("typing_hours_per_day: must be between 0 and 24".into())
        }
        U::GreetingName(Some(name)) => {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                Err("greeting_name: use null to clear it".into())
            } else if trimmed.chars().count() > MAX_GREETING_NAME_LEN
                || trimmed.chars().any(char::is_control)
            {
                Err(format!(
                    "greeting_name: at most {MAX_GREETING_NAME_LEN} printable characters"
                ))
            } else {
                Ok(())
            }
        }
        U::Onboarding(state) => {
            if let Some(step) = &state.current_step {
                check_id("onboarding.current_step", step)?;
            }
            check_ids("onboarding.completed_steps", &state.completed_steps)?;
            check_ids("onboarding.checklist_done", &state.checklist_done)
        }
        U::VibeCoding(v) => check_ids("vibe_coding.editors", &v.editors),
        _ => Ok(()),
    }
}

/// Validates and applies one update.
pub fn apply_update(settings: &mut ForkSettings, update: ForkSettingUpdate) -> Result<(), String> {
    validate(&update)?;
    use ForkSettingUpdate as U;
    match update {
        U::Hotkey { action, bindings } => {
            settings.hotkeys.insert(action, bindings);
        }
        U::CleanupLevel(v) => settings.cleanup_level = v,
        U::Styles(v) => settings.styles = v,
        U::ContextAwareness(v) => settings.context_awareness = v,
        U::TransformsEnabled(v) => settings.transforms_enabled = v,
        U::CommandModeEnabled(v) => settings.command_mode_enabled = v,
        U::AutoApplyTransform(v) => settings.auto_apply_transform = v,
        U::BarAlwaysVisible(v) => settings.bar_always_visible = v,
        U::BarPosition(v) => settings.bar_position = v,
        U::BarHiddenUntilMs(v) => settings.bar_hidden_until_ms = v,
        U::BarHideInRecordings(v) => settings.bar_hide_in_recordings = v,
        U::BarXByDisplay(v) => settings.bar_x_by_display = v,
        U::LiveTextInPill(v) => settings.live_text_in_pill = v,
        U::ScratchpadOpenBehavior(v) => settings.scratchpad_open_behavior = v,
        U::ScratchpadOnBar(v) => settings.scratchpad_on_bar = v,
        U::TextRetentionDays(v) => settings.text_retention_days = v,
        U::AudioRetentionDays(v) => settings.audio_retention_days = v,
        U::Notifications(v) => settings.notifications = v,
        U::UpdateChecksEnabled(v) => settings.update_checks_enabled = v,
        U::UpdateChannel(v) => settings.update_channel = v,
        U::ShowInDock(v) => settings.show_in_dock = v,
        U::Onboarding(v) => settings.onboarding = v,
        U::DismissedBanners(v) => settings.dismissed_banners = v,
        U::SeenNewTags(v) => settings.seen_new_tags = v,
        U::FirstUseChoice(v) => settings.first_use_choice = v,
        U::TypingWpm(v) => settings.typing_wpm = v,
        U::TypingHoursPerDay(v) => settings.typing_hours_per_day = v,
        U::GreetingName(v) => settings.greeting_name = v.map(|s| s.trim().to_owned()),
        U::AutoLearnEnabled(v) => settings.auto_learn_enabled = v,
        U::VibeCoding(v) => settings.vibe_coding = v,
    }
    Ok(())
}

/// Checks a loaded settings object the way updates are checked; used after
/// salvage so a hand-edited file cannot smuggle in out-of-range values.
/// Returns the names of the fields that were reset to their defaults.
fn sanitize(settings: &mut ForkSettings) -> Vec<String> {
    let defaults = ForkSettings::default();
    let mut reset = Vec::new();
    let mut bad_actions = Vec::new();
    for (action, bindings) in &settings.hotkeys {
        if validate_bindings(action, bindings).is_err() {
            bad_actions.push(action.clone());
        }
    }
    for action in bad_actions {
        match defaults.hotkeys.get(&action) {
            Some(d) => settings.hotkeys.insert(action.clone(), d.clone()),
            None => settings.hotkeys.remove(&action),
        };
        reset.push(format!("hotkeys.{action}"));
    }
    // Every other field: rebuild it as an update and validate it.
    let checks: Vec<(&str, ForkSettingUpdate)> = vec![
        ("styles", ForkSettingUpdate::Styles(settings.styles)),
        (
            "auto_apply_transform",
            ForkSettingUpdate::AutoApplyTransform(settings.auto_apply_transform.clone()),
        ),
        (
            "bar_hidden_until_ms",
            ForkSettingUpdate::BarHiddenUntilMs(settings.bar_hidden_until_ms),
        ),
        (
            "bar_x_by_display",
            ForkSettingUpdate::BarXByDisplay(settings.bar_x_by_display.clone()),
        ),
        (
            "text_retention_days",
            ForkSettingUpdate::TextRetentionDays(settings.text_retention_days),
        ),
        (
            "audio_retention_days",
            ForkSettingUpdate::AudioRetentionDays(settings.audio_retention_days),
        ),
        (
            "dismissed_banners",
            ForkSettingUpdate::DismissedBanners(settings.dismissed_banners.clone()),
        ),
        (
            "seen_new_tags",
            ForkSettingUpdate::SeenNewTags(settings.seen_new_tags.clone()),
        ),
        (
            "first_use_choice",
            ForkSettingUpdate::FirstUseChoice(settings.first_use_choice.clone()),
        ),
        (
            "typing_wpm",
            ForkSettingUpdate::TypingWpm(settings.typing_wpm),
        ),
        (
            "typing_hours_per_day",
            ForkSettingUpdate::TypingHoursPerDay(settings.typing_hours_per_day),
        ),
        (
            "greeting_name",
            ForkSettingUpdate::GreetingName(settings.greeting_name.clone()),
        ),
        (
            "onboarding",
            ForkSettingUpdate::Onboarding(settings.onboarding.clone()),
        ),
        (
            "vibe_coding",
            ForkSettingUpdate::VibeCoding(settings.vibe_coding.clone()),
        ),
    ];
    for (field, update) in checks {
        if validate(&update).is_err() {
            reset_field(settings, &defaults, field);
            reset.push(field.to_owned());
        }
    }
    reset
}

fn reset_field(settings: &mut ForkSettings, defaults: &ForkSettings, field: &str) {
    let d = defaults.clone();
    match field {
        "styles" => settings.styles = d.styles,
        "auto_apply_transform" => settings.auto_apply_transform = d.auto_apply_transform,
        "bar_hidden_until_ms" => settings.bar_hidden_until_ms = d.bar_hidden_until_ms,
        "bar_x_by_display" => settings.bar_x_by_display = d.bar_x_by_display,
        "text_retention_days" => settings.text_retention_days = d.text_retention_days,
        "audio_retention_days" => settings.audio_retention_days = d.audio_retention_days,
        "dismissed_banners" => settings.dismissed_banners = d.dismissed_banners,
        "seen_new_tags" => settings.seen_new_tags = d.seen_new_tags,
        "first_use_choice" => settings.first_use_choice = d.first_use_choice,
        "typing_wpm" => settings.typing_wpm = d.typing_wpm,
        "typing_hours_per_day" => settings.typing_hours_per_day = d.typing_hours_per_day,
        "greeting_name" => settings.greeting_name = d.greeting_name,
        "onboarding" => settings.onboarding = d.onboarding,
        "vibe_coding" => settings.vibe_coding = d.vibe_coding,
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Decode, migrate, salvage
// ---------------------------------------------------------------------------

/// A migration from `from` to `from + 1`, on the raw JSON object (before it
/// is typed), so a renamed or reshaped field can be carried over.
pub struct Migration {
    pub from: u32,
    pub apply: fn(&mut Map<String, Value>),
}

/// Every migration, in order. Append only; never edit a shipped one.
///
/// v0 is a settings object without `schema_version` (pre-F-07 builds or a
/// hand-made file). v0 → v1 drops keys this build does not know (they could
/// only come from such a file) and stamps the version.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        from: 0,
        apply: migrate_v0_to_v1,
    },
    Migration {
        from: 1,
        apply: migrate_v1_to_v2,
    },
    Migration {
        from: 2,
        apply: migrate_v2_to_v3,
    },
    Migration {
        from: 3,
        apply: migrate_v3_to_v4,
    },
    Migration {
        from: 4,
        apply: migrate_v4_to_v5,
    },
    Migration {
        from: 5,
        apply: migrate_v5_to_v6,
    },
];

fn migrate_v0_to_v1(map: &mut Map<String, Value>) {
    let known = known_fields();
    map.retain(|k, _| known.contains(&k.as_str()));
}

/// v1 → v2 (S-06): a stored `hotkeys` map gets the new default binding of
/// `copy_last` (⌃⌘C), which v1 did not know; a map without `hotkeys` takes
/// all defaults anyway. Bindings the user set are never touched.
fn migrate_v1_to_v2(map: &mut Map<String, Value>) {
    let Some(Value::Object(hotkeys)) = map.get_mut("hotkeys") else {
        return;
    };
    let defaults = default_hotkeys();
    let id = hotkey_actions::COPY_LAST;
    if !hotkeys.contains_key(id) {
        if let Some(value) = defaults.get(id).and_then(|b| serde_json::to_value(b).ok()) {
            hotkeys.insert(id.to_string(), value);
        }
    }
}

/// v2 → v3 (S-21): add the built-in transform shortcuts. A user's existing
/// entries, including deliberately unset transform bindings, are preserved.
fn migrate_v2_to_v3(map: &mut Map<String, Value>) {
    let Some(Value::Object(hotkeys)) = map.get_mut("hotkeys") else {
        return;
    };
    let defaults = default_hotkeys();
    for id in ["transform.polish", "transform.prompt_engineer"] {
        if !hotkeys.contains_key(id) {
            if let Some(value) = defaults.get(id).and_then(|b| serde_json::to_value(b).ok()) {
                hotkeys.insert(id.to_string(), value);
            }
        }
    }
}

/// v3 → v4 (S-22): add ⌥O for the latest transform's diff. Preserve an
/// existing entry, including an intentionally unset one.
fn migrate_v3_to_v4(map: &mut Map<String, Value>) {
    let Some(Value::Object(hotkeys)) = map.get_mut("hotkeys") else {
        return;
    };
    let id = crate::fork::transforms::VIEW_CHANGES_ACTION;
    if !hotkeys.contains_key(id) {
        if let Some(value) = default_hotkeys()
            .get(id)
            .and_then(|bindings| serde_json::to_value(bindings).ok())
        {
            hotkeys.insert(id.to_string(), value);
        }
    }
}

/// v4 → v5 (S-23): add Command Mode's fn+Ctrl hold without changing an
/// existing entry, including one the user deliberately unset.
fn migrate_v4_to_v5(map: &mut Map<String, Value>) {
    let Some(Value::Object(hotkeys)) = map.get_mut("hotkeys") else {
        return;
    };
    let id = hotkey_actions::COMMAND_MODE;
    if !hotkeys.contains_key(id) {
        if let Some(value) = default_hotkeys()
            .get(id)
            .and_then(|bindings| serde_json::to_value(bindings).ok())
        {
            hotkeys.insert(id.to_string(), value);
        }
    }
}

/// v5 → v6 (S-24): the S-14 Scratchpad location choice becomes the open
/// behavior. Both former choices resume the last note; a missing value uses
/// the same default. Existing hotkeys and every other setting stay untouched.
fn migrate_v5_to_v6(map: &mut Map<String, Value>) {
    if map.contains_key("scratchpad_open_behavior") {
        map.insert(
            "scratchpad_open_behavior".into(),
            Value::String("resume_last".into()),
        );
    }
}

fn known_fields() -> Vec<&'static str> {
    // The field names of `ForkSettings`, taken from its own serialization.
    static FIELDS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    FIELDS
        .get_or_init(|| match serde_json::to_value(ForkSettings::default()) {
            Ok(Value::Object(map)) => map.keys().cloned().collect(),
            _ => Vec::new(),
        })
        .iter()
        .map(String::as_str)
        .collect()
}

/// What happened while reading the stored settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadStatus {
    /// No file or no `settings` key: defaults were used.
    Fresh,
    /// Read as is.
    Loaded,
    /// Migrated from an older `schema_version`.
    Migrated { from: u32 },
    /// The stored value was unreadable in part or in whole; the listed fields
    /// fell back to their defaults. The file must be backed up.
    Salvaged { dropped: Vec<String> },
    /// The file was written by a newer build (`schema_version` above
    /// [`CURRENT_SCHEMA_VERSION`]); readable fields were kept and the file
    /// must be backed up before it is overwritten.
    Newer { version: u32 },
}

impl LoadStatus {
    /// `true` when the original file must be backed up before saving.
    pub fn needs_backup(&self) -> bool {
        matches!(self, LoadStatus::Salvaged { .. } | LoadStatus::Newer { .. })
    }
}

/// Decodes the stored `settings` value with an explicit migration list.
pub fn decode_with(
    stored: Option<&Value>,
    migrations: &[Migration],
    current: u32,
) -> (ForkSettings, LoadStatus) {
    let Some(stored) = stored else {
        return (ForkSettings::default(), LoadStatus::Fresh);
    };
    let Some(map) = stored.as_object() else {
        return (
            ForkSettings::default(),
            LoadStatus::Salvaged {
                dropped: vec!["<not an object>".into()],
            },
        );
    };
    let mut map = map.clone();
    let version = map
        .get("schema_version")
        .and_then(Value::as_u64)
        .map(|v| u32::try_from(v).unwrap_or(u32::MAX))
        .unwrap_or(0);

    let mut status = LoadStatus::Loaded;
    if version > current {
        status = LoadStatus::Newer { version };
    } else if version < current {
        for m in migrations
            .iter()
            .filter(|m| m.from >= version && m.from < current)
        {
            (m.apply)(&mut map);
        }
        status = LoadStatus::Migrated { from: version };
    }
    map.insert("schema_version".into(), Value::from(current));

    let object = Value::Object(map.clone());
    let mut settings = match serde_json::from_value::<ForkSettings>(object) {
        Ok(s) => s,
        Err(_) => {
            let (s, dropped) = salvage(&map);
            if !matches!(status, LoadStatus::Newer { .. }) {
                status = LoadStatus::Salvaged { dropped };
            }
            s
        }
    };
    let reset = sanitize(&mut settings);
    if !reset.is_empty() {
        status = match status {
            LoadStatus::Salvaged { mut dropped } => {
                dropped.extend(reset);
                LoadStatus::Salvaged { dropped }
            }
            LoadStatus::Newer { version } => LoadStatus::Newer { version },
            _ => LoadStatus::Salvaged { dropped: reset },
        };
    }
    (settings, status)
}

/// Decodes the stored `settings` value with this build's [`MIGRATIONS`].
pub fn decode_stored(stored: Option<&Value>) -> (ForkSettings, LoadStatus) {
    decode_with(stored, MIGRATIONS, CURRENT_SCHEMA_VERSION)
}

/// Keeps every stored field that is valid on its own; the rest take their
/// defaults (same approach as Handy's `salvage_settings`, #1619).
fn salvage(stored: &Map<String, Value>) -> (ForkSettings, Vec<String>) {
    let mut merged = match serde_json::to_value(ForkSettings::default()) {
        Ok(Value::Object(map)) => map,
        _ => return (ForkSettings::default(), vec!["<defaults>".into()]),
    };
    let mut dropped = Vec::new();
    for (key, value) in stored {
        if !merged.contains_key(key) {
            dropped.push(key.clone());
            continue;
        }
        let previous = merged.insert(key.clone(), value.clone());
        if serde_json::from_value::<ForkSettings>(Value::Object(merged.clone())).is_err() {
            dropped.push(key.clone());
            match previous {
                Some(p) => merged.insert(key.clone(), p),
                None => merged.remove(key),
            };
        }
    }
    let settings = serde_json::from_value(Value::Object(merged)).unwrap_or_default();
    (settings, dropped)
}

/// State of the settings file on disk before the store plugin opens it.
/// The store plugin silently starts empty on a file it cannot parse and
/// would overwrite it on the next save, so the fork looks first.
#[derive(Debug)]
pub enum RawFile {
    Missing,
    /// Parsed as a JSON object; the `settings` value, if any.
    Readable(Option<Value>),
    /// Not JSON, or JSON that is not an object.
    Unreadable,
}

pub fn read_raw(path: &Path) -> RawFile {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return RawFile::Missing,
        Err(e) => {
            log::warn!("fork settings: cannot read {}: {e}", path.display());
            return RawFile::Unreadable;
        }
    };
    match serde_json::from_slice::<Value>(&bytes) {
        Ok(Value::Object(mut map)) => RawFile::Readable(map.remove(STORE_KEY)),
        _ => RawFile::Unreadable,
    }
}

/// Copies `path` to `fork-settings.<reason>-<ts>.json` next to it.
pub fn backup_file(path: &Path, reason: &str) -> Option<PathBuf> {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S%3f");
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    let target = path.with_file_name(format!("{stem}.{reason}-{stamp}.json"));
    match std::fs::copy(path, &target) {
        Ok(_) => Some(target),
        Err(e) => {
            log::error!(
                "fork settings: failed to back up {} to {}: {e}",
                path.display(),
                target.display()
            );
            None
        }
    }
}

/// Reads the file at `path`, backing it up when it is unreadable, salvaged or
/// newer. Returns the settings and whether they must be written back.
pub fn load_from_path(path: &Path) -> (ForkSettings, LoadStatus) {
    let (settings, status) = match read_raw(path) {
        RawFile::Missing => (ForkSettings::default(), LoadStatus::Fresh),
        RawFile::Unreadable => (
            ForkSettings::default(),
            LoadStatus::Salvaged {
                dropped: vec!["<unreadable file>".into()],
            },
        ),
        RawFile::Readable(value) => decode_stored(value.as_ref()),
    };
    match &status {
        LoadStatus::Salvaged { dropped } => {
            let backup = backup_file(path, "corrupt");
            log::warn!(
                "fork settings: {} was damaged; kept the readable fields, reset {:?} to defaults; backup: {:?}",
                path.display(),
                dropped,
                backup
            );
        }
        LoadStatus::Newer { version } => {
            let backup = backup_file(path, &format!("newer-v{version}"));
            log::warn!(
                "fork settings: {} has schema_version {version} (this build knows {CURRENT_SCHEMA_VERSION}); kept the readable fields; backup: {:?}",
                path.display(),
                backup
            );
        }
        LoadStatus::Migrated { from } => log::info!(
            "fork settings: migrated from schema_version {from} to {CURRENT_SCHEMA_VERSION}"
        ),
        LoadStatus::Fresh | LoadStatus::Loaded => {}
    }
    (settings, status)
}

// ---------------------------------------------------------------------------
// Product settings (ENFORCED, FIRST_RUN_DEFAULTS)
// ---------------------------------------------------------------------------

/// Inputs of [`apply_product_settings`] that do not live in either store.
#[derive(Debug, Clone, Copy)]
pub struct ProductContext {
    /// Launch at login may be turned on. `false` in debug builds and for any
    /// binary that is not inside an `.app` bundle, so dev and `--no-bundle`
    /// release runs never register a `target/` binary as a login item; the
    /// key then stays unrecorded and the first bundled run applies it.
    pub autostart_allowed: bool,
}

impl Default for ProductContext {
    fn default() -> Self {
        Self {
            autostart_allowed: !cfg!(debug_assertions)
                && std::env::current_exe()
                    .map(|exe| is_inside_app_bundle(&exe))
                    .unwrap_or(false),
        }
    }
}

/// True when `exe` lives at `<Name>.app/Contents/MacOS/<binary>`.
fn is_inside_app_bundle(exe: &std::path::Path) -> bool {
    let mut parts = exe.ancestors().skip(1);
    let macos = parts.next();
    let contents = parts.next();
    let bundle = parts.next();
    matches!(
        (macos, contents, bundle),
        (Some(m), Some(c), Some(b))
            if m.file_name().is_some_and(|n| n == "MacOS")
                && c.file_name().is_some_and(|n| n == "Contents")
                && b.extension().is_some_and(|e| e == "app")
    )
}

#[cfg(test)]
mod bundle_tests {
    use super::is_inside_app_bundle;
    use std::path::Path;

    #[test]
    fn only_binaries_inside_an_app_bundle_may_register_a_login_item() {
        assert!(is_inside_app_bundle(Path::new(
            "/Applications/Wispr Bro.app/Contents/MacOS/handy"
        )));
        assert!(!is_inside_app_bundle(Path::new(
            "/Users/me/wispr-bro/src-tauri/target/release/handy"
        )));
        assert!(!is_inside_app_bundle(Path::new("/usr/local/bin/handy")));
    }
}

/// An `ENFORCED` rule: sets the value and returns `true` if it changed.
pub(crate) struct EnforcedRule {
    pub key: &'static str,
    pub apply: fn(&mut AppSettings, &ForkSettings) -> bool,
}

/// Values the product depends on, re-applied on every start (R5 §4.4). The
/// user cannot change them through Handy's UI once the fork replaces it; a
/// change made there anyway is undone at the next start.
pub(crate) const ENFORCED: &[EnforcedRule] = &[
    EnforcedRule {
        key: "overlay_style",
        apply: enforce_overlay_off,
    },
    EnforcedRule {
        key: "recording_retention_period",
        apply: enforce_keep_history,
    },
    EnforcedRule {
        key: "post_process_enabled",
        apply: enforce_post_process_hotkey_off,
    },
    EnforcedRule {
        key: "experimental_enabled",
        apply: enforce_experimental_off,
    },
    EnforcedRule {
        key: "post_process_providers",
        apply: enforce_builtin_providers,
    },
    EnforcedRule {
        key: "post_process_provider_id",
        apply: enforce_local_provider,
    },
    EnforcedRule {
        key: "update_checks_enabled",
        apply: enforce_update_checks_mirror,
    },
    EnforcedRule {
        key: "filler_word_removal_enabled",
        apply: enforce_filler_removal_level,
    },
];

fn set<T: PartialEq>(slot: &mut T, value: T) -> bool {
    if *slot == value {
        false
    } else {
        *slot = value;
        true
    }
}

/// Gated by [`HANDY_OVERLAY_REPLACED`] (S-03).
fn enforce_overlay_off(handy: &mut AppSettings, _: &ForkSettings) -> bool {
    HANDY_OVERLAY_REPLACED && set(&mut handy.overlay_style, OverlayStyle::None)
}

/// The fork's retention job (S-09) owns deletion. Handy's `Never` deletes
/// nothing (`HistoryManager::cleanup_old_entries`), so `history_limit` is
/// left alone (unused under `Never`).
fn enforce_keep_history(handy: &mut AppSettings, _: &ForkSettings) -> bool {
    set(
        &mut handy.recording_retention_period,
        RecordingRetentionPeriod::Never,
    )
}

/// `post_process_enabled` is what registers Handy's second hotkey
/// (`transcribe_with_post_process`, `shortcut/mod.rs`). Cleanup runs through
/// the fork pipeline (S-12a), never through that hotkey.
fn enforce_post_process_hotkey_off(handy: &mut AppSettings, _: &ForkSettings) -> bool {
    set(&mut handy.post_process_enabled, false)
}

fn enforce_experimental_off(handy: &mut AppSettings, _: &ForkSettings) -> bool {
    set(&mut handy.experimental_enabled, false)
}

fn enforce_update_checks_mirror(handy: &mut AppSettings, fork: &ForkSettings) -> bool {
    set(&mut handy.update_checks_enabled, fork.update_checks_enabled)
}

/// R5 §4.10 stage 5: Handy's filler and stutter removal is off at cleanup
/// level None (S-12a), on otherwise. Mirrored at once when the level changes
/// (`fork_set_setting`) and re-applied on every start.
fn enforce_filler_removal_level(handy: &mut AppSettings, fork: &ForkSettings) -> bool {
    set(
        &mut handy.filler_word_removal_enabled,
        fork.cleanup_level != CleanupLevel::None,
    )
}

/// `true` for an `http(s)` URL whose host is `localhost` or a loopback IP
/// (see [`crate::fork::net::is_loopback`]).
pub fn is_loopback_url(url: &str) -> bool {
    crate::fork::net::is_loopback(url)
}

/// DEC-04 / F-08: Handy's stored provider list becomes exactly the fork's
/// built-in list ([`crate::fork::llm::default_providers`]). Providers stored
/// by an older build (OpenAI, Anthropic, Groq, …) are dropped with their API
/// key and model slots; `local` and Apple Intelligence are re-synced from the
/// built-ins; the user's custom provider keeps its base URL and API format
/// (its URL is checked by [`enforce_local_provider`]).
fn enforce_builtin_providers(handy: &mut AppSettings, _: &ForkSettings) -> bool {
    let builtins = crate::fork::llm::default_providers();
    let wanted: Vec<_> = builtins
        .into_iter()
        .map(|builtin| {
            match handy
                .post_process_providers
                .iter()
                .find(|p| p.id == builtin.id)
            {
                Some(existing) if builtin.id == CUSTOM_PROVIDER_ID => {
                    let mut kept = builtin.clone();
                    kept.base_url = existing.base_url.clone();
                    kept.api_format = existing.api_format;
                    kept
                }
                _ => builtin,
            }
        })
        .collect();
    let same = serde_json::to_value(&wanted).ok()
        == serde_json::to_value(&handy.post_process_providers).ok();
    let mut changed = !same;
    if changed {
        handy.post_process_providers = wanted;
    }
    let ids: Vec<String> = handy
        .post_process_providers
        .iter()
        .map(|p| p.id.clone())
        .collect();
    let before = handy.post_process_api_keys.len() + handy.post_process_models.len();
    handy.post_process_api_keys.retain(|id, _| ids.contains(id));
    handy.post_process_models.retain(|id, _| ids.contains(id));
    changed |= before != handy.post_process_api_keys.len() + handy.post_process_models.len();
    changed
}

/// `true` for providers that keep audio text on this Mac: the fork's `local`
/// provider, Apple Intelligence (on device), and the custom provider while it
/// points at loopback.
pub(crate) fn is_on_device_provider(handy: &AppSettings, id: &str) -> bool {
    let Some(provider) = handy.post_process_providers.iter().find(|p| p.id == id) else {
        return false;
    };
    match id {
        LOCAL_PROVIDER_ID | crate::settings::APPLE_INTELLIGENCE_PROVIDER_ID => true,
        _ => is_loopback_url(&provider.base_url),
    }
}

/// DEC-04: no cloud provider. The custom provider must stay on loopback (a
/// non-loopback URL is reset to Handy's Ollama default). A selected cloud
/// provider becomes `local` when that provider exists (F-08/S-10), otherwise
/// the loopback `custom` provider. Post-processing itself stays off
/// (`post_process_enabled`), so no request is made either way.
fn enforce_local_provider(handy: &mut AppSettings, _: &ForkSettings) -> bool {
    let mut changed = false;
    if let Some(custom) = handy
        .post_process_providers
        .iter_mut()
        .find(|p| p.id == CUSTOM_PROVIDER_ID)
    {
        if !is_loopback_url(&custom.base_url) {
            custom.base_url = LOOPBACK_CUSTOM_BASE_URL.into();
            changed = true;
        }
    }
    let selected = handy.post_process_provider_id.clone();
    if !is_on_device_provider(handy, &selected) {
        let target = if is_on_device_provider(handy, LOCAL_PROVIDER_ID) {
            LOCAL_PROVIDER_ID
        } else {
            CUSTOM_PROVIDER_ID
        };
        changed |= set(&mut handy.post_process_provider_id, target.into());
    }
    changed
}

/// A `FIRST_RUN_DEFAULTS` entry: applied once, then recorded.
pub(crate) struct FirstRunDefault {
    pub key: &'static str,
    pub apply: fn(&mut AppSettings),
    pub allowed: fn(&ProductContext) -> bool,
}

fn always(_: &ProductContext) -> bool {
    true
}

/// Applied once and recorded in `applied_first_run_defaults`, so a later
/// choice the user makes in Advanced persists across launches (R5 §4.4,
/// DEC-14).
pub(crate) const FIRST_RUN_DEFAULTS: &[FirstRunDefault] = &[
    // Concealed paste: Handy's receipt-sequenced paste (`reliable_paste`)
    // declares the nspasteboard concealed/transient types so clipboard
    // managers skip the transcript; it falls back to the legacy paste.
    FirstRunDefault {
        key: "reliable_paste",
        apply: |h| h.reliable_paste = true,
        allowed: always,
    },
    FirstRunDefault {
        key: "audio_feedback",
        apply: |h| h.audio_feedback = true,
        allowed: always,
    },
    FirstRunDefault {
        key: "log_level",
        apply: |h| h.log_level = LogLevel::Info,
        allowed: always,
    },
    FirstRunDefault {
        key: "paste_method",
        apply: |h| h.paste_method = PasteMethod::CtrlV,
        allowed: always,
    },
    FirstRunDefault {
        key: "keyboard_implementation",
        apply: |h| h.keyboard_implementation = KeyboardImplementation::HandyKeys,
        allowed: always,
    },
    FirstRunDefault {
        key: "theme",
        apply: |h| h.theme = Theme::System,
        allowed: always,
    },
    FirstRunDefault {
        key: "autostart_enabled",
        apply: |h| h.autostart_enabled = true,
        allowed: |ctx| ctx.autostart_allowed,
    },
];

/// What [`apply_product_settings`] changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProductReport {
    /// `ENFORCED` keys whose Handy value changed.
    pub enforced: Vec<&'static str>,
    /// `FIRST_RUN_DEFAULTS` keys applied (and recorded) by this call.
    pub first_run: Vec<&'static str>,
}

impl ProductReport {
    pub fn handy_changed(&self) -> bool {
        !self.enforced.is_empty() || !self.first_run.is_empty()
    }
}

/// Applies both lists to Handy's settings. Idempotent: a second call with the
/// result of the first changes nothing.
pub(crate) fn apply_product_settings(
    handy: &mut AppSettings,
    fork: &mut ForkSettings,
    ctx: &ProductContext,
) -> ProductReport {
    let mut report = ProductReport::default();
    for entry in FIRST_RUN_DEFAULTS {
        if fork
            .applied_first_run_defaults
            .iter()
            .any(|k| k == entry.key)
            || !(entry.allowed)(ctx)
        {
            continue;
        }
        (entry.apply)(handy);
        fork.applied_first_run_defaults.push(entry.key.to_owned());
        report.first_run.push(entry.key);
    }
    for rule in ENFORCED {
        if (rule.apply)(handy, fork) {
            report.enforced.push(rule.key);
        }
    }
    report
}

/// The ENFORCED values as read back from Handy's settings (self-check log).
pub(crate) fn enforced_snapshot(handy: &AppSettings) -> Value {
    serde_json::json!({
        "overlay_style": handy.overlay_style,
        "overlay_enforced": HANDY_OVERLAY_REPLACED,
        "recording_retention_period": handy.recording_retention_period,
        "post_process_enabled": handy.post_process_enabled,
        "experimental_enabled": handy.experimental_enabled,
        "post_process_provider_id": handy.post_process_provider_id,
        "custom_base_url": handy
            .post_process_providers
            .iter()
            .find(|p| p.id == CUSTOM_PROVIDER_ID)
            .map(|p| p.base_url.clone()),
        "update_checks_enabled": handy.update_checks_enabled,
    })
}

/// Handy's `AppSettings` as the public [`super::ForkRuntime`] trait carries
/// it. Opaque outside the crate, so Handy's settings type does not become
/// part of the crate's public interface.
pub struct HandySettings(pub(crate) AppSettings);

// ---------------------------------------------------------------------------
// Store handle (Tauri)
// ---------------------------------------------------------------------------

/// Fork settings handle, managed by the plugin `setup`. The file is opened by
/// the first [`ForkSettingsHandle::get`]/[`ForkSettingsHandle::update`],
/// because the store plugin is registered after the fork plugin and is not
/// usable during `setup`.
pub struct ForkSettingsHandle {
    /// Path given to the store plugin: relative (resolved against the app
    /// data dir) or absolute (portable mode, tests).
    store_path: PathBuf,
    cache: Mutex<Option<ForkSettings>>,
}

impl ForkSettingsHandle {
    pub fn new(store_path: impl Into<PathBuf>) -> Self {
        Self {
            store_path: store_path.into(),
            cache: Mutex::new(None),
        }
    }

    fn with_loaded<R: tauri::Runtime, T>(
        &self,
        app: &tauri::AppHandle<R>,
        f: impl FnOnce(
            &mut ForkSettings,
            &dyn Fn(&ForkSettings) -> Result<(), String>,
        ) -> Result<T, String>,
    ) -> Result<T, String> {
        use tauri_plugin_store::StoreExt;
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if cache.is_none() {
            let resolved = tauri_plugin_store::resolve_store_path(app, &self.store_path)
                .map_err(|e| format!("cannot resolve {}: {e}", self.store_path.display()))?;
            let (settings, status) = load_from_path(&resolved);
            let store = app
                .store(&self.store_path)
                .map_err(|e| format!("cannot open the fork settings store: {e}"))?;
            if status != LoadStatus::Loaded {
                write_store(&store, &settings)?;
            }
            *cache = Some(settings);
        }
        let save = |s: &ForkSettings| -> Result<(), String> {
            let store = app
                .store(&self.store_path)
                .map_err(|e| format!("cannot open the fork settings store: {e}"))?;
            write_store(&store, s)
        };
        let settings = cache.as_mut().expect("loaded above");
        let result = f(settings, &save);
        if result.is_ok() {
            SHOW_IN_DOCK.store(settings.show_in_dock, Ordering::Release);
        }
        result
    }

    /// Current settings (loads the file on first use).
    pub fn get<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
    ) -> Result<ForkSettings, String> {
        self.with_loaded(app, |s, _| Ok(s.clone()))
    }

    /// Runs `f` on the settings and saves them if `f` returns `Ok`. On error
    /// the in-memory copy is left as it was.
    pub fn update<R: tauri::Runtime, T>(
        &self,
        app: &tauri::AppHandle<R>,
        f: impl FnOnce(&mut ForkSettings) -> Result<T, String>,
    ) -> Result<(ForkSettings, T), String> {
        self.with_loaded(app, |settings, save| {
            let mut next = settings.clone();
            let out = f(&mut next)?;
            if next != *settings {
                save(&next)?;
                *settings = next;
            }
            Ok((settings.clone(), out))
        })
    }
}

fn write_store<R: tauri::Runtime>(
    store: &tauri_plugin_store::Store<R>,
    settings: &ForkSettings,
) -> Result<(), String> {
    let value = serde_json::to_value(settings).map_err(|e| e.to_string())?;
    store.set(STORE_KEY, value);
    store
        .save()
        .map_err(|e| format!("cannot save the fork settings: {e}"))
}

/// Called from the plugin `setup`: manages the handle (no file I/O).
pub fn register<R: tauri::Runtime, A: tauri::Manager<R>>(app: &A) {
    app.manage(ForkSettingsHandle::new(crate::portable::store_path(
        FORK_SETTINGS_FILE,
    )));
}

/// Called from `fork::on_ready`: loads the fork settings and applies the
/// product settings to Handy's settings. Returns the report, or `None` when
/// it was skipped.
pub fn on_ready<R: super::ForkRuntime>(app: &tauri::AppHandle<R>) -> Option<ProductReport> {
    use tauri::Manager;
    let handle = app.try_state::<ForkSettingsHandle>()?;
    let Some(HandySettings(mut handy)) = R::handy_settings(app) else {
        log::debug!("fork settings: Handy settings unavailable on this runtime; skipping");
        return None;
    };
    let identifier = app.config().identifier.clone();
    if !super::is_product_identity(&identifier) {
        log::warn!(
            "fork settings: running as {identifier:?}, not {PRODUCT_IDENTIFIER:?}; not touching Handy's settings (build with the identity overlay)"
        );
        return None;
    }
    let before = handy.clone();
    let ctx = ProductContext::default();
    let result = handle.update(app, |fork| {
        Ok(apply_product_settings(&mut handy, fork, &ctx))
    });
    match result {
        Ok((_, report)) => {
            if report.handy_changed() {
                R::write_handy_settings(app, &HandySettings(before), HandySettings(handy), &report);
                log::info!(
                    "fork settings: product settings applied (enforced {:?}, first-run {:?})",
                    report.enforced,
                    report.first_run
                );
            } else {
                log::info!("fork settings: product settings already in place");
            }
            Some(report)
        }
        Err(e) => {
            log::error!("fork settings: cannot apply product settings: {e}");
            None
        }
    }
}

/// Wry side of [`super::ForkRuntime::write_handy_settings`]: writes through
/// Handy's `settings::write_settings`, then runs the side effects Handy's own
/// setting commands would have run for the keys that changed.
pub(crate) fn write_handy_settings_wry(
    app: &tauri::AppHandle,
    before: &AppSettings,
    mut after: AppSettings,
    report: &ProductReport,
) {
    use tauri::{Emitter, Manager};
    let keyboard_target = after.keyboard_implementation;
    // Handy's command switches backends (unregister, re-register); it reads
    // the current value from the store, so the store keeps the old one until
    // the command runs.
    after.keyboard_implementation = before.keyboard_implementation;
    crate::settings::write_settings(app, after.clone());

    let changed = |key: &str| report.enforced.contains(&key) || report.first_run.contains(&key);
    if changed("post_process_enabled") && !after.post_process_enabled {
        if let Some(binding) = after.bindings.get("transcribe_with_post_process").cloned() {
            if let Err(e) = crate::shortcut::unregister_shortcut(app, binding) {
                log::debug!("fork settings: post-process hotkey was not registered: {e}");
            }
        }
        crate::secure_input::reconcile_fallback(app);
    }
    if changed("log_level") {
        let cli_debug = app
            .try_state::<crate::CliArgs>()
            .map(|a| a.debug)
            .unwrap_or(false);
        if !cli_debug {
            // Same runtime effect as Handy's `set_log_level` command.
            let level: tauri_plugin_log::LogLevel = after.log_level.into();
            let level: log::Level = level.into();
            crate::FILE_LOG_LEVEL.store(
                level.to_level_filter() as u8,
                std::sync::atomic::Ordering::Relaxed,
            );
        }
    }
    if changed("autostart_enabled") {
        crate::autostart::apply_autostart(app, after.autostart_enabled);
    }
    if changed("theme") {
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        crate::shortcut::apply_window_theme(app, after.theme);
    }
    if changed("overlay_style") {
        crate::overlay::update_overlay_enabled_cache(after.overlay_style != OverlayStyle::None);
    }
    if keyboard_target != before.keyboard_implementation {
        let name = match keyboard_target {
            KeyboardImplementation::Tauri => "tauri",
            KeyboardImplementation::HandyKeys => "handy_keys",
        };
        if let Err(e) =
            crate::shortcut::change_keyboard_implementation_setting(app.clone(), name.into())
        {
            log::warn!("fork settings: keyboard backend switch failed: {e}");
        }
    }
    // Handy's settings store listens for this and re-reads every setting.
    let _ = app.emit(
        "settings-changed",
        serde_json::json!({ "setting": "fork_product_settings" }),
    );
}

/// Test stand-in for Handy's settings store on `MockRuntime` (Handy's
/// `get_settings`/`write_settings` are typed to `Wry`).
#[cfg(test)]
#[derive(Default)]
pub(crate) struct FakeHandyStore(Mutex<Option<AppSettings>>);

#[cfg(test)]
impl FakeHandyStore {
    pub fn with(settings: AppSettings) -> Self {
        Self(Mutex::new(Some(settings)))
    }
    pub fn get(&self) -> Option<AppSettings> {
        self.0.lock().unwrap().clone()
    }
    pub fn set(&self, settings: AppSettings) {
        *self.0.lock().unwrap() = Some(settings);
    }
}
