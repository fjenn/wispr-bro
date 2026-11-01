// SPDX-License-Identifier: GPL-3.0-or-later
//! S-21 Transforms core: two built-in rewrites on the fork hotkey tap.

pub mod record;
pub mod replace;
pub mod run;
pub mod selection;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::{AppHandle, Manager, Runtime};

use crate::fork::settings::ForkSettingsHandle;
use crate::fork::shortcut::actions::{Handler, Registry};

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

/// Called before the fork tap compiles its profile. With Transforms off the
/// actions have no handlers, so their chords are neither caught nor swallowed.
pub fn on_ready<R: Runtime + 'static>(app: &AppHandle<R>) {
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

pub fn sync_registration<R: Runtime + 'static>(app: &AppHandle<R>, enabled: bool) {
    sync_actions(&crate::fork::shortcut::registry(), enabled, |kind| {
        let app = app.clone();
        Arc::new(move || run::spawn(&app, kind))
    });
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
