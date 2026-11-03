// SPDX-License-Identifier: GPL-3.0-or-later
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type {
  HotkeyActivation,
  HotkeyBinding,
  HotkeyStatus,
} from "../../bindings";
import { commands } from "../../bindings";
import { FORK_NAMESPACE } from "../../i18n";
import { Button, IconButton } from "../../ui";
import { Plus } from "../../ui/icons";
import { useForkSettings } from "../useForkSettings";
import { BindingRow } from "./BindingRow";
import {
  actionDefinition,
  actionIds,
  canAddBinding,
  defaultHotkeys,
  MAX_BINDINGS,
  nextBindingActivation,
} from "./model";

interface CaptureTarget {
  action: string;
  index: number;
}

export function HotkeysPanel({ onDone }: { onDone: () => void }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, setSetting } = useForkSettings();
  const [status, setStatus] = useState<HotkeyStatus | null>(null);
  const [capture, setCapture] = useState<CaptureTarget | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const result = await commands.forkHotkeyStatus();
      if (result.status === "ok") setStatus(result.data);
    } catch (cause) {
      console.warn("fork: hotkey status failed", cause);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh, settings?.hotkeys]);

  const save = useCallback(
    async (action: string, bindings: HotkeyBinding[]) => {
      setError(null);
      const result = await setSetting("hotkey", { action, bindings });
      if (result.status === "error") {
        setError(t("settingsModal.hotkeys.saveFailed"));
        return false;
      }
      return true;
    },
    [setSetting, t],
  );

  if (!settings) {
    return (
      <p className="text-sm text-(--wb-ink-2)" data-testid="wb-hotkeys-loading">
        {t("settingsModal.hotkeys.loading")}
      </p>
    );
  }

  const ids = actionIds(settings.hotkeys);
  const reset = async () => {
    setBusy(true);
    setError(null);
    const defaults = defaultHotkeys();
    for (const id of ids) {
      const result = await setSetting("hotkey", {
        action: id,
        bindings: defaults[id] ?? [],
      });
      if (result.status === "error") {
        setError(t("settingsModal.hotkeys.resetFailed"));
        break;
      }
    }
    setBusy(false);
  };

  const busyCapture = capture !== null || busy;
  return (
    <div className="flex flex-col gap-6" data-testid="wb-hotkeys-panel">
      <div className="flex flex-col gap-1">
        <p className="type-body text-(--wb-ink-2)">
          {t("settingsModal.hotkeys.description", {
            count: MAX_BINDINGS,
            max: MAX_BINDINGS,
          })}
        </p>
        <p className="type-small text-(--wb-ink-2)">
          {t("settingsModal.hotkeys.secureInputHelp")}
        </p>
      </div>
      <div className="flex flex-col gap-3">
        {ids.map((id) => {
          const definition = actionDefinition(id);
          const bindings = settings.hotkeys[id] ?? [];
          const activations: HotkeyActivation[] =
            id === "hands_free"
              ? ["double_tap", "press"]
              : [definition.activation];
          const adding =
            capture?.action === id && capture.index === bindings.length;
          const name = t(`settingsModal.hotkeys.actions.${id}`, {
            defaultValue: id,
          });
          return (
            <section
              key={id}
              aria-labelledby={`wb-hotkey-title-${id}`}
              className="flex flex-wrap items-start justify-between gap-x-6 gap-y-3 rounded-(--wb-radius-sm) border border-(--wb-line) bg-(--wb-surface-tint) px-5 py-4"
              data-testid={`wb-hotkey-action-${id}`}
            >
              <div className="flex min-w-48 flex-1 flex-col gap-0.5 pt-2">
                <h3
                  id={`wb-hotkey-title-${id}`}
                  className="text-[15px] leading-5 font-medium"
                >
                  {name}
                </h3>
                <p className="text-[13px] leading-[18px] text-(--wb-ink-2)">
                  {t(`settingsModal.hotkeys.help.${id}`, { defaultValue: "" })}
                </p>
                <span className="sr-only">
                  {t("settingsModal.hotkeys.count", {
                    count: bindings.length,
                    max: MAX_BINDINGS,
                  })}
                </span>
              </div>
              <div className="flex items-end gap-2">
                <div className="flex w-[264px] max-w-full flex-col gap-2">
                  {bindings.length === 0 && !adding && (
                    <p className="flex h-10 items-center rounded-(--wb-radius-sm) border border-dashed border-(--wb-line-strong) px-3 text-[14px] text-(--wb-ink-2)">
                      {t("settingsModal.hotkeys.notSet")}
                    </p>
                  )}
                  {bindings.map((binding, index) => (
                    <BindingRow
                      key={`${index}-${binding.chord}-${binding.activation}`}
                      actionId={id}
                      binding={binding}
                      index={index}
                      activations={activations}
                      status={status}
                      capturing={
                        capture?.action === id && capture.index === index
                      }
                      captureBusy={busyCapture}
                      onCapture={() => setCapture({ action: id, index })}
                      onCaptured={(chord) => {
                        setCapture(null);
                        if (!chord) return;
                        const next = bindings.map((current, candidate) =>
                          candidate === index ? { ...current, chord } : current,
                        );
                        void save(id, next);
                      }}
                      onActivation={(activation) => {
                        const next = bindings.map((current, candidate) =>
                          candidate === index
                            ? { ...current, activation }
                            : current,
                        );
                        void save(id, next);
                      }}
                      onDelete={() => {
                        void save(
                          id,
                          bindings.filter(
                            (_, candidate) => candidate !== index,
                          ),
                        );
                      }}
                    />
                  ))}
                  {adding && (
                    <BindingRow
                      actionId={id}
                      binding={{
                        chord: "",
                        activation: nextBindingActivation(id, bindings),
                      }}
                      index={bindings.length}
                      activations={activations}
                      status={status}
                      capturing
                      captureBusy={busy}
                      onCapture={() => undefined}
                      onCaptured={(chord) => {
                        setCapture(null);
                        if (!chord) return;
                        void save(id, [
                          ...bindings,
                          {
                            chord,
                            activation: nextBindingActivation(id, bindings),
                          },
                        ]);
                      }}
                      onActivation={() => undefined}
                    />
                  )}
                </div>
                <IconButton
                  variant="outline"
                  size="lg"
                  aria-label={t("settingsModal.hotkeys.addFor", {
                    action: name,
                  })}
                  title={t("settingsModal.hotkeys.add")}
                  disabled={busyCapture || !canAddBinding(bindings)}
                  onClick={() =>
                    setCapture({ action: id, index: bindings.length })
                  }
                  className="mb-0.5"
                  data-testid={`wb-hotkey-${id}-add`}
                >
                  <Plus size={18} />
                </IconButton>
              </div>
            </section>
          );
        })}
      </div>
      {error && (
        <p role="alert" className="text-[14px] text-(--wb-danger)">
          {error}
        </p>
      )}
      <div className="sticky bottom-0 flex items-center justify-between gap-3 border-t border-(--wb-line) bg-(--wb-surface) py-4">
        <Button
          variant="secondary"
          disabled={busyCapture}
          onClick={() => void reset()}
          data-testid="wb-hotkeys-reset"
        >
          {busy
            ? t("settingsModal.hotkeys.resetting")
            : t("settingsModal.hotkeys.reset")}
        </Button>
        <Button
          variant="primary"
          disabled={busyCapture}
          onClick={onDone}
          data-testid="wb-hotkeys-done"
        >
          {t("settingsModal.hotkeys.done")}
        </Button>
      </div>
    </div>
  );
}
