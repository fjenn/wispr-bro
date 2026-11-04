// SPDX-License-Identifier: GPL-3.0-or-later
//! Transforms: built-in and custom rewrites, auto-apply and the run handoff.

pub mod auto;
pub mod command;
#[cfg(test)]
pub mod eval;
pub mod literal;
#[cfg(test)]
mod long_polish_bench;
pub mod record;
pub mod replace;
pub mod run;
pub mod selection;
pub mod store;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Manager};

use crate::fork::settings::ForkSettingsHandle;
use crate::fork::shortcut::actions::{Handler, Registry};

pub const VIEW_CHANGES_ACTION: &str = "transform.view_changes";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformKind {
    Polish,
    PromptEngineer,
}

impl TransformKind {
    pub const ALL: [TransformKind; 2] = [TransformKind::Polish, TransformKind::PromptEngineer];

    pub fn builtin_key(self) -> &'static str {
        match self {
            TransformKind::Polish => "polish",
            TransformKind::PromptEngineer => "prompt_engineer",
        }
    }

    pub fn action_id(self) -> String {
        format!("transform.{}", self.builtin_key())
    }

    pub fn from_action(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.action_id() == id)
    }
}

fn sync_actions(registry: &Registry, enabled: bool, handler: impl Fn(TransformKind) -> Handler) {
    for kind in TransformKind::ALL {
        if enabled {
            if let Err(error) = registry.register(&kind.action_id(), handler(kind)) {
                log::warn!(
                    "transforms: could not register {}: {error}",
                    kind.builtin_key()
                );
            }
        } else {
            registry.unregister(&kind.action_id());
        }
    }
}

static CUSTOM_ACTIONS: Mutex<Vec<(String, i64)>> = Mutex::new(Vec::new());

fn sync_custom_actions<R: crate::fork::ForkRuntime>(app: &AppHandle<R>, enabled: bool) {
    let next = if enabled {
        app.try_state::<crate::fork::db::ForkDb>()
            .and_then(|db| db.with_conn(|conn| store::list(conn)).ok())
            .unwrap_or_default()
            .into_iter()
            .filter(|entry| entry.builtin_key.is_none())
            .map(|entry| (entry.action_id, entry.id))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let registry = crate::fork::shortcut::registry();
    let mut previous = CUSTOM_ACTIONS.lock().unwrap_or_else(|e| e.into_inner());
    for (action, _) in previous.iter() {
        registry.unregister(action);
    }
    if enabled {
        for (action, id) in &next {
            let app = app.clone();
            let id = *id;
            let action_id = action.clone();
            if let Err(error) = registry.register(
                action,
                Arc::new(move || {
                    if !crate::fork::tutorials::practice::intercept(&app, &action_id) {
                        run::spawn_custom(&app, id);
                    }
                }),
            ) {
                log::warn!("transforms: could not register {action}: {error}");
            }
        }
    }
    registry.unregister(VIEW_CHANGES_ACTION);
    if enabled {
        let app = app.clone();
        if let Err(error) = registry.register(
            VIEW_CHANGES_ACTION,
            Arc::new(move || {
                if !crate::fork::tutorials::practice::intercept(&app, VIEW_CHANGES_ACTION) {
                    record::view_changes(&app);
                }
            }),
        ) {
            log::warn!("transforms: could not register View changes: {error}");
        }
    }
    *previous = next;
}

/// Called before the fork tap compiles its profile. With Transforms off the
/// actions have no handlers, so their chords are neither caught nor swallowed.
pub fn on_ready<R: crate::fork::ForkRuntime>(app: &AppHandle<R>) {
    static OBSERVE_ESC: AtomicBool = AtomicBool::new(false);
    if !OBSERVE_ESC.swap(true, Ordering::AcqRel) {
        crate::fork::shortcut::registry().observe_esc(Arc::new(|context| {
            if !context.cancelled_take {
                run::esc();
            }
        }));
    }
    let enabled = app
        .try_state::<ForkSettingsHandle>()
        .and_then(|handle| handle.get(app).ok())
        .is_some_and(|settings| settings.transforms_enabled);
    sync_registration(app, enabled);
}

pub fn sync_registration<R: crate::fork::ForkRuntime>(app: &AppHandle<R>, enabled: bool) {
    sync_actions(&crate::fork::shortcut::registry(), enabled, |kind| {
        let app = app.clone();
        // A lesson's practice step in front gets the shortcut instead (S-31).
        Arc::new(move || {
            if !crate::fork::tutorials::practice::intercept(&app, &kind.action_id()) {
                run::spawn(&app, kind);
            }
        })
    });
    sync_custom_actions(app, enabled);
    run::settings_changed(enabled);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::settings::default_hotkeys;
    use crate::fork::shortcut::bindings;
    use crate::fork::shortcut::blocking::{self, Extra};
    use crate::fork::shortcut::processor::Blocking;

    #[test]
    fn hotkeys_register_and_swallow_only_while_transforms_are_on() {
        let registry = Registry::default();
        sync_actions(&registry, false, |_| Arc::new(|| {}));
        let off = bindings::compile(&default_hotkeys(), |id| registry.handles(id));
        assert!(off
            .keyed
            .iter()
            .all(|binding| TransformKind::from_action(&binding.action).is_none()));
        assert!(TransformKind::ALL
            .iter()
            .all(|kind| !registry.handles(&kind.action_id())));

        sync_actions(&registry, true, |_| Arc::new(|| {}));
        let on = bindings::compile(&default_hotkeys(), |id| registry.handles(id));
        for (kind, chord) in [
            (TransformKind::Polish, "option+1"),
            (TransformKind::PromptEngineer, "option+2"),
        ] {
            let binding = on
                .keyed
                .iter()
                .find(|binding| binding.action == kind.action_id())
                .unwrap();
            assert_eq!(binding.hotkey, chord.parse().unwrap());
            let blocked = blocking::hotkeys(&on, Blocking::default(), Extra::default());
            assert!(
                blocked.contains(&binding.hotkey),
                "{chord} must swallow its Option character"
            );
        }
        // These are fork runtime ids, never Handy manager binding ids.
        assert!(TransformKind::ALL
            .iter()
            .all(|kind| { !crate::fork::shortcut::owns_in(true, &kind.action_id()) }));
    }
}
