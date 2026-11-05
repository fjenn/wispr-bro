// SPDX-License-Identifier: GPL-3.0-or-later
// S-36: Settings › Vibe coding. Off until set up (DEC-41); the setup picks
// editors and confirms screen-reader mode; file tags need Cursor or
// Windsurf among them. axe light/dark, RTL and keyboard-only.
import type { Page } from "@playwright/test";
import { test, expect, type IpcMock } from "./fixtures";
import { expectNoSeriousAxeFindings } from "./hub-fixture";
import { forkSettingsDefaults } from "./settings-fixture";
import { openSettingsModal } from "./settings-modal-fixture";

type Vibe = typeof forkSettingsDefaults.vibe_coding & {
  editors: string[];
};

async function openVibe(
  page: Page,
  ipc: IpcMock,
  vibe: Partial<Vibe> = {},
  extra: Record<string, unknown> = {},
) {
  let settings = {
    ...forkSettingsDefaults,
    ...extra,
    vibe_coding: { ...forkSettingsDefaults.vibe_coding, ...vibe },
  };
  await openSettingsModal(page, ipc, "vibe", { fork_get_settings: settings });
  ipc.handle("fork_get_settings", () => settings);
  ipc.handle("fork_set_setting", (args) => {
    const update = args.update as { key: string; value: unknown };
    settings = { ...settings, [update.key]: update.value };
    return settings;
  });
  await expect(page.getByTestId("wb-settings-vibe")).toBeVisible();
}

test("off until set up: Set up walks through editors and screen-reader mode", async ({
  page,
  ipc,
}) => {
  await openVibe(page, ipc);
  const tags = page.getByTestId("wb-vibe-file-tags");
  await expect(tags).toBeDisabled();
  await expect(tags).not.toBeChecked();
  await expect(page.getByTestId("wb-vibe-context-off")).toHaveCount(0);

  await page.getByTestId("wb-vibe-setup-start").click();
  const setup = page.getByTestId("wb-vibe-setup");
  await expect(setup).toBeVisible();
  await expect(page.getByTestId("wb-vibe-steps").locator("li")).toHaveCount(3);
  const finish = page.getByTestId("wb-vibe-finish");
  await expect(finish).toBeDisabled();
  await page.getByTestId("wb-vibe-editor-cursor").check();
  await page.getByTestId("wb-vibe-editor-vscode").check();
  await expect(finish).toBeDisabled();
  await page.getByTestId("wb-vibe-confirm").check();
  await finish.click();

  await ipc.expectCalled("fork_set_setting", {
    update: {
      key: "vibe_coding",
      value: {
        enabled: true,
        setup_completed: true,
        editors: ["vscode", "cursor"],
        file_tags_enabled: false,
      },
    },
  });
  await expect(setup).toHaveCount(0);
  await expect(page.getByTestId("wb-vibe-enabled")).toBeChecked();
  await expect(page.getByTestId("wb-vibe-editors")).toContainText(
    "VS Code, Cursor",
  );
  await expect(page.getByTestId("wb-vibe-change")).toBeFocused();
  await expect(tags).toBeEnabled();
  // The toggle shows the saved value (no optimistic state), so it flips
  // only after the fork_set_setting round trip: click, then wait for it.
  // `check()` asserts the flip right after the click and fails under load.
  await tags.click();
  await ipc.expectCalled("fork_set_setting", {
    update: {
      key: "vibe_coding",
      value: { file_tags_enabled: true, editors: ["vscode", "cursor"] },
    },
  });
  await expect(tags).toBeChecked();
});

test("file tags stay off without a chat editor; Cancel keeps the setup", async ({
  page,
  ipc,
}) => {
  await openVibe(page, ipc, {
    enabled: true,
    setup_completed: true,
    editors: ["vscode"],
  });
  const tags = page.getByTestId("wb-vibe-file-tags");
  await expect(tags).toBeDisabled();
  await expect(page.getByTestId("wb-vibe-files")).toContainText("Cursor");

  await page.getByTestId("wb-vibe-enabled").click(); // saved value, see above
  await ipc.expectCalled("fork_set_setting", {
    update: { key: "vibe_coding", value: { enabled: false } },
  });

  // Cancel keeps the setup as it was.
  await page.getByTestId("wb-vibe-change").click();
  await page.getByTestId("wb-vibe-cancel").click();
  await expect(page.getByTestId("wb-vibe-setup")).toHaveCount(0);
  await expect(page.getByTestId("wb-vibe-change")).toBeFocused();
});

test("changing editors keeps file tags while a chat editor is left", async ({
  page,
  ipc,
}) => {
  await openVibe(page, ipc, {
    enabled: true,
    setup_completed: true,
    editors: ["cursor"],
    file_tags_enabled: true,
  });
  await expect(page.getByTestId("wb-vibe-file-tags")).toBeChecked();
  await page.getByTestId("wb-vibe-change").click();
  await expect(page.getByTestId("wb-vibe-editor-cursor")).toBeChecked();
  await page.getByTestId("wb-vibe-editor-cursor").uncheck();
  await page.getByTestId("wb-vibe-editor-windsurf").check();
  await page.getByTestId("wb-vibe-confirm").check();
  await page.getByTestId("wb-vibe-finish").click();
  await ipc.expectCalled("fork_set_setting", {
    update: {
      key: "vibe_coding",
      value: { editors: ["windsurf"], file_tags_enabled: true },
    },
  });
});

test("says when context awareness is off", async ({ page, ipc }) => {
  await openVibe(page, ipc, {}, { context_awareness: false });
  await expect(page.getByTestId("wb-vibe-context-off")).toBeVisible();
});

test("keyboard-only setup", async ({ page, ipc }) => {
  await openVibe(page, ipc);
  await page.getByTestId("wb-vibe-setup-start").focus();
  await page.keyboard.press("Enter");
  // Focus moves to the setup's heading.
  await expect(page.getByTestId("wb-vibe-setup").locator("h4")).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(page.getByTestId("wb-vibe-editor-vscode")).toBeFocused();
  await page.keyboard.press("Space");
  await page.keyboard.press("Tab");
  await page.keyboard.press("Tab");
  await page.keyboard.press("Tab");
  await expect(page.getByTestId("wb-vibe-confirm")).toBeFocused();
  await page.keyboard.press("Space");
  await page.keyboard.press("Tab");
  await expect(page.getByTestId("wb-vibe-cancel")).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(page.getByTestId("wb-vibe-finish")).toBeFocused();
  await page.keyboard.press("Enter");
  await ipc.expectCalled("fork_set_setting", {
    update: {
      key: "vibe_coding",
      value: { setup_completed: true, editors: ["vscode"] },
    },
  });
  await expect(page.getByTestId("wb-vibe-change")).toBeFocused();
});

test("RTL mirrors the page", async ({ page, ipc }) => {
  await openVibe(page, ipc, {
    enabled: true,
    setup_completed: true,
    editors: ["cursor"],
  });
  await page.getByTestId("wb-settings-modal").evaluate((dialog) => {
    dialog.setAttribute("dir", "rtl");
  });
  const panel = await page.getByTestId("wb-vibe-recognize").boundingBox();
  const change = await page.getByTestId("wb-vibe-change").boundingBox();
  // The button sits at the start (left) edge of its row in RTL.
  expect(panel && change && change.x < panel.x + panel.width / 2).toBe(true);
});

for (const theme of ["light", "dark"] as const) {
  test(`axe: not set up, setup and set up (${theme})`, async ({
    page,
    ipc,
  }) => {
    await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
    await openVibe(page, ipc, {}, { context_awareness: false });
    await expectNoSeriousAxeFindings(page);
    await page.getByTestId("wb-vibe-setup-start").click();
    await page.getByTestId("wb-vibe-editor-windsurf").check();
    await expectNoSeriousAxeFindings(page);
    await page.getByTestId("wb-vibe-confirm").check();
    await page.getByTestId("wb-vibe-finish").click();
    await expect(page.getByTestId("wb-vibe-enabled")).toBeChecked();
    await expectNoSeriousAxeFindings(page);
  });
}
