// SPDX-License-Identifier: GPL-3.0-or-later
// S-30: the first-use chooser (every option has a scene and a matching
// hint), "Try it now" → the spotlight, the "Use it in your own apps" banner,
// the setup checklist (auto-ticks from state and events, the backend's
// try_in_app tick, coach marks, chaining, Open at login, hide, all done),
// and the `fork-spotlight` page (hint above/below the cut-out, fallback
// shortcut wording, inactive, RTL). Keyboard-only, reduced motion and axe
// in light and dark. Mocked IPC; see hub-fixture.ts.
import { readFileSync } from "node:fs";
import AxeBuilder from "@axe-core/playwright";
import type { Page } from "@playwright/test";
import type { ForkSpotlightState } from "../../src/fork/bindings";
import { test as base } from "./fixtures";
import {
  test,
  expect,
  openHub,
  expectNoSeriousAxeFindings,
  type HubState,
} from "./hub-fixture";
import type { IpcMock } from "./fixtures";
import { expectRtl, RTL_APP_SETTINGS } from "./pill-page";

const en = JSON.parse(
  readFileSync(
    new URL("../../src/i18n/locales/en/fork.json", import.meta.url),
    "utf8",
  ),
);
const CHOICES = [
  "ai_chat",
  "note",
  "message",
  "email",
  "comment",
  "document",
  "coding",
] as const;

function completed(checklist_done: string[] = []) {
  return {
    status: "completed",
    current_step: null,
    completed_steps: [],
    checklist_done,
    completed_at_ms: 1,
  };
}

interface Backend {
  words: number;
  snippets: number;
  spotlightStarts: string[];
  spotlightAvailable: boolean;
  trigger: string;
}

/** The S-30 backend: spotlight, checklist ticks, counts, hotkey trigger. */
function installBackend(ipc: IpcMock, hub: HubState, b: Backend): void {
  ipc.handle("fork_spotlight_start", (args) => {
    b.spotlightStarts.push(args.choice as string);
    return b.spotlightAvailable;
  });
  ipc.respond("fork_spotlight_stop", null);
  ipc.respond("fork_spotlight_state", {
    seq: 0,
    active: false,
    choice: null,
    hole: null,
    edge: "bottom",
  });
  ipc.handle("fork_checklist_mark", (args) => {
    const onboarding = hub.forkSettings.onboarding as {
      checklist_done: string[];
    };
    if (!onboarding.checklist_done.includes(args.id as string)) {
      onboarding.checklist_done = [
        ...onboarding.checklist_done,
        args.id as string,
      ];
    }
    return onboarding;
  });
  ipc.handle("fork_dictionary_list", () => ({
    entries: [],
    counts: { words: b.words, rules: 0, suggested: 0 },
  }));
  ipc.handle("fork_snippets_list", () => ({
    snippets: [],
    counts: { snippets: b.snippets, uses: 0 },
  }));
  ipc.handle("fork_hotkey_status", () => ({
    running: true,
    error: null,
    trigger: b.trigger,
    globe: "do_nothing",
    fn_blocked: false,
    issues: [],
    bindings: [],
  }));
  ipc.handle("change_autostart_setting", (args) => {
    hub.handy.autostart_enabled = args.enabled;
    return null;
  });
}

function backend(over: Partial<Backend> = {}): Backend {
  return {
    words: 0,
    snippets: 0,
    spotlightStarts: [],
    spotlightAvailable: true,
    trigger: "fn",
    ...over,
  };
}

async function open(
  page: Page,
  ipc: IpcMock,
  hub: HubState,
  b: Backend,
  options: {
    fork?: Record<string, unknown>;
    handy?: Record<string, unknown>;
  } = {},
) {
  await openHub(page, ipc, hub, {
    fork: { onboarding: completed(), ...(options.fork ?? {}) },
    handy: options.handy,
    beforeNavigate: () => installBackend(ipc, hub, b),
  });
}

const chooser = (page: Page) => page.getByTestId("wb-first-use");

test.describe("first-use chooser", () => {
  test("opens once setup is done, with seven ways and a scene each", async ({
    page,
    ipc,
    hub,
  }) => {
    const b = backend();
    await open(page, ipc, hub, b);
    await expect(chooser(page)).toBeVisible();
    await expect(
      page.getByRole("heading", { name: en.firstUse.title }),
    ).toBeVisible();
    const radios = chooser(page).getByRole("radio");
    await expect(radios).toHaveCount(7);
    for (const id of CHOICES) {
      await page.getByTestId(`wb-first-use-${id}`).click();
      await expect(page.getByTestId(`wb-first-use-${id}`)).toHaveAttribute(
        "aria-checked",
        "true",
      );
      const scene = page.getByTestId("wb-scene");
      await expect(scene).toHaveAttribute("data-choice", id);
      await expect(scene.getByRole("img")).toHaveAccessibleName(
        new RegExp(en.firstUse.scenes[id].text.replace(/[.?!()]/g, ".")),
      );
      await expect(scene).toContainText(en.firstUse.scenes[id].app);
      // Every option has its hint for the spotlight.
      expect(en.rust.spotlight.hint[id]).toBeTruthy();
    }
  });

  test("the scene plays: the words, the bar, then the clean text", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub, backend());
    await page.getByTestId("wb-first-use-email").click();
    const text = page.getByTestId("wb-scene-text");
    await expect(text).toHaveText(en.firstUse.scenes.email.text, {
      timeout: 9000,
    });
    // Pause holds the frame.
    await page.getByTestId("wb-scene-pause").click();
    await expect(page.getByTestId("wb-scene-pause")).toHaveText(
      en.firstUse.scene.play,
    );
  });

  test("reduced motion shows the finished scene and no pause", async ({
    page,
    ipc,
    hub,
  }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await open(page, ipc, hub, backend());
    await expect(page.getByTestId("wb-scene-text")).toHaveText(
      en.firstUse.scenes.ai_chat.text,
    );
    await expect(page.getByTestId("wb-scene-pause")).toHaveCount(0);
  });

  test("keyboard only: arrows choose, Try it now saves and starts the hint", async ({
    page,
    ipc,
    hub,
  }) => {
    const b = backend();
    await open(page, ipc, hub, b);
    // Focus starts on the selected option.
    await expect(page.getByTestId("wb-first-use-ai_chat")).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowDown");
    await expect(page.getByTestId("wb-first-use-email")).toBeFocused();
    await expect(page.getByTestId("wb-first-use-email")).toHaveAttribute(
      "aria-checked",
      "true",
    );
    await page.keyboard.press("End");
    await expect(page.getByTestId("wb-first-use-coding")).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(page.getByTestId("wb-first-use-ai_chat")).toBeFocused();
    await page.keyboard.press("ArrowUp");
    await page.keyboard.press("ArrowUp");
    await expect(page.getByTestId("wb-first-use-document")).toBeFocused();
    // Tab: the radio group is one stop; Pause, then the footer buttons.
    const tryNow = page.getByTestId("wb-first-use-try");
    for (
      let i = 0;
      i < 6 && !(await tryNow.evaluate((el) => el === document.activeElement));
      i++
    ) {
      await page.keyboard.press("Tab");
    }
    await expect(tryNow).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(chooser(page)).toHaveCount(0);
    await ipc.expectCalled("fork_set_setting", {
      update: { key: "first_use_choice", value: "document" },
    });
    expect(b.spotlightStarts).toEqual(["document"]);
    // The home shows the hint in its banner.
    const banner = page.getByTestId("wb-banner-first-use.own-apps");
    await expect(banner).toContainText(en.firstUse.banner.title);
    await expect(banner).toContainText(en.rust.spotlight.hint.document);
    await expect(banner).toContainText(en.rust.spotlight.how);
  });

  test("Maybe later closes it for good; Esc too", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub, backend());
    await page.getByTestId("wb-first-use-later").click();
    await expect(chooser(page)).toHaveCount(0);
    expect(hub.forkSettings.dismissed_banners).toContain("first-use.chooser");
    await page.reload();
    await expect(page.getByTestId("wb-sidebar")).toBeVisible();
    await expect(chooser(page)).toHaveCount(0);
  });

  test("Esc closes it like Maybe later", async ({ page, ipc, hub }) => {
    await open(page, ipc, hub, backend());
    await expect(chooser(page)).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(chooser(page)).toHaveCount(0);
    expect(hub.forkSettings.dismissed_banners).toContain("first-use.chooser");
  });

  test("does not open before setup ends or after a choice", async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub, {
      beforeNavigate: () => installBackend(ipc, hub, backend()),
    });
    await expect(chooser(page)).toHaveCount(0);
    await expect(page.getByTestId("wb-checklist")).toHaveCount(0);
    await expect(
      page.getByTestId("wb-banner-hub.dictation.start"),
    ).toBeVisible();
  });

  test("RTL keeps the order and arrow keys", async ({ page, ipc, hub }) => {
    await open(page, ipc, hub, backend(), { handy: RTL_APP_SETTINGS });
    await expect(page.getByTestId("wb-hub")).toHaveAttribute("dir", "rtl");
    await expect(chooser(page)).toBeVisible();
    await page.keyboard.press("ArrowDown");
    await expect(page.getByTestId("wb-first-use-note")).toBeFocused();
  });
});

test.describe("own-apps banner", () => {
  const chosen = { first_use_choice: "email" };

  test("follows the spotlight: Hide the hint, Show the hint again", async ({
    page,
    ipc,
    hub,
  }) => {
    const b = backend();
    await open(page, ipc, hub, b, { fork: chosen });
    await expect(chooser(page)).toHaveCount(0);
    const banner = page.getByTestId("wb-banner-first-use.own-apps");
    await expect(banner).toContainText(en.rust.spotlight.hint.email);
    await ipc.waitForListener("fork-spotlight-state");
    const on: ForkSpotlightState = {
      seq: 3,
      active: true,
      choice: "email",
      hole: { x: 600, y: 860, w: 248, h: 64 },
      edge: "bottom",
    };
    await ipc.emit("fork-spotlight-state", on);
    await expect(banner).toContainText(en.firstUse.banner.active);
    await page.getByTestId("wb-first-use-stop").click();
    await ipc.expectCalled("fork_spotlight_stop");
    await ipc.emit("fork-spotlight-state", { ...on, seq: 4, active: false });
    await page.getByTestId("wb-first-use-again").click();
    await expect.poll(() => b.spotlightStarts).toEqual(["email"]);
    // An older state never wins.
    await ipc.emit("fork-spotlight-state", { ...on, seq: 2 });
    await expect(banner).not.toContainText(en.firstUse.banner.active);
  });

  test("Try something else reopens the chooser on the saved choice", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub, backend(), { fork: chosen });
    await page.getByTestId("wb-first-use-other").click();
    await expect(chooser(page)).toBeVisible();
    await expect(page.getByTestId("wb-first-use-email")).toHaveAttribute(
      "aria-checked",
      "true",
    );
  });

  test("says so when there is no bar to point at", async ({
    page,
    ipc,
    hub,
  }) => {
    const b = backend({ spotlightAvailable: false });
    await open(page, ipc, hub, b, { fork: chosen });
    await page.getByTestId("wb-first-use-again").click();
    await expect(
      page.getByTestId("wb-banner-first-use.own-apps"),
    ).toContainText(en.firstUse.noBar);
  });

  test("uses Control and Option on the fallback profile", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub, backend({ trigger: "ctrl+option" }), {
      fork: chosen,
    });
    await expect(
      page.getByTestId("wb-banner-first-use.own-apps"),
    ).toContainText(en.rust.spotlight.howFallback);
  });

  test("gives way to the promo banner once tried in another app", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub, backend(), {
      fork: { ...chosen, onboarding: completed(["try_in_app"]) },
    });
    await expect(page.getByTestId("wb-banner-first-use.own-apps")).toHaveCount(
      0,
    );
    await expect(
      page.getByTestId("wb-banner-hub.dictation.start"),
    ).toBeVisible();
  });
});

test.describe("setup checklist", () => {
  const chosen = { first_use_choice: "note" };
  const item = (page: Page, id: string) =>
    page.getByTestId(`wb-checklist-${id}`);

  test("lists the four items with progress and the next step open", async ({
    page,
    ipc,
    hub,
  }) => {
    await open(page, ipc, hub, backend(), { fork: chosen });
    const card = page.getByTestId("wb-checklist");
    await expect(card).toBeVisible();
    await expect(card.getByRole("heading")).toHaveText(en.checklist.title);
    await expect(card.getByRole("listitem")).toHaveCount(4);
    await expect(page.getByTestId("wb-checklist-progress")).toHaveText(
      "0 of 4 done",
    );
    await expect(page.getByTestId("wb-checklist-run-try_in_app")).toHaveText(
      en.checklist.items.try_in_app.action,
    );
  });

  test("ticks itself from the app's state and from events", async ({
    page,
    ipc,
    hub,
  }) => {
    const b = backend({ words: 2 });
    await open(page, ipc, hub, b, { fork: chosen });
    await expect(item(page, "add_word")).toHaveAttribute("data-done", "true");
    await ipc.expectCalled("fork_checklist_mark", { id: "add_word" });
    await expect(page.getByTestId("wb-checklist-progress")).toHaveText(
      "1 of 4 done",
    );
    // A snippet is created elsewhere: its event re-checks.
    b.snippets = 1;
    await ipc.waitForListener("fork-snippets-changed");
    await ipc.emit("fork-snippets-changed", {
      counts: { snippets: 1, uses: 0 },
    });
    await expect(item(page, "create_snippet")).toHaveAttribute(
      "data-done",
      "true",
    );
    // The backend ticks "Try it in another app" (first real take).
    await ipc.emit("fork-settings-changed", {
      key: "onboarding",
      settings: {
        ...hub.forkSettings,
        onboarding: completed(["add_word", "create_snippet", "try_in_app"]),
      },
    });
    await expect(item(page, "try_in_app")).toHaveAttribute("data-done", "true");
    await expect(page.getByTestId("wb-checklist-progress")).toHaveText(
      "3 of 4 done",
    );
    await expect(
      page.getByTestId("wb-checklist-run-launch_at_login"),
    ).toBeVisible();
  });

  test("Show me reopens the chooser", async ({ page, ipc, hub }) => {
    await open(page, ipc, hub, backend(), { fork: chosen });
    await page.getByTestId("wb-checklist-run-try_in_app").click();
    await expect(chooser(page)).toBeVisible();
  });

  test("Add a word opens Dictionary with a coach mark, then chains", async ({
    page,
    ipc,
    hub,
  }) => {
    const b = backend();
    await open(page, ipc, hub, b, {
      fork: { ...chosen, onboarding: completed(["try_in_app"]) },
    });
    await page.getByTestId("wb-checklist-run-add_word").click();
    await expect(page.getByTestId("wb-main")).toHaveAttribute(
      "data-page",
      "dictionary",
    );
    const coach = page.locator(".driver-popover.wb-coach");
    await expect(coach).toBeVisible();
    await expect(coach).toContainText(en.checklist.items.add_word.coachTitle);
    await page.keyboard.press("Escape");
    await expect(coach).toHaveCount(0);
    // The word is added: the item ticks, and the next step opens itself.
    b.words = 1;
    await ipc.emit("fork-dictionary-changed", {
      counts: { words: 1, rules: 0, suggested: 0 },
    });
    await expect(item(page, "add_word")).toHaveAttribute("data-done", "true");
    await expect(page.getByTestId("wb-main")).toHaveAttribute(
      "data-page",
      "snippets",
    );
    await expect(coach).toContainText(
      en.checklist.items.create_snippet.coachTitle,
    );
  });

  test("Open at login turns it on and ticks", async ({ page, ipc, hub }) => {
    await open(page, ipc, hub, backend(), {
      fork: {
        ...chosen,
        onboarding: completed(["try_in_app", "add_word", "create_snippet"]),
      },
      handy: { autostart_enabled: false },
    });
    await page.getByTestId("wb-checklist-run-launch_at_login").click();
    await ipc.expectCalled("change_autostart_setting", { enabled: true });
    await expect(page.getByTestId("wb-checklist-all-done")).toHaveText(
      en.checklist.allDone,
    );
  });

  test("hides for good with ✕", async ({ page, ipc, hub }) => {
    await open(page, ipc, hub, backend(), { fork: chosen });
    await page.getByTestId("wb-checklist-hide").click();
    await expect(page.getByTestId("wb-checklist")).toHaveCount(0);
    expect(hub.forkSettings.dismissed_banners).toContain("hub.checklist");
  });

  test("is not in the collapsed sidebar", async ({ page, ipc, hub }) => {
    await open(page, ipc, hub, backend(), { fork: chosen });
    await page.getByTestId("wb-sidebar-toggle").click();
    await expect(page.getByTestId("wb-checklist")).toHaveCount(0);
  });
});

for (const theme of ["light", "dark"] as const) {
  test(`axe: chooser, banner, checklist and coach mark (${theme})`, async ({
    page,
    ipc,
    hub,
  }) => {
    test.setTimeout(180_000);
    await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
    await open(page, ipc, hub, backend(), { handy: { theme } });
    await expect(chooser(page)).toBeVisible();
    await expectNoSeriousAxeFindings(page);
    await page.getByTestId("wb-first-use-try").click();
    await expect(chooser(page)).toHaveCount(0);
    await expect(page.getByTestId("wb-checklist")).toBeVisible();
    await expectNoSeriousAxeFindings(page);
    // The first item is done elsewhere; Add a word is next.
    await ipc.emit("fork-settings-changed", {
      key: "onboarding",
      settings: {
        ...hub.forkSettings,
        first_use_choice: "ai_chat",
        onboarding: completed(["try_in_app"]),
      },
    });
    await page.getByTestId("wb-checklist-run-add_word").click();
    await expect(page.locator(".driver-popover.wb-coach")).toBeVisible();
    await expectNoSeriousAxeFindings(page, { exclude: [".driver-overlay"] });
  });
}

// ---------------------------------------------------------------------------
// The `fork-spotlight` page
// ---------------------------------------------------------------------------

const PAGE = "/src/fork/spotlight/index.html";
const spot = base.extend({});
spot.use({ ipcSetup: { windowLabel: "fork-spotlight" } });

function spotState(over: Partial<ForkSpotlightState> = {}): ForkSpotlightState {
  return {
    seq: 1,
    active: true,
    choice: "email",
    hole: { x: 632, y: 850, w: 248, h: 64 },
    edge: "bottom",
    ...over,
  };
}

async function openSpotlight(
  page: Page,
  ipc: IpcMock,
  state: ForkSpotlightState,
  options: { trigger?: string; handy?: Record<string, unknown> } = {},
) {
  await page.setViewportSize({ width: 1512, height: 982 });
  ipc.respond({
    get_app_settings: { theme: "light", app_language: "en", ...options.handy },
    fork_spotlight_state: state,
    fork_hotkey_status: {
      running: true,
      error: null,
      trigger: options.trigger ?? "fn",
      globe: "do_nothing",
      fn_blocked: false,
      issues: [],
      bindings: [],
    },
  });
  await page.goto(PAGE);
}

spot.describe("spotlight page", () => {
  spot("dims around the bar with the hint above it", async ({ page, ipc }) => {
    await openSpotlight(page, ipc, spotState());
    const root = page.getByTestId("wb-spotlight");
    await expect(root).toHaveAttribute("data-active", "true");
    const hint = page.getByTestId("wb-spotlight-hint");
    await expect(hint).toContainText(en.rust.spotlight.label);
    await expect(hint).toContainText(en.rust.spotlight.hint.email);
    await expect(hint).toContainText(en.rust.spotlight.how);
    await expect(hint).toContainText(en.rust.spotlight.escape);
    const hole = await page.getByTestId("wb-spotlight-hole").boundingBox();
    const box = await hint.boundingBox();
    expect(hole && box).toBeTruthy();
    expect(box!.y + box!.height).toBeLessThanOrEqual(hole!.y);
    expect(
      Math.abs(box!.x + box!.width / 2 - (hole!.x + hole!.width / 2)),
    ).toBeLessThan(2);
    // Nothing on the page takes a click (the window ignores the mouse).
    await expect(page.getByRole("button")).toHaveCount(0);
  });

  spot("puts the hint below a bar at the top", async ({ page, ipc }) => {
    await openSpotlight(
      page,
      ipc,
      spotState({ edge: "top", hole: { x: 632, y: 20, w: 248, h: 64 } }),
    );
    const hole = await page.getByTestId("wb-spotlight-hole").boundingBox();
    const box = await page.getByTestId("wb-spotlight-hint").boundingBox();
    expect(box!.y).toBeGreaterThanOrEqual(hole!.y + hole!.height);
  });

  spot("every choice has its hint", async ({ page, ipc }) => {
    await openSpotlight(page, ipc, spotState({ choice: "ai_chat" }));
    await ipc.waitForListener("fork-spotlight-state");
    let seq = 2;
    for (const choice of CHOICES) {
      await ipc.emit("fork-spotlight-state", spotState({ seq: seq++, choice }));
      await expect(page.getByTestId("wb-spotlight-hint")).toContainText(
        en.rust.spotlight.hint[choice],
      );
    }
  });

  spot(
    "fallback profile wording; clears when it ends",
    async ({ page, ipc }) => {
      await openSpotlight(page, ipc, spotState(), { trigger: "ctrl+option" });
      await expect(page.getByTestId("wb-spotlight-hint")).toContainText(
        en.rust.spotlight.howFallback,
      );
      await ipc.waitForListener("fork-spotlight-state");
      await ipc.emit(
        "fork-spotlight-state",
        spotState({ seq: 5, active: false }),
      );
      await expect(page.getByTestId("wb-spotlight")).toHaveAttribute(
        "data-active",
        "false",
      );
      await expect(page.getByTestId("wb-spotlight-hint")).toHaveCount(0);
    },
  );

  spot("right to left", async ({ page, ipc }) => {
    await openSpotlight(page, ipc, spotState(), { handy: RTL_APP_SETTINGS });
    await expectRtl(page);
    await expect(page.getByTestId("wb-spotlight-hint")).toBeVisible();
  });

  for (const theme of ["light", "dark"] as const) {
    spot(`axe (${theme}, reduced motion)`, async ({ page, ipc }) => {
      spot.setTimeout(180_000);
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      await openSpotlight(page, ipc, spotState(), { handy: { theme } });
      await expect(page.getByTestId("wb-spotlight-hint")).toBeVisible();
      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
        // A page title is not shown anywhere: the window has no title bar.
        .disableRules(["document-title"])
        .analyze();
      const serious = results.violations.filter(
        (v) => v.impact === "serious" || v.impact === "critical",
      );
      expect(serious.map((v) => v.id)).toEqual([]);
    });
  }
});
