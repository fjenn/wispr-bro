// SPDX-License-Identifier: GPL-3.0-or-later
// S-15a Dictation home: greeting and its name sources, the stats rail
// against a fixture, live refresh on `fork-stats-updated`, the voice profile
// bar, the promo banner, keyboard use, layout and axe in light and dark
// (R5 S-15a tests, §5.5, §11.2). Mocked IPC; see hub-fixture.ts.
import type { Page } from "@playwright/test";
import type { StatsSummary } from "../../src/fork/bindings";
import {
  test,
  expect,
  openHub,
  expectNoSeriousAxeFindings,
} from "./hub-fixture";
import { ipcError, NOT_READY } from "./fixtures";

const title = (page: Page) =>
  page.getByTestId("wb-main").getByRole("heading", { level: 1 });
const statValue = (page: Page, id: "words" | "wpm" | "streak") =>
  page.getByTestId(`wb-stat-${id}-value`);

/** A profile with some history (numbers chosen to need grouping). */
function fixtureStats(over: Partial<StatsSummary> = {}): StatsSummary {
  return {
    total_words: 12_345,
    total_dictations: 321,
    today_words: 210,
    today_dictations: 9,
    speech_ms: 5_400_000,
    words_per_minute: 137.4,
    current_streak_days: 6,
    longest_streak_days: 11,
    active_days: 40,
    fixes_dictionary: 3,
    fixes_llm: 12,
    ...over,
  };
}

test.describe("dictation home", () => {
  test("is the landing page and greets with the macOS first name", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub);
    await expect(page.getByTestId("wb-main")).toHaveAttribute(
      "data-page",
      "dictation",
    );
    await expect(title(page)).toHaveText("Hello, Sam");
    await expect(title(page)).toHaveAttribute("data-name-source", "mac");
    await ipc.expectCalled("fork_account_name");
  });

  test("the greeting override wins over the macOS name", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub, { fork: { greeting_name: "Ana María" } });
    await expect(title(page)).toHaveText("Hello, Ana María");
    await expect(title(page)).toHaveAttribute("data-name-source", "setting");
  });

  test("greeting falls back when there is no name", async ({
    page,
    ipc,
    hub,
  }) => {
    hub.accountName = null;
    await openHub(page, ipc, hub);
    await expect(title(page)).toHaveText("Hello");
    await expect(title(page)).toHaveAttribute("data-name-source", "none");
  });

  test("greeting falls back when the name cannot be read", async ({
    page,
    ipc,
    hub,
  }) => {
    // Two full Hub loads (open, then reload): room for a loaded machine.
    test.setTimeout(180_000);
    await openHub(page, ipc, hub);
    ipc.handle("fork_account_name", () => ipcError("boom"));
    await page.reload();
    await expect(title(page)).toHaveText("Hello");
  });

  test("the name editor saves, resets and works by keyboard", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub);
    const edit = page.getByTestId("wb-greeting-edit");
    await expect(edit).toHaveAccessibleName("Change the name in the greeting");

    // Keyboard: focus the pencil, Enter opens, the field takes focus.
    await edit.focus();
    await page.keyboard.press("Enter");
    const editor = page.getByTestId("wb-greeting-editor");
    await expect(editor).toBeVisible();
    await expect(editor).toHaveAttribute("role", "dialog");
    const input = page.getByTestId("wb-greeting-input");
    await expect(input).toBeFocused();
    await expect(input).toHaveValue("Sam");
    await expect(page.getByLabel("Your name")).toBeVisible();
    // No override yet: nothing to reset.
    await expect(page.getByTestId("wb-greeting-reset")).toHaveCount(0);

    // Esc closes and returns focus to the pencil.
    await page.keyboard.press("Escape");
    await expect(editor).toBeHidden();
    await expect(edit).toBeFocused();

    // An empty name cannot be saved.
    await page.keyboard.press("Enter");
    await input.fill("   ");
    await expect(page.getByTestId("wb-greeting-save")).toBeDisabled();

    // Enter saves a trimmed name.
    await input.fill("  Alex ");
    await page.keyboard.press("Enter");
    await ipc.expectCalled("fork_set_setting", {
      update: { key: "greeting_name", value: "Alex" },
    });
    await expect(editor).toBeHidden();
    await expect(title(page)).toHaveText("Hello, Alex");

    // Back to the Mac's name.
    await edit.click();
    await page.getByTestId("wb-greeting-reset").click();
    await ipc.expectCalled("fork_set_setting", {
      update: { key: "greeting_name", value: null },
    });
    await expect(title(page)).toHaveText("Hello, Sam");
  });

  test("saving the Mac's own name stores no override", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub);
    await page.getByTestId("wb-greeting-edit").click();
    await page.getByTestId("wb-greeting-save").click();
    await expect(page.getByTestId("wb-greeting-editor")).toBeHidden();
    expect(await ipc.calls("fork_set_setting")).toHaveLength(0);
  });

  test("a rejected name keeps the editor open with an error", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub);
    ipc.handle("fork_set_setting", () =>
      ipcError({ kind: "InvalidSetting", message: "greeting_name: bad" }),
    );
    await page.getByTestId("wb-greeting-edit").click();
    await page.getByTestId("wb-greeting-input").fill("Zed");
    await page.getByTestId("wb-greeting-save").click();
    await expect(page.getByRole("alert")).toHaveText(
      "The name could not be saved. Try again.",
    );
    await expect(page.getByTestId("wb-greeting-input")).toHaveAttribute(
      "aria-invalid",
      "true",
    );
    await expect(title(page)).toHaveText("Hello, Sam");
  });

  test("without a Mac name the reset button clears the name", async ({
    page,
    ipc,
    hub,
  }) => {
    hub.accountName = null;
    await openHub(page, ipc, hub, { fork: { greeting_name: "Jo" } });
    await page.getByTestId("wb-greeting-edit").click();
    await expect(page.getByTestId("wb-greeting-reset")).toHaveText(
      "Remove the name",
    );
    await page.getByTestId("wb-greeting-reset").click();
    await expect(title(page)).toHaveText("Hello");
  });

  test("the rail's numbers match the fixture", async ({ page, ipc, hub }) => {
    hub.stats = fixtureStats();
    await openHub(page, ipc, hub);
    const rail = page.getByRole("complementary", { name: "Your stats" });
    await expect(rail).toBeVisible();
    await expect(statValue(page, "words")).toHaveText("12,345");
    await expect(statValue(page, "wpm")).toHaveText("137");
    await expect(statValue(page, "streak")).toHaveText("6");
    // Each number is read with its label (dt/dd).
    await expect(rail.getByRole("term")).toHaveText([
      "Total words",
      "Words per minute",
      "Day streak",
    ]);
    await expect(rail.getByRole("definition")).toHaveText([
      "12,345",
      "137",
      "6",
    ]);
    // The main column holds History (S-15b, history.spec.ts).
    await expect(page.getByTestId("wb-history")).toBeVisible();
  });

  test("numbers follow the app language's grouping", async ({
    page,
    ipc,
    hub,
  }) => {
    hub.stats = fixtureStats({ total_words: 1_234_567 });
    await openHub(page, ipc, hub, { handy: { app_language: "de" } });
    await expect(statValue(page, "words")).toHaveText("1.234.567");
  });

  test("a new profile shows zeros and a dash for words per minute", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub);
    await expect(statValue(page, "words")).toHaveText("0");
    await expect(statValue(page, "streak")).toHaveText("0");
    await expect(statValue(page, "wpm")).toHaveText("–Not enough speech yet");
    await expect(statValue(page, "wpm").locator(".sr-only")).toHaveText(
      "Not enough speech yet",
    );
    await expect(page.getByTestId("wb-empty-dictation")).toContainText(
      "Your dictations will show up here",
    );
  });

  test("the rail refreshes live on fork-stats-updated", async ({
    page,
    ipc,
    hub,
  }) => {
    hub.stats = fixtureStats({
      total_words: 100,
      total_dictations: 4,
      words_per_minute: null,
      current_streak_days: 1,
    });
    await openHub(page, ipc, hub);
    await expect(statValue(page, "words")).toHaveText("100");
    await ipc.waitForListener("fork-stats-updated");

    // Ten takes, one event each (the real-Mac check S-15a.1 does the same).
    for (let take = 1; take <= 10; take++) {
      hub.stats = fixtureStats({
        total_words: 100 + take * 12,
        total_dictations: 4 + take,
        words_per_minute: 120 + take,
        current_streak_days: 2,
      });
      await ipc.emit("fork-stats-updated", { day: "2026-10-30" });
      await expect(statValue(page, "words")).toHaveText(
        String(100 + take * 12),
      );
    }
    await expect(statValue(page, "wpm")).toHaveText("130");
    await expect(statValue(page, "streak")).toHaveText("2");
    await expect(page.getByTestId("wb-voice-profile-bar")).toHaveAttribute(
      "aria-valuenow",
      "220",
    );
  });

  test("a burst of events reads the stats at most twice more", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub);
    await expect(statValue(page, "words")).toHaveText("0");
    await ipc.waitForListener("fork-stats-updated");
    // Only the events count here: with parallel workers another page can
    // take focus and fire the hook's focus/visibility refresh, a third read
    // that is not the burst's (Mac wave 2 fixes run, under load).
    await page.evaluate(() => {
      const stop = (e: Event) => e.stopImmediatePropagation();
      window.addEventListener("focus", stop, true);
      document.addEventListener("visibilitychange", stop, true);
    });
    // Slow replies so the events overlap the read in flight.
    ipc.handle("fork_stats_summary", async () => {
      await new Promise((r) => setTimeout(r, 300));
      return hub.stats;
    });
    ipc.clearCalls();
    hub.stats = fixtureStats({ total_words: 42 });
    for (let i = 0; i < 6; i++) {
      await ipc.emit("fork-stats-updated", { day: "2026-10-30" });
    }
    await expect(statValue(page, "words")).toHaveText("42");
    await page.waitForTimeout(800);
    const reads = await ipc.calls("fork_stats_summary");
    expect(reads.length).toBeGreaterThanOrEqual(1);
    expect(reads.length).toBeLessThanOrEqual(2);
  });

  test("before the backend is ready the rail shows dashes, then numbers", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub);
    ipc.respond("fork_stats_summary", NOT_READY);
    await page.reload();
    await expect(page.getByTestId("wb-stats-rail")).toHaveAttribute(
      "data-status",
      "unavailable",
    );
    await expect(statValue(page, "words")).toHaveText("–Not available yet");
    // Ready now: the hook retries by itself (1, 2, 4 s …), no event needed.
    ipc.handle("fork_stats_summary", () => fixtureStats());
    await expect(statValue(page, "words")).toHaveText("12,345", {
      timeout: 10_000,
    });
  });

  test("voice profile: progress, then unlocked with a link to Insights", async ({
    page,
    ipc,
    hub,
  }) => {
    hub.stats = fixtureStats({ total_words: 850 });
    await openHub(page, ipc, hub);
    const bar = page.getByTestId("wb-voice-profile-bar");
    await expect(bar).toHaveAttribute("role", "progressbar");
    await expect(bar).toHaveAccessibleName("Your voice profile");
    await expect(bar).toHaveAttribute("aria-valuenow", "850");
    await expect(bar).toHaveAttribute("aria-valuemax", "1700");
    await expect(bar).toHaveAttribute("aria-valuetext", "850 of 1,700 words");
    const fill = bar.locator("i");
    const width = await fill.evaluate((el) => (el as HTMLElement).style.width);
    expect(parseFloat(width)).toBe(50);
    await expect(page.getByTestId("wb-voice-profile")).toContainText(
      "Unlocks at 1,700 words.",
    );
    await expect(page.getByTestId("wb-voice-profile-open")).toHaveCount(0);

    await ipc.waitForListener("fork-stats-updated");
    hub.stats = fixtureStats({ total_words: 1_700 });
    await ipc.emit("fork-stats-updated", { day: "2026-10-30" });
    await expect(page.getByTestId("wb-voice-profile")).toHaveAttribute(
      "data-unlocked",
      "true",
    );
    await expect(bar).toHaveAttribute("aria-valuenow", "1700");
    await page.getByTestId("wb-voice-profile-open").click();
    await expect(page.getByTestId("wb-main")).toHaveAttribute(
      "data-page",
      "insights",
    );
    await expect(page).toHaveURL(/#\/hub\/insights\/voice$/);
  });

  test("the voice profile bar never overflows past the goal", async ({
    page,
    ipc,
    hub,
  }) => {
    hub.stats = fixtureStats({ total_words: 9_999 });
    await openHub(page, ipc, hub);
    const bar = page.getByTestId("wb-voice-profile-bar");
    await expect(bar).toHaveAttribute("aria-valuenow", "1700");
    expect(
      await bar.locator("i").evaluate((el) => (el as HTMLElement).style.width),
    ).toBe("100%");
  });

  test("the promo banner dismisses and stays dismissed", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub);
    const banner = page.getByTestId("wb-banner-hub.dictation.start");
    await expect(banner).toBeVisible();
    await banner.getByRole("button", { name: "Dismiss" }).click();
    await expect(banner).toBeHidden();
    await ipc.expectCalled("fork_set_setting", {
      update: { key: "dismissed_banners", value: ["hub.dictation.start"] },
    });
    await page.reload();
    await expect(page.getByTestId("wb-stats-rail")).toBeVisible();
    await expect(banner).toHaveCount(0);
  });

  test("layout: rail on the right when wide, stacked under the greeting when narrow", async ({
    page,
    ipc,
    hub,
  }) => {
    hub.stats = fixtureStats();
    await openHub(page, ipc, hub);
    const rail = page.getByTestId("wb-home-rail");
    const banner = page.getByTestId("wb-banner-hub.dictation.start");
    let r = (await rail.boundingBox())!;
    let b = (await banner.boundingBox())!;
    expect(r.x).toBeGreaterThan(b.x + b.width - 1);
    expect(r.width).toBeCloseTo(250, 0); // W2: the reference's 250 pt rail

    // Minimum window: the page is too narrow for two columns.
    await page.setViewportSize({ width: 720, height: 600 });
    r = (await rail.boundingBox())!;
    b = (await banner.boundingBox())!;
    expect(r.y + r.height).toBeLessThanOrEqual(b.y);
    // Nothing scrolls sideways.
    const overflow = await page
      .getByTestId("wb-main")
      .evaluate((el) => el.scrollWidth - el.clientWidth);
    expect(overflow).toBeLessThanOrEqual(0);
  });

  test("RTL: the rail sits on the left and reads in order", async ({
    page,
    ipc,
    hub,
  }) => {
    hub.stats = fixtureStats();
    await openHub(page, ipc, hub, { handy: { app_language: "ar" } });
    const r = (await page.getByTestId("wb-home-rail").boundingBox())!;
    const b = (await page
      .getByTestId("wb-banner-hub.dictation.start")
      .boundingBox())!;
    expect(r.x + r.width).toBeLessThanOrEqual(b.x + 1);
    await expectNoSeriousAxeFindings(page);
  });

  for (const theme of ["light", "dark"] as const) {
    test(`axe: home, editor and unlocked profile (${theme})`, async ({
      page,
      ipc,
      hub,
    }) => {
      hub.stats = fixtureStats();
      await page.emulateMedia({ colorScheme: theme });
      await openHub(page, ipc, hub, { handy: { theme } });
      await expect(statValue(page, "words")).toHaveText("12,345");
      await expectNoSeriousAxeFindings(page);

      await page.getByTestId("wb-greeting-edit").click();
      await expect(page.getByTestId("wb-greeting-editor")).toBeVisible();
      await expectNoSeriousAxeFindings(page);
      await page.keyboard.press("Escape");

      // Zero state (dash for WPM) and a narrow page.
      hub.stats = fixtureStats({
        total_words: 0,
        total_dictations: 0,
        words_per_minute: null,
      });
      await ipc.emit("fork-stats-updated", { day: "2026-10-30" });
      await page.setViewportSize({ width: 720, height: 600 });
      await expect(statValue(page, "words")).toHaveText("0");
      await expectNoSeriousAxeFindings(page);
    });
  }
});
