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
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { Check, ChevronRight, Globe, Pencil, RotateCcw } from "../ui/icons";
import { useTranslation } from "react-i18next";
import { commands as handyCommands } from "@/bindings";
import { commands, events, type HotkeyStatus } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button, FOCUS_RING } from "../ui";
import { CentredLayout, TestCard } from "./Layout";
import { HotkeysDialog } from "./HotkeysDialog";
import {
  AppleDictationWarning,
  useRecheckOnFocus,
} from "../settings/hotkeys/AppleDictation";

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
const FALLBACK_KEYS = ["⌃", "⌥"] as const;
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
  apple_dictation: "none",
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

  // Apple's Dictation shortcut may change in System Settings meanwhile.
  const recheck = useCallback(() => void startTest(), [startTest]);
  useRecheckOnFocus(recheck);

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

  const keyCap = (label: ReactNode, corner?: ReactNode) => (
    <span
      className={`relative grid size-14 place-items-center rounded-(--wb-radius-md) border text-[15px] font-medium shadow-[inset_0_-2px_0_var(--wb-line)] motion-safe:transition-[background-color,border-color,color,transform] motion-safe:duration-(--wb-dur-fast) ${
        pressed
          ? "scale-95 border-(--wb-accent) bg-(--wb-accent) text-(--wb-on-ink) shadow-none"
          : "border-(--wb-line-2) bg-(--wb-surface) text-(--wb-ink)"
      }`}
    >
      {corner ? (
        <>
          <span className="absolute end-2 top-1.5 text-[12px] leading-none">
            {corner}
          </span>
          <span className="absolute start-2 bottom-2 leading-none">
            {label}
          </span>
        </>
      ) : (
        label
      )}
    </span>
  );

  return (
    <CentredLayout
      title={t("onboardingFlow.shortcut.title")}
      subtitle={t(
        unbound
          ? "onboardingFlow.shortcut.unboundInstruction"
          : fallback
            ? "onboardingFlow.shortcut.fallbackInstruction"
            : "onboardingFlow.shortcut.instruction",
      )}
    >
      <TestCard
        title={
          <h2 className="type-heading m-0" id="wb-shortcut-question">
            {t("onboardingFlow.shortcut.question")}
          </h2>
        }
        footer={
          <>
            <Button
              variant="outline"
              size="sm"
              onClick={() => setEditorOpen(true)}
              data-testid="wb-edit-shortcut"
              leading={<Pencil size={14} aria-hidden="true" />}
            >
              {t("onboardingFlow.shortcut.edit")}
            </Button>
            <span className="flex-1" />
            <Button
              variant="secondary"
              size="sm"
              aria-pressed={answer === "no"}
              aria-describedby="wb-shortcut-question"
              onClick={() => void sayNo()}
              data-testid="wb-shortcut-no"
            >
              {t("onboardingFlow.no")}
            </Button>
            <Button
              variant="primary"
              size="sm"
              aria-describedby="wb-shortcut-question"
              onClick={onContinue}
              data-testid="wb-shortcut-yes"
            >
              {t("onboardingFlow.yes")}
            </Button>
          </>
        }
      >
        <div className="flex flex-col items-center gap-3 rounded-(--wb-radius-sm) bg-(--wb-surface-tint) px-5 pt-5 pb-3">
          <span
            className="flex items-center gap-2"
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
              <>
                {keyCap(FALLBACK_KEYS[0])}
                {keyCap(FALLBACK_KEYS[1])}
              </>
            ) : (
              keyCap(<Globe size={16} aria-hidden="true" />, FN_KEY)
            )}
          </span>
          <p
            className={`m-0 min-h-[18px] text-center text-[13px] leading-[18px] ${
              lit && !pressed
                ? "font-medium text-(--wb-ink)"
                : "text-(--wb-ink-2)"
            }`}
            role="status"
            data-testid="wb-shortcut-state"
          >
            {lit && !pressed && (
              <Check
                size={14}
                aria-hidden="true"
                className="me-1 inline-block align-[-2px] text-(--wb-success)"
              />
            )}
            {pressed
              ? t("onboardingFlow.shortcut.pressed")
              : lit
                ? t("onboardingFlow.shortcut.worked")
                : t("onboardingFlow.shortcut.notPressed")}
          </p>
        </div>
      </TestCard>

      {status?.trigger === "fn" && status.apple_dictation === "active" && (
        <div className="mt-4 w-full max-w-[460px] rounded-(--wb-radius-md) bg-(--wb-surface) p-4 shadow-(--wb-elev-2)">
          <AppleDictationWarning preview={preview} />
        </div>
      )}
      {status?.trigger === "fn" && status.apple_dictation === "turned_off" && (
        <p
          role="status"
          className="m-0 mt-4 w-full max-w-[460px] text-center text-[13px] leading-[18px] text-(--wb-ink-2)"
          data-testid="wb-apple-dictation-turned-off"
        >
          {t("hub.appleDictationNote.body")}
        </p>
      )}

      {diagnosis && (
        <section
          aria-labelledby="wb-shortcut-fix-title"
          className="mt-4 flex w-full max-w-[460px] flex-col items-start gap-3 rounded-(--wb-radius-md) bg-(--wb-surface) p-5 text-start shadow-(--wb-elev-2)"
          data-testid={`wb-diagnosis-panel-${diagnosis}`}
        >
          <h2
            id="wb-shortcut-fix-title"
            className="type-card-title m-0"
            tabIndex={-1}
          >
            {t(`onboardingFlow.shortcut.diagnoses.${diagnosis}.title`)}
          </h2>
          <p className="m-0 text-[13px] leading-[18px] text-(--wb-ink-2)">
            {t(`onboardingFlow.shortcut.diagnoses.${diagnosis}.body`)}
          </p>
          {diagnosis === "secure_input" && (
            <p
              className="m-0 text-[13px] font-medium"
              data-testid="wb-secure-input-state"
            >
              {t(
                secureInput
                  ? "onboardingFlow.shortcut.diagnoses.secure_input.active"
                  : "onboardingFlow.shortcut.diagnoses.secure_input.inactive",
              )}
            </p>
          )}
          <div className="flex flex-wrap gap-2">
            {diagnosis === "tap" && (
              <Button
                variant="primary"
                size="sm"
                onClick={() => openPane("accessibility")}
                data-testid="wb-diagnosis-tap-open"
              >
                {t("onboardingFlow.shortcut.diagnoses.tap.action")}
              </Button>
            )}
            {diagnosis === "no_fn" && (
              <Button
                variant="primary"
                size="sm"
                disabled={busy || fallback}
                onClick={() => void useFallback()}
                data-testid="wb-diagnosis-no_fn-action"
                leading={fallback && <Check size={14} aria-hidden="true" />}
              >
                {t(
                  fallback
                    ? "onboardingFlow.shortcut.diagnoses.no_fn.changed"
                    : "onboardingFlow.shortcut.diagnoses.no_fn.action",
                )}
              </Button>
            )}
            {diagnosis === "globe" && (
              <>
                <Button
                  variant="primary"
                  size="sm"
                  disabled={busy || globeFixed}
                  onClick={() => void fixGlobe()}
                  data-testid="wb-diagnosis-globe-action"
                  leading={globeFixed && <Check size={14} aria-hidden="true" />}
                >
                  {t(
                    globeFixed
                      ? "onboardingFlow.shortcut.diagnoses.globe.changed"
                      : "onboardingFlow.shortcut.diagnoses.globe.consent",
                  )}
                </Button>
                <Button
                  size="sm"
                  onClick={() => openPane("keyboard")}
                  data-testid="wb-diagnosis-globe-open"
                >
                  {t("onboardingFlow.shortcut.diagnoses.globe.open")}
                </Button>
              </>
            )}
            <Button
              variant="outline"
              size="sm"
              onClick={() => void tryAgain()}
              data-testid="wb-shortcut-try-again"
              leading={<RotateCcw size={14} aria-hidden="true" />}
            >
              {t("onboardingFlow.shortcut.tryAgain")}
            </Button>
          </div>
          {error && (
            <p className="m-0 text-[13px] font-medium" role="alert">
              {t("onboardingFlow.shortcut.changeFailed")}
            </p>
          )}
          <div className="w-full border-t border-(--wb-line) pt-3">
            <p className="type-micro m-0 mb-1.5 text-(--wb-ink-3)">
              {t("onboardingFlow.shortcut.otherCauses")}
            </p>
            <ul className="m-0 flex list-none flex-col p-0">
              {others.map((choice) => (
                <li key={choice}>
                  <button
                    type="button"
                    className={`-mx-2 inline-flex cursor-pointer items-center gap-1.5 rounded-(--wb-radius-sm) px-2 py-1 text-start text-[13px] leading-[18px] text-(--wb-ink-2) transition-colors duration-(--wb-dur-fast) hover:bg-(--wb-fill-hover) hover:text-(--wb-ink) ${FOCUS_RING}`}
                    onClick={() => setDiagnosis(choice)}
                    data-testid={`wb-diagnosis-${choice}`}
                  >
                    <ChevronRight
                      size={14}
                      aria-hidden="true"
                      className="shrink-0 rtl:-scale-x-100"
                    />
                    {t(`onboardingFlow.shortcut.diagnoses.${choice}.choice`)}
                  </button>
                </li>
              ))}
            </ul>
          </div>
        </section>
      )}

      <HotkeysDialog
        open={editorOpen}
        onClose={() => {
          setEditorOpen(false);
          void startTest();
        }}
      />
    </CentredLayout>
  );
}
