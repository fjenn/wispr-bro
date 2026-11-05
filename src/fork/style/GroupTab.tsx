// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * One style group's tab (S-19a/b; flowmatch: Wispr Flow's structure). One
 * banner with the apps and websites that use the group as round chips
 * (each chip's menu moves it to another group, takes it out, returns it to
 * its default group and turns "Press Return after dictating" on or off;
 * websites are the few `shownSites` as our own monogram chips) and a "+"
 * that adds an app or a website (the website dialog offers the sites
 * dictated on with no group yet); then the group's three presets as cards.
 * Nothing else is on the tab. Other also covers every app not in a group.
 */
import { useEffect, useId, useRef, useState } from "react";
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
import { FOCUS_RING, Hero, Menu, type MenuGroup } from "../ui";
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
    <li className="relative -ms-3 first:ms-0">
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
            className={`relative grid size-14 cursor-pointer place-items-center rounded-full border border-white/30 bg-white/20 shadow-[0_2px_10px_rgba(0,0,0,0.25)] backdrop-blur-sm transition-transform duration-(--wb-dur-fast) hover:z-10 hover:scale-105 focus-visible:z-10 motion-reduce:transition-none ${FOCUS_RING} ${app.installed ? "" : "opacity-70"}`}
          >
            <AppIcon bundleId={app.bundle_id} name={app.name} size={36} />
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
  const addRef = useRef<HTMLButtonElement>(null);
  const [initial, setInitial] = useState("A");
  useEffect(() => {
    let active = true;
    void commands
      .forkAccountName()
      .then((name) => {
        const first = name ? [...name.trim()][0] : undefined;
        if (active && first) setInitial(first.toLocaleUpperCase());
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);
  const afterMenu = useRef<(() => void) | null>(null);
  const name = useId();
  const appsHeading = useId();
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
        title={t(`style.groups.${group}.bannerTitle`)}
        body={
          group === "other"
            ? `${t("style.apps.allOthers")} ${t("style.englishOnly")}`
            : t("style.englishOnly")
        }
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
              {visibleSites.map((site) => (
                <SiteChip key={site.host} site={site} group={group} />
              ))}
              <li className="relative -ms-3 first:ms-0">
                <Menu
                  label={t("style.addMenu")}
                  testId="wb-style-add-menu"
                  align="end"
                  groups={[
                    {
                      items: [
                        {
                          id: "style-add-app",
                          label: t("style.apps.addShort"),
                          onSelect: () => {
                            afterMenu.current = () => setPicking(true);
                          },
                        },
                        {
                          id: "style-add-site",
                          label: t("style.sites.addShort"),
                          onSelect: () => {
                            afterMenu.current = () => setAddingSite(true);
                          },
                        },
                      ],
                    },
                  ]}
                  onCloseAutoFocus={(e) => {
                    // Focus goes back to "+" first, so the dialog returns
                    // it there when it closes.
                    const then = afterMenu.current;
                    if (!then) return;
                    afterMenu.current = null;
                    e.preventDefault();
                    addRef.current?.focus();
                    then();
                  }}
                  trigger={
                    <button
                      ref={addRef}
                      type="button"
                      data-testid="wb-style-add"
                      aria-label={t("style.addMenu")}
                      title={t("style.addMenu")}
                      className={`grid size-14 cursor-pointer place-items-center rounded-full border border-white/30 bg-white/20 text-(--wb-on-hero) shadow-[0_2px_10px_rgba(0,0,0,0.25)] backdrop-blur-sm transition-colors duration-(--wb-dur-fast) hover:bg-white/25 ${FOCUS_RING}`}
                    >
                      <Plus size={24} aria-hidden="true" />
                    </button>
                  }
                />
              </li>
            </ul>
          </section>
        }
      />

      {status === "unavailable" && (
        <p role="alert" className="-mt-2 text-[13px] text-(--wb-ink-2)">
          {t("style.apps.unavailable")}
        </p>
      )}

      <section aria-labelledby={presetsHeading}>
        <h2 id={presetsHeading} className="sr-only">
          {t("style.presetsHeading")}
        </h2>
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
                initial={initial}
                onSelect={() => choose(preset)}
              />
            ))}
          </div>
        </fieldset>
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
