// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * How to add the Wispr Bro keyboard, as three drawn steps (our own drawing
 * of an iOS Settings row per step; no screenshot). "Open Settings" opens
 * this app's page in Settings, which has a Keyboards row:
 *
 *   1  Open Settings › Keyboards     ⌨ Keyboards ›
 *   2  Wispr Bro                     ▪ Wispr Bro ◉
 *   3  Allow Full Access               Allow Full Access ◉
 *
 * Steps 1-2 tick once iOS reports the keyboard as added, step 3 once the
 * keyboard has reported Full Access. Used by onboarding and by Home's
 * setup sheet.
 */
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { BrandGlyph } from "../ui";
import { Check, ChevronRight, Keyboard, LockKeyhole } from "../ui/icons";

function DrawnRow({
  tile,
  label,
  end,
}: {
  tile?: ReactNode;
  label: string;
  end: ReactNode;
}) {
  return (
    <span
      aria-hidden="true"
      className="flex h-10 items-center gap-3 rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) px-3 text-[15px] leading-5 text-(--wb-ink) shadow-(--wb-elev-1)"
    >
      {tile && (
        <span className="grid size-7 shrink-0 place-items-center rounded-[7px] bg-(--wb-secondary) text-(--wb-ink-2)">
          {tile}
        </span>
      )}
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {end}
    </span>
  );
}

const chevron = (
  <ChevronRight
    size={16}
    className="shrink-0 text-(--wb-ink-disabled) rtl:-scale-x-100"
  />
);

/** A drawn iOS switch, on. */
const drawnSwitch = (
  <span className="flex h-[26px] w-[44px] shrink-0 items-center justify-end rounded-full bg-(--wb-success) p-[2px]">
    <span className="size-[22px] rounded-full bg-(--wb-surface) shadow-(--wb-elev-1)" />
  </span>
);

export function KeyboardSteps({
  keyboardEnabled,
  fullAccess,
}: {
  keyboardEnabled: boolean;
  fullAccess: boolean;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const appName = t("rust.appName");
  const steps: { text: string; done: boolean; row: ReactNode }[] = [
    {
      text: t("ios.keyboard.steps.open"),
      done: keyboardEnabled,
      row: (
        <DrawnRow
          tile={<Keyboard size={16} />}
          label={t("ios.keyboard.drawn.keyboards")}
          end={chevron}
        />
      ),
    },
    {
      text: t("ios.keyboard.steps.enable", { appName }),
      done: keyboardEnabled,
      row: (
        <DrawnRow
          tile={<BrandGlyph size={16} />}
          label={appName}
          end={drawnSwitch}
        />
      ),
    },
    {
      text: t("ios.keyboard.steps.fullAccess"),
      done: keyboardEnabled && fullAccess,
      row: (
        <DrawnRow
          label={t("ios.keyboard.drawn.fullAccess")}
          end={drawnSwitch}
        />
      ),
    },
  ];
  return (
    <ol className="flex flex-col gap-3" data-testid="wb-ios-keyboard-steps">
      {steps.map((step, index) => (
        <li
          key={step.text}
          data-done={step.done}
          className="grid grid-cols-[28px_minmax(0,1fr)] gap-x-3 gap-y-2"
        >
          <span
            aria-hidden="true"
            className={`grid size-7 place-items-center rounded-full text-[14px] font-semibold tabular-nums ${
              step.done
                ? "bg-(--wb-success) text-(--wb-surface)"
                : "bg-(--wb-secondary) text-(--wb-ink)"
            }`}
          >
            {step.done ? <Check size={16} /> : index + 1}
          </span>
          <span className="self-center text-[16px] leading-[22px] text-(--wb-ink)">
            {step.text}
          </span>
          <span className="col-start-2">{step.row}</span>
        </li>
      ))}
      {!keyboardEnabled && (
        <li className="ps-10 text-[13px] leading-[18px] text-(--wb-ink-3)">
          {t("ios.keyboard.elsewhere")}
        </li>
      )}
    </ol>
  );
}

/** Why Full Access is asked for, in two honest lines. */
export function FullAccessNote({ compact = false }: { compact?: boolean }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  if (compact) {
    return (
      <p
        className="flex items-start gap-2 text-[13px] leading-[18px] text-(--wb-ink-2)"
        data-testid="wb-ios-full-access-note"
      >
        <LockKeyhole
          size={16}
          aria-hidden="true"
          className="mt-px shrink-0 text-(--wb-ink-3)"
        />
        <span>
          <strong className="font-semibold text-(--wb-ink)">
            {t("ios.keyboard.privacyTitle")}
          </strong>{" "}
          {t("ios.keyboard.privacy")}
        </span>
      </p>
    );
  }
  return (
    <div
      className="rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface-tint) p-4"
      data-testid="wb-ios-full-access-note"
    >
      <h2 className="type-card-title m-0 text-(--wb-ink)">
        {t("ios.keyboard.privacyTitle")}
      </h2>
      <p className="mt-1 mb-0 text-[15px] leading-[21px] text-(--wb-ink-2)">
        {t("ios.keyboard.privacy")}
      </p>
    </div>
  );
}
