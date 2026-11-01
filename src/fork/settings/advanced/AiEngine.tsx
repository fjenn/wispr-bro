// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Settings › Advanced › "AI engine" (S-10). Shows whether the local cleanup
 * model is ready and lets the user try it on a sample sentence, cleaned the
 * way a dictation would be (S-12a: level, gate, guards). S-14 mounts this
 * component in the Settings modal's Advanced section. S-12b: the Apple
 * Intelligence row below it, and Try it
 * also runs on Apple's model while it stands in for the cleanup model.
 */
import { useCallback, useEffect, useId, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  commands,
  events,
  type ForkLlmError,
  type ForkLlmStatus,
  type ForkLlmTryResult,
} from "../../bindings";
import { FORK_NAMESPACE } from "../../i18n";
import { Button } from "../../ui";
import { AppleEngine, useAppleStatus } from "./AppleEngine";

type TryState =
  | { kind: "idle" }
  | { kind: "running" }
  | { kind: "done"; result: ForkLlmTryResult }
  | { kind: "error"; error: ForkLlmError | { kind: "Ipc"; message: string } };

/** Percent of the download, 0-100, or `null` when the size is unknown. */
export function downloadPercent(status: ForkLlmStatus): number | null {
  if (status.total_bytes <= 0) return null;
  return Math.min(
    100,
    Math.floor((status.downloaded_bytes / status.total_bytes) * 100),
  );
}

/** i18n key of the one-line state shown at the top. */
export function stateKey(status: ForkLlmStatus): string {
  return `aiEngine.state.${status.state}`;
}

export function AiEngine() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [status, setStatus] = useState<ForkLlmStatus | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [text, setText] = useState(() => t("aiEngine.try.sample"));
  const [tryState, setTryState] = useState<TryState>({ kind: "idle" });
  const textId = useId();
  const apple = useAppleStatus(
    tryState.kind === "done" ? tryState.result : null,
  );

  useEffect(() => {
    let disposed = false;
    const unlisten = events.forkLlmStatus.listen((e) => {
      if (!disposed) setStatus(e.payload);
    });
    void commands.forkLlmStatus().then((r) => {
      if (disposed) return;
      if (r.status === "ok") setStatus(r.data);
      else setLoadError(r.error.kind);
    });
    return () => {
      disposed = true;
      void unlisten.then((f) => f());
    };
  }, []);

  const retry = useCallback(async () => {
    const r = await commands.forkLlmRetry();
    if (r.status === "ok") setStatus(r.data);
  }, []);

  const tryIt = useCallback(async () => {
    setTryState({ kind: "running" });
    try {
      const r = await commands.forkLlmTry(text);
      setTryState(
        r.status === "ok"
          ? { kind: "done", result: r.data }
          : { kind: "error", error: r.error },
      );
    } catch (e) {
      setTryState({
        kind: "error",
        error: { kind: "Ipc", message: String(e) },
      });
    }
  }, [text]);

  if (!status) {
    return (
      <p className="text-xs text-ink-3" data-testid="fork-ai-engine-loading">
        {loadError
          ? t("aiEngine.loadError", { message: loadError })
          : t("aiEngine.loading")}
      </p>
    );
  }

  const percent = downloadPercent(status);
  const ready = status.state === "ready";
  // Try it also works while Apple's model stands in for the cleanup model.
  const canTry = ready || apple?.availability === "available";
  const serverKey = `aiEngine.server.${status.server.phase}`;

  return (
    <div className="flex flex-col gap-3 text-sm" data-testid="fork-ai-engine">
      <div className="flex flex-col gap-1">
        <p
          role="status"
          aria-live="polite"
          data-testid="fork-ai-engine-state"
          data-state={status.state}
          className={ready ? "font-medium text-ink" : "text-ink-2"}
        >
          {t(stateKey(status), { percent: percent ?? 0 })}
        </p>
        {status.state === "downloading" && percent !== null && (
          <progress
            className="w-full"
            max={100}
            value={percent}
            aria-label={t("aiEngine.progressLabel")}
            aria-valuetext={t("aiEngine.state.downloading", { percent })}
          />
        )}
        {status.model_name && (
          <p className="text-xs text-ink-3">
            {t("aiEngine.model", { name: status.model_name })}
          </p>
        )}
        {(ready || status.server.phase !== "stopped") && (
          <p className="text-xs text-ink-3" data-testid="fork-ai-engine-server">
            {t(serverKey)}
            {status.server.device === "cpu" && ` · ${t("aiEngine.cpuOnly")}`}
          </p>
        )}
        {status.state === "download_failed" && (
          <div className="flex items-center gap-2">
            {status.error && (
              <p role="alert" className="text-xs text-red-500">
                {t("aiEngine.downloadError", { message: status.error })}
              </p>
            )}
            <Button onClick={() => void retry()}>{t("aiEngine.retry")}</Button>
          </div>
        )}
      </div>

      <div className="flex flex-col gap-2">
        <label htmlFor={textId} className="text-xs font-medium">
          {t("aiEngine.try.label")}
        </label>
        <textarea
          id={textId}
          data-testid="fork-ai-engine-input"
          value={text}
          rows={3}
          maxLength={2000}
          onChange={(e) => setText(e.currentTarget.value)}
          className="rounded border border-line-strong bg-wash px-2 py-1 text-sm"
        />
        <div>
          <Button
            variant="primary"
            data-testid="fork-ai-engine-try"
            disabled={
              !canTry || tryState.kind === "running" || text.trim() === ""
            }
            aria-describedby={canTry ? undefined : `${textId}-hint`}
            onClick={() => void tryIt()}
          >
            {tryState.kind === "running"
              ? t("aiEngine.try.running")
              : t("aiEngine.try.button")}
          </Button>
          {!canTry && (
            <p id={`${textId}-hint`} className="mt-1 text-xs text-ink-3">
              {t("aiEngine.try.notReady")}
            </p>
          )}
        </div>
        <div aria-live="polite" data-testid="fork-ai-engine-result">
          {tryState.kind === "done" && (
            <div className="flex flex-col gap-1">
              <p className="text-xs font-medium">{t("aiEngine.try.result")}</p>
              <p className="rounded bg-wash px-2 py-1 select-text">
                {tryState.result.cleaned || t("aiEngine.try.empty")}
              </p>
              {tryState.result.why && (
                <p
                  className="text-xs text-ink-2"
                  data-testid="fork-ai-engine-why"
                >
                  {t(`cleanup.why.${tryState.result.why}`)}
                </p>
              )}
              {tryState.result.engine === "apple" && (
                <p
                  className="text-xs text-ink-2"
                  data-testid="fork-ai-engine-engine"
                >
                  {t("aiEngine.try.ranOnApple")}
                </p>
              )}
              <p className="text-xs text-ink-3">
                {tryState.result.start_ms > 0
                  ? t("aiEngine.try.timingCold", {
                      ms: tryState.result.cleanup_ms,
                      start: tryState.result.start_ms,
                    })
                  : t("aiEngine.try.timing", {
                      ms: tryState.result.cleanup_ms,
                    })}
              </p>
            </div>
          )}
          {tryState.kind === "error" && (
            <p role="alert" className="text-xs text-red-500">
              {tryState.error.kind === "LowMemory"
                ? t("aiEngine.try.lowMemory")
                : tryState.error.kind === "NotReady"
                  ? t("aiEngine.try.notReady")
                  : tryState.error.kind === "BadReply"
                    ? t("aiEngine.try.badReply")
                    : t("aiEngine.try.failed", {
                        message:
                          "message" in tryState.error
                            ? tryState.error.message
                            : "",
                      })}
            </p>
          )}
        </div>
      </div>
      <AppleEngine status={apple} />
    </div>
  );
}
