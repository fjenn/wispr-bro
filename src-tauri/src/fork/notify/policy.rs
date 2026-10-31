// SPDX-License-Identifier: GPL-3.0-or-later
//! What a notification is, and the rules about where it may go (DEC-21).
//!
//! Pure: no Tauri, no database, no AppKit. [`inbox_allowed`] decides whether
//! an entry is created at all (the Suggestions, Announcements and Milestones
//! toggles filter at creation, C-91); [`system_allowed`] decides whether a
//! macOS notification is also sent. Only four kinds may ever reach macOS:
//! model ready, update ready, queued takes ready and errors needing action
//! (today: shortcut problems).
//!
//! Actions: every entry has a default action (the click) stored with it.
//! Some kinds add buttons ([`buttons`]); [`resolve`] maps a click or a button
//! to the action to run, for the inbox and the macOS delegate alike.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::fork::settings::NotificationSettings;

/// Interpolation arguments of an entry's strings (`{{name}}` → value).
pub type Args = BTreeMap<String, String>;

/// The kind of a notification. Stored as its snake_case name in
/// `notifications.kind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum NotificationKind {
    /// A speech model finished downloading: dictation works.
    ModelReady,
    /// An app update is downloaded and installs on restart (S-28 posts it).
    UpdateReady,
    /// Takes recorded without a model were transcribed (S-07c posts it).
    QueuedTakesReady,
    /// The hotkey cannot work (no Accessibility permission, tap failure).
    ShortcutProblem,
    /// The app quit unexpectedly last time; a local crash file exists.
    CrashReport,
    /// `fork.db` could not be read and was recreated.
    DataRecovered,
    /// Takes that were recording when the app quit are in History (S-08,
    /// DEC-55).
    TakeRecovered,
    /// Word-count and streak milestones.
    Milestone,
    /// A usage tip (Suggestions toggle).
    Tip,
    /// A new feature in this version (Announcements toggle).
    Announcement,
}

impl NotificationKind {
    pub const ALL: [NotificationKind; 10] = [
        NotificationKind::ModelReady,
        NotificationKind::UpdateReady,
        NotificationKind::QueuedTakesReady,
        NotificationKind::ShortcutProblem,
        NotificationKind::CrashReport,
        NotificationKind::DataRecovered,
        NotificationKind::TakeRecovered,
        NotificationKind::Milestone,
        NotificationKind::Tip,
        NotificationKind::Announcement,
    ];

    /// The stored name (same as the serde name).
    pub fn as_str(self) -> &'static str {
        match self {
            NotificationKind::ModelReady => "model_ready",
            NotificationKind::UpdateReady => "update_ready",
            NotificationKind::QueuedTakesReady => "queued_takes_ready",
            NotificationKind::ShortcutProblem => "shortcut_problem",
            NotificationKind::CrashReport => "crash_report",
            NotificationKind::DataRecovered => "data_recovered",
            NotificationKind::TakeRecovered => "take_recovered",
            NotificationKind::Milestone => "milestone",
            NotificationKind::Tip => "tip",
            NotificationKind::Announcement => "announcement",
        }
    }

    pub fn parse(name: &str) -> Option<NotificationKind> {
        Self::ALL.into_iter().find(|k| k.as_str() == name)
    }

    /// The toggle group the kind belongs to.
    pub fn group(self) -> NotificationGroup {
        match self {
            NotificationKind::Tip => NotificationGroup::Suggestions,
            NotificationKind::Announcement => NotificationGroup::Announcements,
            NotificationKind::Milestone => NotificationGroup::Milestones,
            _ => NotificationGroup::App,
        }
    }
}

/// Toggle groups (C-91). `App` entries (models, updates, problems) are always
/// recorded in the inbox.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationGroup {
    App,
    Suggestions,
    Announcements,
    Milestones,
}

/// Whether an entry of `kind` is created at all.
pub fn inbox_allowed(kind: NotificationKind, s: &NotificationSettings) -> bool {
    match kind.group() {
        NotificationGroup::App => true,
        NotificationGroup::Suggestions => s.suggestions,
        NotificationGroup::Announcements => s.announcements,
        NotificationGroup::Milestones => s.milestones,
    }
}

/// Whether an entry of `kind` may also become a macOS notification. Only the
/// four DEC-21 kinds, each behind its own toggle and the master switch.
pub fn system_allowed(kind: NotificationKind, s: &NotificationSettings) -> bool {
    if !s.system {
        return false;
    }
    match kind {
        NotificationKind::ModelReady => s.model_ready,
        NotificationKind::UpdateReady => s.update_ready,
        NotificationKind::QueuedTakesReady => s.queued_takes_ready,
        NotificationKind::ShortcutProblem => s.errors,
        _ => false,
    }
}

/// What activating a notification does.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NotificationAction {
    /// Show the Hub on `page` (a Hub page id), optionally at `section`.
    OpenPage {
        page: String,
        section: Option<String>,
    },
    /// Show the Hub and open the notifications panel.
    OpenNotifications,
    /// System Settings › Privacy & Security › Accessibility.
    OpenAccessibilitySettings,
    /// Reveal a crash file (by name, inside the crash folder) in Finder.
    RevealCrashReport { file_name: String },
    /// Copy the latest dictation to the clipboard (no window).
    CopyLatest,
}

impl NotificationAction {
    pub fn page(page: &str, section: Option<&str>) -> NotificationAction {
        NotificationAction::OpenPage {
            page: page.into(),
            section: section.map(str::to_string),
        }
    }

    /// `true` when running it needs the Hub on screen.
    pub fn shows_window(&self) -> bool {
        matches!(
            self,
            NotificationAction::OpenPage { .. } | NotificationAction::OpenNotifications
        )
    }
}

/// A button on a notification (macOS action, inbox button).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum NotificationButton {
    /// "Copy latest".
    CopyLatest,
    /// "Open": the entry's default action.
    Open,
    /// "Open Settings" (Accessibility pane).
    OpenAccessibility,
}

impl NotificationButton {
    pub const ALL: [NotificationButton; 3] = [
        NotificationButton::CopyLatest,
        NotificationButton::Open,
        NotificationButton::OpenAccessibility,
    ];

    /// The macOS action identifier.
    pub fn id(self) -> &'static str {
        match self {
            NotificationButton::CopyLatest => "copy_latest",
            NotificationButton::Open => "open",
            NotificationButton::OpenAccessibility => "open_accessibility",
        }
    }

    pub fn parse(id: &str) -> Option<NotificationButton> {
        Self::ALL.into_iter().find(|b| b.id() == id)
    }

    /// The label's `fork.json` key (shared by Rust and the inbox).
    pub fn label_key(self) -> &'static str {
        match self {
            NotificationButton::CopyLatest => "rust.notify.buttons.copyLatest",
            NotificationButton::Open => "rust.notify.buttons.open",
            NotificationButton::OpenAccessibility => "rust.notify.buttons.openAccessibility",
        }
    }
}

/// Value of the `reason` arg of a shortcut problem caused by a missing
/// Accessibility permission.
pub const REASON_PERMISSION: &str = "permission";

/// The buttons an entry shows. A shortcut problem offers the Accessibility
/// pane only when the permission is what is missing (`reason` arg).
pub fn buttons(kind: NotificationKind, args: &Args) -> Vec<NotificationButton> {
    match kind {
        NotificationKind::QueuedTakesReady => {
            vec![NotificationButton::CopyLatest, NotificationButton::Open]
        }
        NotificationKind::ShortcutProblem
            if args.get("reason").map(String::as_str) == Some(REASON_PERMISSION) =>
        {
            vec![NotificationButton::OpenAccessibility]
        }
        _ => Vec::new(),
    }
}

/// macOS categories: every kind with buttons has one, registered with all
/// its buttons. Kept in step with [`buttons`] (tested).
pub const CATEGORIES: [(&str, &[NotificationButton]); 2] = [
    (
        "wb.queued_takes",
        &[NotificationButton::CopyLatest, NotificationButton::Open],
    ),
    (
        "wb.shortcut_problem",
        &[NotificationButton::OpenAccessibility],
    ),
];

/// The macOS category of an entry, when it shows buttons.
pub fn category_id(kind: NotificationKind, args: &Args) -> Option<&'static str> {
    if buttons(kind, args).is_empty() {
        return None;
    }
    match kind {
        NotificationKind::QueuedTakesReady => Some(CATEGORIES[0].0),
        NotificationKind::ShortcutProblem => Some(CATEGORIES[1].0),
        _ => None,
    }
}

/// The action a click (`button == None`) or a button runs. A button the
/// entry does not offer falls back to the click, so a stale macOS
/// notification can never run something its entry does not show.
pub fn resolve(
    kind: NotificationKind,
    args: &Args,
    default: Option<&NotificationAction>,
    button: Option<NotificationButton>,
) -> Option<NotificationAction> {
    let offered = button.filter(|b| buttons(kind, args).contains(b));
    match offered {
        Some(NotificationButton::CopyLatest) => Some(NotificationAction::CopyLatest),
        Some(NotificationButton::OpenAccessibility) => {
            Some(NotificationAction::OpenAccessibilitySettings)
        }
        Some(NotificationButton::Open) | None => default.cloned(),
    }
}
