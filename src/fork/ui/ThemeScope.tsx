// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * A subtree pinned to one theme (Design Phase 0b). `data-wb-theme` on a
 * wrapper already re-points the `--wb-*` tokens for everything inside it
 * (`styles/tokens.css`), but popups render in a portal on `<body>`, outside
 * the wrapper, so they used to follow the window's theme. `ThemeScope` also
 * publishes the pinned theme through context; every portalled primitive
 * (`Popover`, `Menu`, `Select`, `Tooltip`, `Dialog`, `Sheet`) puts it back on
 * its own root with `usePinnedThemeProps()`.
 *
 * Use it for anything shown in a fixed theme regardless of the window: the
 * gallery boards, a dark lesson stage, a light preview inside dark mode.
 */
import {
  createContext,
  forwardRef,
  useContext,
  type HTMLAttributes,
} from "react";

export type PinnedTheme = "light" | "dark";

const PinnedThemeContext = createContext<PinnedTheme | undefined>(undefined);

export interface ThemeScopeProps extends HTMLAttributes<HTMLDivElement> {
  theme: PinnedTheme;
}

export const ThemeScope = forwardRef<HTMLDivElement, ThemeScopeProps>(
  function ThemeScope({ theme, className = "", children, ...rest }, ref) {
    return (
      <PinnedThemeContext.Provider value={theme}>
        <div
          ref={ref}
          data-wb-theme={theme}
          className={`text-(--wb-ink) ${className}`}
          {...rest}
        >
          {children}
        </div>
      </PinnedThemeContext.Provider>
    );
  },
);

/** The theme the nearest `ThemeScope` pins, or `undefined` (follow the window). */
export function usePinnedTheme(): PinnedTheme | undefined {
  return useContext(PinnedThemeContext);
}

/** Spread on a portalled root so it keeps the pinned theme. */
export function usePinnedThemeProps(): { "data-wb-theme"?: PinnedTheme } {
  const theme = usePinnedTheme();
  return theme ? { "data-wb-theme": theme } : {};
}
