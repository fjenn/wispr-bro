// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Transforms lesson (S-31, D-43, D-206; R1 §10, our own wording and
 * drawings).
 *
 * - **Try it out:** (1) three example cards (Original / Polish / Prompt
 *   Engineer) and a benefits ticker; (2) practice in a drawn chat window:
 *   "Select the text, then press ⌥1" (the user's own Polish shortcut). The
 *   rewrite is a real one through the transform preview (S-22) when the AI
 *   engine is ready, else a written example that says so; it replaces the
 *   selection as one undoable edit. On success: "Text rewritten. Press ⌥O
 *   to view the changes; ⌘Z undoes it." with the drawn bar spotlighted on
 *   "Done · View changes", which (or the View changes shortcut) shows the
 *   word diff. Last: "Turn on Transforms" (Done when already on).
 * - **How it works:** three animated scenes (select, press, see the
 *   changes), then "Try it yourself".
 * - **Re-entry:** Help › Essentials, the Transforms page banner (Try it /
 *   How it works) and its "Start tutorial", the setup checklist's "Try a
 *   Transform" (ticked by this practice or any real transform run).
 */
import { useEffect, useMemo, useRef, useState } from "react";
import { motion } from "motion/react";
import { Check, Mail, MessageCircle, WandSparkles } from "../../ui/icons";
import { useTranslation } from "react-i18next";
import { commands, type DiffOp, type TransformEntry } from "../../bindings";
import { navigate } from "../../hub/nav";
import { FORK_NAMESPACE } from "../../i18n";
import { chordLabel } from "../../settings/hotkeys/model";
import { useForkSettingsStore } from "../../settings/store";
import { DiffWords } from "../../transforms/DiffCard";
import { Button } from "../../ui";
import { useMotionPresets } from "../../ui/motion";
import { PauseButton } from "../PauseButton";
import { Keys } from "../RichText";
import { registerLesson, type LessonStepContext } from "../lessonRegistry";
import {
  PracticeComposer,
  replaceSelection,
  selectionOf,
} from "../PracticeComposer";
import { useLessonShortcuts } from "../SuccessDetector";
import { useBeatScript, type BeatScript } from "../scenes/beats";
import { changeCount, wordDiff } from "../scenes/wordDiff";

export const TRANSFORMS_LESSON = "transforms";
export const TRY_TRANSFORM = "try_transform";

const POLISH = "transform.polish";
const PROMPT = "transform.prompt_engineer";
const VIEW = "transform.view_changes";
/** S-21/S-22 defaults, for a profile whose hotkeys are not loaded yet. */
const DEFAULT_CHORDS: Record<string, string> = {
  [POLISH]: "option+1",
  [PROMPT]: "option+2",
  [VIEW]: "option+o",
};
/** A real rewrite that takes longer than this shows the example instead. */
export const PRACTICE_TIMEOUT_MS = 12_000;
const UNDO = "⌘ Z";

function chordsFor(action: string): string[] {
  const bound = useForkSettingsStore
    .getState()
    .settings?.hotkeys[action]?.map((b) => b.chord)
    .filter(Boolean);
  if (bound && bound.length > 0) return bound;
  return DEFAULT_CHORDS[action] ? [DEFAULT_CHORDS[action]] : [];
}

function shortcut(action: string): string {
  const chord = chordsFor(action)[0];
  return chord ? chordLabel(chord) : "";
}

function shortcutValues(): Record<string, string> {
  return {
    shortcut: shortcut(POLISH),
    prompt: shortcut(PROMPT),
    view: shortcut(VIEW),
    undo: UNDO,
  };
}

/* ---------- Try it out, step 1: examples and the benefits ticker ---------- */

const BENEFITS = [
  "shorter",
  "clearer",
  "order",
  "structure",
  "tone",
  "anywhere",
] as const;

function Examples() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const m = useMotionPresets();
  const [paused, setPaused] = useState(false);
  const benefits = BENEFITS.map((b) => t(`lessons.transforms.benefits.${b}`));
  const ticks = () =>
    benefits.map((b) => (
      <li key={b}>
        <Check size={14} aria-hidden="true" />
        {b}
      </li>
    ));
  return (
    <div className="flex flex-col gap-8" data-testid="wb-lesson-examples">
      <motion.ul
        className="wb-lesson-cards"
        aria-label={t("lessons.transforms.cards.label")}
        variants={m.stagger.container}
        initial="hidden"
        animate="shown"
      >
        {(["original", "polish", "prompt"] as const).map((kind) => (
          <motion.li
            key={kind}
            variants={m.stagger.item}
            className="wb-lesson-card"
            data-kind={kind}
            data-testid={`wb-lesson-card-${kind}`}
          >
            <h3>
              {t(`lessons.transforms.cards.${kind}.name`)}
              {kind !== "original" && (
                <Keys>{shortcut(kind === "polish" ? POLISH : PROMPT)}</Keys>
              )}
            </h3>
            <p>{t(`lessons.transforms.cards.${kind}.text`)}</p>
          </motion.li>
        ))}
      </motion.ul>
      <div
        className="wb-ticker"
        data-paused={paused || undefined}
        data-testid="wb-lesson-ticker"
      >
        <div className="wb-ticker-view">
          <div className="wb-ticker-track">
            <ul aria-label={t("lessons.transforms.benefits.label")}>
              {ticks()}
            </ul>
            {/* The second copy makes the loop seamless; read once. */}
            <ul aria-hidden="true">{ticks()}</ul>
          </div>
        </div>
        <PauseButton
          paused={paused}
          onToggle={() => setPaused((p) => !p)}
          pauseLabel={t("lessons.common.pause")}
          playLabel={t("lessons.common.play")}
          testId="wb-lesson-ticker-pause"
          className="motion-reduce:hidden"
        />
      </div>
    </div>
  );
}

/* ---------- How it works: three drawn scenes ---------- */

type Chapter = "select" | "press" | "changes";
type Beat =
  | "typed"
  | "selecting"
  | "selected"
  | "keys"
  | "working"
  | "replaced"
  | "diff";

const SCRIPTS: Record<Chapter, BeatScript<Beat>> = {
  select: {
    beats: [
      [0, "typed"],
      [900, "selecting"],
      [1800, "selected"],
    ],
    ms: 4200,
  },
  press: {
    beats: [
      [0, "selected"],
      [700, "keys"],
      [1500, "working"],
      [2600, "replaced"],
    ],
    ms: 5600,
  },
  changes: {
    beats: [
      [0, "replaced"],
      [900, "diff"],
    ],
    ms: 5200,
  },
};

function Scene({ chapter }: { chapter: Chapter }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [paused, setPaused] = useState(false);
  const { beat, still } = useBeatScript(SCRIPTS[chapter], paused);
  const draft = t("lessons.transforms.scene.draft");
  const polished = t("lessons.transforms.scene.polished");
  const ops = useMemo(() => wordDiff(draft, polished), [draft, polished]);
  const replaced = beat === "replaced" || beat === "diff";
  const values = shortcutValues();
  const keys =
    chapter === "changes" ? values.view.split(" ") : values.shortcut.split(" ");
  const bar = beat === "working" ? "working" : replaced ? "done" : "idle";

  return (
    <div className="wb-lesson-scene" data-testid="wb-lesson-scene">
      <div
        role="img"
        aria-label={t(`lessons.transforms.how.${chapter}.scene`, values)}
        className="wb-lesson-stage"
        data-beat={beat}
        data-chapter={chapter}
      >
        <div className="wb-scene-window" aria-hidden="true">
          <div className="wb-scene-titlebar">
            <span className="wb-scene-dots">
              <i />
              <i />
              <i />
            </span>
            <Mail size={14} />
            <span>{t("lessons.transforms.scene.app")}</span>
          </div>
          <div className="wb-scene-draft" data-testid="wb-lesson-scene-text">
            {replaced ? polished : <mark>{draft}</mark>}
          </div>
        </div>
        <div className="wb-scene-keys" aria-hidden="true">
          {keys.map((k) => (
            <span key={k}>{k}</span>
          ))}
        </div>
        <div className="wb-scene-diff" aria-hidden="true">
          <h4>{t("transforms.diff.changes", { count: changeCount(ops) })}</h4>
          <DiffWords ops={ops} />
        </div>
        <DrawnBar state={bar} />
      </div>
      {!still && (
        <PauseButton
          paused={paused}
          onToggle={() => setPaused((p) => !p)}
          pauseLabel={t("lessons.common.pause")}
          playLabel={t("lessons.common.play")}
          testId="wb-lesson-scene-pause"
          tone="hud"
          className="wb-scene-control"
        />
      )}
    </div>
  );
}

/* ---------- The drawn bar ---------- */

function DrawnBar({
  state,
  spot = false,
  onView,
  viewRef,
}: {
  state: "idle" | "working" | "done";
  spot?: boolean;
  onView?: () => void;
  viewRef?: React.Ref<HTMLButtonElement>;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <div
      className="wb-lesson-bar"
      data-state={state}
      data-spot={spot || undefined}
      data-testid="wb-lesson-bar"
      role={onView ? "group" : undefined}
      aria-label={
        onView ? t("lessons.transforms.practice.barLabel") : undefined
      }
      aria-hidden={onView ? undefined : true}
    >
      {state === "working" && (
        <>
          <span className="wb-lesson-spin" aria-hidden="true" />
          <span>{t("lessons.transforms.practice.working")}</span>
        </>
      )}
      {state === "done" && (
        <>
          <Check size={14} aria-hidden="true" />
          <span>{t("lessons.transforms.practice.bar.done")}</span>
          {onView ? (
            <button
              ref={viewRef}
              type="button"
              onClick={onView}
              data-testid="wb-lesson-view-changes"
            >
              {t("lessons.transforms.practice.bar.view")}
            </button>
          ) : (
            <span className="wb-lesson-bar-action">
              {t("lessons.transforms.practice.bar.view")}
            </span>
          )}
        </>
      )}
    </div>
  );
}

/* ---------- Try it out, step 2: practice ---------- */

type Phase = "idle" | "select_first" | "working" | "done";

interface Rewrite {
  source: string;
  result: string;
  example: boolean;
}

/** The example rewrite for an action (no engine, an error or a timeout). */
function exampleFor(action: string, t: (key: string) => string): string {
  return t(
    action === PROMPT
      ? "lessons.transforms.practice.example.prompt"
      : "lessons.transforms.practice.example.polish",
  );
}

async function realRewrite(
  entry: TransformEntry | undefined,
  source: string,
): Promise<string | null> {
  if (!entry) return null;
  const timeout = new Promise<null>((resolve) =>
    window.setTimeout(() => resolve(null), PRACTICE_TIMEOUT_MS),
  );
  const run = commands
    .forkTransformPreview(
      entry.id,
      {
        name: entry.name,
        description: entry.description,
        rules: entry.rules,
        prompt: entry.prompt,
        examples: entry.examples,
      },
      source,
    )
    .then((r) => (r.status === "ok" && r.data.trim() ? r.data : null))
    .catch(() => null);
  return Promise.race([run, timeout]);
}

function Practice({ ctx }: { ctx: LessonStepContext }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const hotkeys = useForkSettingsStore((s) => s.settings?.hotkeys);
  const field = useRef<HTMLTextAreaElement>(null);
  const viewRef = useRef<HTMLButtonElement>(null);
  const [entries, setEntries] = useState<TransformEntry[]>([]);
  const [phase, setPhase] = useState<Phase>("idle");
  const [rewrite, setRewrite] = useState<Rewrite | null>(null);
  const [spot, setSpot] = useState(false);
  const [diff, setDiff] = useState<DiffOp[] | null>(null);
  const [changes, setChanges] = useState(0);
  const phaseRef = useRef(phase);
  phaseRef.current = phase;

  useEffect(() => {
    let live = true;
    void commands
      .forkTransformsList()
      .then((r) => {
        if (live && r.status === "ok") setEntries(r.data);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, []);

  // Every transform the user has (built-in or custom) and View changes.
  const chords = useMemo(() => {
    const map: Record<string, string[]> = {};
    const actions = new Set([
      POLISH,
      PROMPT,
      ...entries.map((e) => e.action_id),
    ]);
    for (const action of [...actions, VIEW]) map[action] = chordsFor(action);
    return map;
    // `hotkeys` changes the chords `chordsFor` reads.
  }, [entries, hotkeys]);

  const showChanges = async (r: Rewrite) => {
    setSpot(false);
    try {
      const value = await commands.forkTransformDiff(r.source, r.result);
      setDiff(value.ops);
      setChanges(value.changes);
    } catch {
      const ops = wordDiff(r.source, r.result);
      setDiff(ops);
      setChanges(changeCount(ops));
    }
  };

  const onAction = async (action: string) => {
    if (phaseRef.current === "working") return;
    if (action === VIEW) {
      if (rewrite) void showChanges(rewrite);
      return;
    }
    const el = field.current;
    if (!el) return;
    const selection = selectionOf(el);
    if (!selection.text.trim()) {
      setPhase("select_first");
      return;
    }
    setPhase("working");
    setDiff(null);
    setSpot(false);
    const entry =
      entries.find((e) => e.action_id === action) ??
      entries.find((e) => e.builtin_key === "polish");
    const real = await realRewrite(entry, selection.text);
    let next: Rewrite;
    if (real !== null) {
      replaceSelection(el, selection.start, selection.end, real);
      next = { source: selection.text, result: real, example: false };
    } else {
      // The example rewrites the whole message, whatever was selected.
      const source = el.value;
      const result = exampleFor(action, t);
      replaceSelection(el, 0, source.length, result);
      next = { source, result, example: true };
    }
    setRewrite(next);
    setPhase("done");
    setSpot(true);
    ctx.complete();
  };

  // The spotlight puts focus on the bar's View changes (keyboard: Enter).
  useEffect(() => {
    if (spot) viewRef.current?.focus();
  }, [spot]);

  useLessonShortcuts(chords, (action) => void onAction(action));

  const values = shortcutValues();
  return (
    <div
      className="mx-auto flex w-full max-w-[560px] flex-col gap-3"
      data-testid="wb-lesson-practice"
      data-phase={phase}
    >
      <PracticeComposer
        ref={field}
        app={t("lessons.transforms.practice.app")}
        icon={MessageCircle}
        context={t("lessons.transforms.practice.context")}
        label={t("lessons.transforms.practice.fieldLabel")}
        defaultValue={t("lessons.transforms.practice.draft")}
      >
        {spot && (
          <button
            type="button"
            tabIndex={-1}
            aria-hidden="true"
            className="wb-lesson-dim"
            onClick={() => setSpot(false)}
            data-testid="wb-lesson-spotlight"
          />
        )}
        <DrawnBar
          state={
            phase === "working" ? "working" : phase === "done" ? "done" : "idle"
          }
          spot={spot}
          viewRef={viewRef}
          onView={rewrite ? () => void showChanges(rewrite) : undefined}
        />
      </PracticeComposer>
      <div
        aria-live="polite"
        className="type-small min-h-5 text-center text-(--wb-ink-2)"
      >
        {phase === "select_first" && (
          <p data-testid="wb-lesson-select-first">
            {t("lessons.transforms.practice.selectFirst", values)}
          </p>
        )}
        {phase === "working" && (
          <p>{t("lessons.transforms.practice.working")}</p>
        )}
        {phase === "done" && rewrite?.example && (
          <p data-testid="wb-lesson-example-note">
            {t("lessons.transforms.practice.exampleNote")}
          </p>
        )}
        {phase === "done" && spot && (
          <p>{t("lessons.transforms.practice.spotlight", values)}</p>
        )}
      </div>
      {diff && (
        <section
          aria-labelledby="wb-lesson-diff-title"
          className="wb-lesson-changes"
          data-testid="wb-lesson-diff"
        >
          <div className="flex items-center justify-between gap-2">
            <h3 id="wb-lesson-diff-title" className="type-small font-semibold">
              {t("transforms.diff.changes", { count: changes })}
            </h3>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setDiff(null)}
              data-testid="wb-lesson-diff-hide"
            >
              {t("lessons.transforms.practice.hideChanges")}
            </Button>
          </div>
          <DiffWords ops={diff} />
        </section>
      )}
    </div>
  );
}

/* ---------- The lesson ---------- */

let registered = false;

export function registerTransformsLesson(): void {
  if (registered) return;
  registered = true;
  registerLesson({
    id: TRANSFORMS_LESSON,
    icon: WandSparkles,
    titleKey: "lessons.transforms.title",
    tone: "plain",
    steps: [
      {
        id: "examples",
        titleKey: "lessons.transforms.intro.title",
        bodyKey: "lessons.transforms.intro.body",
        continueKey: "lessons.transforms.intro.continue",
        render: () => <Examples />,
      },
      {
        id: "practice",
        gated: true,
        titleKey: "lessons.transforms.practice.title",
        bodyKey: "lessons.transforms.practice.body",
        doneTitleKey: "lessons.transforms.practice.doneTitle",
        doneBodyKey: "lessons.transforms.practice.doneBody",
        values: shortcutValues,
        render: (ctx) => <Practice ctx={ctx} />,
      },
    ],
    howItWorks: (["select", "press", "changes"] as const).map((chapter) => ({
      id: chapter,
      titleKey: `lessons.transforms.how.${chapter}.title`,
      bodyKey: `lessons.transforms.how.${chapter}.body`,
      values: shortcutValues,
      render: () => <Scene key={chapter} chapter={chapter} />,
    })),
    finish: {
      labelKey: "lessons.transforms.finish.turnOn",
      onLabelKey: "lessons.transforms.finish.done",
      isOn: () =>
        useForkSettingsStore.getState().settings?.transforms_enabled === true,
      turnOn: async () => {
        const result = await useForkSettingsStore
          .getState()
          .setSetting("transforms_enabled", true);
        return result.status === "ok";
      },
      after: () => navigate("transforms"),
    },
    help: { labelKey: "lessons.transforms.help", order: 10 },
    checklist: {
      id: TRY_TRANSFORM,
      order: 35,
      icon: WandSparkles,
      titleKey: "checklist.items.try_transform.title",
      actionKey: "checklist.items.try_transform.action",
      bodyKey: "checklist.items.try_transform.body",
      // Any real transform run counts too.
      isDone: async () => {
        const result = await commands.forkTransformLatest();
        return result.status === "ok" && result.data != null;
      },
      refreshOn: ["fork-transform-view-changes"],
    },
  });
}
