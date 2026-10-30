// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed fork events.
//!
//! tauri-specta (rc.21) derives an event's wire name from its type name in
//! kebab-case and has no per-event override, so every fork event type is named
//! `Fork…` (`ForkPingEvent` → `fork-ping-event`). Events are collected by the
//! fork specta builder ([`super::specta_builder`]) and mounted into the same
//! `EventRegistry` that Handy's builder uses (see `docs/fork/ARCHITECTURE.md`).

use serde::{Deserialize, Serialize};
use specta::Type;

/// Sample event: emitted by `fork_ping` after it replies, so the debug panel
/// (and the round-trip tests) can prove the event path works end to end.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkPingEvent {
    /// Monotonic counter of `fork_ping` calls since launch.
    pub seq: u32,
}

/// Emitted after a fork setting changes (`fork_set_setting`), with the full
/// new settings, so every window's `useForkSettings()` store stays in sync.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkSettingsChanged {
    /// The `key` of the update that caused the change.
    pub key: String,
    pub settings: super::settings::ForkSettings,
}

/// What one take's delivery ended in (`fork::insert`, S-04): the take's
/// `paste_outcome` (R5 §4.3). `outcome` is `None` for a take cancelled
/// before the paste.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ForkPasteOutcome {
    Confirmed,
    Unverified,
    Card,
    Refused,
    Queued,
}

/// Why a take went to the card (`fork.json` > `insert.cardReason.*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ForkCardReason {
    Sleep,
    ScreenLocked,
    SessionInactive,
    PasteFailed,
    DiskFull,
}

/// Emitted once per delivered take. A card carries the text, so the pill can
/// show it with Copy (S-05); a pasted take does not.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkInsertOutcome {
    /// Delivery counter since launch.
    pub take: u32,
    pub outcome: Option<ForkPasteOutcome>,
    pub card_reason: Option<ForkCardReason>,
    pub text: Option<String>,
}

/// Emitted after a take changed the stats (`fork::db::stats`), so the Home
/// rail (S-15a) and Insights (S-25) refresh live.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ForkStatsUpdated {
    /// The local day whose numbers changed (`YYYY-MM-DD`).
    pub day: String,
}
