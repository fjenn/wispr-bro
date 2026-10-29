// SPDX-License-Identifier: GPL-3.0-or-later
//! Wispr Bro fork layer.
//!
//! All fork code lives under this module. Handy's files reach it only through
//! the one-line hooks listed in R5 §4.2:
//!
//! - `main.rs`: [`early_init`] before `run()`.
//! - `lib.rs` plugin chain: `.plugin(fork::init())`.
//! - `lib.rs` builder: `.invoke_handler(fork::compose_invoke(invoke_handler))`.
//! - `tray.rs`: [`tray::extend`] in `build_menu`, [`tray::icon_path`] in
//!   `get_icon_path` (F-01).
//! - `lib.rs` log target, `actions.rs` and `managers/transcription_jobs.rs`
//!   WAV names: [`FILE_PREFIX`] (F-01).
//!
//! Init order (tauri 2.11.5, see `docs/fork/ARCHITECTURE.md` for line refs):
//! plugins initialize inside `Builder::build()`, so the plugin `setup` below
//! runs *before* Handy's `.setup()` closure, which only runs on the first
//! `RunEvent::Ready`. The plugin's `on_event(RunEvent::Ready)` is dispatched
//! right after Handy's closure returns, so [`on_ready`] can rely on Handy's
//! managers being in state.

pub mod commands;
pub mod db;
pub mod events;
pub mod i18n;
pub mod settings;
#[cfg(test)]
mod testutil;
pub mod tray;

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

/// Prefix of the files the app names itself: the log file
/// (`wispr-bro.log`) and recordings (`wispr-bro-<ts>.wav`,
/// `wispr-bro-file-<ms>.wav`). Handy used `handy`. Existing recordings keep
/// their names; history rows store the full file name.
pub const FILE_PREFIX: &str = "wispr-bro";

/// Name of a dictation recording (`actions.rs` hook; Handy: `handy-<ts>.wav`).
pub fn recording_file_name(timestamp_secs: i64) -> String {
    format!("{FILE_PREFIX}-{timestamp_secs}.wav")
}

/// Name of a transcribed-file copy (`managers/transcription_jobs.rs` hook;
/// Handy: `handy-file-<ms>.wav`).
pub fn file_job_name(timestamp_millis: i64) -> String {
    format!("{FILE_PREFIX}-file-{timestamp_millis}.wav")
}

/// Our updater kill switch. Honoured alongside Handy's `HANDY_DISABLE_UPDATER`
/// (E-94, R5 §4.11).
pub const WISPRBRO_DISABLE_UPDATER: &str = "WISPRBRO_DISABLE_UPDATER";
/// Handy's updater kill switch, read once by
/// `settings::update_checks_forced_disabled`.
pub const HANDY_DISABLE_UPDATER: &str = "HANDY_DISABLE_UPDATER";

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

/// Same truthiness as Handy's `utils::env_flag_enabled`, on a value.
fn flag_value_enabled(value: Option<&str>) -> bool {
    value.is_some_and(|v| {
        !matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "" | "0" | "false" | "no" | "off"
        )
    })
}

/// E-94: when `WISPRBRO_DISABLE_UPDATER` is truthy and Handy's own flag is
/// not, returns the variable to set so Handy's single reader
/// (`settings::update_checks_forced_disabled`) sees the updater as forced off.
/// No upstream edit is needed: Handy reads the env lazily, after `main.rs`
/// has called [`early_init`].
pub(crate) fn updater_env_to_set(
    get: impl Fn(&str) -> Option<String>,
) -> Option<(&'static str, &'static str)> {
    let ours = flag_value_enabled(get(WISPRBRO_DISABLE_UPDATER).as_deref());
    let handy = flag_value_enabled(get(HANDY_DISABLE_UPDATER).as_deref());
    (ours && !handy).then_some((HANDY_DISABLE_UPDATER, "1"))
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
/// F-01: maps `WISPRBRO_DISABLE_UPDATER` onto Handy's `HANDY_DISABLE_UPDATER`
/// (see [`updater_env_to_set`]). F-08 adds `HF_HOME` under the app data dir
/// and the panic hook. Keep it free of Tauri calls: it runs before the Tauri
/// builder exists, while the process is still single-threaded.
pub fn early_init() {
    if let Some((key, value)) = updater_env_to_set(|k| std::env::var(k).ok()) {
        std::env::set_var(key, value);
    }
}

/// The in-app Tauri plugin `fork`.
///
/// `setup` only registers fork state ([`ForkState`]) and mounts the fork events
/// into the shared `EventRegistry`. Everything that touches Handy managers,
/// windows, menus or the tray belongs in [`on_ready`].
pub fn init<R: ForkRuntime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new(PLUGIN_NAME)
        .setup(|app, _api| {
            app.manage(ForkState::default());
            db::register(app);
            settings::register(app);
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
pub fn on_ready<R: ForkRuntime>(app: &AppHandle<R>) {
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

    // Handy names its main window "Handy" (`lib.rs` setup); it exists by now.
    let locale = i18n::current_locale(R::handy_app_language(app).as_deref());
    if let Some(window) = app.get_webview_window("main") {
        let title = i18n::t(locale, "rust.window.mainTitle", &[]);
        // Applied asynchronously on the main thread; the self-check reads it
        // back later.
        match window.set_title(&title) {
            Ok(()) => log::info!("fork: main window title set to {title:?}"),
            Err(e) => log::warn!("fork: failed to set the main window title: {e}"),
        }
    }

    // F-07: fork settings, ENFORCED and FIRST_RUN_DEFAULTS (every start).
    settings::on_ready(app);

    state.mark_ready();
    log::info!(
        "fork ready ({} v{}, locale {locale})",
        app.package_info().name,
        env!("CARGO_PKG_VERSION")
    );
    commands::selfcheck::start(app);
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
                commands::selfcheck::fork_selfcheck_report::<$runtime>,
                commands::settings::fork_get_settings::<$runtime>,
                commands::settings::fork_set_setting::<$runtime>,
            ])
            .events(tauri_specta::collect_events![
                events::ForkPingEvent,
                events::ForkSettingsChanged
            ])
    };
}

/// A Tauri runtime the fork can be mounted on: `tauri::Wry` in the app and
/// `tauri::test::MockRuntime` in tests.
pub trait ForkRuntime: Runtime {
    /// The fork's tauri-specta builder for this runtime.
    fn fork_specta_builder() -> Builder<Self>;

    /// Handy's app-language setting (`AppSettings::app_language`). Handy's
    /// settings API is typed to `Wry`; other runtimes return `None` (the OS
    /// locale is used then).
    fn handy_app_language(app: &AppHandle<Self>) -> Option<String>;

    /// Handy's settings through Handy's `settings::get_settings` (F-07).
    /// `None` where Handy's settings API is unavailable.
    fn handy_settings(app: &AppHandle<Self>) -> Option<settings::HandySettings>;

    /// Writes Handy's settings through Handy's `settings::write_settings` and
    /// runs the runtime side effects of the keys in `report` (F-07).
    fn write_handy_settings(
        app: &AppHandle<Self>,
        before: &settings::HandySettings,
        after: settings::HandySettings,
        report: &settings::ProductReport,
    );
}

impl ForkRuntime for tauri::Wry {
    fn fork_specta_builder() -> Builder<Self> {
        fork_specta_builder!(tauri::Wry)
    }

    fn handy_app_language(app: &AppHandle<Self>) -> Option<String> {
        Some(crate::settings::get_settings(app).app_language)
    }

    fn handy_settings(app: &AppHandle<Self>) -> Option<settings::HandySettings> {
        Some(settings::HandySettings(crate::settings::get_settings(app)))
    }

    fn write_handy_settings(
        app: &AppHandle<Self>,
        before: &settings::HandySettings,
        after: settings::HandySettings,
        report: &settings::ProductReport,
    ) {
        settings::write_handy_settings_wry(app, &before.0, after.0, report);
    }
}

#[cfg(test)]
impl ForkRuntime for tauri::test::MockRuntime {
    fn fork_specta_builder() -> Builder<Self> {
        fork_specta_builder!(tauri::test::MockRuntime)
    }

    fn handy_app_language(_app: &AppHandle<Self>) -> Option<String> {
        None
    }

    /// Tests stand in for Handy's store with `settings::FakeHandyStore`
    /// (managed state); without it the product step is skipped.
    fn handy_settings(app: &AppHandle<Self>) -> Option<settings::HandySettings> {
        app.try_state::<settings::FakeHandyStore>()?
            .get()
            .map(settings::HandySettings)
    }

    fn write_handy_settings(
        app: &AppHandle<Self>,
        _before: &settings::HandySettings,
        after: settings::HandySettings,
        _report: &settings::ProductReport,
    ) {
        if let Some(store) = app.try_state::<settings::FakeHandyStore>() {
            store.set(after.0);
        }
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

#[cfg(test)]
mod identity_tests {
    //! F-01: the effective config (Handy's `tauri.conf.json` with the fork
    //! overlay applied the way `tauri --config` applies it, an RFC 7396 JSON
    //! merge patch) carries our identity, and the version lines agree.

    use serde_json::Value;
    use std::path::Path;

    fn read_json(rel: &str) -> Value {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    /// RFC 7396 merge patch, as `json_patch::merge` (used by tauri-cli and
    /// tauri-build for `--config` / `TAURI_CONFIG`): objects merge
    /// recursively, `null` deletes, everything else (arrays too) replaces.
    fn merge_patch(target: &mut Value, patch: &Value) {
        let Value::Object(patch_map) = patch else {
            *target = patch.clone();
            return;
        };
        if !target.is_object() {
            *target = Value::Object(Default::default());
        }
        let map = target.as_object_mut().unwrap();
        for (key, value) in patch_map {
            if value.is_null() {
                map.remove(key);
            } else {
                merge_patch(map.entry(key.clone()).or_insert(Value::Null), value);
            }
        }
    }

    fn effective_config() -> (Value, tauri::utils::config::Config) {
        let mut merged = read_json("tauri.conf.json");
        merge_patch(&mut merged, &read_json("tauri.fork.conf.json"));
        let parsed = serde_json::from_value(merged.clone()).expect("effective config parses");
        (merged, parsed)
    }

    fn overlay_version() -> String {
        read_json("tauri.fork.conf.json")["version"]
            .as_str()
            .expect("overlay has a version")
            .to_string()
    }

    #[test]
    fn effective_config_has_our_identity() {
        let (raw, config) = effective_config();
        assert_eq!(config.product_name.as_deref(), Some("Wispr Bro"));
        assert_eq!(config.identifier, "com.wisprbro.desktop");
        assert_eq!(config.version.as_deref(), Some(overlay_version().as_str()));
        assert_eq!(
            config.bundle.macos.minimum_system_version.as_deref(),
            Some("13.0")
        );

        // Icons come only from icons-fork/ and exist.
        let icons = &config.bundle.icon;
        assert!(!icons.is_empty());
        for icon in icons {
            assert!(icon.starts_with("icons-fork/"), "{icon}");
            assert!(
                Path::new(env!("CARGO_MANIFEST_DIR")).join(icon).is_file(),
                "missing {icon}"
            );
        }

        // The updater must never point at Handy's feed or trust Handy's key
        // (F-02 fills in the real owner, endpoint and key).
        let updater = &raw["plugins"]["updater"];
        let handy_key = read_json("tauri.conf.json")["plugins"]["updater"]["pubkey"].clone();
        assert_ne!(updater["pubkey"], handy_key);
        for endpoint in updater["endpoints"].as_array().unwrap() {
            let url = endpoint.as_str().unwrap();
            assert!(!url.contains("cjpais") && !url.contains("Handy"), "{url}");
        }

        // Handy's file is untouched for identity (overlay only).
        let base = read_json("tauri.conf.json");
        assert_eq!(base["identifier"], "com.pais.handy");
        assert_eq!(base["productName"], "Handy");
    }

    #[test]
    fn cargo_and_package_versions_equal_the_overlay_version() {
        let overlay = overlay_version();
        assert_eq!(env!("CARGO_PKG_VERSION"), overlay, "Cargo.toml version");
        assert_eq!(
            read_json("../package.json")["version"].as_str(),
            Some(overlay.as_str()),
            "package.json version"
        );
        assert!(
            overlay.starts_with("1.0.0-"),
            "internal builds are 1.0.0-alpha.N/-beta.N"
        );
    }

    #[test]
    fn app_menu_and_window_names_contain_no_handy() {
        // Tauri's default macOS app menu ("About …", "Hide …", "Quit …") is
        // built from the product name; Handy defines no menu of its own.
        let (_, config) = effective_config();
        assert!(!config.product_name.unwrap().contains("Handy"));
        let lib = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
            .unwrap();
        assert!(
            !lib.contains(".menu(") && !lib.contains("set_menu("),
            "Handy now builds its own app menu; F-01/S-26a must localize it"
        );
        for code in super::i18n::locales() {
            for key in ["rust.window.mainTitle", "rust.appName"] {
                let s = super::i18n::t(code, key, &[]);
                assert!(!s.contains("Handy") && !s.is_empty(), "{code}: {key} = {s}");
            }
        }
    }

    #[test]
    fn e94_both_env_vars_disable_the_updater() {
        use super::{updater_env_to_set, HANDY_DISABLE_UPDATER, WISPRBRO_DISABLE_UPDATER};
        use std::collections::HashMap;

        // Effective "forced off" as Handy computes it after early_init ran.
        let forced_off = |env: &[(&str, &str)]| {
            let mut env: HashMap<String, String> = env
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            if let Some((k, v)) = updater_env_to_set(|k| env.get(k).cloned()) {
                env.insert(k.to_string(), v.to_string());
            }
            super::flag_value_enabled(env.get(HANDY_DISABLE_UPDATER).map(String::as_str))
        };

        assert!(!forced_off(&[]));
        assert!(forced_off(&[(HANDY_DISABLE_UPDATER, "1")]));
        assert!(forced_off(&[(WISPRBRO_DISABLE_UPDATER, "1")]));
        assert!(forced_off(&[
            (WISPRBRO_DISABLE_UPDATER, "true"),
            (HANDY_DISABLE_UPDATER, "0")
        ]));
        assert!(!forced_off(&[(WISPRBRO_DISABLE_UPDATER, "0")]));
        assert!(!forced_off(&[(WISPRBRO_DISABLE_UPDATER, "off")]));
        // Handy's own setting is never turned *off* by ours.
        assert!(forced_off(&[
            (HANDY_DISABLE_UPDATER, "yes"),
            (WISPRBRO_DISABLE_UPDATER, "0")
        ]));
    }

    #[test]
    fn e94_truthiness_matches_handy_env_flag_enabled() {
        for value in [
            "1", "true", "TRUE", "yes", "on", " 1 ", "0", "false", "no", "off", "", "x",
        ] {
            let var = "WB_TEST_E94_TRUTHINESS";
            std::env::set_var(var, value);
            assert_eq!(
                super::flag_value_enabled(Some(value)),
                crate::utils::env_flag_enabled(var),
                "{value:?}"
            );
        }
        std::env::remove_var("WB_TEST_E94_TRUTHINESS");
        assert!(!super::flag_value_enabled(None));
    }

    #[test]
    fn file_prefix_hooks_are_in_place() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let read = |rel: &str| std::fs::read_to_string(root.join(rel)).unwrap();
        assert_eq!(super::FILE_PREFIX, "wispr-bro");
        assert_eq!(super::recording_file_name(17), "wispr-bro-17.wav");
        assert_eq!(super::file_job_name(18), "wispr-bro-file-18.wav");
        assert!(read("src/actions.rs").contains("crate::fork::recording_file_name("));
        assert!(read("src/managers/transcription_jobs.rs").contains("crate::fork::file_job_name("));
        let lib = read("src/lib.rs");
        assert_eq!(
            lib.matches("file_name: Some(fork::FILE_PREFIX.into())")
                .count(),
            2
        );
        assert!(!lib.contains("file_name: Some(\"handy\".into())"));
        let tray = read("src/tray.rs");
        assert!(
            tray.contains("crate::fork::tray::extend(app, &menu, &mut tooltip, &inputs.locale)?;")
        );
        assert!(tray.contains("crate::fork::tray::icon_path(&theme, state, warning)"));
    }
}
