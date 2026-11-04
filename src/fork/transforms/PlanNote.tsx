// SPDX-License-Identifier: GPL-3.0-or-later
// OT2-D: tells the user which parts of a transform run exactly, without AI.
import { useTranslation } from "react-i18next";
import type { LiteralEdit, TransformPlan } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Badge } from "../ui";
import { Check } from "../ui/icons";

export function useEditLabel() {
  const { t } = useTranslation(FORK_NAMESPACE);
  return (edit: LiteralEdit): string => {
    if (edit.kind === "case") return t(`transforms.plan.${edit.mode}`);
    const find =
      edit.find.kind === "text"
        ? `“${edit.find.value}”`
        : t(`transforms.plan.${edit.find.value}`);
    return edit.with === ""
      ? t("transforms.plan.remove", { find })
      : t("transforms.plan.replace", { find, with: `“${edit.with}”` });
  };
}

export function PlanNote({ plan }: { plan: TransformPlan | null }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const label = useEditLabel();
  if (!plan || plan.edits.length === 0) return null;
  return (
    <div
      className="flex flex-col gap-2 rounded-(--wb-radius-sm) bg-(--wb-surface-tint) px-3 py-2.5"
      role="status"
      data-testid="wb-transform-plan"
      data-exact={plan.uses_model ? "false" : "true"}
    >
      <p className="type-small m-0 flex items-center gap-1.5 font-medium text-(--wb-ink)">
        <Check size={14} aria-hidden="true" />
        {plan.uses_model
          ? t("transforms.plan.mixed")
          : t("transforms.plan.exact")}
      </p>
      <ul className="m-0 flex list-none flex-wrap gap-1.5 p-0">
        {plan.edits.map((edit, index) => (
          <li key={index}>
            <Badge tone="neutral">
              <bdi>{label(edit)}</bdi>
            </Badge>
          </li>
        ))}
      </ul>
    </div>
  );
}
