// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The one-time pairing, both sides of it:
 *
 *   Mac                               iPhone
 *   ┌ Pair with iPhone ────────┐      ┌ Enter the code ──────────┐
 *   │       4 8 2  9 1 3       │      │  shown on Finns MacBook  │
 *   │ Type this code on your   │      │  [ _ _ _ _ _ _ ]         │
 *   │ iPhone.                  │      │           Cancel  Pair   │
 *   └──────────────────────────┘      └──────────────────────────┘
 *
 * The Mac's copy is mounted once in the Hub, so the code shows whatever
 * page is up; the iPhone's on its Sync page. A failure says why here too.
 */
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { Button, Dialog, FOCUS_RING } from "../ui";
import { Loader2, TriangleAlert } from "../ui/icons";
import { useAutoSync, type AutoSync } from "./useAutoSync";

export function PairingDialog({ auto }: { auto: AutoSync }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const pairing = auto.status?.pairing ?? null;
  const [code, setCode] = useState("");
  const input = useRef<HTMLInputElement>(null);
  const stage = pairing?.stage ?? null;
  // With Settings open (a native modal <dialog>, everything else inert)
  // the dialog must live inside it; looked up each time one opens.
  const [container, setContainer] = useState<HTMLElement | null>(null);
  const open = pairing !== null;
  useLayoutEffect(() => {
    if (open) {
      setContainer(document.querySelector<HTMLElement>("dialog[open]"));
    }
  }, [open]);
  useEffect(() => {
    if (stage === "enter_code") setCode("");
  }, [stage]);

  const device = pairing?.device_name || t("sync.devices.mac");
  const waiting = stage === "connecting" || stage === "checking";
  const close = () => auto.sendCode(null);

  let title = "";
  let description: string | undefined;
  if (stage === "show_code") {
    title = t("sync.pairing.showTitle", {
      device: pairing?.device_name || t("sync.devices.iphone"),
    });
  } else if (stage === "failed") {
    title = t("sync.pairing.failedTitle");
  } else if (stage) {
    title = t("sync.pairing.enterTitle");
    description = t("sync.pairing.enterBody", { device });
  }

  const footer =
    stage === "enter_code" || waiting ? (
      <>
        <Button
          variant="secondary"
          onClick={close}
          data-testid="wb-pair-cancel"
          className="max-sm:h-11 max-sm:flex-1"
        >
          {t("sync.review.cancel")}
        </Button>
        <Button
          variant="primary"
          disabled={waiting || code.length !== 6}
          aria-busy={waiting}
          onClick={() => auto.sendCode(code)}
          data-testid="wb-pair-submit"
          className="max-sm:h-11 max-sm:flex-1"
        >
          {t(waiting ? "sync.pairing.checking" : "sync.pairing.pair")}
        </Button>
      </>
    ) : stage === "failed" ? (
      <Button
        variant="primary"
        onClick={close}
        data-testid="wb-pair-close"
        className="max-sm:h-11 max-sm:flex-1"
      >
        {t("sync.review.done")}
      </Button>
    ) : undefined;

  return (
    <>
      <Dialog
        container={container}
        open={open}
        onOpenChange={(next) => !next && close()}
        title={title}
        description={description}
        closeLabel={t("sync.review.cancel")}
        // The Mac's code stays up until the phone has typed it.
        dismissible={stage !== "show_code"}
        footer={footer}
        contentFades={false}
        initialFocusRef={input}
        className="max-w-[420px]"
        testId="wb-pair-dialog"
      >
        {pairing?.stage === "show_code" && (
          <div className="flex flex-col items-center gap-4 pt-2 pb-1">
            <p
              aria-label={pairing.code.split("").join(" ")}
              data-testid="wb-pair-code"
              className="font-(family-name:--wb-font-mono) text-[40px] leading-[48px] font-semibold tracking-[0.18em] text-(--wb-ink) tabular-nums"
            >
              {pairing.code.slice(0, 3)}
              <span className="inline-block w-[0.35em]" />
              {pairing.code.slice(3)}
            </p>
            <p className="text-center text-[14px] leading-5 text-(--wb-ink-2)">
              {t("sync.pairing.showBody")}
            </p>
          </div>
        )}
        {(stage === "enter_code" || waiting) && (
          <div className="flex flex-col gap-3">
            {stage === "connecting" ? (
              <p
                role="status"
                className="flex items-center gap-2.5 text-[15px] leading-[22px] text-(--wb-ink-2)"
              >
                <Loader2
                  size={18}
                  aria-hidden="true"
                  className="shrink-0 animate-spin"
                />
                {t("sync.pairing.connecting", { device })}
              </p>
            ) : (
              <input
                ref={input}
                value={code}
                disabled={waiting}
                onChange={(event) =>
                  setCode(
                    event.currentTarget.value.replace(/\D/g, "").slice(0, 6),
                  )
                }
                onKeyDown={(event) => {
                  if (event.key === "Enter" && code.length === 6) {
                    auto.sendCode(code);
                  }
                }}
                inputMode="numeric"
                pattern="[0-9]*"
                autoComplete="one-time-code"
                aria-label={t("sync.pairing.codeLabel")}
                placeholder="000000"
                data-testid="wb-pair-input"
                className={`h-14 w-full rounded-(--wb-radius-md) border border-(--wb-line-strong) bg-(--wb-surface) text-center font-(family-name:--wb-font-mono) text-[28px] tracking-[0.3em] text-(--wb-ink) tabular-nums placeholder:text-(--wb-ink-disabled) ${FOCUS_RING}`}
              />
            )}
          </div>
        )}
        {pairing?.stage === "failed" && (
          <p
            role="alert"
            data-testid="wb-pair-problem"
            data-problem={pairing.problem}
            className="flex items-start gap-3 text-[15px] leading-[22px] text-(--wb-ink-2)"
          >
            <TriangleAlert
              size={20}
              aria-hidden="true"
              className="mt-px shrink-0 text-(--wb-danger)"
            />
            {auto.status?.role === "mac"
              ? t(`sync.pairing.macProblems.${pairing.problem}`, {
                  defaultValue: t("sync.pairing.macProblems.failed"),
                })
              : t(`sync.pairing.problems.${pairing.problem}`, {
                  defaultValue: t("sync.pairing.problems.failed"),
                })}
          </p>
        )}
      </Dialog>
    </>
  );
}

/**
 * The Mac's pairing code, wherever the user is in the Hub: a phone asked
 * to pair, the app came forward, and the code is on screen.
 */
export function MacPairingGate() {
  const auto = useAutoSync();
  if (auto.status?.role !== "mac") return null;
  return <PairingDialog auto={auto} />;
}

/** "Unpair …?": nothing is deleted, the two just stop syncing. */
export function UnpairDialog({
  device,
  onClose,
  onConfirm,
}: {
  /** The device's name; `null` keeps the dialog closed. */
  device: string | null;
  onClose: () => void;
  onConfirm: () => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const open = device !== null;
  const [container, setContainer] = useState<HTMLElement | null>(null);
  useLayoutEffect(() => {
    if (open) {
      setContainer(document.querySelector<HTMLElement>("dialog[open]"));
    }
  }, [open]);
  // The name stays while the dialog animates out.
  const [name, setName] = useState("");
  useEffect(() => {
    if (device) setName(device);
  }, [device]);
  return (
    <Dialog
      container={container}
      open={open}
      onOpenChange={(next) => !next && onClose()}
      title={t("sync.auto.unpairTitle", { device: device ?? name })}
      closeLabel={t("sync.review.cancel")}
      contentFades={false}
      className="max-w-[420px]"
      testId="wb-unpair-dialog"
      footer={
        <>
          <Button
            variant="secondary"
            onClick={onClose}
            className="max-sm:h-11 max-sm:flex-1"
          >
            {t("sync.review.cancel")}
          </Button>
          <Button
            variant="destructive"
            onClick={onConfirm}
            data-testid="wb-unpair-confirm"
            className="max-sm:h-11 max-sm:flex-1"
          >
            {t("sync.auto.unpair")}
          </Button>
        </>
      }
    >
      <p className="text-[15px] leading-[22px] text-(--wb-ink-2)">
        {t("sync.auto.unpairBody")}
      </p>
    </Dialog>
  );
}
