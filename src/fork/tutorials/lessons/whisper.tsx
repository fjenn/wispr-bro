// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The whisper lesson (S-32, B-42, B-43, P-1; R1 §11-§12, our own wording
 * and drawings): dictating quietly, then a microphone quiz.
 *
 * Try it out:
 * 1. a drawn scene: a soft voice still lands as clean text;
 * 2. how to whisper with the built-in microphone (about 30 cm away, breath
 *    in the voice, a steady pace, let go after the last word);
 * 3. practice: whisper a short email into a drawn composer. The take is a
 *    real dictation through S-29b's practice mode (`fork_onboarding_
 *    practice`): a take started with Wispr Bro in front comes back as
 *    `fork-practice-take` and is never pasted, never counted in the stats
 *    and never ticks "Try it in another app". Continue enables when at
 *    least 70 % of the sentence's words came through (`whisperMatch.ts`);
 *    below that the step shows tips (get closer, a firmer whisper, the input
 *    volume with a button to Sound settings) and the next take tries again.
 *    With no speech model ready yet the take waits in the queue (S-07c,
 *    it is in History) and the composer shows a written example that says
 *    it is one, as S-31 does with its rewrite. Skip is always offered;
 * 4-8. the microphone quiz and its table (`MicQuiz.tsx`), Done.
 *
 * Re-entry: the home banner's "Show me how" (C-29), Help › Essentials,
 * the setup checklist's "Try whispering" (ticked when the lesson closes
 * after the practice passed or showed the example).
 */
import { useEffect, useRef, useState } from "react";
import { Ear, Mail, Mic, Pause, Play } from "../../ui/icons";
import { useTranslation } from "react-i18next";
import { events as handyEvents } from "@/bindings";
import { commands, events } from "../../bindings";
import { FORK_NAMESPACE } from "../../i18n";
import { Button } from "../../ui";
import { WhisperDistanceArt } from "../art/MicArt";
import { registerLesson, type LessonStepContext } from "../lessonRegistry";
import {
  QuizComfortable,
  QuizMoves,
  QuizOwned,
  QuizResults,
  QuizWorkplace,
} from "../MicQuiz";
import { PracticeComposer } from "../PracticeComposer";
import { useBeatScript, type BeatScript } from "../scenes/beats";
import { wordMatch, WHISPER_PASS } from "./whisperMatch";

export const WHISPER_LESSON = "whisper";
export const TRY_WHISPER = "try_whisper";

/** The practice mode renews itself before the backend's two-minute expiry. */
const RENEW_MS = 60_000;
/** A take ended, no text this long after: "Nothing came through". */
export const NOTHING_MS = 20_000;

/* ---------- Step 1: a soft voice still works ---------- */

type SoftBeat = "quiet" | "words" | "landed";
const SOFT: BeatScript<SoftBeat> = {
  beats: [
    [0, "quiet"],
    [800, "words"],
    [2800, "landed"],
  ],
  ms: 6000,
};

function SoftScene() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [paused, setPaused] = useState(false);
  const { beat, still } = useBeatScript(SOFT, paused);
  return (
    <div className="wb-lesson-scene" data-testid="wb-lesson-scene">
      <div
        role="img"
        aria-label={t("lessons.whisper.soft.scene", {
          text: t("lessons.whisper.soft.text"),
        })}
        className="wb-lesson-stage wb-whisper-stage"
        data-beat={beat}
      >
        <p className="wb-whisper-caption" aria-hidden="true">
          {t("lessons.whisper.soft.spoken")}
        </p>
        <div className="wb-practice-window" aria-hidden="true">
          <div className="wb-practice-titlebar">
            <span className="wb-practice-dots">
              <i />
              <i />
              <i />
            </span>
            <Mail size={14} aria-hidden="true" />
            <span>{t("lessons.whisper.soft.app")}</span>
          </div>
          <div className="wb-practice-body">
            <div className="wb-scene-draft wb-whisper-landing">
              {t("lessons.whisper.soft.text")}
            </div>
          </div>
        </div>
        <div className="wb-lesson-bar wb-whisper-bar" aria-hidden="true">
          <span className="wb-whisper-levels">
            {[0, 1, 2, 3, 4].map((i) => (
              <i key={i} />
            ))}
          </span>
        </div>
      </div>
      {!still && (
        <button
          type="button"
          className="wb-lesson-pause self-start"
          onClick={() => setPaused((p) => !p)}
          aria-pressed={paused}
          data-testid="wb-lesson-scene-pause"
        >
          {paused ? (
            <Play size={14} aria-hidden="true" />
          ) : (
            <Pause size={14} aria-hidden="true" />
          )}
          {paused ? t("lessons.common.play") : t("lessons.common.pause")}
        </button>
      )}
    </div>
  );
}

/* ---------- Step 2: how to whisper with the built-in mic ---------- */

const HOW_TIPS = ["distance", "breath", "pace", "release"] as const;

function HowToWhisper() {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <div className="flex flex-col gap-4" data-testid="wb-whisper-how">
      <figure className="m-0 flex flex-col items-center gap-1 rounded-(--wb-radius-md) bg-(--wb-accent-soft) px-4 py-3 text-(--wb-ink)">
        <WhisperDistanceArt className="h-24 w-full max-w-[320px]" />
        <figcaption className="text-[13px] font-medium text-(--wb-ink-2)">
          {t("lessons.whisper.how.distance")}
        </figcaption>
      </figure>
      <ul className="m-0 flex list-none flex-col gap-2 p-0">
        {HOW_TIPS.map((tip, i) => (
          <li
            key={tip}
            className="flex gap-3 text-[14px] leading-5"
            data-testid={`wb-whisper-tip-${tip}`}
          >
            <span
              aria-hidden="true"
              className="grid size-6 shrink-0 place-items-center rounded-full bg-(--wb-fill-active) text-[12px] font-semibold"
            >
              {i + 1}
            </span>
            <span>
              <strong className="font-semibold">
                {t(`lessons.whisper.how.tips.${tip}.title`)}
              </strong>{" "}
              <span className="text-(--wb-ink-2)">
                {t(`lessons.whisper.how.tips.${tip}.body`)}
              </span>
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}

/* ---------- Step 3: whisper an email ---------- */

type Phase =
  | "idle"
  | "listening"
  | "working"
  | "nothing"
  | "low"
  | "passed"
  | "example";

function WhisperPractice({ ctx }: { ctx: LessonStepContext }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const sentence = t("lessons.whisper.practice.sentence");
  const [trigger, setTrigger] = useState<string | null>("fn");
  const [phase, setPhase] = useState<Phase>("idle");
  const [text, setText] = useState("");
  const [score, setScore] = useState<number | null>(null);
  const nothingTimer = useRef<number>();
  const complete = useRef(ctx.complete);
  complete.current = ctx.complete;
  const sentenceRef = useRef(sentence);
  sentenceRef.current = sentence;

  // Practice mode (S-29b's: never pasted, out of the stats) while mounted.
  useEffect(() => {
    let alive = true;
    const on = () =>
      void commands.forkOnboardingPractice(true).catch(() => undefined);
    on();
    const renew = window.setInterval(on, RENEW_MS);
    void commands
      .forkHotkeyStatus()
      .then((result) => {
        if (alive && result.status === "ok") setTrigger(result.data.trigger);
      })
      .catch(() => undefined);
    return () => {
      alive = false;
      window.clearInterval(renew);
      void commands.forkOnboardingPractice(false).catch(() => undefined);
    };
  }, []);

  useEffect(() => {
    const offPhase = handyEvents.recordingStateEvent.listen(({ payload }) => {
      const p = payload.phase;
      if (p === "started" || p === "locked") {
        window.clearTimeout(nothingTimer.current);
        setPhase("listening");
      } else if (p === "cancelled") {
        setPhase((now) => (now === "listening" ? "idle" : now));
      } else {
        setPhase((now) => (now === "listening" ? "working" : now));
        window.clearTimeout(nothingTimer.current);
        nothingTimer.current = window.setTimeout(() => {
          setPhase((now) => (now === "working" ? "nothing" : now));
        }, NOTHING_MS);
      }
    });
    const offTake = events.forkPracticeTake.listen(({ payload }) => {
      window.clearTimeout(nothingTimer.current);
      if (payload.kind === "queued") {
        // No speech model yet: the take waits (it is in History); show
        // what a whispered take looks like, and say it is an example.
        setText(sentenceRef.current);
        setScore(null);
        setPhase("example");
        complete.current();
        return;
      }
      const heard = payload.text?.trim() ?? "";
      if (!heard) {
        setPhase("nothing");
        return;
      }
      const share = wordMatch(sentenceRef.current, heard);
      setText(heard);
      setScore(share);
      if (share >= WHISPER_PASS) {
        setPhase("passed");
        complete.current();
      } else {
        setPhase("low");
      }
    });
    return () => {
      window.clearTimeout(nothingTimer.current);
      for (const off of [offPhase, offTake]) {
        void off.then((f) => f()).catch(() => undefined);
      }
    };
  }, []);

  const fallback = trigger === "ctrl+option";
  const key = t(
    fallback
      ? "lessons.whisper.practice.keyFallback"
      : "lessons.whisper.practice.key",
  );
  const percent = score == null ? 0 : Math.round(score * 100);
  const status: Record<Phase, string> = {
    idle: t("lessons.whisper.practice.status.idle", { key }),
    listening: t("lessons.whisper.practice.status.listening", { key }),
    working: t("lessons.whisper.practice.status.working"),
    nothing: t("lessons.whisper.practice.status.nothing"),
    low: t("lessons.whisper.practice.status.low", { percent }),
    passed: t("lessons.whisper.practice.status.passed", { percent }),
    example: t("lessons.whisper.practice.status.example"),
  };

  return (
    <div
      className="flex flex-col gap-3"
      data-testid="wb-whisper-practice"
      data-phase={phase}
    >
      {trigger ? (
        <p
          className="m-0 text-[14px] leading-5"
          data-testid="wb-whisper-instruction"
        >
          {t("lessons.whisper.practice.instruction", { key })}
        </p>
      ) : (
        <p
          className="m-0 text-[14px] leading-5"
          data-testid="wb-whisper-unbound"
        >
          {t("lessons.whisper.practice.unbound")}
        </p>
      )}
      <blockquote
        className="m-0 rounded-(--wb-radius-md) border-s-4 border-(--wb-ink) bg-(--wb-accent-soft) px-4 py-3 text-[15px] leading-6"
        data-testid="wb-whisper-sentence"
      >
        <span className="sr-only">
          {t("lessons.whisper.practice.sentenceLabel")}{" "}
        </span>
        {sentence}
      </blockquote>
      <PracticeComposer
        app={t("lessons.whisper.practice.app")}
        icon={Mail}
        context={t("lessons.whisper.practice.context")}
        label={t("lessons.whisper.practice.fieldLabel")}
        value={text}
        placeholder={t("lessons.whisper.practice.placeholder")}
      />
      <p
        role="status"
        className="m-0 flex items-center gap-2 text-[13px] text-(--wb-ink-2)"
        data-testid="wb-whisper-status"
      >
        <Mic
          size={16}
          aria-hidden="true"
          className={
            phase === "listening"
              ? "text-(--wb-accent) motion-safe:animate-pulse"
              : "text-(--wb-ink-3)"
          }
        />
        {status[phase]}
      </p>
      {phase === "example" && (
        <p
          className="m-0 text-[12.5px] leading-5 text-(--wb-ink-2)"
          data-testid="wb-whisper-example-note"
        >
          {t("lessons.whisper.practice.exampleNote")}
        </p>
      )}
      {(phase === "low" || phase === "nothing") && (
        <section
          aria-labelledby="wb-whisper-tips-title"
          className="flex flex-col gap-2 rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) p-3"
          data-testid="wb-whisper-tips"
        >
          <h4
            id="wb-whisper-tips-title"
            className="m-0 flex items-center gap-2 text-[14px] font-semibold"
          >
            <Ear size={16} aria-hidden="true" />
            {t("lessons.whisper.practice.tips.title")}
          </h4>
          <ul className="m-0 flex list-disc flex-col gap-1 ps-5 text-[13px] leading-5">
            <li>{t("lessons.whisper.practice.tips.closer")}</li>
            <li>{t("lessons.whisper.practice.tips.firmer")}</li>
            <li>{t("lessons.whisper.practice.tips.volume")}</li>
          </ul>
          <Button
            className="self-start px-2.5! py-1! text-[12.5px]!"
            onClick={() =>
              void commands
                .forkOnboardingOpenPane("sound_input")
                .catch(() => undefined)
            }
            data-testid="wb-whisper-open-sound"
          >
            {t("lessons.whisper.practice.tips.openSound")}
          </Button>
        </section>
      )}
    </div>
  );
}

/* ---------- The lesson ---------- */

let registered = false;

export function registerWhisperLesson(): void {
  if (registered) return;
  registered = true;
  registerLesson({
    id: WHISPER_LESSON,
    icon: Ear,
    titleKey: "lessons.whisper.title",
    steps: [
      {
        id: "soft",
        titleKey: "lessons.whisper.soft.title",
        bodyKey: "lessons.whisper.soft.body",
        render: () => <SoftScene />,
      },
      {
        id: "how",
        titleKey: "lessons.whisper.how.title",
        bodyKey: "lessons.whisper.how.body",
        render: () => <HowToWhisper />,
      },
      {
        id: "practice",
        gated: true,
        titleKey: "lessons.whisper.practice.title",
        bodyKey: "lessons.whisper.practice.body",
        doneTitleKey: "lessons.whisper.practice.doneTitle",
        doneBodyKey: "lessons.whisper.practice.doneBody",
        render: (ctx) => <WhisperPractice ctx={ctx} />,
      },
      {
        id: "quiz_comfortable",
        titleKey: "lessons.whisper.quiz.comfortable.title",
        bodyKey: "lessons.whisper.quiz.comfortable.body",
        render: () => <QuizComfortable />,
      },
      {
        id: "quiz_workplace",
        titleKey: "lessons.whisper.quiz.workplace.title",
        bodyKey: "lessons.whisper.quiz.workplace.body",
        render: () => <QuizWorkplace />,
      },
      {
        id: "quiz_moves",
        titleKey: "lessons.whisper.quiz.moves.title",
        bodyKey: "lessons.whisper.quiz.moves.body",
        render: () => <QuizMoves />,
      },
      {
        id: "quiz_owned",
        titleKey: "lessons.whisper.quiz.owned.title",
        bodyKey: "lessons.whisper.quiz.owned.body",
        continueKey: "lessons.whisper.quiz.owned.continue",
        render: () => <QuizOwned />,
      },
      {
        id: "results",
        titleKey: "lessons.whisper.results.title",
        bodyKey: "lessons.whisper.results.body",
        continueKey: "lessons.whisper.results.done",
        render: () => <QuizResults />,
      },
    ],
    help: { labelKey: "lessons.whisper.help", order: 20 },
    checklist: {
      id: TRY_WHISPER,
      order: 37,
      icon: Ear,
      titleKey: "checklist.items.try_whisper.title",
      actionKey: "checklist.items.try_whisper.action",
      bodyKey: "checklist.items.try_whisper.body",
    },
  });
}
