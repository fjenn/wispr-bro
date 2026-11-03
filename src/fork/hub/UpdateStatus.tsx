// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Update status (S-28, E-51): the Hub footer's "Up to date", "Update ready ·
 * Restart now" and "Couldn't check · Retry", and the fuller block on the
 * About page. The backend (`fork/update/scheduler.rs`) checks, downloads and
 * installs on its own; this only shows `fork-update-state` and offers the
 * one action that fits the state. Replaces Handy's `UpdateChecker`, which
 * the fork no longer mounts.
 */
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands, events, type ForkUpdateState } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button, FOCUS_RING } from "../ui";
import { navigate } from "./nav";

/** The live update state (null until the first answer). */
export function useUpdateState(): [
  ForkUpdateState | null,
  (s: ForkUpdateState) => void,
] {
  const [state, setState] = useState<ForkUpdateState | null>(null);
  useEffect(() => {
    let cancelled = false;
    commands
      .forkUpdateState()
      .then((s) => {
        if (!cancelled) setState(s);
      })
      .catch(() => {
        // No state, no status line; the version still shows.
      });
    const stop = events.forkUpdateState.listen((event) => {
      if (!cancelled) setState(event.payload);
    });
    return () => {
      cancelled = true;
      void stop.then((f) => f());
    };
  }, []);
  return [state, setState];
}

type Problem = "busy" | "failed" | null;

/** Check and restart with the refusal reason kept for the UI. */
function useUpdateActions(setState: (s: ForkUpdateState) => void) {
  const [problem, setProblem] = useState<Problem>(null);
  const check = useCallback(async () => {
    setProblem(null);
    try {
      const result = await commands.forkUpdateCheckNow();
      if (result.status === "ok") setState(result.data);
      else setProblem("failed");
    } catch {
      setProblem("failed");
    }
  }, [setState]);
  const restart = useCallback(async () => {
    setProblem(null);
    try {
      const result = await commands.forkUpdateRestartNow();
      if (result.status === "error") {
        const error = result.error;
        const busy =
          error.kind === "Internal" && error.message.includes("dictation");
        setProblem(busy ? "busy" : "failed");
      }
    } catch {
      setProblem("failed");
    }
  }, []);
  return { problem, check, restart };
}

function timeLabel(ms: number, language: string): string {
  return new Intl.DateTimeFormat(language, {
    weekday: "short",
    hour: "numeric",
    minute: "2-digit",
  }).format(new Date(ms));
}

interface LineProps {
  state: ForkUpdateState;
  onCheck: () => void;
  onRestart: () => void;
  problem: Problem;
  compact: boolean;
}

/** One line: what is happening, plus the single action that fits. */
function StatusLine({ state, onCheck, onRestart, problem, compact }: LineProps) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const linkClass = `cursor-pointer rounded-sm font-medium text-(--wb-ink) underline underline-offset-2 ${FOCUS_RING}`;
  const action = (label: string, onClick: () => void, testId: string) =>
    compact ? (
      <button
        type="button"
        data-testid={testId}
        className={linkClass}
        onClick={onClick}
      >
        {label}
      </button>
    ) : (
      <Button data-testid={testId} onClick={onClick}>
        {label}
      </Button>
    );
  const version = state.version ?? "";
  let text: string;
  let act: React.ReactNode = null;
  switch (state.phase) {
    case "off":
      text = t("hub.update.off");
      act = action(
        t("hub.update.turnOn"),
        () => navigate("settings", "privacy"),
        "wb-update-turn-on",
      );
      break;
    case "idle":
      text = compact ? "" : t("hub.update.notChecked");
      act = action(t("hub.update.check"), onCheck, "wb-update-check");
      break;
    case "checking":
      text = t("hub.update.checking");
      break;
    case "up_to_date":
      text = t("hub.update.upToDate");
      if (!compact) {
        act = action(t("hub.update.checkAgain"), onCheck, "wb-update-check");
      }
      break;
    case "check_failed":
      text = t("hub.update.checkFailed");
      act = action(t("hub.update.retry"), onCheck, "wb-update-retry");
      break;
    case "downloading":
      text = t("hub.update.downloading", { version });
      break;
    case "ready":
      text = t("hub.update.ready", { version });
      act = action(t("hub.update.restartNow"), onRestart, "wb-update-restart");
      break;
    case "installing":
      text = t("hub.update.installing", { version });
      break;
    case "installed":
      text = state.busy
        ? t("hub.update.installedWaiting", { version })
        : t("hub.update.restarting");
      break;
    case "install_failed":
      text = t("hub.update.installFailed");
      act = action(t("hub.update.retry"), onCheck, "wb-update-retry");
      break;
  }
  return (
    <span className="flex items-center gap-1.5" data-phase={state.phase}>
      {text && (
        <span data-testid="wb-update-text" role="status" aria-live="polite">
          {text}
        </span>
      )}
      {text && act && compact && <span aria-hidden="true">·</span>}
      {act}
      {problem && (
        <span data-testid="wb-update-problem" role="alert">
          {problem === "busy"
            ? t("hub.update.busy")
            : t("hub.update.actionFailed")}
        </span>
      )}
    </span>
  );
}

/** The footer's status, left of the version. */
export function UpdateStatus() {
  const [state, setState] = useUpdateState();
  const { problem, check, restart } = useUpdateActions(setState);
  if (!state) return null;
  return (
    <span data-testid="wb-update-status" className="flex items-center">
      <StatusLine
        state={state}
        onCheck={() => void check()}
        onRestart={() => void restart()}
        problem={problem}
        compact
      />
    </span>
  );
}

/** About page: status, details and the action as a button. */
export function UpdatePanel() {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const [state, setState] = useUpdateState();
  const { problem, check, restart } = useUpdateActions(setState);
  if (!state) return null;
  const details: string[] = [];
  if (state.last_checked_ms !== null && state.phase !== "off") {
    details.push(
      t("hub.update.lastChecked", {
        time: timeLabel(state.last_checked_ms, i18n.language),
      }),
    );
  }
  if (state.paused_until_ms !== null) {
    details.push(
      t("hub.update.paused", {
        time: timeLabel(state.paused_until_ms, i18n.language),
      }),
    );
  }
  if (state.needs_password && state.phase !== "off") {
    details.push(t("hub.update.needsPassword"));
  }
  if (!state.automatic && state.phase !== "off") {
    details.push(t("hub.update.manualOnly"));
  }
  details.push(
    state.phase === "ready" ? t("hub.update.readyHint") : t("hub.update.policy"),
  );
  return (
    <div
      data-testid="wb-update-panel"
      className="flex flex-col gap-2 text-sm text-(--wb-ink-2)"
    >
      <div className="flex flex-wrap items-center gap-3 text-(--wb-ink)">
        <StatusLine
          state={state}
          onCheck={() => void check()}
          onRestart={() => void restart()}
          problem={problem}
          compact={false}
        />
      </div>
      {details.map((line) => (
        <p key={line}>{line}</p>
      ))}
    </div>
  );
}
