// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The setup checklist card in the sidebar (S-30, B-44, C-10): "Get
 * started", n of m done, the registered items (`checklistRegistry.tsx`) in
 * order, the next open one expanded with its button. Items tick themselves
 * (see the registry); when the item started from this card ticks, the next
 * open one is shown (chaining): its `reveal` opens its page with a coach
 * mark, or the card points at its row. Chaining never runs an item's `run`
 * (which may change a setting: Open at login turns autostart on), and only
 * happens while the window is on screen, so a tick that happened while it
 * was hidden never pulls the user around.
 *
 * Shown after setup is done (onboarding `completed`) until hidden with ✕
 * (`dismissed_banners` › `hub.checklist`). When everything is done it says
 * so and stays until hidden.
 *
 * Design W5 (REFERENCE-SPEC §1 "Sidebar card"): a white card with a 13 px
 * title, a 4 pt ink progress bar, then 13 px items with 18 pt hollow
 * circles that fill with a tick when done (the label stays readable, no
 * strike-through). The next item gets a quiet "Show me →" link instead of a
 * button block; its one-line explanation describes that link for screen
 * readers, so the card stays as compact as the reference's.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { ArrowRight, Check, X } from "../ui/icons";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { commands } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { openChooser } from "../onboarding/firstUseStore";
import { useForkSettingsStore } from "../settings/store";
import { Button, IconButton, ProgressLine } from "../ui";
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
/** Pause between a tick and the next step being shown. */
export const CHAIN_DELAY_MS = 700;
/** How long the card points at the next row (until a click ends it). */
export const HIGHLIGHT_MS = 8000;

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

  /** The item last started or shown from the card (chaining follows it). */
  const started = useRef<string | null>(null);
  const chainTimer = useRef<number | undefined>(undefined);
  const highlightTimer = useRef<number | undefined>(undefined);
  /** The row chaining points at (an item without `reveal`). */
  const [highlight, setHighlight] = useState<string | null>(null);
  useEffect(
    () => () => {
      window.clearTimeout(chainTimer.current);
      window.clearTimeout(highlightTimer.current);
    },
    [],
  );

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

  /** The user's click: the item's own step. */
  const run = useCallback(
    (item: ChecklistItem) => {
      started.current = item.id;
      setHighlight(null);
      void item.run(context(item));
    },
    [context],
  );

  /** Chaining: show the item (its page, or its row), never run it. */
  const reveal = useCallback(
    (item: ChecklistItem) => {
      if (item.reveal) {
        started.current = item.id;
        const { navigate: go, coach } = context(item);
        void item.reveal({ navigate: go, coach });
        return;
      }
      setHighlight(item.id);
      window.clearTimeout(highlightTimer.current);
      highlightTimer.current = window.setTimeout(
        () => setHighlight(null),
        HIGHLIGHT_MS,
      );
      document
        .querySelector(`[data-testid="wb-checklist-${item.id}"]`)
        ?.scrollIntoView?.({ block: "nearest" });
    },
    [context],
  );

  // Chaining: the started item ticked → the next open one is shown.
  const doneKey = done.join(",");
  useEffect(() => {
    const id = started.current;
    if (!id || !done.includes(id)) return;
    started.current = null;
    const next = nextOpenItem(items, done);
    if (!next || !windowInFront()) return;
    window.clearTimeout(chainTimer.current);
    chainTimer.current = window.setTimeout(() => reveal(next), CHAIN_DELAY_MS);
    // `done` is read through its key.
  }, [doneKey, items, reveal]);

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
      className="flex flex-col gap-3 rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) px-3.5 pt-3 pb-3.5 text-(--wb-ink)"
    >
      <div className="flex items-start gap-2">
        <div className="min-w-0 flex-1 pt-0.5">
          <h2
            id="wb-checklist-title"
            className="text-[13px] leading-[18px] font-semibold"
          >
            {t("checklist.title")}
          </h2>
          <p
            className="text-[12px] leading-4 text-(--wb-ink-3)"
            data-testid="wb-checklist-progress"
            aria-live="polite"
          >
            {t("checklist.progress", progress)}
          </p>
        </div>
        <IconButton
          aria-label={t("checklist.hide")}
          title={t("checklist.hide")}
          size="sm"
          round
          onClick={hide}
          data-testid="wb-checklist-hide"
          className="-me-1.5 -mt-0.5"
        >
          <X size={14} />
        </IconButton>
      </div>
      <div aria-hidden="true">
        <ProgressLine
          value={progress.total ? progress.done / progress.total : 0}
          label={t("checklist.progress", progress)}
          tone="ink"
          thickness={4}
        />
      </div>
      {next ? (
        <ol className="flex flex-col" aria-label={t("checklist.label")}>
          {items.map((item) => {
            const isDone = done.includes(item.id);
            const isNext = item.id === next.id;
            const bodyId = `wb-checklist-body-${item.id}`;
            return (
              <li
                key={item.id}
                data-testid={`wb-checklist-${item.id}`}
                data-done={isDone || undefined}
                data-next={isNext || undefined}
                data-highlight={(isNext && highlight === item.id) || undefined}
                className="wb-checklist-row rounded-(--wb-radius-sm)"
              >
                <div className="flex items-start gap-2.5 py-[7px] text-[13px] leading-[18px]">
                  <span
                    aria-hidden="true"
                    className={`grid size-[18px] shrink-0 place-items-center rounded-full transition-colors duration-(--wb-dur-base) ${
                      isDone
                        ? "bg-(--wb-fill-active) text-(--wb-ink)"
                        : isNext
                          ? "border-[1.5px] border-(--wb-ink)"
                          : "border-[1.5px] border-(--wb-line-strong)"
                    }`}
                  >
                    {isDone && <Check size={12} />}
                  </span>
                  <span
                    className={`min-w-0 flex-1 ${isDone ? "text-(--wb-ink-2)" : isNext ? "font-medium" : ""}`}
                  >
                    {t(item.titleKey)}
                    {isDone && (
                      <span className="sr-only"> {t("checklist.done")}</span>
                    )}
                  </span>
                </div>
                {isNext && (
                  <div className="-mt-1 ps-[22px] pb-1">
                    {item.bodyKey && (
                      <span id={bodyId} className="sr-only">
                        {t(item.bodyKey, { appName })}
                      </span>
                    )}
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-7! gap-1! px-1.5! text-[12.5px]! text-(--wb-ink)!"
                      onClick={() => run(item)}
                      aria-describedby={item.bodyKey ? bodyId : undefined}
                      trailing={
                        <ArrowRight size={12} className="rtl:-scale-x-100" />
                      }
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
