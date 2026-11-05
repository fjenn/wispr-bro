# Fork UI tests (`tests/fork/`)

Playwright specs for Wispr Bro's fork UI, run against the Vite dev server
with a **mocked Tauri IPC** (no Rust backend). R5 §11.2.

```bash
bun run wb:test:ui                       # = playwright test -c playwright.fork.config.ts
WB_PW_CHANNEL=chrome bun run wb:test:ui  # use the installed Chrome instead of Playwright's download
bun run wb:test:ui -- --ui               # Playwright UI mode
```

Handy's own `playwright.config.ts` (`testDir: ./tests`) also discovers these
specs; they pass under either config. It is not edited (R5 §4.2).

## Files

| File                      | What                                                                              |
| ------------------------- | --------------------------------------------------------------------------------- |
| `fixtures.ts`             | The `test` fixture with `ipc` (use this in every new spec)                        |
| `ipc-mock.ts`             | The in-page mock the fixture installs (`installTauriMock`); usable on its own too |
| `ipc-fixture.spec.ts`     | Self-tests of the fixture on a bare page (no app code)                            |
| `debug-panel.spec.ts`     | Fork debug panel (F-05), written with the fixture                                 |
| `brand-overrides.spec.ts` | Brand overrides (F-01), uses `installTauriMock` directly                          |
| `hub-fixture.ts`          | `openHub` for a returning user, mocked settings, stats and account name (S-13)    |
| `home.spec.ts`            | Dictation home (S-15a): greeting, stats rail, live refresh, voice profile         |
| `dictionary.spec.ts`      | Dictionary (S-16): page, Add word dialog, import wizard, drops, export, axe       |
| `first-use.spec.ts`       | First-use chooser, bar spotlight page, setup checklist (S-30)                     |

## Writing a spec

```ts
// SPDX-License-Identifier: GPL-3.0-or-later
import { test, expect, ipcError, NOT_READY } from "./fixtures";

test("pill shows listening", async ({ page, ipc }) => {
  // 1. Register replies before or after navigation (both work).
  ipc.respond({ fork_get_settings: { cleanup_level: "light" } });
  ipc.respond("fork_model_status", NOT_READY); // reject with ForkError
  ipc.handle("fork_set_setting", (args) => {
    // compute a reply
    return args.key === "cleanup_level"
      ? null
      : ipcError({ kind: "Internal", message: "bad key" });
  });
  ipc.respondSequence("fork_llm_status", ["loading", "ready"]); // last repeats
  ipc.emitOnInvoke("fork_start_take", "fork-pill-state", {
    state: "listening",
  });

  // 2. Navigate. The mock is already installed for every page load.
  await page.goto("/#/fork/debug");

  // 3. Push backend events from the test. Wait for the page to listen first,
  //    or the event is dropped (emit returns the number of listeners).
  await ipc.waitForListener("fork-pill-state");
  await ipc.emit("fork-pill-state", { state: "processing" });

  // 4. Assert on the commands the page invoked.
  await ipc.expectCalled("fork_set_setting", { key: "cleanup_level" }); // args subset match
  expect(await ipc.commandNames()).toContain("fork_get_settings");
  expect(await ipc.calls("fork_set_setting")).toHaveLength(1);
});
```

### `ipc` API

| Method                                   | Does                                                                          |
| ---------------------------------------- | ----------------------------------------------------------------------------- |
| `respond(cmd, value)` / `respond({...})` | Static reply. `ipcError(x)` / `NOT_READY` reject the invoke with `x`          |
| `handle(cmd, (args, n) => value)`        | Computed reply (may be async); `n` is the 1-based call count for that command |
| `respondSequence(cmd, [a, b])`           | Replies in order, the last one repeats                                        |
| `emitOnInvoke(cmd, event, payload)`      | Emits `event` each time `cmd` resolves (a backend side effect)                |
| `emit(event, payload)`                   | Emits a backend event now; returns the listener count                         |
| `waitForListener(event)`                 | Waits until the page has called `listen(event)`                               |
| `listenerCount(event)`                   | Current listener count                                                        |
| `calls(cmd?)`, `commandNames()`          | Recorded invokes `{ cmd, args }` in order (event-plugin calls included)       |
| `expectCalled(cmd, argsSubset?)`         | Polls until the command was invoked (with matching args)                      |
| `clearCalls()`                           | Forgets recorded calls                                                        |
| `unhandledCommands()`                    | Commands the page invoked that had no handler (they resolved to `null`)       |

Unregistered commands resolve with `null`, which is what Handy's pages tolerate
at startup. Check `ipc.unhandledCommands()` when a page misbehaves: it usually
names the reply you forgot.

`fork_ping` mirrors the backend: each reply gets an increasing `seq` and
`fork-ping-event` is emitted (unless `emitOnInvoke` is set for it).

### Options

```ts
test.use({ ipcSetup: { os: { version: "26.0" }, windowLabel: "fork-pill" } });
```

`os` overrides the OS plugin globals (default macOS 15, aarch64);
`windowLabel` is the window/webview label the page sees (default `main`; use
`fork-pill`, `fork-scratchpad`, … for fork windows). `responses` sets static
in-page replies (handlers registered with `ipc` take precedence).

### Typed payloads

Fork command and event types are generated in `src/fork/bindings.ts`. Import
types from there (`import type { ForkModelStatus } from "../../src/fork/bindings"`)
and write `ipc.respond("fork_model_status", status satisfies ForkModelStatus)`
so a Rust type change breaks the spec at compile time in your editor.
Command names are the snake_case Rust names (`fork_model_status`); event names
are kebab-case type names (`ForkPingEvent` → `fork-ping-event`).

### How it works

`fixtures.ts` exposes a Playwright binding (`window.__wbIpcRemote`) and adds
`installTauriMock` as an init script. In the page, every `invoke` is recorded
in `window.__WB_IPC__.calls`; `plugin:event|listen/unlisten/emit` are handled
in-page; any other command first asks the binding (your `handle`/`respond`),
then the static `responses`, else resolves `null`. `ipc.emit` calls
`window.__WB_IPC__.emit` with `page.evaluate`. Args and replies cross the
binding as JSON, so return plain data.

### Conventions (R5 §11.2)

- One spec per page and per pill state; light and dark; one RTL locale;
  reduced motion; axe checks on every spec (added with the first page slice).
- Use `data-testid` hooks and roles, never text that translations change,
  unless the test is about the text.
- New source files start with `// SPDX-License-Identifier: GPL-3.0-or-later`.
- What Playwright cannot prove (real asset protocol under CSP, notifications,
  menus, AX, hotkeys) goes in `docs/fork/MAC-TEST-SCRIPT.md`.
