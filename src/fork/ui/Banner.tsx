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
import { useState, type CSSProperties, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettingsStore } from "../settings/store";
import { motion } from "motion/react";
import { Button, FOCUS_RING, IconButton } from "./Button";
import { ArrowRight, X } from "./icons";
import { useMotionPresets } from "./motion";

/** A clickable suggestion on the banner (a word to add, a snippet to try). */
export interface BannerSuggestion {
  id: string;
  label: string;
  onClick: () => void;
  testId?: string;
}

/** A clickable "cue → text" example (Snippets). */
export interface BannerPair {
  id: string;
  cue: string;
  text: string;
  onClick: () => void;
  testId?: string;
}

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
  /** Clickable chips after the action, e.g. words to add (flowmatch). */
  suggestions?: BannerSuggestion[];
  /** Label of the suggestions list, read by screen readers. */
  suggestionsLabel?: string;
  /** Clickable "cue → text" rows above the action (flowmatch). */
  pairs?: BannerPair[];
  /** Label of the pairs list, read by screen readers. */
  pairsLabel?: string;
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
  /**
   * Where the photo's subject starts, as a fraction of the photo's width
   * from its left edge (OT2-E: the Home mic at 0.58). The photo is then
   * anchored at its start edge, so cropping only ever pushes the subject
   * further from the text, and the text column stops short of it; in RTL
   * the photo is mirrored with the text.
   */
  imageSubjectAt?: number;
}

export function Banner({
  id,
  title,
  body,
  examples,
  examplesLabel,
  action,
  secondaryAction,
  suggestions,
  suggestionsLabel,
  pairs,
  pairsLabel,
  live = false,
  art,
  aside,
  image,
  imageSubjectAt,
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
      suggestions={suggestions}
      suggestionsLabel={suggestionsLabel}
      pairs={pairs}
      pairsLabel={pairsLabel}
      live={live}
      art={art}
      aside={aside}
      image={image}
      imageSubjectAt={imageSubjectAt}
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
  suggestions,
  suggestionsLabel,
  pairs,
  pairsLabel,
  live = false,
  art,
  aside,
  image,
  imageSubjectAt,
  onDismiss,
  dismissLabel,
}: HeroProps) {
  const m = useMotionPresets();
  const chip = `rounded-(--wb-radius-sm) px-3.5 py-1.5 text-[14px] transition-colors duration-(--wb-dur-fast)`;
  const titleId = `wb-banner-${id}-title`;
  // With a known subject the text column ends 24 px before it. Banner
  // photos are 4:1 and the hero is at least 180 px tall, so under
  // `object-cover` anchored at the start the photo is at least
  // max(width, 720 px) wide and the subject starts at least
  // `imageSubjectAt` × that from the start edge. The section is the
  // `hero` container: its width is 100cqw plus the 64 px padding, and the
  // text column starts 32 px in, hence the 56 px.
  const textMax =
    image && imageSubjectAt !== undefined
      ? `calc(${imageSubjectAt} * max(100cqw + 64px, 720px) - 56px)`
      : undefined;
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
            data-subject-at={imageSubjectAt}
            className={`absolute inset-0 -z-20 size-full object-cover ${
              imageSubjectAt !== undefined ? "object-left rtl:-scale-x-100" : ""
            }`}
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
        data-testid={`wb-banner-${id}-text`}
        className={`flex min-w-0 flex-1 flex-col gap-2 ${
          textMax
            ? "max-w-(--wb-hero-text-max)"
            : image
              ? "@min-[560px]/hero:max-w-[64%]"
              : ""
        }`}
        style={
          textMax
            ? ({ "--wb-hero-text-max": textMax } as CSSProperties)
            : undefined
        }
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
        {pairs && pairs.length > 0 && (
          <ul aria-label={pairsLabel} className="mt-2 flex flex-col gap-2">
            {pairs.map((pair) => (
              <li key={pair.id}>
                <motion.button
                  type="button"
                  {...m.press}
                  onClick={pair.onClick}
                  data-testid={pair.testId}
                  className={`group/pair flex max-w-full cursor-pointer items-center gap-2 rounded-(--wb-radius-sm) text-start ${FOCUS_RING}`}
                >
                  <span
                    dir="auto"
                    className={`${chip} shrink-0 bg-white/20 text-(--wb-on-hero) italic group-hover/pair:bg-(--wb-cream) group-hover/pair:text-(--wb-on-cream) group-focus-visible/pair:bg-(--wb-cream) group-focus-visible/pair:text-(--wb-on-cream)`}
                  >
                    “{pair.cue}”
                  </span>
                  <ArrowRight
                    size={16}
                    aria-hidden="true"
                    className="shrink-0 text-(--wb-on-hero-2) rtl:-scale-x-100"
                  />
                  <span
                    dir="auto"
                    className={`${chip} min-w-0 truncate bg-white/20 text-(--wb-on-hero) group-hover/pair:bg-(--wb-cream) group-hover/pair:text-(--wb-on-cream) group-focus-visible/pair:bg-(--wb-cream) group-focus-visible/pair:text-(--wb-on-cream)`}
                  >
                    {pair.text}
                  </span>
                </motion.button>
              </li>
            ))}
          </ul>
        )}
        {(action ||
          secondaryAction ||
          (suggestions && suggestions.length > 0)) && (
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
            {suggestions && suggestions.length > 0 && (
              <ul
                aria-label={suggestionsLabel}
                className="flex flex-wrap items-center gap-2"
                data-testid={`wb-banner-${id}-suggestions`}
              >
                {suggestions.map((s) => (
                  <li key={s.id}>
                    <motion.button
                      type="button"
                      {...m.press}
                      onClick={s.onClick}
                      data-testid={s.testId}
                      dir="auto"
                      className={`${chip} h-9 cursor-pointer bg-white/20 font-medium text-(--wb-on-hero) hover:bg-(--wb-cream) hover:text-(--wb-on-cream) ${FOCUS_RING}`}
                    >
                      {s.label}
                    </motion.button>
                  </li>
                ))}
              </ul>
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
