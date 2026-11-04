// SPDX-License-Identifier: GPL-3.0-or-later
import { useTranslation } from "react-i18next";
import type { TransformRules } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Checkbox } from "../ui";

export const RULE_KEYS = [
  "shorter",
  "clearer",
  "better_order",
  "headings_lists",
  "keep_tone",
] as const satisfies (keyof TransformRules)[];

export function RulesList({
  value,
  onChange,
}: {
  value: TransformRules;
  onChange: (rules: TransformRules) => void;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (
    <fieldset className="flex flex-col gap-2" data-testid="wb-transform-rules">
      <legend className="mb-1 text-sm font-semibold">
        {t("transforms.editor.rules")}
      </legend>
      {RULE_KEYS.map((key) => (
        <label
          key={key}
          className="flex cursor-pointer items-start gap-3 rounded-(--wb-radius-sm) px-2 py-2 hover:bg-(--wb-fill-hover)"
        >
          <Checkbox
            className="mt-px"
            checked={value[key]}
            onChange={(event) =>
              onChange({ ...value, [key]: event.currentTarget.checked })
            }
            data-testid={`wb-transform-rule-${key}`}
          />
          <span className="flex flex-col gap-0.5">
            <span className="text-sm font-medium">
              {t(`transforms.rules.${key}.title`)}
            </span>
            <span className="text-xs text-(--wb-ink-2)">
              {t(`transforms.rules.${key}.help`)}
            </span>
          </span>
        </label>
      ))}
    </fieldset>
  );
}
