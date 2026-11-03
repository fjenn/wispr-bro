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
 * Answers live for one opening of the lesson (`useMicQuizStore`, reset on
 * each open); nothing is saved or sent anywhere. Copy results puts plain
 * text on the clipboard (Wispr's "Email results" is cloud, R2 B-43).
 */
import { useState } from "react";
import { Check, Copy, ExternalLink } from "lucide-react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import { create } from "zustand";
import { MICROPHONES_DOC_URL } from "../hub/links";
import { FORK_NAMESPACE } from "../i18n";
import { Button, FOCUS_RING } from "../ui";
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

const OPTION =
  "flex cursor-pointer items-center gap-3 rounded-(--wb-radius-card) border border-(--wb-line) bg-(--wb-card) p-3 text-start hover:bg-(--wb-hover) has-[:checked]:border-(--wb-accent) has-[:checked]:bg-(--wb-selected) has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-2 has-[:focus-visible]:outline-(--wb-accent)";

/** One choice in a group (native radio or checkbox: arrows, Space). */
function Option({
  type,
  name,
  checked,
  onChange,
  label,
  hint,
  art,
  testId,
}: {
  type: "radio" | "checkbox";
  name: string;
  checked: boolean;
  onChange: () => void;
  label: string;
  hint?: string;
  art?: React.ReactNode;
  testId: string;
}) {
  return (
    <label className={OPTION} data-testid={testId}>
      <input
        type={type}
        name={name}
        checked={checked}
        onChange={onChange}
        className="sr-only"
      />
      {art}
      <span className="flex min-w-0 flex-1 flex-col">
        <span className="text-[14px] font-medium">{label}</span>
        {hint && (
          <span className="text-[12.5px] leading-4 text-(--wb-ink-2)">
            {hint}
          </span>
        )}
      </span>
      <span
        aria-hidden="true"
        className={`grid size-5 shrink-0 place-items-center border border-(--wb-line-strong) ${type === "radio" ? "rounded-full" : "rounded-[5px]"} ${checked ? "border-(--wb-accent-fill) bg-(--wb-accent-fill) text-(--wb-on-accent)" : ""}`}
      >
        {checked && <Check size={12} strokeWidth={3} />}
      </span>
    </label>
  );
}

function Group({
  labelKey,
  children,
  columns = 2,
}: {
  labelKey: string;
  children: React.ReactNode;
  columns?: 1 | 2;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <fieldset className="m-0 border-0 p-0">
      <legend className="sr-only">{t(labelKey)}</legend>
      <div className={`grid gap-2 ${columns === 2 ? "sm:grid-cols-2" : ""}`}>
        {children}
      </div>
    </fieldset>
  );
}

const micArt = (kind: MicKind) => (
  <MicArt
    kind={kind}
    className="grid size-12 shrink-0 place-items-center text-(--wb-ink) [&>svg]:size-12"
  />
);

/** Question 1: a comfortable microphone, and more about mics. */
export function QuizComfortable() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const a = useAnswers();
  return (
    <div className="flex flex-col gap-3" data-testid="wb-quiz-comfortable">
      <Group labelKey="lessons.whisper.quiz.comfortable.title">
        {MIC_KINDS.map((kind) => (
          <Option
            key={kind}
            type="radio"
            name="wb-quiz-comfortable"
            checked={a.comfortable === kind}
            onChange={() => a.set({ comfortable: kind })}
            label={t(`lessons.whisper.mics.${kind}.name`)}
            hint={t(`lessons.whisper.mics.${kind}.hint`)}
            art={micArt(kind)}
            testId={`wb-quiz-comfortable-${kind}`}
          />
        ))}
      </Group>
      <button
        type="button"
        onClick={() => void openUrl(MICROPHONES_DOC_URL).catch(() => undefined)}
        className={`inline-flex cursor-pointer items-center gap-1.5 self-start rounded text-[13px] font-medium text-(--wb-ink) underline underline-offset-2 hover:text-(--wb-ink-2) ${FOCUS_RING}`}
        data-testid="wb-quiz-more-mics"
      >
        {t("lessons.whisper.quiz.more")}
        <ExternalLink size={13} aria-hidden="true" />
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
      <Group labelKey="lessons.whisper.quiz.workplace.title" columns={1}>
        {WORKPLACES.map((place) => (
          <Option
            key={place}
            type="radio"
            name="wb-quiz-workplace"
            checked={a.workplace === place}
            onChange={() => a.set({ workplace: place })}
            label={t(`lessons.whisper.quiz.workplace.${place}.label`)}
            hint={t(`lessons.whisper.quiz.workplace.${place}.hint`)}
            testId={`wb-quiz-workplace-${place}`}
          />
        ))}
      </Group>
    </div>
  );
}

/** Question 3: one desk or different places. */
export function QuizMoves() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const a = useAnswers();
  return (
    <div data-testid="wb-quiz-moves">
      <Group labelKey="lessons.whisper.quiz.moves.title" columns={1}>
        {MOVES.map((moves) => (
          <Option
            key={moves}
            type="radio"
            name="wb-quiz-moves"
            checked={a.moves === moves}
            onChange={() => a.set({ moves })}
            label={t(`lessons.whisper.quiz.moves.${moves}.label`)}
            hint={t(`lessons.whisper.quiz.moves.${moves}.hint`)}
            testId={`wb-quiz-moves-${moves}`}
          />
        ))}
      </Group>
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
      <Group labelKey="lessons.whisper.quiz.owned.title">
        {MIC_KINDS.map((kind) => (
          <Option
            key={kind}
            type="checkbox"
            name={`wb-quiz-owned-${kind}`}
            checked={a.owned.includes(kind)}
            onChange={() => toggle(kind)}
            label={t(`lessons.whisper.mics.${kind}.name`)}
            art={micArt(kind)}
            testId={`wb-quiz-owned-${kind}`}
          />
        ))}
      </Group>
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
    <span className="inline-flex items-center gap-1.5">
      <span aria-hidden="true" className="inline-flex gap-0.5">
        {[1, 2, 3].map((n) => (
          <i
            key={n}
            className={`block size-1.5 rounded-full ${n <= value ? "bg-(--wb-accent-fill)" : "bg-(--wb-line-strong)"}`}
          />
        ))}
      </span>
      {t(RATING_KEY[value])}
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

  const th =
    "px-2.5 py-2 text-start text-[12px] font-semibold text-(--wb-ink-2) align-bottom";
  const td = "px-2.5 py-2 text-[13px] align-middle";
  return (
    <div
      className="flex flex-col gap-3"
      data-testid="wb-quiz-results"
      data-best={best}
    >
      <p className="m-0 flex items-center gap-3 rounded-(--wb-radius-card) bg-(--wb-banner) p-3 text-[14px]">
        {micArt(best)}
        <span>
          {t("lessons.whisper.results.bestLine", {
            mic: t(`lessons.whisper.mics.${best}.name`),
          })}{" "}
          <span className="text-(--wb-ink-2)">
            {t(`lessons.whisper.mics.${best}.why`)}
          </span>
        </span>
      </p>
      <div className="overflow-x-auto rounded-(--wb-radius-card) border border-(--wb-line) bg-(--wb-card)">
        <table className="w-full border-collapse" data-testid="wb-quiz-table">
          <caption className="sr-only">
            {t("lessons.whisper.results.caption")}
          </caption>
          <thead className="border-b border-(--wb-line)">
            <tr>
              <th scope="col" className={th}>
                {t("lessons.whisper.results.columns.mic")}
              </th>
              <th scope="col" className={th}>
                {t("lessons.whisper.results.columns.quality")}
              </th>
              <th scope="col" className={th}>
                {t("lessons.whisper.results.columns.others")}
              </th>
              <th scope="col" className={th}>
                {t("lessons.whisper.results.columns.home")}
              </th>
              <th scope="col" className={th}>
                {t("lessons.whisper.results.columns.price")}
              </th>
              <th scope="col" className={th}>
                {t("lessons.whisper.results.columns.best")}
              </th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr
                key={row.kind}
                data-testid={`wb-quiz-row-${row.kind}`}
                data-best={row.best || undefined}
                className={`border-b border-(--wb-line) last:border-b-0 ${row.best ? "bg-(--wb-selected)" : ""}`}
              >
                <th scope="row" className={`${td} text-start font-medium`}>
                  {t(`lessons.whisper.mics.${row.kind}.name`)}
                </th>
                <td className={td}>
                  <RatingCell value={row.quality} />
                </td>
                <td className={td}>
                  <RatingCell value={row.aroundOthers} />
                </td>
                <td className={td}>
                  <RatingCell value={row.atHome} />
                </td>
                <td className={td}>{priceLabel(row, t)}</td>
                <td className={td}>
                  {row.best ? (
                    <span className="inline-flex items-center gap-1 rounded-full bg-(--wb-accent-fill) px-2 py-0.5 text-[12px] font-semibold text-(--wb-on-accent)">
                      <Check size={12} strokeWidth={3} aria-hidden="true" />
                      {t("lessons.whisper.results.columns.best")}
                    </span>
                  ) : (
                    <span className="sr-only">
                      {t("lessons.whisper.results.notBest")}
                    </span>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className="flex flex-wrap items-center gap-3">
        <Button onClick={() => void copy()} data-testid="wb-quiz-copy">
          <Copy size={14} aria-hidden="true" />
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
