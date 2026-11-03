// SPDX-License-Identifier: GPL-3.0-or-later
import { test, expect, ipcError, NOT_READY } from "./fixtures";
import { expectNoSeriousAxeFindings } from "./hub-fixture";
import { forkSettingsDefaults } from "./settings-fixture";
import {
  chooseOption,
  expectOptions,
  openSettingsModal,
} from "./settings-modal-fixture";

function persistentSettings(ipc: import("./fixtures").IpcMock) {
  let current = structuredClone(forkSettingsDefaults);
  ipc.handle("fork_get_settings", () => current);
  ipc.handle("fork_set_setting", (args) => {
    const update = args.update as { key: string; value: unknown };
    current = { ...current, [update.key]: update.value };
    return current;
  });
}

test("privacy promises name both allowed hosts and every local folder opens", async ({
  page,
  ipc,
}) => {
  await openSettingsModal(page, ipc, "privacy");
  const privacy = page.getByTestId("wb-settings-privacy");
  await expect(privacy).toContainText("huggingface.co");
  await expect(privacy).toContainText("github.com");
  await expect(privacy).toContainText("no account, telemetry, cloud dictation");
  await expect(privacy).not.toContainText("auto-learn");

  await expect(privacy).toContainText(
    "Your dictation and data stay on this Mac.",
  );
  await expect(page.getByTestId("wb-privacy-path-models")).toHaveText(
    "/test/Data/huggingface",
  );
  for (const kind of ["data", "models", "recordings", "logs"]) {
    await expect(page.getByTestId(`wb-privacy-path-${kind}`)).toContainText(
      "/test/Data",
    );
    await page.getByTestId(`wb-privacy-show-${kind}`).click();
    await ipc.expectCalled("fork_show_data_location", { kind });
  }
});

test("retention, context, and update choices persist across reload", async ({
  page,
  ipc,
}) => {
  await openSettingsModal(page, ipc, "privacy");
  persistentSettings(ipc);
  await expect(page.getByTestId("wb-privacy-text-retention")).toHaveText(
    "Forever",
  );
  await expect(page.getByTestId("wb-privacy-audio-retention")).toHaveText(
    "14 days",
  );
  await expectOptions(page, "wb-privacy-text-retention", [
    "Forever",
    "1 year",
    "90 days",
    "30 days",
  ]);
  // Escape closes only the list, never the Settings modal around it.
  await expect(page.getByTestId("wb-settings-modal")).toBeVisible();
  await expectOptions(page, "wb-privacy-audio-retention", [
    "Never keep audio",
    "1 day",
    "7 days",
    "14 days",
    "30 days",
    "90 days",
    "Forever",
  ]);

  await chooseOption(page, "wb-privacy-text-retention", "30");
  await chooseOption(page, "wb-privacy-audio-retention", "0");
  // Controlled toggles change after the backend answers (no optimistic UI).
  await page.getByTestId("wb-privacy-context").click();
  await expect(page.getByTestId("wb-privacy-context")).not.toBeChecked();
  await page.getByTestId("wb-privacy-updates").click();
  await expect(page.getByTestId("wb-privacy-updates")).not.toBeChecked();
  // S-34: auto-learn is off by default (DEC-20) and opts in here.
  await expect(page.getByTestId("wb-privacy-learn")).not.toBeChecked();
  await page.getByTestId("wb-privacy-learn").click();
  await expect(page.getByTestId("wb-privacy-learn")).toBeChecked();
  await ipc.expectCalled("fork_set_setting", {
    update: { key: "auto_learn_enabled", value: true },
  });
  await ipc.expectCalled("fork_set_setting", {
    update: { key: "text_retention_days", value: 30 },
  });
  await ipc.expectCalled("fork_set_setting", {
    update: { key: "audio_retention_days", value: 0 },
  });

  await page.reload();
  await expect(page.getByTestId("wb-privacy-text-retention")).toHaveText(
    "30 days",
  );
  await expect(page.getByTestId("wb-privacy-audio-retention")).toHaveText(
    "Never keep audio",
  );
  await expect(page.getByTestId("wb-privacy-context")).not.toBeChecked();
  await expect(page.getByTestId("wb-privacy-updates")).not.toBeChecked();
  await expect(page.getByTestId("wb-privacy-learn")).toBeChecked();
});

test("export, copy, and save diagnostics use the selected paths", async ({
  page,
  ipc,
}) => {
  await openSettingsModal(page, ipc, "privacy");
  ipc.respond("plugin:dialog|save", "/Users/test/Desktop/my-data");
  await page.getByTestId("wb-privacy-export-button").click();
  await ipc.expectCalled("fork_export_data", {
    path: "/Users/test/Desktop/my-data.zip",
  });
  await expect(
    page.getByText("Export saved and shown in Finder."),
  ).toBeVisible();

  await page.getByTestId("wb-privacy-copy-diagnostics").click();
  await ipc.expectCalled("fork_copy_diagnostics", {});
  await expect(page.getByText("Sanitized diagnostics copied.")).toBeVisible();

  ipc.respond("plugin:dialog|save", "/Users/test/Desktop/diagnostics.zip");
  await page.getByTestId("wb-privacy-save-diagnostics").click();
  await ipc.expectCalled("fork_save_diagnostics", {
    path: "/Users/test/Desktop/diagnostics.zip",
  });
  await expect(page.getByText("Sanitized diagnostics saved.")).toBeVisible();
});

test("delete all requires exact typed confirmation and carries the model choice", async ({
  page,
  ipc,
}) => {
  await openSettingsModal(page, ipc, "privacy");
  await page.getByTestId("wb-privacy-delete-button").click();
  const dialog = page.getByTestId("wb-delete-dialog");
  await expect(dialog).toBeVisible();
  await expect(page.getByTestId("wb-delete-confirmation")).toBeFocused();
  await expect(page.getByTestId("wb-delete-confirm")).toBeDisabled();
  await page.getByTestId("wb-delete-confirmation").fill("delete");
  await expect(page.getByTestId("wb-delete-confirm")).toBeDisabled();
  await page.getByTestId("wb-delete-models").check();
  await page.getByTestId("wb-delete-confirmation").fill("DELETE");
  await expect(page.getByTestId("wb-delete-confirm")).toBeEnabled();
  await page.evaluate(() => localStorage.setItem("wb-probe", "1"));
  await page.getByTestId("wb-delete-confirm").click();
  await ipc.expectCalled("fork_erase_all_data", {
    confirmation: "DELETE",
    deleteModels: true,
  });
  expect(
    await page.evaluate(() => localStorage.getItem("wb-probe")),
  ).toBeNull();
});

test("a failed delete-all reports it and leaves the dialog usable", async ({
  page,
  ipc,
}) => {
  await openSettingsModal(page, ipc, "privacy");
  ipc.respond("fork_erase_all_data", NOT_READY);
  await page.getByTestId("wb-privacy-delete-button").click();
  await page.getByTestId("wb-delete-confirmation").fill("DELETE");
  await page.getByTestId("wb-delete-confirm").click();
  await expect(
    page.getByText("Delete all did not finish.", { exact: false }),
  ).toBeVisible();
  await expect(page.getByTestId("wb-delete-confirm")).toBeEnabled();
  await expect(page.getByTestId("wb-delete-cancel")).toBeEnabled();
});

test("keyboard only: reach Delete all, type the word, confirm", async ({
  page,
  ipc,
}) => {
  await openSettingsModal(page, ipc, "privacy");
  const del = page.getByTestId("wb-privacy-delete-button");
  await del.focus();
  await page.keyboard.press("Enter");
  await expect(page.getByTestId("wb-delete-confirmation")).toBeFocused();
  await page.keyboard.type("DELETE");
  await page.keyboard.press("Shift+Tab");
  await expect(page.getByTestId("wb-delete-models")).toBeFocused();
  await page.keyboard.press("Space");
  await expect(page.getByTestId("wb-delete-models")).toBeChecked();
  await page.keyboard.press("Tab");
  await page.keyboard.press("Tab");
  await page.keyboard.press("Tab");
  await expect(page.getByTestId("wb-delete-confirm")).toBeFocused();
  await page.keyboard.press("Enter");
  await ipc.expectCalled("fork_erase_all_data", {
    confirmation: "DELETE",
    deleteModels: true,
  });
});

test("RTL mirrors the page and keeps paths left-to-right", async ({
  page,
  ipc,
}) => {
  await openSettingsModal(page, ipc, "privacy");
  await page.getByTestId("wb-settings-modal").evaluate((dialog) => {
    dialog.setAttribute("dir", "rtl");
  });
  const panel = await page.getByTestId("wb-privacy-locations").boundingBox();
  const show = await page.getByTestId("wb-privacy-show-data").boundingBox();
  // The Show in Finder button sits at the start (left) edge in RTL.
  expect(panel && show && show.x - panel.x < panel.width / 2).toBe(true);
  await expect(page.getByTestId("wb-privacy-path-data")).toHaveAttribute(
    "dir",
    "ltr",
  );
  await page.getByTestId("wb-privacy-delete-button").click();
  await expect(page.getByTestId("wb-delete-dialog")).toBeVisible();
  expect(
    await page
      .getByTestId("wb-delete-dialog")
      .evaluate((d) => getComputedStyle(d).direction),
  ).toBe("rtl");
});

for (const theme of ["light", "dark"] as const) {
  test(`axe and reduced motion: page and delete dialog (${theme})`, async ({
    page,
    ipc,
  }) => {
    await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
    await openSettingsModal(page, ipc, "privacy");
    await expect(page.getByTestId("wb-privacy-path-data")).toHaveText(
      "/test/Data",
    );
    await expectNoSeriousAxeFindings(page);
    await page.getByTestId("wb-privacy-delete-button").click();
    await expect(page.getByTestId("wb-delete-dialog")).toBeVisible();
    await expectNoSeriousAxeFindings(page);
  });
}

test("delete dialog supports keyboard cancellation and export failures keep data controls usable", async ({
  page,
  ipc,
}) => {
  await openSettingsModal(page, ipc, "privacy");
  await page.getByTestId("wb-privacy-delete-button").click();
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("wb-delete-dialog")).not.toBeVisible();
  await expect(page.getByTestId("wb-settings-modal")).toBeVisible();

  ipc.respond("plugin:dialog|save", "/Users/test/Desktop/data.zip");
  ipc.respond(
    "fork_export_data",
    ipcError({ kind: "Internal", message: "disk full" }),
  );
  await page.getByTestId("wb-privacy-export-button").click();
  await expect(
    page.getByText("Could not export your data. Nothing was deleted."),
  ).toBeVisible();
  await expect(page.getByTestId("wb-privacy-export-button")).toBeEnabled();
});

test("cancelling the save panel invokes no writer", async ({ page, ipc }) => {
  await openSettingsModal(page, ipc, "privacy");
  ipc.respond("plugin:dialog|save", null);
  await page.getByTestId("wb-privacy-export-button").click();
  await ipc.expectCalled("plugin:dialog|save");
  expect(await ipc.calls("fork_export_data")).toHaveLength(0);
  await page.getByTestId("wb-privacy-save-diagnostics").click();
  expect(await ipc.calls("fork_save_diagnostics")).toHaveLength(0);
});
