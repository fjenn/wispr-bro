// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Move to Applications (S-29a), shown only when the app runs from outside
 * an Applications folder (Downloads, a disk image). The move happens only
 * on the button; the app then reopens from Applications and onboarding
 * resumes here, sees the new place and goes on by itself.
 */
import { useEffect, useState } from "react";
import { ArrowRight, FolderInput } from "../ui/icons";
import { useTranslation } from "react-i18next";
import { commands } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { BrandGlyph, Button } from "../ui";
import { CentredLayout, TestCard } from "./Layout";

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
    <CentredLayout
      title={t("onboardingFlow.move.title")}
      subtitle={t("onboardingFlow.move.body")}
      action={
        <div className="flex w-full max-w-[300px] flex-col gap-2">
          <Button
            variant="primary"
            size="lg"
            block
            disabled={busy}
            onClick={() => void move()}
            data-testid="wb-move-action"
          >
            {busy
              ? t("onboardingFlow.move.moving")
              : t("onboardingFlow.move.action")}
          </Button>
          <Button
            variant="ghost"
            size="lg"
            block
            disabled={busy}
            onClick={onDone}
            data-testid="wb-move-skip"
          >
            {t("onboardingFlow.move.skip")}
          </Button>
        </div>
      }
    >
      <TestCard>
        <div
          aria-hidden="true"
          className="flex items-center justify-center gap-5 rounded-(--wb-radius-sm) bg-(--wb-surface-tint) px-6 py-6"
        >
          <span className="grid size-16 place-items-center rounded-[18px] bg-(--wb-hud) text-(--wb-on-hud) shadow-(--wb-elev-1)">
            <BrandGlyph size={30} />
          </span>
          <ArrowRight
            size={20}
            className="text-(--wb-ink-3) rtl:-scale-x-100"
          />
          <span className="grid size-16 place-items-center rounded-[18px] border border-(--wb-line-2) bg-(--wb-surface) text-(--wb-accent)">
            <FolderInput size={32} />
          </span>
        </div>
        {error && (
          <p
            role="alert"
            className="m-0 text-center text-[13px] leading-[18px] text-(--wb-danger)"
            data-testid="wb-move-error"
          >
            {t("onboardingFlow.move.error")}
          </p>
        )}
      </TestCard>
    </CentredLayout>
  );
}
