// SPDX-License-Identifier: GPL-3.0-or-later
// Design W5: screenshots of every lesson, quiz, first-use and checklist
// screen, light and dark, at the Hub's default size. Skipped unless
// WB_DESIGN_SHOTS_DIR is set:
//
//   WB_DESIGN_SHOTS_DIR=/tmp/shots bun run wb:test:ui -- lesson-shots
//
// Mocked data only (no personal data). Reduced motion, so every scene shows
// its finished frame.
import path from "node:path";
import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import type { IpcMock } from "./fixtures";
import { test, expect, openHub, type HubState } from "./hub-fixture";

const dir = process.env.WB_DESIGN_SHOTS_DIR;
const en = JSON.parse(
  readFileSync(
    new URL("../../src/i18n/locales/en/fork.json", import.meta.url),
    "utf8",
  ),
);
const SENTENCE: string = en.lessons.whisper.practice.sentence;
const REWRITE =
  "Could we move the review to Thursday? The numbers aren't ready yet.";

function completed(checklist_done: string[] = []) {
  return {
    status: "completed",
    current_step: null,
    completed_steps: [],
    checklist_done,
    completed_at_ms: 1,
  };
}

function install(ipc: IpcMock, hub: HubState): void {
  const rules = {
    shorter: true,
    clearer: true,
    better_order: true,
    headings_lists: false,
    keep_tone: true,
  };
  const entry = (id: number, key: string, name: string) => ({
    id,
    key,
    action_id: `transform.${key}`,
    builtin_key: key,
    name,
    description: "",
    rules,
    prompt: `${name} this text.`,
    examples: [],
    replacements: [],
    updated_at: 0,
  });
  ipc.respond("fork_transforms_list", [
    entry(1, "polish", "Polish"),
    entry(2, "prompt_engineer", "Prompt Engineer"),
  ]);
  ipc.handle("fork_transform_preview", () => ({
    text: REWRITE,
    outcome: "changed",
    exact: false,
  }));
  ipc.handle("fork_transform_diff", (args) => ({
    ops: [
      { kind: "delete", text: String(args.source).slice(0, 40) },
      { kind: "insert", text: "Could we move" },
      { kind: "equal", text: " the review" },
    ],
    changes: 2,
    degraded: false,
  }));
  ipc.respond("fork_transform_latest", null);
  ipc.respond("fork_lesson_practice", null);
  ipc.respond("fork_onboarding_practice", null);
  ipc.respond("fork_onboarding_open_pane", null);
  ipc.respond("fork_spotlight_start", true);
  ipc.respond("fork_spotlight_stop", null);
  ipc.respond("fork_spotlight_state", {
    seq: 0,
    active: false,
    choice: null,
    hole: null,
    edge: "bottom",
  });
  ipc.handle("fork_hotkey_status", () => ({
    running: true,
    error: null,
    trigger: "fn",
    globe: "do_nothing",
    fn_blocked: false,
    issues: [],
    bindings: [],
  }));
  ipc.handle("fork_checklist_mark", (args) => {
    const onboarding = hub.forkSettings.onboarding as {
      checklist_done: string[];
    };
    onboarding.checklist_done = [...onboarding.checklist_done, String(args.id)];
    return onboarding;
  });
  ipc.respond("fork_snippets_list", {
    snippets: [],
    counts: { snippets: 0, uses: 0 },
  });
  ipc.respond("fork_dictionary_list", {
    entries: [],
    counts: { words: 0, rules: 0, suggested: 0 },
  });
}

test.describe("lesson shots", () => {
  test.skip(!dir, "set WB_DESIGN_SHOTS_DIR to write screenshots");
  test.setTimeout(240_000);

  for (const theme of ["light", "dark"] as const) {
    const shot = (page: Page, name: string) =>
      page.screenshot({ path: path.join(dir!, `${name}-${theme}.png`) });
    const primary = (page: Page) => page.getByTestId("wb-lesson-primary");
    const step = (page: Page, lesson: string, id: string) =>
      expect(page.getByTestId(`wb-lesson-${lesson}`)).toHaveAttribute(
        "data-step",
        id,
      );
    const settle = (page: Page) => page.waitForTimeout(500);

    test(`transforms ${theme}`, async ({ page, ipc, hub }) => {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      await openHub(page, ipc, hub, {
        hash: "#/hub/transforms",
        handy: { theme },
        fork: {
          first_use_choice: "email",
          onboarding: completed(["try_in_app"]),
        },
        beforeNavigate: () => install(ipc, hub),
      });
      await page.getByTestId("wb-transforms-try-it").click();
      await step(page, "transforms", "examples");
      await settle(page);
      await shot(page, "transforms-1-examples");
      await primary(page).click();
      await step(page, "transforms", "practice");
      await settle(page);
      await shot(page, "transforms-2-practice");
      await page
        .getByTestId("wb-practice-field")
        .evaluate((el: HTMLTextAreaElement) => {
          el.focus();
          el.setSelectionRange(0, el.value.length);
        });
      await page.keyboard.press("Alt+1");
      await expect(page.getByTestId("wb-lesson-bar")).toHaveAttribute(
        "data-state",
        "done",
      );
      await settle(page);
      await shot(page, "transforms-3-spotlight");
      await page.getByTestId("wb-lesson-view-changes").click();
      await expect(page.getByTestId("wb-lesson-diff")).toBeVisible();
      await settle(page);
      await shot(page, "transforms-4-changes");
      await page.keyboard.press("Escape");
      await page.getByTestId("wb-transforms-how-it-works").click();
      await step(page, "transforms", "select");
      await settle(page);
      await shot(page, "transforms-5-how-select");
      await primary(page).click();
      await primary(page).click();
      await step(page, "transforms", "changes");
      await settle(page);
      await shot(page, "transforms-6-how-changes");
    });

    test(`whisper ${theme}`, async ({ page, ipc, hub }) => {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      await openHub(page, ipc, hub, {
        handy: { theme },
        beforeNavigate: () => install(ipc, hub),
      });
      await page.getByTestId("wb-home-whisper").click();
      await step(page, "whisper", "soft");
      await settle(page);
      await shot(page, "whisper-1-soft");
      await primary(page).click();
      await step(page, "whisper", "how");
      await settle(page);
      await shot(page, "whisper-2-how");
      await primary(page).click();
      await step(page, "whisper", "practice");
      await ipc.waitForListener("fork-practice-take");
      await ipc.waitForListener("recording-state-event");
      await settle(page);
      await shot(page, "whisper-3-practice");
      const words = SENTENCE.split(/\s+/);
      await ipc.emit("recording-state-event", { phase: "started" });
      await ipc.emit("recording-state-event", { phase: "stopped" });
      await ipc.emit("fork-practice-take", {
        kind: "text",
        text: words.slice(0, 6).join(" "),
        late: false,
      });
      await expect(page.getByTestId("wb-whisper-tips")).toBeVisible();
      await settle(page);
      await shot(page, "whisper-4-tips");
      await ipc.emit("recording-state-event", { phase: "started" });
      await ipc.emit("recording-state-event", { phase: "stopped" });
      await ipc.emit("fork-practice-take", {
        kind: "text",
        text: SENTENCE,
        late: false,
      });
      await expect(primary(page)).toBeEnabled();
      await settle(page);
      await shot(page, "whisper-5-passed");
      await primary(page).click();
      await step(page, "whisper", "quiz_comfortable");
      await page.getByTestId("wb-quiz-comfortable-wired_headset").click();
      await settle(page);
      await shot(page, "whisper-6-quiz-mics");
      await primary(page).click();
      await page.getByTestId("wb-quiz-workplace-shared").click();
      await settle(page);
      await shot(page, "whisper-7-quiz-workplace");
      await primary(page).click();
      await page.getByTestId("wb-quiz-moves-different_places").click();
      await settle(page);
      await shot(page, "whisper-8-quiz-moves");
      await primary(page).click();
      await step(page, "whisper", "quiz_owned");
      await settle(page);
      await shot(page, "whisper-9-quiz-owned");
      await primary(page).click();
      await step(page, "whisper", "results");
      await settle(page);
      await shot(page, "whisper-10-results");
    });

    test(`first use and checklist ${theme}`, async ({ page, ipc, hub }) => {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      await openHub(page, ipc, hub, {
        handy: { theme },
        fork: { onboarding: completed() },
        beforeNavigate: () => install(ipc, hub),
      });
      await expect(page.getByTestId("wb-first-use")).toBeVisible();
      await settle(page);
      await shot(page, "first-use-1-email");
      await page.getByTestId("wb-first-use-coding").click();
      await settle(page);
      await shot(page, "first-use-2-coding");
      await page.getByTestId("wb-first-use-later").click();
      await expect(page.getByTestId("wb-first-use")).toHaveCount(0);
      await expect(page.getByTestId("wb-checklist")).toBeVisible();
      await settle(page);
      await shot(page, "checklist-1-start");
    });

    test(`checklist progress ${theme}`, async ({ page, ipc, hub }) => {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      await openHub(page, ipc, hub, {
        handy: { theme },
        fork: {
          first_use_choice: "email",
          onboarding: completed(["try_in_app", "add_word"]),
        },
        beforeNavigate: () => install(ipc, hub),
      });
      await expect(page.getByTestId("wb-checklist")).toBeVisible();
      await settle(page);
      await page
        .getByTestId("wb-checklist")
        .screenshot({ path: path.join(dir!, `checklist-2-card-${theme}.png`) });
      await shot(page, "checklist-2-progress");
    });
  }
});
