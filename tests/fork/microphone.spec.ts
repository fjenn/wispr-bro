// SPDX-License-Identifier: GPL-3.0-or-later
// S-08 Settings › Sound (on the fork debug panel until S-14): the
// microphone list order and labels, the Bluetooth caution and the "skipped
// headset" note, choosing a mic, the device-change refresh, the level meter
// (start, live bars, end of session, errors, stop on unmount), the sound and
// mute switches through Handy's commands, keyboard only, RTL, and axe in
// light and dark (R5 S-08, §5.5, §11.2). Mocked IPC; the rules themselves
// are tested in Rust (`fork::audio::choice`, `fork::commands::audio`).
//
// WB_SHOTS_DIR=<dir> also writes screenshots.
import path from "node:path";
import type { Page } from "@playwright/test";
import type { ForkMicList } from "../../src/fork/bindings";
import { ipcError, type IpcMock } from "./fixtures";
import {
  test,
  expect,
  openHub,
  expectNoSeriousAxeFindings,
  type HubState,
} from "./hub-fixture";

const shotsDir = process.env.WB_SHOTS_DIR;
async function shot(page: Page, name: string) {
  if (!shotsDir) return;
  await page
    .locator('[data-testid="wb-settings-modal"]')
    .screenshot({
      path: path.join(shotsDir, `s08-${name}.png`),
      animations: "disabled",
      timeout: 10_000,
    })
    .catch((e) => console.warn(`screenshot ${name} skipped: ${e}`));
}

function entry(
  value: string,
  over: Partial<ForkMicList["entries"][number]> = {},
): ForkMicList["entries"][number] {
  return {
    value,
    name: value === "auto" ? "" : value,
    kind: value === "auto" ? "auto" : "device",
    recommended: false,
    wireless: false,
    system_default: false,
    transport: "unknown",
    ...over,
  };
}

/** A MacBook with AirPods as the macOS default, never chosen. */
function airpodsList(over: Partial<ForkMicList> = {}): ForkMicList {
  return {
    entries: [
      entry("MacBook Pro Microphone", {
        recommended: true,
        transport: "built_in",
      }),
      entry("auto"),
      entry("Shure MV7", { transport: "usb" }),
      entry("AirPods Pro", {
        wireless: true,
        system_default: true,
        transport: "bluetooth",
      }),
    ],
    selected: "auto",
    follow_system: false,
    in_use: "MacBook Pro Microphone",
    system_default: "AirPods Pro",
    avoided: "AirPods Pro",
    ...over,
  };
}

/** A tiny backend: the list follows `fork_mic_choose`. */
function installMics(ipc: IpcMock, start: ForkMicList = airpodsList()) {
  const state = { list: structuredClone(start) };
  ipc.handle("fork_mic_list", () => state.list);
  ipc.handle("fork_mic_choose", (args) => {
    const value = args.value as string;
    const l = state.list;
    l.selected = value;
    l.follow_system = value === "auto";
    l.avoided = null;
    l.in_use = value === "auto" ? l.system_default : value;
    return l;
  });
  ipc.respond({ fork_mic_monitor_start: 7, fork_mic_monitor_stop: null });
  return state;
}

async function openSound(
  page: Page,
  ipc: IpcMock,
  hub: HubState,
  handy: Record<string, unknown> = {},
) {
  await openHub(page, ipc, hub, {
    hash: "#/hub/settings/general",
    handy: {
      audio_feedback: true,
      sound_theme: "marimba",
      audio_feedback_volume: 1,
      mute_while_recording: false,
      ...handy,
    },
  });
  const mic = page.getByTestId("wb-settings-general");
  await expect(mic.getByTestId("wb-mic-picker")).toBeVisible({
    timeout: 15_000,
  });
  return {
    mic,
    sound: page.getByTestId("wb-system-sounds"),
  };
}

const options = (page: Page) => page.getByTestId("wb-mic-option");

test.describe("Microphone", () => {
  test("lists built-in (recommended), Auto-detect, wired, then Bluetooth with a caution", async ({
    page,
    ipc,
    hub,
  }) => {
    installMics(ipc);
    const { mic } = await openSound(page, ipc, hub);
    await expect(options(page)).toHaveText([
      "MacBook Pro Microphone (recommended)",
      /^Auto-detect \(system setting\)Now: AirPods Pro$/,
      "Shure MV7",
      "AirPods ProBluetooth",
    ]);
    await expect(mic.getByTestId("wb-mic-wireless-tag")).toHaveCount(1);
    await expect(mic.getByTestId("wb-mic-caution")).toHaveText(
      "Wired or built-in mics work best; Bluetooth can drop out.",
    );
    // The radio group is described by the caution.
    const group = mic.getByRole("group", { name: "Microphone to use" });
    await expect(group).toHaveAccessibleDescription(/Bluetooth can drop out/);
    await expect(mic.getByRole("radio", { name: /Auto-detect/ })).toBeChecked();
    await expect(mic.getByTestId("wb-mic-avoided")).toContainText(
      "set to AirPods Pro",
    );
    await expect(mic.getByTestId("wb-mic-in-use")).toHaveText(
      "Dictation listens with: MacBook Pro Microphone",
    );
    await shot(page, "list");
  });

  test("choosing a mic writes it and shows what is in use", async ({
    page,
    ipc,
    hub,
  }) => {
    installMics(ipc);
    const { mic } = await openSound(page, ipc, hub);
    await mic.getByRole("radio", { name: "Shure MV7" }).check();
    await ipc.expectCalled("fork_mic_choose", { value: "Shure MV7" });
    await expect(mic.getByRole("radio", { name: "Shure MV7" })).toBeChecked();
    await expect(mic.getByTestId("wb-mic-in-use")).toHaveText(
      "Dictation listens with: Shure MV7",
    );
    await expect(mic.getByTestId("wb-mic-avoided")).toHaveCount(0);
  });

  test("“Use AirPods Pro anyway” chooses Auto-detect explicitly", async ({
    page,
    ipc,
    hub,
  }) => {
    installMics(ipc);
    const { mic } = await openSound(page, ipc, hub);
    await mic.getByRole("button", { name: "Use AirPods Pro anyway" }).click();
    await ipc.expectCalled("fork_mic_choose", { value: "auto" });
    await expect(mic.getByTestId("wb-mic-in-use")).toHaveText(
      "Dictation listens with: AirPods Pro",
    );
  });

  test("a device change refreshes the list", async ({ page, ipc, hub }) => {
    const state = installMics(ipc, airpodsList({ avoided: null }));
    const { mic } = await openSound(page, ipc, hub);
    await expect(options(page)).toHaveCount(4);
    state.list.entries = state.list.entries.filter(
      (e) => e.value !== "Shure MV7",
    );
    await ipc.waitForListener("fork-mic-devices-changed");
    await ipc.emit("fork-mic-devices-changed", { inputs: 2 });
    await expect(options(page)).toHaveCount(3);
    await expect(mic.getByText("Shure MV7")).toHaveCount(0);
  });

  test("a Mac with no Bluetooth mic shows no caution", async ({
    page,
    ipc,
    hub,
  }) => {
    installMics(
      ipc,
      airpodsList({
        entries: [
          entry("MacBook Air Microphone", {
            recommended: true,
            system_default: true,
            transport: "built_in",
          }),
          entry("auto"),
        ],
        system_default: "MacBook Air Microphone",
        in_use: "MacBook Air Microphone",
        avoided: null,
      }),
    );
    const { mic } = await openSound(page, ipc, hub);
    await expect(options(page)).toHaveCount(2);
    await expect(mic.getByTestId("wb-mic-caution")).toHaveCount(0);
    await expect(mic.getByTestId("wb-mic-avoided")).toHaveCount(0);
  });

  test("a failed list says so and retries", async ({ page, ipc, hub }) => {
    // Fails until the test lets it (React's dev double mount calls twice).
    let fail = true;
    ipc.handle("fork_mic_list", () =>
      fail
        ? ipcError({ kind: "Internal", message: "HAL busy" })
        : airpodsList(),
    );
    await openHub(page, ipc, hub, { hash: "#/hub/settings/general" });
    const mic = page.getByTestId("wb-settings-general");
    await expect(mic.getByRole("alert")).toContainText("HAL busy");
    fail = false;
    await mic.getByRole("button", { name: "Try again" }).click();
    await expect(options(page)).toHaveCount(4);
  });
});

test.describe("Mic test", () => {
  test("shows live bars, then ends with the session", async ({
    page,
    ipc,
    hub,
  }) => {
    installMics(ipc);
    const { mic } = await openSound(page, ipc, hub);
    const meter = mic.getByRole("meter", { name: "Microphone level" });
    await expect(meter).toHaveAttribute("aria-valuetext", "Not testing");
    const button = mic.getByTestId("wb-mic-test");
    await expect(button).toHaveText("Test microphone");
    await button.click();
    await ipc.expectCalled("fork_mic_monitor_start");
    await expect(button).toHaveText("Stop test");
    await expect(button).toHaveAttribute("aria-pressed", "true");
    await ipc.waitForListener("fork-mic-level");
    await ipc.emit("fork-mic-level", {
      session: 7,
      running: true,
      level: 0.5,
      error: null,
    });
    await expect(mic.getByTestId("wb-mic-bars")).toHaveAttribute(
      "data-lit",
      "6",
    );
    await expect(meter).toHaveAttribute("aria-valuenow", "50");
    // Another session's levels are ignored.
    await ipc.emit("fork-mic-level", {
      session: 3,
      running: true,
      level: 1,
      error: null,
    });
    await expect(mic.getByTestId("wb-mic-bars")).toHaveAttribute(
      "data-lit",
      "6",
    );
    await shot(page, "meter");
    await ipc.emit("fork-mic-level", {
      session: 7,
      running: false,
      level: 0,
      error: null,
    });
    await expect(button).toHaveText("Test microphone");
    await expect(mic.getByTestId("wb-mic-bars")).toHaveAttribute(
      "data-lit",
      "0",
    );
  });

  // Owner test 1: the backend sends levels above the room's noise floor;
  // a steady room lights nothing, speech most of the meter, and the lit
  // bars vary per bar (still under Reduce Motion).
  for (const theme of ["light", "dark"] as const) {
    test(`noise, then speech, then silence (${theme})`, async ({
      page,
      ipc,
      hub,
    }) => {
      await page.emulateMedia({ colorScheme: theme });
      installMics(ipc);
      const { mic } = await openSound(page, ipc, hub);
      const bars = mic.getByTestId("wb-mic-bars");
      await mic.getByTestId("wb-mic-test").click();
      await ipc.expectCalled("fork_mic_monitor_start");
      await ipc.waitForListener("fork-mic-level");
      const send = (level: number) =>
        ipc.emit("fork-mic-level", {
          session: 7,
          running: true,
          level,
          error: null,
        });
      for (const level of [0.01, 0.03, 0.02]) await send(level);
      await expect(bars).toHaveAttribute("data-lit", "0");
      await shot(page, `meter-noise-${theme}`);

      const heights = async () =>
        bars
          .locator("span")
          .evaluateAll((els) =>
            els.map((e) => (e as HTMLElement).style.height),
          );
      await send(0.9);
      await expect(bars).toHaveAttribute("data-lit", "11");
      const first = await heights();
      await send(0.92);
      await expect(bars).toHaveAttribute("data-lit", "12");
      const second = await heights();
      expect(second).not.toEqual(first);
      await shot(page, `meter-speech-${theme}`);
      await expectNoSeriousAxeFindings(page);

      for (const level of [0.4, 0.1, 0.02, 0]) await send(level);
      await expect(bars).toHaveAttribute("data-lit", "0");
      await expect(mic.getByRole("meter")).toHaveAttribute(
        "aria-valuenow",
        "0",
      );
    });
  }

  test("Reduce Motion keeps the bars at rest heights", async ({
    page,
    ipc,
    hub,
  }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    installMics(ipc);
    const { mic } = await openSound(page, ipc, hub);
    const bars = mic.getByTestId("wb-mic-bars");
    await mic.getByTestId("wb-mic-test").click();
    await ipc.waitForListener("fork-mic-level");
    const heights = () =>
      bars
        .locator("span")
        .evaluateAll((els) => els.map((e) => (e as HTMLElement).style.height));
    await ipc.emit("fork-mic-level", {
      session: 7,
      running: true,
      level: 0.9,
      error: null,
    });
    await expect(bars).toHaveAttribute("data-lit", "11");
    const first = await heights();
    await ipc.emit("fork-mic-level", {
      session: 7,
      running: true,
      level: 0.88,
      error: null,
    });
    await expect(bars).toHaveAttribute("data-lit", "11");
    expect(await heights()).toEqual(first);
    expect(first[0]).toBe("30%");
  });

  test("Stop ends the session; closing the panel stops a running test", async ({
    page,
    ipc,
    hub,
  }) => {
    installMics(ipc);
    const { mic } = await openSound(page, ipc, hub);
    const button = mic.getByTestId("wb-mic-test");
    await button.click();
    await expect(button).toHaveText("Stop test");
    await button.click();
    await ipc.expectCalled("fork_mic_monitor_stop");
    await button.click();
    await expect(button).toHaveText("Stop test");
    const before = (await ipc.calls("fork_mic_monitor_stop")).length;
    await page.keyboard.press("Escape");
    await expect(page.getByTestId("wb-settings-modal")).toHaveCount(0);
    await expect
      .poll(async () => (await ipc.calls("fork_mic_monitor_stop")).length)
      .toBe(before + 1);
  });

  test("explains a denied microphone", async ({ page, ipc, hub }) => {
    installMics(ipc);
    const { mic } = await openSound(page, ipc, hub);
    await mic.getByTestId("wb-mic-test").click();
    await ipc.waitForListener("fork-mic-level");
    await ipc.emit("fork-mic-level", {
      session: 7,
      running: false,
      level: 0,
      error: "permission_denied",
    });
    await expect(mic.getByTestId("wb-mic-meter-error")).toHaveText(
      /Microphone access is off/,
    );
  });
});

test.describe("Sounds", () => {
  test("switches write Handy's settings and Play previews both sounds", async ({
    page,
    ipc,
    hub,
  }) => {
    installMics(ipc);
    ipc.respond({
      change_audio_feedback_setting: null,
      change_sound_theme_setting: null,
      change_audio_feedback_volume_setting: null,
      change_mute_while_recording_setting: null,
      play_test_sound: null,
    });
    const { sound } = await openSound(page, ipc, hub);
    await page.getByTestId("wb-settings-nav-system").click();
    const enabled = sound.getByRole("checkbox", {
      name: "Play a sound when dictation starts and stops",
    });
    await expect(enabled).toBeChecked();
    const mute = sound.getByRole("checkbox", {
      name: "Mute all audio while dictating",
    });
    await expect(mute).not.toBeChecked();
    await expect(mute).toHaveAccessibleDescription(/same volume/);

    await sound.getByRole("combobox", { name: "Sound" }).selectOption("pop");
    await ipc.expectCalled("change_sound_theme_setting", { theme: "pop" });
    await sound.getByRole("button", { name: "Play" }).click();
    await expect
      .poll(async () =>
        (await ipc.calls("play_test_sound")).map((c) => c.args.soundType),
      )
      .toEqual(["start", "stop"]);
    await mute.check();
    await ipc.expectCalled("change_mute_while_recording_setting", {
      enabled: true,
    });
    await enabled.uncheck();
    await ipc.expectCalled("change_audio_feedback_setting", { enabled: false });
    await expect(sound.getByRole("button", { name: "Play" })).toBeDisabled();
    await shot(page, "sounds");
  });
});

test("keyboard only: pick a mic with the arrows and start the test", async ({
  page,
  ipc,
  hub,
}) => {
  installMics(ipc);
  const { mic } = await openSound(page, ipc, hub);
  await mic.getByRole("radio", { name: /Auto-detect/ }).focus();
  await page.keyboard.press("ArrowDown");
  await ipc.expectCalled("fork_mic_choose", { value: "Shure MV7" });
  await expect(mic.getByRole("radio", { name: "Shure MV7" })).toBeFocused();
  // Tab leaves the group (one stop) and reaches the test button.
  await page.keyboard.press("Tab");
  await expect(mic.getByTestId("wb-mic-test")).toBeFocused();
  await page.keyboard.press("Enter");
  await ipc.expectCalled("fork_mic_monitor_start");
  await page.keyboard.press("Space");
  await ipc.expectCalled("fork_mic_monitor_stop");
});

test("RTL: the list mirrors (radios on the right) and stays accessible", async ({
  page,
  ipc,
  hub,
}) => {
  installMics(ipc);
  const { mic } = await openSound(page, ipc, hub, { app_language: "ar" });
  await page.evaluate(() => {
    document.documentElement.dir = "rtl";
  });
  const radio = mic.getByRole("radio").first();
  const label = options(page).first().locator("span").first();
  const r = await radio.boundingBox();
  const l = await label.boundingBox();
  expect(r && l && r.x > l.x).toBe(true);
  await expectNoSeriousAxeFindings(page);
  await shot(page, "rtl");
});

for (const theme of ["light", "dark"] as const) {
  test(`axe: microphone and sounds (${theme})`, async ({ page, ipc, hub }) => {
    await page.emulateMedia({ colorScheme: theme });
    installMics(ipc);
    const { mic } = await openSound(page, ipc, hub, { theme });
    await expectNoSeriousAxeFindings(page);
    await mic.getByTestId("wb-mic-test").click();
    await ipc.waitForListener("fork-mic-level");
    await ipc.emit("fork-mic-level", {
      session: 7,
      running: false,
      level: 0,
      error: "no_input_device",
    });
    await expect(mic.getByTestId("wb-mic-meter-error")).toBeVisible();
    await expectNoSeriousAxeFindings(page);
    await shot(page, `axe-${theme}`);
  });
}
