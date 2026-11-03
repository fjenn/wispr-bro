// SPDX-License-Identifier: GPL-3.0-or-later
// Design W6: screenshots of the Scratchpad (page and note window) and every
// W6 dialog and popover, light and dark, for the before/after comparison.
// Skipped unless WB_DESIGN_SHOTS_DIR is set:
//
//   WB_DESIGN_SHOTS_DIR=/tmp/shots bun run wb:test:ui -- design-shots-w6
//
// The images show mocked data only (no personal data).
import path from "node:path";
import type { Page } from "@playwright/test";
import type { Note } from "../../src/fork/bindings";
import { test, expect, openHub, type HubState } from "./hub-fixture";
import type { IpcMock } from "./fixtures";

const dir = process.env.WB_DESIGN_SHOTS_DIR;
const NOW = Date.now();

const note = (id: number, body: string, at: number, pinned = false): Note => ({
  id,
  title: body.split("\n")[0] ?? "",
  body,
  pinned,
  created_at: at,
  updated_at: at,
});

const NOTES = [
  note(
    1,
    "Launch checklist\nConfirm the release notes, tag the build and post the download link once the notarisation finishes.",
    NOW - 3 * 3_600_000,
    true,
  ),
  note(
    2,
    "Ideas for the offsite\nA walk in the morning, short demos after lunch, and one open hour where nobody presents anything.",
    NOW - 4 * 86_400_000,
  ),
  note(
    3,
    "Reply to Maya\nThanks for the notes on the draft. I moved the summary to the top and cut the second section.",
    NOW - 9 * 86_400_000,
  ),
];

function installNotes(ipc: IpcMock) {
  ipc.respond("fork_scratchpad_list", NOTES);
  ipc.handle(
    "fork_scratchpad_get",
    (a) => NOTES.find((n) => n.id === a.id) ?? null,
  );
  ipc.handle(
    "fork_scratchpad_save",
    (a) => NOTES.find((n) => n.id === a.id) ?? null,
  );
}

const DICT_CSV =
  'phrase,replacement\r\nSiobhán,\r\nKubernetes,\r\nwanna,want to\r\nO\'Brien,\r\n"say, ""hi""",Hi\r\n';
const SNIP_CSV =
  'trigger,text\r\nmy sign-off,"Best wishes,\nSam"\r\nmy email,sam@example.com\r\nmy address,"12 Harbour Lane\nKiel"\r\n';

function dictRow(r: number, phrase: string, replacement: string | null = null) {
  return { row: r, phrase, replacement };
}

function installImports(ipc: IpcMock, kind: "dictionary" | "snippets") {
  ipc.respond(
    "plugin:dialog|open",
    kind === "dictionary" ? "/Users/sam/words.csv" : "/Users/sam/snippets.csv",
  );
  ipc.handle("fork_dictionary_read_file", (a) => ({
    status: "read",
    name: String(a.path).split("/").pop(),
    text: kind === "dictionary" ? DICT_CSV : SNIP_CSV,
  }));
  ipc.respond("fork_dictionary_import_review", {
    new_entries: [
      dictRow(2, "Siobhán"),
      dictRow(5, "O'Brien"),
      dictRow(6, 'say, "hi"', "Hi"),
    ],
    duplicates: [{ incoming: dictRow(3, "Kubernetes"), in_file: false }],
    conflicts: [
      {
        incoming: dictRow(4, "wanna", "want to"),
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
  });
  ipc.respond("fork_dictionary_import", {
    added: 3,
    replaced: 1,
    skipped: 1,
    kept: 0,
    invalid: 1,
    over_limit: 0,
  });
  const snip = (r: number, trigger: string, body: string) => ({
    row: r,
    trigger,
    body,
  });
  ipc.respond("fork_snippets_list", {
    snippets: [],
    counts: { snippets: 0, uses: 0 },
  });
  ipc.respond("fork_snippets_import_review", {
    new_snippets: [snip(2, "my sign-off", "Best wishes,\nSam")],
    duplicates: [
      { incoming: snip(3, "my email", "sam@example.com"), in_file: false },
    ],
    conflicts: [
      {
        incoming: snip(4, "my address", "12 Harbour Lane\nKiel"),
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
    invalid: [],
    room: 997,
    ignored_rows: 0,
  });
  ipc.respond("fork_snippets_import", {
    added: 1,
    replaced: 1,
    skipped: 1,
    kept: 0,
    invalid: 0,
    over_limit: 0,
  });
}

function installInbox(ipc: IpcMock, empty = false) {
  const entry = (over: Record<string, unknown>) => ({
    kind: "model_ready",
    title_key: "rust.notify.modelReady.title",
    body_key: "rust.notify.modelReady.body",
    args: {},
    action: { type: "open_page", page: "dictation", section: null },
    buttons: [],
    created_at: Date.now() - 5 * 60_000,
    read: false,
    ...over,
  });
  const items = empty
    ? []
    : [
        entry({
          id: 3,
          kind: "queued_takes_ready",
          title_key: "rust.notify.queuedTakesReady.title_other",
          body_key: "rust.notify.queuedTakesReady.body",
          args: { count: "3" },
          buttons: ["copy_latest", "open"],
          created_at: Date.now() - 20_000,
        }),
        entry({
          id: 2,
          kind: "milestone",
          title_key: "rust.notify.milestoneWords.title_other",
          body_key: "rust.notify.milestoneWords.body_other",
          args: { count: "10000" },
          created_at: Date.now() - 3 * 3_600_000,
        }),
        entry({ id: 1, read: true, created_at: Date.now() - 2 * 86_400_000 }),
      ];
  ipc.handle("fork_notifications_list", () => ({
    items,
    unread: items.filter((i) => !i.read).length,
    total: items.length,
  }));
}

async function snap(page: Page, name: string, theme: string) {
  await page.waitForTimeout(500);
  await page.screenshot({ path: path.join(dir!, `w6-${name}-${theme}.png`) });
}

async function hubAt(
  page: Page,
  ipc: IpcMock,
  hub: HubState,
  theme: "light" | "dark",
  hash = "",
  extra: () => void = () => undefined,
  handy: Record<string, unknown> = {},
) {
  await page.emulateMedia({ colorScheme: theme });
  await openHub(page, ipc, hub, {
    hash,
    handy: { theme, ...handy },
    beforeNavigate: extra,
  });
}

test.describe("design shots W6", () => {
  test.skip(!dir, "set WB_DESIGN_SHOTS_DIR to write screenshots");
  test.setTimeout(240_000);

  for (const theme of ["light", "dark"] as const) {
    test(`scratchpad ${theme}`, async ({ page, ipc, hub }) => {
      await hubAt(page, ipc, hub, theme, "#/hub/scratchpad", () =>
        installNotes(ipc),
      );
      await expect(page.getByTestId("wb-note-1")).toBeVisible();
      await snap(page, "scratchpad-page", theme);
      await page.getByTestId("wb-note-1").hover();
      await snap(page, "scratchpad-page-hover", theme);
      await page.getByTestId("wb-scratchpad-shortcut").click();
      await snap(page, "scratchpad-shortcut", theme);

      await page.setViewportSize({ width: 560, height: 440 });
      await page.goto("/src/fork/scratchpad/index.html?note=1");
      await expect(page.getByTestId("wb-note-body")).toBeVisible();
      await snap(page, "note-window", theme);
      await page.getByTestId("wb-note-sidebar").click();
      await page.getByTestId("wb-note-search-toggle").click();
      await snap(page, "note-window-sidebar", theme);
      await page.goto("/src/fork/scratchpad/index.html");
      await expect(page.getByTestId("wb-note-window")).toBeVisible();
      await snap(page, "note-window-empty", theme);
    });

    test(`notifications and help ${theme}`, async ({ page, ipc, hub }) => {
      await hubAt(page, ipc, hub, theme, "", () => installInbox(ipc), {});
      await page.getByTestId("wb-notifications-bell").click();
      await expect(page.getByTestId("wb-notifications-panel")).toBeVisible();
      await snap(page, "notifications", theme);
      await page.getByTestId("wb-notifications-more").click();
      await snap(page, "notifications-menu", theme);
      await page.keyboard.press("Escape");
      await page.keyboard.press("Escape");

      await hubAt(page, ipc, hub, theme, "", () => installInbox(ipc, true));
      await page.getByTestId("wb-notifications-bell").click();
      await snap(page, "notifications-empty", theme);
      await page.keyboard.press("Escape");

      await page.getByTestId("wb-nav-help").click();
      await expect(page.getByTestId("wb-help-menu")).toBeVisible();
      await snap(page, "help", theme);
      await page.getByTestId("wb-menu-item-report-problem").click();
      await expect(page.getByTestId("wb-report-dialog")).toBeVisible();
      await snap(page, "report-problem", theme);
    });

    test(`feature popup and what's new ${theme}`, async ({
      page,
      ipc,
      hub,
    }) => {
      await hubAt(page, ipc, hub, theme, "", () => {
        installInbox(ipc);
        ipc.respond("fork_notifications_feature", {
          id: "inbox",
          dismiss_id: "feature.inbox",
          title_key: "rust.notify.features.inbox.title",
          body_key: "rust.notify.features.inbox.body",
          action: { type: "open_panel" },
          entry_id: 5,
        });
      });
      await expect(page.getByTestId("wb-feature-popup")).toBeVisible({
        timeout: 10_000,
      });
      await snap(page, "feature-popup", theme);

      await hubAt(
        page,
        ipc,
        hub,
        theme,
        "",
        () =>
          ipc.respond("fork_update_state", {
            phase: "up_to_date",
            busy: false,
            version: null,
            progress: null,
            error: null,
            failures: 0,
            paused_until: null,
            needs_password: false,
            channel: "stable",
            last_checked_at: null,
          }),
        {
          show_whats_new_on_update: true,
          whats_new_last_seen_version: "0.9.7",
        },
      );
      await expect(page.getByTestId("wb-whats-new")).toBeVisible({
        timeout: 10_000,
      });
      await snap(page, "whats-new", theme);
    });

    test(`dictionary dialogs ${theme}`, async ({ page, ipc, hub }) => {
      await hubAt(page, ipc, hub, theme, "#/hub/dictionary", () =>
        installImports(ipc, "dictionary"),
      );
      await page.getByTestId("wb-dictionary-add").click();
      await page.getByTestId("wb-dictionary-phrase").fill("Siobhán");
      await snap(page, "add-word", theme);
      await page.getByRole("switch").last().click();
      await page.getByTestId("wb-dictionary-phrase").fill("wanna");
      await page.getByTestId("wb-dictionary-replacement").fill("want to");
      ipc.respond("fork_dictionary_add", {
        status: "conflict",
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
      });
      await page.getByTestId("wb-dictionary-save").click();
      await expect(page.getByTestId("wb-dictionary-feedback")).toBeVisible();
      await snap(page, "add-word-conflict", theme);
      await page.keyboard.press("Escape");

      await page.getByTestId("wb-dictionary-import").click();
      await page.getByTestId("wb-import-wizard-format").click();
      await snap(page, "import-choose", theme);
      await page.getByTestId("wb-import-wizard-browse").click();
      await expect(page.getByTestId("wb-import-wizard")).toHaveAttribute(
        "data-step",
        "columns",
      );
      await snap(page, "import-columns", theme);
      await page.getByTestId("wb-import-wizard-next").click();
      await expect(page.getByTestId("wb-import-wizard")).toHaveAttribute(
        "data-step",
        "review",
      );
      await snap(page, "import-review", theme);
      await page.getByTestId("wb-import-wizard-import").click();
      await expect(page.getByTestId("wb-import-wizard")).toHaveAttribute(
        "data-step",
        "done",
      );
      await snap(page, "import-done", theme);
    });

    test(`snippet dialogs ${theme}`, async ({ page, ipc, hub }) => {
      await hubAt(page, ipc, hub, theme, "#/hub/snippets", () =>
        installImports(ipc, "snippets"),
      );
      await page.getByTestId("wb-snippets-add").click();
      await page.getByTestId("wb-snippet-trigger").fill("my sign-off");
      await page
        .getByTestId("wb-snippet-body")
        .fill("Best wishes,\nSam\n\nSent from my desk");
      await snap(page, "snippet", theme);
      await page.keyboard.press("Escape");
      await page.getByTestId("wb-snippets-import").click();
      await page.getByTestId("wb-import-wizard-browse").click();
      await page.getByTestId("wb-import-wizard-next").click();
      await expect(page.getByTestId("wb-import-wizard")).toHaveAttribute(
        "data-step",
        "review",
      );
      await snap(page, "snippets-import-review", theme);
    });
  }
});
