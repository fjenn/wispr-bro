// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The onboarding stage on a phone: one column with an optional picture,
 * the title, one line, the step's content, and the call to action pinned
 * above the home indicator. `../../onboarding/Layout` hands its split
 * layout to this on iPhone, so shared steps (Languages) need no copy.
 */
import type { ReactNode } from "react";

export function PhoneLayout({
  title,
  subtitle,
  children,
  action,
  secondary,
  art,
  testId,
}: {
  title: ReactNode;
  subtitle?: ReactNode;
  children?: ReactNode;
  /** The pinned call to action (a `Button size="lg" block`). */
  action?: ReactNode;
  /** A quiet second action under it ("Skip for now"). */
  secondary?: ReactNode;
  /** A picture above the title. */
  art?: ReactNode;
  testId?: string;
}) {
  return (
    <div
      className="flex h-full min-h-0 flex-col"
      data-testid="wb-onboarding-stage"
      data-layout="phone"
    >
      <div className="wb-ios-onboarding-scroll">
        <section
          className="flex min-h-full flex-col px-6 pt-2 pb-4"
          data-testid={testId}
        >
          {art && (
            <div
              aria-hidden="true"
              className="mb-6 flex h-[200px] shrink-0 items-center justify-center overflow-hidden rounded-(--wb-radius-xl) bg-(--wb-canvas-warm)"
            >
              {art}
            </div>
          )}
          <h1 className="type-heading-xl m-0 text-balance text-(--wb-ink)">
            {title}
          </h1>
          {subtitle && (
            <p className="mt-2.5 mb-0 text-[17px] leading-[24px] text-pretty text-(--wb-ink-2)">
              {subtitle}
            </p>
          )}
          <div className="mt-6 flex min-h-0 flex-1 flex-col">{children}</div>
        </section>
      </div>
      {(action || secondary) && (
        <div className="wb-ios-onboarding-actions">
          {action}
          {secondary}
        </div>
      )}
    </div>
  );
}
