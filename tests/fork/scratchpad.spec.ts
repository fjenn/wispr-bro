// SPDX-License-Identifier: GPL-3.0-or-later
import type { Page } from "@playwright/test";
import type { Note } from "../../src/fork/bindings";
import type { IpcMock } from "./fixtures";
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
      fork_scratchpad_polish: "Alpha café, polished.",
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

async function openWindow(page: Page, ipc: IpcMock, fake: FakeNotes, id = 2) {
  fake.install(ipc);
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
    await expectNoSeriousAxeFindings(page);
  }
});
