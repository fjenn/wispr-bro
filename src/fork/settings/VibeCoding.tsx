// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Settings › Vibe coding (S-36, D-59/D-60, DEC-41). "Recognize code names"
 * stays off until the setup: the user picks their editors and turns on each
 * editor's screen-reader mode (VS Code's "Editor: Accessibility Support"),
 * so the editor exposes its text through Accessibility. "Tag files in chat"
 * then works in Cursor's and Windsurf's chat inputs. Nothing here reads a
 * project: the backend (`fork/context/ide.rs`, `fork/text/file_tags.rs`)
 * uses only what the editor shows on screen.
 */
import { useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Code2 } from "../ui/icons";
import type { VibeCodingSettings } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";
import { ForkToggle, Panel } from "./Controls";
import { useForkSettings } from "./useForkSettings";

/** The editors the backend knows (`ide::Editor` ids), with product names. */
export const VIBE_EDITORS = [
  { id: "vscode", name: "VS Code", chat: false },
  { id: "cursor", name: "Cursor", chat: true },
  { id: "windsurf", name: "Windsurf", chat: true },
] as const;

const hasChat = (editors: string[]) =>
  VIBE_EDITORS.some((editor) => editor.chat && editors.includes(editor.id));

const names = (editors: string[]) =>
  VIBE_EDITORS.filter((editor) => editors.includes(editor.id))
    .map((editor) => editor.name)
    .join(", ");

function Setup({
  current,
  busy,
  onCancel,
  onFinish,
}: {
  current: VibeCodingSettings;
  busy: boolean;
  onCancel: (() => void) | null;
  onFinish: (editors: string[]) => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [editors, setEditors] = useState<string[]>(current.editors);
  const [confirmed, setConfirmed] = useState(false);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const stepsId = useId();
  const confirmId = useId();

  useEffect(() => {
    headingRef.current?.focus();
  }, []);

  const toggle = (id: string, on: boolean) =>
    setEditors((list) =>
      on
        ? VIBE_EDITORS.map((e) => e.id).filter(
            (e) => e === id || list.includes(e),
          )
        : list.filter((e) => e !== id),
    );
  const ready = editors.length > 0 && confirmed && !busy;

  return (
    <form
      aria-labelledby={`${stepsId}-title`}
      className="flex flex-col gap-4"
      data-testid="wb-vibe-setup"
      onSubmit={(event) => {
        event.preventDefault();
        if (ready) onFinish(editors);
      }}
    >
      <h4
        id={`${stepsId}-title`}
        ref={headingRef}
        tabIndex={-1}
        className="text-sm font-semibold outline-none"
      >
        {t("settingsModal.vibe.setup.title")}
      </h4>
      <fieldset className="flex flex-col gap-1">
        <legend className="mb-1 text-sm font-medium">
          {t("settingsModal.vibe.setup.stepEditors")}
        </legend>
        {VIBE_EDITORS.map((editor) => (
          <label
            key={editor.id}
            className="flex cursor-pointer items-center gap-2 py-1 text-sm"
          >
            <input
              type="checkbox"
              checked={editors.includes(editor.id)}
              onChange={(event) =>
                toggle(editor.id, event.currentTarget.checked)
              }
              data-testid={`wb-vibe-editor-${editor.id}`}
            />
            {editor.name}
          </label>
        ))}
      </fieldset>
      <div className="flex flex-col gap-2">
        <p className="text-sm font-medium">
          {t("settingsModal.vibe.setup.stepScreenReader")}
        </p>
        <ol
          className="ms-5 list-decimal text-sm text-(--wb-ink-2)"
          data-testid="wb-vibe-steps"
        >
          <li>{t("settingsModal.vibe.setup.openSettings")}</li>
          <li>{t("settingsModal.vibe.setup.search")}</li>
          <li>{t("settingsModal.vibe.setup.setOn")}</li>
        </ol>
        <p className="text-xs text-(--wb-ink-3)">
          {t("settingsModal.vibe.setup.palette")}
        </p>
      </div>
      <label
        htmlFor={confirmId}
        className="flex cursor-pointer items-center gap-2 text-sm"
      >
        <input
          id={confirmId}
          type="checkbox"
          checked={confirmed}
          onChange={(event) => setConfirmed(event.currentTarget.checked)}
          data-testid="wb-vibe-confirm"
        />
        {t("settingsModal.vibe.setup.confirm")}
      </label>
      <div className="flex flex-wrap justify-end gap-2">
        {onCancel && (
          <Button onClick={onCancel} data-testid="wb-vibe-cancel">
            {t("settingsModal.vibe.setup.cancel")}
          </Button>
        )}
        <Button
          type="submit"
          variant="primary"
          disabled={!ready}
          data-testid="wb-vibe-finish"
        >
          {t("settingsModal.vibe.setup.finish")}
        </Button>
      </div>
    </form>
  );
}

export function VibeCoding() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, pending, setSetting } = useForkSettings();
  const [setupOpen, setSetupOpen] = useState(false);
  const setupButton = useRef<HTMLButtonElement>(null);
  const restoreFocus = useRef(false);

  useEffect(() => {
    if (!setupOpen && restoreFocus.current) {
      restoreFocus.current = false;
      setupButton.current?.focus();
    }
  }, [setupOpen]);

  if (!settings) return null;
  const vibe = settings.vibe_coding;
  const busy = !!pending.vibe_coding;
  const save = (next: VibeCodingSettings) => setSetting("vibe_coding", next);
  const close = () => {
    restoreFocus.current = true;
    setSetupOpen(false);
  };
  const chatReady = vibe.setup_completed && hasChat(vibe.editors);

  return (
    <div className="flex flex-col gap-6" data-testid="wb-settings-vibe">
      <p className="text-sm text-(--wb-ink-2)" data-testid="wb-vibe-intro">
        {t("settingsModal.vibe.intro")}
      </p>
      {!settings.context_awareness && (
        <p
          role="note"
          className="rounded-(--wb-radius-sm) border border-(--wb-line-strong) px-3 py-2 text-sm"
          data-testid="wb-vibe-context-off"
        >
          {t("settingsModal.vibe.contextOff")}
        </p>
      )}
      <Panel
        title={t("settingsModal.vibe.recognize")}
        testId="wb-vibe-recognize"
      >
        {setupOpen ? (
          <Setup
            current={vibe}
            busy={busy}
            onCancel={close}
            onFinish={(editors) => {
              void save({
                enabled: true,
                setup_completed: true,
                editors,
                file_tags_enabled: vibe.file_tags_enabled && hasChat(editors),
              }).then((result) => {
                if (result.status === "ok") close();
              });
            }}
          />
        ) : vibe.setup_completed ? (
          <div className="flex flex-col gap-2">
            <ForkToggle
              label={t("settingsModal.vibe.recognize")}
              hint={t("settingsModal.vibe.recognizeHint")}
              checked={vibe.enabled}
              disabled={busy}
              onChange={(checked) => void save({ ...vibe, enabled: checked })}
              testId="wb-vibe-enabled"
            />
            <div className="flex flex-wrap items-center justify-between gap-2">
              <span
                className="text-sm text-(--wb-ink-2)"
                data-testid="wb-vibe-editors"
              >
                {t("settingsModal.vibe.setUpFor", {
                  editors: names(vibe.editors),
                })}
              </span>
              <Button
                ref={setupButton}
                onClick={() => setSetupOpen(true)}
                data-testid="wb-vibe-change"
              >
                {t("settingsModal.vibe.change")}
              </Button>
            </div>
          </div>
        ) : (
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="flex items-start gap-3">
              <Code2
                size={20}
                aria-hidden="true"
                className="mt-0.5 shrink-0 text-(--wb-ink-2)"
              />
              <p className="max-w-md text-sm text-(--wb-ink-2)">
                {t("settingsModal.vibe.recognizeHint")}
              </p>
            </div>
            <Button
              ref={setupButton}
              variant="primary"
              onClick={() => setSetupOpen(true)}
              data-testid="wb-vibe-setup-start"
            >
              {t("settingsModal.vibe.setUp")}
            </Button>
          </div>
        )}
      </Panel>
      <Panel title={t("settingsModal.vibe.fileTags")} testId="wb-vibe-files">
        <ForkToggle
          label={t("settingsModal.vibe.fileTags")}
          hint={
            chatReady
              ? t("settingsModal.vibe.fileTagsHint")
              : vibe.setup_completed
                ? t("settingsModal.vibe.fileTagsNeedsChat")
                : t("settingsModal.vibe.fileTagsNeedsSetup")
          }
          checked={chatReady && vibe.file_tags_enabled}
          disabled={busy || !chatReady}
          onChange={(checked) =>
            void save({ ...vibe, file_tags_enabled: checked })
          }
          testId="wb-vibe-file-tags"
        />
      </Panel>
    </div>
  );
}
