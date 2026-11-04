// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The fork's icon set (Design Phase 0, SYSTEM-PLAN §3): Lucide glyphs (ISC,
 * NOTICE) wrapped by `glyph()` in `Icon.tsx` (1.5 stroke, fixed sizes,
 * `currentColor`, hidden from screen readers unless labelled). Import glyphs
 * from here, never from `lucide-react` (`scripts/wb-guards.ts` enforces it).
 * A glyph Lucide lacks is drawn on Lucide's 24 grid at 1.5 stroke and added
 * under `src/fork/ui/icons/`.
 *
 * Brand marks are the one exception to the stroke rule: they are drawn
 * filled, unmodified, from their owner's official file (`GitHubMark`).
 */
import { createElement, forwardRef } from "react";
import * as L from "lucide-react";
import { glyph, type Glyph, type GlyphProps } from "./Icon";

export { Icon, glyph, ICON_SIZES, ICON_STROKE } from "./Icon";
export type { Glyph, GlyphProps, IconProps, IconSize } from "./Icon";

export const ALargeSmall = glyph(L.ALargeSmall);
export const AppWindow = glyph(L.AppWindow);
export const ArrowLeft = glyph(L.ArrowLeft);
export const ArrowRight = glyph(L.ArrowRight);
export const AtSign = glyph(L.AtSign);
export const AudioLines = glyph(L.AudioLines);
export const Bell = glyph(L.Bell);
export const BellOff = glyph(L.BellOff);
export const Bold = glyph(L.Bold);
export const BookA = glyph(L.BookA);
export const BookOpen = glyph(L.BookOpen);
export const Bug = glyph(L.Bug);
export const ChartColumn = glyph(L.ChartColumn);
export const Check = glyph(L.Check);
export const CheckCheck = glyph(L.CheckCheck);
export const CheckCircle2 = glyph(L.CheckCircle2);
export const ChevronDown = glyph(L.ChevronDown);
export const ChevronLeft = glyph(L.ChevronLeft);
export const ChevronRight = glyph(L.ChevronRight);
export const ChevronUp = glyph(L.ChevronUp);
export const Circle = glyph(L.Circle);
export const CircleHelp = glyph(L.CircleHelp);
export const Clipboard = glyph(L.Clipboard);
export const ClipboardCopy = glyph(L.ClipboardCopy);
export const Clock = glyph(L.Clock);
export const Clock3 = glyph(L.Clock3);
export const Code2 = glyph(L.Code2);
export const CodeXml = glyph(L.CodeXml);
export const Copy = glyph(L.Copy);
export const CornerDownLeft = glyph(L.CornerDownLeft);
export const Download = glyph(L.Download);
export const Ear = glyph(L.Ear);
export const Expand = glyph(L.Expand);
export const ExternalLink = glyph(L.ExternalLink);
export const FileAudio = glyph(L.FileAudio);
export const FilePlus = glyph(L.FilePlus);
export const FileText = glyph(L.FileText);
export const FileUp = glyph(L.FileUp);
export const Flag = glyph(L.Flag);
export const Flame = glyph(L.Flame);
export const FolderInput = glyph(L.FolderInput);
export const FolderOpen = glyph(L.FolderOpen);
export const Gauge = glyph(L.Gauge);
export const Globe = glyph(L.Globe);
export const GraduationCap = glyph(L.GraduationCap);
export const HandHeart = glyph(L.HandHeart);
export const Heart = glyph(L.Heart);
export const ImageDown = glyph(L.ImageDown);
export const Info = glyph(L.Info);
export const Italic = glyph(L.Italic);
export const Keyboard = glyph(L.Keyboard);
export const Laptop = glyph(L.Laptop);
export const Languages = glyph(L.Languages);
export const Link = glyph(L.Link);
export const List = glyph(L.List);
export const ListOrdered = glyph(L.ListOrdered);
export const Loader2 = glyph(L.Loader2);
export const Lock = glyph(L.Lock);
export const LockKeyhole = glyph(L.LockKeyhole);
export const Mail = glyph(L.Mail);
export const Maximize2 = glyph(L.Maximize2);
export const MessageCircle = glyph(L.MessageCircle);
export const MessageSquareQuote = glyph(L.MessageSquareQuote);
export const MessageSquareText = glyph(L.MessageSquareText);
export const MessagesSquare = glyph(L.MessagesSquare);
export const Mic = glyph(L.Mic);
export const Minimize = glyph(L.Minimize);
export const Minimize2 = glyph(L.Minimize2);
export const Minus = glyph(L.Minus);
export const Moon = glyph(L.Moon);
export const MoreHorizontal = glyph(L.MoreHorizontal);
export const NotebookPen = glyph(L.NotebookPen);
export const Palette = glyph(L.Palette);
export const PanelLeft = glyph(L.PanelLeft);
export const PanelLeftClose = glyph(L.PanelLeftClose);
export const PanelLeftOpen = glyph(L.PanelLeftOpen);
export const Paperclip = glyph(L.Paperclip);
export const PartyPopper = glyph(L.PartyPopper);
export const Pause = glyph(L.Pause);
export const Pencil = glyph(L.Pencil);
export const Pin = glyph(L.Pin);
export const PinOff = glyph(L.PinOff);
export const Play = glyph(L.Play);
export const Plus = glyph(L.Plus);
export const Power = glyph(L.Power);
export const Redo2 = glyph(L.Redo2);
export const RefreshCw = glyph(L.RefreshCw);
export const RotateCcw = glyph(L.RotateCcw);
export const Save = glyph(L.Save);
export const Search = glyph(L.Search);
export const SearchX = glyph(L.SearchX);
export const SendHorizontal = glyph(L.SendHorizontal);
export const Settings = glyph(L.Settings);
export const Settings2 = glyph(L.Settings2);
export const Share = glyph(L.Share);
export const ShieldCheck = glyph(L.ShieldCheck);
export const SlidersHorizontal = glyph(L.SlidersHorizontal);
export const Smile = glyph(L.Smile);
export const Sparkles = glyph(L.Sparkles);
export const SpellCheck = glyph(L.SpellCheck);
export const Square = glyph(L.Square);
export const SquarePen = glyph(L.SquarePen);
export const Star = glyph(L.Star);
export const Strikethrough = glyph(L.Strikethrough);
export const Sun = glyph(L.Sun);
export const TextQuote = glyph(L.TextQuote);
export const ThumbsDown = glyph(L.ThumbsDown);
export const ThumbsUp = glyph(L.ThumbsUp);
export const Trash2 = glyph(L.Trash2);
export const TriangleAlert = glyph(L.TriangleAlert);
export const Type = glyph(L.Type);
export const Underline = glyph(L.Underline);
export const Undo2 = glyph(L.Undo2);
export const Upload = glyph(L.Upload);
export const WandSparkles = glyph(L.WandSparkles);
export const X = glyph(L.X);

/**
 * GitHub's mark, the Invertocat (OT2-E), for links to our GitHub project
 * (About › View source code / View license, Handy on GitHub). The path is
 * GitHub's official `GitHub_Invertocat_Black.svg` from the logo package on
 * GitHub's brand site (`GitHub_Logos.zip`, read 2026-11-04; the address is
 * in NOTICE §10), unchanged
 * except its fill: one solid colour (`currentColor`, the ink of the button
 * it sits in), never mixed, at its own 98:96 proportions. GitHub's logo
 * guidelines allow the Invertocat as a button linking to a project on
 * GitHub; it is GitHub's trademark, not ours (NOTICE §10,
 * LICENSE-REGISTER §2a). Hidden from screen readers unless labelled; the
 * button's text names the link.
 */
const GitHubMarkSvg = forwardRef<SVGSVGElement, GlyphProps>(function GitHubMark(
  {
    size = 18,
    strokeWidth: _stroke,
    absoluteStrokeWidth: _abs,
    filled: _filled,
    ...rest
  },
  ref,
) {
  const labelled = rest["aria-label"] !== undefined || rest.role !== undefined;
  return createElement(
    "svg",
    {
      ref,
      viewBox: "0 0 98 96",
      width: size,
      height: size,
      fill: "currentColor",
      "data-glyph": "github-mark",
      ...(labelled ? {} : { "aria-hidden": rest["aria-hidden"] ?? true }),
      ...rest,
    },
    createElement("path", { d: GITHUB_MARK_PATH }),
  );
});

const GITHUB_MARK_PATH =
  "M41.4395 69.3848C28.8066 67.8535 19.9062 58.7617 19.9062 46.9902C19.9062 42.2051 21.6289 37.0371 24.5 33.5918C23.2559 30.4336 23.4473 23.7344 24.8828 20.959C28.7109 20.4805 33.8789 22.4902 36.9414 25.2656C40.5781 24.1172 44.4062 23.543 49.0957 23.543C53.7852 23.543 57.6133 24.1172 61.0586 25.1699C64.0254 22.4902 69.2891 20.4805 73.1172 20.959C74.457 23.543 74.6484 30.2422 73.4043 33.4961C76.4668 37.1328 78.0937 42.0137 78.0937 46.9902C78.0937 58.7617 69.1934 67.6621 56.3691 69.2891C59.623 71.3945 61.8242 75.9883 61.8242 81.252L61.8242 91.2051C61.8242 94.0762 64.2168 95.7031 67.0879 94.5547C84.4102 87.9512 98 70.6289 98 49.1914C98 22.1074 75.9883 6.69539e-07 48.9043 4.309e-07C21.8203 1.92261e-07 -1.9479e-07 22.1074 -4.3343e-07 49.1914C-6.20631e-07 70.4375 13.4941 88.0469 31.6777 94.6504C34.2617 95.6074 36.75 93.8848 36.75 91.3008L36.75 83.6445C35.4102 84.2188 33.6875 84.6016 32.1562 84.6016C25.8398 84.6016 22.1074 81.1563 19.4277 74.7441C18.375 72.1602 17.2266 70.6289 15.0254 70.3418C13.877 70.2461 13.4941 69.7676 13.4941 69.1934C13.4941 68.0449 15.4082 67.1836 17.3223 67.1836C20.0977 67.1836 22.4902 68.9063 24.9785 72.4473C26.8926 75.2227 28.9023 76.4668 31.2949 76.4668C33.6875 76.4668 35.2187 75.6055 37.4199 73.4043C39.0469 71.7773 40.291 70.3418 41.4395 69.3848Z";
GitHubMarkSvg.displayName = "GitHubMark";
export const GitHubMark = GitHubMarkSvg as Glyph;
