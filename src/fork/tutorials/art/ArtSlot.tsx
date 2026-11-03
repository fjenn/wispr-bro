// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * An image slot in a lesson (Design W5; SYSTEM-PLAN §6). The whisper lesson
 * leads with a picture of someone speaking softly to a laptop; the W7 art
 * agent sources the licensed photo. Until a `photo` is passed, the slot
 * shows a placeholder drawn from tokens only: a dark, warm gradient with a
 * soft grain-like highlight (the graded photos' palette) and our own line
 * drawing on top (`children`). The caption stays visible either way.
 *
 * The picture is decorative (the caption and the step's text say what it
 * shows), so it is hidden from assistive tech; `slot` names it for the art
 * agent and the tests (`data-art-slot`).
 */
import type { ReactNode } from "react";

export interface ArtPhoto {
  src: string;
  srcSet?: string;
  avifSrcSet?: string;
  width: number;
  height: number;
}

export function ArtSlot({
  slot,
  photo,
  caption,
  children,
  className = "",
}: {
  /** Which picture belongs here (`lesson-whisper`). */
  slot: string;
  photo?: ArtPhoto;
  caption?: ReactNode;
  /** Drawn over the placeholder (or the photo). */
  children?: ReactNode;
  className?: string;
}) {
  return (
    <figure
      className={`m-0 flex flex-col items-center gap-2 ${className}`}
      data-art-slot={slot}
      data-testid={`wb-art-${slot}`}
    >
      <div
        aria-hidden="true"
        className="wb-art-slot"
        data-placeholder={photo ? undefined : true}
      >
        {photo && (
          <picture>
            {photo.avifSrcSet && (
              <source type="image/avif" srcSet={photo.avifSrcSet} />
            )}
            <img
              src={photo.src}
              srcSet={photo.srcSet}
              width={photo.width}
              height={photo.height}
              alt=""
              decoding="async"
            />
          </picture>
        )}
        {children && <div className="wb-art-slot-overlay">{children}</div>}
      </div>
      {caption && (
        <figcaption className="type-small text-center text-(--wb-ink-2)">
          {caption}
        </figcaption>
      )}
    </figure>
  );
}
