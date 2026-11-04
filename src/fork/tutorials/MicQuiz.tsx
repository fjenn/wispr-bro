// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The microphone quiz (S-32, B-43, P-1), the second half of the whisper
 * lesson: four questions, then a recommendation table with a "Best match"
 * column (rules: `micTable.ts`). Our own questions, wording and drawings.
 *
 * 1. "Pick a microphone you'd be comfortable using": four drawn kinds and a
 *    "More about microphones" link to `docs/fork/user/microphones.md`;
 * 2. "Where do you usually work?" (private / a bit of both / around others);
 * 3. "Do you move between places?" (one desk / different places);
 * 4. "Which microphones do you have?" (any; the built-in one starts ticked);
 * → the table (mic quality, whispering around others, whispering at home,
 *   price, Best match) with Copy results; Done closes the lesson.
 *
 * Every question can be left unanswered (Continue is always on), so the
 * quiz never dead-ends; an unanswered question weighs nothing either way.
 * Questions are option cards (Design W5): a stacked list of title + one
 * line, or a row of drawn microphone tiles; the table has a column per
 * microphone with its drawing on top and the Best match column marked by a
 * green band.
 *
 * Answers live for one opening of the lesson (`useMicQuizStore`, reset on
 * each open); nothing is saved or sent anywhere. Copy results puts plain
 * text on the clipboard (Wispr's "Email results" is cloud, R2 B-43).
 */
import { useState } from "react";
import { Check, Copy, ExternalLink, Star } from "../ui/icons";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import { create } from "zustand";
import { MICROPHONES_DOC_URL } from "../hub/links";
import { FORK_NAMESPACE } from "../i18n";
import { Button, CheckCard, FOCUS_RING, OptionCard, RadioGroup } from "../ui";
import { MicArt } from "./art/MicArt";
import { useLessonStore } from "./lessonRegistry";
import {
  DEFAULT_ANSWERS,
  MIC_KINDS,
  MOVES,
  WORKPLACES,
  recommend,
  type MicKind,
  type MicRow,
  type QuizAnswers,
  type Rating,
} from "./micTable";

interface QuizStore extends QuizAnswers {
  /** The lesson opening these answers belong to. */
  seq: number;
}

export const useMicQuizStore = create<QuizStore>()(() => ({
  ...DEFAULT_ANSWERS,
  seq: -1,
}));

/** The answers of the lesson opened now (a new opening starts afresh). */
function useAnswers(): QuizAnswers & {
  set: (a: Partial<QuizAnswers>) => void;
} {
  const seq = useLessonStore((s) => s.open?.seq ?? 0);
  const store = useMicQuizStore();
  const fresh = store.seq !== seq;
  const answers: QuizAnswers = fresh ? DEFAULT_ANSWERS : store;
  return {
    comfortable: answers.comfortable,
    workplace: answers.workplace,
    moves: answers.moves,
    owned: answers.owned,
    set: (over) =>
      useMicQuizStore.setState((now) => ({
        ...(now.seq === seq ? now : DEFAULT_ANSWERS),
        ...over,
        seq,
      })),
  };
}

const micArt = (kind: MicKind, size: "md" | "lg" = "lg") => (
  <MicArt
    kind={kind}
    className={`grid shrink-0 place-items-center text-(--wb-ink) ${size === "lg" ? "size-16 [&>svg]:size-16" : "size-9 [&>svg]:size-9"}`}
  />
);

/** A one-of-many question: option cards in a named group. */
function Choices<T extends string>({
  labelKey,
  name,
  value,
  onChange,
  options,
  layout = "list",
}: {
  labelKey: string;
  name: string;
  value: T | null;
  onChange: (value: T) => void;
  options: readonly {
    value: T;
    title: string;
    hint?: string;
    art?: React.ReactNode;
  }[];
  layout?: "list" | "grid";
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <RadioGroup
      label={t(labelKey)}
      name={name}
      value={value ?? undefined}
      onValueChange={(v) => onChange(v as T)}
      layout={layout}
      columns={4}
      className={
        layout === "list"
          ? "mx-auto w-full max-w-[360px] gap-3!"
          : "mx-auto w-full max-w-[680px] gap-3! max-sm:grid-cols-2!"
      }
    >
      {options.map((o) => (
        // The wrapper carries the test id (the input is inside).
        <div key={o.value} data-testid={`${name}-${o.value}`}>
          <OptionCard
            value={o.value}
            title={o.title}
            description={o.hint}
            media={o.art}
            className={`h-full ${o.art ? "p-4! text-center" : "px-5! py-4!"}`}
          />
        </div>
      ))}
    </RadioGroup>
  );
}

/** Question 1: a comfortable microphone, and more about mics. */
export function QuizComfortable() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const a = useAnswers();
  return (
    <div
      className="flex flex-col items-center gap-5"
      data-testid="wb-quiz-comfortable"
    >
      <Choices
        labelKey="lessons.whisper.quiz.comfortable.title"
        name="wb-quiz-comfortable"
        value={a.comfortable}
        onChange={(comfortable) => a.set({ comfortable })}
        layout="grid"
        options={MIC_KINDS.map((kind) => ({
          value: kind,
          title: t(`lessons.whisper.mics.${kind}.name`),
          art: micArt(kind),
        }))}
      />
      <button
        type="button"
        onClick={() => void openUrl(MICROPHONES_DOC_URL).catch(() => undefined)}
        className={`type-small inline-flex cursor-pointer items-center gap-1.5 rounded font-medium text-(--wb-ink-2) underline decoration-(--wb-line-2) underline-offset-4 hover:text-(--wb-ink) ${FOCUS_RING}`}
        data-testid="wb-quiz-more-mics"
      >
        {t("lessons.whisper.quiz.more")}
        <ExternalLink size={14} aria-hidden="true" />
      </button>
    </div>
  );
}

/** Question 2: where the user works. */
export function QuizWorkplace() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const a = useAnswers();
  return (
    <div data-testid="wb-quiz-workplace">
      <Choices
        labelKey="lessons.whisper.quiz.workplace.title"
        name="wb-quiz-workplace"
        value={a.workplace}
        onChange={(workplace) => a.set({ workplace })}
        options={WORKPLACES.map((place) => ({
          value: place,
          title: t(`lessons.whisper.quiz.workplace.${place}.label`),
          hint: t(`lessons.whisper.quiz.workplace.${place}.hint`),
        }))}
      />
    </div>
  );
}

/** Question 3: one desk or different places. */
export function QuizMoves() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const a = useAnswers();
  return (
    <div data-testid="wb-quiz-moves">
      <Choices
        labelKey="lessons.whisper.quiz.moves.title"
        name="wb-quiz-moves"
        value={a.moves}
        onChange={(moves) => a.set({ moves })}
        options={MOVES.map((moves) => ({
          value: moves,
          title: t(`lessons.whisper.quiz.moves.${moves}.label`),
          hint: t(`lessons.whisper.quiz.moves.${moves}.hint`),
        }))}
      />
    </div>
  );
}

/** Question 4: the microphones the user has (any). */
export function QuizOwned() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const a = useAnswers();
  const toggle = (kind: MicKind) =>
    a.set({
      owned: a.owned.includes(kind)
        ? a.owned.filter((k) => k !== kind)
        : MIC_KINDS.filter((k) => k === kind || a.owned.includes(k)),
    });
  return (
    <div data-testid="wb-quiz-owned">
      <fieldset className="m-0 border-0 p-0">
        <legend className="sr-only">
          {t("lessons.whisper.quiz.owned.title")}
        </legend>
        <div className="mx-auto grid w-full max-w-[680px] grid-cols-4 gap-3 max-sm:grid-cols-2">
          {MIC_KINDS.map((kind) => (
            <div key={kind} data-testid={`wb-quiz-owned-${kind}`}>
              <CheckCard
                checked={a.owned.includes(kind)}
                onCheckedChange={() => toggle(kind)}
                title={t(`lessons.whisper.mics.${kind}.name`)}
                media={micArt(kind)}
                className="h-full p-4! text-center"
              />
            </div>
          ))}
        </div>
      </fieldset>
    </div>
  );
}

const RATING_KEY: Record<Rating, string> = {
  1: "lessons.whisper.results.rating.fair",
  2: "lessons.whisper.results.rating.good",
  3: "lessons.whisper.results.rating.great",
};

function RatingCell({ value }: { value: Rating }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <span className="inline-flex flex-col items-center gap-0.5">
      <span aria-hidden="true" className="inline-flex gap-0.5">
        {[1, 2, 3].map((n) => (
          <Star
            key={n}
            size={14}
            className={
              n <= value
                ? "fill-current text-(--wb-ink)"
                : "text-(--wb-line-strong)"
            }
          />
        ))}
      </span>
      <span className="text-[12px] leading-4 text-(--wb-ink-2)">
        {t(RATING_KEY[value])}
      </span>
    </span>
  );
}

function priceLabel(row: MicRow, t: (k: string) => string): string {
  if (row.owned) return t("lessons.whisper.results.price.owned");
  return t(`lessons.whisper.results.price.${row.price}`);
}

/** Plain text for Copy results (the user's own answers and the table). */
export function resultsText(
  answers: QuizAnswers,
  t: (key: string, values?: Record<string, string>) => string,
): string {
  const { rows, best } = recommend(answers);
  const mic = (kind: MicKind) => t(`lessons.whisper.mics.${kind}.name`);
  const none = t("lessons.whisper.results.copy.unanswered");
  const lines = [
    t("lessons.whisper.results.copy.heading", { appName: t("rust.appName") }),
    "",
    t("lessons.whisper.results.copy.best", { mic: mic(best) }),
    "",
    `${t("lessons.whisper.quiz.comfortable.title")}: ${answers.comfortable ? mic(answers.comfortable) : none}`,
    `${t("lessons.whisper.quiz.workplace.title")}: ${answers.workplace ? t(`lessons.whisper.quiz.workplace.${answers.workplace}.label`) : none}`,
    `${t("lessons.whisper.quiz.moves.title")}: ${answers.moves ? t(`lessons.whisper.quiz.moves.${answers.moves}.label`) : none}`,
    `${t("lessons.whisper.quiz.owned.title")}: ${answers.owned.length ? answers.owned.map(mic).join(", ") : t("lessons.whisper.results.copy.noneOwned")}`,
    "",
  ];
  for (const row of rows) {
    lines.push(
      `${mic(row.kind)}${row.best ? ` (${t("lessons.whisper.results.columns.best")})` : ""}: ` +
        [
          `${t("lessons.whisper.results.columns.quality")} ${t(RATING_KEY[row.quality])}`,
          `${t("lessons.whisper.results.columns.others")} ${t(RATING_KEY[row.aroundOthers])}`,
          `${t("lessons.whisper.results.columns.home")} ${t(RATING_KEY[row.atHome])}`,
          `${t("lessons.whisper.results.columns.price")} ${priceLabel(row, t)}`,
        ].join(", "),
    );
  }
  return lines.join("\n");
}

/** The recommendation table with the Best match column and Copy results. */
export function QuizResults() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const a = useAnswers();
  const { rows, best } = recommend(a);
  const [copied, setCopied] = useState<"idle" | "ok" | "failed">("idle");

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(resultsText(a, t));
      setCopied("ok");
    } catch {
      setCopied("failed");
    }
  };

  const CRITERIA = ["quality", "others", "home", "price"] as const;
  const cell = (row: MicRow, criterion: (typeof CRITERIA)[number]) => {
    switch (criterion) {
      case "quality":
        return <RatingCell value={row.quality} />;
      case "others":
        return <RatingCell value={row.aroundOthers} />;
      case "home":
        return <RatingCell value={row.atHome} />;
      case "price":
        return (
          <span className="text-[13px] leading-4">{priceLabel(row, t)}</span>
        );
    }
  };
  return (
    <div
      className="flex flex-col items-center gap-4"
      data-testid="wb-quiz-results"
      data-best={best}
    >
      <p className="type-body m-0 max-w-[560px] text-center text-(--wb-ink-2)">
        <span className="font-semibold text-(--wb-ink)">
          {t("lessons.whisper.results.bestLine", {
            mic: t(`lessons.whisper.mics.${best}.name`),
          })}
        </span>{" "}
        {t(`lessons.whisper.mics.${best}.why`)}
      </p>
      <div className="w-full overflow-x-auto pt-6">
        <table className="wb-mic-table" data-testid="wb-quiz-table">
          <caption className="sr-only">
            {t("lessons.whisper.results.caption")}
          </caption>
          <thead>
            <tr>
              <td className="wb-mic-corner" />
              {rows.map((row) => (
                <th
                  key={row.kind}
                  scope="col"
                  data-testid={`wb-quiz-row-${row.kind}`}
                  data-best={row.best || undefined}
                >
                  {row.best && (
                    <span className="wb-mic-band" data-testid="wb-quiz-best">
                      {t("lessons.whisper.results.columns.best")}
                    </span>
                  )}
                  <span className="flex flex-col items-center gap-2">
                    {micArt(row.kind, "md")}
                    <span>{t(`lessons.whisper.mics.${row.kind}.name`)}</span>
                  </span>
                  {!row.best && (
                    <span className="sr-only">
                      {t("lessons.whisper.results.notBest")}
                    </span>
                  )}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {CRITERIA.map((criterion) => (
              <tr key={criterion}>
                <th scope="row">
                  {t(`lessons.whisper.results.columns.${criterion}`)}
                </th>
                {rows.map((row) => (
                  <td
                    key={row.kind}
                    data-best={row.best || undefined}
                    data-testid={`wb-quiz-cell-${row.kind}-${criterion}`}
                  >
                    {cell(row, criterion)}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className="flex flex-wrap items-center justify-center gap-3">
        <Button
          onClick={() => void copy()}
          leading={<Copy size={14} aria-hidden="true" />}
          data-testid="wb-quiz-copy"
        >
          {copied === "ok"
            ? t("lessons.whisper.results.copied")
            : t("lessons.whisper.results.copy.label")}
        </Button>
        <span
          aria-live="polite"
          className="text-[12.5px] text-(--wb-ink-2)"
          data-testid="wb-quiz-copy-status"
        >
          {copied === "failed" ? t("lessons.whisper.results.copyFailed") : ""}
        </span>
      </div>
    </div>
  );
}
