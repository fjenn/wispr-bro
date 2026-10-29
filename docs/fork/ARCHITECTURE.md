# Wispr Bro fork architecture

How the fork layer plugs into Handy. This is the F-05 baseline; later slices add
modules under the same namespace but must keep these rules true. The plan is
`docs/research/R5-prd-and-slices.md` §4.1–§4.2.

Line references are for **tauri 2.11.5**, **tauri-specta 2.0.0-rc.21**
(`tauri-specta-macros` 2.0.0-rc.16), and Handy's files as of this fork's branch
point. They were read from `~/.cargo/registry/src/*/tauri-2.11.5/` and friends.
Re-check them when Tauri or tauri-specta is upgraded.

## Where fork code lives

| Layer          | Location                                                                                                                                 |
| -------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| Rust           | `src-tauri/src/fork/` (`mod.rs`, `events.rs`, `i18n.rs`, `tray.rs`, `commands/` (incl. `selfcheck.rs`), `testutil.rs` under `cfg(test)`) |
| React          | `src/fork/` (`index.ts`, `i18n.ts`, `bindings.ts` (generated), `debug/`, `ui/`)                                                          |
| Strings        | `src/i18n/locales/<lng>/fork.json` (+ `fork.flags.json` sidecars)                                                                        |
| Capabilities   | `src-tauri/capabilities/fork.json`                                                                                                       |
| Config overlay | `src-tauri/tauri.fork.conf.json` (identity, F-01)                                                                                        |
| Icons          | `src-tauri/icons-fork/` (app, + `src/*.svg` sources), `src-tauri/resources/fork/tray/` (tray)                                            |
| Scripts        | `scripts/wb-*.ts`                                                                                                                        |
| UI tests       | `tests/fork/`, `playwright.fork.config.ts`                                                                                               |

Every fork source file starts with `// SPDX-License-Identifier: GPL-3.0-or-later`.
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
  object exists. Empty in F-05; F-08 fills it (`HF_HOME`, panic hook).

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

Not set yet: `bundle.externalBin` (S-10), `app.security` CSP (F-08),
`license`, Linux `/usr/lib/Handy` paths and the Windows `signCommand` (F-02/F-04,
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
`X-Title: Handy` LLM header (`llm_client.rs:195`, F-08/S-10), the
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
- **Deviation (debug host mount).** F-05 has no `App.tsx` hook (that is S-13).
  So `registerForkI18n` in `src/fork/index.ts` also starts the fork frontend:
  in the `main` webview only, it lazy-loads `src/fork/debug/mount.tsx`, which
  renders a hidden host into its own root (`#fork-debug-root`). Other webviews
  (Handy's overlay) skip it, and the panel code is a separate chunk. S-13 moves
  this into `ForkApp` and removes the side effect.
- **Debug panel** (`src/fork/debug/DebugPanel.tsx`): opened by the same chord
  as Handy's debug-mode toggle (⌘⇧D / Ctrl+Shift+D; `App.tsx` handler) or by
  the route `#/fork/debug`, but only when `fork_ping` reports
  `debug_build` (`cfg(debug_assertions)`) or `cli_debug` (`--debug`). Handy's
  `--debug` override is runtime-only and not visible through
  `get_app_settings`, hence the flag on `fork_ping`. It shows `fork_ping`,
  `fork_model_status` and the last `ForkPingEvent`; later slices mount early
  panels with `registerForkDebugSection`.
- Fork components use `useTranslation("fork")`.

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

## Tests

| What                                                              | Where                                                                                                                                    |
| ----------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| Init order, `NotReady` before Ready, Handy state after            | `fork/testutil.rs` `model_status_is_not_ready_before_ready_and_handy_state_after`                                                        |
| Handy command still resolves through `compose_invoke`             | `handy_command_still_resolves_through_composed_handler`                                                                                  |
| Fork event round trip through the shared registry                 | `fork_event_round_trips_through_mount_events`                                                                                            |
| Headless modes skip `on_ready`, no fork windows                   | `headless_cli_mode_skips_on_ready_and_opens_no_windows`                                                                                  |
| No Handy `fork_*` command; one `invoke_handler`; plain `build.rs` | `no_handy_command_name_starts_with_fork_prefix`, `compose_invoke_is_the_only_invoke_handler`, `build_rs_still_ends_in_plain_tauri_build` |
| Fork bindings current; Handy bindings untouched                   | `fork_bindings_are_current_and_handy_bindings_untouched`                                                                                 |
| UI smoke through mocked IPC                                       | `tests/fork/debug-panel.spec.ts` (`bunx playwright test -c playwright.fork.config.ts`)                                                   |
| Identity, version lines, E-94, hook placement (F-01)              | `fork::identity_tests` in `fork/mod.rs`                                                                                                  |
| `rust.*` strings in 26 locales, no "Handy"; tray label/icons      | `fork::i18n::tests`, `fork::tray::tests`                                                                                                 |
| Brand overrides in 26 locales                                     | `scripts/wb-brand-overrides.test.ts` (`bun run wb:test:scripts`), `tests/fork/brand-overrides.spec.ts`                                   |

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
