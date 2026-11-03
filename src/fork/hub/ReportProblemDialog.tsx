// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Help › Report a problem (S-26b, R2 C-103): says what the GitHub issue
 * will contain, offers Copy diagnostics (S-27's sanitized logs, never
 * dictations) and opens the prefilled issue in the browser. The native Help
 * menu's ⌘/ opens the issue directly; its body explains how to add
 * diagnostics from here.
 */
import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ClipboardCopy, ExternalLink, Loader2 } from "../ui/icons";
import { Dialog } from "../ui/Dialog";
import { commands } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";

export interface ReportProblemDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

type CopyState = "idle" | "busy" | "copied" | "failed";

export function ReportProblemDialog({
  open,
  onOpenChange,
}: ReportProblemDialogProps) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [copy, setCopy] = useState<CopyState>("idle");
  const openRef = useRef<HTMLButtonElement>(null);

  const change = (next: boolean) => {
    if (!next) setCopy("idle");
    onOpenChange(next);
  };

  const copyDiagnostics = async () => {
    setCopy("busy");
    try {
      const result = await commands.forkCopyDiagnostics();
      setCopy(result.status === "ok" ? "copied" : "failed");
    } catch {
      setCopy("failed");
    }
  };

  const openIssue = async () => {
    try {
      await commands.forkOpenIssue("problem");
    } finally {
      change(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={change}
      title={t("hub.help.report.title")}
      description={t("hub.help.report.body")}
      closeLabel={t("hub.help.report.close")}
      initialFocusRef={openRef as React.RefObject<HTMLElement>}
      contentFades={false}
      footer={
        <>
          <Button onClick={() => change(false)}>
            {t("hub.help.report.cancel")}
          </Button>
          <Button
            ref={openRef}
            variant="primary"
            onClick={() => void openIssue()}
            data-testid="wb-report-open"
          >
            <ExternalLink size={14} aria-hidden />
            {t("hub.help.report.open")}
          </Button>
        </>
      }
    >
      <section
        data-testid="wb-report-dialog"
        className="flex flex-col gap-3 rounded-(--wb-radius-sm) border border-(--wb-line) bg-(--wb-surface-tint) px-5 py-4"
      >
        <div className="flex flex-col gap-1">
          <h3 className="text-[15px] font-semibold text-(--wb-ink)">
            {t("hub.help.report.diagnosticsTitle")}
          </h3>
          <p className="text-[13px] leading-[18px] text-(--wb-ink-2)">
            {t("hub.help.report.diagnosticsBody")}
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <Button
            variant="outline"
            size="sm"
            onClick={() => void copyDiagnostics()}
            disabled={copy === "busy"}
            data-testid="wb-report-copy-diagnostics"
          >
            {copy === "busy" ? (
              <Loader2 size={14} className="animate-spin" aria-hidden />
            ) : (
              <ClipboardCopy size={14} aria-hidden />
            )}
            {t("hub.help.report.copy")}
          </Button>
          <p
            role="status"
            data-testid="wb-report-copy-status"
            className="text-[13px] text-(--wb-ink-2)"
          >
            {copy === "copied" && t("hub.help.report.copied")}
            {copy === "failed" && t("hub.help.report.copyFailed")}
          </p>
        </div>
      </section>
    </Dialog>
  );
}
