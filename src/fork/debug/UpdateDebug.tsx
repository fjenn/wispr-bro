// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Debug panel section for F-02: the running version, the update channel and
 * its endpoints, the key downloads are verified against, and "Check now" /
 * "Install and relaunch" through `fork/update` (internal alphas update this
 * way until S-28 adds the footer and tray UI).
 */
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  commands,
  type ForkError,
  type ForkUpdateCheck,
  type ForkUpdateStatus,
} from "../bindings";
import { FORK_NAMESPACE } from "../i18n";

type Busy = "idle" | "checking" | "installing";

const BUTTON =
  "rounded border border-mid-gray/30 px-2 py-0.5 hover:bg-mid-gray/10 disabled:opacity-50 disabled:cursor-not-allowed";

function errorText(e: ForkError): string {
  return e.kind === "NotReady" ? e.kind : e.message;
}

export function UpdateDebug() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [status, setStatus] = useState<ForkUpdateStatus | null>(null);
  const [check, setCheck] = useState<ForkUpdateCheck | null>(null);
  const [busy, setBusy] = useState<Busy>("idle");
  const [message, setMessage] = useState<string | null>(null);

  const readStatus = useCallback(async () => {
    try {
      setStatus(await commands.forkUpdateStatus());
    } catch (e) {
      setMessage(t("debug.error", { message: String(e) }));
    }
  }, [t]);

  useEffect(() => {
    void readStatus();
  }, [readStatus]);

  const runCheck = async () => {
    setBusy("checking");
    setMessage(null);
    const result = await commands.forkUpdateCheck();
    setBusy("idle");
    if (result.status === "ok") {
      setCheck(result.data);
      setMessage(
        result.data.available
          ? t("debug.update.available", {
              version: result.data.available.version,
            })
          : t("debug.update.upToDate"),
      );
    } else {
      setCheck(null);
      setMessage(
        t("debug.update.failed", { message: errorText(result.error) }),
      );
    }
  };

  const install = async () => {
    setBusy("installing");
    setMessage(t("debug.update.installing"));
    const result = await commands.forkUpdateInstall();
    setBusy("idle");
    if (result.status === "ok") {
      setMessage(
        result.data
          ? t("debug.update.installed", { version: result.data })
          : t("debug.update.upToDate"),
      );
    } else {
      setMessage(
        t("debug.update.failed", { message: errorText(result.error) }),
      );
    }
  };

  const channel = status
    ? t(`settingsModal.advanced.updateChannels.${status.channel}`)
    : t("debug.pending");

  return (
    <div className="flex flex-col gap-2 text-sm">
      <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-xs">
        <dt className="text-ink-3">{t("debug.update.version")}</dt>
        <dd data-testid="fork-debug-update-version" className="font-mono">
          {status?.current_version ?? t("debug.pending")}
        </dd>
        <dt className="text-ink-3">{t("debug.update.channel")}</dt>
        <dd data-testid="fork-debug-update-channel">{channel}</dd>
        <dt className="text-ink-3">{t("debug.update.key")}</dt>
        <dd className="font-mono">{status?.pubkey_id ?? "…"}</dd>
        <dt className="text-ink-3">{t("debug.update.endpoints")}</dt>
        <dd>
          <ul className="font-mono break-all select-text" dir="ltr">
            {(status?.endpoints ?? []).map((e) => (
              <li key={e}>{e}</li>
            ))}
          </ul>
        </dd>
      </dl>
      {status && !status.checks_enabled && (
        <p data-testid="fork-debug-update-off" className="text-xs text-ink-3">
          {t("debug.update.checksOff")}
        </p>
      )}
      <div className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          onClick={() => void runCheck()}
          disabled={busy !== "idle" || !status?.checks_enabled}
          className={BUTTON}
        >
          {busy === "checking"
            ? t("debug.update.checking")
            : t("debug.update.check")}
        </button>
        {check?.available && (
          <button
            type="button"
            onClick={() => void install()}
            disabled={busy !== "idle"}
            className={BUTTON}
          >
            {t("debug.update.install", { version: check.available.version })}
          </button>
        )}
      </div>
      <p role="status" data-testid="fork-debug-update-message">
        {message}
      </p>
    </div>
  );
}
