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
      speaking_wpm: 150,
      speaking_source: "measured",
      time_saved_ms: 1_400_000,
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
    await expect(page.getByTestId("wb-insights-speed-value")).toHaveText("150");
    await expect(page.getByTestId("wb-insights-speed")).toContainText(
      "Words per minute",
    );
    // Speaking against the measured typing speed, as two bars.
    await expect(page.getByTestId("wb-insights-bar-speaking")).toContainText(
      "Speaking150",
    );
    await expect(page.getByTestId("wb-insights-bar-typing")).toContainText(
      "Your typing50",
    );
    await expect(page.getByTestId("wb-insights-typing-test")).toHaveText(
      "Retake typing test",
    );
    // 1,750 words: 35 min at 50 wpm typing, 11⅔ min speaking.
    await expect(page.getByTestId("wb-insights-saved-value")).toHaveText(
      "23 minutes",
    );
    await expect(page.getByTestId("wb-insights-saved")).toContainText(
      "Time saved vs typing",
    );
    await expect(page.getByTestId("wb-insights-saved-basis")).toHaveText(
      "150 wpm speaking vs 50 wpm typing",
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
      "4-day streak",
    );
    await expect(page.getByTestId("wb-insights-categories-used")).toHaveText(
      "3",
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
    // The current streak's four days are outlined, no others.
    await expect(heat.locator("rect[data-streak]")).toHaveCount(4);
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
      speaking_wpm: 155,
      speaking_source: "measured",
      time_saved_ms: 7_200_000,
    };
    await openInsights(page, ipc, hub, b);
    await expect(page.getByTestId("wb-insights-speed-compare")).toHaveText(
      "3.9× the speed of an average typist (40 wpm)",
    );
    await expect(page.getByTestId("wb-insights-bar-typing")).toContainText(
      "Average typing40",
    );
    await expect(page.getByTestId("wb-insights-typing-test")).toHaveText(
      "Take the typing test",
    );
    await expect(page.getByTestId("wb-insights-saved-value")).toHaveText(
      "2 hours",
    );
    await expect(page.getByTestId("wb-insights-saved-basis")).toHaveText(
      "155 wpm speaking vs 40 wpm average typing",
    );
  });

  test("retake the typing test: the result is saved and the page recalculates", async ({
    page,
    ipc,
    hub,
  }) => {
    const b = fresh();
    await openInsights(page, ipc, hub, b, { fork: { typing_wpm: 50 } });
    const before = (await ipc.calls("fork_insights_usage")).length;
    // Keyboard only: focus the button and press Enter.
    await page.getByTestId("wb-insights-typing-test").focus();
    await page.keyboard.press("Enter");
    const dialog = page.getByTestId("wb-typing-dialog");
    await expect(
      page.getByRole("dialog", { name: "How fast do you type?" }),
    ).toBeVisible();
    await expect(page.getByTestId("wb-typing-recalculate")).toBeDisabled();
    // The whole paragraph at once ends the test early; the result is capped
    // at the setting's bound (the wpm math itself is S-29b's clock test).
    const sample = await page.getByTestId("wb-typing-sample").innerText();
    await page.getByTestId("wb-typing-input").fill(sample);
    await expect(page.getByTestId("wb-typing-result")).toHaveText(
      "You type about 300 words a minute.",
    );
    // What the backend then computes from the new speed.
    b.usage = usageFixture({
      speed: {
        words_per_minute: 150,
        typing_wpm: 300,
        typing_source: "measured",
        times_typing: 0.5,
        speaking_wpm: 150,
        speaking_source: "measured",
        time_saved_ms: 0,
      },
    });
    await page.getByTestId("wb-typing-recalculate").click();
    await expect(dialog).toBeHidden();
    expect(hub.forkSettings.typing_wpm).toBe(300);
    await expect
      .poll(async () => (await ipc.calls("fork_insights_usage")).length)
      .toBeGreaterThan(before);
    await expect(page.getByTestId("wb-insights-speed-compare")).toHaveText(
      "0.5× your typing speed of 300 wpm",
    );
    // A typist faster than the speaker saves nothing, never less.
    await expect(page.getByTestId("wb-insights-saved-value")).toHaveText(
      "0 minutes",
    );
    await expect(page.getByTestId("wb-insights-saved-basis")).toHaveText(
      "150 wpm speaking vs 300 wpm typing",
    );
  });

  test("closing the typing test keeps the stored speed", async ({
    page,
    ipc,
    hub,
  }) => {
    await openInsights(page, ipc, hub, fresh(), { fork: { typing_wpm: 50 } });
    await page.getByTestId("wb-insights-typing-test").click();
    await expect(page.getByTestId("wb-typing-dialog")).toBeVisible();
    await page.getByTestId("wb-typing-input").fill("Every");
    await page.keyboard.press("Escape");
    await expect(page.getByTestId("wb-typing-dialog")).toBeHidden();
    expect(hub.forkSettings.typing_wpm).toBe(50);
    await expect(page.getByTestId("wb-insights-typing-test")).toBeFocused();
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
        speaking_wpm: 200,
        speaking_source: "estimate",
        time_saved_ms: 0,
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
      "0-day streak",
    );
    // No words, nothing saved yet; the estimate says so; the typing test
    // is still offered.
    await expect(page.getByTestId("wb-insights-saved-value")).toHaveText(
      "0 minutes",
    );
    await expect(page.getByTestId("wb-insights-saved-basis")).toHaveText(
      "About 200 wpm speaking vs 40 wpm average typing",
    );
    await expect(page.getByTestId("wb-insights-typing-test")).toBeVisible();
  });

  test("time saved is the hero: big figure on top, three stats in a row under it", async ({
    page,
    ipc,
    hub,
  }) => {
    const b = fresh();
    // The owner's example: 150,000 words, 150 wpm speaking, 37.5 typing:
    // 3,000 min = 50 h, shown as days and hours.
    b.usage = usageFixture({
      total_words: 150_000,
      speed: {
        words_per_minute: 150,
        typing_wpm: 38,
        typing_source: "measured",
        times_typing: 3.9,
        speaking_wpm: 150,
        speaking_source: "measured",
        time_saved_ms: 3_000 * 60_000,
      },
    });
    await openInsights(page, ipc, hub, b);
    const hero = page.getByTestId("wb-insights-saved");
    await expect(page.getByTestId("wb-insights-saved-value")).toHaveText(
      "2 days, 2 hours",
    );
    await expect(
      page.getByTestId("wb-insights-saved-figure").locator("[data-unit]"),
    ).toHaveCount(2);
    await expect(hero.getByRole("heading")).toHaveText("Time saved vs typing");
    const cards = ["speed", "fixes", "words"].map((id) =>
      page.getByTestId(`wb-insights-${id}`),
    );
    const heroBox = (await hero.boundingBox())!;
    const boxes = await Promise.all(
      cards.map(async (c) => (await c.boundingBox())!),
    );
    // Hero first, full width; the three cards under it in one row.
    for (const box of boxes) {
      expect(box.y).toBeGreaterThanOrEqual(heroBox.y + heroBox.height);
      expect(Math.abs(box.y - boxes[0].y)).toBeLessThan(1);
    }
    expect(boxes[0].x).toBeLessThan(boxes[1].x);
    expect(boxes[1].x).toBeLessThan(boxes[2].x);
    expect(
      Math.abs(heroBox.width - (boxes[2].x + boxes[2].width - boxes[0].x)),
    ).toBeLessThan(2);
    // The hero's number is far larger than a stat card's.
    const size = (sel: string) =>
      page
        .locator(sel)
        .first()
        .evaluate((el) => parseFloat(getComputedStyle(el).fontSize));
    expect(
      await size('[data-testid="wb-insights-saved-figure"]'),
    ).toBeGreaterThanOrEqual(
      2 * (await size('[data-testid="wb-insights-words"] p')),
    );
    // The words card no longer repeats the time saved.
    await expect(page.getByTestId("wb-insights-words")).not.toContainText(
      /saved/i,
    );
    await expect(page.getByTestId("wb-insights-words-value")).toHaveText(
      "150,000",
    );
  });

  test("words without a measured speaking rate use the 200 wpm estimate", async ({
    page,
    ipc,
    hub,
  }) => {
    const b = fresh();
    b.usage = usageFixture({
      total_words: 900,
      speed: {
        words_per_minute: null,
        typing_wpm: 45,
        typing_source: "average",
        times_typing: null,
        speaking_wpm: 200,
        speaking_source: "estimate",
        time_saved_ms: 930_000,
      },
    });
    await openInsights(page, ipc, hub, b);
    await expect(page.getByTestId("wb-insights-saved-value")).toHaveText(
      "16 minutes",
    );
    await expect(page.getByTestId("wb-insights-saved-basis")).toHaveText(
      "About 200 wpm speaking vs 45 wpm average typing",
    );
    await expect(page.getByTestId("wb-insights-typing-test")).toHaveText(
      "Take the typing test",
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
    // Load-only flake under a parallel release build: 180 s (repo rule).
    test.setTimeout(180_000);
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
    // Rendering the PNG is CPU-bound; under load it took over 5 s.
    await ipc.expectCalled(
      "fork_insights_save_image",
      { path: "/Users/test/Desktop/insights.png" },
      60_000,
    );
    const [call] = await ipc.calls("fork_insights_save_image");
    const png = Buffer.from(String(call.args.pngBase64), "base64");
    expect(png.subarray(0, 8)).toEqual(
      Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    );
    // A 2× render of the export area plus a 24 px margin on each side.
    const area = await page
      .getByTestId("wb-insights-export")
      .evaluate((e) => [
        (e as HTMLElement).offsetWidth,
        (e as HTMLElement).offsetHeight,
      ]);
    expect(png.readUInt32BE(16)).toBe(2 * (area[0] + 48));
    expect(png.readUInt32BE(20)).toBe(2 * (area[1] + 48));
    await expect(page.getByText("Saved the image.")).toBeVisible();
  });

  test("cancelling the save panel writes nothing; a failure says so", async ({
    page,
    ipc,
    hub,
  }) => {
    // Load-only flake under a parallel release build: 180 s (repo rule).
    test.setTimeout(180_000);
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
    ).toBeVisible({ timeout: 60_000 });
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
    // The typing test in Arabic: the paragraph stays left to right.
    await page.getByTestId("wb-insights-typing-test").click();
    await expect(page.getByTestId("wb-typing-sample")).toHaveAttribute(
      "dir",
      "ltr",
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

    test(`axe: the typing test dialog in ${theme}`, async ({
      page,
      ipc,
      hub,
    }) => {
      await openInsights(page, ipc, hub, fresh(), { handy: { theme } });
      await page.getByTestId("wb-insights-typing-test").click();
      await expect(page.getByTestId("wb-typing-sample")).toBeVisible();
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
