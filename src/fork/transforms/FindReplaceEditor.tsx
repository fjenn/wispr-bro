// SPDX-License-Identifier: GPL-3.0-or-later
// OT2-D: exact "Find and replace" rows. They run as code, never through the
// local AI, so "a → 0" always does exactly that.
import { useTranslation } from "react-i18next";
import { ArrowRight, Plus, X } from "../ui/icons";
import type { LiteralRule } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button, Checkbox, FOCUS_RING } from "../ui";

export const MAX_RULES = 20;
export const MAX_RULE_CHARS = 200;

const FIELD = `min-w-0 flex-1 rounded-(--wb-radius-sm) border border-(--wb-line-strong) bg-(--wb-surface) px-3 py-1.5 text-[15px] select-text transition-colors duration-(--wb-dur-fast) hover:border-(--wb-ink-3) ${FOCUS_RING}`;

export function FindReplaceEditor({
  value,
  onChange,
}: {
  value: LiteralRule[];
  onChange: (rules: LiteralRule[]) => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const set = (index: number, patch: Partial<LiteralRule>) =>
    onChange(
      value.map((rule, at) => (at === index ? { ...rule, ...patch } : rule)),
    );

  return (
    <fieldset
      className="flex flex-col gap-3"
      data-testid="wb-transform-find-replace"
    >
      <legend className="text-sm font-semibold">
        {t("transforms.findReplace.title")}
      </legend>
      {value.map((rule, index) => (
        <div
          key={index}
          className="flex flex-wrap items-center gap-2"
          data-testid={`wb-transform-replace-row-${index}`}
        >
          <div className="flex min-w-0 flex-[1_1_320px] items-center gap-2">
            <input
              value={rule.find}
              maxLength={MAX_RULE_CHARS}
              placeholder={t("transforms.findReplace.findPlaceholder")}
              aria-label={t("transforms.findReplace.find", {
                number: index + 1,
              })}
              onChange={(event) =>
                set(index, { find: event.currentTarget.value })
              }
              className={FIELD}
              data-testid={`wb-transform-replace-find-${index}`}
            />
            <ArrowRight
              size={14}
              aria-hidden="true"
              className="shrink-0 text-(--wb-ink-3) rtl:-scale-x-100"
            />
            <input
              value={rule.replace}
              maxLength={MAX_RULE_CHARS}
              placeholder={t("transforms.findReplace.replacePlaceholder")}
              aria-label={t("transforms.findReplace.replace", {
                number: index + 1,
              })}
              onChange={(event) =>
                set(index, { replace: event.currentTarget.value })
              }
              className={FIELD}
              data-testid={`wb-transform-replace-with-${index}`}
            />
          </div>
          <label className="flex cursor-pointer items-center gap-2 text-xs text-(--wb-ink-2)">
            <Checkbox
              checked={rule.match_case}
              onChange={(event) =>
                set(index, { match_case: event.currentTarget.checked })
              }
              data-testid={`wb-transform-replace-case-${index}`}
            />
            {t("transforms.findReplace.matchCase")}
          </label>
          <button
            type="button"
            onClick={() => onChange(value.filter((_, at) => at !== index))}
            aria-label={t("transforms.findReplace.remove", {
              number: index + 1,
            })}
            title={t("transforms.findReplace.remove", { number: index + 1 })}
            className={`grid size-7 cursor-pointer place-items-center rounded-(--wb-radius-xs) text-(--wb-ink-2) transition-colors duration-(--wb-dur-fast) hover:bg-(--wb-fill-hover) hover:text-(--wb-ink) ${FOCUS_RING}`}
            data-testid={`wb-transform-replace-remove-${index}`}
          >
            <X size={14} aria-hidden="true" />
          </button>
        </div>
      ))}
      {value.length < MAX_RULES && (
        <div>
          <Button
            onClick={() =>
              onChange([...value, { find: "", replace: "", match_case: false }])
            }
            data-testid="wb-transform-replace-add"
          >
            <Plus size={14} aria-hidden="true" />
            {t("transforms.findReplace.add")}
          </Button>
        </div>
      )}
    </fieldset>
  );
}
