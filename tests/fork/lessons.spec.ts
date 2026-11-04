// SPDX-License-Identifier: GPL-3.0-or-later
// S-31: the lesson engine and the Transforms lesson. Entry points (the
// Transforms banner's Try it / How it works, Start tutorial, Help ›
// Essentials, the checklist's "Try a Transform"), Try it out (example
// cards and ticker, practice: select + the Polish shortcut from the page's
// keydown or from the backend's `fork-lesson-shortcut`, the real preview
// or the example fallback, "Select … first", the spotlighted bar, View
// changes, one undo step, Skip), Turn on Transforms / Done, the checklist
// tick, How it works (three scenes, Back, Try it yourself), a custom
// chord, reduced motion, RTL, keyboard-only, axe light and dark. Mocked
// IPC; see hub-fixture.ts.
import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import type { TransformEntry } from "../../src/fork/bindings";
import type { IpcMock } from "./fixtures";
import {
  test,
  expect,
  openHub,
  expectNoSeriousAxeFindings,
  type HubState,
  type OpenHubOptions,
} from "./hub-fixture";
import { expectRtl, RTL_APP_SETTINGS } from "./pill-page";

// The rendered text of a string: headings mark their italic word with
// `<em>` and shortcuts with `<kbd>` (Design W5), which never show as text.
const en = JSON.parse(
  readFileSync(
    new URL("../../src/i18n/locales/en/fork.json", import.meta.url),
    "utf8",
  ),
  (_key, value) =>
    typeof value === "string" ? value.replace(/<\/?(em|kbd)>/g, "") : value,
);
const L = en.lessons.transforms;
const DRAFT: string = L.practice.draft;
const REWRITE =
  "Could we move the review to Thursday? The numbers aren't ready.";

const RULES = {
  shorter: true,
  clearer: true,
  better_order: true,
  headings_lists: false,
  keep_tone: true,
};
const entry = (id: number, key: string, name: string): TransformEntry => ({
  id,
  key,
  action_id: `transform.${key}`,
  builtin_key: key,
  name,
  description: "",
  rules: { ...RULES },
  prompt: `${name} this text.`,
  examples: [],
  replacements: [],
  updated_at: 0,
});

interface Backend {
  /** What the preview answers (`null`: an error, no engine). */
  preview: string | null;
  latest: unknown;
}

function install(ipc: IpcMock, hub: HubState, b: Backend): void {
  ipc.respond("fork_transforms_list", [
    entry(1, "polish", "Polish"),
    entry(2, "prompt_engineer", "Prompt Engineer"),
  ]);
  ipc.handle("fork_transform_preview", () => {
    if (b.preview === null)
      return { text: null, outcome: "no_engine", exact: false };
    return { text: b.preview, outcome: "changed", exact: false };
  });
  ipc.handle("fork_transform_diff", (args) => ({
    ops: [
      { kind: "delete", text: String(args.source).slice(0, 20) },
      { kind: "insert", text: "Could we move" },
      { kind: "equal", text: " the review" },
    ],
    changes: 2,
    degraded: false,
  }));
  ipc.handle("fork_transform_latest", () => b.latest);
  ipc.respond("fork_lesson_practice", null);
  ipc.handle("fork_checklist_mark", (args) => {
    const onboarding = hub.forkSettings.onboarding as {
      checklist_done?: string[];
    };
    const done = onboarding.checklist_done ?? [];
    if (!done.includes(args.id as string)) {
      onboarding.checklist_done = [...done, args.id as string];
    }
    return onboarding;
  });
}

async function open(
  page: Page,
  ipc: IpcMock,
  hub: HubState,
  b: Partial<Backend> = {},
  options: OpenHubOptions = {},
): Promise<Backend> {
  const backend: Backend = { preview: REWRITE, latest: null, ...b };
  await openHub(page, ipc, hub, {
    hash: "#/hub/transforms",
    ...options,
    beforeNavigate: () => {
      install(ipc, hub, backend);
      options.beforeNavigate?.();
    },
  });
  await expect(page.getByTestId("wb-transforms")).toBeVisible();
  return backend;
}

const lesson = (page: Page) => page.getByTestId("wb-lesson-transforms");
const heading = (page: Page) => page.getByTestId("wb-lesson-heading");
const primary = (page: Page) => page.getByTestId("wb-lesson-primary");
const field = (page: Page) => page.getByTestId("wb-practice-field");
// The sheet's back chevron (named "Back"; Design W5 moved it to the Sheet).
const back = (page: Page) =>
  page
    .getByRole("dialog")
    .getByRole("button", { name: en.lessons.common.back, exact: true });

async function toPractice(page: Page): Promise<void> {
  await page.getByTestId("wb-transforms-try-it").click();
  await expect(lesson(page)).toHaveAttribute("data-step", "examples");
  await primary(page).click();
  await expect(lesson(page)).toHaveAttribute("data-step", "practice");
}

async function selectAll(page: Page): Promise<void> {
  await field(page).evaluate((el: HTMLTextAreaElement) => {
    el.focus();
    el.setSelectionRange(0, el.value.length);
  });
}

test.describe("Transforms lesson: Try it out", () => {
  test("examples, then practice with the Polish shortcut, View changes and undo", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub);
    await page.getByTestId("wb-transforms-try-it").click();
    await expect(lesson(page)).toBeVisible();
    await expect(heading(page)).toHaveText(L.intro.title);
    for (const kind of ["original", "polish", "prompt"]) {
      await expect(page.getByTestId(`wb-lesson-card-${kind}`)).toContainText(
        L.cards[kind].text,
      );
    }
    await expect(page.getByTestId("wb-lesson-card-polish")).toContainText(
      "⌥ 1",
    );
    await expect(page.getByTestId("wb-lesson-ticker")).toContainText(
      L.benefits.tone,
    );
    await expect(page.getByTestId("wb-progress-dashes")).toHaveAttribute(
      "aria-label",
      "Step 1 of 2",
    );
    await expect(page.getByTestId("wb-lesson-skip")).toHaveCount(0);
    await primary(page).click();

    await expect(heading(page)).toHaveText("Select the text, then press ⌥ 1");
    await expect(heading(page)).toBeFocused();
    // One italic word and the shortcut as key caps (Design W5).
    await expect(heading(page).locator("em")).toHaveText("Select");
    await expect(heading(page).locator("kbd")).toHaveText(["⌥", "1"]);
    await expect(primary(page)).toBeDisabled();
    await expect(page.getByTestId("wb-lesson-skip")).toBeVisible();
    await expect(field(page)).toHaveValue(DRAFT);
    await ipc.expectCalled("fork_lesson_practice", { on: true });

    // Keyboard only: into the field, select all, ⌥1.
    await field(page).focus();
    await page.keyboard.press("ControlOrMeta+a");
    await page.keyboard.press("Alt+1");
    await ipc.expectCalled("fork_transform_preview", { id: 1, source: DRAFT });
    await expect(field(page)).toHaveValue(REWRITE);
    await expect(heading(page)).toHaveText(L.practice.doneTitle);
    await expect(page.getByTestId("wb-lesson-body")).toHaveText(
      "Press ⌥ O to view the changes. ⌘ Z undoes it.",
    );
    // The bar is spotlighted; its View changes has focus.
    await expect(page.getByTestId("wb-lesson-spotlight")).toBeVisible();
    await expect(page.getByTestId("wb-lesson-bar")).toHaveAttribute(
      "data-spot",
      "true",
    );
    await expect(page.getByTestId("wb-lesson-view-changes")).toBeFocused();
    await expect(page.getByTestId("wb-lesson-example-note")).toHaveCount(0);
    await page.keyboard.press("Enter");
    await expect(page.getByTestId("wb-lesson-diff")).toContainText("2 changes");
    await expect(page.getByTestId("wb-lesson-spotlight")).toHaveCount(0);
    await ipc.expectCalled("fork_transform_diff", {
      source: DRAFT,
      result: REWRITE,
    });
    // One undo step puts the original back (as ⌘Z does in the field).
    await field(page).evaluate(() => document.execCommand("undo"));
    await expect(field(page)).toHaveValue(DRAFT);

    await expect(primary(page)).toBeEnabled();
    await expect(primary(page)).toHaveText(L.finish.turnOn);
    await primary(page).click();
    await expect(lesson(page)).toHaveCount(0);
    await expect.poll(() => hub.forkSettings.transforms_enabled).toBe(true);
    await ipc.expectCalled("fork_checklist_mark", { id: "try_transform" });
    await ipc.expectCalled("fork_lesson_practice", { on: false });
    await expect(page.getByTestId("wb-transforms-enabled")).toBeChecked();
  });

  test("⌥O shows the changes; a shortcut with nothing selected asks for a selection", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub);
    await toPractice(page);
    await field(page).evaluate((el: HTMLTextAreaElement) => {
      el.focus();
      el.setSelectionRange(3, 3);
    });
    await page.keyboard.press("Alt+1");
    await expect(page.getByTestId("wb-lesson-select-first")).toHaveText(
      "Select some of the message first, then press ⌥ 1 again.",
    );
    expect(await ipc.calls("fork_transform_preview")).toHaveLength(0);
    // ⌥1 typed no character into the field.
    await expect(field(page)).toHaveValue(DRAFT);
    await selectAll(page);
    await page.keyboard.press("Alt+1");
    await expect(heading(page)).toHaveText(L.practice.doneTitle);
    await page.keyboard.press("Alt+o");
    await expect(page.getByTestId("wb-lesson-diff")).toBeVisible();
    await page.getByTestId("wb-lesson-diff-hide").click();
    await expect(page.getByTestId("wb-lesson-diff")).toHaveCount(0);
  });

  test("the backend hands over the shortcut while Transforms are on", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub, {}, { fork: { transforms_enabled: true } });
    await toPractice(page);
    await selectAll(page);
    await ipc.waitForListener("fork-lesson-shortcut");
    // An unrelated action is ignored.
    await ipc.emit("fork-lesson-shortcut", { action: "dictation.toggle" });
    await ipc.emit("fork-lesson-shortcut", { action: "transform.polish" });
    await expect(field(page)).toHaveValue(REWRITE);
    await expect(heading(page)).toHaveText(L.practice.doneTitle);
    await ipc.emit("fork-lesson-shortcut", {
      action: "transform.view_changes",
    });
    await expect(page.getByTestId("wb-lesson-diff")).toBeVisible();
    // Transforms are on already: the last button is Done.
    await expect(primary(page)).toHaveText(L.finish.done);
    await ipc.clearCalls();
    await primary(page).click();
    await expect(lesson(page)).toHaveCount(0);
    expect(
      (await ipc.calls("fork_set_setting")).filter(
        (c) => (c.args.update as { key: string }).key === "transforms_enabled",
      ),
    ).toHaveLength(0);
  });

  test("Prompt Engineer's shortcut runs Prompt Engineer", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub);
    await toPractice(page);
    await selectAll(page);
    await page.keyboard.press("Alt+2");
    await ipc.expectCalled("fork_transform_preview", { id: 2 });
    await expect(heading(page)).toHaveText(L.practice.doneTitle);
  });

  test("without a ready engine the example rewrite says so", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub, { preview: null });
    await toPractice(page);
    await field(page).evaluate((el: HTMLTextAreaElement) => {
      el.focus();
      el.setSelectionRange(0, 10);
    });
    await page.keyboard.press("Alt+1");
    await expect(field(page)).toHaveValue(L.practice.example.polish);
    await expect(page.getByTestId("wb-lesson-example-note")).toHaveText(
      L.practice.exampleNote,
    );
    await expect(primary(page)).toBeEnabled();
  });

  test("Skip never dead-ends: Turn on Transforms without the practice", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub);
    await toPractice(page);
    await page.getByTestId("wb-lesson-skip").click();
    await expect(page.getByTestId("wb-lesson-skip")).toHaveCount(0);
    await primary(page).click();
    await expect(lesson(page)).toHaveCount(0);
    await expect.poll(() => hub.forkSettings.transforms_enabled).toBe(true);
    // Skipped: the checklist item does not tick.
    expect(await ipc.calls("fork_checklist_mark")).toHaveLength(0);
  });

  test("Esc closes; Back returns to the examples", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub);
    await toPractice(page);
    await back(page).click();
    await expect(lesson(page)).toHaveAttribute("data-step", "examples");
    await page.keyboard.press("Escape");
    await expect(lesson(page)).toHaveCount(0);
    await ipc.expectCalled("fork_lesson_practice", { on: false });
    expect(hub.forkSettings.transforms_enabled).toBe(false);
  });

  test("a custom chord is the one shown and the one that works", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(
      page,
      ipc,
      hub,
      {},
      {
        fork: {
          hotkeys: {
            "transform.polish": [
              { chord: "ctrl+option+1", activation: "press" },
            ],
          },
        },
      },
    );
    await toPractice(page);
    await expect(heading(page)).toHaveText("Select the text, then press ⌃ ⌥ 1");
    await selectAll(page);
    await page.keyboard.press("Alt+1");
    await expect(heading(page)).toHaveText("Select the text, then press ⌃ ⌥ 1");
    await page.keyboard.press("Control+Alt+1");
    await expect(heading(page)).toHaveText(L.practice.doneTitle);
  });
});

test.describe("Transforms lesson: How it works", () => {
  test("three scenes, Back, then Try it yourself", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub);
    await page.getByTestId("wb-transforms-how-it-works").click();
    await expect(lesson(page)).toHaveAttribute("data-mode", "how");
    await expect(heading(page)).toHaveText(L.how.select.title);
    await expect(page.getByTestId("wb-lesson-scene")).toBeVisible();
    await expect(page.getByTestId("wb-lesson-scene-pause")).toBeVisible();
    await expect(page.getByTestId("wb-progress-dashes")).toHaveAttribute(
      "aria-label",
      "Step 1 of 3",
    );
    await primary(page).click();
    await expect(heading(page)).toHaveText("Press ⌥ 1");
    await expect(
      page.getByTestId("wb-lesson-scene").getByRole("img"),
    ).toHaveAccessibleName(/⌥ 1 is pressed/);
    await back(page).click();
    await expect(heading(page)).toHaveText(L.how.select.title);
    await primary(page).click();
    await primary(page).click();
    await expect(heading(page)).toHaveText(L.how.changes.title);
    await expect(primary(page)).toHaveText(en.lessons.common.tryIt);
    await primary(page).click();
    await expect(lesson(page)).toHaveAttribute("data-mode", "try");
    await expect(heading(page)).toHaveText(L.intro.title);
    // Back from the first Try step returns to the last scene.
    await back(page).click();
    await expect(heading(page)).toHaveText(L.how.changes.title);
  });

  test("reduced motion shows the finished scene and no pause button", async ({
    page,
    ipc,
    hub,
  }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await open(page, ipc, hub);
    await page.getByTestId("wb-transforms-how-it-works").click();
    await expect(page.locator(".wb-lesson-stage")).toHaveAttribute(
      "data-beat",
      "selected",
    );
    await expect(page.getByTestId("wb-lesson-scene-pause")).toHaveCount(0);
    await primary(page).click();
    await primary(page).click();
    await expect(page.locator(".wb-lesson-stage")).toHaveAttribute(
      "data-beat",
      "diff",
    );
    await expect(page.locator(".wb-scene-diff")).toContainText("changes");
  });
});

test.describe("Lesson re-entry points", () => {
  test("Try it and Help › Essentials open the lesson", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub);
    await page.getByTestId("wb-transforms-try-it").click();
    await expect(lesson(page)).toHaveAttribute("data-step", "examples");
    await page.keyboard.press("Escape");
    await expect(lesson(page)).toHaveCount(0);

    await page.evaluate(() => {
      window.location.hash = "#/hub/dictation";
    });
    await page.getByTestId("wb-nav-help").click();
    await expect(page.getByTestId("wb-help-menu")).toBeVisible();
    await page.getByTestId("wb-menu-item-lesson-transforms").click();
    await expect(lesson(page)).toHaveAttribute("data-step", "examples");
    await expect(page.getByTestId("wb-help-menu")).toHaveCount(0);
    // The dialog keeps focus (the menu did not take it back).
    await expect(
      page.locator('[role="dialog"]').filter({ has: lesson(page) }),
    ).toBeFocused();
  });

  const completed = (checklist_done: string[]) => ({
    status: "completed",
    current_step: null,
    completed_steps: [],
    checklist_done,
    completed_at_ms: 1,
  });

  test("the checklist's Try a Transform opens the lesson and ticks after practice", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(
      page,
      ipc,
      hub,
      {},
      {
        fork: {
          first_use_choice: "email",
          onboarding: completed(["try_in_app", "add_word", "create_snippet"]),
        },
      },
    );
    const item = page.getByTestId("wb-checklist-try_transform");
    await expect(item).toContainText(en.checklist.items.try_transform.title);
    await page.getByTestId("wb-checklist-run-try_transform").click();
    await expect(lesson(page)).toHaveAttribute("data-step", "examples");
    await primary(page).click();
    await selectAll(page);
    await page.keyboard.press("Alt+1");
    await expect(heading(page)).toHaveText(L.practice.doneTitle);
    // No tick (and so no chaining) while the lesson is open.
    expect(await ipc.calls("fork_checklist_mark")).toHaveLength(0);
    await page.keyboard.press("Escape");
    await ipc.expectCalled("fork_checklist_mark", { id: "try_transform" });
  });

  test("a real transform run ticks the checklist item", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(
      page,
      ipc,
      hub,
      {
        latest: {
          run_id: 1,
          transform_id: 1,
          app_bundle_id: "com.apple.TextEdit",
          source_text: "a",
          result_text: "A.",
          rating: null,
          created_at_ms: 1,
        },
      },
      {
        fork: {
          first_use_choice: "email",
          onboarding: completed(["try_in_app"]),
        },
      },
    );
    await ipc.expectCalled("fork_checklist_mark", { id: "try_transform" });
  });
});

test("RTL: the lesson mirrors and still works", async ({ page, ipc, hub }) => {
  await open(page, ipc, hub, {}, { handy: RTL_APP_SETTINGS });
  await expectRtl(page);
  await page.getByTestId("wb-transforms-try-it").click();
  await expect(lesson(page)).toBeVisible();
  const dir = await page.evaluate(
    () =>
      getComputedStyle(document.querySelector('[role="dialog"]')!).direction,
  );
  expect(dir).toBe("rtl");
  await primary(page).click();
  await selectAll(page);
  await page.keyboard.press("Alt+1");
  await expect(page.getByTestId("wb-lesson-bar")).toHaveAttribute(
    "data-state",
    "done",
  );
});

for (const theme of ["light", "dark"] as const) {
  test(`axe: examples, practice with the spotlight, a scene (${theme})`, async ({
    page,
    ipc,
    hub,
  }) => {
    test.setTimeout(180_000);
    await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
    await open(page, ipc, hub, {}, { handy: { theme } });
    await expectNoSeriousAxeFindings(page);
    await page.getByTestId("wb-transforms-try-it").click();
    await expect(lesson(page)).toBeVisible();
    await expectNoSeriousAxeFindings(page);
    await primary(page).click();
    await selectAll(page);
    await page.keyboard.press("Alt+1");
    await expect(page.getByTestId("wb-lesson-spotlight")).toBeVisible();
    await expectNoSeriousAxeFindings(page);
    await page.getByTestId("wb-lesson-view-changes").click();
    await expect(page.getByTestId("wb-lesson-diff")).toBeVisible();
    await expectNoSeriousAxeFindings(page);
    await page.keyboard.press("Escape");
    await page.getByTestId("wb-transforms-how-it-works").click();
    await expect(page.getByTestId("wb-lesson-scene")).toBeVisible();
    await expectNoSeriousAxeFindings(page);
  });
}
