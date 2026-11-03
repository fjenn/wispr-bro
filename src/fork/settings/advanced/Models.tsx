// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Settings › Advanced › Speech models (S-07b). The only place a model is
 * named (B-207): which model the silent policy chose for this Mac and the
 * user's languages, what it is doing in the background, "Choose the model
 * automatically", and, folded below, Handy's own Models page (wrapped, not
 * edited) for people who want to pick. Picking there pins the model.
 * Retired and licence-restricted models are hidden from Handy's list.
 *
 * S-14 mounts this component in the Settings modal's Advanced section.
 */
import { useCallback, useEffect, useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { ModelsSettings } from "@/components/settings/models";
import { useModelStore } from "@/stores/modelStore";
import {
  commands,
  events,
  type Chip,
  type ForkModelPolicy,
} from "../../bindings";
import { FORK_NAMESPACE } from "../../i18n";
import { FOCUS_RING, Switch } from "../../ui";

/** i18n key and values for a chip (`speechModels.chip.*`). */
export function chipLabel(chip: Chip): {
  key: string;
  values: Record<string, unknown>;
} {
  switch (chip.kind) {
    case "apple_silicon": {
      const suffix = chip.tier === "base" ? "" : `_${chip.tier}`;
      return {
        key: `speechModels.chip.apple${suffix}`,
        values: { generation: chip.generation },
      };
    }
    case "intel":
      return { key: "speechModels.chip.intel", values: {} };
    default:
      return { key: "speechModels.chip.unknown", values: {} };
  }
}

/** Keeps models the fork hides out of Handy's model store. */
function useHiddenModels(enabled: boolean) {
  useEffect(() => {
    if (!enabled) return;
    let hidden = new Set<string>();
    const prune = () => {
      const s = useModelStore.getState();
      const kept = s.models.filter((m) => !hidden.has(m.id));
      if (kept.length !== s.models.length) s.setModels(kept);
    };
    const off = useModelStore.subscribe((s) => s.models, prune);
    void commands
      .forkModelsHidden()
      .then((ids) => {
        hidden = new Set(ids);
        prune();
      })
      .catch((e) => console.warn("fork: hidden models failed", e));
    if (!useModelStore.getState().initialized) {
      void useModelStore.getState().initialize();
    }
    return off;
  }, [enabled]);
}

export function SpeechModels() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [status, setStatus] = useState<ForkModelPolicy | null>(null);
  const [showList, setShowList] = useState(false);
  const [busy, setBusy] = useState(false);
  const toggleId = useId();
  const listId = useId();
  const appName = t("rust.appName");

  useEffect(() => {
    let disposed = false;
    const unlisten = events.forkModelPolicy.listen((e) => {
      if (!disposed) setStatus(e.payload);
    });
    void commands
      .forkModelPolicyStatus()
      .then((s) => {
        if (!disposed) setStatus((prev) => prev ?? s);
      })
      .catch((e) => console.warn("fork: model policy status failed", e));
    return () => {
      disposed = true;
      void unlisten.then((f) => f());
    };
  }, []);

  useHiddenModels(showList);

  const setAutomatic = useCallback(async (on: boolean) => {
    setBusy(true);
    try {
      setStatus(await commands.forkModelPolicySetAutomatic(on));
    } catch (e) {
      console.warn("fork: could not change automatic", e);
    } finally {
      setBusy(false);
    }
  }, []);

  if (!status) {
    return (
      <p className="text-[13px] text-ink-2" data-testid="fork-models-loading">
        {t("speechModels.loading")}
      </p>
    );
  }

  const downloading = status.phase === "downloading";
  const phaseText =
    downloading && status.percent == null
      ? t("speechModels.phase.downloadingNoPercent")
      : t(`speechModels.phase.${status.phase}`, {
          percent: status.percent ?? 0,
        });
  const chip = status.machine ? chipLabel(status.machine.chip) : null;
  const languages = status.languages
    .map((code) => {
      try {
        return (
          new Intl.DisplayNames([navigator.language], { type: "language" }).of(
            code,
          ) ?? code
        );
      } catch {
        return code;
      }
    })
    .join(", ");

  return (
    <div className="flex flex-col gap-3 text-[14px]" data-testid="fork-models">
      <p className="text-ink-2">{t("speechModels.intro", { appName })}</p>

      <div className="flex flex-col gap-1">
        <p
          className="text-[15px] font-medium text-ink"
          data-testid="fork-models-current"
        >
          {status.current_model
            ? t("speechModels.current", { name: status.current_model })
            : t("speechModels.none")}
        </p>
        <p
          role="status"
          aria-live="polite"
          data-testid="fork-models-phase"
          data-phase={status.phase}
          className="text-ink-2"
        >
          {status.automatic ? phaseText : t("speechModels.pinned", { appName })}
        </p>
        {status.automatic && status.next_model && (
          <p className="text-ink-2" data-testid="fork-models-next">
            {t("speechModels.next", { name: status.next_model })}
          </p>
        )}
        {status.automatic && downloading && status.percent != null && (
          <progress
            className="h-1 w-full accent-(--wb-accent)"
            max={100}
            value={status.percent}
            aria-label={t("speechModels.progressLabel")}
            aria-valuetext={phaseText}
          />
        )}
        {status.automatic && status.slower && (
          <p className="text-ink-2" data-testid="fork-models-slower">
            {t("speechModels.slower")}
          </p>
        )}
      </div>

      {(languages || chip) && (
        <ul className="flex flex-col gap-0.5 text-[13px] text-ink-2">
          {languages && <li>{t("speechModels.languages", { languages })}</li>}
          {chip && status.machine && (
            <li data-testid="fork-models-machine">
              {t("speechModels.machine", {
                chip: t(chip.key, chip.values),
                ram: status.machine.ram_gb,
              })}
              {status.machine.free_disk_gb != null &&
                ` · ${t("speechModels.freeDisk", { gb: status.machine.free_disk_gb })}`}
            </li>
          )}
        </ul>
      )}

      <div className="flex items-center justify-between gap-4 border-t border-(--wb-line) pt-3">
        <div className="flex min-w-0 flex-col gap-0.5">
          <label htmlFor={toggleId} className="text-[15px] font-medium">
            {t("speechModels.automatic")}
          </label>
          <span
            id={`${toggleId}-hint`}
            className="text-[14px] text-(--wb-ink-2)"
          >
            {t("speechModels.automaticHint")}
          </span>
        </div>
        <Switch
          id={toggleId}
          checked={status.automatic}
          disabled={busy}
          ariaDescribedBy={`${toggleId}-hint`}
          testId="fork-models-automatic"
          onChange={(checked) => void setAutomatic(checked)}
        />
      </div>

      <div>
        <button
          type="button"
          aria-expanded={showList}
          aria-controls={listId}
          data-testid="fork-models-list-toggle"
          className={`cursor-pointer rounded text-[14px] font-medium text-(--wb-ink) underline decoration-(--wb-line-strong) underline-offset-[3px] hover:decoration-(--wb-ink) ${FOCUS_RING}`}
          onClick={() => setShowList((v) => !v)}
        >
          {t("speechModels.list")}
        </button>
      </div>
      {showList && (
        <div id={listId} className="flex min-h-[420px] flex-col">
          <ModelsSettings />
        </div>
      )}
    </div>
  );
}
