// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Wispr Bro art (Design W7, docs/fork/design/SYSTEM-PLAN.md §6). One place
 * that names every bundled picture so pages pick art by slot, not by path.
 * Sources, licences and changes: docs/fork/LICENSE-REGISTER.md §2 "Art".
 *
 * - `photos`: banner and lesson photos, graded warm and dark. WebP and AVIF
 *   at 1x and 2x; `src` is the 2x WebP (the `Banner image` slot takes one
 *   URL), `srcSet`/`avifSrcSet` are there for a `<picture>`.
 * - `ForkArt`: our SVGs inlined, so the characters' ink and the "How it
 *   works" stills follow the app theme through the `--wb-*` tokens (an
 *   `<img>` of the same file renders the light fallback colours).
 *   `--wb-art-fill` / `--wb-art-fill-2` / `--wb-art-ink` recolour a
 *   character locally.
 * - `artUrl`: the same SVGs as URLs, for CSS backgrounds (the pattern).
 */
import type { CSSProperties } from "react";

import dictionary1xAvif from "./photos/banner-dictionary@1x.avif";
import dictionary1x from "./photos/banner-dictionary@1x.webp";
import dictionary2xAvif from "./photos/banner-dictionary@2x.avif";
import dictionary2x from "./photos/banner-dictionary@2x.webp";
import home1xAvif from "./photos/banner-home@1x.avif";
import home1x from "./photos/banner-home@1x.webp";
import home2xAvif from "./photos/banner-home@2x.avif";
import home2x from "./photos/banner-home@2x.webp";
import insights1xAvif from "./photos/banner-insights@1x.avif";
import insights1x from "./photos/banner-insights@1x.webp";
import insights2xAvif from "./photos/banner-insights@2x.avif";
import insights2x from "./photos/banner-insights@2x.webp";
import scratchpad1xAvif from "./photos/banner-scratchpad@1x.avif";
import scratchpad1x from "./photos/banner-scratchpad@1x.webp";
import scratchpad2xAvif from "./photos/banner-scratchpad@2x.avif";
import scratchpad2x from "./photos/banner-scratchpad@2x.webp";
import snippets1xAvif from "./photos/banner-snippets@1x.avif";
import snippets1x from "./photos/banner-snippets@1x.webp";
import snippets2xAvif from "./photos/banner-snippets@2x.avif";
import snippets2x from "./photos/banner-snippets@2x.webp";
import style1xAvif from "./photos/banner-style@1x.avif";
import style1x from "./photos/banner-style@1x.webp";
import style2xAvif from "./photos/banner-style@2x.avif";
import style2x from "./photos/banner-style@2x.webp";
import transforms1xAvif from "./photos/banner-transforms@1x.avif";
import transforms1x from "./photos/banner-transforms@1x.webp";
import transforms2xAvif from "./photos/banner-transforms@2x.avif";
import transforms2x from "./photos/banner-transforms@2x.webp";
import whisper1xAvif from "./photos/lesson-whisper@1x.avif";
import whisper1x from "./photos/lesson-whisper@1x.webp";
import whisper2xAvif from "./photos/lesson-whisper@2x.avif";
import whisper2x from "./photos/lesson-whisper@2x.webp";

import mark from "./brand/mark.svg?raw";
import patternDarkUrl from "./brand/pattern-dark.svg";
import patternLightUrl from "./brand/pattern-light.svg";
import wordmark from "./brand/wordmark.svg?raw";
import emptyDictionary from "./characters/empty-dictionary.svg?raw";
import emptyHistory from "./characters/empty-history.svg?raw";
import emptyInsights from "./characters/empty-insights.svg?raw";
import emptyNotifications from "./characters/empty-notifications.svg?raw";
import emptyScratchpad from "./characters/empty-scratchpad.svg?raw";
import emptySearch from "./characters/empty-search.svg?raw";
import emptySnippets from "./characters/empty-snippets.svg?raw";
import emptySuggested from "./characters/empty-suggested.svg?raw";
import emptyUnavailable from "./characters/empty-unavailable.svg?raw";
import onboardingLanguages from "./characters/onboarding-languages.svg?raw";
import onboardingShare from "./characters/onboarding-share.svg?raw";
import onboardingWelcome from "./characters/onboarding-welcome.svg?raw";
import howScratchpad from "./how/scratchpad.svg?raw";
import howTransforms from "./how/transforms.svg?raw";
import howWhisper from "./how/whisper.svg?raw";
import stickerArrow from "./stickers/arrow.svg?raw";
import stickerRocket from "./stickers/rocket.svg?raw";

export interface Photo {
  /** 2x WebP: what the `Banner image` slot gets. */
  src: string;
  srcSet: string;
  avifSrcSet: string;
  /** Pixel size of the 2x file (the 1x file is half). */
  width: number;
  height: number;
}

function photo(
  x1: string,
  x2: string,
  a1: string,
  a2: string,
  width: number,
  height: number,
): Photo {
  return {
    src: x2,
    srcSet: `${x1} 1x, ${x2} 2x`,
    avifSrcSet: `${a1} 1x, ${a2} 2x`,
    width,
    height,
  };
}

/** Banner photos are 4:1 (1920 × 480 at 2x); the lesson photo is 16:9. */
export const photos = {
  home: photo(home1x, home2x, home1xAvif, home2xAvif, 1920, 480),
  scratchpad: photo(
    scratchpad1x,
    scratchpad2x,
    scratchpad1xAvif,
    scratchpad2xAvif,
    1920,
    480,
  ),
  transforms: photo(
    transforms1x,
    transforms2x,
    transforms1xAvif,
    transforms2xAvif,
    1920,
    480,
  ),
  style: photo(style1x, style2x, style1xAvif, style2xAvif, 1920, 480),
  dictionary: photo(
    dictionary1x,
    dictionary2x,
    dictionary1xAvif,
    dictionary2xAvif,
    1920,
    480,
  ),
  snippets: photo(
    snippets1x,
    snippets2x,
    snippets1xAvif,
    snippets2xAvif,
    1920,
    480,
  ),
  insights: photo(
    insights1x,
    insights2x,
    insights1xAvif,
    insights2xAvif,
    1920,
    480,
  ),
  lessonWhisper: photo(
    whisper1x,
    whisper2x,
    whisper1xAvif,
    whisper2xAvif,
    1280,
    720,
  ),
} as const satisfies Record<string, Photo>;

const SVG = {
  mark,
  wordmark,
  "onboarding-welcome": onboardingWelcome,
  "onboarding-languages": onboardingLanguages,
  "onboarding-share": onboardingShare,
  "empty-history": emptyHistory,
  "empty-dictionary": emptyDictionary,
  "empty-suggested": emptySuggested,
  "empty-snippets": emptySnippets,
  "empty-insights": emptyInsights,
  "empty-notifications": emptyNotifications,
  "empty-search": emptySearch,
  "empty-unavailable": emptyUnavailable,
  "empty-scratchpad": emptyScratchpad,
  "how-transforms": howTransforms,
  "how-whisper": howWhisper,
  "how-scratchpad": howScratchpad,
  "sticker-arrow": stickerArrow,
  "sticker-rocket": stickerRocket,
} as const;

export type ArtName = keyof typeof SVG;

/** SVG files as URLs (CSS backgrounds). */
export const artUrl = {
  patternLight: patternLightUrl,
  patternDark: patternDarkUrl,
} as const;

/** Our own static SVG files, comments stripped. */
function markup(name: ArtName): string {
  return SVG[name].replace(/<!--.*?-->/gs, "");
}

/**
 * One inlined, decorative SVG. `width` sets the size (height follows the
 * drawing's aspect); the art is hidden from screen readers.
 */
export function ForkArt({
  name,
  width,
  className,
  style,
}: {
  name: ArtName;
  width?: number | string;
  className?: string;
  style?: CSSProperties;
}) {
  return (
    <span
      aria-hidden="true"
      data-art={name}
      className={`inline-block [&>svg]:block [&>svg]:h-auto [&>svg]:w-full ${className ?? ""}`}
      style={{ width, ...style }}
      dangerouslySetInnerHTML={{ __html: markup(name) }}
    />
  );
}
