// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * "Add an app" to a style group (S-19a, R2 D-20): the apps installed on
 * this Mac (`fork_style_installed_apps`), searchable, each with its icon
 * and the group it is in now. Picking one moves it into the group. Apps
 * already in the group are listed but cannot be picked again.
 */
import { useEffect, useId, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Loader2 } from "../ui/icons";
import { Dialog } from "../ui/Dialog";
import { commands, type StyleGroup, type StylePickerApp } from "../bindings";
import { FORK_NAMESPACE } from "../i18n";
import { INPUT_CLASS } from "../dictionary/AddWordDialog";
import { Button, FOCUS_RING } from "../ui";
import { AppIcon } from "./AppIcon";

export interface AppPickerProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  group: StyleGroup;
  /** Adds the app to `group`; resolves when done. */
  onPick: (app: StylePickerApp) => Promise<void>;
}

const fold = (s: string) =>
  s.normalize("NFD").replace(/\p{M}/gu, "").toLocaleLowerCase();

export function AppPicker({
  open,
  onOpenChange,
  group,
  onPick,
}: AppPickerProps) {
  const { t } = useTranslation(FORK_NAMESPACE);
  const [apps, setApps] = useState<StylePickerApp[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const searchId = useId();
  const listId = useId();

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setBusy(null);
    setFailed(false);
    setApps(null);
    let live = true;
    commands
      .forkStyleInstalledApps()
      .then((r) => {
        if (!live) return;
        if (r.status === "ok") setApps(r.data);
        else setFailed(true);
      })
      .catch(() => live && setFailed(true));
    return () => {
      live = false;
    };
  }, [open]);

  const shown = useMemo(() => {
    const q = fold(query.trim());
    return (apps ?? []).filter(
      (a) => !q || fold(a.name).includes(q) || fold(a.bundle_id).includes(q),
    );
  }, [apps, query]);

  const pick = async (app: StylePickerApp) => {
    setBusy(app.bundle_id);
    try {
      await onPick(app);
      onOpenChange(false);
    } finally {
      setBusy(null);
    }
  };

  const groupName = t(`style.groups.${group}.label`);

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !busy && onOpenChange(o)}
      title={t("style.picker.title", { group: groupName })}
      description={t("style.picker.description", { group: groupName })}
      closeLabel={t("style.picker.close")}
      initialFocusRef={searchRef as React.RefObject<HTMLElement>}
      className="max-w-lg!"
      contentFades={false}
      footer={
        <Button onClick={() => onOpenChange(false)} disabled={!!busy}>
          {t("style.picker.cancel")}
        </Button>
      }
    >
      <div className="flex flex-col gap-3" data-testid="wb-style-picker">
        <label htmlFor={searchId} className="sr-only">
          {t("style.picker.search")}
        </label>
        <input
          id={searchId}
          ref={searchRef}
          type="search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={t("style.picker.search")}
          aria-controls={listId}
          data-testid="wb-style-picker-search"
          className={INPUT_CLASS}
        />
        {apps === null && !failed && (
          <p
            className="flex items-center gap-2 text-[13px] text-(--wb-ink-2)"
            role="status"
          >
            <Loader2 size={14} className="animate-spin" aria-hidden />
            {t("style.picker.loading")}
          </p>
        )}
        {failed && (
          <p role="alert" className="text-[13px] text-(--wb-ink-2)">
            {t("style.picker.failed")}
          </p>
        )}
        {apps !== null && shown.length === 0 && (
          <p role="status" className="text-[13px] text-(--wb-ink-2)">
            {t("style.picker.none")}
          </p>
        )}
        <ul
          id={listId}
          aria-label={t("style.picker.listLabel")}
          className="flex max-h-80 flex-col gap-0.5 overflow-y-auto"
          data-testid="wb-style-picker-list"
        >
          {shown.map((app) => {
            const here = app.group === group;
            return (
              <li key={app.bundle_id}>
                <button
                  type="button"
                  disabled={here || !!busy}
                  onClick={() => void pick(app)}
                  data-testid="wb-style-picker-app"
                  data-bundle-id={app.bundle_id}
                  className={`flex w-full cursor-pointer items-center gap-3 rounded-(--wb-radius-sm) px-2.5 py-1.5 text-start text-[13px] hover:bg-(--wb-fill-hover) disabled:cursor-default disabled:opacity-60 ${FOCUS_RING}`}
                >
                  <AppIcon bundleId={app.bundle_id} name={app.name} size={24} />
                  <span className="min-w-0 flex-1 truncate">{app.name}</span>
                  <span className="shrink-0 text-[12px] text-(--wb-ink-2)">
                    {busy === app.bundle_id ? (
                      <Loader2 size={14} className="animate-spin" aria-hidden />
                    ) : (
                      t(
                        here
                          ? "style.picker.alreadyHere"
                          : "style.picker.inGroup",
                        {
                          group: t(`style.groups.${app.group}.label`),
                        },
                      )
                    )}
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
      </div>
    </Dialog>
  );
}
