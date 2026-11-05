// SPDX-License-Identifier: GPL-3.0-or-later
import type { Page } from "@playwright/test";
import type { Note } from "../../src/fork/bindings";
import type { IpcMock } from "./fixtures";
import { forkSettingsDefaults } from "./settings-fixture";
import {
  expect,
  expectNoSeriousAxeFindings,
  openHub,
  test,
  type HubState,
} from "./hub-fixture";

let nextId = 3;
const note = (id: number, body: string, updatedAt = id): Note => ({
  id,
  title:
    body
      .split("\n")
      .find((line) => line.trim())
      ?.trim() ?? "",
  body,
  pinned: false,
  created_at: id,
  updated_at: updatedAt,
});

class FakeNotes {
  constructor() {
    nextId = 3;
  }
  notes = [note(1, "alpha café", 10), note(2, "beta launch", 20)];
  saves: { id: number; body: string }[] = [];
  expanded = false;

  install(ipc: IpcMock, hub?: HubState) {
    const sorted = () =>
      [...this.notes].sort(
        (a, b) =>
          Number(b.pinned) - Number(a.pinned) || b.updated_at - a.updated_at,
      );
    ipc.handle("fork_scratchpad_list", (args) => {
      const query = String(args.query ?? "").toLocaleLowerCase();
      return sorted().filter(
        (item) => !query || item.body.toLocaleLowerCase().includes(query),
      );
    });
    ipc.handle(
      "fork_scratchpad_get",
      (args) => this.notes.find((item) => item.id === args.id) ?? null,
    );
    ipc.handle("fork_scratchpad_save", (args) => {
      const id = Number(args.id);
      const body = String(args.body);
      this.saves.push({ id, body });
      const current = this.notes.find((item) => item.id === id);
      if (!current) throw new Error("missing fake note");
      Object.assign(current, note(id, body, 30 + this.saves.length), {
        pinned: current.pinned,
      });
      return current;
    });
    ipc.handle("fork_scratchpad_set_pinned", (args) => {
      const current = this.notes.find((item) => item.id === args.id);
      if (!current) throw new Error("missing fake note");
      current.pinned = Boolean(args.pinned);
      return current;
    });
    ipc.handle("fork_scratchpad_delete", (args) => {
      this.notes = this.notes.filter((item) => item.id !== args.id);
      return null;
    });
    ipc.handle("fork_scratchpad_new", () => {
      const made = note(nextId++, "");
      this.notes.push(made);
      return made;
    });
    ipc.handle("fork_scratchpad_show_note", (args) =>
      this.notes.find((item) => item.id === args.id),
    );
    ipc.handle("fork_scratchpad_open", () => {
      if (hub?.forkSettings.scratchpad_open_behavior === "new_note") {
        const made = note(nextId++, "");
        this.notes.push(made);
        return made;
      }
      return sorted()[0];
    });
    ipc.handle("fork_scratchpad_expand", () => {
      this.expanded = !this.expanded;
      return this.expanded;
    });
    ipc.respond({
      fork_scratchpad_close: null,
      fork_scratchpad_copy: null,
      fork_scratchpad_polish: {
        outcome: "changed",
        text: "Alpha café, polished.",
      },
      fork_scratchpad_record: null,
      fork_llm_status: {
        state: "ready",
        model_name: "Muesli",
        downloaded_bytes: 0,
        total_bytes: 0,
        server: { phase: "stopped" },
        error: null,
      },
      is_recording: false,
      fork_hotkey_status: {
        running: true,
        error: null,
        trigger: "fn",
        globe: "do_nothing",
        fn_blocked: false,
        apple_dictation: "off",
        issues: [],
        bindings: [
          {
            action: "dictate",
            chord: "fn",
            activation: "hold",
            label: { text: "fn", spoken: "Function" },
          },
        ],
      },
    });
  }
}

async function openPage(
  page: Page,
  ipc: IpcMock,
  hub: HubState,
  fake: FakeNotes,
  options: Parameters<typeof openHub>[3] = {},
) {
  await openHub(page, ipc, hub, {
    hash: "#/hub/scratchpad",
    ...options,
    beforeNavigate: () => {
      fake.install(ipc, hub);
      options.beforeNavigate?.();
    },
  });
  await expect(page.getByTestId("wb-scratchpad-page")).toBeVisible();
}

/**
 * The fork settings the note window reads (the text size since audit OT2),
 * kept across reloads like the real store.
 */
function installSettings(ipc: IpcMock, start: Record<string, unknown> = {}) {
  let current: Record<string, unknown> = {
    ...structuredClone(forkSettingsDefaults),
    ...start,
  };
  ipc.handle("fork_get_settings", () => current);
  ipc.handle("fork_set_setting", (args) => {
    const update = args.update as { key: string; value: unknown };
    current = { ...current, [update.key]: update.value };
    return current;
  });
}

async function openWindow(
  page: Page,
  ipc: IpcMock,
  fake: FakeNotes,
  id = 2,
  settings: Record<string, unknown> = {},
) {
  fake.install(ipc);
  installSettings(ipc, settings);
  await page.goto(`/src/fork/scratchpad/index.html?note=${id}`);
  await expect(page.getByTestId("wb-note-window")).toBeVisible();
  await expect(page.getByTestId("wb-note-body")).toBeVisible();
}

test("Hub preferences, unset shortcut proposal and New note", async ({
  page,
  ipc,
  hub,
}) => {
  const fake = new FakeNotes();
  await openPage(page, ipc, hub, fake);
  await expect(page.getByTestId("wb-scratchpad-shortcut-value")).toHaveText(
    "Set shortcut",
  );
  await page.getByTestId("wb-scratchpad-on-bar").check();
  await expect.poll(() => hub.forkSettings.scratchpad_on_bar).toBe(true);
  await page.getByTestId("wb-scratchpad-shortcut").click();
  await expect(page.getByTestId("wb-scratchpad-proposed")).toHaveText(
    "Control + Option + S",
  );
  await expect(page.getByTestId("wb-scratchpad-shortcut-close")).toBeFocused();
  await page.getByTestId("wb-scratchpad-use-proposed").click();
  await expect(page.getByTestId("wb-scratchpad-shortcut-dialog")).toHaveCount(
    0,
  );
  await expect(page.getByTestId("wb-scratchpad-shortcut-value")).toHaveText(
    "⌃ ⌥ S",
  );
  expect(hub.forkSettings.hotkey).toEqual({
    action: "scratchpad.open",
    bindings: [{ chord: "ctrl+option+s", activation: "press" }],
  });
  await page.getByTestId("wb-scratchpad-new").click();
  await ipc.expectCalled("fork_scratchpad_new", {});
});

test("Recents edit, trigram-search model, pin and delete", async ({
  page,
  ipc,
  hub,
}) => {
  const fake = new FakeNotes();
  await openPage(page, ipc, hub, fake);
  await expect(page.getByTestId("wb-note-1")).toBeVisible();
  await page.getByTestId("wb-scratchpad-search-open").click();
  await page.getByTestId("wb-scratchpad-search").fill("café");
  await expect(page.getByTestId("wb-note-1")).toBeVisible();
  await expect(page.getByTestId("wb-note-2")).toHaveCount(0);
  await page.getByTestId("wb-scratchpad-search").fill("no match");
  await expect(page.getByText("No notes match your search.")).toBeVisible();
  await page.getByTestId("wb-scratchpad-search").fill("");
  await page.getByTestId("wb-note-pin-1").click();
  await expect(page.getByTestId("wb-note-pin-1")).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  expect(fake.notes[0].pinned).toBe(true);
  await page.getByTestId("wb-note-edit-2").click();
  await ipc.expectCalled("fork_scratchpad_show_note", { id: 2 });
  page.once("dialog", (dialog) => void dialog.accept());
  await page.getByTestId("wb-note-delete-2").click();
  await expect(page.getByTestId("wb-note-2")).toHaveCount(0);
});

test("500 ms autosave is debounced and persisted text survives reload", async ({
  page,
  ipc,
}) => {
  const fake = new FakeNotes();
  await openWindow(page, ipc, fake);
  const editor = page.getByTestId("wb-note-body");
  await expect(editor).toHaveValue("beta launch");
  await editor.fill("crash-safe words");
  await page.waitForTimeout(300);
  expect(fake.saves).toEqual([]);
  await expect.poll(() => fake.saves.length, { timeout: 1500 }).toBe(1);
  expect(fake.saves[0]).toEqual({ id: 2, body: "crash-safe words" });
  await expect(page.getByTestId("wb-note-save-state")).toHaveText("Saved");
  await page.reload();
  await expect(page.getByTestId("wb-note-body")).toHaveValue(
    "crash-safe words",
  );
});

test("tabs, keyboard selection and switching flush the previous note", async ({
  page,
  ipc,
}) => {
  const fake = new FakeNotes();
  await openWindow(page, ipc, fake);
  await page.getByTestId("wb-note-body").fill("edited beta");
  await page.getByTestId("wb-note-tab-1").click();
  await expect(page.getByTestId("wb-note-body")).toHaveValue("alpha café");
  expect(
    fake.saves.some((save) => save.id === 2 && save.body === "edited beta"),
  ).toBe(true);
  await page.getByTestId("wb-note-new-tab").click();
  await expect(page.getByTestId("wb-note-body")).toHaveValue("");
  await expect(page.getByTestId("wb-note-tab-3")).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await page.getByTestId("wb-note-tab-3").focus();
  await page.keyboard.press("ArrowRight");
  await expect(page.getByTestId("wb-note-tab-1")).toBeFocused();
});

test("window tools: sidebar, search, Polish, text size, Copy, expand and close", async ({
  page,
  ipc,
}) => {
  const fake = new FakeNotes();
  await openWindow(page, ipc, fake);
  await page.getByTestId("wb-note-sidebar").click();
  await expect(page.getByTestId("wb-note-sidebar-panel")).toBeVisible();
  await page.getByTestId("wb-note-search-toggle").click();
  await expect(page.getByTestId("wb-note-search")).toBeFocused();
  await page.getByTestId("wb-note-search").fill("alpha");
  await expect(
    page
      .getByTestId("wb-note-sidebar-panel")
      .getByRole("button", { name: "alpha café" }),
  ).toBeVisible();
  await expect(
    page
      .getByTestId("wb-note-sidebar-panel")
      .getByRole("button", { name: "beta launch" }),
  ).toHaveCount(0);
  await page.getByTestId("wb-note-text-size").click();
  await expect(page.getByTestId("wb-note-body")).toHaveCSS("font-size", "20px");
  await page.getByTestId("wb-note-polish").click();
  await expect(page.getByTestId("wb-note-body")).toHaveValue(
    "Alpha café, polished.",
  );
  // rc9: a polished note says so and offers Undo.
  await expect(page.getByTestId("wb-note-polish-notice")).toHaveText(
    /Note polished/,
  );
  await page.getByTestId("wb-note-polish-undo").click();
  await expect(page.getByTestId("wb-note-body")).toHaveValue("beta launch");
  await expect(page.getByTestId("wb-note-polish-notice")).toHaveCount(0);
  await page.getByTestId("wb-note-copy").click();
  await expect(page.getByTestId("wb-note-copy")).toHaveText("Copied");
  await page.getByTestId("wb-note-expand").click();
  await expect(page.getByTestId("wb-note-expand")).toHaveAttribute(
    "aria-label",
    "Restore note window size",
  );
  await page.getByTestId("wb-note-close").click();
  await ipc.expectCalled("fork_scratchpad_close", {});
});

test("shortcut dialog supports Escape and page/window pass axe in light/dark RTL with reduced motion", async ({
  page,
  ipc,
  hub,
}) => {
  test.setTimeout(120_000);
  const fake = new FakeNotes();
  for (const theme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
    await openPage(page, ipc, hub, fake, {
      handy: { theme, app_language: "ar" },
    });
    await expect(page.getByTestId("wb-hub")).toHaveAttribute("dir", "rtl");
    await expectNoSeriousAxeFindings(page);
    await page.getByTestId("wb-scratchpad-shortcut").click();
    await expectNoSeriousAxeFindings(page);
    await page.keyboard.press("Escape");
    await expect(page.getByTestId("wb-scratchpad-shortcut-dialog")).toHaveCount(
      0,
    );
    await openWindow(page, ipc, fake);
    await expect(page.getByTestId("wb-note-window")).toHaveAttribute(
      "dir",
      "rtl",
    );
    // Mac wave 6: macOS keeps the traffic lights on the left in every
    // language, so the strip keeps its room for them on the left in RTL.
    const strip = page.getByTestId("wb-note-window").locator("header");
    expect(await strip.evaluate((el) => getComputedStyle(el).paddingLeft)).toBe(
      "90px",
    );
    await expect(page.getByTestId("wb-note-body")).toHaveAttribute(
      "dir",
      "auto",
    );
    await expectNoSeriousAxeFindings(page);
  }
});

// Audit OT2: the text size is a fork setting, so a parked or reopened
// window keeps it.
test("the text size is saved and comes back in a new window", async ({
  page,
  ipc,
}) => {
  const fake = new FakeNotes();
  await openWindow(page, ipc, fake, 2, { scratchpad_text_size: 24 });
  await expect(page.getByTestId("wb-note-body")).toHaveCSS("font-size", "24px");
  await page.getByTestId("wb-note-text-size").focus();
  await page.keyboard.press("Enter");
  await ipc.expectCalled("fork_set_setting", {
    update: { key: "scratchpad_text_size", value: 16 },
  });
  await expect(page.getByTestId("wb-note-body")).toHaveCSS("font-size", "16px");
  await page.getByTestId("wb-note-text-size").click();
  await expect(page.getByTestId("wb-note-body")).toHaveCSS("font-size", "20px");
  // A rebuilt window (parked after 60 s, or the next launch) reads it back.
  await page.reload();
  await expect(page.getByTestId("wb-note-body")).toBeVisible();
  await expect(page.getByTestId("wb-note-body")).toHaveCSS("font-size", "20px");
});

// rc9: Polish never ends silently, and waits for a model on its way.
test("Polish says when it could not polish, and gets ready first", async ({
  page,
  ipc,
}) => {
  const fake = new FakeNotes();
  await openWindow(page, ipc, fake);
  ipc.respond("fork_scratchpad_polish", { outcome: "refused", text: null });
  await page.getByTestId("wb-note-polish").click();
  await expect(page.getByTestId("wb-note-polish-notice")).toHaveText(
    /Couldn't polish this note\./,
  );
  await expect(page.getByTestId("wb-note-body")).toHaveValue("beta launch");
  for (const theme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: theme });
    await expectNoSeriousAxeFindings(page);
  }
  await page.getByTestId("wb-note-polish-dismiss").click();
  await expect(page.getByTestId("wb-note-polish-notice")).toHaveCount(0);

  ipc.respond("fork_scratchpad_polish", { outcome: "no_change", text: null });
  await page.getByTestId("wb-note-polish").click();
  await expect(page.getByTestId("wb-note-polish-notice")).toHaveText(
    /already reads well/,
  );

  // The cleanup model is downloading: "Getting ready…" until the command
  // answers.
  ipc.respond("fork_llm_status", {
    state: "downloading",
    model_name: "Muesli",
    downloaded_bytes: 1,
    total_bytes: 2,
    server: { phase: "stopped" },
    error: null,
  });
  let release: () => void = () => undefined;
  const answered = new Promise<void>((resolve) => (release = resolve));
  ipc.handle("fork_scratchpad_polish", async () => {
    await answered;
    return { outcome: "not_ready", text: null };
  });
  await page.getByTestId("wb-note-polish").click();
  await expect(page.getByTestId("wb-note-save-state")).toHaveText(
    "Getting ready…",
  );
  release();
  await expect(page.getByTestId("wb-note-polish-notice")).toHaveText(
    /still getting ready/,
  );
  await expect(page.getByTestId("wb-note-save-state")).toHaveText("Saved");
});

// rc9: Record dictates into the note like a click on the bar.
test("Record starts and stops a take without taking focus from the note", async ({
  page,
  ipc,
}) => {
  const fake = new FakeNotes();
  await openWindow(page, ipc, fake);
  const record = page.getByTestId("wb-note-record");
  await expect(record).toHaveText("Record");
  await expect(record).toHaveAttribute("title", "Dictate into this note (fn)");
  await expect(record).toHaveAttribute("aria-pressed", "false");
  await page.getByTestId("wb-note-body").focus();
  await record.click();
  await ipc.expectCalled("fork_scratchpad_record", {});
  await expect(page.getByTestId("wb-note-body")).toBeFocused();

  await ipc.waitForListener("recording-state-event");
  await ipc.emit("recording-state-event", { phase: "locked" });
  await expect(record).toHaveText("Stop");
  await expect(record).toHaveAttribute("aria-pressed", "true");
  await expect(record).toHaveAttribute("title", "Stop dictating");
  await ipc.waitForListener("fork-pill-levels");
  await ipc.emit("fork-pill-levels", { bars: Array(12).fill(1) });
  const bar = page.getByTestId("wb-note-record-level").locator("span").first();
  await expect(bar).toHaveCSS("height", "14px");
  for (const theme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: theme });
    await expectNoSeriousAxeFindings(page);
  }
  await record.click();
  await expect
    .poll(async () => (await ipc.calls("fork_scratchpad_record")).length)
    .toBe(2);
  await ipc.emit("recording-state-event", { phase: "stopped" });
  await expect(record).toHaveText("Record");

  // Keyboard: Tab to Record, Enter starts the take and the note gets the
  // focus back for the paste.
  await record.focus();
  await page.keyboard.press("Enter");
  await expect(page.getByTestId("wb-note-body")).toBeFocused();
  await expect
    .poll(async () => (await ipc.calls("fork_scratchpad_record")).length)
    .toBe(3);
});
