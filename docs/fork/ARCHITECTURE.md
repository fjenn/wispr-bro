# Wispr Bro fork architecture

How the fork layer plugs into Handy. This is the F-05 baseline; later slices add
modules under the same namespace but must keep these rules true. The plan is
`docs/research/R5-prd-and-slices.md` §4.1–§4.2.

Line references are for **tauri 2.11.5**, **tauri-specta 2.0.0-rc.21**
(`tauri-specta-macros` 2.0.0-rc.16), and Handy's files as of this fork's branch
point. They were read from `~/.cargo/registry/src/*/tauri-2.11.5/` and friends.
Re-check them when Tauri or tauri-specta is upgraded.

## Where fork code lives

| Layer          | Location                                                                                    |
| -------------- | ------------------------------------------------------------------------------------------- |
| Rust           | `src-tauri/src/fork/` (`mod.rs`, `events.rs`, `commands/`, `testutil.rs` under `cfg(test)`) |
| React          | `src/fork/` (`index.ts`, `i18n.ts`, `bindings.ts` (generated), `debug/`, `ui/`)             |
| Strings        | `src/i18n/locales/<lng>/fork.json` (+ `fork.flags.json` sidecars)                           |
| Capabilities   | `src-tauri/capabilities/fork.json`                                                          |
| Config overlay | `src-tauri/tauri.fork.conf.json` (empty until F-01)                                         |
| Scripts        | `scripts/wb-*.ts`                                                                           |
| UI tests       | `tests/fork/`, `playwright.fork.config.ts`                                                  |

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
