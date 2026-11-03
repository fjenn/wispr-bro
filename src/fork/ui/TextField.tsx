// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Text inputs (Design Phase 0): `TextField` (40 pt, or 36 / 32), `TextArea`
 * and `SearchField` (a magnifier inside the field). White on the panel with
 * a visible 3:1 edge (`--wb-line-strong`), 8 pt radius, purple focus ring;
 * `invalid` turns the edge red and sets `aria-invalid`.
 */
import {
  forwardRef,
  type InputHTMLAttributes,
  type ReactNode,
  type TextareaHTMLAttributes,
} from "react";
import { Search } from "./icons";

const FIELD =
  "w-full rounded-(--wb-radius-sm) border bg-(--wb-surface) text-[15px] text-(--wb-ink) transition-[border-color,box-shadow] duration-(--wb-dur-fast) placeholder:text-(--wb-ink-3) focus-visible:outline-2 focus-visible:outline-offset-0 focus-visible:outline-(--wb-accent) disabled:cursor-default disabled:opacity-60";

const edge = (invalid: boolean) =>
  invalid
    ? "border-(--wb-danger)"
    : "border-(--wb-line-strong) hover:border-(--wb-ink-3)";

const HEIGHTS = { sm: "h-8 px-2.5", md: "h-9 px-3", lg: "h-10 px-3" };

export interface TextFieldProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, "size"> {
  size?: keyof typeof HEIGHTS;
  invalid?: boolean;
  /** A glyph inside the field, at the start. */
  leading?: ReactNode;
  /** Something inside the field, at the end (a clear button, a kbd). */
  trailing?: ReactNode;
}

export const TextField = forwardRef<HTMLInputElement, TextFieldProps>(
  function TextField(
    {
      size = "lg",
      invalid = false,
      leading,
      trailing,
      className = "",
      ...rest
    },
    ref,
  ) {
    const input = (
      <input
        ref={ref}
        aria-invalid={invalid || undefined}
        className={`${FIELD} ${HEIGHTS[size]} ${edge(invalid)} ${leading ? "ps-9" : ""} ${trailing ? "pe-9" : ""} ${leading || trailing ? "" : className}`}
        {...rest}
      />
    );
    if (!leading && !trailing) return input;
    return (
      <span className={`relative flex items-center ${className}`}>
        {leading && (
          <span className="pointer-events-none absolute start-3 text-(--wb-ink-3)">
            {leading}
          </span>
        )}
        {input}
        {trailing && (
          <span className="absolute end-1.5 flex items-center">{trailing}</span>
        )}
      </span>
    );
  },
);

export const SearchField = forwardRef<
  HTMLInputElement,
  Omit<TextFieldProps, "leading" | "type">
>(function SearchField(props, ref) {
  return (
    <TextField
      ref={ref}
      type="search"
      size="md"
      leading={<Search size={16} />}
      {...props}
    />
  );
});

export interface TextAreaProps
  extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  invalid?: boolean;
}

export const TextArea = forwardRef<HTMLTextAreaElement, TextAreaProps>(
  function TextArea({ invalid = false, className = "", ...rest }, ref) {
    return (
      <textarea
        ref={ref}
        aria-invalid={invalid || undefined}
        className={`${FIELD} ${edge(invalid)} min-h-24 px-3 py-2.5 leading-[22px] ${className}`}
        {...rest}
      />
    );
  },
);
