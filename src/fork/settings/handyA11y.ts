// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Accessible names for Handy's settings rows hosted in Settings (S-37, R5
 * §5.5). Handy's components are not edited (R5 §4.2) and its
 * `translation.json` gets no new keys, so the few controls Handy leaves
 * unnamed are named here, from the row they sit in:
 *
 * - an icon-only button (Handy's `ResetButton`) → "Reset <row title>";
 * - a text, number or react-select field with no label → the row's title;
 * - a text field that reports `aria-expanded` without a role (the language
 *   search over its drawer) → `role="combobox"`, which that state needs.
 *
 * A row is the nearest ancestor with an `h3` title (Handy's
 * `SettingContainer`). Only `[data-handy-content]` subtrees are touched,
 * and only attributes Handy does not set itself, so React never fights the
 * change; a MutationObserver repeats it when Handy re-renders a row.
 */
import { useEffect, type RefObject } from "react";
import { useTranslation } from "react-i18next";
import { FORK_NAMESPACE } from "../i18n";

const NAMED = ["aria-label", "aria-labelledby", "title"];

function named(el: Element): boolean {
  if (NAMED.some((a) => el.hasAttribute(a))) return true;
  if (el instanceof HTMLInputElement && (el.labels?.length ?? 0) > 0) {
    return true;
  }
  return (el.textContent ?? "").trim().length > 0 && el.tagName === "BUTTON";
}

/** The title of the setting row `el` is in, if any. */
export function rowTitle(el: Element, root: Element): string | null {
  for (let node = el.parentElement; node; node = node.parentElement) {
    const h3 = node.querySelector("h3");
    if (h3) return (h3.textContent ?? "").trim() || null;
    if (node === root) break;
  }
  return null;
}

/** Names the unnamed controls under `root`. Returns how many it changed. */
export function repairHandyControls(
  root: ParentNode,
  resetLabel: (setting: string) => string,
): number {
  let changed = 0;
  for (const host of root.querySelectorAll("[data-handy-content]")) {
    for (const button of host.querySelectorAll("button")) {
      if (named(button)) continue;
      const title = rowTitle(button, host);
      if (!title) continue;
      button.setAttribute("aria-label", resetLabel(title));
      changed++;
    }
    for (const field of host.querySelectorAll("input, textarea, select")) {
      const type = field.getAttribute("type");
      if (type === "checkbox" || type === "radio" || type === "hidden") {
        continue;
      }
      if (field.hasAttribute("aria-expanded") && !field.hasAttribute("role")) {
        field.setAttribute("role", "combobox");
        changed++;
      }
      if (named(field)) continue;
      const title = rowTitle(field, host);
      if (!title) continue;
      field.setAttribute("aria-label", title);
      changed++;
    }
  }
  return changed;
}

/** Keeps Handy's rows under `ref` named while the modal is open. */
export function useHandyA11y(ref: RefObject<HTMLElement | null>): void {
  const { t } = useTranslation(FORK_NAMESPACE);
  useEffect(() => {
    const root = ref.current;
    if (!root) return;
    const label = (setting: string) =>
      t("settingsModal.resetSetting", { setting });
    repairHandyControls(root, label);
    const observer = new MutationObserver(() =>
      repairHandyControls(root, label),
    );
    observer.observe(root, { childList: true, subtree: true });
    return () => observer.disconnect();
  }, [ref, t]);
}
