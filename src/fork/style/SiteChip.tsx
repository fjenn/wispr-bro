// SPDX-License-Identifier: GPL-3.0-or-later
import { useTranslation } from "react-i18next";
import { CornerDownLeft } from "../ui/icons";
import { toast } from "sonner";
import { commands, type StyleGroup, type StyleSite } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { FOCUS_RING, Menu, type MenuGroup } from "../ui";
import { STYLE_GROUPS, useStyleStore } from "./useStyle";

export function SiteChip({
  site,
  group,
}: {
  site: StyleSite;
  group: StyleGroup;
}) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const change = useStyleStore((s) => s.change);
  const groupLabel = (g: StyleGroup) => t(`style.groups.${g}.label`);

  const moveTo = (to: StyleGroup | null, message: string) => {
    const before = site.user_group;
    void change(() => commands.forkStyleSetSiteGroup(site.host, to)).then(
      (ok) => {
        if (!ok) {
          toast.error(t("style.apps.changeFailed"));
          return;
        }
        toast(message, {
          action: {
            label: t("style.apps.undo"),
            onClick: () =>
              void change(() =>
                commands.forkStyleSetSiteGroup(site.host, before),
              ),
          },
        });
      },
    );
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
            t("style.apps.moved", { app: site.name, group: groupLabel(g) }),
          ),
      })),
    },
  ];
  const extra = [];
  if (site.user_group && site.user_group !== site.builtin_group) {
    extra.push({
      id: "default",
      label: t("style.apps.useDefault", {
        group: groupLabel(site.builtin_group),
      }),
      onSelect: () =>
        moveTo(
          null,
          t("style.apps.moved", {
            app: site.name,
            group: groupLabel(site.builtin_group),
          }),
        ),
    });
  }
  if (group !== "other") {
    extra.push({
      id: "remove",
      label: t("style.apps.remove", { group: groupLabel(group) }),
      onSelect: () => {
        const before = site.user_group;
        void change(() => commands.forkStyleRemoveSite(site.host)).then(
          (ok) => {
            if (!ok) {
              toast.error(t("style.apps.changeFailed"));
              return;
            }
            toast(t("style.apps.removed", { app: site.name }), {
              action: {
                label: t("style.apps.undo"),
                onClick: () =>
                  void change(() =>
                    commands.forkStyleSetSiteGroup(site.host, before),
                  ),
              },
            });
          },
        );
      },
    });
  }
  if (extra.length) groups.push({ items: extra });
  groups.push({
    items: [
      {
        id: "auto-send",
        label: t("style.apps.autoSend"),
        checked: site.auto_send,
        onSelect: () =>
          void change(() =>
            commands.forkStyleSetSiteAutoSend(site.host, !site.auto_send),
          ).then((ok) => {
            if (!ok) toast.error(t("style.apps.changeFailed"));
          }),
      },
    ],
  });

  const label = `${site.name} ${site.host}${
    site.auto_send ? `, ${t("style.apps.autoSendOn")}` : ""
  }`;

  const letter = [...site.name.replace(/^www\./i, "").trim()][0] ?? "";
  const trigger = (
    <button
      type="button"
      data-testid="wb-style-site"
      data-host={site.host}
      data-auto-send={site.auto_send}
      title={site.host}
      aria-label={t("style.apps.chipLabel", { app: label })}
      className={`relative grid size-14 cursor-pointer place-items-center rounded-full border border-white/30 bg-white/20 shadow-[0_2px_10px_rgba(0,0,0,0.25)] backdrop-blur-sm transition-transform duration-(--wb-dur-fast) hover:z-10 hover:scale-105 focus-visible:z-10 motion-reduce:transition-none ${FOCUS_RING}`}
    >
      <span
        aria-hidden="true"
        data-testid="wb-style-site-monogram"
        data-letter={letter.toLocaleUpperCase()}
        className="grid size-9 place-items-center rounded-[9px] bg-(--wb-cream) text-[17px] font-semibold text-(--wb-on-cream) before:content-[attr(data-letter)]"
      />
      {site.auto_send && (
        <span
          aria-hidden="true"
          className="absolute -end-0.5 -bottom-0.5 grid size-4 place-items-center rounded-full bg-(--wb-cream) text-(--wb-on-cream)"
        >
          <CornerDownLeft size={12} />
        </span>
      )}
    </button>
  );

  return (
    <li className="relative -ms-3 first:ms-0">
      <Menu
        label={t("style.apps.menuLabel", { app: site.name })}
        groups={groups}
        testId="wb-style-site-menu"
        trigger={trigger}
      />
    </li>
  );
}
