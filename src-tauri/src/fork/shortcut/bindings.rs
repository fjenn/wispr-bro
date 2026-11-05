// SPDX-License-Identifier: GPL-3.0-or-later
//! Fork hotkey bindings (fork settings `hotkeys`, R5 §4.4) → the profile the
//! engine runs. Up to 4 bindings per action, any action may be unset, each
//! binding has its own activation (checked by `fork::settings`); this file
//! adds what the grammar can run and conflict detection across actions.
//!
//! What S-01 runs:
//! - one **trigger** (`fn`, ≥ 2 modifiers such as `ctrl+option`, or
//!   modifiers plus a key such as `option+space`), bound to `dictate` as
//!   `hold` and/or to `hands_free` as `double_tap`;
//! - `<trigger>+space` bound to `hands_free` as `press` (rule 6; not for a
//!   modifier+key trigger);
//! - `escape` bound to `cancel` as `press` (rule 7);
//! - keyed chords bound as `press` to an action the registry handles
//!   (`actions.rs`), for example ⌃⌘V once paste-last exists.
//!
//! Anything else is reported as [`Issue::Unsupported`] and not registered,
//! so a binding never silently half-works.

use std::collections::{BTreeMap, HashSet};

use handy_keys::{Hotkey, Key};
use serde::Serialize;
use specta::Type;

use super::decision::{compound, Trigger};
use crate::fork::settings::{hotkey_actions, HotkeyActivation, HotkeyBinding};

/// The fallback profile (DEC-09): Ctrl+Opt hold, double-tap Ctrl+Opt or
/// Ctrl+Opt+Space for hands-free. Activated from onboarding or the hotkeys
/// panel, never by default.
pub fn fallback_hotkeys() -> BTreeMap<String, Vec<HotkeyBinding>> {
    use hotkey_actions::*;
    use HotkeyActivation::*;
    let b = |chord: &str, activation| HotkeyBinding {
        chord: chord.into(),
        activation,
    };
    let mut map = crate::fork::settings::default_hotkeys();
    map.insert(DICTATE.into(), vec![b("ctrl+option", Hold)]);
    map.insert(
        HANDS_FREE.into(),
        vec![b("ctrl+option", DoubleTap), b("ctrl+option+space", Press)],
    );
    map
}

/// A problem with one binding. The binding is skipped; the rest still run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Issue {
    /// The chord does not parse.
    Invalid { action: String, chord: String },
    /// Two actions claim the same chord in ways that cannot both work.
    Conflict {
        action: String,
        other: String,
        chord: String,
    },
    /// Valid, but not something this build can run (see the module doc).
    Unsupported {
        action: String,
        chord: String,
        reason: String,
    },
}

/// A keyed binding that fires an action on key-down.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyedBinding {
    pub hotkey: Hotkey,
    pub action: String,
}

/// What the engine runs.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Profile {
    /// `None`: no trigger bound (dictation by hotkey is off).
    pub trigger: Option<Trigger>,
    /// `dictate` holds the trigger (push to talk).
    pub hold: bool,
    /// `hands_free` double-taps the trigger.
    pub double_tap: bool,
    /// `hands_free` is `<trigger>+space`.
    pub trigger_space: bool,
    /// `cancel` is Esc.
    pub esc_cancels: bool,
    /// S-23: fn+Ctrl selects instruction mode for the take that starts.
    pub command_mode: bool,
    pub keyed: Vec<KeyedBinding>,
    pub issues: Vec<Issue>,
}

/// `fn+space` → `fn+space`, `Option+Ctrl` → `ctrl+option`: one spelling per
/// chord, so equal chords compare equal.
pub fn normalize(chord: &str) -> Option<(String, Hotkey)> {
    let hotkey: Hotkey = chord.parse().ok()?;
    let m = compound(hotkey.modifiers);
    let mut parts: Vec<String> = Vec::new();
    for (bit, name) in [
        (handy_keys::Modifiers::FN, "fn"),
        (handy_keys::Modifiers::CTRL, "ctrl"),
        (handy_keys::Modifiers::OPT, "option"),
        (handy_keys::Modifiers::SHIFT, "shift"),
        (handy_keys::Modifiers::CMD, "command"),
    ] {
        if m.contains(bit) {
            parts.push(name.into());
        }
    }
    if let Some(k) = hotkey.key {
        parts.push(k.to_string().to_ascii_lowercase());
    }
    let hotkey = Hotkey {
        modifiers: m,
        key: hotkey.key,
    };
    Some((parts.join("+"), hotkey))
}

/// Builds the profile. `handled` says whether the action registry has a
/// handler for a keyed action id.
pub fn compile(
    hotkeys: &BTreeMap<String, Vec<HotkeyBinding>>,
    handled: impl Fn(&str) -> bool,
) -> Profile {
    use hotkey_actions::*;
    use HotkeyActivation::*;
    let mut p = Profile::default();
    // Pass 1: parse and detect conflicts. Same chord in two actions is a
    // conflict unless it is the trigger's hold + double-tap pair.
    let mut seen: Vec<(String, String, HotkeyActivation)> = Vec::new(); // (chord, action, act)
    let mut parsed: Vec<(String, String, Hotkey, HotkeyActivation)> = Vec::new();
    for (action, bindings) in hotkeys {
        for b in bindings {
            let Some((chord, hotkey)) = normalize(&b.chord) else {
                p.issues.push(Issue::Invalid {
                    action: action.clone(),
                    chord: b.chord.clone(),
                });
                continue;
            };
            let clash = seen.iter().find(|(c, a, act)| {
                c == &chord
                    && a != action
                    && !matches!((act, b.activation), (Hold, DoubleTap) | (DoubleTap, Hold))
            });
            if let Some((_, other, _)) = clash {
                p.issues.push(Issue::Conflict {
                    action: action.clone(),
                    other: other.clone(),
                    chord,
                });
                continue;
            }
            seen.push((chord.clone(), action.clone(), b.activation));
            parsed.push((action.clone(), chord, hotkey, b.activation));
        }
    }
    let unsupported = |p: &mut Profile, action: &str, chord: &str, reason: &str| {
        p.issues.push(Issue::Unsupported {
            action: action.into(),
            chord: chord.into(),
            reason: reason.into(),
        })
    };
    // Pass 2: the trigger comes from `dictate` hold, else `hands_free`
    // double-tap.
    let trigger_of = |want_action: &str, want: HotkeyActivation| {
        parsed
            .iter()
            .filter(|(a, _, _, act)| a == want_action && *act == want)
            .find_map(|(_, _, h, _)| Trigger::from_hotkey(h))
    };
    p.trigger = trigger_of(DICTATE, Hold).or_else(|| trigger_of(HANDS_FREE, DoubleTap));
    let mut keyed_seen: HashSet<Hotkey> = HashSet::new();
    for (action, chord, hotkey, act) in &parsed {
        let as_trigger = Trigger::from_hotkey(hotkey);
        let is_trigger = as_trigger.is_some() && as_trigger == p.trigger;
        let is_trigger_space = p.trigger.is_some_and(|t| {
            t.key().is_none()
                && hotkey.key == Some(Key::Space)
                && compound(hotkey.modifiers) == t.modifiers()
        });
        match (action.as_str(), act) {
            (DICTATE, Hold) if is_trigger => p.hold = true,
            (HANDS_FREE, DoubleTap) if is_trigger => p.double_tap = true,
            (HANDS_FREE, Press) if is_trigger_space => p.trigger_space = true,
            (CANCEL, Press) if *chord == "escape" => p.esc_cancels = true,
            (COMMAND_MODE, Hold) if *chord == "fn+ctrl" && handled(action) => p.command_mode = true,
            (COMMAND_MODE, _) => unsupported(
                &mut p,
                action,
                chord,
                "Command Mode uses fn+ctrl as a hold binding while enabled",
            ),
            (DICTATE, Hold) | (HANDS_FREE, DoubleTap) if as_trigger.is_some() => unsupported(
                &mut p,
                action,
                chord,
                "one trigger at a time: this one differs from the dictation trigger",
            ),
            (_, Hold | DoubleTap) => unsupported(
                &mut p,
                action,
                chord,
                "hold and double-tap need fn, two modifiers, or a modifier with a key",
            ),
            (DICTATE | HANDS_FREE | CANCEL, _) => unsupported(
                &mut p,
                action,
                chord,
                "this action runs on the trigger, <trigger>+space or Esc",
            ),
            (PRESS_ENTER, Press) if !super::actions::press_enter::is_allowed(hotkey) => {
                unsupported(
                    &mut p,
                    action,
                    chord,
                    "Press Enter cannot be bound to Return itself",
                )
            }
            (_, Press) if hotkey.key.is_some() && handled(action) => {
                if keyed_seen.insert(*hotkey) {
                    p.keyed.push(KeyedBinding {
                        hotkey: *hotkey,
                        action: action.clone(),
                    });
                }
            }
            (_, Press) if hotkey.key.is_some() => {
                unsupported(&mut p, action, chord, "no handler for this action yet")
            }
            _ => unsupported(
                &mut p,
                action,
                chord,
                "only press bindings with a key fire actions",
            ),
        }
    }
    p
}
