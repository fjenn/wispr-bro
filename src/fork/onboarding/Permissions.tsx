// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Permissions (S-29a; Design W1 split layout): Microphone and Accessibility
 * as tall outlined rows on the left, each with its Allow button and a tick
 * as soon as macOS reports the grant (Handy's `usePermissions` polls). The
 * right pane shows the pictures for the permission being asked now: the
 * macOS prompt over its System Settings pane (one pair for macOS 13-15 and
 * one for macOS 26).
 *
 * After a row has been asked once and is still not on, a "Reset and ask
 * again" link clears the app's own record (`tccutil`, no password) and asks
 * macOS afresh. macOS keeps a record per signature, so a copy installed over
 * an earlier, differently signed one (a test build) can show its switch on
 * while the switch never counts, and turning it off and on does not help.
 */
import { useState } from "react";
import { Check } from "../ui/icons";
import { useTranslation } from "react-i18next";
import { requestMicrophonePermission } from "tauri-plugin-macos-permissions-api";
import type { Permissions as PermissionState } from "@/components/onboarding/usePermissions";
import { commands } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";
import { SplitLayout } from "./Layout";
import { PermissionArt, type PermissionKind } from "./PermissionArt";

function PermissionRow({
  kind,
  status,
  asked,
  primary,
  onRequest,
  canReset,
  onReset,
}: {
  kind: PermissionKind;
  status: PermissionState["microphone"];
  asked: boolean;
  primary: boolean;
  onRequest: () => void;
  /** Asked once and still not on: offer the reset. */
  canReset: boolean;
  onReset: () => Promise<boolean>;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const granted = status === "granted";
  const waiting = status === "waiting";
  const titleId = `wb-onboarding-${kind}`;
  const [resetState, setResetState] = useState<"idle" | "busy" | "failed">(
    "idle",
  );
  const reset = async () => {
    setResetState("busy");
    setResetState((await onReset()) ? "idle" : "failed");
  };
  return (
    <section
      aria-labelledby={titleId}
      className={`rounded-(--wb-radius-md) border bg-(--wb-surface) px-5 py-4 transition-colors duration-(--wb-dur-fast) ${
        primary && !granted ? "border-(--wb-ink)" : "border-(--wb-line-2)"
      }`}
      data-testid={`wb-permission-${kind}`}
      data-status={status}
    >
      <div className="flex items-center gap-4 text-start">
        <div className="min-w-0 flex-1">
          <h2 id={titleId} className="type-card-title m-0">
            {t(`onboardingFlow.permissions.${kind}.title`)}
          </h2>
          <p className="mt-0.5 mb-0 text-[13px] leading-[18px] text-(--wb-ink-2)">
            {t(`onboardingFlow.permissions.${kind}.why`)}
          </p>
        </div>
        {granted ? (
          <span
            className="inline-flex shrink-0 items-center gap-1.5 text-[13px] font-medium text-(--wb-ink)"
            data-testid={`wb-permission-${kind}-granted`}
          >
            <span className="sr-only">
              {t("onboardingFlow.permissions.allowed")}
            </span>
            <Check size={20} aria-hidden="true" />
          </span>
        ) : (
          <Button
            variant={primary ? "primary" : "secondary"}
            size="sm"
            disabled={status === "checking" || waiting}
            onClick={onRequest}
            aria-describedby={titleId}
            data-testid={`wb-permission-${kind}-allow`}
          >
            {waiting
              ? t("onboardingFlow.permissions.waiting")
              : asked
                ? t("onboardingFlow.permissions.openSettings")
                : t(`onboardingFlow.permissions.${kind}.action`)}
          </Button>
        )}
      </div>
      {waiting && (
        <p
          className="mt-3 mb-0 text-start text-[13px] leading-[18px] text-(--wb-ink-2)"
          role="status"
        >
          {t(`onboardingFlow.permissions.${kind}.waitingHelp`)}
        </p>
      )}
      {canReset && !granted && (
        <p className="mt-2 mb-0 text-start text-[13px] leading-[18px] text-(--wb-ink-2)">
          {t("onboardingFlow.permissions.stuck")}{" "}
          <button
            type="button"
            className="cursor-pointer border-0 bg-transparent p-0 font-medium text-(--wb-ink) underline underline-offset-2 disabled:cursor-default disabled:opacity-60"
            disabled={resetState === "busy"}
            onClick={() => void reset()}
            data-testid={`wb-permission-${kind}-reset`}
          >
            {t("onboardingFlow.permissions.reset")}
          </button>
          {resetState === "failed" && (
            <span
              className="mt-1 block"
              role="alert"
              data-testid={`wb-permission-${kind}-reset-failed`}
            >
              {t("onboardingFlow.permissions.resetFailed")}
            </span>
          )}
        </p>
      )}
    </section>
  );
}

/** The prompt over its System Settings pane, for one permission. */
function ArtPair({ kind, shown }: { kind: PermissionKind; shown: boolean }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <div
      hidden={!shown}
      className="relative w-full max-w-[460px] pe-[8%] pb-[22%]"
      data-testid={`wb-permission-art-${kind}`}
    >
      <figure className="m-0">
        <figcaption className="type-micro mb-2 text-(--wb-ink-3)">
          {t("onboardingFlow.permissions.pane")}
        </figcaption>
        <div className="overflow-hidden rounded-(--wb-radius-md) bg-(--wb-surface) shadow-(--wb-elev-2)">
          <PermissionArt
            kind={kind}
            art="pane"
            label={t(`onboardingFlow.permissions.${kind}.paneAlt`)}
          />
        </div>
      </figure>
      <figure className="absolute end-0 bottom-0 m-0 w-[50%]">
        <div className="overflow-hidden rounded-(--wb-radius-md) bg-(--wb-surface) shadow-(--wb-elev-3)">
          <PermissionArt
            kind={kind}
            art="prompt"
            label={t(`onboardingFlow.permissions.${kind}.promptAlt`)}
          />
        </div>
        <figcaption className="type-micro mt-2 text-end text-(--wb-ink-3)">
          {t("onboardingFlow.permissions.prompt")}
        </figcaption>
      </figure>
    </div>
  );
}

export function Permissions({
  permissions,
  preview,
  onContinue,
}: {
  permissions: PermissionState;
  preview: boolean;
  onContinue: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const mac = permissions.platform === "macos";
  const micGranted = permissions.microphone === "granted";
  // The pictures follow the permission being asked now.
  const active: PermissionKind =
    !micGranted || !mac ? "microphone" : "accessibility";
  const [accessibilityAsked, setAccessibilityAsked] = useState(false);
  // Clears this app's own record, then asks macOS again; the screen's
  // polling (running since the first ask) ticks the row once it is on.
  const reset = async (kind: PermissionKind) => {
    try {
      const r = await commands.forkOnboardingResetPermission(kind);
      if (r.status !== "ok") return false;
      // macOS asks afresh now; Handy's request for the microphone would
      // open System Settings instead (it prompts only once).
      if (kind === "microphone") await requestMicrophonePermission();
      else await permissions.requestAccessibility();
      return true;
    } catch {
      return false;
    }
  };
  return (
    <SplitLayout
      title={t("onboardingFlow.permissions.title")}
      subtitle={t("onboardingFlow.permissions.subtitle")}
      asideLabel={t(`onboardingFlow.permissions.${active}.title`)}
      aside={
        <>
          <ArtPair kind="microphone" shown={active === "microphone"} />
          {mac && (
            <ArtPair kind="accessibility" shown={active === "accessibility"} />
          )}
        </>
      }
      action={
        <Button
          variant="primary"
          size="lg"
          block
          disabled={!preview && !permissions.allGranted}
          onClick={onContinue}
          data-testid="wb-permissions-continue"
        >
          {t("onboardingFlow.continue")}
        </Button>
      }
    >
      <div className="flex flex-col gap-3">
        <PermissionRow
          kind="microphone"
          status={permissions.microphone}
          asked={permissions.microphoneAsked}
          primary
          onRequest={() => void permissions.requestMicrophone()}
          canReset={mac && !preview && permissions.microphoneAsked}
          onReset={() => reset("microphone")}
        />
        {mac && (
          <PermissionRow
            kind="accessibility"
            status={permissions.accessibility}
            asked={false}
            primary={micGranted}
            onRequest={() => {
              setAccessibilityAsked(true);
              void permissions.requestAccessibility();
            }}
            canReset={!preview && accessibilityAsked}
            onReset={() => reset("accessibility")}
          />
        )}
      </div>
    </SplitLayout>
  );
}
