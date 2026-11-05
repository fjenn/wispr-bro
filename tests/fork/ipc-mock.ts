// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * In-page Tauri IPC mock for fork UI tests. Most specs should use the
 * Playwright fixture in `./fixtures.ts`, which installs this mock and adds
 * test-side handlers, event emission and call assertions (see README.md).
 *
 * Injected with `page.addInitScript(installTauriMock, setup)` before the app
 * loads. Playwright serializes the function source, so it must stay
 * self-contained: no imports or outer helpers are reachable inside it.
 *
 * It mirrors what `@tauri-apps/api/mocks` (`mockWindows` + `mockIPC`) sets up:
 * `window.__TAURI_INTERNALS__` with `invoke`, `transformCallback` and window
 * metadata, the event plugin's `listen`/`unlisten`/`emit`, and the OS plugin
 * globals that Handy's `main.tsx` reads synchronously.
 *
 * Resolution order for a command (other than `plugin:event|*`):
 * 1. the test-side handler, when the fixture exposed `__wbIpcRemote` and a
 *    handler is registered for the command;
 * 2. `responses[cmd]` from the setup (or set at runtime with
 *    `__WB_IPC__.setResponse`);
 * 3. `null`.
 * A value shaped `{ __error: x }` rejects the invoke with `x` (how fork
 * commands return `ForkError`). `fork_ping` behaves like the backend: the
 * reply gets an increasing `seq` and `fork-ping-event` is emitted. Any command
 * listed in `setup.emits` emits those events after it resolves (not when it
 * rejects).
 *
 * In-page API: `window.__WB_IPC__` (`calls`, `emit`, `listenerCount`,
 * `setResponse`, `clearCalls`) and `window.__WB_IPC_CALLS__` (same array as
 * `calls`, kept for older specs).
 */
export type MockResponses = Record<string, unknown>;

export type MockEmit = { event: string; payload?: unknown };

export type MockSetup = {
  /** Marks a setup object (as opposed to a bare `MockResponses` map). */
  __wb: 1;
  responses?: MockResponses;
  /** Events emitted after the command resolves, in order. */
  emits?: Record<string, MockEmit[]>;
  /** Ask the fixture's `__wbIpcRemote` binding first. */
  remote?: boolean;
  /** Overrides for the OS plugin globals (e.g. `{ version: "26.0" }`). */
  os?: Record<string, string>;
  /** Window label reported to the page (default `main`). */
  windowLabel?: string;
};

export type IpcCall = { cmd: string; args: Record<string, unknown> };

/** What the fixture's `__wbIpcRemote` binding returns. */
export type RemoteReply = {
  /** A test-side handler produced `value`. */
  handled: boolean;
  value?: unknown;
  /** Extra events to emit after the command resolves. */
  emits?: MockEmit[];
};

export type WbIpcPageApi = {
  calls: IpcCall[];
  emit: (event: string, payload?: unknown) => number;
  listenerCount: (event: string) => number;
  setResponse: (cmd: string, value: unknown) => void;
  clearCalls: () => void;
};

export function installTauriMock(
  setupOrResponses: MockResponses | MockSetup,
): void {
  type Callback = (payload: unknown) => void;
  const raw = (setupOrResponses ?? {}) as Record<string, unknown>;
  const setup: MockSetup =
    raw.__wb === 1
      ? (raw as MockSetup)
      : { __wb: 1, responses: raw as MockResponses };
  const responses: MockResponses = { ...(setup.responses ?? {}) };
  const emits = setup.emits ?? {};
  const callbacks = new Map<number, Callback>();
  const listeners = new Map<string, number[]>();
  const calls: IpcCall[] = [];
  let nextId = 1;
  let pingSeq = 0;

  const w = window as unknown as Record<string, unknown>;

  const emit = (event: string, payload?: unknown): number => {
    const ids = listeners.get(event) ?? [];
    for (const id of ids) {
      callbacks.get(id)?.({ event, id, payload });
    }
    return ids.length;
  };

  const isError = (v: unknown): v is { __error: unknown } =>
    !!v && typeof v === "object" && "__error" in (v as object);

  const api: WbIpcPageApi = {
    calls,
    emit,
    listenerCount: (event) => (listeners.get(event) ?? []).length,
    setResponse: (cmd, value) => {
      responses[cmd] = value;
    },
    clearCalls: () => {
      calls.length = 0;
    },
  };
  w.__WB_IPC__ = api;
  w.__WB_IPC_CALLS__ = calls;

  w.__TAURI_OS_PLUGIN_INTERNALS__ = {
    platform: "macos",
    os_type: "macos",
    family: "unix",
    version: "15.0",
    arch: "aarch64",
    exe_extension: "",
    eol: "\n",
    ...(setup.os ?? {}),
  };
  const removeListener = (event: string, id: number) => {
    listeners.set(
      event,
      (listeners.get(event) ?? []).filter((x) => x !== id),
    );
  };
  w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: removeListener };
  const label = setup.windowLabel ?? "main";
  w.__TAURI_INTERNALS__ = {
    metadata: {
      currentWindow: { label },
      currentWebview: { windowLabel: label, label },
    },
    transformCallback(cb: Callback) {
      const id = nextId++;
      callbacks.set(id, cb);
      return id;
    },
    unregisterCallback(id: number) {
      callbacks.delete(id);
    },
    convertFileSrc: (path: string) => path,
    async invoke(cmd: string, args: Record<string, unknown> = {}) {
      calls.push({ cmd, args });
      if (cmd === "plugin:event|listen") {
        const event = args.event as string;
        listeners.set(event, [
          ...(listeners.get(event) ?? []),
          args.handler as number,
        ]);
        return args.handler;
      }
      if (cmd === "plugin:event|unlisten") {
        removeListener(args.event as string, args.eventId as number);
        return null;
      }
      if (cmd === "plugin:event|emit" || cmd === "plugin:event|emit_to") {
        emit(args.event as string, args.payload);
        return null;
      }
      if (cmd.startsWith("plugin:event|")) return null;

      let value: unknown = null;
      let found = false;
      const queued = [...(emits[cmd] ?? [])];
      const remote = w.__wbIpcRemote as
        | ((cmd: string, args: unknown) => Promise<RemoteReply>)
        | undefined;
      if (setup.remote && typeof remote === "function") {
        // Args cross the Playwright binding as JSON.
        const reply = await remote(cmd, JSON.parse(JSON.stringify(args)));
        if (reply.handled) {
          value = reply.value;
          found = true;
        }
        queued.push(...(reply.emits ?? []));
      }
      if (!found && Object.hasOwn(responses, cmd)) {
        value = responses[cmd];
        found = true;
      }

      if (cmd === "fork_ping" && !isError(value)) {
        pingSeq += 1;
        const seq = pingSeq;
        value = { ...((value ?? {}) as Record<string, unknown>), seq };
        if (queued.length === 0) {
          queued.push({ event: "fork-ping-event", payload: { seq } });
        }
      }
      if (queued.length > 0 && !isError(value)) {
        setTimeout(() => {
          for (const e of queued) emit(e.event, e.payload);
        }, 0);
      }
      if (isError(value)) throw value.__error;
      return value;
    },
  };
}
