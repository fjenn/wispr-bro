// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Move to Applications (S-29a), shown only when the app runs from outside
 * an Applications folder (Downloads, a disk image). The move happens only
 * on the button; the app then reopens from Applications and onboarding
 * resumes here, sees the new place and goes on by itself.
 */
import { useEffect, useState } from "react";
import { FolderInput } from "lucide-react";
import { useTranslation } from "react-i18next";
import { commands } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";

export function MoveToApplications({
  preview,
  onDone,
}: {
  preview: boolean;
  /** Stable: also called when a resumed run is already in Applications. */
  onDone: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);

  useEffect(() => {
    if (preview) return;
    let alive = true;
    void commands.forkOnboardingAppLocation().then((result) => {
      if (alive && result.status === "ok" && result.data !== "move_needed") {
        onDone();
      }
    });
    return () => {
      alive = false;
    };
  }, [preview, onDone]);

  const move = async () => {
    if (preview) {
      onDone();
      return;
    }
    setBusy(true);
    setError(false);
    const result = await commands.forkOnboardingMoveToApplications();
    // On success the app quits and reopens from Applications.
    if (result.status === "error") {
      console.warn("onboarding: move to Applications failed", result.error);
      setError(true);
      setBusy(false);
    }
  };

  return (
    <div className="mx-auto flex max-w-[540px] flex-col items-center gap-5 text-center">
      <div
        className="grid size-16 place-items-center rounded-2xl bg-(--wb-banner) text-(--wb-accent)"
        aria-hidden="true"
      >
        <FolderInput size={32} />
      </div>
      <div>
        <h1 className="m-0 font-(--wb-font-display) text-3xl font-semibold text-balance">
          {t("onboardingFlow.move.title")}
        </h1>
        <p className="mt-3 mb-0 text-sm leading-6 text-pretty text-(--wb-ink-2)">
          {t("onboardingFlow.move.body")}
        </p>
      </div>
      <p className="m-0 text-xs leading-5 text-(--wb-ink-3)">
        {t("onboardingFlow.move.note")}
      </p>
      {error && (
        <p
          role="alert"
          className="m-0 text-sm text-(--wb-ink)"
          data-testid="wb-move-error"
        >
          {t("onboardingFlow.move.error")}
        </p>
      )}
      <div className="flex flex-wrap justify-center gap-2">
        <Button
          variant="primary"
          disabled={busy}
          onClick={() => void move()}
          data-testid="wb-move-action"
        >
          {busy
            ? t("onboardingFlow.move.moving")
            : t("onboardingFlow.move.action")}
        </Button>
        <Button disabled={busy} onClick={onDone} data-testid="wb-move-skip">
          {t("onboardingFlow.move.skip")}
        </Button>
      </div>
    </div>
  );
}
