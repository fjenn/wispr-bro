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
import { OptionCard, RadioGroup } from "../ui";

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
        count: r.chunks_total,
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
      <p
        className="text-[13px] text-(--wb-ink-3)"
        data-testid="fork-cleanup-loading"
      >
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
    <div className="flex flex-col gap-4" data-testid="fork-cleanup">
      <fieldset
        className="flex flex-col gap-3"
        disabled={!!pending.cleanup_level}
        aria-busy={!!pending.cleanup_level}
      >
        <legend id={legendId} className="type-micro mb-3 text-(--wb-ink-3)">
          {t("cleanup.picker.legend")}
        </legend>
        <RadioGroup
          name={name}
          value={current}
          onValueChange={(level) =>
            void setSetting("cleanup_level", level as CleanupLevel)
          }
          labelledBy={legendId}
          disabled={!!pending.cleanup_level}
        >
          {CLEANUP_LEVELS.map((level) => (
            <OptionCard
              key={level}
              value={level}
              testId={`fork-cleanup-level-${level}`}
              title={
                <>
                  {t(`cleanup.levels.${level}.label`)}
                  {level === "light" && (
                    <span className="ms-2 text-[13px] font-normal text-(--wb-ink-2)">
                      {t("cleanup.picker.recommended")}
                    </span>
                  )}
                </>
              }
              description={
                level === "medium"
                  ? `${t("cleanup.levels.medium.description")} ${t("style.cleanup.mediumNote")}`
                  : t(`cleanup.levels.${level}.description`)
              }
            />
          ))}
        </RadioGroup>
      </fieldset>
      <p className="text-[13px] text-(--wb-ink-3)">
        {t("cleanup.picker.aiChat")}
      </p>
      {error && (
        <p role="alert" className="text-[13px] text-(--wb-danger)">
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
        className="flex flex-col gap-1 border-t border-(--wb-line) pt-4"
      >
        <p className="text-[13px] font-medium text-(--wb-ink)">
          {t("cleanup.last.title")}
        </p>
        {!last || !line ? (
          <p className="text-[13px] text-(--wb-ink-3)">
            {t("cleanup.last.none")}
          </p>
        ) : (
          <>
            {last.notice && (
              <p className="text-[13px] font-medium text-(--wb-ink)">
                {t("cleanup.notice")}
              </p>
            )}
            <p className="text-[13px] text-(--wb-ink-2)">
              {t(line.key, {
                ...line.values,
                reason:
                  typeof line.values.reason === "string"
                    ? reasonText(line.values.reason)
                    : undefined,
              })}
            </p>
            {last.guard_reasons.length > 0 && (
              <p className="text-[13px] text-(--wb-ink-3)">
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
