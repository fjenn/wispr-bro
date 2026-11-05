// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The one way fork code draws an icon (Design Phase 0, SYSTEM-PLAN §3).
 *
 * Lucide (ISC) glyphs at a 1.5 stroke that stays 1.5 px at every size
 * (`absoluteStrokeWidth`), in `currentColor`, at one of the fixed sizes:
 * 14 inline, 16 in rows, 18 in navigation (the default), 20 in the title
 * strip; 12 for tiny badges and 24-48 only for empty-state or hero glyphs.
 *
 * Glyphs come from `./icons` (the wrapped set); `scripts/wb-guards.ts` fails
 * on any other `lucide-react` import under `src/fork/`. A glyph accepts the
 * usual SVG props, but its stroke is always 1.5 and its size snaps to the
 * scale, so the set never mixes again.
 *
 * ```tsx
 * import { Mic } from "../ui/icons";
 * <Mic size={16} />            // a glyph on its own
 * <Icon icon={page.icon} />    // a glyph passed around as data
 * ```
 */
import { forwardRef, type ComponentType, type SVGProps } from "react";
import type { LucideIcon, LucideProps } from "lucide-react";

export const ICON_SIZES = [12, 14, 16, 18, 20, 24, 32, 40, 48] as const;
export type IconSize = (typeof ICON_SIZES)[number];
export const ICON_STROKE = 1.5;

export interface GlyphProps
  extends Omit<SVGProps<SVGSVGElement>, "ref" | "stroke" | "strokeWidth"> {
  size?: IconSize;
  /** Ignored: the stroke is always {@link ICON_STROKE}. */
  strokeWidth?: number | string;
  /** Ignored. */
  absoluteStrokeWidth?: boolean;
  /** Filled variant, for state only (a selected radio, a record dot). */
  filled?: boolean;
}

/** A wrapped glyph (what `./icons` exports). */
export type Glyph = ComponentType<
  GlyphProps & { ref?: React.Ref<SVGSVGElement> }
>;

/** Wraps a Lucide glyph in the fork's stroke and size rules. */
export function glyph(Raw: LucideIcon): Glyph {
  const Wrapped = forwardRef<SVGSVGElement, GlyphProps>(function Glyph(
    {
      size = 18,
      strokeWidth: _ignored,
      absoluteStrokeWidth: _ignoredAbs,
      filled = false,
      ...rest
    },
    ref,
  ) {
    const props: LucideProps = {
      ...(rest as LucideProps),
      size,
      strokeWidth: ICON_STROKE,
      absoluteStrokeWidth: true,
      fill: filled ? "currentColor" : "none",
    };
    if (props["aria-label"] === undefined && props.role === undefined) {
      props["aria-hidden"] = props["aria-hidden"] ?? true;
    }
    return <Raw ref={ref} {...props} />;
  });
  Wrapped.displayName = Raw.displayName ?? "Glyph";
  return Wrapped as Glyph;
}

export interface IconProps extends GlyphProps {
  icon: Glyph;
}

/** Renders a glyph passed as data (a page's icon, a menu item's icon). */
export function Icon({ icon: G, ...rest }: IconProps) {
  return <G {...rest} />;
}
