// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Debug panel section for S-18: the macOS notification permission (read
 * without prompting) and a poster that sends a sample of any kind through
 * the real path after a delay, so a tester can switch to another app first
 * (`docs/fork/MAC-TEST-SCRIPT.md` S-18).
 */
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  commands,
  type NotificationKind,
  type NotificationPermission,
} from "../bindings";
import { FORK_NAMESPACE } from "../i18n";

const KINDS: NotificationKind[] = [
  "model_ready",
  "update_ready",
  "queued_takes_ready",
  "shortcut_problem",
  "crash_report",
  "data_recovered",
  "take_recovered",
  "milestone",
  "tip",
  "announcement",
];

export function NotificationsDebug() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [permission, setPermission] = useState<NotificationPermission | null>(
    null,
  );
  const [kind, setKind] = useState<NotificationKind>("model_ready");
  const [delay, setDelay] = useState(5);
  const [message, setMessage] = useState<string | null>(null);

  const readPermission = useCallback(async () => {
    const result = await commands.forkNotificationsPermission();
    setPermission(result.status === "ok" ? result.data : null);
  }, []);

  useEffect(() => {
    void readPermission();
  }, [readPermission]);

  const post = async () => {
    const result = await commands.forkNotificationsDebugPost(
      kind,
      Math.max(0, Math.min(60, delay)) * 1000,
    );
    setMessage(
      result.status === "ok"
        ? t("debug.notifications.posted", { seconds: delay })
        : t("debug.error", { message: JSON.stringify(result.error) }),
    );
  };

  return (
    <div className="flex flex-col gap-2 text-sm">
      <p data-testid="fork-debug-notifications-permission">
        {t("debug.notifications.permission", {
          state: permission ?? t("debug.pending"),
        })}
      </p>
      <div className="flex flex-wrap items-center gap-2">
        <label className="flex items-center gap-1">
          {t("debug.notifications.kind")}
          <select
            value={kind}
            onChange={(e) => setKind(e.target.value as NotificationKind)}
            className="rounded border border-mid-gray/30 bg-background px-1"
          >
            {KINDS.map((k) => (
              <option key={k} value={k}>
                {k}
              </option>
            ))}
          </select>
        </label>
        <label className="flex items-center gap-1">
          {t("debug.notifications.delay")}
          <input
            type="number"
            min={0}
            max={60}
            value={delay}
            onChange={(e) => setDelay(Number(e.target.value))}
            className="w-16 rounded border border-mid-gray/30 bg-background px-1"
          />
        </label>
        <button
          type="button"
          onClick={() => void post()}
          className="rounded border border-mid-gray/30 px-2 py-0.5 hover:bg-mid-gray/10"
        >
          {t("debug.notifications.post")}
        </button>
        <button
          type="button"
          onClick={() => void readPermission()}
          className="rounded border border-mid-gray/30 px-2 py-0.5 hover:bg-mid-gray/10"
        >
          {t("debug.runAgain")}
        </button>
      </div>
      {message && <p role="status">{message}</p>}
    </div>
  );
}
