// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Cleanup level picker (S-12a): None / Light / Medium, saved in the fork
 * setting `cleanup_level`. Mounted in Style › Cleanup (S-19a, `CleanupTab`)
 * and Settings › Advanced (S-14).
 *
 * Below it, the last dictation's cleanup result (`fork_llm_last_cleanup`,
 * then the `ForkCleanupResult` event), so a take that was inserted without
 * cleanup says why until the pill (S-03) shows it.
 */
import { useEffect, useId, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  commands,
  events,
  type CleanupLevel,
  type ForkCleanupResult,
} from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettings } from "../settings/useForkSettings";

export const CLEANUP_LEVELS: readonly CleanupLevel[] = [
  "none",
  "light",
  "medium",
];

/** The line describing a take's cleanup, as an i18n key and its values. */
export function resultLine(r: ForkCleanupResult): {
  key: string;
  values: Record<string, string | number>;
} {
  if (r.outcome === "applied") {
    return {
      key: "cleanup.last.applied",
      values: { count: r.fixes, ms: r.llm_ms },
    };
  }
  if (r.why && r.chunks_applied > 0) {
    // Cleaned in part: some paragraphs went through, others did not.
    return {
      key: "cleanup.last.partial",
      values: {
        applied: r.chunks_applied,
        total: r.chunks_total,
        reason: r.why,
      },
    };
  }
  return { key: `cleanup.why.${r.why ?? "error"}`, values: {} };
}

function useLastCleanup(): ForkCleanupResult | null {
  const [last, setLast] = useState<ForkCleanupResult | null>(null);
  useEffect(() => {
    let disposed = false;
    const unlisten = events.forkCleanupResult.listen((e) => {
      if (!disposed) setLast(e.payload);
    });
    void commands
      .forkLlmLastCleanup()
      .then((r) => {
        // An event that arrived first is newer.
        if (!disposed && r) setLast((cur) => cur ?? r);
      })
      .catch((e) => console.warn("fork: fork_llm_last_cleanup failed", e));
    return () => {
      disposed = true;
      void unlisten.then((f) => f());
    };
  }, []);
  return last;
}

export function CleanupLevelPicker() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, status, error, pending, setSetting } = useForkSettings();
  const last = useLastCleanup();
  const name = useId();
  const legendId = useId();

  if (!settings) {
    return (
      <p className="text-xs text-ink-3" data-testid="fork-cleanup-loading">
        {status === "error"
          ? t("cleanup.loadError", {
              message:
                error && "message" in error ? error.message : error?.kind,
            })
          : t("cleanup.loading")}
      </p>
    );
  }

  const current = settings.cleanup_level;
  const line = last ? resultLine(last) : null;
  const reasonText = (why: string) => t(`cleanup.reason.${why}`);

  return (
    <div className="flex flex-col gap-3 text-sm" data-testid="fork-cleanup">
      <fieldset
        className="flex flex-col gap-2"
        aria-labelledby={legendId}
        disabled={!!pending.cleanup_level}
        aria-busy={!!pending.cleanup_level}
      >
        <legend id={legendId} className="mb-1 font-medium">
          {t("cleanup.picker.legend")}
        </legend>
        {CLEANUP_LEVELS.map((level) => {
          const id = `${name}-${level}`;
          return (
            <label
              key={level}
              htmlFor={id}
              data-testid={`fork-cleanup-level-${level}`}
              data-selected={current === level}
              className={`flex cursor-pointer items-start gap-2 rounded border px-3 py-2 ${
                current === level
                  ? "border-ink bg-wash"
                  : "border-line-strong hover:bg-wash"
              }`}
            >
              <input
                id={id}
                type="radio"
                name={name}
                value={level}
                checked={current === level}
                onChange={() => void setSetting("cleanup_level", level)}
                aria-describedby={`${id}-desc`}
                className="mt-1"
              />
              <span className="flex flex-col">
                <span className="font-medium">
                  {t(`cleanup.levels.${level}.label`)}
                  {level === "light" && (
                    <span className="ms-2 text-xs font-normal text-ink-2">
                      {t("cleanup.picker.recommended")}
                    </span>
                  )}
                </span>
                <span id={`${id}-desc`} className="text-xs text-ink-2">
                  {t(`cleanup.levels.${level}.description`)}
                </span>
              </span>
            </label>
          );
        })}
      </fieldset>
      <p className="text-xs text-ink-3">{t("cleanup.picker.aiChat")}</p>
      {error && (
        <p role="alert" className="text-xs text-red-500">
          {t("cleanup.setError", {
            message: "message" in error ? error.message : error.kind,
          })}
        </p>
      )}
      <div
        aria-live="polite"
        data-testid="fork-cleanup-last"
        data-outcome={last?.outcome ?? ""}
        data-notice={last?.notice ? "true" : "false"}
        className="flex flex-col gap-1"
      >
        <p className="text-xs font-medium">{t("cleanup.last.title")}</p>
        {!last || !line ? (
          <p className="text-xs text-ink-3">{t("cleanup.last.none")}</p>
        ) : (
          <>
            {last.notice && (
              <p className="text-xs font-medium">{t("cleanup.notice")}</p>
            )}
            <p className="text-xs text-ink-2">
              {t(line.key, {
                ...line.values,
                reason:
                  typeof line.values.reason === "string"
                    ? reasonText(line.values.reason)
                    : undefined,
              })}
            </p>
            {last.guard_reasons.length > 0 && (
              <p className="text-xs text-ink-3">
                {t("cleanup.last.checks", {
                  checks: last.guard_reasons.join(", "),
                })}
              </p>
            )}
          </>
        )}
      </div>
    </div>
  );
}
