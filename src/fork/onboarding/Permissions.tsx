// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Permissions (S-29a): Microphone and Accessibility rows, each with a
 * picture of the macOS prompt and of its System Settings pane; a row ticks
 * as soon as macOS reports the grant (Handy's `usePermissions` polls).
 */
import { Check, Keyboard, Mic } from "../ui/icons";
import { useTranslation } from "react-i18next";
import type { Permissions as PermissionState } from "@/components/onboarding/usePermissions";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";
import { PermissionArt, type PermissionKind } from "./PermissionArt";

function PermissionRow({
  kind,
  status,
  asked,
  primary,
  onRequest,
}: {
  kind: PermissionKind;
  status: PermissionState["microphone"];
  asked: boolean;
  primary: boolean;
  onRequest: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const granted = status === "granted";
  const waiting = status === "waiting";
  const titleId = `wb-onboarding-${kind}`;
  return (
    <section
      aria-labelledby={titleId}
      className="rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) p-4"
      data-testid={`wb-permission-${kind}`}
      data-status={status}
    >
      <div className="flex flex-wrap items-center gap-3 text-start">
        <span
          className={`grid size-10 shrink-0 place-items-center rounded-xl ${
            granted
              ? "bg-(--wb-ink) text-(--wb-on-ink)"
              : "bg-(--wb-accent-soft) text-(--wb-accent)"
          }`}
          aria-hidden="true"
        >
          {granted ? (
            <Check size={20} />
          ) : kind === "microphone" ? (
            <Mic size={20} />
          ) : (
            <Keyboard size={20} />
          )}
        </span>
        <div className="min-w-[180px] flex-1">
          <h2 id={titleId} className="m-0 text-sm font-semibold">
            {t(`onboardingFlow.permissions.${kind}.title`)}
          </h2>
          <p className="mt-1 mb-0 text-xs leading-5 text-(--wb-ink-2)">
            {t(`onboardingFlow.permissions.${kind}.why`)}
          </p>
        </div>
        {granted ? (
          <span
            className="inline-flex items-center gap-1.5 text-xs font-semibold text-(--wb-ink)"
            data-testid={`wb-permission-${kind}-granted`}
          >
            <Check
              size={16}
              aria-hidden="true"
              className="text-(--wb-accent)"
            />
            {t("onboardingFlow.permissions.allowed")}
          </span>
        ) : (
          <Button
            variant={primary ? "primary" : "secondary"}
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
      {!granted && (
        <div className="mt-4 grid grid-cols-2 gap-3">
          {(["prompt", "pane"] as const).map((art) => (
            <figure key={art} className="m-0 min-w-0">
              <PermissionArt
                kind={kind}
                art={art}
                label={t(`onboardingFlow.permissions.${kind}.${art}Alt`)}
              />
              <figcaption className="mt-1.5 text-center text-[11px] text-(--wb-ink-3)">
                {t(`onboardingFlow.permissions.${art}`)}
              </figcaption>
            </figure>
          ))}
        </div>
      )}
      {waiting && (
        <p
          className="mt-3 mb-0 text-start text-xs leading-5 text-(--wb-ink-2)"
          role="status"
        >
          {t(`onboardingFlow.permissions.${kind}.waitingHelp`)}
        </p>
      )}
    </section>
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
  return (
    <div className="mx-auto flex w-full max-w-[620px] flex-col items-center gap-5">
      <div className="text-center">
        <h1 className="m-0 font-(--wb-font-display) text-3xl font-semibold text-balance">
          {t("onboardingFlow.permissions.title")}
        </h1>
        <p className="mt-2 mb-0 text-sm leading-6 text-pretty text-(--wb-ink-2)">
          {t("onboardingFlow.permissions.subtitle")}
        </p>
      </div>
      <div className="flex w-full flex-col gap-4">
        <PermissionRow
          kind="microphone"
          status={permissions.microphone}
          asked={permissions.microphoneAsked}
          primary
          onRequest={() => void permissions.requestMicrophone()}
        />
        {mac && (
          <PermissionRow
            kind="accessibility"
            status={permissions.accessibility}
            asked={false}
            primary={micGranted}
            onRequest={() => void permissions.requestAccessibility()}
          />
        )}
      </div>
      <Button
        variant="primary"
        disabled={!preview && !permissions.allGranted}
        onClick={onContinue}
        data-testid="wb-permissions-continue"
      >
        {t("onboardingFlow.continue")}
      </Button>
    </div>
  );
}
