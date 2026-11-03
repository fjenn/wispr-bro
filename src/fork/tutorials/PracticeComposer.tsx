// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * A simulated app for practice steps (S-31, B-40; Design W5): a drawn
 * composer where the user does the real gesture (select text and press the
 * shortcut, S-31; dictate into it, S-32). Our own drawing, no other app's
 * look: `chat` is a message composer (formatting row, the other person's
 * last message, the field, an actions row with send), `mail` a new-message
 * window (a header band with the app, the field). Only the field is real;
 * the rest is decoration hidden from assistive tech.
 *
 * `replaceSelection` writes through the editing command so the field's own
 * undo works: ⌘Z after a practice rewrite puts the original back, as it
 * does after a real transform.
 */
import { forwardRef, type ReactNode } from "react";
import {
  AtSign,
  Bold,
  Italic,
  Link,
  List,
  ListOrdered,
  Mic,
  Paperclip,
  Plus,
  SendHorizontal,
  Smile,
  Strikethrough,
  Type,
  Underline,
  type Glyph,
} from "../ui/icons";

export interface PracticeComposerProps {
  /** Window title (the simulated app). */
  app: string;
  icon: Glyph;
  /** `chat` (formatting and actions rows) or `mail` (a header band). */
  variant?: "chat" | "mail";
  /** A line above the field (the message being answered), optional. */
  context?: string;
  /** Accessible name of the field. */
  label: string;
  /** The field's starting text (an uncontrolled field the user edits). */
  defaultValue?: string;
  /**
   * The field's text when the lesson fills it (S-32: the take); the field
   * is then read-only, the take is what is graded.
   */
  value?: string;
  placeholder?: string;
  /** Drawn under the window (the bar, a spotlight). */
  children?: ReactNode;
}

const FORMAT: Glyph[] = [
  Bold,
  Italic,
  Underline,
  Strikethrough,
  Link,
  List,
  ListOrdered,
];
const ACTIONS: Glyph[] = [Plus, Type, Smile, AtSign, Paperclip, Mic];

export const PracticeComposer = forwardRef<
  HTMLTextAreaElement,
  PracticeComposerProps
>(function PracticeComposer(
  {
    app,
    icon: Icon,
    variant = "chat",
    context,
    label,
    defaultValue,
    value,
    placeholder,
    children,
  },
  ref,
) {
  return (
    <div
      className="wb-practice"
      data-variant={variant}
      data-testid="wb-practice-composer"
    >
      <div className="wb-practice-window">
        {variant === "mail" ? (
          <div className="wb-practice-header" aria-hidden="true">
            <Icon size={16} />
            <span>{app}</span>
          </div>
        ) : (
          <div className="wb-practice-tools" aria-hidden="true">
            {FORMAT.map((G, i) => (
              <G key={i} size={14} />
            ))}
          </div>
        )}
        <div className="wb-practice-body">
          {context && (
            <div className="wb-practice-thread">
              {variant === "chat" && (
                <span className="wb-practice-from" aria-hidden="true">
                  <Icon size={12} />
                  {app}
                </span>
              )}
              <p
                className="wb-practice-context"
                data-testid="wb-practice-context"
              >
                {context}
              </p>
            </div>
          )}
          <textarea
            ref={ref}
            aria-label={label}
            {...(value !== undefined
              ? { value, readOnly: true }
              : { defaultValue })}
            placeholder={placeholder}
            spellCheck={false}
            rows={4}
            className="wb-practice-field"
            data-testid="wb-practice-field"
          />
        </div>
        {variant === "chat" && (
          <div className="wb-practice-actions" aria-hidden="true">
            {ACTIONS.map((G, i) => (
              <G key={i} size={16} />
            ))}
            <SendHorizontal size={16} className="ms-auto rtl:-scale-x-100" />
          </div>
        )}
      </div>
      {children}
    </div>
  );
});

/** The selected text and its range (empty when nothing is selected). */
export function selectionOf(field: HTMLTextAreaElement): {
  text: string;
  start: number;
  end: number;
} {
  const start = field.selectionStart ?? 0;
  const end = field.selectionEnd ?? start;
  return { text: field.value.slice(start, end), start, end };
}

/**
 * Replaces `start..end` with `text` as one undoable edit and leaves the new
 * text selected (as a transform does).
 */
export function replaceSelection(
  field: HTMLTextAreaElement,
  start: number,
  end: number,
  text: string,
): void {
  field.focus();
  field.setSelectionRange(start, end);
  let inserted = false;
  try {
    // Deprecated but the one way to keep the field's native undo stack.
    inserted = document.execCommand("insertText", false, text);
  } catch {
    inserted = false;
  }
  if (!inserted || field.value.slice(start, start + text.length) !== text) {
    field.setRangeText(text, start, end, "end");
    field.dispatchEvent(new Event("input", { bubbles: true }));
  }
  field.setSelectionRange(start, start + text.length);
}
