// SPDX-License-Identifier: GPL-3.0-or-later
// S-17 Snippets page: the banner with two examples, the Add snippet
// dialog (disabled until both fields are filled, 60 / 4,000-character
// limits, line breaks kept, ⌘Return saves, duplicates and conflicts), edit,
// delete with undo, search and sort, live counts (an edit elsewhere and a
// take that used a snippet), the four-step import wizard with multi-line
// cells (a `.csv` dropped on the import step is imported, never
// transcribed), export, keyboard use, RTL and axe in light and dark (R5
// S-17 tests, §5.5, §11.2). Mocked IPC with a small in-test backend; the
// matcher and the store are tested in Rust (`fork::text::snippets`,
// `fork::snippets::tests`).
//
// WB_SHOTS_DIR=<dir> also writes screenshots of each state.
import path from "node:path";
import type { Page } from "@playwright/test";
import type {
  SnippetEntry,
  SnippetImportReview,
  SnippetImportRow,
  SnippetInput,
  SnippetSaveOutcome,
} from "../../src/fork/bindings";
import { ipcError, type IpcMock } from "./fixtures";
import {
  test,
  expect,
  openHub,
  expectNoSeriousAxeFindings,
  type HubState,
} from "./hub-fixture";

const shotsDir = process.env.WB_SHOTS_DIR;
async function shot(page: Page, name: string) {
  if (!shotsDir) return;
  await page
    .screenshot({
      path: path.join(shotsDir, `s17-${name}.png`),
      animations: "disabled",
      timeout: 10_000,
    })
    .catch((e) => console.warn(`screenshot ${name} skipped: ${e}`));
}

/** Picks a sort order in the page's Radix select (W2: was a native select). */
async function pickSort(page: Page, value: string) {
  await page.getByTestId("wb-snippets-sort").click();
  await page.getByTestId("wb-snippets-sort-option-" + value).click();
  await expect(
    page.getByTestId("wb-snippets-sort-option-" + value),
  ).toHaveCount(0);
}

const ADDRESS = "Alex Morgan\n12 Harbour Lane\nKiel 24103";

/** A small stand-in for `fork::snippets` (key = lowercase trigger). */
class FakeSnippets {
  snippets: SnippetEntry[] = [];
  nextId = 1;
  clock = 1_700_000_000_000;

  seed(input: SnippetInput, over: Partial<SnippetEntry> = {}) {
    const s = this.make(input);
    Object.assign(s, over);
    this.snippets.push(s);
    return s;
  }

  make(input: SnippetInput): SnippetEntry {
    this.clock += 1000;
    return {
      id: this.nextId++,
      trigger: input.trigger.trim().replace(/\s+/g, " "),
      body: input.body.replace(/\s+$/, ""),
      source: "user",
      created_at: this.clock,
      updated_at: this.clock,
      hit_count: 0,
      last_hit_at: null,
    };
  }

  list() {
    return {
      snippets: [...this.snippets].sort((a, b) => b.created_at - a.created_at),
      counts: {
        snippets: this.snippets.length,
        uses: this.snippets.reduce((n, s) => n + s.hit_count, 0),
      },
    };
  }

  check(input: SnippetInput, exceptId?: number): SnippetSaveOutcome | null {
    const trigger = input.trigger.trim();
    if (!trigger) return { status: "invalid", problem: "empty_trigger" };
    if ([...trigger].length > 60)
      return { status: "invalid", problem: "trigger_too_long" };
    const other = this.snippets.find(
      (s) =>
        s.id !== exceptId && s.trigger.toLowerCase() === trigger.toLowerCase(),
    );
    if (!other) return null;
    return other.body === input.body.replace(/\s+$/, "")
      ? { status: "duplicate", existing: other }
      : { status: "conflict", existing: other };
  }

  add(input: SnippetInput): SnippetSaveOutcome {
    const problem = this.check(input);
    if (problem) return problem;
    const snippet = this.make(input);
    this.snippets.push(snippet);
    return { status: "saved", snippet };
  }

  update(id: number, input: SnippetInput): SnippetSaveOutcome {
    const at = this.snippets.findIndex((s) => s.id === id);
    if (at < 0) return { status: "missing" };
    const problem = this.check(input, id);
    if (problem) return problem;
    const next = {
      ...this.make(input),
      id,
      created_at: this.snippets[at].created_at,
      hit_count: this.snippets[at].hit_count,
    };
    this.snippets[at] = next;
    return { status: "saved", snippet: next };
  }

  install(ipc: IpcMock) {
    ipc.handle("fork_snippets_list", () => this.list());
    ipc.handle("fork_snippets_add", (a) => this.add(a.input as SnippetInput));
    ipc.handle("fork_snippets_update", (a) =>
      this.update(a.id as number, a.input as SnippetInput),
    );
    ipc.handle("fork_snippets_delete", (a) => {
      const ids = a.ids as number[];
      const gone = this.snippets.filter((s) => ids.includes(s.id));
      this.snippets = this.snippets.filter((s) => !ids.includes(s.id));
      return gone;
    });
    ipc.handle("fork_snippets_restore", (a) => {
      const back = a.snippets as SnippetEntry[];
      this.snippets.push(...back);
      return back.length;
    });
  }
}

const snipTest = test.extend<{ snips: FakeSnippets }>({
  snips: async ({}, use) => {
    await use(new FakeSnippets());
  },
});

async function openSnippets(
  page: Page,
  ipc: IpcMock,
  hub: HubState,
  snips: FakeSnippets,
  options: Parameters<typeof openHub>[3] = {},
) {
  await openHub(page, ipc, hub, { hash: "#/hub/snippets", ...options });
  snips.install(ipc);
  await page.reload();
  await expect(page.getByTestId("wb-snippets")).toBeVisible();
}

const rows = (page: Page) => page.getByTestId("wb-snippet-entry");
const counts = (page: Page) => page.getByTestId("wb-snippets-counts");

snipTest.describe("snippets page", () => {
  snipTest(
    "empty page: banner with two examples, empty state, count at zero",
    async ({ page, ipc, hub, snips }) => {
      await openSnippets(page, ipc, hub, snips);
      await expect(page.getByTestId("wb-page-title")).toHaveText("Snippets");
      const banner = page.getByTestId("wb-banner-snippets.examples");
      await expect(banner).toContainText("Say a few words, get the whole text");
      await expect(
        banner.getByRole("list", { name: "Examples" }).getByRole("listitem"),
      ).toHaveCount(2);
      await expect(page.getByTestId("wb-snippets-empty")).toBeVisible();
      await expect(counts(page)).toHaveText("0 snippets");
      await expect(page.getByTestId("wb-snippets-export")).toBeDisabled();
      await expect(page.getByTestId("wb-empty-snippets")).toHaveCount(0);
      await shot(page, "empty");
    },
  );

  snipTest(
    "the banner dismissal persists",
    async ({ page, ipc, hub, snips }) => {
      await openSnippets(page, ipc, hub, snips);
      const banner = page.getByTestId("wb-banner-snippets.examples");
      await banner.getByRole("button", { name: "Dismiss" }).click();
      await expect(banner).toBeHidden();
      expect(hub.forkSettings.dismissed_banners).toContain("snippets.examples");
      await page.reload();
      await expect(page.getByTestId("wb-snippets")).toBeVisible();
      await expect(banner).toBeHidden();
    },
  );

  snipTest(
    "Add snippet: disabled until both fields are filled; line breaks kept",
    async ({ page, ipc, hub, snips }) => {
      await openSnippets(page, ipc, hub, snips);
      await page.getByTestId("wb-snippets-add").click();
      const dialog = page.getByRole("dialog", { name: "Add snippet" });
      await expect(dialog).toBeVisible();
      const save = page.getByTestId("wb-snippet-save");
      await expect(page.getByTestId("wb-snippet-trigger")).toBeFocused();
      await expect(save).toBeDisabled();
      await page.getByTestId("wb-snippet-trigger").fill("my address");
      await expect(save).toBeDisabled();
      await page.getByTestId("wb-snippet-body").fill(ADDRESS);
      await expect(save).toBeEnabled();
      await shot(page, "add");
      await save.click();
      await expect(dialog).toBeHidden();
      await ipc.expectCalled("fork_snippets_add", {
        input: { trigger: "my address", body: ADDRESS },
      });
      await expect(rows(page)).toHaveCount(1);
      await expect(page.getByTestId("wb-snippet-body-text")).toHaveText(
        ADDRESS,
      );
      await expect(rows(page).first()).toContainText("3 lines");
      await expect(page.getByText("Added “my address”.")).toBeVisible();
      await expect(counts(page)).toHaveText("1 snippet");
    },
  );

  snipTest(
    "Return makes a new line in the text; ⌘Return saves",
    async ({ page, ipc, hub, snips }) => {
      await openSnippets(page, ipc, hub, snips);
      await page.getByTestId("wb-snippets-add").click();
      // The dialog focuses its first field a frame after opening; a fill
      // before that lands in the wrong field under load.
      await expect(page.getByTestId("wb-snippet-trigger")).toBeFocused();
      await page.getByTestId("wb-snippet-trigger").fill("my sign-off");
      await page.getByTestId("wb-snippet-body").click();
      await page.keyboard.type("Best wishes,");
      await page.keyboard.press("Enter");
      await page.keyboard.type("Finn");
      await expect(page.getByRole("dialog")).toBeVisible();
      await page.keyboard.press("ControlOrMeta+Enter");
      await ipc.expectCalled("fork_snippets_add", {
        input: { trigger: "my sign-off", body: "Best wishes,\nFinn" },
      });
      await expect(page.getByRole("dialog")).toBeHidden();
    },
  );

  snipTest(
    "60 and 4,000 characters at most; the counters and the button say so",
    async ({ page, ipc, hub, snips }) => {
      await openSnippets(page, ipc, hub, snips);
      await page.getByTestId("wb-snippets-add").click();
      // The dialog focuses its first field a frame after opening; a fill
      // before that lands in the wrong field under load.
      await expect(page.getByTestId("wb-snippet-trigger")).toBeFocused();
      await page.getByTestId("wb-snippet-trigger").fill("é".repeat(60));
      await page.getByTestId("wb-snippet-body").fill("x".repeat(4000));
      await expect(page.getByTestId("wb-snippet-trigger-counter")).toHaveText(
        "60/60",
      );
      await expect(page.getByTestId("wb-snippet-body-counter")).toHaveText(
        "4000/4000",
      );
      await expect(page.getByTestId("wb-snippet-save")).toBeEnabled();
      await page.getByTestId("wb-snippet-trigger").fill("é".repeat(61));
      await expect(page.getByTestId("wb-snippet-trigger")).toHaveAttribute(
        "aria-invalid",
        "true",
      );
      await expect(page.getByTestId("wb-snippet-save")).toBeDisabled();
      await page.getByTestId("wb-snippet-trigger").fill("ok");
      await page.getByTestId("wb-snippet-body").fill("x".repeat(4001));
      await expect(page.getByTestId("wb-snippet-body-counter")).toHaveText(
        "4001/4000",
      );
      await expect(page.getByTestId("wb-snippet-save")).toBeDisabled();
    },
  );

  snipTest(
    "duplicates and conflicts are explained; Replace it edits the existing one",
    async ({ page, ipc, hub, snips }) => {
      const existing = snips.seed({
        trigger: "my email",
        body: "a@example.com",
      });
      await openSnippets(page, ipc, hub, snips);
      await page.getByTestId("wb-snippets-add").click();
      // The dialog focuses its first field a frame after opening; a fill
      // before that lands in the wrong field under load.
      await expect(page.getByTestId("wb-snippet-trigger")).toBeFocused();
      await page.getByTestId("wb-snippet-trigger").fill("My Email");
      await page.getByTestId("wb-snippet-body").fill("a@example.com");
      await page.getByTestId("wb-snippet-save").click();
      const feedback = page.getByTestId("wb-snippet-feedback");
      await expect(feedback).toHaveAttribute("data-kind", "duplicate");
      await expect(feedback).toHaveText(
        "“my email” is already saved with this text.",
      );
      await page.getByTestId("wb-snippet-body").fill("b@example.com");
      await page.getByTestId("wb-snippet-save").click();
      await expect(feedback).toHaveAttribute("data-kind", "conflict");
      await shot(page, "conflict");
      await page.getByTestId("wb-snippet-replace-existing").click();
      await ipc.expectCalled("fork_snippets_update", { id: existing.id });
      await expect(page.getByRole("dialog")).toBeHidden();
      await expect(page.getByTestId("wb-snippet-body-text")).toHaveText(
        "b@example.com",
      );
    },
  );

  // Audit OT2: Undo's result is checked; a failure or a skip is said.
  snipTest(
    "a failed or skipped undo says so",
    async ({ page, ipc, hub, snips }) => {
      snips.seed({ trigger: "my address", body: "Kiel" });
      snips.seed({ trigger: "sign off", body: "Best, Kim" });
      await openSnippets(page, ipc, hub, snips);
      ipc.respond(
        "fork_snippets_restore",
        ipcError({ kind: "Internal", message: "disk full" }),
      );
      await page.getByRole("button", { name: "Delete my address" }).click();
      await page
        .getByRole("listitem")
        .filter({ hasText: "Deleted “my address”." })
        .getByRole("button", { name: "Undo" })
        .click();
      await expect(
        page.getByText(
          "“my address” couldn't be restored. Please add it again.",
        ),
      ).toBeVisible();
      ipc.respond("fork_snippets_restore", 0);
      await page.getByRole("button", { name: "Delete sign off" }).click();
      await page
        .getByRole("listitem")
        .filter({ hasText: "Deleted “sign off”." })
        .getByRole("button", { name: "Undo" })
        .click();
      await expect(
        page.getByText(
          "“sign off” wasn't restored: another snippet now uses that phrase.",
        ),
      ).toBeVisible();
    },
  );

  snipTest("edit, delete and undo", async ({ page, ipc, hub, snips }) => {
    snips.seed({ trigger: "my address", body: "Kiel" }, { hit_count: 2 });
    await openSnippets(page, ipc, hub, snips);
    await expect(page.getByTestId("wb-snippet-hits")).toHaveText(
      "Used 2 times",
    );
    await page.getByRole("button", { name: "Edit my address" }).click();
    const dialog = page.getByRole("dialog", { name: "Edit snippet" });
    await expect(page.getByTestId("wb-snippet-body")).toHaveValue("Kiel");
    await page.getByTestId("wb-snippet-body").fill(ADDRESS);
    await dialog.getByRole("button", { name: "Save" }).click();
    await ipc.expectCalled("fork_snippets_update", {
      input: { trigger: "my address", body: ADDRESS },
    });
    await expect(page.getByTestId("wb-snippet-body-text")).toHaveText(ADDRESS);
    // The use count stays across an edit.
    await expect(page.getByTestId("wb-snippet-hits")).toHaveText(
      "Used 2 times",
    );

    await page.getByRole("button", { name: "Delete my address" }).click();
    await expect(rows(page)).toHaveCount(0);
    await expect(page.getByText("Deleted “my address”.")).toBeVisible();
    await page.getByRole("button", { name: "Undo" }).click();
    await ipc.expectCalled("fork_snippets_restore");
    await expect(rows(page)).toHaveCount(1);
  });

  snipTest(
    "search covers triggers and texts, ignoring case and accents; sort",
    async ({ page, ipc, hub, snips }) => {
      snips.seed({ trigger: "my address", body: ADDRESS });
      snips.seed({ trigger: "café menu", body: "Soup" }, { hit_count: 9 });
      snips.seed(
        { trigger: "booking link", body: "https://example.com/book" },
        { source: "import" },
      );
      await openSnippets(page, ipc, hub, snips);
      await expect(counts(page)).toHaveText("3 snippets · used 9 times");
      await expect(page.getByText("Imported")).toBeVisible();
      const triggers = () =>
        page.getByTestId("wb-snippet-trigger-text").allTextContents();
      expect(await triggers()).toEqual([
        "booking link",
        "café menu",
        "my address",
      ]);
      await pickSort(page, "az");
      expect(await triggers()).toEqual([
        "booking link",
        "café menu",
        "my address",
      ]);
      await pickSort(page, "za");
      expect((await triggers())[0]).toBe("my address");
      await pickSort(page, "used");
      expect((await triggers())[0]).toBe("café menu");

      await page.getByTestId("wb-snippets-search").fill("CAFE");
      await expect(rows(page)).toHaveCount(1);
      await page.getByTestId("wb-snippets-search").fill("harbour");
      await expect(rows(page)).toHaveCount(1);
      await page.getByTestId("wb-snippets-search").fill("nothing here");
      await expect(page.getByTestId("wb-snippets-no-match")).toBeVisible();
      await page.getByRole("button", { name: "Clear the search" }).click();
      await expect(rows(page)).toHaveCount(3);
    },
  );

  snipTest(
    "counts update live: another window, then a take that used a snippet",
    async ({ page, ipc, hub, snips }) => {
      await openSnippets(page, ipc, hub, snips);
      await ipc.waitForListener("fork-snippets-changed");
      const s = snips.seed({ trigger: "my address", body: ADDRESS });
      await ipc.emit("fork-snippets-changed", {
        counts: { snippets: 1, uses: 0 },
      });
      await expect(rows(page)).toHaveCount(1);
      await expect(counts(page)).toHaveText("1 snippet");
      // A take expanded it: the backend counts it and says so.
      s.hit_count = 1;
      await ipc.emit("fork-snippets-changed", {
        counts: { snippets: 1, uses: 1 },
      });
      await expect(page.getByTestId("wb-snippet-hits")).toHaveText(
        "Used 1 time",
      );
      await expect(counts(page)).toHaveText("1 snippet · used 1 time");
      // The stats event after a take also refreshes.
      s.hit_count = 2;
      await ipc.emit("fork-stats-updated", { day: "2026-10-31" });
      await expect(page.getByTestId("wb-snippet-hits")).toHaveText(
        "Used 2 times",
      );
      s.hit_count = 3;
      const before = (await ipc.calls("fork_snippets_list")).length;
      await page.getByTestId("wb-snippets-refresh").click();
      await expect
        .poll(async () => (await ipc.calls("fork_snippets_list")).length)
        .toBeGreaterThan(before);
      await expect(page.getByTestId("wb-snippet-hits")).toHaveText(
        "Used 3 times",
      );
    },
  );

  snipTest(
    "keyboard only: add a snippet and reach every control",
    async ({ page, ipc, hub, snips }) => {
      await openSnippets(page, ipc, hub, snips);
      await page.getByTestId("wb-snippets-import").focus();
      await page.keyboard.press("Tab"); // Export (disabled) is skipped
      await expect(page.getByTestId("wb-snippets-add")).toBeFocused();
      await page.keyboard.press("Enter");
      await expect(page.getByTestId("wb-snippet-trigger")).toBeFocused();
      await page.keyboard.type("my email");
      await page.keyboard.press("Tab");
      await expect(page.getByTestId("wb-snippet-body")).toBeFocused();
      await page.keyboard.type("finn@example.com");
      await page.keyboard.press("ControlOrMeta+Enter");
      await expect(rows(page)).toHaveCount(1);
      await expect(page.getByTestId("wb-snippets-add")).toBeFocused();
      await page.keyboard.press("Enter");
      await page.keyboard.press("Escape");
      await expect(page.getByRole("dialog")).toBeHidden();
      await page.getByTestId("wb-snippets-search").focus();
      await page.keyboard.press("Tab");
      await expect(page.getByTestId("wb-snippets-sort")).toBeFocused();
      await page.keyboard.press("Tab");
      await expect(page.getByTestId("wb-snippets-refresh")).toBeFocused();
      await page.keyboard.press("Tab");
      await expect(page.getByTestId("wb-snippet-edit")).toBeFocused();
      await page.keyboard.press("Tab");
      await expect(page.getByTestId("wb-snippet-delete")).toBeFocused();
    },
  );

  snipTest(
    "export writes a CSV to the path the save panel returns",
    async ({ page, ipc, hub, snips }) => {
      snips.seed({ trigger: "my address", body: ADDRESS });
      await openSnippets(page, ipc, hub, snips);
      ipc.respond("plugin:dialog|save", "/Users/test/Desktop/snippets.csv");
      ipc.respond("fork_snippets_export", 1);
      await page.getByTestId("wb-snippets-export").click();
      await ipc.expectCalled("fork_snippets_export", {
        path: "/Users/test/Desktop/snippets.csv",
      });
      await expect(page.getByText("Exported 1 snippet.")).toBeVisible();
    },
  );

  snipTest(
    "RTL: the page mirrors and stays accessible",
    async ({ page, ipc, hub, snips }) => {
      snips.seed({ trigger: "my address", body: ADDRESS }, { hit_count: 1 });
      await openSnippets(page, ipc, hub, snips, {
        handy: { app_language: "ar" },
      });
      await expect(page.getByTestId("wb-hub")).toHaveAttribute("dir", "rtl");
      await expect(rows(page)).toHaveCount(1);
      await expectNoSeriousAxeFindings(page);
      await shot(page, "rtl");
    },
  );
});

// ---------------------------------------------------------------- import

const CSV =
  'trigger,text\r\nmy sign-off,"Best wishes,\nFinn"\r\nmy email,a@example.com\r\nmy address,"12 Harbour Lane\nKiel"\r\n,"no trigger"\r\n';

function review(): SnippetImportReview {
  const row = (r: number, trigger: string, body: string) => ({
    row: r,
    trigger,
    body,
  });
  return {
    new_snippets: [row(2, "my sign-off", "Best wishes,\nFinn")],
    duplicates: [
      { incoming: row(3, "my email", "a@example.com"), in_file: false },
    ],
    conflicts: [
      {
        incoming: row(4, "my address", "12 Harbour Lane\nKiel"),
        existing: {
          id: 9,
          trigger: "my address",
          body: "Old Street 1",
          source: "user",
          created_at: 1,
          updated_at: 1,
          hit_count: 0,
          last_hit_at: null,
        },
      },
    ],
    invalid: [{ row: 5, trigger: "", problem: "empty_trigger" }],
    room: 997,
    ignored_rows: 0,
  };
}

function installImport(ipc: IpcMock) {
  ipc.respond("plugin:dialog|open", "/Users/test/snippets.csv");
  ipc.handle("fork_dictionary_read_file", (a) => ({
    status: "read",
    name: String(a.path).split("/").pop(),
    text: CSV,
  }));
  ipc.respond("fork_snippets_import_review", review());
  ipc.respond("fork_snippets_import", {
    added: 1,
    replaced: 1,
    skipped: 1,
    kept: 0,
    invalid: 1,
    over_limit: 0,
  });
}

const wizard = (page: Page) => page.getByTestId("wb-import-wizard");

snipTest.describe("import wizard", () => {
  snipTest(
    "four steps with multi-line cells: choose, match columns, review, done",
    async ({ page, ipc, hub, snips }) => {
      await openSnippets(page, ipc, hub, snips);
      installImport(ipc);
      await page.getByTestId("wb-snippets-import").click();
      await expect(
        page.getByRole("dialog", { name: "Import snippets" }),
      ).toBeVisible();
      await expect(wizard(page)).toHaveAttribute("data-step", "choose");
      await page.getByTestId("wb-import-wizard-format").click();
      await expect(page.getByTestId("wb-snippets-format")).toBeVisible();
      await shot(page, "import-1-choose");

      await page.getByTestId("wb-import-wizard-browse").click();
      await ipc.expectCalled("fork_dictionary_read_file", {
        path: "/Users/test/snippets.csv",
      });
      await expect(wizard(page)).toHaveAttribute("data-step", "columns");
      await expect(page.getByTestId("wb-import-wizard-header")).toBeChecked();
      await expect(
        page.getByTestId("wb-import-wizard-column-trigger"),
      ).toHaveText("Column A: trigger");
      await expect(page.getByTestId("wb-import-wizard-column-body")).toHaveText(
        "Column B: text",
      );
      await shot(page, "import-2-columns");

      await page.getByTestId("wb-import-wizard-next").click();
      await expect(wizard(page)).toHaveAttribute("data-step", "review");
      const sent = (await ipc.calls("fork_snippets_import_review"))[0].args
        .rows as SnippetImportRow[];
      expect(sent).toEqual([
        { row: 2, trigger: "my sign-off", body: "Best wishes,\nFinn" },
        { row: 3, trigger: "my email", body: "a@example.com" },
        { row: 4, trigger: "my address", body: "12 Harbour Lane\nKiel" },
        { row: 5, trigger: "", body: "no trigger" },
      ]);
      await expect(page.getByTestId("wb-import-count-new")).toHaveAttribute(
        "data-count",
        "1",
      );
      await expect(
        page.getByTestId("wb-import-count-conflicts"),
      ).toHaveAttribute("data-count", "1");
      await page.getByTestId("wb-conflict-4").check();
      await page.getByText("Show the rows that can't be imported").click();
      await expect(page.getByTestId("wb-snippets-invalid")).toContainText(
        "Type what you will say first.",
      );
      await shot(page, "import-3-review");

      await page.getByTestId("wb-import-wizard-import").click();
      await ipc.expectCalled("fork_snippets_import", { replaceRows: [4] });
      await expect(wizard(page)).toHaveAttribute("data-step", "done");
      await expect(page.getByTestId("wb-import-added")).toHaveText(
        "2 snippets imported",
      );
      await shot(page, "import-4-done");
      await page.getByTestId("wb-import-wizard-done").click();
      await expect(wizard(page)).toBeHidden();
    },
  );

  snipTest(
    "a .csv dropped on the import step is imported, never transcribed (C-205)",
    async ({ page, ipc, hub, snips }) => {
      await openSnippets(page, ipc, hub, snips);
      installImport(ipc);
      await ipc.waitForListener("tauri://drag-drop");
      const files = ["/Users/test/Dropped snippets.csv"];
      await ipc.emit("tauri://drag-drop", {
        paths: files,
        position: { x: 600, y: 400 },
      });
      await page.getByTestId("wb-snippets-import").click();
      const zone = page.getByTestId("wb-import-wizard-drop");
      await expect(zone).toBeVisible();
      await page.waitForFunction(() =>
        document.getAnimations().every((a) => a.playState !== "running"),
      );
      const box = (await zone.boundingBox())!;
      const scale = await page.evaluate(() => window.devicePixelRatio || 1);
      const position = {
        x: (box.x + box.width / 2) * scale,
        y: (box.y + box.height / 2) * scale,
      };
      await ipc.emit("tauri://drag-enter", { paths: files, position });
      await expect(zone).toHaveAttribute("data-drop-active", "true");
      await ipc.emit("tauri://drag-drop", { paths: files, position });
      await ipc.expectCalled("fork_dictionary_read_file", { path: files[0] });
      await expect(wizard(page)).toHaveAttribute("data-step", "columns");
      await expect(
        page.getByText("Dropped snippets.csv: 4 rows"),
      ).toBeVisible();
      expect(await ipc.calls("fork_dictionary_read_file")).toHaveLength(1);
      expect(await ipc.commandNames()).not.toContain("transcribe_file");
    },
  );
});

for (const theme of ["light", "dark"] as const) {
  snipTest(
    `axe: page, dialog and every wizard step (${theme})`,
    async ({ page, ipc, hub, snips }) => {
      // Many axe passes: room for a loaded machine (they hit 30 s there).
      snipTest.setTimeout(180_000);
      await page.emulateMedia({ colorScheme: theme });
      snips.seed({ trigger: "my address", body: ADDRESS }, { hit_count: 3 });
      snips.seed({ trigger: "my email", body: "finn@example.com" });
      snips.seed(
        { trigger: "booking link", body: "https://example.com/book" },
        { source: "import" },
      );
      await openSnippets(page, ipc, hub, snips, { handy: { theme } });
      await expectNoSeriousAxeFindings(page);
      await shot(page, `list-${theme}`);

      await page.getByTestId("wb-snippets-add").click();
      // The dialog clears its fields when it opens; under the full parallel
      // suite that effect could land after the fills, leaving Save disabled.
      // Fill until the values stick.
      await expect(async () => {
        await page.getByTestId("wb-snippet-trigger").fill("my email");
        await page.getByTestId("wb-snippet-body").fill("other@example.com");
        await expect(page.getByTestId("wb-snippet-save")).toBeEnabled({
          timeout: 1_000,
        });
      }).toPass({ timeout: 15_000 });
      await page.getByTestId("wb-snippet-save").click();
      await expect(page.getByTestId("wb-snippet-feedback")).toBeVisible();
      await expectNoSeriousAxeFindings(page);
      await shot(page, `dialog-${theme}`);
      await page.keyboard.press("Escape");

      installImport(ipc);
      await page.getByTestId("wb-snippets-import").click();
      await page.getByTestId("wb-import-wizard-format").click();
      await expectNoSeriousAxeFindings(page);
      await page.getByTestId("wb-import-wizard-browse").click();
      await expect(wizard(page)).toHaveAttribute("data-step", "columns");
      await expectNoSeriousAxeFindings(page);
      await page.getByTestId("wb-import-wizard-next").click();
      await page.getByText("Show the rows that can't be imported").click();
      await page.getByText("Show the new snippets").click();
      await expectNoSeriousAxeFindings(page);
      await page.getByTestId("wb-import-wizard-import").click();
      await expect(wizard(page)).toHaveAttribute("data-step", "done");
      await expectNoSeriousAxeFindings(page);
    },
  );
}
