// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Shared setup for the Wispr Bro for iPhone specs: the same mocked backend
 * as the Hub specs (`hub-fixture.ts`), with the OS plugin reporting `ios`,
 * an iPhone-sized touch viewport, and a stand-in for the iOS commands
 * (`ios_session_state`, `ios_setup_state`, …) whose state a spec can change.
 *
 *   import { test, expect, openIos } from "./ios-fixture";
 *   test("…", async ({ page, ipc, hub, ios }) => {
 *     ios.setup.keyboardEnabled = false;
 *     await openIos(page, ipc, hub, ios);
 *   });
 */
import type { Page } from "@playwright/test";
import type { ForkSpeechSetup } from "../../src/fork/bindings";
import type { IosSession, IosSetup } from "../../src/fork/ios/commands";
import type { IpcMock } from "./fixtures";
import {
  test as base,
  expect,
  installHubReplies,
  type HubState,
  type OpenHubOptions,
} from "./hub-fixture";

export { expect };

/** iPhone 15 / 16: 393 × 852 pt at 3×. */
export const IPHONE = { width: 393, height: 852 };
/** Its safe areas (Dynamic Island, home indicator), in pt. */
export const SAFE_AREA = { top: 59, bottom: 34 };

export interface IosState {
  session: IosSession;
  setup: IosSetup;
  speech: ForkSpeechSetup;
  /** Seconds; 0 immediately, -1 never. */
  timeout: number;
  autoReturn: boolean;
  /**
   * `fork_llm_status().state` (cleanup): `unsupported` = an iPhone without
   * the memory for the cleanup model.
   */
  llmState: string;
  /** Bytes of the cleanup model's download so far and in total. */
  llmDownloaded: number;
  llmTotal: number;
}

export function readySpeech(
  over: Partial<ForkSpeechSetup> = {},
): ForkSpeechSetup {
  return {
    phase: "ready",
    percent: 100,
    downloaded_bytes: 0,
    total_bytes: 0,
    problem: null,
    needed_mb: null,
    queued_takes: 0,
    eta_seconds: null,
    starter_languages: ["en"],
    ...over,
  } as ForkSpeechSetup;
}

export const test = base.extend<{ ios: IosState }>({
  ipcSetup: { os: { platform: "ios", os_type: "ios", version: "18.0" } },
  viewport: IPHONE,
  deviceScaleFactor: 3,
  hasTouch: true,
  isMobile: true,
  ios: async ({}, use) => {
    await use({
      session: { state: "noSession" },
      setup: { keyboardEnabled: true, fullAccess: true, micPermission: 2 },
      speech: readySpeech(),
      timeout: 300,
      autoReturn: true,
      llmState: "ready",
      llmDownloaded: 0,
      llmTotal: 1_274_396_096,
    });
  },
});

/** The iOS commands, answered from `ios` (and changing it). */
export function installIosBackend(ipc: IpcMock, ios: IosState): void {
  ipc.handle("ios_session_state", () => ios.session);
  ipc.handle("ios_setup_state", () => ios.setup);
  ipc.handle("ios_start_session", () => {
    ios.session = {
      state: "idle",
      endsAt: ios.timeout > 0 ? Date.now() + ios.timeout * 1000 : null,
    };
    return null;
  });
  ipc.handle("ios_end_session", () => {
    ios.session = { state: "noSession" };
    return null;
  });
  ipc.handle("ios_request_mic", () => {
    ios.setup = { ...ios.setup, micPermission: 2 };
    return null;
  });
  ipc.handle("ios_open_settings", () => null);
  ipc.handle("ios_get_session_timeout", () => ios.timeout);
  ipc.handle("ios_set_session_timeout", (args) => {
    ios.timeout = args.seconds as number;
    return null;
  });
  ipc.handle("ios_get_auto_return", () => ios.autoReturn);
  ipc.handle("ios_set_auto_return", (args) => {
    ios.autoReturn = args.enabled as boolean;
    return null;
  });
  ipc.handle("fork_speech_setup_status", () => ios.speech);
  ipc.handle("fork_speech_setup_refresh", () => null);
  ipc.handle("fork_speech_setup_retry", () => null);
  ipc.handle("fork_llm_status", () => ({
    state: ios.llmState,
    model_name: null,
    downloaded_bytes: ios.llmDownloaded,
    total_bytes: ios.llmTotal,
    server: "stopped",
    error: null,
  }));
  ipc.handle("fork_llm_retry", () => {
    ios.llmState = "downloading";
    return null;
  });
}

export interface OpenIosOptions extends Omit<OpenHubOptions, "size"> {
  /** Draw the iPhone's safe areas (default true). */
  safeArea?: boolean;
}

/** Sets the safe-area insets a real iPhone reports (Chromium reports 0). */
export async function applySafeArea(page: Page): Promise<void> {
  await page.addStyleTag({
    content: `html[data-wb-ios]{--wb-ios-top:${SAFE_AREA.top}px !important;--wb-ios-bottom:${SAFE_AREA.bottom}px !important}`,
  });
}

/** Opens the iPhone app as a returning user and waits for the tab bar. */
export async function openIos(
  page: Page,
  ipc: IpcMock,
  hub: HubState,
  ios: IosState,
  options: OpenIosOptions = {},
): Promise<void> {
  installHubReplies(ipc, hub, {
    ...options,
    beforeNavigate: () => {
      installIosBackend(ipc, ios);
      options.beforeNavigate?.();
    },
  });
  await page.goto(`/${options.hash ?? ""}`, { waitUntil: "load" });
  await page.waitForSelector('[data-testid="wb-ios-tabs"]', {
    state: "visible",
    timeout: 0,
  });
  if (options.safeArea !== false) await applySafeArea(page);
}

/** Opens first-run onboarding on the iPhone at `step`. */
export async function openIosOnboarding(
  page: Page,
  ipc: IpcMock,
  hub: HubState,
  ios: IosState,
  step: string,
  options: OpenIosOptions = {},
): Promise<void> {
  installHubReplies(ipc, hub, {
    ...options,
    handy: { onboarding_completed: false, ...options.handy },
    beforeNavigate: () => {
      installIosBackend(ipc, ios);
      ipc.handle("fork_onboarding_complete", () => ({ status: "completed" }));
      ipc.handle("complete_onboarding", () => {
        hub.handy.onboarding_completed = true;
        return null;
      });
      options.beforeNavigate?.();
    },
  });
  await page.addInitScript((s) => {
    try {
      if (!sessionStorage.getItem("wb.test.seeded")) {
        localStorage.setItem("wb.ios.onboardingStep", s);
        sessionStorage.setItem("wb.test.seeded", "1");
      }
    } catch {
      // storage unavailable
    }
  }, step);
  await page.goto("/", { waitUntil: "load" });
  await page.waitForSelector('[data-testid="wb-onboarding"]', {
    state: "visible",
    timeout: 0,
  });
  if (options.safeArea !== false) await applySafeArea(page);
}
