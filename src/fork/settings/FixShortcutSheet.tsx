// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Fix sheet (S-02, A-208): what to do when the dictation shortcut stopped
 * working. A drawing of System Settings › Privacy & Security ›
 * Accessibility with the app's switch on, three steps, Open Settings and
 * Check again; under "Still not working?" the reset-and-re-grant recovery
 * for an entry that no longer matches the app after an update (Yap's
 * guide), and "Relaunch to finish" when the grant is back but the listener
 * could only be rebuilt by a new process (RS-02 fallback).
 *
 * The status line is live: the health check rebuilds the shortcut on its
 * own a few seconds after the switch moves, without a relaunch.
 */
import { useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  CheckCircle2,
  ChevronLeft,
  Loader2,
  Mic,
  Minus,
  Plus,
  TriangleAlert,
} from "../ui/icons";
import { Dialog } from "../ui/Dialog";
import { commands, type HealthStatus } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";
import { useShortcutHealth } from "./shortcutHealth";

type StatusKey = "healthy" | "lost" | "recovering" | "needsRelaunch";

export function statusKey(status: HealthStatus | null): StatusKey {
  if (!status || status.state === "healthy") return "healthy";
  if (status.needs_relaunch) return "needsRelaunch";
  return status.state === "recovering" ? "recovering" : "lost";
}

/** The Accessibility pane, drawn (not a screenshot), switch on. */
function AccessibilityPicture() {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <div
      role="img"
      aria-label={t("shortcutHealth.sheet.pictureLabel")}
      data-testid="wb-fix-picture"
      className="overflow-hidden rounded-[10px] border border-(--wb-line) bg-(--wb-surface) text-(--wb-ink) shadow-sm"
    >
      <div aria-hidden className="flex flex-col">
        <div className="flex items-center gap-1.5 border-b border-(--wb-line) px-3 py-2">
          <span className="size-2.5 rounded-full bg-(--wb-line-strong)" />
          <span className="size-2.5 rounded-full bg-(--wb-line-strong)" />
          <span className="size-2.5 rounded-full bg-(--wb-line-strong)" />
          <span className="ms-3 flex items-center text-[12px] text-(--wb-ink-2)">
            <ChevronLeft size={12} />
            {t("shortcutHealth.sheet.pictureBack")}
          </span>
        </div>
        <div className="flex flex-col gap-2 px-4 py-3">
          <span className="text-[13px] font-semibold">
            {t("shortcutHealth.sheet.pictureTitle")}
          </span>
          <div className="flex items-center gap-3 rounded-[8px] border border-(--wb-line) px-3 py-2">
            <span className="grid size-6 place-items-center rounded-[6px] bg-(--wb-ink) text-(--wb-panel)">
              <Mic size={14} />
            </span>
            <span className="flex-1 text-[13px]">
              {t("rust.window.mainTitle")}
            </span>
            <span className="relative inline-flex h-[18px] w-[32px] items-center rounded-full bg-(--wb-ink) outline-2 outline-offset-2 outline-(--wb-accent)">
              <span className="absolute end-[2px] size-[14px] rounded-full bg-white" />
            </span>
          </div>
          <span className="flex gap-2 text-(--wb-ink-3)">
            <Minus size={12} />
            <Plus size={12} />
          </span>
        </div>
      </div>
    </div>
  );
}

export function FixShortcutSheet() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const open = useShortcutHealth((s) => s.sheetOpen);
  const status = useShortcutHealth((s) => s.status);
  const close = useShortcutHealth((s) => s.closeSheet);
  const recheck = useShortcutHealth((s) => s.recheck);
  const [resetFailed, setResetFailed] = useState(false);
  const [busy, setBusy] = useState(false);
  const openRef = useRef<HTMLButtonElement>(null);
  const resetTitleId = useId();
  const key = statusKey(status);
  const permission = status?.reason !== "tap_stopped";

  const run = async (action: () => Promise<unknown>) => {
    setBusy(true);
    try {
      await action();
    } finally {
      setBusy(false);
    }
  };

  const reset = () =>
    run(async () => {
      setResetFailed(false);
      try {
        const r = await commands.forkShortcutResetPermission();
        if (r.status !== "ok") setResetFailed(true);
      } catch {
        setResetFailed(true);
      }
    });

  const StatusIcon =
    key === "healthy"
      ? CheckCircle2
      : key === "recovering"
        ? Loader2
        : TriangleAlert;

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => (o ? undefined : close())}
      title={t("shortcutHealth.sheet.title")}
      description={t(
        permission
          ? "shortcutHealth.sheet.introPermission"
          : "shortcutHealth.sheet.introStopped",
      )}
      closeLabel={t("shortcutHealth.sheet.close")}
      initialFocusRef={openRef as React.RefObject<HTMLElement>}
      contentFades={false}
      footer={
        <>
          <Button
            onClick={() => void run(recheck)}
            disabled={busy}
            data-testid="wb-fix-check"
          >
            {t("shortcutHealth.sheet.checkAgain")}
          </Button>
          <Button onClick={close} data-testid="wb-fix-close">
            {t("shortcutHealth.sheet.close")}
          </Button>
        </>
      }
    >
      <div data-testid="wb-fix-sheet" className="flex flex-col gap-4">
        <p
          role="status"
          data-testid="wb-fix-status"
          data-state={key}
          className="flex items-center gap-2 text-[14px] font-medium"
        >
          <StatusIcon
            size={16}
            aria-hidden
            className={key === "recovering" ? "animate-spin" : undefined}
          />
          {t(`shortcutHealth.sheet.status.${key}`)}
        </p>
        {key === "needsRelaunch" && (
          <Button
            variant="primary"
            onClick={() => void commands.forkShortcutRelaunch()}
            data-testid="wb-fix-relaunch"
          >
            {t("shortcutHealth.sheet.relaunch")}
          </Button>
        )}
        <AccessibilityPicture />
        <section className="flex flex-col gap-2">
          <h3 className="text-[14px] font-semibold">
            {t("shortcutHealth.sheet.stepsTitle")}
          </h3>
          <ol className="flex list-decimal flex-col gap-1 ps-5 text-[13px] leading-5 text-(--wb-ink-2)">
            <li>{t("shortcutHealth.sheet.step1")}</li>
            <li>{t("shortcutHealth.sheet.step2")}</li>
            <li>{t("shortcutHealth.sheet.step3")}</li>
          </ol>
          <div>
            <Button
              ref={openRef}
              variant="primary"
              onClick={() => void commands.forkShortcutOpenAccessibility()}
              data-testid="wb-fix-open-settings"
            >
              {t("shortcutHealth.sheet.openSettings")}
            </Button>
          </div>
        </section>
        <section
          aria-labelledby={resetTitleId}
          className="flex flex-col gap-2 border-t border-(--wb-line) pt-4"
        >
          <h3 id={resetTitleId} className="text-[14px] font-semibold">
            {t("shortcutHealth.sheet.resetTitle")}
          </h3>
          <p className="text-[13px] leading-5 text-(--wb-ink-2)">
            {t("shortcutHealth.sheet.resetBody")}
          </p>
          <ol className="flex list-decimal flex-col gap-1 ps-5 text-[13px] leading-5 text-(--wb-ink-2)">
            <li>{t("shortcutHealth.sheet.resetStep1")}</li>
            <li>{t("shortcutHealth.sheet.resetStep2")}</li>
          </ol>
          <div>
            <Button
              onClick={() => void reset()}
              disabled={busy}
              data-testid="wb-fix-reset"
            >
              {busy && (
                <Loader2 size={14} className="animate-spin" aria-hidden />
              )}
              {t("shortcutHealth.sheet.reset")}
            </Button>
          </div>
          {resetFailed && (
            <p
              role="alert"
              data-testid="wb-fix-reset-failed"
              className="text-[13px] leading-5 text-(--wb-ink)"
            >
              {t("shortcutHealth.sheet.resetFailed")}
            </p>
          )}
        </section>
      </div>
    </Dialog>
  );
}
