// SPDX-License-Identifier: GPL-3.0-or-later
/** A page's serif title (the page's only `h1`) with optional actions. */
import type { ReactNode } from "react";

export function PageHeader({
  title,
  children,
}: {
  title: string;
  /** Page actions, right-aligned (e.g. Import, Add new). */
  children?: ReactNode;
}) {
  return (
    <div className="flex items-end justify-between gap-4">
      <h1
        data-testid="wb-page-title"
        className="font-(family-name:--wb-font-display) text-[30px] leading-tight font-medium text-(--wb-ink)"
      >
        {title}
      </h1>
      {children && <div className="flex items-center gap-2">{children}</div>}
    </div>
  );
}
