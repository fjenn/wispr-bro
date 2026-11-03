// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * "How fast do you type?" (S-29b, B-29, W-18): a 30-second typing test on a
 * paragraph in the chosen language (`samples/`). The clock starts at the
 * first key; the live number is words a minute on correctly typed
 * characters (`typing.ts`), with the seconds left. Pasting is blocked (it
 * would not measure typing). At the end: Try again, or Recalculate, which
 * saves the result (`typing_wpm`) and returns to the time-saved estimate.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Keyboard } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useSettings } from "@/hooks/useSettings";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettings } from "../settings/useForkSettings";
import { Button } from "../ui";
import { defaultSampleLanguage, sampleFor, TYPING_SAMPLES } from "./samples";
import {
  MIN_LIVE_SECONDS,
  savableWpm,
  TEST_SECONDS,
  wordsPerMinute,
} from "./typing";

type Run = "ready" | "running" | "done";

const TICK_MS = 200;

function languageName(code: string, ui: string): string {
  try {
    return new Intl.DisplayNames([ui], { type: "language" }).of(code) ?? code;
  } catch {
    return code;
  }
}

export function TypingTest({
  preview,
  onDone,
  onSkip,
}: {
  preview: boolean;
  /** The result was saved: back to the estimate. */
  onDone: () => void;
  onSkip: () => void;
}) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const { settings: handy } = useSettings();
  const { setSetting } = useForkSettings();
  const spoken = handy?.spoken_languages ?? [];
  const [language, setLanguage] = useState(() =>
    defaultSampleLanguage(spoken, i18n.language),
  );
  const sample = sampleFor(language).text;
  const [typed, setTyped] = useState("");
  const [run, setRun] = useState<Run>("ready");
  const [elapsed, setElapsed] = useState(0);
  const [busy, setBusy] = useState(false);
  const started = useRef<number | null>(null);
  const input = useRef<HTMLTextAreaElement>(null);

  const reset = useCallback(() => {
    started.current = null;
    setTyped("");
    setElapsed(0);
    setRun("ready");
    input.current?.focus();
  }, []);

  // The clock: ticks while running, stops at 30 s.
  useEffect(() => {
    if (run !== "running") return;
    const tick = window.setInterval(() => {
      if (started.current == null) return;
      const seconds = (performance.now() - started.current) / 1000;
      if (seconds >= TEST_SECONDS) {
        setElapsed(TEST_SECONDS);
        setRun("done");
      } else {
        setElapsed(seconds);
      }
    }, TICK_MS);
    return () => window.clearInterval(tick);
  }, [run]);

  // The test ends early when the whole paragraph is typed.
  const finishedEarly = run === "running" && typed.length >= sample.length;
  useEffect(() => {
    if (!finishedEarly || started.current == null) return;
    setElapsed(
      Math.max(MIN_LIVE_SECONDS, (performance.now() - started.current) / 1000),
    );
    setRun("done");
  }, [finishedEarly]);

  const onType = (value: string) => {
    if (run === "done") return;
    if (run === "ready" && value) {
      started.current = performance.now();
      setRun("running");
    }
    setTyped(value);
  };

  const wpm = wordsPerMinute(sample, typed, elapsed);
  const live = elapsed >= MIN_LIVE_SECONDS ? wpm : null;
  const result = run === "done" ? savableWpm(wpm) : null;
  const secondsLeft = Math.max(0, Math.ceil(TEST_SECONDS - elapsed));

  const recalculate = async () => {
    if (result == null) return;
    if (!preview) {
      setBusy(true);
      await setSetting("typing_wpm", result);
      setBusy(false);
    }
    onDone();
  };

  const chars = useMemo(() => Array.from(sample), [sample]);
  const typedChars = Array.from(typed);

  return (
    <div className="mx-auto flex w-full max-w-[640px] flex-col items-center gap-4">
      <div className="flex w-full items-start justify-between gap-3">
        <div className="min-w-0 flex-1 text-start">
          <h1 className="m-0 font-(--wb-font-display) text-3xl font-semibold text-balance">
            {t("onboardingFlow.typing.title")}
          </h1>
          <p className="mt-2 mb-0 text-sm leading-6 text-pretty text-(--wb-ink-2)">
            {t("onboardingFlow.typing.instruction", {
              count: TEST_SECONDS,
              seconds: TEST_SECONDS,
            })}
          </p>
        </div>
        <button
          type="button"
          onClick={onSkip}
          data-testid="wb-typing-skip"
          className="shrink-0 cursor-pointer rounded-lg px-3 py-1.5 text-sm font-medium text-(--wb-ink-2) hover:bg-(--wb-hover) hover:text-(--wb-ink) focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-(--wb-accent)"
        >
          {t("onboardingFlow.typing.skip")}
        </button>
      </div>

      <div className="flex w-full flex-wrap items-center justify-between gap-3">
        <label className="flex items-center gap-2 text-sm text-(--wb-ink-2)">
          {t("onboardingFlow.typing.language")}
          <select
            value={language}
            disabled={run === "running"}
            onChange={(e) => {
              setLanguage(e.target.value);
              reset();
            }}
            data-testid="wb-typing-language"
            className="cursor-pointer rounded-lg border border-(--wb-line-strong) bg-(--wb-card) px-2 py-1 text-sm text-(--wb-ink) focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-(--wb-accent) disabled:cursor-default disabled:opacity-60"
          >
            {TYPING_SAMPLES.map((s) => (
              <option key={s.language} value={s.language}>
                {languageName(s.language, i18n.language)}
              </option>
            ))}
          </select>
        </label>
        <div className="flex items-center gap-4 text-sm tabular-nums">
          <span data-testid="wb-typing-wpm" data-wpm={live ?? ""}>
            {live == null
              ? t("onboardingFlow.typing.wpmPending")
              : t("onboardingFlow.typing.wpm", { count: live })}
          </span>
          <span
            className="text-(--wb-ink-2)"
            data-testid="wb-typing-seconds"
            data-seconds={secondsLeft}
          >
            {t("onboardingFlow.typing.secondsLeft", { count: secondsLeft })}
          </span>
        </div>
      </div>

      <div className="w-full rounded-(--wb-radius-card) border border-(--wb-line) bg-(--wb-card) p-4 text-start">
        <p className="sr-only" lang={language}>
          {sample}
        </p>
        <p
          aria-hidden="true"
          lang={language}
          dir="ltr"
          className="m-0 font-mono text-[14px] leading-7 break-words whitespace-pre-wrap"
          data-testid="wb-typing-sample"
        >
          {chars.map((c, i) => {
            const at = typedChars[i];
            const cls =
              at === undefined
                ? "text-(--wb-ink-3)"
                : at === c
                  ? "text-(--wb-ink)"
                  : "rounded-sm bg-(--wb-tag-beta-bg) text-(--wb-tag-beta-ink) underline";
            return (
              <span key={i} className={cls}>
                {c}
              </span>
            );
          })}
        </p>
      </div>

      <label className="sr-only" htmlFor="wb-typing-input">
        {t("onboardingFlow.typing.inputLabel")}
      </label>
      <textarea
        id="wb-typing-input"
        ref={input}
        value={typed}
        readOnly={run === "done"}
        onChange={(e) => onType(e.target.value)}
        onPaste={(e) => e.preventDefault()}
        onDrop={(e) => e.preventDefault()}
        spellCheck={false}
        autoCorrect="off"
        autoCapitalize="off"
        autoComplete="off"
        lang={language}
        dir="ltr"
        rows={3}
        placeholder={t("onboardingFlow.typing.placeholder")}
        data-testid="wb-typing-input"
        className="w-full resize-none rounded-(--wb-radius-control) select-text border border-(--wb-line-strong) bg-(--wb-card) px-3 py-2 font-mono text-[14px] leading-6 text-(--wb-ink) outline-none placeholder:text-(--wb-ink-3) focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-(--wb-accent) read-only:opacity-70"
      />

      <div
        role="status"
        className="min-h-6 text-center text-sm"
        data-testid="wb-typing-result"
      >
        {run === "done" &&
          (result == null ? (
            <span className="text-(--wb-ink-2)">
              {t("onboardingFlow.typing.nothing")}
            </span>
          ) : (
            <span className="inline-flex items-center gap-2 font-semibold">
              <Keyboard size={16} aria-hidden="true" />
              {t("onboardingFlow.typing.result", { count: result })}
            </span>
          ))}
      </div>

      <div className="flex flex-wrap items-center justify-center gap-3">
        <Button
          onClick={reset}
          disabled={run === "ready"}
          data-testid="wb-typing-again"
        >
          {t("onboardingFlow.typing.tryAgain")}
        </Button>
        <Button
          variant="primary"
          onClick={() => void recalculate()}
          disabled={result == null || busy}
          data-testid="wb-typing-recalculate"
        >
          {t("onboardingFlow.typing.recalculate")}
        </Button>
      </div>
    </div>
  );
}
