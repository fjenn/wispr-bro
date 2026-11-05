// SPDX-License-Identifier: GPL-3.0-or-later
// The Transforms page (S-22; flowmatch, after Wispr Flow's structure): the
// title with the on/off switch and the "view changes" key pill, one banner
// (Try it, How it works), one auto-apply row, then "My Transforms" (Reset,
// settings for Command Mode and the ⌥-key note, New transform) as a grid of
// cards ending with "Create your own". The model download row shows only
// while it downloads or failed (OT2-H); ⌥O swaps the grid for the diff.
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Info, Plus, RotateCcw, Settings2 } from "../ui/icons";
import {
  commands,
  events,
  type ForkTransformViewChanges,
  type TransformEntry,
  type TransformInput,
  type WordDiff,
} from "../bindings";
import { navigate, useHubNav } from "../hub/nav";
import { PageHeader } from "../ui/PageHeader";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettings } from "../settings/useForkSettings";
import {
  Button,
  FOCUS_RING,
  Hero,
  IconButton,
  KbdCombo,
  Select,
  SettingRow,
  Switch,
  Tag,
  Tooltip,
} from "../ui";
import { photos } from "@/assets/fork/art";
import { chordLabel } from "../settings/hotkeys/model";
import { openLesson, TRANSFORMS_LESSON, useLesson } from "../tutorials";
import { ModelStatus } from "./ModelStatus";
import { DiffCard } from "./DiffCard";
import { useTransformViewStore } from "./store";
import { CreateCard, TransformCard } from "./TransformCard";
import { TransformModal } from "./TransformModal";
import { TransformSettings } from "./TransformSettings";

function blankInput(t: (key: string) => string): TransformInput {
  return {
    name: t("transforms.new.name"),
    // XF2: one instruction field, empty until the user says what to do.
    description: "",
    rules: {
      shorter: false,
      clearer: false,
      better_order: false,
      headings_lists: false,
      keep_tone: false,
    },
    prompt: "",
    examples: [],
    replacements: [],
  };
}

export function TransformsPage() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, pending, setSetting } = useForkSettings();
  const section = useHubNav((state) => state.section);
  const currentRun = useTransformViewStore((state) => state.currentRun);
  const [entries, setEntries] = useState<TransformEntry[]>([]);
  const [selected, setSelected] = useState<TransformEntry | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [error, setError] = useState(false);
  const [loading, setLoading] = useState(true);
  const [diff, setDiff] = useState<WordDiff | null>(null);
  // S-31: the lesson's entry points appear once it is registered.
  const lesson = useLesson(TRANSFORMS_LESSON);

  const load = useCallback(async () => {
    const result = await commands.forkTransformsList();
    if (result.status === "ok") {
      setEntries(result.data);
      setError(false);
    } else setError(true);
    setLoading(false);
  }, []);

  useEffect(() => {
    void load();
  }, [load]);
  useEffect(() => {
    let active = true;
    let off: (() => void) | undefined;
    void events.forkTransformsChanged
      .listen(() => void load())
      .then((unlisten) => {
        if (active) off = unlisten;
        else unlisten();
      });
    return () => {
      active = false;
      off?.();
    };
  }, [load]);

  useEffect(() => {
    if (section !== "changes") return;
    if (!currentRun) {
      void commands.forkTransformLatest().then((result) => {
        if (result.status === "ok" && result.data)
          useTransformViewStore.setState({ currentRun: result.data });
      });
    }
  }, [section, currentRun]);

  useEffect(() => {
    if (!currentRun || section !== "changes") {
      setDiff(null);
      return;
    }
    let active = true;
    void commands
      .forkTransformDiff(currentRun.source_text, currentRun.result_text)
      .then((value) => {
        if (active) setDiff(value);
      });
    return () => {
      active = false;
    };
  }, [currentRun, section]);

  const create = async () => {
    const result = await commands.forkTransformCreate(blankInput(t));
    if (result.status === "ok") {
      setEntries((current) => [...current, result.data]);
      setSelected(result.data);
    } else setError(true);
  };
  const reset = async () => {
    if (!window.confirm(t("transforms.resetConfirm"))) return;
    const result = await commands.forkTransformsReset();
    if (result.status === "error") {
      setError(true);
      return;
    }
    setEntries(result.data);
    await setSetting("auto_apply_transform", null);
    for (const entry of entries.filter((entry) => entry.builtin_key === null)) {
      await setSetting("hotkey", { action: entry.action_id, bindings: [] });
    }
    for (const [action, chord] of [
      ["transform.polish", "option+1"],
      ["transform.prompt_engineer", "option+2"],
      ["transform.view_changes", "option+o"],
    ] as const) {
      await setSetting("hotkey", {
        action,
        bindings: [{ chord, activation: "press" }],
      });
    }
  };
  const remove = async (entry: TransformEntry) => {
    if (!window.confirm(t("transforms.deleteConfirm", { name: entry.name })))
      return;
    const result = await commands.forkTransformDelete(entry.id);
    if (result.status === "ok") {
      setEntries((current) => current.filter((item) => item.id !== entry.id));
    } else setError(true);
  };
  const rate = async (run: ForkTransformViewChanges, rating: -1 | 1) => {
    const result = await commands.forkTransformRate(run.run_id, rating);
    if (result.status === "ok")
      useTransformViewStore.setState({ currentRun: result.data });
    else setError(true);
  };
  const retry = async () => {
    if (!currentRun) return;
    const result = await commands.forkTransformRetry(currentRun.run_id);
    if (result.status === "ok")
      useTransformViewStore.setState({ currentRun: result.data });
    else throw new Error(t("transforms.diff.actionFailed"));
  };

  const enabled = settings?.transforms_enabled ?? false;
  const viewChord = settings?.hotkeys["transform.view_changes"]?.[0]?.chord;
  const autoOn = settings?.auto_apply_transform != null;

  return (
    <div
      data-testid="wb-transforms"
      className="@container/page flex flex-col gap-8"
    >
      <PageHeader
        title={t("hub.pages.transforms.title")}
        tag={<Tag kind="beta" />}
      >
        <span
          id="wb-transforms-enable-title"
          className="text-[15px] text-(--wb-ink)"
        >
          {t("transforms.enable.title")}
        </span>
        <Tooltip text={t("transforms.subtitle")}>
          <button
            type="button"
            aria-label={t("transforms.subtitle")}
            className={`grid size-7 cursor-default place-items-center rounded-full text-(--wb-ink-2) ${FOCUS_RING}`}
          >
            <Info size={16} aria-hidden="true" />
          </button>
        </Tooltip>
        <Switch
          checked={enabled}
          disabled={!settings || !!pending.transforms_enabled}
          onChange={(checked) => void setSetting("transforms_enabled", checked)}
          ariaLabelledBy="wb-transforms-enable-title"
          testId="wb-transforms-enabled"
        />
        {viewChord && (
          <button
            type="button"
            onClick={() => navigate("transforms", "changes")}
            data-testid="wb-transforms-view-changes"
            className={`ms-2 flex h-9 cursor-pointer items-center gap-2 rounded-full border border-(--wb-line) bg-(--wb-surface) ps-1.5 pe-3.5 text-[14px] text-(--wb-ink-2) transition-colors duration-(--wb-dur-fast) hover:bg-(--wb-surface-tint) hover:text-(--wb-ink) ${FOCUS_RING}`}
          >
            <KbdCombo keys={chordLabel(viewChord).split(" ")} />
            {t("transforms.viewChanges")}
          </button>
        )}
      </PageHeader>

      {section === "changes" && currentRun && diff ? (
        <DiffCard
          run={currentRun}
          diff={diff}
          onConfigure={() => {
            const entry = entries.find(
              (entry) => entry.id === currentRun.transform_id,
            );
            if (entry) setSelected(entry);
          }}
          onRate={(rating) => void rate(currentRun, rating)}
          onRetry={retry}
        />
      ) : (
        <>
          <Hero
            id="transforms"
            image={photos.transforms.src}
            title={t("transforms.banner.title")}
            body={t("transforms.subtitle")}
            action={
              lesson
                ? {
                    label: t("transforms.banner.tryIt"),
                    onClick: () => openLesson(TRANSFORMS_LESSON, "try"),
                    testId: "wb-transforms-try-it",
                  }
                : undefined
            }
            secondaryAction={
              lesson
                ? {
                    label: t("transforms.banner.howItWorks"),
                    onClick: () => openLesson(TRANSFORMS_LESSON, "how"),
                    testId: "wb-transforms-how-it-works",
                  }
                : undefined
            }
          />
          {/* Only while the instruction model downloads or fails (OT2-H). */}
          <ModelStatus
            wanted={enabled || (settings?.command_mode_enabled ?? false)}
          />
          <SettingRow
            title={t("transforms.auto.title")}
            description={t("transforms.auto.description")}
            testId="wb-transforms-auto-row"
            className="border-transparent! bg-transparent! px-0! py-0!"
            control={(ids) => (
              <>
                <Select
                  value={settings?.auto_apply_transform ?? undefined}
                  placeholder={t("transforms.auto.none")}
                  onValueChange={(value) =>
                    void setSetting("auto_apply_transform", value || null)
                  }
                  disabled={
                    !enabled || !autoOn || !!pending.auto_apply_transform
                  }
                  label={t("transforms.auto.choice")}
                  testId="wb-transforms-auto-choice"
                  className="w-48"
                  options={entries.map((entry) => ({
                    value: entry.key,
                    label: entry.name,
                  }))}
                />
                <Switch
                  checked={autoOn}
                  disabled={!enabled || !!pending.auto_apply_transform}
                  onChange={(checked) =>
                    void setSetting(
                      "auto_apply_transform",
                      checked ? (entries[0]?.key ?? null) : null,
                    )
                  }
                  ariaLabelledBy={ids.labelledBy}
                  ariaDescribedBy={ids.describedBy}
                  testId="wb-transforms-auto"
                />
              </>
            )}
          />
          <section
            aria-labelledby="wb-transforms-mine"
            className="flex flex-col gap-5"
          >
            <div className="flex flex-wrap items-center justify-between gap-3">
              <h2
                id="wb-transforms-mine"
                className="type-serif-title text-[36px]! leading-[42px]! text-(--wb-ink)"
              >
                {t("transforms.myTransforms")}
              </h2>
              <div className="flex items-center gap-2">
                <Button
                  variant="ghost"
                  onClick={() => void reset()}
                  data-testid="wb-transforms-reset"
                >
                  <RotateCcw size={16} aria-hidden="true" />
                  {t("transforms.reset")}
                </Button>
                <IconButton
                  aria-label={t("transforms.settings.title")}
                  title={t("transforms.settings.title")}
                  variant="ghost"
                  onClick={() => setSettingsOpen(true)}
                  data-testid="wb-transforms-settings"
                >
                  <Settings2 size={18} aria-hidden="true" />
                </IconButton>
                <Button
                  variant="primary"
                  onClick={() => void create()}
                  data-testid="wb-transforms-create"
                >
                  <Plus size={16} aria-hidden="true" />
                  {t("transforms.createNew")}
                </Button>
              </div>
            </div>
            {error && (
              <p role="alert" className="text-[13px] text-(--wb-danger)">
                {t("transforms.error")}
              </p>
            )}
            {loading ? (
              <p role="status" className="text-[15px] text-(--wb-ink-2)">
                {t("transforms.loading")}
              </p>
            ) : (
              <ul
                className="grid grid-cols-1 gap-4 @min-[480px]/page:grid-cols-2 @min-[660px]/page:grid-cols-3"
                aria-labelledby="wb-transforms-mine"
              >
                {entries.map((entry) => (
                  <TransformCard
                    key={entry.id}
                    entry={entry}
                    hotkey={settings?.hotkeys[entry.action_id]?.[0]?.chord}
                    onOpen={() => setSelected(entry)}
                    onDelete={() => void remove(entry)}
                  />
                ))}
                <CreateCard onCreate={() => void create()} />
              </ul>
            )}
          </section>
        </>
      )}
      <TransformSettings open={settingsOpen} onOpenChange={setSettingsOpen} />
      {selected && (
        <TransformModal
          entry={selected}
          onClose={() => setSelected(null)}
          onSaved={(entry) => {
            setSelected(entry);
            setEntries((current) =>
              current.map((item) => (item.id === entry.id ? entry : item)),
            );
          }}
        />
      )}
    </div>
  );
}
