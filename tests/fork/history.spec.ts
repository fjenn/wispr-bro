// SPDX-License-Identifier: GPL-3.0-or-later
// S-15b History and row actions: grouping by local day (across midnight and
// time zones), search, every row action (play, copy, flag, Undo / Redo AI
// edit, retry, save audio, delete with undo), background jobs, live
// updates, keyboard use, 10,000-row scrolling, axe in light and dark and
// RTL (R5 S-15b tests, §5.5, §11.2). Mocked IPC; see hub-fixture.ts.
import type { Page } from "@playwright/test";
import type {
  FlagKind,
  HistoryPage,
  HistoryRow,
} from "../../src/fork/bindings";
import {
  test,
  expect,
  openHub,
  expectNoSeriousAxeFindings,
  type HubState,
} from "./hub-fixture";
import type { IpcMock } from "./fixtures";

/** 2026-10-31 10:00 in the browser's time zone (set per describe). */
const NOW_LOCAL = "2026-10-31T10:00:00";

/** An instant written with the zone offset the describe block's zone has. */
const local = (iso: string): number => Date.parse(iso);

let seq = 0;
function row(over: Partial<HistoryRow> & { created_at: number }): HistoryRow {
  seq += 1;
  return {
    id: seq,
    text: `Dictation ${seq}`,
    copy_text: `Dictation ${seq}`,
    variant: "final",
    ai_edited: false,
    source: "dictation",
    source_name: null,
    app_name: "TextEdit",
    file_name: `wispr-bro-${seq}.wav`,
    audio: "available",
    flagged: false,
    cleanup_level: "light",
    queue_status: null,
    ...over,
  };
}

/** The backend's history, newest first by id. */
class FakeHistory {
  rows: HistoryRow[] = [];
  deleted = new Set<number>();
  raw = new Map<number, string>();
  final = new Map<number, { text: string; copy: string }>();

  add(r: HistoryRow, raw?: string) {
    this.rows.push(r);
    if (raw) this.raw.set(r.id, raw);
    this.rows.sort((a, b) => b.id - a.id);
    return r;
  }
  visible() {
    return this.rows.filter((r) => !this.deleted.has(r.id));
  }
  page(cursor: number | null, limit: number): HistoryPage {
    const older = this.visible().filter(
      (r) => cursor === null || r.id < cursor,
    );
    const rows = older.slice(0, limit);
    return {
      rows,
      next: older.length > limit ? rows[rows.length - 1].id : null,
    };
  }
  find(query: string, offset: number, limit: number): HistoryPage {
    const q = query.toLowerCase();
    const hits = this.visible().filter((r) => r.text.toLowerCase().includes(q));
    const rows = hits.slice(offset, offset + limit);
    return {
      rows,
      next: hits.length > offset + limit ? offset + limit : null,
    };
  }
  byId(id: number) {
    return this.rows.find((r) => r.id === id);
  }
}

function installHistory(ipc: IpcMock, h: FakeHistory) {
  ipc.handle("fork_history_page", (a) =>
    h.page((a.cursor as number | null) ?? null, a.limit as number),
  );
  ipc.handle("fork_history_find", (a) =>
    h.find(a.query as string, a.offset as number, a.limit as number),
  );
  ipc.handle("fork_history_rows", (a) =>
    (a.ids as number[])
      .map((id) => h.byId(id))
      .filter((r) => r && !h.deleted.has(r.id)),
  );
  ipc.handle("fork_history_set_variant", (a) => {
    const r = h.byId(a.id as number);
    if (!r) return null;
    if (a.variant === "raw" && r.variant === "final") {
      h.final.set(r.id, { text: r.text, copy: r.copy_text });
      r.text = h.raw.get(r.id) ?? r.text;
      r.copy_text = r.text;
    } else if (a.variant === "final" && r.variant === "raw") {
      const f = h.final.get(r.id);
      if (f) {
        r.text = f.text;
        r.copy_text = f.copy;
      }
    }
    r.variant = a.variant as "final" | "raw";
    return r;
  });
  ipc.handle("fork_history_delete", (a) => {
    for (const id of a.ids as number[]) h.deleted.add(id);
    return (a.ids as number[]).length;
  });
  ipc.handle("fork_history_undo_delete", (a) => {
    for (const id of a.ids as number[]) h.deleted.delete(id);
    return (a.ids as number[]).length;
  });
  ipc.handle("fork_history_retry", () => null);
  ipc.handle("fork_history_save_audio", () => null);
  ipc.handle("get_audio_file_path", (a) => `/Users/test/rec/${a.fileName}`);
}

async function open(
  page: Page,
  ipc: IpcMock,
  hub: HubState,
  h: FakeHistory,
  options: Parameters<typeof openHub>[3] = {},
) {
  await openHub(page, ipc, hub, {
    ...options,
    beforeNavigate: () => installHistory(ipc, h),
  });
  await expect(page.getByTestId("wb-history")).toBeVisible();
}

const rows = (page: Page) => page.getByTestId("wb-history-row");
const rowById = (page: Page, id: number) =>
  page.locator(`[data-testid="wb-history-row"][data-id="${id}"]`);
const days = (page: Page) => page.getByTestId("wb-history-day");

/** A clipboard that records what was written. */
async function recordClipboard(page: Page) {
  await page.addInitScript(() => {
    const w = window as unknown as { __copied: string[] };
    w.__copied = [];
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: async (text: string) => {
          w.__copied.push(text);
        },
      },
    });
  });
}
const copied = (page: Page) =>
  page.evaluate(() => (window as unknown as { __copied: string[] }).__copied);

test.beforeEach(() => {
  seq = 0;
});

test.describe("history", () => {
  test.use({ timezoneId: "Europe/Amsterdam" });

  test.beforeEach(async ({ page }) => {
    await page.clock.setFixedTime(new Date(`${NOW_LOCAL}+01:00`));
  });

  test("rows are grouped by day: Today, Yesterday, then dates", async ({
    page,
    ipc,
    hub,
  }) => {
    const h = new FakeHistory();
    h.add(row({ created_at: local("2026-10-29T18:30:00+01:00") }));
    h.add(row({ created_at: local("2026-10-30T23:59:00+01:00") }));
    h.add(row({ created_at: local("2026-10-31T00:01:00+01:00") }));
    h.add(
      row({
        created_at: local("2026-10-31T09:41:00+01:00"),
        text: "Coffee at 3.",
      }),
    );
    await open(page, ipc, hub, h);
    await expect(days(page)).toHaveText([
      "Today",
      "Yesterday",
      "Thursday, October 29",
    ]);
    await expect(rows(page)).toHaveCount(4);
    // 00:01 is today, 23:59 the day before.
    await expect(rowById(page, 3)).toContainText("12:01");
    await expect(rowById(page, 4)).toContainText("9:41");
    await expect(rowById(page, 4).getByTestId("wb-history-text")).toHaveText(
      "Coffee at 3.",
    );
    const order = await page
      .locator('[data-testid="wb-history-day"], [data-testid="wb-history-row"]')
      .evaluateAll((els) =>
        els
          .map((e) => ({ y: e.getBoundingClientRect().top, t: e.dataset }))
          .sort((a, b) => a.y - b.y)
          .map((e) => e.t.day ?? e.t.id),
      );
    expect(order).toEqual([
      "2026-10-31",
      "4",
      "3",
      "2026-10-30",
      "2",
      "2026-10-29",
      "1",
    ]);
  });

  test("files, practice takes, failed takes and older years are labelled", async ({
    page,
    ipc,
    hub,
  }) => {
    const h = new FakeHistory();
    h.add(row({ created_at: local("2025-12-24T12:00:00+01:00") }));
    h.add(
      row({
        created_at: local("2026-10-31T08:00:00+01:00"),
        source: "file",
        source_name: "Team call.m4a",
      }),
    );
    h.add(
      row({
        created_at: local("2026-10-31T08:10:00+01:00"),
        source: "practice",
      }),
    );
    h.add(
      row({
        created_at: local("2026-10-31T08:20:00+01:00"),
        text: "",
        copy_text: "",
      }),
    );
    await open(page, ipc, hub, h);
    await expect(days(page).last()).toHaveText("Wednesday, December 24, 2025");
    await expect(
      rowById(page, 2).getByTestId("wb-history-tag-file"),
    ).toHaveText("FileTeam call.m4a");
    await expect(
      rowById(page, 3).getByTestId("wb-history-tag-practice"),
    ).toBeVisible();
    await expect(rowById(page, 4).getByTestId("wb-history-text")).toHaveText(
      /Transcription failed/,
    );
    await expect(
      rowById(page, 4).getByTestId("wb-history-copy"),
    ).toBeDisabled();
    await expect(
      rowById(page, 4).getByTestId("wb-history-flag"),
    ).toBeDisabled();
  });

  test("waiting rows keep their audio; failed queued rows offer Retry", async ({
    page,
    ipc,
    hub,
  }) => {
    const h = new FakeHistory();
    h.add(
      row({
        created_at: local("2026-10-31T09:00:00+01:00"),
        text: "",
        copy_text: "",
        queue_status: "waiting",
      }),
    );
    h.add(
      row({
        created_at: local("2026-10-31T09:01:00+01:00"),
        text: "",
        copy_text: "",
        queue_status: "failed",
      }),
    );
    await open(page, ipc, hub, h);
    const waiting = rowById(page, 1);
    await expect(waiting.getByTestId("wb-history-waiting")).toHaveText(
      "Waiting for speech model",
    );
    await expect(waiting.getByTestId("wb-history-play")).toBeEnabled();
    await expect(waiting.getByTestId("wb-history-copy")).toBeDisabled();
    await waiting.getByTestId("wb-history-more").click();
    await expect(page.getByTestId("wb-menu-item-retry")).toHaveAttribute(
      "data-disabled",
      "",
    );
    await page.keyboard.press("Escape");

    const failed = rowById(page, 2);
    await expect(failed.getByTestId("wb-history-text")).toContainText(
      "Transcription failed",
    );
    await failed.getByTestId("wb-history-more").click();
    await page.getByTestId("wb-menu-item-retry").click();
    await ipc.expectCalled("fork_history_retry", { id: 2, level: null });
    await expectNoSeriousAxeFindings(page);
  });

  test("copy uses the shown variant; Undo and Redo AI edit never paste", async ({
    page,
    ipc,
    hub,
  }) => {
    await recordClipboard(page);
    const h = new FakeHistory();
    h.add(
      row({
        created_at: local("2026-10-31T09:00:00+01:00"),
        text: "Coffee at 3.",
        copy_text: "Coffee at 3, see you there.",
        ai_edited: true,
      }),
      "so um coffee at 2 actually 3",
    );
    await open(page, ipc, hub, h);
    const r = rowById(page, 1);
    await r.hover();
    await r.getByTestId("wb-history-copy").click();
    await expect(r.getByTestId("wb-history-copy")).toHaveAccessibleName(
      "Copied",
    );
    expect(await copied(page)).toEqual(["Coffee at 3, see you there."]);

    await ipc.clearCalls();
    await r.getByTestId("wb-history-more").click();
    await page.getByTestId("wb-menu-item-undo-ai").click();
    await ipc.expectCalled("fork_history_set_variant", {
      id: 1,
      variant: "raw",
    });
    await expect(r.getByTestId("wb-history-text")).toHaveText(
      "so um coffee at 2 actually 3",
    );
    await expect(r.getByTestId("wb-history-tag-raw")).toBeVisible();
    await r.hover();
    await r.getByTestId("wb-history-copy").click();
    await expect.poll(() => copied(page)).toHaveLength(2);
    expect((await copied(page))[1]).toBe("so um coffee at 2 actually 3");

    await r.getByTestId("wb-history-more").click();
    await page.getByTestId("wb-menu-item-redo-ai").click();
    await expect(r.getByTestId("wb-history-text")).toHaveText("Coffee at 3.");
    const names = await ipc.commandNames();
    expect(names.filter((n) => /paste|deliver|insert/i.test(n))).toEqual([]);
  });

  test("rows without an AI edit have no Undo AI edit", async ({
    page,
    ipc,
    hub,
  }) => {
    const h = new FakeHistory();
    h.add(row({ created_at: local("2026-10-31T09:00:00+01:00") }));
    await open(page, ipc, hub, h);
    await rowById(page, 1).getByTestId("wb-history-more").click();
    await expect(page.getByTestId("wb-history-menu")).toBeVisible();
    await expect(page.getByTestId("wb-menu-item-undo-ai")).toHaveCount(0);
    await expect(page.getByTestId("wb-menu-item-retry")).toBeVisible();
  });

  test("retry: at the setting or another level, with job progress", async ({
    page,
    ipc,
    hub,
  }) => {
    const h = new FakeHistory();
    h.add(row({ created_at: local("2026-10-31T09:00:00+01:00") }));
    await open(page, ipc, hub, h, { fork: { cleanup_level: "light" } });
    const r = rowById(page, 1);
    await r.getByTestId("wb-history-more").click();
    // The setting's level is the plain Retry; the others are offered.
    await expect(page.getByTestId("wb-menu-item-retry-light")).toHaveCount(0);
    await page.getByTestId("wb-menu-item-retry").click();
    await ipc.expectCalled("fork_history_retry", { id: 1, level: null });

    await r.getByTestId("wb-history-more").click();
    await page.getByTestId("wb-menu-item-retry-medium").click();
    await ipc.expectCalled("fork_history_retry", { id: 1, level: "medium" });

    await ipc.waitForListener("transcription-job");
    await ipc.emit("transcription-job", {
      entry_id: 1,
      status: { state: "running", percent: 40 },
    });
    await expect(r.getByTestId("wb-history-text")).toHaveText(
      "Transcribing… 40%",
    );
    await r.getByTestId("wb-history-more").click();
    await expect(page.getByTestId("wb-menu-item-retry")).toHaveAttribute(
      "data-disabled",
      "",
    );
    await page.keyboard.press("Escape");

    // The job finished: Handy's update event brings the new text.
    h.byId(1)!.text = "The new transcript.";
    await ipc.emit("transcription-job", {
      entry_id: 1,
      status: { state: "finished" },
    });
    await ipc.emit("history-update-payload", {
      action: "updated",
      entry: { id: 1 },
    });
    await expect(r.getByTestId("wb-history-text")).toHaveText(
      "The new transcript.",
    );
  });

  test("delete hides the row and Undo brings it back", async ({
    page,
    ipc,
    hub,
  }) => {
    const h = new FakeHistory();
    h.add(row({ created_at: local("2026-10-31T09:00:00+01:00") }));
    h.add(row({ created_at: local("2026-10-31T09:05:00+01:00") }));
    await open(page, ipc, hub, h);
    await rowById(page, 2).getByTestId("wb-history-more").click();
    await page.getByTestId("wb-menu-item-delete").click();
    await expect(rowById(page, 2)).toHaveCount(0);
    await ipc.expectCalled("fork_history_delete", { ids: [2] });
    const toast = page.locator("[data-sonner-toast]", { hasText: "Deleted" });
    await expect(toast).toBeVisible();
    await toast.getByRole("button", { name: "Undo" }).click();
    await ipc.expectCalled("fork_history_undo_delete", { ids: [2] });
    await expect(rowById(page, 2)).toBeVisible();
    // Back in its place: newest first.
    await expect(rows(page).first()).toHaveAttribute("data-id", "2");
  });

  test("save audio as… asks where and copies the recording", async ({
    page,
    ipc,
    hub,
  }) => {
    const h = new FakeHistory();
    h.add(row({ created_at: local("2026-10-31T09:41:00+01:00") }));
    ipc.respond("plugin:dialog|save", "/Users/test/Desktop/take.wav");
    await open(page, ipc, hub, h);
    await rowById(page, 1).getByTestId("wb-history-more").click();
    await page.getByTestId("wb-menu-item-save-audio").click();
    await ipc.expectCalled("plugin:dialog|save");
    const [dialog] = await ipc.calls("plugin:dialog|save");
    expect(JSON.stringify(dialog.args)).toContain(
      "dictation-2026-10-31-0941.wav",
    );
    await ipc.expectCalled("fork_history_save_audio", {
      id: 1,
      path: "/Users/test/Desktop/take.wav",
    });
    await expect(
      page.locator("[data-sonner-toast]", { hasText: "Recording saved." }),
    ).toBeVisible();

    // Cancelled panel: nothing is written.
    await ipc.clearCalls();
    ipc.respond("plugin:dialog|save", null);
    await rowById(page, 1).getByTestId("wb-history-more").click();
    await page.getByTestId("wb-menu-item-save-audio").click();
    await ipc.expectCalled("plugin:dialog|save");
    expect(await ipc.calls("fork_history_save_audio")).toHaveLength(0);
  });

  test("expired audio: play and save are off with the reason", async ({
    page,
    ipc,
    hub,
  }) => {
    const h = new FakeHistory();
    h.add(
      row({ created_at: local("2026-10-01T09:00:00+01:00"), audio: "expired" }),
    );
    h.add(
      row({ created_at: local("2026-10-31T09:00:00+01:00"), audio: "missing" }),
    );
    await open(page, ipc, hub, h);
    const old = rowById(page, 1);
    await expect(old.getByTestId("wb-history-play")).toBeDisabled();
    await expect(old.getByTestId("wb-history-play")).toHaveAccessibleName(
      "Audio removed after 14 days",
    );
    await old.getByTestId("wb-history-more").click();
    const save = page.getByTestId("wb-menu-item-save-audio");
    await expect(save).toHaveText("Audio removed after 14 days");
    await expect(save).toHaveAttribute("data-disabled", "");
    await expect(page.getByTestId("wb-menu-item-retry")).toHaveAttribute(
      "data-disabled",
      "",
    );
    await page.keyboard.press("Escape");
    await expect(
      rowById(page, 2).getByTestId("wb-history-play"),
    ).toHaveAccessibleName("No recording for this dictation");
  });

  test("play opens the player for that row only", async ({
    page,
    ipc,
    hub,
  }) => {
    const h = new FakeHistory();
    h.add(row({ created_at: local("2026-10-31T09:00:00+01:00") }));
    await open(page, ipc, hub, h);
    const r = rowById(page, 1);
    await r.getByTestId("wb-history-play").click();
    await expect(r.getByTestId("wb-history-play")).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    await ipc.expectCalled("get_audio_file_path", {
      fileName: "wispr-bro-1.wav",
    });
    await expect(r.locator("audio")).toHaveCount(1);
    await r.getByTestId("wb-history-play").click();
    await expect(r.locator("audio")).toHaveCount(0);
  });

  test("flag: pick a word, type the fix, it goes to the dictionary", async ({
    page,
    ipc,
    hub,
  }) => {
    const h = new FakeHistory();
    h.add(
      row({
        created_at: local("2026-10-31T09:00:00+01:00"),
        text: "Call Sioban tomorrow, okay?",
      }),
    );
    const flags: Array<{ from: string; to: string; kind: FlagKind }> = [];
    ipc.handle("fork_history_flag", (a) => {
      flags.push({
        from: a.from as string,
        to: a.to as string,
        kind: a.kind as FlagKind,
      });
      const r = h.byId(1)!;
      r.flagged = true;
      if (flags.length === 2 && a.replace === null) {
        return {
          outcome: {
            status: "conflict",
            existing: {
              id: 7,
              kind: "rule",
              phrase: "tomorrow",
              replacement: "tmrw",
              source: "user",
              status: "active",
              created_at: 1,
              updated_at: 1,
              hit_count: 0,
              last_hit_at: null,
            },
          },
          row: null,
        };
      }
      return {
        outcome: {
          status: "saved",
          entry: {
            id: 9,
            kind: a.kind === "rule" ? "rule" : "word",
            phrase: a.kind === "rule" ? a.from : a.to,
            replacement: a.kind === "rule" ? a.to : null,
            source: "flag",
            status: "active",
            created_at: 1,
            updated_at: 1,
            hit_count: 0,
            last_hit_at: null,
          },
        },
        row: r,
      };
    });
    await open(page, ipc, hub, h);
    await rowById(page, 1).getByTestId("wb-history-flag").click();
    const dialog = page.getByTestId("wb-flag-dialog");
    await expect(dialog).toBeVisible();
    const words = dialog.getByTestId("wb-flag-word");
    await expect(words).toHaveText(["Call", "Sioban", "tomorrow", "okay"]);
    await expect(page.getByTestId("wb-flag-save")).toBeDisabled();
    await words.nth(1).click();
    await expect(words.nth(1)).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByTestId("wb-flag-to")).toHaveValue("Sioban");
    // Unchanged text is not a fix.
    await expect(page.getByTestId("wb-flag-save")).toBeDisabled();
    await page.getByTestId("wb-flag-to").fill("Siobhán");
    await page.getByTestId("wb-flag-save").click();
    await expect(dialog).toBeHidden();
    expect(flags[0]).toEqual({ from: "Sioban", to: "Siobhán", kind: "rule" });
    await expect(
      page.locator("[data-sonner-toast]", {
        hasText: "From now on “Sioban” becomes “Siobhán”.",
      }),
    ).toBeVisible();
    await expect(
      rowById(page, 1).getByTestId("wb-history-tag-flagged"),
    ).toBeVisible();

    // A conflict offers "Replace it", which edits the existing entry.
    await rowById(page, 1).getByTestId("wb-history-flag").click();
    await words.nth(2).click();
    await page.getByTestId("wb-flag-to").fill("tmrw.");
    await page.getByTestId("wb-flag-save").click();
    await expect(page.getByTestId("wb-flag-feedback")).toHaveAttribute(
      "data-kind",
      "conflict",
    );
    await page.getByTestId("wb-flag-replace-existing").click();
    await ipc.expectCalled("fork_history_flag", { replace: 7 });
    await expect(dialog).toBeHidden();

    // A word instead of a replacement.
    await rowById(page, 1).getByTestId("wb-history-flag").click();
    await words.nth(0).click();
    await words.nth(1).click(); // widens the pick to "Call Sioban"
    await expect(page.getByTestId("wb-flag-to")).toHaveValue("Call Sioban");
    await page.getByTestId("wb-flag-to").fill("Kubernetes");
    await page.getByTestId("wb-flag-kind-word").check();
    await page.getByTestId("wb-flag-save").click();
    expect(flags.at(-1)).toEqual({
      from: "Call Sioban",
      to: "Kubernetes",
      kind: "word",
    });
  });

  test("search: as you type, results grouped, Esc goes back", async ({
    page,
    ipc,
    hub,
  }) => {
    const h = new FakeHistory();
    h.add(
      row({
        created_at: local("2026-10-30T09:00:00+01:00"),
        text: "Order coffee",
      }),
    );
    h.add(
      row({ created_at: local("2026-10-31T09:00:00+01:00"), text: "Tea time" }),
    );
    h.add(
      row({
        created_at: local("2026-10-31T09:30:00+01:00"),
        text: "More coffee please",
      }),
    );
    await open(page, ipc, hub, h);
    await expect(rows(page)).toHaveCount(3);
    await page.getByTestId("wb-history-search-open").click();
    const box = page.getByRole("searchbox", { name: "Search your dictations" });
    await expect(box).toBeFocused();
    await box.pressSequentially("coffee");
    await ipc.expectCalled("fork_history_find", { query: "coffee", offset: 0 });
    expect(
      (await ipc.calls("fork_history_find")).filter(
        (c) => (c.args.query as string).length < 6,
      ),
    ).toHaveLength(0); // debounced: no search per key
    await expect(rows(page)).toHaveCount(2);
    await expect(days(page)).toHaveText(["Today", "Yesterday"]);
    await expect(page.getByTestId("wb-history-results")).toHaveText(
      "2 dictations found",
    );
    await box.fill("zebra");
    await expect(page.getByTestId("wb-history-no-results")).toContainText(
      "No dictation contains “zebra”.",
    );
    await box.press("Escape");
    await expect(rows(page)).toHaveCount(3);
    await expect(page.getByTestId("wb-history-search-open")).toBeFocused();
  });

  test("a new take appears on top live", async ({ page, ipc, hub }) => {
    const h = new FakeHistory();
    h.add(row({ created_at: local("2026-10-31T09:00:00+01:00") }));
    await open(page, ipc, hub, h);
    await ipc.waitForListener("history-update-payload");
    h.add(
      row({
        created_at: local("2026-10-31T09:59:00+01:00"),
        text: "Fresh take.",
      }),
    );
    await ipc.emit("history-update-payload", {
      action: "added",
      entry: { id: 2 },
    });
    await expect(rows(page).first()).toHaveAttribute("data-id", "2");
    await expect(rows(page).first()).toContainText("Fresh take.");
    // Read again after the paste settled (what was inserted, S-17).
    h.byId(2)!.copy_text = "Fresh take, expanded.";
    await expect
      .poll(async () => (await ipc.calls("fork_history_rows")).length, {
        timeout: 5_000,
      })
      .toBeGreaterThanOrEqual(2);
    await ipc.emit("history-update-payload", { action: "deleted", id: 1 });
    await expect(rowById(page, 1)).toHaveCount(0);
  });

  test("keyboard: tabbing into a row shows its actions; the menu works by keys", async ({
    page,
    ipc,
    hub,
  }) => {
    const h = new FakeHistory();
    h.add(
      row({
        created_at: local("2026-10-31T09:00:00+01:00"),
        ai_edited: true,
        text: "Clean text.",
      }),
      "raw text",
    );
    await open(page, ipc, hub, h);
    const actions = rowById(page, 1).getByTestId("wb-history-actions");
    await expect(actions).toHaveCSS("opacity", "0");
    await rowById(page, 1).getByTestId("wb-history-play").focus();
    await expect(actions).toHaveCSS("opacity", "1");
    await page.keyboard.press("Tab"); // copy
    await page.keyboard.press("Tab"); // flag
    await page.keyboard.press("Tab"); // more
    await expect(rowById(page, 1).getByTestId("wb-history-more")).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.getByTestId("wb-history-menu")).toBeVisible();
    await expect(page.getByTestId("wb-menu-item-undo-ai")).toBeFocused();
    await page.keyboard.press("Enter");
    await ipc.expectCalled("fork_history_set_variant", { variant: "raw" });
    await expect(rowById(page, 1).getByTestId("wb-history-more")).toBeFocused();
    // Esc closes the flag dialog and focus returns.
    await rowById(page, 1).getByTestId("wb-history-flag").focus();
    await page.keyboard.press("Enter");
    await expect(page.getByTestId("wb-flag-dialog")).toBeVisible();
    await expect(page.getByTestId("wb-flag-word").first()).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(page.getByTestId("wb-flag-dialog")).toBeHidden();
  });

  test("10,000 rows scroll smoothly: few rows in the DOM, pages on demand", async ({
    page,
    ipc,
    hub,
  }) => {
    const h = new FakeHistory();
    const start = local("2026-10-31T09:00:00+01:00");
    for (let i = 0; i < 10_000; i++) {
      h.add(
        row({
          created_at: start - (10_000 - i) * 7 * 60_000,
          text:
            i % 7 === 0
              ? `Take ${i}: a longer dictation that wraps onto a second line in the history list so rows differ in height.`
              : `Take ${i}`,
        }),
      );
    }
    test.setTimeout(240_000);
    await open(page, ipc, hub, h);
    await expect(rows(page).first()).toBeVisible();
    // One page at first: History never reads the whole history at once.
    expect((await ipc.calls("fork_history_page")).length).toBe(1);
    const main = page.getByTestId("wb-main");

    // 1. Keep scrolling to the end until the oldest take is there; every
    //    page is read on demand, and the DOM stays small all the way.
    const load = await main.evaluate(async (el) => {
      const frame = () => new Promise((r) => requestAnimationFrame(r));
      let maxMounted = 0;
      const t0 = performance.now();
      while (performance.now() - t0 < 180_000) {
        el.scrollTop = el.scrollHeight;
        await frame();
        await frame();
        maxMounted = Math.max(
          maxMounted,
          el.querySelectorAll('[data-testid="wb-history-row"]').length,
        );
        if (el.querySelector('[data-testid="wb-history-row"][data-id="1"]'))
          return { maxMounted, done: true };
        await new Promise((r) => setTimeout(r, 30));
      }
      return { maxMounted, done: false };
    });
    expect(load.done).toBe(true);
    expect(load.maxMounted).toBeLessThan(80);
    expect(
      (await ipc.calls("fork_history_page")).length,
    ).toBeGreaterThanOrEqual(100);

    // 2. Scroll back up through all 10,000 rows in screen-sized steps and
    //    time each step to its next frame.
    const run = await main.evaluate(async (el) => {
      const frame = () => new Promise((r) => requestAnimationFrame(r));
      const times: number[] = [];
      let maxMounted = 0;
      while (el.scrollTop > 0 && times.length < 2_000) {
        const t0 = performance.now();
        el.scrollTop = Math.max(0, el.scrollTop - el.clientHeight);
        await frame();
        times.push(performance.now() - t0);
        maxMounted = Math.max(
          maxMounted,
          el.querySelectorAll('[data-testid="wb-history-row"]').length,
        );
      }
      times.sort((a, b) => a - b);
      return {
        steps: times.length,
        p95: times[Math.floor(times.length * 0.95)],
        maxMounted,
      };
    });
    console.log(
      `10,000 rows: ${run.steps} steps, p95 ${run.p95.toFixed(1)} ms`,
    );
    expect(run.steps).toBeGreaterThan(100);
    expect(run.maxMounted).toBeLessThan(80);
    // A step and its next frame (60 Hz = 16.7 ms); 86 ms p95 measured at load
    // average 150 on the M5 Max while a release build ran, so the bound holds
    // on a busy runner (the DOM bound above is what catches a list that
    // renders every row).
    expect(run.p95).toBeLessThan(150);
    await expect(rows(page).first()).toHaveAttribute("data-id", "10000");
  });

  for (const theme of ["light", "dark"] as const) {
    test(`axe: list, menu and flag dialog (${theme})`, async ({
      page,
      ipc,
      hub,
    }) => {
      test.setTimeout(120_000);
      const h = new FakeHistory();
      h.add(
        row({
          created_at: local("2026-10-30T09:00:00+01:00"),
          audio: "expired",
        }),
      );
      h.add(
        row({
          created_at: local("2026-10-31T09:00:00+01:00"),
          source: "file",
          source_name: "memo.m4a",
          flagged: true,
        }),
      );
      h.add(
        row({
          created_at: local("2026-10-31T09:30:00+01:00"),
          ai_edited: true,
          variant: "raw",
        }),
      );
      h.add(
        row({
          created_at: local("2026-10-31T09:45:00+01:00"),
          text: "",
          copy_text: "",
          queue_status: "waiting",
        }),
      );
      await page.emulateMedia({ colorScheme: theme });
      await open(page, ipc, hub, h, { handy: { theme } });
      await rowById(page, 3).hover();
      await expectNoSeriousAxeFindings(page);
      await rowById(page, 3).getByTestId("wb-history-more").click();
      await expect(page.getByTestId("wb-history-menu")).toBeVisible();
      await expectNoSeriousAxeFindings(page);
      await page.keyboard.press("Escape");
      await rowById(page, 2).getByTestId("wb-history-flag").click();
      await page.getByTestId("wb-flag-word").first().click();
      await expect(page.getByTestId("wb-flag-dialog")).toBeVisible();
      await expectNoSeriousAxeFindings(page);
      await page.keyboard.press("Escape");
      await page.getByTestId("wb-history-search-open").click();
      await expectNoSeriousAxeFindings(page);
    });
  }

  test("RTL: times on the right, actions on the left, axe clean", async ({
    page,
    ipc,
    hub,
  }) => {
    const h = new FakeHistory();
    h.add(
      row({ created_at: local("2026-10-31T09:00:00+01:00"), text: "مرحبا" }),
    );
    await open(page, ipc, hub, h, { handy: { app_language: "ar" } });
    const r = rowById(page, 1);
    const text = (await r.getByTestId("wb-history-text").boundingBox())!;
    const acts = (await r.getByTestId("wb-history-actions").boundingBox())!;
    // W2: the actions float over the row's end (the reference's hover
    // strip), so in RTL they sit over the left end of the text.
    expect(acts.x).toBeLessThanOrEqual(text.x + 1);
    expect(acts.x + acts.width).toBeLessThan(text.x + text.width / 2);
    await expectNoSeriousAxeFindings(page);
  });
});

// The same instants seen from two other zones: groups follow the Mac's
// zone at display time, not UTC.
for (const [zone, expected] of [
  ["America/Los_Angeles", ["Today", "Yesterday"]],
  ["Pacific/Auckland", ["Today"]],
] as const) {
  test.describe(`history in ${zone}`, () => {
    test.use({ timezoneId: zone });
    test("groups by the local day", async ({ page, ipc, hub }) => {
      // 2026-10-31 07:30 UTC and 2026-10-30 23:30 UTC.
      await page.clock.setFixedTime(new Date("2026-10-31T09:00:00Z"));
      const h = new FakeHistory();
      h.add(row({ created_at: Date.parse("2026-10-30T23:30:00Z") }));
      h.add(row({ created_at: Date.parse("2026-10-31T07:30:00Z") }));
      await open(page, ipc, hub, h);
      // LA: 16:30 on the 30th and 00:30 on the 31st (now 02:00 on the 31st).
      // Auckland: 12:30 and 20:30 on the 31st (now 22:00 on the 31st).
      await expect(days(page)).toHaveText([...expected]);
    });
  });
}

test.describe("history across midnight", () => {
  test.use({ timezoneId: "Europe/Amsterdam" });
  test("Today becomes Yesterday at local midnight without a reload", async ({
    page,
    ipc,
    hub,
  }) => {
    await page.clock.install({ time: new Date("2026-10-31T23:59:30+01:00") });
    const h = new FakeHistory();
    h.add(row({ created_at: local("2026-10-31T23:00:00+01:00") }));
    await open(page, ipc, hub, h);
    await expect(days(page)).toHaveText(["Today"]);
    await page.clock.runFor(60_000);
    await expect(days(page)).toHaveText(["Yesterday"]);
  });
});
