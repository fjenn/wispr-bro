// SPDX-License-Identifier: GPL-3.0-or-later
// S-25 Insights: both tabs against the hand-computed fixture of the Rust
// tests (fork/stats/insights.rs, voice.rs), the locked and unlocked voice
// profile, the deep link from Home, live refresh, "Save as image" through
// the save panel, no percentile anywhere, keyboard-only use, RTL, and axe in
// light and dark (R5 S-25 tests, §5.5, §11.2). Mocked IPC; see hub-fixture.ts.
import type { Page } from "@playwright/test";
import type { UsageInsights, VoiceProfile } from "../../src/fork/bindings";
import {
  test,
  expect,
  openHub,
  expectNoSeriousAxeFindings,
  type HubState,
} from "./hub-fixture";
import { ipcError, type IpcMock } from "./fixtures";

/** 365 heatmap days ending 2026-10-31 with `words` on the last days. */
function heatmap(tail: number[]): UsageInsights["heatmap"] {
  const end = Date.UTC(2026, 9, 31);
  const band = (w: number) => [1, 250, 500, 750].filter((f) => w >= f).length;
  return Array.from({ length: 365 }, (_, i) => {
    const date = new Date(end - (364 - i) * 86_400_000)
      .toISOString()
      .slice(0, 10);
    const k = i - (365 - tail.length);
    const words = k >= 0 ? tail[k] : 0;
    return { date, words, level: band(words) };
  });
}

/** The Rust test `usage_matches_a_hand_computed_fixture`, as served. */
function usageFixture(over: Partial<UsageInsights> = {}): UsageInsights {
  return {
    total_words: 1_750,
    total_dictations: 23,
    speech_ms: 700_000,
    speed: {
      words_per_minute: 150,
      typing_wpm: 50,
      typing_source: "measured",
      times_typing: 3,
    },
    fixes_dictionary: 7,
    fixes_llm: 31,
    categories: [
      { category: "documents", words: 875, dictations: 9, share: 0.5 },
      { category: "ai_prompts", words: 0, dictations: 0, share: 0 },
      { category: "email", words: 525, dictations: 8, share: 0.3 },
      { category: "work", words: 0, dictations: 0, share: 0 },
      { category: "personal", words: 350, dictations: 6, share: 0.2 },
      { category: "other", words: 0, dictations: 0, share: 0 },
    ],
    heatmap: heatmap([800, 300, 120, 530]),
    heatmap_active_days: 4,
    current_streak_days: 4,
    longest_streak_days: 4,
    first_day: "2026-10-28",
    today: "2026-10-31",
    ...over,
  };
}

function lockedVoice(total = 1_234): VoiceProfile {
  return {
    unlocked: false,
    total_words: total,
    goal: 1_700,
    dictations_read: 0,
    phrase: null,
    peak: null,
    word: null,
    corrected: null,
  };
}

function unlockedVoice(): VoiceProfile {
  return {
    unlocked: true,
    total_words: 2_000,
    goal: 1_700,
    dictations_read: 9,
    phrase: { phrase: "Can we move the proposal", dictations: 4 },
    peak: {
      hour: 15,
      hour_words: 45,
      hour_share: 45 / 77,
      app_name: "Mail",
      app_words: 40,
      app_share: 40 / 77,
    },
    word: { word: "proposal", uses: 8 },
    corrected: { heard: "cooper netties", written: "Kubernetes", times: 1 },
  };
}

interface Backend {
  usage: UsageInsights;
  voice: VoiceProfile;
}

async function openInsights(
  page: Page,
  ipc: IpcMock,
  hub: HubState,
  backend: Backend,
  options: Parameters<typeof openHub>[3] = {},
) {
  ipc.handle("fork_insights_usage", () => backend.usage);
  ipc.handle("fork_insights_voice", () => backend.voice);
  await openHub(page, ipc, hub, { hash: "#/hub/insights", ...options });
  await expect(page.getByTestId("wb-insights")).toHaveAttribute(
    "data-status",
    "ready",
  );
}

const fresh = (): Backend => ({ usage: usageFixture(), voice: lockedVoice() });

test.describe("insights", () => {
  test("usage numbers match the hand-computed fixture", async ({
    page,
    ipc,
    hub,
  }) => {
    await openInsights(page, ipc, hub, fresh());
    await expect(page.getByTestId("wb-page-title")).toHaveText("Insights");
    await expect(page.getByRole("tab", { name: "Usage" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await expect(page.getByTestId("wb-insights-speed-value")).toHaveText(
      "150 wpm",
    );
    await expect(page.getByTestId("wb-insights-speed-compare")).toHaveText(
      "3.0× your typing speed of 50 wpm",
    );
    await expect(page.getByTestId("wb-insights-words-value")).toHaveText(
      "1,750",
    );
    await expect(page.getByTestId("wb-insights-words")).toContainText(
      "in 23 dictations",
    );
    await expect(page.getByTestId("wb-insights-fixes-dictionary")).toHaveText(
      "7",
    );
    await expect(page.getByTestId("wb-insights-fixes-ai")).toHaveText("31");
    // The category list screen readers get (the chart is aria-hidden).
    await expect(
      page.getByTestId("wb-insights-categories-list").locator("li"),
    ).toHaveText([
      "Documents: 50%, 875 words",
      "AI prompts: 0%, 0 words",
      "Email: 30%, 525 words",
      "Work messages: 0%, 0 words",
      "Personal messages: 20%, 350 words",
      "Everything else: 0%, 0 words",
    ]);
    // Recharts drew six bars with their share as direct labels.
    const chart = page.getByTestId("wb-insights-categories-chart");
    await expect(chart.locator(".recharts-bar-rectangle")).toHaveCount(6);
    await expect(chart.locator(".recharts-label-list")).toContainText("50%");
    // Streak and heatmap: bands 4, 2, 1, 3 on the last four days.
    await expect(page.getByTestId("wb-insights-streak-current")).toHaveText(
      "4 days",
    );
    await expect(page.getByTestId("wb-insights-streak-longest")).toHaveText(
      "4 days",
    );
    const heat = page.getByTestId("wb-insights-heatmap");
    await expect(heat).toHaveAttribute("role", "img");
    await expect(heat).toHaveAttribute(
      "aria-label",
      "4 days with dictation in the last year. Current streak: 4 days. Longest streak: 4 days.",
    );
    const cells = heat.locator("rect[data-level]");
    await expect(cells).toHaveCount(365);
    const levels = await cells.evaluateAll((els) =>
      els.slice(-4).map((e) => e.getAttribute("data-level")),
    );
    expect(levels).toEqual(["4", "2", "1", "3"]);
    await expect(cells.last()).toContainText("Oct 31, 2026: 530 words");
    await expect(
      page.getByTestId("wb-insights-legend").locator("li"),
    ).toHaveText([
      "No words",
      "1 to 249 words",
      "250 to 499 words",
      "500 to 749 words",
      "750 words or more",
    ]);
  });

  test("without a typing test the average typist is the reference", async ({
    page,
    ipc,
    hub,
  }) => {
    const b = fresh();
    b.usage.speed = {
      words_per_minute: 155,
      typing_wpm: 40,
      typing_source: "average",
      times_typing: 3.9,
    };
    await openInsights(page, ipc, hub, b);
    await expect(page.getByTestId("wb-insights-speed-compare")).toHaveText(
      "3.9× the speed of an average typist (40 wpm)",
    );
  });

  test("an empty profile explains itself", async ({ page, ipc, hub }) => {
    const b = fresh();
    b.usage = usageFixture({
      total_words: 0,
      total_dictations: 0,
      speech_ms: 0,
      speed: {
        words_per_minute: null,
        typing_wpm: 40,
        typing_source: "average",
        times_typing: null,
      },
      fixes_dictionary: 0,
      fixes_llm: 0,
      categories: usageFixture().categories.map((c) => ({
        ...c,
        words: 0,
        dictations: 0,
        share: 0,
      })),
      heatmap: heatmap([]),
      heatmap_active_days: 0,
      current_streak_days: 0,
      longest_streak_days: 0,
      first_day: null,
    });
    b.voice = lockedVoice(0);
    await openInsights(page, ipc, hub, b);
    await expect(page.getByTestId("wb-insights-speed-value")).toHaveText(
      "Dictate a little more to see your speed.",
    );
    await expect(page.getByTestId("wb-insights-categories")).toContainText(
      "Dictate in a few apps to see where your words go.",
    );
    await expect(page.getByTestId("wb-insights-streak-current")).toHaveText(
      "0 days",
    );
  });

  test("no percentile anywhere", async ({ page, ipc, hub }) => {
    await openInsights(page, ipc, hub, fresh());
    await expect(page.getByTestId("wb-insights-usage")).toBeVisible();
    await expect(page.getByTestId("wb-main")).not.toContainText(/percentile/i);
    await page.getByRole("tab", { name: "Voice" }).click();
    await expect(page.getByTestId("wb-main")).not.toContainText(/percentile/i);
  });

  test("voice is locked below 1,700 words with blurred sample cards", async ({
    page,
    ipc,
    hub,
  }) => {
    await openInsights(page, ipc, hub, fresh());
    await page.getByRole("tab", { name: "Voice" }).click();
    await expect(page).toHaveURL(/#\/hub\/insights\/voice$/);
    const tab = page.getByTestId("wb-insights-voice");
    await expect(tab).toHaveAttribute("data-unlocked", "false");
    await expect(page.getByTestId("wb-voice-locked")).toContainText(
      "Your voice profile unlocks at 1,700 words",
    );
    await expect(page.getByTestId("wb-voice-locked")).toContainText(
      "1,234 of 1,700 words so far",
    );
    const bar = page.getByTestId("wb-voice-locked-bar");
    await expect(bar).toHaveAttribute("aria-valuenow", "1234");
    await expect(bar).toHaveAttribute("aria-valuetext", "1,234 of 1,700 words");
    for (const id of ["phrase", "peak", "word", "corrected"]) {
      const card = page.getByTestId(`wb-voice-card-${id}`);
      await expect(card).toContainText("Locked");
      const sample = page.getByTestId(`wb-voice-card-${id}-sample`);
      await expect(sample).toHaveAttribute("aria-hidden", "true");
      expect(
        await sample.evaluate((e) => getComputedStyle(e).filter),
      ).toContain("blur");
    }
    // The sample text is not the user's data.
    await expect(tab).not.toContainText("proposal");
  });

  test("unlocked voice cards show the profile", async ({ page, ipc, hub }) => {
    const b = fresh();
    b.voice = unlockedVoice();
    await openInsights(page, ipc, hub, b, { hash: "#/hub/insights/voice" });
    const tab = page.getByTestId("wb-insights-voice");
    await expect(tab).toHaveAttribute("data-unlocked", "true");
    await expect(page.getByTestId("wb-voice-locked")).toHaveCount(0);
    await expect(page.getByTestId("wb-voice-phrase")).toHaveText(
      "“Can we move the proposal”",
    );
    await expect(page.getByTestId("wb-voice-card-phrase")).toContainText(
      "In 4 dictations",
    );
    await expect(page.getByTestId("wb-voice-peak-hour")).toHaveText(
      /^3\s*–\s*4\sPM$/,
    );
    await expect(page.getByTestId("wb-voice-peak-app")).toHaveText(
      "Most words in Mail",
    );
    await expect(page.getByTestId("wb-voice-word")).toHaveText("proposal");
    await expect(page.getByTestId("wb-voice-card-word")).toContainText(
      "Used 8 times",
    );
    await expect(page.getByTestId("wb-voice-corrected")).toHaveText(
      "cooper netties → Kubernetes",
    );
    await expect(page.getByTestId("wb-voice-card-corrected")).toContainText(
      "Fixed 1 time",
    );
    await expect(page.getByTestId("wb-voice-footnote")).toHaveText(
      "Worked out on this Mac from your latest 9 dictations. Nothing leaves your Mac.",
    );
  });

  test("a card without enough data says so", async ({ page, ipc, hub }) => {
    const b = fresh();
    b.voice = { ...unlockedVoice(), phrase: null, corrected: null };
    await openInsights(page, ipc, hub, b, { hash: "#/hub/insights/voice" });
    await expect(page.getByTestId("wb-voice-card-phrase")).toContainText(
      "Not enough dictations yet.",
    );
    await expect(page.getByTestId("wb-voice-card-corrected")).toContainText(
      "Not enough dictations yet.",
    );
  });

  test("the Home voice-profile button opens the Voice tab", async ({
    page,
    ipc,
    hub,
  }) => {
    const b = fresh();
    b.voice = unlockedVoice();
    hub.stats = { ...hub.stats, total_words: 2_000, total_dictations: 30 };
    ipc.handle("fork_insights_usage", () => b.usage);
    ipc.handle("fork_insights_voice", () => b.voice);
    await openHub(page, ipc, hub);
    await page.getByTestId("wb-voice-profile-open").click();
    await expect(page.getByTestId("wb-main")).toHaveAttribute(
      "data-page",
      "insights",
    );
    await expect(page.getByRole("tab", { name: "Voice" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await expect(page.getByTestId("wb-voice-word")).toHaveText("proposal");
  });

  test("refreshes live and unlocks the profile after a take", async ({
    page,
    ipc,
    hub,
  }) => {
    const b = fresh();
    b.voice = lockedVoice(1_690);
    await openInsights(page, ipc, hub, b, { hash: "#/hub/insights/voice" });
    await expect(page.getByTestId("wb-insights-voice")).toHaveAttribute(
      "data-unlocked",
      "false",
    );
    b.usage = usageFixture({ total_words: 1_760, total_dictations: 24 });
    b.voice = unlockedVoice();
    await ipc.waitForListener("fork-stats-updated");
    await ipc.emit("fork-stats-updated", { day: "2026-10-31" });
    await expect(page.getByTestId("wb-insights-voice")).toHaveAttribute(
      "data-unlocked",
      "true",
    );
    await page.getByRole("tab", { name: "Usage" }).click();
    await expect(page.getByTestId("wb-insights-words-value")).toHaveText(
      "1,760",
    );
  });

  test("a backend that is not ready yet is retried", async ({
    page,
    ipc,
    hub,
  }) => {
    const b = fresh();
    // Two calls fail: React's dev StrictMode mounts the page twice.
    ipc.handle("fork_insights_usage", (_a, n) =>
      n <= 2 ? ipcError({ kind: "NotReady" }) : b.usage,
    );
    ipc.handle("fork_insights_voice", () => b.voice);
    await openHub(page, ipc, hub, { hash: "#/hub/insights" });
    await expect(page.getByTestId("wb-insights-unavailable")).toBeVisible();
    await expect(page.getByTestId("wb-insights-words-value")).toHaveText(
      "1,750",
      { timeout: 5_000 },
    );
  });

  test("save as image renders a PNG and writes it where the user chose", async ({
    page,
    ipc,
    hub,
  }) => {
    await openInsights(page, ipc, hub, fresh());
    await expect(
      page.getByTestId("wb-insights-categories-chart").locator(".recharts-bar"),
    ).toBeVisible();
    ipc.respond("plugin:dialog|save", "/Users/test/Desktop/insights.png");
    ipc.respond("fork_insights_save_image", 123_456);
    await page.getByTestId("wb-insights-save").click();
    await ipc.expectCalled("plugin:dialog|save");
    const [dialog] = await ipc.calls("plugin:dialog|save");
    expect(JSON.stringify(dialog.args)).toContain(
      "Wispr Bro insights 2026-10-31.png",
    );
    await ipc.expectCalled("fork_insights_save_image", {
      path: "/Users/test/Desktop/insights.png",
    });
    const [call] = await ipc.calls("fork_insights_save_image");
    const png = Buffer.from(String(call.args.pngBase64), "base64");
    expect(png.subarray(0, 8)).toEqual(
      Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    );
    // 2× render of the page area: wider than the window.
    const width = png.readUInt32BE(16);
    expect(width).toBeGreaterThan(1000);
    await expect(page.getByText("Saved the image.")).toBeVisible();
  });

  test("cancelling the save panel writes nothing; a failure says so", async ({
    page,
    ipc,
    hub,
  }) => {
    await openInsights(page, ipc, hub, fresh());
    ipc.respond("plugin:dialog|save", null);
    await page.getByTestId("wb-insights-save").click();
    await ipc.expectCalled("plugin:dialog|save");
    expect(await ipc.calls("fork_insights_save_image")).toHaveLength(0);

    ipc.respond("plugin:dialog|save", "/Users/test/insights.png");
    ipc.handle("fork_insights_save_image", () =>
      ipcError({ kind: "Internal", message: "disk full" }),
    );
    await page.getByTestId("wb-insights-save").click();
    await expect(
      page.getByText("The image could not be saved. Try again."),
    ).toBeVisible();
  });

  test("keyboard only: tabs, arrow keys and the save button", async ({
    page,
    ipc,
    hub,
  }) => {
    const b = fresh();
    b.voice = unlockedVoice();
    await openInsights(page, ipc, hub, b);
    const save = page.getByTestId("wb-insights-save");
    await save.focus();
    await expect(save).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(page.getByRole("tab", { name: "Usage" })).toBeFocused();
    await page.keyboard.press("ArrowRight");
    await expect(page.getByRole("tab", { name: "Voice" })).toBeFocused();
    await expect(page.getByRole("tab", { name: "Voice" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await expect(page.getByTestId("wb-voice-word")).toBeVisible();
    // Enter on the save button opens the save panel.
    ipc.respond("plugin:dialog|save", null);
    await save.focus();
    await page.keyboard.press("Enter");
    await ipc.expectCalled("plugin:dialog|save");
  });

  test("RTL: the page mirrors, bars grow from the right, axe passes", async ({
    page,
    ipc,
    hub,
  }) => {
    await openInsights(page, ipc, hub, fresh(), {
      handy: { app_language: "ar" },
    });
    await expect(page.getByTestId("wb-hub")).toHaveAttribute("dir", "rtl");
    // The heatmap's time axis stays left to right.
    await expect(page.getByTestId("wb-insights-heatmap")).toHaveAttribute(
      "dir",
      "ltr",
    );
    const bar = page
      .getByTestId("wb-insights-categories-chart")
      .locator(".recharts-bar-rectangle path")
      .first();
    const box = await bar.boundingBox();
    const chartBox = await page
      .getByTestId("wb-insights-categories-chart")
      .boundingBox();
    expect(box && chartBox).toBeTruthy();
    // The documents bar (50 %) ends nearer the right edge than the left.
    expect(box!.x + box!.width).toBeGreaterThan(
      chartBox!.x + chartBox!.width / 2,
    );
    await expectNoSeriousAxeFindings(page);
  });

  for (const theme of ["light", "dark"] as const) {
    test(`axe: usage and voice in ${theme}`, async ({ page, ipc, hub }) => {
      await openInsights(page, ipc, hub, fresh(), { handy: { theme } });
      await expect(
        page.getByTestId("wb-insights-heatmap").locator("rect").first(),
      ).toBeVisible();
      await expectNoSeriousAxeFindings(page);
      await page.getByRole("tab", { name: "Voice" }).click();
      await expect(page.getByTestId("wb-voice-locked")).toBeVisible();
      await expectNoSeriousAxeFindings(page);
    });

    test(`axe: unlocked voice in ${theme}`, async ({ page, ipc, hub }) => {
      const b = fresh();
      b.voice = unlockedVoice();
      await openInsights(page, ipc, hub, b, {
        handy: { theme },
        hash: "#/hub/insights/voice",
      });
      await expect(page.getByTestId("wb-voice-word")).toBeVisible();
      await expectNoSeriousAxeFindings(page);
    });
  }

  test("narrow window: one column, the heatmap fits without scrolling", async ({
    page,
    ipc,
    hub,
  }) => {
    await openInsights(page, ipc, hub, fresh(), {
      size: { width: 720, height: 600 },
    });
    const heat = page.getByTestId("wb-insights-heatmap");
    await expect(heat.locator("rect").first()).toBeVisible();
    const overflow = await heat.evaluate((el) => {
      const svg = el.querySelector("svg");
      return svg
        ? svg.getBoundingClientRect().width - el.getBoundingClientRect().width
        : 0;
    });
    expect(overflow).toBeLessThanOrEqual(1);
  });
});
