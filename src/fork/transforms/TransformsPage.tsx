// SPDX-License-Identifier: GPL-3.0-or-later
// XF2 (owner, rc6): Scratchpad-level simplicity. A title and one line, the
// list of transforms (name, shortcut, delete) and one "New transform";
// auto-apply, Command Mode, the ⌥-key note and Reset live behind the
// settings button.
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Settings2 } from "../ui/icons";
import {
  commands,
  events,
  type ForkTransformViewChanges,
  type TransformEntry,
  type TransformInput,
  type WordDiff,
} from "../bindings";
import { useHubNav } from "../hub/nav";
import { PageHeader } from "../ui/PageHeader";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettings } from "../settings/useForkSettings";
import { Button, FOCUS_RING, IconButton, Switch, Tag } from "../ui";
import { openLesson, TRANSFORMS_LESSON, useLesson } from "../tutorials";
import { ModelStatus } from "./ModelStatus";
import { DiffCard } from "./DiffCard";
import { useTransformViewStore } from "./store";
import { TransformRow } from "./TransformRow";
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
  const link = `cursor-pointer rounded-(--wb-radius-xs) font-medium text-(--wb-ink) underline-offset-4 hover:underline ${FOCUS_RING}`;

  return (
    <div
      data-testid="wb-transforms"
      className="@container/page flex flex-col gap-6"
    >
      <div className="flex flex-col gap-1.5">
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
          <Switch
            checked={enabled}
            disabled={!settings || !!pending.transforms_enabled}
            onChange={(checked) =>
              void setSetting("transforms_enabled", checked)
            }
            ariaLabelledBy="wb-transforms-enable-title"
            testId="wb-transforms-enabled"
          />
          <IconButton
            aria-label={t("transforms.settings.title")}
            title={t("transforms.settings.title")}
            variant="ghost"
            onClick={() => setSettingsOpen(true)}
            data-testid="wb-transforms-settings"
            className="ms-1"
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
        </PageHeader>
        <p className="flex flex-wrap items-baseline gap-x-3 gap-y-1 text-[15px] text-(--wb-ink-2)">
          <span>{t("transforms.subtitle")}</span>
          {lesson && (
            <>
              <button
                type="button"
                className={link}
                onClick={() => openLesson(TRANSFORMS_LESSON, "try")}
                data-testid="wb-transforms-try-it"
              >
                {t("transforms.banner.tryIt")}
              </button>
              <button
                type="button"
                className={link}
                onClick={() => openLesson(TRANSFORMS_LESSON, "how")}
                data-testid="wb-transforms-how-it-works"
              >
                {t("transforms.banner.howItWorks")}
              </button>
            </>
          )}
        </p>
      </div>
      {/* Only while the instruction model downloads or fails (OT2-H). */}
      <ModelStatus
        wanted={enabled || (settings?.command_mode_enabled ?? false)}
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
        <div>
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
          <ul
            className="flex flex-col gap-2"
            aria-label={t("transforms.myTransforms")}
          >
            {entries.map((entry) => (
              <TransformRow
                key={entry.id}
                entry={entry}
                hotkey={settings?.hotkeys[entry.action_id]?.[0]?.chord}
                onOpen={() => setSelected(entry)}
                onDelete={() => void remove(entry)}
              />
            ))}
          </ul>
        </div>
      )}
      <TransformSettings
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
        entries={entries}
        onReset={() => void reset()}
      />
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
