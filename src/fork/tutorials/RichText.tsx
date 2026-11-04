// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Lesson and chooser text with two marks (Design W5): `<em>` is the one
 * italic word of a serif sheet heading (purple on the Transforms lesson,
 * `accent`), and `<kbd>…</kbd>` turns a shortcut label ("⌥ 1", "⌘ Z") into
 * key caps, one per space-separated key. The text content stays the same
 * string the translation renders (the caps keep a space between them), so
 * screen readers and tests read "Select the text, then press ⌥ 1".
 *
 * `plainText` strips the marks for places that need a plain string (an
 * accessible name, the clipboard).
 */
import { Children, Fragment, isValidElement, type ReactNode } from "react";
import { Trans } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { Kbd } from "../ui";
import "./richText.css";

const MARKS = /<\/?(em|kbd)>/g;

/** The string without its `<em>`/`<kbd>` marks. */
export function plainText(text: string): string {
  return text.replace(MARKS, "");
}

function textOf(node: ReactNode): string {
  return Children.toArray(node)
    .map((child) =>
      typeof child === "string" || typeof child === "number"
        ? String(child)
        : isValidElement<{ children?: ReactNode }>(child)
          ? textOf(child.props.children)
          : "",
    )
    .join("");
}

/** Key caps for a shortcut label; `size="lg"` sits in a serif heading. */
export function Keys({
  children,
  size = "md",
}: {
  children?: ReactNode;
  size?: "md" | "lg";
}) {
  const keys = textOf(children).split(/\s+/).filter(Boolean);
  return (
    <span className="wb-keys whitespace-nowrap" data-size={size}>
      {keys.map((key, i) => (
        <Fragment key={`${key}-${i}`}>
          {i > 0 && " "}
          <Kbd
            className={
              size === "lg"
                ? "h-8! min-w-8! px-2! align-[0.18em] text-[16px]!"
                : "align-[0.08em]"
            }
          >
            {key}
          </Kbd>
        </Fragment>
      ))}
    </span>
  );
}

export function RichText({
  i18nKey,
  values,
  accent = false,
  keySize = "md",
}: {
  i18nKey: string;
  values?: Record<string, string | number>;
  /** The italic word in the brand purple (Transforms). */
  accent?: boolean;
  keySize?: "md" | "lg";
}) {
  return (
    <Trans
      ns={FORK_NAMESPACE}
      i18nKey={i18nKey}
      values={values}
      shouldUnescape
      components={{
        em: <em className="wb-em" data-accent={accent || undefined} />,
        kbd: <Keys size={keySize} />,
      }}
    />
  );
}
