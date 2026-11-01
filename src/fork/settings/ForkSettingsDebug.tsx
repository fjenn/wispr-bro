// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Fork settings diagnostics in Settings › Advanced (F-07, S-14). Shows the
 * effective fork settings and two live controls, so the store and
 * `useForkSettings()` remain directly inspectable.
 */
import { useId } from "react";
import { useTranslation } from "react-i18next";
import type { CleanupLevel } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettings } from "./useForkSettings";

const LEVELS: CleanupLevel[] = ["none", "light", "medium"];

export function ForkSettingsDebug() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, status, error, pending, setSetting } = useForkSettings();
  const contextId = useId();
  const contextHintId = useId();
  const levelId = useId();

  if (!settings) {
    return (
      <p className="text-xs text-ink-3" data-testid="fork-settings-loading">
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
    <div className="flex flex-col gap-2 text-sm">
      <label htmlFor={contextId} className="flex items-center gap-2">
        <input
          id={contextId}
          type="checkbox"
          aria-describedby={contextHintId}
          data-testid="fork-settings-context-awareness"
          checked={settings.context_awareness}
          disabled={!!pending.context_awareness}
          onChange={(e) =>
            void setSetting("context_awareness", e.currentTarget.checked)
          }
        />
        {t("settings.debug.contextAwareness")}
      </label>
      <p id={contextHintId} className="-mt-1 ps-6 text-xs text-ink-3">
        {t("settings.debug.contextAwarenessHint")}
      </p>
      <label htmlFor={levelId} className="flex items-center gap-2">
        {t("settings.debug.cleanupLevel")}
        <select
          id={levelId}
          data-testid="fork-settings-cleanup-level"
          value={settings.cleanup_level}
          disabled={!!pending.cleanup_level}
          onChange={(e) =>
            void setSetting(
              "cleanup_level",
              e.currentTarget.value as CleanupLevel,
            )
          }
          className="rounded border border-line-strong bg-wash px-1 py-0.5"
        >
          {LEVELS.map((level) => (
            <option key={level} value={level}>
              {t(`settings.debug.levels.${level}`)}
            </option>
          ))}
        </select>
      </label>
      {error && (
        <p role="alert" className="text-xs text-red-500">
          {t("settings.debug.setError", {
            message: "message" in error ? error.message : error.kind,
          })}
        </p>
      )}
      <details>
        <summary className="text-xs cursor-pointer">
          {t("settings.debug.all")}
        </summary>
        <pre
          data-testid="fork-settings-json"
          className="text-xs whitespace-pre-wrap break-all font-mono select-text"
        >
          {JSON.stringify(settings, null, 2)}
        </pre>
      </details>
    </div>
  );
}
