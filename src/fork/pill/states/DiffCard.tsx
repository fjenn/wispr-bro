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
  X,
} from "lucide-react";
import { commands, type ForkPillState } from "../../bindings";
import { FORK_NAMESPACE } from "../../i18n";
import { DiffWords } from "../../transforms/DiffCard";

export function DiffCard({ state }: { state: ForkPillState }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [retrying, setRetrying] = useState(false);
  const value = state.diff;
  if (!value) return null;
  const retry = async () => {
    setRetrying(true);
    await commands.forkPillAction("retry_diff");
    setRetrying(false);
  };
  return (
    <div className="wb-pill-diff" data-testid="wb-pill-diff">
      <div className="wb-pill-diff-header">
        <strong role="status">
          {t("transforms.diff.changes", { count: value.diff.changes })}
        </strong>
        <button
          type="button"
          className="wb-pill-diff-link"
          onClick={() => void commands.forkPillAction("configure_diff")}
          data-testid="wb-pill-diff-configure"
        >
          <Settings2 size={13} aria-hidden="true" />
          {t("transforms.diff.configure")}
        </button>
        <button
          type="button"
          className="wb-pill-btn"
          onClick={() => void commands.forkPillAction("dismiss")}
          aria-label={t("pill.dismiss")}
          title={t("pill.dismiss")}
          data-testid="wb-pill-dismiss"
        >
          <X size={14} aria-hidden="true" />
        </button>
      </div>
      <div className="wb-pill-diff-body">
        <DiffWords ops={value.diff.ops} />
      </div>
      <div className="wb-pill-diff-actions">
        <button
          type="button"
          className="wb-pill-fix"
          onClick={() => void commands.forkPillAction("copy_diff")}
          data-testid="wb-pill-diff-copy"
        >
          {state.copied ? (
            <Check size={13} aria-hidden="true" />
          ) : (
            <Clipboard size={13} aria-hidden="true" />
          )}
          {state.copied
            ? t("transforms.diff.copied")
            : t("transforms.diff.copy")}
        </button>
        <button
          type="button"
          className="wb-pill-diff-link"
          disabled={retrying}
          onClick={() => void retry()}
          data-testid="wb-pill-diff-retry"
        >
          <RotateCcw size={13} aria-hidden="true" />
          {retrying
            ? t("transforms.diff.retrying")
            : t("transforms.diff.retry")}
        </button>
        <span className="wb-pill-diff-spacer" />
        {([1, -1] as const).map((rating) => {
          const Icon = rating === 1 ? ThumbsUp : ThumbsDown;
          const label = t(
            rating === 1 ? "transforms.diff.good" : "transforms.diff.bad",
          );
          return (
            <button
              key={rating}
              type="button"
              className="wb-pill-btn"
              aria-label={label}
              title={label}
              aria-pressed={value.run.rating === rating}
              onClick={() =>
                void commands.forkPillAction({ rate_diff: { rating } })
              }
              data-testid={
                rating === 1 ? "wb-pill-diff-good" : "wb-pill-diff-bad"
              }
            >
              <Icon size={13} aria-hidden="true" />
            </button>
          );
        })}
      </div>
    </div>
  );
}
