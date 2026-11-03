// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Fork settings diagnostics in Settings › Advanced (F-07, S-14). Shows the
 * effective fork settings and two live controls, so the store and
 * `useForkSettings()` remain directly inspectable.
 */
import { useTranslation } from "react-i18next";
import type { CleanupLevel } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";
import { SelectRow, ToggleRow } from "./Controls";
import { useForkSettings } from "./useForkSettings";

const LEVELS: CleanupLevel[] = ["none", "light", "medium"];

export function ForkSettingsDebug() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, status, error, pending, setSetting } = useForkSettings();

  const useSafeTransformShortcuts = async () => {
    for (const [action, chord] of [
      ["transform.polish", "ctrl+option+1"],
      ["transform.prompt_engineer", "ctrl+option+2"],
    ] as const) {
      const result = await setSetting("hotkey", {
        action,
        bindings: [{ chord, activation: "press" }],
      });
      if (result.status === "error") break;
    }
  };

  if (!settings) {
    return (
      <p
        className="text-[13px] text-(--wb-ink-2)"
        data-testid="fork-settings-loading"
      >
        {status === "error"
          ? t("settings.debug.loadError", {
              message:
                error && "message" in error ? error.message : error?.kind,
            })
          : t("settings.debug.loading")}
      </p>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      <ToggleRow
        label={t("settings.debug.contextAwareness")}
        hint={t("settings.debug.contextAwarenessHint")}
        checked={settings.context_awareness}
        disabled={!!pending.context_awareness}
        onChange={(checked) => void setSetting("context_awareness", checked)}
        testId="fork-settings-context-awareness"
      />
      <ToggleRow
        label={t("settings.debug.transforms")}
        hint={t("settings.debug.transformsHint")}
        checked={settings.transforms_enabled}
        disabled={!!pending.transforms_enabled}
        onChange={(checked) => void setSetting("transforms_enabled", checked)}
        testId="fork-settings-transforms"
      >
        <div>
          <Button
            variant="outline"
            size="sm"
            title={t("settings.debug.transformsSafeShortcutsHint")}
            disabled={!!pending.hotkey}
            onClick={() => void useSafeTransformShortcuts()}
          >
            {t("settings.debug.transformsSafeShortcuts")}
          </Button>
        </div>
      </ToggleRow>
      <SelectRow
        label={t("settings.debug.cleanupLevel")}
        value={settings.cleanup_level}
        disabled={!!pending.cleanup_level}
        options={LEVELS.map((level) => ({
          value: level,
          label: t(`settings.debug.levels.${level}`),
        }))}
        onChange={(value) =>
          void setSetting("cleanup_level", value as CleanupLevel)
        }
        testId="fork-settings-cleanup-level"
        width="w-40"
      />
      {error && (
        <p role="alert" className="text-[13px] text-(--wb-danger)">
          {t("settings.debug.setError", {
            message: "message" in error ? error.message : error.kind,
          })}
        </p>
      )}
      <details>
        <summary className="cursor-pointer text-[13px] text-(--wb-ink-2)">
          {t("settings.debug.all")}
        </summary>
        <pre
          data-testid="fork-settings-json"
          className="mt-2 font-mono text-xs break-all whitespace-pre-wrap select-text"
        >
          {JSON.stringify(settings, null, 2)}
        </pre>
      </details>
    </div>
  );
}
