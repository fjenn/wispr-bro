// SPDX-License-Identifier: GPL-3.0-or-later
// S-37a: the accessibility sweep, R5 §5.5. Axe (0 serious/critical, WCAG
// 2.1 AA) on every Hub page, every Settings section (Handy's hosted
// sections included), the Hub's dialogs and popovers and every onboarding
// step, in light, dark and Increase Contrast (`prefers-contrast: more`, both
// themes); a keyboard-only walk of every Hub page, the Settings modal and
// onboarding's first screen (every Tab stop has a name and a visible focus
// ring); Reduce Motion (nothing keeps animating: spinners and scenes stand
// still). The per-feature specs keep their own axe checks with data; this
// spec is the inventory. Mocked IPC; see hub-fixture.ts and
// onboarding-fixture.ts.
import AxeBuilder from "@axe-core/playwright";
import type { Page } from "@playwright/test";
import { HUB_PAGE_IDS } from "../../src/fork/hub/nav";
import { test, expect, openHub, type HubState } from "./hub-fixture";
import type { IpcMock } from "./fixtures";
import { openOnboarding, STEPS } from "./onboarding-fixture";

type Theme = "light" | "dark";
interface Look {
  name: string;
  theme: Theme;
  contrast: "no-preference" | "more";
}
const LOOKS: Look[] = [
  { name: "light", theme: "light", contrast: "no-preference" },
  { name: "dark", theme: "dark", contrast: "no-preference" },
  { name: "light, more contrast", theme: "light", contrast: "more" },
  { name: "dark, more contrast", theme: "dark", contrast: "more" },
];

const SETTINGS_SECTIONS = [
  "general",
  "system",
  "vibe",
  "privacy",
  "advanced",
  "about",
  "hotkeys",
] as const;

interface Finding {
  where: string;
  id: string;
  help: string;
  nodes: string[];
}

/** Under Reduce Motion nothing may still be animating after a moment. */
async function expectStill(page: Page, where: string): Promise<void> {
  const deadline = Date.now() + 3_000;
  let running: string[] = [];
  do {
    running = await page.evaluate(() =>
      document
        .getAnimations()
        .filter((a) => a.playState === "running")
        .map((a) => {
          const el = (a.effect as KeyframeEffect | null)?.target as
            | Element
            | null
            | undefined;
          const name =
            (a as CSSAnimation).animationName ??
            (a as CSSTransition).transitionProperty ??
            "animation";
          return `${name} on ${el?.tagName.toLowerCase() ?? "?"}.${el?.getAttribute("class")?.split(" ").slice(0, 3).join(".") ?? ""}`;
        }),
    );
    if (running.length === 0) return;
    await page.waitForTimeout(100);
  } while (Date.now() < deadline);
  expect(running, `${where}: still animating under Reduce Motion`).toEqual([]);
}

/** Axe over the whole document, Handy's hosted sections included. */
async function axe(page: Page, where: string, out: Finding[]): Promise<void> {
  await expectStill(page, where);
  const results = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
    .analyze();
  for (const v of results.violations) {
    if (v.impact !== "serious" && v.impact !== "critical") continue;
    out.push({
      where,
      id: v.id,
      help: v.help,
      nodes: v.nodes.map((n) => n.target.join(" ")).slice(0, 5),
    });
  }
}

async function look(page: Page, l: Look): Promise<void> {
  await page.emulateMedia({
    colorScheme: l.theme,
    contrast: l.contrast,
    reducedMotion: "reduce",
  });
}

async function hub(
  page: Page,
  ipc: IpcMock,
  state: HubState,
  l: Look,
): Promise<void> {
  await look(page, l);
  await openHub(page, ipc, state, { handy: { theme: l.theme } });
}

async function openSection(page: Page, section: string): Promise<void> {
  await page.evaluate((s) => {
    window.location.hash = `#/hub/settings/${s}`;
  }, section);
  await expect(page.getByTestId("wb-settings-modal")).toBeVisible();
  await page.waitForTimeout(150);
}

/**
 * Tabs through the page (up to `max` stops, or until focus comes back to
 * the first stop) and returns every stop without an accessible name or a
 * visible focus indicator (outline or box-shadow ring).
 */
async function tabWalk(page: Page, where: string, max = 80): Promise<string[]> {
  await page.evaluate(() => {
    (document.activeElement as HTMLElement | null)?.blur();
    document.body.focus();
  });
  const problems: string[] = [];
  const seen: string[] = [];
  for (let i = 0; i < max; i++) {
    await page.keyboard.press("Tab");
    const stop = await page.evaluate(() => {
      const el = document.activeElement as HTMLElement | null;
      if (!el || el === document.body) return null;
      const s = getComputedStyle(el);
      const ring =
        (s.outlineStyle !== "none" && parseFloat(s.outlineWidth) > 0) ||
        (s.boxShadow !== "none" && s.boxShadow !== "") ||
        // react-select (Handy's Select) rings its control, not its input.
        !!el.closest(".app-select__control--is-focused");
      const labelled =
        el.getAttribute("aria-label") ||
        el.getAttribute("aria-labelledby") ||
        el.getAttribute("title") ||
        (el as HTMLInputElement).labels?.length ||
        (el.innerText ?? "").trim() ||
        el.getAttribute("placeholder");
      const id =
        el.getAttribute("data-testid") ||
        `${el.tagName.toLowerCase()}${el.id ? `#${el.id}` : ""}:${(el.getAttribute("aria-label") ?? el.innerText ?? "").trim().slice(0, 30)}`;
      return { id, ring, labelled: !!labelled };
    });
    if (!stop) continue;
    if (seen.includes(stop.id) && seen[0] === stop.id) break;
    seen.push(stop.id);
    if (!stop.ring) problems.push(`${where}: no focus ring on ${stop.id}`);
    if (!stop.labelled) problems.push(`${where}: no name on ${stop.id}`);
  }
  expect(seen.length, `${where}: Tab reaches controls`).toBeGreaterThan(0);
  return problems;
}

test.describe.configure({ timeout: 180_000 });

for (const l of LOOKS) {
  test.describe(`axe sweep (${l.name})`, () => {
    test("every Hub page", async ({ page, ipc, hub: state }) => {
      await hub(page, ipc, state, l);
      const found: Finding[] = [];
      for (const id of HUB_PAGE_IDS) {
        if (id === "settings") continue;
        await page.getByTestId(`wb-nav-${id}`).click();
        await expect(page.getByTestId(`wb-nav-${id}`)).toHaveAttribute(
          "aria-current",
          "page",
        );
        await axe(page, `page ${id}`, found);
      }
      expect(found).toEqual([]);
    });

    test("every Settings section", async ({ page, ipc, hub: state }) => {
      await hub(page, ipc, state, l);
      const found: Finding[] = [];
      for (const section of SETTINGS_SECTIONS) {
        await openSection(page, section);
        await axe(page, `settings ${section}`, found);
      }
      expect(found).toEqual([]);
    });

    test("Hub dialogs and popovers", async ({ page, ipc, hub: state }) => {
      await hub(page, ipc, state, l);
      const found: Finding[] = [];
      const open = async (
        where: string,
        nav: string | null,
        opener: string,
      ) => {
        if (nav) await page.getByTestId(`wb-nav-${nav}`).click();
        await page.getByTestId(opener).first().click();
        await axe(page, where, found);
        await page.keyboard.press("Escape");
      };
      await open("notifications", null, "wb-notifications-bell");
      await open("help popover", null, "wb-nav-help");
      await open("add word", "dictionary", "wb-dictionary-add");
      await open("dictionary import", "dictionary", "wb-dictionary-import");
      await open("new snippet", "snippets", "wb-snippets-add");
      await open("add app or website", "style", "wb-style-add");
      await page.getByTestId("wb-nav-help").click();
      await page.getByTestId("wb-menu-item-report-problem").click();
      await axe(page, "report a problem", found);
      await page.keyboard.press("Escape");
      expect(found).toEqual([]);
    });

    for (const step of STEPS.filter((s) => s !== "move_to_applications")) {
      test(`onboarding ${step}`, async ({ page, ipc }) => {
        await look(page, l);
        await openOnboarding(page, ipc, {
          step: step === "welcome" ? undefined : step,
          theme: l.theme,
        });
        const found: Finding[] = [];
        await axe(page, `onboarding ${step}`, found);
        expect(found).toEqual([]);
      });
    }
  });
}

test.describe("keyboard only", () => {
  test("every Hub page: each Tab stop is named and shows a focus ring", async ({
    page,
    ipc,
    hub: state,
  }) => {
    await hub(page, ipc, state, LOOKS[0]);
    const problems: string[] = [];
    for (const id of HUB_PAGE_IDS) {
      if (id === "settings") continue;
      // Keyboard only: focus the sidebar item and press Enter.
      await page.getByTestId(`wb-nav-${id}`).focus();
      await page.keyboard.press("Enter");
      await expect(page.getByTestId(`wb-nav-${id}`)).toHaveAttribute(
        "aria-current",
        "page",
      );
      problems.push(...(await tabWalk(page, `page ${id}`)));
    }
    expect(problems).toEqual([]);
  });

  test("every Settings section: named stops, focus rings, Esc closes", async ({
    page,
    ipc,
    hub: state,
  }) => {
    await hub(page, ipc, state, LOOKS[1]);
    const problems: string[] = [];
    for (const section of SETTINGS_SECTIONS) {
      await openSection(page, section);
      problems.push(...(await tabWalk(page, `settings ${section}`, 60)));
    }
    expect(problems).toEqual([]);
    await page.getByTestId("wb-settings-modal").focus();
    await page.keyboard.press("Escape");
    await expect(page.getByTestId("wb-settings-modal")).toBeHidden();
  });

  test("onboarding welcome: named stops and focus rings", async ({
    page,
    ipc,
  }) => {
    await look(page, LOOKS[0]);
    await openOnboarding(page, ipc);
    expect(await tabWalk(page, "onboarding welcome")).toEqual([]);
  });
});

// Mac wave 3, S-37.3: WebKit exposed History Limit's spin buttons as an
// unnamed stepper (AXIncrementor). The fork's Handy a11y layer marks
// Handy's number fields and tokens.css hides their spin buttons. Audit OT2
// removed History Limit from Advanced (Handy's own cleanup deleted History
// behind the fork's retention), so no hosted section has a number field
// today; this keeps any that comes back marked.
test("Handy's number fields: every hosted one is marked", async ({
  page,
  ipc,
  hub: state,
}) => {
  await hub(page, ipc, state, LOOKS[0]);
  for (const section of SETTINGS_SECTIONS) {
    await openSection(page, section);
    const unmarked = await page
      .locator(
        "[data-handy-content] input[type='number']:not([data-wb-no-spinner])",
      )
      .count();
    expect(unmarked, `settings ${section}`).toBe(0);
  }
});
