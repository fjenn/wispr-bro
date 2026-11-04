// SPDX-License-Identifier: GPL-3.0-or-later
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { GraduationCap, Info, Plus, RotateCcw, ShieldCheck } from "../ui/icons";
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
import { Banner, Button, KbdCombo, Select, Switch, Tag, Tooltip } from "../ui";
import { chordLabel } from "../settings/hotkeys/model";
import { openLesson, TRANSFORMS_LESSON, useLesson } from "../tutorials";
import { CommandModeCard } from "./CommandModeCard";
import { DiffCard } from "./DiffCard";
import { useTransformViewStore } from "./store";
import { TransformCard, CreateTransformCard } from "./TransformCard";
import { TransformModal } from "./TransformModal";
import { photos } from "@/assets/fork/art";

function blankInput(t: (key: string) => string): TransformInput {
  return {
    name: t("transforms.new.name"),
    description: t("transforms.new.description"),
    rules: {
      shorter: true,
      clearer: true,
      better_order: true,
      headings_lists: false,
      keep_tone: true,
    },
    prompt: t("transforms.new.prompt"),
    examples: [],
  };
}

export function TransformsPage() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, pending, setSetting } = useForkSettings();
  const section = useHubNav((state) => state.section);
  const currentRun = useTransformViewStore((state) => state.currentRun);
  const [entries, setEntries] = useState<TransformEntry[]>([]);
  const [selected, setSelected] = useState<TransformEntry | null>(null);
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
  const safeShortcuts = async () => {
    for (const [action, chord] of [
      ["transform.polish", "ctrl+option+1"],
      ["transform.prompt_engineer", "ctrl+option+2"],
      ["transform.view_changes", "ctrl+option+o"],
    ] as const) {
      const result = await setSetting("hotkey", {
        action,
        bindings: [{ chord, activation: "press" }],
      });
      if (result.status === "error") break;
    }
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

  return (
    <div
      data-testid="wb-transforms"
      className="@container/page flex flex-col gap-6"
    >
      <PageHeader
        title={t("hub.pages.transforms.title")}
        tag={<Tag kind="beta" />}
      >
        <div className="flex items-center gap-2">
          <span
            id="wb-transforms-enable-title"
            className="text-[15px] font-medium text-(--wb-ink)"
          >
            {t("transforms.enable.title")}
          </span>
          {/* Hover help for sighted users; the switch reads the same text
              through aria-describedby. */}
          <Tooltip text={t("transforms.enable.help")}>
            <span
              aria-hidden="true"
              className="grid size-5 place-items-center rounded-full text-(--wb-ink-3)"
            >
              <Info size={14} aria-hidden="true" />
            </span>
          </Tooltip>
          <span id="wb-transforms-enable-help" className="sr-only">
            {t("transforms.enable.help")}
          </span>
          <Switch
            checked={enabled}
            disabled={!settings || !!pending.transforms_enabled}
            onChange={(checked) =>
              void setSetting("transforms_enabled", checked)
            }
            ariaLabelledBy="wb-transforms-enable-title"
            ariaDescribedBy="wb-transforms-enable-help"
            testId="wb-transforms-enabled"
          />
        </div>
        {viewChord && (
          <span className="hidden items-center gap-2 rounded-(--wb-radius-sm) border border-(--wb-line-2) bg-(--wb-surface) py-1 ps-1.5 pe-3 text-[13px] text-(--wb-ink-2) @min-[700px]/page:flex">
            <KbdCombo keys={chordLabel(viewChord).split(" ")} />
            {t("pill.viewChanges")}
          </span>
        )}
      </PageHeader>
      <Banner
        id="transforms.rewrite"
        image={photos.transforms.src}
        title={t("transforms.banner.title")}
        body={t("transforms.banner.body")}
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
      <section
        aria-labelledby="wb-transforms-auto-title"
        className="flex flex-col gap-3"
      >
        <div className="flex flex-wrap items-center gap-x-6 gap-y-3">
          <div className="flex min-w-60 flex-1 flex-col gap-0.5">
            <h2
              id="wb-transforms-auto-title"
              className="text-[15px] leading-5 font-medium text-(--wb-ink)"
            >
              {t("transforms.auto.title")}
            </h2>
            <p
              id="wb-transforms-auto-help"
              className="text-[13px] leading-[18px] text-(--wb-ink-2)"
            >
              {t("transforms.auto.help")}
            </p>
          </div>
          <div className="flex items-center gap-3">
            <Select
              value={settings?.auto_apply_transform ?? undefined}
              placeholder={t("transforms.auto.none")}
              onValueChange={(value) =>
                void setSetting("auto_apply_transform", value || null)
              }
              disabled={
                !enabled ||
                !settings?.auto_apply_transform ||
                !!pending.auto_apply_transform
              }
              label={t("transforms.auto.choice")}
              testId="wb-transforms-auto-choice"
              className="w-52"
              options={entries.map((entry) => ({
                value: entry.key,
                label: entry.name,
              }))}
            />
            <Switch
              checked={settings?.auto_apply_transform != null}
              disabled={!enabled || !!pending.auto_apply_transform}
              onChange={(checked) =>
                void setSetting(
                  "auto_apply_transform",
                  checked ? (entries[0]?.key ?? null) : null,
                )
              }
              ariaLabelledBy="wb-transforms-auto-title"
              ariaDescribedBy="wb-transforms-auto-help"
              testId="wb-transforms-auto"
            />
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-2 rounded-(--wb-radius-sm) bg-(--wb-surface-tint) px-4 py-2.5 text-[13px] text-(--wb-ink-2)">
          <ShieldCheck size={16} aria-hidden="true" className="shrink-0" />
          <p className="min-w-0 flex-1">{t("transforms.shortcuts.warning")}</p>
          <Button
            size="sm"
            variant="outline"
            disabled={!settings || !!pending.hotkey}
            onClick={() => void safeShortcuts()}
            data-testid="wb-transforms-safe-shortcuts"
          >
            {t("transforms.shortcuts.safe")}
          </Button>
        </div>
      </section>
      <CommandModeCard
        checked={settings?.command_mode_enabled ?? false}
        disabled={!settings || !!pending.command_mode_enabled}
        onChange={(checked) => void setSetting("command_mode_enabled", checked)}
        shortcut={
          settings ? settings.hotkeys["command_mode"]?.[0]?.chord : "fn+ctrl"
        }
      />
      {error && (
        <p role="alert" className="text-[13px] text-(--wb-danger)">
          {t("transforms.error")}
        </p>
      )}
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
        <section
          aria-labelledby="wb-my-transforms"
          className="flex flex-col gap-5 pt-2"
        >
          <div className="flex flex-wrap items-center justify-between gap-3">
            <h2
              id="wb-my-transforms"
              className="type-serif-title text-(--wb-ink)"
            >
              {t("transforms.myTransforms")}
            </h2>
            <div className="flex flex-wrap items-center gap-1">
              {lesson && (
                <Button
                  variant="ghost"
                  onClick={() => openLesson(TRANSFORMS_LESSON, "try")}
                  data-testid="wb-transforms-tutorial"
                >
                  <GraduationCap size={16} aria-hidden="true" />
                  {t("transforms.startTutorial")}
                </Button>
              )}
              <Button
                variant="ghost"
                onClick={() => void reset()}
                data-testid="wb-transforms-reset"
              >
                <RotateCcw size={16} aria-hidden="true" />
                {t("transforms.reset")}
              </Button>
              <Button
                variant="primary"
                onClick={() => void create()}
                data-testid="wb-transforms-create"
                className="ms-1"
              >
                <Plus size={16} aria-hidden="true" />
                {t("transforms.createNew")}
              </Button>
            </div>
          </div>
          <div className="grid gap-6 @min-[640px]/page:grid-cols-3">
            {entries.map((entry) => (
              <TransformCard
                key={entry.id}
                entry={entry}
                hotkey={settings?.hotkeys[entry.action_id]?.[0]?.chord}
                onOpen={() => setSelected(entry)}
                onDelete={() => void remove(entry)}
              />
            ))}
            <CreateTransformCard onClick={() => void create()} />
          </div>
          {loading && (
            <p role="status" className="text-[15px] text-(--wb-ink-2)">
              {t("transforms.loading")}
            </p>
          )}
          {!loading && entries.length === 0 && (
            <p className="text-[15px] text-(--wb-ink-2)">
              {t("transforms.empty")}
            </p>
          )}
        </section>
      )}
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
