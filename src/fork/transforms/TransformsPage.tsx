// SPDX-License-Identifier: GPL-3.0-or-later
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, RotateCcw, ShieldCheck } from "lucide-react";
import {
  commands,
  events,
  type ForkTransformViewChanges,
  type TransformEntry,
  type TransformInput,
  type WordDiff,
} from "../bindings";
import { navigate, useHubNav } from "../hub/nav";
import { PageHeader } from "../hub/PageHeader";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettings } from "../settings/useForkSettings";
import { Banner, Button } from "../ui";
import { CommandModeCard } from "./CommandModeCard";
import { DiffCard } from "./DiffCard";
import { useTransformViewStore } from "./store";
import { TransformCard, CreateTransformCard } from "./TransformCard";
import { TransformModal } from "./TransformModal";

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

  return (
    <div data-testid="wb-transforms" className="flex flex-col gap-6">
      <PageHeader title={t("hub.pages.transforms.title")}>
        <Button onClick={() => void reset()} data-testid="wb-transforms-reset">
          <RotateCcw size={14} aria-hidden="true" />
          {t("transforms.reset")}
        </Button>
        <Button
          variant="primary"
          onClick={() => void create()}
          data-testid="wb-transforms-create"
        >
          <Plus size={14} aria-hidden="true" />
          {t("transforms.createNew")}
        </Button>
      </PageHeader>
      <Banner
        id="transforms.rewrite"
        title={t("transforms.banner.title")}
        body={t("transforms.banner.body")}
      />
      <section
        aria-labelledby="wb-transforms-enable-title"
        className="flex flex-col gap-4 rounded-(--wb-radius-card) border border-(--wb-line) bg-(--wb-card) p-5"
      >
        <label className="flex cursor-pointer items-start gap-3">
          <input
            type="checkbox"
            role="switch"
            checked={settings?.transforms_enabled ?? false}
            disabled={!settings || !!pending.transforms_enabled}
            onChange={(event) =>
              void setSetting("transforms_enabled", event.currentTarget.checked)
            }
            className="mt-1 size-4 accent-(--wb-accent-fill)"
            data-testid="wb-transforms-enabled"
          />
          <span>
            <span
              id="wb-transforms-enable-title"
              className="block text-sm font-semibold"
            >
              {t("transforms.enable.title")}
            </span>
            <span className="mt-1 block text-sm text-(--wb-ink-2)">
              {t("transforms.enable.help")}
            </span>
          </span>
        </label>
        <div className="ms-7 flex flex-col gap-2 border-t border-(--wb-line) pt-4">
          <label className="flex cursor-pointer items-center gap-3 text-sm font-medium">
            <input
              type="checkbox"
              checked={settings?.auto_apply_transform != null}
              disabled={
                !settings?.transforms_enabled || !!pending.auto_apply_transform
              }
              onChange={(event) =>
                void setSetting(
                  "auto_apply_transform",
                  event.currentTarget.checked
                    ? (entries[0]?.key ?? null)
                    : null,
                )
              }
              className="size-4 accent-(--wb-accent-fill)"
              data-testid="wb-transforms-auto"
            />
            {t("transforms.auto.title")}
          </label>
          <p className="text-xs text-(--wb-ink-2)">
            {t("transforms.auto.help")}
          </p>
          <label htmlFor="wb-transform-auto-choice" className="sr-only">
            {t("transforms.auto.choice")}
          </label>
          <select
            id="wb-transform-auto-choice"
            value={settings?.auto_apply_transform ?? ""}
            disabled={
              !settings?.transforms_enabled ||
              !settings.auto_apply_transform ||
              !!pending.auto_apply_transform
            }
            onChange={(event) =>
              void setSetting(
                "auto_apply_transform",
                event.currentTarget.value || null,
              )
            }
            className="w-full max-w-sm rounded-(--wb-radius-control) border border-(--wb-line-strong) bg-(--wb-card) px-3 py-2 text-sm"
            data-testid="wb-transforms-auto-choice"
          >
            {!settings?.auto_apply_transform && (
              <option value="">{t("transforms.auto.none")}</option>
            )}
            {entries.map((entry) => (
              <option key={entry.key} value={entry.key}>
                {entry.name}
              </option>
            ))}
          </select>
        </div>
        <div className="ms-7 flex items-start gap-2 text-xs text-(--wb-ink-2)">
          <ShieldCheck
            size={15}
            aria-hidden="true"
            className="mt-0.5 shrink-0"
          />
          <p>{t("transforms.shortcuts.warning")}</p>
          <Button
            className="shrink-0 px-2 py-1"
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
      />
      {error && (
        <p role="alert" className="text-sm text-red-700 dark:text-red-300">
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
          className="flex flex-col gap-3"
        >
          <h2
            id="wb-my-transforms"
            className="font-(--wb-font-display) text-xl"
          >
            {t("transforms.myTransforms")}
          </h2>
          <div className="grid gap-3 sm:grid-cols-2">
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
            <p role="status" className="text-sm text-(--wb-ink-2)">
              {t("transforms.loading")}
            </p>
          )}
          {!loading && entries.length === 0 && (
            <p className="text-sm text-(--wb-ink-2)">{t("transforms.empty")}</p>
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
