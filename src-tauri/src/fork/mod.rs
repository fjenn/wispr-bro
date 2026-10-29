// SPDX-License-Identifier: GPL-3.0-or-later
//! Wispr Bro fork layer.
//!
//! All fork code lives under this module. Handy's files reach it only through
//! the one-line hooks listed in R5 §4.2:
//!
//! - `main.rs`: [`early_init`] before `run()`.
//! - `lib.rs` plugin chain: `.plugin(fork::init())`.
//! - `lib.rs` builder: `.invoke_handler(fork::compose_invoke(invoke_handler))`.
//!
//! Init order (tauri 2.11.5, see `docs/fork/ARCHITECTURE.md` for line refs):
//! plugins initialize inside `Builder::build()`, so the plugin `setup` below
//! runs *before* Handy's `.setup()` closure, which only runs on the first
//! `RunEvent::Ready`. The plugin's `on_event(RunEvent::Ready)` is dispatched
//! right after Handy's closure returns, so [`on_ready`] can rely on Handy's
//! managers being in state.

pub mod commands;
pub mod events;
#[cfg(test)]
mod testutil;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use tauri::ipc::Invoke;
use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Manager, RunEvent, Runtime};
use tauri_specta::Builder;

use crate::CliArgs;

/// Name of the in-app plugin. It registers no commands (R5 §4.1).
pub const PLUGIN_NAME: &str = "fork";

/// Prefix that routes a command to the fork handler.
pub const FORK_COMMAND_PREFIX: &str = "fork_";

/// Where the fork bindings are written in debug builds. Relative to the
/// working directory of `tauri dev` (`src-tauri/`), exactly like Handy's
/// `../src/bindings.ts` export in `lib.rs`.
pub const FORK_BINDINGS_PATH: &str = "../src/fork/bindings.ts";

/// Handy's model manager type as the fork reads it from Tauri state. Tests
/// swap in a fake with the same `Arc<…>` state shape, because Handy's real
/// `ModelManager::new` needs a `Wry` `AppHandle` that unit tests cannot build.
#[cfg(not(test))]
pub(crate) type HandyModelManager = crate::managers::model::ModelManager;
#[cfg(test)]
pub(crate) type HandyModelManager = testutil::FakeModelManager;

/// Fork state registered by the plugin `setup`.
#[derive(Debug, Default)]
pub struct ForkState {
    ready: AtomicBool,
    ping_seq: AtomicU32,
}

impl ForkState {
    /// `true` once [`on_ready`] has run (never in headless CLI modes).
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    fn mark_ready(&self) {
        self.ready.store(true, Ordering::Release);
    }

    fn next_ping_seq(&self) -> u32 {
        self.ping_seq.fetch_add(1, Ordering::Relaxed) + 1
    }
}

/// Called from `main.rs` before `handy_app_lib::run()`.
///
/// Empty for now. F-08 fills it (sets `HF_HOME` under the app data dir and
/// installs the panic hook). Keep it free of Tauri calls: it runs before the
/// Tauri builder exists.
pub fn early_init() {}

/// The in-app Tauri plugin `fork`.
///
/// `setup` only registers fork state ([`ForkState`]) and mounts the fork events
/// into the shared `EventRegistry`. Everything that touches Handy managers,
/// windows, menus or the tray belongs in [`on_ready`].
pub fn init<R: ForkRuntime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new(PLUGIN_NAME)
        .setup(|app, _api| {
            app.manage(ForkState::default());
            // Merges into the same registry Handy's builder mounts into later
            // (`EventRegistry::get_or_manage`), so order does not matter.
            specta_builder::<R>().mount_events(app);
            Ok(())
        })
        .on_event(|app, event| {
            if let RunEvent::Ready = event {
                on_ready(app);
            }
        })
        .build()
}

/// `true` for Handy's headless one-shot CLI modes. Mirrors `headless_mode` in
/// `lib.rs::run`.
pub fn is_headless(args: &CliArgs) -> bool {
    args.transcribe_file.is_some() || args.list_devices || args.list_models
}

/// Runs once, on `RunEvent::Ready`, after Handy's `.setup()` closure (and so
/// after `initialize_core_logic`) has returned.
pub fn on_ready<R: Runtime>(app: &AppHandle<R>) {
    let headless = app
        .try_state::<CliArgs>()
        .map(|args| is_headless(&args))
        .unwrap_or(false);
    if headless {
        log::debug!("fork: headless CLI mode, skipping on_ready");
        return;
    }

    let Some(state) = app.try_state::<ForkState>() else {
        log::error!("fork: ForkState missing at Ready; plugin setup did not run");
        return;
    };

    if app
        .try_state::<std::sync::Arc<HandyModelManager>>()
        .is_none()
    {
        log::warn!("fork: Handy's ModelManager is not managed at Ready");
    }

    state.mark_ready();
    log::info!("fork ready");
}

/// Builds the fork's tauri-specta builder for one concrete runtime.
///
/// A second builder with no `plugin_name`, so its commands are app commands
/// and its events keep their plain names. It is a macro because specta's
/// `collect_functions!` expands each command into an inner item, which cannot
/// name a generic parameter of an enclosing fn; each runtime therefore gets its
/// own instantiation through [`ForkRuntime`].
macro_rules! fork_specta_builder {
    ($runtime:path) => {
        tauri_specta::Builder::<$runtime>::new()
            .commands(tauri_specta::collect_commands![
                commands::fork_ping::<$runtime>,
                commands::fork_model_status::<$runtime>,
            ])
            .events(tauri_specta::collect_events![events::ForkPingEvent])
    };
}

/// A Tauri runtime the fork can be mounted on: `tauri::Wry` in the app and
/// `tauri::test::MockRuntime` in tests.
pub trait ForkRuntime: Runtime {
    /// The fork's tauri-specta builder for this runtime.
    fn fork_specta_builder() -> Builder<Self>;
}

impl ForkRuntime for tauri::Wry {
    fn fork_specta_builder() -> Builder<Self> {
        fork_specta_builder!(tauri::Wry)
    }
}

#[cfg(test)]
impl ForkRuntime for tauri::test::MockRuntime {
    fn fork_specta_builder() -> Builder<Self> {
        fork_specta_builder!(tauri::test::MockRuntime)
    }
}

/// The fork's tauri-specta builder (see [`ForkRuntime`]).
pub fn specta_builder<R: ForkRuntime>() -> Builder<R> {
    R::fork_specta_builder()
}

/// `true` when `command` belongs to the fork handler.
pub fn is_fork_command(command: &str) -> bool {
    command.starts_with(FORK_COMMAND_PREFIX)
}

/// Wraps Handy's invoke handler: commands named `fork_*` go to the fork
/// handler, everything else to Handy's. Routing is by name, before the call,
/// because a handler consumes the `Invoke`.
///
/// In debug builds this also exports `src/fork/bindings.ts`, the same way
/// Handy exports `src/bindings.ts`.
pub fn compose_invoke<R, H>(handy: H) -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static
where
    R: ForkRuntime,
    H: Fn(Invoke<R>) -> bool + Send + Sync + 'static,
{
    let fork_builder = specta_builder::<R>();

    // Tests check the file instead of rewriting it (`bindings_are_current`).
    #[cfg(all(debug_assertions, not(test)))] // <- Only export on non-release builds
    fork_builder
        .export(bindings_language(), FORK_BINDINGS_PATH)
        .expect("Failed to export fork typescript bindings");

    let fork = fork_builder.invoke_handler();
    move |invoke: Invoke<R>| {
        if is_fork_command(invoke.message.command()) {
            fork(invoke)
        } else {
            handy(invoke)
        }
    }
}

/// Same exporter settings as Handy's `lib.rs` export.
#[cfg(debug_assertions)]
fn bindings_language() -> specta_typescript::Typescript {
    use specta_typescript::{BigIntExportBehavior, Typescript};
    Typescript::default().bigint(BigIntExportBehavior::Number)
}
