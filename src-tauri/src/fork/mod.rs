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

pub mod app_menu;
pub mod audio;
pub mod ax;
pub mod clipboard;
pub mod commands;
pub mod context;
pub mod db;
pub mod diagnostics;
pub mod dictionary;
pub mod erase;
pub mod events;
pub mod export;
pub mod hub;
pub mod i18n;
pub mod insert;
pub mod llm;
pub mod models;
pub mod net;
pub mod notify;
pub mod onboarding;
#[cfg(test)]
mod perf_bench;
pub mod pill;
/// The publik edition only (docs/fork/EDITIONS.md); never in the normal build.
#[cfg(feature = "publik")]
pub mod publik;
pub mod recovery;
pub mod restore;
pub mod scratchpad;
pub mod settings;
pub mod share;
pub mod shortcut;
pub mod snippets;
pub mod startup;
pub mod stats;
#[cfg(test)]
mod testutil;
pub mod text;
pub mod transforms;
pub mod tray;
pub mod tutorials;
pub mod update;
/// Third-party code kept as received (see each file's header).
pub mod vendor;

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

/// Points the text pipeline's accent stage (S-38) at the dictionary's
/// active words in `fork.db`, read once per take like the rules.
fn install_word_source<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    text::install_word_source(move || {
        let Some(fork_db) = app.try_state::<db::ForkDb>() else {
            return Vec::new();
        };
        fork_db
            .with_conn(|conn| dictionary::store::active_words(conn))
            .unwrap_or_else(|e| {
                log::warn!("text: could not read the dictionary words: {e}");
                Vec::new()
            })
    });
}

/// The `actions.rs` record-start hook (R5 §4.2): runs when a dictation's
/// recording starts. S-09: the frontmost app and the low-disk watch; S-03,
/// S-05 and S-10 add the pill state, the AX target and the LLM prewarm.
pub fn on_record_start<R: ForkRuntime>(app: &AppHandle<R>) {
    context::on_record_start();
    // S-28: a take is activity; updates wait 20 minutes after it.
    update::idle_gate::note_activity();
    // S-20: the text near the caret, read once on the AX thread (never
    // waits; DEC-24: on unless the user switched it off). S-36: in a listed
    // editor the same read also collects code and file names.
    let settings = app
        .try_state::<settings::ForkSettingsHandle>()
        .and_then(|h| h.get(app).ok());
    let enabled = settings.as_ref().is_none_or(|s| s.context_awareness);
    let vibe = settings.map(|s| s.vibe_coding).unwrap_or_default();
    let languages = R::handy_settings(app)
        .map(|settings::HandySettings(s)| s.spoken_languages)
        .unwrap_or_default();
    context::awareness::on_record_start(enabled, vibe, languages);
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
    // The publik edition's release self test answers and exits before
    // anything else starts (docs/fork/EDITIONS.md).
    #[cfg(feature = "publik")]
    publik::client::self_test_if_requested();
    if let Some((key, value)) = updater_env_to_set(|k| std::env::var(k).ok()) {
        std::env::set_var(key, value);
    }
    // Test builds only: a WAV file as the microphone (Mac waves never use
    // the speakers). Before any recorder opens.
    audio::test_mic::init(&compiled_identifier());
    // Plain Handy builds (`cargo build`, `bun run tauri dev`) keep Handy's
    // behaviour: no crash files, no HF_HOME change.
    if !is_product_build() {
        return;
    }
    // Mac wave 2 finding 1: no "reopen windows?" alert after a crash
    // (before Tauri creates NSApplication).
    restore::opt_out();
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

/// The `main.rs` CLI hook (S-28): the relaunch after an update starts
/// hidden when the Hub was hidden ([`update::relaunch`]). Runs after
/// [`early_init`], before Tauri; plain Handy builds are untouched.
pub fn cli_args(mut args: CliArgs) -> CliArgs {
    if is_product_build() {
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        let dir = crate::portable::data_dir()
            .map(|d| d.join("logs"))
            .or_else(|| early_log_dir(home.as_deref()));
        update::relaunch::apply_at_start(dir, &mut args.start_hidden);
    }
    args
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
            // Audit OT2: no app activation until `on_ready` (a hidden
            // launch must not take focus; see `end_launch_activation_guard`).
            #[cfg(target_os = "macos")]
            if let Some(mtm) = objc2::MainThreadMarker::new() {
                pill::activation::suppress_at_launch(mtm);
            }
            app.manage(ForkState::default());
            // F-08: crash files follow Tauri's (portable-aware) log dir.
            if is_product_identity(&app.config().identifier) {
                if let Some(dir) = app_log_dir(app) {
                    diagnostics::crash::set_crash_dir(dir);
                }
            }
            db::register(app);
            settings::register(app);
            // S-38: hidden Hub / Scratchpad windows are parked.
            hub::park::register(app);
            // S-10: LLM runtime state (nothing starts before on_ready).
            llm::runtime::register(app);
            // OT2-H: the instruction model for transforms and Command Mode.
            llm::instruct::register(app);
            // Merges into the same registry Handy's builder mounts into later
            // (`EventRegistry::get_or_manage`), so order does not matter.
            specta_builder::<R>().mount_events(app);
            Ok(())
        })
        .on_event(|app, event| match event {
            RunEvent::Ready => on_ready(app),
            // S-07c: commit the model download's resume offset. S-08 (DEC-55):
            // a take still recording when quit is requested is saved.
            // S-38: not when the only windows left were parked (no code:
            // every Tauri window is gone, but nobody asked to quit).
            RunEvent::ExitRequested { code, api, .. } => {
                if hub::park::keeps_running(app, *code) {
                    api.prevent_exit();
                    log::debug!("fork: exit request without windows, Hub parked: kept running");
                    return;
                }
                models::setup::on_exit(app);
                audio::permission::on_exit(app);
                // S-28: a downloaded update installs at quit (not when the
                // exit is our own restart).
                if *code != Some(tauri::RESTART_EXIT_CODE) {
                    update::scheduler::on_exit(app);
                }
            }
            // S-10: the sidecar never outlives the app (the watchdog covers
            // a crash or SIGKILL). S-08: the take is saved if the quit
            // skipped `ExitRequested`, and the audio is never left muted.
            RunEvent::Exit => {
                models::setup::on_exit(app);
                audio::permission::on_exit(app);
                audio::mute::restore_on_exit();
                llm::runtime::on_exit(app);
                llm::instruct::on_exit(app);
                // S-28: a quit that skipped `ExitRequested` (installs once).
                update::scheduler::on_exit(app);
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

/// Audit OT2: app activation was off since the plugin's `setup` (inside
/// `Builder::build`, before tao's `applicationDidFinishLaunching:` calls
/// `activateIgnoringOtherApps:` and before Handy's `.setup()` builds the
/// hidden Hub, whose wry build calls `activate`), so a `--start-hidden`
/// launch (login item, the quiet relaunch after an update) never takes
/// focus from the app in front. Turns it back on; a launch that showed the
/// Hub (Handy's `show_main_window` ran inside the guard) focuses it now.
fn end_launch_activation_guard<R: Runtime>(app: &AppHandle<R>) {
    #[cfg(target_os = "macos")]
    {
        let Some(mtm) = objc2::MainThreadMarker::new() else {
            return;
        };
        if !pill::activation::end_launch(mtm) {
            return;
        }
        if let Some(main) = app.get_webview_window("main") {
            if main.is_visible().unwrap_or(false) {
                if let Err(e) = main.set_focus() {
                    log::warn!("fork: could not focus the Hub after launch: {e}");
                }
            } else {
                log::info!("fork: hidden launch, app not activated");
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

/// Audit OT2: Handy launches a `--start-hidden` app as Accessory (no Dock
/// icon) whenever a tray icon exists (`lib.rs`
/// `apply_startup_activation_policy`), so "Show app in Dock" (on by
/// default, S-14) lost its icon after every login launch and quiet relaunch
/// after an update (S-28). Setting it back to Regular does not activate the
/// app.
fn keep_dock_icon_at_hidden_launch<R: Runtime>(app: &AppHandle<R>) {
    #[cfg(target_os = "macos")]
    if is_product_build() && settings::show_in_dock() {
        if let Err(e) = app.set_activation_policy(tauri::ActivationPolicy::Regular) {
            log::warn!("fork: could not keep the Dock icon: {e}");
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

/// Runs once, on `RunEvent::Ready`, after Handy's `.setup()` closure (and so
/// after `initialize_core_logic`) has returned.
pub fn on_ready<R: ForkRuntime>(app: &AppHandle<R>) {
    end_launch_activation_guard(app);
    let headless = app
        .try_state::<CliArgs>()
        .map(|args| is_headless(&args))
        .unwrap_or(false);
    if headless {
        log::debug!("fork: headless CLI mode, skipping on_ready");
        return;
    }
    // The publik edition: nothing below runs until the person has signed in;
    // the gate calls `on_ready` again then (docs/fork/EDITIONS.md).
    #[cfg(feature = "publik")]
    if publik::gate::hold(app) {
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
    // alpha.3: settle the install location before onboarding gets far. A
    // launch from Downloads or a mounted disk image moves/copies itself and
    // relaunches; an installed copy already there is opened instead. Only a
    // failed move shows one small native prompt.
    #[cfg(target_os = "macos")]
    onboarding::ensure_location_at_startup(app);
    keep_dock_icon_at_hidden_launch(app);
    // S-04: session observers and recording phases for delivery.
    insert::on_ready(app);
    db::jobs::start(app);
    // S-11: the dictionary rules the text pipeline applies, read per take.
    startup::init();
    install_rule_source(app);
    install_word_source(app);
    // S-17: the snippets stage 11 expands, read per delivery.
    snippets::install(app);
    // S-19a: the Style page's app rules, cached for the take path.
    commands::style::on_ready(app);
    // S-36: set-up editors keep their AX tree built while in front.
    if let Some(s) = app
        .try_state::<settings::ForkSettingsHandle>()
        .and_then(|h| h.get(app).ok())
    {
        context::ide::sync_warm(&s);
    }
    // S-34: auto-learn hears Return, keypad Enter and Tab from the fork tap.
    dictionary::learn::on_ready();
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
    // S-26a: localized app menus, tray contents and live menu counts.
    app_menu::on_ready(app);
    // S-15b: History plays recordings through the asset protocol; the
    // static scope covers `$APPDATA/**`, portable mode's `Data/` is added
    // here (only Handy's recordings dir).
    commands::history::allow_recordings_in_asset_scope(app);

    // S-18: notifications inbox, macOS delegate (no prompt), producers.
    notify::on_ready(app);

    // S-10: stale sidecar kill, prewarm wiring, silent model download.
    llm::runtime::on_ready(app);
    // OT2-H: the instruction model (on disk: verified; else on demand).
    llm::instruct::on_ready(app);
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
        // engine compiles the bindings. S-21's handlers follow the same rule.
        R::recovery_start(app);
        transforms::on_ready(app);
        scratchpad::on_ready(app);
        shortcut::start(app);
        // S-29a (DEC-44): a Globe change of ours is undone when fn is no
        // longer the trigger (bindings edited while the app was closed).
        shortcut::reconcile_globe(app);
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
    audio::test_mic::log_status();
    if is_product_build() && !restore::opted_out() {
        log::warn!("startup: window restoration is still on (crash prompt possible)");
    }
    // S-28: background update checks, idle installs, quiet relaunch.
    if let Some(hidden) = update::relaunch::relaunched() {
        log::info!(
            "update: relaunched after an update ({})",
            if hidden { "hidden" } else { "shown" }
        );
    }
    update::scheduler::start(app);
    update::start_selftest(app);
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
                commands::onboarding::fork_onboarding_go_to::<$runtime>,
                commands::onboarding::fork_onboarding_complete::<$runtime>,
                commands::onboarding::fork_onboarding_app_location,
                commands::onboarding::fork_onboarding_move_to_applications::<$runtime>,
                commands::onboarding::fork_onboarding_key_test::<$runtime>,
                commands::onboarding::fork_onboarding_practice,
                commands::tutorials::fork_lesson_practice,
                commands::onboarding::fork_onboarding_globe_do_nothing::<$runtime>,
                commands::onboarding::fork_onboarding_open_pane::<$runtime>,
                commands::onboarding::fork_onboarding_reset_permission::<$runtime>,
                commands::hotkeys::fork_shortcut_health,
                commands::hotkeys::fork_shortcut_recheck,
                commands::hotkeys::fork_shortcut_open_accessibility,
                commands::hotkeys::fork_shortcut_reset_permission::<$runtime>,
                commands::hotkeys::fork_shortcut_relaunch::<$runtime>,
                commands::onboarding::fork_checklist_mark::<$runtime>,
                commands::onboarding::fork_spotlight_start,
                commands::onboarding::fork_spotlight_stop,
                commands::onboarding::fork_spotlight_click,
                commands::onboarding::fork_spotlight_state,
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
                commands::llm::fork_instruct_status::<$runtime>,
                commands::llm::fork_instruct_retry::<$runtime>,
                commands::dictionary::fork_dictionary_list::<$runtime>,
                commands::dictionary::fork_dictionary_add::<$runtime>,
                commands::dictionary::fork_dictionary_update::<$runtime>,
                commands::dictionary::fork_dictionary_delete::<$runtime>,
                commands::dictionary::fork_dictionary_restore::<$runtime>,
                commands::dictionary::fork_dictionary_read_file,
                commands::dictionary::fork_dictionary_import_review::<$runtime>,
                commands::dictionary::fork_dictionary_import::<$runtime>,
                commands::dictionary::fork_dictionary_export::<$runtime>,
                commands::dictionary::fork_dictionary_accept::<$runtime>,
                commands::dictionary::fork_dictionary_reject::<$runtime>,
                commands::notifications::fork_notifications_list::<$runtime>,
                commands::notifications::fork_notifications_mark_read::<$runtime>,
                commands::notifications::fork_notifications_mark_all_read::<$runtime>,
                commands::notifications::fork_notifications_activate::<$runtime>,
                commands::notifications::fork_notifications_feature::<$runtime>,
                commands::notifications::fork_notifications_permission::<$runtime>,
                commands::notifications::fork_notifications_debug_post::<$runtime>,
                commands::pill::fork_pill_snapshot::<$runtime>,
                commands::pill::fork_pill_ready::<$runtime>,
                commands::hub::fork_hub_ready::<$runtime>,
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
                commands::transforms::fork_transforms_list::<$runtime>,
                commands::transforms::fork_transform_create::<$runtime>,
                commands::transforms::fork_transform_update::<$runtime>,
                commands::transforms::fork_transform_delete::<$runtime>,
                commands::transforms::fork_transforms_reset::<$runtime>,
                commands::transforms::fork_transform_preview::<$runtime>,
                commands::transforms::fork_transform_plan,
                commands::transforms::fork_transform_latest::<$runtime>,
                commands::transforms::fork_transform_diff,
                commands::transforms::fork_transform_rate::<$runtime>,
                commands::transforms::fork_transform_retry::<$runtime>,
                commands::transforms::fork_transform_read_example,
                commands::scratchpad::fork_scratchpad_list::<$runtime>,
                commands::scratchpad::fork_scratchpad_get::<$runtime>,
                commands::scratchpad::fork_scratchpad_save::<$runtime>,
                commands::scratchpad::fork_scratchpad_set_pinned::<$runtime>,
                commands::scratchpad::fork_scratchpad_delete::<$runtime>,
                commands::scratchpad::fork_scratchpad_open::<$runtime>,
                commands::scratchpad::fork_scratchpad_new::<$runtime>,
                commands::scratchpad::fork_scratchpad_show_note::<$runtime>,
                commands::scratchpad::fork_scratchpad_close::<$runtime>,
                commands::scratchpad::fork_scratchpad_expand::<$runtime>,
                commands::scratchpad::fork_scratchpad_copy::<$runtime>,
                commands::scratchpad::fork_scratchpad_polish::<$runtime>,
                commands::scratchpad::fork_scratchpad_record::<$runtime>,
                commands::update::fork_update_status::<$runtime>,
                commands::update::fork_update_check::<$runtime>,
                commands::update::fork_update_install::<$runtime>,
                commands::update::fork_update_state::<$runtime>,
                commands::update::fork_update_check_now::<$runtime>,
                commands::update::fork_update_restart_now::<$runtime>,
                commands::privacy::fork_data_locations::<$runtime>,
                commands::privacy::fork_show_data_location::<$runtime>,
                commands::privacy::fork_export_data::<$runtime>,
                commands::privacy::fork_copy_diagnostics::<$runtime>,
                commands::privacy::fork_save_diagnostics::<$runtime>,
                commands::privacy::fork_erase_all_data::<$runtime>,
                commands::help::fork_open_issue::<$runtime>,
                commands::help::fork_share_sheet::<$runtime>,
            ])
            .events(tauri_specta::collect_events![
                events::ForkPingEvent,
                events::ForkSettingsChanged,
                events::ForkInsertOutcome,
                shortcut::ForkHotkeyEvent,
                events::ForkStatsUpdated,
                llm::runtime::ForkLlmStatus,
                llm::instruct::ForkInstructStatus,
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
                commands::style::ForkAppRulesChanged,
                transforms::record::ForkTransformViewChanges,
                commands::transforms::ForkTransformsChanged,
                scratchpad::ForkScratchpadChanged,
                scratchpad::ForkScratchpadOpen,
                app_menu::ForkMenuAction,
                update::scheduler::ForkUpdateState,
                onboarding::practice::ForkPracticeTake,
                tutorials::practice::ForkLessonShortcut,
                shortcut::health::ForkShortcutHealthChanged,
                shortcut::health::ForkShortcutFixRequested,
                pill::spotlight::ForkSpotlightState
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

    /// Mac wave 6 test hook (`hub::test_keep_painting`): keeps a window's
    /// web view painting while it is occluded, in test debug builds only.
    fn test_keep_painting(_window: &tauri::WebviewWindow<Self>) {}

    /// Applies Handy's appearance setting to the main window, as Handy's
    /// `.setup()` does after building it (S-38: a rebuilt Hub). No-op on
    /// runtimes without Handy's settings.
    fn hub_apply_theme(app: &AppHandle<Self>);

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
    /// S-27 delete-all phase 1: stop recording, jobs and downloads.
    fn stop_jobs_for_erase(app: &AppHandle<Self>) -> Result<(), String>;
    /// S-27 delete-all phase 2: unload speech models and stop the sidecar.
    fn unload_models_for_erase(app: &AppHandle<Self>) -> Result<(), String>;
    /// S-27 delete-all phase 3 (before files go): close the settings stores
    /// and clear webview storage.
    fn release_for_erase(app: &AppHandle<Self>);
    /// S-26a: whether this runtime has native menus. `MockRuntime` has
    /// none (its main-thread calls run inline on the caller and AppKit menus
    /// must not be built off the main thread), so tests never build menus or
    /// list the Mac's microphones for them.
    fn native_menus() -> bool;
    /// S-26a menus: Copy last dictation (Dictation menu), as ⌃⌘C does.
    fn menu_copy_last(app: &AppHandle<Self>);
    /// S-26a menus: Paste last dictation (tray), as ⌃⌘V does.
    fn menu_paste_last(app: &AppHandle<Self>);
    /// S-26a tray: a take is in progress (Handy's tray state; Cancel shows).
    fn menu_dictation_in_progress(app: &AppHandle<Self>) -> bool;
    /// S-26a tray: Secure Input blocks the shortcuts (Handy's warning item).
    fn menu_shortcuts_blocked(app: &AppHandle<Self>) -> bool;
    /// S-28: a take records, transcribes, cleans up or is delivered, or a
    /// transform runs (updates never install or relaunch then).
    fn update_busy(app: &AppHandle<Self>) -> bool;
    /// S-28: what AppKit says about the app and the main window, which
    /// Tauri's `is_visible` does not see (Mac wave 2 finding 2). Fills
    /// `app_hidden` and `main_on_screen`; `MockRuntime` reads
    /// `update::scheduler::FakeAppKit`.
    fn update_appkit_view(app: &AppHandle<Self>, view: &mut update::scheduler::HubView);
    /// S-26b: shows the macOS share sheet in the main window. `MockRuntime`
    /// records the request instead (`share::recorded`).
    fn show_share_sheet(app: &AppHandle<Self>, request: share::ShareRequest) -> Result<(), String>;
    /// S-02: rebuilds Handy's handy-keys tap (`shortcut::handy_keys`).
    fn handy_keys_rebuild(app: &AppHandle<Self>) -> Result<(), String>;
    /// S-02: Handy's Secure Input monitor, `None` without it.
    fn secure_input_info(app: &AppHandle<Self>) -> Option<SecureInputInfo>;
    /// S-02: re-runs Handy's Carbon fallback (off the calling thread).
    fn secure_input_reconcile(app: &AppHandle<Self>);
    /// S-02: re-syncs Handy's tray (the warning badge).
    fn tray_refresh(app: &AppHandle<Self>);
}

/// What Handy's Secure Input monitor knows (S-02).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SecureInputInfo {
    /// Secure Input is on right now (live check).
    pub enabled: bool,
    /// On for ≥ 3 s: Handy's Carbon shadows are registered.
    pub sustained: bool,
    /// The holder from the IORegistry, when known.
    pub pid: Option<i32>,
    /// Its process name (`ps`).
    pub process: Option<String>,
}

impl ForkRuntime for tauri::Wry {
    fn models_start(app: &AppHandle<Self>) {
        models::setup::start(app);
    }

    fn handy_keys_rebuild(app: &AppHandle<Self>) -> Result<(), String> {
        crate::shortcut::handy_keys::rebuild_shortcuts(app)
    }

    fn secure_input_info(app: &AppHandle<Self>) -> Option<SecureInputInfo> {
        app.try_state::<crate::commands::ShortcutsInitialized>()?;
        let s = crate::secure_input::get_secure_input_status(app.clone());
        Some(SecureInputInfo {
            enabled: s.enabled,
            sustained: s.sustained,
            pid: s.culprit_pid,
            process: s.culprit_name,
        })
    }

    fn secure_input_reconcile(app: &AppHandle<Self>) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            crate::secure_input::reconcile_fallback(&app);
        });
    }

    fn tray_refresh(app: &AppHandle<Self>) {
        crate::tray::refresh_tray_icon(app);
    }

    fn recovery_start(app: &AppHandle<Self>) {
        recovery::on_ready(app);
        insert::own_caret::on_ready(app);
    }

    fn stop_jobs_for_erase(app: &AppHandle<Self>) -> Result<(), String> {
        erase::stop_jobs(app)
    }

    fn unload_models_for_erase(app: &AppHandle<Self>) -> Result<(), String> {
        erase::unload_models(app)
    }

    fn release_for_erase(app: &AppHandle<Self>) {
        erase::release_for_erase(app);
    }

    fn native_menus() -> bool {
        true
    }

    fn menu_copy_last(app: &AppHandle<Self>) {
        shortcut::actions::copy_last::run(app);
    }

    fn menu_paste_last(app: &AppHandle<Self>) {
        shortcut::actions::paste_last::run(app);
    }

    fn menu_dictation_in_progress(app: &AppHandle<Self>) -> bool {
        crate::tray::dictation_in_progress(app)
    }

    fn menu_shortcuts_blocked(app: &AppHandle<Self>) -> bool {
        crate::secure_input::tray_warning_active(app) || shortcut::health::tray_warning()
    }

    fn update_appkit_view(app: &AppHandle<Self>, view: &mut update::scheduler::HubView) {
        update::scheduler::appkit_view(app, view);
    }

    fn update_busy(app: &AppHandle<Self>) -> bool {
        crate::tray::dictation_in_progress(app)
            || app
                .try_state::<crate::TranscriptionCoordinator>()
                .is_some_and(|c| c.is_busy())
            || transforms::run::is_active()
    }

    fn show_share_sheet(app: &AppHandle<Self>, request: share::ShareRequest) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            let window = app
                .get_webview_window("main")
                .ok_or_else(|| "no main window".to_string())?;
            share::show(&window, request)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (app, request);
            Err("the share sheet is available on macOS".into())
        }
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

    fn test_keep_painting(window: &tauri::WebviewWindow<Self>) {
        #[cfg(target_os = "macos")]
        hub::test_keep_painting(window);
        #[cfg(not(target_os = "macos"))]
        let _ = window;
    }

    fn hub_apply_theme(app: &AppHandle<Self>) {
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        crate::shortcut::apply_window_theme(app, crate::settings::get_settings(app).theme);
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let _ = app;
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

    fn handy_keys_rebuild(_app: &AppHandle<Self>) -> Result<(), String> {
        Ok(())
    }

    fn secure_input_info(_app: &AppHandle<Self>) -> Option<SecureInputInfo> {
        None
    }

    fn secure_input_reconcile(_app: &AppHandle<Self>) {}

    fn tray_refresh(_app: &AppHandle<Self>) {}

    fn models_start(_app: &AppHandle<Self>) {}

    fn recovery_start(_app: &AppHandle<Self>) {}

    fn stop_jobs_for_erase(_app: &AppHandle<Self>) -> Result<(), String> {
        Ok(())
    }

    fn unload_models_for_erase(_app: &AppHandle<Self>) -> Result<(), String> {
        Ok(())
    }

    fn release_for_erase(_app: &AppHandle<Self>) {}

    fn native_menus() -> bool {
        false
    }

    fn menu_copy_last(_app: &AppHandle<Self>) {}

    fn menu_paste_last(_app: &AppHandle<Self>) {}

    fn menu_dictation_in_progress(_app: &AppHandle<Self>) -> bool {
        false
    }

    fn menu_shortcuts_blocked(_app: &AppHandle<Self>) -> bool {
        false
    }

    /// Tests set it with `update::scheduler::FakeAppKit`.
    fn update_appkit_view(app: &AppHandle<Self>, view: &mut update::scheduler::HubView) {
        if let Some(fake) = app.try_state::<update::scheduler::FakeAppKit>() {
            let fake = *fake.0.lock().unwrap_or_else(|e| e.into_inner());
            view.app_hidden = fake.app_hidden;
            view.main_on_screen = fake.main_on_screen;
        }
    }

    /// Tests set it with `update::scheduler::FakeBusy`.
    fn update_busy(app: &AppHandle<Self>) -> bool {
        app.try_state::<update::scheduler::FakeBusy>()
            .is_some_and(|b| b.0.load(std::sync::atomic::Ordering::Acquire))
    }

    fn show_share_sheet(
        _app: &AppHandle<Self>,
        request: share::ShareRequest,
    ) -> Result<(), String> {
        share::recorded::REQUESTS
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(request);
        Ok(())
    }

    fn hub_title_bar_height(_window: &tauri::WebviewWindow<Self>) -> f64 {
        0.0
    }

    fn hub_hide_title_text(_window: &tauri::WebviewWindow<Self>) {}

    fn hub_apply_theme(_app: &AppHandle<Self>) {}

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
    // The publik edition's sign-in window (docs/fork/EDITIONS.md).
    #[cfg(feature = "publik")]
    let gate = publik::gate::invoke_handler::<R>();
    move |invoke: Invoke<R>| {
        #[cfg(feature = "publik")]
        if publik::gate::handles(invoke.message.command()) {
            return gate(invoke);
        }
        if is_fork_command(invoke.message.command()) {
            fork(invoke)
        } else {
            let observed = observe_handy_command::<R>(invoke.message.command());
            let app = observed.map(|_| invoke.message.webview().app_handle().clone());
            let handled = handy(invoke);
            if let (Some(after), Some(app)) = (observed, app) {
                after(app);
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
///
/// Audit OT2: Handy's Custom words row syncs into the Dictionary page at
/// once (both ways: the page already rewrites Handy's list), and turning
/// the menu bar icon off never leaves the app without a Dock icon too.
pub fn observe_handy_command<R: ForkRuntime>(command: &str) -> Option<fn(AppHandle<R>)> {
    match command {
        "update_custom_words" => Some(|app| {
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(300));
                dictionary::migrate::sync(&app);
            });
        }),
        "change_show_tray_icon_setting" => Some(|app| {
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(300));
                commands::settings::keep_a_way_back(&app);
            });
        }),
        "change_spoken_languages_setting" => Some(|_| {
            std::thread::spawn(|| {
                // Handy writes the setting synchronously in its command;
                // give the store a moment anyway.
                std::thread::sleep(std::time::Duration::from_millis(300));
                models::setup::refresh();
                // S-26a: the menus' Languages ▸.
                app_menu::request_refresh(app_menu::Refresh::Force);
            });
        }),
        // S-26a: the menus' Microphone ▸ follows Handy's own picker too.
        "set_selected_microphone" => Some(|_| {
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_millis(300));
                app_menu::request_refresh(app_menu::Refresh::Devices);
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
        // S-38: crash reports, `ps`, Console and permission prompts show the
        // executable's name, not Handy's crate name.
        assert_eq!(config.main_binary_name.as_deref(), Some("wispr-bro"));
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
