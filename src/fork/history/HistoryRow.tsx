// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * One History row (S-15b, C-34, C-37): the time, the text (folded to four
 * lines with "Show more"), tags for file and practice takes, and the
 * actions play, copy, flag and ⋮.
 *
 * The actions show on hover and whenever focus is inside the row
 * (`focus-within`), so a keyboard user tabbing in sees them; they are
 * always in the tab order. Play opens Handy's `AudioPlayer` under the row
 * and starts it (one recording plays at a time, `AudioPlayerGroup`).
 * A background job (a retry or a file, C-47) shows its progress in place of
 * the text.
 */
import {
  Check,
  Copy,
  FileAudio,
  Flag,
  GraduationCap,
  Play,
  Square,
} from "../ui/icons";
import { useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { AudioPlayer } from "@/components/ui/AudioPlayer";
import type { ActiveJobStatus } from "@/stores/transcriptionJobsStore";
import type { CleanupLevel, HistoryRow as Row } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { timeLabel } from "./dates";
import { ACTION_BUTTON, RowMenu, audioReason } from "./RowMenu";
import { WaitingRow } from "./WaitingRow";

/** Long text folds to this many lines until "Show more". */
export const FOLDED_LINES = 4;

export interface HistoryRowProps {
  row: Row;
  job?: ActiveJobStatus;
  level: CleanupLevel;
  audioDays: number | null;
  loadAudio: (fileName: string) => Promise<string | null>;
  /** The recording could not be found when Play was pressed. */
  onAudioMissing: (row: Row) => void;
  onCopy: (row: Row) => Promise<boolean>;
  onFlag: (row: Row) => void;
  onToggleVariant: (row: Row) => void;
  onRetry: (row: Row, level: CleanupLevel | null) => void;
  onSaveAudio: (row: Row) => void;
  onDelete: (row: Row) => void;
}

export function HistoryRow({
  row,
  job,
  level,
  audioDays,
  loadAudio,
  onAudioMissing,
  onCopy,
  onFlag,
  onToggleVariant,
  onRetry,
  onSaveAudio,
  onDelete,
}: HistoryRowProps) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  /** The recording's URL while the player is open. */
  const [audioSrc, setAudioSrc] = useState<string | null>(null);
  const [loadingAudio, setLoadingAudio] = useState(false);
  const playing = audioSrc !== null;
  const [copied, setCopied] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [overflows, setOverflows] = useState(false);
  const textRef = useRef<HTMLParagraphElement>(null);
  const waiting = row.queue_status === "waiting";
  const transcribing = row.queue_status === "transcribing";
  const busy = job !== undefined || transcribing;
  const hasText = row.text.trim().length > 0;
  const folded = hasText && !busy && !expanded;
  const noAudio = audioReason(row, audioDays, t);

  useLayoutEffect(() => {
    const el = textRef.current;
    if (!el || !folded) return;
    const measure = () => setOverflows(el.scrollHeight > el.clientHeight + 1);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, [folded, row.text]);

  // Handy's player loads a lazy source only from its own button; History's
  // Play finds the file first so the player starts at once.
  const togglePlayer = async () => {
    if (playing) {
      setAudioSrc(null);
      return;
    }
    setLoadingAudio(true);
    const src = await loadAudio(row.file_name);
    setLoadingAudio(false);
    if (src) setAudioSrc(src);
    else onAudioMissing(row);
  };

  const copy = async () => {
    if (!hasText) return;
    if (await onCopy(row)) {
      setCopied(true);
      setTimeout(() => setCopied(false), 2_000);
    }
  };

  let body: React.ReactNode;
  if (job) {
    body =
      job.state === "running" && job.percent > 0
        ? t("history.job.percent", { percent: job.percent })
        : job.state === "queued"
          ? t("history.job.waiting")
          : t("history.job.transcribing");
  } else if (transcribing) {
    body = t("history.job.waiting");
  } else if (hasText) {
    body = row.text;
  } else if (waiting) {
    body = <WaitingRow />;
  } else if (row.source === "file") {
    body = t("history.notTranscribed");
  } else {
    body = t("history.failed");
  }

  const time = timeLabel(row.created_at, i18n.language);

  return (
    <div
      data-testid="wb-history-row"
      data-id={row.id}
      data-variant={row.variant}
      role="group"
      aria-label={t("history.rowLabel", { time })}
      className="group relative flex gap-4 rounded-(--wb-radius-md) px-3 py-3 hover:bg-(--wb-fill-hover) focus-within:bg-(--wb-fill-hover)"
    >
      <div className="w-16 shrink-0 pt-0.5 text-[12.5px] text-(--wb-ink-3) tabular-nums">
        {time}
      </div>
      <div className="flex min-w-0 flex-1 flex-col gap-1.5">
        {(row.source === "file" ||
          row.source === "practice" ||
          row.variant === "raw" ||
          row.flagged) && (
          <div className="flex flex-wrap items-center gap-1.5 text-[11.5px] text-(--wb-ink-2)">
            {row.source === "file" && (
              <span
                data-testid="wb-history-tag-file"
                className="flex max-w-full min-w-0 items-center gap-1 rounded-full bg-(--wb-fill-active) px-2 py-0.5"
                title={row.source_name ?? undefined}
              >
                <FileAudio size={12} aria-hidden="true" className="shrink-0" />
                <span className="sr-only">{t("history.tag.file")}</span>
                <span className="truncate">
                  {row.source_name ?? t("history.tag.file")}
                </span>
              </span>
            )}
            {row.source === "practice" && (
              <span
                data-testid="wb-history-tag-practice"
                className="flex items-center gap-1 rounded-full bg-(--wb-fill-active) px-2 py-0.5"
              >
                <GraduationCap size={12} aria-hidden="true" />
                {t("history.tag.practice")}
              </span>
            )}
            {row.variant === "raw" && (
              <span
                data-testid="wb-history-tag-raw"
                className="rounded-full bg-(--wb-fill-active) px-2 py-0.5"
              >
                {t("history.tag.raw")}
              </span>
            )}
            {row.flagged && (
              <span
                data-testid="wb-history-tag-flagged"
                className="flex items-center gap-1 rounded-full bg-(--wb-fill-active) px-2 py-0.5"
              >
                <Flag size={12} aria-hidden="true" />
                {t("history.tag.flagged")}
              </span>
            )}
          </div>
        )}
        <p
          ref={textRef}
          data-testid="wb-history-text"
          className={`text-[14px] leading-[1.45] break-words whitespace-pre-wrap ${
            busy
              ? "text-(--wb-ink-2) motion-safe:animate-pulse"
              : hasText
                ? "cursor-text text-(--wb-ink) select-text"
                : "text-(--wb-ink-3) italic"
          }`}
          style={
            folded
              ? {
                  display: "-webkit-box",
                  WebkitLineClamp: FOLDED_LINES,
                  WebkitBoxOrient: "vertical",
                  overflow: "hidden",
                }
              : undefined
          }
        >
          {body}
        </p>
        {hasText && !busy && (expanded || overflows) && (
          <button
            type="button"
            data-testid="wb-history-more-text"
            aria-expanded={expanded}
            onClick={() => setExpanded(!expanded)}
            className="self-start cursor-pointer rounded-[5px] text-[12.5px] font-medium text-(--wb-ink-2) hover:text-(--wb-ink) focus-visible:outline-2 focus-visible:outline-(--wb-accent)"
          >
            {expanded ? t("history.showLess") : t("history.showMore")}
          </button>
        )}
        {audioSrc && (
          <AudioPlayer
            src={audioSrc}
            autoPlay
            className="mt-1 w-full max-w-md"
          />
        )}
      </div>
      <div
        data-testid="wb-history-actions"
        className="flex shrink-0 items-start gap-0.5 opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100 has-[[data-state=open]]:opacity-100 has-[[aria-pressed=true]]:opacity-100 motion-reduce:transition-none [@media(hover:none)]:opacity-100"
      >
        <button
          type="button"
          data-testid="wb-history-play"
          aria-label={
            noAudio ??
            (playing
              ? t("history.actions.hidePlayer")
              : t("history.actions.play"))
          }
          title={
            noAudio ??
            (playing
              ? t("history.actions.hidePlayer")
              : t("history.actions.play"))
          }
          aria-pressed={playing}
          disabled={noAudio !== null || loadingAudio}
          onClick={() => void togglePlayer()}
          className={ACTION_BUTTON}
        >
          {playing ? (
            <Square size={14} aria-hidden="true" />
          ) : (
            <Play size={16} aria-hidden="true" />
          )}
        </button>
        <button
          type="button"
          data-testid="wb-history-copy"
          aria-label={
            copied ? t("history.actions.copied") : t("history.actions.copy")
          }
          title={
            copied ? t("history.actions.copied") : t("history.actions.copy")
          }
          disabled={!hasText || busy}
          onClick={() => void copy()}
          className={ACTION_BUTTON}
        >
          {copied ? (
            <Check size={16} aria-hidden="true" />
          ) : (
            <Copy size={16} aria-hidden="true" />
          )}
        </button>
        <button
          type="button"
          data-testid="wb-history-flag"
          aria-label={t("history.actions.flag")}
          title={t("history.actions.flag")}
          aria-haspopup="dialog"
          disabled={!hasText || busy}
          onClick={() => onFlag(row)}
          className={ACTION_BUTTON}
        >
          <Flag size={16} aria-hidden="true" />
        </button>
        <RowMenu
          row={row}
          level={level}
          audioDays={audioDays}
          busy={busy || waiting}
          onToggleVariant={() => onToggleVariant(row)}
          onRetry={(l) => onRetry(row, l)}
          onSaveAudio={() => onSaveAudio(row)}
          onDelete={() => onDelete(row)}
        />
      </div>
      <span aria-live="polite" className="sr-only">
        {copied ? t("history.actions.copied") : ""}
      </span>
    </div>
  );
}
