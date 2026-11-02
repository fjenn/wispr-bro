// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { ChevronRight, X } from "lucide-react";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { getLanguageDirection } from "@/lib/utils/rtl";
import { FORK_NAMESPACE } from "../i18n";
import { navigate, useHubNav } from "../hub/nav";
import { FOCUS_RING } from "../ui";
import { About } from "./About";
import { Advanced } from "./Advanced";
import { DataPrivacy } from "./DataPrivacy";
import { General } from "./General";
import { System } from "./System";
import { HotkeysPanel } from "./hotkeys/HotkeysPanel";

const SECTIONS = ["general", "system", "privacy", "advanced", "about"] as const;
type Section = (typeof SECTIONS)[number] | "hotkeys";

function sectionFromHash(value: string | null): Section {
  if (value === "hotkeys" || value === "shortcuts") return "hotkeys";
  if (value === "models") return "advanced";
  if (value === "microphone" || value === "languages") return "general";
  return SECTIONS.includes(value as (typeof SECTIONS)[number])
    ? (value as (typeof SECTIONS)[number])
    : "general";
}

export function SettingsModal() {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const requested = useHubNav((state) => state.section);
  const section = sectionFromHash(requested);
  const dialogRef = useRef<HTMLDialogElement>(null);
  const previousFocus = useRef<Element | null>(null);
  const title = t(`settingsModal.sections.${section}`);

  useEffect(() => {
    const dialog = dialogRef.current;
    previousFocus.current ??= document.activeElement;
    if (dialog && !dialog.open) dialog.showModal();
    return () => {
      const previous = previousFocus.current;
      const target =
        previous instanceof HTMLElement && previous.isConnected
          ? previous
          : document.querySelector<HTMLElement>(
              '[data-testid="wb-nav-settings"]',
            );
      target?.focus();
    };
  }, []);

  useEffect(() => {
    if (requested === "history") navigate("dictation", "history");
    if (requested === "microphone" || requested === "languages") {
      document
        .getElementById(
          `wb-general-${requested === "microphone" ? "mic" : "languages"}`,
        )
        ?.scrollIntoView({ block: "start" });
    }
  }, [requested]);

  const close = () => navigate("dictation");
  const select = (next: Section) => navigate("settings", next);

  return (
    <dialog
      ref={dialogRef}
      aria-labelledby="wb-settings-title"
      dir={getLanguageDirection(i18n.language)}
      className="m-auto h-[min(680px,calc(100vh-32px))] w-[min(920px,calc(100vw-32px))] max-w-none overflow-hidden rounded-2xl border border-(--wb-line) bg-(--wb-canvas) p-0 text-(--wb-ink) shadow-(--wb-shadow) backdrop:bg-black/45"
      data-testid="wb-settings-modal"
      onCancel={(event) => {
        event.preventDefault();
        close();
      }}
    >
      <div className="grid h-full grid-cols-[190px_minmax(0,1fr)]">
        <aside className="flex min-h-0 flex-col border-e border-(--wb-line) bg-(--wb-sidebar) p-4">
          <div className="flex h-10 items-center justify-between gap-2">
            <h1
              id="wb-settings-title"
              className="font-(--wb-font-display) text-xl"
            >
              {t("settingsModal.title")}
            </h1>
            <button
              type="button"
              aria-label={t("settingsModal.close")}
              title={t("settingsModal.close")}
              onClick={close}
              className={`grid size-8 cursor-pointer place-items-center rounded-full text-(--wb-ink-2) hover:bg-(--wb-hover) ${FOCUS_RING}`}
              data-testid="wb-settings-close"
            >
              <X size={17} aria-hidden="true" />
            </button>
          </div>
          <nav aria-label={t("settingsModal.navigation")} className="mt-5">
            <ul className="flex flex-col gap-1">
              {SECTIONS.map((id) => (
                <li key={id}>
                  <button
                    type="button"
                    aria-current={section === id ? "location" : undefined}
                    aria-expanded={
                      id === "advanced" ? section === id : undefined
                    }
                    onClick={() => select(id)}
                    className={`flex w-full cursor-pointer items-center gap-2 rounded-(--wb-radius-control) px-3 py-2 text-start text-sm ${FOCUS_RING} ${
                      section === id
                        ? "bg-(--wb-selected) font-semibold"
                        : "text-(--wb-ink-2) hover:bg-(--wb-hover)"
                    }`}
                    data-testid={`wb-settings-nav-${id}`}
                  >
                    <span className="flex-1">
                      {t(`settingsModal.sections.${id}`)}
                    </span>
                    {id === "advanced" && (
                      <ChevronRight
                        size={14}
                        aria-hidden="true"
                        className={`transition-transform motion-reduce:transition-none ${section === id ? "rotate-90" : ""}`}
                      />
                    )}
                  </button>
                </li>
              ))}
            </ul>
          </nav>
        </aside>
        <div
          className="min-h-0 overflow-y-auto"
          data-testid="wb-settings-content"
        >
          <header className="sticky top-0 z-10 border-b border-(--wb-line) bg-(--wb-canvas) px-6 py-5">
            <h2 className="font-(--wb-font-display) text-2xl">{title}</h2>
          </header>
          <div className="p-6">
            <ErrorBoundary context={`settings ${section}`}>
              {section === "general" && (
                <General onChangeHotkeys={() => select("hotkeys")} />
              )}
              {section === "system" && <System />}
              {section === "privacy" && <DataPrivacy />}
              {section === "advanced" && <Advanced />}
              {section === "about" && <About />}
              {section === "hotkeys" && (
                <HotkeysPanel onDone={() => select("general")} />
              )}
            </ErrorBoundary>
          </div>
        </div>
      </div>
    </dialog>
  );
}
