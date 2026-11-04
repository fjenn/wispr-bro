// SPDX-License-Identifier: GPL-3.0-or-later
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Play, RotateCcw, X } from "../ui/icons";
import {
  commands,
  type HotkeyBinding,
  type TransformEntry,
  type TransformInput,
} from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button, FOCUS_RING } from "../ui";
import { BindingRow } from "../settings/hotkeys/BindingRow";
import { useForkSettings } from "../settings/useForkSettings";
import { ExamplesEditor } from "./ExamplesEditor";
import { RulesList } from "./RulesList";

function inputFor(entry: TransformEntry): TransformInput {
  return {
    name: entry.name,
    description: entry.description,
    rules: { ...entry.rules },
    prompt: entry.prompt,
    examples: [...entry.examples],
  };
}

export function TransformModal({
  entry,
  onClose,
  onSaved,
}: {
  entry: TransformEntry;
  onClose: () => void;
  onSaved: (entry: TransformEntry) => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, setSetting } = useForkSettings();
  const [draft, setDraft] = useState<TransformInput>(() => inputFor(entry));
  const [saveState, setSaveState] = useState<"saved" | "saving" | "error">(
    "saved",
  );
  const [capturing, setCapturing] = useState(false);
  const [previewing, setPreviewing] = useState(false);
  const [preview, setPreview] = useState<string | null>(null);
  const [example, setExample] = useState(() =>
    t("transforms.editor.exampleText"),
  );
  const dialogRef = useRef<HTMLDialogElement>(null);
  const original = useRef(inputFor(entry));
  const draftRef = useRef(draft);
  const dirty = useRef(false);
  const saveSequence = useRef(0);
  draftRef.current = draft;

  useEffect(() => {
    const dialog = dialogRef.current;
    if (dialog && !dialog.open) dialog.showModal();
  }, []);

  const save = useCallback(
    async (value: TransformInput) => {
      const sequence = ++saveSequence.current;
      setSaveState("saving");
      const result = await commands.forkTransformUpdate(entry.id, value);
      if (sequence !== saveSequence.current) return result.status === "ok";
      if (result.status === "ok") {
        dirty.current = false;
        original.current = value;
        setSaveState("saved");
        onSaved(result.data);
        return true;
      }
      setSaveState("error");
      return false;
    },
    [entry.id, onSaved],
  );

  useEffect(() => {
    if (JSON.stringify(draft) === JSON.stringify(original.current)) return;
    dirty.current = true;
    const timer = window.setTimeout(() => void save(draft), 550);
    return () => window.clearTimeout(timer);
  }, [draft, save]);

  const close = async () => {
    saveSequence.current += 1;
    if (dirty.current) await save(draftRef.current);
    onClose();
  };
  const update = <K extends keyof TransformInput>(
    key: K,
    value: TransformInput[K],
  ) => {
    setDraft((current) => ({ ...current, [key]: value }));
    setPreview(null);
  };
  const runPreview = async () => {
    setPreviewing(true);
    setPreview(null);
    const result = await commands.forkTransformPreview(
      entry.id,
      draft,
      example,
    );
    if (result.status === "ok") setPreview(result.data);
    else setSaveState("error");
    setPreviewing(false);
  };
  const bindings = settings?.hotkeys[entry.action_id] ?? [];
  const binding: HotkeyBinding = bindings[0] ?? {
    chord: "",
    activation: "press",
  };
  const saveHotkey = async (next: HotkeyBinding[]) => {
    const result = await setSetting("hotkey", {
      action: entry.action_id,
      bindings: next,
    });
    if (result.status === "error") setSaveState("error");
  };

  return (
    <dialog
      ref={dialogRef}
      aria-labelledby="wb-transform-editor-title"
      className="wb-modal m-auto h-[min(760px,calc(100vh-32px))] w-[min(720px,calc(100vw-32px))] max-w-none overflow-hidden rounded-(--wb-radius-lg) border-0 bg-(--wb-surface) p-0 text-(--wb-ink) shadow-(--wb-elev-3)"
      data-testid="wb-transform-modal"
      onCancel={(event) => {
        event.preventDefault();
        void close();
      }}
    >
      <div className="flex h-full min-h-0 flex-col">
        <header className="flex items-center justify-between gap-4 border-b border-(--wb-line) px-6 py-4">
          <div>
            <h2 id="wb-transform-editor-title" className="type-heading m-0">
              {t("transforms.editor.title", { name: entry.name })}
            </h2>
            <p
              className="type-small mt-0.5 text-(--wb-ink-2)"
              role="status"
              data-testid="wb-transform-autosave"
            >
              {t(`transforms.editor.autosave.${saveState}`)}
            </p>
          </div>
          <button
            type="button"
            onClick={() => void close()}
            aria-label={t("transforms.editor.close")}
            title={t("transforms.editor.close")}
            className={`grid size-8 cursor-pointer place-items-center rounded-(--wb-radius-sm) text-(--wb-ink-2) transition-colors duration-(--wb-dur-fast) hover:bg-(--wb-fill-hover) hover:text-(--wb-ink) ${FOCUS_RING}`}
            data-testid="wb-transform-close"
          >
            <X size={18} aria-hidden="true" />
          </button>
        </header>
        <div className="flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto p-6">
          <div className="grid gap-4 sm:grid-cols-2">
            <label className="flex flex-col gap-1.5 text-[14px] font-medium">
              {t("transforms.editor.name")}
              <input
                value={draft.name}
                maxLength={80}
                onChange={(event) => update("name", event.currentTarget.value)}
                className={`rounded-(--wb-radius-sm) border border-(--wb-line-strong) bg-(--wb-surface) px-3 py-2 text-[15px] font-normal select-text transition-colors duration-(--wb-dur-fast) hover:border-(--wb-ink-3) ${FOCUS_RING}`}
                data-testid="wb-transform-name"
              />
            </label>
            <div className="flex flex-col gap-1.5">
              <span className="text-[14px] font-medium">
                {t("transforms.editor.hotkey")}
              </span>
              {capturing || binding.chord ? (
                <BindingRow
                  actionId={entry.action_id}
                  binding={binding}
                  index={0}
                  capturing={capturing}
                  captureBusy={false}
                  status={null}
                  activations={["press"]}
                  onCapture={() => setCapturing(true)}
                  onCaptured={(chord) => {
                    setCapturing(false);
                    if (chord)
                      void saveHotkey([{ chord, activation: "press" }]);
                  }}
                  onActivation={() => undefined}
                  onDelete={
                    bindings.length ? () => void saveHotkey([]) : undefined
                  }
                />
              ) : (
                <Button
                  onClick={() => setCapturing(true)}
                  data-testid="wb-transform-hotkey-add"
                >
                  {t("transforms.editor.addHotkey")}
                </Button>
              )}
            </div>
          </div>
          <label className="flex flex-col gap-1.5 text-[14px] font-medium">
            {t("transforms.editor.description")}
            <textarea
              value={draft.description}
              maxLength={500}
              rows={2}
              onChange={(event) =>
                update("description", event.currentTarget.value)
              }
              className={`resize-y rounded-(--wb-radius-sm) border border-(--wb-line-strong) bg-(--wb-surface) px-3 py-2 text-[15px] font-normal select-text transition-colors duration-(--wb-dur-fast) hover:border-(--wb-ink-3) ${FOCUS_RING}`}
              data-testid="wb-transform-description"
            />
          </label>
          <section
            aria-labelledby="wb-transform-example-title"
            className="flex flex-col gap-3 rounded-(--wb-radius-md) bg-(--wb-surface-tint) p-4"
          >
            <h3
              id="wb-transform-example-title"
              className="type-micro m-0 text-(--wb-ink-3)"
            >
              {t("transforms.editor.example")}
            </h3>
            <label htmlFor="wb-transform-example-input" className="sr-only">
              {t("transforms.editor.exampleInput")}
            </label>
            <textarea
              id="wb-transform-example-input"
              value={example}
              rows={3}
              onChange={(event) => {
                setExample(event.currentTarget.value);
                setPreview(null);
              }}
              className={`resize-y rounded-(--wb-radius-sm) border border-(--wb-line-strong) bg-(--wb-surface) p-3 text-[15px] select-text ${FOCUS_RING}`}
              data-testid="wb-transform-example-input"
            />
            <div>
              <Button
                disabled={previewing || !example.trim()}
                onClick={() => void runPreview()}
                data-testid="wb-transform-preview"
              >
                <Play size={14} aria-hidden="true" />
                {previewing
                  ? t("transforms.editor.previewing")
                  : t("transforms.editor.preview")}
              </Button>
            </div>
            {preview !== null && (
              <p
                className="whitespace-pre-wrap border-t border-(--wb-line) pt-3 type-body leading-6 select-text"
                data-testid="wb-transform-preview-result"
              >
                {preview}
              </p>
            )}
          </section>
          <RulesList
            value={draft.rules}
            onChange={(rules) => update("rules", rules)}
          />
          <label className="flex flex-col gap-1.5 text-[14px] font-medium">
            {t("transforms.editor.customPrompt")}
            <span className="type-small font-normal text-(--wb-ink-2)">
              {t("transforms.editor.customPromptHelp")}
            </span>
            <textarea
              value={draft.prompt}
              maxLength={6000}
              rows={5}
              onChange={(event) => update("prompt", event.currentTarget.value)}
              className={`resize-y rounded-(--wb-radius-sm) border border-(--wb-line-strong) bg-(--wb-surface) px-3 py-2 text-[15px] font-normal select-text transition-colors duration-(--wb-dur-fast) hover:border-(--wb-ink-3) ${FOCUS_RING}`}
              data-testid="wb-transform-prompt"
            />
          </label>
          <ExamplesEditor
            value={draft.examples}
            onChange={(examples) => update("examples", examples)}
          />
        </div>
        <footer className="flex items-center justify-between border-t border-(--wb-line) px-6 py-4">
          <Button
            variant="ghost"
            leading={<RotateCcw size={14} aria-hidden="true" />}
            disabled={saveState === "saving"}
            onClick={() => {
              setDraft(inputFor(entry));
              setPreview(null);
            }}
            data-testid="wb-transform-reset"
          >
            {t("transforms.editor.reset")}
          </Button>
          <Button
            variant="primary"
            onClick={() => void close()}
            data-testid="wb-transform-done"
          >
            {t("transforms.editor.done")}
          </Button>
        </footer>
      </div>
    </dialog>
  );
}
