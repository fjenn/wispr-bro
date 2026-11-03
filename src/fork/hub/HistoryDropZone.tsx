// SPDX-License-Identifier: GPL-3.0-or-later
import { useRef } from "react";
import { useTranslation } from "react-i18next";
import { FileAudio } from "../ui/icons";
import { transcribeFiles } from "@/components/settings/history/transcribeFiles";
import { useDropZone } from "./dropRouter";

export function HistoryDropZone({ children }: { children: React.ReactNode }) {
  const { t } = useTranslation();
  const ref = useRef<HTMLDivElement>(null);
  const active = useDropZone(ref, {
    onDrop: async (paths) => {
      await transcribeFiles(paths);
    },
  });
  return (
    <div
      ref={ref}
      data-testid="wb-drop-zone-history"
      data-drop-active={active || undefined}
      className="relative w-full"
    >
      {children}
      {active && (
        <div className="pointer-events-none absolute inset-0 z-10 flex flex-col items-center justify-center gap-2 rounded-(--wb-radius-card) border-2 border-dashed border-(--wb-accent) bg-(--wb-canvas)/90 text-center">
          <FileAudio
            size={32}
            aria-hidden="true"
            className="text-(--wb-ink-2)"
          />
          <p className="text-base font-semibold">
            {t("settings.history.dropTitle")}
          </p>
          <p className="text-sm text-(--wb-ink-2)">
            {t("settings.history.dropDescription")}
          </p>
        </div>
      )}
    </div>
  );
}
