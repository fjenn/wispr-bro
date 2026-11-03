// SPDX-License-Identifier: GPL-3.0-or-later
import { useTranslation } from "react-i18next";
import { CornerDownLeft, Globe } from "../ui/icons";
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

  return (
    <li>
      <Menu
        label={t("style.apps.menuLabel", { app: site.name })}
        groups={groups}
        testId="wb-style-site-menu"
        trigger={
          <button
            type="button"
            data-testid="wb-style-site"
            data-host={site.host}
            data-auto-send={site.auto_send}
            aria-label={t("style.apps.chipLabel", { app: label })}
            className={`flex h-9 cursor-pointer items-center gap-2 rounded-(--wb-radius-sm) border border-(--wb-line-2) bg-(--wb-surface) px-3 text-[13px] text-(--wb-ink) transition-colors duration-(--wb-dur-fast) hover:bg-(--wb-surface-tint) ${FOCUS_RING}`}
          >
            <Globe size={16} aria-hidden="true" className="text-(--wb-ink-2)" />
            <span>{site.name}</span>{" "}
            <span className="max-w-48 truncate text-[12px] text-(--wb-ink-3)">
              {site.host}
            </span>
            {site.auto_send && (
              <CornerDownLeft
                size={14}
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
