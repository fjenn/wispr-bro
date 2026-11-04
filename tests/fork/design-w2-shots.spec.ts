// SPDX-License-Identifier: GPL-3.0-or-later
// Design W2: screenshots of every Hub page W2 owns (Home, History, Insights,
// Dictionary, Snippets, Style, Transforms, Share), filled and empty, light
// and dark, at the Hub's default size and a tall size that shows the whole
// page. Skipped unless WB_DESIGN_SHOTS_DIR is set:
//
//   WB_DESIGN_SHOTS_DIR=/tmp/shots bun run wb:test:ui -- design-w2-shots
//
// Mocked data only (no personal data).
import path from "node:path";
import type { Page } from "@playwright/test";
import type {
  DictionaryEntry,
  HistoryRow,
  SnippetEntry,
  TransformEntry,
  UsageInsights,
} from "../../src/fork/bindings";
import type { IpcMock } from "./fixtures";
import { test, openHub, type HubState } from "./hub-fixture";

const dir = process.env.WB_DESIGN_SHOTS_DIR;
/** Optional comma-separated page ids to shoot (all when unset). */
const only = process.env.WB_W2_ONLY?.split(",");
const wanted = (id: string) => !only || only.includes(id);

const NOW = Date.now();
const MIN = 60_000;
const HOUR = 60 * MIN;
const TEXTS = [
  "Can we move the design review to Thursday afternoon? I'd like Priya to join.",
  "Thanks for the notes. I'll send the revised draft tonight.",
  "Reminder: pick up the cables and the second monitor from the office.",
  "Dear Jordan,\n\nThe invoice is attached. Let me know if anything looks off.\n\nBest,\nSam",
  "Let's ship the onboarding fix first and leave the bar tweaks for Monday.",
  "Book a table for four at eight.",
];

function historyRows(): HistoryRow[] {
  return TEXTS.map((text, i) => ({
    id: 100 - i,
    text,
    copy_text: text,
    variant: "final",
    ai_edited: i === 0,
    source: "dictation",
    source_name: null,
    app_name: "Mail",
    file_name: `wb-${i}.wav`,
    audio: "available",
    flagged: false,
    cleanup_level: "light",
    queue_status: null,
    created_at: NOW - (i < 3 ? i * 7 * MIN : HOUR * 26 + i * MIN),
  }));
}

function heatmap(): UsageInsights["heatmap"] {
  const end = Date.UTC(2026, 9, 31);
  return Array.from({ length: 365 }, (_, i) => {
    const words = i > 330 ? [0, 120, 300, 640, 900][(i * 7) % 5] : 0;
    const level = [1, 250, 500, 750].filter((f) => words >= f).length;
    const date = new Date(end - (364 - i) * 86_400_000)
      .toISOString()
      .slice(0, 10);
    return { date, words, level };
  });
}

function usage(empty = false): UsageInsights {
  return {
    total_words: empty ? 0 : 12_840,
    total_dictations: empty ? 0 : 214,
    speech_ms: empty ? 0 : 5_400_000,
    speed: {
      words_per_minute: empty ? null : 142,
      typing_wpm: 48,
      typing_source: "measured",
      times_typing: empty ? null : 3,
    },
    fixes_dictionary: empty ? 0 : 37,
    fixes_llm: empty ? 0 : 412,
    categories: [
      { category: "documents", words: 5100, dictations: 80, share: 0.4 },
      { category: "ai_prompts", words: 3200, dictations: 50, share: 0.25 },
      { category: "email", words: 2300, dictations: 40, share: 0.18 },
      { category: "work", words: 1300, dictations: 26, share: 0.1 },
      { category: "personal", words: 640, dictations: 12, share: 0.05 },
      { category: "other", words: 300, dictations: 6, share: 0.02 },
    ].map((c) => (empty ? { ...c, words: 0, dictations: 0, share: 0 } : c)),
    heatmap: empty
      ? heatmap().map((d) => ({ ...d, words: 0, level: 0 }))
      : heatmap(),
    heatmap_active_days: empty ? 0 : 24,
    current_streak_days: empty ? 0 : 6,
    longest_streak_days: empty ? 0 : 11,
    first_day: empty ? null : "2026-09-12",
    today: "2026-10-31",
  } as UsageInsights;
}

function dictionary(): DictionaryEntry[] {
  const words: [string, string | null, number, string][] = [
    ["Kubernetes", null, 14, "user"],
    ["Siobhán", null, 6, "user"],
    ["gonna", "going to", 22, "user"],
    ["Wispr Bro", null, 3, "user"],
    ["PostgreSQL", null, 0, "flag"],
    ["Priya", null, 9, "import"],
    ["teh", "the", 0, "learned"],
  ];
  return words.map(([phrase, replacement, hits, source], i) => ({
    id: i + 1,
    kind: replacement ? "rule" : "word",
    phrase,
    replacement,
    source,
    status: i === 6 ? "suggested" : "active",
    created_at: NOW - i * HOUR,
    updated_at: NOW - i * HOUR,
    hit_count: hits,
    last_hit_at: hits ? NOW - i * HOUR : null,
  })) as DictionaryEntry[];
}

function snippets(): SnippetEntry[] {
  const list: [string, string, number][] = [
    ["my address", "Alex Morgan\n12 Harbour Lane\nKiel 24103", 12],
    ["calendar link", "https://cal.example.com/sam/30min", 31],
    ["intro call", "Thanks for the call today. Here is a short recap:", 4],
    ["sign off", "Best,\nSam", 0],
  ];
  return list.map(([trigger, body, hits], i) => ({
    id: i + 1,
    trigger,
    body,
    source: "user",
    created_at: NOW - i * HOUR,
    updated_at: NOW - i * HOUR,
    hit_count: hits,
    last_hit_at: hits ? NOW - i * HOUR : null,
  })) as SnippetEntry[];
}

const RULES = {
  shorter: true,
  clearer: true,
  better_order: true,
  headings_lists: false,
  keep_tone: true,
};
function transforms(): TransformEntry[] {
  return [
    [1, "polish", "Polish"],
    [2, "prompt_engineer", "Prompt Engineer"],
    [3, "custom-3", "Make it friendly"],
  ].map(([id, key, name]) => ({
    id: id as number,
    key: key as string,
    action_id: `transform.${key}`,
    builtin_key: (key as string).startsWith("custom-") ? null : (key as string),
    name: name as string,
    description: "Clearer and shorter",
    rules: { ...RULES },
    prompt: "Polish this text.",
    examples: [],
    replacements: [],
    updated_at: 0,
  }));
}

/** A stand-in app icon: a rounded square with a letter (no real logos). */
function icon(bundleId: string): string {
  const hues = [150, 210, 265, 20, 45];
  const hue = hues[bundleId.length % hues.length];
  const letter = bundleId.split(".").pop()![0].toUpperCase();
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect x="4" y="4" width="56" height="56" rx="14" fill="hsl(${hue} 55% 48%)"/><text x="32" y="42" font-family="Helvetica" font-size="28" font-weight="700" fill="white" text-anchor="middle">${letter}</text></svg>`;
  return `data:image/svg+xml;base64,${Buffer.from(svg).toString("base64")}`;
}

function install(ipc: IpcMock, hub: HubState, filled: boolean) {
  if (filled) {
    Object.assign(hub.stats, {
      total_words: 12_840,
      total_dictations: 214,
      words_per_minute: 142.6,
      current_streak_days: 6,
      today_words: 310,
      today_dictations: 9,
    });
    const rows = historyRows();
    ipc.respond("fork_history_page", { rows, next: null });
    ipc.respond("fork_history_rows", rows);
    const entries = dictionary();
    ipc.respond("fork_dictionary_list", {
      entries,
      counts: { words: 5, rules: 1, suggested: 1 },
    });
    const snips = snippets();
    ipc.respond("fork_snippets_list", {
      snippets: snips,
      counts: { snippets: snips.length, uses: 47 },
    });
    ipc.respond("fork_transforms_list", transforms());
    ipc.respond("fork_style_overview", {
      apps: [
        ["com.apple.MobileSMS", "Messages", "personal"],
        ["net.whatsapp.WhatsApp", "WhatsApp", "personal"],
        ["com.tinyspeck.slackmacgap", "Slack", "work"],
        ["com.apple.mail", "Mail", "email"],
        ["com.apple.Notes", "Notes", "other"],
      ].map(([bundle_id, name, group]) => ({
        bundle_id,
        name,
        installed: true,
        group,
        builtin_group: group,
        user_group: null,
        auto_send: false,
        ai_chat: false,
      })),
      sites: [],
    });
  } else {
    ipc.respond("fork_snippets_list", {
      snippets: [],
      counts: { snippets: 0, uses: 0 },
    });
  }
  ipc.handle("fork_style_app_icon", (a) => icon(String(a.bundleId)));
  ipc.respond("fork_insights_usage", usage(!filled));
  ipc.respond("fork_insights_voice", {
    unlocked: false,
    total_words: filled ? 1_234 : 0,
    goal: 1_700,
    dictations_read: 0,
    phrase: null,
    peak: null,
    word: null,
    corrected: null,
  });
}

const PAGES = [
  "dictation",
  "insights",
  "dictionary",
  "snippets",
  "style",
  "transforms",
  "share",
] as const;

async function snap(page: Page, name: string) {
  await page.waitForTimeout(500);
  await page.screenshot({ path: path.join(dir!, `${name}.png`) });
}

test.describe("design W2 shots", () => {
  test.skip(!dir, "set WB_DESIGN_SHOTS_DIR to write screenshots");
  test.setTimeout(240_000);

  for (const theme of ["light", "dark"] as const) {
    for (const filled of [true, false]) {
      const kind = filled ? "filled" : "empty";
      test(`w2 ${kind} ${theme}`, async ({ page, ipc, hub }) => {
        await page.emulateMedia({ colorScheme: theme });
        await openHub(page, ipc, hub, {
          handy: { theme },
          fork: filled
            ? {
                transforms_enabled: true,
                hotkeys: {
                  "transform.polish": [
                    { chord: "option+1", activation: "press" },
                  ],
                  "transform.prompt_engineer": [
                    { chord: "option+2", activation: "press" },
                  ],
                  "transform.view_changes": [
                    { chord: "option+o", activation: "press" },
                  ],
                  command_mode: [{ chord: "fn+ctrl", activation: "hold" }],
                },
              }
            : {},
          size: { width: 1040, height: 720 },
          beforeNavigate: () => install(ipc, hub, filled),
        });
        for (const id of PAGES) {
          if (!wanted(id)) continue;
          await page.setViewportSize({ width: 1040, height: 720 });
          await page.getByTestId(`wb-nav-${id}`).click();
          await snap(page, `${id}-${kind}-${theme}`);
          if (filled) {
            await page.setViewportSize({ width: 1040, height: 1400 });
            await snap(page, `${id}-${kind}-${theme}-tall`);
          }
        }
        if (filled && !only) {
          await page.setViewportSize({ width: 1040, height: 720 });
          await page.getByTestId("wb-nav-insights").click();
          await page.getByRole("tab", { name: /voice/i }).click();
          await snap(page, `insights-voice-${theme}`);
          await page.getByTestId("wb-nav-style").click();
          await page.getByRole("tab", { name: /cleanup/i }).click();
          await snap(page, `style-cleanup-${theme}`);
          await page.getByTestId("wb-nav-dictionary").click();
          await page.getByRole("tab", { name: /suggested/i }).click();
          await snap(page, `dictionary-suggested-${theme}`);
          await page.getByTestId("wb-nav-dictation").click();
          await page.getByTestId("wb-history-row").first().hover();
          await snap(page, `dictation-hover-${theme}`);
        }
      });
    }
  }
});
