// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The onboarding's two screen layouts (Design W1; REFERENCE-SPEC §1
 * "Onboarding window", SYSTEM-PLAN §7 W1):
 *
 * - `SplitLayout` (Welcome, Languages, Permissions, the closing summary):
 *   a left pane on the panel colour with the title, one line, the choices
 *   and the black call to action pinned low (360 pt wide); a right pane on
 *   the warm canvas with a picture, a drawn macOS screen or a live demo.
 * - `CentredLayout` (Move to Applications, the microphone and shortcut
 *   tests, practice, celebration, time saved, typing test): a full-width
 *   warm canvas with the decorative glyph pattern, a centred title and
 *   line, a centred test card and the call to action at the bottom. `tone
 *   "dark"` pins the stage dark in either theme (the reference's Learn
 *   screens); the header above it keeps the window's theme.
 *
 * The flow (`OnboardingFlow`) draws the header, the progress line and the
 * Back link; a screen draws only its stage. Titles take `<em>` for the one
 * emphasised word (`Title`), so translators keep the emphasis.
 */
import type { ReactElement, ReactNode } from "react";
import { Trans } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { artUrl, ForkArt, type ArtName } from "@/assets/fork/art";
import {
  FOCUS_RING,
  ThemeScope,
  useWindowTheme,
  type PinnedTheme,
} from "../ui";

/** Side padding of both layouts; the flow's Back link lines up with it. */
export const STAGE_PAD_X = "clamp(32px, 7vw, 80px)";

/** A title string with an optional `<em>` word, rendered as markup. */
export function Title({
  i18nKey,
  values,
  components,
}: {
  i18nKey: string;
  values?: Record<string, unknown>;
  /** Extra tags the string uses (`<n/>` for a rolling number). */
  components?: Record<string, ReactElement>;
}) {
  return (
    <Trans
      ns={FORK_NAMESPACE}
      i18nKey={i18nKey}
      values={values}
      components={{ em: <em />, ...components }}
    />
  );
}

/** The quiet "Skip" link at the top end of a stage (practice, typing). */
export function SkipLink({
  onClick,
  testId,
  children,
}: {
  onClick: () => void;
  testId: string;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      data-testid={testId}
      style={{ insetInlineEnd: STAGE_PAD_X }}
      className={`absolute top-5 z-10 cursor-pointer rounded-(--wb-radius-sm) px-2 py-1 text-[15px] font-medium text-(--wb-ink-2) transition-colors duration-(--wb-dur-fast) hover:bg-(--wb-fill-hover) hover:text-(--wb-ink) ${FOCUS_RING}`}
    >
      {children}
    </button>
  );
}

export function SplitLayout({
  title,
  subtitle,
  children,
  action,
  aside,
  asideLabel,
  testId,
}: {
  title: ReactNode;
  subtitle?: ReactNode;
  children?: ReactNode;
  /** The pinned call to action (a `Button size="lg" block`). */
  action?: ReactNode;
  /** The right pane: a picture, a drawn screen or a demo. */
  aside: ReactNode;
  /** Names the right pane when it is a region of its own (else hidden). */
  asideLabel?: string;
  testId?: string;
}) {
  return (
    <div
      className="grid h-full min-h-0 grid-cols-[minmax(0,44fr)_minmax(0,56fr)]"
      data-testid="wb-onboarding-stage"
      data-layout="split"
      data-tone="light"
    >
      <section
        className="flex min-h-0 flex-col overflow-y-auto bg-(--wb-panel) pt-[76px] pb-8"
        style={{ paddingInline: STAGE_PAD_X }}
        data-testid={testId}
      >
        <h1 className="type-heading-xl m-0 max-w-[440px] text-balance">
          {title}
        </h1>
        {subtitle && (
          <p className="mt-3 mb-0 max-w-[420px] text-[15px] leading-[22px] text-pretty text-(--wb-ink-2)">
            {subtitle}
          </p>
        )}
        <div className="mt-7 flex min-h-0 max-w-[440px] flex-1 flex-col">
          {children}
        </div>
        {action && <div className="max-w-[360px] shrink-0 pt-6">{action}</div>}
      </section>
      <aside
        aria-label={asideLabel}
        aria-hidden={asideLabel ? undefined : true}
        className="relative flex min-h-0 items-center justify-center overflow-hidden bg-(--wb-canvas-warm) p-8"
      >
        {aside}
      </aside>
    </div>
  );
}

export function CentredLayout({
  title,
  subtitle,
  children,
  action,
  skip,
  tone = "warm",
  align = "center",
  width = 560,
  top = 76,
  testId,
}: {
  title?: ReactNode;
  subtitle?: ReactNode;
  children?: ReactNode;
  /** The bottom call to action (300 pt wide). */
  action?: ReactNode;
  /** A `SkipLink` at the top end. */
  skip?: ReactNode;
  tone?: "warm" | "dark";
  /** `start`: a left-aligned stage (the celebration). */
  align?: "center" | "start";
  /** Width of the content column, in px. */
  width?: number;
  /** Space above the title, in px (less for a tall card). */
  top?: number;
  testId?: string;
}) {
  const windowTheme = useWindowTheme();
  const stage = (
    <div
      className="relative h-full min-h-0 overflow-hidden bg-(--wb-canvas-warm) text-(--wb-ink)"
      data-testid="wb-onboarding-stage"
      data-layout="centred"
      data-tone={tone}
    >
      <StagePattern theme={tone === "dark" ? "dark" : windowTheme} />
      {skip}
      <div className="relative h-full overflow-y-auto">
        <div
          className={`mx-auto flex min-h-full w-full flex-col pb-8 ${
            align === "center" ? "items-center text-center" : "items-start"
          }`}
          style={{ maxWidth: width + 64, paddingInline: 32, paddingTop: top }}
          data-testid={testId}
        >
          {title && (
            <h1 className="type-heading-xl m-0 text-balance">{title}</h1>
          )}
          {subtitle && (
            <p className="mt-3 mb-0 max-w-[520px] text-[15px] leading-[22px] text-pretty text-(--wb-ink-2)">
              {subtitle}
            </p>
          )}
          {children}
          {action && (
            <div className="mt-auto flex w-full shrink-0 justify-center pt-8">
              {action}
            </div>
          )}
        </div>
      </div>
    </div>
  );
  return tone === "dark" ? (
    <ThemeScope theme="dark" className="wb-stage-dark h-full">
      {stage}
    </ThemeScope>
  ) : (
    stage
  );
}

/** The white test card of the centred screens (516 pt in the reference). */
export function TestCard({
  title,
  children,
  footer,
  className = "",
  testId,
}: {
  title?: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  className?: string;
  testId?: string;
}) {
  return (
    <section
      className={`mt-9 flex w-full max-w-[460px] flex-col gap-4 rounded-(--wb-radius-md) bg-(--wb-surface) p-5 text-start shadow-(--wb-elev-2) ${className}`}
      data-testid={testId}
    >
      {title}
      {children}
      {footer && (
        <div className="flex flex-wrap items-center gap-2">{footer}</div>
      )}
    </section>
  );
}

/**
 * The decorative glyph pattern behind centred screens: W7's tile of our
 * logo's rounded bars (`brand/pattern-{light,dark}.svg`, 3-5 % contrast),
 * in the stage's theme. No Wispr Flow shape.
 */
export function StagePattern({ theme }: { theme: PinnedTheme }) {
  return (
    <div
      aria-hidden="true"
      className="pointer-events-none absolute inset-0 bg-[length:720px_720px] bg-[position:-120px_-60px]"
      style={{
        backgroundImage: `url("${theme === "dark" ? artUrl.patternDark : artUrl.patternLight}")`,
      }}
      data-testid="wb-onboarding-pattern"
    />
  );
}

/**
 * A picture slot (Design W1) filled with W7's art (`src/assets/fork/art/`,
 * LICENSE-REGISTER §2a): the onboarding characters and the celebration
 * sticker, inlined so their ink follows the theme. Decorative.
 */
export function ArtSlot({
  name,
  size = "lg",
  className = "",
}: {
  name: ArtName;
  /** `lg` fills a split layout's right pane; `sm` is a sticker. */
  size?: "lg" | "sm";
  className?: string;
}) {
  return (
    <ForkArt
      name={name}
      width={size === "lg" ? "min(82%, 420px)" : 112}
      className={className}
    />
  );
}
