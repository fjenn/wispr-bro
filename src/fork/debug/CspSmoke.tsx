// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * CSP smoke section of the fork debug panel (F-08, DEC-43). Runs
 * `runCspSmoke()` on demand and lists each check and any CSP violation.
 */
import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { runCspSmoke, type CspSmokeResult } from "./cspSmokeChecks";

const CHECKS = [
  "media",
  "dataImage",
  "blobImage",
  "forkCommand",
  "probe",
] as const;

export function CspSmoke() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<CspSmokeResult | null>(null);

  const run = useCallback(async () => {
    setRunning(true);
    try {
      setResult(await runCspSmoke());
    } finally {
      setRunning(false);
    }
  }, []);

  return (
    <div className="flex flex-col gap-2 text-sm" data-testid="fork-csp-smoke">
      <p className="text-xs text-ink-3">{t("debug.csp.description")}</p>
      <div>
        <button
          type="button"
          onClick={() => void run()}
          disabled={running}
          data-testid="fork-csp-smoke-run"
          className="px-2 py-1 text-sm rounded border border-line-strong bg-wash hover:bg-logo-primary/10 cursor-pointer disabled:opacity-50"
        >
          {running ? t("debug.csp.running") : t("debug.csp.run")}
        </button>
      </div>
      {result && (
        <>
          <p
            role="status"
            data-testid="fork-csp-smoke-verdict"
            className={
              result.ok ? "text-xs text-green-600" : "text-xs text-red-500"
            }
          >
            {result.ok ? t("debug.csp.pass") : t("debug.csp.fail")}
          </p>
          <ul className="text-xs flex flex-col gap-1">
            {CHECKS.map((key) => {
              const check = result.checks[key];
              return (
                <li key={key} data-testid={`fork-csp-check-${key}`}>
                  <span className="font-semibold">
                    {t(`debug.csp.checks.${key}`)}
                  </span>
                  {": "}
                  {check.state === "ok"
                    ? t("debug.csp.ok", { detail: check.detail })
                    : t("debug.csp.failed", { detail: check.detail })}
                </li>
              );
            })}
          </ul>
          <p className="text-xs" data-testid="fork-csp-violations">
            {result.violations.length === 0
              ? t("debug.csp.noViolations")
              : t("debug.csp.violations", {
                  count: result.violations.length,
                })}
          </p>
          {result.violations.length > 0 && (
            <pre className="text-xs whitespace-pre-wrap break-all font-mono select-text">
              {JSON.stringify(result.violations, null, 2)}
            </pre>
          )}
        </>
      )}
    </div>
  );
}
