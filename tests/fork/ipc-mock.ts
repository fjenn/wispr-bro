// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Minimal Tauri IPC mock for fork UI tests, injected with
 * `page.addInitScript(installTauriMock, responses)` before the app loads.
 *
 * It mirrors what `@tauri-apps/api/mocks` (`mockWindows` + `mockIPC`) sets up:
 * `window.__TAURI_INTERNALS__` with `invoke`, `transformCallback` and window
 * metadata, the event plugin's `listen`/`emit`, and the OS plugin globals
 * that Handy's `main.tsx` reads synchronously. Commands listed in
 * `responses` resolve with that value (or reject with `{ __error: … }`);
 * every other command resolves with `null`. Calls are recorded in
 * `window.__WB_IPC_CALLS__`. Invoking `fork_ping` also emits
 * `fork-ping-event`, like the backend does.
 */
export type MockResponses = Record<string, unknown>;

export function installTauriMock(responses: MockResponses): void {
  type Callback = (payload: unknown) => void;
  const callbacks = new Map<number, Callback>();
  const listeners = new Map<string, number[]>();
  const calls: { cmd: string; args: unknown }[] = [];
  let nextId = 1;
  let pingSeq = 0;

  const w = window as unknown as Record<string, unknown>;
  w.__WB_IPC_CALLS__ = calls;

  const emit = (event: string, payload: unknown) => {
    for (const id of listeners.get(event) ?? []) {
      callbacks.get(id)?.({ event, id, payload });
    }
  };

  w.__TAURI_OS_PLUGIN_INTERNALS__ = {
    platform: "macos",
    os_type: "macos",
    family: "unix",
    version: "15.0",
    arch: "aarch64",
    exe_extension: "",
    eol: "\n",
  };
  w.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: (event: string, id: number) => {
      listeners.set(
        event,
        (listeners.get(event) ?? []).filter((x) => x !== id),
      );
    },
  };
  w.__TAURI_INTERNALS__ = {
    metadata: {
      currentWindow: { label: "main" },
      currentWebview: { windowLabel: "main", label: "main" },
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
      if (cmd === "plugin:event|emit") {
        emit(args.event as string, args.payload);
        return null;
      }
      if (cmd.startsWith("plugin:event|")) return null;

      if (cmd === "fork_ping") {
        pingSeq += 1;
        const base = (responses.fork_ping ?? {}) as Record<string, unknown>;
        const reply = { ...base, seq: pingSeq };
        setTimeout(() => emit("fork-ping-event", { seq: pingSeq }), 0);
        return reply;
      }
      if (Object.hasOwn(responses, cmd)) {
        const value = responses[cmd] as { __error?: unknown } | null;
        if (value && typeof value === "object" && "__error" in value) {
          throw value.__error;
        }
        return value;
      }
      return null;
    },
  };
}
