// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Apple Intelligence row of Settings › Advanced › AI engine (S-12b):
 * whether Apple's on-device model can clean dictations on this Mac, what
 * Wispr Bro uses it for (RS-08: a stand-in while the cleanup model cannot
 * run), and how often it declined or was throttled since launch. The user
 * never picks an engine; this only explains which one runs.
 */
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands, type ForkAppleStatus } from "../../bindings";
import { FORK_NAMESPACE } from "../../i18n";

/** i18n key of the one-line state. */
export function appleStateKey(status: ForkAppleStatus): string {
  return status.availability === "available"
    ? `aiEngine.apple.state.available_${status.role}`
    : `aiEngine.apple.state.${status.availability}`;
}

/**
 * Loads Apple's status once and again whenever `refresh` changes (after a
 * Try it, so its counts move).
 */
export function useAppleStatus(refresh: unknown): ForkAppleStatus | null {
  const [status, setStatus] = useState<ForkAppleStatus | null>(null);
  useEffect(() => {
    let disposed = false;
    commands
      .forkLlmAppleStatus()
      .then((s) => {
        if (!disposed) setStatus(s);
      })
      .catch((e) => console.warn("fork: fork_llm_apple_status failed", e));
    return () => {
      disposed = true;
    };
  }, [refresh]);
  return status;
}

export function AppleEngine({ status }: { status: ForkAppleStatus | null }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  if (!status) return null;
  const available = status.availability === "available";
  return (
    <section
      className="flex flex-col gap-1 border-t border-(--wb-line) pt-3"
      aria-labelledby="fork-ai-engine-apple-title"
      data-testid="fork-ai-engine-apple"
      data-availability={status.availability}
    >
      <h3 id="fork-ai-engine-apple-title" className="text-[13px] font-medium">
        {t("aiEngine.apple.title")}
      </h3>
      <p
        className={available ? "text-ink" : "text-ink-2"}
        data-testid="fork-ai-engine-apple-state"
      >
        {t(appleStateKey(status))}
      </p>
      {available && status.requests > 0 && (
        <p
          className="text-[13px] text-ink-2"
          data-testid="fork-ai-engine-apple-counts"
        >
          {t("aiEngine.apple.counts", {
            requests: status.requests,
            refusals: status.refusals,
            rateLimited: status.rate_limited,
          })}
        </p>
      )}
    </section>
  );
}
