// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Home on iPhone: the Mac's greeting and stats, then what only the phone
 * has: the dictation session, the setup that must be finished first, and
 * a field to try the keyboard in.
 *
 *   Hello, Sam
 *   ┌ setup (until all four are done) ─ or ─ session ┐
 *   │ microphone · keyboard · Full Access · model     │
 *   └─────────────────────────────────────────────────┘
 *   ┌ words · wpm · streak ┐  voice profile
 *   ┌ Try it ──────────────┐
 *   Style ›  Insights ›
 *
 * History has its own tab (`IosHistory`).
 */
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { commands } from "../bindings";
import { Greeting } from "../home/Greeting";
import { StatsRail } from "../home/StatsRail";
import { useStatsSummary } from "../home/useStatsSummary";
import { VoiceProfileBar } from "../home/VoiceProfileBar";
import { navigate } from "../hub/nav";
import { FORK_NAMESPACE } from "../i18n";
import { Button, Dialog, ProgressLine, TextArea, toast } from "../ui";
import { ChartColumn, Check, Palette } from "../ui/icons";
import { iosCommands, type IosSession } from "./commands";
import { sessionTimeoutLabel } from "./format";
import { FullAccessNote, KeyboardSteps } from "./KeyboardSteps";
import { IosGroup, IosNavRow } from "./parts";
import { setupProgress, useIosStore, type SetupProgress } from "./useIos";

export function IosHome() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { summary, status } = useStatsSummary();
  const session = useIosStore((s) => s.session);
  const setup = useIosStore((s) => s.setup);
  const speech = useIosStore((s) => s.speech);
  const progress = setupProgress(setup, speech);

  return (
    <div data-testid="wb-home" className="flex flex-col gap-5">
      <Greeting />
      {progress.known &&
        (progress.done ? (
          <SessionCard session={session} />
        ) : (
          <SetupCard progress={progress} />
        ))}
      <aside
        aria-label={t("home.stats.label")}
        data-testid="wb-home-rail"
        className="@container overflow-hidden rounded-(--wb-radius-lg) border border-(--wb-line) bg-(--wb-surface-tint)"
      >
        <StatsRail summary={summary} status={status} layout="row" />
        <VoiceProfileBar totalWords={summary?.total_words ?? null} />
      </aside>
      <TryIt />
      <IosGroup>
        <IosNavRow
          icon={Palette}
          label={t("hub.pages.style.title")}
          onClick={() => navigate("style")}
          testId="wb-ios-home-style"
        />
        <IosNavRow
          icon={ChartColumn}
          label={t("hub.pages.insights.title")}
          onClick={() => navigate("insights")}
          testId="wb-ios-home-insights"
        />
      </IosGroup>
    </div>
  );
}

/* ── Session ───────────────────────────────────────────────────────── */

function SessionCard({ session }: { session: IosSession | null }) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const refresh = useIosStore((s) => s.refresh);
  const timeout = useIosStore((s) => s.timeout);
  const [busy, setBusy] = useState(false);
  const state = session?.state ?? "noSession";
  const on =
    state === "idle" || state === "listening" || state === "transcribing";

  const act = async (action: () => Promise<void>) => {
    setBusy(true);
    try {
      await action();
    } catch (e) {
      console.warn("ios: session command failed", e);
      toast.error(t("ios.session.failed"));
    }
    await refresh();
    setBusy(false);
  };

  let line: string;
  if (state === "error") {
    line = session?.message || t("ios.session.errorBody");
  } else if (on) {
    const ends = endTime(session?.endsAt, i18n.language);
    line = ends
      ? t("ios.session.onBodyEnds", { time: ends })
      : timeout < 0
        ? t("ios.session.onBodyNever")
        : t("ios.session.onBody");
  } else {
    line = t("ios.session.offBody");
  }

  return (
    <section
      aria-labelledby="wb-ios-session-title"
      data-testid="wb-ios-session"
      data-state={state}
      className={`wb-ios-session ${on ? "wb-ios-session-on" : ""}`}
    >
      <p
        className={`flex items-center gap-2 text-[13px] leading-[18px] font-semibold tracking-[0.02em] ${
          state === "error"
            ? "text-(--wb-danger)"
            : on
              ? "text-(--wb-accent)"
              : "text-(--wb-ink-3)"
        }`}
        data-testid="wb-ios-session-status"
      >
        <span
          aria-hidden="true"
          className={`wb-ios-dot ${state === "listening" ? "wb-ios-dot-live" : ""}`}
        />
        {t(`ios.session.status.${state}`)}
      </p>
      <h2
        id="wb-ios-session-title"
        className="type-serif-title mt-2 text-(--wb-ink)"
      >
        {on ? t("ios.session.onTitle") : t("ios.session.offTitle")}
      </h2>
      <p className="mt-1.5 text-[15px] leading-[21px] text-(--wb-ink-2)">
        {line}
      </p>
      <div className="mt-5">
        {on ? (
          <Button
            variant="outline"
            size="lg"
            block
            disabled={busy}
            onClick={() => void act(iosCommands.endSession)}
            data-testid="wb-ios-session-end"
          >
            {t("ios.session.end")}
          </Button>
        ) : (
          <Button
            variant="primary"
            size="lg"
            block
            disabled={busy}
            onClick={() => void act(iosCommands.startSession)}
            data-testid="wb-ios-session-start"
          >
            {state === "error"
              ? t("ios.session.retry")
              : t("ios.session.start")}
          </Button>
        )}
      </div>
      <button
        type="button"
        onClick={() => navigate("settings", "session")}
        className="wb-ios-link -mb-2 mt-1 self-center"
        data-testid="wb-ios-session-timeout"
      >
        {t("ios.session.timeoutLine", {
          timeout: sessionTimeoutLabel(t, timeout, "sentence"),
        })}
      </button>
    </section>
  );
}

/** `endsAt` as a clock time; seconds or milliseconds since the epoch. */
function endTime(
  endsAt: number | null | undefined,
  language: string,
): string | null {
  if (typeof endsAt !== "number" || !Number.isFinite(endsAt) || endsAt <= 0) {
    return null;
  }
  const ms = endsAt < 1e11 ? endsAt * 1000 : endsAt;
  if (ms < Date.now()) return null;
  try {
    return new Intl.DateTimeFormat(language, {
      hour: "numeric",
      minute: "2-digit",
    }).format(new Date(ms));
  } catch {
    return null;
  }
}

/* ── Setup ─────────────────────────────────────────────────────────── */

type StepId = "mic" | "keyboard" | "fullAccess" | "model";

function SetupCard({ progress }: { progress: SetupProgress }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const setup = useIosStore((s) => s.setup);
  const speech = useIosStore((s) => s.speech);
  const refresh = useIosStore((s) => s.refresh);
  const refreshSpeech = useIosStore((s) => s.refreshSpeech);
  const [howTo, setHowTo] = useState(false);
  const [busy, setBusy] = useState(false);
  const appName = t("rust.appName");

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    try {
      await action();
    } catch (e) {
      console.warn("ios: setup command failed", e);
      toast.error(t("ios.setup.failed"));
    }
    await refresh();
    setBusy(false);
  };

  const micDenied = setup?.micPermission === 1;
  const order: StepId[] = ["mic", "keyboard", "fullAccess", "model"];
  const done: Record<StepId, boolean> = {
    mic: progress.mic,
    keyboard: progress.keyboard,
    fullAccess: progress.fullAccess,
    model: progress.model,
  };
  // The first open step the user can act on carries the button.
  const current = order.find((id) => id !== "model" && !done[id]);
  const count = order.filter((id) => done[id]).length;

  const phase = speech?.phase;
  const percent = Math.round(speech?.percent ?? 0);
  const modelFailed = phase === "failed";
  const modelLine = progress.model
    ? t("ios.setup.model.ready")
    : modelFailed
      ? t("ios.setup.model.failed")
      : phase === "downloading" || phase === "verifying"
        ? t("ios.setup.model.downloading", { percent })
        : t("ios.setup.model.waiting");

  const hint: Record<StepId, string> = {
    mic: micDenied ? t("ios.setup.mic.denied") : t("ios.setup.mic.hint"),
    keyboard: t("ios.setup.keyboard.hint"),
    fullAccess: t("ios.setup.fullAccess.hint"),
    model: modelLine,
  };

  const action = (id: StepId) => {
    if (id === "mic" && !micDenied) {
      return (
        <Button
          variant="primary"
          size="sm"
          disabled={busy}
          onClick={() => void run(iosCommands.requestMic)}
          data-testid="wb-ios-setup-mic-allow"
        >
          {t("ios.setup.mic.allow")}
        </Button>
      );
    }
    if (id === "mic") {
      return (
        <Button
          variant="primary"
          size="sm"
          disabled={busy}
          onClick={() => void run(iosCommands.openSettings)}
          data-testid="wb-ios-setup-mic-settings"
        >
          {t("ios.setup.openSettings")}
        </Button>
      );
    }
    return (
      <Button
        variant="primary"
        size="sm"
        onClick={() => setHowTo(true)}
        data-testid={`wb-ios-setup-${id}-show`}
      >
        {t("ios.setup.showMe")}
      </Button>
    );
  };

  return (
    <section
      aria-labelledby="wb-ios-setup-title"
      data-testid="wb-ios-setup"
      className="wb-ios-session"
    >
      <p className="text-[13px] leading-[18px] font-semibold tracking-[0.02em] text-(--wb-ink-3) tabular-nums">
        {t("ios.setup.count", { done: count, total: order.length })}
      </p>
      <h2
        id="wb-ios-setup-title"
        className="type-serif-title mt-2 text-(--wb-ink)"
      >
        {t("ios.setup.title")}
      </h2>
      <p className="mt-1.5 text-[15px] leading-[21px] text-(--wb-ink-2)">
        {t("ios.setup.body")}
      </p>
      <ol className="mt-4 flex flex-col">
        {order.map((id) => (
          <li
            key={id}
            data-testid={`wb-ios-setup-${id}`}
            data-done={done[id]}
            className="flex items-center gap-3 border-t border-(--wb-line-2) py-3 first:border-t-0"
          >
            <span
              aria-hidden="true"
              className={`grid size-6 shrink-0 place-items-center rounded-full ${
                done[id]
                  ? "bg-(--wb-success) text-(--wb-surface)"
                  : "border-[1.5px] border-(--wb-line-strong)"
              }`}
            >
              {done[id] && <Check size={14} />}
            </span>
            <span className="flex min-w-0 flex-1 flex-col gap-0.5">
              <span
                className={`text-[16px] leading-[22px] font-medium ${done[id] ? "text-(--wb-ink-2)" : "text-(--wb-ink)"}`}
              >
                {t(`ios.setup.${id}.title`, { appName })}
              </span>
              {!done[id] && (
                <span className="text-[13px] leading-[18px] text-(--wb-ink-2)">
                  {hint[id]}
                </span>
              )}
              {id === "model" &&
                !done.model &&
                (phase === "downloading" || phase === "verifying") && (
                  <ProgressLine
                    value={percent / 100}
                    label={t("ios.setup.model.title")}
                    thickness={4}
                    className="mt-1.5 rounded-full"
                    testId="wb-ios-setup-model-progress"
                  />
                )}
            </span>
            {!done[id] && id === current && action(id)}
            {id === "model" && modelFailed && (
              <Button
                variant="secondary"
                size="sm"
                onClick={() =>
                  void commands.forkSpeechSetupRetry().then(refreshSpeech)
                }
                data-testid="wb-ios-setup-model-retry"
              >
                {t("ios.setup.model.retry")}
              </Button>
            )}
          </li>
        ))}
      </ol>
      <Dialog
        open={howTo}
        onOpenChange={setHowTo}
        title={t("ios.keyboard.sheetTitle", { appName })}
        closeLabel={t("ios.close")}
        testId="wb-ios-keyboard-sheet"
        footer={
          <Button
            variant="primary"
            size="lg"
            block
            onClick={() => void run(iosCommands.openSettings)}
            data-testid="wb-ios-keyboard-open-settings"
          >
            {t("ios.setup.openSettings")}
          </Button>
        }
      >
        <div className="flex flex-col gap-5">
          <KeyboardSteps
            keyboardEnabled={progress.keyboard}
            fullAccess={progress.fullAccess}
          />
          <FullAccessNote />
        </div>
      </Dialog>
    </section>
  );
}

/* ── Try it ────────────────────────────────────────────────────────── */

function TryIt() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [text, setText] = useState("");
  const appName = t("rust.appName");
  return (
    <section
      aria-labelledby="wb-ios-try-title"
      data-testid="wb-ios-try"
      className="flex flex-col gap-2"
    >
      <div className="flex min-h-7 items-center justify-between px-1">
        <h2 id="wb-ios-try-title" className="type-card-title text-(--wb-ink)">
          {t("ios.tryIt.title")}
        </h2>
        {text && (
          <button
            type="button"
            className="wb-ios-link"
            onClick={() => setText("")}
            data-testid="wb-ios-try-clear"
          >
            {t("ios.tryIt.clear")}
          </button>
        )}
      </div>
      <TextArea
        value={text}
        onChange={(event) => setText(event.currentTarget.value)}
        rows={4}
        placeholder={t("ios.tryIt.placeholder")}
        aria-labelledby="wb-ios-try-title"
        aria-describedby="wb-ios-try-hint"
        className="w-full resize-none rounded-(--wb-radius-lg)!"
        data-testid="wb-ios-try-field"
      />
      <p
        id="wb-ios-try-hint"
        className="px-1 text-[13px] leading-[18px] text-(--wb-ink-3)"
      >
        {t("ios.tryIt.hint", { appName })}
      </p>
    </section>
  );
}
