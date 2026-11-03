// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Hero banner (S-13, C-23; Design Phase 0): the dark, warm banner at the top
 * of a feature page. A serif headline in white, one sentence at 82 % white,
 * optional example chips, a cream primary button and a quiet text button.
 * `image` puts a photo under a left-to-right dark grade (W7 sources the
 * photos; without one the banner is a warm near-black with a soft glow);
 * over a photo the text keeps to the graded left ~64 % and example chips sit
 * on a dark wash, so they stay legible on bright photos (W2).
 * Each feature page brings its own content (C-29, C-61, C-73, D-206).
 *
 * Dismissal persists in the fork setting `dismissed_banners` under `id`, so a
 * dismissed banner stays gone across windows and relaunches. While the
 * settings load the banner is not shown (no flash of a dismissed banner); if
 * they cannot be read it is shown and still dismisses for this session.
 */
import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettingsStore } from "../settings/store";
import { Button, IconButton } from "./Button";
import { X } from "./icons";

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
  /**
   * Interactive content on the right, read by screen readers (the Style
   * page's app icons and "+"). Shown instead of `art` when both are set.
   */
  aside?: ReactNode;
  /** Background photo URL (bundled asset); graded dark on the left. */
  image?: string;
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
  aside,
  image,
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

  return (
    <Hero
      id={id}
      title={title}
      body={body}
      examples={examples}
      examplesLabel={examplesLabel}
      action={action}
      secondaryAction={secondaryAction}
      live={live}
      art={art}
      aside={aside}
      image={image}
      onDismiss={dismiss}
      dismissLabel={t("ui.banner.dismiss")}
    />
  );
}

export interface HeroProps extends Omit<BannerProps, "id"> {
  /** Used for the test id and the heading's id. */
  id: string;
  /** Shows a dismiss button (Banner wires it to `dismissed_banners`). */
  onDismiss?: () => void;
  dismissLabel?: string;
}

/** The hero banner's look without the dismissal store (pages, gallery). */
export function Hero({
  id,
  title,
  body,
  examples,
  examplesLabel,
  action,
  secondaryAction,
  live = false,
  art,
  aside,
  image,
  onDismiss,
  dismissLabel,
}: HeroProps) {
  const titleId = `wb-banner-${id}-title`;
  return (
    <section
      aria-labelledby={titleId}
      data-testid={`wb-banner-${id}`}
      className="@container/hero relative isolate flex min-h-[180px] items-center gap-6 overflow-hidden rounded-(--wb-radius-lg) bg-(--wb-hero) px-8 py-7 text-(--wb-on-hero)"
    >
      {image ? (
        <>
          <img
            src={image}
            alt=""
            aria-hidden="true"
            className="absolute inset-0 -z-20 size-full object-cover"
          />
          <div
            aria-hidden="true"
            className="absolute inset-0 -z-10 bg-(image:--wb-hero-grade) rtl:-scale-x-100"
          />
        </>
      ) : (
        <div
          aria-hidden="true"
          className="absolute inset-0 -z-10 bg-[radial-gradient(120%_140%_at_100%_0%,rgba(154,114,186,0.28)_0%,rgba(154,114,186,0)_55%),radial-gradient(90%_120%_at_85%_120%,rgba(196,140,90,0.22)_0%,rgba(196,140,90,0)_60%)] rtl:-scale-x-100"
        />
      )}
      <div
        className={`flex min-w-0 flex-1 flex-col gap-2 ${image ? "@min-[560px]/hero:max-w-[64%]" : ""}`}
      >
        <h2 id={titleId} className="type-serif-title pe-8 text-(--wb-on-hero)">
          {title}
        </h2>
        <p
          className="max-w-[60ch] text-[15px] leading-[22px] text-(--wb-on-hero-2)"
          aria-live={live ? "polite" : undefined}
        >
          {body}
        </p>
        {examples && examples.length > 0 && (
          <ul aria-label={examplesLabel} className="mt-1 flex flex-wrap gap-2">
            {examples.map((example) => (
              <li
                key={example}
                className="rounded-full border border-white/20 bg-black/30 px-3 py-1 text-[13px] text-(--wb-on-hero)"
              >
                {example}
              </li>
            ))}
          </ul>
        )}
        {(action || secondaryAction) && (
          <div className="mt-3 flex flex-wrap items-center gap-2">
            {action && (
              <Button
                variant="cream"
                onClick={action.onClick}
                data-testid={action.testId}
              >
                {action.label}
              </Button>
            )}
            {secondaryAction && (
              <Button
                variant="ghost"
                className="text-(--wb-on-hero)! hover:bg-white/10!"
                onClick={secondaryAction.onClick}
                data-testid={secondaryAction.testId}
              >
                {secondaryAction.label}
              </Button>
            )}
          </div>
        )}
      </div>
      {aside ? (
        <div className="flex max-w-[45%] shrink-0 flex-wrap items-center justify-end">
          {aside}
        </div>
      ) : (
        art && (
          <div aria-hidden="true" className="hidden shrink-0 sm:block">
            {art}
          </div>
        )
      )}
      {onDismiss && dismissLabel && (
        <IconButton
          aria-label={dismissLabel}
          round
          size="sm"
          onClick={onDismiss}
          className="absolute end-3 top-3 text-(--wb-on-hero-2)! hover:bg-white/10! hover:text-(--wb-on-hero)!"
        >
          <X size={16} />
        </IconButton>
      )}
    </section>
  );
}
