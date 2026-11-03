// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Hub footer (S-13, C-26): background-job status on the left (Handy's
 * `TranscriptionStatus`, which opens History). Handy's model selector is not
 * shown (B-207: the user never picks a model; models stay in Settings).
 *
 * The right side shows the update status (S-28, `UpdateStatus`; Handy's
 * `UpdateChecker` is no longer mounted) next to the version.
 */
import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { useTranslation } from "react-i18next";
import TranscriptionStatus from "@/components/footer/TranscriptionStatus";
import { FORK_NAMESPACE } from "../i18n";
import { navigate } from "./nav";
import { UpdateStatus } from "./UpdateStatus";

export function Footer() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [version, setVersion] = useState("");

  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch((e) => console.warn("fork: getVersion failed", e));
  }, []);

  return (
    <footer
      data-testid="wb-footer"
      className="flex h-9 shrink-0 items-center justify-between gap-4 border-t border-(--wb-line) px-5 text-[12px] text-(--wb-ink-3)"
    >
      <div data-testid="wb-footer-jobs" className="flex items-center gap-3">
        <TranscriptionStatus
          onShowHistory={() => navigate("dictation", "history")}
        />
      </div>
      <div className="flex items-center gap-1.5">
        <UpdateStatus />
        {version && (
          <span data-testid="wb-footer-version">
            {t("hub.footer.version", { version })}
          </span>
        )}
      </div>
    </footer>
  );
}
