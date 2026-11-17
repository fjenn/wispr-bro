// SPDX-License-Identifier: GPL-3.0-or-later
// Wispr Bro for iPhone: screenshots of every screen at 393 × 852 @3x, light
// and dark, on mocked data (no personal data). Skipped unless
// WB_IOS_SHOTS_DIR is set:
//
//   WB_IOS_SHOTS_DIR=/tmp/ios-shots bun run wb:test:ui -- ios-shots
import path from "node:path";
import type { Page } from "@playwright/test";
import type {
  DictionaryEntry,
  HistoryRow,
  SnippetEntry,
  UsageInsights,
  VoiceProfile,
} from "../../src/fork/bindings";
import type { IpcMock } from "./fixtures";
import {
  test,
  expect,
  openIos,
  openIosOnboarding,
  readySpeech,
  type IosState,
} from "./ios-fixture";

const dir = process.env.WB_IOS_SHOTS_DIR;

const HISTORY_TEXTS = [
  "Can we move the proposal review to Thursday afternoon? I'd like Priya to join, and she is out until Wednesday.",
  "Reminder to self: pick up the parcel before six and call the dentist about next week.",
  "Thanks for the quick turnaround. The second draft reads much better, especially the opening paragraph.",
  "Milk, eggs, coffee beans, two lemons and something nice for Saturday.",
  "I'll be ten minutes late, start without me.",
  "The train leaves at 7:42 from platform nine, so let's meet at the kiosk at half past seven.",
];

function historyRows(): HistoryRow[] {
  const now = Date.now();
  return HISTORY_TEXTS.map((text, i) => ({
    id: 100 - i,
    text,
    copy_text: text,
    variant: "final",
    ai_edited: false,
    source: "dictation",
    source_name: null,
    app_name: ["Messages", "Notes", "Mail", "Notes", "Messages", "Mail"][i],
    file_name: `wispr-bro-${i}.wav`,
    audio: "available",
    flagged: i === 2,
    cleanup_level: "light",
    queue_status: null,
    created_at: now - (i < 3 ? i * 47 : 24 * 60 + i * 53) * 60_000,
  })) as HistoryRow[];
}

function installHistory(ipc: IpcMock, rows: HistoryRow[]) {
  ipc.respond("fork_history_page", { rows, next: null });
  ipc.respond("fork_history_find", { rows: rows.slice(0, 2), next: null });
  ipc.respond("fork_history_rows", rows);
}

function installDictionary(ipc: IpcMock) {
  const base = 1_700_000_000_000;
  const entry = (
    id: number,
    phrase: string,
    replacement: string | null,
    hits: number,
  ): DictionaryEntry =>
    ({
      id,
      kind: replacement ? "rule" : "word",
      phrase,
      replacement,
      source: "user",
      status: "active",
      created_at: base + id * 1000,
      updated_at: base + id * 1000,
      hit_count: hits,
      last_hit_at: hits ? base + id * 2000 : null,
    }) as DictionaryEntry;
  const entries = [
    entry(6, "Kubernetes", null, 12),
    entry(5, "Priya Raghunathan", null, 4),
    entry(4, "gonna", "going to", 9),
    entry(3, "Wispr Bro", null, 2),
    entry(2, "my email", "sam.fielding@example.com", 3),
    entry(1, "Saoirse", null, 0),
  ];
  ipc.respond("fork_dictionary_list", {
    entries,
    counts: { words: 4, rules: 2, suggested: 0 },
  });
}

function installSnippets(ipc: IpcMock) {
  const base = 1_700_000_000_000;
  const snippet = (
    id: number,
    trigger: string,
    body: string,
    hits: number,
  ): SnippetEntry =>
    ({
      id,
      trigger,
      body,
      source: "user",
      created_at: base + id * 1000,
      updated_at: base + id * 1000,
      hit_count: hits,
      last_hit_at: hits ? base + id * 2000 : null,
    }) as SnippetEntry;
  const snippets = [
    snippet(3, "my address", "Alex Morgan\n12 Harbour Lane\nKiel 24103", 7),
    snippet(
      2,
      "meeting link",
      "https://meet.example.com/alex-morgan-weekly",
      3,
    ),
    snippet(1, "sign off", "Thanks so much, and talk soon.\nAlex", 11),
  ];
  ipc.respond("fork_snippets_list", {
    snippets,
    counts: { snippets: 3, uses: 21 },
  });
}

function installInsights(ipc: IpcMock) {
  const end = Date.now();
  const band = (w: number) => [1, 250, 500, 750].filter((f) => w >= f).length;
  const heatmap = Array.from({ length: 365 }, (_, i) => {
    const date = new Date(end - (364 - i) * 86_400_000)
      .toISOString()
      .slice(0, 10);
    const words =
      i > 300 && (i * 7) % 5 !== 0 ? ((i * 137) % 900) + (i % 3) * 40 : 0;
    return { date, words, level: band(words) };
  });
  const usage: UsageInsights = {
    total_words: 12_840,
    total_dictations: 412,
    speech_ms: 5_400_000,
    speed: {
      words_per_minute: 142,
      typing_wpm: 40,
      typing_source: "average",
      times_typing: 3.5,
      speaking_wpm: 142,
      speaking_source: "measured",
      time_saved_ms: 13_800_000,
    },
    fixes_dictionary: 37,
    fixes_llm: 210,
    categories: [
      { category: "documents", words: 3_210, dictations: 80, share: 0.25 },
      { category: "ai_prompts", words: 1_284, dictations: 40, share: 0.1 },
      { category: "email", words: 3_852, dictations: 120, share: 0.3 },
      { category: "work", words: 1_926, dictations: 60, share: 0.15 },
      { category: "personal", words: 2_568, dictations: 112, share: 0.2 },
      { category: "other", words: 0, dictations: 0, share: 0 },
    ],
    heatmap,
    heatmap_active_days: 48,
    current_streak_days: 4,
    longest_streak_days: 11,
    first_day: heatmap[301].date,
    today: heatmap[364].date,
  } as UsageInsights;
  const voice: VoiceProfile = {
    unlocked: true,
    total_words: 12_840,
    goal: 1_700,
    dictations_read: 200,
    phrase: { phrase: "Can we move the proposal", dictations: 4 },
    peak: {
      hour: 15,
      hour_words: 450,
      hour_share: 0.31,
      app_name: "Mail",
      app_words: 400,
      app_share: 0.3,
    },
    word: { word: "proposal", uses: 28 },
    corrected: { heard: "cooper netties", written: "Kubernetes", times: 6 },
  } as VoiceProfile;
  ipc.handle("fork_insights_usage", () => usage);
  ipc.handle("fork_insights_voice", () => voice);
}

test.describe("ios shots", () => {
  test.skip(!dir, "set WB_IOS_SHOTS_DIR to write screenshots");
  test.setTimeout(240_000);

  for (const theme of ["light", "dark"] as const) {
    const snap = async (page: Page, name: string) => {
      await page.waitForTimeout(500);
      await page.screenshot({ path: path.join(dir!, `${name}-${theme}.png`) });
    };
    const settle = (page: Page) =>
      page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });

    test(`tabs ${theme}`, async ({ page, ipc, hub, ios }) => {
      await settle(page);
      Object.assign(hub.stats, {
        total_words: 12_840,
        total_dictations: 412,
        words_per_minute: 142.6,
        current_streak_days: 4,
      });
      const rows = historyRows();
      await openIos(page, ipc, hub, ios, {
        handy: { theme, audio_feedback: true, spoken_languages: ["en", "nl"] },
        beforeNavigate: () => {
          installHistory(ipc, rows);
          installDictionary(ipc);
          installSnippets(ipc);
          installInsights(ipc);
        },
      });
      await expect(page.getByTestId("wb-ios-session")).toBeVisible();
      await snap(page, "home-session-off");
      await page.getByTestId("wb-ios-session-start").click();
      await expect(page.getByTestId("wb-ios-session")).toHaveAttribute(
        "data-state",
        "idle",
      );
      await snap(page, "home-session-on");
      ios.session = { state: "listening" };
      await expect(page.getByTestId("wb-ios-session")).toHaveAttribute(
        "data-state",
        "listening",
      );
      await snap(page, "home-session-listening");
      await page.getByTestId("wb-main").evaluate((el) => el.scrollTo(0, 9999));
      await snap(page, "home-scrolled");

      await page.getByTestId("wb-ios-tab-history").click();
      await expect(page.getByTestId("wb-history-row").first()).toBeVisible();
      await snap(page, "history");

      await page.getByTestId("wb-ios-tab-dictionary").click();
      await expect(page.getByTestId("wb-dictionary")).toBeVisible();
      await snap(page, "dictionary");

      await page.getByTestId("wb-ios-tab-snippets").click();
      await expect(page.getByTestId("wb-snippets")).toBeVisible();
      await snap(page, "snippets");

      await page.getByTestId("wb-ios-tab-settings").click();
      await expect(page.getByTestId("wb-ios-settings")).toBeVisible();
      await snap(page, "settings");
      await page.getByTestId("wb-main").evaluate((el) => el.scrollTo(0, 9999));
      await snap(page, "settings-scrolled");

      for (const sub of [
        "languages",
        "cleanup",
        "session",
        "privacy",
        "about",
        "licences",
      ]) {
        await page.getByTestId("wb-main").evaluate((el) => el.scrollTo(0, 0));
        await page.getByTestId(`wb-ios-settings-${sub}`).click();
        await expect(page.getByTestId("wb-ios-back")).toBeVisible();
        await snap(page, `settings-${sub}`);
        await page.getByTestId("wb-ios-back").click();
      }
      await page.getByTestId("wb-ios-settings-style").click();
      await expect(page.getByTestId("wb-style")).toBeVisible();
      await snap(page, "style");
      await page.getByTestId("wb-ios-back").click();

      await page.getByTestId("wb-ios-tab-home").click();
      await page.getByTestId("wb-ios-home-insights").click();
      await expect(page.getByTestId("wb-insights")).toBeVisible();
      await page.mouse.move(2, 2);
      await snap(page, "insights");
      await page.getByTestId("wb-main").evaluate((el) => el.scrollTo(0, 600));
      await snap(page, "insights-scrolled");
    });

    test(`empty and setup ${theme}`, async ({ page, ipc, hub, ios }) => {
      await settle(page);
      ios.setup = {
        keyboardEnabled: false,
        fullAccess: false,
        micPermission: 0,
      };
      ios.speech = readySpeech({
        phase: "downloading",
        percent: 42,
        downloaded_bytes: 38_000_000,
        total_bytes: 90_000_000,
        eta_seconds: 95,
      });
      ios.llmState = "ready";
      await openIos(page, ipc, hub, ios, { handy: { theme } });
      await expect(page.getByTestId("wb-ios-setup")).toBeVisible();
      await snap(page, "home-setup");
      await page.getByTestId("wb-ios-setup-mic-allow").click();
      await expect(
        page.getByTestId("wb-ios-setup-keyboard-show"),
      ).toBeVisible();
      await snap(page, "home-setup-keyboard");
      await page.getByTestId("wb-ios-setup-keyboard-show").click();
      await expect(page.getByTestId("wb-ios-keyboard-sheet")).toBeVisible();
      await snap(page, "home-keyboard-sheet");
      await page.keyboard.press("Escape");

      await page.getByTestId("wb-ios-tab-history").click();
      await expect(page.getByTestId("wb-empty-dictation")).toBeVisible();
      await snap(page, "history-empty");
      await page.getByTestId("wb-ios-tab-dictionary").click();
      await snap(page, "dictionary-empty");
      await page.getByTestId("wb-ios-tab-snippets").click();
      await snap(page, "snippets-empty");
      await page.getByTestId("wb-ios-tab-settings").click();
      await page.getByTestId("wb-ios-settings-cleanup").click();
      await snap(page, "settings-cleanup-available");
    });

    test(`sheets and menus ${theme}`, async ({ page, ipc, hub, ios }) => {
      await settle(page);
      const rows = historyRows();
      await openIos(page, ipc, hub, ios, {
        handy: { theme },
        hash: "#/hub/dictionary",
        beforeNavigate: () => {
          installHistory(ipc, rows);
          installDictionary(ipc);
          installSnippets(ipc);
        },
      });
      await page.getByTestId("wb-dictionary-add").click();
      await expect(page.getByTestId("wb-dictionary-dialog")).toBeVisible();
      await snap(page, "sheet-add-word");
      await page.keyboard.press("Escape");
      await page.getByTestId("wb-dictionary-search-open").click();
      await page.getByTestId("wb-dictionary-search").fill("pri");
      await snap(page, "dictionary-search");

      await page.getByTestId("wb-ios-tab-snippets").click();
      await page.getByTestId("wb-snippets-add").click();
      await expect(page.getByTestId("wb-snippet-dialog")).toBeVisible();
      await snap(page, "sheet-add-snippet");
      await page.keyboard.press("Escape");
      await page.getByTestId("wb-snippets-sort").click();
      await snap(page, "menu-sort");
      await page.keyboard.press("Escape");

      await page.getByTestId("wb-ios-tab-history").click();
      await expect(page.getByTestId("wb-history-row").first()).toBeVisible();
      await page
        .getByTestId("wb-history-row")
        .first()
        .getByRole("button", { name: /more/i })
        .click();
      await snap(page, "menu-history-row");
      await page.keyboard.press("Escape");
      await page.getByTestId("wb-history-search-open").click();
      await page.getByTestId("wb-history-search").fill("proposal");
      await snap(page, "history-search");
      await page.getByTestId("wb-history-search-close").click();
      await page.getByTestId("wb-history-flag").first().click();
      await snap(page, "sheet-flag");
      await page.keyboard.press("Escape");

      await page.getByTestId("wb-ios-tab-settings").click();
      await page.getByTestId("wb-ios-settings-privacy").click();
      await page.getByTestId("wb-privacy-delete-button").click();
      await expect(page.getByTestId("wb-delete-dialog")).toBeVisible();
      await snap(page, "sheet-delete-all");
      await page.getByTestId("wb-delete-cancel").click();
      await page.getByTestId("wb-privacy-text-retention").click();
      await snap(page, "select-retention");
      await page.keyboard.press("Escape");
      await page.getByTestId("wb-ios-back").click();
      await page.getByTestId("wb-ios-settings-licences").click();
      await page.getByText("llama.cpp").click();
      await expect(page.getByTestId("wb-ios-licence-text")).toContainText(
        "Permission",
      );
      await snap(page, "sheet-licence");
      await page.keyboard.press("Escape");

      await page.getByTestId("wb-ios-tab-home").click();
      await page.getByTestId("wb-greeting-edit").click();
      await snap(page, "home-name-editor");
    });

    const STATES: Record<string, (ios: IosState) => void> = {
      "microphone-granted": () => undefined,
      "microphone-denied": (ios) => {
        ios.setup = { ...ios.setup, micPermission: 1 };
      },
      "keyboard-added": (ios) => {
        ios.setup = { ...ios.setup, fullAccess: false };
      },
      "keyboard-ready": () => undefined,
      "model-ready": () => undefined,
      "model-failed": (ios) => {
        ios.speech = readySpeech({
          phase: "failed",
          percent: 12,
          problem: "offline",
        });
      },
      "done-pending": (ios) => {
        ios.setup = { ...ios.setup, keyboardEnabled: false, fullAccess: false };
      },
    };
    for (const [name, arrange] of Object.entries(STATES)) {
      test(`onboarding state ${name} ${theme}`, async ({
        page,
        ipc,
        hub,
        ios,
      }) => {
        await settle(page);
        arrange(ios);
        const step = name.split("-")[0];
        await openIosOnboarding(page, ipc, hub, ios, step, {
          handy: { theme },
        });
        await expect(page.getByTestId("wb-onboarding")).toHaveAttribute(
          "data-step",
          step,
        );
        await snap(page, `onboarding-${name}`);
      });
    }

    for (const step of [
      "welcome",
      "languages",
      "microphone",
      "keyboard",
      "model",
      "practice",
      "done",
    ]) {
      test(`onboarding ${step} ${theme}`, async ({ page, ipc, hub, ios }) => {
        await settle(page);
        if (step === "microphone" || step === "keyboard") {
          ios.setup = {
            keyboardEnabled: false,
            fullAccess: false,
            micPermission: 0,
          };
        }
        if (step === "model") {
          ios.speech = readySpeech({
            phase: "downloading",
            percent: 42,
            downloaded_bytes: 38_000_000,
            total_bytes: 90_000_000,
            eta_seconds: 95,
          });
        }
        await openIosOnboarding(page, ipc, hub, ios, step, {
          handy: { theme },
        });
        await expect(page.getByTestId("wb-onboarding")).toHaveAttribute(
          "data-step",
          step,
        );
        await snap(page, `onboarding-${step}`);
      });
    }
  }
});
