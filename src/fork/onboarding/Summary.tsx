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
import { Check } from "../ui/icons";
import { useTranslation } from "react-i18next";
import { useSettings } from "@/hooks/useSettings";
import { languageList } from "@/lib/constants/languages";
import { commands } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { ForkToggle } from "../settings/Controls";
import { useForkSettings } from "../settings/useForkSettings";
import { Button } from "../ui";

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
  const rows: { id: string; label: string; value: string }[] = [
    {
      id: "languages",
      label: t("onboardingFlow.summary.languages"),
      value: spoken.length
        ? languageList(spoken, i18n.language)
        : t("onboardingFlow.summary.notSet"),
    },
    {
      id: "shortcut",
      label: t("onboardingFlow.summary.shortcut"),
      value:
        trigger === "fn"
          ? t("onboardingFlow.summary.shortcutFn")
          : trigger === "ctrl+option"
            ? t("onboardingFlow.summary.shortcutFallback")
            : t("onboardingFlow.summary.notSet"),
    },
    {
      id: "microphone",
      label: t("onboardingFlow.summary.microphone"),
      value: mic || t("onboardingFlow.summary.micDefault"),
    },
    {
      id: "typing",
      label: t("onboardingFlow.summary.typing"),
      value:
        wpm != null
          ? t("onboardingFlow.summary.typingWpm", { count: wpm })
          : t("onboardingFlow.summary.typingSkipped"),
    },
  ];

  return (
    <div className="mx-auto flex w-full max-w-[560px] flex-col items-center gap-5">
      <div className="text-center">
        <h1 className="m-0 font-(--wb-font-display) text-3xl font-semibold text-balance">
          {t("onboardingFlow.summary.title")}
        </h1>
        <p className="mt-2 mb-0 text-sm leading-6 text-pretty text-(--wb-ink-2)">
          {t("onboardingFlow.summary.subtitle")}
        </p>
      </div>
      <dl
        className="m-0 w-full overflow-hidden rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) text-sm"
        data-testid="wb-summary-list"
      >
        {rows.map((row, index) => (
          <div
            key={row.id}
            className={`flex items-start gap-3 px-4 py-3 ${
              index > 0 ? "border-t border-(--wb-line)" : ""
            }`}
            data-testid={`wb-summary-${row.id}`}
          >
            <Check
              size={16}
              aria-hidden="true"
              className="mt-0.5 shrink-0 text-(--wb-accent)"
            />
            <dt className="w-32 shrink-0 font-semibold">{row.label}</dt>
            <dd className="m-0 min-w-0 flex-1 break-words text-(--wb-ink-2)">
              {row.value}
            </dd>
          </div>
        ))}
      </dl>
      <div className="w-full rounded-(--wb-radius-md) border border-(--wb-line) bg-(--wb-surface) px-4 py-2 text-start">
        <ForkToggle
          label={t("onboardingFlow.summary.launchAtLogin")}
          hint={t("onboardingFlow.summary.launchAtLoginHint")}
          checked={launch}
          onChange={setLaunch}
          testId="wb-summary-launch"
        />
      </div>
      <p className="m-0 text-center text-xs leading-5 text-(--wb-ink-3)">
        {t("onboardingFlow.summary.later")}
      </p>
      <Button
        variant="primary"
        className="px-5 py-2.5"
        disabled={busy}
        onClick={() => onFinish(launch)}
        data-testid="wb-summary-finish"
      >
        {t("onboardingFlow.summary.finish")}
      </Button>
    </div>
  );
}
