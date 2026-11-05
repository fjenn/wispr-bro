// SPDX-License-Identifier: GPL-3.0-or-later
// S-16 Dictionary page: tabs, search, sort, refresh, the Add word dialog
// (disabled until filled, 60-character limit, duplicates and conflicts),
// edit, delete with undo, live counts, the banner, the four-step import
// wizard (browse, drop, column mapping, review with conflicts, done; a
// `.csv` dropped on the import step is imported, never transcribed),
// export, keyboard use, RTL and axe in light and dark (R5 S-16 tests, §5.5,
// §11.2). Mocked IPC with a small in-test backend; the store's real rules
// are tested in Rust (`fork::dictionary::tests`).
//
// WB_SHOTS_DIR=<dir> also writes screenshots of each state.
import path from "node:path";
import type { Page } from "@playwright/test";
import type {
  DictionaryEntry,
  EntryInput,
  ImportReview,
  ImportRow,
  SaveOutcome,
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
  // Evidence only: a slow or failed capture never fails the test.
  await page
    .screenshot({
      path: path.join(shotsDir, `s16-${name}.png`),
      animations: "disabled",
      timeout: 10_000,
    })
    .catch((e) => console.warn(`screenshot ${name} skipped: ${e}`));
}

/** Clicks a tab and waits until it is selected (retries a lost click). */
async function selectTab(page: Page, name: RegExp) {
  const t = page.getByRole("tab", { name });
  await expect(async () => {
    await t.click();
    await expect(t).toHaveAttribute("aria-selected", "true", { timeout: 1000 });
  }).toPass();
}

/** A small stand-in for `fork::dictionary` (key = kind + lowercase text). */
class FakeDictionary {
  entries: DictionaryEntry[] = [];
  nextId = 1;
  clock = 1_700_000_000_000;

  seed(input: EntryInput, over: Partial<DictionaryEntry> = {}) {
    const e = this.make(input);
    Object.assign(e, over);
    this.entries.push(e);
    return e;
  }

  make(input: EntryInput): DictionaryEntry {
    this.clock += 1000;
    const replacement = input.replacement?.trim() || null;
    return {
      id: this.nextId++,
      kind: replacement ? "rule" : "word",
      phrase: input.phrase.trim(),
      replacement,
      source: "user",
      status: "active",
      created_at: this.clock,
      updated_at: this.clock,
      hit_count: 0,
      last_hit_at: null,
    };
  }

  list() {
    const active = this.entries.filter((e) => e.status === "active");
    return {
      entries: [...this.entries].sort((a, b) => b.created_at - a.created_at),
      counts: {
        words: active.filter((e) => e.kind === "word").length,
        rules: active.filter((e) => e.kind === "rule").length,
        suggested: this.entries.filter((e) => e.status === "suggested").length,
      },
    };
  }

  check(input: EntryInput, exceptId?: number): SaveOutcome | null {
    const phrase = input.phrase.trim();
    if (!phrase) return { status: "invalid", problem: "empty" };
    if ([...phrase].length > 60)
      return { status: "invalid", problem: "too_long" };
    const kind = input.replacement?.trim() ? "rule" : "word";
    const other = this.entries.find(
      (e) =>
        e.id !== exceptId &&
        e.status === "active" &&
        e.kind === kind &&
        e.phrase.toLowerCase() === phrase.toLowerCase(),
    );
    if (!other) return null;
    const same =
      kind === "word"
        ? other.phrase === phrase
        : other.replacement === input.replacement?.trim();
    return same
      ? { status: "duplicate", existing: other }
      : { status: "conflict", existing: other };
  }

  add(input: EntryInput): SaveOutcome {
    const problem = this.check(input);
    if (problem) return problem;
    const entry = this.make(input);
    this.entries.push(entry);
    return { status: "saved", entry };
  }

  update(id: number, input: EntryInput): SaveOutcome {
    const at = this.entries.findIndex((e) => e.id === id);
    if (at < 0) return { status: "missing" };
    const problem = this.check(input, id);
    if (problem) return problem;
    const next = {
      ...this.make(input),
      id,
      created_at: this.entries[at].created_at,
    };
    this.entries[at] = next;
    return { status: "saved", entry: next };
  }

  install(ipc: IpcMock) {
    ipc.handle("fork_dictionary_list", () => this.list());
    ipc.handle("fork_dictionary_add", (a) => this.add(a.input as EntryInput));
    ipc.handle("fork_dictionary_update", (a) =>
      this.update(a.id as number, a.input as EntryInput),
    );
    ipc.handle("fork_dictionary_delete", (a) => {
      const ids = a.ids as number[];
      const gone = this.entries.filter((e) => ids.includes(e.id));
      this.entries = this.entries.filter((e) => !ids.includes(e.id));
      return gone;
    });
    ipc.handle("fork_dictionary_restore", (a) => {
      const back = a.entries as DictionaryEntry[];
      this.entries.push(...back);
      return back.length;
    });
    ipc.handle("fork_dictionary_accept", (a) => {
      const ids = a.ids as number[];
      let accepted = 0;
      for (const e of this.entries) {
        if (ids.includes(e.id) && e.status === "suggested") {
          e.status = "active";
          accepted += 1;
        }
      }
      return { accepted, full: 0 };
    });
    ipc.handle("fork_dictionary_reject", (a) => {
      const ids = a.ids as number[];
      const before = this.entries.length;
      this.entries = this.entries.filter(
        (e) => !(ids.includes(e.id) && e.status === "suggested"),
      );
      return before - this.entries.length;
    });
  }
}

/** Picks a sort order in the sort button's menu (flowmatch). */
async function pickSort(page: Page, value: string) {
  await page.getByTestId("wb-dictionary-sort").click();
  await page.getByTestId("wb-menu-item-wb-dictionary-sort-" + value).click();
  await expect(
    page.getByTestId("wb-menu-item-wb-dictionary-sort-" + value),
  ).toHaveCount(0);
}

const dictTest = test.extend<{ dict: FakeDictionary }>({
  dict: async ({}, use) => {
    await use(new FakeDictionary());
  },
});

async function openDictionary(
  page: Page,
  ipc: IpcMock,
  hub: HubState,
  dict: FakeDictionary,
  options: Parameters<typeof openHub>[3] = {},
) {
  await openHub(page, ipc, hub, { hash: "#/hub/dictionary", ...options });
  dict.install(ipc);
  await page.reload();
  await expect(page.getByTestId("wb-dictionary")).toBeVisible();
}

const rows = (page: Page) => page.getByTestId("wb-dictionary-entry");
const tab = (page: Page, name: RegExp) => page.getByRole("tab", { name });

dictTest.describe("dictionary page", () => {
  dictTest(
    "empty page: banner with suggestion chips, empty state, tabs at zero",
    async ({ page, ipc, hub, dict }) => {
      await openDictionary(page, ipc, hub, dict);
      await expect(page.getByTestId("wb-page-title")).toHaveText("Dictionary");
      const banner = page.getByTestId("wb-banner-dictionary.teach");
      await expect(banner).toContainText("Wispr Bro spells it your way.");
      // flowmatch: no History yet, so the chips are examples; a chip opens
      // Add word with it filled in.
      await expect(
        banner
          .getByRole("list", { name: "Words to add" })
          .getByRole("listitem"),
      ).toHaveText(["Siobhán", "Kubernetes", "Anna-Lena"]);
      await page.getByTestId("wb-dictionary-suggestion").first().click();
      await expect(page.getByTestId("wb-dictionary-phrase")).toHaveValue(
        "Siobhán",
      );
      await page.keyboard.press("Escape");
      await expect(page.getByRole("dialog")).toBeHidden();
      await expect(page.getByTestId("wb-dictionary-empty")).toBeVisible();
      await expect(tab(page, /^All$/)).toHaveAttribute("aria-selected", "true");
      await expect(page.getByTestId("wb-dictionary-export")).toBeDisabled();
      await selectTab(page, /Suggested/);
      await expect(
        page.getByTestId("wb-dictionary-empty-suggested"),
      ).toBeVisible();
      await shot(page, "empty");
    },
  );

  dictTest(
    "the banner dismissal persists",
    async ({ page, ipc, hub, dict }) => {
      await openDictionary(page, ipc, hub, dict);
      const banner = page.getByTestId("wb-banner-dictionary.teach");
      await banner.getByRole("button", { name: "Dismiss" }).click();
      await expect(banner).toBeHidden();
      await ipc.expectCalled("fork_set_setting");
      expect(hub.forkSettings.dismissed_banners).toContain("dictionary.teach");
      await page.reload();
      await expect(page.getByTestId("wb-dictionary")).toBeVisible();
      await expect(banner).toBeHidden();
    },
  );

  dictTest(
    "banner chips suggest names from recent History not in the dictionary",
    async ({ page, ipc, hub, dict }) => {
      dict.seed({ phrase: "Kubernetes", replacement: null });
      await openDictionary(page, ipc, hub, dict, {
        beforeNavigate: () =>
          ipc.respond("fork_history_page", {
            rows: [
              { id: 1, created_at: 1, text: "Ask Priyanka about Kubernetes." },
              {
                id: 2,
                created_at: 2,
                text: "Priyanka moved the Zephyr launch.",
              },
            ],
            next: null,
          }),
      });
      const chips = page.getByTestId("wb-dictionary-suggestion");
      await expect(chips).toHaveText(["Priyanka", "Zephyr"]);
    },
  );

  dictTest(
    "Add word: disabled until filled, non-ASCII and apostrophes, then listed",
    async ({ page, ipc, hub, dict }) => {
      await openDictionary(page, ipc, hub, dict);
      await page.getByTestId("wb-dictionary-add").click();
      const dialog = page.getByRole("dialog", { name: "Add to dictionary" });
      await expect(dialog).toBeVisible();
      const save = page.getByTestId("wb-dictionary-save");
      await expect(save).toBeDisabled();
      await expect(page.getByTestId("wb-dictionary-phrase")).toBeFocused();
      await page.getByTestId("wb-dictionary-phrase").fill("   ");
      await expect(save).toBeDisabled();
      await page.getByTestId("wb-dictionary-phrase").fill("Siobhán");
      await expect(save).toBeEnabled();
      await shot(page, "add-word");
      await save.click();
      await expect(dialog).toBeHidden();
      await ipc.expectCalled("fork_dictionary_add", {
        input: { phrase: "Siobhán", replacement: null },
      });
      await expect(rows(page)).toHaveCount(1);
      await expect(rows(page).first()).toContainText("Siobhán");
      await expect(page.getByText("Added “Siobhán”.")).toBeVisible();
      await expect(tab(page, /^All$/)).toBeVisible();

      // An apostrophe survives (Handy's old editor stripped it, C-58).
      await page.getByTestId("wb-dictionary-add").click();
      await page.getByTestId("wb-dictionary-phrase").fill("O'Brien");
      await page.keyboard.press("Enter");
      await expect(rows(page)).toHaveCount(2);
      await expect(rows(page).first()).toContainText("O'Brien");
    },
  );

  dictTest(
    "Replace a word: both fields needed; the row shows from → to",
    async ({ page, ipc, hub, dict }) => {
      await openDictionary(page, ipc, hub, dict);
      await page.getByTestId("wb-dictionary-add").click();
      await page
        .getByRole("switch", { name: "Replace a word" })
        .check({ force: true });
      const save = page.getByTestId("wb-dictionary-save");
      await page.getByLabel("When I say").fill("gonna");
      await expect(save).toBeDisabled();
      await page.getByLabel("Write", { exact: true }).fill("going to");
      await expect(save).toBeEnabled();
      await shot(page, "add-rule");
      await save.click();
      await ipc.expectCalled("fork_dictionary_add", {
        input: { phrase: "gonna", replacement: "going to" },
      });
      const row = rows(page).first();
      await expect(row).toHaveAttribute("data-kind", "rule");
      await expect(row.getByTestId("wb-dictionary-phrase-text")).toHaveText(
        "gonna",
      );
      await expect(
        row.getByTestId("wb-dictionary-replacement-text"),
      ).toHaveText("going to");
    },
  );

  dictTest(
    "60 characters at most; the counter and the button say so",
    async ({ page, ipc, hub, dict }) => {
      await openDictionary(page, ipc, hub, dict);
      await page.getByTestId("wb-dictionary-add").click();
      await page.getByTestId("wb-dictionary-phrase").fill("é".repeat(60));
      await expect(page.getByTestId("wb-dictionary-counter")).toHaveText(
        "60/60",
      );
      await expect(page.getByTestId("wb-dictionary-save")).toBeEnabled();
      await page.getByTestId("wb-dictionary-phrase").fill("é".repeat(61));
      await expect(page.getByTestId("wb-dictionary-counter")).toHaveText(
        "61/60",
      );
      await expect(page.getByTestId("wb-dictionary-phrase")).toHaveAttribute(
        "aria-invalid",
        "true",
      );
      await expect(page.getByTestId("wb-dictionary-save")).toBeDisabled();
    },
  );

  dictTest(
    "duplicates and conflicts are explained; Replace it edits the existing entry",
    async ({ page, ipc, hub, dict }) => {
      dict.seed({ phrase: "Kubernetes", replacement: null });
      const rule = dict.seed({ phrase: "gonna", replacement: "going to" });
      await openDictionary(page, ipc, hub, dict);
      await page.getByTestId("wb-dictionary-add").click();
      await page.getByTestId("wb-dictionary-phrase").fill("Kubernetes");
      await page.getByTestId("wb-dictionary-save").click();
      const feedback = page.getByTestId("wb-dictionary-feedback");
      await expect(feedback).toHaveAttribute("data-kind", "duplicate");
      await expect(feedback).toHaveText(
        "“Kubernetes” is already in your dictionary.",
      );

      await page
        .getByRole("switch", { name: "Replace a word" })
        .check({ force: true });
      await page.getByLabel("When I say").fill("Gonna");
      await page.getByLabel("Write", { exact: true }).fill("going to be");
      await page.getByTestId("wb-dictionary-save").click();
      await expect(feedback).toHaveAttribute("data-kind", "conflict");
      await expect(feedback).toContainText(
        "“gonna” already becomes “going to”.",
      );
      await shot(page, "conflict");
      await page.getByTestId("wb-dictionary-replace-existing").click();
      await ipc.expectCalled("fork_dictionary_update", { id: rule.id });
      await expect(page.getByRole("dialog")).toBeHidden();
      await expect(
        page.getByTestId("wb-dictionary-replacement-text"),
      ).toHaveText("going to be");
    },
  );

  dictTest("edit, delete and undo", async ({ page, ipc, hub, dict }) => {
    dict.seed({ phrase: "Siobhan", replacement: null });
    await openDictionary(page, ipc, hub, dict);
    await page.getByRole("button", { name: "Edit Siobhan" }).click();
    const dialog = page.getByRole("dialog", { name: "Edit dictionary entry" });
    await expect(page.getByTestId("wb-dictionary-phrase")).toHaveValue(
      "Siobhan",
    );
    await page.getByTestId("wb-dictionary-phrase").fill("Siobhán");
    await dialog.getByRole("button", { name: "Save" }).click();
    await ipc.expectCalled("fork_dictionary_update", {
      input: { phrase: "Siobhán", replacement: null },
    });
    await expect(rows(page).first()).toContainText("Siobhán");

    await page.getByRole("button", { name: "Delete Siobhán" }).click();
    await expect(rows(page)).toHaveCount(0);
    await expect(page.getByText("Deleted “Siobhán”.")).toBeVisible();
    await page.getByRole("button", { name: "Undo" }).click();
    await ipc.expectCalled("fork_dictionary_restore");
    await expect(rows(page)).toHaveCount(1);
  });

  // Audit OT2: Undo's result is checked; a failure or a skip is said.
  dictTest(
    "a failed or skipped undo says so",
    async ({ page, ipc, hub, dict }) => {
      dict.seed({ phrase: "Siobhan", replacement: null });
      dict.seed({ phrase: "Kubernetes", replacement: null });
      await openDictionary(page, ipc, hub, dict);
      ipc.respond(
        "fork_dictionary_restore",
        ipcError({ kind: "Internal", message: "disk full" }),
      );
      await page.getByRole("button", { name: "Delete Siobhan" }).click();
      await page
        .getByRole("listitem")
        .filter({ hasText: "Deleted “Siobhan”." })
        .getByRole("button", { name: "Undo" })
        .click();
      await expect(
        page.getByText("“Siobhan” couldn't be restored. Please add it again."),
      ).toBeVisible();
      ipc.respond("fork_dictionary_restore", 0);
      await page.getByRole("button", { name: "Delete Kubernetes" }).click();
      await page
        .getByRole("listitem")
        .filter({ hasText: "Deleted “Kubernetes”." })
        .getByRole("button", { name: "Undo" })
        .click();
      await expect(
        page.getByText(
          "“Kubernetes” wasn't restored: your dictionary already has it.",
        ),
      ).toBeVisible();
    },
  );

  dictTest(
    "search ignores case and accents; sort; tabs by source",
    async ({ page, ipc, hub, dict }) => {
      dict.seed({ phrase: "Siobhán", replacement: null });
      dict.seed({ phrase: "Anna", replacement: "Anna-Lena" }, { hit_count: 7 });
      dict.seed({ phrase: "Zoë", replacement: null }, { source: "migrated" });
      dict.seed(
        { phrase: "Kubernetees", replacement: null },
        { source: "learn" },
      );
      dict.seed(
        { phrase: "postgress", replacement: null },
        { status: "suggested", source: "learn" },
      );
      await openDictionary(page, ipc, hub, dict);
      await expect(tab(page, /^All$/)).toBeVisible();
      await expect(tab(page, /^Added by you$/)).toBeVisible();
      await expect(tab(page, /^Suggested$/)).toBeVisible();
      await expect(page.getByText("Used 7 times")).toBeVisible();
      await expect(
        page.getByText("From your earlier custom words"),
      ).toBeVisible();

      const phrases = () =>
        page.getByTestId("wb-dictionary-phrase-text").allTextContents();
      expect(await phrases()).toEqual([
        "Kubernetees",
        "Zoë",
        "Anna",
        "Siobhán",
      ]);
      await pickSort(page, "az");
      expect(await phrases()).toEqual([
        "Anna",
        "Kubernetees",
        "Siobhán",
        "Zoë",
      ]);
      await pickSort(page, "used");
      expect((await phrases())[0]).toBe("Anna");

      await page.getByTestId("wb-dictionary-search-open").click();
      await page.getByTestId("wb-dictionary-search").fill("SIOBHAN");
      await expect(rows(page)).toHaveCount(1);
      await page.getByTestId("wb-dictionary-search").fill("lena");
      await expect(rows(page)).toHaveCount(1);
      await page.getByTestId("wb-dictionary-search").fill("nothing here");
      await expect(page.getByTestId("wb-dictionary-no-match")).toBeVisible();
      await page.getByRole("button", { name: "Clear the search" }).click();
      await expect(rows(page)).toHaveCount(4);

      await selectTab(page, /Added by you/);
      await expect(rows(page)).toHaveCount(3);
      await selectTab(page, /Suggested/);
      await expect(rows(page)).toHaveCount(1);
      await page
        .getByRole("button", { name: "Add postgress to your dictionary" })
        .click();
      await ipc.expectCalled("fork_dictionary_accept", { ids: [5] });
    },
  );

  dictTest(
    "counts and rows update live; Refresh re-reads",
    async ({ page, ipc, hub, dict }) => {
      await openDictionary(page, ipc, hub, dict);
      await ipc.waitForListener("fork-dictionary-changed");
      // Another window (or Handy's own settings) added a word.
      dict.seed({ phrase: "Anna-Lena", replacement: null });
      await ipc.emit("fork-dictionary-changed", {
        counts: { words: 1, rules: 0, suggested: 0 },
      });
      await expect(rows(page)).toHaveCount(1);
      await expect(tab(page, /^All$/)).toBeVisible();
      // A take used a replacement: its hit count shows after the stats event.
      const r = dict.seed({ phrase: "gonna", replacement: "going to" });
      await ipc.emit("fork-stats-updated", { day: "2026-10-30" });
      await expect(rows(page)).toHaveCount(2);
      r.hit_count = 2;
      const before = (await ipc.calls("fork_dictionary_list")).length;
      await page.getByTestId("wb-dictionary-refresh").click();
      await expect
        .poll(async () => (await ipc.calls("fork_dictionary_list")).length)
        .toBeGreaterThan(before);
      await expect(page.getByText("Used 2 times")).toBeVisible();
    },
  );

  dictTest(
    "keyboard only: add a word and close the dialog",
    async ({ page, ipc, hub, dict }) => {
      await openDictionary(page, ipc, hub, dict);
      await page.getByTestId("wb-dictionary-import").focus();
      await page.keyboard.press("Tab");
      await expect(page.getByTestId("wb-dictionary-add")).toBeFocused();
      await page.keyboard.press("Enter");
      await expect(page.getByTestId("wb-dictionary-phrase")).toBeFocused();
      await page.keyboard.type("Kubernetes");
      await page.keyboard.press("Enter");
      await expect(rows(page)).toHaveCount(1);
      await expect(page.getByTestId("wb-dictionary-add")).toBeFocused();
      await page.keyboard.press("Enter");
      await page.keyboard.press("Escape");
      await expect(page.getByRole("dialog")).toBeHidden();
      // The row's buttons are reachable and named.
      await page.getByTestId("wb-dictionary-search-open").focus();
      await page.keyboard.press("Tab");
      await expect(page.getByTestId("wb-dictionary-sort")).toBeFocused();
      await page.keyboard.press("Tab");
      await expect(page.getByTestId("wb-dictionary-refresh")).toBeFocused();
    },
  );

  dictTest(
    "export writes a CSV to the path the save panel returns",
    async ({ page, ipc, hub, dict }) => {
      dict.seed({ phrase: "Siobhán", replacement: null });
      await openDictionary(page, ipc, hub, dict);
      ipc.respond("plugin:dialog|save", "/Users/test/Desktop/words.csv");
      ipc.respond("fork_dictionary_export", 1);
      await page.getByTestId("wb-dictionary-export").click();
      await ipc.expectCalled("fork_dictionary_export", {
        path: "/Users/test/Desktop/words.csv",
      });
      await expect(page.getByText("Exported 1 entry.")).toBeVisible();
    },
  );

  dictTest(
    "RTL: the page mirrors and stays accessible",
    async ({ page, ipc, hub, dict }) => {
      dict.seed({ phrase: "gonna", replacement: "going to" });
      await openDictionary(page, ipc, hub, dict, {
        handy: { app_language: "ar" },
      });
      await expect(page.getByTestId("wb-hub")).toHaveAttribute("dir", "rtl");
      await expect(rows(page)).toHaveCount(1);
      await expectNoSeriousAxeFindings(page);
    },
  );
});

// ---------------------------------------------------------------- import

const CSV =
  'phrase,replacement\r\nSiobhán,\r\nKubernetes,\r\nwanna,want to\r\nO\'Brien,\r\n"say, ""hi""",Hi\r\n';

function review(): ImportReview {
  const row = (
    r: number,
    phrase: string,
    replacement: string | null = null,
  ) => ({
    row: r,
    kind: replacement ? ("rule" as const) : ("word" as const),
    phrase,
    replacement,
  });
  return {
    new_entries: [
      row(2, "Siobhán"),
      row(5, "O'Brien"),
      row(6, 'say, "hi"', "Hi"),
    ],
    duplicates: [{ incoming: row(3, "Kubernetes"), in_file: false }],
    conflicts: [
      {
        incoming: row(4, "wanna", "want to"),
        existing: {
          id: 9,
          kind: "rule",
          phrase: "wanna",
          replacement: "wants to",
          source: "user",
          status: "active",
          created_at: 1,
          updated_at: 1,
          hit_count: 0,
          last_hit_at: null,
        },
      },
    ],
    invalid: [{ row: 7, phrase: "x".repeat(61), problem: "too_long" }],
    room: 4990,
  };
}

function installImport(ipc: IpcMock) {
  ipc.respond("plugin:dialog|open", "/Users/test/words.csv");
  ipc.handle("fork_dictionary_read_file", (a) => ({
    status: "read",
    name: String(a.path).split("/").pop(),
    text: CSV,
  }));
  ipc.respond("fork_dictionary_import_review", review());
  ipc.respond("fork_dictionary_import", {
    added: 3,
    replaced: 1,
    skipped: 1,
    kept: 0,
    invalid: 1,
    over_limit: 0,
  });
}

const wizard = (page: Page) => page.getByTestId("wb-import-wizard");

dictTest.describe("import wizard", () => {
  dictTest(
    "four steps: choose, match columns, review conflicts, done",
    async ({ page, ipc, hub, dict }) => {
      await openDictionary(page, ipc, hub, dict);
      installImport(ipc);
      await page.getByTestId("wb-dictionary-import").click();
      await expect(wizard(page)).toHaveAttribute("data-step", "choose");
      await expect(
        page.getByRole("dialog", { name: "Import dictionary" }),
      ).toBeVisible();
      await expect(page.getByTestId("wb-import-wizard-step-title")).toHaveText(
        "Step 1 of 4: Choose a file",
      );
      await page.getByTestId("wb-import-wizard-format").click();
      await expect(page.getByTestId("wb-dictionary-format")).toBeVisible();
      await expect(page.getByTestId("wb-import-wizard-format")).toHaveAttribute(
        "aria-expanded",
        "true",
      );
      await shot(page, "import-1-choose");

      await page.getByTestId("wb-import-wizard-browse").click();
      await ipc.expectCalled("fork_dictionary_read_file", {
        path: "/Users/test/words.csv",
      });
      await expect(wizard(page)).toHaveAttribute("data-step", "columns");
      await expect(page.getByTestId("wb-import-wizard-header")).toBeChecked();
      await expect(
        page.getByTestId("wb-import-wizard-column-phrase"),
      ).toHaveText("Column A: phrase");
      await expect(
        page.getByTestId("wb-import-wizard-column-replacement"),
      ).toHaveText("Column B: replacement");
      const preview = page.getByTestId("wb-import-wizard-preview");
      await expect(preview.locator("tbody tr")).toHaveCount(5);
      await expect(preview.locator("tbody tr").nth(4)).toContainText(
        'say, "hi"',
      );
      await shot(page, "import-2-columns");

      // Without a header row the first record becomes data.
      await page.getByTestId("wb-import-wizard-header").uncheck();
      await expect(preview.locator("tbody tr").first()).toContainText("phrase");
      await page.getByTestId("wb-import-wizard-header").check();

      // The column pickers are listboxes: a column can be left out and
      // picked again.
      const replacementColumn = page.getByTestId(
        "wb-import-wizard-column-replacement",
      );
      await replacementColumn.click();
      await page
        .getByTestId("wb-import-wizard-column-replacement-option-none")
        .click();
      await expect(replacementColumn).toHaveText("Not in this file");
      await expect(preview.locator("tbody tr").nth(2)).not.toContainText(
        "want to",
      );
      await replacementColumn.focus();
      await page.keyboard.press("Enter");
      await page.getByRole("option", { name: "Column B: replacement" }).click();
      await expect(replacementColumn).toHaveText("Column B: replacement");
      await expect(preview.locator("tbody tr").nth(2)).toContainText("want to");

      await page.getByTestId("wb-import-wizard-next").click();
      await expect(wizard(page)).toHaveAttribute("data-step", "review");
      const sent = (await ipc.calls("fork_dictionary_import_review"))[0].args
        .rows as ImportRow[];
      expect(sent).toEqual([
        { row: 2, phrase: "Siobhán", replacement: null },
        { row: 3, phrase: "Kubernetes", replacement: null },
        { row: 4, phrase: "wanna", replacement: "want to" },
        { row: 5, phrase: "O'Brien", replacement: null },
        { row: 6, phrase: 'say, "hi"', replacement: "Hi" },
      ]);
      await expect(page.getByTestId("wb-import-count-new")).toHaveAttribute(
        "data-count",
        "3",
      );
      await expect(
        page.getByTestId("wb-import-count-duplicates"),
      ).toHaveAttribute("data-count", "1");
      await expect(
        page.getByTestId("wb-import-count-conflicts"),
      ).toHaveAttribute("data-count", "1");
      await expect(page.getByTestId("wb-import-count-invalid")).toHaveAttribute(
        "data-count",
        "1",
      );
      await page.getByTestId("wb-conflict-4").check();
      await page.getByText("Show the rows that can't be imported").click();
      await expect(page.getByTestId("wb-dictionary-invalid")).toContainText(
        "Words can be up to 60 characters.",
      );
      await shot(page, "import-3-review");

      await page.getByTestId("wb-import-wizard-import").click();
      await ipc.expectCalled("fork_dictionary_import", { replaceRows: [4] });
      await expect(wizard(page)).toHaveAttribute("data-step", "done");
      await expect(page.getByTestId("wb-import-added")).toHaveText(
        "4 entries imported",
      );
      await expect(page.getByTestId("wb-import-wizard-summary")).toContainText(
        "1 replaced with the file's version",
      );
      await shot(page, "import-4-done");
      await page.getByTestId("wb-import-wizard-done").click();
      await expect(wizard(page)).toBeHidden();
    },
  );

  dictTest(
    "a .csv dropped on the import step is imported, never transcribed (C-205)",
    async ({ page, ipc, hub, dict }) => {
      await openDictionary(page, ipc, hub, dict);
      installImport(ipc);
      await ipc.waitForListener("tauri://drag-drop");
      const files = ["/Users/test/Dropped words.csv"];
      // Dropped on the page itself: no zone there, ignored.
      await ipc.emit("tauri://drag-drop", {
        paths: files,
        position: { x: 600, y: 400 },
      });
      await page.getByTestId("wb-dictionary-import").click();
      const zone = page.getByTestId("wb-import-wizard-drop");
      await expect(zone).toBeVisible();
      // Measure after the dialog's open animation, or the point can miss.
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
      await expect(page.getByText("Dropped words.csv: 5 rows")).toBeVisible();
      expect(await ipc.calls("fork_dictionary_read_file")).toHaveLength(1);
      expect(await ipc.commandNames()).not.toContain("transcribe_file");
    },
  );

  dictTest(
    "files over 3 MB, other types and empty files are refused with a reason",
    async ({ page, ipc, hub, dict }) => {
      await openDictionary(page, ipc, hub, dict);
      ipc.respond("plugin:dialog|open", "/Users/test/words.csv");
      ipc.respondSequence("fork_dictionary_read_file", [
        { status: "too_large", bytes: 3_250_586 },
        { status: "not_csv" },
        { status: "read", name: "e.csv", text: "phrase,replacement\r\n" },
      ]);
      await page.getByTestId("wb-dictionary-import").click();
      const error = page.getByTestId("wb-import-wizard-error");
      await page.getByTestId("wb-import-wizard-browse").click();
      await expect(error).toHaveText(
        "That file is 3.1 MB. Files can be up to 3 MB.",
      );
      await page.getByTestId("wb-import-wizard-browse").click();
      await expect(error).toContainText("That isn't a .csv file.");
      await page.getByTestId("wb-import-wizard-browse").click();
      await expect(error).toHaveText("That file has no rows to import.");
      await expect(wizard(page)).toHaveAttribute("data-step", "choose");
    },
  );

  dictTest(
    "semicolon CSVs and a file without a header map by position",
    async ({ page, ipc, hub, dict }) => {
      await openDictionary(page, ipc, hub, dict);
      installImport(ipc);
      ipc.respond("fork_dictionary_read_file", {
        status: "read",
        name: "de.csv",
        text: "Straße;Strasse\nMüller;\n",
      });
      await page.getByTestId("wb-dictionary-import").click();
      await page.getByTestId("wb-import-wizard-browse").click();
      await expect(
        page.getByTestId("wb-import-wizard-header"),
      ).not.toBeChecked();
      await page.getByTestId("wb-import-wizard-next").click();
      const sent = (await ipc.calls("fork_dictionary_import_review"))[0].args
        .rows as ImportRow[];
      expect(sent).toEqual([
        { row: 1, phrase: "Straße", replacement: "Strasse" },
        { row: 2, phrase: "Müller", replacement: null },
      ]);
    },
  );
});

// Owner test 1: a save that fails says why and keeps the dialog open;
// a dictionary whose data file cannot be opened says so on the page.
dictTest.describe("dictionary failures", () => {
  dictTest(
    "a save that cannot reach the data file shows a clear message",
    async ({ page, ipc, hub, dict }) => {
      await openDictionary(page, ipc, hub, dict);
      ipc.handle("fork_dictionary_add", () =>
        ipcError({
          kind: "Internal",
          message: "fork.db migration to version 6 failed",
        }),
      );
      await page.getByTestId("wb-dictionary-add").click();
      await page.getByTestId("wb-dictionary-phrase").fill("Siobhán");
      await page.getByTestId("wb-dictionary-save").click();
      const feedback = page.getByTestId("wb-dictionary-feedback");
      await expect(feedback).toHaveAttribute("data-kind", "failed");
      await expect(feedback).toHaveAttribute("role", "alert");
      await expect(feedback).toHaveText(
        "Not saved: Wispr Bro can't open its data file. Quit and reopen the app, then try again. If it keeps happening, choose Help › Report a problem.",
      );
      // The dialog stays open with the word, so nothing typed is lost.
      await expect(page.getByTestId("wb-dictionary-phrase")).toHaveValue(
        "Siobhán",
      );
      await expect(page.getByTestId("wb-dictionary-save")).toBeEnabled();
      await shot(page, "save-failed");

      ipc.handle("fork_dictionary_add", () => ipcError({ kind: "NotReady" }));
      await page.getByTestId("wb-dictionary-save").click();
      await expect(feedback).toHaveText(
        "Not saved yet: Wispr Bro is still starting. Try again in a moment.",
      );

      ipc.handle("fork_dictionary_add", (a) => dict.add(a.input as EntryInput));
      await page.getByTestId("wb-dictionary-save").click();
      await expect(page.getByTestId("wb-dictionary-dialog")).toBeHidden();
      await expect(rows(page)).toHaveCount(1);
    },
  );

  dictTest(
    "a dictionary that cannot be opened says so instead of 'not available yet'",
    async ({ page, ipc, hub, dict }) => {
      await openHub(page, ipc, hub, { hash: "#/hub/dictionary" });
      dict.install(ipc);
      ipc.handle("fork_dictionary_list", () =>
        ipcError({ kind: "Internal", message: "fork.db migration failed" }),
      );
      await page.reload();
      const broken = page.getByTestId("wb-dictionary-broken");
      await expect(broken).toBeVisible();
      await expect(broken).toContainText("Your dictionary can't be opened");
      await expect(broken).toContainText(
        "Wispr Bro can't read its data file right now.",
      );
      await expect(page.getByTestId("wb-dictionary-unavailable")).toHaveCount(
        0,
      );
      await shot(page, "broken");
      await expectNoSeriousAxeFindings(page);
    },
  );
});

// S-34: auto-learn suggestions (Accept / Reject) and the opt-in toggle.
dictTest.describe("dictionary suggestions (S-34)", () => {
  const seedSuggestions = (dict: FakeDictionary) => {
    dict.seed({ phrase: "Siobhán", replacement: null });
    dict.seed(
      { phrase: "Kubernetes", replacement: null },
      { status: "suggested", source: "learn" },
    );
    dict.seed(
      { phrase: "Aoife", replacement: "Aoibhe" },
      { status: "suggested", source: "learn" },
    );
  };

  dictTest(
    "the toggle is off by default and opts in from the Suggested tab",
    async ({ page, ipc, hub, dict }) => {
      await openDictionary(page, ipc, hub, dict);
      await selectTab(page, /Suggested/);
      const toggle = page.getByTestId("wb-dictionary-learn-toggle");
      await expect(toggle).not.toBeChecked();
      await expect(page.getByTestId("wb-dictionary-learn")).toContainText(
        "Only the word you fixed is kept",
      );
      await expect(
        page.getByTestId("wb-dictionary-empty-suggested"),
      ).toContainText("When Wispr Bro can learn");
      await toggle.click();
      await ipc.expectCalled("fork_set_setting", {
        update: { key: "auto_learn_enabled", value: true },
      });
      await expect(toggle).toBeChecked();
      expect(hub.forkSettings.auto_learn_enabled).toBe(true);
      await expect(
        page.getByTestId("wb-dictionary-empty-suggested"),
      ).toContainText("press Return, Tab or move on");
      // Only the Suggested tab carries the toggle.
      await selectTab(page, /^All/);
      await expect(page.getByTestId("wb-dictionary-learn")).toHaveCount(0);
      await shot(page, "learn-on");
    },
  );

  dictTest(
    "accept and reject a suggestion; suggested rows cannot be edited",
    async ({ page, ipc, hub, dict }) => {
      seedSuggestions(dict);
      hub.forkSettings.auto_learn_enabled = true;
      await openDictionary(page, ipc, hub, dict);
      await expect(tab(page, /^Suggested$/)).toBeVisible();
      await selectTab(page, /Suggested/);
      await expect(rows(page)).toHaveCount(2);
      await expect(rows(page).getByTestId("wb-dictionary-edit")).toHaveCount(0);
      await expect(rows(page).getByTestId("wb-dictionary-delete")).toHaveCount(
        0,
      );
      await shot(page, "suggested");

      await page
        .getByRole("button", { name: "Don't learn Aoife → Aoibhe" })
        .click();
      await ipc.expectCalled("fork_dictionary_reject", { ids: [3] });
      await expect(
        page.getByText("“Aoife → Aoibhe” won't be suggested again."),
      ).toBeVisible();
      await expect(rows(page)).toHaveCount(1);

      await page
        .getByRole("button", { name: "Add Kubernetes to your dictionary" })
        .click();
      await ipc.expectCalled("fork_dictionary_accept", { ids: [2] });
      await expect(page.getByText("Added “Kubernetes”.")).toBeVisible();
      await expect(
        page.getByTestId("wb-dictionary-empty-suggested"),
      ).toBeVisible();
      await expect(tab(page, /^All$/)).toBeVisible();
      // A learned word is not "Added by you".
      await expect(tab(page, /^Added by you$/)).toBeVisible();
    },
  );

  dictTest(
    "a full dictionary keeps the suggestion waiting",
    async ({ page, ipc, hub, dict }) => {
      seedSuggestions(dict);
      await openDictionary(page, ipc, hub, dict);
      ipc.respond("fork_dictionary_accept", { accepted: 0, full: 1 });
      await selectTab(page, /Suggested/);
      await page
        .getByRole("button", { name: "Add Kubernetes to your dictionary" })
        .click();
      await expect(
        page.getByText("Your dictionary is full. Delete some entries first."),
      ).toBeVisible();
    },
  );

  dictTest(
    "keyboard only: toggle, accept and reject",
    async ({ page, ipc, hub, dict }) => {
      seedSuggestions(dict);
      await openDictionary(page, ipc, hub, dict);
      await tab(page, /Suggested/).focus();
      await page.keyboard.press("Enter");
      await expect(tab(page, /Suggested/)).toHaveAttribute(
        "aria-selected",
        "true",
      );
      const toggle = page.getByTestId("wb-dictionary-learn-toggle");
      await toggle.focus();
      await page.keyboard.press("Space");
      await expect(toggle).toBeChecked();
      const accept = page.getByRole("button", {
        name: "Add Kubernetes to your dictionary",
      });
      await accept.focus();
      await page.keyboard.press("Enter");
      await ipc.expectCalled("fork_dictionary_accept", { ids: [2] });
      const reject = page.getByRole("button", {
        name: "Don't learn Aoife → Aoibhe",
      });
      await reject.focus();
      await page.keyboard.press("Enter");
      await ipc.expectCalled("fork_dictionary_reject", { ids: [3] });
    },
  );

  dictTest(
    "RTL mirrors the suggestion row",
    async ({ page, ipc, hub, dict }) => {
      seedSuggestions(dict);
      await openDictionary(page, ipc, hub, dict, {
        handy: { app_language: "ar" },
      });
      await expect(page.getByTestId("wb-hub")).toHaveAttribute("dir", "rtl");
      // Tab names are Arabic here: the third tab is Suggested.
      const suggestedTab = page.getByRole("tab").nth(2);
      await suggestedTab.click();
      await expect(suggestedTab).toHaveAttribute("aria-selected", "true");
      const row = rows(page).first();
      const text = await row
        .getByTestId("wb-dictionary-phrase-text")
        .boundingBox();
      const accept = await row
        .getByTestId("wb-dictionary-accept")
        .boundingBox();
      expect(text && accept && accept.x < text.x).toBe(true);
      await expectNoSeriousAxeFindings(page);
    },
  );

  for (const theme of ["light", "dark"] as const) {
    dictTest(
      `axe: the Suggested tab with suggestions (${theme})`,
      async ({ page, ipc, hub, dict }) => {
        await page.emulateMedia({ colorScheme: theme });
        seedSuggestions(dict);
        hub.forkSettings.auto_learn_enabled = true;
        await openDictionary(page, ipc, hub, dict, { handy: { theme } });
        await selectTab(page, /Suggested/);
        await expect(rows(page)).toHaveCount(2);
        await expectNoSeriousAxeFindings(page);
        await shot(page, `suggested-${theme}`);
      },
    );
  }
});

for (const theme of ["light", "dark"] as const) {
  dictTest(
    `axe: page, dialog and every wizard step (${theme})`,
    async ({ page, ipc, hub, dict }) => {
      // Many axe passes: room for a loaded machine (they hit 30 s there).
      dictTest.setTimeout(180_000);
      await page.emulateMedia({ colorScheme: theme });
      dict.seed({ phrase: "Siobhán", replacement: null });
      dict.seed({ phrase: "gonna", replacement: "going to" }, { hit_count: 3 });
      dict.seed({ phrase: "Zoë", replacement: null }, { source: "import" });
      await openDictionary(page, ipc, hub, dict, { handy: { theme } });
      await expectNoSeriousAxeFindings(page);
      await shot(page, `list-${theme}`);

      await page.getByTestId("wb-dictionary-add").click();
      await page
        .getByRole("switch", { name: "Replace a word" })
        .check({ force: true });
      await expectNoSeriousAxeFindings(page);
      await shot(page, `dialog-${theme}`);
      await page.keyboard.press("Escape");

      installImport(ipc);
      await page.getByTestId("wb-dictionary-import").click();
      await page.getByTestId("wb-import-wizard-format").click();
      await expectNoSeriousAxeFindings(page);
      await page.getByTestId("wb-import-wizard-browse").click();
      await expect(wizard(page)).toHaveAttribute("data-step", "columns");
      await expectNoSeriousAxeFindings(page);
      await page.getByTestId("wb-import-wizard-next").click();
      await page.getByText("Show the rows that can't be imported").click();
      await page.getByText("Show the new entries").click();
      await expectNoSeriousAxeFindings(page);
      await shot(page, `import-review-${theme}`);
      await page.getByTestId("wb-import-wizard-import").click();
      await expect(wizard(page)).toHaveAttribute("data-step", "done");
      await expectNoSeriousAxeFindings(page);
    },
  );
}
