// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Fork debug panel: shows `fork_ping`, `fork_model_status` and the last
 * `ForkPingEvent`. Also the mount point for fork panels that land before
 * their home page (R5 §1 build-once rule): register them with
 * `registerForkDebugSection` and they render below the built-in checks.
 */
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import {
  commands,
  events,
  type ForkError,
  type ForkModelStatus,
  type ForkPingEvent,
  type ForkPingReply,
  type Result,
} from "../bindings";
import { FORK_NAMESPACE } from "../i18n";

export interface ForkDebugSection {
  id: string;
  /** Key in the `fork` namespace. */
  titleKey: string;
  render: () => ReactNode;
}

const sections: ForkDebugSection[] = [];

/** Adds a panel to the debug panel (idempotent per `id`). */
export function registerForkDebugSection(section: ForkDebugSection): void {
  if (!sections.some((s) => s.id === section.id)) sections.push(section);
}

type Outcome<T> =
  | { state: "pending" }
  | { state: "done"; result: Result<T, ForkError> }
  | { state: "failed"; message: string };

async function run<T>(call: () => Promise<Result<T, ForkError>>) {
  try {
    return { state: "done", result: await call() } as Outcome<T>;
  } catch (e) {
    return { state: "failed", message: String(e) } as Outcome<T>;
  }
}

function Card({
  title,
  testId,
  children,
}: {
  title: string;
  testId: string;
  children: ReactNode;
}) {
  return (
    <section
      data-testid={testId}
      aria-label={title}
      className="rounded-lg border border-line bg-wash p-3 flex flex-col gap-2"
    >
      <h3 className="text-sm font-semibold">{title}</h3>
      {children}
    </section>
  );
}

function OutcomeView<T>({ outcome }: { outcome: Outcome<T> }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  if (outcome.state === "pending") {
    return <p className="text-xs text-ink-3">{t("debug.pending")}</p>;
  }
  if (outcome.state === "failed") {
    return (
      <p role="alert" className="text-xs text-red-500">
        {t("debug.error", { message: outcome.message })}
      </p>
    );
  }
  const { result } = outcome;
  if (result.status === "error" && result.error.kind === "NotReady") {
    return (
      <p className="text-xs text-ink-3">{t("debug.modelStatus.notReady")}</p>
    );
  }
  const body = result.status === "ok" ? result.data : result.error;
  return (
    <pre className="text-xs whitespace-pre-wrap break-all font-mono select-text">
      {JSON.stringify(body, null, 2)}
    </pre>
  );
}

export function DebugPanel({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [ping, setPing] = useState<Outcome<ForkPingReply>>({
    state: "pending",
  });
  const [models, setModels] = useState<Outcome<ForkModelStatus>>({
    state: "pending",
  });
  const [lastEvent, setLastEvent] = useState<ForkPingEvent | null>(null);
  const closeRef = useRef<HTMLButtonElement>(null);

  const refresh = useCallback(async () => {
    setPing({ state: "pending" });
    setModels({ state: "pending" });
    setPing(await run(commands.forkPing));
    setModels(await run(commands.forkModelStatus));
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    events.forkPingEvent
      .listen((event) => setLastEvent(event.payload))
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch((e) => console.warn("fork: listen failed", e));
    void refresh();
    closeRef.current?.focus();
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [refresh]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="fork-debug-title"
      data-testid="fork-debug-panel"
      className="fixed inset-0 z-[100] flex items-center justify-center bg-black/40 p-4"
    >
      <div className="w-full max-w-lg max-h-full overflow-y-auto rounded-xl border border-line bg-background text-text shadow-lg p-4 flex flex-col gap-3">
        <header className="flex items-start justify-between gap-3">
          <div>
            <h2 id="fork-debug-title" className="text-base font-semibold">
              {t("debug.title")}
            </h2>
            <p className="text-xs text-ink-3">{t("debug.description")}</p>
          </div>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => void refresh()}
              className="px-2 py-1 text-sm rounded border border-line-strong bg-wash hover:bg-logo-primary/10 cursor-pointer"
            >
              {t("debug.runAgain")}
            </button>
            <button
              ref={closeRef}
              type="button"
              onClick={onClose}
              className="px-2 py-1 text-sm rounded border border-line-strong bg-wash hover:bg-logo-primary/10 cursor-pointer"
            >
              {t("debug.close")}
            </button>
          </div>
        </header>
        <Card title={t("debug.ping.title")} testId="fork-debug-ping">
          <OutcomeView outcome={ping} />
        </Card>
        <Card title={t("debug.modelStatus.title")} testId="fork-debug-models">
          <OutcomeView outcome={models} />
        </Card>
        <Card title={t("debug.lastEvent.title")} testId="fork-debug-event">
          {lastEvent ? (
            <pre className="text-xs font-mono select-text">
              {JSON.stringify(lastEvent)}
            </pre>
          ) : (
            <p className="text-xs text-ink-3">{t("debug.lastEvent.none")}</p>
          )}
        </Card>
        {sections.map((section) => (
          <Card
            key={section.id}
            title={t(section.titleKey)}
            testId={`fork-debug-section-${section.id}`}
          >
            {section.render()}
          </Card>
        ))}
      </div>
    </div>
  );
}
