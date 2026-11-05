// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * "New!" / "Beta" tag beside a label (S-13, C-15; Design Phase 0: a warm
 * orange "New!" chip and a black "Beta" chip).
 *
 * A "new" tag disappears once the user has seen the thing it marks: pass
 * `seenId` and it hides when that id is in the fork setting `seen_new_tags`.
 * Marking an id as seen is the owner's job (`markNewTagSeen`), so a page can
 * decide what "seen" means (the Hub marks a page when it is opened).
 */
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettingsStore } from "../settings/store";

export type TagKind = "new" | "beta";

const STYLES: Record<TagKind, string> = {
  new: "bg-(--wb-tag-new-bg) text-(--wb-tag-new-ink)",
  beta: "bg-(--wb-tag-beta-bg) text-(--wb-tag-beta-ink)",
};

export interface TagProps {
  kind: TagKind;
  /** For `new`: hide once this id is in `seen_new_tags`. */
  seenId?: string;
  className?: string;
  /** `lg`: beside a page title or a tab (flowmatch), a rounded box. */
  size?: "sm" | "lg";
  /** `accent`: the purple variant for a tab label. */
  tone?: "default" | "accent";
}

/** `true` when a "new" tag with this id should still show. */
export function useNewTagVisible(seenId: string | undefined): boolean {
  const seen = useForkSettingsStore((s) => s.settings?.seen_new_tags);
  // Until the settings load, show nothing rather than flash a tag that the
  // user has already seen.
  if (!seenId) return true;
  if (!seen) return false;
  return !seen.includes(seenId);
}

/** Records that the "new" tag `seenId` was seen (idempotent). */
export async function markNewTagSeen(seenId: string): Promise<void> {
  const store = useForkSettingsStore.getState();
  const seen = store.settings?.seen_new_tags;
  if (!seen || seen.includes(seenId) || store.pending.seen_new_tags) return;
  await store.setSetting("seen_new_tags", [...seen, seenId]);
}

export function Tag({
  kind,
  seenId,
  className = "",
  size = "sm",
  tone = "default",
}: TagProps) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const visible = useNewTagVisible(kind === "new" ? seenId : undefined);
  if (!visible) return null;
  return (
    <span
      data-testid={`wb-tag-${kind}`}
      className={`inline-flex shrink-0 items-center leading-none ${
        size === "lg"
          ? "h-[26px] rounded-[7px] px-2 text-[14px] font-medium"
          : "h-5 rounded-full px-2 text-[11px] font-semibold"
      } ${tone === "accent" ? "bg-(--wb-accent-soft) text-(--wb-accent)" : STYLES[kind]} ${className}`}
    >
      {t(`ui.tag.${kind}`)}
    </span>
  );
}
