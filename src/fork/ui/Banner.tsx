// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Dismissible example banner (S-13, C-23): a serif heading, one line of
 * text, optional example chips and an optional action. Each feature page
 * brings its own content (C-29, C-61, C-73, D-206).
 *
 * Dismissal persists in the fork setting `dismissed_banners` under `id`, so a
 * dismissed banner stays gone across windows and relaunches. While the
 * settings load the banner is not shown (no flash of a dismissed banner); if
 * they cannot be read it is shown and still dismisses for this session.
 */
import { useState, type ReactNode } from "react";
import { X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettingsStore } from "../settings/store";
import { Button } from "./Button";

export interface BannerProps {
  /** Stable id stored in `dismissed_banners` (lowercase, `a-z0-9_.-`). */
  id: string;
  title: string;
  body: string;
  /** Short examples shown as chips. */
  examples?: string[];
  /** Label of the examples list, read by screen readers. */
  examplesLabel?: string;
  action?: { label: string; onClick: () => void; testId?: string };
  /** A second, quieter action beside `action` (S-30). */
  secondaryAction?: { label: string; onClick: () => void; testId?: string };
  /** Polite live region for `body` (it changes while shown, S-30). */
  live?: boolean;
  /** Decorative art on the right (hidden from screen readers). */
  art?: ReactNode;
}

export function Banner({
  id,
  title,
  body,
  examples,
  examplesLabel,
  action,
  secondaryAction,
  live = false,
  art,
}: BannerProps) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const status = useForkSettingsStore((s) => s.status);
  const dismissed = useForkSettingsStore((s) =>
    s.settings?.dismissed_banners.includes(id),
  );
  const [hiddenNow, setHiddenNow] = useState(false);

  if (hiddenNow || dismissed) return null;
  if (status === "idle" || status === "loading") return null;

  const dismiss = () => {
    setHiddenNow(true);
    const { settings, setSetting } = useForkSettingsStore.getState();
    if (!settings || settings.dismissed_banners.includes(id)) return;
    void setSetting("dismissed_banners", [...settings.dismissed_banners, id]);
  };

  const titleId = `wb-banner-${id}-title`;
  return (
    <section
      aria-labelledby={titleId}
      data-testid={`wb-banner-${id}`}
      className="relative flex items-center gap-6 overflow-hidden rounded-(--wb-radius-card) bg-(--wb-banner) px-6 py-5 text-(--wb-ink)"
    >
      <div className="flex min-w-0 flex-1 flex-col gap-2">
        <h2
          id={titleId}
          className="pe-8 font-(family-name:--wb-font-display) text-[22px] leading-tight font-medium"
        >
          {title}
        </h2>
        <p
          className="text-[13.5px] leading-5 text-(--wb-ink-2)"
          aria-live={live ? "polite" : undefined}
        >
          {body}
        </p>
        {examples && examples.length > 0 && (
          <ul aria-label={examplesLabel} className="mt-1 flex flex-wrap gap-2">
            {examples.map((example) => (
              <li
                key={example}
                className="rounded-full border border-(--wb-line) bg-(--wb-card) px-3 py-1 text-[12.5px] text-(--wb-ink)"
              >
                {example}
              </li>
            ))}
          </ul>
        )}
        {(action || secondaryAction) && (
          <div className="mt-2 flex flex-wrap gap-2">
            {action && (
              <Button
                variant="primary"
                onClick={action.onClick}
                data-testid={action.testId}
              >
                {action.label}
              </Button>
            )}
            {secondaryAction && (
              <Button
                onClick={secondaryAction.onClick}
                data-testid={secondaryAction.testId}
              >
                {secondaryAction.label}
              </Button>
            )}
          </div>
        )}
      </div>
      {art && (
        <div aria-hidden="true" className="hidden shrink-0 sm:block">
          {art}
        </div>
      )}
      <button
        type="button"
        onClick={dismiss}
        aria-label={t("ui.banner.dismiss")}
        className="absolute end-3 top-3 grid size-7 cursor-pointer place-items-center rounded-full text-(--wb-ink-2) hover:bg-(--wb-hover) hover:text-(--wb-ink) focus-visible:outline-2 focus-visible:outline-(--wb-accent)"
      >
        <X size={16} aria-hidden="true" />
      </button>
    </section>
  );
}
