// SPDX-License-Identifier: GPL-3.0-or-later
//! Every notification the app can post, as [`Notice`] constructors, plus the
//! fixed lists behind milestones, tips and feature announcements.
//!
//! Strings are `fork.json` keys under `rust.notify.*`, so the Rust side
//! (macOS notifications, `fork/i18n.rs`) and the inbox (i18next, namespace
//! `fork`) translate the same text. Wording is ours (R5 §1: never Wispr
//! Flow's text).

use super::policy::{Args, NotificationAction, NotificationKind, REASON_PERMISSION};

/// One notification to post.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub kind: NotificationKind,
    pub title_key: String,
    pub body_key: Option<String>,
    pub args: Args,
    /// What a click does.
    pub action: Option<NotificationAction>,
    /// Entries with a key exist at most once (`notifications.dedupe_key`).
    pub dedupe_key: Option<String>,
}

fn notice(kind: NotificationKind, key: &str) -> Notice {
    Notice {
        kind,
        title_key: format!("rust.notify.{key}.title"),
        body_key: Some(format!("rust.notify.{key}.body")),
        args: Args::new(),
        action: None,
        dedupe_key: None,
    }
}

impl Notice {
    fn arg(mut self, name: &str, value: impl ToString) -> Notice {
        self.args.insert(name.into(), value.to_string());
        self
    }

    fn action(mut self, action: NotificationAction) -> Notice {
        self.action = Some(action);
        self
    }

    fn dedupe(mut self, key: String) -> Notice {
        self.dedupe_key = Some(key);
        self
    }

    /// A speech model is downloaded: dictation works now.
    pub fn model_ready() -> Notice {
        notice(NotificationKind::ModelReady, "modelReady")
            .action(NotificationAction::page("dictation", None))
    }

    /// S-07b (B-208): no speech model meets the latency budget on this Mac;
    /// the fastest one is used. Said once (inbox only).
    pub fn slower_on_this_mac() -> Notice {
        notice(NotificationKind::SlowerMac, "slowerMac").dedupe("slower-mac".into())
    }

    /// An update is downloaded and installs at the next restart (S-28).
    pub fn update_ready(version: &str) -> Notice {
        notice(NotificationKind::UpdateReady, "updateReady")
            .arg("version", version)
            .action(NotificationAction::page("settings", Some("about")))
            .dedupe(format!("update:{version}"))
    }

    /// `count` takes recorded without a model are transcribed (S-07c).
    pub fn queued_takes_ready(count: u32) -> Notice {
        let mut n = notice(NotificationKind::QueuedTakesReady, "queuedTakesReady")
            .arg("count", count)
            .action(NotificationAction::page("settings", Some("history")));
        n.title_key = plural_key("rust.notify.queuedTakesReady.title", count);
        n
    }

    /// The hotkey tap could not start. `permission`: the Accessibility
    /// permission is missing (the entry then offers the Settings pane).
    pub fn shortcut_problem(permission: bool) -> Notice {
        let mut n = notice(NotificationKind::ShortcutProblem, "shortcutProblem")
            .action(NotificationAction::page("settings", Some("general")));
        if permission {
            n = n.arg("reason", REASON_PERMISSION);
            n.body_key = Some("rust.notify.shortcutProblem.bodyPermission".into());
        }
        n
    }

    /// A crash file from an earlier run (`crash-<ts>.txt`).
    pub fn crash_report(file_name: &str) -> Notice {
        notice(NotificationKind::CrashReport, "crashReport")
            .action(NotificationAction::RevealCrashReport {
                file_name: file_name.into(),
            })
            .dedupe(format!("crash:{file_name}"))
    }

    /// `fork.db` was unreadable and was recreated (the old file kept).
    pub fn data_recovered(at_ms: i64) -> Notice {
        notice(NotificationKind::DataRecovered, "dataRecovered")
            .action(NotificationAction::page("settings", Some("history")))
            .dedupe(format!("recovered:{at_ms}"))
    }

    /// `count` takes that were recording when the app quit (Quit & Reopen
    /// after a Microphone revoke, DEC-55) are in History now (S-08).
    pub fn take_recovered(count: u32) -> Notice {
        let mut n = notice(NotificationKind::TakeRecovered, "takeRecovered")
            .arg("count", count)
            .action(NotificationAction::page("settings", Some("history")));
        n.title_key = plural_key("rust.notify.takeRecovered.title", count);
        n
    }

    /// Total words passed `words` (one of [`WORD_MILESTONES`]).
    pub fn words_milestone(words: u64) -> Notice {
        notice(NotificationKind::Milestone, "milestoneWords")
            .arg("words", words)
            .action(NotificationAction::page("insights", None))
            .dedupe(format!("milestone:words:{words}"))
    }

    /// A streak of `days` (one of [`STREAK_MILESTONES`]) that began on
    /// `start_day` (`YYYY-MM-DD`): each streak celebrates each length once.
    pub fn streak_milestone(days: u32, start_day: &str) -> Notice {
        notice(NotificationKind::Milestone, "milestoneStreak")
            .arg("days", days)
            .action(NotificationAction::page("insights", None))
            .dedupe(format!("milestone:streak:{days}:{start_day}"))
    }

    /// One of [`TIPS`].
    pub fn tip(tip: &Tip) -> Notice {
        let key = format!("tips.{}", tip.key);
        let mut n = notice(NotificationKind::Tip, &key).dedupe(format!("tip:{}", tip.id));
        n.action = tip
            .action
            .as_ref()
            .map(|(page, section)| NotificationAction::page(page, *section));
        n
    }

    /// One of [`FEATURES`].
    pub fn announcement(feature: &Feature) -> Notice {
        let key = format!("features.{}", feature.key);
        let mut n =
            notice(NotificationKind::Announcement, &key).dedupe(format!("feature:{}", feature.id));
        n.action = Some(feature.action());
        n
    }
}

/// `<base>_one` for 1, else `<base>_other` (i18next's plural suffixes; the
/// inbox re-resolves plurals per language with i18next's own rules).
pub fn plural_key(base: &str, count: u32) -> String {
    if count == 1 {
        format!("{base}_one")
    } else {
        format!("{base}_other")
    }
}

/// Total-word milestones.
pub const WORD_MILESTONES: [u64; 9] = [
    1_000, 5_000, 10_000, 25_000, 50_000, 100_000, 250_000, 500_000, 1_000_000,
];

/// Day-streak milestones.
pub const STREAK_MILESTONES: [u32; 7] = [3, 7, 14, 30, 60, 100, 365];

/// The highest milestone in `list` that `value` has reached.
pub fn highest_reached<T: Copy + PartialOrd>(list: &[T], value: T) -> Option<T> {
    list.iter().copied().filter(|m| *m <= value).last()
}

/// A usage tip, posted once after the user has made `after_dictations`
/// dictations (Suggestions toggle). Only features that exist are tipped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tip {
    /// Stable id (dedupe key); never reuse one.
    pub id: &'static str,
    /// `fork.json` key under `rust.notify.tips`.
    pub key: &'static str,
    pub after_dictations: u64,
    /// (page, section) the click opens.
    pub action: Option<(&'static str, Option<&'static str>)>,
}

/// At most one tip a day, in this order.
pub const TIPS: &[Tip] = &[
    Tip {
        id: "hands_free",
        key: "handsFree",
        after_dictations: 2,
        action: None,
    },
    Tip {
        id: "cancel",
        key: "cancel",
        after_dictations: 5,
        action: None,
    },
    Tip {
        id: "history",
        key: "history",
        after_dictations: 10,
        action: Some(("settings", Some("history"))),
    },
    Tip {
        id: "dark_mode",
        key: "darkMode",
        after_dictations: 20,
        action: None,
    },
    Tip {
        id: "custom_words",
        key: "customWords",
        after_dictations: 40,
        action: Some(("dictionary", None)),
    },
];

/// Minimum time between two tips.
pub const TIP_INTERVAL_MS: i64 = 24 * 60 * 60 * 1000;

/// The tip to post now: the first one in [`TIPS`] that is due and not posted
/// yet, unless a tip was posted less than [`TIP_INTERVAL_MS`] ago.
pub fn next_tip(
    dictations: u64,
    posted: &dyn Fn(&str) -> bool,
    last_tip_at_ms: Option<i64>,
    now_ms: i64,
) -> Option<&'static Tip> {
    if last_tip_at_ms.is_some_and(|at| now_ms - at < TIP_INTERVAL_MS) {
        return None;
    }
    TIPS.iter()
        .find(|tip| dictations >= tip.after_dictations && !posted(tip.id))
}

/// A feature announced once, in the version it arrived in (C-202): a popup
/// in the Hub and an inbox entry (Announcements toggle).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Feature {
    /// Stable id (dedupe key and popup dismissal id `feature.<id>`).
    pub id: &'static str,
    /// `fork.json` key under `rust.notify.features`.
    pub key: &'static str,
    /// The app version that introduced it (exact `CARGO_PKG_VERSION`).
    pub version: &'static str,
    /// Hub page the popup's button opens; `None` opens the inbox.
    pub page: Option<(&'static str, Option<&'static str>)>,
}

impl Feature {
    pub fn action(&self) -> NotificationAction {
        match self.page {
            Some((page, section)) => NotificationAction::page(page, section),
            None => NotificationAction::OpenNotifications,
        }
    }
}

/// Announced features, newest last.
pub const FEATURES: &[Feature] = &[Feature {
    id: "inbox",
    key: "inbox",
    version: "1.0.0-alpha.1",
    page: None,
}];

/// The feature announced in `version`, if any (the newest when several).
pub fn feature_for_version(version: &str) -> Option<&'static Feature> {
    FEATURES.iter().rev().find(|f| f.version == version)
}
