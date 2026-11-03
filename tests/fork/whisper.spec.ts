// SPDX-License-Identifier: GPL-3.0-or-later
// S-32: the whisper lesson and the microphone quiz. Entry points (the home
// banner's "Show me how", Help › Essentials, the checklist's "Try
// whispering" between Try a Transform and Open at login), the soft-voice
// scene, how to whisper, the practice take (practice mode on/off, a take
// with ≥ 70 % of the words passes, fewer shows tips and keeps Continue off,
// Sound settings, an empty take, the queued take's written example, the
// Control+Option and unbound copy, Skip), the four quiz questions and the
// table with its Best match, More about microphones, Copy results, Done,
// the checklist tick after the practice and S-30's chaining that only
// points at the next item. Keyboard-only, reduced motion, RTL, axe light
// and dark. Mocked IPC; see hub-fixture.ts.
import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
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

const en = JSON.parse(
  readFileSync(
    new URL("../../src/i18n/locales/en/fork.json", import.meta.url),
    "utf8",
  ),
);
const W = en.lessons.whisper;
const SENTENCE: string = W.practice.sentence;
const MICS_DOC =
  "https://github.com/fjenn/wispr-bro/blob/wispr-bro/docs/fork/user/microphones.md";

interface Backend {
  trigger: string | null;
}

function install(ipc: IpcMock, hub: HubState, b: Backend): void {
  ipc.respond("fork_onboarding_practice", null);
  ipc.respond("fork_onboarding_open_pane", null);
  ipc.handle("fork_hotkey_status", () => ({
    running: true,
    error: null,
    trigger: b.trigger,
    globe: "do_nothing",
    fn_blocked: false,
    issues: [],
    bindings: [],
  }));
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
  ipc.handle("change_autostart_setting", (args) => {
    hub.handy.autostart_enabled = args.enabled;
    return null;
  });
  ipc.respond("fork_snippets_list", {
    snippets: [],
    counts: { snippets: 0, uses: 0 },
  });
  ipc.respond("fork_transform_latest", null);
}

async function open(
  page: Page,
  ipc: IpcMock,
  hub: HubState,
  b: Partial<Backend> = {},
  options: OpenHubOptions = {},
): Promise<Backend> {
  const backend: Backend = { trigger: "fn", ...b };
  await openHub(page, ipc, hub, {
    ...options,
    beforeNavigate: () => {
      install(ipc, hub, backend);
      options.beforeNavigate?.();
    },
  });
  return backend;
}

const lesson = (page: Page) => page.getByTestId("wb-lesson-whisper");
const heading = (page: Page) => page.getByTestId("wb-lesson-heading");
const primary = (page: Page) => page.getByTestId("wb-lesson-primary");
const skip = (page: Page) => page.getByTestId("wb-lesson-skip");
const field = (page: Page) => page.getByTestId("wb-practice-field");
const step = (page: Page, id: string) =>
  expect(lesson(page)).toHaveAttribute("data-step", id);

async function fromBanner(page: Page): Promise<void> {
  await page.getByTestId("wb-home-whisper").click();
  await step(page, "soft");
}

async function toPractice(page: Page, ipc: IpcMock): Promise<void> {
  await fromBanner(page);
  await primary(page).click();
  await step(page, "how");
  await primary(page).click();
  await step(page, "practice");
  await ipc.waitForListener("fork-practice-take");
  await ipc.waitForListener("recording-state-event");
}

async function whisper(ipc: IpcMock, text: string): Promise<void> {
  await ipc.emit("recording-state-event", { phase: "started" });
  await ipc.emit("recording-state-event", { phase: "stopped" });
  await ipc.emit("fork-practice-take", { kind: "text", text, late: false });
}

/** The first ⌈70 %⌉ of the sentence's words, and one word fewer. */
function partial(share: "pass" | "short"): string {
  const words = SENTENCE.split(/\s+/);
  const need = Math.ceil(words.length * 0.7);
  return words.slice(0, share === "pass" ? need : need - 1).join(" ");
}

function passPercent(): number {
  const words = SENTENCE.split(/\s+/);
  return Math.round((Math.ceil(words.length * 0.7) / words.length) * 100);
}

test.beforeEach(async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
});

test.describe("Whisper lesson: the practice take", () => {
  test("banner → scene → how → a whispered take that passes", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub);
    await expect(page.getByTestId("wb-home-whisper")).toHaveText(
      en.hub.pages.dictation.banner.quiet,
    );
    await fromBanner(page);
    await expect(heading(page)).toHaveText(W.soft.title);
    await expect(page.getByTestId("wb-lesson-scene")).toBeVisible();
    await primary(page).click();
    await expect(heading(page)).toHaveText(W.how.title);
    for (const tip of ["distance", "breath", "pace", "release"]) {
      await expect(page.getByTestId(`wb-whisper-tip-${tip}`)).toBeVisible();
    }
    await expect(page.getByTestId("wb-whisper-distance-art")).toBeVisible();
    await primary(page).click();
    await step(page, "practice");
    await ipc.expectCalled("fork_onboarding_practice", { on: true });
    await expect(page.getByTestId("wb-whisper-instruction")).toContainText(
      "Hold fn",
    );
    await expect(page.getByTestId("wb-whisper-sentence")).toContainText(
      SENTENCE,
    );
    // Gated: Continue off, Skip offered.
    await expect(primary(page)).toBeDisabled();
    await expect(skip(page)).toBeVisible();

    await ipc.waitForListener("fork-practice-take");
    await ipc.waitForListener("recording-state-event");
    await ipc.emit("recording-state-event", { phase: "started" });
    await expect(page.getByTestId("wb-whisper-practice")).toHaveAttribute(
      "data-phase",
      "listening",
    );
    await ipc.emit("recording-state-event", { phase: "stopped" });
    await expect(page.getByTestId("wb-whisper-practice")).toHaveAttribute(
      "data-phase",
      "working",
    );
    await ipc.emit("fork-practice-take", {
      kind: "text",
      text: partial("pass"),
      late: false,
    });
    await expect(field(page)).toHaveValue(partial("pass"));
    await expect(heading(page)).toHaveText(W.practice.doneTitle);
    await expect(primary(page)).toBeEnabled();
    await expect(skip(page)).toHaveCount(0);
    await expect(page.getByTestId("wb-whisper-status")).toContainText(
      `${passPercent()}% of the words came through`,
    );
    // The field holds the take; it is not typed into.
    await expect(field(page)).toHaveAttribute("readonly", "");
    await primary(page).click();
    await step(page, "quiz_comfortable");
    // Leaving the practice step turns practice mode off.
    await ipc.expectCalled("fork_onboarding_practice", { on: false });
  });

  test("under 70 % shows tips and keeps Continue off; the next take can pass", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub);
    await toPractice(page, ipc);
    await whisper(ipc, partial("short"));
    await expect(page.getByTestId("wb-whisper-practice")).toHaveAttribute(
      "data-phase",
      "low",
    );
    const tips = page.getByTestId("wb-whisper-tips");
    await expect(tips).toContainText(W.practice.tips.closer);
    await expect(tips).toContainText(W.practice.tips.volume);
    await expect(primary(page)).toBeDisabled();
    await expect(skip(page)).toBeVisible();
    await page.getByTestId("wb-whisper-open-sound").click();
    await ipc.expectCalled("fork_onboarding_open_pane", {
      pane: "sound_input",
    });
    await whisper(ipc, SENTENCE.toLowerCase());
    await expect(heading(page)).toHaveText(W.practice.doneTitle);
    await expect(page.getByTestId("wb-whisper-tips")).toHaveCount(0);
    await expect(primary(page)).toBeEnabled();
  });

  test("an empty take says nothing came through", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub);
    await toPractice(page, ipc);
    await whisper(ipc, "  ");
    await expect(page.getByTestId("wb-whisper-status")).toHaveText(
      W.practice.status.nothing,
    );
    await expect(page.getByTestId("wb-whisper-tips")).toBeVisible();
    await expect(primary(page)).toBeDisabled();
  });

  test("with no speech model yet, a written example that says so", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub);
    await toPractice(page, ipc);
    await ipc.emit("recording-state-event", { phase: "started" });
    await ipc.emit("recording-state-event", { phase: "stopped" });
    await ipc.emit("fork-practice-take", {
      kind: "queued",
      text: null,
      late: false,
    });
    await expect(page.getByTestId("wb-whisper-example-note")).toHaveText(
      W.practice.exampleNote,
    );
    await expect(page.getByTestId("wb-whisper-status")).toHaveText(
      W.practice.status.example,
    );
    await expect(field(page)).toHaveValue(SENTENCE);
    await expect(primary(page)).toBeEnabled();
    // The queued take's own text replaces the example when it arrives.
    await ipc.emit("fork-practice-take", {
      kind: "text",
      text: SENTENCE,
      late: true,
    });
    await expect(page.getByTestId("wb-whisper-example-note")).toHaveCount(0);
    await expect(page.getByTestId("wb-whisper-status")).toContainText("100%");
  });

  test("Control+Option wording, and no shortcut: Skip still goes on", async ({
    page,
    ipc,
    hub,
  }) => {
    const b = await open(page, ipc, hub, { trigger: "ctrl+option" });
    await toPractice(page, ipc);
    await expect(page.getByTestId("wb-whisper-instruction")).toContainText(
      "Hold Control and Option",
    );
    await page.keyboard.press("Escape");
    await expect(lesson(page)).toHaveCount(0);

    b.trigger = null;
    await toPractice(page, ipc);
    await expect(page.getByTestId("wb-whisper-unbound")).toHaveText(
      W.practice.unbound,
    );
    await skip(page).click();
    await expect(primary(page)).toBeEnabled();
    await primary(page).click();
    await step(page, "quiz_comfortable");
  });
});

test.describe("Whisper lesson: the microphone quiz", () => {
  async function toQuiz(page: Page, ipc: IpcMock): Promise<void> {
    await toPractice(page, ipc);
    await skip(page).click();
    await primary(page).click();
    await step(page, "quiz_comfortable");
  }

  test("unanswered questions never block; the default match is a headset", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub);
    await toQuiz(page, ipc);
    for (const id of ["quiz_workplace", "quiz_moves", "quiz_owned"]) {
      await expect(primary(page)).toBeEnabled();
      await primary(page).click();
      await step(page, id);
    }
    // The built-in microphone starts ticked.
    await expect(
      page.getByTestId("wb-quiz-owned-built_in").locator("input"),
    ).toBeChecked();
    await expect(primary(page)).toHaveText(W.quiz.owned.continue);
    await primary(page).click();
    await step(page, "results");
    await expect(page.getByTestId("wb-quiz-results")).toHaveAttribute(
      "data-best",
      "wired_headset",
    );
    await expect(primary(page)).toHaveText(W.results.done);
    await primary(page).click();
    await expect(lesson(page)).toHaveCount(0);
  });

  test("answers pick the best match; More about microphones; Copy results", async ({
    page,
    ipc,
    hub,
  }) => {
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
    await open(page, ipc, hub);
    await toQuiz(page, ipc);
    for (const kind of [
      "built_in",
      "wired_headset",
      "wireless_earbuds",
      "desk_usb",
    ]) {
      await expect(
        page.getByTestId(`wb-quiz-comfortable-${kind}`),
      ).toContainText(W.mics[kind].name);
      await expect(page.getByTestId(`wb-mic-art-${kind}`)).toBeVisible();
    }
    await page.getByTestId("wb-quiz-more-mics").click();
    await ipc.expectCalled("plugin:opener|open_url", { url: MICS_DOC });
    await page.getByTestId("wb-quiz-comfortable-desk_usb").click();
    await primary(page).click();
    await page.getByTestId("wb-quiz-workplace-private").click();
    await primary(page).click();
    await page.getByTestId("wb-quiz-moves-one_desk").click();
    await primary(page).click();
    await primary(page).click();
    const results = page.getByTestId("wb-quiz-results");
    await expect(results).toHaveAttribute("data-best", "desk_usb");
    const row = page.getByTestId("wb-quiz-row-desk_usb");
    await expect(row).toHaveAttribute("data-best", "true");
    await expect(row).toContainText(W.results.columns.best);
    await expect(
      page.locator("[data-best='true'][data-testid^=wb-quiz-row]"),
    ).toHaveCount(1);
    // The table's columns.
    const table = page.getByTestId("wb-quiz-table");
    for (const col of ["mic", "quality", "others", "home", "price", "best"]) {
      await expect(
        table.getByRole("columnheader", { name: W.results.columns[col] }),
      ).toBeVisible();
    }
    await expect(page.getByTestId("wb-quiz-row-built_in")).toContainText(
      W.results.price.owned,
    );
    await page.getByTestId("wb-quiz-copy").click();
    await expect(page.getByTestId("wb-quiz-copy")).toHaveText(W.results.copied);
    const copied = await page.evaluate(
      () => (window as unknown as { __copied: string[] }).__copied,
    );
    expect(copied).toHaveLength(1);
    expect(copied[0]).toContain(`Best match: ${W.mics.desk_usb.name}`);
    expect(copied[0]).toContain(W.quiz.workplace.private.label);

    // Back keeps the answers within one opening …
    await page.getByTestId("wb-lesson-back").click();
    await page.getByTestId("wb-lesson-back").click();
    await expect(
      page.getByTestId("wb-quiz-moves-one_desk").locator("input"),
    ).toBeChecked();
    // … and a new opening starts afresh.
    await page.keyboard.press("Escape");
    await toQuiz(page, ipc);
    await expect(
      page.getByTestId("wb-quiz-comfortable-desk_usb").locator("input"),
    ).not.toBeChecked();
  });

  test("keyboard only: the whole lesson, arrows in a question, Space on a box", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub);
    await page.getByTestId("wb-home-whisper").focus();
    await page.keyboard.press("Enter");
    await step(page, "soft");
    await expect(
      page.locator('[role="dialog"]').filter({ has: lesson(page) }),
    ).toBeFocused();
    const press = async (testId: string) => {
      await page.getByTestId(testId).focus();
      await page.keyboard.press("Enter");
    };
    await press("wb-lesson-primary");
    await step(page, "how");
    await press("wb-lesson-primary");
    await step(page, "practice");
    await press("wb-lesson-skip");
    await press("wb-lesson-primary");
    await step(page, "quiz_comfortable");
    // Tab into the radio group, arrows move the choice.
    const first = page
      .getByTestId("wb-quiz-comfortable-wired_headset")
      .locator("input");
    await first.focus();
    await page.keyboard.press("Space");
    await expect(first).toBeChecked();
    await page.keyboard.press("ArrowDown");
    await expect(
      page.getByTestId("wb-quiz-comfortable-desk_usb").locator("input"),
    ).toBeChecked();
    await press("wb-lesson-primary");
    await press("wb-lesson-primary");
    await press("wb-lesson-primary");
    await step(page, "quiz_owned");
    const earbuds = page
      .getByTestId("wb-quiz-owned-wireless_earbuds")
      .locator("input");
    await earbuds.focus();
    await page.keyboard.press("Space");
    await expect(earbuds).toBeChecked();
    await press("wb-lesson-primary");
    await step(page, "results");
    await press("wb-lesson-primary");
    await expect(lesson(page)).toHaveCount(0);
  });
});

test.describe("Whisper lesson: entry points and the checklist", () => {
  const completed = (checklist_done: string[]) => ({
    status: "completed",
    current_step: null,
    completed_steps: [],
    checklist_done,
    completed_at_ms: 1,
  });

  test("Help › Essentials opens the lesson", async ({ page, ipc, hub }) => {
    await open(page, ipc, hub);
    await page.getByTestId("wb-nav-help").click();
    await page.getByTestId("wb-menu-item-lesson-whisper").click();
    await step(page, "soft");
    await expect(
      page.locator('[role="dialog"]').filter({ has: lesson(page) }),
    ).toBeFocused();
  });

  test("Try whispering sits between Try a Transform and Open at login, ticks after the practice, then only points at Open at login", async ({
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
          onboarding: completed([
            "try_in_app",
            "add_word",
            "create_snippet",
            "try_transform",
          ]),
        },
        handy: { autostart_enabled: false },
      },
    );
    const rows = page.locator('li[data-testid^="wb-checklist-"]');
    const ids = await rows.evaluateAll((els) =>
      els.map((el) => el.getAttribute("data-testid")),
    );
    expect(ids.indexOf("wb-checklist-try_whisper")).toBe(
      ids.indexOf("wb-checklist-try_transform") + 1,
    );
    expect(ids.indexOf("wb-checklist-launch_at_login")).toBe(
      ids.indexOf("wb-checklist-try_whisper") + 1,
    );
    await expect(page.getByTestId("wb-checklist-try_whisper")).toContainText(
      en.checklist.items.try_whisper.title,
    );
    await page.getByTestId("wb-checklist-run-try_whisper").click();
    await step(page, "soft");
    await primary(page).click();
    await primary(page).click();
    await ipc.waitForListener("fork-practice-take");
    await whisper(ipc, SENTENCE);
    await expect(heading(page)).toHaveText(W.practice.doneTitle);
    // No tick while the lesson is open.
    expect(await ipc.calls("fork_checklist_mark")).toHaveLength(0);
    await page.keyboard.press("Escape");
    await ipc.expectCalled("fork_checklist_mark", { id: "try_whisper" });
    await expect(page.getByTestId("wb-checklist-try_whisper")).toHaveAttribute(
      "data-done",
      "true",
    );
    // Chaining points at Open at login; it never turns it on.
    await expect(
      page.getByTestId("wb-checklist-launch_at_login"),
    ).toHaveAttribute("data-highlight", "true");
    await page.waitForTimeout(1200);
    expect(await ipc.calls("change_autostart_setting")).toHaveLength(0);
    expect(hub.handy.autostart_enabled).toBe(false);
    await expect(lesson(page)).toHaveCount(0);
  });

  test("a skipped practice does not tick Try whispering", async ({
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
          onboarding: completed(["try_in_app"]),
        },
      },
    );
    await expect(page.getByTestId("wb-checklist-try_whisper")).toBeVisible();
    await fromBanner(page);
    await primary(page).click();
    await primary(page).click();
    await skip(page).click();
    await page.keyboard.press("Escape");
    await expect(lesson(page)).toHaveCount(0);
    expect(await ipc.calls("fork_checklist_mark")).toHaveLength(0);
  });
});

test("reduced motion: the scene is still, with no pause button", async ({
  page,
  ipc,
  hub,
}) => {
  await open(page, ipc, hub);
  await fromBanner(page);
  await expect(page.locator(".wb-whisper-stage")).toHaveAttribute(
    "data-beat",
    "landed",
  );
  await expect(page.getByTestId("wb-lesson-scene-pause")).toHaveCount(0);
  const img = page.locator(".wb-whisper-stage");
  await expect(img).toHaveAttribute("role", "img");
  await expect(img).toHaveAttribute("aria-label", /Remind me to send/);
});

test("with motion: the scene plays and pauses", async ({ page, ipc, hub }) => {
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await open(page, ipc, hub);
  await fromBanner(page);
  const pause = page.getByTestId("wb-lesson-scene-pause");
  await expect(pause).toHaveAttribute("aria-pressed", "false");
  await pause.click();
  await expect(pause).toHaveAttribute("aria-pressed", "true");
});

test("RTL: the lesson and the table mirror", async ({ page, ipc, hub }) => {
  await open(page, ipc, hub, {}, { handy: RTL_APP_SETTINGS });
  await expectRtl(page);
  await fromBanner(page);
  const dir = await page.evaluate(
    () =>
      getComputedStyle(document.querySelector('[role="dialog"]')!).direction,
  );
  expect(dir).toBe("rtl");
  await primary(page).click();
  await primary(page).click();
  await skip(page).click();
  for (let i = 0; i < 5; i += 1) await primary(page).click();
  await step(page, "results");
  const table = page.getByTestId("wb-quiz-table");
  expect(await table.evaluate((el) => getComputedStyle(el).direction)).toBe(
    "rtl",
  );
});

for (const theme of ["light", "dark"] as const) {
  test(`axe: scene, how, practice with tips, quiz, results (${theme})`, async ({
    page,
    ipc,
    hub,
  }) => {
    test.setTimeout(180_000);
    await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
    await open(page, ipc, hub, {}, { handy: { theme } });
    await expectNoSeriousAxeFindings(page);
    await fromBanner(page);
    await expectNoSeriousAxeFindings(page);
    await primary(page).click();
    await step(page, "how");
    await expectNoSeriousAxeFindings(page);
    await primary(page).click();
    await step(page, "practice");
    await ipc.waitForListener("fork-practice-take");
    await whisper(ipc, partial("short"));
    await expect(page.getByTestId("wb-whisper-tips")).toBeVisible();
    await expectNoSeriousAxeFindings(page);
    await whisper(ipc, SENTENCE);
    await expect(heading(page)).toHaveText(W.practice.doneTitle);
    await primary(page).click();
    await page.getByTestId("wb-quiz-comfortable-wired_headset").click();
    await expectNoSeriousAxeFindings(page);
    await primary(page).click();
    await page.getByTestId("wb-quiz-workplace-shared").click();
    await expectNoSeriousAxeFindings(page);
    await primary(page).click();
    await primary(page).click();
    await expectNoSeriousAxeFindings(page);
    await primary(page).click();
    await step(page, "results");
    await expectNoSeriousAxeFindings(page);
  });
}
