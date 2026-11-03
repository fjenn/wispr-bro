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
import { Check, Copy, Share } from "../ui/icons";
import { commands } from "../bindings";
import { BREW_CASK_PUBLISHED, BREW_COMMAND, DOWNLOAD_URL } from "../hub/links";
import { PageHeader } from "../hub/PageHeader";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";
import { ForkArt } from "@/assets/fork/art";

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
    <div className="flex flex-col gap-2">
      <label htmlFor={fieldId} className="type-micro text-(--wb-ink-3)">
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
          className={`h-10 min-w-0 flex-1 rounded-(--wb-radius-sm) border border-(--wb-line-strong) bg-(--wb-surface) px-3 text-[14px] text-(--wb-ink) focus-visible:outline-2 focus-visible:outline-offset-0 focus-visible:outline-(--wb-accent) ${
            mono ? "font-mono" : ""
          }`}
        />
        <Button
          size="lg"
          variant="primary"
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
        className="min-h-[1lh] text-[13px] text-(--wb-ink-2)"
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
    <div className="@container flex flex-col gap-6" data-testid="wb-share">
      <PageHeader title={t("hub.pages.share.title")} />
      <div className="grid overflow-hidden rounded-(--wb-radius-lg) border border-(--wb-line) bg-(--wb-surface) @min-[680px]:grid-cols-[minmax(0,1fr)_280px]">
        <div className="flex flex-col gap-8 p-8">
          <section className="flex flex-col gap-6">
            <div className="flex flex-col gap-2">
              <h2 className="type-sheet text-(--wb-ink)">
                {t("share.heading", { appName })}
              </h2>
              <p className="max-w-[52ch] text-[15px] leading-[22px] text-(--wb-ink-2)">
                {t("share.body", { appName })}
              </p>
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
          <section className="flex flex-wrap items-center justify-between gap-4 border-t border-(--wb-line) pt-6">
            <div className="flex min-w-0 flex-col gap-0.5">
              <h2 className="text-[15px] leading-5 font-medium text-(--wb-ink)">
                {t("share.shareTitle")}
              </h2>
              <p className="text-[13px] leading-[18px] text-(--wb-ink-2)">
                {t("share.shareBody")}
              </p>
            </div>
            <div className="flex flex-col items-end gap-1">
              <Button
                ref={shareRef}
                variant="secondary"
                onClick={() => void openShareSheet()}
                data-testid="wb-share-sheet"
              >
                <Share size={16} aria-hidden />
                {t("share.shareButton")}
              </Button>
              <p
                role="status"
                className="text-[13px] text-(--wb-ink-2) empty:hidden"
                data-testid="wb-share-sheet-status"
              >
                {shareFailed && t("share.shareFailed")}
              </p>
            </div>
          </section>
        </div>
        <div
          aria-hidden="true"
          className="hidden place-items-center bg-(--wb-surface-tint) p-6 @min-[680px]:grid"
        >
          <ForkArt name="onboarding-share" width={220} />
        </div>
      </div>
    </div>
  );
}
