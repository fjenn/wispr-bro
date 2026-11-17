// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The My Voice tab: the three things that shape how the user's dictations
 * are written, as three rows under one large title and nothing else. A
 * row pushes its page (the Hub's Dictionary and Snippets, the phone's
 * Style), which keeps its own location (`#/hub/dictionary`, …); Back, or
 * the tab again, returns here (`./nav`).
 *
 * Each row carries one quiet line of what is there now: how many words,
 * how many snippets, the style in use.
 */
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands, events } from "../bindings";
import { navigate } from "../hub/nav";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettings } from "../settings/useForkSettings";
import {
  BookA,
  ChevronRight,
  Palette,
  TextQuote,
  type Glyph,
} from "../ui/icons";
import type { VoicePage } from "./nav";
import { IosTitle } from "./parts";

const ICONS: Record<VoicePage, Glyph> = {
  dictionary: BookA,
  snippets: TextQuote,
  style: Palette,
};

/** How many dictionary entries and snippets there are, kept current. */
function useCounts(): { words: number | null; snippets: number | null } {
  const [words, setWords] = useState<number | null>(null);
  const [snippets, setSnippets] = useState<number | null>(null);
  useEffect(() => {
    let active = true;
    const offs: (() => void)[] = [];
    const loadWords = () =>
      void commands
        .forkDictionaryList()
        .then((result) => {
          if (!active || result.status !== "ok") return;
          const { words: plain, rules } = result.data.counts;
          setWords(plain + rules);
        })
        .catch(() => undefined);
    const loadSnippets = () =>
      void commands
        .forkSnippetsList()
        .then((result) => {
          if (active && result.status === "ok") {
            setSnippets(result.data.counts.snippets);
          }
        })
        .catch(() => undefined);
    loadWords();
    loadSnippets();
    const keep = (listening: Promise<() => void>) =>
      void listening
        .then((off) => (active ? offs.push(off) : off()))
        .catch(() => undefined);
    keep(events.forkDictionaryChanged.listen(loadWords));
    keep(events.forkSnippetsChanged.listen(loadSnippets));
    return () => {
      active = false;
      offs.forEach((off) => off());
    };
  }, []);
  return { words, snippets };
}

export function IosMyVoice() {
  const { t } = useTranslation(FORK_NAMESPACE);
  const { settings } = useForkSettings();
  const { words, snippets } = useCounts();
  const preset = settings?.styles.other;

  const rows: { id: VoicePage; summary: string }[] = [
    {
      id: "dictionary",
      summary: words === null ? "" : t("ios.voice.words", { count: words }),
    },
    {
      id: "snippets",
      summary:
        snippets === null
          ? ""
          : t("snippets.counts.snippets", { count: snippets }),
    },
    {
      id: "style",
      summary: preset ? t(`style.presets.${preset}.label`) : "",
    },
  ];

  return (
    <div className="flex flex-col gap-5" data-testid="wb-ios-voice">
      <IosTitle>{t("ios.tabs.voice")}</IosTitle>
      <nav aria-label={t("ios.tabs.voice")} className="flex flex-col gap-3">
        {rows.map(({ id, summary }) => {
          const Icon = ICONS[id];
          return (
            <button
              key={id}
              type="button"
              className="wb-ios-voice-row"
              onClick={() => navigate(id)}
              data-testid={`wb-ios-voice-${id}`}
            >
              <span aria-hidden="true" className="wb-ios-voice-icon">
                <Icon size={24} />
              </span>
              <span className="flex min-w-0 flex-1 flex-col gap-0.5 text-start">
                <span className="text-[17px] leading-[22px] font-semibold text-(--wb-ink)">
                  {t(`hub.pages.${id}.title`)}
                </span>
                <span
                  className="min-h-5 truncate text-[14px] leading-5 text-(--wb-ink-3)"
                  data-testid={`wb-ios-voice-${id}-summary`}
                >
                  {summary}
                </span>
              </span>
              <ChevronRight
                size={20}
                aria-hidden="true"
                className="shrink-0 text-(--wb-ink-disabled) rtl:-scale-x-100"
              />
            </button>
          );
        })}
      </nav>
    </div>
  );
}
