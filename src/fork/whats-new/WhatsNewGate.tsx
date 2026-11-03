// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * What's New (S-28, C-99): after an update, the Hub shows our note for the
 * new version once (`releaseNotes.ts`), and never while a dictation is in
 * progress (`busy` in `fork-update-state`; the dialog opens when the take
 * ends). Handy's "Show What's New after updates" setting and its
 * `whats_new_last_seen_version` record what was seen, as in Handy's gate;
 * Handy's own notes are not shown.
 */
import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { useTranslation } from "react-i18next";
import { Dialog } from "@/components/ui";
import { MarkdownContent } from "@/components/whats-new/MarkdownContent";
import { useSettings } from "@/hooks/useSettings";
import { useUpdateState } from "../hub/UpdateStatus";
import { FORK_NAMESPACE } from "../i18n";
import { Button } from "../ui";
import { noteToShow, type ReleaseNote } from "./releaseNotes";

export function WhatsNewGate() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, isLoading, updateSetting } = useSettings();
  const [update] = useUpdateState();
  const [note, setNote] = useState<ReleaseNote | null>(null);
  const [dismissed, setDismissed] = useState<string | null>(null);

  const enabled = settings?.show_whats_new_on_update ?? false;
  const lastSeen = settings?.whats_new_last_seen_version ?? "";

  useEffect(() => {
    if (isLoading || !settings || !enabled) {
      setNote(null);
      return;
    }
    let cancelled = false;
    getVersion()
      .then((current) => {
        if (!cancelled) setNote(noteToShow(current, lastSeen));
      })
      .catch(() => {
        // No version, no note.
      });
    return () => {
      cancelled = true;
    };
  }, [isLoading, settings, enabled, lastSeen]);

  // Never while dictating: wait for the update state, and for `busy` false.
  if (!note || dismissed === note.version || !update || update.busy) {
    return null;
  }

  const close = () => {
    setDismissed(note.version);
    void updateSetting("whats_new_last_seen_version", note.version);
  };

  return (
    <Dialog
      open
      title={t("whatsNew.title", { version: note.version })}
      description={t("whatsNew.label")}
      closeLabel={t("whatsNew.close")}
      onOpenChange={(open) => {
        if (!open) close();
      }}
      footer={
        <div className="flex justify-end">
          <Button data-testid="wb-whats-new-close" onClick={close}>
            {t("whatsNew.done")}
          </Button>
        </div>
      }
    >
      <div data-testid="wb-whats-new">
        <MarkdownContent markdown={note.markdown} />
      </div>
    </Dialog>
  );
}
