// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * "What do you want to try first?" (S-30, B-31): the Hub opens with this
 * once setup is done. Seven ways to start, each with its own animated
 * scene; "Try it now" saves the choice and dims the screen around the Flow
 * Bar with a matching hint (`fork/pill/spotlight.rs`), "Maybe later"
 * closes it for good (the checklist's "Show me" brings it back).
 *
 * The options are a radio group (arrow keys move and select, Home/End,
 * one tab stop), the scene follows the selection. It opens on the saved
 * choice, or the first option.
 */
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import { Dialog } from "../ui/Dialog";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettingsStore } from "../settings/store";
import { Button, FOCUS_RING } from "../ui";
import {
  CHOICE_ICONS,
  CHOOSER_DISMISSED_ID,
  FIRST_USE_CHOICES,
  isFirstUseChoice,
  type FirstUseChoice,
} from "./firstUse";
import { closeChooser, tryNow, useFirstUseStore } from "./firstUseStore";
import { Scene } from "./scenes/Scene";

/** Whether the chooser shows now (first run after setup, or asked for). */
export function useChooserOpen(): boolean {
  const requested = useFirstUseStore((s) => s.chooserRequested);
  const closed = useFirstUseStore((s) => s.chooserClosed);
  const settings = useForkSettingsStore((s) => s.settings);
  if (requested) return true;
  if (closed || !settings) return false;
  return (
    settings.onboarding.status === "completed" &&
    settings.first_use_choice == null &&
    !settings.dismissed_banners.includes(CHOOSER_DISMISSED_ID)
  );
}

export function FirstUseChooser() {
  const open = useChooserOpen();
  if (!open) return null;
  return <ChooserDialog />;
}

function ChooserDialog() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const saved = useForkSettingsStore((s) => s.settings?.first_use_choice);
  const [choice, setChoice] = useState<FirstUseChoice>(
    isFirstUseChoice(saved) ? saved : FIRST_USE_CHOICES[0],
  );
  const [busy, setBusy] = useState(false);
  const refs = useRef<Record<string, HTMLButtonElement | null>>({});
  const selectedRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    selectedRef.current = refs.current[choice] ?? null;
  }, [choice]);

  const move = (event: KeyboardEvent<HTMLDivElement>) => {
    const i = FIRST_USE_CHOICES.indexOf(choice);
    const last = FIRST_USE_CHOICES.length - 1;
    const next =
      event.key === "ArrowDown" || event.key === "ArrowRight"
        ? (i + 1) % FIRST_USE_CHOICES.length
        : event.key === "ArrowUp" || event.key === "ArrowLeft"
          ? (i + last) % FIRST_USE_CHOICES.length
          : event.key === "Home"
            ? 0
            : event.key === "End"
              ? last
              : null;
    if (next == null) return;
    event.preventDefault();
    const id = FIRST_USE_CHOICES[next];
    setChoice(id);
    refs.current[id]?.focus();
  };

  const start = async () => {
    setBusy(true);
    try {
      await tryNow(choice);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && !busy && closeChooser()}
      title={
        <span className="font-(family-name:--wb-font-display) text-[22px] font-medium">
          {t("firstUse.title")}
        </span>
      }
      description={t("firstUse.subtitle")}
      closeLabel={t("firstUse.close")}
      initialFocusRef={selectedRef as React.RefObject<HTMLElement>}
      closeOnBackdrop={false}
      className="max-w-[820px]!"
      contentFades={false}
      footer={
        <div className="flex justify-end gap-2">
          <Button
            variant="ghost"
            onClick={closeChooser}
            disabled={busy}
            data-testid="wb-first-use-later"
          >
            {t("firstUse.later")}
          </Button>
          <Button
            variant="primary"
            onClick={() => void start()}
            disabled={busy}
            data-testid="wb-first-use-try"
          >
            {t("firstUse.tryNow")}
          </Button>
        </div>
      }
    >
      <div
        data-testid="wb-first-use"
        className="grid grid-cols-1 gap-5 sm:grid-cols-[minmax(0,240px)_minmax(0,1fr)]"
      >
        <div
          role="radiogroup"
          aria-label={t("firstUse.choicesLabel")}
          onKeyDown={move}
          className="flex flex-col gap-1.5"
        >
          {FIRST_USE_CHOICES.map((id) => {
            const Icon = CHOICE_ICONS[id];
            const selected = id === choice;
            return (
              <button
                key={id}
                ref={(el) => {
                  refs.current[id] = el;
                }}
                type="button"
                role="radio"
                aria-checked={selected}
                tabIndex={selected ? 0 : -1}
                data-testid={`wb-first-use-${id}`}
                onClick={() => setChoice(id)}
                className={`flex cursor-pointer items-center gap-2.5 rounded-(--wb-radius-control) border px-3 py-2 text-start text-[13.5px] font-medium transition-colors ${FOCUS_RING} ${
                  selected
                    ? "border-(--wb-accent) bg-(--wb-selected) text-(--wb-ink)"
                    : "border-(--wb-line) bg-(--wb-card) text-(--wb-ink-2) hover:bg-(--wb-hover) hover:text-(--wb-ink)"
                }`}
              >
                <Icon size={16} aria-hidden="true" className="shrink-0" />
                <span>{t(`firstUse.choices.${id}`)}</span>
              </button>
            );
          })}
        </div>
        <Scene choice={choice} />
      </div>
    </Dialog>
  );
}
