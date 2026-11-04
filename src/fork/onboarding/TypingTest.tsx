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
import { RotateCcw, X } from "../ui/icons";
import { useTranslation } from "react-i18next";
import { useSettings } from "@/hooks/useSettings";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettings } from "../settings/useForkSettings";
import {
  Button,
  IconButton,
  Select,
  StatNumber,
  ThemeScope,
  useWindowTheme,
} from "../ui";
import { CentredLayout, Title } from "./Layout";
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
  const windowTheme = useWindowTheme();
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
    <CentredLayout tone="dark" width={640} top={56}>
      <ThemeScope
        theme={windowTheme}
        className="relative w-full rounded-(--wb-radius-lg) bg-(--wb-surface) px-9 pt-7 pb-7 text-start shadow-(--wb-elev-3)"
      >
        <span className="absolute end-4 top-4">
          <IconButton
            aria-label={t("onboardingFlow.typing.skip")}
            onClick={onSkip}
            data-testid="wb-typing-skip"
          >
            <X size={18} />
          </IconButton>
        </span>
        <h1 className="type-sheet m-0 pe-10 text-[32px] leading-[38px] text-balance">
          <Title i18nKey="onboardingFlow.typing.title" />
        </h1>
        <p className="mt-2 mb-0 text-[13px] leading-[18px] text-pretty text-(--wb-ink-2)">
          {t("onboardingFlow.typing.instruction", {
            count: TEST_SECONDS,
            seconds: TEST_SECONDS,
          })}
        </p>
        <div className="mt-4 flex items-center gap-3">
          <Select
            value={language}
            onValueChange={(next) => {
              setLanguage(next);
              reset();
            }}
            disabled={run === "running"}
            label={t("onboardingFlow.typing.language")}
            size="sm"
            testId="wb-typing-language"
            className="min-w-[180px]"
            options={TYPING_SAMPLES.map((s) => ({
              value: s.language,
              label: languageName(s.language, i18n.language),
            }))}
          />
        </div>

        <div
          className={`relative mt-4 cursor-text rounded-(--wb-radius-sm) outline-offset-[6px] has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-(--wb-accent) ${
            run === "done" ? "opacity-70" : ""
          }`}
        >
          <p className="sr-only" lang={language}>
            {sample}
          </p>
          <p
            aria-hidden="true"
            lang={language}
            dir="ltr"
            className="type-mono m-0 text-[15px] leading-[26px] break-words whitespace-pre-wrap"
            data-testid="wb-typing-sample"
          >
            {chars.map((c, i) => {
              const at = typedChars[i];
              const cls =
                at === undefined
                  ? "text-(--wb-ink-3)"
                  : at === c
                    ? "text-(--wb-ink)"
                    : "rounded-[2px] bg-(--wb-danger-soft) text-(--wb-danger) underline";
              return (
                <span key={i} className={cls}>
                  {c}
                </span>
              );
            })}
          </p>
          <label className="sr-only" htmlFor="wb-typing-input">
            {t("onboardingFlow.typing.inputLabel")}
          </label>
          {/* The field lies over the paragraph with see-through text, so
              the caret moves through the paragraph as you type. */}
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
            data-testid="wb-typing-input"
            className="type-mono absolute inset-0 m-0 size-full resize-none overflow-hidden border-0 bg-transparent p-0 text-[15px] leading-[26px] break-words whitespace-pre-wrap text-transparent caret-(--wb-ink) outline-none select-text"
          />
        </div>

        <div
          role="status"
          className="mt-4 min-h-5 text-[15px]"
          data-testid="wb-typing-result"
        >
          {run === "done" &&
            (result == null ? (
              <span className="text-(--wb-ink-2)">
                {t("onboardingFlow.typing.nothing")}
              </span>
            ) : (
              <span className="font-medium">
                {t("onboardingFlow.typing.result", { count: result })}
              </span>
            ))}
        </div>

        <div className="mt-5 flex flex-wrap items-end gap-x-8 gap-y-4">
          <div
            className="flex flex-col"
            data-testid="wb-typing-wpm"
            data-wpm={live ?? ""}
          >
            <span className="type-display text-[44px] leading-[48px] tabular-nums">
              {live == null ? "—" : <StatNumber value={live} />}
            </span>
            <span className="text-[13px] text-(--wb-ink-2)">
              {t("onboardingFlow.typing.wpmUnit", { count: live ?? 0 })}
            </span>
          </div>
          <div
            className="flex flex-col"
            data-testid="wb-typing-seconds"
            data-seconds={secondsLeft}
          >
            <span className="type-display text-[44px] leading-[48px] tabular-nums">
              {secondsLeft}
            </span>
            <span className="text-[13px] text-(--wb-ink-2)">
              {t("onboardingFlow.typing.secondsUnit", { count: secondsLeft })}
            </span>
          </div>
          <div className="ms-auto flex flex-wrap items-center gap-2 pb-1">
            <Button
              variant="primary"
              onClick={() => void recalculate()}
              disabled={result == null || busy}
              data-testid="wb-typing-recalculate"
            >
              {t("onboardingFlow.typing.recalculate")}
            </Button>
            <Button
              variant="ghost"
              onClick={reset}
              disabled={run === "ready"}
              data-testid="wb-typing-again"
              leading={<RotateCcw size={16} aria-hidden="true" />}
            >
              {t("onboardingFlow.typing.tryAgain")}
            </Button>
          </div>
        </div>
      </ThemeScope>
    </CentredLayout>
  );
}
