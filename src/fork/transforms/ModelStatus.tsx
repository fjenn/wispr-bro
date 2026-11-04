// SPDX-License-Identifier: GPL-3.0-or-later
// OT2-H: the instruction model's one-time download, shown on the
// Transforms page while Transforms or Command Mode waits for it.
// Downloading → checking the download → a brief "ready" that fades out
// (owner, rc5: the row sat at 100 % with a native bar).
import { useCallback, useEffect, useRef, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { commands, events, type ForkInstructStatus } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button, ProgressLine, useMotionPresets } from "../ui";
import { CheckCircle2, Download, Loader2, TriangleAlert } from "../ui/icons";

/** How long "Transforms are ready" stays before the row fades out. */
export const READY_LINGER_MS = 2500;

const ACTIVE = new Set(["downloading", "verifying", "download_failed"]);

export function ModelStatus({ wanted }: { wanted: boolean }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const m = useMotionPresets();
  const [status, setStatus] = useState<ForkInstructStatus | null>(null);
  // "Ready" shows only after this page saw the download or check run.
  const [justReady, setJustReady] = useState(false);
  const sawWork = useRef(false);

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

  const state = status?.state;
  useEffect(() => {
    if (state && ACTIVE.has(state)) {
      sawWork.current = true;
      setJustReady(false);
    } else if (state === "ready" && sawWork.current) {
      sawWork.current = false;
      setJustReady(true);
      const timer = window.setTimeout(
        () => setJustReady(false),
        READY_LINGER_MS,
      );
      return () => window.clearTimeout(timer);
    }
  }, [state]);

  const retry = useCallback(async () => {
    const r = await commands.forkInstructRetry();
    if (r.status === "ok") setStatus(r.data);
  }, []);

  const visible =
    wanted &&
    !!status &&
    (ACTIVE.has(status.state) || (status.state === "ready" && justReady));
  const percent =
    status && status.total_bytes > 0
      ? Math.min(
          100,
          Math.floor((status.downloaded_bytes / status.total_bytes) * 100),
        )
      : 0;
  const failed = state === "download_failed";
  const text = failed
    ? t("transforms.model.failed")
    : state === "ready"
      ? t("transforms.model.ready")
      : state === "verifying"
        ? t("transforms.model.verifying")
        : t("transforms.model.downloading", { percent });
  const Glyph = failed
    ? TriangleAlert
    : state === "ready"
      ? CheckCircle2
      : state === "verifying"
        ? Loader2
        : Download;

  return (
    <AnimatePresence initial={false}>
      {visible && (
        <motion.div
          key="wb-transforms-model"
          {...m.collapse}
          className="overflow-hidden"
        >
          <div
            className="flex flex-col gap-2 rounded-(--wb-radius-sm) bg-(--wb-surface-tint) px-3 py-2.5"
            data-testid="wb-transforms-model"
            data-state={state}
          >
            <div className="flex min-h-7 flex-wrap items-center gap-x-3 gap-y-2">
              <p
                role={failed ? "alert" : "status"}
                aria-live="polite"
                className="type-small m-0 flex min-w-0 flex-1 items-center gap-1.5 text-(--wb-ink-2)"
              >
                <Glyph
                  size={14}
                  aria-hidden="true"
                  className={`shrink-0 ${failed ? "text-(--wb-danger)" : "text-(--wb-ink-3)"}`}
                />
                <span>{text}</span>
              </p>
              {failed && (
                <Button
                  size="sm"
                  onClick={() => void retry()}
                  data-testid="wb-transforms-model-retry"
                >
                  {t("transforms.model.retry")}
                </Button>
              )}
            </div>
            {(state === "downloading" || state === "verifying") && (
              <ProgressLine
                value={state === "verifying" ? 1 : percent / 100}
                label={t("transforms.model.progressLabel")}
                valueText={text}
                tone="ink"
                thickness={4}
                testId="wb-transforms-model-progress"
              />
            )}
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
