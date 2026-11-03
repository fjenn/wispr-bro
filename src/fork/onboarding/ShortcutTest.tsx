// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Test your shortcut (S-29a). While this screen is open the backend runs
 * the shortcut test (`fork_onboarding_key_test`): pressing the dictation
 * trigger lights the on-screen key and starts no take. "Edit shortcut"
 * opens the hotkeys panel (S-14). "No" picks the likely cause from the
 * engine's state and offers the rest:
 *
 * - `tap`: the key listener is not running (Accessibility not granted, or
 *   granted after launch): open the pane, then try again (the backend
 *   restarts the listener).
 * - `secure_input`: another app holds Secure Input (Handy's live check).
 * - `no_fn`: the keyboard has no fn/🌐 key: switch to Ctrl+Option.
 * - `globe`: macOS gives the 🌐 key its own action (emoji, input source,
 *   dictation): DEC-44, set it to Do Nothing with consent (restored when fn
 *   stops being the trigger), or open Keyboard settings.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { Check, Keyboard } from "../ui/icons";
import { useTranslation } from "react-i18next";
import { commands as handyCommands } from "@/bindings";
import { commands, events, type HotkeyStatus } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";
import { HotkeysDialog } from "./HotkeysDialog";

export type Diagnosis = "tap" | "secure_input" | "no_fn" | "globe";

export const DIAGNOSES: readonly Diagnosis[] = [
  "tap",
  "secure_input",
  "globe",
  "no_fn",
];

/** Globe settings that fire on every fn press (RS-01). */
const GLOBE_ACTIONS = ["emoji", "input_source", "dictation"];

/** Key caps (symbols, not words: the same in every language). */
const FALLBACK_KEYS = "⌃ ⌥";
const FN_KEY = "fn";

/** The test renews itself before the backend's two-minute expiry. */
const RENEW_MS = 60_000;

/** The most likely cause of "the key did not light" (pure, unit-tested). */
export function likelyDiagnosis(
  status: HotkeyStatus | null,
  secureInput: boolean,
): Diagnosis {
  if (status && !status.running) return "tap";
  if (secureInput) return "secure_input";
  if (status?.trigger === "fn" && GLOBE_ACTIONS.includes(status.globe)) {
    return "globe";
  }
  return "no_fn";
}

/** A preview's engine: running, fn, Globe on Emoji (every branch shows). */
const PREVIEW_STATUS: HotkeyStatus = {
  running: true,
  error: null,
  trigger: "fn",
  globe: "emoji",
  fn_blocked: true,
  issues: [],
  bindings: [],
};

async function secureInputOn(): Promise<boolean> {
  try {
    return (await handyCommands.getSecureInputStatus()).enabled;
  } catch {
    return false;
  }
}

export function ShortcutTest({
  preview,
  onContinue,
}: {
  preview: boolean;
  onContinue: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [status, setStatus] = useState<HotkeyStatus | null>(
    preview ? PREVIEW_STATUS : null,
  );
  const [pressed, setPressed] = useState(false);
  const [lit, setLit] = useState(false);
  const [answer, setAnswer] = useState<"yes" | "no" | null>(null);
  const [diagnosis, setDiagnosis] = useState<Diagnosis | null>(null);
  const [secureInput, setSecureInput] = useState(false);
  const [editorOpen, setEditorOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const alive = useRef(true);

  /** Starts (or renews) the backend test; also restarts a dead listener. */
  const startTest = useCallback(async () => {
    if (preview) return;
    const result = await commands.forkOnboardingKeyTest(true);
    if (alive.current && result.status === "ok") setStatus(result.data);
  }, [preview]);

  useEffect(() => {
    alive.current = true;
    if (preview) {
      return () => {
        alive.current = false;
      };
    }
    void startTest();
    const renew = window.setInterval(() => void startTest(), RENEW_MS);
    const unlisten = events.forkHotkeyEvent.listen(({ payload }) => {
      if (payload.kind !== "key_test") return;
      const down = payload.detail === "down";
      setPressed(down);
      if (down) setLit(true);
    });
    return () => {
      alive.current = false;
      window.clearInterval(renew);
      void unlisten.then((off) => off()).catch(() => undefined);
      void commands.forkOnboardingKeyTest(false);
    };
  }, [preview, startTest]);

  const sayNo = async () => {
    setAnswer("no");
    setError(false);
    const secure = preview ? false : await secureInputOn();
    setSecureInput(secure);
    setDiagnosis(likelyDiagnosis(status, secure));
  };

  const tryAgain = async () => {
    setAnswer(null);
    setDiagnosis(null);
    setError(false);
    setPressed(false);
    setLit(false);
    await startTest();
  };

  const run = async (
    action: () => Promise<
      { status: "ok"; data: HotkeyStatus } | { status: "error" }
    >,
    previewChange: Partial<HotkeyStatus>,
  ) => {
    setBusy(true);
    setError(false);
    if (preview) {
      setStatus((current) => ({
        ...(current ?? PREVIEW_STATUS),
        ...previewChange,
      }));
    } else {
      const result = await action();
      if (result.status === "ok") setStatus(result.data);
      else setError(true);
    }
    setBusy(false);
  };

  const useFallback = () =>
    run(() => commands.forkUseHotkeyProfile(true), {
      trigger: "ctrl+option",
    });
  const fixGlobe = () =>
    run(() => commands.forkOnboardingGlobeDoNothing(), {
      globe: "do_nothing",
      fn_blocked: false,
    });
  const openPane = (pane: "accessibility" | "keyboard") => {
    if (!preview) void commands.forkOnboardingOpenPane(pane);
  };

  const fallback = status?.trigger === "ctrl+option";
  const unbound = !!status && status.trigger === null;
  const globeFixed = status?.globe === "do_nothing";
  const others = DIAGNOSES.filter((d) => d !== diagnosis);

  return (
    <div className="mx-auto flex w-full max-w-[600px] flex-col items-center gap-5">
      <div className="text-center">
        <h1 className="m-0 font-(--wb-font-display) text-3xl font-semibold text-balance">
          {t("onboardingFlow.shortcut.title")}
        </h1>
        <p className="mt-2 mb-0 text-sm leading-6 text-pretty text-(--wb-ink-2)">
          {t(
            unbound
              ? "onboardingFlow.shortcut.unboundInstruction"
              : fallback
                ? "onboardingFlow.shortcut.fallbackInstruction"
                : "onboardingFlow.shortcut.instruction",
          )}
        </p>
      </div>
      <div className="flex w-full flex-col items-center gap-5 rounded-(--wb-radius-card) border border-(--wb-line) bg-(--wb-card) p-5">
        <div
          className={`grid min-h-24 min-w-32 place-items-center rounded-2xl border-2 px-6 py-4 motion-safe:transition-[background-color,border-color,transform] motion-safe:duration-150 ${
            pressed
              ? "scale-95 border-(--wb-accent) bg-(--wb-banner) text-(--wb-accent)"
              : "border-(--wb-line-strong) bg-(--wb-canvas) text-(--wb-ink)"
          }`}
          data-testid="wb-shortcut-key"
          data-pressed={pressed}
          role="img"
          aria-label={t(
            fallback
              ? "onboardingFlow.shortcut.fallbackKeyLabel"
              : "onboardingFlow.shortcut.keyLabel",
          )}
        >
          {fallback ? (
            <span className="text-lg font-semibold" aria-hidden="true">
              {FALLBACK_KEYS}
            </span>
          ) : (
            <span
              className="flex flex-col items-center leading-none"
              aria-hidden="true"
            >
              <span className="text-2xl">🌐</span>
              <span className="mt-2 text-sm font-semibold">{FN_KEY}</span>
            </span>
          )}
        </div>
        <p
          className="m-0 min-h-5 text-center text-xs text-(--wb-ink-2)"
          role="status"
          data-testid="wb-shortcut-state"
        >
          {pressed
            ? t("onboardingFlow.shortcut.pressed")
            : lit
              ? t("onboardingFlow.shortcut.worked")
              : t("onboardingFlow.shortcut.notPressed")}
        </p>
        <fieldset className="m-0 flex flex-col items-center gap-3 border-0 p-0">
          <legend className="mb-1 text-center text-sm font-medium">
            {t("onboardingFlow.shortcut.question")}
          </legend>
          <div className="flex gap-2">
            <Button
              variant={answer === "yes" ? "primary" : "secondary"}
              aria-pressed={answer === "yes"}
              onClick={() => {
                setAnswer("yes");
                setDiagnosis(null);
              }}
              data-testid="wb-shortcut-yes"
            >
              {t("onboardingFlow.yes")}
            </Button>
            <Button
              variant={answer === "no" ? "primary" : "secondary"}
              aria-pressed={answer === "no"}
              onClick={() => void sayNo()}
              data-testid="wb-shortcut-no"
            >
              {t("onboardingFlow.no")}
            </Button>
          </div>
        </fieldset>
        <Button
          variant="ghost"
          onClick={() => setEditorOpen(true)}
          data-testid="wb-edit-shortcut"
        >
          <Keyboard size={16} aria-hidden="true" />
          {t("onboardingFlow.shortcut.edit")}
        </Button>
      </div>

      {diagnosis && (
        <section
          aria-labelledby="wb-shortcut-fix-title"
          className="flex w-full flex-col items-start gap-3 rounded-(--wb-radius-card) border border-(--wb-line) bg-(--wb-card) p-4 text-start"
          data-testid={`wb-diagnosis-panel-${diagnosis}`}
        >
          <h2
            id="wb-shortcut-fix-title"
            className="m-0 text-sm font-semibold"
            tabIndex={-1}
          >
            {t(`onboardingFlow.shortcut.diagnoses.${diagnosis}.title`)}
          </h2>
          <p className="m-0 text-xs leading-5 text-(--wb-ink-2)">
            {t(`onboardingFlow.shortcut.diagnoses.${diagnosis}.body`)}
          </p>
          {diagnosis === "tap" && (
            <Button
              variant="primary"
              onClick={() => openPane("accessibility")}
              data-testid="wb-diagnosis-tap-open"
            >
              {t("onboardingFlow.shortcut.diagnoses.tap.action")}
            </Button>
          )}
          {diagnosis === "secure_input" && (
            <p
              className="m-0 text-xs font-medium"
              data-testid="wb-secure-input-state"
            >
              {t(
                secureInput
                  ? "onboardingFlow.shortcut.diagnoses.secure_input.active"
                  : "onboardingFlow.shortcut.diagnoses.secure_input.inactive",
              )}
            </p>
          )}
          {diagnosis === "no_fn" && (
            <Button
              variant="primary"
              disabled={busy || fallback}
              onClick={() => void useFallback()}
              data-testid="wb-diagnosis-no_fn-action"
            >
              {fallback && <Check size={16} aria-hidden="true" />}
              {t(
                fallback
                  ? "onboardingFlow.shortcut.diagnoses.no_fn.changed"
                  : "onboardingFlow.shortcut.diagnoses.no_fn.action",
              )}
            </Button>
          )}
          {diagnosis === "globe" && (
            <div className="flex flex-wrap gap-2">
              <Button
                variant="primary"
                disabled={busy || globeFixed}
                onClick={() => void fixGlobe()}
                data-testid="wb-diagnosis-globe-action"
              >
                {globeFixed && <Check size={16} aria-hidden="true" />}
                {t(
                  globeFixed
                    ? "onboardingFlow.shortcut.diagnoses.globe.changed"
                    : "onboardingFlow.shortcut.diagnoses.globe.consent",
                )}
              </Button>
              <Button
                onClick={() => openPane("keyboard")}
                data-testid="wb-diagnosis-globe-open"
              >
                {t("onboardingFlow.shortcut.diagnoses.globe.open")}
              </Button>
            </div>
          )}
          {error && (
            <p className="m-0 text-xs font-medium" role="alert">
              {t("onboardingFlow.shortcut.changeFailed")}
            </p>
          )}
          <Button
            variant="secondary"
            onClick={() => void tryAgain()}
            data-testid="wb-shortcut-try-again"
          >
            {t("onboardingFlow.shortcut.tryAgain")}
          </Button>
          <div className="w-full border-t border-(--wb-line) pt-3">
            <p className="m-0 mb-2 text-xs text-(--wb-ink-2)">
              {t("onboardingFlow.shortcut.otherCauses")}
            </p>
            <div className="flex flex-wrap gap-2">
              {others.map((choice) => (
                <Button
                  key={choice}
                  variant="ghost"
                  className="h-auto px-2.5 py-1.5 text-start text-xs whitespace-normal underline decoration-(--wb-line-strong) underline-offset-4"
                  onClick={() => setDiagnosis(choice)}
                  data-testid={`wb-diagnosis-${choice}`}
                >
                  {t(`onboardingFlow.shortcut.diagnoses.${choice}.choice`)}
                </Button>
              ))}
            </div>
          </div>
        </section>
      )}

      <Button
        variant="primary"
        disabled={answer !== "yes" && !lit}
        onClick={onContinue}
        data-testid="wb-shortcut-finish"
      >
        {t("onboardingFlow.continue")}
      </Button>

      {editorOpen && (
        <HotkeysDialog
          onClose={() => {
            setEditorOpen(false);
            void startTest();
          }}
        />
      )}
    </div>
  );
}
