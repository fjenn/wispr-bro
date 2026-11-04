// SPDX-License-Identifier: GPL-3.0-or-later
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { Plus, Upload, X } from "../ui/icons";
import { commands } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { Button, FOCUS_RING } from "../ui";

export const MAX_EXAMPLES = 3;
export const MAX_EXAMPLE_CHARS = 1_500;

export function ExamplesEditor({
  value,
  onChange,
}: {
  value: string[];
  onChange: (examples: string[]) => void;
}) {
  const { t, i18n } = useTranslation(FORK_NAMESPACE);
  const [error, setError] = useState(false);
  const chars = new Intl.NumberFormat(i18n.language).format(MAX_EXAMPLE_CHARS);

  const load = async () => {
    setError(false);
    try {
      const path = await open({
        multiple: false,
        filters: [
          {
            name: t("transforms.examples.fileType"),
            extensions: ["txt", "md"],
          },
        ],
      });
      if (!path) return;
      const result = await commands.forkTransformReadExample(path);
      if (result.status === "ok") onChange([...value, result.data]);
      else setError(true);
    } catch {
      setError(true);
    }
  };

  return (
    <fieldset
      className="flex flex-col gap-3"
      data-testid="wb-transform-examples"
    >
      <legend className="text-sm font-semibold">
        {t("transforms.examples.title")}
      </legend>
      <p className="text-xs leading-5 text-(--wb-ink-2)">
        {t("transforms.examples.help", {
          count: MAX_EXAMPLES,
          max: MAX_EXAMPLES,
          chars,
        })}
      </p>
      {value.map((example, index) => (
        <div
          key={index}
          className="relative"
          data-testid={`wb-transform-example-${index}`}
        >
          <label htmlFor={`wb-example-${index}`} className="sr-only">
            {t("transforms.examples.label", { number: index + 1 })}
          </label>
          <textarea
            id={`wb-example-${index}`}
            value={example}
            maxLength={MAX_EXAMPLE_CHARS}
            rows={4}
            onChange={(event) =>
              onChange(
                value.map((current, at) =>
                  at === index ? event.currentTarget.value : current,
                ),
              )
            }
            className={`w-full resize-y rounded-(--wb-radius-sm) border border-(--wb-line-strong) bg-(--wb-panel) p-3 pe-10 text-sm select-text ${FOCUS_RING}`}
          />
          <span className="absolute bottom-2 end-2 text-[11px] text-(--wb-ink-3)">
            {t("transforms.examples.count", {
              count: example.length,
              max: MAX_EXAMPLE_CHARS,
            })}
          </span>
          <button
            type="button"
            onClick={() => onChange(value.filter((_, at) => at !== index))}
            aria-label={t("transforms.examples.remove", { number: index + 1 })}
            title={t("transforms.examples.remove", { number: index + 1 })}
            className={`absolute end-2 top-2 grid size-6 cursor-pointer place-items-center rounded-(--wb-radius-xs) bg-(--wb-surface) text-(--wb-ink-2) ${FOCUS_RING}`}
          >
            <X size={14} aria-hidden="true" />
          </button>
        </div>
      ))}
      {error && (
        <p role="alert" className="type-small text-(--wb-danger)">
          {t("transforms.examples.readError", { chars })}
        </p>
      )}
      {value.length < MAX_EXAMPLES && (
        <div className="flex flex-wrap gap-2">
          <Button
            onClick={() => onChange([...value, ""])}
            data-testid="wb-transform-example-add"
          >
            <Plus size={14} aria-hidden="true" />
            {t("transforms.examples.paste")}
          </Button>
          <Button
            onClick={() => void load()}
            data-testid="wb-transform-example-load"
          >
            <Upload size={14} aria-hidden="true" />
            {t("transforms.examples.load")}
          </Button>
        </div>
      )}
    </fieldset>
  );
}
