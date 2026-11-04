// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * One style group's tab (S-19a/b): the apps and sites that use it (icons;
 * "+" adds an app; each chip's menu moves it to another group, takes it out,
 * returns it to its default group and turns "Press Enter after dictating"
 * on or off; a few headline websites plus the ones the user added or
 * dictated on (`shownSites`; the rest of the built-in list matches
 * silently; "+" adds a website by its domain and its dialog offers the
 * websites dictated on with no group yet). The presets come right after the
 * apps, before the websites (declutter pass: the choice people come for
 * first). Other also covers every app not in a group.
 */
import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { CornerDownLeft, Plus } from "../ui/icons";
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
import { FOCUS_RING, Hero, Menu, SectionLabel, type MenuGroup } from "../ui";
import { photos } from "@/assets/fork/art";
import { AppIcon } from "./AppIcon";
import { AppPicker } from "./AppPicker";
import { PresetCard } from "./PresetCard";
import { SiteChip } from "./SiteChip";
import { AddSiteDialog } from "./AddSiteDialog";
import { EXAMPLES, PRESETS } from "./presets";
import { shownSites } from "./shownSites";
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
    <li className="relative -ms-2 first:ms-0">
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
            title={
              app.ai_chat ? `${app.name} · ${t("style.apps.aiChat")}` : app.name
            }
            className={`relative grid size-11 cursor-pointer place-items-center rounded-full border border-white/25 bg-white/15 shadow-[0_2px_8px_rgba(0,0,0,0.25)] transition-transform duration-(--wb-dur-fast) hover:z-10 hover:scale-105 focus-visible:z-10 motion-reduce:transition-none ${FOCUS_RING} ${app.installed ? "" : "opacity-70"}`}
          >
            <AppIcon bundleId={app.bundle_id} name={app.name} size={30} />
            <span className="sr-only">{app.name}</span>
            {app.auto_send && (
              <span
                aria-hidden="true"
                className="absolute -end-0.5 -bottom-0.5 grid size-4 place-items-center rounded-full bg-(--wb-cream) text-(--wb-on-cream)"
              >
                <CornerDownLeft size={12} />
              </span>
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
  const sites = useStyleStore((s) => s.sites);
  const status = useStyleStore((s) => s.status);
  const change = useStyleStore((s) => s.change);
  const { settings, pending, setSetting } = useForkSettings();
  const [picking, setPicking] = useState(false);
  const [addingSite, setAddingSite] = useState(false);
  const name = useId();
  const appsHeading = useId();
  const sitesHeading = useId();
  const presetsHeading = useId();

  const members = apps.filter((a) => a.group === group);
  const visibleSites = shownSites(sites, group, apps);
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

  const addSite = async (host: string, to: StyleGroup) => {
    const ok = await change(() => commands.forkStyleSetSiteGroup(host, to));
    if (ok) {
      toast.success(
        t("style.apps.added", {
          app: host,
          group: t(`style.groups.${to}.label`),
        }),
      );
    } else {
      toast.error(t("style.apps.changeFailed"));
    }
    return ok;
  };

  return (
    <div
      className="flex flex-col gap-6 pt-6"
      data-testid={`wb-style-group-${group}`}
    >
      <Hero
        id={`style-${group}`}
        image={photos.style.src}
        title={groupName}
        body={t(`style.groups.${group}.description`)}
        aside={
          <section aria-labelledby={appsHeading}>
            <h2 id={appsHeading} className="sr-only">
              {t("style.apps.heading")}
            </h2>
            <ul
              className="flex flex-wrap items-center justify-end gap-y-2"
              aria-label={t("style.apps.listLabel", { group: groupName })}
              data-testid="wb-style-apps"
            >
              {members.map((app) => (
                <AppChip key={app.bundle_id} app={app} group={group} />
              ))}
              <li className="relative -ms-2 first:ms-0">
                <button
                  type="button"
                  onClick={() => setPicking(true)}
                  data-testid="wb-style-add-app"
                  aria-label={t("style.apps.addShort")}
                  title={t("style.apps.addShort")}
                  className={`grid size-11 cursor-pointer place-items-center rounded-full border border-white/25 bg-white/15 text-(--wb-on-hero) shadow-[0_2px_8px_rgba(0,0,0,0.25)] transition-colors duration-(--wb-dur-fast) hover:bg-white/25 ${FOCUS_RING}`}
                >
                  <Plus size={20} aria-hidden="true" />
                </button>
              </li>
            </ul>
          </section>
        }
      />

      {(group === "other" || status === "unavailable") && (
        <div className="-mt-2 flex flex-col gap-1">
          {group === "other" && (
            <p className="text-[13px] text-(--wb-ink-2)">
              {t("style.apps.allOthers")}
            </p>
          )}
          {status === "unavailable" && (
            <p role="alert" className="text-[13px] text-(--wb-ink-2)">
              {t("style.apps.unavailable")}
            </p>
          )}
        </div>
      )}

      <section aria-labelledby={presetsHeading} className="flex flex-col gap-4">
        <h2 id={presetsHeading} className="sr-only">
          {t("style.presetsHeading")}
        </h2>
        <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
          <span className="type-micro text-(--wb-ink-3)">
            {t("style.before")}
          </span>
          <p
            lang="en"
            dir="ltr"
            data-testid="wb-style-before"
            className="text-start text-[15px] text-(--wb-ink-2) italic"
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
          <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
            {PRESETS[group].map((preset, index) => (
              <PresetCard
                key={preset}
                preset={preset}
                name={name}
                index={index}
                checked={current === preset}
                example={example.after[preset] ?? ""}
                onSelect={() => choose(preset)}
              />
            ))}
          </div>
        </fieldset>
        <p
          className="text-[13px] text-(--wb-ink-3)"
          data-testid="wb-style-english-note"
        >
          {t("style.englishOnly")}
        </p>
      </section>

      <section
        aria-labelledby={sitesHeading}
        className="flex flex-col gap-2.5"
        data-testid="wb-style-sites-section"
      >
        <SectionLabel id={sitesHeading}>
          {t("style.sites.heading")}
        </SectionLabel>
        <ul
          className="flex flex-wrap items-center gap-2"
          aria-label={t("style.sites.listLabel", { group: groupName })}
          data-testid="wb-style-sites"
        >
          {visibleSites.map((site) => (
            <SiteChip key={site.host} site={site} group={group} />
          ))}
          <li>
            <button
              type="button"
              onClick={() => setAddingSite(true)}
              data-testid="wb-style-add-site"
              className={`flex h-9 cursor-pointer items-center gap-1.5 rounded-(--wb-radius-sm) border border-dashed border-(--wb-line-strong) px-3 text-[13px] text-(--wb-ink-2) transition-colors duration-(--wb-dur-fast) hover:bg-(--wb-surface-tint) hover:text-(--wb-ink) ${FOCUS_RING}`}
            >
              <Plus size={16} aria-hidden="true" />
              {t("style.sites.addShort")}
            </button>
          </li>
        </ul>
      </section>

      <AddSiteDialog
        open={addingSite}
        onOpenChange={setAddingSite}
        group={group}
        sites={sites}
        onAdd={addSite}
      />
      <AppPicker
        open={picking}
        onOpenChange={setPicking}
        group={group}
        onPick={add}
      />
    </div>
  );
}
