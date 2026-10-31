// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * One style group's tab (S-19a, R2 D-18 .. D-25, D-207, D-61): the apps
 * that use it (icons; "+" adds one; each app's menu moves it to another
 * group, takes it out, returns it to its default group and turns "Press
 * Enter after dictating" on or off), then the group's three presets as
 * before/after cards. Other also covers every app not in a group.
 */
import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { CornerDownLeft, Plus } from "lucide-react";
import { toast } from "sonner";
import {
  commands,
  type StyleApp,
  type StyleGroup,
  type StylePickerApp,
  type StylePreset,
} from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { useForkSettings } from "../settings/useForkSettings";
import { Button, FOCUS_RING, Menu, type MenuGroup } from "../ui";
import { AppIcon } from "./AppIcon";
import { AppPicker } from "./AppPicker";
import { PresetCard } from "./PresetCard";
import { EXAMPLES, PRESETS } from "./presets";
import { STYLE_GROUPS, useStyleStore } from "./useStyle";

function AppChip({ app, group }: { app: StyleApp; group: StyleGroup }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const change = useStyleStore((s) => s.change);
  const groupLabel = (g: StyleGroup) => t(`style.groups.${g}.label`);

  const moveTo = (to: StyleGroup | null, message: string) => {
    const before = app.user_group;
    void change(() =>
      commands.forkStyleSetAppGroup(app.bundle_id, app.name, to),
    ).then((ok) => {
      if (!ok) {
        toast.error(t("style.apps.changeFailed"));
        return;
      }
      toast(message, {
        action: {
          label: t("style.apps.undo"),
          onClick: () =>
            void change(() =>
              commands.forkStyleSetAppGroup(app.bundle_id, app.name, before),
            ),
        },
      });
    });
  };

  const groups: MenuGroup[] = [
    {
      label: t("style.apps.moveTo"),
      items: STYLE_GROUPS.filter((g) => g !== group).map((g) => ({
        id: `move-${g}`,
        label: groupLabel(g),
        onSelect: () =>
          moveTo(
            g,
            t("style.apps.moved", { app: app.name, group: groupLabel(g) }),
          ),
      })),
    },
  ];
  const extra = [];
  if (app.user_group && app.user_group !== app.builtin_group) {
    extra.push({
      id: "default",
      label: t("style.apps.useDefault", {
        group: groupLabel(app.builtin_group),
      }),
      onSelect: () =>
        moveTo(
          null,
          t("style.apps.moved", {
            app: app.name,
            group: groupLabel(app.builtin_group),
          }),
        ),
    });
  }
  if (group !== "other") {
    extra.push({
      id: "remove",
      label: t("style.apps.remove", { group: groupLabel(group) }),
      onSelect: () => {
        const before = app.user_group;
        void change(() =>
          commands.forkStyleRemoveApp(app.bundle_id, app.name),
        ).then((ok) => {
          if (!ok) {
            toast.error(t("style.apps.changeFailed"));
            return;
          }
          toast(t("style.apps.removed", { app: app.name }), {
            action: {
              label: t("style.apps.undo"),
              onClick: () =>
                void change(() =>
                  commands.forkStyleSetAppGroup(
                    app.bundle_id,
                    app.name,
                    before,
                  ),
                ),
            },
          });
        });
      },
    });
  }
  if (extra.length) groups.push({ items: extra });
  groups.push({
    items: [
      {
        id: "auto-send",
        label: t("style.apps.autoSend"),
        checked: app.auto_send,
        onSelect: () =>
          void change(() =>
            commands.forkStyleSetAutoSend(
              app.bundle_id,
              app.name,
              !app.auto_send,
            ),
          ).then((ok) => {
            if (!ok) toast.error(t("style.apps.changeFailed"));
          }),
      },
    ],
  });

  const label = [
    app.name,
    app.auto_send ? t("style.apps.autoSendOn") : null,
    !app.installed ? t("style.apps.notInstalled") : null,
  ]
    .filter(Boolean)
    .join(", ");

  return (
    <li>
      <Menu
        label={t("style.apps.menuLabel", { app: app.name })}
        groups={groups}
        testId="wb-style-app-menu"
        trigger={
          <button
            type="button"
            data-testid="wb-style-app"
            data-bundle-id={app.bundle_id}
            data-auto-send={app.auto_send}
            aria-label={t("style.apps.chipLabel", { app: label })}
            title={app.ai_chat ? t("style.apps.aiChat") : undefined}
            className={`flex cursor-pointer items-center gap-2 rounded-full border border-(--wb-line) bg-(--wb-card) py-1 ps-1 pe-3 text-[13px] text-(--wb-ink) hover:bg-(--wb-hover) ${FOCUS_RING} ${app.installed ? "" : "opacity-70"}`}
          >
            <AppIcon bundleId={app.bundle_id} name={app.name} size={24} />
            <span className="max-w-40 truncate">{app.name}</span>
            {app.auto_send && (
              <CornerDownLeft
                size={13}
                aria-hidden="true"
                className="text-(--wb-ink-2)"
              />
            )}
          </button>
        }
      />
    </li>
  );
}

export function GroupTab({ group }: { group: StyleGroup }) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const apps = useStyleStore((s) => s.apps);
  const status = useStyleStore((s) => s.status);
  const change = useStyleStore((s) => s.change);
  const { settings, pending, setSetting } = useForkSettings();
  const [picking, setPicking] = useState(false);
  const name = useId();
  const appsHeading = useId();
  const presetsHeading = useId();

  const members = apps.filter((a) => a.group === group);
  const groupName = t(`style.groups.${group}.label`);
  const current = settings?.styles[group];
  const example = EXAMPLES[group];

  const choose = (preset: StylePreset) => {
    if (!settings) return;
    void setSetting("styles", { ...settings.styles, [group]: preset });
  };

  const add = async (app: StylePickerApp) => {
    const ok = await change(() =>
      commands.forkStyleSetAppGroup(app.bundle_id, app.name, group),
    );
    if (ok) {
      toast.success(t("style.apps.added", { app: app.name, group: groupName }));
    } else {
      toast.error(t("style.apps.changeFailed"));
    }
  };

  return (
    <div
      className="flex flex-col gap-6 pt-5"
      data-testid={`wb-style-group-${group}`}
    >
      <p className="text-[13.5px] text-(--wb-ink-2)">
        {t(`style.groups.${group}.description`)}
      </p>

      <section aria-labelledby={appsHeading} className="flex flex-col gap-2.5">
        <h2
          id={appsHeading}
          className="text-[13px] font-semibold text-(--wb-ink)"
        >
          {t("style.apps.heading")}
        </h2>
        <ul
          className="flex flex-wrap items-center gap-2"
          aria-label={t("style.apps.listLabel", { group: groupName })}
          data-testid="wb-style-apps"
        >
          {members.map((app) => (
            <AppChip key={app.bundle_id} app={app} group={group} />
          ))}
          <li>
            <Button
              variant="secondary"
              onClick={() => setPicking(true)}
              data-testid="wb-style-add-app"
              className="rounded-full! px-2.5! py-1.5!"
            >
              <Plus size={15} aria-hidden="true" />
              <span>{t("style.apps.addShort")}</span>
            </Button>
          </li>
        </ul>
        {group === "other" && (
          <p className="text-[12.5px] text-(--wb-ink-2)">
            {t("style.apps.allOthers")}
          </p>
        )}
        {status === "ready" && members.length === 0 && group !== "other" && (
          <p
            className="text-[12.5px] text-(--wb-ink-2)"
            data-testid="wb-style-apps-empty"
          >
            {t("style.apps.empty")}
          </p>
        )}
        {status === "unavailable" && (
          <p role="alert" className="text-[12.5px] text-(--wb-ink-2)">
            {t("style.apps.unavailable")}
          </p>
        )}
      </section>

      <section aria-labelledby={presetsHeading} className="flex flex-col gap-3">
        <h2
          id={presetsHeading}
          className="text-[13px] font-semibold text-(--wb-ink)"
        >
          {t("style.presetsHeading")}
        </h2>
        <div className="flex flex-col gap-1">
          <span className="text-[12px] font-medium text-(--wb-ink-2)">
            {t("style.before")}
          </span>
          <p
            lang="en"
            dir="ltr"
            data-testid="wb-style-before"
            className="text-start text-[13px] text-(--wb-ink-2) italic"
          >
            {example.before}
          </p>
        </div>
        <fieldset
          disabled={!settings || !!pending.styles}
          aria-busy={!!pending.styles}
        >
          <legend className="sr-only">
            {t("style.presetsLegend", { group: groupName })}
          </legend>
          <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
            {PRESETS[group].map((preset) => (
              <PresetCard
                key={preset}
                preset={preset}
                name={name}
                checked={current === preset}
                example={example.after[preset] ?? ""}
                onSelect={() => choose(preset)}
              />
            ))}
          </div>
        </fieldset>
        <p
          className="text-[12.5px] text-(--wb-ink-2)"
          data-testid="wb-style-english-note"
        >
          {t("style.englishOnly")}
        </p>
      </section>

      <AppPicker
        open={picking}
        onOpenChange={setPicking}
        group={group}
        onPick={add}
      />
    </div>
  );
}
