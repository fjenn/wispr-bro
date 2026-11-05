// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * A Hub page's header row (Design Phase 0; flowmatch): the page title in sans 28/500
 * (the page's only `h1`; serif is for display moments, not page titles), an
 * optional tag beside it, and page-level actions right-aligned on the same
 * line.
 */
import type { ReactNode } from "react";

export function PageHeader({
  title,
  tag,
  children,
}: {
  title: string;
  /** Beside the title (a Beta tag). */
  tag?: ReactNode;
  /** Page actions, right-aligned (e.g. Import, Add new). */
  children?: ReactNode;
}) {
  return (
    <div className="flex min-h-9 items-center justify-between gap-4">
      <div className="flex min-w-0 items-center gap-2.5">
        <h1
          data-testid="wb-page-title"
          className="type-title truncate text-(--wb-ink)"
        >
          {title}
        </h1>
        {tag}
      </div>
      {children && <div className="flex items-center gap-2">{children}</div>}
    </div>
  );
}
