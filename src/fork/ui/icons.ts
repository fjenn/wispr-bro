// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The fork's icon set (Design Phase 0, SYSTEM-PLAN §3): Lucide glyphs (ISC,
 * NOTICE) wrapped by `glyph()` in `Icon.tsx` (1.5 stroke, fixed sizes,
 * `currentColor`, hidden from screen readers unless labelled). Import glyphs
 * from here, never from `lucide-react` (`scripts/wb-guards.ts` enforces it).
 * A glyph Lucide lacks is drawn on Lucide's 24 grid at 1.5 stroke and added
 * under `src/fork/ui/icons/`.
 */
import * as L from "lucide-react";
import { glyph } from "./Icon";

export { Icon, glyph, ICON_SIZES, ICON_STROKE } from "./Icon";
export type { Glyph, GlyphProps, IconProps, IconSize } from "./Icon";

export const AppWindow = glyph(L.AppWindow);
export const ArrowLeft = glyph(L.ArrowLeft);
export const ArrowRight = glyph(L.ArrowRight);
export const AudioLines = glyph(L.AudioLines);
export const Bell = glyph(L.Bell);
export const BellOff = glyph(L.BellOff);
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
export const Heart = glyph(L.Heart);
export const ImageDown = glyph(L.ImageDown);
export const Info = glyph(L.Info);
export const Keyboard = glyph(L.Keyboard);
export const Languages = glyph(L.Languages);
export const Loader2 = glyph(L.Loader2);
export const Lock = glyph(L.Lock);
export const LockKeyhole = glyph(L.LockKeyhole);
export const Mail = glyph(L.Mail);
export const MessageCircle = glyph(L.MessageCircle);
export const MessageSquareQuote = glyph(L.MessageSquareQuote);
export const MessageSquareText = glyph(L.MessageSquareText);
export const MessagesSquare = glyph(L.MessagesSquare);
export const Mic = glyph(L.Mic);
export const Minimize = glyph(L.Minimize);
export const Minus = glyph(L.Minus);
export const Moon = glyph(L.Moon);
export const MoreHorizontal = glyph(L.MoreHorizontal);
export const NotebookPen = glyph(L.NotebookPen);
export const Palette = glyph(L.Palette);
export const PanelLeft = glyph(L.PanelLeft);
export const PanelLeftClose = glyph(L.PanelLeftClose);
export const PanelLeftOpen = glyph(L.PanelLeftOpen);
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
export const Settings = glyph(L.Settings);
export const Settings2 = glyph(L.Settings2);
export const Share = glyph(L.Share);
export const ShieldCheck = glyph(L.ShieldCheck);
export const Sparkles = glyph(L.Sparkles);
export const SpellCheck = glyph(L.SpellCheck);
export const Square = glyph(L.Square);
export const Star = glyph(L.Star);
export const Sun = glyph(L.Sun);
export const TextQuote = glyph(L.TextQuote);
export const ThumbsDown = glyph(L.ThumbsDown);
export const ThumbsUp = glyph(L.ThumbsUp);
export const Trash2 = glyph(L.Trash2);
export const TriangleAlert = glyph(L.TriangleAlert);
export const Type = glyph(L.Type);
export const Undo2 = glyph(L.Undo2);
export const Upload = glyph(L.Upload);
export const WandSparkles = glyph(L.WandSparkles);
export const X = glyph(L.X);
