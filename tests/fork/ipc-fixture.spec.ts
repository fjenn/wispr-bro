// SPDX-License-Identifier: GPL-3.0-or-later
// F-10 harness self-tests: the IPC fixture on a bare page (no app code), so a
// failure here points at the fixture, not at a fork page.
import { test, expect, ipcError, type IpcCall } from "./fixtures";
import type { Page } from "@playwright/test";

const BARE = "/__wb_ipc_selftest";

async function openBarePage(page: Page): Promise<void> {
  await page.route(`**${BARE}`, (route) =>
    route.fulfill({
      contentType: "text/html",
      body: "<!doctype html><html><body>ipc self-test</body></html>",
    }),
  );
  await page.goto(BARE);
}

type Internals = {
  invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
  transformCallback: (cb: (e: unknown) => void) => number;
};

/** Invokes through `__TAURI_INTERNALS__` the way `@tauri-apps/api` does. */
async function invoke(
  page: Page,
  cmd: string,
  args: Record<string, unknown> = {},
): Promise<{ ok: unknown } | { err: unknown }> {
  return page.evaluate(
    async ([c, a]) => {
      const t = (window as unknown as { __TAURI_INTERNALS__: Internals })
        .__TAURI_INTERNALS__;
      try {
        return {
          ok: await t.invoke(c as string, a as Record<string, unknown>),
        };
      } catch (e) {
        return { err: e instanceof Error ? `Error: ${e.message}` : e };
      }
    },
    [cmd, args] as const,
  );
}

/** Subscribes like `listen()`; payloads land in `window.__got[event]`. */
async function listen(page: Page, event: string): Promise<number> {
  return page.evaluate(async (e) => {
    const w = window as unknown as {
      __TAURI_INTERNALS__: Internals;
      __got?: Record<string, unknown[]>;
    };
    w.__got ??= {};
    w.__got[e] = [];
    const handler = w.__TAURI_INTERNALS__.transformCallback((msg) =>
      w.__got![e].push((msg as { payload: unknown }).payload),
    );
    return (await w.__TAURI_INTERNALS__.invoke("plugin:event|listen", {
      event: e,
      target: { kind: "Any" },
      handler,
    })) as number;
  }, event);
}

async function received(page: Page, event: string): Promise<unknown[]> {
  return page.evaluate(
    (e) =>
      (window as unknown as { __got?: Record<string, unknown[]> }).__got?.[e] ??
      [],
    event,
  );
}

test.describe("IPC fixture self-test", () => {
  test("static responses, handlers, sequences and rejections", async ({
    page,
    ipc,
  }) => {
    ipc
      .respond("fork_a", { a: 1 })
      .respond({ fork_b: "b" })
      .respondSequence("fork_seq", ["first", "second"])
      .handle("fork_echo", (args) => ({ echoed: args }))
      .respond("fork_fail", ipcError({ kind: "NotReady" }));
    await openBarePage(page);

    expect(await invoke(page, "fork_a")).toEqual({ ok: { a: 1 } });
    expect(await invoke(page, "fork_b")).toEqual({ ok: "b" });
    expect(await invoke(page, "fork_seq")).toEqual({ ok: "first" });
    expect(await invoke(page, "fork_seq")).toEqual({ ok: "second" });
    expect(await invoke(page, "fork_seq")).toEqual({ ok: "second" });
    expect(await invoke(page, "fork_echo", { level: "light" })).toEqual({
      ok: { echoed: { level: "light" } },
    });
    expect(await invoke(page, "fork_fail")).toEqual({
      err: { kind: "NotReady" },
    });
    // Unregistered commands resolve with null and are reported.
    expect(await invoke(page, "fork_unknown")).toEqual({ ok: null });
    expect(ipc.unhandledCommands()).toEqual(["fork_unknown"]);

    // Handlers registered after navigation apply immediately.
    ipc.respond("fork_unknown", 5);
    expect(await invoke(page, "fork_unknown")).toEqual({ ok: 5 });
  });

  test("records calls with args and can clear them", async ({ page, ipc }) => {
    await openBarePage(page);
    await invoke(page, "fork_set_setting", { key: "cleanup", value: "light" });
    await invoke(page, "get_app_settings");
    const calls: IpcCall[] = await ipc.calls();
    expect(calls.map((c) => c.cmd)).toEqual([
      "fork_set_setting",
      "get_app_settings",
    ]);
    expect(await ipc.calls("fork_set_setting")).toEqual([
      { cmd: "fork_set_setting", args: { key: "cleanup", value: "light" } },
    ]);
    await ipc.expectCalled("fork_set_setting", { key: "cleanup" });
    await ipc.clearCalls();
    expect(await ipc.calls()).toEqual([]);
  });

  test("emits events from the test and on invoke; unlisten stops delivery", async ({
    page,
    ipc,
  }) => {
    ipc
      .respond("fork_start", null)
      .emitOnInvoke("fork_start", "fork-pill-state", { state: "listening" });
    await openBarePage(page);

    expect(await ipc.emit("fork-pill-state", { state: "idle" })).toBe(0);
    const id = await listen(page, "fork-pill-state");
    await ipc.waitForListener("fork-pill-state");
    expect(await ipc.listenerCount("fork-pill-state")).toBe(1);

    expect(await ipc.emit("fork-pill-state", { state: "idle" })).toBe(1);
    await invoke(page, "fork_start");
    await expect
      .poll(() => received(page, "fork-pill-state"))
      .toEqual([{ state: "idle" }, { state: "listening" }]);

    await invoke(page, "plugin:event|unlisten", {
      event: "fork-pill-state",
      eventId: id,
    });
    expect(await ipc.listenerCount("fork-pill-state")).toBe(0);
    expect(await ipc.emit("fork-pill-state", { state: "late" })).toBe(0);
  });

  test("fork_ping mirrors the backend: seq increments and the event fires", async ({
    page,
    ipc,
  }) => {
    ipc.respond("fork_ping", { message: "pong" });
    await openBarePage(page);
    await listen(page, "fork-ping-event");
    expect(await invoke(page, "fork_ping")).toEqual({
      ok: { message: "pong", seq: 1 },
    });
    expect(await invoke(page, "fork_ping")).toEqual({
      ok: { message: "pong", seq: 2 },
    });
    await expect
      .poll(() => received(page, "fork-ping-event"))
      .toEqual([{ seq: 1 }, { seq: 2 }]);
  });
});

test.describe("IPC fixture options", () => {
  test.use({ ipcSetup: { os: { version: "26.0" }, windowLabel: "fork-pill" } });

  test("OS globals and window label are configurable", async ({
    page,
    ipc,
  }) => {
    await openBarePage(page);
    const seen = await page.evaluate(() => {
      const w = window as unknown as {
        __TAURI_OS_PLUGIN_INTERNALS__: { version: string; platform: string };
        __TAURI_INTERNALS__: { metadata: { currentWindow: { label: string } } };
      };
      return {
        version: w.__TAURI_OS_PLUGIN_INTERNALS__.version,
        platform: w.__TAURI_OS_PLUGIN_INTERNALS__.platform,
        label: w.__TAURI_INTERNALS__.metadata.currentWindow.label,
      };
    });
    expect(seen).toEqual({
      version: "26.0",
      platform: "macos",
      label: "fork-pill",
    });
    expect(await ipc.calls()).toEqual([]);
  });
});
