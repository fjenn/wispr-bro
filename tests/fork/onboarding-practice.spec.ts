// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * S-29b onboarding: the Practice stage. Hands-free practice (text from the
 * backend's practice take; rc9: its text in the first language the first
 * speech model hears; no model yet: a plain wait with bytes, time left and
 * "Checking the download…", Continue still open, the take fills later and
 * the step moves on by itself at 90 s), the dark celebration (confetti,
 * none with reduced motion; rc9: the typing test is offered here and its
 * result updates the comparison), time saved (rc9: the slider always
 * shows, 25 hours a week to start, the number follows at once), the typing
 * test (wpm math, seconds left, language picker, Try again, Recalculate)
 * and the summary (launch at login, completion). No screen names a model;
 * axe in light and dark, RTL and keyboard-only use.
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
/** A speech model is ready (no wait on the practice screen). */
const READY = { phase: "ready", percent: null, total_bytes: 0 };

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
    await practice(page, ipc, { setup: READY });
    await ipc.expectCalled("fork_onboarding_practice", { on: true });
    await expect(page.getByTestId("wb-practice-model-wait")).toHaveCount(0);
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
    await expect(page.getByTestId("wb-first-dictation-times")).toHaveAttribute(
      "data-times",
      "4.4",
    );
    // rc9: the typing test is offered here; no result yet, so "Take".
    await expect(page.getByTestId("wb-first-dictation-take-test")).toHaveText(
      "Take the typing test",
    );
    await page.getByTestId("wb-first-dictation-continue").click();

    await expectStep(page, "time_saved");
    // Time saved stays on the dark stage; the estimate shows at once from
    // 25 hours a week (alpha.3) at the 45 wpm average:
    // 25 × (1 − 45/200) = 19.375 → 19.5.
    await expect(stage(page)).toHaveAttribute("data-tone", "dark");
    const headline = page.getByTestId("wb-time-saved-headline");
    await expect(headline).toHaveAttribute("data-hours", "19.5");
    await expect(headlineText(page)).toHaveText(
      "Dictating could save you about 19.5 hours a week",
    );
    await expect(page.getByTestId("wb-time-saved-hours-value")).toHaveText(
      "25 hours a week",
    );
    await expect(page.getByTestId("wb-time-saved-take-test")).toHaveCount(0);
    await expect(page.getByTestId("wb-time-saved-skip")).toHaveCount(0);
  });

  test("rc10: the practice's speed is speaking; typing stays 45 until the test", async ({
    page,
    ipc,
  }) => {
    // The owner's report: after dictating the email the next step read as
    // "how fast you type". The take is spoken: 16 words in 6 s = 160 wpm.
    const be = await practice(page, ipc, { setup: READY }, true);
    await take(ipc, "started");
    await take(ipc, "locked");
    await page.clock.runFor(6_000);
    await take(ipc, "stopped");
    await ipc.emit("fork-practice-take", {
      kind: "text",
      text: "Hi Sam, thanks for the notes. Can we meet on Thursday to go over the plan?",
      late: false,
    });
    const next = page.getByTestId("wb-practice-continue");
    await expect(next).toBeEnabled();
    await next.click();
    await expectStep(page, "first_dictation");
    await page.clock.runFor(1_000);
    const speaking = page.getByTestId("wb-first-dictation-speaking");
    await expect(speaking).toContainText("You spoke at160 words a minute");
    await expect(speaking).toHaveAttribute("data-wpm", "160");
    // Typing is not measured by a take: the 45 wpm average, labelled so.
    await expect(page.getByTestId("wb-first-dictation-typing")).toContainText(
      "Typing, on average45 words a minute",
    );
    await expect(page.getByTestId("wb-first-dictation-times")).toHaveAttribute(
      "data-times",
      "3.6",
    );
    await expect(page.getByTestId("wb-first-dictation-take-test")).toHaveText(
      "Take the typing test",
    );
    expect(be.fork.typing_wpm).toBeNull();
    // The test is optional: Continue goes on with the 45 wpm average.
    await page.getByTestId("wb-first-dictation-continue").click();
    await expectStep(page, "time_saved");
    await page.clock.runFor(1_000);
    expect(be.fork.typing_wpm).toBeNull();
    await expect(page.getByTestId("wb-time-saved-math")).toHaveText(
      "25 h of typing at 45 wpm ≈ 5.6 h by voice at 200 wpm",
    );
  });

  test("rc10: a result from an earlier run says Retake and drives the maths", async ({
    page,
    ipc,
  }) => {
    // Help › Show the intro again keeps settings: a real earlier result
    // (the owner's 33 wpm from rc9's test) is the typing speed.
    await practice(page, ipc, {
      step: "first_dictation",
      fork: { typing_wpm: 33, typing_hours_per_day: 7 },
    });
    await expect(page.getByTestId("wb-first-dictation-typing")).toContainText(
      "Your typing33 words a minute",
    );
    await expect(page.getByTestId("wb-first-dictation-speaking")).toContainText(
      "Speaking, on average200 words a minute",
    );
    await expect(page.getByTestId("wb-first-dictation-take-test")).toHaveText(
      "Retake the typing test",
    );
    await page.getByTestId("wb-first-dictation-continue").click();
    await expectStep(page, "time_saved");
    // 35 h × 33/200 = 5.8 h by voice; 35 − 5.8 = 29.2 → 29 saved.
    await expect(page.getByTestId("wb-time-saved-headline")).toHaveAttribute(
      "data-hours",
      "29",
    );
    await expect(page.getByTestId("wb-time-saved-math")).toHaveText(
      "35 h of typing at 33 wpm ≈ 5.8 h by voice at 200 wpm",
    );
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
    // rc9: back to the celebration, which now compares the measured speed.
    await expectStep(page, "first_dictation");
    // The step change animates on the fake clock.
    await page.clock.runFor(1_000);
    expect(be.fork.typing_wpm).toBe(final);
    await expect(page.getByTestId("wb-first-dictation-typing")).toContainText(
      `Your typing${final} words a minute`,
    );
    await expect(page.getByTestId("wb-first-dictation-times")).toHaveAttribute(
      "data-times",
      String(Math.round((200 / final) * 10) / 10),
    );
    await expect(page.getByTestId("wb-first-dictation-take-test")).toHaveText(
      "Retake the typing test",
    );
    await page.getByTestId("wb-first-dictation-continue").click();
    await expectStep(page, "time_saved");
    await page.clock.runFor(1_000);
    // 25 h a week × (1 − wpm/200), to the half hour.
    const hours = Math.round(25 * Math.max(0, 1 - final / 200) * 2) / 2;
    const headline = page.getByTestId("wb-time-saved-headline");
    await expect(headline).toHaveAttribute("data-hours", String(hours));
    await expect(headlineText(page)).toHaveText(
      `Dictating could save you about ${hours} hours a week`,
    );
    await expect(page.getByTestId("wb-time-saved-basis")).toContainText(
      `your typing speed of ${final} words a minute`,
    );

    // The slider changes the estimate and is saved.
    const slider = page.getByTestId("wb-time-saved-slider");
    await slider.focus();
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowRight");
    await expect(page.getByTestId("wb-time-saved-hours-value")).toHaveText(
      "27 hours a week",
    );
    // Stored per day over a five-day week.
    expect(be.fork.typing_hours_per_day).toBe(5.4);
    const more = Math.round(27 * Math.max(0, 1 - final / 200) * 2) / 2;
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

  test("no model yet: a plain wait with progress; the take records and fills later", async ({
    page,
    ipc,
  }) => {
    await practice(page, ipc);
    // The fixture's model is still downloading: the wait is shown at once,
    // with bytes, and never blocks the rest of the onboarding.
    const wait = page.getByTestId("wb-practice-model-wait");
    await expect(wait).toBeVisible();
    await expect(wait).toContainText("Getting your speech model ready");
    await expect(wait).toContainText("This happens only once");
    await expect(page.getByTestId("wb-practice-model-bytes")).toHaveText(
      "2 of 90 MB ·",
    );
    await expect(
      page.getByTestId("wb-practice-model-progress"),
    ).toHaveAttribute("aria-valuenow", "2");
    await expect(page.getByTestId("wb-practice-model-eta")).toHaveCount(0);
    await expect(page.getByTestId("wb-practice-continue")).toBeEnabled();
    await take(ipc, "started");
    await take(ipc, "stopped");
    await ipc.emit("fork-practice-take", {
      kind: "queued",
      text: null,
      late: false,
    });
    const status = page.getByTestId("wb-practice-status");
    await expect(status).toHaveAttribute("data-phase", "queued");
    await expect(status).toHaveText("Saved. Waiting for the speech model…");
    await expect(page.getByTestId("wb-practice-body")).toHaveAttribute(
      "placeholder",
      "Your words will appear here as soon as the speech model is ready.",
    );
    await expect(page.getByTestId("wb-practice-queued-note")).toContainText(
      "History",
    );
    const setup = {
      problem: null,
      needed_mb: null,
      queued_takes: 1,
      starter_languages: ["en"],
    };
    await ipc.emit("fork-speech-setup", {
      ...setup,
      phase: "downloading",
      percent: 41,
      downloaded_bytes: 303_000_000,
      total_bytes: 739_000_000,
      eta_seconds: 150,
    });
    await expect(page.getByTestId("wb-practice-model-eta")).toHaveText(
      "about 3 minutes left",
    );
    await expect(page.getByTestId("wb-practice-model-bytes")).toHaveText(
      "303 of 739 MB ·",
    );
    await ipc.emit("fork-speech-setup", {
      ...setup,
      phase: "downloading",
      percent: 97,
      downloaded_bytes: 717_000_000,
      total_bytes: 739_000_000,
      eta_seconds: 10,
    });
    await expect(page.getByTestId("wb-practice-model-eta")).toHaveText(
      "less than a minute left",
    );
    // The checksum: said plainly, never a stuck 99 %.
    await ipc.emit("fork-speech-setup", {
      ...setup,
      phase: "verifying",
      percent: 99,
      downloaded_bytes: 739_000_000,
      total_bytes: 739_000_000,
      eta_seconds: null,
    });
    await expect(wait).toContainText("Checking the download…");
    await expect(page.getByTestId("wb-practice-model-progress")).toHaveCount(0);
    await expect(wait).not.toContainText("99");
    await ipc.emit("fork-speech-setup", {
      ...setup,
      phase: "ready",
      percent: null,
      downloaded_bytes: 0,
      total_bytes: 0,
      eta_seconds: null,
    });
    await expect(wait).toHaveCount(0);
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

  test("right-to-left: the wait mirrors and stays reachable by keyboard", async ({
    page,
    ipc,
  }) => {
    await practice(page, ipc, {
      handy: { app_language: "ar" },
      setup: { eta_seconds: 300 },
    });
    await expect(flow(page)).toHaveAttribute("dir", "rtl");
    const wait = page.getByTestId("wb-practice-model-wait");
    await expect(wait).toBeVisible();
    await expect(wait).toHaveCSS("text-align", "start");
    const next = page.getByTestId("wb-practice-continue");
    for (
      let i = 0;
      i < 40 && !(await next.evaluate((e) => e === document.activeElement));
      i++
    ) {
      await page.keyboard.press("Tab");
    }
    await expect(next).toBeFocused();
    await page.keyboard.press("Enter");
    await expectStep(page, "first_dictation");
  });

  test("a failed download offers Retry on the practice screen", async ({
    page,
    ipc,
  }) => {
    await practice(page, ipc, {
      setup: { phase: "failed", percent: null, problem: "server_error" },
    });
    await expect(page.getByTestId("wb-practice-model-wait")).toContainText(
      "Couldn't download the speech model",
    );
    await page.getByTestId("wb-practice-model-retry").click();
    await ipc.expectCalled("fork_speech_setup_retry");
  });

  test("the practice email follows the user's languages", async ({
    page,
    ipc,
  }) => {
    // Dutch alone: the practice is in Dutch (the app stays English).
    await practice(page, ipc, {
      handy: { spoken_languages: ["nl"] },
      setup: { starter_languages: ["nl"] },
    });
    const subject = page
      .getByTestId("wb-practice-composer")
      .locator("dd")
      .nth(1);
    await expect(subject).toHaveText("Donderdag");
    await expect(subject).toHaveAttribute("lang", "nl");
    await expect(page.getByTestId("wb-practice-body")).toHaveAttribute(
      "placeholder",
      /Try saying: “Hoi Sam/,
    );
  });

  test("English + Dutch: the practice runs in the starter's English", async ({
    page,
    ipc,
  }) => {
    await practice(page, ipc, {
      handy: { spoken_languages: ["nl", "en"] },
      setup: { ...READY, starter_languages: ["en"] },
    });
    await expect(page.getByTestId("wb-practice-body")).toHaveAttribute(
      "placeholder",
      "Try saying: “Hi Sam, thanks for the notes. Can we meet on Thursday at ten?”",
    );
  });

  test("French + Dutch: the practice runs in French", async ({ page, ipc }) => {
    await practice(page, ipc, {
      handy: { spoken_languages: ["nl", "fr"] },
      setup: { ...READY, starter_languages: ["fr"] },
    });
    await expect(page.getByTestId("wb-practice-body")).toHaveAttribute(
      "placeholder",
      /Bonjour Sam/,
    );
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

  test("skip paths: practice, the typing test, and the estimate", async ({
    page,
    ipc,
  }) => {
    const be = await practice(page, ipc);
    await page.getByTestId("wb-practice-skip").click();
    // Skipping the practice skips the celebration too.
    await expectStep(page, "time_saved");
    // The 45 wpm average at 25 h a week, at once:
    // 25 × (1 − 45/200) → 19.5.
    const headline = page.getByTestId("wb-time-saved-headline");
    await expect(headline).toHaveAttribute("data-hours", "19.5");
    await expect(headlineText(page)).toHaveText(
      "Dictating could save you about 19.5 hours a week",
    );
    await expect(page.getByTestId("wb-time-saved-basis")).toContainText(
      "average typing speed of 45",
    );
    await page.getByTestId("wb-time-saved-continue").click();
    await expectStep(page, "summary");
    await expect(page.getByTestId("wb-summary-typing")).toContainText(
      "Not measured",
    );
    expect(be.fork.typing_wpm).toBeNull();
  });

  test("the typing test's ✕ goes back to the celebration", async ({
    page,
    ipc,
  }) => {
    const be = await openOnboarding(page, ipc, { step: "first_dictation" });
    await page.getByTestId("wb-first-dictation-take-test").click();
    await expectStep(page, "typing_test");
    await expect(stage(page)).toHaveAttribute("data-tone", "dark");
    await page.getByTestId("wb-typing-input").fill(ENGLISH);
    await expect(page.getByTestId("wb-typing-again")).toBeEnabled();
    await page.getByTestId("wb-typing-skip").click();
    // Nothing is saved without Recalculate.
    expect(be.fork.typing_wpm).toBeNull();
    await expectStep(page, "first_dictation");
    await expect(page.getByTestId("wb-first-dictation-take-test")).toHaveText(
      "Take the typing test",
    );
    await page.getByTestId("wb-onboarding-back").click();
    await expectStep(page, "practice");
  });

  test("the estimate counts up to its number at once", async ({
    page,
    ipc,
  }) => {
    await page.emulateMedia({ reducedMotion: "no-preference" });
    await openOnboarding(page, ipc, { step: "time_saved" });
    const headline = page.getByTestId("wb-time-saved-headline");
    await expect(headline).toHaveAttribute("data-hours", "19.5");
    await expect(headlineText(page)).toHaveText(
      "Dictating could save you about 19.5 hours a week",
    );
    await expect(page.getByTestId("wb-time-saved-slider")).toBeVisible();
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
    // No model *name* (B-207). rc9: the wait says "speech model" plainly
    // (owner's call: waiting must be obvious), never which one.
    const forbidden = /parakeet|whisper|moonshine|sensevoice|canary|turbo/i;
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
      "Waiting for the speech model",
    );
    await check();
    await ipc.emit("fork-practice-take", {
      kind: "text",
      text: "Hi.",
      late: true,
    });
    await page.getByTestId("wb-practice-continue").click();
    await check();
    await page.getByTestId("wb-first-dictation-take-test").click();
    await check();
    await page.getByTestId("wb-typing-skip").click();
    await page.getByTestId("wb-first-dictation-continue").click();
    await check();
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
        if (step === "typing_test") {
          await page.getByTestId("wb-typing-input").fill("Every mroning");
        }
        await expect(page.locator("main h1")).toBeVisible();
        await expectNoSeriousAxeFindings(page);
      });
    }
  }
});
