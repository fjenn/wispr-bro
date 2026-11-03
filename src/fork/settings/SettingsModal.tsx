// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Settings modal (S-14; Design W3 after REFERENCE-SPEC §1 "Settings"):
 * a 960 × 640 modal over the Hub with a 210 pt nav on the chrome (glyphs,
 * micro group labels, the version pinned at the bottom) and a white pane
 * with a serif section title, one intro line and tinted row cards. A native
 * `<dialog>` (top layer, inert page, Esc) that springs in; sections
 * crossfade (`settings.css`; both collapse under Reduce Motion).
 */
import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { getVersion } from "@tauri-apps/api/app";
import {
  ChevronRight,
  CodeXml,
  Info,
  Laptop,
  Settings2,
  ShieldCheck,
  SlidersHorizontal,
  X,
} from "../ui/icons";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { getLanguageDirection } from "@/lib/utils/rtl";
import { FORK_NAMESPACE } from "../i18n";
import { navigate, useHubNav } from "../hub/nav";
import { FOCUS_RING, IconButton } from "../ui";
import { About } from "./About";
import { Advanced } from "./Advanced";
import { DataPrivacy } from "./DataPrivacy";
import { General } from "./General";
import { System } from "./System";
import { VibeCoding } from "./VibeCoding";
import { useHandyA11y } from "./handyA11y";
import { HotkeysPanel } from "./hotkeys/HotkeysPanel";
import "./settings.css";

const SECTIONS = [
  "general",
  "system",
  "vibe",
  "privacy",
  "advanced",
  "about",
] as const;
type NavSection = (typeof SECTIONS)[number];
type Section = NavSection | "hotkeys";

/** The nav's two groups, in order (the reference's SETTINGS / ACCOUNT). */
const GROUPS: { id: "settings" | "app"; items: NavSection[] }[] = [
  { id: "settings", items: ["general", "system", "vibe", "advanced"] },
  { id: "app", items: ["privacy", "about"] },
];

const ICONS: Record<NavSection, ReactNode> = {
  general: <SlidersHorizontal size={18} />,
  system: <Laptop size={18} />,
  vibe: <CodeXml size={18} />,
  privacy: <ShieldCheck size={18} />,
  advanced: <Settings2 size={18} />,
  about: <Info size={18} />,
};

/** The one-line intro under each section title (REFERENCE-SPEC §9). */
const INTROS: Partial<Record<Section, string>> = {
  general: "settingsModal.intro.general",
  system: "settingsModal.intro.system",
  vibe: "settingsModal.vibe.intro",
  privacy: "settingsModal.intro.privacy",
  advanced: "settingsModal.intro.advanced",
  about: "settingsModal.about.tagline",
};

function sectionFromHash(value: string | null): Section {
  if (value === "hotkeys" || value === "shortcuts") return "hotkeys";
  if (value === "models") return "advanced";
  if (value === "microphone" || value === "languages") return "general";
  return SECTIONS.includes(value as NavSection)
    ? (value as NavSection)
    : "general";
}

function useAppVersion(): string | null {
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => {
    void getVersion()
      .then(setVersion)
      .catch(() => setVersion(null));
  }, []);
  return version;
}

export function SettingsModal() {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const requested = useHubNav((state) => state.section);
  const section = sectionFromHash(requested);
  const dialogRef = useRef<HTMLDialogElement>(null);
  const previousFocus = useRef<Element | null>(null);
  // S-37: Handy's hosted rows get names for their unnamed controls.
  const contentRef = useRef<HTMLDivElement>(null);
  useHandyA11y(contentRef);
  const version = useAppVersion();
  const title = t(`settingsModal.sections.${section}`);
  const intro = INTROS[section];

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

  // A new section starts at the top of the pane.
  useEffect(() => {
    if (requested !== "microphone" && requested !== "languages") {
      contentRef.current?.scrollTo({ top: 0 });
    }
  }, [section, requested]);

  const close = () => navigate("dictation");
  const select = (next: Section) => navigate("settings", next);

  return (
    <dialog
      ref={dialogRef}
      aria-labelledby="wb-settings-title"
      dir={getLanguageDirection(i18n.language)}
      className="wb-settings-dialog m-auto h-[min(640px,calc(100vh-48px))] w-[min(960px,calc(100vw-48px))] max-w-none overflow-hidden rounded-(--wb-radius-lg) border-0 bg-(--wb-surface) p-0 text-(--wb-ink) shadow-(--wb-elev-3)"
      data-testid="wb-settings-modal"
      onCancel={(event) => {
        event.preventDefault();
        close();
      }}
    >
      <IconButton
        aria-label={t("settingsModal.close")}
        title={t("settingsModal.close")}
        variant="secondary"
        size="sm"
        onClick={close}
        className="absolute end-4 top-4 z-20"
        data-testid="wb-settings-close"
      >
        <X size={16} />
      </IconButton>
      <div className="grid h-full grid-cols-[210px_minmax(0,1fr)]">
        <aside className="flex min-h-0 flex-col bg-(--wb-chrome) px-2 pt-5 pb-3">
          <nav
            aria-label={t("settingsModal.navigation")}
            className="flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto"
          >
            {GROUPS.map((group) => {
              // The first group's label is the dialog's title.
              const labelId =
                group.id === "settings"
                  ? "wb-settings-title"
                  : `wb-settings-group-${group.id}`;
              return (
                <div key={group.id} className="flex flex-col gap-1">
                  {group.id === "settings" ? (
                    <h1
                      id={labelId}
                      className="type-micro px-3 pb-1 text-(--wb-ink-3)"
                    >
                      {t("settingsModal.title")}
                    </h1>
                  ) : (
                    <p
                      id={labelId}
                      className="type-micro px-3 pb-1 text-(--wb-ink-3)"
                    >
                      {t("settingsModal.navGroups.app")}
                    </p>
                  )}
                  <ul aria-labelledby={labelId} className="flex flex-col">
                    {group.items.map((id) => (
                      <li key={id}>
                        <NavItem
                          id={id}
                          current={section === id}
                          onSelect={() => select(id)}
                          label={t(`settingsModal.sections.${id}`)}
                        />
                      </li>
                    ))}
                  </ul>
                </div>
              );
            })}
          </nav>
          <p
            className="type-small px-3 pt-3 text-(--wb-ink-3) tabular-nums"
            data-testid="wb-settings-version"
          >
            {t("settingsModal.about.version", { version: version ?? "…" })}
          </p>
        </aside>
        <div
          ref={contentRef}
          className="min-h-0 overflow-y-auto"
          data-testid="wb-settings-content"
        >
          <div key={section} className="wb-settings-section px-12 pt-11 pb-12">
            <header className={`pe-8 ${intro ? "mb-8" : "mb-3"}`}>
              <h2 className="type-serif-title text-(--wb-ink)">
                {section === "hotkeys"
                  ? t("settingsModal.sections.hotkeys")
                  : title}
              </h2>
              {intro && (
                <p
                  className="type-body mt-2 max-w-[560px] text-(--wb-ink-2)"
                  data-testid={`wb-${section === "vibe" ? "vibe" : "settings"}-intro`}
                >
                  {t(intro)}
                </p>
              )}
            </header>
            <ErrorBoundary context={`settings ${section}`}>
              {section === "general" && (
                <General onChangeHotkeys={() => select("hotkeys")} />
              )}
              {section === "system" && <System />}
              {section === "vibe" && <VibeCoding />}
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

function NavItem({
  id,
  current,
  label,
  onSelect,
}: {
  id: NavSection;
  current: boolean;
  label: string;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      aria-current={current ? "location" : undefined}
      aria-expanded={id === "advanced" ? current : undefined}
      onClick={onSelect}
      className={`flex h-9 w-full cursor-pointer items-center gap-3 rounded-(--wb-radius-sm) px-3 text-start text-[15px] leading-5 transition-colors duration-(--wb-dur-fast) ${FOCUS_RING} ${
        current
          ? "bg-(--wb-fill-active) text-(--wb-ink)"
          : "text-(--wb-ink) hover:bg-(--wb-fill-hover)"
      }`}
      data-testid={`wb-settings-nav-${id}`}
    >
      <span aria-hidden="true" className="shrink-0 text-(--wb-ink-2)">
        {ICONS[id]}
      </span>
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {id === "advanced" && (
        <ChevronRight
          size={14}
          aria-hidden="true"
          className={`shrink-0 text-(--wb-ink-3) transition-transform duration-(--wb-dur-fast) rtl:-scale-x-100 ${current ? "rotate-90" : ""}`}
        />
      )}
    </button>
  );
}
