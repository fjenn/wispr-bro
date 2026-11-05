// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The end of setup (S-29b, B-36): the launch-at-login choice and a summary
 * of the choices made (languages, shortcut, microphone, typing speed), each
 * changeable later in Settings. No speech model is named (B-207). "Launch
 * at login" starts checked (a dictation app is useful only while it runs)
 * and is written once, with Handy's setting, when the user presses Start
 * dictating; the flow then completes and the resting bar appears (DEC-38).
 */
import { useEffect, useState } from "react";
import { Check, Minus } from "../ui/icons";
import { useTranslation } from "react-i18next";
import { useSettings } from "@/hooks/useSettings";
import { languageList } from "@/lib/constants/languages";
import { commands } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { chordLabel } from "../settings/hotkeys/model";
import { useForkSettings } from "../settings/useForkSettings";
import { Button, Switch } from "../ui";
import { ArtSlot, SplitLayout } from "./Layout";

export function Summary({
  preview,
  busy,
  onFinish,
}: {
  preview: boolean;
  busy: boolean;
  onFinish: (launchAtLogin: boolean) => void;
}) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const { settings: handy } = useSettings();
  const { settings: fork } = useForkSettings();
  const [launch, setLaunch] = useState(true);
  const [trigger, setTrigger] = useState<string | null>(preview ? "fn" : null);
  const [mic, setMic] = useState<string | null>(null);

  useEffect(() => {
    if (preview) return;
    let alive = true;
    void commands.forkHotkeyStatus().then((r) => {
      if (alive && r.status === "ok") setTrigger(r.data.trigger);
    });
    void commands.forkMicList().then((r) => {
      if (alive && r.status === "ok") setMic(r.data.in_use);
    });
    return () => {
      alive = false;
    };
  }, [preview]);

  const spoken = handy?.spoken_languages ?? [];
  const wpm = fork?.typing_wpm ?? null;
  // `set: false` rows (nothing chosen, or skipped) get a neutral dash, not
  // the green tick (OT2-E, Mac wave 6: a tick beside "Not set").
  const rows: { id: string; label: string; value: string; set: boolean }[] = [
    {
      id: "languages",
      set: spoken.length > 0,
      label: t("onboardingFlow.summary.languages"),
      value: spoken.length
        ? languageList(spoken, i18n.language)
        : t("onboardingFlow.summary.notSet"),
    },
    {
      id: "shortcut",
      set: Boolean(trigger),
      label: t("onboardingFlow.summary.shortcut"),
      value:
        trigger === "fn"
          ? t("onboardingFlow.summary.shortcutFn")
          : trigger === "ctrl+option"
            ? t("onboardingFlow.summary.shortcutFallback")
            : trigger
              ? chordLabel(trigger)
              : t("onboardingFlow.summary.notSet"),
    },
    {
      id: "microphone",
      set: true,
      label: t("onboardingFlow.summary.microphone"),
      value: mic || t("onboardingFlow.summary.micDefault"),
    },
    {
      id: "typing",
      set: wpm != null,
      label: t("onboardingFlow.summary.typing"),
      value:
        wpm != null
          ? t("onboardingFlow.summary.typingWpm", { count: wpm })
          : t("onboardingFlow.summary.typingSkipped"),
    },
  ];

  return (
    <SplitLayout
      title={t("onboardingFlow.summary.title")}
      subtitle={t("onboardingFlow.summary.subtitle")}
      aside={<ArtSlot name="onboarding-welcome" />}
      action={
        <Button
          variant="primary"
          size="lg"
          block
          disabled={busy}
          onClick={() => onFinish(launch)}
          data-testid="wb-summary-finish"
        >
          {t("onboardingFlow.summary.finish")}
        </Button>
      }
    >
      <dl
        className="m-0 flex flex-col gap-2.5 text-[15px] leading-5"
        data-testid="wb-summary-list"
      >
        {rows.map((row) => (
          <div
            key={row.id}
            className="flex items-start gap-3"
            data-testid={`wb-summary-${row.id}`}
            data-state={row.set ? "set" : "unset"}
          >
            {row.set ? (
              <Check
                size={16}
                aria-hidden="true"
                className="mt-0.5 shrink-0 text-(--wb-success)"
              />
            ) : (
              <Minus
                size={16}
                aria-hidden="true"
                data-testid={`wb-summary-${row.id}-unset`}
                className="mt-0.5 shrink-0 text-(--wb-ink-3)"
              />
            )}
            <dt className="w-[120px] shrink-0 text-(--wb-ink-2)">
              {row.label}
            </dt>
            <dd className="m-0 min-w-0 flex-1 font-medium break-words">
              {row.value}
            </dd>
          </div>
        ))}
      </dl>
      <div className="mt-7 flex items-center gap-4 rounded-(--wb-radius-md) bg-(--wb-surface-tint) px-4 py-3.5">
        <div className="min-w-0 flex-1">
          <label
            htmlFor="wb-summary-launch-input"
            className="block text-[15px] leading-5 font-medium"
          >
            {t("onboardingFlow.summary.launchAtLogin")}
          </label>
          <p
            id="wb-summary-launch-hint"
            className="mt-0.5 mb-0 text-[13px] leading-[18px] text-(--wb-ink-2)"
          >
            {t("onboardingFlow.summary.launchAtLoginHint")}
          </p>
        </div>
        <Switch
          id="wb-summary-launch-input"
          checked={launch}
          onChange={setLaunch}
          ariaDescribedBy="wb-summary-launch-hint"
          testId="wb-summary-launch"
        />
      </div>
    </SplitLayout>
  );
}
