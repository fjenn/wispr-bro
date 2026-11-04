// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * A shortcut drawn as key caps (Design W3, after o-0178): "fn Space" →
 * [fn] [Space]; a named input ("Back mouse button") stays one cap. The text keeps its spaces between the caps, so it reads and
 * copies as the label it was built from.
 */
import { Kbd } from "../../ui";
import { chordLabel } from "./model";

export function ChordKeys({
  chord,
  text,
  pressed = false,
  capClassName,
}: {
  /** A stored chord (`fn+space`), labelled with `chordLabel`. */
  chord?: string;
  /** A label already made for display (the backend's `label.text`). */
  text?: string;
  /** Keys held right now (the dictation key capture): accent caps. */
  pressed?: boolean;
  capClassName?: string;
}) {
  const label = text ?? (chord ? chordLabel(chord) : "");
  const parts = label.split(" ").filter(Boolean);
  // A named input ("Back mouse button") stays one cap; keys split.
  const named = parts.some((part) => part !== "fn" && /^[a-z]{3,}$/.test(part));
  const keys = named ? [label] : parts;
  return (
    <span className="inline-flex flex-wrap items-center gap-1">
      {keys.map((key, index) => (
        <span key={`${key}-${index}`} className="contents">
          {index > 0 && " "}
          <Kbd pressed={pressed} className={capClassName}>
            {key}
          </Kbd>
        </span>
      ))}
    </span>
  );
}
