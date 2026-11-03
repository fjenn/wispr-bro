// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The first-use chooser's options (S-30, B-31). The ids are the ones
 * `fork/pill/spotlight.rs` accepts, in the same order (a Rust test reads
 * this list). Each option has a scene (`scenes/`) and a hint on the bar
 * spotlight (`rust.spotlight.hint.<id>`); a Playwright test checks both.
 */
import { type Glyph } from "../ui/icons";
import {
  CodeXml,
  FileText,
  Mail,
  MessageCircle,
  MessagesSquare,
  NotebookPen,
  Sparkles,
} from "../ui/icons";

export const FIRST_USE_CHOICES = [
  "ai_chat",
  "note",
  "message",
  "email",
  "comment",
  "document",
  "coding",
] as const;

export type FirstUseChoice = (typeof FIRST_USE_CHOICES)[number];

export const CHOICE_ICONS: Record<FirstUseChoice, Glyph> = {
  ai_chat: Sparkles,
  note: NotebookPen,
  message: MessageCircle,
  email: Mail,
  comment: MessagesSquare,
  document: FileText,
  coding: CodeXml,
};

export function isFirstUseChoice(value: unknown): value is FirstUseChoice {
  return FIRST_USE_CHOICES.includes(value as FirstUseChoice);
}

/** The banner id the chooser's "Maybe later" stores in `dismissed_banners`. */
export const CHOOSER_DISMISSED_ID = "first-use.chooser";
/** The home banner after the chooser (S-30): "Use it in your own apps". */
export const OWN_APPS_BANNER_ID = "first-use.own-apps";
