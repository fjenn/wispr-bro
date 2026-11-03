# Wispr Bro fork architecture

How the fork layer plugs into Handy. This is the F-05 baseline; later slices add
modules under the same namespace but must keep these rules true. The plan is
`docs/research/R5-prd-and-slices.md` §4.1–§4.2.

Line references are for **tauri 2.11.5**, **tauri-specta 2.0.0-rc.21**
(`tauri-specta-macros` 2.0.0-rc.16), and Handy's files as of this fork's branch
point. They were read from `~/.cargo/registry/src/*/tauri-2.11.5/` and friends.
Re-check them when Tauri or tauri-specta is upgraded.

## Where fork code lives

| Layer          | Location                                                                                                                                                                                                                                                                                                           |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Rust           | `src-tauri/src/fork/` (`mod.rs`, `events.rs`, `i18n.rs`, `tray.rs`, `app_menu.rs` (S-26a), `net.rs`, `settings.rs`, `commands/` (incl. `selfcheck.rs`, `csp.rs`, `history.rs`), `db/`, `context/`, `stats/`, `llm/`, `diagnostics/`, `models/`, `ax/` (S-05), `recovery/` (S-06), `testutil.rs` under `cfg(test)`) |
| React          | `src/fork/` (`index.ts`, `i18n.ts`, `bindings.ts` (generated), `debug/`, `ui/`)                                                                                                                                                                                                                                    |
| Strings        | `src/i18n/locales/<lng>/fork.json` (+ `fork.flags.json` sidecars)                                                                                                                                                                                                                                                  |
| Capabilities   | `src-tauri/capabilities/fork.json`                                                                                                                                                                                                                                                                                 |
| Config overlay | `src-tauri/tauri.fork.conf.json` (identity, F-01)                                                                                                                                                                                                                                                                  |
| Icons          | `src-tauri/icons-fork/` (app, + `src/*.svg` sources), `src-tauri/resources/fork/tray/` (tray)                                                                                                                                                                                                                      |
| Scripts        | `scripts/wb-*.ts`                                                                                                                                                                                                                                                                                                  |
| UI tests       | `tests/fork/`, `playwright.fork.config.ts`                                                                                                                                                                                                                                                                         |
| Cleanup eval   | `eval/cleanup/` (items, README, `prompts/` (RS-07 upstream prompts, MIT), gitignored `reports/`), `scripts/wb-eval-cleanup.ts`, RS-07 bench `scripts/wb-bench-cleanup.ts`                                                                                                                                          |
| Manual tests   | `docs/fork/MAC-TEST-SCRIPT.md`                                                                                                                                                                                                                                                                                     |
| CI guards      | `scripts/wb-guard*.ts`, `ci/`, `.github/workflows/wb-*.yml`, `about.toml`                                                                                                                                                                                                                                          |

Every fork source file starts with `// SPDX-License-Identifier: GPL-3.0-or-later`.
Exception: AGPL ports start with `// SPDX-License-Identifier: AGPL-3.0-only`
(today `fork/diagnostics/sanitizer.rs`); every port is listed in `docs/fork/PORTS.md`.
Generated files (`src/fork/bindings.ts`) and JSON files are exempt.

## Upstream hooks (F-05)

| File                    | Hook                                                                                          |
| ----------------------- | --------------------------------------------------------------------------------------------- |
| `src-tauri/src/lib.rs`  | `pub mod fork;` (public so `main.rs` can reach `early_init`)                                  |
| `src-tauri/src/lib.rs`  | `.plugin(fork::init())` in the plugin chain (`lib.rs:901`)                                    |
| `src-tauri/src/lib.rs`  | `.invoke_handler(fork::compose_invoke(invoke_handler))` (`lib.rs:1134`)                       |
| `src-tauri/src/main.rs` | `handy_app_lib::fork::early_init();` before `run()` (`main.rs:37`)                            |
| `src/i18n/index.ts`     | `import { registerForkI18n } from "@/fork";` + `registerForkI18n(i18n);` after `init`         |
| `src-tauri/Cargo.toml`  | `[dev-dependencies]` block marked `# fork: … (begin)` / `(end)`: `tauri` with feature `test`  |
| `package.json`          | scripts `wb:dev`, `wb:build`, `wb:translate`, `wb:check:translations` (no dependency changes) |

## Upstream hooks (F-01)

| File                                               | Hook                                                                                                                                           |
| -------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/Cargo.toml`                             | ⚠ `version = "1.0.0-alpha.1"` (DEC-48; re-apply at each sync)                                                                                 |
| `package.json`                                     | ⚠ `"version": "1.0.0-alpha.1"` (DEC-48); scripts `wb:brand`, `wb:icons`, `wb:test:scripts`; `wb:check:translations` also runs the brand check |
| `src-tauri/src/tray.rs` `build_menu`               | `crate::fork::tray::extend(app, &menu, &mut tooltip, &inputs.locale)?;` before the final `Ok`                                                  |
| `src-tauri/src/tray.rs` `get_icon_path`            | first statement `if let Some(path) = crate::fork::tray::icon_path(&theme, state, warning) { return path; }` (3 lines after rustfmt)            |
| `src-tauri/src/lib.rs` log target                  | `file_name: Some("handy".into())` → `Some(fork::FILE_PREFIX.into())` (2 lines, portable and normal target)                                     |
| `src-tauri/src/actions.rs:701`                     | `format!("handy-{}.wav", …)` → `crate::fork::recording_file_name(…)` (2 lines after rustfmt)                                                   |
| `src-tauri/src/managers/transcription_jobs.rs:134` | `format!("handy-file-{}.wav", …)` → `crate::fork::file_job_name(…)`                                                                            |

`tauri.conf.json`, `Info.plist`, `Entitlements.plist` and Handy's 26
`translation.json` files are **not** edited.

`build.rs` is not edited and still ends in plain `tauri_build::build()`.

## Init order

Verified in tauri 2.11.5:

1. **`Builder::build()`** (`app.rs:2243`) ends by calling
   `app.manager.initialize_plugins(handle)` (`app.rs:2440`), which runs every
   plugin's `setup` in registration order (`manager/mod.rs:483-489`,
   `plugin.rs:906-915`). So the **fork plugin's `setup` runs inside `build()`**,
   before the event loop starts and before Handy's own `.setup()` closure.
2. **`App::run()`** starts the event loop. On the first
   `RuntimeRunEvent::Ready` (`app.rs:1423-1428`) Tauri calls `setup(&mut self)`
   (`app.rs:2521-2535`): it creates config windows (none for Handy) and then runs
   the app's `.setup()` closure (`app.rs:2531`). That is Handy's closure at
   `lib.rs:916`, which mounts Handy's events (`lib.rs:925`) and synchronously
   creates the managers through `initialize_core_logic` (`lib.rs:1053`, managed
   at `lib.rs:192-219`).
3. Right after that, still for the same `Ready`, Tauri calls
   `on_event_loop_event` (`app.rs:1427`), which converts the event to
   `RunEvent::Ready` and dispatches it to every plugin's `on_event`
   (`app.rs:2644-2648`). The fork plugin's `on_event` calls **`fork::on_ready`**.
4. Then the app's own `run` callback sees `RunEvent::Ready` (`app.rs:1428`).

Rules that follow:

- **Plugin `setup`** (`fork::init`) only registers fork state: `ForkState`, and
  mounts fork events into the event registry. Later slices add the fork
  settings store and the `fork.db` handle here. It must not touch Handy
  managers, windows, menus or the tray; they do not exist yet.
- **`fork::on_ready`** does everything that needs Handy (today: confirm the
  `ModelManager` is managed, mark `ForkState` ready, log `fork ready`).
- **Headless CLI modes** (`--transcribe-file`, `--list-devices`,
  `--list-models`; `fork::is_headless` mirrors `headless_mode` at `lib.rs:811`)
  make `on_ready` return immediately. Handy's setup returns early in that mode
  (`lib.rs:934`) and the process exits from a worker thread; the fork creates
  no windows and stays "not ready".
- **Fork commands** read Handy state with `try_state` and return the typed
  error `ForkError::NotReady` (`{ "kind": "NotReady" }`) when it is missing or
  `on_ready` has not run. They never panic.
- **`fork::early_init`** runs in `main.rs` before `run()`, before any Tauri
  object exists. F-01 maps the updater kill switch; F-08 adds the panic hook
  and `HF_HOME` (see "Privacy cleanups (F-08)").

The tests in `src-tauri/src/fork/testutil.rs` prove the order on a real Tauri
app (`tauri::test::MockRuntime`): `fork_model_status` returns `NotReady` after
`build()` and Handy-shaped model state after `Ready`.

## Command routing

- Fork commands are **app commands** named `fork_*`, collected by a **second
  tauri-specta `Builder`** with no `plugin_name` (`fork::specta_builder`).
- `fork::compose_invoke(handy_handler)` returns one handler for
  `tauri::Builder::invoke_handler` (the only `invoke_handler` call in Handy's
  builder). It reads `invoke.message.command()` (public, `ipc/mod.rs:543`) and
  sends a name starting with `fork_` to the fork handler and every other name
  to Handy's. Routing must happen by name before the call because a handler
  consumes the `Invoke`.
- No Handy command may start with `fork_` (test
  `no_handy_command_name_starts_with_fork_prefix` parses `collect_commands![…]`
  in `lib.rs` and the generated `src/bindings.ts`).
- **ACL**: Tauri checks the ACL only for `plugin:` commands, when the app
  defines an app ACL manifest, or for remote origins (`webview/mod.rs:1819-1827`).
  Handy's `build.rs` defines no app manifest (no `__app-acl__` entry in
  `gen/schemas/acl-manifests.json`), so `fork_*` commands from local pages need
  no permission, in debug and in release.
- The `fork` plugin registers **no commands**. An in-app plugin has no
  permission manifest, so `fork:default` does not exist; plugin commands would
  need an `InlinedPlugin` manifest in `build.rs` and the `plugin:fork|` prefix.

### Bindings

- `compose_invoke` exports `src/fork/bindings.ts` under `cfg(debug_assertions)`
  with the same exporter settings as Handy's export at `lib.rs:799-805`
  (`Typescript::default().bigint(BigIntExportBehavior::Number)`, path relative
  to `src-tauri/`). Handy's `src/bindings.ts` is regenerated by Handy's own
  builder only and never contains fork commands or events.
- Test `fork_bindings_are_current_and_handy_bindings_untouched` fails when
  `src/fork/bindings.ts` is stale and checks that fork codegen leaves Handy's
  file byte-identical. Regenerate with a debug run (`bun run wb:dev`) or
  `WB_WRITE_FORK_BINDINGS=1 cargo test --lib fork::`.
- **Deviation (one builder per runtime).** specta's `collect_functions!`
  expands each command into an inner item, which cannot name a generic
  parameter of the enclosing fn (`E0401`). So the builder is built by a macro
  per concrete runtime, behind the `fork::ForkRuntime` trait: `tauri::Wry` in
  the app and `tauri::test::MockRuntime` in tests. `fork::init` and
  `compose_invoke` are generic over `ForkRuntime`.

## Identity (F-01)

### Overlay and how Tauri applies it

`bun run wb:dev` / `wb:build` pass `--config src-tauri/tauri.fork.conf.json`.
tauri-cli 2.11 applies it as an **RFC 7396 JSON merge patch** on top of
`tauri.conf.json` (`json_patch::merge`; tauri-build 2.6.3 does the same with
the `TAURI_CONFIG` env var the CLI forwards, `tauri-build/src/lib.rs:487-489`).
Objects merge, `null` deletes, **arrays replace** (so `bundle.icon` is ours
only). Plain `cargo build` / `bun run tauri dev` still build Handy's identity.

| Key                                  | Value                                                                                                                                                                                                                                                                                                                                                                                      |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `productName`                        | `Wispr Bro` (also names Tauri's default macOS app menu)                                                                                                                                                                                                                                                                                                                                    |
| `mainBinaryName`                     | `wispr-bro` (S-38): the bundle's `Contents/MacOS/wispr-bro`, the name crash reports, `ps`, Console and permission prompts show. The Cargo crate and lib stay `handy`/`handy_app_lib`, so `target/{debug,release}/handy` keeps Handy's name; scripts read `CFBundleExecutable` from a bundle. TCC follows the bundle id, so grants are unaffected; crash pickup matches `.ips` by bundle id |
| `identifier`                         | `com.wisprbro.desktop` (data, logs, TCC are separate from Handy)                                                                                                                                                                                                                                                                                                                           |
| `version`                            | `1.0.0-alpha.1` = `Cargo.toml` = `package.json` (DEC-48)                                                                                                                                                                                                                                                                                                                                   |
| `bundle.icon`                        | `icons-fork/{32x32,128x128,128x128@2x}.png`, `icon.icns`, `icon.ico`                                                                                                                                                                                                                                                                                                                       |
| `bundle.macOS.minimumSystemVersion`  | `13.0` (DEC-22)                                                                                                                                                                                                                                                                                                                                                                            |
| `plugins.updater.pubkey`/`endpoints` | Ours (F-02): minisign key `89E085217ACC46CF` and the stable feed `https://github.com/fjenn/wispr-bro/releases/latest/download/latest.json`. Never Handy's feed/key, so a Wispr Bro build can never install a Handy update. The beta feed is chosen at runtime (`fork/update/channel.rs`). The guard and `channel::tests` pin both; changing either strands installs (RELEASING.md).        |

`bundle.externalBin`: `target/wb-sidecar/llama-server` (S-10, below). `bundle.windows.signCommand: null` (F-02, E-59): the merge patch deletes Handy's Azure Trusted Signing command, so no Wispr Bro build can ever call it. Not set yet: `license`, Linux `/usr/lib/Handy` paths (F-04, not M1 macOS).

**Usage strings.** Tauri 2.11 has `bundle.macOS.infoPlist` (tauri-utils
2.9.3 `config.rs:661-666`: a path to a plist merged with the default one).
F-01 does not need it: Handy's `Info.plist` has only
`NSMicrophoneUsageDescription` = "Request microphone access to transcribe
audio locally", which names no brand (macOS prefixes the prompt with the
bundle name, now "Wispr Bro"). There is no `NSAppleEventsUsageDescription`
(AppleScript is banned), and the Accessibility prompt has no usage string. So
`Info.plist` stays untouched. If a later slice needs fork wording, add
`bundle.macOS.infoPlist` in the overlay and verify on a bundled build which
file wins (not verified in F-01).

**Version.** `Cargo.toml` and `package.json` carry `1.0.0-alpha.1`, so
`CARGO_PKG_VERSION` (Handy's tray label, `settings.rs:605` What's New) and
`getVersion()` agree with the overlay. Test
`cargo_and_package_versions_equal_the_overlay_version`.

### Names Handy hard-codes

| Where                                   | Handy                                   | Fork                                                                                                                   |
| --------------------------------------- | --------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| Main window title (`lib.rs` setup)      | `Handy`                                 | `fork::on_ready` sets `rust.window.mainTitle`                                                                          |
| Tray version item + tooltip (`tray.rs`) | `Handy v…`                              | `fork::tray::extend` → `rust.tray.version`/`versionDev`                                                                |
| Tray glyphs (`resources/tray_*.png`)    | Handy hand                              | `fork::tray::icon_path` → `resources/fork/tray/*` (Dark/Light themes; Linux "Colored" keeps Handy's until Linux ships) |
| Log file                                | `handy.log`                             | `wispr-bro.log`                                                                                                        |
| Recordings                              | `handy-<ts>.wav`, `handy-file-<ms>.wav` | `wispr-bro-<ts>.wav`, `wispr-bro-file-<ms>.wav` (existing rows keep their stored names)                                |
| Web UI strings (33 keys × 26 locales)   | "Handy"                                 | `fork.json > overrides` (below)                                                                                        |

These could not be set "in `on_ready`" as the slice text says: the log target
is built in the plugin chain before any fork code runs, and the WAV names are
string literals at the call site. Each needed the one-line hook listed above.

Known leftovers (not user-visible UI, out of F-01): `cli.rs` clap name/about
(`handy`, "Handy - Speech to Text", shown by `--help`), the
`handy-external-script-*.sh` temp file (`clipboard.rs:1021`), the
`HANDY_DISABLE_UPDATER` text in a Rust error (`shortcut/mod.rs:831`), the
Cargo package/binary name `handy` and `description = "Handy"`, the crate
name `handy_app_lib` in log targets, `index.html` `<title>handy</title>`
(not shown: the native window title is set by Rust), and the initial tray
tooltip in `lib.rs` (replaced by Handy's first `update_tray_menu` during setup).

### Updater kill switch (E-94)

`fork::early_init` (runs in `main.rs` before `run()`) sets
`HANDY_DISABLE_UPDATER=1` when `WISPRBRO_DISABLE_UPDATER` is truthy (same
truthiness as Handy's `utils::env_flag_enabled`) and Handy's flag is not.
Handy reads its flag lazily (`settings::update_checks_forced_disabled`), so
both variables disable the updater with no upstream edit. Tests:
`e94_both_env_vars_disable_the_updater`, `e94_truthiness_matches_handy_env_flag_enabled`.

### Icons (PLACEHOLDER)

The app icon (five white waveform bars on an indigo rounded square) and the
tray glyphs are **original placeholders**, not final artwork, and contain no
Handy or Wispr Flow artwork. Sources: `src-tauri/icons-fork/src/*.svg`.
Regenerate with `bun run wb:icons` (`scripts/wb-icons.ts`, runs
`tauri icon` for the app icons, drops the android/ios outputs, and renders
64×64 white and black tray glyphs). Tray glyphs live in
`src-tauri/resources/fork/tray/` rather than `icons-fork/` (deviation):
Handy's `bundle.resources: ["resources/**/*"]` bundles them in every build,
so the `icon_path` hook can never point at a missing file, even without the
overlay. Replace the art before the first build that leaves the team.

### Rust strings (`fork/i18n.rs`)

`rust.*` keys live in each `fork.json` and are embedded for all 26 locales
with `include_str!`. Locale: Handy's `app_language` setting (read through
`ForkRuntime::handy_app_language`, `Wry` only), then the OS locale
(`tauri_plugin_os::locale`), then English; tag matching mirrors Handy's
`tray_i18n.rs` (exact, Chinese script/region, language subtag). Missing keys
fall back to English. Keys today: `rust.appName`, `rust.tray.version`,
`rust.tray.versionDev`, `rust.window.mainTitle` (copied English, flagged
`untranslated`; they are brand + version only). Tests:
`every_rust_key_resolves_in_every_locale`, `no_rust_string_names_handy`,
`embeds_all_26_locale_directories`. The tray hook itself needs a native menu,
which muda builds only on the macOS main thread, so `cargo test` covers its
two pure functions and the self-check below covers the live tray.

### Brand overrides (`scripts/wb-brand-overrides.ts`)

For every locale, every key of Handy's `translation.json` whose English or
local text contains `Handy` (or `HANDY_`) gets a rebranded copy under
`fork.json > overrides` (33 keys; 35 in `nl`, which says "Over Handy" for
"About"). The brand is written literally as "Wispr Bro" (deviation from
`{{appName}}`: Handy's call sites pass no `appName`, so a variable would need
i18next default variables; a literal keeps overrides plain strings). Grammar:
Turkish suffixes follow vowel harmony (`Handy'nin` → `Wispr Bro'nun`,
`'yi` → `'yu`, `'ye` → `'ya`); Danish/Swedish genitive `Handys` → `Wispr Bros`;
compounds keep their hyphen. `HANDY_DISABLE_UPDATER` becomes
`WISPRBRO_DISABLE_UPDATER`. Generated values are flagged
`"overrides.<key>": "machine"` in `fork.flags.json` (or `"untranslated"` when
the locale lacks the key); an override without a flag is treated as reviewed
and kept on re-runs. `src/fork/i18n.ts` merges them with
`addResourceBundle(lng, "translation", overrides, true, true)`.
`wb-translate-missing.ts` leaves `overrides` and `overrides.*` flags alone
(they are per-locale, not English-shaped). Checks: `bun run
wb:check:translations` (runs both scripts in `--check` mode),
`bun run wb:test:scripts` (`scripts/wb-brand-overrides.test.ts`: every key in
all 26 locales, no "Handy" left, placeholders and `<tags>` preserved), and
`tests/fork/brand-overrides.spec.ts` (Playwright: loads the real
`src/i18n/index.ts` in the page and checks en, de, tr).

### Startup self-check (`WISPRBRO_SELFCHECK=1`)

`fork::on_ready` calls `commands::selfcheck::start`. When the env var is
truthy (debug **and** release), it evaluates a script in the `main` webview
that goes through the real IPC path: `plugin:event|listen` for
`fork-ping-event`, `fork_ping` and `fork_model_status` through
`compose_invoke`, waits for the event, then reports with the fork command
`fork_selfcheck_report`, which logs one line
`fork selfcheck: PASS|FAIL …` plus an identity line (product name,
identifier, version, live window title, last tray tooltip). The command
refuses reports when the env var is off. This is how the F-05 exit criterion
(routing and ACL in a release build) is checked without clicking the hidden
debug panel.

## Event naming and the shared registry

- tauri-specta derives an event's name from its type name in kebab-case and has
  no per-event override (`tauri-specta-macros/src/lib.rs:23`). Every fork event
  type is named `Fork…`: `ForkPingEvent` → `fork-ping-event`.
- `Builder::mount_events` (`tauri-specta builder.rs:278-290`) calls
  `EventRegistry::get_or_manage` (`event.rs:40-46`), which reuses the registry
  already in Tauri state or creates it. Both builders therefore **merge into
  one `EventRegistry`**, whichever mounts first: the fork plugin mounts in its
  `setup` (inside `build()`), Handy mounts in its `.setup()` on `Ready`. The
  registry key is the event type's specta id, so names cannot collide across
  the two builders as long as the `Fork…` prefix rule holds.
- Test `fork_event_round_trips_through_mount_events` mounts a Handy event into
  the same app, listens for `ForkPingEvent`, calls `fork_ping` through the
  composed handler and receives the event.

## Windows and capabilities

`src-tauri/capabilities/fork.json` grants windows `main` and `fork-*` only
`core:default`, `store:default`, `dialog:default`, `opener:default`. Never
`*`. Fork commands need no permission (see ACL above). S-18 added no
`notification:default`: notifications are native (`objc2-user-notifications`,
RS-10), with no Tauri plugin and no capability.

## Frontend

- `src/i18n/index.ts` calls `registerForkI18n(i18n)` once after `init`
  (imported from `src/fork/index.ts`). It adds namespace `fork` for every
  `locales/<lng>/fork.json`; a top-level `overrides` object, if present, is
  merged into Handy's `translation` namespace (brand overrides, F-01).
- **Deviation (debug host mount).** F-05 had no `App.tsx` hook, so
  `registerForkI18n` in `src/fork/index.ts` also starts the fork frontend: in
  the `main` webview only, it lazy-loads `src/fork/debug/mount.tsx`, which
  renders a hidden host into its own root (`#fork-debug-root`). Other webviews
  (Handy's overlay) skip it, and the panel code is a separate chunk. F-05
  planned to move this into `ForkApp` in S-13; S-13 **keeps it** as a separate
  root, because `ForkApp` only mounts once onboarding is done, while the panel
  must also open over onboarding and the startup self-check
  (`WISPRBRO_SELFCHECK`) needs `window.__wbCspSmoke` in any state.
- **Debug panel** (`src/fork/debug/DebugPanel.tsx`): opened by the same chord
  as Handy's debug-mode toggle (⌘⇧D / Ctrl+Shift+D; `App.tsx` handler) or by
  the route `#/fork/debug`, but only when `fork_ping` reports
  `debug_build` (`cfg(debug_assertions)`) or `cli_debug` (`--debug`). Handy's
  `--debug` override is runtime-only and not visible through
  `get_app_settings`, hence the flag on `fork_ping`. It shows `fork_ping`,
  `fork_model_status` and the last `ForkPingEvent`; later slices mount early
  panels with `registerForkDebugSection`.
- Fork components use `useTranslation("fork")`.

## Hub (S-13)

`src/App.tsx` renders `ForkApp` (`src/fork/hub/ForkApp.tsx`) in place of
Handy's sidebar and section layout: one import and one JSX swap
(`ci/hooks.txt` budget 42; Handy's now-unused helpers above it are left
alone so upstream edits still merge). Handy's onboarding switch around it is
unchanged until S-29a, and `ForkApp` gets the three onboarding callbacks
Handy's Debug section needs as props.

| File                                      | Role                                                                                                                                                                                                   |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `src/fork/hub/nav.tsx`                    | Page ids, the zustand nav store, `navigate(page, section?)` (the deep-link API), URL hash mirror `#/hub/<page>[/<section>]`                                                                            |
| `src/fork/hub/pages.tsx`                  | `HUB_PAGES`: sidebar order, icon, tag and the component per page slot. A feature slice mounts its page by replacing its entry's `component`; until then a placeholder (empty state + one action) shows |
| `src/fork/hub/Sidebar.tsx`                | Buttons with `aria-current`, collapse (icons + tooltips), New!/Beta tags, setup-checklist slot (`setSidebarCard`, S-30), Help menu                                                                     |
| `src/fork/hub/Header.tsx`                 | Drag strip, theme toggle (light ⇄ dark, saved in Handy's `theme`), notifications bell (`setNotificationsPanel`, S-18)                                                                                  |
| `src/fork/hub/Footer.tsx`                 | Background jobs (Handy's `TranscriptionStatus`); Handy's update checker and version until S-28                                                                                                         |
| `src/fork/hub/HandySettingsPage.tsx`      | The Settings slot until S-14: Handy's sections as tabs (section = tab), History is a drop zone                                                                                                         |
| `src/fork/hub/dropRouter.tsx`             | `useDropRouter` (one listener) + `useDropZone(ref, …)`: a file drop reaches only the zone under the pointer; outside every zone it is ignored (RE-33)                                                  |
| `src/fork/ui/*`                           | `Banner` (dismissal in `dismissed_banners`), `EmptyState`, `Tag` (`seen_new_tags`), `Button`, Radix `Menu`, `Popover`, `Tabs`                                                                          |
| `src/fork/styles/tokens.css`, `fonts.css` | `--wb-*` light/dark tokens (AA pairs tested), retune of Handy's background, text and grey tokens; Newsreader + Source Serif 4 faces                                                                    |
| `src-tauri/src/fork/hub.rs`               | From `on_ready`: main window 1040 × 720 page (fits the work area), min 720 × 600 (Handy's), title text hidden, overlay title bar for onboarded users                                                   |

**Title bar.** On macOS the Hub runs under an overlay title bar: `fork::hub`
sets it at Ready when onboarding is done, and `ForkApp` sets it on mount and
restores the standard bar on unmount (onboarding screens). The drag strips
use `data-tauri-drag-region`, which needs `core:window:allow-start-dragging`;
the switch needs `core:window:allow-set-title-bar-style`. Both are in
`capabilities/fork.json`; `core:default` grants neither.

**Fonts.** `scripts/wb-fonts.ts` copies the Fontsource woff2 subsets into
`src/assets/fonts/fork/` (a fork path in `wb-guard-lib.ts`), pins each by
sha256 (`--check` runs in `wb-ci.yml`) and writes the OFL texts in
`LICENSES/`. Nothing is fetched at runtime.

**Fork npm packages.** npm and bun have no comment keys inside dependency
maps, so the fork's packages cannot sit between `//fork` markers. They are:
`@radix-ui/react-dropdown-menu` 2.2.1, `@radix-ui/react-popover` 1.3.1,
`@radix-ui/react-tabs` 1.2.1 (runtime, S-13); `@axe-core/playwright` 4.14.0,
`@fontsource-variable/newsreader` 5.3.0, `@fontsource-variable/source-serif-4`
5.3.1 (dev, S-13). The same list is in the `package.json` comment of
`ci/hooks.txt`.

## Dictation home (S-15a)

`src/fork/home/` fills the Hub's Dictation slot (`HUB_PAGES` in
`pages.tsx`), the page the Hub opens on. No Handy file is touched.

| File                                   | Role                                                                                                                                                                                                                    |
| -------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/fork/home/Home.tsx`               | Layout by container query on the page's own width: ≥ 680 px greeting above, main column + 240 px rail; narrower, one column (greeting, rail, main). History slot (S-15b) until then an empty state → Settings › History |
| `src/fork/home/Greeting.tsx`           | The page's `h1` "Hello, <name>": `greeting_name` (fork setting) → `fork_account_name` (macOS) → "Hello". Pencil button opens a Radix popover to set or clear the override                                               |
| `src/fork/home/useStatsSummary.ts`     | `fork_stats_summary` on mount, on every `ForkStatsUpdated`, on window focus/visibility and after local midnight; overlapping triggers collapse into one extra read; `NotReady` retried at 1, 2, 4, 8, 16 s              |
| `src/fork/home/StatsRail.tsx`          | `<dl>` of total words, words per minute (rounded; a dash with "Not enough speech yet" under 3 s of timed speech) and current day streak, grouped per the app language                                                   |
| `src/fork/home/VoiceProfileBar.tsx`    | `role="progressbar"` over Handy's `ProgressLine` (recoloured), goal 1,700 words (DEC-50); unlocked → "Open Insights" (`navigate("insights", "voice")`, S-25)                                                            |
| `src/fork/home/PromoBanner.tsx`        | The Hub `Banner` with id `hub.dictation.start` (S-13's id, so an earlier dismissal holds)                                                                                                                               |
| `src-tauri/src/fork/commands/stats.rs` | `fork_account_name`: first word of `NSFullUserName` (objc2-foundation, in tree), surrounding punctuation trimmed, ≤ 64 chars; `None` off macOS or for an empty name                                                     |

The numbers are S-09's `daily_stats` read as they are (`fork_stats_summary`
stayed in `commands/history.rs`); the rail does no maths of its own, so it
matches `daily_stats` by construction. `Popover` (`src/fork/ui/`) gained an
optional controlled `open`/`onOpenChange` for the name editor.

## History and row actions (S-15b)

History fills the home's main column (`Home.tsx` renders `HistoryList`;
the S-15a empty-state slot is gone). Handy's own Settings › History page
stays reachable (hosted until S-14) and still is the drop zone for files.
One Handy hook line changed; no migration (every column was already in
0001/0006).

| File                                     | Role                                                                                                                                                                                                                                                                                                                            |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/fork/db/history_list.rs`  | The read side: Handy's rows (through `HandyHistory`, which gained `page`) merged with `entry_meta`: shown text and copy text by variant, `ai_edited`, audio state, source/file tags, flag. `page` refills from the next Handy page around soft-deleted rows; `find` pages S-09's search by offset; `set_variant`; `record_flag` |
| `src-tauri/src/fork/commands/history.rs` | `fork_history_page`, `_find`, `_rows`, `_set_variant`, `_retry`, `_flag` (+ `save_flag`), `_save_audio` (+ `copy_atomically`); `fork_history_delete` now also cancels a queued retry; `allow_recordings_in_asset_scope` (Ready)                                                                                                 |
| `src-tauri/src/fork/llm/post_process.rs` | `set_retry_level` / `take_retry_level`: one cleanup level per entry for its next job; `post_process_job` takes the `HistoryEntry`; `plan` takes a level override                                                                                                                                                                |
| `src-tauri/src/fork/db/entry_meta.rs`    | A retry (`write(…, retry = true)`) resets `shown_variant` to `final` and clears `inserted_text` (the old take's snippet no longer applies)                                                                                                                                                                                      |
| `src/fork/history/HistoryList.tsx`       | Section with heading, search, "Transcribe a file…" (Handy's picker); the virtual list (@tanstack/react-virtual 3.14.23, MIT) in `wb-main`'s scroll, day headings + rows measured as they render, next page 15 items before the end; every action handler                                                                        |
| `src/fork/history/HistoryRow.tsx`        | Time, tags (file name, Practice, "Before AI cleanup", Flagged), text folded to 4 lines, job progress (Handy's `transcriptionJobsStore`), actions play/copy/flag/⋮ shown on hover and `focus-within`; Play mounts Handy's `AudioPlayer` with autoplay                                                                            |
| `src/fork/history/RowMenu.tsx`           | ⋮ on S-13's `Menu`: Undo/Redo AI edit (only when the LLM changed the text), Retry transcript + the two other levels, Save audio as… (disabled with the reason as its label), Delete transcript                                                                                                                                  |
| `src/fork/history/FlagDialog.tsx`        | "Wrong word?": words as `aria-pressed` buttons (`Intl.Segmenter`, so CJK and accents work), widen to ≤ 4 neighbours, correction field, replacement (default) or word; S-16's feedback (duplicate, conflict + "Replace it", invalid)                                                                                             |
| `src/fork/history/Search.tsx`            | Search button → field (`role="search"`), 200 ms debounce, Esc clears, closes and refocuses the button                                                                                                                                                                                                                           |
| `src/fork/history/{useHistory,dates}.ts` | Zustand store (pages, search, live `history-update-payload` added/updated/deleted, a new row read again after 1.5 s for `inserted_text`, `NotReady` retried 1-16 s); local-day keys, Today/Yesterday/dates, the midnight timer                                                                                                  |

**Texts.** `entry_meta.final_text` is shown (the spoken words: search and
stats use it). "Raw" (Undo AI edit) is Handy's `transcription_text`, the
deterministic text before the LLM cleanup (S-11's rules, Handy's filler
removal and word fix included): what Wispr's "Undo AI edit" gives back.
`ai_edited` = Handy's `post_processed_text` exists and differs. Copy copies
the shown variant: raw, or `inserted_text` (a snippet's saved text, S-17)
else the final text. The variant never changes the stats or the search
index, and nothing is pasted again (DEC-19).

**Retry.** Handy's `TranscriptionJobManager::retry` (background job, ahead
of files; C-43). The level asked for is kept in a per-entry slot the job's
`post_process_job` takes (one retry each); `None` uses the setting. The
replaced text goes to `previous_text` (S-09) and the row shows the new text
as made. Level None on a retry skips the LLM only: Handy's filler removal
follows the global setting.

**Audio.** `available` (no `audio_deleted_at`, WAV on disk), `expired`
(`audio_deleted_at` after the take: retention) or `missing` (set at the take's
start by a full disk, or the file is gone). Play uses Handy's
`get_audio_file_path` + `convertFileSrc(…, "asset")`; portable mode's
recordings dir is outside the static `$APPDATA/**` scope, so Ready adds that
one directory to the asset scope at run time (the E-88 note under CSP).
Save audio as… = tauri-plugin-dialog `save` (`dictation-YYYY-MM-DD-HHMM.wav`)

- `fork_history_save_audio` (copy to `<name>.partial`, rename).

**Flag** (DEC-18) writes through S-16's store with source `flag`
(`store::add`, or `store::update` of the conflicting entry for "Replace
it"), then `entry_meta.flagged = 1` and one `corrections` row (source
`flag`, which S-25 counts). A duplicate still flags the row. A blank
correction is refused in Rust too (it would turn a replacement into a
word).

**Delete** = S-09's soft delete: the row leaves the list at once, a sonner
toast "Deleted · Undo" stays 9 s (the purge runs after 10 s), Undo restores
and re-reads the row.

**Fork npm package.** `@tanstack/react-virtual` 3.14.23 (MIT, runtime; R4
"Virtual list"). Handy's own `AudioPlayer`, `copyToClipboard` and
`pickAndTranscribeFiles` are reused by import.

## Insights (S-25)

`src/fork/insights/` fills the Hub's Insights slot. Tabs Usage and Voice
(the tab is the Hub section: `#/hub/insights/voice`, `navigate("insights",
"voice")` from the Home voice-profile button) and "Save as image". Every
number is computed on this Mac from `fork.db`; no Handy file is touched and
there is no migration (the tables are S-09's).

| File                                                           | Role                                                                                                                                                                                                                                                                                                       |
| -------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/fork/stats/insights.rs`                         | Pure: speed vs typing (`typing_wpm` from the typing test, else 40 wpm "average typist"; ratio to one decimal), the six categories with shares in a fixed order, the 365-day heatmap with bands 0 / 1-249 / 250-499 / 500-749 / 750+, streaks (S-09's `streak.rs`). No percentile anywhere (C-51)           |
| `src-tauri/src/fork/stats/voice.rs`                            | Pure: the voice profile. Locked below 1,700 lifetime words (DEC-50). Phrase (3-5 words, within a clause, counted once per dictation, ≥ 3 dictations, score dictations × words), peak hour and app by words, most used word (stopwords-iso + S-12a's hesitations and fillers excluded), most corrected word |
| `src-tauri/src/fork/stats/tables/stopwords-iso.json`           | stopwords-iso 1.1.0, unmodified (MIT data; `LICENSES/MIT-stopwords-iso.txt`, NOTICE, PORTS.md); the languages in the history plus English are used                                                                                                                                                         |
| `src-tauri/src/fork/db/insights.rs`                            | Read side: `daily_stats` totals and days, `daily_app_stats` per category, the newest 10,000 counted live dictations (`entry_meta`), `corrections` rows of source `flag`/`learn`                                                                                                                            |
| `src-tauri/src/fork/commands/insights.rs`                      | `fork_insights_usage`, `fork_insights_voice` (async; rows read under the lock, cards computed on a blocking thread), `fork_insights_save_image` (base64 PNG → the path from the save panel; `.png` name, PNG magic and ≤ 25 MB checked)                                                                    |
| `src/fork/insights/InsightsPage.tsx`                           | Header with "Save as image", the tabs, a dated line; the export area is the dated line plus the visible tab                                                                                                                                                                                                |
| `src/fork/insights/UsageTab.tsx`                               | Speed, total words, fixes (dictionary / AI cleanup), "Where you dictate", the streak card with figures, heatmap and a text legend                                                                                                                                                                          |
| `src/fork/insights/CategoryChart.tsx` (lazy)                   | Recharts 3.10.1 horizontal bars, one series (`--wb-bar`), fixed order, share as direct labels, hover tooltip; `aria-hidden` with an `sr-only` list of the same numbers; RTL = reversed axis, labels on the right                                                                                           |
| `src/fork/insights/Heatmap.tsx` (lazy)                         | react-activity-calendar 3.2.1, `--wb-heat-0..4`, `Intl` month and weekday names, locale week start, 8-14 px cells sized to the card (a narrow card shows the latest weeks that fit), `role="img"` with a summary, a `<title>` per cell, always left to right                                               |
| `src/fork/insights/VoiceTab.tsx`                               | Locked: progress bar + four cards with blurred generic sample text (`aria-hidden`; nothing of the user's to uncover). Unlocked: the four cards and a footnote with the dictations read                                                                                                                     |
| `src/fork/insights/saveImage.ts`                               | Save panel → html-to-image 1.11.13 (`toPng`, 2×, page background, 24 px margin, a warm-up pass for WebKit fonts; loaded on click) → `fork_insights_save_image`                                                                                                                                             |
| `src/fork/insights/useInsights.ts`                             | Both reads on mount, every `ForkStatsUpdated`, focus/visibility and after midnight; `NotReady` retried at 1-16 s                                                                                                                                                                                           |
| `src/fork/insights/insights.css`, `heatColors.ts`, `format.ts` | Heat and bar tokens (light/dark pairs, `data-theme` wins), the band colours, day formatting shared with the lazy chunks                                                                                                                                                                                    |

**Most corrected word.** S-12a wrote no `corrections` rows for LLM edits
("S-25 decides"). S-25 does not add any: the card diffs what was heard
(`entry_meta.asr_text`, verbatim) against what was inserted (`final_text`)
word by word (LCS, case and punctuation ignored) and counts substitutions of
1-3 words on each side, so dictionary rules, Handy's word fixes and AI
cleanup all count, and a removed filler is not a correction. Rows of source
`rule` are skipped (already in the diff); `flag` and `learn` rows (S-15b,
S-34) are added, since they do not change the stored text.

**CSP.** "Save as image" needs nothing the §4.11 CSP does not already allow:
html-to-image reads the page's own stylesheets and fonts from `'self'`,
inlines them as `data:`, draws a `data:` SVG on a canvas and returns a
`data:` PNG; the bytes go to Rust over IPC. Verified in a release build (see
the S-25 run).

**Fork npm packages.** `recharts` 3.10.1, `react-activity-calendar` 3.2.1 and
`html-to-image` 1.11.13 (all MIT, runtime; R4 "Charts"). Rust: `base64` 0.22
(already in the tree) in the `Cargo.toml` fork block.

## Notifications (S-18)

A local inbox plus a few macOS notifications (DEC-21). No Tauri
notification plugin (RS-10: tauri-plugin-notification 2.4.0 has no click or
action support on macOS desktop and fakes the permission state); macOS
notifications are `UNUserNotificationCenter` through
`objc2-user-notifications` 0.3.2 (no default features) and `block2`, both
already in the tree. No Handy file is touched apart from the `Cargo.toml` fork
block; the delegate is installed from `fork::on_ready`.

| File                                                      | Role                                                                                                                                                                                                                                                                                 |
| --------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `src-tauri/src/fork/notify.rs`                            | `post` (toggle filter → inbox row → `ForkNotificationsChanged` → macOS when allowed and no app window has focus), `activate` (one path for inbox clicks, inbox buttons, macOS clicks and macOS buttons), the producers wired at Ready, `ForkNotificationOpened`, `ForkToastsDismiss` |
| `src-tauri/src/fork/notify/policy.rs`                     | Kinds, toggle groups, `system_allowed` (only model ready, update ready, queued takes ready, shortcut problems), actions, buttons, macOS categories, `resolve` (a button an entry does not offer falls back to the click)                                                             |
| `src-tauri/src/fork/notify/catalog.rs`                    | `Notice` constructors (keys `rust.notify.*`, args, default action, dedupe key), word/streak milestones, tips (one a day, by dictation count), the per-version feature list                                                                                                           |
| `src-tauri/src/fork/notify/inbox.rs`                      | `fork.db` › `notifications` CRUD: insert with `ON CONFLICT (dedupe_key) DO NOTHING`, newest 200 kept, list by filter, mark read / all read                                                                                                                                           |
| `src-tauri/src/fork/notify/macos.rs`                      | `UNUserNotificationCenter`: delegate (click, buttons, "Notification Settings"), categories set before each post, authorization asked by the first delivery only, `bundled()` gate (an unbundled process would raise an Objective-C exception)                                        |
| `src-tauri/src/fork/commands/notifications.rs`            | `fork_notifications_{list,mark_read,mark_all_read,activate,feature,permission,debug_post}`; `debug_post` only in debug builds or with `--debug`                                                                                                                                      |
| `src-tauri/src/fork/db/migrations/0005_notifications.sql` | `notifications.dedupe_key` + partial unique index                                                                                                                                                                                                                                    |
| `src/fork/hub/notifications.ts`                           | Zustand inbox store, entry text (plural keys re-resolved by i18next, numbers grouped per language, `appName`), relative times, `activateNotification`, `runHubAction`                                                                                                                |
| `src/fork/hub/NotificationsPanel.tsx`                     | Bell (unread dot, count in its name), panel (All/Unread, ⋯ menu with Mark all as read and the Suggestions/Announcements/Milestones/Mac notifications toggles, entries with their buttons, empty states), `useNotificationsLive` (events, toasts, Esc)                                |
| `src/fork/hub/FeaturePopup.tsx`                           | The version's feature popup (Handy's `Dialog`), once: `feature.<id>` in `dismissed_banners`                                                                                                                                                                                          |
| `src/fork/debug/NotificationsDebug.tsx`                   | Debug panel: permission state (no prompt) and a delayed sample poster                                                                                                                                                                                                                |
| `scripts/wb-s18-notifications.swift`                      | Mac test helper: lists, waits for, clicks and presses buttons on Notification Center banners through AX                                                                                                                                                                              |

**When macOS is used.** `post` sends a macOS notification only when the kind
is one of the four DEC-21 kinds, the master switch (`notifications.system`)
and that kind's toggle are on, and no window of the app has focus. While the
Hub is in front the new entry is a sonner toast instead (and the delegate's
`willPresent` would show a banner anyway, since the app only posts while
away). The permission request is part of the first delivery
(`getNotificationSettings` → `NotDetermined` → `requestAuthorization`), so
launching, onboarding and every inbox-only entry never prompt.

**Producers (today).** Handy's `model-download-complete` (language detector
excluded) → model ready; the hotkey tap failing to start after onboarding →
shortcut problem (with the Accessibility button when `AXIsProcessTrusted` is
false); the newest crash file under 7 days in the log dir → crash report
(reveals the file); a recreated `fork.db` (`report_db_recovered`, queued per
database path) → data recovered; each `ForkStatsUpdated` → the highest word
milestone and streak milestone not posted yet, and at most one tip a day; the
current version's feature → an announcement at Ready plus the popup.
`models::setup` posts one queued-takes-ready batch (S-07c), and S-28 posts
update ready through `notify::post`.

**Toasts and Esc.** New entries toast while the window has focus. Esc closes
toasts from the window (a `keydown` listener that never stops the event) and
from the hotkey tap (the Esc registry emits `ForkToastsDismiss`), so it is
observed and never blocked.

**Debug.** `WISPRBRO_NOTIFY_DEMO="<ms>:<kind>,…"` (debug builds, or any
build started with `--debug`) posts a sample of each kind `<ms>` apart
through the real path (a sample's dedupe key is cleared first, so samples
repeat); the Mac test script uses it.

**What macOS needs.** `UNUserNotificationCenter` refuses a process that is
not a bundle LaunchServices accepts: an unbundled `tauri dev` binary would
raise an exception (gated by `macos::bundled()`), and an ad-hoc bundle under
`/tmp` is refused ("not allowed"). An ad-hoc release bundle in
`~/Applications` works (S-18 Mac run); the notarized Developer ID build is
Mac test S-18.2.

## Flow Bar (S-03)

The bar is two windows. At rest it is a **native** `NSPanel` (no web view,
no WebKit process): RS-03 measured the always-visible web view pill growing
~49 MB over 97 takes. Around a take a `fork-pill` **web view** panel shows
the active states; it is built when fn goes down (or a take starts some
other way), shown once its page has painted the current state, and
destroyed `WEB_LINGER_MS` (10 s) after the take, so its WebContent process
exits. Handy's `recording_overlay` is never built in product builds, and
F-07's `ENFORCED` now turns Handy's `overlay_style` off
(`HANDY_OVERLAY_REPLACED = true`).

| File                                                               | Role                                                                                                                                                                                                                                                                       |
| ------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/fork/pill/model.rs`                                 | Pure state machine: held, hands-free, transcribing, cleaning up, message; mic label 1.5 s; warnings; message durations; a 150 ms grace after Handy's hide (Handy hides first and reports an empty take right after); resting visibility (DEC-38, DEC-14, Hide for an hour) |
| `src-tauri/src/fork/pill/mod.rs`                                   | The controller thread `wb-pill`: hooks and listeners → model → `panel::Scene` on the main thread; `ForkPillState` / `ForkPillLevels` events; settings writes (hide for an hour, drag positions); VoiceOver announcements (`rust.pill.*`)                                   |
| `src-tauri/src/fork/pill/native.rs`                                | The resting panel and view: capsule, hover bubble (native tracking area), click → hands-free take, right-click menu, horizontal drag, a "bridge" capsule while the web view loads, VoiceOver button                                                                        |
| `src-tauri/src/fork/pill/panel.rs`                                 | Both windows on the main thread: placement, the web view's lifecycle (tauri-nspanel, Tolkovo set-up), content protection, screen-parameter and Space observers                                                                                                             |
| `src-tauri/src/fork/pill/geometry.rs`, `placement.rs`, `levels.rs` | Sizes (→ `src/fork/pill/geometry.gen.json`, a test fails when stale), pure placement over screen rects, 30 fps level frames                                                                                                                                                |
| `src-tauri/src/fork/commands/pill.rs`                              | `fork_pill_snapshot`, `fork_pill_ready(seq)`, `fork_pill_action`                                                                                                                                                                                                           |
| `src/fork/pill/`                                                   | The web view: `Pill.tsx`, `states/` (Listening, Working, Message), `usePill.ts` (state + ready handshake, levels), `pill.css` (dark capsule in both themes, reduced motion)                                                                                                |
| `src/fork/settings/BarSettings.tsx`                                | Always show the bar, position, hide in screen recordings, show again, forget positions (debug panel until S-14)                                                                                                                                                            |

How a take reaches the bar: the hotkey engine's effects
(`fork::shortcut` host `observe`, also the fn-down prewarm), Handy's
`RecordingStateEvent`, the `overlay.rs` hooks (`on_overlay_state` for
transcribing/processing, `on_overlay_hide`), `transcription-complete-event`
(empty → "No speech heard", silent → "Mic looks muted", failed),
`recording-error` (permission, no device) and `fork-insert-outcome`
(card → "Not pasted"). A click on the bar goes to the hotkey engine
(`shortcut::pill_action`), so a bar take is an ordinary hands-free take:
fn stops it, Esc cancels it.

The show handshake: each state change gets a `seq`; the page renders it
and calls `fork_pill_ready(seq)` after two animation frames; only then is
the web panel shown or resized. Until the first paint the native panel
shows a bridge capsule of the active size, so fn-down → bar on screen is
`T_show` + ~10 ms, not the web view's load time.

Clicks: the web panel sets `_setPreventsActivation:` once at creation
(DEC-28 fallback). Without it, on macOS 26.6.2 a click on the web view
made Wispr Bro (a Regular app with a Dock icon, unlike the Accessory RS-03
spike) the frontmost app 5/5, and the paste then went to Wispr Bro. The
native panel never activates the app. `WISPRBRO_PILL_ALLOW_ACTIVATION=1`
leaves the flag off, to re-run that check on a new macOS.

## First speech model, mic gate and offline queue (S-07a, S-07c)

A new install downloads a small speech model at launch (at Welcome), and a
press before it is ready still records. Its WAV and queue row are durable
across relaunch. Late transcription updates History and posts one batch
notification; it never pastes, shows a card or writes the clipboard. Only
Copy latest, History Copy or ⌃⌘V writes it.

| File                                                        | Role                                                                                                                                                                                                                                                                                                   |
| ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `src-tauri/src/fork/models/catalog.fork.json`, `starter.rs` | Which model comes first: the candidates in order (parakeet-110m, whisper-base, Parakeet TDT 0.6B v3) and the languages each is unusable for (whisper-base: nl, RS-09). Pick = first candidate usable for every language the user speaks, else for the first one. Pins come from Handy's `catalog.json` |
| `src-tauri/src/fork/models/readiness.rs`                    | SpeakoFlow `speech_readiness.rs` port (MIT): Ready / UseOther / Downloading / Missing from Handy's model list and `selected_model`                                                                                                                                                                     |
| `src-tauri/src/fork/models/download.rs`                     | Pure download state machine around Handy's downloader: disk check (size + 256 MB), backoff 5/15/30/60/120 s, `Retry-After` (≤ 600 s), captive portal (every 30 s), sha256 against the pin (one extra download on a mismatch), 6 transfers, then "Couldn't download · Retry"                            |
| `src-tauri/src/fork/models/probe.rs`                        | After a failed transfer: one `Range: bytes=0-0` GET of the pinned resolve URL, redirects only to allowed hosts → offline / captive portal / rate limited / server error / reachable                                                                                                                    |
| `src-tauri/src/fork/models/setup.rs`                        | The setup loop and three `actions.rs` hooks; model selection at idle, launch recovery and ordered drain; quit cancels the active transfer and waits for hf-hub's committed resume offset                                                                                                               |
| `src-tauri/src/fork/models/queue.rs`                        | Durable `fork.db` › `queued_takes` (`waiting` → `transcribing` → `ready`/`failed`), idempotent enqueue/bind, relaunch recovery, ordered starts, one batch counter; failed rows rejoin only through explicit History Retry                                                                              |
| `src-tauri/src/fork/commands/models.rs`                     | `fork_speech_setup_status`, `fork_speech_setup_retry`, `fork_speech_setup_refresh`                                                                                                                                                                                                                     |
| `src/fork/pill/states/SettingUp.tsx`, `Card.tsx`            | "Setting up… 40%" (ring, ✕) and the card (two lines, Copy, ✕); setup problems are bar messages with Retry                                                                                                                                                                                              |
| `src/fork/history/WaitingRow.tsx`                           | "Waiting for speech model" in the saved row; Play/Delete stay available, Copy/Flag/Retry wait                                                                                                                                                                                                          |
| `src/fork/onboarding/OnboardingFlow.tsx`                    | S-29a replaced Handy's flow (and S-07a's `firstRun.ts` hook inside it): no screen picks, names or waits for a model; see "Onboarding (S-29a)"                                                                                                                                                          |

A queued take, end to end: the mic gate lets the take record; at stop
`transcribe_take` durably queues the WAV name and returns an empty
transcription. Handy then saves the WAV and an empty History row
(`entry_meta::on_saved` binds it), and the bar shows "Setting up". Launch
recovery resets interrupted jobs, rebinds rows and, if necessary, creates a
History row from a surviving queued WAV before any transcription starts.
Once a model is selected, rows enter Handy's `TranscriptionJobManager` in
recording order. Completion sets `paste_outcome = queued` and posts one
"N dictations ready" inbox/macOS notification for the batch. Copy latest
uses S-18's explicit clipboard action; Open goes to History. A failed job
keeps its audio and History row with Retry.

Test identities: `fork::is_product_build()` is also true for
`com.wisprbro.<slice>check` identifiers (`is_test_identity`), so a real-Mac
run gets the product (hotkeys, bar, product settings) with its own app
data, WebKit storage and TCC rows; `early_init` puts `HF_HOME` and crash
files under the build's own identifier.

## Microphone and sounds (S-08)

Which mic a take uses, what happens when it goes away, the level meter,
the sounds and "Mute all audio while dictating". Everything is in
`fork/audio/`; Handy is touched only in `managers/audio.rs` (the existing
`recorder_config` hook, plus the two `osascript` bodies replaced by calls).

| File                                                          | Role                                                                                                                                                                                                                                                                 |
| ------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/fork/audio/transport.rs`                       | HAL snapshot (device UID, name, transport, has input; default input), never opens a device; `Transport` and `is_wireless` (Bluetooth/LE, iPhone over the air, AirPlay); cpal ID ↔ UID (`coreaudio:<uid>`). Readers ported from handy-recorder `headset_macos.rs`    |
| `src-tauri/src/fork/audio/choice.rs`                          | Pure rules: the wireless-default rule, which device a take opens, the list order (built-in "(recommended)", Auto-detect, wired, wireless)                                                                                                                            |
| `src-tauri/src/fork/audio/mod.rs`                             | `recorder_config` hook: prepared mic (S-01) + the wireless rule for `device: None`; records the opened device (UID, name) for the bar label and the watch; `on_ready`                                                                                                |
| `src-tauri/src/fork/audio/watch.rs`                           | Thread `wb-mic-watch`: HAL poll 1 Hz at rest, 5 Hz in a take; `ForkMicDevicesChanged`; take mic gone → bar warning `mic_lost` (audio before it is kept by Handy); at rest re-prepares a stale prepared mic (never in a take, not within 2 s of a press, ≤ 1 per 3 s) |
| `src-tauri/src/fork/audio/monitor.rs`                         | Mic-test level meter: its own handy-recorder, `ForkMicLevel` at 20 Hz (RMS on a −60..0 dBFS scale, fast attack, slow release), ends on stop, 60 s, or a take start. Not a take: no Handy manager, history or bar                                                     |
| `src-tauri/src/fork/audio/mute.rs` (AGPL, Tambourine)         | Mute sessions on CoreAudio: device mute flag, else volume scalars to 0 with exact restore; restored on the device that was muted; user's own mute left alone; `set_mute`/`get_mute` hooks; restore at `RunEvent::Exit`                                               |
| `src-tauri/src/fork/audio/permission.rs`                      | DEC-55: at `ExitRequested`/`Exit`, a take still recording is stopped and saved as `<data>/recovered/wispr-bro-recovered-<ms>.wav`; at the next launch each is handed to Handy's file transcription (History) and removed; inbox entry `take_recovered`               |
| `src-tauri/src/fork/commands/audio.rs`                        | `fork_mic_list`, `fork_mic_choose` (Handy `selected_microphone` + fork `mic_follow_system`, then re-prepare), `fork_mic_monitor_start` / `_stop`                                                                                                                     |
| `src/fork/settings/MicrophonePicker.tsx`, `SoundSettings.tsx` | Settings › Sound on the debug panel until S-14: radio list with the Bluetooth caution and the "skipped headset" note, `MicLevelMeter`; sounds on/off, theme, volume, Play, mute while dictating (Handy's settings through Handy's store)                             |

The choice is two settings: Handy's `selected_microphone` (a name, or
`None` for the system default) and the fork's `mic_follow_system`. `None`
with the flag off means "never chosen": the system default, except that a
wireless default is replaced by the built-in mic (if the Mac has one).
"Auto-detect (system setting)" sets the flag and follows the default even
when it is Bluetooth. The sounds are Handy's own Marimba and Pop files
(MIT, Handy's repo), on by default through F-07's `FIRST_RUN_DEFAULTS`; no
new sound file was added.

## Onboarding (S-29a)

The fork's first-run flow replaces Handy's: `src/App.tsx` imports
`OnboardingFlow` from `@/fork/onboarding/` instead of
`./components/onboarding/` (one changed line, `ci/hooks.txt`). Handy's
onboarding files are untouched; S-07a/b's hook in Handy's
`OnboardingFlow.tsx` and `src/fork/onboarding/firstRun.{ts,css}` were
removed, because that flow is no longer mounted.

| File                                                                                              | What                                                                                                                                                                                                                                                                                                                              |
| ------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/fork/onboarding/state.rs`                                                          | Step ids and order (`ORDER`), resume (`resume_step`), `go_to` (Back drops later completion; a finished run starts over and clears the S-30 checklist), `complete`. The state lives in fork settings › `onboarding` (RE-16). A test checks `steps.ts` lists the same ids                                                           |
| `src-tauri/src/fork/onboarding/native.rs`                                                         | Move to Applications: `rename` into `/Applications`, else `~/Applications` (no write access); `ditto` copy for another volume, a disk image or a translocated launch, quarantine flag cleared on the copy; never overwrites; then `open -n` the new copy and quit                                                                 |
| `src-tauri/src/fork/shortcut/globe.rs`                                                            | DEC-44 (OpenWhispr port, MIT): Globe → Do Nothing through `TISUpdateFnUsageType` after the user's consent; the user's value goes to `globe-preference.json` in the data folder first; `reconcile` restores it at launch and on every binding change once fn is no longer the trigger, unless the user picked another action since |
| `src-tauri/src/fork/shortcut/mod.rs`                                                              | `retry_tap` (the tap could not start before Accessibility; the shortcut test starts it, no relaunch), `set_key_test` (2-minute self-expiring test mode: trigger presses emit `fork-hotkey-event` `key_test` down/up and start no take)                                                                                            |
| `src-tauri/src/fork/commands/onboarding.rs`                                                       | `fork_onboarding_go_to`, `_complete`, `_app_location`, `_move_to_applications`, `_key_test`, `_globe_do_nothing`, `_open_pane` (Accessibility / Keyboard settings, local URL scheme only)                                                                                                                                         |
| `src/fork/onboarding/steps.ts`                                                                    | `welcome`, `move_to_applications`, `languages` (stage Welcome), `permissions`, `mic_test`, `shortcut_test` (Setup); Back targets. S-29b appends Practice                                                                                                                                                                          |
| `src/fork/onboarding/{Welcome,MoveToApplications,Languages,Permissions,MicTest,ShortcutTest}.tsx` | The screens; Languages reuses Handy's `LanguagePicker` and saves `spoken_languages`; Permissions reuses Handy's `usePermissions`; MicTest reuses S-08's meter and list; ShortcutTest reuses the S-14 hotkeys panel (`HotkeysDialog.tsx`)                                                                                          |
| `src/fork/onboarding/{Stepper,Help,PermissionArt}.tsx`, `art/*.svg`, `art.css`                    | Stepper with Back/Help; permission pictures inlined so they follow the app theme (macOS 13-15 and macOS 26 sets)                                                                                                                                                                                                                  |

Rules: every step change is saved before the screen changes (a relaunch
resumes there; the log line `onboarding: step <id>` carries only the id);
no screen reads the speech-model download (it starts from Welcome, S-07a
queues earlier takes); the shortcut test ends when its screen unmounts and
expires on its own; "No" picks the likely cause from the engine
(`running`, Handy's Secure Input check, Globe setting) and links the others.
Preview (debug) is inert. Tests: `cargo test --lib onboarding`,
`fork::shortcut`; Playwright `tests/fork/onboarding.spec.ts`.

## Silent model policy, upgrades and cleanup (S-07b)

The user never picks a speech model. After the first model (S-07a) is
ready, the fork works out the best model for this Mac and the user's
languages, downloads it in the background, swaps it in at a quiet moment
once it has loaded, and deletes the model it replaced. Nothing of this
reaches the bar; Settings › Advanced › Speech models (on the debug panel
until S-14) is the only place a model is named.

| File                                               | Role                                                                                                                                                                                                                                                                                                                                           |
| -------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/fork/models/policy.fork.json`       | The data: RS-09's short-take p50/p95 on the M5 Max (CPU numbers where measured), peak footprints, per-language rankings with and without CC-BY models (DEC-26; `*` for languages without a row), machine-class factors, RAM-tier footprint caps, the 5 GB disk reserve, §5.1 budgets (Apple Silicon and Intel) and the policy's `retired` list |
| `src-tauri/src/fork/models/policy.rs`              | `decide`: a pure function from languages, `Machine`, measured factor, on-disk models and the DEC-26 switch to the pick (rules in the module docs); ≥ 60 vectors in `policy_tests.rs`                                                                                                                                                           |
| `src-tauri/src/fork/models/machine.rs`             | sysinfo 0.39 (`system`, `disk`): chip from the brand string (M1-M5 × base/Pro/Max/Ultra, Intel), RAM, free space on the model volume (macOS `AvailableCapacityForImportantUsage`, purgeable-aware)                                                                                                                                             |
| `src-tauri/src/fork/models/speed.rs`               | `fork.db` › `asr_speed`: key-up → text of warm 2-20 s takes (lengths and times only); the median of the newest 5 over RS-09's p50 is the measured factor, used when slower than the class                                                                                                                                                      |
| `src-tauri/src/fork/models/upgrade.rs`             | `step` (runs in the S-07a setup loop on every kick): who chose the selection (a different selection while the fork's model is still on disk pins it), the decision, background download (S-07a's state machine in `Mode::Background`: no bar, no Retry wait), swap at a quiet moment, cleanup; `ForkModelPolicy` (`fork-model-policy`)         |
| `src-tauri/src/fork/models/cleanup.rs`             | `fork.db` › `speech_models` (origin `starter`/`policy`/`selection`, `loaded_at`, `superseded_at`, `deleted_at`) and `model_policy_state`; `to_delete`: superseded or retired, not selected, only after the selection loaded, never a model the user downloaded                                                                                 |
| `src-tauri/src/fork/db/migrations/0007_models.sql` | The three tables above                                                                                                                                                                                                                                                                                                                         |
| `src-tauri/src/fork/commands/models.rs`            | `fork_model_policy_status`, `fork_model_policy_set_automatic`, `fork_models_hidden`                                                                                                                                                                                                                                                            |
| `src-tauri/src/fork/tray.rs`                       | Product builds: Handy's Model ▸ submenu and Unload are gone; since S-26a the existing `extend` hook replaces the whole tray with `fork::app_menu::fill_tray` (no new `tray.rs` line)                                                                                                                                                           |
| `src/fork/settings/advanced/Models.tsx`            | Speech models panel: model in use, background phase and progress, "slower on this Mac", this Mac and the languages, "Choose the model automatically", and Handy's Models page folded under "All models" (wrapped, retired and restricted models pruned from Handy's model store)                                                               |

**The rule** (`policy::decide`): candidates hear every language the user
speaks (Handy's `spoken_languages`; else the macOS preferred languages plus
English, DEC-02) and are ranked by their worst rank over those languages;
the first whose expected p50/p95 (RS-09 × factor) meets §5.1, whose
footprint fits the RAM tier and whose file fits free disk − 5 GB is the
pick; else the fastest that fits, marked "slower on this Mac" (one inbox
entry, `NotificationKind::SlowerMac`). Intel (DEC-29, not built in M1)
uses the CPU numbers and the Intel budget. With RS-09's table: English →
parakeet-unified-en on every Apple Silicon class; nl/fr → turbo on M3-M5
Max and the Ultras, Parakeet v3 elsewhere; ja/zh → turbo or
whisper-small; en + nl → turbo or Parakeet v3; without CC-BY the Whisper
ladder (turbo, small, base).

**Triggers** (`setup::Kick`): launch, the first model ready, a language
change (Handy's `change_spoken_languages_setting` command is observed in
`fork::compose_invoke`, no Handy hook; onboarding's
`fork_speech_setup_refresh`), five more timed takes, a Handy download or
selection change, "Choose automatically". A catalog update arrives with an
app update and is picked up at launch.

**Swap and cleanup.** The background file is checked against its pin like
the first model. It is selected (`switch_active_model`, which loads it)
only when nothing records or transcribes and no take ended in the last
10 s; a load failure leaves the old selection and the pick is skipped for
the session. Then the old model is marked superseded and every
superseded `starter`/`policy` model, plus a retired selection the fork
migrated away from and Handy's retired ONNX files, is deleted. Handy's
`model-download-complete` notification ("Dictation is ready") is posted
only for the first model.

## i18n and the untranslated marking scheme

- English source: `src/i18n/locales/en/fork.json`. All 26 locales have a
  `fork.json` with the same keys.
- `bun run wb:translate` (`scripts/wb-translate-missing.ts`, W-25) fills keys
  missing from non-English files using a pluggable `TranslationBackend`
  (today `copy-english`; S-10 adds the local `llama-server` one), keeps
  `{{placeholders}}` identical (a backend result with different placeholders is
  replaced by English) and drops keys removed from English.
- **Flags live in a sidecar** `locales/<lng>/fork.flags.json`:
  `{ "<dotted.key>": "untranslated" | "machine" }`. `fork.json` stays a plain
  i18next resource with exactly the English shape, so i18next, Handy's
  `check-translations.ts` (reads only `translation.json`) and ESLint are
  unaffected, and the sidecar is never bundled. A reviewed string has no flag.
- `bun run wb:check:translations` exits 1 on missing or extra keys or a
  placeholder mismatch. Flags pass until S-37 (R5 §4.12).

## `fork.db` (F-06)

`src-tauri/src/fork/db/` (`mod.rs`, `fts.rs`, `migrations/0001_init.sql`,
`tests.rs`). Schema: every M1 table of R5 §4.3; M2 tables arrive with N-04.

- The plugin `setup` calls `db::register(app)`, which only manages a
  `ForkDb` handle for `<app data dir>/fork.db`. The file is opened lazily by
  the first `ForkDb::with_conn`, so `setup` does no database I/O.
- One connection per process behind a mutex; pragmas `journal_mode=WAL`,
  `busy_timeout=5000`, `synchronous=NORMAL`, `foreign_keys=ON`.
- Migrations: `rusqlite_migration` on `fork.db`'s own `user_version`. Append
  to `MIGRATIONS` in `db/mod.rs`; never edit a shipped file. Before migrating
  an existing database (version ≥ 1) the WAL is checkpointed and the file is
  copied to `fork.backup-v<N>-<ts>.db` (newest 3 kept). A newer
  `user_version` than the build knows returns `DbError::SchemaTooNew` and the
  file is left untouched.
- Corruption: a file without the SQLite header, or with a failing open or
  `PRAGMA quick_check`, is renamed with its `-wal`/`-shm` to
  `fork.corrupt-<ts>.db*` and recreated; `db::report_db_recovered` logs a
  warning (S-18 turns it into an inbox entry).
- `ForkDb::delete_all` closes the connection and removes `fork.db`, `-wal`,
  `-shm`, plus the backup and corrupt copies (they hold the same data).
- Writes keyed by a Handy history row use `db::upsert_entry_meta` (UPDATE,
  then INSERT if absent; column names checked against the table).
- Search: `history_fts` and `notes_fts` are contentless FTS5 `trigram` tables
  (`contentless_delete=1`) whose rowid is the content row's key, maintained
  by the app via `db::fts`. Queries under 3 characters use `LIKE` on the
  content table.
- `history.db` is never opened by this module.
- The handle lives next to Handy's `history.db`: the portable `Data/` dir in
  portable mode (S-09 fix; F-06 used `app_data_dir()` even then).

## History data model (S-09)

`fork/db/{entry_meta,stats,retention,reconcile,disk,handy,jobs}.rs`,
`fork/db/migrations/0002_history.sql`, `fork/stats/{wordcount,streak,wpm}.rs`,
`fork/context/{frontmost,categories,take}.rs` + `categories.json`,
`fork/commands/history.rs`.

**Hooks** (budgets in `ci/hooks.txt`):

| Upstream file                                | Hook                                                                   |
| -------------------------------------------- | ---------------------------------------------------------------------- |
| `managers/history.rs` `insert_entry`         | `crate::fork::db::on_saved(&self.app_handle, &entry);`                 |
| `managers/history.rs` `update_transcription` | `crate::fork::db::on_retried(&self.app_handle, &entry);`               |
| `managers/history.rs` `delete_entry`         | `crate::fork::db::on_deleted(&self.app_handle, id);`                   |
| `actions.rs` `TranscribeAction::start`       | `crate::fork::on_record_start(app);` (frontmost app, low-disk watch)   |
| `actions.rs` stop task, WAV name             | `crate::fork::take_file_name(sample_count)` (ms names; binds the take) |
| `actions.rs` history save                    | `if wav_saved \|\| crate::fork::keep_take(&wav_path_for_verify) {`     |
| `managers/transcription.rs` `finish_text`    | `crate::fork::text::process(…)` (S-11; verbatim ASR text, see below)   |

**How a take finds its row.** `fork::context::take::Ledger` (process-wide,
bounded to 16 per queue): record start sets the _current_ take (time, local
day, frontmost app and its taxonomy); `take_file_name` binds it to the WAV
name with its speech time (samples / 16 kHz, VAD-trimmed); the text
pipeline notes (kept text → verbatim text + pipeline record, S-11);
`on_saved` looks both up by the entry's
`file_name` and `transcription_text`, writes the row, and registers
(entry id, final text) for `fork::insert`'s outcome, which
`on_paste_outcome` matches by text (trailing space ignored). Rows the fork
never saw start (retries of unknown rows, crashes) get Handy's save time and
app `other`.

**Writes** (`entry_meta::write`): one transaction per entry with its search
document and its stats change. The first write sets time, day, app,
taxonomy, source and speech time; later writes (a retry, a second save)
change only the texts, counts and `previous_text`, and move the stats by the
delta. `counted = 1` while the row's words are in `daily_stats`; only
non-empty `dictation` rows count. Deletes never subtract.

**Jobs** (`db::jobs`, thread `fork-history-jobs`, started in `on_ready`):
launch reconcile (purge expired soft deletes, backfill Handy rows without
`entry_meta`, drop orphans), then retention at launch, every 6 h and on a
retention setting change. Handy's rows are read and deleted only through
`HistoryManager` (`db::handy::HandyHistory`, driven with `block_on` on the
fork's own threads); a WAV past the audio limit is a plain file delete.
Debug builds honour `WB_DEBUG_RETENTION_CLOCK_OFFSET_DAYS` (Mac test S-09.2).

**Soft delete:** `fork_history_delete` sets `deleted_at` (search and History
hide the row), `fork_history_undo_delete` clears it, and a purge thread
deletes through Handy after 10 s; the launch reconcile finishes purges a quit
interrupted.

**Full disk:** during a recording `db::disk` polls free space every second;
below 50 MB it stops the recording through `fork::insert::on_disk_full`
(`Interruption::DiskFull`, so the take goes to the card with
`CardReason::DiskFull`). If the WAV still fails to save, `keep_take` keeps
the history row with text only (`audio_deleted_at` set).

**App taxonomy** (`context/categories.json`): exact bundle id, then
OpenWhispr's whole-token signatures on name + bundle id (email, work,
personal), then browsers (`other` until S-19b's host rules), else `other`.

## Fork settings (F-07)

`src-tauri/src/fork/settings.rs` (+ `settings/tests.rs`), commands in
`fork/commands/settings.rs`, frontend `src/fork/settings/{store.ts,useForkSettings.ts,ForkSettingsDebug.tsx}`.

- **Store.** `ForkSettings` (serde container default, specta types) lives in
  `fork-settings.json` under key `"settings"` via tauri-plugin-store
  (`portable::store_path`, so portable mode works). The plugin `setup` only
  manages `ForkSettingsHandle` (the store plugin is registered after the fork
  plugin, `lib.rs:909`); the file is opened on first use.
- **Corrupt file.** tauri-plugin-store silently starts empty on a file it
  cannot parse and would overwrite it, so the fork reads the raw file first:
  unparseable JSON, a field that fails to deserialize or fails validation
  (field-by-field salvage like Handy's `salvage_settings`), or a newer
  `schema_version` → copy to `fork-settings.<corrupt|newer-vN>-<ts>.json`, keep
  the readable fields, defaults for the rest, `warn!` naming the reset fields.
- **Migrations.** `schema_version` (current 1) + `MIGRATIONS` on the raw JSON
  object; append only. v0 (no version) → v1 drops unknown keys.
- **Commands.** `fork_get_settings`; `fork_set_setting(update)` where
  `update` is the adjacently tagged `ForkSettingUpdate`
  (`{ key: "cleanup_level", value: "medium" }`, one variant per settable
  field; `schema_version` and `applied_first_run_defaults` are not settable).
  Wrong types fail at IPC deserialization; ranges/ids/limits (≤ 4 bindings per
  action, WPM 1–300, …) return `ForkError::InvalidSetting`. Success saves,
  returns the settings and emits `ForkSettingsChanged` (`fork-settings-changed`,
  full settings). Setting `update_checks_enabled` mirrors into Handy at once.
- **`apply_product_settings`** (pure, idempotent) runs from `fork::on_ready`
  every start. `ENFORCED`: `recording_retention_period = never` (Handy deletes
  nothing; `history_limit` is unused then and left alone), `post_process_enabled
= false` (this is what registers Handy's second hotkey), `experimental_enabled
= false`, provider not cloud (custom provider's `base_url` reset to
  `http://localhost:11434/v1` if not loopback; a cloud selection becomes `local`
  if that provider exists, else `custom`; `local`, Apple Intelligence and a
  loopback `custom` stay), `update_checks_enabled` mirrored from the fork, and
  `overlay_style = none` **gated by `HANDY_OVERLAY_REPLACED` (false until S-03
  replaces the overlay; S-03 flips it)**. `FIRST_RUN_DEFAULTS` (once each, keys
  recorded in `applied_first_run_defaults`): `reliable_paste` (Handy's
  concealed-type paste), `audio_feedback`, `log_level = info`, `paste_method =
ctrl_v`, `keyboard_implementation = handy_keys`, `theme = system`,
  `autostart_enabled` (release builds only, so a dev run never registers
  `target/debug` as a login item).
- **Writes to Handy** go through `ForkRuntime::{handy_settings,
write_handy_settings}` (opaque `HandySettings` newtype so `AppSettings` stays
  crate-private): Wry uses Handy's `get_settings`/`write_settings`, then the
  side effects Handy's commands would run (unregister the post-process hotkey,
  `FILE_LOG_LEVEL` unless `--debug`, `apply_autostart`, `apply_window_theme`,
  `change_keyboard_implementation_setting`) and emits Handy's
  `settings-changed`. Tests use `settings::FakeHandyStore` on `MockRuntime`.
- **Identity guard.** The product step runs only when the identifier is
  `com.wisprbro.desktop`; a build without the overlay (`com.pais.handy`) never
  rewrites a real Handy install's settings.
- **Self-check.** The script also calls `fork_get_settings`, an unchanged
  `fork_set_setting` and an invalid one (must be `InvalidSetting`); debug builds
  log the effective fork settings and `enforced_snapshot` read back from Handy.
- **Frontend.** `useForkSettingsStore` (Zustand) + `useForkSettings()`
  (load once, `setSetting(key, value)` typed per key, follows
  `ForkSettingsChanged`). Mounted as the debug panel's "Fork settings"
  section until S-14.

## Privacy cleanups (F-08)

The R5 §1 promise before any feature work: no cloud providers, Info logs with
no settings dump, nothing contacts `blob.handy.computer`, a strict CSP.

### Upstream hooks (F-08)

| File                                                                | Hook                                                                                                                                                                                                                                                                                                                                          |
| ------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ⚠ `src-tauri/src/settings.rs` `default_post_process_providers`     | first statement `return crate::fork::llm::default_providers();` + `#[allow(unreachable_code)]` on the fn. Handy's cloud list stays below as dead code, so upstream merges of that list do not conflict                                                                                                                                        |
| ⚠ `src-tauri/src/settings.rs` `default_log_level`                  | body `crate::fork::diagnostics::default_log_level()` (Info)                                                                                                                                                                                                                                                                                   |
| ⚠ `src-tauri/src/settings.rs` `load_or_create_app_settings`        | `debug!("Loaded settings: {:?}", settings);` → `crate::fork::diagnostics::settings_summary(&settings);`                                                                                                                                                                                                                                       |
| `src-tauri/src/llm_client.rs` `create_client`                       | `reqwest::Client::builder().default_headers(headers)` (2 lines) → `crate::fork::llm::client_builder(provider, headers)?` (1 line). Every Handy LLM request (`send_chat_completion*`, `send_anthropic_message`, `fetch_models`) builds its client here                                                                                         |
| `src-tauri/src/catalog/mod.rs` `mirror_files`                       | `.filter_map(\|base\| crate::fork::net::mirror_base(base))` after `MIRRORS.iter()`: the only builder of mirror URLs                                                                                                                                                                                                                           |
| `src-tauri/src/managers/model.rs` `download_model`                  | `ModelSource::Url { url, sha256 } => (crate::fork::net::checked_download_url(url)?, sha256.clone())` (3 lines after rustfmt): Handy's five legacy URL models point at the mirror                                                                                                                                                              |
| Handy tests                                                         | `#[ignore = "fork: …"]` on `every_offered_model_has_mirror_fallbacks_with_hashes` (catalog), `test_every_retired_model_has_a_downloadable_replacement` (model.rs; it asserts a mirror) and `post_process_defaults_sync_builtin_api_format_but_keep_custom` (settings.rs; needs the Anthropic built-in). They assert what DEC-04/DEC-16 remove |
| ⚠ `src-tauri/Cargo.toml`, `Cargo.lock`                             | removed `rdev` (no `rdev::`/`use rdev` anywhere; Handy's keyboard backends are `handy-keys` and the global-shortcut plugin)                                                                                                                                                                                                                   |
| ⚠ `package.json`, `bun.lock`, `.nix/bun.nix`, `.nix/bun-lock-hash` | removed `@tauri-apps/plugin-sql` (no import in `src/`, no `tauri-plugin-sql` crate, no `sql:` capability; `history.rs` only migrates its old `_sqlx_migrations` table). The `.nix` files are regenerated by Handy's `postinstall` (`check-nix-deps.ts`)                                                                                       |
| `src-tauri/tauri.fork.conf.json`                                    | `app.security` (CSP, below); fork file, not upstream                                                                                                                                                                                                                                                                                          |

`main.rs` is not edited again: `fork::early_init()` (F-05) does the new work.
`src-tauri/src/fork/` files: `net.rs`, `llm/{mod,providers,tests}.rs`,
`diagnostics/{mod,sanitizer,crash}.rs`, `models/{mod,license_filter}.rs`,
`commands/csp.rs`. Frontend: `src/fork/debug/{CspSmoke.tsx,cspSmokeChecks.ts,cspViolations.ts}`.

### Network allowlist (`fork/net.rs`)

- `is_allowed_host(url)`: `https`, no credentials, host in `github.com`,
  `objects.githubusercontent.com`, `release-assets.githubusercontent.com`,
  `huggingface.co`, or a subdomain of `hf.co` / `huggingface.co`.
- `is_loopback(url)`: `http(s)`, no credentials, `localhost` or a loopback IP.
  Two parsers must agree (a literal split of the authority and the WHATWG
  parser reqwest connects with), so `http://0x7f000001/`, `user@host`,
  backslashes or whitespace read as "not loopback". `fork::settings::is_loopback_url`
  now delegates here.
- `mirror_base(base)` returns `None` for any mirror not on the allowlist, i.e.
  `https://blob.handy.computer`; `checked_download_url(url)` errors before any
  request for a non-allowed URL. A Hugging Face failure is now reported
  instead of retried on the mirror.

### Providers and the backend refusal (DEC-04)

- `fork::llm::default_providers()`: `local` (placeholder until S-10:
  `http://127.0.0.1/v1`, structured output on, no models endpoint, label
  "Local"), Apple Intelligence (macOS arm64, on device, same entry as Handy's)
  and `custom` (Ollama default, loopback only). No cloud provider.
- `fork::llm::client_builder` (the `llm_client.rs` hook) refuses before any
  network call: the `local` placeholder ("not available in this build yet")
  and any non-loopback base URL ("Request refused …"). Handy's callers turn the
  error into "fall back to the original transcript", so selecting `local`
  cannot crash or send anything. It also sends neutral headers (drops
  `Referer`, `User-Agent`, `X-Title`, `HTTP-Referer`; reqwest adds no default
  user agent), disables proxies (`no_proxy`) and follows redirects only to
  other loopback URLs (max 5), so a local server cannot bounce text to a
  remote host. The header removal is generic and could go upstream as an
  opt-out, but it lives in the fork so the hook stays one line.
- `apply_product_settings` (F-07 `ENFORCED`) gained `post_process_providers`,
  run before `post_process_provider_id`: Handy's stored list becomes exactly
  the built-in list (old cloud entries removed with their API-key and model
  slots; `local`/Apple re-synced; the user's `custom` keeps its base URL and
  API format). A cloud selection now always becomes `local` (it is always in
  the list). Handy's `ensure_post_process_defaults` only adds missing
  built-ins and never prunes, hence the rule. Product identity only, like the
  rest of F-07.
- Handy's `default_post_process_provider_id()` still returns `"openai"`; on a
  fresh install `apply_product_settings` maps it to `local` at the first
  `on_ready`. In a plain Handy build the id matches no provider, so
  post-processing is skipped.

### Logs and diagnostics (`fork/diagnostics/`)

- Default file log level Info (`default_log_level`). Handy's `FILE_LOG_LEVEL`
  atomic (`lib.rs:59`) still starts at Debug until the settings are read in
  Handy's setup, so the first Tauri runtime line can be `DEBUG` (not hooked;
  no user data in it).
- `settings_summary` logs one Info line `Loaded settings (summary): {…}`:
  enums, booleans, ids the app ships (model, provider, language codes) and
  counts. Custom words, prompts, filler words, device names, external script
  path, API keys and endpoint URLs appear only as counts or flags.
- `sanitizer.rs` (`AGPL-3.0-only`): port of Voquill
  `apps/desktop/src-tauri/src/utils/log_sanitizer.rs` at
  `a5dfbe0e3a35807271293a0ef02c91902abc0104` (voquill/voquill), its 14 tests
  kept, plus `$HOME` → `~`, e-mails, bearer tokens, API keys, `transcript:` /
  `prompt:` / `custom words:` lines and user-text JSON fields. Listed in
  `docs/fork/PORTS.md` and `NOTICE` §3/§4 (F-04).
- `crash.rs`: `fork::early_init` installs a panic hook chained in front of the
  previous one. It writes `crash-<YYYYMMDD-HHMMSS.mmm>-<pid>.txt` (version,
  build, OS/arch, time, thread, location, message, backtrace; sanitized) to the
  log dir, keeps the newest 10, never panics, then calls the previous hook.
  The dir is `~/Library/Logs/com.wisprbro.desktop` (or portable `Data/logs`)
  from `early_init`, replaced in the plugin `setup` by Tauri's resolved log
  dir. Nothing is uploaded; S-27 reads the files (and picks up macOS `.ips`
  reports, see "Data and Privacy (S-27)").

### `early_init` and identity

`early_init` runs before Tauri, so it cannot read `app.config().identifier`.
`fork::compiled_identifier()` rebuilds the identifier from the two env vars
tauri-build emits for every target from the merged config (including
`TAURI_CONFIG` from `--config`): `TAURI_ANDROID_PACKAGE_NAME_PREFIX`
(`com_wisprbro`) and `TAURI_ANDROID_PACKAGE_NAME_APP_NAME` (`desktop`)
(`tauri-build-2.6.3/src/lib.rs:493-508`). Only when it is
`com.wisprbro.desktop` does `early_init`:

1. call `portable::init()` first (idempotent `OnceLock`; `run()` calls it
   again), so portable mode keeps its own `HF_HOME` and records the user's
   previous one,
2. set the crash dir and install the panic hook,
3. set `HF_HOME` (DEC-39) to `~/Library/Application Support/com.wisprbro.desktop/huggingface`
   unless portable (same `huggingface` sub-folder portable mode uses). macOS
   only; Windows/Linux layout is S-07a. hf-hub reads `HF_HOME` through
   `Cache::from_env()`, so downloads, `is_downloaded` and delete use it.
   `on_ready` logs `fork: HF_HOME under the app data dir: true|false` (no path).

Plain Handy builds (`cargo build`, `cargo test`, `bun run tauri dev`) skip all
three. A user-set `HF_HOME` is overridden under our identity (product rule).

### CSP (DEC-43)

`tauri.fork.conf.json` carries the R5 §4.11 block exactly: `csp`, `devCsp`
(`csp` plus `http://localhost:1420` in `script-src`, `style-src`, `img-src`,
`connect-src` and `ws://localhost:1420` in `connect-src`) and
`assetProtocol { enable: true, scope.allow: ["$APPDATA/**"] }` (merge patch:
Handy's `requireLiteralLeadingDot: false` stays). Recordings live in
`$APPDATA/recordings`, inside the scope. Portable mode's data dir is not added
to the scope yet (E-88 runtime scope, later slice).

- **Tauri applies the CSP only to pages it serves.** In tauri 2.11.5 a desktop
  `tauri dev` loads the Vite dev server directly (`protocol/tauri.rs`: only
  mobile proxies it), so `devCsp` is inert on macOS. It would also block Vite
  dev's inline React-refresh preamble if it were applied. Release builds embed
  `dist/`, hash Tauri's own inline scripts and send the CSP header.
- **Smoke check** (`src/fork/debug/cspSmokeChecks.ts`): plays a local WAV
  through `convertFileSrc(…, "asset")` (`fork_csp_smoke_media`: newest
  recording, else a 0.1 s silent fixture `fork-csp-smoke.wav` written to the
  app data dir; debug builds, `--debug` or self-check only), loads a canvas as
  a `data:` and a `blob:` image, calls `fork_ping`, and checks that a
  loopback probe image (`http://127.0.0.1:9/…`, not in `img-src`) is blocked
  by a `securitypolicyviolation`. A recorder installed when the fork frontend
  starts (`cspViolations.ts`) catches violations from page load too.
  Mounted as the debug panel's "Content Security Policy" section
  (`CspSmoke.tsx`) and exposed as `window.__wbCspSmoke` for the self-check.
- **Self-check**: the `WISPRBRO_SELFCHECK` script runs `__wbCspSmoke()` and
  logs `fork selfcheck: CSP smoke: {…}`. Release builds must report the CSP
  enforced (probe blocked, no other violation); debug builds accept an
  unenforced CSP when every allowed source loads.

### Model licence filter (`fork/models/license_filter.rs`, DEC-27)

Hidden from the fork's catalog view and never default: the 12
`moonshine-{tiny,base}-{ar,ja,ko,uk,vi,zh}` models (non-commercial Moonshine
Community License; catalog says `mit`) and any `cc-by-nc*` model
(`canary-1b`, `granite-speech-5.0-470m-turboctc-nc`). Never default (and also
not shown): `other`-licensed models and the NVIDIA Open Model License ones
(Sortformer, multitalker-parakeet). Licences come from the bundled
`catalog/catalog.json` (`include_str!`). `catalog_view(models)` keeps a model
that is already downloaded. **Not hooked into Handy's model picker**: no hook
is listed for `commands/models.rs`, the fork never mounts that UI in M1, and
S-07a/S-07b (silent model policy) and S-14 (Advanced list) call
`catalog_view` / `may_be_default` on `ModelManager::get_available_models()`.

### F-08 tests

| What                                                                          | Test                                                                                                                                                                                     |
| ----------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Settings defaults/read have no cloud provider                                 | `fork::llm::tests::{default_list_has_no_cloud_provider, handy_settings_defaults_come_from_the_fork_list}`                                                                                |
| Stored `openai` (and other cloud entries, keys, models) migrated to `local`   | `fork::settings::tests::a_stored_openai_provider_is_migrated_to_local`, `the_custom_provider_keeps_the_users_loopback_url_and_format`                                                    |
| Non-loopback URL refused before any request (listener sees no connection)     | `fork::llm::tests::non_loopback_custom_url_is_refused_before_any_request`, `refusal_covers_cloud_and_disguised_hosts_and_a_stopped_local_llm`                                            |
| `local` sends nothing, does not panic                                         | `fork::llm::tests::selecting_local_sends_nothing_while_the_sidecar_is_stopped`                                                                                                           |
| Neutral headers on the wire                                                   | `fork::llm::tests::loopback_requests_carry_neutral_headers`, `neutral_headers_drop_app_identifying_headers_only`                                                                         |
| Startup summary has no custom words, prompts, devices, keys; Info default     | `fork::diagnostics::tests::*`                                                                                                                                                            |
| No download path builds a `blob.handy.computer` URL (`inert-hosts.txt` proof) | `fork::models::tests::downloader_never_builds_a_blob_handy_computer_url`, `fork::net::tests::the_handy_mirror_is_never_a_mirror_base`                                                    |
| Sanitizer vectors (14 Voquill + 4 fork)                                       | `fork::diagnostics::sanitizer::tests::*`                                                                                                                                                 |
| Panic hook writes a sanitized file and chains; pruning                        | `fork::diagnostics::crash::tests::*`                                                                                                                                                     |
| Licence filter                                                                | `fork::models::license_filter::tests::*`                                                                                                                                                 |
| Allowlist / loopback parsing                                                  | `fork::net::tests::*`                                                                                                                                                                    |
| CSP block exact; compiled identity; `HF_HOME` gating                          | `fork::identity_tests::{effective_config_has_the_r5_csp, compiled_identifier_reads_the_build_identity}`, `fork::models::tests::hf_home_only_for_our_identity_and_never_in_portable_mode` |
| CSP smoke panel (Playwright, devCsp header)                                   | `tests/fork/csp-smoke.spec.ts`                                                                                                                                                           |
| Self-check verdict incl. CSP                                                  | `fork::commands::selfcheck::tests::*`                                                                                                                                                    |

## Releases and updates (F-02)

Owner-facing steps, secrets and key custody: `docs/fork/RELEASING.md`.

### Files

| File                                            | Role                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ----------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/fork/update/channel.rs`          | Channel → endpoints (`updater_builder().endpoints(…)`, DEC-33): stable = the overlay's `releases/latest/download/latest.json`; beta = the rolling `beta` prerelease's `latest.json` **then** stable. Pins `PUBKEY_KEY_ID` and the owner. `is_update_url`: downloads only from `github.com/fjenn/wispr-bro/releases/…` over https (R5 §1), or loopback in debug builds. Debug-only `WISPRBRO_UPDATE_ENDPOINT` (loopback only) points both feeds at a stand-in.                                                                                                                                 |
| `src-tauri/src/fork/update/mod.rs`              | `check` / `install` / `status` on tauri-plugin-updater 2.10.1. One plugin check per endpoint; `pick_newest` takes the highest semver and tolerates one failing feed (no beta release yet). `download_verified` refuses foreign URLs, then the plugin downloads and verifies the minisign signature against the overlay pubkey. Refuses with `Disabled` while Handy's effective `update_checks_enabled` is off (setting or kill switch). Debug-only `WISPRBRO_UPDATE_SELFTEST=1`: one check + install at `on_ready`, logged as `fork update selftest: …`, then relaunch (the Mac update test). |
| `src-tauri/src/fork/commands/update.rs`         | `fork_update_status`, `fork_update_check`, `fork_update_install` (relaunches after install).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `src/fork/debug/UpdateDebug.tsx`                | Debug panel "Updates": version, channel, key id, feeds, Check now, Install and relaunch. S-28 builds the product UI (footer, tray, scheduler, idle gate) on the same calls.                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `scripts/wb-release/macos.sh`                   | The release pipeline (this Mac and CI): guard → sidecar signed → `tauri build --bundles app` with the Developer ID (Tauri's own notarization kept off) → nested-code check → notarize + staple app → updater `.app.tar.gz` from the stapled app + `tauri signer sign` → DMG (hdiutil) signed, notarized, stapled → `latest.json`, `SHA256SUMS` → `verify.sh`. `--unsigned`: secret-free dry run.                                                                                                                                                                                              |
| `scripts/wb-release/verify.sh`                  | wb-verify: DMG signature/Gatekeeper/staple, app `codesign --verify --deep --strict`, Team ID + hardened runtime on every Mach-O, identifier/version/min macOS, `spctl -t exec` = Notarized Developer ID, staple, updater signature, archive layout, the extracted app again, `--list-devices` smoke under a throwaway `HOME`.                                                                                                                                                                                                                                                                 |
| `scripts/wb-release/{tag,manifest,minisign}.ts` | Tag format/order/channel; `latest.json` writer (signature checked first, asset names GitHub will not rewrite); minisign verifier (same check as the plugin: Ed25519 over BLAKE2b-512 + the trusted-comment signature). Tests: `release.test.ts`.                                                                                                                                                                                                                                                                                                                                              |
| `.github/workflows/wb-build.yml`                | macos-26 aarch64 build through `macos.sh` (temporary keychain, notary API key), then `verify` on a fresh runner. Signing only on `wispr-bro` or a `v*` tag.                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `.github/workflows/wb-release.yml`              | On `v*` tags on `wispr-bro`: preflight (tag, branch, all guards `--release`), notices, egress, build, publish (draft for stable/alpha/rc; rolling `beta` prerelease for `-beta.N`) with the GPL assets.                                                                                                                                                                                                                                                                                                                                                                                       |
| `scripts/wb-f02/update-e2e.sh`                  | Mac test F-02.3: A → B over a loopback GitHub stand-in, stable ignores beta, designated requirement unchanged.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |

### Decisions

- **No tauri-action.** It can neither staple the DMG (RS-12) nor maintain the
  rolling beta release; `macos.sh` calls the same Tauri CLI and is the one
  path the local acceptance ran. `gh release` publishes.
- **The updater archive is rebuilt from the stapled app** (Tauri's own
  archive is made before notarization) and re-signed with `tauri signer
sign`; an update therefore installs an app that passes Gatekeeper offline.
- **DMG from hdiutil**, notarized and stapled separately after the app.
- **Beta reads both feeds** and takes the newer, so beta testers are never
  stuck behind a stable release (`1.0.0-beta.3 < 1.0.0`).
- **Identity guard pins the key id and the exact stable feed**, not just "a
  key on some owner's repo".

## CI guards (F-03)

`bun run wb:guards` runs every source guard (R5 §4.12); add `-- --release`
for release mode, `--base <ref>` for another upstream ref, `--binary <path>`
to also scan a built app. Each guard also runs alone (`wb:guard:<name>`).
Shared code: `scripts/wb-guard-lib.ts` (file listing, `FORK_PATHS`, list
parsing, URL/host extraction). Self-tests: `scripts/wb-guards.test.ts`
(`bun run wb:test:guards`, also part of `wb:test:scripts`).

| Guard                      | Checks                                                                                                                                                                                                                                                                                                                                                         | Config                                                                         |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| `wb-guard-identity.ts`     | `tauri.conf.json` + overlay (RFC 7396 merge) has no `com.pais.handy`, `cjpais/Handy` or Handy pubkey prefix; productName/identifier ours; updater pubkey is ours (key id `89E085217ACC46CF`, F-02) and the only endpoint is our stable feed on `github.com/fjenn/wispr-bro`; Cargo/package/overlay versions (and tag) agree; `--release`: no F-02 placeholders | —                                                                              |
| `wb-guard-plumbing.ts`     | `build.rs` `fn main` ends in `tauri_build::build()`, no `AppManifest`/`InlinedPlugin`; no Handy `fork_*` command; one `.invoke_handler(fork::compose_invoke(invoke_handler))`; upstream-bound files are fork-free and MIT-clean                                                                                                                                | `ci/upstream-bound.txt`                                                        |
| `wb-guard-hooks.ts`        | `git diff --numstat` vs `merge-base(HEAD, upstream/main)`: every changed upstream file listed and within budget; generated lockfiles (`lock` entries) may change only with their manifest changed and within budget; `sha256:` entries (only `LICENSE`) must hash to the pinned text; new files only under `FORK_PATHS`                                        | `ci/hooks.txt`                                                                 |
| `wb-guard-hosts.ts`        | URL hosts in fork code ⊆ allowed (`[link]` only in `openUrl(…)`, `[schema]` only as `"$schema"`, `[test]` only in fork test code); hosts in upstream code ⊆ baseline ∪ inert ∪ allowed (never `[test]`); list entries carry checked comments                                                                                                                   | `ci/allowed-hosts.txt`, `ci/upstream-hosts-baseline.txt`, `ci/inert-hosts.txt` |
| `wb-guard-bundle-hosts.ts` | hosts in `dist/**/*.{js,css,html}` and Mach-O strings ⊆ allowed (except `[test]`) ∪ inert; skips without a build                                                                                                                                                                                                                                               | same                                                                           |
| `wb-guard-telemetry.ts`    | whole-name match of banned packages in `Cargo.lock`, `Cargo.toml`, `bun.lock`, `package.json`, TS/JS imports and Rust `use`                                                                                                                                                                                                                                    | `ci/telemetry-packages.txt`                                                    |
| `wb-guard-spdx.ts`         | SPDX header in fork code; AGPL files listed in `NOTICE` (§4, since F-04); no GPL/AGPL header outside fork paths                                                                                                                                                                                                                                                | `ci/generated.txt`, `NOTICE`                                                   |

**Hook budgets and lockfiles.** `ci/hooks.txt` budgets equal the measured
diff (F-08 added `settings.rs` 7, `llm_client.rs` 3, `catalog/mod.rs` 2,
`managers/model.rs` 5, and raised `Cargo.toml` to 7 and `package.json` to
28). A generated lockfile is listed as `<path> lock` instead of a budget:
`Cargo.lock`, `bun.lock` and the bun2nix files `.nix/bun.nix` /
`.nix/bun-lock-hash` (regenerated from `bun.lock` by Handy's `postinstall`,
`scripts/check-nix-deps.ts`). Any change to such a file passes only if its
manifest (`Cargo.toml` for `Cargo.lock`, `package.json` for the others) is
listed with a numeric budget, is changed and is within it; lockfile churn
with an untouched manifest (a bare `cargo update`) fails, and `lock` on any
other path is a list error.

**Whole-file replacements (F-04).** `LICENSE` is Handy's MIT text replaced
by the verbatim GPL-3.0 text (674 added + 21 deleted lines). It is listed as
`LICENSE sha256:<hex>` instead of a budget: a 695-line budget would accept
any other 695-line edit, while the pin accepts exactly the GNU text
(`gpl-3.0.txt`, sha256 `3972dc97…`). The guard fails when the file is
missing or its hash differs; only paths in `REPLACEABLE`
(`wb-guard-hooks.ts`, today just `LICENSE`) may use the mode. Handy's MIT
text lives on unchanged in `LICENSES/MIT-Handy.txt`. `README.md` has a
normal budget (2: the licence sentence).

**Test-only hosts.** Tests that prove a refusal name hosts the app must
never contact. They go in the `[test]` section of `ci/allowed-hosts.txt`,
accepted only in fork test code: `scripts/wb-*.test.ts`, `tests/fork/**`,
fork `tests.rs` / `testutil.rs` files whose parent module declares them
with exactly `#[cfg(test)] mod <name>;`, and, in other fork Rust files, the
body of a `#[cfg(test)] mod <name> { … }` block (found by brace matching on
a token stream that skips strings, raw strings, chars and nested comments).
Anything the lexer cannot read, or `cfg(any(test, …))`, does not count as
test code. Comments of production fork code do not count either (F-04
removed that exception): a doc comment names a refused host in words, not
as a URL. Spoof hosts in tests use RFC 2606 names (`*.example`:
`hf.co.evil.example`, `github.com.evil.example`, `localhost.evil.example`,
`127.0.0.1.nip.example`); `refusal:` entries remain only where the test
needs the real name (`api.openai.com` and the other cloud LLM hosts,
`blob.handy.computer`, `handy.computer`, the bare `hf.co` apex and the
no-dot lookalike `evilhuggingface.co`, which must end in the real `.co`). Each entry's comment starts with a category the
guard checks: `example:` (RFC 2606/6761 name), `private:` (RFC 1918 /
link-local IPv4), `loopback:` (127/8 in any spelling, `[::1]`) or
`refusal:` (a real host named to prove refusal, no wildcard). `[test]`
hosts are never accepted in production fork code (`hosts/test-only`), in
upstream code or in the built bundle.

**Inert hosts.** `ci/inert-hosts.txt` sections and their required comment:
`[identifier]` `reason:`, `[proven]` `proof:` (fork test paths; the
self-tests check every named `fork::…::fn` exists as a Rust test) and
`[link]` `source:` (click-only `openUrl` links, with their locations). Since
F-08: Handy's cloud LLM hosts and `blob.handy.computer` are `[proven]`
(`fork::llm::tests::*`, `fork::settings::tests::a_stored_openai_provider_is_migrated_to_local`,
`fork::models::tests::downloader_never_builds_a_blob_handy_computer_url`,
`fork::net::tests::the_handy_mirror_is_never_a_mirror_base`) and
`handy.computer` is a `[link]` until S-26b replaces Handy's links.

Workflows: `wb-ci.yml` (PR/push to `wispr-bro`: guards, script tests,
translations, lint, `wb:format:check`, fork Playwright; `cargo test` and
`wb:clippy` on macos-26; `cargo-about` licence report from `about.toml` and
`bun scripts/wb-notices.ts --check`), `wb-dco.yml` (F-04: every PR commit
signed off) and
`wb-egress.yml` (nightly: release build, `scripts/wb-egress/session.ts` under
a pf anchor for a dedicated user, bundle-host guard). The choices (fork-only
clippy, `.wb-prettierignore`, egress isolation) are explained in each
workflow's header.

## GPL compliance and notices (F-04)

| File                               | What                                                                                                                                                                   |
| ---------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `LICENSE`                          | Verbatim GPL-3.0 (`gpl-3.0.txt` from gnu.org, pinned by sha256 in `ci/hooks.txt`)                                                                                      |
| `LICENSES/MIT-Handy.txt`           | Handy's MIT text, unchanged (© 2025 CJ Pais)                                                                                                                          |
| `LICENSES/OFL-1.1-NotoSansCJK.txt` | OFL for Handy's Noto Sans CJK subset font (copyright line read from the font)                                                                                          |
| `NOTICE`                           | §1 licence, §2 Handy, §3 ports (from `PORTS.md`), §4 AGPL-3.0-only files (read by the SPDX guard), §5 Apache NOTICE files, §6 model attributions (DEC-26), §7 fonts    |
| `docs/fork/LICENSE-REGISTER.md`    | Licence register (R4 §5 + DEC-26/DEC-27): source of truth for what may be a default                                                                                    |
| `scripts/wb-notices.ts`            | Generates `THIRD-PARTY-NOTICES.md` (committed): cargo-about JSON + npm runtime closure + manual C/C++ list + NOTICE §6/§7; fails on unaccepted licences and list drift |
| `CONTRIBUTING-WB.md`, `wb-dco.yml` | DCO sign-off, SPDX, thin-layer and no-Wispr-Flow rules; inline DCO check on PRs (upstream Handy commits exempt)                                                        |
| `docs/fork/RELEASING.md`           | Stub: F-02 must attach `THIRD-PARTY-NOTICES.md`, `LICENSE`, `NOTICE` and a `git archive` source tarball to every release                                               |

`README.md` gets one sentence (GPL-3.0 for the fork, Handy's code MIT).
Handy's About page shows no licence and its source link is a hard-coded
`openUrl` in `AboutSettings.tsx` (not a translation string), so the fork
i18n overrides cannot change it; S-26b (links) and S-39 (About /
Acknowledgements page) show "GPL-3.0", our source link and NOTICE §6.
`src-tauri/Cargo.toml` gets no `license` field (thin-layer rule).

## Local LLM runtime (S-10)

The `llama-server` sidecar, its supervisor and the `local` provider (R5
§4.5, DEC-03, DEC-49). Numbers: `docs/fork/spikes/RS-06-llm-runtime.md`.

### Files

| File                                      | Role                                                                                                                                        |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `ci/llama-server/build.sh`                | Builds the static sidecar from llama.cpp b11624 (sha256-pinned source), checks dylibs/minos/version, signs ad hoc with the hardened runtime |
| `fork/llm/catalog.rs`                     | Pinned models (repo, revision, sha256, size, licence, resident MiB) and the tier rule (`select`)                                            |
| `fork/llm/download.rs`                    | hf-hub download into `HF_HOME/hub` at the pinned revision, fixed endpoint, no token, stall watchdog, backoff, sha256 + verification marker  |
| `fork/llm/process.rs`                     | Spawn line, cleared env, SIGTERM→SIGKILL, PID file + stale kill, forked kqueue watchdog, stderr tail                                        |
| `fork/llm/transport.rs`                   | DEC-49 endpoint choice (socket, else loopback TCP), random key, reqwest client config shared with Handy's chain                             |
| `fork/llm/health.rs`                      | `/health` probe and wait                                                                                                                    |
| `fork/llm/supervisor.rs` (+ `tests`)      | One start at a time, restart once → CPU → backoff, crash restart, idle stop, wake check, completions                                        |
| `fork/llm/chat.rs`                        | The request (temperature 0, seed, JSON schema, no thinking) and the streamed reply                                                          |
| `fork/llm/memory.rs`                      | Available memory, the headroom rule, the ordered-load planner                                                                               |
| `fork/llm/prompt.rs`                      | OpenWhispr cleanup prompt (seed for S-12a), budgets, reply parsing                                                                          |
| `fork/llm/runtime.rs`                     | App wiring: state, events, download trigger, prewarm, wake, first-launch warm-up, `WISPRBRO_SELFCHECK_LLM`                                  |
| `fork/llm/rs06.rs`                        | RS-06 bench (ignored test)                                                                                                                  |
| `fork/commands/llm.rs`                    | `fork_llm_status`, `fork_llm_try`, `fork_llm_retry`; event `fork-llm-status` (`ForkLlmStatus`)                                              |
| `src/fork/settings/advanced/AiEngine.tsx` | Settings › Advanced › AI engine, mounted on the debug panel until S-14                                                                      |
| `scripts/wb-translate-llama.ts`           | `llama-server` backend of `wb-translate-missing.ts` (`--backend llama-server --server … --model …`)                                         |

No new upstream hook: the plugin's `setup` registers the runtime state,
`on_ready` starts it, `RunEvent::Exit` stops the server; prewarm listens to
Handy's `recording-state-event` and `model-state-changed`; Handy's chain
reaches the sidecar through F-08's `client_builder` hook. `package.json`
gained `wb:llama-server` and the build step in `wb:dev`/`wb:build`.

### Lifecycle

1. **Launch** (`on_ready`): kill a `llama-server` named in
   `<app data>/llm/llama-server.pid` (only if that PID is a `llama-server`).
   Pick the model by RAM (`catalog::select`; none on Intel or in a plain
   Handy build). If Handy has a speech model on disk, check/download the
   cleanup model; otherwise wait for `model-download-complete`.
2. **Ready**: the model is verified; the server is not running. Right after
   the first download (and after an update changes the sidecar) the runtime
   starts and stops it once to pay Metal's shader compile (~3.5 s on an M5
   Max) outside a take.
3. **Key-down**: the planner wakes the server at once if ASR is loaded,
   else after ASR loads or at key-up; the memory rule can veto the start.
4. **Requests**: `Supervisor::complete` (fork) or Handy's chain (`local`
   provider) over the socket with the key; streamed replies so Esc frees the
   slot at once.
5. **Idle**: stopped 120 s after the last request. **Wake**: `/health`
   probe, restart if dead. **Crash**: restart once in the background.
   **Quit / app crash**: `RunEvent::Exit` stops it; on SIGKILL or a crash
   the watchdog kills it within milliseconds.

### Signing (F-02)

- `build.sh` signs ad hoc with `--options runtime` and the identifier
  `com.wisprbro.desktop.llama-server`. RS-06: Metal works under the hardened
  runtime with no entitlement (no `allow-jit` / `allow-unsigned-executable-memory`).
- `scripts/wb-release/macos.sh` re-signs it with the Developer ID, the
  hardened runtime and a secure timestamp before `tauri build`; Tauri then
  signs its copy in `Contents/MacOS` again with the app's identity and
  `Entitlements.plist` (Tauri 2.11 has no per-binary entitlements; harmless,
  TCC attributes the sidecar to the app). The script then checks every Mach-O
  in the bundle for Team ID `446XT9839S` + `flags=runtime` before
  notarization, and `verify.sh` checks the sidecar's timestamp.

## Text pipeline (S-11)

`fork/text/{mod,fold,rules,placeholders,lines,commands,scratch,whitespace,counts}.rs`,
`fork/text/tables/commands.<lang>.json` (de, en, es, fr, it, ja, pl, pt,
zh), `fork/db/rules.rs`, `fork/db/migrations/0003_text.sql`. R5 §4.10 is
the stage list; this is how it is wired.

**Hooks.** `managers/transcription.rs` `finish_text` wraps Handy's call:

```rust
crate::fork::text::process(text, &output_language, |text| {
    post_process_transcription_text(text, settings, …)
})
```

so Handy's stages (script conversion, fuzzy custom words, fillers,
stutters, normalize) run once per line inside the fork pipeline, and
`audio_toolkit/text.rs` stays untouched. After the LLM,
`fork::insert::deliver` calls `fork::text::finish` first (no new upstream
line; S-04's `actions.rs` hook is unchanged).

**Order per take** (`text::run`): sanitize (VoiceTypr) → per line: dictionary
rules with outputs replaced by Private Use Area placeholders → Handy's
stages → placeholders restored (if one moved, the line is redone with the
outputs inserted plainly) → voice commands and "scratch that" over the
whole text → line-safe whitespace. Rules run before Handy's script
conversion because Handy's stages are one unit; their outputs are
placeholders while Handy runs, so nothing rewrites them.

**Rules** come from `fork.db` (`dictionary_entries` kind `rule`, status
`active`) through a source `fork::on_ready` installs; each take reads them
(one indexed query) and recompiles only when they changed, so S-16's edits
apply to the next take with no reload signal. Headless CLI runs have no
source and no rules.

**Voice commands** are chosen by the output language Handy resolved
(`OutputLanguageEvidence::language()`): that language's table, every table
when unknown, none for a language without one. Clause-edge rule, literal
escape and the CJK right-edge rule are documented in
`fork/text/commands.rs`.

**Per take** the pipeline's record (stage order and change counts, rule
hits, provenance) goes through the take ledger (`Ledger::note_processed`,
keyed by the kept text like S-09's verbatim text) to `on_saved` /
`on_retried`: `entry_meta.text_stages` (JSON), `fix_count_dictionary`,
`daily_stats.fixes_dictionary`, one `corrections` row per hit (source
`rule`, replaced on retry) and `dictionary_entries.hit_count`. The log gets
one line per take without text: `text: rules=1 handy=1 commands=3
scratch=1 whitespace=0 (language en, 2 dictionary rule(s) active)`.

**After the LLM** (S-12a): `text::note_rewrite(kept, output)` registers the
rule provenance of the kept text for the LLM's output; `finish` restores
any rule output the LLM reverted (VoiceTypr's final guard) and re-runs the
whitespace stage, then re-keys the take's paste-outcome match to the new
text. Without a registered rewrite `finish` only normalizes whitespace,
which is a no-op on pipeline output.

**Fail-open.** A panic in a fork stage returns Handy's own result for the
whole text (Handy's stages keep their own `fail_open_text_transform`).

## Dictionary (S-16)

One store (`fork.db` `dictionary_entries`, migration `0004_dictionary.sql`),
two consumers that never mix (R2 C-67):

- **Words** (`kind = 'word'`) are mirrored into Handy's `custom_words`
  setting, which Handy's recognizer path already reads (decoder vocabulary
  when the model supports it, else its fuzzy word fix). No Handy file
  changes: `fork::dictionary::migrate::sync` writes the list through
  `ForkRuntime::write_handy_settings` (Handy's `get_settings` /
  `write_settings`, what Handy's `update_custom_words` command does) and
  emits `settings-changed` so Handy's settings store re-reads it.
- **Replacements** (`kind = 'rule'`) are read per take by the S-11 rule
  stage (`db::rules::active_rules`); they are never given to the recognizer.

| File                                                | Role                                                                                                                                                                                                |
| --------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/fork/dictionary/mod.rs`              | Types, validation (`clean`: tidy whitespace, ≤ 60 characters for a word or "from", ≤ 200 for a replacement, no control characters), `ForkDictionaryChanged`                                         |
| `src-tauri/src/fork/dictionary/store.rs`            | Add / edit / delete / restore; one entry per kind and key (`norm`, a Unicode case fold); a suggested or rejected entry becomes active when re-added; 5,000 entries at most; the migration 0004 hook |
| `src-tauri/src/fork/dictionary/migrate.rs`          | Three-way merge with Handy's list against `dictionary_handy_mirror` (one-time migration, then words added or removed in Handy's own editor follow)                                                  |
| `src-tauri/src/fork/dictionary/import.rs`           | `.csv` read (≤ 3 MB, UTF-8 / UTF-16 BOM / Windows-1252), review (new, duplicates, conflicts, invalid) and commit in one transaction; formula unguard                                                |
| `src-tauri/src/fork/dictionary/export.rs`           | CSV with the `csv` crate: UTF-8 BOM, `phrase,replacement`, formula guard (`'` before `= + - @`), atomic write                                                                                       |
| `src-tauri/src/fork/commands/dictionary.rs`         | `fork_dictionary_{list,add,update,delete,restore,read_file,import_review,import,export}`; every change syncs Handy's list and emits the event                                                       |
| `src/fork/dictionary/DictionaryPage.tsx`            | Tabs All / Added by you / Suggested with live counts, search (case and accent blind), sort, refresh, banner, rows with edit / delete (Undo toast)                                                   |
| `src/fork/dictionary/AddWordDialog.tsx`             | Add / edit: a word, or "Replace a word" from → to; disabled until filled and within the limits; duplicate and conflict feedback ("Replace it")                                                      |
| `src/fork/dictionary/ImportWizard.tsx`              | The dictionary's fields, review and summary for the shared wizard                                                                                                                                   |
| `src/fork/dictionary/useDictionary.ts`              | Live list (`ForkDictionaryChanged`, `ForkStatsUpdated` for hit counts, focus), `NotReady` retry, search key and sort                                                                                |
| `src/fork/ui/ImportWizard.tsx`, `ui/importTable.ts` | The shared four-step CSV wizard (S-17 reuses it): choose (open panel or a drop zone on the step, C-205), map columns with preview, review, done; papaparse                                          |

**Sync points.** `fork::on_ready` (a thread), every page command that
changes something, and `fork_dictionary_list` (so words added in Handy's
own Settings › Advanced editor, still hosted until S-14, show up). The
mirror records what Handy holds after the write, read back, so a failed
write never looks like a removal.

**Format.** `docs/fork/user/import-format.md`; the same rules are in the wizard
under "Accepted format" (shown in place: the app opens no web page).

**Fork npm package.** `papaparse` 5.7.0 (runtime, MIT) and
`@types/papaparse` 5.5.2 (dev). Rust: `csv` 1.4 and `encoding_rs` 0.8 (the
latter already compiled) in the `# fork:` dependency block.

## Dictionary auto-learn (S-34)

Opt-in, off by default (DEC-20, `ForkSettings::auto_learn_enabled`, toggled
on Dictionary › Suggested and on Settings › Data and Privacy). No Handy
file, no migration (the S-16 schema already had `status` `suggested` /
`rejected`, source `learn` and `corrections` source `learn`).

```
fork::insert::mac::deliver ── pasted, kind Take, no auto Return ──▶ learn::on_pasted
   (setting on, AX trusted, text ≤ 12,000 chars)            │ thread "wb-learn"
                                                            ▼
AX thread: watcher::mac::arm ── S-20 exclusions ──▶ ctx.watched (+ S-05 lease)
fork tap (engine.event): Return / keypad Enter / Tab ──▶ Registry::notify_commit_key ──▶ channel
every 250 ms: watcher::mac::poll ──▶ Session::observe ──▶ commit (key, focus moved, field emptied)
                                                        or nothing (30 s, not found, secure input…)
diff::edit_of (16-char anchors) ──▶ diff::corrections ──▶ review::suggest ──▶ ForkDictionaryChanged
```

| File                                              | Role                                                                                                                                                                                                                    |
| ------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/fork/dictionary/learn/mod.rs`      | Orchestration: setting check, one watch at a time (a newer paste or `stop()` ends the older), commit-key channel, saving, the log (outcome, counts, ms; never text)                                                     |
| `src-tauri/src/fork/dictionary/learn/watcher.rs`  | `Session` (pure): baseline = first read containing the paste, last non-empty value, commit rules, 30 s window, 3 s to find the paste, 100,000-character cap; `is_commit_key`; `mac::{arm,poll,disarm}` on the AX thread |
| `src-tauri/src/fork/dictionary/learn/diff.rs`     | `comparable`, `edit_of`, `corrections` (word LCS, 1-3 words a side, any number of hunks, rewrite cutoff, everyday words, edit-distance ratio, VoiceInk's candidate checks)                                              |
| `src-tauri/src/fork/dictionary/learn/review.rs`   | Correction → suggestion (ASCII → word, else replacement; everyday words dropped), known/rejected check, caps (5 per edit, 100 waiting), `accept`, `reject`                                                              |
| `src-tauri/src/fork/ax/thread.rs`                 | `Ctx::watched` (the watched element, its pid, role, frame and lease)                                                                                                                                                    |
| `src-tauri/src/fork/shortcut/{actions,engine}.rs` | Commit-key observers on the action registry; the engine tells them on every qualifying key-down (never blocks the key)                                                                                                  |
| `src-tauri/src/fork/commands/dictionary.rs`       | `fork_dictionary_accept`, `fork_dictionary_reject`                                                                                                                                                                      |
| `src/fork/dictionary/DictionaryPage.tsx`          | Suggested tab: the toggle, Accept ✓ / Reject ✕ rows (no edit/delete)                                                                                                                                                    |
| `src/fork/settings/DataPrivacy.tsx`               | "Learn from my corrections" row                                                                                                                                                                                         |

**Rules.** Not watched: Secure Input on, password managers, banking apps
and sites, terminals, Wispr Bro, secure / numeric / sensitive fields,
browser address bars (S-20's `exclusions.json` and `field_exclusion`), a
field over 100,000 characters, a take sent by an automatic Return. The
same element is followed by `CFEqual`, or the same role, x and width (a
chat composer grows in height). A deleted learned entry and a rejected
suggestion stay as `rejected` rows, so they never return; adding the word
by hand makes it active. Erase (S-27) calls `learn::stop()` first.

## Cleanup levels (S-12a)

R5 §4.10 stage 8 on the local LLM: every dictation is cleaned at the
user's level (None / Light / Medium, fork setting `cleanup_level`, default
Light) by `fork::llm::post_process`, with the guard chain between the model
and the field. Eval: `eval/cleanup/README.md` "S-12a"; numbers in R5 S-12a
"As built".

### Files

| File                                         | Role                                                                                                                                                        |
| -------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `fork/llm/post_process.rs` (+ `_tests`)      | The core `clean()` (gate, lead-ins, blocks, line fallback, deadlines, outcome) and the hooks `post_process` / `post_process_job`; `ForkCleanupResult` event |
| `fork/llm/guards.rs` (+ `_tests`)            | The guard chain (R5 §4.5 order): ports of SpeakoFlow, FreeFlow, BetterWispr `accepted()`, Muesli markers; our entity, formatting, content-word, language    |
| `fork/llm/markers.rs`, `tables/markers.json` | Per-language hesitations, the owner's filler table, repair markers, stopwords, assistant openers; word segmentation                                         |
| `fork/llm/leadins.rs`                        | Sentence lead-ins removed without the model ("nou dus …", "alors du coup …", "so …")                                                                        |
| `fork/llm/prompt.rs`, `prompts/*.txt`        | Prompt builder: OpenWhispr `cleanupPrompt` (MIT, byte-identical to the RS-07 input) + our Medium, vocabulary and language fragments                         |
| `fork/llm/deadline.rs`                       | Request, take and server-start budgets                                                                                                                      |
| `fork/llm/eval_run.rs`                       | Ignored test: the eval set through `clean()` on the real sidecar (JSONL for `wb-eval-cleanup.ts --replay`)                                                  |
| `src/fork/style/CleanupLevelPicker.tsx`      | The level picker and the last take's result; on the debug panel until S-19a                                                                                 |

### Hooks

| File                             | Hook                                                                                                                                     |
| -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| ⚠ `actions.rs` (stop task)      | `process_transcription_output(&ah, …)` → `crate::fork::llm::post_process(&ah, &transcription, post_process)` (same signature and result) |
| `managers/transcription_jobs.rs` | the same for retries and file transcriptions → `crate::fork::llm::post_process_job(…)`; the now-unused import removed                    |
| `managers/transcription.rs`      | Handy's Info line "Transcription result: …" logs `crate::fork::diagnostics::redact(&final_result)` (`[N chars]`) instead of the text     |

Handy's `process_transcription_output` / `post_process_transcription`
(its cloud-era chain, Apple call included) stay untouched; the fork calls
them only where it does not own cleanup (headless CLI, a plain Handy build
without the LLM runtime). Apple Foundation Models: "Apple Foundation Models (S-12b)" below.

### One take

1. **Plan** (`plan`): level from fork settings; the take's language from
   Handy's pipeline (ledger, `TextRecord.language`), else a **hint** from the
   text among the user's `spoken_languages` (word votes over the tables,
   then whatlang ≥ 0.8 from 8 words); the dictionary's words (Handy's custom
   words, S-16 mirror) and the replacements' outputs for the prompt.
   Skips, in order: an AI-chat target (`is_ai_chat` of the frontmost app,
   where the text will go; dictations only), no model yet, the memory rule.
2. **`clean`**: level None or no words → skipped. Sentence lead-ins of the
   prompt language go (`leadins.rs`). Light and already clean (no filler,
   repair or stutter; capitalized and punctuated; open-flow's gate) →
   skipped, no request. Otherwise the text is cut into **blocks** of ≤ 120
   words (lines grouped; a short letter is one request); each block is one
   request with the R5 deadline, inside a take deadline whose 6 s cap
   grows per 200 words (24 s at most), plus 2.5 s for a cold server start.
3. **Guards** on each reply. A multi-line reply that only lost its line
   breaks is **realigned** (word alignment puts the input's breaks back) and
   guarded again; one that came back as a list, or cannot be realigned, is
   sent again line by line (clean lines skipped). Any rejection, timeout or
   error keeps that block's deterministic text.
4. **After**: stage 9 (`restore_spellings`: `note_rewrite` + `finish`)
   puts back replacement outputs the model undid; the record
   (`cleanup_level`, `cleanup_engine`, `cleanup_outcome`, `fix_count_llm`
   = runs of changed words, `daily_stats.fixes_llm`) goes through the
   ledger to `on_saved` / `on_retried`; one Info line without text
   (`cleanup: applied light (local, 1/1 chunk(s), 212 ms, 2 fix(es))`);
   `ForkCleanupResult` with `notice` (not ready, low memory, guard,
   timeout, error → "Inserted without cleanup", shown by S-03's pill).

The history row's `post_processed_text` is the cleaned text (Handy's own
column), so History and `entry_meta.final_text` show what was inserted;
`asr_text` keeps the verbatim recognizer text. Level None also turns
Handy's filler removal off (`ENFORCED` rule `filler_word_removal_enabled`,
mirrored at once on change).

### Guard order and what each catches

`json` → `empty` (unless the input was only fillers) → `punctuation_only`
→ `channel` → `prompt_echo` (SpeakoFlow 6-grams + the user-turn framing)
→ `too_long` → `executed_instruction` (FreeFlow) → `word_band` (BetterWispr,
lower bound on the words that must stay) → `question_mark` → `new_tokens`
(≤ 25 %, a joined word or an added/dropped ending is not new) →
`assistant_marker` → `entity` (digits, URLs, e-mails, code spans,
dictionary words; a number inside a self-repair may go; no new digits) →
`line_count` → `formatting` (no bullets, numbering, headings or bold the
input lacked) → `content_words` (stopwords, fillers and up to 8 words
before a repair marker excluded; Light 1 in 10 may go, Medium 1 in 4; a
word changed into a look-alike, "negeer" → "Neger", never) → `language`
(Handy `lang_id` of the output vs the take's or the input's language).
The guards use only the take's known language, never the hint.

## Snippets (S-17)

A snippet is a spoken trigger and the plain text written instead (DEC-12;
`body_html` stays NULL). Store: `fork.db` `snippets` (created by 0001),
extended by migration `0006_snippets.sql` (`last_hit_at`, `source`, and
`entry_meta.inserted_text`). Expansion is stage 11 of the text pipeline and
runs **after** the LLM step, inside `fork::insert::deliver`:

```text
finish_text hook → fork::text::process (stages 3, 2/4/5, 7, 12; no snippets)
  → fork::llm::post_process (S-12a; its stage 9 is text::after_llm: 9 → 12,
    no snippets) → history save
  → insert::deliver → fork::text::finish: 9 → 12 → 11 (snippets) → paste
```

Stage 12 runs before the snippets, so a trigger meets tidy spacing and the
saved text is inserted exactly as written (its own spacing, blank lines,
indentation), and nothing scans it again. `text::finish_with` is the pure
form the tests use; a panic in the stage inserts the text without snippets.
`finish` is the only entry that expands snippets: S-12a's stage 9 after the
LLM calls `text::after_llm` (the same stages without 11), so a saved text
is expanded once, after History saved the spoken words, and never scanned
twice. A unit test reads `actions.rs`, `insert/mod.rs` and
`llm/post_process.rs` and fails if this order changes.

| File                                                 | Role                                                                                                                                                                                                                                                                                                                |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/fork/text/snippets.rs`                | The matcher (MIT AND GPL: OpenWhispr `snippets.ts` port). `aho-corasick` over triggers folded like S-11 (plus Turkish İ/ı → i), NFC through `icu_normalizer` (already compiled), alone rule (punctuation, symbols, spacing ignored), inline boundaries (letters, digits, `_`, combining marks), `'s` yes / `n't` no |
| `src-tauri/src/fork/text/snippets_tests.rs`          | The acceptance table (50 cases) and Unicode boundaries                                                                                                                                                                                                                                                              |
| `src-tauri/src/fork/text/mod.rs`                     | `install_snippets(source, sink)`, `active_snippets` (compiled once per change), `finish`/`finish_with`, `SnippetUse`                                                                                                                                                                                                |
| `src-tauri/src/fork/snippets/mod.rs`                 | Types, validation (`clean`: trigger ≤ 60 characters on one line with a letter or digit; text ≤ 4,000, `\r\n` → `\n`, trailing whitespace and leading blank lines dropped), `ForkSnippetsChanged`, `install` (DB source, the use recorder on its own thread)                                                         |
| `src-tauri/src/fork/snippets/store.rs`               | Add / edit (keeps the use count) / delete / restore; one snippet per `trigger_norm`; 1,000 at most; `record_use` (hit counts, `text_stages` gets `{"stage":"snippets","changes":n}`, `inserted_text`)                                                                                                               |
| `src-tauri/src/fork/snippets/import.rs`, `export.rs` | CSV `trigger,text` through S-16's reader and formula guard; ≤ 1,000 rows per file; multi-line cells; atomic write (`dictionary::export::write_atomic`)                                                                                                                                                              |
| `src-tauri/src/fork/commands/snippets.rs`            | `fork_snippets_{list,add,update,delete,restore,import_review,import,export}`; the file is read by `fork_dictionary_read_file`                                                                                                                                                                                       |
| `src/fork/snippets/`                                 | `SnippetsPage` (banner `snippets.examples` with three examples, live counts, search over trigger and text, sort, refresh, Undo), `SnippetDialog` (⌘Return saves), `ImportWizard` (S-16's shared wizard), `useSnippets`                                                                                              |

**History.** Handy's row and `entry_meta.final_text` keep the spoken words
(the stats count those, so a long saved text never inflates words per
minute); `entry_meta.inserted_text` holds what was pasted, for S-15b's Copy.
The paste outcome still finds the row: `finish` rekeys the ledger's
awaiting entry to the inserted text, as S-11 does.

**Live counts.** Every page edit and every take that expanded a snippet
emits `ForkSnippetsChanged` (with the counts); the page also re-reads on
`ForkStatsUpdated` and focus.

**Logs.** One INFO line per delivery with counts only (`snippets=1`);
neither triggers nor texts are logged.

## Styles: app groups and presets (S-19a)

Every dictation goes to one style group (Personal / Work / Email / Other)
by the app it is inserted into, and the group's preset shapes the text.
**No Handy file is touched**: everything runs in the S-12a cleanup step
(`fork::llm::post_process`, the existing `actions.rs` hook) and S-04's
delivery.

```text
post_process::plan   frontmost app → app_rules::resolve (site rule > the user's
                     app rule > categories.json) → group → preset (fork setting
                     `styles`) when the take is English and the app is not an
                     AI chat; auto-send if the app has it on
post_process::clean  the level's own prompt, whatever the preset
                     (prompt::style_fragment is None for all four)
post_process::run    stage 9 → stage 10 (text::style_rules) → History;
                     auto-send armed for the final text unless the cleanup fell
                     back ("Inserted without cleanup")
insert::deliver      takes the arming → stage 9/12/11 → paste → Return only after
                     a confirmed paste into the same app (submit_strict)
```

| File                                      | Role                                                                                                                                                                                                                                                                                                                                    |
| ----------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/fork/text/style_rules.rs`  | Stage 10 (Apache-2.0 BetterWispr `StyleFormatter` port): Formal unchanged, Relaxed drops commas (not in numbers or at a line end) and the final period, Very relaxed also lowers sentence starts (not "iPhone", "NASA" or dictionary words), Upbeat makes the final period "!". No `\n` and no word is removed. `is_english` (the gate) |
| `src-tauri/src/fork/context/app_rules.rs` | `app_rules` store (migration 0007), `resolve` (BetterWispr `WritingMode` tiers and site match, GPL), the cached copy the take path reads (`current`, `reload`), auto-send arming per take (`arm_auto_send` / `take_armed`, keyed by the final text, bounded)                                                                            |
| `src-tauri/src/fork/context/apps.rs`      | Installed apps (the usual folders + running apps, cached 60 s) and Finder icons as 64 px PNG `data:` URLs (NSWorkspace, NSBitmapImageRep; cached)                                                                                                                                                                                       |
| `src-tauri/src/fork/commands/style.rs`    | `fork_style_{overview,installed_apps,app_icon,set_app_group,remove_app,set_auto_send}`, `ForkAppRulesChanged`; `on_ready` loads the cache                                                                                                                                                                                               |
| `src/fork/style/`                         | `StylePage` (tabs; the tab is the hub section), `GroupTab` (app chips with a menu: move, remove, back to the usual group, Undo, "Press Enter after dictating"; presets), `PresetCard`, `AppPicker` ("+"), `AppIcon`, `CleanupTab` (S-12a's `CleanupLevelPicker`), `presets.ts` (examples), `useStyle`                                   |

**Groups.** The built-in table is S-09's `categories.json`; the user's
rules override it per app (`style_group` NULL = the built-in group, the row
then only carries auto-send). "Remove from a group" puts the app in Other
(or drops its rule when its built-in group is Other). The page lists the
user's rules, the installed apps of the built-in table and the installed
apps the user dictated into (`entry_meta`); Other also stands for every
app in no group. `entry_meta.style_group` records the group with the
user's rules applied (`TakeContext::new`). Site rules (`kind = 'host'`)
are stored and matched already; S-19b supplies the page's host.

**Presets.** Personal: Formal / Relaxed / Very relaxed; Work, Email,
Other: Formal / Relaxed / Upbeat (validated by F-07's `check_styles`).
Defaults are F-07's: Personal Relaxed, the others Formal. Every preset is
deterministic, so the cleanup prompt stays byte for byte RS-07's for every
style. An Upbeat prompt fragment (exclamation marks on friendly sentences)
was built and measured on the S-12a eval: it caused guard fallbacks and
the Light "language kept" gate fell below 99 %, so it is not shipped
(`prompt::style_fragment` keeps the slot). Relaxed keeps commas inside
brackets and backtick code (the eval's `code-009`). Styles also apply at
level None (they change punctuation and case, never words) and after a
cleanup fallback (on the deterministic text).

**English only (R2 D-26).** The recognizer's language, else the guess from
the user's spoken languages, else Handy's detector; a text too short to
tell counts as English unless the user speaks only other languages.
Cleanup itself works in every language.

**Auto-send (R2 D-61, RV-48).** Off by default, per app. The bar shows
"Sends automatically" in the label's place at the take start (`pill`
`Input::SendsAutomatically`). Return is sent only when the cleanup did not
fall back, the paste was `confirmed` (never `unverified`, never a card),
and the app in front is still the one it was armed for. Handy's global
`auto_submit` keeps its own rule.

**Logs.** `style: <preset> for <group> (applied | no change)` and the
auto-send decision; never the text.

## Accessibility layer and the card (S-05)

R5 §4.6 (W-05) and S-05: where a take's text lands, and the card when it
cannot. **No Handy file is touched**: the check runs inside S-04's
`fork::insert::deliver` (the existing `actions.rs` hook), for every paste
method, and the target captures follow Handy's `recording-state-event`.

| File                                              | What                                                                                                                                                                                                                                                                                                                                               |
| ------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/fork/ax/mod.rs`                    | The API: `on_take_started(id)` (capture + Chromium/Electron lease), `capture(id)` (key-up), `focus_now(captured, budget) -> Option<Focus>`, `on_take_done(id)`, `on_exit()` (leases back)                                                                                                                                                          |
| `src-tauri/src/fork/ax/thread.rs`                 | The one AX thread (`wb-ax`): FIFO jobs, `run(budget, f)` / `post(f)`, 0.2 s messaging timeout per call, app-element cache, lease table, the last 8 captured targets (AX elements stay on this thread)                                                                                                                                              |
| `src-tauri/src/fork/ax/focus.rs`                  | The verdict-sized read (frontmost pid from NSWorkspace, `AXFocusedUIElement`, `AXFocusedWindow`, role, subrole, enabled, `AXEditable`, `AXProtectedContent`, settable `AXValue` / `AXSelectedText`, selected range; never text) and the "same element as at key-up" comparison (`CFEqual`, else same role and frame)                               |
| `src-tauri/src/fork/ax/editable.rs`               | The verdict (OpenWhispr port, MIT; moved from the RS-04 spike): text roles editable, password fields secure, controls and "no window" not editable, containers / web areas / empty Electron trees unknown (fail-open); S-05 adds Gecko (not trusted: fail open) and Finder (desktop and file views: card)                                          |
| `src-tauri/src/fork/ax/element.rs`                | AX wrappers and app helpers (SpeakoFlow port, MIT; moved from the spike); `family_of`, `secure_input_pid` (`kCGSSessionSecureInputPID`) added                                                                                                                                                                                                      |
| `src-tauri/src/fork/ax/lease.rs`, `ax/markers.rs` | `AXManualAccessibility` lease table and text-marker selection (VoiceInk ports, GPL-3.0-only; moved from the spike; markers serve S-21)                                                                                                                                                                                                             |
| `src-tauri/src/fork/insert/target.rs`             | The target rule and the decision table `plan(had_target, focus) -> Route` (pure); the take's start / key-up capture ids, fed by Handy's recording phases and the hotkey engine's auto-stops (DEC-32)                                                                                                                                               |
| `src-tauri/src/fork/insert/{mod,mac}.rs`          | `route_take` before the paste (after the FIFO slot, the cancel and the session check); a card goes to `publish_outcome` → `fork::pill::show_card`; the log line `insert: take N target {route, app, family, role, subrole, verdict, same_as_key_up, ax_ms}` (no text)                                                                              |
| `src-tauri/src/fork/pill/{model,mod}.rs`          | S-07a's card carries `CardContent { text, reason }`: `show_insert_card(reason, text)`, `ForkPillState.card_reason`, `text` = `None` for a password field, Copy from the model's full text (`clipboard::nspb::write_text`, concealed for a password field), the reason as the VoiceOver announcement; Handy's "failed" report never replaces a card |
| `src/fork/pill/states/Card.tsx`, `pill.css`       | S-07a's card; S-05 adds the reason as its label and the hidden text of a password field                                                                                                                                                                                                                                                            |
| `src-tauri/crates/wb-spike-ax/src/ax/mod.rs`      | The RS-04 spike compiles `editable`, `element`, `lease`, `markers` from `fork/ax/` through `#[path]`; `--watch SECS` prints the product verdict for whatever has focus                                                                                                                                                                             |

**Delivery order** (`fork::insert`, per take): S-11's text stages → FIFO
slot → cancelled or session interrupted? (S-04 reports it) → **target
check** (read ≤ 300 ms, else paste: fail-open) → S-04's paste and
verification. The decision table:

| Focus at delivery                                                                                                        | Route                                             | `paste_outcome` |
| ------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------- | --------------- |
| AX did not answer, no permission                                                                                         | paste (fail-open)                                 | as S-04         |
| another app, or another element than at key-up                                                                           | card `focus_moved`                                | `card`          |
| password field (`AXSecureTextField`, `AXProtectedContent`), or Secure Input held by the focused app while AX cannot tell | card `secure_field` (text hidden; Copy concealed) | `refused`       |
| control (button, list, table, link, …), no window, Finder without a text field                                           | card `no_text_field`                              | `card`          |
| text field, terminal, web area, container, empty Electron tree, Gecko                                                    | paste                                             | as S-04         |

**Card behaviour.** One card for S-05's takes that were not pasted (S-07c's
late queue results go only to History and the batch notification); the reason is its label. It stays until ✕, Esc
(observed from the hotkey tap through the Esc registry, never blocked),
Copy ("Copied", then gone) or the next take (the text is in History). A whitespace-only text shows "No speech heard"
instead and copies nothing. Copy replaces the clipboard with plain text and
does not restore it (R5 §4.7); a password field's text is written with only
`org.nspasteboard.ConcealedType`. The text of a password field never
reaches the web view or `ForkInsertOutcome`.

## Apple Foundation Models (S-12b)

Apple's on-device model as a second cleanup engine behind the same
`clean()` (gate, prompt, guard chain, deadlines, fail-open). RS-08
(`docs/fork/spikes/RS-08-apple-fm.md`) decided its role: **fallback only**
(`fork::llm::apple::ROLE`): it cleans a take only when the local model
cannot (not downloaded yet, failed, or the memory rule says no). Handy's
bridge (`swift/apple_intelligence.swift`, `apple_intelligence.rs`,
`build.rs`) is not edited and never called by the fork.

### Files

| File                                                 | Role                                                                                                                                                                                                                                                   |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `crates/wb-apple-fm/swift/fork_apple_fm.swift`       | The Swift bridge (hex port + MacParakeet error mapping + TypeWhisper prewarm): availability codes, context size, language support, prewarm, one guided (`{text}`) or plain generation with greedy sampling, a cancelling timeout, cancel by request id |
| `crates/wb-apple-fm/build.rs`                        | Its own `swiftc` build (`-module-name WbAppleFm`, static lib, weak FoundationModels); without the macOS 26 SDK, full Xcode or Apple Silicon (or `WB_APPLE_FM_STUB=1`) it sets `cfg(wb_apple_fm_stub)` and no Swift is built                            |
| `crates/wb-apple-fm/src/lib.rs`                      | Safe Rust: `availability()` (10 s cache), `complete()`, `cancel()`, `prewarm()`, `supports_language()`; the stub                                                                                                                                       |
| `fork/llm/apple.rs`                                  | `AppleEngine` (the `Engine` of `clean()`: `spawn_blocking`, cancel on drop, reply → `{"text"}` for the guards), `ROLE`, availability for the UI, counters, `WISPRBRO_SELFCHECK_APPLE`                                                                  |
| `fork/llm/post_process.rs` `choose`, `prewarm_apple` | The engine per take; the key-down prewarm with the last take's instructions                                                                                                                                                                            |
| `fork/llm/rs08.rs`                                   | RS-08 latency bench (ignored test)                                                                                                                                                                                                                     |
| `src/fork/settings/advanced/AppleEngine.tsx`         | The Apple Intelligence row of the AI engine panel (debug panel until S-14)                                                                                                                                                                             |

All symbols are `wb_apple_fm_*` (a unit test checks there is no overlap with
Handy's `@_cdecl` names). The bridge returns numeric error codes only, never
model or transcript text.

### One take

`plan()` → `choose(ROLE, ai_chat, local_ready, memory_ok, apple_ok)`:
AI chat → no engine; with `Fallback` the sidecar when its model is ready and
memory allows, else Apple's model when `availability` is `available` and it
supports the take's language (or the hint; unknown counts as supported),
else the S-12a skip reason. `run()` cleans with `AppleEngine` and records
`cleanup_engine = apple`. The request deadline is the same R5 formula as
for the sidecar; Swift cancels the task when it passes, and dropping the
future (take bound, Esc) cancels it by id. A refusal, guardrail, throttling
(`rateLimited`) or unsupported-language error is an `error` outcome with the
"Inserted without cleanup" notice.

**Prewarm.** On Handy's `recording-state-event` `started` (key-down),
`prewarm_apple` prewarms a session with the last Apple take's instructions
(or the level's plain prompt) when Apple's model would clean this take; the
next request with the same instructions uses that session once (Swift
`Prewarmed`), so the instructions are not prefilled after key-up.

**"Try it"** cleans on Apple's model when it would stand in, and says so
("Ran on Apple Intelligence"); the panel shows Apple's availability,
role and counts (`fork_llm_apple_status`).

**Debug-only switches:** `WB_DEBUG_LLM_HOLD_MODEL=1` keeps the local model
"not ready" (never checked or downloaded) so the fallback can be tested;
`WISPRBRO_SELFCHECK_APPLE=1` (or `=local`, expecting the local engine)
runs `plan`+`run` on our own 50-word samples with the app in the
background and logs `fork apple selfcheck: PASS|FAIL …` (the
selfcheck itself is in every build, like `WISPRBRO_SELFCHECK_LLM`).

## Recovery shortcuts (S-06)

R5 S-06 (R2 A-26, A-27, A-28): paste the last dictation again (⌃⌘V), copy
it (⌃⌘C), "Press Enter" on a key or a mouse button (unset by default), and
"Cancelled · Undo" on the bar for 4 s after a cancel. One new Handy hook
(`utils.rs`) and one small upstream-bound method (`managers/audio.rs`).

| File                                                                                    | What                                                                                                                                                                                                                                                                                                                                                          |
| --------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/utils.rs` (hook)                                                         | `cancel_current_operation`: `audio_manager.cancel_recording()` → `crate::fork::recovery::cancel_keeping_audio(app, &audio_manager)`                                                                                                                                                                                                                           |
| `src-tauri/src/managers/audio.rs` (MIT-clean, upstream-bound)                           | `cancel_recording_keeping_samples() -> Option<Vec<f32>>`: Handy's cancel, returning the samples it used to discard; `cancel_recording()` calls it                                                                                                                                                                                                             |
| `src-tauri/src/fork/recovery/mod.rs`                                                    | The hook (product builds only; plain Handy discards as before), the take in flight (named by `fork::take_file_name`, saved by `entry_meta::on_saved`, delivered by `fork::insert`), the silent-cancel note from the hotkey engine, the 4 s timer, Undo (WAV, empty History row, Handy's job queue, delivery), the job listener, the four action registrations |
| `src-tauri/src/fork/recovery/undo.rs`                                                   | The window, pure (SpeakoFlow `dictation_recovery.rs` port, MIT): one offer, kept exactly `UNDO_WINDOW_MS` = 4 000 ms, taken once; the 0.5 s speech minimum; what Undo does (`Plan`); Handy's short-take padding                                                                                                                                               |
| `src-tauri/src/fork/recovery/last.rs`                                                   | The last transcript: newest `entry_meta` row with `source = 'dictation'`, not deleted, with text; `raw` shows `asr_text`, else `inserted_text` (a snippet expanded) or `final_text`; a `refused` take copies concealed                                                                                                                                        |
| `src-tauri/src/fork/shortcut/actions/{paste_last,copy_last,press_enter,undo_cancel}.rs` | The handlers (each spawns its own thread; the engine thread never waits)                                                                                                                                                                                                                                                                                      |
| `src-tauri/src/fork/insert/{mod,mac}.rs`                                                | `deliver_as(text, app, Kind)`: `Take` (the `actions.rs` hook, unchanged behaviour), `Undo { target }` (text stages, auto-submit, focus compared with the capture taken at the cancel), `Again` (paste-last: no stages, no auto-submit, focus now); only `Take` records through the take ledger                                                                |
| `src-tauri/src/fork/insert/target.rs`                                                   | `capture_now()`: the element focused at the cancel                                                                                                                                                                                                                                                                                                            |
| `src-tauri/src/fork/pill/{model,mod}.rs`                                                | `PillMessage::Cancelled` (info, `PillFix::Undo`, 4 s), `NoTranscript`, `CopiedLast`; `Input::Undoing` ("Transcribing…" from the offer); ✕ / Esc on the offer drop the audio; VoiceOver: "Undo is on the dictation bar for 4 seconds."                                                                                                                         |
| `src-tauri/src/fork/settings.rs`                                                        | Action ids `copy_last`, `press_enter`, `undo_cancel`; default `copy_last` = ⌃⌘C; schema v2 (migration v1 → v2 adds `copy_last` to a stored `hotkeys` map)                                                                                                                                                                                                     |
| `src-tauri/src/fork/shortcut/{bindings,labels,mod}.rs`                                  | Press Enter refuses Return / keypad Enter; mouse buttons have spoken labels; the engine host notes silent cancels (no second press, DEC-54 chord, reset)                                                                                                                                                                                                      |
| `src/fork/settings/RecoveryShortcuts.tsx`                                               | The four rows on the debug panel until S-14 (record through Handy's `start_handy_keys_recording` / `handy-keys-event`, Escape cancels, Clear, Use default, the mouse-button note, unusable bindings); `src/fork/pill/states/Message.tsx` labels the Undo button                                                                                               |

**Undo, one take.**

1. A cancel the user asked for (Esc, ✕, a third quick press, `--cancel`)
   reaches `cancel_current_operation`. The hook takes the recorder's samples;
   with ≥ 0.5 s of speech it pads them like Handy, names the WAV now
   (`take_file_name`, which binds the take to its app and day), captures the
   focused element, keeps the take for 4 s and shows "Cancelled · Undo".
   A cancel while transcribing keeps Handy's WAV instead (and the History
   row, if Handy saved it before the cancel landed). The hotkey engine marks
   silent cancels first (`cancel_is_silent`), so a tap with no second press,
   a DEC-54 chord or a reset never offers Undo.
2. After 4 s, on ✕ or Esc on the offer, or when a new take starts, the
   samples are dropped; a WAV no History row points at is deleted.
3. Undo (the bar's button, or `undo_cancel` when bound) takes it out once:
   the WAV is written (or waited for), an empty History row is saved,
   Handy's job queue transcribes it (`retry`: the text pipeline and cleanup
   run as for any retry), and the text is delivered as `Kind::Undo`: into the
   element that had focus at the cancel if it still has focus, else on the
   card (`focus_moved`). The row gets its `paste_outcome` by id.

**Paste-last and copy-last** go through the same delivery as a take
(clipboard snapshot and byte-identical restore, the 3 s hex wait for the
hotkey's modifiers, S-05's target check) but paste the text History shows
as is. Copy-last is an explicit copy: plain text, never restored (§4.7).

**Press Enter** posts Return with no modifiers once the binding's
modifiers are up (≤ 1.5 s). handy-keys never blocks mouse events, so a
mouse-button binding also clicks in the app under the pointer; the settings
say so and suggest a side button.

## Context awareness (S-20)

R5 S-20 (R2 D-58, DEC-24): names near the caret help spell the take, on
by default, all on this Mac. The text is read once at record start and
turned into **spelling hints** (names and terms) on the AX thread; the
text itself never leaves that call. The hints live in memory until the take
ends. One new Handy hook (`managers/transcription.rs`, the recognizer's
vocabulary); no migration (`entry_meta.context_used` exists since S-09).

| File                                                                                        | What                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/fork/context/caret.rs` (GPL-3.0-only, FluidVoice port)                       | The read: exclusions first (Secure Input anywhere, password managers, banking/finance apps and sites, terminals, Wispr Bro itself, secure / numeric / sensitive fields, a browser's address bar), then the caret window (≤ 800 before, ≤ 300 selected, ≤ 400 after) and the lines around the field (its siblings, then each ancestor's other children) up to 1,500 characters in all; 70 ms walk, 50 ms per call, > 100 ms in all → nothing |
| `src-tauri/src/fork/context/exclusions.json`                                                | The lists: FluidVoice's password-manager markers, exact bundle ids, finance markers (a whole bundle-id segment or app-name word), finance hosts (or any host label with "bank"), sensitive field words                                                                                                                                                                                                                                      |
| `src-tauri/src/fork/context/hints.rs`                                                       | Pure: `extract` (name-like words: capitalized and not a sentence's first word, non-ASCII letters, an inner capital, or a first word followed by another name; never stopwords or all caps), `correct` (Handy's custom-word scoring: normalized Levenshtein, Soundex boost, 25 % length rule, 2-word joins; a lowercase word changes only for accents), `relevant`, `vocabulary_terms`                                                       |
| `src-tauri/src/fork/context/awareness.rs`                                                   | The take's slot: queued at record start, armed at key-up (`take_file_name`), read by the three uses, cleared at the take's end (`pill::on_overlay_hide`, a cancel) or after 120 s; `note_used` → pill glyph and `entry_meta.context_used`; the only log line has counts                                                                                                                                                                     |
| `src-tauri/src/managers/transcription.rs` (hook)                                            | `run_setup`: `vocabulary: crate::fork::context::awareness::vocabulary(info.supports_vocabulary, vocabulary_for_model(…))` (Whisper gets the hints after the user's words)                                                                                                                                                                                                                                                                   |
| `src-tauri/src/fork/text/mod.rs`                                                            | Stage `context` after stage 12 (`context_stage`): letters only, never inside a dictionary rule's output (stage 9's offsets follow the edits)                                                                                                                                                                                                                                                                                                |
| `src-tauri/src/fork/llm/{prompt,post_process}.rs`, `prompts/context.txt`                    | `Spec.context` / `Request.context`, a separate CONTEXT fragment (S-19a's style is another field); only hints the transcript has or nearly has; a hint already spelled right joins the guards' keep list; Apple's prewarm prompt never carries them                                                                                                                                                                                          |
| `src-tauri/src/fork/pill/{model,mod}.rs`, `src/fork/pill/states/{ContextGlyph,Working}.tsx` | `Input::ContextUsed`, `ForkPillState.context`: the glyph on "Transcribing…" / "Cleaning up…" of the take that used hints                                                                                                                                                                                                                                                                                                                    |
| `src-tauri/src/fork/{db/entry_meta,commands/history}.rs`                                    | `context_used` written per take, `ForkEntryMeta.context_used` for History (S-15b shows "Context used")                                                                                                                                                                                                                                                                                                                                      |
| `eval/context/names.jsonl`, `src-tauri/src/fork/context/names_eval.rs`, `scripts/wb-s20/`   | The 30-item name set and its eval through the shipped path; the real-Mac drivers                                                                                                                                                                                                                                                                                                                                                            |

**One take.** Record start → `fork::on_record_start` reads the setting
(`context_awareness`) and Handy's spoken languages and posts the read; the
read finishes long before key-up (p95 on this Mac in the S-20 run). Key-up
arms the slot. The recognizer gets the hints as vocabulary when its model
takes one; the text pipeline's `context` stage fixes near misses; the
cleanup prompt names the hints the text has. Any of the three marks the take
used. Handy hides its bar after the paste: the slot is cleared.

## Vibe coding (S-36)

R5 S-36 (R2 D-59, D-60, DEC-41): in VS Code, Cursor and Windsurf, code
names on screen spell the take, and in Cursor's or Windsurf's chat a spoken
visible file name becomes an `@` tag. Off until the user runs the setup in
Settings › Vibe coding. It rides S-20's one read (no new AX call, thread or
hook) and only adds to a read context awareness allows; nothing lists a
directory, opens a project file or asks a shell, index or extension (exit
criterion: AX only). No Handy file, no migration; `VibeCodingSettings`
gains `file_tags_enabled` (serde default, schema stays v6).

| File                                      | What                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/fork/context/ide.rs`       | `Editor` (bundle ids `com.microsoft.VSCode`/`…VSCodeInsiders`, `com.todesktop.230313mzl4w4u92`, `com.exafunction.windsurf`), `access` (both features need `setup_completed` with that editor listed; then `enabled` / `file_tags_enabled`; tags only for editors with a chat), `is_chat_input` (a text input whose labels name a chat/composer/cascade/agent or offer `@` mentions; never search, terminal or the editor buffer; no fail-open), `Identifiers` (exact camelCase, PascalCase, snake_case ASCII tokens, 3-40 chars, ≤ 20 per take), `spoken_segments` (`HTTPServer` → "http server"), `correct` (stage `identifiers`: the exact spoken form becomes the spelling; longest first; ambiguous forms, glued tokens and rule outputs untouched) |
| `src-tauri/src/fork/text/file_tags.rs`    | `FileNames` (names with an extension from AX tab and explorer rows, ≤ 40, paths keep their last part, URLs dropped), `rewrite`/`rewrite_outside` (spoken "x dot y" or written `x.y` → `@x.y` only for a visible name, spelled as shown; never in a path, URL, address, code span or when two spellings collide; idempotent)                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `src-tauri/src/fork/context/caret.rs`     | `IdeScan` on the `LineCollector`: when an editor applies, the walk also scans text for code names (8,000 characters, 1,500 elements, the same time budget) and, when the focus is its chat input, the text under tab and row elements (`AXTab`, `AXRow`, `AXOutlineRow`, `AXCell`, `AXRadioButton`/`AXTabButton`) for file names. Only names leave the read                                                                                                                                                                                                                                                                                                                                                                                             |
| `src-tauri/src/fork/context/awareness.rs` | The slot holds `identifiers` and `file_names` next to the hints; `hints()` puts code names first (same cap), so the recognizer vocabulary, the spelling stage and the cleanup prompt get them too. The log line adds `cursor chat: N code name(s), M file name(s)`: counts and the editor id only                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `src-tauri/src/fork/text/{mod,counts}.rs` | Stages `identifiers` (before `context`) and `file_tags` (after it) in `process`; `after_llm` tags again what a cleanup wrote out (not `finish`, so snippet text stays as saved)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `src/fork/settings/VibeCoding.tsx`        | Settings › Vibe coding: intro, context-off note, Recognize code names (Set up → editors, three steps to turn on Editor: Accessibility Support, confirmation; then the switch and Change editors), Tag files in chat (enabled only with Cursor or Windsurf set up). `tests/fork/vibe-coding.spec.ts`                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `scripts/wb-s36/`                         | The Mac-wave drivers (`setup.sh`, `take.sh`: Cursor in a throwaway profile and workspace)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |

**Decisions.** The setup cannot see an editor's setting without reading its
files, so the user confirms it (the read then simply finds more text); file
tags also need the setup (DEC-41) although tabs are in the AX tree without
screen-reader mode. A chat input is recognized by its labels only.

## Data and Privacy (S-27)

Settings › Data and Privacy (`src/fork/settings/DataPrivacy.tsx`) states the
privacy facts and makes them controllable. Commands live in
`fork/commands/privacy.rs`; all are fork-namespace (`fork_data_locations`,
`fork_show_data_location`, `fork_export_data`, `fork_copy_diagnostics`,
`fork_save_diagnostics`, `fork_erase_all_data`). No Handy file, no
migration, no new setting: retention, context awareness and update checks
bind the existing F-07 fork settings.

- **Page statements are tested.** `privacy::tests::page_statements_match_the_code`
  reads the English strings and checks them against the code: every host in
  `fork::net::ALLOWED_HOSTS`/`_SUFFIXES` is GitHub or Hugging Face, the
  retention defaults (text forever, audio 14 days), update checks default on,
  and the export/delete wording. The rest is covered by the `export`, `erase`
  and `diagnostics::bundle` tests and the egress guard (F-03).
- **Locations.** Data = portable `Data/` or Tauri's app data dir; models =
  `HF_HOME` (DEC-39, under app data) else `<data>/huggingface`; recordings =
  `<data>/recordings`; logs = `fork::app_log_dir` (portable `Data/logs`). In
  portable mode all four sit under `Data/` (E-88, unit-tested). "Show in
  Finder" creates a missing folder, then reveals it (opener plugin).
- **Export (`fork/export.rs`).** A zip written to a sibling temp file, then
  renamed: `manifest.json` (`schema_version` 1, product, app version, time),
  `history.csv` + `history.json` (rows read through `fork::db::handy`, R5
  §4.3: the fork never runs SQL on Handy's `history.db`), a `VACUUM INTO`
  snapshot of `fork.db` (consistent while the app runs), both settings
  stores, and `recordings/` (streamed, symlinks skipped). Models and logs are
  excluded. The file is revealed in Finder.
- **Delete all (`fork/erase.rs`).** Only in a ready product/test identity and
  only with the exact word `DELETE` (not translated: the backend checks the
  same word and it is typeable on every layout). Order, enforced by
  `erase_in_order` (a failed phase stops the list; no relaunch after a
  partial erase): stop recording, transcription jobs (every History id
  cancelled) and model downloads → unload the speech model and stop the
  cleanup sidecar → close both settings stores and clear WebKit storage →
  delete everything in the data dir and the log dir (symlinks unlinked, never
  followed; `$HOME`, relative and symlinked roots refused) except, when models
  are kept, `models/`, `huggingface/` and `llm/verified/` → wait 500 ms for
  WebKit, `app.restart()`. Fresh `fork-settings.json` means onboarding
  `not_started`, so the relaunch shows onboarding. TCC grants and the login
  item are system state and stay (the login item follows the default setting
  at the next launch).
- **Diagnostics (`fork/diagnostics/bundle.rs`).** Reads only `.log`, `.txt`
  and `.ips` files in the log dir (newest 20), each passed through the F-08
  sanitizer. Copy: newest 4 files, last 48 KB each, onto the pasteboard.
  Save: a zip with `diagnostics.txt` plus `logs/<name>` (last 2 MB each).
  Databases, settings, History and recordings are never read. Nothing is
  uploaded.
- **macOS crash reports (`crash.rs`).** At launch (product/test identity
  only; `fork::notify`'s crash producer) `.ips` files under
  `~/Library/Logs/DiagnosticReports` younger than 7 days whose JSON header
  names our bundle id are copied, sanitized, as
  `crash-<time>-ips-<name>.txt` into the log dir (mtime kept, existing copies
  skipped, then pruned to 10). Other apps' reports are never read past the
  header line. The existing next-launch inbox notice then shows them once.
- **Scripted Mac checks:** `scripts/wb-s27/{seed.py,verify.py,zap-paths.sh}`
  with S-25's `ui.swift` driver (MAC-TEST-SCRIPT S-27).

## App menus and tray (S-26a)

R5 S-26a (R2 A-56, A-57, A-59, A-63, C-100, C-101; R4 W-11). No new Handy
hook: the app menu is installed from `fork::on_ready`, and the tray uses the
F-01 `tray.rs` `build_menu` hook that already existed.

| File                                         | What                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| -------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/fork/app_menu.rs`             | One cached `MenuSnapshot` (word total, dictionary words + rules, snippets, a ≤ 40-character one-line preview of the last transcript or "text hidden" for a password-field take, microphones, the user's languages, update availability). Pure `MenuSpec` builders (`dictation_spec`, `vocabulary_spec`, `help_items`, `tray_specs`) describe both menus; later slices add items by id there and in `on_menu`. Native side: Tauri's `Menu::default` with its English labels localized from `rust.menu.*`, Dictation and Vocabulary inserted after Edit, our items in Tauri's Help submenu registered with `set_as_help_menu_for_nsapp()` (macOS adds its search box), Window registered as the windows menu |
| `src-tauri/src/fork/app_menu/languages.json` | Native names for every language code the speech catalog offers (a menu never shows a bare code; test checks the catalog)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `src-tauri/src/fork/tray.rs`                 | `extend` → `app_menu::fill_tray` in product builds: Open Wispr Bro, Check for updates… (Handy's `check_updates`, dropped when forced off), Paste last transcript (disabled until one exists), Shortcuts, Microphone ▸, Languages ▸, Documentation, Quit; Handy's Cancel (during a take) and Secure Input warning keep their ids so Handy's tray handler still runs them                                                                                                                                                                                                                                                                                                                                    |
| `src-tauri/src/fork/commands/audio.rs`       | `choose` (the microphone choice on a worker thread) shared by `fork_mic_choose` and the menus                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `src/fork/hub/menuActions.ts`                | `fork-menu-action` → `navigate(page, section)`; Dictionary/Snippets open their Add dialogs on section `add`, Settings scrolls to `microphone` / `languages`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |

**Live counts.** `fork-stats-updated`, `fork-dictionary-changed`,
`fork-snippets-changed`, `fork-mic-devices-changed`, Handy's
`history-update-payload` and `settings-changed`, History delete/undo/variant
and Handy's microphone/language commands (`observe_handy_command`) schedule
one coalesced worker read (`refresh`); database and device reads never run on
the main thread. When the snapshot changed, the main thread updates the
existing native items in place (text, enabled, checked) and rebuilds a
submenu only when its shape changed. Handy rebuilds its tray only on its own
inputs, so the tray menu the hook last filled is kept (`TrayMenu`) and synced
the same way. A changed app language re-installs the app menu.

**Decisions.** Languages ▸ lists only the user's languages (checked; a click
removes one, never the last) plus "Add or remove languages…" to Settings: the
full ~100-language list and adding from a menu (which can start a model
download) stay in Settings' searchable picker. Help › Documentation opens
`docs/fork/user` in the GitHub repository (C-102: no hosted help center).
Help › Shortcuts / Microphone / Languages and Vocabulary › Add word… / New
snippet… are Hub deep links. The menu preview never shows a concealed take.

## Performance and reliability hardening, part 1 (S-38)

Part 1 covers the slices merged up to S-24 and F-02; part 2 covers the later
slices and the full §5 runs on the reference machines. Measurements are in
`docs/fork/PERF.md`.

**Hidden windows are parked** (`fork/hub/park.rs`, §5.2). Handy closes the
main window by hiding it, a login-item start builds it hidden, and the
Scratchpad window is hidden the same way, so each kept a WebContent process
for the whole session (the Hub's ≈ 106 MB RSS was most of S-03's 287-293 MB
against the 250 MB ceiling). A parkable window (`main`, `fork-scratchpad`)
that stays hidden for `PARK_AFTER` (60 s) is destroyed:

```
CloseRequested (Handy hides) ─► schedule(label): generation += 1, timer 60 s
                                   │
    shown / ensure_main ─► generation += 1 (the timer's park is stale)
                                   ▼
park(label, generation) on the main thread: same generation, window exists,
not visible, not minimized, and for `main` its page reported ready
  ─► main: keep frame (logical), main_parked = true, tray theme override
  ─► window.destroy()  (its WebContent process exits)
```

- **Rebuild.** `ensure_main` is the first line of Handy's `show_main_window`
  (the one `lib.rs` hook; tray Settings, Dock click, second launch, the
  command) and of the fork's show paths (`show_main_then`: notification Open,
  View changes, the S-26a menu deep links). It replays Handy's builder (label `main`, `/`, Handy's
  size and minimum, resizable, hidden, portable `data_directory`), then
  `R::hub_apply_theme` (Handy's `apply_window_theme`), `hub::style_main`
  (the S-13 chrome) and the parked frame. The Scratchpad's own `open` already
  builds its window when there is none; it now calls `park::shown` and
  `park::watch`.
- **Events for a new page.** `fork_hub_ready` (from
  `useNotificationPageOpen`, after its listener, the View-changes and the
  menu-action listeners are registered: `src/fork/hub/ready.ts`) marks the page ready and
  runs what `show_main_then` queued. A notification click or ⌥O therefore
  reaches a freshly built Hub; before S-38 an event sent while the page was
  still loading was lost.
- **Start-up work.** Handy's `App.tsx` initializes Enigo and the shortcuts
  from the page, so `main` is parked only after its page reported ready; a
  Hub that started hidden is scheduled from `fork_hub_ready`.
- **Exit.** With every Tauri window destroyed, Tauri sends `ExitRequested`
  without a code. The fork plugin prevents it while the Hub is parked and
  skips its quit-time work; a real quit (tray, ⌘Q, updater restart) always
  has a code.
- **Tray glyph.** Handy's `get_current_theme` reads the main window's theme
  and falls back to dark without one. While parked, `fork::tray::icon_path`
  uses Handy's appearance setting, or the macOS appearance
  (`AppleInterfaceStyle`) for "System".
- macOS only; `WB_DEBUG_PARK_AFTER_S` (debug builds) sets the delay or `off`.
  Not covered: Handy's own update checker in the page (S-28 moves update
  checks to Rust); a system appearance change while parked updates the tray
  glyph only on the next tray refresh.

**Flaky tests fixed** (CONTEXT execution protocol: fix, don't re-run):

| Test                                                                                                           | Cause                                                                                                                                                                                                                                                       | Fix                                                                                                               |
| -------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `fork::db::tests::wal_allows_a_writer_and_two_readers_without_busy_errors`                                     | Readers opened their connections while the writer was mid-burst; a fresh connection's first read of the WAL index can rebuild it under the WAL write lock, and on a loaded Mac the writer's 5 s busy wait ran out (1 failure in ~20 loaded full-suite runs) | Readers open and read once before a barrier releases all four threads; the app opens its connection at start too  |
| Handy `clipboard::tests::external_script_does_not_wait_for_inherited_stdio`                                    | 1 s wait for a script whose background child lived 3 s; starting `sh` alone took longer under load                                                                                                                                                          | Child lives 60 s, wait 20 s, child killed after (`ci/hooks.txt` entry)                                            |
| `fork::stats::voice` full profile, `fork::text::snippets` 1,000 snippets, `fork::db::history_list` 10,000 rows | Wall-clock speed bounds fail when the thread waits for a core                                                                                                                                                                                               | `testutil::CpuTimer` (thread CPU time)                                                                            |
| `tests/fork/{pill,pill-card,pill-context,recovery}.spec.ts` RTL                                                | The test wrote `dir="rtl"`; the page's own language effect could run later and put `ltr` back                                                                                                                                                               | Ask for an RTL app language (`get_app_settings`) and wait for the page's `dir`/`lang` (`tests/fork/pill-page.ts`) |
| `pill.spec.ts` axe in every state                                                                              | One test looped over 48 page loads, past its 120 s budget when busy                                                                                                                                                                                         | One test per state and theme                                                                                      |

**Manifest.** The fork dependency block had landed under
`[target.'cfg(unix)'.dependencies]` (after `signal-hook`); it is now part of
`[dependencies]`, so the fork compiles for every target.

**Harness** (`scripts/wb-soak/`, R5 S-38): `idle.py` samples the app's and
its WebKit processes' RSS and the app's CPU (the S-03 `procs` method) and
prints a §5.2 verdict; `soak.py` is the exactly-once soak (CLI
`--toggle-transcription`, WAVs into a loopback input with `play.swift`, AX
read-back with `axread.swift`, clipboard digest from `wb-s05/clip.swift`,
card from `wb-s03-pill.swift`; outcome per take: confirmed / duplicate /
card / wrong_target / lost).

## Tests

| What                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | Where                                                                                                                                                                                                                                                                                               |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Init order, `NotReady` before Ready, Handy state after                                                                                                                                                                                                                                                                                                                                                                                                                                                                | `fork/testutil.rs` `model_status_is_not_ready_before_ready_and_handy_state_after`                                                                                                                                                                                                                   |
| Handy command still resolves through `compose_invoke`                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | `handy_command_still_resolves_through_composed_handler`                                                                                                                                                                                                                                             |
| Fork event round trip through the shared registry                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | `fork_event_round_trips_through_mount_events`                                                                                                                                                                                                                                                       |
| Headless modes skip `on_ready`, no fork windows                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | `headless_cli_mode_skips_on_ready_and_opens_no_windows`                                                                                                                                                                                                                                             |
| No Handy `fork_*` command; one `invoke_handler`; plain `build.rs`                                                                                                                                                                                                                                                                                                                                                                                                                                                     | `no_handy_command_name_starts_with_fork_prefix`, `compose_invoke_is_the_only_invoke_handler`, `build_rs_still_ends_in_plain_tauri_build`                                                                                                                                                            |
| Fork bindings current; Handy bindings untouched                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | `fork_bindings_are_current_and_handy_bindings_untouched`                                                                                                                                                                                                                                            |
| UI smoke through mocked IPC                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           | `tests/fork/debug-panel.spec.ts` (`bunx playwright test -c playwright.fork.config.ts`)                                                                                                                                                                                                              |
| Identity, version lines, E-94, hook placement (F-01)                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | `fork::identity_tests` in `fork/mod.rs`                                                                                                                                                                                                                                                             |
| `rust.*` strings in 26 locales, no "Handy"; tray label/icons                                                                                                                                                                                                                                                                                                                                                                                                                                                          | `fork::i18n::tests`, `fork::tray::tests`                                                                                                                                                                                                                                                            |
| S-26a menu structure (Edit kept, Help registered), tray without model items, disabled states, live counts and preview, concealed takes, 3-locale strings, native language names; menu deep links (axe light/dark, RTL, keyboard)                                                                                                                                                                                                                                                                                      | `fork::app_menu::tests`, `tests/fork/menus.spec.ts`; real Mac: `scripts/wb-s26a/`                                                                                                                                                                                                                   |
| Fork settings: defaults, migrations, salvage, validation, ENFORCED/FIRST_RUN, commands + event on MockRuntime                                                                                                                                                                                                                                                                                                                                                                                                         | `fork::settings::tests` (`src-tauri/src/fork/settings/tests.rs`), `tests/fork/fork-settings.spec.ts`                                                                                                                                                                                                |
| Brand overrides in 26 locales                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         | `scripts/wb-brand-overrides.test.ts` (`bun run wb:test:scripts`), `tests/fork/brand-overrides.spec.ts`                                                                                                                                                                                              |
| F-10 fakes (clock, AX, pasteboard, coordinator, network)                                                                                                                                                                                                                                                                                                                                                                                                                                                              | `fork/testutil.rs` `fake_tests` (traits are test-only until their owning slice moves them; see the module doc)                                                                                                                                                                                      |
| Playwright IPC fixture (commands and events)                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | `tests/fork/fixtures.ts`, self-tests `tests/fork/ipc-fixture.spec.ts`, usage `tests/fork/README.md` (`bun run wb:test:ui`)                                                                                                                                                                          |
| Cleanup eval metrics, guard stub, item labels                                                                                                                                                                                                                                                                                                                                                                                                                                                                         | `scripts/wb-eval-cleanup.test.ts`; runner `bun run wb:eval:cleanup` (`eval/cleanup/README.md`)                                                                                                                                                                                                      |
| RS-07 bench harness (prompt provenance, request shapes, loopback-only spawn)                                                                                                                                                                                                                                                                                                                                                                                                                                          | `scripts/wb-bench-cleanup.test.ts`; report `docs/fork/spikes/RS-07-cleanup-bakeoff.md`                                                                                                                                                                                                              |
| S-10 LLM runtime: supervisor (fake HTTP server on the real socket/TCP), spawn line, env scrub, watchdog, stale PID, transport, streamed client + abort, download verification, memory planner, provider routing                                                                                                                                                                                                                                                                                                       | `fork::llm::*` tests; real sidecar: ignored `real_*` and `rs06_bench` (`docs/fork/spikes/RS-06-llm-runtime.md`); UI `tests/fork/ai-engine.spec.ts`; translation backend `scripts/wb-translate-llama.test.ts`                                                                                        |
| S-16 dictionary: validation, duplicates / conflicts, suggestions, undo, words vs replacements routing, Handy migration and mirror, migration 0004, CSV read / review / commit / export round trip; page, dialog, wizard, drop routing, axe                                                                                                                                                                                                                                                                            | `fork::dictionary::tests`; `tests/fork/dictionary.spec.ts`                                                                                                                                                                                                                                          |
| S-15a home: greeting sources and name editor, rail against a fixture, live refresh, event bursts, `NotReady` retry, voice profile, banner, layout, RTL, axe light/dark; account-name parsing                                                                                                                                                                                                                                                                                                                          | `tests/fork/home.spec.ts`; `fork::commands::stats::tests`                                                                                                                                                                                                                                           |
| S-05 AX layer and card: decision table with a fake AX provider (editable, secure, no window, desktop/Finder, terminal, Electron empty tree → fail-open, Gecko, focus moved before key-up → paste there, after → card), capture bookkeeping incl. DEC-32 auto-stop, lease timing, verdict, card model (stays, Copy → Copied, Esc and ✕, never during a take), state event hides a password field's text, refused outcome never broadcasts text; UI every reason, hidden text, Copy, ✕, keyboard, RTL, axe light/dark   | `fork::insert::target::tests`, `fork::ax::{editable,lease,element}::tests`, `fork::pill::model::tests`, `fork::pill::tests`, `fork::insert::tests`; `tests/fork/pill-card.spec.ts`; real Mac `scripts/wb-s05/` (MAC-TEST-SCRIPT S-05)                                                               |
| S-06 recovery: the undo window (exactly 4 s, taken once, newer replaces, leftovers), the 0.5 s minimum, padding; silent-cancel notes; the take in flight; job events; the last-transcript query (dictation only, failed/empty/deleted skipped, shown variant, inserted text, concealed); bindings (defaults, Press Enter never Return, mouse buttons, conflicts); a mouse-button binding fires in the engine; labels; settings migration v1 → v2; pill offer, Undoing, messages; the take's `finish` still runs first | `fork::recovery::{tests,undo::tests,last::tests}`, `fork::shortcut::tests::{bindings,engine,labels,actions}`, `fork::settings::tests`, `fork::pill::model::tests`, `fork::text::tests`; `tests/fork/recovery.spec.ts`, `tests/fork/pill.spec.ts`; real Mac `scripts/wb-s06/` (MAC-TEST-SCRIPT S-06) |
| S-38 parking: generations, visible/minimized/not-ready windows kept, rebuild with frame, deliveries wait for the page, codeless exit refused only while parked, `lib.rs` hook; tray glyph while parked                                                                                                                                                                                                                                                                                                                | `fork::hub::park::tests`, `fork::tray::tests`                                                                                                                                                                                                                                                       |
| S-18 notifications: macOS only for the four kinds and only while away, no permission request before the first real need, toggles filter creation, inbox CRUD and dedupe, milestones/tips, clicks and buttons routed (Hub and macOS), Esc closes toasts; UI list/filter/menu/toggles/toasts/popup/keyboard/axe                                                                                                                                                                                                         | `fork::notify::tests`, `fork::notify::macos::tests`; `tests/fork/notifications.spec.ts`                                                                                                                                                                                                             |

**Test limits.** Handy's real commands and managers are typed to `tauri::Wry`
and a Wry event loop cannot start inside `cargo test` on macOS (main thread).
The Rust tests therefore run the real fork plugin and `compose_invoke` on
`MockRuntime` with stand-ins for Handy's setup closure, model manager
(`testutil::FakeModelManager`, swapped in through the `HandyModelManager` type
alias under `cfg(test)`) and one Handy command. The real wiring is covered by
`cargo build` (the real `compose_invoke(invoke_handler)` must type-check against
Handy's handler), the source checks above, and running the app.

Note: Handy's `playwright.config.ts` (`testDir: ./tests`) also discovers
`tests/fork/`; those specs pass under either config.
