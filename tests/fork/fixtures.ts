// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Playwright fixture for fork UI tests: a mocked Tauri IPC for fork commands
 * and events (R5 §11.2). Usage is documented in `tests/fork/README.md`.
 *
 *   import { test, expect } from "./fixtures";
 *   test("…", async ({ page, ipc }) => {
 *     ipc.respond({ fork_model_status: { known_models: 1, downloaded_model_ids: [] } });
 *     await page.goto("/#/fork/debug");
 *     await ipc.waitForListener("fork-ping-event");
 *     await ipc.emit("fork-ping-event", { seq: 7 });
 *     await ipc.expectCalled("fork_model_status");
 *   });
 *
 * The mock is installed before every page load, so specs only register
 * responses/handlers (any time, also after navigation) and navigate.
 */
import { test as base, expect, type Page } from "@playwright/test";
import {
  installTauriMock,
  type IpcCall,
  type MockEmit,
  type MockSetup,
  type RemoteReply,
} from "./ipc-mock";

export { expect };
export type { IpcCall, MockEmit } from "./ipc-mock";

/** Test-side command handler. Return `ipcError(x)` to reject with `x`. */
export type IpcHandler = (
  args: Record<string, unknown>,
  call: number,
) => unknown | Promise<unknown>;

/** Rejection value for a handler or `respond` (how fork commands fail). */
export function ipcError(error: unknown): { __error: unknown } {
  return { __error: error };
}

/** The `NotReady` error every fork command returns before `on_ready`. */
export const NOT_READY = ipcError({ kind: "NotReady" });

export class IpcMock {
  private handlers = new Map<string, IpcHandler>();
  private emitsOnInvoke = new Map<string, MockEmit[]>();
  private counts = new Map<string, number>();
  private unhandled = new Set<string>();

  constructor(private readonly page: Page) {}

  /** Called by the page for every non-event command. */
  async dispatch(
    cmd: string,
    args: Record<string, unknown>,
  ): Promise<RemoteReply> {
    const n = (this.counts.get(cmd) ?? 0) + 1;
    this.counts.set(cmd, n);
    const emits = this.emitsOnInvoke.get(cmd);
    const handler = this.handlers.get(cmd);
    if (!handler) {
      this.unhandled.add(cmd);
      return { handled: false, emits };
    }
    return { handled: true, value: await handler(args, n), emits };
  }

  /** Registers a handler for a command (replaces an earlier one). */
  handle(cmd: string, handler: IpcHandler): this {
    this.handlers.set(cmd, handler);
    return this;
  }

  /** Static replies: `respond("fork_x", value)` or `respond({ fork_x: value })`. */
  respond(cmdOrMap: string | Record<string, unknown>, value?: unknown): this {
    const map = typeof cmdOrMap === "string" ? { [cmdOrMap]: value } : cmdOrMap;
    for (const [cmd, v] of Object.entries(map)) this.handle(cmd, () => v);
    return this;
  }

  /** Replies in order; the last one repeats. */
  respondSequence(cmd: string, values: unknown[]): this {
    return this.handle(
      cmd,
      (_args, n) => values[Math.min(n, values.length) - 1],
    );
  }

  /** Emits `event` every time `cmd` resolves (like a backend side effect). */
  emitOnInvoke(cmd: string, event: string, payload?: unknown): this {
    this.emitsOnInvoke.set(cmd, [
      ...(this.emitsOnInvoke.get(cmd) ?? []),
      { event, payload },
    ]);
    return this;
  }

  /** Emits a backend event to the page now; returns the listener count. */
  async emit(event: string, payload?: unknown): Promise<number> {
    return this.page.evaluate(
      ([e, p]) =>
        (
          window as unknown as {
            __WB_IPC__: { emit: (e: string, p: unknown) => number };
          }
        ).__WB_IPC__.emit(e as string, p),
      [event, payload] as const,
    );
  }

  /** Number of page listeners for `event`. */
  async listenerCount(event: string): Promise<number> {
    return this.page.evaluate(
      (e) =>
        (
          window as unknown as {
            __WB_IPC__: { listenerCount: (e: string) => number };
          }
        ).__WB_IPC__.listenerCount(e),
      event,
    );
  }

  /** Waits until the page listens for `event` (avoids emitting too early). */
  async waitForListener(event: string, timeout = 5000): Promise<void> {
    await this.page.waitForFunction(
      (e) => {
        const api = (
          window as unknown as {
            __WB_IPC__?: { listenerCount: (e: string) => number };
          }
        ).__WB_IPC__;
        return !!api && api.listenerCount(e) > 0;
      },
      event,
      { timeout },
    );
  }

  /** Recorded invokes (all, or for one command), in order. */
  async calls(cmd?: string): Promise<IpcCall[]> {
    const all = await this.page.evaluate(
      () =>
        (window as unknown as { __WB_IPC__?: { calls: IpcCall[] } }).__WB_IPC__
          ?.calls ?? [],
    );
    return cmd ? all.filter((c) => c.cmd === cmd) : all;
  }

  /** Command names invoked so far, in order. */
  async commandNames(): Promise<string[]> {
    return (await this.calls()).map((c) => c.cmd);
  }

  async clearCalls(): Promise<void> {
    await this.page.evaluate(() =>
      (
        window as unknown as { __WB_IPC__: { clearCalls: () => void } }
      ).__WB_IPC__.clearCalls(),
    );
  }

  /** Polls until `cmd` was invoked (optionally with matching args). */
  async expectCalled(
    cmd: string,
    args?: Record<string, unknown>,
    timeout = 5000,
  ): Promise<void> {
    await expect
      .poll(
        async () => {
          const calls = await this.calls(cmd);
          return args
            ? calls.some((c) => isSubset(args, c.args))
            : calls.length > 0;
        },
        { message: `expected ${cmd} to be invoked`, timeout },
      )
      .toBe(true);
  }

  /** Commands the page invoked that had no handler (resolved from setup or `null`). */
  unhandledCommands(): string[] {
    return [...this.unhandled].sort();
  }
}

function isSubset(expected: unknown, actual: unknown): boolean {
  if (expected === actual) return true;
  if (
    !expected ||
    !actual ||
    typeof expected !== "object" ||
    typeof actual !== "object"
  ) {
    return false;
  }
  return Object.entries(expected as Record<string, unknown>).every(([k, v]) =>
    isSubset(v, (actual as Record<string, unknown>)[k]),
  );
}

type IpcFixtureOptions = {
  /** Static setup for the in-page mock (OS globals, window label, responses). */
  ipcSetup: Omit<MockSetup, "__wb" | "remote">;
};

export const test = base.extend<IpcFixtureOptions & { ipc: IpcMock }>({
  ipcSetup: [{}, { option: true }],
  ipc: async ({ page, ipcSetup }, use) => {
    const ipc = new IpcMock(page);
    await page.exposeFunction(
      "__wbIpcRemote",
      (cmd: string, args: Record<string, unknown>) => ipc.dispatch(cmd, args),
    );
    const setup: MockSetup = { ...ipcSetup, __wb: 1, remote: true };
    await page.addInitScript(installTauriMock, setup);
    await use(ipc);
  },
});
