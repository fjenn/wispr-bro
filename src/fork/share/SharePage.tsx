// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Share with a friend (S-26b; replaces Wispr's referral and "free month",
 * R2 B-30, C-11). Copy the download link (and the Homebrew line once S-39
 * publishes the cask, `BREW_CASK_PUBLISHED`), or open the macOS share sheet (`fork_share_sheet`, `NSSharingServicePicker`).
 *
 * No tracking: the link is the same for everyone, there is no referral code
 * and the app counts nothing. Copy uses the web clipboard API (a user
 * gesture in our own window), as the transform diff card does.
 */
import { useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, Copy, Heart, Share } from "lucide-react";
import { commands } from "../bindings";
import {
  BREW_CASK_PUBLISHED,
  BREW_COMMAND,
  DOWNLOAD_URL,
} from "../hub/links";
import { PageHeader } from "../hub/PageHeader";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";

type CopyState = "idle" | "copied" | "failed";

/** How long "Copied" stays before the button reads "Copy" again. */
const COPIED_MS = 2000;

function CopyRow({
  id,
  label,
  value,
  copyLabel,
  mono = false,
}: {
  id: string;
  label: string;
  value: string;
  copyLabel: string;
  mono?: boolean;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const fieldId = useId();
  const [state, setState] = useState<CopyState>("idle");
  const timer = useRef<number | undefined>(undefined);
  useEffect(() => () => window.clearTimeout(timer.current), []);

  const copy = async () => {
    window.clearTimeout(timer.current);
    try {
      await navigator.clipboard.writeText(value);
      setState("copied");
    } catch {
      setState("failed");
    }
    timer.current = window.setTimeout(() => setState("idle"), COPIED_MS);
  };

  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={fieldId} className="text-[13px] font-medium">
        {label}
      </label>
      <div className="flex items-center gap-2">
        <input
          id={fieldId}
          readOnly
          value={value}
          onFocus={(e) => e.currentTarget.select()}
          data-testid={`wb-share-${id}`}
          dir="ltr"
          className={`min-w-0 flex-1 rounded-(--wb-radius-control) border border-(--wb-line-strong) bg-(--wb-canvas) px-3 py-2 text-[13px] text-(--wb-ink) ${
            mono ? "font-mono" : ""
          }`}
        />
        <Button
          onClick={() => void copy()}
          aria-label={copyLabel}
          data-testid={`wb-share-copy-${id}`}
        >
          {state === "copied" ? (
            <Check size={14} aria-hidden />
          ) : (
            <Copy size={14} aria-hidden />
          )}
          {state === "copied" ? t("share.copied") : t("share.copy")}
        </Button>
      </div>
      <p
        role="status"
        className="min-h-[1lh] text-[12px] text-(--wb-ink-2)"
        data-testid={`wb-share-status-${id}`}
      >
        {state === "copied" && t("share.copied")}
        {state === "failed" && t("share.copyFailed")}
      </p>
    </div>
  );
}

export function SharePage() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const appName = t("rust.appName");
  const shareRef = useRef<HTMLButtonElement>(null);
  const [shareFailed, setShareFailed] = useState(false);

  const openShareSheet = async () => {
    const rect = shareRef.current?.getBoundingClientRect();
    if (!rect) return;
    setShareFailed(false);
    try {
      const result = await commands.forkShareSheet({
        x: rect.left,
        y: rect.top,
        width: rect.width,
        height: rect.height,
      });
      if (result.status !== "ok") setShareFailed(true);
    } catch {
      setShareFailed(true);
    }
  };

  return (
    <div className="flex flex-col gap-6" data-testid="wb-share">
      <PageHeader title={t("hub.pages.share.title")} />
      <section className="flex flex-col gap-5 rounded-(--wb-radius-card) border border-(--wb-line) bg-(--wb-card) p-6">
        <div className="flex items-start gap-3">
          <Heart
            size={22}
            aria-hidden
            className="mt-1 shrink-0 text-(--wb-accent)"
          />
          <div className="flex flex-col gap-1">
            <h2 className="font-(family-name:--wb-font-display) text-[22px] leading-tight">
              {t("share.heading", { appName })}
            </h2>
            <p className="max-w-[60ch] text-[13.5px] text-(--wb-ink-2)">
              {t("share.body", { appName })}
            </p>
          </div>
        </div>
        <CopyRow
          id="link"
          label={t("share.downloadLabel")}
          value={DOWNLOAD_URL}
          copyLabel={t("share.copyLink")}
        />
        {BREW_CASK_PUBLISHED && (
          <CopyRow
            id="brew"
            label={t("share.brewLabel")}
            value={BREW_COMMAND}
            copyLabel={t("share.copyBrew")}
            mono
          />
        )}
      </section>
      <section className="flex flex-wrap items-center justify-between gap-4 rounded-(--wb-radius-card) border border-(--wb-line) bg-(--wb-card) p-6">
        <div className="flex flex-col gap-1">
          <h2 className="text-[15px] font-semibold">{t("share.shareTitle")}</h2>
          <p className="text-[13px] text-(--wb-ink-2)">
            {t("share.shareBody")}
          </p>
        </div>
        <div className="flex flex-col items-end gap-1">
          <Button
            ref={shareRef}
            variant="primary"
            onClick={() => void openShareSheet()}
            data-testid="wb-share-sheet"
          >
            <Share size={14} aria-hidden />
            {t("share.shareButton")}
          </Button>
          <p
            role="status"
            className="text-[12px] text-(--wb-ink-2)"
            data-testid="wb-share-sheet-status"
          >
            {shareFailed && t("share.shareFailed")}
          </p>
        </div>
      </section>
    </div>
  );
}
