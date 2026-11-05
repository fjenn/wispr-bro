// SPDX-License-Identifier: GPL-3.0-or-later
// S-26a: native-menu events follow the Hub's real deep links, including Help
// targets and the Dictionary menu's Add/New dialogs.
import {
  test,
  expect,
  expectNoSeriousAxeFindings,
  openHub,
} from "./hub-fixture";

const microphoneList = {
  entries: [
    {
      value: "auto",
      name: "",
      kind: "auto",
      recommended: false,
      wireless: false,
      system_default: false,
      transport: "unknown",
    },
  ],
  selected: "auto",
  follow_system: true,
  in_use: "Built-in Microphone",
  system_default: "Built-in Microphone",
  avoided: null,
};

async function emitMenuAction(
  ipc: Parameters<typeof openHub>[1],
  page: string,
  section: string,
) {
  await ipc.waitForListener("fork-menu-action");
  expect(await ipc.emit("fork-menu-action", { page, section })).toBe(1);
}

test("Help and tray Shortcuts open the single hotkeys panel", async ({
  page,
  ipc,
  hub,
}) => {
  await openHub(page, ipc, hub);
  await emitMenuAction(ipc, "settings", "hotkeys");
  await expect(page.getByTestId("wb-settings-modal")).toBeVisible();
  await expect(page.getByTestId("wb-hotkeys-panel")).toBeVisible();
  await expect(page).toHaveURL(/#\/hub\/settings\/hotkeys$/);
});

test("Help › Show the intro again reruns onboarding from Welcome", async ({
  page,
  ipc,
  hub,
}) => {
  await openHub(page, ipc, hub);
  await ipc.waitForListener("fork-menu-action");
  expect(
    await ipc.emit("fork-menu-action", { page: "onboarding", section: null }),
  ).toBe(1);
  await expect(page.getByTestId("wb-onboarding")).toBeVisible();
  await expect(page.getByTestId("wb-hub")).toHaveCount(0);
  // Starts at Welcome; nothing is erased on the way in.
  await ipc.expectCalled("fork_onboarding_go_to", { step: "welcome" });
  expect(
    (await ipc.commandNames()).filter((c) => /erase|reset|delete/i.test(c)),
  ).toEqual([]);
});

for (const section of ["microphone", "languages"] as const) {
  test(`Help ${section} opens and scrolls General to its target`, async ({
    page,
    ipc,
    hub,
  }) => {
    await openHub(page, ipc, hub, {
      beforeNavigate: () => {
        ipc.respond("fork_mic_list", microphoneList);
      },
    });
    await emitMenuAction(ipc, "settings", section);
    await expect(page.getByTestId("wb-settings-general")).toBeVisible();
    const id =
      section === "microphone" ? "wb-general-mic" : "wb-general-languages";
    await expect(page.locator(`#${id}`)).toBeVisible();
    await expect(page).toHaveURL(new RegExp(`#\/hub\/settings\/${section}$`));
  });
}

test("Dictionary menu Add word opens, closes and repeats its dialog deep link", async ({
  page,
  ipc,
  hub,
}) => {
  await openHub(page, ipc, hub);
  await emitMenuAction(ipc, "dictionary", "add");
  await expect(page.getByTestId("wb-dictionary")).toBeVisible();
  await expect(
    page.getByRole("dialog", { name: "Add to dictionary" }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(
    page.getByRole("dialog", { name: "Add to dictionary" }),
  ).toHaveCount(0);
  await expect(page).toHaveURL(/#\/hub\/dictionary$/);
  await emitMenuAction(ipc, "dictionary", "add");
  await expect(
    page.getByRole("dialog", { name: "Add to dictionary" }),
  ).toBeVisible();
});

test("Dictionary menu New snippet opens, closes and repeats its dialog deep link", async ({
  page,
  ipc,
  hub,
}) => {
  await openHub(page, ipc, hub, {
    beforeNavigate: () => {
      ipc.respond("fork_snippets_list", {
        snippets: [],
        counts: { snippets: 0, uses: 0 },
      });
    },
  });
  await emitMenuAction(ipc, "snippets", "add");
  await expect(page.getByTestId("wb-snippets")).toBeVisible();
  await expect(page.getByRole("dialog", { name: "Add snippet" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog", { name: "Add snippet" })).toHaveCount(
    0,
  );
  await expect(page).toHaveURL(/#\/hub\/snippets$/);
  await emitMenuAction(ipc, "snippets", "add");
  await expect(page.getByRole("dialog", { name: "Add snippet" })).toBeVisible();
});

for (const theme of ["light", "dark"] as const) {
  test(`axe: menu-opened dialogs and Settings targets (${theme})`, async ({
    page,
    ipc,
    hub,
  }) => {
    await page.emulateMedia({ colorScheme: theme });
    await openHub(page, ipc, hub, {
      handy: { theme },
      beforeNavigate: () => {
        ipc.respond("fork_mic_list", microphoneList);
        ipc.respond("fork_snippets_list", {
          snippets: [],
          counts: { snippets: 0, uses: 0 },
        });
      },
    });
    await emitMenuAction(ipc, "dictionary", "add");
    await expect(
      page.getByRole("dialog", { name: "Add to dictionary" }),
    ).toBeVisible();
    await expectNoSeriousAxeFindings(page);
    await page.keyboard.press("Escape");
    await emitMenuAction(ipc, "snippets", "add");
    await expect(
      page.getByRole("dialog", { name: "Add snippet" }),
    ).toBeVisible();
    await expectNoSeriousAxeFindings(page);
    await page.keyboard.press("Escape");
    await emitMenuAction(ipc, "settings", "languages");
    await expect(page.getByTestId("wb-settings-general")).toBeVisible();
    await expectNoSeriousAxeFindings(page);
  });
}

test("keyboard only: a menu-opened dialog takes focus and Escape returns to the page", async ({
  page,
  ipc,
  hub,
}) => {
  await openHub(page, ipc, hub);
  await emitMenuAction(ipc, "dictionary", "add");
  const dialog = page.getByRole("dialog", { name: "Add to dictionary" });
  await expect(dialog).toBeVisible();
  // Focus is inside the dialog, so typing goes straight into it.
  await expect
    .poll(() =>
      page.evaluate(() => !!document.activeElement?.closest("[role=dialog]")),
    )
    .toBe(true);
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(page.getByTestId("wb-dictionary")).toBeVisible();
});

test("RTL: menu deep links open mirrored and stay accessible", async ({
  page,
  ipc,
  hub,
}) => {
  await openHub(page, ipc, hub, { handy: { app_language: "ar" } });
  await expect(page.getByTestId("wb-hub")).toHaveAttribute("dir", "rtl");
  await emitMenuAction(ipc, "dictionary", "add");
  await expect(page.getByRole("dialog")).toBeVisible();
  await expectNoSeriousAxeFindings(page);
  await page.keyboard.press("Escape");
  await emitMenuAction(ipc, "settings", "hotkeys");
  await expect(page.getByTestId("wb-hotkeys-panel")).toBeVisible();
  await expectNoSeriousAxeFindings(page);
});
