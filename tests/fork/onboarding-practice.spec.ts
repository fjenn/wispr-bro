// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * S-29b onboarding: the Practice stage. Hands-free practice (text from the
 * backend's practice take, a slow starter download: records, shows
 * progress, fills later, moves on by itself at 90 s), the dark celebration
 * (confetti, none with reduced motion), time saved (counter at 0, then the
 * typing test's number, the slider, the skip paths), the typing test (wpm
 * math, seconds left, language picker, Try again, Recalculate) and the
 * summary (launch at login, completion). No screen names a model; axe in
 * light and dark, RTL and keyboard-only use.
 */
import type { Page } from "@playwright/test";
import { test, expect, type IpcMock } from "./fixtures";
import { expectNoSeriousAxeFindings } from "./hub-fixture";
import {
  expectStep,
  flow,
  openOnboarding,
  type Options,
} from "./onboarding-fixture";

const DUTCH = "Elke ochtend gaat de kleine bakkerij";
const ENGLISH = "Every morning the small bakery on the corner";

async function practice(
  page: Page,
  ipc: IpcMock,
  options: Options = {},
  clock = false,
) {
  if (clock) await page.clock.install();
  const be = await openOnboarding(page, ipc, { step: "practice", ...options });
  if ((options.step ?? "practice") === "practice") {
    await ipc.waitForListener("fork-practice-take");
    await ipc.waitForListener("recording-state-event");
  }
  return be;
}

const stage = (page: Page) => page.getByTestId("wb-onboarding-stage");
const headlineText = (page: Page) =>
  page.getByTestId("wb-time-saved-headline-text");

const take = (ipc: IpcMock, phase: string) =>
  ipc.emit("recording-state-event", { phase });

test.describe("S-29b practice", () => {
  test.beforeEach(async ({ page }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
  });

  test("a hands-free take fills the composer, then the celebration and the rest", async ({
    page,
    ipc,
  }) => {
    const be = await practice(page, ipc);
    await ipc.expectCalled("fork_onboarding_practice", { on: true });
    await expect(page.getByTestId("wb-practice-instruction")).toContainText(
      "fn and Space",
    );
    const status = page.getByTestId("wb-practice-status");
    const next = page.getByTestId("wb-practice-continue");
    await expect(next).toBeDisabled();

    await take(ipc, "started");
    await take(ipc, "locked");
    await expect(status).toHaveAttribute("data-phase", "listening");
    await expect(status).toContainText("Listening");
    await take(ipc, "stopped");
    await expect(status).toHaveAttribute("data-phase", "working");
    await ipc.emit("fork-practice-take", {
      kind: "text",
      text: "Hi Sam, thanks for the notes.",
      late: false,
    });
    await expect(page.getByTestId("wb-practice-body")).toHaveValue(
      "Hi Sam, thanks for the notes.",
    );
    // A second take adds to the email.
    await ipc.emit("fork-practice-take", {
      kind: "text",
      text: "Can we meet on Thursday?",
      late: false,
    });
    await expect(page.getByTestId("wb-practice-body")).toHaveValue(
      "Hi Sam, thanks for the notes. Can we meet on Thursday?",
    );
    await expect(next).toBeEnabled();
    await next.click();

    await expectStep(page, "first_dictation");
    await ipc.expectCalled("fork_onboarding_practice", { on: false });
    await expect(stage(page)).toHaveAttribute("data-tone", "dark");
    await expect(
      page.locator(".wb-stage-dark [data-testid=wb-onboarding-stage]"),
    ).toBeVisible();
    await expect(page.locator("main h1")).toHaveText("That's it!");
    // Both speeds are labelled as averages (no measured voice speed).
    await expect(page.getByTestId("wb-first-dictation-typing")).toContainText(
      "Typing, on average45 words a minute",
    );
    await expect(page.getByTestId("wb-first-dictation-speaking")).toContainText(
      "200 words a minute",
    );
    // Reduced motion: no confetti at all.
    await expect(
      page.getByTestId("wb-first-dictation-confetti"),
    ).not.toHaveAttribute("data-fired", "true");
    await page.getByTestId("wb-first-dictation-continue").click();

    await expectStep(page, "time_saved");
    // Time saved and the typing test stay on the dark stage.
    await expect(stage(page)).toHaveAttribute("data-tone", "dark");
    const headline = page.getByTestId("wb-time-saved-headline");
    await expect(headline).toHaveAttribute("data-hours", "0");
    await expect(headlineText(page)).toHaveText(
      "Dictating could save you about 0 hours a week",
    );
    await page.getByTestId("wb-time-saved-test").click();
    await expectStep(page, "typing_test");
    await page.getByTestId("wb-typing-input").fill(ENGLISH);
    await expect(page.getByTestId("wb-typing-again")).toBeEnabled();
    // Nothing is saved before Recalculate (the clock-driven test below).
    expect(be.fork.typing_wpm).toBeNull();
  });

  test("wpm math, seconds left, Recalculate and the counter update", async ({
    page,
    ipc,
  }) => {
    const be = await practice(page, ipc, { step: "typing_test" }, true);
    const input = page.getByTestId("wb-typing-input");
    const seconds = page.getByTestId("wb-typing-seconds");
    const wpm = page.getByTestId("wb-typing-wpm");
    await expect(seconds).toHaveAttribute("data-seconds", "30");
    await expect(wpm).toContainText("—");
    await expect(page.getByTestId("wb-typing-recalculate")).toBeDisabled();

    // 100 correct characters = 20 words, then 10 wrong ones that do not
    // count. The clock starts at the first key.
    const sample = await page.getByTestId("wb-typing-sample").innerText();
    expect(sample.startsWith(ENGLISH)).toBe(true);
    const correct = 100;
    const text = sample.slice(0, correct) + "#".repeat(10);
    await input.fill(text);
    await page.clock.runFor(15_000);
    // 15 s in: words / 0.25 min.
    await expect(wpm).toHaveAttribute(
      "data-wpm",
      String(Math.round(correct / 5 / 0.25)),
    );
    await expect(seconds).toHaveAttribute("data-seconds", "15");
    await page.clock.runFor(16_000);
    const final = Math.round(correct / 5 / 0.5);
    await expect(page.getByTestId("wb-typing-result")).toHaveText(
      `You type about ${final} words a minute.`,
    );
    await expect(seconds).toHaveAttribute("data-seconds", "0");
    // The field is closed once the time is up.
    await expect(input).toHaveAttribute("readonly", "");

    // Try again starts over.
    await page.getByTestId("wb-typing-again").click();
    await expect(seconds).toHaveAttribute("data-seconds", "30");
    await expect(input).toHaveValue("");
    await input.fill(text);
    await page.clock.runFor(31_000);
    await page.getByTestId("wb-typing-recalculate").click();
    await expectStep(page, "time_saved");
    // The step change animates on the fake clock.
    await page.clock.runFor(1_000);
    expect(be.fork.typing_wpm).toBe(final);
    // 2 h a day × 5 days × (1 − wpm/200), to the half hour.
    const hours = Math.round(2 * 5 * Math.max(0, 1 - final / 200) * 2) / 2;
    const headline = page.getByTestId("wb-time-saved-headline");
    await expect(headline).toHaveAttribute("data-hours", String(hours));
    await expect(headlineText(page)).toHaveText(
      `Dictating could save you about ${hours} hours a week`,
    );
    await expect(page.getByTestId("wb-time-saved-basis")).toContainText(
      `your typing speed of ${final} words a minute`,
    );
    // Measured: the link offers to retake the test.
    await expect(page.getByTestId("wb-time-saved-take-test")).toHaveText(
      "Retake the typing test",
    );

    // The slider changes the estimate and is saved.
    const slider = page.getByTestId("wb-time-saved-slider");
    await slider.focus();
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowRight");
    await expect(page.getByTestId("wb-time-saved-hours-value")).toHaveText(
      "3 hours a day",
    );
    expect(be.fork.typing_hours_per_day).toBe(3);
    const more = Math.round(3 * 5 * Math.max(0, 1 - final / 200) * 2) / 2;
    await expect(headline).toHaveAttribute("data-hours", String(more));
  });

  test("the test ends early when the whole paragraph is typed", async ({
    page,
    ipc,
  }) => {
    await practice(page, ipc, { step: "typing_test" }, true);
    const sample = await page.getByTestId("wb-typing-sample").innerText();
    await page.getByTestId("wb-typing-input").fill("E");
    await page.clock.runFor(10_000);
    await page.getByTestId("wb-typing-input").fill(sample);
    await expect(page.getByTestId("wb-typing-result")).toContainText(
      "You type about",
    );
    await expect(page.getByTestId("wb-typing-recalculate")).toBeEnabled();
  });

  test("the typing test speaks the user's language and pastes nothing", async ({
    page,
    ipc,
  }) => {
    await practice(page, ipc, {
      step: "typing_test",
      handy: { spoken_languages: ["nl", "en"] },
    });
    const sample = page.getByTestId("wb-typing-sample");
    await expect(sample).toContainText(DUTCH);
    const pick = async (name: string) => {
      await page.getByTestId("wb-typing-language").click();
      await page.getByRole("option", { name }).click();
    };
    await pick("French");
    await expect(sample).toContainText("Chaque matin");
    await pick("English");
    await expect(sample).toContainText(ENGLISH);
    // A paste does not count as typing.
    await page.getByTestId("wb-typing-input").focus();
    await page.evaluate(() => {
      const el = document.querySelector(
        "[data-testid=wb-typing-input]",
      ) as HTMLTextAreaElement;
      const data = new DataTransfer();
      data.setData("text/plain", "pasted");
      el.dispatchEvent(
        new ClipboardEvent("paste", {
          clipboardData: data,
          bubbles: true,
          cancelable: true,
        }),
      );
    });
    await expect(page.getByTestId("wb-typing-input")).toHaveValue("");
    // Owner report (rc8): typing straight after a change, without clicking
    // the text box, types the new paragraph; the picker's type-ahead never
    // gets the keys and the language stays.
    await pick("French");
    const typingInput = page.getByTestId("wb-typing-input");
    await expect(typingInput).toBeFocused();
    await page.keyboard.type("Chaque");
    await expect(typingInput).toHaveValue("Chaque");
    await expect(sample).toContainText("Chaque matin");
    await expect(page.getByTestId("wb-typing-language")).toContainText(
      "French",
    );
  });

  test("a slow starter: the take records, shows progress, fills later", async ({
    page,
    ipc,
  }) => {
    await practice(page, ipc);
    // Nothing asks for the download until a take waits for it.
    expect(await ipc.calls("fork_speech_setup_status")).toHaveLength(0);
    await take(ipc, "started");
    await take(ipc, "stopped");
    await ipc.emit("fork-practice-take", {
      kind: "queued",
      text: null,
      late: false,
    });
    const status = page.getByTestId("wb-practice-status");
    await expect(status).toHaveAttribute("data-phase", "queued");
    await expect(status).toHaveText("Setting up… 2%");
    await expect(page.getByTestId("wb-practice-queued-note")).toContainText(
      "History",
    );
    await ipc.emit("fork-speech-setup", {
      phase: "downloading",
      percent: 41,
      downloaded_bytes: 41,
      total_bytes: 100,
      problem: null,
      needed_mb: null,
      queued_takes: 1,
    });
    await expect(status).toHaveText("Setting up… 41%");
    await ipc.emit("fork-practice-take", {
      kind: "text",
      text: "Hello from the queue.",
      late: true,
    });
    await expect(page.getByTestId("wb-practice-body")).toHaveValue(
      "Hello from the queue.",
    );
    await expect(page.getByTestId("wb-practice-continue")).toBeEnabled();
  });

  test("a queued take moves the step on by itself after 90 seconds", async ({
    page,
    ipc,
  }) => {
    await practice(page, ipc, {}, true);
    await take(ipc, "started");
    await take(ipc, "stopped");
    await ipc.emit("fork-practice-take", {
      kind: "queued",
      text: null,
      late: false,
    });
    await expect(page.getByTestId("wb-practice-status")).toHaveAttribute(
      "data-phase",
      "queued",
    );
    await page.clock.runFor(89_000);
    await expectStep(page, "practice");
    await page.clock.runFor(2_000);
    await expectStep(page, "first_dictation");
    await ipc.expectCalled("fork_onboarding_go_to", {
      step: "first_dictation",
    });
  });

  test("no text after a take says so; a cancelled take resets", async ({
    page,
    ipc,
  }) => {
    await practice(page, ipc, {}, true);
    const status = page.getByTestId("wb-practice-status");
    await take(ipc, "started");
    await take(ipc, "cancelled");
    await expect(status).toHaveAttribute("data-phase", "idle");
    await take(ipc, "started");
    await take(ipc, "stopped");
    await page.clock.runFor(21_000);
    await expect(status).toHaveAttribute("data-phase", "nothing");
    await expect(status).toContainText("Nothing came through");
  });

  test("the Control + Option profile gets its own instructions", async ({
    page,
    ipc,
  }) => {
    await practice(page, ipc, { status: { trigger: "ctrl+option" } });
    await expect(page.getByTestId("wb-practice-instruction")).toContainText(
      "Control, Option and Space",
    );
    await take(ipc, "started");
    await expect(page.getByTestId("wb-practice-status")).toContainText(
      "press Control and Option",
    );
  });

  test("skip paths: practice, the typing test, and the estimate's link", async ({
    page,
    ipc,
  }) => {
    const be = await practice(page, ipc);
    await page.getByTestId("wb-practice-skip").click();
    // Skipping the practice skips the celebration too.
    await expectStep(page, "time_saved");
    const headline = page.getByTestId("wb-time-saved-headline");
    await expect(headline).toHaveAttribute("data-hours", "0");
    await page.getByTestId("wb-time-saved-skip").click();
    // The 45 wpm average: 2 × 5 × (1 − 45/200) = 7.75 → 8.
    await expect(headline).toHaveAttribute("data-hours", "8");
    await expect(headlineText(page)).toHaveText(
      "Dictating could save you about 8 hours a week",
    );
    await expect(page.getByTestId("wb-time-saved-basis")).toContainText(
      "average typing speed of 45",
    );
    // Owner report (rc8): no result yet, so "Take", never "Retake".
    await expect(page.getByTestId("wb-time-saved-take-test")).toHaveText(
      "Take the typing test",
    );
    await page.getByTestId("wb-time-saved-take-test").click();
    await expectStep(page, "typing_test");
    await page.getByTestId("wb-typing-skip").click();
    await expectStep(page, "time_saved");
    expect(be.fork.typing_wpm).toBeNull();
    await page.getByTestId("wb-time-saved-skip").click();
    await page.getByTestId("wb-time-saved-continue").click();
    await expectStep(page, "summary");
    await expect(page.getByTestId("wb-summary-typing")).toContainText(
      "Not measured",
    );
  });

  test("the counter starts at 0 and counts up", async ({ page, ipc }) => {
    await page.emulateMedia({ reducedMotion: "no-preference" });
    await openOnboarding(page, ipc, { step: "time_saved" });
    const headline = page.getByTestId("wb-time-saved-headline");
    await expect(headline).toHaveAttribute("data-hours", "0");
    await expect(headlineText(page)).toHaveText(
      "Dictating could save you about 0 hours a week",
    );
    await page.getByTestId("wb-time-saved-skip").click();
    await expect(headline).toHaveAttribute("data-hours", "8");
    await expect(headlineText(page)).toHaveText(
      "Dictating could save you about 8 hours a week",
    );
  });

  test("confetti bursts without reduced motion", async ({ page, ipc }) => {
    await page.emulateMedia({ reducedMotion: "no-preference" });
    await openOnboarding(page, ipc, { step: "first_dictation" });
    await expect(
      page.getByTestId("wb-first-dictation-confetti"),
    ).toHaveAttribute("data-fired", "true");
  });

  test("the summary lists the choices, sets launch at login and finishes", async ({
    page,
    ipc,
  }) => {
    const be = await openOnboarding(page, ipc, {
      step: "summary",
      handy: { spoken_languages: ["nl", "fr"] },
    });
    be.fork.typing_wpm = 52;
    await expect(page.getByTestId("wb-summary-languages")).toContainText(
      "Dutch",
    );
    await expect(page.getByTestId("wb-summary-languages")).toContainText(
      "French",
    );
    await expect(page.getByTestId("wb-summary-shortcut")).toContainText(
      "Hold fn",
    );
    await expect(page.getByTestId("wb-summary-microphone")).toContainText(
      "MacBook Pro Microphone",
    );
    const launch = page.getByTestId("wb-summary-launch");
    await expect(launch).toBeChecked();
    await launch.uncheck();
    await page.getByTestId("wb-summary-finish").click();
    await ipc.expectCalled("change_autostart_setting", { enabled: false });
    await ipc.expectCalled("fork_onboarding_complete");
    await ipc.expectCalled("complete_onboarding");
    await expect(page.getByTestId("wb-onboarding")).toHaveCount(0);
    expect(be.fork.onboarding.status).toBe("completed");
  });

  test("Back walks the Practice stage", async ({ page, ipc }) => {
    await openOnboarding(page, ipc, { step: "summary" });
    await page.getByTestId("wb-onboarding-back").click();
    await expectStep(page, "time_saved");
    await page.getByTestId("wb-onboarding-back").click();
    await expectStep(page, "practice");
    await page.getByTestId("wb-onboarding-back").click();
    await expectStep(page, "shortcut_test");
  });

  test("no Practice screen names a speech model", async ({ page, ipc }) => {
    const forbidden = /parakeet|whisper|moonshine|sensevoice|\bmodel\b/i;
    await practice(page, ipc);
    const check = async () =>
      expect(await flow(page).innerText()).not.toMatch(forbidden);
    await check();
    await take(ipc, "started");
    await take(ipc, "stopped");
    await ipc.emit("fork-practice-take", {
      kind: "queued",
      text: null,
      late: false,
    });
    await expect(page.getByTestId("wb-practice-status")).toContainText(
      "Setting up",
    );
    await check();
    await ipc.emit("fork-practice-take", {
      kind: "text",
      text: "Hi.",
      late: true,
    });
    await page.getByTestId("wb-practice-continue").click();
    await check();
    await page.getByTestId("wb-first-dictation-continue").click();
    await check();
    await page.getByTestId("wb-time-saved-test").click();
    await check();
    await page.getByTestId("wb-typing-skip").click();
    await page.getByTestId("wb-time-saved-skip").click();
    await page.getByTestId("wb-time-saved-continue").click();
    await expectStep(page, "summary");
    await check();
  });

  test("keyboard only: from Practice to the end without a pointer", async ({
    page,
    ipc,
  }) => {
    const be = await practice(page, ipc);
    const press = async (testId: string) => {
      const target = page.getByTestId(testId);
      for (
        let i = 0;
        i < 40 && !(await target.evaluate((e) => e === document.activeElement));
        i++
      ) {
        await page.keyboard.press("Tab");
      }
      await expect(target).toBeFocused();
      await page.keyboard.press("Enter");
    };
    await press("wb-practice-skip");
    await expectStep(page, "time_saved");
    await expect(page.locator("main h1")).toBeFocused();
    await press("wb-time-saved-skip");
    await press("wb-time-saved-continue");
    await expectStep(page, "summary");
    await press("wb-summary-finish");
    await ipc.expectCalled("fork_onboarding_complete");
    expect(be.handy.autostart_enabled).toBe(true);
  });

  test("right-to-left: the flow mirrors, the typing sample stays left to right", async ({
    page,
    ipc,
  }) => {
    await openOnboarding(page, ipc, {
      step: "typing_test",
      handy: { app_language: "ar" },
    });
    await expect(flow(page)).toHaveAttribute("dir", "rtl");
    await expect(page.getByTestId("wb-typing-sample")).toHaveAttribute(
      "dir",
      "ltr",
    );
    const skip = await page.getByTestId("wb-typing-skip").boundingBox();
    const box = await flow(page).boundingBox();
    expect(skip && box && skip.x < box.width / 2).toBe(true);
  });

  for (const theme of ["light", "dark"] as const) {
    for (const step of [
      "practice",
      "first_dictation",
      "time_saved",
      "typing_test",
      "summary",
    ] as const) {
      test(`axe: ${step} in ${theme}`, async ({ page, ipc }) => {
        test.setTimeout(180_000);
        await page.emulateMedia({
          colorScheme: theme,
          reducedMotion: "reduce",
        });
        await openOnboarding(page, ipc, { step, theme });
        if (step === "time_saved") {
          await page.getByTestId("wb-time-saved-skip").click();
        }
        if (step === "typing_test") {
          await page.getByTestId("wb-typing-input").fill("Every mroning");
        }
        await expect(page.locator("main h1")).toBeVisible();
        await expectNoSeriousAxeFindings(page);
      });
    }
  }
});
