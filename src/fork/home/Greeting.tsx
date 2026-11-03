// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The home greeting (S-15a, C-28, C-217): "Hello, <first name>" as the
 * page's serif `h1`.
 *
 * The name is the fork setting `greeting_name` when the user set one, else
 * the first word of the macOS account's full name (`fork_account_name`),
 * else none (just "Hello"). The pencil button beside it opens a small
 * editor to set or clear the override. Until both sources have answered the
 * heading keeps its space but shows nothing, so the name never flickers from
 * the fallback to the real one.
 */
import { useEffect, useId, useState, type FormEvent } from "react";
import { Pencil } from "../ui/icons";
import { useTranslation } from "react-i18next";
import { commands } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettingsStore } from "../settings/store";
import { Button, FOCUS_RING, Popover } from "../ui";

/** Same limit as `MAX_GREETING_NAME_LEN` in `fork/settings.rs`. */
export const GREETING_NAME_MAX = 64;

/** The macOS account's first name: `undefined` while loading. */
function useAccountName(): string | null | undefined {
  const [name, setName] = useState<string | null | undefined>(undefined);
  useEffect(() => {
    let alive = true;
    commands
      .forkAccountName()
      .then((n) => alive && setName(typeof n === "string" && n ? n : null))
      .catch((e) => {
        console.warn("fork: reading the account name failed", e);
        if (alive) setName(null);
      });
    return () => {
      alive = false;
    };
  }, []);
  return name;
}

export function Greeting() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const status = useForkSettingsStore((s) => s.status);
  const override = useForkSettingsStore(
    (s) => s.settings?.greeting_name ?? null,
  );
  const account = useAccountName();
  const [editing, setEditing] = useState(false);

  const settled =
    account !== undefined && status !== "idle" && status !== "loading";
  const name = override ?? account ?? null;
  const text = name
    ? t("home.greeting.named", { name })
    : t("home.greeting.anonymous");
  const editLabel = t("home.greeting.edit");

  return (
    <div className="group/greet flex min-w-0 items-center gap-1.5">
      <h1
        data-testid="wb-page-title"
        data-name-source={
          !settled ? "pending" : override ? "setting" : account ? "mac" : "none"
        }
        aria-busy={!settled}
        className={`type-title min-w-0 truncate text-(--wb-ink) ${settled ? "" : "invisible"}`}
      >
        {text}
      </h1>
      <Popover
        open={editing}
        onOpenChange={setEditing}
        label={t("home.nameEditor.label")}
        side="bottom"
        align="start"
        testId="wb-greeting-editor"
        trigger={
          <button
            type="button"
            data-testid="wb-greeting-edit"
            aria-label={editLabel}
            title={editLabel}
            disabled={!settled}
            className={`grid size-7 shrink-0 cursor-pointer place-items-center rounded-(--wb-radius-sm) text-(--wb-ink-3) opacity-0 transition-opacity duration-(--wb-dur-fast) group-hover/greet:opacity-100 hover:bg-(--wb-fill-hover) hover:text-(--wb-ink) focus-visible:opacity-100 disabled:invisible data-[state=open]:opacity-100 [@media(hover:none)]:opacity-100 ${FOCUS_RING}`}
          >
            <Pencil size={14} aria-hidden="true" />
          </button>
        }
      >
        {editing && (
          <NameEditor
            current={name ?? ""}
            hasOverride={override !== null}
            accountName={account ?? null}
            onDone={() => setEditing(false)}
          />
        )}
      </Popover>
    </div>
  );
}

function NameEditor({
  current,
  hasOverride,
  accountName,
  onDone,
}: {
  current: string;
  hasOverride: boolean;
  accountName: string | null;
  onDone: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const setSetting = useForkSettingsStore((s) => s.setSetting);
  const [value, setValue] = useState(current);
  const [failed, setFailed] = useState(false);
  const [saving, setSaving] = useState(false);
  const fieldId = useId();
  const hintId = useId();
  const errorId = useId();
  const trimmed = value.trim();

  const save = async (next: string | null) => {
    setSaving(true);
    setFailed(false);
    const result = await setSetting("greeting_name", next);
    setSaving(false);
    if (result.status === "ok") {
      onDone();
    } else {
      setFailed(true);
    }
  };

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (!trimmed || saving) return;
    // The Mac's own name needs no override.
    if (!hasOverride && trimmed === accountName) {
      onDone();
      return;
    }
    void save(trimmed);
  };

  return (
    <form onSubmit={onSubmit} className="flex flex-col gap-3 p-4">
      <label
        htmlFor={fieldId}
        className="text-[13px] font-medium text-(--wb-ink)"
      >
        {t("home.nameEditor.field")}
      </label>
      <input
        id={fieldId}
        data-testid="wb-greeting-input"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        maxLength={GREETING_NAME_MAX}
        autoComplete="given-name"
        spellCheck={false}
        aria-describedby={failed ? `${hintId} ${errorId}` : hintId}
        aria-invalid={failed || undefined}
        className="rounded-(--wb-radius-sm) border border-(--wb-line-strong) bg-(--wb-panel) px-3 py-2 text-[14px] text-(--wb-ink) select-text focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-(--wb-accent)"
      />
      <p id={hintId} className="text-[12.5px] text-(--wb-ink-2)">
        {t("home.nameEditor.hint")}
      </p>
      {failed && (
        <p
          id={errorId}
          role="alert"
          className="text-[12.5px] font-medium text-(--wb-ink)"
        >
          {t("home.nameEditor.saveFailed")}
        </p>
      )}
      <div className="flex flex-wrap items-center justify-end gap-2">
        {hasOverride && (
          <Button
            variant="ghost"
            data-testid="wb-greeting-reset"
            disabled={saving}
            onClick={() => void save(null)}
            className="me-auto"
          >
            {accountName
              ? t("home.nameEditor.useAccountName")
              : t("home.nameEditor.clear")}
          </Button>
        )}
        <Button
          type="submit"
          variant="primary"
          data-testid="wb-greeting-save"
          disabled={!trimmed || saving}
        >
          {t("home.nameEditor.save")}
        </Button>
      </div>
    </form>
  );
}
