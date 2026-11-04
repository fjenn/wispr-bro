// SPDX-License-Identifier: GPL-3.0-or-later
import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Check,
  Clipboard,
  RotateCcw,
  Settings2,
  ThumbsDown,
  ThumbsUp,
} from "../ui/icons";
import type { DiffOp, ForkTransformViewChanges, WordDiff } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button, FOCUS_RING } from "../ui";

/** A changed word: a small chip, 2 px from a neighbouring chip. */
const CHIP = "mx-px rounded-(--wb-radius-xs) px-0.5 box-decoration-clone";

export function DiffWords({ ops }: { ops: DiffOp[] }) {
  return (
    <p
      className="wb-diff-words m-0 whitespace-pre-wrap break-words type-body leading-6 select-text"
      data-testid="wb-diff-words"
    >
      {ops.map((op, index) => {
        if (op.kind === "delete") {
          return (
            <del
              key={index}
              className={`${CHIP} bg-(--wb-danger-soft) text-(--wb-ink-2) decoration-(--wb-danger)`}
              data-diff="delete"
            >
              {op.text}
            </del>
          );
        }
        if (op.kind === "insert") {
          return (
            <ins
              key={index}
              className={`${CHIP} bg-(--wb-data-4) text-(--wb-ink) no-underline`}
              data-diff="insert"
            >
              {op.text}
            </ins>
          );
        }
        return <span key={index}>{op.text}</span>;
      })}
    </p>
  );
}

interface DiffCardProps {
  run: ForkTransformViewChanges;
  diff: WordDiff;
  onConfigure: () => void;
  onRate: (rating: -1 | 1) => void;
  onRetry: () => Promise<void>;
}

export function DiffCard({
  run,
  diff,
  onConfigure,
  onRate,
  onRetry,
}: DiffCardProps) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [copied, setCopied] = useState(false);
  const [retrying, setRetrying] = useState(false);
  const [error, setError] = useState(false);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(run.result_text);
      setCopied(true);
    } catch {
      setError(true);
    }
  };
  const retry = async () => {
    setRetrying(true);
    setError(false);
    try {
      await onRetry();
    } catch {
      setError(true);
    } finally {
      setRetrying(false);
    }
  };

  return (
    <section
      aria-labelledby="wb-diff-title"
      className="flex flex-col gap-4 rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) p-5 shadow-(--wb-elev-1)"
      data-testid="wb-transform-diff"
    >
      <div className="flex items-center justify-between gap-3">
        <h2
          id="wb-diff-title"
          className="type-card-title"
          data-testid="wb-diff-count"
        >
          {t("transforms.diff.changes", { count: diff.changes })}
        </h2>
        <Button
          variant="ghost"
          size="sm"
          onClick={onConfigure}
          data-testid="wb-diff-configure"
        >
          <Settings2 size={14} aria-hidden="true" />
          {t("transforms.diff.configure")}
        </Button>
      </div>
      <div className="max-h-72 overflow-y-auto rounded-(--wb-radius-sm) border border-(--wb-line) bg-(--wb-panel) p-4">
        <DiffWords ops={diff.ops} />
      </div>
      {diff.degraded && (
        <p className="type-small text-(--wb-ink-2)">
          {t("transforms.diff.simpleView")}
        </p>
      )}
      {error && (
        <p role="alert" className="type-small text-(--wb-danger)">
          {t("transforms.diff.actionFailed")}
        </p>
      )}
      <div className="flex flex-wrap items-center gap-2">
        <Button onClick={() => void copy()} data-testid="wb-diff-copy">
          {copied ? (
            <Check size={14} aria-hidden="true" />
          ) : (
            <Clipboard size={14} aria-hidden="true" />
          )}
          {copied ? t("transforms.diff.copied") : t("transforms.diff.copy")}
        </Button>
        <Button
          disabled={retrying}
          onClick={() => void retry()}
          data-testid="wb-diff-retry"
        >
          <RotateCcw size={14} aria-hidden="true" />
          {retrying
            ? t("transforms.diff.retrying")
            : t("transforms.diff.retry")}
        </Button>
        <div
          className="ms-auto flex items-center gap-1"
          role="group"
          aria-label={t("transforms.diff.feedback")}
        >
          {([1, -1] as const).map((rating) => {
            const selected = run.rating === rating;
            const Icon = rating === 1 ? ThumbsUp : ThumbsDown;
            const label = t(
              rating === 1 ? "transforms.diff.good" : "transforms.diff.bad",
            );
            return (
              <button
                key={rating}
                type="button"
                aria-label={label}
                title={label}
                aria-pressed={selected}
                onClick={() => onRate(rating)}
                className={`grid size-8 cursor-pointer place-items-center rounded-(--wb-radius-sm) ${selected ? "bg-(--wb-fill-active) text-(--wb-accent)" : "text-(--wb-ink-2) hover:bg-(--wb-fill-hover)"} ${FOCUS_RING}`}
                data-testid={rating === 1 ? "wb-diff-good" : "wb-diff-bad"}
              >
                <Icon size={16} aria-hidden="true" />
              </button>
            );
          })}
        </div>
      </div>
    </section>
  );
}
