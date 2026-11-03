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
 *
 * Design W5: a white `Sheet` split in two, the serif question (one italic
 * word), the options as a centred stack of chips and the black "Try it now"
 * on the left, the scene on its colourful wallpaper on the right.
 */
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import { Sheet } from "../ui/Dialog";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettingsStore } from "../settings/store";
import { plainText, RichText } from "../tutorials/RichText";
import { Button, Chip } from "../ui";
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
    <Sheet
      open
      onOpenChange={(o) => !o && !busy && closeChooser()}
      label={plainText(t("firstUse.title"))}
      closeLabel={t("firstUse.close")}
      initialFocusRef={selectedRef as React.RefObject<HTMLElement>}
      closeOnBackdrop={false}
      tone="plain"
      className="wb-chooser-sheet"
    >
      <div
        data-testid="wb-first-use"
        className="grid h-full min-h-0 grid-cols-1 gap-8 md:grid-cols-[minmax(0,300px)_minmax(0,1fr)]"
      >
        <div className="flex min-h-0 flex-col items-center gap-6 pt-2">
          <div className="flex flex-col items-center gap-2 text-center">
            <h2 className="type-sheet text-balance text-(--wb-ink)">
              <RichText i18nKey="firstUse.title" />
            </h2>
            <p className="type-small text-balance text-(--wb-ink-2)">
              {t("firstUse.subtitle")}
            </p>
          </div>
          <div
            role="radiogroup"
            aria-label={t("firstUse.choicesLabel")}
            onKeyDown={move}
            className="flex flex-col items-center gap-2.5"
          >
            {FIRST_USE_CHOICES.map((id) => {
              const Icon = CHOICE_ICONS[id];
              const selected = id === choice;
              return (
                <Chip
                  key={id}
                  ref={(el) => {
                    refs.current[id] = el;
                  }}
                  role="radio"
                  aria-pressed={undefined}
                  aria-checked={selected}
                  tabIndex={selected ? 0 : -1}
                  selected={selected}
                  onClick={() => setChoice(id)}
                  leading={<Icon size={16} className="shrink-0" />}
                  testId={`wb-first-use-${id}`}
                >
                  {t(`firstUse.choices.${id}`)}
                </Chip>
              );
            })}
          </div>
          <div className="mt-auto flex flex-col items-center gap-1 pb-1">
            <Button
              variant="primary"
              onClick={() => void start()}
              disabled={busy}
              className="min-w-[132px]"
              data-testid="wb-first-use-try"
            >
              {t("firstUse.tryNow")}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={closeChooser}
              disabled={busy}
              data-testid="wb-first-use-later"
            >
              {t("firstUse.later")}
            </Button>
          </div>
        </div>
        <Scene choice={choice} />
      </div>
    </Sheet>
  );
}
