// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * History search (S-15b, C-36): a search button that opens a field in
 * place. Typing searches as you go (200 ms after the last key, S-09's
 * index: substrings, any case); Esc clears and closes, and focus goes back
 * to the button.
 */
import { Search as SearchIcon, X } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { FOCUS_RING } from "../ui";

export const SEARCH_DEBOUNCE_MS = 200;

export interface SearchProps {
  query: string;
  onQuery: (query: string) => void;
}

export function Search({ query, onQuery }: SearchProps) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [open, setOpen] = useState(query.length > 0);
  const [text, setText] = useState(query);
  const inputRef = useRef<HTMLInputElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const id = useId();

  useEffect(() => {
    const trimmed = text.trim();
    if (trimmed === query) return;
    const timer = setTimeout(() => onQuery(trimmed), SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [text, query, onQuery]);

  useEffect(() => {
    if (open) inputRef.current?.focus();
  }, [open]);

  const close = () => {
    setText("");
    onQuery("");
    setOpen(false);
    requestAnimationFrame(() => buttonRef.current?.focus());
  };

  if (!open) {
    return (
      <button
        ref={buttonRef}
        type="button"
        data-testid="wb-history-search-open"
        aria-label={t("history.search.open")}
        title={t("history.search.open")}
        onClick={() => setOpen(true)}
        className={`grid size-8 cursor-pointer place-items-center rounded-(--wb-radius-control) text-(--wb-ink-2) hover:bg-(--wb-hover) hover:text-(--wb-ink) ${FOCUS_RING}`}
      >
        <SearchIcon size={16} aria-hidden="true" />
      </button>
    );
  }

  return (
    <div
      role="search"
      className="flex min-w-0 items-center gap-1 rounded-(--wb-radius-control) border border-(--wb-line-strong) bg-(--wb-card) ps-2.5 pe-1 focus-within:ring-2 focus-within:ring-(--wb-accent)"
    >
      <SearchIcon
        size={15}
        aria-hidden="true"
        className="shrink-0 text-(--wb-ink-3)"
      />
      <label htmlFor={id} className="sr-only">
        {t("history.search.label")}
      </label>
      <input
        ref={inputRef}
        id={id}
        type="search"
        data-testid="wb-history-search"
        value={text}
        placeholder={t("history.search.placeholder")}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            close();
          } else if (e.key === "Enter") {
            onQuery(text.trim());
          }
        }}
        className="w-48 min-w-0 bg-transparent py-1.5 text-[13.5px] text-(--wb-ink) outline-none placeholder:text-(--wb-ink-3) [&::-webkit-search-cancel-button]:hidden"
      />
      <button
        type="button"
        data-testid="wb-history-search-close"
        aria-label={t("history.search.close")}
        title={t("history.search.close")}
        onClick={close}
        className={`grid size-6 shrink-0 cursor-pointer place-items-center rounded-[6px] text-(--wb-ink-3) hover:bg-(--wb-hover) hover:text-(--wb-ink) ${FOCUS_RING}`}
      >
        <X size={14} aria-hidden="true" />
      </button>
    </div>
  );
}
