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
//! - `actions.rs` paste: [`insert::deliver`] in place of `utils::paste` (S-04;
//!   S-11's post-LLM text stages run inside it).
//! - `managers/transcription.rs` `finish_text`: [`text::process`] wraps
//!   Handy's deterministic stages per line (S-11; S-09's verbatim text
//!   goes through it).
//! - `llm_client.rs`: [`llm::client_builder`] (F-08); S-10 routes `local`
//!   to the sidecar there, with no new upstream line.
//! - `overlay.rs`: the Flow Bar's hooks ([`pill`], S-03).
//!
//! S-18's notifications ([`notify`]) need no upstream hook: they listen to
//! Handy's `model-download-complete` event and install their macOS delegate
//! from [`on_ready`].
//!
//! Init order (tauri 2.11.5, see `docs/fork/ARCHITECTURE.md` for line refs):
//! plugins initialize inside `Builder::build()`, so the plugin `setup` below
//! runs *before* Handy's `.setup()` closure, which only runs on the first
//! `RunEvent::Ready`. The plugin's `on_event(RunEvent::Ready)` is dispatched
//! right after Handy's closure returns, so [`on_ready`] can rely on Handy's
//! managers being in state.

pub mod audio;
pub mod ax;
pub mod clipboard;
pub mod commands;
pub mod context;
pub mod db;
pub mod diagnostics;
pub mod dictionary;
pub mod events;
pub mod hub;
pub mod i18n;
pub mod insert;
pub mod llm;
pub mod models;
pub mod net;
pub mod notify;
pub mod pill;
pub mod recovery;
pub mod settings;
pub mod shortcut;
pub mod snippets;
pub mod stats;
#[cfg(test)]
mod testutil;
pub mod text;
pub mod tray;

pub use settings::PRODUCT_IDENTIFIER;

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

/// Name of a dictation recording at `timestamp_millis` (Handy:
/// `handy-<seconds>.wav`).
pub fn recording_file_name(timestamp_millis: i64) -> String {
    format!("{FILE_PREFIX}-{timestamp_millis}.wav")
}

/// The `actions.rs` hook that names a stopped take's WAV. Milliseconds, and
/// never the same value twice in a process, so two takes stopped in the same
/// second never share a file (RE-35). Also binds the take that just stopped
/// to this file name and its speech time (`sample_count` at 16 kHz), so the
/// history row knows its app and day (`fork::context::take`).
pub fn take_file_name(sample_count: usize) -> String {
    static LAST_MS: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
    let now = context::now_ms();
    let mut last = LAST_MS.load(Ordering::SeqCst);
    let ms = loop {
        let next = now.max(last + 1);
        match LAST_MS.compare_exchange(last, next, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => break next,
            Err(current) => last = current,
        }
    };
    let name = recording_file_name(ms);
    context::take::ledger().bind(
        &name,
        stats::wpm::samples_to_ms(sample_count),
        context::current_context,
    );
    // S-06: the take a cancel during transcription would bring back.
    recovery::note_take_file(&name);
    // S-20: the stopped take may now use its context hints.
    context::awareness::on_stopped();
    name
}

/// The `actions.rs` hook for a take whose WAV could not be saved (disk
/// full): keep its history row, text only (`fork::db::disk`).
pub fn keep_take(wav: &std::path::Path) -> bool {
    db::disk::keep_without_audio(wav)
}

/// Points the text pipeline at `fork.db` for its dictionary rules. A take
/// reads them once (a small indexed query), so S-16's edits apply to the
/// next take without a reload signal; a database error means no rules for
/// that take, never a failed take.
fn install_rule_source<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    text::install_rule_source(move || {
        let Some(fork_db) = app.try_state::<db::ForkDb>() else {
            return Vec::new();
        };
        fork_db
            .with_conn(|conn| db::rules::active_rules(conn))
            .unwrap_or_else(|e| {
                log::warn!("text: could not read the dictionary rules: {e}");
                Vec::new()
            })
    });
}

/// The `actions.rs` record-start hook (R5 §4.2): runs when a dictation's
/// recording starts. S-09: the frontmost app and the low-disk watch; S-03,
/// S-05 and S-10 add the pill state, the AX target and the LLM prewarm.
pub fn on_record_start<R: ForkRuntime>(app: &AppHandle<R>) {
    context::on_record_start();
    // S-20: the text near the caret, read once on the AX thread (never
    // waits; DEC-24: on unless the user switched it off).
    let enabled = app
        .try_state::<settings::ForkSettingsHandle>()
        .and_then(|h| h.get(app).ok())
        .is_none_or(|s| s.context_awareness);
    let languages = R::handy_settings(app)
        .map(|settings::HandySettings(s)| s.spoken_languages)
        .unwrap_or_default();
    context::awareness::on_record_start(enabled, languages);
    match crate::portable::data_dir()
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| app.path().app_data_dir())
    {
        Ok(dir) => db::disk::on_record_start(app, dir),
        Err(e) => log::debug!("disk: no app data dir to watch: {e}"),
    }
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
    // Plain Handy builds (`cargo build`, `bun run tauri dev`) keep Handy's
    // behaviour: no crash files, no HF_HOME change.
    if !is_product_build() {
        return;
    }
    // Idempotent (`OnceLock`); `run()` calls it again. Run first so portable
    // mode sets its own HF_HOME and records the user's previous one.
    crate::portable::init();
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    // F-08: crash files go to the log dir. Until Tauri resolves it (plugin
    // `setup` replaces this), use the macOS path for our identifier.
    if let Some(dir) = crate::portable::data_dir()
        .map(|d| d.join("logs"))
        .or_else(|| early_log_dir(home.as_deref()))
    {
        diagnostics::crash::set_crash_dir(dir);
    }
    diagnostics::crash::install();
    // DEC-39: models under our app data, before any manager starts.
    if let Some(hf_home) = models::hf_home_to_set(
        true,
        crate::portable::is_portable(),
        home.as_deref(),
        &compiled_identifier(),
    ) {
        std::env::set_var("HF_HOME", hf_home);
    }
}

/// Identifier the binary was built with, from the env vars tauri-build emits
/// for every target (`TAURI_ANDROID_PACKAGE_NAME_PREFIX` / `_APP_NAME`, the
/// identifier split at its last dot with `_` for `.`; `tauri-build`
/// `lib.rs:493-508`). Available before any Tauri object exists, which is what
/// `early_init` needs.
pub fn compiled_identifier() -> String {
    let prefix = option_env!("TAURI_ANDROID_PACKAGE_NAME_PREFIX").unwrap_or_default();
    let app = option_env!("TAURI_ANDROID_PACKAGE_NAME_APP_NAME").unwrap_or_default();
    format!("{}.{app}", prefix.replace('_', "."))
}

/// `true` when built with the identity overlay (`com.wisprbro.desktop`), or
/// with a test identity of ours ([`is_test_identity`]).
pub fn is_product_build() -> bool {
    is_product_identity(&compiled_identifier())
}

/// The product identifier or a test identity.
pub fn is_product_identity(id: &str) -> bool {
    id == PRODUCT_IDENTIFIER || is_test_identity(id)
}

/// A test identity for real-Mac checks (`com.wisprbro.<slice>check`, e.g.
/// `com.wisprbro.s07acheck`, S-07a): the product build with its own app
/// data, WebKit storage and TCC rows, so a run never touches the owner's
/// app. It behaves exactly like the product (hotkeys, the bar, product
/// settings); its data lives under its own identifier.
pub fn is_test_identity(id: &str) -> bool {
    id.strip_prefix("com.wisprbro.")
        .and_then(|rest| rest.strip_suffix("check"))
        .is_some_and(|slice| !slice.is_empty() && slice.chars().all(|c| c.is_ascii_alphanumeric()))
}

/// Portable-aware log dir for any runtime (Handy's `portable::app_log_dir`
/// is typed to `Wry`).
pub fn app_log_dir<R: Runtime>(app: &AppHandle<R>) -> Option<std::path::PathBuf> {
    match crate::portable::data_dir() {
        Some(dir) => Some(dir.join("logs")),
        None => app.path().app_log_dir().ok(),
    }
}

/// Tauri's macOS `app_log_dir()` for our identifier: `~/Library/Logs/<id>`.
fn early_log_dir(home: Option<&std::path::Path>) -> Option<std::path::PathBuf> {
    cfg!(target_os = "macos")
        .then(|| home.map(|h| h.join("Library/Logs").join(compiled_identifier())))?
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
            // F-08: crash files follow Tauri's (portable-aware) log dir.
            if is_product_identity(&app.config().identifier) {
                if let Some(dir) = app_log_dir(app) {
                    diagnostics::crash::set_crash_dir(dir);
                }
            }
            db::register(app);
            settings::register(app);
            // S-10: LLM runtime state (nothing starts before on_ready).
            llm::runtime::register(app);
            // Merges into the same registry Handy's builder mounts into later
            // (`EventRegistry::get_or_manage`), so order does not matter.
            specta_builder::<R>().mount_events(app);
            Ok(())
        })
        .on_event(|app, event| match event {
            RunEvent::Ready => on_ready(app),
            // S-08 (DEC-55): a take still recording when the app is asked
            // to quit (Quit & Reopen after a Microphone revoke) is saved.
            RunEvent::ExitRequested { .. } => audio::permission::on_exit(app),
            // S-10: the sidecar never outlives the app (the watchdog covers
            // a crash or SIGKILL). S-08: the take is saved if the quit
            // skipped `ExitRequested`, and the audio is never left muted.
            RunEvent::Exit => {
                audio::permission::on_exit(app);
                audio::mute::restore_on_exit();
                llm::runtime::on_exit(app);
                // S-05: apps whose accessibility tree we switched on get it
                // switched back.
                ax::on_exit();
            }
            _ => {}
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
    // S-04: session observers and recording phases for delivery.
    insert::on_ready(app);
    db::jobs::start(app);
    // S-11: the dictionary rules the text pipeline applies, read per take.
    install_rule_source(app);
    // S-17: the snippets stage 11 expands, read per delivery.
    snippets::install(app);
    // S-19a: the Style page's app rules, cached for the take path.
    commands::style::on_ready(app);
    // S-16: Handy's custom words and the dictionary's words (one-time
    // migration, then the mirror), off the main thread.
    {
        let app = app.clone();
        std::thread::spawn(move || {
            dictionary::migrate::sync(&app);
        });
    }

    // S-13: Hub window size and title bar.
    hub::on_ready(app);
    // S-15b: History plays recordings through the asset protocol; the
    // static scope covers `$APPDATA/**`, portable mode's `Data/` is added
    // here (only Handy's recordings dir).
    commands::history::allow_recordings_in_asset_scope(app);

    // S-18: notifications inbox, macOS delegate (no prompt), producers.
    notify::on_ready(app);

    // S-10: stale sidecar kill, prewarm wiring, silent model download.
    llm::runtime::on_ready(app);
    // S-12b: WISPRBRO_SELFCHECK_APPLE (Mac test).
    llm::apple::selfcheck(app);

    // F-08 / DEC-39: report (without the path) whether `early_init` put the
    // Hugging Face cache under the app data dir.
    if is_product_build() {
        let data_dir = crate::portable::data_dir()
            .cloned()
            .or_else(|| app.path().app_data_dir().ok());
        let under = std::env::var_os("HF_HOME")
            .map(std::path::PathBuf::from)
            .zip(data_dir)
            .is_some_and(|(hf, data)| hf.starts_with(data));
        log::info!("fork: HF_HOME under the app data dir: {under}");
    }

    // S-01: the fork's hotkey engine and the prepared microphone (product
    // builds only; plain Handy builds keep Handy's hotkeys, see
    // `shortcut::owns`).
    if is_product_build() {
        // S-07a: the speech model check and the first-run download, before
        // the hotkeys, so the first press already knows.
        R::models_start(app);
        // S-08: the wireless-default rule, the device watch, saved takes.
        audio::on_ready(app);
        audio::enable_prepared(app);
        // S-06: Undo and the recovery actions, registered before the
        // engine compiles the bindings.
        R::recovery_start(app);
        shortcut::start(app);
        // S-03: the Flow Bar (macOS; replaces Handy's overlay).
        R::pill_start(app);
    }

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
                commands::csp::fork_csp_smoke_media::<$runtime>,
                commands::hotkeys::fork_hotkey_status::<$runtime>,
                commands::hotkeys::fork_use_hotkey_profile::<$runtime>,
                commands::history::fork_stats_summary::<$runtime>,
                commands::history::fork_history_search::<$runtime>,
                commands::history::fork_history_meta::<$runtime>,
                commands::history::fork_history_delete::<$runtime>,
                commands::history::fork_history_undo_delete::<$runtime>,
                commands::history::fork_history_page::<$runtime>,
                commands::history::fork_history_find::<$runtime>,
                commands::history::fork_history_rows::<$runtime>,
                commands::history::fork_history_set_variant::<$runtime>,
                commands::history::fork_history_retry::<$runtime>,
                commands::history::fork_history_flag::<$runtime>,
                commands::history::fork_history_save_audio::<$runtime>,
                commands::stats::fork_account_name,
                commands::llm::fork_llm_status::<$runtime>,
                commands::llm::fork_llm_try::<$runtime>,
                commands::llm::fork_llm_retry::<$runtime>,
                commands::dictionary::fork_dictionary_list::<$runtime>,
                commands::dictionary::fork_dictionary_add::<$runtime>,
                commands::dictionary::fork_dictionary_update::<$runtime>,
                commands::dictionary::fork_dictionary_delete::<$runtime>,
                commands::dictionary::fork_dictionary_restore::<$runtime>,
                commands::dictionary::fork_dictionary_read_file,
                commands::dictionary::fork_dictionary_import_review::<$runtime>,
                commands::dictionary::fork_dictionary_import::<$runtime>,
                commands::dictionary::fork_dictionary_export::<$runtime>,
                commands::notifications::fork_notifications_list::<$runtime>,
                commands::notifications::fork_notifications_mark_read::<$runtime>,
                commands::notifications::fork_notifications_mark_all_read::<$runtime>,
                commands::notifications::fork_notifications_activate::<$runtime>,
                commands::notifications::fork_notifications_feature::<$runtime>,
                commands::notifications::fork_notifications_permission::<$runtime>,
                commands::notifications::fork_notifications_debug_post::<$runtime>,
                commands::pill::fork_pill_snapshot::<$runtime>,
                commands::pill::fork_pill_ready::<$runtime>,
                commands::pill::fork_pill_action::<$runtime>,
                commands::llm::fork_llm_last_cleanup,
                commands::snippets::fork_snippets_list::<$runtime>,
                commands::snippets::fork_snippets_add::<$runtime>,
                commands::snippets::fork_snippets_update::<$runtime>,
                commands::snippets::fork_snippets_delete::<$runtime>,
                commands::snippets::fork_snippets_restore::<$runtime>,
                commands::snippets::fork_snippets_import_review::<$runtime>,
                commands::snippets::fork_snippets_import::<$runtime>,
                commands::snippets::fork_snippets_export::<$runtime>,
                commands::models::fork_speech_setup_status,
                commands::models::fork_speech_setup_retry,
                commands::models::fork_speech_setup_refresh,
                commands::audio::fork_mic_list::<$runtime>,
                commands::audio::fork_mic_choose::<$runtime>,
                commands::audio::fork_mic_monitor_start::<$runtime>,
                commands::audio::fork_mic_monitor_stop,
                commands::insights::fork_insights_usage::<$runtime>,
                commands::insights::fork_insights_voice::<$runtime>,
                commands::insights::fork_insights_save_image,
                commands::llm::fork_llm_apple_status,
                commands::models::fork_model_policy_status,
                commands::models::fork_model_policy_set_automatic,
                commands::models::fork_models_hidden::<$runtime>,
                commands::style::fork_style_overview::<$runtime>,
                commands::style::fork_style_installed_apps::<$runtime>,
                commands::style::fork_style_app_icon,
                commands::style::fork_style_set_app_group::<$runtime>,
                commands::style::fork_style_remove_app::<$runtime>,
                commands::style::fork_style_set_auto_send::<$runtime>,
                commands::style::fork_style_set_site_group::<$runtime>,
                commands::style::fork_style_remove_site::<$runtime>,
                commands::style::fork_style_set_site_auto_send::<$runtime>,
            ])
            .events(tauri_specta::collect_events![
                events::ForkPingEvent,
                events::ForkSettingsChanged,
                events::ForkInsertOutcome,
                shortcut::ForkHotkeyEvent,
                events::ForkStatsUpdated,
                llm::runtime::ForkLlmStatus,
                dictionary::ForkDictionaryChanged,
                notify::ForkNotificationsChanged,
                notify::ForkNotificationOpened,
                notify::ForkToastsDismiss,
                pill::ForkPillState,
                pill::ForkPillLevels,
                llm::post_process::ForkCleanupResult,
                snippets::ForkSnippetsChanged,
                models::setup::ForkSpeechSetup,
                audio::watch::ForkMicDevicesChanged,
                audio::monitor::ForkMicLevel,
                models::upgrade::ForkModelPolicy,
                commands::style::ForkAppRulesChanged
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

    /// Title bar height of a window in logical pixels (S-13, `fork::hub`).
    /// Needs the real NSWindow; other runtimes return 0.
    fn hub_title_bar_height(window: &tauri::WebviewWindow<Self>) -> f64;

    /// Hides a window's title text (macOS, S-13). No-op elsewhere.
    fn hub_hide_title_text(window: &tauri::WebviewWindow<Self>);

    /// Handy's history manager (S-09 jobs), `None` before it exists.
    fn handy_history(app: &AppHandle<Self>) -> Option<std::sync::Arc<dyn db::handy::HandyHistory>>;
    /// Starts the Flow Bar (S-03; macOS product builds). No-op elsewhere.
    fn pill_start(app: &AppHandle<Self>);
    /// Starts the speech model setup (S-07a; product builds). No-op on
    /// runtimes without Handy's managers.
    fn models_start(app: &AppHandle<Self>);
    /// Starts Undo and registers the recovery actions (S-06; product
    /// builds). No-op on runtimes without Handy's managers.
    fn recovery_start(app: &AppHandle<Self>);
}

impl ForkRuntime for tauri::Wry {
    fn models_start(app: &AppHandle<Self>) {
        models::setup::start(app);
    }

    fn recovery_start(app: &AppHandle<Self>) {
        recovery::on_ready(app);
    }

    fn pill_start(app: &AppHandle<Self>) {
        #[cfg(target_os = "macos")]
        pill::start(app);
        #[cfg(not(target_os = "macos"))]
        let _ = app;
    }

    fn hub_title_bar_height(window: &tauri::WebviewWindow<Self>) -> f64 {
        hub::title_bar_height(window)
    }

    fn hub_hide_title_text(window: &tauri::WebviewWindow<Self>) {
        #[cfg(target_os = "macos")]
        hub::hide_title_text(window);
        #[cfg(not(target_os = "macos"))]
        let _ = window;
    }

    fn fork_specta_builder() -> Builder<Self> {
        fork_specta_builder!(tauri::Wry)
    }

    fn handy_app_language(app: &AppHandle<Self>) -> Option<String> {
        Some(crate::settings::get_settings(app).app_language)
    }

    fn handy_history(app: &AppHandle<Self>) -> Option<std::sync::Arc<dyn db::handy::HandyHistory>> {
        let hm = app.try_state::<std::sync::Arc<crate::managers::history::HistoryManager>>()?;
        Some(std::sync::Arc::new(db::handy::HandyHistoryManager(
            hm.inner().clone(),
        )))
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
    fn pill_start(_app: &AppHandle<Self>) {}

    fn models_start(_app: &AppHandle<Self>) {}

    fn recovery_start(_app: &AppHandle<Self>) {}

    fn hub_title_bar_height(_window: &tauri::WebviewWindow<Self>) -> f64 {
        0.0
    }

    fn hub_hide_title_text(_window: &tauri::WebviewWindow<Self>) {}

    fn fork_specta_builder() -> Builder<Self> {
        fork_specta_builder!(tauri::test::MockRuntime)
    }

    fn handy_app_language(_app: &AppHandle<Self>) -> Option<String> {
        None
    }

    /// Tests stand in for Handy's store with `settings::FakeHandyStore`
    /// (managed state); without it the product step is skipped.
    fn handy_history(app: &AppHandle<Self>) -> Option<std::sync::Arc<dyn db::handy::HandyHistory>> {
        app.try_state::<std::sync::Arc<db::handy::fake::FakeHistory>>()
            .map(|h| h.inner().clone() as std::sync::Arc<dyn db::handy::HandyHistory>)
    }

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
            let observed = observe_handy_command(invoke.message.command());
            let handled = handy(invoke);
            if let Some(after) = observed {
                after();
            }
            handled
        }
    }
}

/// Handy commands the fork reacts to, without a hook in Handy's file: what
/// to do once Handy has taken the call. S-07b: a change of the spoken
/// languages (Settings › General, onboarding) re-picks the speech model.
/// Handy's command may still be running (async commands), so the reaction
/// only wakes the fork, which reads the settings when it gets to it.
pub fn observe_handy_command(command: &str) -> Option<fn()> {
    match command {
        "change_spoken_languages_setting" => Some(|| {
            std::thread::spawn(|| {
                // Handy writes the setting synchronously in its command;
                // give the store a moment anyway.
                std::thread::sleep(std::time::Duration::from_millis(300));
                models::setup::refresh();
            });
        }),
        _ => None,
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

    #[test]
    fn test_identities_run_the_product_runtime_but_nothing_else_does() {
        use super::is_product_identity;
        assert!(is_product_identity("com.wisprbro.desktop"));
        assert!(is_product_identity("com.wisprbro.s08check"));
        assert!(is_product_identity("com.wisprbro.s15acheck"));
        for other in [
            "com.pais.handy",
            "com.wisprbro.check",
            "com.wisprbro.s08",
            "com.wisprbro.s08.check",
            "com.wisprbro.desktop2",
            "org.wisprbro.s08check",
        ] {
            assert!(!is_product_identity(other), "{other}");
        }
    }

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

    /// DEC-43 / R5 §4.11: the CSP exactly as specified, merged over Handy's
    /// `csp: null` and asset scope `**`.
    #[test]
    fn effective_config_has_the_r5_csp() {
        let (raw, _) = effective_config();
        let security = &raw["app"]["security"];
        let csp = security["csp"].as_str().expect("csp set");
        assert_eq!(
            csp,
            "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: asset: http://asset.localhost; media-src 'self' blob: asset: http://asset.localhost; font-src 'self' data:; connect-src 'self' ipc: http://ipc.localhost; object-src 'none'; base-uri 'self'; frame-ancestors 'none'"
        );
        let dev = security["devCsp"].as_str().expect("devCsp set");
        for (directive, extra) in [
            ("script-src", "http://localhost:1420"),
            ("style-src", "http://localhost:1420"),
            ("img-src", "http://localhost:1420"),
            ("connect-src", "http://localhost:1420"),
            ("connect-src", "ws://localhost:1420"),
        ] {
            let d = dev
                .split(';')
                .map(str::trim)
                .find(|d| d.starts_with(directive))
                .unwrap();
            assert!(d.contains(extra), "{directive}: {d}");
        }
        // devCsp = csp + the dev-server sources, nothing else.
        let strip = |s: &str| {
            s.replace(" http://localhost:1420", "")
                .replace(" ws://localhost:1420", "")
        };
        assert_eq!(strip(dev), csp);
        assert_eq!(security["assetProtocol"]["enable"], true);
        assert_eq!(
            security["assetProtocol"]["scope"]["allow"],
            serde_json::json!(["$APPDATA/**"])
        );
        // No remote source anywhere.
        for token in csp.split_whitespace().map(|t| t.trim_end_matches(';')) {
            assert!(
                !token.starts_with("https:") && token != "*" && !token.contains("://")
                    || token.ends_with(".localhost"),
                "remote source {token}"
            );
        }
    }

    #[test]
    fn compiled_identifier_reads_the_build_identity() {
        // `cargo test` builds without the overlay, so this is Handy's id and
        // `early_init` leaves HF_HOME and the panic hook alone.
        let (_, config) = effective_config();
        let base = read_json("tauri.conf.json");
        let id = super::compiled_identifier();
        assert!(
            id == base["identifier"] || id == config.identifier,
            "unexpected compiled identifier {id}"
        );
        assert_eq!(super::is_product_build(), id == "com.wisprbro.desktop");
        // Test identities (S-07a) are product builds with their own data.
        assert!(super::is_test_identity("com.wisprbro.s07acheck"));
        assert!(super::is_test_identity("com.wisprbro.s18check"));
        assert!(super::is_product_identity("com.wisprbro.desktop"));
        assert!(!super::is_test_identity("com.wisprbro.desktop"));
        assert!(!super::is_test_identity("com.wisprbro.check"));
        assert!(!super::is_test_identity("com.wisprbro.a.bcheck"));
        assert!(!super::is_test_identity("com.pais.handy"));
        assert!(!super::is_product_identity("com.wisprbro.desktopx"));
        let main =
            std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main.rs"))
                .unwrap();
        assert!(main.contains("handy_app_lib::fork::early_init();"));
    }

    #[test]
    fn file_prefix_hooks_are_in_place() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let read = |rel: &str| std::fs::read_to_string(root.join(rel)).unwrap();
        assert_eq!(super::FILE_PREFIX, "wispr-bro");
        assert_eq!(super::recording_file_name(17), "wispr-bro-17.wav");
        let a = super::take_file_name(16_000);
        let b = super::take_file_name(16_000);
        assert_ne!(a, b, "two takes in the same millisecond get two files");
        assert!(a.starts_with("wispr-bro-") && a.ends_with(".wav"));
        let ms: i64 = a["wispr-bro-".len()..a.len() - 4].parse().unwrap();
        assert!(ms > 1_700_000_000_000, "milliseconds, not seconds: {ms}");
        assert_eq!(
            crate::fork::context::take::ledger()
                .take_bound(&b)
                .map(|t| t.audio_ms),
            Some(1_000)
        );
        assert_eq!(super::file_job_name(18), "wispr-bro-file-18.wav");
        assert!(read("src/actions.rs").contains("crate::fork::take_file_name(sample_count)"));
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
