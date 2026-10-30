// SPDX-License-Identifier: GPL-3.0-or-later
//! Raw tap events → grammar inputs (pure). Which events matter depends on
//! the active profile's trigger (`bindings.rs`): the fn key, or a held
//! modifier pair such as Ctrl+Opt. Also the stuck-trigger guard of rule 9.
//!
//! Rules, from RS-01 and the ported vectors (OpenWhispr `FN_INTERRUPTED`,
//! anarlog `decision.rs`, MIT; see `docs/fork/PORTS.md`):
//! - The trigger going down/up is `FnDown`/`FnUp`; key repeats and
//!   duplicate edges are absorbed by the grammar.
//! - Every non-modifier key-down (and a mouse click with a modifier held,
//!   which is all handy-keys reports) is `KeyDown`. Key-ups are not inputs.
//! - Other modifiers are not inputs: Shift/Cmd/Opt/Ctrl with fn neither
//!   interrupt nor stop a take (DEC-31: Ctrl passes through while Command
//!   Mode is off). For a modifier-pair trigger, adding a third modifier
//!   neither ends nor starts the press.

use handy_keys::{Hotkey, Key as HkKey, KeyEvent, Modifiers};
use serde::Serialize;

use super::processor::{Input, Key, Ms};

/// What starts a press.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub enum Trigger {
    /// The fn / 🌐 key (DEC-09 default).
    Fn,
    /// Two or more modifiers held together (fallback: Ctrl+Opt). Compound
    /// bits only (`Modifiers::CTRL`, not `CTRL_LEFT`): either side counts.
    Mods(#[serde(skip)] Modifiers),
}

impl Trigger {
    /// Ctrl+Opt (the fallback profile, DEC-09).
    pub const CTRL_OPT: Trigger = Trigger::Mods(Modifiers::CTRL.union(Modifiers::OPT));

    /// The trigger as a modifier-only hotkey (for the blocking set).
    pub fn modifiers(self) -> Modifiers {
        match self {
            Trigger::Fn => Modifiers::FN,
            Trigger::Mods(m) => m,
        }
    }

    /// From a modifier-only hotkey: `fn` alone, or ≥ 2 modifier groups
    /// without fn. A single ordinary modifier (Shift, Opt, …) is not a
    /// trigger: it would fire on every shortcut that uses it.
    pub fn from_hotkey(h: &Hotkey) -> Option<Trigger> {
        if h.key.is_some() {
            return None;
        }
        let m = compound(h.modifiers);
        if m == Modifiers::FN {
            return Some(Trigger::Fn);
        }
        if m.contains(Modifiers::FN) {
            return None;
        }
        let groups = [
            Modifiers::CMD,
            Modifiers::SHIFT,
            Modifiers::CTRL,
            Modifiers::OPT,
        ]
        .iter()
        .filter(|g| m.intersects(**g))
        .count();
        (groups >= 2).then_some(Trigger::Mods(m))
    }

    /// Whether `modifiers` (side-specific, as the tap reports them) hold the
    /// whole trigger.
    pub fn held_in(self, modifiers: Modifiers) -> bool {
        match self {
            Trigger::Fn => modifiers.contains(Modifiers::FN),
            Trigger::Mods(m) => [
                Modifiers::CMD,
                Modifiers::SHIFT,
                Modifiers::CTRL,
                Modifiers::OPT,
            ]
            .iter()
            .filter(|g| m.intersects(**g))
            .all(|g| modifiers.intersects(*g)),
        }
    }
}

/// Side-specific bits folded into their compound group bits.
pub fn compound(m: Modifiers) -> Modifiers {
    let mut out = m & Modifiers::FN;
    for g in [
        Modifiers::CMD,
        Modifiers::SHIFT,
        Modifiers::CTRL,
        Modifiers::OPT,
    ] {
        if m.intersects(g) {
            out |= g;
        }
    }
    out
}

/// One translated event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Not an input (modifier change, key-up, …).
    Nothing,
    /// Feed this to the grammar.
    Input(Input),
}

/// Translates raw events for one trigger. Holds the trigger's last state.
#[derive(Clone, Debug)]
pub struct Translator {
    trigger: Trigger,
    down: bool,
}

impl Translator {
    pub fn new(trigger: Trigger) -> Self {
        Self {
            trigger,
            down: false,
        }
    }

    pub fn trigger(&self) -> Trigger {
        self.trigger
    }

    /// The trigger is down as far as the events say.
    pub fn is_down(&self) -> bool {
        self.down
    }

    /// Forgets the trigger state (stuck recovery synthesised a release).
    pub fn force_up(&mut self) {
        self.down = false;
    }

    pub fn translate(&mut self, ev: &KeyEvent) -> Decision {
        match ev.key {
            // FlagsChanged of a modifier (key None), or of fn.
            None => self.modifiers_changed(ev),
            Some(k) if ev.is_key_down => Decision::Input(Input::KeyDown(classify(k))),
            Some(_) => Decision::Nothing,
        }
    }

    fn modifiers_changed(&mut self, ev: &KeyEvent) -> Decision {
        let now_down = match self.trigger {
            // fn is tracked by its own edge, as in the RS-01 spike: Shift or
            // Cmd going down while fn is held must not read as a release.
            Trigger::Fn => match ev.changed_modifier {
                Some(m) if m == Modifiers::FN => ev.is_key_down,
                _ => self.down,
            },
            t => t.held_in(ev.modifiers),
        };
        if now_down == self.down {
            return Decision::Nothing;
        }
        self.down = now_down;
        Decision::Input(if now_down { Input::FnDown } else { Input::FnUp })
    }
}

/// The grammar's view of a key.
pub fn classify(k: HkKey) -> Key {
    match k {
        HkKey::Space => Key::Space,
        HkKey::Escape => Key::Escape,
        other => Key::Other(key_name(other)),
    }
}

/// Key name for logs and effects.
pub fn key_name(k: HkKey) -> String {
    match k {
        HkKey::Escape => "Escape".into(),
        HkKey::Space => "Space".into(),
        other => other.to_string(),
    }
}

/// Key name for the log: ordinary typing (letters, digits, punctuation)
/// with the trigger up is never recorded by name. The tap sees every
/// keystroke on the Mac; the log only needs to know a key was pressed.
pub fn logged_key_name(k: HkKey, trigger_down: bool) -> String {
    let name = key_name(k);
    let printable =
        name.chars().count() == 1 || name.starts_with("Keypad") || name == "JisUnderscore";
    if trigger_down || !printable {
        name
    } else {
        "(typing)".into()
    }
}

/// Rule 9: "if the tap thinks fn is down but the system's flag state says
/// up for 1 s, synthesize the release".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StuckGuard {
    /// Since when the tap and the system have disagreed.
    since: Option<Ms>,
}

impl StuckGuard {
    /// How long the disagreement must last.
    pub const AFTER_MS: Ms = 1_000;
    /// How often the engine looks while the trigger is down.
    pub const POLL_MS: Ms = 250;

    /// Returns true when the release should be synthesised now.
    /// `physically_down` is `None` when the system state is unavailable.
    pub fn observe(&mut self, now: Ms, tap_down: bool, physically_down: Option<bool>) -> bool {
        match (tap_down, physically_down) {
            (true, Some(false)) => {
                let since = *self.since.get_or_insert(now);
                if now.saturating_sub(since) >= Self::AFTER_MS {
                    self.since = None;
                    return true;
                }
                false
            }
            _ => {
                self.since = None;
                false
            }
        }
    }
}
