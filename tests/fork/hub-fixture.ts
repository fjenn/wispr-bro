// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Shared setup for the Hub specs (S-13): a returning user (onboarding done,
 * permissions granted) so `App.tsx` renders `ForkApp`, plus fork settings
 * that the specs can read back after the page writes them.
 *
 *   import { test, expect, openHub } from "./hub-fixture";
 *   test("…", async ({ page, ipc, hub }) => {
 *     await openHub(page, ipc, { theme: "dark" });
 *     expect(hub.forkSettings.dismissed_banners).toEqual([]);
 *   });
 */
import AxeBuilder from "@axe-core/playwright";
import type { Page } from "@playwright/test";
import type { StatsSummary } from "../../src/fork/bindings";
import { test as base, expect, type IpcMock } from "./fixtures";

export { expect };

/** Fork settings as `fork_get_settings` returns them (Rust defaults). */
export function defaultForkSettings(): Record<string, unknown> {
  return {
    schema_version: 1,
    hotkeys: {},
    cleanup_level: "light",
    styles: {
      personal: "relaxed",
      work: "formal",
      email: "formal",
      other: "formal",
    },
    context_awareness: true,
    transforms_enabled: false,
    command_mode_enabled: false,
    auto_apply_transform: null,
    bar_always_visible: true,
    bar_position: "bottom",
    bar_hidden_until_ms: null,
    bar_hide_in_recordings: false,
    bar_x_by_display: {},
    live_text_in_pill: false,
    text_retention_days: null,
    audio_retention_days: 14,
    notifications: {},
    update_checks_enabled: true,
    update_channel: "stable",
    show_in_dock: true,
    onboarding: {},
    dismissed_banners: [],
    seen_new_tags: [],
    first_use_choice: null,
    typing_wpm: null,
    typing_hours_per_day: 2,
    greeting_name: null,
    auto_learn_enabled: false,
    vibe_coding: {},
    applied_first_run_defaults: [],
  };
}

/** `fork_stats_summary` for a profile with no dictations (S-09 shape). */
export function emptyStats(): StatsSummary {
  return {
    total_words: 0,
    total_dictations: 0,
    today_words: 0,
    today_dictations: 0,
    speech_ms: 0,
    words_per_minute: null,
    current_streak_days: 0,
    longest_streak_days: 0,
    active_days: 0,
    fixes_dictionary: 0,
    fixes_llm: 0,
  };
}

/** Handy settings for a returning user. */
export function handySettings(
  overrides: Record<string, unknown> = {},
): Record<string, unknown> {
  return {
    onboarding_completed: true,
    selected_model: "parakeet-tdt-0.6b-v3",
    theme: "light",
    app_language: "en",
    debug_mode: false,
    post_process_enabled: false,
    update_checks_enabled: false,
    show_whats_new_on_update: false,
    whats_new_last_seen_version: "1.0.0-alpha.1",
    spoken_languages: ["en"],
    custom_words: [],
    bindings: {},
    ...overrides,
  };
}

export interface HubState {
  /** The fork settings the mocked backend holds (updated by writes). */
  forkSettings: Record<string, unknown>;
  /** Handy settings the mocked backend holds. */
  handy: Record<string, unknown>;
  /** What `fork_stats_summary` returns (change it, then emit `fork-stats-updated`). */
  stats: StatsSummary;
  /** What `fork_account_name` returns (the macOS first name, or null). */
  accountName: string | null;
}

export interface OpenHubOptions {
  handy?: Record<string, unknown>;
  fork?: Record<string, unknown>;
  /** URL hash to open, e.g. `#/hub/settings/history`. */
  hash?: string;
  /** Viewport (default 1040×720, the Hub's default page size). */
  size?: { width: number; height: number };
  /** Runs after the default replies are installed, before navigation. */
  beforeNavigate?: () => void;
}

export const test = base.extend<{ hub: HubState }>({
  hub: async ({}, use) => {
    await use({
      forkSettings: defaultForkSettings(),
      handy: handySettings(),
      stats: emptyStats(),
      accountName: "Sam",
    });
  },
});

/**
 * Installs the backend replies a returning user needs and opens the Hub.
 * Waits until the sidebar is visible.
 */
export async function openHub(
  page: Page,
  ipc: IpcMock,
  hub: HubState,
  options: OpenHubOptions = {},
): Promise<void> {
  Object.assign(hub.handy, options.handy ?? {});
  Object.assign(hub.forkSettings, options.fork ?? {});
  await page.setViewportSize(options.size ?? { width: 1040, height: 720 });

  ipc.handle("get_app_settings", () => hub.handy);
  ipc.handle("get_default_settings", () => handySettings());
  ipc.handle("change_theme_setting", (args) => {
    hub.handy.theme = args.theme;
    return null;
  });
  ipc.respond({
    "plugin:macos-permissions|check_accessibility_permission": true,
    "plugin:macos-permissions|check_microphone_permission": true,
    get_retired_models: [],
    get_secure_input_status: { active: false, sustained: false },
    get_available_models: [],
    get_available_microphones: [],
    get_available_output_devices: [],
    get_current_model: "",
    check_custom_sounds: { start: false, stop: false },
    is_update_checks_locked: false,
    "plugin:app|version": "1.0.0-alpha.1",
    fork_ping: {
      message: "pong",
      seq: 0,
      ready: true,
      debug_build: false,
      cli_debug: false,
    },
  });
  ipc.handle("fork_get_settings", () => hub.forkSettings);
  ipc.handle("fork_set_setting", (args) => {
    const update = args.update as { key: string; value: unknown };
    hub.forkSettings[update.key] = update.value;
    return hub.forkSettings;
  });

  ipc.handle("fork_stats_summary", () => hub.stats);
  // S-16: an empty dictionary unless the spec installs its own backend
  // (handlers registered later take precedence).
  ipc.respond("fork_dictionary_list", {
    entries: [],
    counts: { words: 0, rules: 0, suggested: 0 },
  });
  ipc.handle("fork_account_name", () => hub.accountName);
  // S-15b: an empty history unless the spec installs its own backend.
  ipc.respond("fork_history_page", { rows: [], next: null });
  ipc.respond("fork_history_find", { rows: [], next: null });
  ipc.respond("fork_history_rows", []);
  // A spec's own backend, installed over these defaults.
  options.beforeNavigate?.();

  await page.goto(`/${options.hash ?? ""}`);
  await expect(page.getByTestId("wb-sidebar")).toBeVisible();
}

/** Axe on the whole page: no serious or critical findings (R5 §5.5). */
export async function expectNoSeriousAxeFindings(
  page: Page,
  options: { exclude?: string[] } = {},
): Promise<void> {
  // Let open/close animations finish: axe reads opacity mid-fade as low
  // contrast.
  await page.waitForFunction(() =>
    document.getAnimations().every((a) => a.playState !== "running"),
  );
  // Handy's own settings sections, hosted unchanged on the Settings page
  // until S-14 rebuilds them, are outside the Hub's scope (S-37 audits them).
  let builder = new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
    .exclude("[data-handy-content]");
  for (const selector of options.exclude ?? []) {
    builder = builder.exclude(selector);
  }
  const results = await builder.analyze();
  const serious = results.violations.filter(
    (v) => v.impact === "serious" || v.impact === "critical",
  );
  expect(
    serious.map((v) => ({
      id: v.id,
      help: v.help,
      nodes: v.nodes.map((n) => n.target.join(" ")).slice(0, 5),
    })),
  ).toEqual([]);
}
