// SPDX-License-Identifier: GPL-3.0-or-later
// OT2-H: the instruction model's one-time download, shown on the
// Transforms page while Transforms or Command Mode waits for it.
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands, events, type ForkInstructStatus } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";

export function ModelStatus({ wanted }: { wanted: boolean }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [status, setStatus] = useState<ForkInstructStatus | null>(null);

  useEffect(() => {
    let disposed = false;
    const unlisten = events.forkInstructStatus.listen((e) => {
      if (!disposed) setStatus(e.payload);
    });
    void commands.forkInstructStatus().then((r) => {
      if (!disposed && r.status === "ok") setStatus(r.data);
    });
    return () => {
      disposed = true;
      void unlisten.then((f) => f());
    };
  }, []);

  const retry = useCallback(async () => {
    const r = await commands.forkInstructRetry();
    if (r.status === "ok") setStatus(r.data);
  }, []);

  if (!wanted || !status) return null;
  const { state } = status;
  if (
    state !== "downloading" &&
    state !== "verifying" &&
    state !== "download_failed"
  )
    return null;
  const percent =
    status.total_bytes > 0
      ? Math.min(
          100,
          Math.floor((status.downloaded_bytes / status.total_bytes) * 100),
        )
      : 0;
  return (
    <div
      className="flex flex-wrap items-center gap-3 rounded-(--wb-radius-sm) bg-(--wb-surface-tint) px-3 py-2.5"
      data-testid="wb-transforms-model"
      data-state={state}
    >
      <p
        role={state === "download_failed" ? "alert" : "status"}
        aria-live="polite"
        className="type-small m-0 text-(--wb-ink-2)"
      >
        {state === "download_failed"
          ? t("transforms.model.failed")
          : t("transforms.model.downloading", { percent })}
      </p>
      {state === "downloading" && (
        <progress
          className="w-40"
          max={100}
          value={percent}
          aria-label={t("transforms.model.downloading", { percent })}
        />
      )}
      {state === "download_failed" && (
        <Button
          onClick={() => void retry()}
          data-testid="wb-transforms-model-retry"
        >
          {t("transforms.model.retry")}
        </Button>
      )}
    </div>
  );
}
