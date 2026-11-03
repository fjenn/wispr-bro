// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * What's New (S-28, C-99): after an update, the Hub shows our note for the
 * new version once (`releaseNotes.ts`), and never while a dictation is in
 * progress (`busy` in `fork-update-state`; the dialog opens when the take
 * ends). Handy's "Show What's New after updates" setting and its
 * `whats_new_last_seen_version` record what was seen, as in Handy's gate;
 * Handy's own notes are not shown.
 *
 * Mac wave 3 (finding 4): it opened on top of the first-use chooser on a
 * profile's first Hub open. The notes are for updates, so:
 *
 * - a new user (the first-use chooser still due to open by itself, S-30:
 *   setup just finished) gets the running version marked seen, silently;
 * - otherwise it waits (not dismissed) while the chooser, a lesson, the
 *   bar spotlight or any other dialog is open, one modal at a time.
 *
 * It decides only once the fork settings are loaded, so it never opens a
 * moment before the chooser does.
 */
import { useEffect, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { useTranslation } from "react-i18next";
import { Dialog } from "@/components/ui";
import { MarkdownContent } from "@/components/whats-new/MarkdownContent";
import { useSettings } from "@/hooks/useSettings";
import { useOtherDialogOpen } from "../hub/featurePopupGate";
import { useUpdateState } from "../hub/UpdateStatus";
import { FORK_NAMESPACE } from "../i18n";
import { useChooserOpen } from "../onboarding/FirstUseChooser";
import { CHOOSER_DISMISSED_ID } from "../onboarding/firstUse";
import { useFirstUseStore } from "../onboarding/firstUseStore";
import { useForkSettingsStore } from "../settings/store";
import { useLessonStore } from "../tutorials/lessonRegistry";
import { Button } from "../ui";
import { noteToShow, type ReleaseNote } from "./releaseNotes";

/** The marker inside What's New's own dialog. */
const OWN = '[data-testid="wb-whats-new"]';
/**
 * After the last blocker closes, before the dialog opens: the closing
 * dialog hands focus back first (opened in the same moment, What's New saw
 * that focus move outside and closed itself, marked seen).
 */
export const AFTER_BLOCKER_MS = 400;

/** `true` once `blocked` has been false for `ms`. */
function useClearFor(blocked: boolean, ms: number): boolean {
  const [clear, setClear] = useState(!blocked);
  useEffect(() => {
    if (blocked) {
      setClear(false);
      return;
    }
    const timer = window.setTimeout(() => setClear(true), ms);
    return () => window.clearTimeout(timer);
  }, [blocked, ms]);
  return clear && !blocked;
}

/**
 * `true` for a new user (the first-use chooser is due to open by itself),
 * `null` until the fork settings are loaded.
 */
function useNewUser(): boolean | null {
  const fork = useForkSettingsStore((s) => s.settings);
  if (!fork) return null;
  return (
    fork.onboarding.status === "completed" &&
    fork.first_use_choice == null &&
    !fork.dismissed_banners.includes(CHOOSER_DISMISSED_ID)
  );
}

export function WhatsNewGate() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings, isLoading, updateSetting } = useSettings();
  const [update] = useUpdateState();
  const [note, setNote] = useState<ReleaseNote | null>(null);
  const [dismissed, setDismissed] = useState<string | null>(null);
  const newUser = useNewUser();
  // Latched: making the chooser's choice must not turn a new user into
  // one who gets this version's note.
  const wasNewUser = useRef(false);
  if (newUser) wasNewUser.current = true;
  const chooser = useChooserOpen();
  const lesson = useLessonStore((s) => s.open != null);
  const spotlight = useFirstUseStore((s) => s.spotlight?.active ?? false);
  const otherDialog = useOtherDialogOpen(OWN);
  const clear = useClearFor(
    chooser || lesson || spotlight || otherDialog,
    AFTER_BLOCKER_MS,
  );

  const enabled = settings?.show_whats_new_on_update ?? false;
  const lastSeen = settings?.whats_new_last_seen_version ?? "";

  useEffect(() => {
    if (isLoading || !settings || !enabled || newUser === null) {
      setNote(null);
      return;
    }
    let cancelled = false;
    getVersion()
      .then((current) => {
        if (cancelled) return;
        const next = noteToShow(current, lastSeen);
        if (next && wasNewUser.current) {
          // A new install: the version it came with counts as seen.
          setNote(null);
          void updateSetting("whats_new_last_seen_version", current);
          return;
        }
        setNote(next);
      })
      .catch(() => {
        // No version, no note.
      });
    return () => {
      cancelled = true;
    };
  }, [isLoading, settings, enabled, lastSeen, newUser, updateSetting]);

  // Never while dictating: wait for the update state, and for `busy` false.
  // Never over another modal or the first-use flow.
  if (!note || dismissed === note.version || !update || update.busy || !clear) {
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
