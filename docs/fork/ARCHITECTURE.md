# Wispr Bro fork architecture

How the fork layer plugs into Handy. This is the F-05 baseline; later slices add
modules under the same namespace but must keep these rules true. The plan is
`docs/research/R5-prd-and-slices.md` §4.1–§4.2.

Line references are for **tauri 2.11.5**, **tauri-specta 2.0.0-rc.21**
(`tauri-specta-macros` 2.0.0-rc.16), and Handy's files as of this fork's branch
point. They were read from `~/.cargo/registry/src/*/tauri-2.11.5/` and friends.
Re-check them when Tauri or tauri-specta is upgraded.

## Where fork code lives

| Layer          | Location                                                                                                                                                                                                                                                  |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Rust           | `src-tauri/src/fork/` (`mod.rs`, `events.rs`, `i18n.rs`, `tray.rs`, `net.rs`, `settings.rs`, `commands/` (incl. `selfcheck.rs`, `csp.rs`, `history.rs`), `db/`, `context/`, `stats/`, `llm/`, `diagnostics/`, `models/`, `testutil.rs` under `cfg(test)`) |
| React          | `src/fork/` (`index.ts`, `i18n.ts`, `bindings.ts` (generated), `debug/`, `ui/`)                                                                                                                                                                           |
| Strings        | `src/i18n/locales/<lng>/fork.json` (+ `fork.flags.json` sidecars)                                                                                                                                                                                         |
| Capabilities   | `src-tauri/capabilities/fork.json`                                                                                                                                                                                                                        |
| Config overlay | `src-tauri/tauri.fork.conf.json` (identity, F-01)                                                                                                                                                                                                         |
| Icons          | `src-tauri/icons-fork/` (app, + `src/*.svg` sources), `src-tauri/resources/fork/tray/` (tray)                                                                                                                                                             |
| Scripts        | `scripts/wb-*.ts`                                                                                                                                                                                                                                         |
| UI tests       | `tests/fork/`, `playwright.fork.config.ts`                                                                                                                                                                                                                |
| Cleanup eval   | `eval/cleanup/` (items, README, `prompts/` (RS-07 upstream prompts, MIT), gitignored `reports/`), `scripts/wb-eval-cleanup.ts`, RS-07 bench `scripts/wb-bench-cleanup.ts`                                                                                 |
| Manual tests   | `docs/fork/MAC-TEST-SCRIPT.md`                                                                                                                                                                                                                            |
| CI guards      | `scripts/wb-guard*.ts`, `ci/`, `.github/workflows/wb-*.yml`, `about.toml`                                                                                                                                                                                 |

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

| Key                                  | Value                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `productName`                        | `Wispr Bro` (also names Tauri's default macOS app menu)                                                                                                                                                                                                                                                                                                                                                                |
| `identifier`                         | `com.wisprbro.desktop` (data, logs, TCC are separate from Handy)                                                                                                                                                                                                                                                                                                                                                       |
| `version`                            | `1.0.0-alpha.1` = `Cargo.toml` = `package.json` (DEC-48)                                                                                                                                                                                                                                                                                                                                                               |
| `bundle.icon`                        | `icons-fork/{32x32,128x128,128x128@2x}.png`, `icon.icns`, `icon.ico`                                                                                                                                                                                                                                                                                                                                                   |
| `bundle.macOS.minimumSystemVersion`  | `13.0` (DEC-22)                                                                                                                                                                                                                                                                                                                                                                                                        |
| `plugins.updater.pubkey`/`endpoints` | **TODO F-02** placeholders `TODO-F02-<owner>-updater-pubkey`, `https://github.com/<owner>/wispr-bro/releases/latest/download/latest.json`. Deliberately _not_ Handy's feed/key, so a Wispr Bro build can never install a Handy update. The plugin parses endpoints at startup (the `<owner>` URL parses; `<`/`>` are percent-encoded) and reads the key only when verifying a download, so the build and startup work. |

`bundle.externalBin`: `target/wb-sidecar/llama-server` (S-10, below). Not set yet: `license`, Linux `/usr/lib/Handy` paths and the Windows `signCommand` (F-02/F-04,
not M1 macOS).

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
`core:default`, `store:default`, `dialog:default`, `opener:default`
(`notification:default` arrives with S-18). Never `*`. Fork commands need no
permission (see ACL above).

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
  dir. Nothing is uploaded; S-27 reads the files.

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

## CI guards (F-03)

`bun run wb:guards` runs every source guard (R5 §4.12); add `-- --release`
for release mode, `--base <ref>` for another upstream ref, `--binary <path>`
to also scan a built app. Each guard also runs alone (`wb:guard:<name>`).
Shared code: `scripts/wb-guard-lib.ts` (file listing, `FORK_PATHS`, list
parsing, URL/host extraction). Self-tests: `scripts/wb-guards.test.ts`
(`bun run wb:test:guards`, also part of `wb:test:scripts`).

| Guard                      | Checks                                                                                                                                                                                                                                                                                                                  | Config                                                                         |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| `wb-guard-identity.ts`     | `tauri.conf.json` + overlay (RFC 7396 merge) has no `com.pais.handy`, `cjpais/Handy` or Handy pubkey prefix; productName/identifier ours; pubkey and a `github.com/<owner>/wispr-bro` endpoint present; Cargo/package/overlay versions (and tag) agree; `--release`: no F-02 placeholders                               | —                                                                              |
| `wb-guard-plumbing.ts`     | `build.rs` `fn main` ends in `tauri_build::build()`, no `AppManifest`/`InlinedPlugin`; no Handy `fork_*` command; one `.invoke_handler(fork::compose_invoke(invoke_handler))`; upstream-bound files are fork-free and MIT-clean                                                                                         | `ci/upstream-bound.txt`                                                        |
| `wb-guard-hooks.ts`        | `git diff --numstat` vs `merge-base(HEAD, upstream/main)`: every changed upstream file listed and within budget; generated lockfiles (`lock` entries) may change only with their manifest changed and within budget; `sha256:` entries (only `LICENSE`) must hash to the pinned text; new files only under `FORK_PATHS` | `ci/hooks.txt`                                                                 |
| `wb-guard-hosts.ts`        | URL hosts in fork code ⊆ allowed (`[link]` only in `openUrl(…)`, `[schema]` only as `"$schema"`, `[test]` only in fork test code); hosts in upstream code ⊆ baseline ∪ inert ∪ allowed (never `[test]`); list entries carry checked comments                                                                            | `ci/allowed-hosts.txt`, `ci/upstream-hosts-baseline.txt`, `ci/inert-hosts.txt` |
| `wb-guard-bundle-hosts.ts` | hosts in `dist/**/*.{js,css,html}` and Mach-O strings ⊆ allowed (except `[test]`) ∪ inert; skips without a build                                                                                                                                                                                                        | same                                                                           |
| `wb-guard-telemetry.ts`    | whole-name match of banned packages in `Cargo.lock`, `Cargo.toml`, `bun.lock`, `package.json`, TS/JS imports and Rust `use`                                                                                                                                                                                             | `ci/telemetry-packages.txt`                                                    |
| `wb-guard-spdx.ts`         | SPDX header in fork code; AGPL files listed in `NOTICE` (§4, since F-04); no GPL/AGPL header outside fork paths                                                                                                                                                                                                         | `ci/generated.txt`, `NOTICE`                                                   |

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

### Signing (F-02 slots in here)

- `build.sh` signs ad hoc with `--options runtime` and the identifier
  `com.wisprbro.desktop.llama-server`. RS-06: Metal works under the hardened
  runtime with no entitlement (no `allow-jit` / `allow-unsigned-executable-memory`).
- `tauri build` re-signs every `externalBin` with the app's identity, the
  hardened runtime and the app's `Entitlements.plist` (verified on the ad-hoc
  debug bundle: `flags=adhoc,runtime`, entitlements = microphone). With the
  Developer ID in `APPLE_SIGNING_IDENTITY`, F-02 gets a Developer-ID-signed,
  timestamped sidecar for notarization with no change here. The sidecar does
  not need the microphone entitlement; Tauri 2.11 has no per-binary
  entitlements, which is harmless (TCC attributes it to the app).
- Gatekeeper (`spctl --assess`) rejects the ad-hoc binary, as expected; the
  notarized check is S-10.1 in the Mac test script.

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

## Tests

| What                                                                                                                                                                                                            | Where                                                                                                                                                                                                        |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Init order, `NotReady` before Ready, Handy state after                                                                                                                                                          | `fork/testutil.rs` `model_status_is_not_ready_before_ready_and_handy_state_after`                                                                                                                            |
| Handy command still resolves through `compose_invoke`                                                                                                                                                           | `handy_command_still_resolves_through_composed_handler`                                                                                                                                                      |
| Fork event round trip through the shared registry                                                                                                                                                               | `fork_event_round_trips_through_mount_events`                                                                                                                                                                |
| Headless modes skip `on_ready`, no fork windows                                                                                                                                                                 | `headless_cli_mode_skips_on_ready_and_opens_no_windows`                                                                                                                                                      |
| No Handy `fork_*` command; one `invoke_handler`; plain `build.rs`                                                                                                                                               | `no_handy_command_name_starts_with_fork_prefix`, `compose_invoke_is_the_only_invoke_handler`, `build_rs_still_ends_in_plain_tauri_build`                                                                     |
| Fork bindings current; Handy bindings untouched                                                                                                                                                                 | `fork_bindings_are_current_and_handy_bindings_untouched`                                                                                                                                                     |
| UI smoke through mocked IPC                                                                                                                                                                                     | `tests/fork/debug-panel.spec.ts` (`bunx playwright test -c playwright.fork.config.ts`)                                                                                                                       |
| Identity, version lines, E-94, hook placement (F-01)                                                                                                                                                            | `fork::identity_tests` in `fork/mod.rs`                                                                                                                                                                      |
| `rust.*` strings in 26 locales, no "Handy"; tray label/icons                                                                                                                                                    | `fork::i18n::tests`, `fork::tray::tests`                                                                                                                                                                     |
| Fork settings: defaults, migrations, salvage, validation, ENFORCED/FIRST_RUN, commands + event on MockRuntime                                                                                                   | `fork::settings::tests` (`src-tauri/src/fork/settings/tests.rs`), `tests/fork/fork-settings.spec.ts`                                                                                                         |
| Brand overrides in 26 locales                                                                                                                                                                                   | `scripts/wb-brand-overrides.test.ts` (`bun run wb:test:scripts`), `tests/fork/brand-overrides.spec.ts`                                                                                                       |
| F-10 fakes (clock, AX, pasteboard, coordinator, network)                                                                                                                                                        | `fork/testutil.rs` `fake_tests` (traits are test-only until their owning slice moves them; see the module doc)                                                                                               |
| Playwright IPC fixture (commands and events)                                                                                                                                                                    | `tests/fork/fixtures.ts`, self-tests `tests/fork/ipc-fixture.spec.ts`, usage `tests/fork/README.md` (`bun run wb:test:ui`)                                                                                   |
| Cleanup eval metrics, guard stub, item labels                                                                                                                                                                   | `scripts/wb-eval-cleanup.test.ts`; runner `bun run wb:eval:cleanup` (`eval/cleanup/README.md`)                                                                                                               |
| RS-07 bench harness (prompt provenance, request shapes, loopback-only spawn)                                                                                                                                    | `scripts/wb-bench-cleanup.test.ts`; report `docs/fork/spikes/RS-07-cleanup-bakeoff.md`                                                                                                                       |
| S-10 LLM runtime: supervisor (fake HTTP server on the real socket/TCP), spawn line, env scrub, watchdog, stale PID, transport, streamed client + abort, download verification, memory planner, provider routing | `fork::llm::*` tests; real sidecar: ignored `real_*` and `rs06_bench` (`docs/fork/spikes/RS-06-llm-runtime.md`); UI `tests/fork/ai-engine.spec.ts`; translation backend `scripts/wb-translate-llama.test.ts` |
| S-15a home: greeting sources and name editor, rail against a fixture, live refresh, event bursts, `NotReady` retry, voice profile, banner, layout, RTL, axe light/dark; account-name parsing                    | `tests/fork/home.spec.ts`; `fork::commands::stats::tests`                                                                                                                                                    |

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
