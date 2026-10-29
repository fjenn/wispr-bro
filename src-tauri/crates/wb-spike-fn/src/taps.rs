// SPDX-License-Identifier: GPL-3.0-or-later
//! The two event taps RS-01 asks for, both from handy-keys 0.3.4 (MIT, no
//! fork):
//!
//! 1. **Handy's hotkey manager as shipped**: `HotkeyManager::new_with_blocking()`
//!    owned by one thread that polls `try_recv` every 10 ms, registering the
//!    default bindings with the same rules as `init_shortcuts` in
//!    `src-tauri/src/shortcut/handy_keys.rs` (every default binding except
//!    `cancel`; `transcribe_with_post_process` only when post-processing is
//!    on, which the fork turns off). The binding strings are Handy's macOS
//!    defaults from `get_default_settings` in `src-tauri/src/settings.rs`.
//!    Those functions need a Tauri `AppHandle`, so they are mirrored here and
//!    a test checks the upstream source still says the same.
//! 2. **The fork's own tap** (R5 §4.8): `KeyboardListener::new_with_blocking`
//!    with the dynamic blocking set; `{FN}` is always in it.
//!
//! Each tap's events are timestamped on receipt by a thread of its own and
//! forwarded to the engine.

use handy_keys::{
    BlockingHotkeys, Hotkey, HotkeyManager, HotkeyState, KeyEvent, KeyboardListener, Modifiers,
};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::system::{own_taps, TapInfo};

/// Handy's macOS default for `transcribe` (`settings.rs`, get_default_settings).
pub const HANDY_TRANSCRIBE_MACOS: &str = "option+space";
/// Handy's macOS default for `transcribe_with_post_process`.
pub const HANDY_POST_PROCESS_MACOS: &str = "option+shift+space";

/// What Handy's `init_shortcuts` registers on macOS, given the
/// post-processing setting (fork default: off, `fork/settings.rs`).
pub fn handy_bindings(post_process_enabled: bool) -> Vec<(&'static str, &'static str)> {
    let mut v = vec![("transcribe", HANDY_TRANSCRIBE_MACOS)];
    if post_process_enabled {
        v.push(("transcribe_with_post_process", HANDY_POST_PROCESS_MACOS));
    }
    // `cancel` ("escape") is skipped: Handy registers it only while recording.
    v
}

pub enum TapMsg {
    Fork {
        at: Instant,
        ev: KeyEvent,
    },
    Handy {
        at: Instant,
        binding: String,
        pressed: bool,
    },
    /// A tap's thread ended (listener stopped, permission revoked, …).
    TapEnded {
        which: &'static str,
    },
}

pub struct Taps {
    pub blocking: BlockingHotkeys,
    pub handy_registered: Vec<(String, Result<(), String>)>,
    pub handy_tap_ids: Vec<u32>,
    pub fork_tap_ids: Vec<u32>,
}

impl Taps {
    /// Labels the taps this process owns as "handy" / "fork".
    pub fn label(&self, taps: &[TapInfo]) -> Vec<(String, TapInfo)> {
        taps.iter()
            .map(|t| {
                let who = if self.handy_tap_ids.contains(&t.id) {
                    "handy"
                } else if self.fork_tap_ids.contains(&t.id) {
                    "fork"
                } else {
                    "other"
                };
                (who.to_string(), t.clone())
            })
            .collect()
    }
}

pub fn fn_only() -> Hotkey {
    Hotkey::new(Modifiers::FN, None).expect("fn hotkey")
}

fn new_ids(before: &[TapInfo], after: &[TapInfo]) -> Vec<u32> {
    let old: HashSet<u32> = before.iter().map(|t| t.id).collect();
    after
        .iter()
        .map(|t| t.id)
        .filter(|id| !old.contains(id))
        .collect()
}

/// Starts Handy's manager, then the fork's tap (the app's order: Handy's
/// shortcuts initialise at startup, the fork's tap after).
pub fn start<F>(forward: F) -> Result<Taps, String>
where
    F: Fn(TapMsg) + Send + Sync + 'static,
{
    let forward = Arc::new(forward);
    let before = own_taps();

    // 1. Handy's HotkeyManager, owned by its own thread like manager_thread().
    let (init_tx, init_rx) = mpsc::channel::<Result<Vec<(String, Result<(), String>)>, String>>();
    {
        let forward = Arc::clone(&forward);
        std::thread::Builder::new()
            .name("wb-spike-fn-handy".into())
            .spawn(move || {
                let manager = match HotkeyManager::new_with_blocking() {
                    Ok(m) => m,
                    Err(e) => {
                        let _ = init_tx.send(Err(format!("Handy's hotkey manager: {e}")));
                        return;
                    }
                };
                let mut by_id = HashMap::new();
                let mut results = Vec::new();
                for (binding, hotkey) in handy_bindings(false) {
                    let r = hotkey
                        .parse::<Hotkey>()
                        .map_err(|e| e.to_string())
                        .and_then(|h| manager.register(h).map_err(|e| e.to_string()));
                    match r {
                        Ok(id) => {
                            by_id.insert(id, binding.to_string());
                            results.push((binding.to_string(), Ok(())));
                        }
                        Err(e) => results.push((binding.to_string(), Err(e))),
                    }
                }
                let _ = init_tx.send(Ok(results));
                // Handy's loop: drain events, then wait 10 ms for a command.
                loop {
                    while let Some(event) = manager.try_recv() {
                        if let Some(binding) = by_id.get(&event.id) {
                            forward(TapMsg::Handy {
                                at: Instant::now(),
                                binding: binding.clone(),
                                pressed: event.state == HotkeyState::Pressed,
                            });
                        }
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            })
            .map_err(|e| e.to_string())?;
    }
    let handy_registered = init_rx
        .recv()
        .map_err(|_| "Handy's hotkey thread ended during start".to_string())??;
    let after_handy = own_taps();

    // 2. The fork's own blocking listener.
    let blocking: BlockingHotkeys = Arc::new(Mutex::new(HashSet::from([fn_only()])));
    let listener = KeyboardListener::new_with_blocking(Arc::clone(&blocking))
        .map_err(|e| format!("the fork's keyboard listener: {e}"))?;
    {
        let forward = Arc::clone(&forward);
        std::thread::Builder::new()
            .name("wb-spike-fn-fork-tap".into())
            .spawn(move || {
                while let Ok(ev) = listener.recv() {
                    forward(TapMsg::Fork {
                        at: Instant::now(),
                        ev,
                    });
                }
                forward(TapMsg::TapEnded { which: "fork" });
            })
            .map_err(|e| e.to_string())?;
    }
    // The listener thread installs its tap before new_with_blocking returns.
    let after_fork = own_taps();

    Ok(Taps {
        blocking,
        handy_registered,
        handy_tap_ids: new_ids(&before, &after_handy),
        fork_tap_ids: new_ids(&after_handy, &after_fork),
    })
}

/// What handy-keys' `ListenerState::should_block` would decide for `ev`
/// against `set` (state.rs:40-48 and the FlagsChanged rules of
/// platform/macos/listener.rs). Logged as `likely_blocked`: the set is read
/// on receipt, a few µs after the tap callback decided.
pub fn would_block(set: &HashSet<Hotkey>, ev: &KeyEvent) -> bool {
    if ev.key.is_none() && !ev.is_key_down {
        return false; // modifier releases are never blocked
    }
    set.iter()
        .any(|h| h.modifiers.matches(ev.modifiers) && h.key == ev.key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use handy_keys::Key;

    #[test]
    fn mirrors_handys_shipped_defaults() {
        let settings = include_str!("../../../src/settings.rs");
        assert!(settings.contains(&format!(
            "#[cfg(target_os = \"macos\")]\n    let default_shortcut = \"{HANDY_TRANSCRIBE_MACOS}\";"
        )));
        assert!(settings.contains(&format!(
            "#[cfg(target_os = \"macos\")]\n    let default_post_process_shortcut = \"{HANDY_POST_PROCESS_MACOS}\";"
        )));
        let init = include_str!("../../../src/shortcut/handy_keys.rs");
        assert!(init.contains("HotkeyManager::new_with_blocking()"));
        assert!(init.contains("if id == \"cancel\" {\n            continue;"));
        assert!(init.contains(
            "if id == \"transcribe_with_post_process\" && !user_settings.post_process_enabled {"
        ));
        assert!(init.contains("recv_timeout(std::time::Duration::from_millis(10))"));
        assert_eq!(handy_bindings(false), vec![("transcribe", "option+space")]);
        assert_eq!(handy_bindings(true).len(), 2);
        for (_, h) in handy_bindings(true) {
            h.parse::<Hotkey>().expect("parses");
        }
    }

    fn ev(
        modifiers: Modifiers,
        key: Option<Key>,
        down: bool,
        changed: Option<Modifiers>,
    ) -> KeyEvent {
        KeyEvent {
            modifiers,
            key,
            is_key_down: down,
            changed_modifier: changed,
        }
    }

    #[test]
    fn blocking_rules() {
        let mut set = HashSet::from([fn_only()]);
        let fn_down = ev(Modifiers::FN, None, true, Some(Modifiers::FN));
        let fn_up = ev(Modifiers::empty(), None, false, Some(Modifiers::FN));
        let shift_fn = ev(
            Modifiers::FN | Modifiers::SHIFT_LEFT,
            None,
            true,
            Some(Modifiers::FN),
        );
        let fn_home = ev(Modifiers::FN, Some(Key::Home), true, None);
        let fn_space = ev(Modifiers::FN, Some(Key::Space), true, None);
        let esc = ev(Modifiers::empty(), Some(Key::Escape), true, None);
        let fn_esc = ev(Modifiers::FN, Some(Key::Escape), true, None);
        assert!(would_block(&set, &fn_down));
        assert!(!would_block(&set, &fn_up));
        assert!(
            !would_block(&set, &shift_fn),
            "FN matches exactly: Shift+fn passes"
        );
        assert!(!would_block(&set, &fn_home), "chords pass through");
        assert!(!would_block(&set, &fn_space));
        set.insert(Hotkey::new(Modifiers::FN, Key::Space).unwrap());
        assert!(would_block(&set, &fn_space));
        assert!(!would_block(&set, &esc));
        set.insert(Hotkey::new(Modifiers::empty(), Key::Escape).unwrap());
        assert!(would_block(&set, &esc));
        assert!(
            !would_block(&set, &fn_esc),
            "{{Esc}} alone does not cover fn+Esc"
        );
        set.insert(Hotkey::new(Modifiers::FN, Key::Escape).unwrap());
        assert!(would_block(&set, &fn_esc));
    }
}
