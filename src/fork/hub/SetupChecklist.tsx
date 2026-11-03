// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The setup checklist card in the sidebar (S-30, B-44, C-10): "Get
 * started", n of m done, the registered items (`checklistRegistry.tsx`) in
 * order, the next open one expanded with its button. Items tick themselves
 * (see the registry); when the item started from this card ticks, the next
 * open one starts on its own (chaining), but only while the window is on
 * screen, so a tick that happened while it was hidden never pulls the user
 * around.
 *
 * Shown after setup is done (onboarding `completed`) until hidden with ✕
 * (`dismissed_banners` › `hub.checklist`). When everything is done it says
 * so and stays until hidden.
 */
import { useCallback, useEffect, useRef } from "react";
import { Check, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { commands } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { openChooser } from "../onboarding/firstUseStore";
import { useForkSettingsStore } from "../settings/store";
import { Button, FOCUS_RING } from "../ui";
import {
  checklistProgress,
  nextOpenItem,
  useChecklistItems,
  type ChecklistItem,
  type ChecklistRunContext,
} from "./checklistRegistry";
import { registerBuiltInChecklistItems } from "./checklistItems";
import { showCoach } from "./coach";
import { navigate, type HubPageId } from "./nav";

registerBuiltInChecklistItems();

export const CHECKLIST_HIDDEN_ID = "hub.checklist";
/** Pause between a tick and the next step opening by itself. */
export const CHAIN_DELAY_MS = 700;

/** Persists a tick (also used by lessons, S-31). */
export async function markChecklistItem(id: string): Promise<void> {
  const result = await commands.forkChecklistMark(id);
  if (result.status === "error") {
    console.warn("checklist: could not save a tick", result.error);
    return;
  }
  // The backend also emits ForkSettingsChanged; apply the answer at once.
  const onboarding = result.data;
  useForkSettingsStore.setState((s) =>
    s.settings ? { settings: { ...s.settings, onboarding } } : s,
  );
}

/** Runs each item's `isDone` now, on focus and after its events. */
function useAutoTicks(
  items: readonly ChecklistItem[],
  done: readonly string[],
  enabled: boolean,
): void {
  const doneRef = useRef(done);
  doneRef.current = done;

  const check = useCallback(async (item: ChecklistItem) => {
    if (!item.isDone || doneRef.current.includes(item.id)) return;
    try {
      if (await item.isDone()) await markChecklistItem(item.id);
    } catch (e) {
      console.warn(`checklist: ${item.id} check failed`, e);
    }
  }, []);

  useEffect(() => {
    if (!enabled) return;
    const runAll = () => items.forEach((item) => void check(item));
    runAll();
    window.addEventListener("focus", runAll);
    const offs = items.flatMap((item) =>
      (item.refreshOn ?? []).map((name) =>
        listen(name, () => void check(item)),
      ),
    );
    return () => {
      window.removeEventListener("focus", runAll);
      for (const off of offs) {
        void off.then((f) => f()).catch(() => undefined);
      }
    };
  }, [items, enabled, check]);
}

/** The Hub is on screen (a hidden or parked window is not). */
function windowInFront(): boolean {
  return document.visibilityState === "visible";
}

export function SetupChecklist() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const settings = useForkSettingsStore((s) => s.settings);
  const items = useChecklistItems();
  const done = settings?.onboarding.checklist_done ?? [];
  const visible =
    settings != null &&
    settings.onboarding.status === "completed" &&
    !settings.dismissed_banners.includes(CHECKLIST_HIDDEN_ID) &&
    items.length > 0;
  useAutoTicks(items, done, visible);

  /** The item last started from the card (chaining follows it). */
  const started = useRef<string | null>(null);
  const chainTimer = useRef<number | undefined>(undefined);
  useEffect(() => () => window.clearTimeout(chainTimer.current), []);

  const context = useCallback(
    (item: ChecklistItem): ChecklistRunContext => ({
      navigate: (page, section) => navigate(page as HubPageId, section),
      coach: (target, titleKey, bodyKey) =>
        void showCoach(target, {
          title: t(titleKey),
          body: t(bodyKey),
          done: t("checklist.coach.done"),
          close: t("checklist.coach.close"),
        }),
      openChooser,
      markDone: () => markChecklistItem(item.id),
    }),
    [t],
  );

  const run = useCallback(
    (item: ChecklistItem) => {
      started.current = item.id;
      void item.run(context(item));
    },
    [context],
  );

  // Chaining: the started item ticked → the next open one starts.
  const doneKey = done.join(",");
  useEffect(() => {
    const id = started.current;
    if (!id || !done.includes(id)) return;
    started.current = null;
    const next = nextOpenItem(items, done);
    if (!next || !windowInFront()) return;
    window.clearTimeout(chainTimer.current);
    chainTimer.current = window.setTimeout(() => run(next), CHAIN_DELAY_MS);
    // `done` is read through its key.
  }, [doneKey, items, run]);

  if (!visible) return null;

  const progress = checklistProgress(items, done);
  const next = nextOpenItem(items, done);
  const hide = () => {
    const { settings: now, setSetting } = useForkSettingsStore.getState();
    if (!now || now.dismissed_banners.includes(CHECKLIST_HIDDEN_ID)) return;
    void setSetting("dismissed_banners", [
      ...now.dismissed_banners,
      CHECKLIST_HIDDEN_ID,
    ]);
  };
  const appName = t("rust.appName");

  return (
    <section
      aria-labelledby="wb-checklist-title"
      data-testid="wb-checklist"
      className="flex flex-col gap-2.5 rounded-(--wb-radius-card) border border-(--wb-line) bg-(--wb-card) p-3 text-(--wb-ink)"
    >
      <div className="flex items-start gap-2">
        <div className="min-w-0 flex-1">
          <h2
            id="wb-checklist-title"
            className="font-(family-name:--wb-font-display) text-[15px] leading-tight font-semibold"
          >
            {t("checklist.title")}
          </h2>
          <p
            className="mt-0.5 text-[12px] text-(--wb-ink-2)"
            data-testid="wb-checklist-progress"
            aria-live="polite"
          >
            {t("checklist.progress", progress)}
          </p>
        </div>
        <button
          type="button"
          onClick={hide}
          aria-label={t("checklist.hide")}
          title={t("checklist.hide")}
          data-testid="wb-checklist-hide"
          className={`grid size-6 shrink-0 cursor-pointer place-items-center rounded-full text-(--wb-ink-2) hover:bg-(--wb-hover) hover:text-(--wb-ink) ${FOCUS_RING}`}
        >
          <X size={14} aria-hidden="true" />
        </button>
      </div>
      <div
        aria-hidden="true"
        className="h-1 overflow-hidden rounded-full bg-(--wb-line)"
      >
        <div
          className="h-full rounded-full bg-(--wb-accent-fill) transition-[width] duration-300 motion-reduce:transition-none"
          style={{
            width: `${progress.total ? (progress.done / progress.total) * 100 : 0}%`,
          }}
        />
      </div>
      {next ? (
        <ol className="flex flex-col gap-1" aria-label={t("checklist.label")}>
          {items.map((item) => {
            const isDone = done.includes(item.id);
            const isNext = item.id === next.id;
            const Icon = item.icon;
            return (
              <li
                key={item.id}
                data-testid={`wb-checklist-${item.id}`}
                data-done={isDone || undefined}
                className={`rounded-(--wb-radius-control) px-1.5 py-1 ${isNext ? "bg-(--wb-hover)" : ""}`}
              >
                <div className="flex items-center gap-2 text-[12.5px]">
                  <span
                    aria-hidden="true"
                    className={`grid size-[18px] shrink-0 place-items-center rounded-full border ${
                      isDone
                        ? "border-(--wb-accent-fill) bg-(--wb-accent-fill) text-(--wb-on-accent)"
                        : "border-(--wb-line-strong) text-(--wb-ink-2)"
                    }`}
                  >
                    {isDone ? (
                      <Check size={11} strokeWidth={3} />
                    ) : (
                      <Icon size={10} />
                    )}
                  </span>
                  <span
                    className={`min-w-0 flex-1 ${isDone ? "text-(--wb-ink-2) line-through" : "font-medium"}`}
                  >
                    {t(item.titleKey)}
                    {isDone && (
                      <span className="sr-only"> {t("checklist.done")}</span>
                    )}
                  </span>
                </div>
                {isNext && (
                  <div className="mt-1.5 flex flex-col gap-1.5 ps-[26px]">
                    {item.bodyKey && (
                      <p className="text-[12px] leading-4 text-(--wb-ink-2)">
                        {t(item.bodyKey, { appName })}
                      </p>
                    )}
                    <Button
                      variant="primary"
                      className="self-start px-2.5! py-1! text-[12px]!"
                      onClick={() => run(item)}
                      data-testid={`wb-checklist-run-${item.id}`}
                    >
                      {t(item.actionKey)}
                    </Button>
                  </div>
                )}
              </li>
            );
          })}
        </ol>
      ) : (
        <p
          className="text-[12.5px] text-(--wb-ink-2)"
          data-testid="wb-checklist-all-done"
        >
          {t("checklist.allDone")}
        </p>
      )}
    </section>
  );
}

/** For `setSidebarCard` (S-13's slot). */
export function renderSetupChecklist() {
  return <SetupChecklist />;
}
