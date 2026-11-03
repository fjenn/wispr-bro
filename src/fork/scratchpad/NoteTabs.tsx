// SPDX-License-Identifier: GPL-3.0-or-later
import { useTranslation } from "react-i18next";
import type { Note } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { FOCUS_RING } from "../ui";

export function NoteTabs({
  notes,
  selected,
  onSelect,
}: {
  notes: Note[];
  selected: number;
  onSelect: (id: number) => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <div
      role="tablist"
      aria-label={t("scratchpad.tabs")}
      className="flex min-w-0 items-center gap-0.5 overflow-x-auto [scrollbar-width:none]"
      data-testid="wb-note-tabs"
    >
      {notes.map((note) => (
        <button
          key={note.id}
          type="button"
          role="tab"
          aria-selected={selected === note.id}
          aria-controls="wb-note-editor-panel"
          tabIndex={selected === note.id ? 0 : -1}
          onClick={() => onSelect(note.id)}
          onKeyDown={(event) => {
            const index = notes.findIndex((item) => item.id === note.id);
            const next =
              event.key === "ArrowRight"
                ? (index + 1) % notes.length
                : event.key === "ArrowLeft"
                  ? (index - 1 + notes.length) % notes.length
                  : -1;
            if (next >= 0) {
              event.preventDefault();
              onSelect(notes[next].id);
              const target = event.currentTarget.parentElement?.children[next];
              if (target instanceof HTMLElement) target.focus();
            }
          }}
          className={`h-7 max-w-40 min-w-0 shrink-0 cursor-pointer truncate rounded-(--wb-radius-sm) px-2.5 text-[13px] whitespace-nowrap transition-colors duration-(--wb-dur-fast) ${selected === note.id ? "bg-(--wb-fill-active) font-medium text-(--wb-ink)" : "text-(--wb-ink-2) hover:bg-(--wb-fill-hover) hover:text-(--wb-ink)"} ${FOCUS_RING} focus-visible:-outline-offset-2`}
          data-testid={`wb-note-tab-${note.id}`}
        >
          {note.title || t("scratchpad.untitled")}
        </button>
      ))}
    </div>
  );
}
